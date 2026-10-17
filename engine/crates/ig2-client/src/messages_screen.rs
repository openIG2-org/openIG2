//! The messages screen (`AktKep` 10, re/screens/messages.md §3–§5): the inbox's hot-spots (`Messages_MouseInit`
//! ELF 0x5aa340) and input (`Messages_Main` 0x5a9c50), and the open message: its dialogue built from the message's
//! `msgNNNN.txt`/`txtNNNN.txt` into `ParbeszPlace` (`KiertekelUjuzenet` 0x5a8550, `KeresBlokk` 0x5a69c0,
//! `KiertekelBlokk` 0x5a6ac0, `KiirReszlet` 0x5a80b0, `TordelveAttesz` 0x5a51a0), typed out at the part's speed and
//! drawn with its colour codes and answers (`MessagesGfx2D_2` 0x5a1d30, `ValaszKivalasztva` 0x5a97a0).
//!
//! The texts are raw bytes in the game's code page and every position is a byte offset, as in the original.

use ig2_formats::Install;

use crate::font::{Align, Font, Virtual};
use crate::render::{Blend, Frame, Instance, TexId};

/// `MsgStartX` 210, `MsgStartY` 18, `MsgXBekezd` 10, `MsgYKoz` 4, `MsgFont` 2 (ELF 0x11860d8..).
const MSG: (f32, f32, f32, f32, usize) = (210.0, 18.0, 10.0, 4.0, 2);
/// `AnswX` 210, `AnswY` 430, `AnswXSz` 340, `AnswYSz` 46, `AnswFont` 2, colours `AnswSzin` 2 / `AnswSzin2` 6
/// (under the mouse) / `AnswSzin3` 1 (disabled).
const ANSW: (i32, i32, i32, i32, usize) = (210, 430, 340, 46, 2);
/// `UBAYSz` 218, `UBAXSzsz` 157, `UBAYSzsz` 176: the inbox's hot-spot; `UBAXSz` is computed at run time
/// [inf: 0, the list's left edge].
const UBA: (i32, i32, i32, i32) = (0, 218, 157, 176);
/// `SzinKarakter` "spkfzv " and `SzinTomb` (ELF 0x1184a6c / 0x1184a50): `~c` colour codes; −1 blinks 6/2.
const COLOUR_CODES: &[u8; 7] = b"spkfzv ";
const COLOUR_OF: [i32; 7] = [2, 6, 8, 3, 7, -1, 2];
/// `OSZTO` 165.0: a part's typing speed `SS` gives `ParbeszTyperPtrNovelo = SS / 165` (0.4 after the text).
const OSZTO: f32 = 165.0;

/// One answer (`Valasztasok`, 0x5c each: the label to the line's end, `+0x50` the part it prints, `+0x54` the
/// next block (−1 = the end), `+0x58` disabled by the story (`Tiltolista`)).
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub label: Vec<u8>,
    pub part: i32,
    pub next: i32,
    pub disabled: bool,
}

/// `FindNextSor` (0x5a67b0): past the line's `\r` and its `\n`, and past empty lines.
fn next_line(b: &[u8], len: i32, mut i: i32) -> i32 {
    loop {
        while i < len && b.get(i as usize) != Some(&b'\r') {
            i += 1;
        }
        i += 2;
        if !(i < len && b.get(i as usize) == Some(&b'\r')) {
            return i;
        }
    }
}

/// `mesGotoLine` (0x5a6860): the offset of line `n` (counting `\r`s).
fn goto_line(n: i32, b: &[u8], len: i32) -> i32 {
    let (mut i, mut k) = (0, 0);
    while k != n && i < len {
        if b.get(i as usize) == Some(&b'\r') {
            k += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    i
}

fn atoi(b: &[u8]) -> i32 {
    let s: String = b.iter().map(|&c| c as char).collect();
    ig2_formats::dialogue::atoi(&s)
}

/// `strxlen` (0x5a5130): the length on screen (a `~` and its code count 0).
fn xlen(b: &[u8]) -> i32 {
    b.iter().map(|&c| if c == b'~' { -1 } else { 1 }).sum()
}

/// A colour code's `DDColor` index (`~d` digit or a `SzinKarakter` letter); −1 → blinking 6/2 (`AktIdo200`).
fn code_colour(c: u8, blink: bool) -> i32 {
    let k = if c.is_ascii_digit() { (c - b'0') as usize } else { COLOUR_CODES.iter().position(|&x| x == b' ' || x == c).unwrap_or(6) };
    match COLOUR_OF.get(k).copied().unwrap_or(2) {
        -1 => if blink { 2 } else { 6 },
        c => c,
    }
}

/// A run of text on the message panel: x, y, the bytes, the `DDColor` index.
pub type Run = (f32, f32, Vec<u8>, i32);

/// An open message: `ParbeszPlace` and the typer.
#[derive(Debug, Clone)]
pub struct Dialogue {
    /// The inbox entry (`UAktUzenet`).
    pub index: usize,
    /// `#k#` parameters (the entry's `+0x10 + 0x14·k` strings).
    params: Vec<Vec<u8>>,
    msg: Vec<u8>,
    txt: Vec<u8>,
    /// `ParbeszPlace` (NUL-padded) and `ParbeszLength`, `ParbeszTyperPtr`.
    pub place: Vec<u8>,
    pub length: i32,
    pub typed: i32,
    /// `ParbeszTyperPtrNovelo` and `MsgElteltMp`.
    step: f32,
    elapsed: f32,
    /// `ParbeszKezdoSor`: the first line shown (the text scrolls past 13 lines).
    pub first_line: i32,
    pub answers: Vec<Answer>,
    /// `KiirtaMar` (all typed), `ParbeszedVege` (the dialogue is over), `ParbeszedVegeJon` (it ends with this text).
    pub typed_all: bool,
    pub over: bool,
    ends: bool,
    /// `EnVagyTe`: 1 the player speaks, 2 the other side.
    speaker: i32,
    /// `AktValasz0to3x`: the answer under the mouse.
    pub hover: Option<usize>,
}

impl Dialogue {
    /// `BetoltFajlok` (0x5a83d0): `uzenetek/msg%04d.txt` and `txt%04d.txt`, each ending in `\r\n`.
    fn file(install: &Install, name: &str) -> Vec<u8> {
        let mut b = install.read(&format!("dataraw/uzenetek/{name}")).unwrap_or_default();
        if b.last() != Some(&b'\n') {
            b.extend_from_slice(b"\r\n");
        }
        b
    }

    /// `KiertekelUjuzenet`: open inbox entry `index` (message `id`): a plain message prints its part 1 and is over;
    /// a conversation evaluates block 0 and, when its first answer prints a part, takes it at once.
    pub fn open(install: &Install, index: usize, id: i32, params: Vec<Vec<u8>>, conversation: bool, kept: bool) -> Self {
        let mut d = Self {
            index,
            params,
            msg: Self::file(install, &format!("msg{id:04}.txt")),
            txt: Self::file(install, &format!("txt{id:04}.txt")),
            place: vec![0; 0x4000],
            length: -1,
            typed: 0,
            step: 0.4,
            elapsed: 0.0,
            first_line: 0,
            answers: Vec::new(),
            typed_all: false,
            over: false,
            ends: false,
            speaker: 1,
            hover: None,
        };
        if !conversation {
            d.over = true;
            d.length = 0;
            d.print_parts(1);
        } else {
            let b = d.find_block(0);
            d.length = 0;
            d.eval_block(b);
            match d.answers.first().map(|a| (a.part, a.next)) {
                Some((-1, next)) => {
                    let b = d.find_block(next);
                    d.eval_block(b);
                }
                Some((part, next)) => {
                    d.print_parts(part);
                    let b = d.find_block(next);
                    d.eval_block(b);
                }
                None => {}
            }
            // A kept conversation is shown whole (`Messages_Belep`'s replay of `UzenetPlaybackek`) [inf: the
            // played answers aren't saved here, so the dialogue starts over].
            let _ = kept;
        }
        d
    }

    /// `KeresBlokk`: the line after the block header `%02d`, or −1.
    fn find_block(&self, n: i32) -> i32 {
        let key = format!("{n:02}").into_bytes();
        let len = self.msg.len() as i32;
        let (mut i, mut line) = (0, 0);
        while i < len {
            if self.msg[i as usize..].starts_with(&key) {
                return next_line(&self.msg, len, i);
            }
            line += 1;
            i = goto_line(line, &self.msg, len);
        }
        -1
    }

    /// `KiertekelBlokk`: the block's `- PP NN label` lines as the answers.
    fn eval_block(&mut self, mut i: i32) {
        self.answers.clear();
        let len = self.msg.len() as i32;
        while i >= 0 && i < len && self.msg[i as usize] == b'-' {
            let at = |k: i32| self.msg.get((i + k) as usize..(i + k + 2) as usize).unwrap_or(b"");
            let (part, next) = (atoi(at(2)), atoi(at(5)));
            // `mesAtteszSorvegeig`: the label from column 8 to the line's end, `#k#` replaced.
            let mut label = Vec::new();
            let mut k = (i + 8) as usize;
            while k < self.msg.len() && self.msg[k] != b'\n' {
                if self.msg[k] == b'#' && self.msg.get(k + 2) == Some(&b'#') {
                    label.extend_from_slice(self.param(self.msg[k + 1]));
                    k += 3;
                    continue;
                }
                label.push(self.msg[k]);
                k += 1;
            }
            let cut = label.iter().position(|&c| c == b'\r').unwrap_or(label.len()).min(0x50);
            label.truncate(cut);
            self.answers.push(Answer { label, part, next, disabled: false });
            i = next_line(&self.msg, len, i);
        }
    }

    fn param(&self, c: u8) -> &[u8] {
        c.checked_sub(b'0').and_then(|k| self.params.get(k as usize)).map_or(&[][..], |p| &p[..])
    }

    /// `KiirReszletek(part)`: every text part numbered `part`.
    fn print_parts(&mut self, part: i32) {
        let mut from = 0;
        while from != -1 {
            from = self.print_part(part, from);
        }
    }

    /// `KiirReszlet(part, from)` + `KeresReszlet` + `GetTxtFileCommands`: the next part `part` at or after `from`:
    /// its header `PP AAAA SS WW` sets the speed and the speaker (`|C` + 1/2 into the text), then its `+`/`-` lines
    /// are wrapped in. Returns the offset after it, or −1.
    fn print_part(&mut self, part: i32, from: i32) -> i32 {
        let key = format!("{part:02}").into_bytes();
        let len = self.txt.len() as i32;
        let mut i = from;
        let mut at = -1;
        while i < len && at == -1 {
            if self.txt[i as usize..].starts_with(&key) {
                at = i;
            } else {
                i = next_line(&self.txt, len, i);
            }
        }
        if at == -1 {
            return -1;
        }
        let field = |k: i32, n: i32| self.txt.get((at + k) as usize..(at + k + n) as usize).unwrap_or(b"").to_vec();
        let speed = atoi(&field(8, 2));
        self.speaker = if field(11, 2) == b"EN" { 1 } else { 2 };
        self.step = speed as f32 / OSZTO;
        // `PutColorCommand(ParbeszLength, EnVagyTe)`.
        let l = self.length.max(0) as usize;
        self.place[l..l + 3].copy_from_slice(&[b'|', b'C', self.speaker as u8]);
        self.length = (l + 3) as i32;
        let mut p = next_line(&self.txt, len, at);
        while p < len && matches!(self.txt[p as usize], b'-' | b'+') {
            let marker = self.txt[p as usize];
            p += if matches!(self.txt.get(p as usize + 1), Some(b'\r' | b' ')) { 1 } else { 2 };
            let used = self.wrap_in(p as usize, marker != b'-', self.speaker == 2);
            p += used;
        }
        p
    }

    /// `TordelveAttesz(src, ParbeszPlace, ParbeszLength, plus, other)`: one source line (to its `\n`, the `\r`
    /// kept, `#k#` replaced, in `~f…~s` when the other side speaks) word-wrapped at 0x36 visible characters onto
    /// the text; a `-` line first takes back the text's last line end (it flows on). Returns the source bytes used.
    fn wrap_in(&mut self, src: usize, plus: bool, other: bool) -> i32 {
        let start = self.length;
        let mut end = start;
        let mut first = true;
        if !plus {
            // Up to two line ends are taken back (the decompile's two tests; an empty `+` line before a `-` line
            // goes too [obs: decompile only]).
            for _ in 0..2 {
                if end > 0 && self.place[end as usize - 1] == b'\r' {
                    self.place[end as usize - 1] = 0;
                    end -= 1;
                    first = false;
                } else {
                    break;
                }
            }
        }
        // The current line's visible width so far.
        let (mut bytes, mut width) = (0i32, 0i32);
        let mut k = end - 1;
        while k >= 0 && self.place[k as usize] != b'\r' {
            bytes += 1;
            width += if self.place[k as usize] == b'~' { -1 } else { 1 };
            k -= 1;
        }
        if k < 0 {
            bytes = end;
        }
        if self.place.get((end - bytes).max(0) as usize) == Some(&b'|') {
            width -= 3;
        }
        // The source line with its parameters.
        let mut line = Vec::new();
        let mut i = 0usize;
        while let Some(&c) = self.txt.get(src + i) {
            if c == b'\n' {
                break;
            }
            if c == b'#' && self.txt.get(src + i + 2) == Some(&b'#') {
                let p = self.param(self.txt[src + i + 1]).to_vec();
                if p == b"#=#" {
                    line.extend_from_slice(b"(~verror~s)");
                } else {
                    if other {
                        line.extend_from_slice(b"~f");
                    }
                    line.extend(p);
                    if other {
                        line.extend_from_slice(b"~s");
                    }
                }
                i += 3;
                continue;
            }
            line.push(c);
            i += 1;
        }
        let used = i as i32 + 1;
        // The words: a word that still fits (width + its width + 1 < 0x36) follows a space, else a new line.
        let mut rest = &line[..];
        loop {
            let s = rest.iter().take_while(|&&c| c == b' ').count();
            let w = rest[s..].iter().take_while(|&&c| c != b' ').count();
            let word = rest[s..s + w].to_vec();
            let after = rest.get(s + w).copied();
            rest = &rest[s + w..];
            let put = |p: &mut Vec<u8>, at: &mut i32, b: &[u8]| {
                let a = *at as usize;
                if p.len() < a + b.len() + 1 {
                    p.resize(a + b.len() + 0x1000, 0);
                }
                p[a..a + b.len()].copy_from_slice(b);
                p[a + b.len()] = 0;
                *at += b.len() as i32;
            };
            if width + xlen(&word) + 1 < 0x36 {
                if !first {
                    put(&mut self.place, &mut end, b" ");
                    width += 1;
                } else {
                    first = false;
                }
                put(&mut self.place, &mut end, &word);
                width += xlen(&word);
            } else {
                put(&mut self.place, &mut end, b"\r");
                put(&mut self.place, &mut end, &word);
                width = xlen(&word);
            }
            if after.is_none() {
                break;
            }
        }
        let _ = bytes;
        self.length = end;
        used
    }

    /// `ValaszKivalasztva`: answer `k` prints its part and moves to its block (−1: the dialogue ends with it).
    pub fn choose(&mut self, k: usize) {
        let Some(a) = self.answers.get(k).cloned() else { return };
        if a.disabled {
            return;
        }
        self.print_parts(a.part);
        self.ends = false;
        if a.next == -1 {
            self.answers.clear();
            self.ends = true;
        } else {
            let b = self.find_block(a.next);
            self.eval_block(b);
        }
        self.typed_all = false;
    }

    /// The typer for `ms` milliseconds (`MsgElteltMp += ms/500`, a character per `ParbeszTyperPtrNovelo`); at the
    /// end `KiirtaMar`, and a conversation whose last answer led nowhere is over. A lone answer is taken as soon
    /// as the text is typed (`Melyikvalaszfolottvanazeger`).
    pub fn advance(&mut self, ms: f32) {
        self.elapsed += ms / 500.0;
        while self.step < self.elapsed {
            self.elapsed -= self.step;
            if self.typed < self.length {
                self.typed += 1;
                self.typed_all = false;
            } else {
                self.typed_all = true;
                if self.ends {
                    self.over = true;
                }
                self.step = 0.4;
            }
        }
        if self.typed_all && !self.over && self.answers.len() == 1 {
            self.choose(0);
        }
    }

    /// A left click in the text (x 186..512, y 1..167): type the rest at once (`ParbeszTyperPtrNovelo` = 0.001).
    pub fn hurry(&mut self) {
        self.step = 0.001;
    }

    /// The drawing pass of `MessagesGfx2D_2` (`ParbeszLength != −1`, `mesframe` 30): from `ParbeszKezdoSor`,
    /// typed bytes up to `ParbeszTyperPtr`, line `MsgY` at `y = 22 + 11·MsgY`, char `MsgX` at `x = 220 + 6·MsgX`;
    /// `|C`n sets the colour n, `~c` a colour code; past line 12 the text scrolls on by a line. Returns the runs
    /// (x, y, bytes, colour) and the typing cursor's cell.
    pub fn layout(&mut self, blink: bool) -> (Vec<Run>, [f32; 2]) {
        let (x0, y0, indent, gap, size) = MSG;
        let (cw, ch) = (6.0, 7.0);
        let _ = size;
        let mut runs = Vec::new();
        let place = &self.place;
        let len = self.length;
        let mut colour = 6;
        let mut ptr = 0;
        if self.first_line != 0 {
            colour = self.scan_colour(self.first_line, blink);
            ptr = goto_line(self.first_line, place, len) - 1;
        }
        let (mut mx, mut my) = (0i32, 0i32);
        let mut scroll = 0;
        let at = |mx: i32, my: i32| (x0 + indent + mx as f32 * cw, y0 + gap + my as f32 * (gap + ch));
        while my < 13 && ptr < self.typed && ptr < len {
            if place[ptr as usize] == b'|' {
                if place.get(ptr as usize + 1) == Some(&b'C') {
                    colour = place.get(ptr as usize + 2).copied().unwrap_or(2) as i8 as i32;
                }
                ptr += 3;
            }
            while place.get(ptr as usize) == Some(&b'~') && place.get(ptr as usize + 1).is_some_and(|&c| c != 0) {
                colour = code_colour(place[ptr as usize + 1], blink);
                ptr += 2;
            }
            // The run to the line's end, the text's end or the typer.
            let mut n = 0;
            while place.get((ptr + n) as usize).is_some_and(|&c| c != b'\r') && ptr + n < len && ptr + n < self.typed {
                n += 1;
            }
            if ptr + n == len || ptr + n == self.typed {
                n = -1;
            }
            while place.get(ptr as usize) == Some(&b'\r') {
                if place.get(ptr as usize + 1) == Some(&b'\n') {
                    ptr += 1;
                }
                ptr += 1;
                my += 1;
                mx = 0;
                if my > 12 {
                    scroll += 1;
                }
            }
            if n > 0 {
                let mut k = 0;
                while k < n {
                    let mut w = 0;
                    let mut code = false;
                    while place.get((ptr + w + k) as usize) != Some(&b'~') && k + w < n {
                        if place.get((ptr + w + k + 1) as usize) == Some(&b'~') {
                            code = true;
                        }
                        w += 1;
                    }
                    if w == 0 {
                        code = true;
                    } else {
                        let (x, y) = at(mx, my);
                        runs.push((x, y, place[(ptr + k) as usize..(ptr + k + w) as usize].to_vec(), colour));
                        k += w;
                        mx += w;
                    }
                    if code {
                        k += 2;
                        colour = code_colour(place.get((ptr + k - 1) as usize).copied().unwrap_or(b's'), blink);
                    }
                }
                ptr += n;
            }
            if n == -1 {
                let (x, y) = at(mx, my);
                runs.push((x, y, vec![place.get(ptr as usize).copied().unwrap_or(b' ')], colour));
                ptr += 1;
                mx += 1;
            }
        }
        self.first_line += scroll;
        let (x, y) = at(mx, my.min(12));
        (runs, [x, y])
    }

    /// `ScanLastParbeszedColor(line)`: the colour in force at the start of line `line`.
    fn scan_colour(&self, line: i32, blink: bool) -> i32 {
        let mut c = 1;
        for l in 0..line {
            let mut i = goto_line(l, &self.place, self.length) as usize;
            while i < self.place.len() && self.place[i] != b'\r' {
                if self.place[i] == b'|' {
                    c = self.place.get(i + 2).copied().unwrap_or(1) as i32;
                }
                if self.place[i] == b'~' {
                    c = code_colour(self.place.get(i + 1).copied().unwrap_or(b's'), blink);
                }
                i += 1;
            }
        }
        c
    }
}

/// The screen's UI state.
#[derive(Debug, Clone, Default)]
pub struct MessagesScreen {
    /// `AktUzenetPointer`.
    pub first: usize,
    pub open: Option<Dialogue>,
    /// `LenyomasValtozo`: the arrows take a press only after 5 UI frames.
    pub debounce: u32,
    /// Inbox entries opened here (their `+4` read flag is the sim's; not sent yet).
    pub read: Vec<usize>,
}

/// `Messages_MouseInit`'s hot-spots (virtual pixels): 1 up (older first … the list moves), 2 down, 3 the inbox,
/// 4 the answers, 5 the map, 6 the text, 7..=10 leave the screen (10 is the whole screen).
pub const SPOTS: [(u32, [i32; 4]); 10] = [
    (1, [0xa0, 0xbd, 0xd, 0x13]),
    (2, [0xa0, 400, 0xd, 0x13]),
    (3, [UBA.0, UBA.1, UBA.2, UBA.3]),
    (4, [ANSW.0, ANSW.1, ANSW.2, ANSW.3]),
    (5, [0xc9, 0xcc, 0x14e, 0xbc]),
    (6, [MSG.0 as i32, MSG.1 as i32, 0x14d, 0x9c]),
    (7, [0, 0, 0xc3, 0xb4]),
    (8, [0x24a, 0, 0x36, 0x188]),
    (9, [0, 0x19a, 0x8f, 0x46]),
    (10, [0, 0, 0x27f, 0x1df]),
];

pub fn spot_at(at: [f32; 2]) -> Option<u32> {
    SPOTS.iter().find(|(_, [x, y, w, h])| at[0] >= *x as f32 && at[0] < (x + w) as f32 && at[1] >= *y as f32 && at[1] < (y + h) as f32).map(|(id, _)| *id)
}

/// What a press asks of the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    None,
    /// Open inbox entry `index`.
    Open(usize),
    /// Back to the screen before (`MessagesElottiScreen`).
    Leave,
}

impl MessagesScreen {
    pub fn ui_frame(&mut self) {
        if self.debounce < 5 {
            self.debounce += 1;
        }
    }

    /// The mouse over the answers (`Melyikvalaszfolottvanazeger`): quarters of `AnswYSz` from `AnswY`.
    pub fn hover(&mut self, at: [f32; 2]) {
        if let Some(d) = &mut self.open {
            d.hover = None;
            let (x, y, w, h, _) = ANSW;
            let (mx, my) = (at[0] as i32, at[1] as i32);
            if (x..x + w).contains(&mx) && my >= y {
                let r = (my - y).max(0);
                let k = if r <= h / 4 { 0 } else if r <= h / 2 { 1 } else if r <= h / 4 * 3 { 2 } else { 3 };
                d.hover = (k < d.answers.len()).then_some(k);
            }
        }
    }

    /// A left press (`Messages_Main`) on inbox of `n` messages.
    pub fn press(&mut self, at: [f32; 2], n: usize) -> Press {
        let id = spot_at(at);
        let busy = self.open.as_ref().is_some_and(|d| !d.over);
        match id {
            Some(2) if self.debounce == 5 => {
                self.debounce = 0;
                self.first = self.first.saturating_sub(1);
            }
            Some(1) if self.debounce == 5 => {
                self.debounce = 0;
                if self.first + 4 < n {
                    self.first += 1;
                }
            }
            Some(3) if !busy => {
                // `AktUzenet0to3` = 3 − the row (quarters of `UBAYSzsz`), only rows that hold a message.
                let r = at[1] as i32 - UBA.1;
                let q = UBA.3 / 4;
                let k = if r <= q { 3 } else if r <= 2 * q { 2 } else if r <= 3 * q { 1 } else { 0 };
                if k < n {
                    return Press::Open(self.first + k);
                }
            }
            Some(4) => {
                if let Some(d) = &mut self.open {
                    if d.typed_all {
                        if let Some(k) = d.hover {
                            d.choose(k);
                        }
                    }
                }
            }
            Some(5 | 6) => {
                if let Some(d) = &mut self.open {
                    if (0xba..0x201).contains(&(at[0] as i32)) && (1..0xa8).contains(&(at[1] as i32)) {
                        d.hurry();
                    }
                }
            }
            Some(7..=10) => return Press::Leave,
            _ => {}
        }
        Press::None
    }
}

/// The open message's text, cursor, answers and the inbox's frame (`MessagesGfx2D_2`).
#[allow(clippy::too_many_arguments)]
pub fn draw(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], ui: &mut MessagesScreen, n: usize, time: f32) {
    let col = |i: i32| palette.get(i.max(0) as usize).copied().unwrap_or([1.0; 3]);
    let blink = (time * 1000.0 / 200.0) as i64 % 2 == 1;
    let first = ui.first;
    let Some(d) = &mut ui.open else { return };
    // The open message's row: `Keret(UBAXSz + 1, UBAYSz + slot·45 − 1, 155, 43, 0x26)` while it types or talks.
    for (i, slot) in crate::ui::inbox_rows(n, first) {
        if i == d.index && (!d.typed_all || !d.over) {
            let (x, y, w, h) = ((UBA.0 + 1) as f32, (UBA.1 + slot as i32 * 45 - 1) as f32, (UBA.2 - 2) as f32, 43.0);
            for (rx, ry, rw, rh) in [(x + 1.0, y, w - 2.0, 1.0), (x + 1.0, y + h - 1.0, w - 2.0, 1.0), (x, y + 1.0, 1.0, h - 2.0), (x + w - 1.0, y + 1.0, 1.0, h - 2.0)] {
                rect(f, white, v, rx, ry, rw, rh, col(0x26));
            }
        }
    }
    let (runs, cursor) = d.layout(blink);
    for (x, y, t, c) in runs {
        font.text(f, v, &t, x, y, t.len(), MSG.4, Align::Left, col(c), None);
    }
    // The answers once typed (and the message not read before): `AnswX + 10`, `AnswY + 4 + k·11`.
    if d.typed_all && !d.over {
        for (k, a) in d.answers.iter().enumerate() {
            if a.part == -1 {
                continue;
            }
            let c = if a.disabled { 1 } else if d.hover == Some(k) { 6 } else { 2 };
            let y = (ANSW.1 + 4 + k as i32 * (4 + 7)) as f32;
            font.text(f, v, &a.label, (ANSW.0 + 10) as f32, y, a.label.len(), ANSW.4, Align::Left, col(c), None);
        }
    }
    // The typing cursor: a cell in colour 8 every other 200 ms while typing or waiting for an answer.
    if (!d.answers.is_empty() || !d.typed_all) && blink {
        rect(f, white, v, cursor[0], cursor[1], 6.0, 7.0, col(8));
    }
}

#[allow(clippy::too_many_arguments)]
fn rect(f: &mut Frame, white: TexId, v: &Virtual, x: f32, y: f32, w: f32, h: f32, c: [f32; 3]) {
    use crate::render::kind;
    let p = v.px(x + w / 2.0, y + h / 2.0);
    f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, p, [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialogue(msg: &[u8], txt: &[u8]) -> Dialogue {
        Dialogue {
            index: 0,
            params: vec![b"Earth".to_vec()],
            msg: msg.to_vec(),
            txt: txt.to_vec(),
            place: vec![0; 0x400],
            length: 0,
            typed: 0,
            step: 0.4,
            elapsed: 0.0,
            first_line: 0,
            answers: Vec::new(),
            typed_all: false,
            over: false,
            ends: false,
            speaker: 1,
            hover: None,
        }
    }

    #[test]
    fn parts_wrap_flow_and_break_like_tordelve_attesz() {
        let txt = b"01 0001 12 TE\r\n+ ~zFrom #0#~s\r\n+ \r\n- one two\r\n- three\r\n02 0001 12 EN\r\n+ yes\r\n";
        let mut d = dialogue(b"", txt);
        d.print_parts(1);
        let s = &d.place[..d.length as usize];
        // `|C` 2, the title line; the `-` lines take back the empty line's and the title's line ends and flow on.
        assert_eq!(s, b"|C\x02~zFrom ~fEarth~s~s one two three\r".as_slice());
        assert_eq!(d.step, 12.0 / 165.0);
    }

    #[test]
    fn long_lines_break_at_54_visible_characters() {
        let word = b"abcdefghij ";
        let mut line = b"- ".to_vec();
        for _ in 0..8 {
            line.extend_from_slice(word);
        }
        line.extend_from_slice(b"\r\n");
        let mut txt = b"01 0001 12 EN\r\n".to_vec();
        txt.extend(line);
        let mut d = dialogue(b"", &txt);
        d.print_parts(1);
        let s = &d.place[3..d.length as usize];
        let first = s.iter().position(|&c| c == b'\r').unwrap();
        // 4 words + 3 spaces = 43; a fifth would make 54, which isn't < 0x36.
        assert_eq!(first, 43);
    }

    #[test]
    fn blocks_give_answers_and_the_dialogue_ends() {
        let msg = b"00\r\n- 01 02 -----\r\n02\r\n- 02 -1 Fine.\r\n- 03 -1 No.\r\n";
        let txt = b"01 0001 12 TE\r\n+ Hello\r\n02 0001 12 EN\r\n+ Fine\r\n03 0001 12 EN\r\n+ No\r\n";
        let mut d = dialogue(msg, txt);
        let b = d.find_block(0);
        d.eval_block(b);
        assert_eq!(d.answers[0], Answer { label: b"-----".to_vec(), part: 1, next: 2, disabled: false });
        d.choose(0);
        assert_eq!(d.answers.len(), 2);
        assert_eq!(d.answers[1].label, b"No.");
        d.choose(1);
        assert!(d.answers.is_empty());
        d.advance(10_000.0);
        assert!(d.typed_all && d.over);
        assert!(d.place[..d.length as usize].ends_with(b"No\r"));
    }

    #[test]
    fn inbox_rows_and_arrows_follow_messages_main() {
        let mut s = MessagesScreen { debounce: 5, ..Default::default() };
        // Row 0 (y 218..262) is entry first + 3.
        assert_eq!(s.press([50.0, 230.0], 6), Press::Open(3));
        assert_eq!(s.press([50.0, 380.0], 6), Press::Open(0));
        assert_eq!(s.press([50.0, 230.0], 2), Press::None);
        s.press([165.0, 195.0], 6);
        assert_eq!(s.first, 1);
        assert_eq!(s.press([600.0, 100.0], 6), Press::Leave);
    }
}
