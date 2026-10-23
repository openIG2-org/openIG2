//! The Help screen (`AktKep` 1, re/screens/help.md): the topic list of `help/helpmain.txt`, the topic's text wrapped
//! to 69 columns with its fonts, colours and pictures, the scroll track, the exit button. Its state is the exe's
//! globals (`helpframe`, `TopicSel`, `SubTopicSel/Pos`, `TopicActSorPtr`, `BarOldH/Y`, `FSzor`, `HelpTopicSor`), its
//! input `Help_Main` (exe 0x4d7510) over `Help_MouseInit`'s hot-spots (exe 0x4d7640) plus the list clicks of
//! `HelpGfx2D_2` (exe 0x4d5f00), its clock `FRISSIT_Help2` (exe 0x4d5150) and its draw `GFX_Help` (exe 0x4d5540),
//! `HelpGfx2D_1` (0x4d5b80) and `HelpGfx2D_2`.

use std::collections::HashMap;

use crate::font::{Align, Font};
use crate::kit::{fader, fader_design, inside, FaderDesign, Lasers, Tooltip, Virtual};
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};
use crate::ui::UiTexts;

/// `HELP_WIDTH` (exe 0x90af70).
const WIDTH: i32 = 69;
/// `TopicBox`, `TextBox`, `HelpX1/Y1/X2/Y2Vagas` (exe 0x90afac..0x90afd8).
const TOPIC_BOX: [i32; 4] = [19, 47, 131, 383];
const TEXT_BOX: [i32; 4] = [170, 46, 426, 385];
const CLIP: [i32; 4] = [178, 48, 596, 429];
/// `TopicKezdX/Y`, `TopicTav` (= `SubTopicTav`), `SubTopicKezdX`.
const TOPIC_X: i32 = 30;
const TOPIC_Y: i32 = 60;
const TOPIC_STEP: i32 = 15;
const SUB_X: i32 = 38;
/// `TopicTextKezdY`, `TopicMaxLines`, `SorKoz`, `Barky`.
const TEXT_Y: i32 = 55;
const TEXT_END: i32 = 415;
const GAP: i32 = 3;
const BARKY: i32 = 47;
/// `Help_MouseInit`'s hot-spots 1..=6 (1 and 2 are switched off).
const SPOTS: [[i32; 4]; 6] = [[610, 45, 27, 23], [610, 409, 27, 23], [17, 44, 139, 358], [610, 45, 22, 387], [171, 46, 426, 382], [24, 401, 122, 25]];
const LASER: [i32; 3] = [0x4c, 0x4c, 0x4c];

/// A `<TOPICname,file>` of `helpmain.txt` with its `<SUBTOPICname>`s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub name: Vec<u8>,
    pub file: Vec<u8>,
    pub subs: Vec<Vec<u8>>,
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    (from..b.len()).find(|&i| b[i..].starts_with(pat))
}

/// `Help_Belep`'s parse of `helpmain.txt`: at `<TOPIC` the name up to the comma (its first byte taken as it is), the
/// file up to `>`; every `<SUBTOPIC` before the next `<T` adds a name up to `>` (at most 20).
pub fn parse_index(b: &[u8]) -> Vec<Topic> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(t) = find(b, i, b"<TOPIC") {
        let s = t + 6;
        let comma = (s + 1..b.len()).find(|&k| b[k] == b',').unwrap_or(b.len());
        let name = b[s..comma].to_vec();
        let gt = (comma..b.len()).find(|&k| b[k] == b'>').unwrap_or(b.len());
        let file = b.get(comma + 1..gt).unwrap_or_default().to_vec();
        let end = find(b, gt, b"<T").unwrap_or(b.len());
        let mut subs = Vec::new();
        let mut k = gt;
        while let Some(st) = find(&b[..end], k, b"<SUBTOPIC") {
            let e = (st + 9..end).find(|&q| b[q] == b'>').unwrap_or(end);
            if subs.len() < 20 {
                subs.push(b[st + 9..e].to_vec());
            }
            k = e;
        }
        out.push(Topic { name, file, subs });
        i = end;
    }
    out
}

/// `hstrxlen`: +1 a byte, −1 for `~`.
fn xlen(w: &[u8]) -> i32 {
    w.iter().map(|&c| if c == b'~' { -1 } else { 1 }).sum()
}

/// `hTordelveAttesz(line, out, o, plus)` (ELF 0x54c1e0): the line's words onto `out`, a new line before a word that
/// would pass column 69; a `-` line (`plus` false) first drops the `\r`s that end `out` and goes on with a space.
fn wrap_line(src: &[u8], out: &mut Vec<u8>, plus: bool) {
    let mut first = true;
    if !plus {
        let c = out.last().copied();
        if c == Some(b'\r') {
            out.pop();
        }
        first = c != Some(b'\r');
        if out.last() == Some(&b'\r') {
            out.pop();
            first = false;
        }
    }
    let mut width = 0;
    for &c in out.iter().rev().take_while(|&&c| c != b'\r') {
        width += if c == b'<' || c == b'>' { -1 } else { 1 };
    }
    let line = &src[..src.iter().position(|&c| c == b'\n').unwrap_or(src.len())];
    let line = &line[..line.iter().position(|&c| c == 0).unwrap_or(line.len())];
    let mut rest = line;
    loop {
        let skip = rest.iter().take_while(|&&c| c == b' ').count();
        rest = &rest[skip..];
        let n = rest.iter().position(|&c| c == b' ').unwrap_or(rest.len());
        let (word, more) = (&rest[..n], n < rest.len());
        rest = &rest[n..];
        let w = xlen(word);
        if width + w + 1 > WIDTH {
            out.push(b'\r');
            out.extend_from_slice(word);
            width = w;
        } else {
            if !first {
                out.push(b' ');
                width += 1;
            } else {
                first = false;
            }
            out.extend_from_slice(word);
            width += w;
        }
        if !more {
            break;
        }
    }
}

/// `hLoadFileEsTordel` (ELF 0x54c7e0, exe 0x4d7980): a topic file to the screen's text, lines ending in a lone `\r`.
/// The parse runs while a line starts with `-`, `+` or `*`.
pub fn wrap(file: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = file.get(i) {
        if !(c == b'-' || c == b'+' || c == b'*') {
            break;
        }
        let lf = file[i..].iter().position(|&b| b == b'\n').unwrap_or(file.len() - i);
        if file.get(i + 1) == Some(&b'\r') {
            out.push(b'\r');
            i += 3;
            continue;
        }
        let body = file.get(i + 2..).unwrap_or_default();
        if c == b'*' {
            out.extend_from_slice(&body[..body.iter().position(|&b| b == b'\n').unwrap_or(body.len())]);
        } else {
            wrap_line(body, &mut out, c == b'+');
        }
        i += lf + 1;
    }
    out
}

/// A `HelpTopicSor` entry (stride 0x14): colour, font, offset, has a picture, height.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Row {
    pub col: i32,
    pub font: i32,
    pub start: usize,
    pub pic: bool,
    pub height: i32,
}

/// One topic's text and its line table (`HelpTopicPlace`, `GetNumberOfLines`, `HelpPregeneralo`).
#[derive(Debug, Clone, Default)]
pub struct Text {
    pub buf: Vec<u8>,
    /// `N`: the `\r`s + 1.
    pub n: usize,
    /// `HelpKepNevek`: the `<P…>` names in order (kep 0x23 + i).
    pub pics: Vec<Vec<u8>>,
}

impl Text {
    pub fn new(file: &[u8]) -> Self {
        let buf = wrap(file);
        let n = buf.iter().filter(|&&c| c == b'\r').count() + 1;
        let mut pics = Vec::new();
        let mut i = 0;
        while i < buf.len() {
            if buf[i] == b'<' && buf.get(i + 1) == Some(&b'P') {
                let s = i + 2;
                let e = (s..buf.len()).find(|&k| buf[k] == b'>').unwrap_or(buf.len());
                pics.push(buf[s..e].to_vec());
                i = e;
            }
            i += 1;
        }
        Self { buf, n, pics }
    }

    fn at(&self, i: usize) -> u8 {
        self.buf.get(i).copied().unwrap_or(0)
    }

    /// `hlpFindNextLine`.
    fn next_line(&self, p: usize) -> usize {
        let mut k = p;
        while self.at(k) != b'\r' && k < self.buf.len() {
            k += 1;
        }
        k + 1
    }

    /// `ScanLastCol` / `ScanLastFnt`: the last `<Xd>` / `<Xdd>` before `p`, from 1.
    fn scan_last(&self, tag: u8, p: usize) -> i32 {
        let d = |c: u8| c as i32 - b'0' as i32;
        let mut v = 1;
        let mut i = 0;
        while i < p {
            if self.at(i) == b'<' && self.at(i + 1) == tag {
                v = d(self.at(i + 2));
                if self.at(i + 3) != b'>' {
                    v = v * 10 + d(self.at(i + 3));
                    i += 1;
                }
                i += 3;
            }
            i += 1;
        }
        v
    }

    /// `HelpPregeneralo` into the screen's table, which keeps what earlier topics left beyond (entry 0's height,
    /// the picture flags: never cleared).
    pub fn pregen(&self, table: &mut Vec<Row>, char_y: impl Fn(i32) -> i32) {
        if table.len() < self.n {
            table.resize(self.n, Row::default());
        }
        table[0].col = 1;
        table[0].font = 1;
        table[0].start = 0;
        let mut p = 0;
        for row in table.iter_mut().take(self.n).skip(1) {
            p = self.next_line(p);
            row.col = self.scan_last(b'C', p);
            row.font = self.scan_last(b'F', p);
            row.start = p;
            row.height = char_y(row.font) + GAP;
        }
        for row in table.iter_mut().take(self.n) {
            let mut k = row.start;
            while self.at(k) != b'\r' && k < self.buf.len() {
                if self.at(k) == b'<' && self.at(k + 1) == b'P' {
                    row.pic = true;
                    break;
                }
                k += 1;
            }
        }
    }

    /// `FindSubTopic(k)`: the line of the `k`-th `<SUBTOPIC>`, or −1.
    pub fn find_sub(&self, table: &[Row], k: i32) -> i32 {
        let mut found = -1;
        let mut line = 0;
        while line != self.n && found != k {
            let mut p = table.get(line).map_or(0, |r| r.start);
            while self.at(p) != b'\r' && found != k && p < self.buf.len() {
                if self.buf[p..].starts_with(b"<SUBTOPIC>") {
                    p += 9;
                    found += 1;
                }
                p += 1;
            }
            line += 1;
        }
        if found == k {
            line as i32 - 1
        } else {
            -1
        }
    }
}

/// `GetRemainingPixels(i)`: the heights from line `i` on.
fn remaining(table: &[Row], n: usize, i: i32) -> i32 {
    (i.max(0) as usize..n).map(|k| table.get(k).map_or(0, |r| r.height)).sum()
}

/// What the screen asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Back to `HelpElottiScreen`.
    Back,
}

/// The pictures of the shown topic, loaded on demand (`HelpKikeresKepek`: `../help/<name>` → `help/<name>_bmp.pvr`).
#[derive(Default)]
pub struct Pictures {
    map: HashMap<Vec<u8>, Option<(TexId, [f32; 2])>>,
}

impl Pictures {
    pub fn load(&mut self, r: &mut Renderer, install: &ig2_formats::Install, names: &[Vec<u8>]) {
        for n in names {
            self.map.entry(n.clone()).or_insert_with(|| {
                let file = String::from_utf8_lossy(n).to_ascii_lowercase().replace('.', "_");
                let img = crate::assets::Image::load(install, &format!("dataraw/help/{file}.pvr")).ok()?;
                Some((r.add_texture(&img), [img.width as f32, img.height as f32]))
            });
        }
    }

    fn get(&self, n: &[u8]) -> Option<(TexId, [f32; 2])> {
        self.map.get(n).copied().flatten()
    }
}

pub struct Draw<'a> {
    pub font: &'a Font,
    pub white: TexId,
    pub glow: TexId,
    pub v: Virtual,
    pub palette: &'a [[f32; 3]],
    pub texts: &'a UiTexts,
    pub pics: &'a Pictures,
    pub now_ms: f64,
}

/// What survives between visits: `OldTopicSel` and the line table (`HelpTopicSor` is never cleared).
#[derive(Debug, Clone, Default)]
pub struct Memory {
    pub topic: usize,
    pub table: Vec<Row>,
}

pub struct HelpScreen {
    pub topics: Vec<Topic>,
    files: Vec<Vec<u8>>,
    pub sel: usize,
    pub sub_sel: i32,
    pub sub_pos: i32,
    /// `TopicActSorPtr`.
    pub top: i32,
    pub text: Text,
    table: Vec<Row>,
    /// `helpframe` 0..100, `hlpELINDULT`, `hlpKINYITVA`, `hlpBECSUKAS`, `hlpKELLKINYITAS`.
    pub frame: f32,
    started: bool,
    open: bool,
    closing: bool,
    animated: bool,
    zero_frames: u32,
    /// `BarOldH`, `BarOldY`, `FSzor`, `HelpCsuszkaMode`.
    bar_h: i32,
    bar_y: i32,
    fszor: f32,
    drag: bool,
    /// `TopicLastSorOnScreen`, `KepernyoAljan`.
    shown: i32,
    pub bottom: bool,
    faders: FaderDesign,
    lasers: Lasers,
    pub tip: Tooltip,
    /// The pointer and this frame's button state (`Mouse_x/y`, `Mouse_Bal`, `Mouse_Lenyom`, `Mouse_Felenged`,
    /// `Mouse_Megfogva`).
    pub at: [f32; 2],
    held: bool,
    pressed: bool,
    released: bool,
    grabbed: bool,
    act: Option<Act>,
}

impl HelpScreen {
    /// `Help_Belep` (exe 0x4d7060): `index` = `helpmain.txt`, `read` the topic files; `animated` unless coming back
    /// from Help itself.
    pub fn enter(index: &[u8], read: impl Fn(&[u8]) -> Vec<u8>, mem: &Memory, animated: bool, char_y: impl Fn(i32) -> i32) -> Self {
        let topics = parse_index(index);
        let files: Vec<Vec<u8>> = topics.iter().map(|t| read(&t.file)).collect();
        let sel = mem.topic.min(topics.len().saturating_sub(1));
        let mut s = Self {
            topics,
            files,
            sel,
            sub_sel: -1,
            sub_pos: -1,
            top: 0,
            text: Text::default(),
            table: mem.table.clone(),
            frame: if animated { 0.0 } else { 100.0 },
            started: false,
            open: !animated,
            closing: false,
            animated,
            zero_frames: 0,
            bar_h: 10,
            bar_y: 0,
            fszor: 0.0,
            drag: false,
            shown: 0,
            bottom: false,
            faders: FaderDesign::new(1),
            lasers: Lasers::default(),
            tip: Tooltip::default(),
            at: [320.0, 240.0],
            held: false,
            pressed: false,
            released: false,
            grabbed: false,
            act: None,
        };
        s.load_topic(sel, &char_y);
        s
    }

    /// `TopicBetolto` + `GetNumberOfLines` + `TopicActSorPtr = 0` + `HelpPregeneralo`.
    fn load_topic(&mut self, i: usize, char_y: &impl Fn(i32) -> i32) {
        self.sel = i;
        self.text = Text::new(self.files.get(i).map_or(&[][..], |f| &f[..]));
        self.top = 0;
        self.text.pregen(&mut self.table, char_y);
    }

    pub fn memory(&self) -> Memory {
        Memory { topic: self.sel, table: self.table.clone() }
    }

    /// The hot-spot under `at` (1..=6, the lowest id first; 1 and 2 are off), 0 for none.
    pub fn spot_at(&self, at: [f32; 2]) -> u32 {
        (3..=6).find(|&id| inside(SPOTS[id as usize - 1], at)).unwrap_or(0)
    }

    pub fn hover(&mut self, at: [f32; 2]) {
        self.at = at;
        self.tip.moved(at);
    }

    pub fn press(&mut self, at: [f32; 2]) {
        self.hover(at);
        self.held = true;
        self.pressed = true;
    }

    pub fn release(&mut self) {
        self.held = false;
        self.released = true;
    }

    /// Esc: `MainCallBackEventHandler` makes it the exit hot-spot (`AktIkon 0x24`).
    pub fn escape(&mut self) {
        self.leave();
    }

    /// `AktKep = HelpElottiScreen`: `Help_Kilep` closes the screen first when it opened animated.
    fn leave(&mut self) {
        if self.closing || self.act.is_some() {
            return;
        }
        if self.animated {
            self.closing = true;
        } else {
            self.act = Some(Act::Back);
        }
    }

    /// One 10 ms UI tick (`FRISSIT_Help2`, `UpdateSparks`).
    pub fn tick(&mut self) {
        self.faders.tick();
        self.lasers.tick();
        if self.animated && !self.closing && self.started && !self.open {
            self.frame += 2.0;
            if self.frame > 100.0 {
                self.open = true;
                self.frame = 100.0;
            }
        }
        if self.animated && self.closing {
            self.frame -= 2.6;
            if self.frame < 0.0 {
                self.frame = 0.0;
            }
        }
    }

    /// The frame's end: closing, the screen is gone once `helpframe` has been 0 on two passes.
    pub fn done(&mut self) -> Option<Act> {
        if self.closing && self.frame == 0.0 {
            self.zero_frames += 1;
            if self.zero_frames >= 2 {
                self.closing = false;
                self.act = Some(Act::Back);
            }
        }
        self.act.take()
    }

    /// `Help_Main` (exe 0x4d7510), once a frame before the draw: the scroll track's drag and the exit button.
    /// `AktIkon` names the hot-spot while a button is down and on the frame it comes up [obs: probes/help.js], and
    /// `HelpCsuszkaMode` ends only in `GFX_Help` after this: the release frame still drags, at the release point.
    fn main(&mut self) {
        let spot = self.spot_at(self.at);
        if (self.held || self.released) && (spot == 4 || self.grabbed) && !self.closing {
            self.grabbed = true;
            if self.pressed {
                self.drag = true;
            }
            self.sub_sel = -1;
            if self.drag {
                let n = self.text.n as i32;
                if n < 37 {
                    self.top = 0;
                } else {
                    let h = 13212 / n;
                    let d = 367 - h;
                    let m = n - 36;
                    self.top = ((((self.at[1] as i32 - 66) - h / 2) * m + d / 2) / d).clamp(0, m);
                }
            }
        }
        if self.pressed && spot == 6 {
            self.leave();
        }
    }

    pub fn tip_text(&self, texts: &UiTexts, id: u32) -> Vec<u8> {
        if id == 0 {
            return Vec::new();
        }
        texts.get(1, 1, id as u16)
    }

    /// One drawn frame: `Help_Main`, then `GFX_Help` with the passes, then the tooltip.
    pub fn draw(&mut self, f: &mut Frame, d: &Draw, char_y: impl Fn(i32) -> i32) {
        self.main();
        self.started = true;
        if self.released {
            self.drag = false;
        }
        let fr = self.frame / 100.0;
        let g = 1.0 - fr;
        self.draw_faders(f, d, g);
        self.draw_texts(f, d, fr, &char_y);
        // The hover glows over the text and topic boxes (`FadeKerettelKerekitDesign`), not while closing.
        if !self.closing {
            let blink = fader_design::blink(d.now_ms);
            for (i, r) in [(0usize, TEXT_BOX), (1, TOPIC_BOX)] {
                let s = self.faders.s(i);
                let c = [0, (s * 54.0) as i32, (s * 90.0) as i32];
                let t = (self.faders.clock() as f64).rem_euclid(1.0) as f32;
                fader_design::glow(f, d.white, &d.v, r, c, t, 0, blink);
            }
            for (i, r) in [(0usize, TEXT_BOX), (1, TOPIC_BOX)] {
                if let Some((t, style)) = self.faders.glow(i) {
                    let s = self.faders.s(i);
                    fader_design::glow(f, d.white, &d.v, r, [0, (s * 54.0) as i32, (s * 90.0) as i32], t, style, blink);
                }
            }
        }
        let under = if self.held { 0 } else { self.spot_at(self.tip.mouse) };
        let text = self.tip_text(d.texts, under);
        self.tip.update(d.now_ms, if text.is_empty() { 0 } else { under }, text.len(), self.held);
        let shown = self.tip_text(d.texts, self.tip.shown);
        self.tip.draw(f, d.font, d.white, &d.v, d.palette, &shown);
        if self.released {
            self.grabbed = false;
        }
        self.pressed = false;
        self.released = false;
    }

    /// `HelpGfx2D_1`: the four faders, then `HelpPutCsuszkaFader`.
    fn draw_faders(&mut self, f: &mut Frame, d: &Draw, g: f32) {
        let free = (!self.held && !self.released) || self.pressed;
        let boxes: [(usize, [f32; 4], bool); 4] = [
            (0, TEXT_BOX.map(|q| q as f32), true),
            (1, [612.0, (BARKY - 1) as f32, 16.0, 386.0], (1.0 - g) as f64 > 0.2),
            (2, TOPIC_BOX.map(|q| q as f32), true),
            (3, [24.0, 401.0, 122.0, 25.0], true),
        ];
        for (id, r, on) in boxes {
            if !on {
                continue;
            }
            let k = self.faders.s(id);
            let fill = [0.0, (k * 54.0).trunc(), (k * 90.0).trunc()];
            let edge = [0.0, (k * 90.0).trunc(), (k * 130.0).trunc()];
            fader::kirak_fader(f, d.white, &d.v, r, fill, (g * 155.0 + 100.0).trunc(), edge, (g * 225.0 + 30.0).trunc(), self.faders.offset(id));
            self.faders.enter(id, r, self.at, free);
        }
        // `HelpPutCsuszkaFader`: the thumb runs towards its place once a drawn frame.
        let n = self.text.n as i32;
        let (th, ty) = if n <= 36 { (387, 0) } else { let th = 13932 / n; (th, self.top * (387 - th) / (n - 36)) };
        if self.bar_h < th {
            self.bar_h = (self.bar_h + 20).min(th);
        } else if th < self.bar_h {
            self.bar_h = (self.bar_h - 20).max(th);
        }
        let step = 15 + 120 * self.drag as i32;
        if self.bar_y < ty {
            self.bar_y = (self.bar_y + step).min(ty);
        } else if ty < self.bar_y {
            self.bar_y = (self.bar_y - step).max(ty);
        }
        if self.drag {
            if self.fszor < 1.0 {
                self.fszor = ((self.fszor as f64 + 0.1) as f32).min(1.0);
            }
        } else if self.fszor > 0.0 {
            self.fszor = ((self.fszor as f64 - 0.1) as f32).max(0.0);
        }
        let h = if BARKY + self.bar_y + self.bar_h - 4 > 453 { 457 - BARKY - self.bar_y } else { self.bar_h };
        let fz = self.fszor;
        let ci = |a: f32| (fz * 12.0 + a) as i32 as f32;
        fader::fader(
            f,
            d.white,
            &d.v,
            [613.0, (BARKY + self.bar_y) as f32, 14.0, (h - 3) as f32],
            [ci(0.0), ci(54.0), ci(90.0)],
            (g * 155.0 + 100.0) as i32 as f32,
            [ci(0.0), ci(90.0), ci(130.0)],
            (g * 225.0 + 30.0) as i32 as f32,
        );
    }

    fn col(d: &Draw, i: i32) -> [f32; 3] {
        d.palette.get(i.max(0) as usize).copied().unwrap_or([1.0; 3])
    }

    fn row_hit(&self, pos: i32) -> bool {
        let (x, y) = (self.at[0] as i32, self.at[1] as i32);
        (TOPIC_X..=TOPIC_X + 0x76).contains(&x) && pos - 3 < y && y <= pos + 7 + 4
    }

    /// `HelpGfx2D_2`: the lasers, the exit label, the topic list (with its clicks), the text, the pictures.
    fn draw_texts(&mut self, f: &mut Frame, d: &Draw, fr: f32, char_y: &impl Fn(i32) -> i32) {
        let v = &d.v;
        self.lasers.clear();
        if fr != 1.0 && fr != 0.0 {
            self.lasers.shoot(TEXT_BOX, [639, 479], fr, LASER);
            self.lasers.shoot(TOPIC_BOX, [0, 0], fr, LASER);
        }
        self.lasers.draw(f, d.white, d.glow, v, true, fr != 0.0);
        if !self.open || self.closing {
            return;
        }
        let font = d.font;
        let label = d.texts.get(1, 2, 1);
        font.text(f, v, &label, 82.0, 410.0, 10, 2, Align::Centre, Self::col(d, 2), None);
        let click = self.held && self.pressed && !self.grabbed;
        let mut pos = TOPIC_Y;
        for i in 0..self.topics.len() {
            let row = pos;
            if self.row_hit(pos) && click {
                self.sub_sel = -1;
                self.load_topic(i, char_y);
            }
            let name = self.topics[i].name.clone();
            if i == self.sel {
                font.text(f, v, &name, TOPIC_X as f32, row as f32, name.len(), 2, Align::Left, Self::col(d, 2), None);
                pos += TOPIC_STEP;
                let subs = pos;
                for k in 0..self.topics[i].subs.len() as i32 {
                    if self.row_hit(pos) && click {
                        self.sub_sel = k;
                        self.sub_pos = self.text.find_sub(&self.table, k);
                        let n = self.text.n;
                        if remaining(&self.table, n, self.sub_pos) < TOPIC_BOX[3] {
                            let mut j = n as i32;
                            loop {
                                j -= 1;
                                if !(remaining(&self.table, n, j) < TOPIC_BOX[3] && j > 0) {
                                    break;
                                }
                            }
                            self.sub_pos = j;
                        }
                    }
                    let c = if k == self.sub_sel || (self.row_hit(pos) && !self.grabbed) { 0x25 } else { 1 };
                    let sub = &self.topics[i].subs[k as usize];
                    font.text(f, v, sub, SUB_X as f32, (subs + TOPIC_STEP * k) as f32, sub.len(), 2, Align::Left, Self::col(d, c), None);
                    pos += TOPIC_STEP;
                }
            } else {
                pos += TOPIC_STEP;
                let c = if self.row_hit(row) && !self.grabbed { 2 } else { 0x22 };
                font.text(f, v, &name, TOPIC_X as f32, row as f32, name.len(), 2, Align::Left, Self::col(d, c), None);
            }
        }
        self.draw_lines(f, d, char_y);
        self.draw_pictures(f, d);
    }

    /// The text from `TopicActSorPtr` (or the subtopic's line) down to y 415.
    fn draw_lines(&mut self, f: &mut Frame, d: &Draw, char_y: &impl Fn(i32) -> i32) {
        let t = &self.text;
        let mut m = if self.sub_pos != -1 && self.sub_sel != -1 { self.sub_pos } else { self.top };
        self.top = m;
        let row = self.table.get(m.max(0) as usize).copied().unwrap_or_default();
        let (mut col, mut fnt) = (row.col, row.font);
        let mut last_len = 0usize;
        self.shown = 0;
        let mut y = TEXT_Y;
        while y < TEXT_END && m < t.n as i32 - 1 {
            let mut p = self.table.get(m as usize).map_or(0, |r| r.start);
            let mut x = 0usize;
            loop {
                // `FindCommand`.
                if t.at(p) == b'<' {
                    let d2 = |c: u8| c as i32 - b'0' as i32;
                    match t.at(p + 1) {
                        b'C' | b'F' => {
                            let mut val = d2(t.at(p + 2));
                            last_len = 1;
                            if t.at(p + 3) != b'>' {
                                val = val * 10 + d2(t.at(p + 3));
                                last_len = 2;
                            }
                            if t.at(p + 1) == b'C' {
                                col = val;
                            } else {
                                fnt = val;
                            }
                        }
                        b'P' => last_len = (p + 2..t.buf.len()).find(|&k| t.buf[k] == b'>').map_or(0, |e| e - p - 2),
                        _ => {}
                    }
                    if t.at(p + 1) == b'S' && t.at(p + 3) == b'B' && t.at(p + 5) == b'O' {
                        last_len = 7;
                    }
                    p += last_len + 3;
                }
                // `FindSorVege`.
                let mut len = 0;
                while t.at(p + len) != b'\r' && t.at(p + len) != b'<' && p + len < t.buf.len() {
                    len += 1;
                }
                let eol = t.at(p + len) == b'\r';
                if len != 0 {
                    let adv = d.font.advance(fnt.max(0) as usize);
                    d.font.text(f, &d.v, &t.buf[p..p + len], 178.0 + x as f32 * adv, y as f32, len, fnt.max(0) as usize, Align::Left, Self::col(d, col), None);
                    p += len;
                    x += len;
                }
                if eol || p >= t.buf.len() {
                    break;
                }
            }
            m += 1;
            self.shown += 1;
            y += GAP + char_y(fnt);
        }
        self.bottom = y >= TEXT_END;
    }

    /// `KepToScreen(kep, x, y)`: the picture at its size, clipped to (178, 48)..(596, 429).
    fn picture(f: &mut Frame, d: &Draw, name: &[u8], x: i32, y: i32) {
        let Some((tex, [w, h])) = d.pics.get(name) else { return };
        let (x0, y0) = ((x as f32).max(CLIP[0] as f32), (y as f32).max(CLIP[1] as f32));
        let (x1, y1) = ((x as f32 + w).min(CLIP[2] as f32 + 1.0), (y as f32 + h).min(CLIP[3] as f32 + 1.0));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let v = &d.v;
        let mut i = Instance::new(kind::SPRITE, v.px((x0 + x1) / 2.0, (y0 + y1) / 2.0), [(x1 - x0) * v.k / 2.0, (y1 - y0) * v.k / 2.0], [1.0; 4]);
        i.uv = [(x0 - x as f32) / w, (y0 - y as f32) / h, (x1 - x0) / w, (y1 - y0) / h];
        f.push(tex, Blend::GammaAlpha, i);
    }

    /// `HelpKirakPrevPictures` and `HelpKirakOnScreenPictures` (§4.4).
    fn draw_pictures(&self, f: &mut Frame, d: &Draw) {
        let t = &self.text;
        let first = self.top.max(0) as usize;
        let tab = &self.table;
        let height = |k: usize| tab.get(k).map_or(0, |r| r.height);
        // The `<P…>` tags of line `line`: (name, column).
        let tags = |line: usize| {
            let mut out = Vec::new();
            let Some(r) = tab.get(line) else { return out };
            let (mut p, mut col) = (r.start, 0i32);
            while t.at(p) != b'\r' && p < t.buf.len() {
                if t.at(p) == b'<' && t.at(p + 1) == b'P' {
                    let e = (p + 2..t.buf.len()).find(|&k| t.buf[k] == b'>').unwrap_or(t.buf.len());
                    out.push((t.buf[p + 2..e].to_vec(), col));
                    p += e - p + 1;
                    col -= 1;
                }
                col += 1;
                p += 1;
            }
            out
        };
        let x_of = |line: usize, col: i32| 178 + col * d.font.advance(tab.get(line).map_or(1, |r| r.font).max(0) as usize) as i32;
        for (line, row) in tab.iter().enumerate().take(first) {
            if !row.pic {
                continue;
            }
            for (name, col) in tags(line) {
                let s: i32 = (line..first).map(height).sum();
                if d.pics.get(&name).is_some_and(|(_, [_, h])| (s as f32) < h) {
                    Self::picture(f, d, &name, x_of(line, col), TEXT_Y - s);
                }
            }
        }
        for (line, row) in tab.iter().enumerate().take(first + self.shown as usize).skip(first) {
            if !row.pic {
                continue;
            }
            for (name, col) in tags(line) {
                let s: i32 = (first + 1..line + 1).map(height).sum();
                Self::picture(f, d, &name, x_of(line, col), TEXT_Y + s);
            }
        }
    }
}

/// The install's `help/helpmain.txt` and a reader for the topic files.
pub fn install_files(install: &ig2_formats::Install) -> (Vec<u8>, impl Fn(&[u8]) -> Vec<u8> + '_) {
    let index = install.read("dataraw/help/helpmain.txt").unwrap_or_default();
    let read = move |name: &[u8]| install.read(&format!("dataraw/help/{}", String::from_utf8_lossy(name).to_ascii_lowercase())).unwrap_or_default();
    (index, read)
}

/// `CharYMeret[font]` from the font.
pub fn char_y(font: &Font) -> impl Fn(i32) -> i32 + '_ {
    move |k: i32| font.height(k.max(0) as usize) as i32
}

/// `ig2-client --snapshot OUT --screen help [--ui-frames N] [--mouse X,Y] [--script S]`: Help opened on the save's
/// game (as from the starmap) after N UI ticks, one drawn frame each; the top bar over it.
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    use crate::script::Input;
    let l = crate::load(o)?;
    let world = crate::start_world(o, &l.install, l.save.as_ref(), &l.rules, &l.tech_defaults, &l.volatile)?;
    let mut r = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut hud = crate::ui::Hud::load(&mut r, &l.install, &l.tables);
    let glow = l.install.resolve_texture("textures/starmap/", "laservago.bmp").and_then(|p| crate::assets::Image::load(&l.install, &p).ok()).map_or(hud.white, |img| r.add_texture(&img));
    let (index, read) = install_files(&l.install);
    let mut s = {
        let font = hud.font.as_ref().ok_or("no font")?;
        HelpScreen::enter(&index, read, &Memory::default(), true, char_y(font))
    };
    let mut pics = Pictures::default();
    let size = [o.size.0 as f32, o.size.1 as f32];
    let script = o.script.as_deref().map(crate::script::Script::load).transpose()?;
    let (end, grabs) = match &script {
        Some(sc) => (sc.end(), sc.grabs.clone()),
        None => (o.ui_frames, vec![o.ui_frames]),
    };
    s.hover(o.mouse.unwrap_or([320.0, 240.0]));
    for t in 0..=end {
        for e in script.iter().flat_map(|sc| sc.at(t)) {
            match &e.input {
                Input::Move(at) => s.hover(*at),
                Input::Button { button: 1, down: true, at } => s.press(*at),
                Input::Button { button: 1, down: false, .. } => s.release(),
                Input::Key { name, down: true } if name == "escape" => s.escape(),
                _ => {}
            }
        }
        if t > 0 {
            s.tick();
        }
        pics.load(&mut r, &l.install, &s.text.pics);
        let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
        {
            let font = hud.font.as_ref().ok_or("no font")?;
            let d = Draw { font, white: hud.white, glow, v: Virtual::new(size), palette: &hud.palette, texts: &hud.texts, pics: &pics, now_ms: t as f64 * 10.0 };
            s.draw(&mut frame, &d, char_y(font));
        }
        if let Some(a) = s.done() {
            println!("t {t}: {a:?}");
        }
        if grabs.contains(&t) {
            hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Help, 0.0);
            let rgba = r.draw_to_image(&frame, o.size, 0.0);
            let path = if script.is_some() { crate::script::grab_path(out, t) } else { out.to_path_buf() };
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {t} | topic {} top {} lines {} frame {}", path.display(), s.sel, s.top, s.text.n, s.frame);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index() {
        let t = parse_index(b"comment\r\n<TOPICGeneral icons,Toolbar1.txt>\r\n\r\n<TOPICStarmap,starmap1.txt>\r\n<SUBTOPICRADAR>\r\n<SUBTOPICFLEETS>\r\n");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], Topic { name: b"General icons".to_vec(), file: b"Toolbar1.txt".to_vec(), subs: vec![] });
        assert_eq!(t[1].subs, vec![b"RADAR".to_vec(), b"FLEETS".to_vec()]);
    }

    #[test]
    fn wrapping() {
        // `+` starts a line (no space before its first word), `-` goes on with a space, `+ ` alone is an empty line,
        // `*` is copied as it is, anything else ends the parse.
        let w = wrap(b"* <F4>TITLE\r\n+ \r\n+ one two\r\n- three\r\n+\r\nx stop\r\n+ never\r\n");
        assert_eq!(w, b"<F4>TITLE\r\rone two three\r\r".to_vec());
        // A word that would pass column 69 starts a new line.
        let long = format!("+ {}\r\n- bbbb\r\n", "a".repeat(66));
        assert_eq!(wrap(long.as_bytes()), format!("{}\rbbbb\r", "a".repeat(66)).into_bytes());
        // Tags count −1 for `<` and `>` on the line already written, fully in the word.
        let w = wrap(b"+ <C5>abc\r\n- def\r\n");
        assert_eq!(w, b"<C5>abc def\r".to_vec());
    }

    #[test]
    fn table() {
        let t = Text::new(b"* <F3><C5>Head\r\n+ body <Ppic.bmp>\r\n+ <F2>more\r\n");
        assert_eq!(t.n, 4);
        assert_eq!(t.pics, vec![b"pic.bmp".to_vec()]);
        let mut tab = Vec::new();
        t.pregen(&mut tab, |f| [0, 5, 7, 10, 12].get(f as usize).copied().unwrap_or(0));
        assert_eq!((tab[1].col, tab[1].font, tab[1].height, tab[1].pic), (5, 3, 13, true));
        assert_eq!((tab[2].font, tab[2].height), (3, 13));
        assert_eq!((tab[3].font, tab[3].height), (2, 10));
        assert!(!tab[0].pic);
    }
}
