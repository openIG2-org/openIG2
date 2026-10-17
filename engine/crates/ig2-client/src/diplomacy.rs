//! The diplomacy screen (`AktKep` 6, re/screens/diplomacy.md): the contacts' heads with their names, the selected
//! empire's info panel, the menus of proposals and answers (`Menuk`), and the dialogue texts (`GetSzoveg`). The
//! human's messages and answers leave as lockstep commands (`Command::DiplomacyMessage`/`DiplomacyAnswer`, with the
//! screen's own contact-list writes as `Command::DiplomacyScreen`); the 3D frame is [`crate::diplomacy_scene`].

use ig2_formats::Install;
use ig2_net::{Command, DiplomacyScreenOp, Terms};
use ig2_sim::diplomacy::{PlayerSlot, Relation};
use ig2_sim::save::{va, SaveState, PLANET_SIZE, TECHS, TECH_SIZE};

use crate::font::{Align, Font, Virtual};
use crate::render::{Frame, TexId};
use crate::ui::UiTexts;

/// A contact-list slot's size (`AktPlayerek`).
pub const SLOT: usize = 0xc4;
/// Head slots on screen.
pub const HEADS: usize = 6;

/// Menu ids (`Menuk` records, §6).
pub mod menu {
    pub const MAIN: usize = 0;
    pub const OFFER: usize = 1;
    pub const MONEY: usize = 2;
    pub const HOSTAGE: usize = 3;
    pub const PLANETS: usize = 4;
    /// 5..=0x10: the tech areas of an offer.
    pub const TECHS: usize = 5;
    pub const TALK: usize = 0x13;
    pub const EXCHANGE: usize = 0x14;
    pub const FOR: usize = 0x15;
    pub const FOR_MONEY: usize = 0x16;
    pub const FOR_HOSTAGE: usize = 0x17;
    pub const FOR_PLANETS: usize = 0x18;
    /// 0x19..=0x24: the tech areas of the exchange side.
    pub const FOR_TECHS: usize = 0x19;
    pub const SEND: usize = 0x28;
    pub const NEGOTIATE: usize = 0x29;
    pub const COUNTER_REPLY: usize = 0x2a;
    pub const DEAL_REPLY: usize = 0x2b;
    pub const BLOCKED: usize = 0x2c;
    pub const OK: usize = 0x2f;
    pub const SORRY: usize = 0x30;
    pub const TALK_EMPIRES: usize = 0x31;
    pub const INFO: usize = 0x3c;
    pub const FOR_INFO: usize = 0x3d;
    pub const ALLIES: usize = 0x3f;
    pub const COUNT: usize = 0x42;
}

/// One menu entry (0x40 bytes): text, `+0x34` value, `+0x38` sub-menu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub text: Vec<u8>,
    pub value: i32,
    pub sub: i32,
}

impl Entry {
    fn new(text: Vec<u8>, value: i32, sub: i32) -> Self {
        Self { text, value, sub }
    }
}

/// A `Menuk` record: entries, colours, font, place, row gap.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub entries: Vec<Entry>,
    pub hicol: usize,
    pub col: usize,
    pub font: usize,
    pub x: i32,
    pub y: i32,
    pub gap: i32,
}

impl Default for Menu {
    fn default() -> Self {
        // `InitAlmenuTipus`: size 2, row gap 4, colours 34 / 2.
        Self { entries: Vec::new(), hicol: 0x22, col: 2, font: 2, x: 0, y: 0, gap: 4 }
    }
}

/// `CharXMeret`/`CharYMeret` and the column layout (`MenuMaxSorokO`, `MenuOszlopKoz`).
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub width: [i32; 5],
    pub height: [i32; 5],
    pub rows: i32,
    pub col_gap: i32,
}

impl Metrics {
    pub fn new(rules: &ig2_rules::Rules) -> Self {
        let c = &rules.charset;
        let at = |v: &Vec<i32>, i: usize| v.get(i).copied().unwrap_or(6);
        let d = &rules.diplomacy_screen;
        Self {
            width: std::array::from_fn(|i| at(&c.width, i)),
            height: std::array::from_fn(|i| at(&c.height, i)),
            rows: d.menu_rows.max(1),
            col_gap: d.menu_gap,
        }
    }
}

impl Menu {
    fn longest(&self) -> i32 {
        self.entries.iter().map(|e| e.text.len() as i32).max().unwrap_or(0)
    }
    fn columns(&self, m: &Metrics) -> i32 {
        (self.entries.len() as i32 + m.rows - 1) / m.rows
    }
    /// Rows per column: all of them up to `MenuMaxSorokO`, else spread over the columns.
    fn per_column(&self, m: &Metrics) -> i32 {
        let n = self.entries.len() as i32;
        if n > m.rows {
            let c = self.columns(m).max(1);
            (n + c) / c
        } else {
            n
        }
    }
    fn row_h(&self, m: &Metrics) -> i32 {
        m.height[self.font.min(4)] + self.gap
    }
    fn col_w(&self, m: &Metrics) -> i32 {
        m.width[self.font.min(4)] * self.longest() + m.col_gap
    }
    /// `KirakMenu`'s move up: a menu ending below y 449 ends there.
    fn fit(&mut self, m: &Metrics) {
        let h = self.row_h(m) * self.per_column(m);
        if self.y + 0x1f + h > 0x1e0 {
            self.y = 0x1c1 - h;
        }
    }
    /// The frame `FadeKerettelKerekit(x − 4, y − 4, w, h)`.
    pub fn frame(&self, m: &Metrics) -> [f32; 4] {
        let w = self.col_w(m) * self.columns(m) - m.col_gap + 7;
        let h = self.row_h(m) * self.per_column(m) + 5;
        [(self.x - 4) as f32, (self.y - 4) as f32, w as f32, h as f32]
    }
    /// The entry under virtual point `p` (`KirakMenu`'s hover test).
    pub fn row_at(&self, m: &Metrics, p: [f32; 2]) -> Option<usize> {
        let (per, cw, rh) = (self.per_column(m).max(1), self.col_w(m), self.row_h(m));
        let (fh, half) = (m.height[self.font.min(4)], self.gap / 2);
        (0..self.entries.len()).find(|&i| {
            let (c, r) = (i as i32 / per, i as i32 % per);
            let x0 = c * cw + self.x - 4;
            let y0 = rh * r + self.y;
            p[0] >= x0 as f32 && p[0] < (x0 + cw) as f32 && p[1] >= (y0 - half) as f32 && p[1] < (y0 + half + fh) as f32
        })
    }
    /// Where entry `i`'s text starts.
    fn text_at(&self, m: &Metrics, i: usize) -> [f32; 2] {
        let per = self.per_column(m).max(1);
        let (c, r) = (i as i32 / per, i as i32 % per);
        [(c * self.col_w(m) + self.x) as f32, (self.row_h(m) * r + self.y) as f32]
    }
}

/// `Igazitmenu(sub, parent)`: `sub` right of `parent`, level with the parent's (last) entry that opens it.
fn place(menus: &mut [Menu], m: &Metrics, sub: usize, parent: usize) {
    let p = menus[parent].clone();
    let idx = p.entries.iter().rposition(|e| e.sub == sub as i32).unwrap_or(0) as i32;
    let s = &mut menus[sub];
    s.x = p.columns(m) * p.col_w(m) - m.col_gap + p.x + 5;
    s.y = p.y + idx * p.row_h(m);
    s.fit(m);
}

/// What a menu entry means for the message: `(sub, value)` (`Predata1/2`, `Predata3/4`).
fn item_terms(menus: &[Menu], id: usize, row: usize) -> (i32, i32) {
    let v = menus[id].entries.get(row).map_or(0, |e| e.value);
    match id {
        menu::MONEY | menu::FOR_MONEY => (0, MONEY[row.min(5)]),
        menu::PLANETS | menu::FOR_PLANETS => (2, v),
        menu::HOSTAGE | menu::FOR_HOSTAGE => (4, v),
        menu::INFO | menu::FOR_INFO => (1, v),
        _ => (3, v),
    }
}

/// `Kiszamolmoney`: the money menus' sums.
const MONEY: [i32; 6] = [10000, 20000, 50000, 100000, 200000, 400000];

/// The texts the screen reads (`Diplomacy_LoadStrings`, the phrase files).
pub struct Texts {
    /// `DiplomaciaFilePufferek`: `ajanlat.eng`, `ajanlat.eng`, `valaszok.eng` as raw bytes.
    pub files: [Vec<u8>; 3],
    /// `Beszolasok[k]` = `GetString(6, 4, k + 1)`.
    pub phrases: Vec<Vec<u8>>,
    /// `CsaladtagSzovegek[k]` = `GetString(6, 2, k + 1)`.
    pub family: Vec<Vec<u8>>,
    /// `StatuszNevek[k]` = `GetString(6, 5, k + 1)`.
    pub status: Vec<Vec<u8>>,
    /// `Faj_Nev[r]` = `GetString(0x11, 1, r + 5)`.
    pub races: Vec<Vec<u8>>,
    /// The techs' localised names (`Talalmanyok[t]`, re/screens/hud.md §4.1).
    pub techs: Vec<Vec<u8>>,
    /// Whether `a2az` converts articles (`Lang == 4`, Hungarian).
    pub hungarian: bool,
}

impl Texts {
    pub fn load(install: &Install, ui: &UiTexts) -> Self {
        let file = |n: &str| install.read(&format!("dataraw/diplomac/{n}")).unwrap_or_default();
        Self {
            files: [file("ajanlat.eng"), file("ajanlat.eng"), file("valaszok.eng")],
            phrases: (1..=13).map(|k| ui.get(6, 4, k)).collect(),
            family: (1..=5).map(|k| ui.get(6, 2, k)).collect(),
            status: (1..=6).map(|k| ui.get(6, 5, k)).collect(),
            races: (0..20).map(|r| ui.get(0x11, 1, r + 5)).collect(),
            techs: (0..TECHS).map(|t| crate::hud::tech_name(ui, t)).collect(),
            hungarian: install.lang() == ig2_formats::text::Lang::Hu,
        }
    }
    fn phrase(&self, k: usize) -> &[u8] {
        self.phrases.get(k).map_or(&[], Vec::as_slice)
    }
    fn race(&self, r: i32) -> Vec<u8> {
        self.races.get(r.max(0) as usize).cloned().unwrap_or_default()
    }
    fn tech(&self, t: i32) -> Vec<u8> {
        self.techs.get(t.max(0) as usize).cloned().unwrap_or_default()
    }
    /// `GetCsaladtagString(owner, who)`: "the <race> Emperor's wife" … (`CsaladtagSzovegek`, `%s` = `Faj_Nev[owner]`,
    /// indexed by the empire's number as the original does).
    fn family(&self, owner: i32, who: i32) -> Vec<u8> {
        let tpl = self.family.get((who.clamp(1, 5) - 1) as usize).cloned().unwrap_or_default();
        sprintf_s(&tpl, &self.race(owner))
    }
}

/// `GetSzoveg(file, out, a, b, c, d, p1, p2, p3, fallback, seed)` (ELF 0x521ab0, §7): the dialogue text of
/// header `#aaa/bbb[/ccc[/ddd]]`, variant `n·seed/10001` (else `pick(n)`), the `- ` lines joined with spaces,
/// `#1#`..`#3#` replaced, wrapped at 70 columns into at most five lines.
#[allow(clippy::too_many_arguments)]
pub fn get_szoveg(buf: &[u8], key: [i32; 4], params: [&[u8]; 3], fallback: bool, seed: i32, pick: &mut dyn FnMut(usize) -> usize) -> Vec<Vec<u8>> {
    let part = |v: i32| format!("{:03}", v as u32);
    let mut head = format!("#{}/{}", part(key[0]), part(key[1]));
    for v in [key[2], key[3]] {
        if v != -1 {
            head.push('/');
            head.push_str(&part(v));
        }
    }
    let starts = |h: &[u8]| -> Vec<usize> { (0..buf.len()).filter(|&i| buf[i..].starts_with(h)).collect() };
    let found = starts(head.as_bytes());
    let mut at = if found.is_empty() {
        let alt = if fallback { starts(format!("A{}/{}", part(key[2]), part(key[3])).as_bytes()) } else { Vec::new() };
        match alt.first() {
            Some(&i) => i,
            None => return excuse(pick(2)),
        }
    } else {
        let k = if seed != -1 { ((found.len() as f32 * seed as f32) / 10001.0) as usize } else { pick(found.len()) };
        found[k.min(found.len() - 1)]
    };
    while at < buf.len() && buf[at] != b'-' {
        at += 1;
    }
    // `dipAtteszSorvegeig`: each line from its third byte to the CR, joined by single spaces.
    let mut text = Vec::new();
    while at < buf.len() && buf[at] == b'-' {
        let end = buf[at..].iter().position(|&c| c == b'\r' || c == b'\n').map_or(buf.len(), |p| at + p);
        text.extend_from_slice(buf.get(at + 2..end).unwrap_or(&[]));
        text.push(b' ');
        at = end;
        while at < buf.len() && (buf[at] == b'\r' || buf[at] == b'\n') {
            at += 1;
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let mark = [b"#1#", b"#2#", b"#3#"].iter().position(|m| text[i..].starts_with(*m));
        match mark {
            Some(k) if !params[k].is_empty() => {
                out.extend_from_slice(params[k]);
                i += 3;
            }
            _ => {
                out.push(text[i]);
                i += 1;
            }
        }
    }
    wrap(&out)
}

/// The word wrap of `GetSzoveg`: words (with their following space) are added while the line stays under 70.
fn wrap(text: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    let mut i = 0;
    while i < text.len() && lines.len() < 5 {
        let mut line = Vec::new();
        let word = |i: usize| text[i..].iter().position(|&c| c == b' ').unwrap_or(text.len() - i);
        let mut w = word(i);
        while (line.len() as i32 + w as i32) < 0x46 && i < text.len() {
            let n = (w + 1).min(text.len() - i);
            line.extend_from_slice(&text[i..i + n]);
            i += n;
            if i >= text.len() {
                break;
            }
            w = word(i);
        }
        if line.is_empty() {
            // A word longer than the line: the original stops there.
            break;
        }
        while line.last() == Some(&b' ') {
            line.pop();
        }
        lines.push(line.into_iter().take(99).collect());
    }
    lines
}

/// `GetSzoveg`'s two excuses when no phrase matches (code immediates).
fn excuse(k: usize) -> Vec<Vec<u8>> {
    let l: [&str; 3] = if k == 0 {
        [
            "Radio engineers report: - Sir! We are terribly sorry, but due to a",
            "                          deep space storm, we can't send messages into",
            "                          the hyperspace for a few days! (press CANCEL)",
        ]
    } else {
        [
            "Due to a sabotage at the intergalactical telecommunication company",
            "all the deep space intercommunical messages are redirected to",
            "/dev/null... Try sending your messages in the next month... (press CANCEL)",
        ]
    };
    l.iter().map(|s| s.as_bytes().to_vec()).collect()
}

/// `Magyar_a2az_konverter` (ELF 0x6240b0): the article markers 0x95 / 0xB0 become `z`/`Z` before a word that
/// doesn't start with a consonant, and vanish otherwise.
pub fn a2az(s: &[u8]) -> Vec<u8> {
    const CONSONANTS: &[u8] = b"BCDFGHJKLMNPQRSTVWXYZbcdfghjklmnpqrstvwxyz";
    let mut s = s.to_vec();
    while let Some(p) = s.iter().position(|&c| c == 0x95 || c == 0xB0) {
        let mut q = p + 1;
        while q + 1 < s.len() && (s[q] == b' ' || s[q] == b'~') {
            if s[q] == b'~' {
                q += 1;
            }
            q += 1;
        }
        let mut out = s[..p].to_vec();
        if q + 1 < s.len() {
            if !CONSONANTS.contains(&s[q]) {
                out.push(if s[p] == 0x95 { b'z' } else { b'Z' });
            }
            out.extend_from_slice(&s[p + 1..]);
        }
        s = out;
    }
    s
}

/// What the screen reads of the state each frame.
pub struct View {
    pub en: usize,
    pub ido: i32,
    pub month: i32,
    pub tutorial: bool,
    pub slots: Vec<PlayerSlot>,
    /// The slots' raw bytes (dialogue fields the sim doesn't model: `+0x15`, `+0x5c`, `+0x60`, `+0x8c`, `+0x90`).
    pub raw: Vec<Vec<u8>>,
    pub alive: [bool; 19],
    pub races: [u8; 19],
    pub relations: Vec<[Relation; 19]>,
    pub met: Vec<[u8; 19]>,
    /// `BirodalmiEvfordulok[e]` (the ruler's anniversary month), `HanyevesAzUralkodo[e]`,
    /// `VoltmarUdvozloUzenetEbbenAzEvben[e]`.
    pub anniversary: Vec<i32>,
    pub ruler_age: Vec<i32>,
    pub greeted: Vec<u8>,
    /// `Bolygok[p]` names (index 0 unused).
    pub planet_names: Vec<Vec<u8>>,
}

impl View {
    pub fn read(state: &SaveState) -> Self {
        let m = &state.mem;
        let d = state.diplomacy();
        let n = d.players.len();
        let t = ig2_sim::time::GameTime(m.i32(va::IDO));
        Self {
            en: m.i32(va::EN).clamp(0, 18) as usize,
            ido: m.i32(va::IDO),
            month: t.month(),
            tutorial: false,
            raw: (0..n).map(|i| m.read(va::CONTACTS + (i * SLOT) as u32, SLOT)).collect(),
            slots: d.players,
            alive: state.alive(),
            races: state.races(),
            relations: state.relations(),
            met: state.starmap().met.to_vec(),
            anniversary: (0..19).map(|e| m.i32(0xA41D78 + 4 * e)).collect(),
            ruler_age: (0..19).map(|e| m.i32(va::RULER_AGE + 4 * e)).collect(),
            greeted: m.read(va::GREETED, 19),
            planet_names: (0..=m.i32(va::PLANET_COUNT).max(0) as u32)
                .map(|p| {
                    let b = m.read(va::PLANETS + p * PLANET_SIZE as u32, 0x14);
                    b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()
                })
                .collect(),
        }
    }
    pub fn empire(&self, k: usize) -> i32 {
        self.slots.get(k).map_or(-1, |s| s.empire)
    }
    pub fn alive_slot(&self, k: usize) -> bool {
        let e = self.empire(k);
        (0..19).contains(&e) && self.alive[e as usize]
    }
    fn rel(&self, a: usize, b: usize) -> Relation {
        self.relations.get(a).map_or_else(Relation::default, |r| r[b.min(18)])
    }
    fn raw_i32(&self, k: usize, o: usize) -> i32 {
        self.raw.get(k).and_then(|r| r.get(o..o + 4)).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
    }
    fn raw_u8(&self, k: usize, o: usize) -> u8 {
        self.raw.get(k).and_then(|r| r.get(o)).copied().unwrap_or(0)
    }
    fn knows(&self, a: usize, b: usize) -> bool {
        self.met.get(a).is_some_and(|r| r.get(b).is_some_and(|&x| x != 0))
    }
}

/// The power class and stance of `a` towards `b` (`MegallapitViszonyValtozokat` exe 0x4c7c00): `Viszony[a][b]`'s
/// power relation `+0x1c` below 4200 → 0, above 5800 → 2, else 1; allied; hostile (war or cease-fire).
fn stance(v: &View, a: usize, b: usize) -> (i32, bool, bool) {
    let r = v.rel(a, b);
    let p = r.v[14];
    let class = if p < 0x1068 { 0 } else if p > 0x16a8 { 2 } else { 1 };
    (class, r.level == 5, r.level == 1 || r.level == 2)
}

/// The reason code of the human's own proposal (`DiplomGfx2D_1` for negotiate/talk, `MiertValaszoltAztAmitValaszolt
/// (EN, e, 2, cat)` exe 0x4c7e80 for deals): the phrase group by power and stance.
pub fn own_reason(v: &View, e: usize, cat: i32) -> i32 {
    let (class, allied, hostile) = stance(v, v.en, e);
    if cat == 0 || cat == 6 {
        return match class {
            0 => allied as i32,
            1 => 1,
            _ => if allied { 1 } else { 2 },
        };
    }
    if allied && !hostile {
        return 0;
    }
    match (class, hostile) {
        (0, true) => 4,
        (0, false) => 1,
        (1, true) => 5,
        (1, false) => 2,
        (_, true) => 6,
        (_, false) => 3,
    }
}

/// A proposal ready to send (`Keszazajanlat`): the message, its text, and the birthday greeting flag.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Proposal {
    pub declaration: bool,
    pub terms: Terms,
    pub lines: Vec<Vec<u8>>,
    pub greeting: bool,
}

/// An item chosen from a menu: which menu and row, for the message and its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pick {
    menu: usize,
    row: usize,
}

/// An item named for a counter-proposal, with its text.
type Named = (Pick, Vec<u8>);

/// The open menu cascade (`ActFomenu`, `ActFomenuSzam`, `Almenuksel`): the root menu's pressed row, the sub-menu's
/// last hovered row, and the leaf under the pointer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Cascade {
    root: usize,
    row: Option<usize>,
    sub_row: Option<usize>,
}

/// What a frame's pointer did (`Mouse_Lenyom`, `Mouse_Bal`, `Mouse_Felenged`), in virtual pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Input {
    pub at: [f32; 2],
    pub pressed: bool,
    pub held: bool,
    pub released: bool,
}

/// The screen's own state (the original's globals): selection, menus, the composed texts, the fades.
pub struct Screen {
    pub menus: Vec<Menu>,
    pub metrics: Metrics,
    /// `PlayerSelected`.
    pub selected: i32,
    cascade: Option<Cascade>,
    /// `Keszazajanlat` + `KeszAjanlat`: the proposal waiting for "send it now".
    pub proposal: Option<Proposal>,
    /// `Exchangemode`: the first half of an "offer … for …" (`Almenuksel0`).
    exchange: Option<(Pick, Vec<u8>)>,
    /// `AktPlayerek[k].+0x19` and its menu (`+0x1c`/`+0x20`): a counter-proposal being named for a waiting deal.
    counter: Option<(usize, Option<Named>)>,
    /// The texts drawn this frame: own (`Szoveg1`/`Szoveg4`) and the other side's (`Szoveg2/3/5`).
    pub own_text: Vec<Vec<u8>>,
    pub other_text: Vec<Vec<u8>>,
    /// The menus shown this frame, in order (`MenuKirakottak`).
    pub shown: Vec<usize>,
    /// The hovered entry of each shown menu.
    hover: Vec<(usize, Option<usize>)>,
    /// `SzinallitoDesignTablazat`, `infotransp`, `mymstransp`, `thmstransp` (§4).
    pub row_fade: [i32; 10],
    pub info_fade: f32,
    pub own_fade: f32,
    pub other_fade: f32,
    /// `Atfordulas` (0..30 after a new selection) and the UI ticks since entering.
    pub turn: f32,
    pub ui_ticks: u64,
    /// The client's own generator for the unseeded text variants (ADR 0003: not the sim's libc stream).
    rng: u32,
    /// Variant memory so a text doesn't change every frame.
    cache: Option<(Vec<i32>, Vec<Vec<u8>>)>,
}

impl Screen {
    pub fn new(rules: &ig2_rules::Rules, ui: &UiTexts, v: &View) -> Self {
        let metrics = Metrics::new(rules);
        let d = &rules.diplomacy_screen;
        let mut menus = vec![Menu::default(); menu::COUNT];
        let big = |x: i32, y: i32| Menu { font: 3, gap: 6, x, y, ..Menu::default() };
        let [mx, my] = d.menu_xy;
        menus[menu::MAIN] = big(mx, my);
        for (id, texts) in [
            (menu::SEND, vec![(7, 1), (7, 2)]),
            (menu::COUNTER_REPLY, vec![(7, 3), (7, 4)]),
            (menu::DEAL_REPLY, vec![(7, 3), (7, 4), (7, 5)]),
            (menu::OK, vec![(7, 7)]),
            (menu::SORRY, vec![(7, 8)]),
            (menu::EXCHANGE, vec![(10, 5), (10, 6)]),
        ] {
            menus[id] = big(mx, my);
            menus[id].entries = texts.into_iter().map(|(s, i)| Entry::new(ui.get(6, s, i), 0, -1)).collect();
        }
        // `FeltoltMenupontokPre`: the deal reply's "okay, but …" opens the exchange menu; it keeps two entries.
        menus[menu::DEAL_REPLY].entries[2].sub = menu::FOR as i32;
        menus[menu::DEAL_REPLY].entries.truncate(2);
        menus[menu::EXCHANGE].entries[0].sub = menu::FOR as i32;
        menus[menu::BLOCKED] = big(mx, my);
        for id in [menu::OFFER, menu::FOR] {
            menus[id] = Menu { x: 0xa8, y: 0x15e, ..Menu::default() };
        }
        let mut s = Self {
            menus,
            metrics,
            selected: -1,
            cascade: None,
            proposal: None,
            exchange: None,
            counter: None,
            own_text: Vec::new(),
            other_text: Vec::new(),
            shown: Vec::new(),
            hover: Vec::new(),
            row_fade: std::array::from_fn(|k| -10 * k as i32),
            info_fade: 0.0,
            own_fade: 0.0,
            other_fade: 0.0,
            turn: 30.0,
            ui_ticks: 0,
            rng: 0x1234_5678,
            cache: None,
        };
        // `Diplomacy3DInit`: the first slot of a living empire.
        s.selected = (0..v.slots.len()).find(|&k| v.alive_slot(k)).map_or(-1, |k| k as i32);
        s
    }

    fn pick(&mut self, n: usize) -> usize {
        self.rng = self.rng.wrapping_mul(214013).wrapping_add(2531011);
        if n == 0 {
            0
        } else {
            ((self.rng >> 16) & 0x7fff) as usize % n
        }
    }

    /// `FRISSIT_Diplomacy2` (§4), one 10 ms UI tick on an open screen.
    pub fn ui_tick(&mut self) {
        self.ui_ticks += 1;
        for f in &mut self.row_fade {
            *f = (*f + 1).min(100);
        }
        if self.selected != -1 && self.info_fade < 127.0 {
            self.info_fade = (self.info_fade + 1.6).min(127.0);
        }
        self.own_fade = if self.own_text.is_empty() { (self.own_fade - 1.6).max(0.0) } else { (self.own_fade + 16.0).min(127.0) };
        self.other_fade = if self.other_text.is_empty() { (self.other_fade - 1.6).max(0.0) } else { (self.other_fade + 1.6).min(127.0) };
        if self.turn < 30.0 {
            self.turn = (self.turn + 0.2).min(30.0);
        }
    }

    /// The screen opened at rest (no opening animation; the snapshot and the capture comparison).
    pub fn settle(&mut self) {
        self.row_fade = [100; 10];
        self.info_fade = 127.0;
        self.own_fade = if self.own_text.is_empty() { 0.0 } else { 127.0 };
        self.other_fade = if self.other_text.is_empty() { 0.0 } else { 127.0 };
        self.turn = 30.0;
    }

    fn row_bright(&self, k: usize) -> f32 {
        self.row_fade.get(k).map_or(100, |&f| f.max(0)) as f32 / 100.0
    }

    /// The menu cascade from `root` (§6): a press on a root entry opens its sub-menu, which follows the pointer
    /// while the button is held; a release on a leaf picks it. Returns `(root row, sub-menu, leaf menu, leaf row)`.
    fn run_cascade(&mut self, root: usize, inp: &Input) -> Option<(usize, usize, Pick)> {
        let m = self.metrics;
        self.menus[root].fit(&m);
        self.shown.push(root);
        let root_row = self.menus[root].row_at(&m, inp.at);
        if inp.pressed && root_row.is_some() {
            self.cascade = Some(Cascade { root, row: root_row, sub_row: None });
        }
        let Some(mut c) = self.cascade.filter(|c| c.root == root) else {
            self.hover.push((root, root_row));
            return None;
        };
        if inp.held {
            if let Some(r) = root_row {
                if c.row != Some(r) {
                    c.row = Some(r);
                    c.sub_row = None;
                }
            }
        }
        self.hover.push((root, c.row));
        let row = c.row?;
        let sub = self.menus[root].entries.get(row).map_or(-1, |e| e.sub);
        let mut chosen = None;
        if sub >= 0 && !self.menus[sub as usize].entries.is_empty() {
            let sub = sub as usize;
            // `KirakFomenuAlpont`: the sub-menu beside the root, level with the pressed entry (18 px rows).
            place(&mut self.menus, &m, sub, root);
            self.menus[sub].y = self.menus[root].y + row as i32 * 0x12;
            self.menus[sub].fit(&m);
            self.shown.push(sub);
            if let Some(r) = self.menus[sub].row_at(&m, inp.at) {
                c.sub_row = Some(r);
            }
            self.hover.push((sub, c.sub_row));
            if let Some(sr) = c.sub_row {
                let leaf = self.menus[sub].entries[sr].sub;
                if leaf >= 0 && !self.menus[leaf as usize].entries.is_empty() {
                    let leaf = leaf as usize;
                    place(&mut self.menus, &m, leaf, sub);
                    self.shown.push(leaf);
                    let lr = self.menus[leaf].row_at(&m, inp.at);
                    self.hover.push((leaf, lr));
                    if let (true, Some(lr)) = (inp.released, lr) {
                        chosen = Some((row, sub, Pick { menu: leaf, row: lr }));
                    }
                } else if leaf < 0 && inp.released && self.menus[sub].row_at(&m, inp.at) == Some(sr) {
                    chosen = Some((row, sub, Pick { menu: sub, row: sr }));
                }
            }
        } else if sub < 0 && inp.released && root_row == Some(row) {
            chosen = Some((row, root, Pick { menu: root, row }));
        }
        self.cascade = if inp.released { None } else { Some(c) };
        chosen
    }

    /// A one-level menu at the menus' place: the entry released on.
    fn run_plain(&mut self, id: usize, inp: &Input) -> Option<usize> {
        let m = self.metrics;
        self.menus[id].fit(&m);
        self.shown.push(id);
        let r = self.menus[id].row_at(&m, inp.at);
        self.hover.push((id, r));
        if inp.pressed {
            r
        } else {
            None
        }
    }

    /// `FeltoltMenupontok` (§6): the proposal menus for slot `k`.
    fn fill(&mut self, v: &View, ui: &UiTexts, t: &Texts, names: &[Vec<u8>], state: &SaveState, k: usize) {
        let e = v.empire(k).max(0) as usize;
        let en = v.en;
        let rel = v.rel(en, e);
        let s = |i| ui.get(6, 6, i);
        let main = &mut self.menus[menu::MAIN].entries;
        *main = vec![
            Entry::new(s(1), 0, menu::NEGOTIATE as i32),
            Entry::new(s(2), 0, menu::TALK as i32),
            Entry::new(s(3), 0, menu::OFFER as i32),
            Entry::new(s(4), 0, menu::FOR as i32),
            Entry::new(s(5), 0, menu::OFFER as i32),
            Entry::new(s(6), 0, -1),
        ];
        let trade = rel.trade != 0;
        let n = |i| ui.get(6, 8, i);
        let neg: Vec<Entry> = match rel.level {
            1 => vec![Entry::new(n(1), 0, -1), Entry::new(n(2), 1, -1)],
            2 => vec![Entry::new(n(2), 1, -1)],
            3 => vec![Entry::new(n(3), 2, -1), Entry::new(n(4), 3, -1), Entry::new(n(5), 7, menu::ALLIES as i32)],
            4 => vec![Entry::new(n(3), 2, -1), Entry::new(n(6), 4, -1), Entry::new(n(7), 5, -1), Entry::new(n(5), 7, menu::ALLIES as i32), Entry::new(n(8), 8, -1)],
            5 => vec![Entry::new(n(3), 2, -1), Entry::new(n(9), 6, -1), Entry::new(n(5), 7, menu::ALLIES as i32), Entry::new(n(8), 8, -1)],
            _ => Vec::new(),
        };
        let keep = neg.len().saturating_sub((trade && rel.level >= 4) as usize);
        self.menus[menu::NEGOTIATE].entries = neg.into_iter().take(keep).collect();
        let tt = |i| ui.get(6, 9, i);
        let birthday = v.anniversary.get(e) == Some(&v.month) && v.greeted.get(e) == Some(&0);
        let talk = menu::TALK_EMPIRES as i32;
        self.menus[menu::TALK].entries = match rel.level {
            1 => vec![Entry::new(tt(1), 0, -1), Entry::new(tt(2), 1, -1), Entry::new(tt(3), 4, -1)],
            2 => vec![Entry::new(tt(1), 0, -1), Entry::new(tt(2), 1, -1), Entry::new(tt(4), 3, -1), Entry::new(tt(5), 6, talk), Entry::new(tt(3), 4, -1)],
            3 => vec![Entry::new(tt(6), 2, -1), Entry::new(tt(4), 3, -1), Entry::new(tt(5), 6, talk), Entry::new(tt(3), 4, -1), Entry::new(tt(2), 1, -1)],
            4 => vec![Entry::new(tt(6), 2, -1), Entry::new(tt(4), 3, -1), Entry::new(tt(5), 6, talk), Entry::new(tt(7), 9, -1)],
            5 => {
                let mut l = vec![Entry::new(tt(6), 2, -1), Entry::new(tt(4), 3, -1), Entry::new(tt(8), 6, talk)];
                if birthday {
                    l.push(Entry::new(tt(9), 8, -1));
                }
                l
            }
            _ => Vec::new(),
        };
        // The offer side (menu 1) and the exchange side (0x15): money, info, planets, the twelve tech areas.
        let areas: Vec<Vec<u8>> = (1..=12).map(|i| ui.get(4, 3, i)).collect();
        let credits = |k: usize| {
            ["10.000 ", "20.000 ", "50.000 ", "100.000 ", "200.000 ", "400.000 "].iter().enumerate().map(|(i, m)| Entry::new([m.as_bytes(), &ui.get(6, 4, k as u16)].concat(), i as i32, -1)).collect::<Vec<_>>()
        };
        let mut offer = vec![Entry::new(ui.get(6, 10, 1), 0, menu::MONEY as i32), Entry::new(ui.get(6, 10, 4), 300, menu::INFO as i32), Entry::new(ui.get(6, 10, 3), 0, menu::PLANETS as i32)];
        let mut exch = vec![Entry::new(ui.get(6, 10, 1), 0, menu::FOR_MONEY as i32), Entry::new(ui.get(6, 10, 4), 300, menu::FOR_INFO as i32), Entry::new(ui.get(6, 10, 3), 0, menu::FOR_PLANETS as i32)];
        for (i, a) in areas.iter().enumerate() {
            offer.push(Entry::new(a.clone(), 0, (menu::TECHS + i) as i32));
            exch.push(Entry::new(a.clone(), 0, (menu::FOR_TECHS + i) as i32));
        }
        self.menus[menu::OFFER].entries = offer;
        self.menus[menu::FOR].entries = exch;
        self.menus[menu::MONEY].entries = credits(13);
        self.menus[menu::FOR_MONEY].entries = credits(11);
        // Planets: the human's own (menu 4), the empire's that the human sees (0x18).
        let m = &state.mem;
        let planets = m.i32(va::PLANET_COUNT).max(0) as usize;
        let planet = |p: usize| m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE);
        let cname = |b: &[u8], n: usize| b[..b.iter().take(n).position(|&c| c == 0).unwrap_or(n.min(b.len()))].to_vec();
        let mut own = Vec::new();
        let mut theirs = Vec::new();
        for p in 1..=planets {
            let r = planet(p);
            let seen = i16::from_le_bytes([r[0xd8 + 2 * en], r[0xd9 + 2 * en]]);
            if r[0x20] as usize == en {
                own.push(Entry::new(cname(&r, 0x14), p as i32, -1));
            }
            if r[0x20] as usize == e && seen != 0 {
                theirs.push(Entry::new(cname(&r, 0x14), p as i32, -1));
            }
        }
        self.menus[menu::PLANETS].entries = own;
        self.menus[menu::FOR_PLANETS].entries = theirs;
        // Techs (`Talalmanyok`: status `+0x45 + e`, seen `+0xc0 + 9a + b`).
        let tx = t;
        let tech = |t: usize, o: usize| m.u8(va::TECHS + (t * TECH_SIZE + o) as u32);
        for a in 0..12 {
            let (mut give, mut take) = (Vec::new(), Vec::new());
            for t in a * 16 + 1..=(a * 16 + 16).min(TECHS - 1) {
                let name = tx.tech(t as i32);
                let mine = tech(t, 0x45 + en) == 5;
                let seen = tech(t, 0xc0 + 9 * en + e) & 1 != 0;
                if mine && !seen {
                    give.push(Entry::new(name.clone(), t as i32, -1));
                }
                if !mine && seen {
                    take.push(Entry::new(name, t as i32, -1));
                }
            }
            self.menus[menu::TECHS + a].entries = give;
            self.menus[menu::FOR_TECHS + a].entries = take;
        }
        // Hostages: the human's family member held by the selected empire, and the reverse.
        let kidnap = |x: usize| (m.i32(va::KIDNAP + 4 * x as u32), m.i32(va::KIDNAP + 0x24 + 4 * x as u32));
        let fam = |owner: usize, who: i32| t.family(owner as i32, who);
        self.menus[menu::HOSTAGE].entries = match kidnap(e) {
            (who, by) if who > 0 && by == en as i32 => vec![Entry::new(fam(e, who), who, -1)],
            _ => Vec::new(),
        };
        self.menus[menu::FOR_HOSTAGE].entries = match kidnap(en) {
            (who, by) if who > 0 && by == e as i32 => vec![Entry::new(fam(en, who), who, -1)],
            _ => Vec::new(),
        };
        // Empire lists: the living contacts other than the selected one (and with the levels the menu needs).
        let contacts: Vec<usize> = (0..v.slots.len()).filter(|&i| v.alive_slot(i)).map(|i| v.empire(i).max(0) as usize).collect();
        let name = |x: usize| names.get(x).cloned().unwrap_or_default();
        let list = |f: &dyn Fn(usize) -> bool| contacts.iter().copied().filter(|&x| x != e && f(x)).map(|x| Entry::new(name(x), x as i32, -1)).collect::<Vec<_>>();
        self.menus[menu::TALK_EMPIRES].entries = list(&|_| true);
        self.menus[menu::ALLIES].entries = list(&|x| v.rel(en, x).level != 1 && v.rel(e, x).level != 1);
        let mut info = vec![Entry::new(name(en), en as i32, -1)];
        info.extend(list(&|x| x != en && !v.knows(e, x)));
        self.menus[menu::INFO].entries = info;
        self.menus[menu::FOR_INFO].entries = contacts.iter().map(|&x| Entry::new(name(x), x as i32, -1)).collect();
    }

    /// The item text of a picked entry (`GetDemandMenuSzoveg`, the offer side's own builder).
    fn item_text(&self, t: &Texts, p: Pick) -> Vec<u8> {
        let text = self.menus[p.menu].entries.get(p.row).map_or(Vec::new(), |e| e.text.clone());
        let pre = |k: usize| [t.phrase(k), &text].concat();
        match p.menu {
            menu::MONEY | menu::FOR_MONEY => [MONEY[p.row.min(5)].to_string().as_bytes(), t.phrase(5)].concat(),
            menu::HOSTAGE | menu::FOR_HOSTAGE => pre(2),
            menu::PLANETS | menu::FOR_PLANETS => pre(3),
            menu::INFO | menu::FOR_INFO => pre(11),
            x if (menu::TECHS..menu::TECHS + 12).contains(&x) || (menu::FOR_TECHS..menu::FOR_TECHS + 12).contains(&x) => pre(4),
            _ => text,
        }
    }

    /// One frame of `DiplomGfx2D_1`'s input and dialogue (§7) and the head picking of `DiplomGfx2D_2` (§5). Returns
    /// the commands for the lockstep queue.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, v: &View, state: &SaveState, ui: &UiTexts, t: &Texts, names: &[Vec<u8>], heads: &[[f32; 3]], inp: &Input) -> Vec<Command> {
        let mut out = Vec::new();
        self.shown.clear();
        self.hover.clear();
        self.own_text.clear();
        self.other_text.clear();
        // `CheckAndColorize`: a press within 40 px of a living empire's head selects it.
        if inp.pressed {
            for k in 0..v.slots.len().min(HEADS) {
                let Some(h) = heads.get(k) else { continue };
                let x = h[0] + 340.0 - if k > 3 { 30.0 } else { 0.0 };
                let (dx, dy) = (x - inp.at[0], h[1] + 240.0 - inp.at[1]);
                if k as i32 != self.selected && v.alive_slot(k) && (dx * dx + dy * dy).sqrt() < 40.0 {
                    self.selected = k as i32;
                    self.cascade = None;
                    self.proposal = None;
                    self.exchange = None;
                    self.counter = None;
                    self.cache = None;
                    if self.turn >= 30.0 {
                        self.turn = 0.0;
                    }
                    return out;
                }
            }
        }
        if self.selected < 0 || self.selected as usize >= v.slots.len() {
            return out;
        }
        let k = self.selected as usize;
        let s = v.slots[k];
        let e = s.empire.max(0) as usize;
        let blocked = v.raw_u8(k, 0x15) != 0;
        let cooled = s.sent_at + 0xfa - if v.tutorial { 200 } else { 0 } < v.ido;
        self.fill(v, ui, t, names, state, k);
        let race1 = t.race(v.races[e] as i32);
        let reason_of = |cat: i32| own_reason(v, e, cat);
        let mut busy = false;
        // 1. Composing (main menu, or the exchange side of an "offer … for …").
        if s.waiting == 0 && s.asked == 0 && cooled && s.answered == 0 && s.countered == 0 && self.proposal.is_none() {
            busy = true;
            if blocked {
                self.menus[menu::BLOCKED].entries = vec![Entry::new(ui.get(6, 6, 1), 0, menu::NEGOTIATE as i32)];
                if let Some((_, _, p)) = self.run_cascade(menu::BLOCKED, inp) {
                    self.propose_negotiate(v, t, &race1, e, p, reason_of(0));
                }
            } else if let Some((first, first_text)) = self.exchange.clone() {
                // `Exchangemode`: "Offer <item> for:" opens the exchange side; "cancel exchange" drops it.
                let label = [ui.get(6, 10, 8).as_slice(), b" ", &first_text, b" ", &ui.get(6, 10, 7)].concat();
                self.menus[menu::EXCHANGE].entries[0].text = label;
                let m = self.metrics;
                self.menus[menu::EXCHANGE].fit(&m);
                if inp.pressed && self.menus[menu::EXCHANGE].row_at(&m, inp.at) == Some(1) {
                    self.exchange = None;
                } else if let Some((_, _, p)) = self.run_cascade(menu::EXCHANGE, inp) {
                    if p.menu != menu::EXCHANGE {
                        let (a, b) = (item_terms(&self.menus, first.menu, first.row), item_terms(&self.menus, p.menu, p.row));
                        let second = self.item_text(t, p);
                        let reason = reason_of(1);
                        let lines = self.text(&t.files[0], [1, 0, reason, -1], [&race1, &first_text, &second], false, -1);
                        self.proposal = Some(Proposal { declaration: false, terms: Terms { cat: 1, sub: a.0, value: a.1, sub2: b.0, value2: b.1 }, lines, greeting: false });
                        self.exchange = None;
                    }
                }
            } else if let Some((row, _, p)) = self.run_cascade(menu::MAIN, inp) {
                let reason = reason_of(if row <= 1 { row as i32 * 6 } else { row as i32 - 1 });
                match row {
                    0 => self.propose_negotiate(v, t, &race1, e, p, reason),
                    1 => {
                        let sub = if p.menu == menu::TALK { self.menus[menu::TALK].entries[p.row].value } else { 6 };
                        let (value, p2) = if p.menu == menu::TALK_EMPIRES {
                            let x = self.menus[p.menu].entries[p.row].value;
                            (x, t.race(v.races[x.clamp(0, 18) as usize] as i32))
                        } else {
                            (-1, Vec::new())
                        };
                        let age = format!("{}.", v.ruler_age.get(e).copied().unwrap_or(0)).into_bytes();
                        let p3 = if sub == 8 { age } else { Vec::new() };
                        let lines = self.text(&t.files[0], [6, sub, reason, -1], [&race1, &p2, &p3], false, -1);
                        self.proposal = Some(Proposal { declaration: true, terms: Terms { cat: 6, sub, value, sub2: -1, value2: -1 }, lines, greeting: sub == 8 });
                    }
                    2 => self.exchange = Some((p, self.item_text(t, p))),
                    3 | 4 => {
                        let (sub, value) = item_terms(&self.menus, p.menu, p.row);
                        let item = self.item_text(t, p);
                        let cat = if row == 3 { 2 } else { 3 };
                        let lines = self.text(&t.files[0], [cat, 0, reason, -1], [&race1, &item, b""], false, -1);
                        self.proposal = Some(Proposal { declaration: false, terms: Terms { cat, sub, value, sub2: -1, value2: -1 }, lines, greeting: false });
                    }
                    _ => {}
                }
            }
        }
        // 2. "Send it now!" / "Cancel" under the composed proposal.
        if !busy && s.waiting == 0 && s.answered == 0 {
            if let Some(p) = self.proposal.clone() {
                self.own_text = p.lines.clone();
                match self.run_plain(menu::SEND, inp) {
                    Some(0) => {
                        out.push(Command::DiplomacyScreen { empire: e as u8, op: DiplomacyScreenOp::Sent });
                        if p.greeting {
                            out.push(Command::DiplomacyScreen { empire: e as u8, op: DiplomacyScreenOp::Greeted });
                        }
                        out.push(Command::DiplomacyMessage { to: e as i32, declaration: p.declaration, terms: p.terms });
                        self.proposal = None;
                    }
                    Some(1) => self.proposal = None,
                    _ => {}
                }
                busy = true;
            }
        } else if busy {
            if let Some(p) = &self.proposal {
                self.own_text = p.lines.clone();
                let _ = self.run_plain(menu::SEND, inp);
            }
        }
        // 3. A message of this empire waits for the human's answer.
        if !busy && s.waiting != 0 && s.answered == 0 {
            busy = true;
            let msg = s.letter.msg;
            self.other_text = self.waiting_text(v, t, names, k);
            let answer = |yes: bool, counter: bool, extra: Option<(i32, i32)>| {
                let mut terms = Terms { cat: msg.cat, sub: msg.sub, value: msg.value, sub2: msg.sub2, value2: msg.value2 };
                if let Some((a, b)) = extra {
                    terms = Terms { sub: msg.sub2, value: msg.value2, sub2: a, value2: b, ..terms };
                }
                [
                    Command::DiplomacyScreen { empire: e as u8, op: DiplomacyScreenOp::Answered },
                    Command::DiplomacyAnswer { to: e as i32, terms, counter, reply: false, yes },
                ]
            };
            if let Some((slot, chosen)) = self.counter.clone().filter(|c| c.0 == k) {
                // "Okay, but we want … instead!": name the item, then send the counter-proposal.
                match chosen {
                    None => {
                        let m = self.metrics;
                        self.menus[menu::EXCHANGE].entries[0].text = ui.get(6, 10, 5);
                        self.menus[menu::EXCHANGE].fit(&m);
                        if let Some((_, _, p)) = self.run_cascade(menu::EXCHANGE, inp) {
                            if p.menu != menu::EXCHANGE {
                                self.counter = Some((slot, Some((p, self.item_text(t, p)))));
                            }
                        }
                    }
                    Some((p, item)) => {
                        self.own_text = vec![[t.phrase(7), &item, b"..."].concat(), t.phrase(8).to_vec()];
                        match self.run_plain(menu::SEND, inp) {
                            Some(0) => {
                                out.extend(answer(true, true, Some(item_terms(&self.menus, p.menu, p.row))));
                                self.counter = None;
                            }
                            Some(1) => self.counter = None,
                            _ => {}
                        }
                    }
                }
            } else if !terms_hold(v, state, k) {
                if self.run_plain(menu::SORRY, inp) == Some(0) {
                    out.extend(answer(false, false, None));
                }
            } else {
                let deal = msg.cat == 1 || msg.cat == 2 || msg.cat == 3;
                let id = if msg.cat == 0 && matches!(msg.sub, 2 | 4 | 6) || msg.cat == 6 && matches!(msg.sub, 0 | 1 | 2 | 3 | 4 | 6 | 8 | 9) {
                    menu::OK
                } else if deal {
                    menu::DEAL_REPLY
                } else {
                    menu::COUNTER_REPLY
                };
                if id == menu::DEAL_REPLY && msg.cat == 1 {
                    // The "okay, but …" entry is offered for an offer-for (`FeltoltMenupontokPre` keeps two entries
                    // and the frame's `Igazitmenu(0x15, 0x2b)` opens the third's menu) [inf].
                    self.menus[menu::DEAL_REPLY].entries.truncate(2);
                }
                match self.run_plain(id, inp) {
                    Some(0) => out.extend(answer(true, false, None)),
                    Some(1) if id != menu::OK => out.extend(answer(false, false, None)),
                    _ => {}
                }
            }
        }
        // 4. The answer to the human's message arrived, or a counter-answer.
        if !busy && s.answered != 0 {
            self.other_text = self.answer_text(v, t, names, k);
            if self.run_plain(menu::OK, inp) == Some(0) {
                out.push(Command::DiplomacyScreen { empire: e as u8, op: DiplomacyScreenOp::AnswerSeen });
            }
        } else if !busy && s.countered != 0 {
            self.other_text = self.text(&[], [0, 0, -1, -1], [b"", b"", b""], false, -1);
            let c = s.counter_answer;
            if let Some(r @ (0 | 1)) = self.run_plain(menu::COUNTER_REPLY, inp) {
                let terms = Terms { cat: c.msg.cat, sub: c.msg.sub, value: c.msg.value, sub2: c.msg.sub2, value2: c.msg.value2 };
                out.push(Command::DiplomacyScreen { empire: e as u8, op: DiplomacyScreenOp::CounterSeen });
                out.push(Command::DiplomacyAnswer { to: e as i32, terms, counter: false, reply: true, yes: r == 0 });
            }
        }
        if inp.released {
            self.cascade = None;
        }
        out
    }

    fn propose_negotiate(&mut self, v: &View, t: &Texts, race1: &[u8], _e: usize, p: Pick, reason: i32) {
        let (sub, value, p2) = if p.menu == menu::ALLIES {
            // `Faj_Nev` indexed by the third empire's number (the original's indexing).
            let x = self.menus[p.menu].entries[p.row].value;
            (7, x, t.race(x))
        } else {
            (self.menus[menu::NEGOTIATE].entries.get(p.row).map_or(0, |e| e.value), -1, Vec::new())
        };
        let _ = v;
        let lines = self.text(&t.files[0], [0, sub, reason, -1], [race1, &p2, b""], false, -1);
        self.proposal = Some(Proposal { declaration: true, terms: Terms { cat: 0, sub, value, sub2: -1, value2: -1 }, lines, greeting: false });
    }

    /// `GetSzoveg` with the screen's generator, the same variant while the key stays.
    fn text(&mut self, buf: &[u8], key: [i32; 4], params: [&[u8]; 3], fallback: bool, seed: i32) -> Vec<Vec<u8>> {
        let id = [key.to_vec(), vec![seed, buf.len() as i32]].concat();
        if let Some((k, lines)) = &self.cache {
            if *k == id && seed != -1 {
                return lines.clone();
            }
        }
        let mut rng = self.rng;
        let mut pick = |n: usize| {
            rng = rng.wrapping_mul(214013).wrapping_add(2531011);
            if n == 0 {
                0
            } else {
                ((rng >> 16) & 0x7fff) as usize % n
            }
        };
        let lines = get_szoveg(buf, key, params, fallback, seed, &mut pick);
        self.rng = rng;
        let _ = self.pick(1);
        self.cache = Some((id, lines.clone()));
        lines
    }

    /// The waiting message's text (`GetSzoveg(1, dipUzenetSzoveg, …)`, §7.3).
    fn waiting_text(&mut self, v: &View, t: &Texts, names: &[Vec<u8>], k: usize) -> Vec<Vec<u8>> {
        let s = v.slots[k];
        let m = s.letter.msg;
        let (reason, variant) = (v.raw_i32(k, 0x5c), v.raw_i32(k, 0x60));
        let name = |x: i32| names.get(x.clamp(0, 18) as usize).cloned().unwrap_or_default();
        let item = |kind: i32, value: i32, family_of: i32| -> Vec<u8> {
            match kind {
                0 => [value.to_string().as_bytes(), t.phrase(5)].concat(),
                2 => planet_name(v, value),
                3 => t.tech(value),
                6 => [t.phrase(1), &name(value)].concat(),
                5 => [t.phrase(0), &name(value)].concat(),
                4 => [t.phrase(2), &t.family(family_of, value)].concat(),
                1 => [t.phrase(11), &name(value)].concat(),
                _ => Vec::new(),
            }
        };
        let (p2, p3) = match m.cat {
            0 => (if m.sub == 7 { t.race(m.value) } else { Vec::new() }, Vec::new()),
            1 => (item(m.sub, m.value, s.letter.from), item(m.sub2, m.value2, s.letter.to)),
            2 | 3 => (item(m.sub, m.value, s.letter.to), Vec::new()),
            6 if m.sub == 8 => (format!("{}.", v.ruler_age.get(v.en).copied().unwrap_or(0)).into_bytes(), Vec::new()),
            6 if m.sub == 6 => (name(m.value), Vec::new()),
            _ => (Vec::new(), Vec::new()),
        };
        let p1 = t.race(v.en as i32);
        let key = match m.cat {
            0 | 6 => [m.cat, m.sub, reason, -1],
            c => [c, 0, reason, -1],
        };
        self.text(&t.files[1], key, [&p1, &p2, &p3], false, variant)
    }

    /// The arrived answer's text (`GetSzoveg(2, ValaszSzoveg, cat, sub, yes, reason, …, fallback, +0x8c)`, §7.4).
    fn answer_text(&mut self, v: &View, t: &Texts, names: &[Vec<u8>], k: usize) -> Vec<Vec<u8>> {
        let s = v.slots[k];
        let a = s.answer;
        let (reason, variant) = (v.raw_i32(k, 0x90), v.raw_i32(k, 0x8c));
        let name = |x: i32| names.get(x.clamp(0, 18) as usize).cloned().unwrap_or_default();
        let _ = name;
        let sub = if matches!(a.msg.cat, 1..=3) { 0 } else { a.msg.sub };
        let param = |kind: i32, value: i32| -> Vec<u8> {
            let value = value.max(0);
            match kind {
                0 => [value.to_string().as_bytes(), t.phrase(5)].concat(),
                1 => [t.phrase(11), &t.race(value)].concat(),
                2 => [t.phrase(3), &planet_name(v, value)].concat(),
                3 => [t.phrase(4), &t.tech(value)].concat(),
                4 => [t.phrase(2), &t.family(v.en as i32, value)].concat(),
                5 => [t.phrase(0), &t.race(value)].concat(),
                6 => [t.phrase(1), &t.race(value)].concat(),
                _ => Vec::new(),
            }
        };
        let (p2, p3) = match a.msg.cat {
            0 if a.msg.sub == 7 => (t.race(a.msg.value), Vec::new()),
            6 if a.msg.sub == 6 => (t.race(a.msg.value), Vec::new()),
            1 => (param(a.msg.sub, a.msg.value), param(a.msg.sub2, a.msg.value2)),
            2 | 3 => (param(a.msg.sub, a.msg.value), Vec::new()),
            _ => (Vec::new(), Vec::new()),
        };
        let p1 = t.race(v.en as i32);
        self.text(&t.files[2], [a.msg.cat, sub, a.yes as i32, reason], [&p1, &p2, &p3], true, variant)
    }

    /// Draw the 2D part (`DiplomGfx2D_1/2`, the menus' passes, §3 faders, §5, §6).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, design: Option<TexId>, vv: &Virtual, palette: &[[f32; 3]], v: &View, t: &Texts, ui: &UiTexts, names: &[Vec<u8>], rules: &ig2_rules::Rules, state: &SaveState) {
        let d = &rules.diplomacy_screen;
        let col = |i: i32, k: f32| palette.get(i.max(0) as usize).copied().unwrap_or([1.0; 3]).map(|c| c * k);
        let fill = [0.0, 54.0, 90.0];
        let edge = [0.0, 90.0, 130.0];
        let fader = |f: &mut Frame, r: [f32; 4], ft: f32, et: f32| design_fader(f, design, white, vv, r, fill, ft, edge, et);
        let [ix, iy] = d.info_xy.map(|x| x as f32);
        let [mx, _] = d.menu_xy.map(|x| x as f32);
        // The faders of `Diplomacy3DUpdate` (open: f = 1).
        fader(f, [ix - 10.0, iy - 10.0, 172.0, 100.0], 255.0 - self.info_fade, 255.0 - 2.0 * self.info_fade);
        let lines = |n: usize| (14 * n + 10) as f32;
        if self.own_fade > 0.0 {
            fader(f, [160.0, 0x18b as f32 - 9.0, 465.0, lines(self.own_text.len().max(1))], 255.0 - self.own_fade, 255.0 - 2.0 * self.own_fade);
        }
        if self.other_fade > 0.0 {
            fader(f, [mx - 23.0, iy + 15.0 - 25.0, 440.0, lines(self.other_text.len().max(1))], 255.0 - self.other_fade, 255.0 - 2.0 * self.other_fade);
        }
        // The menus: frames, then texts (`KirakMenu` passes 1 and 2).
        let m = &self.metrics;
        for &id in &self.shown {
            let mn = &self.menus[id];
            fader(f, mn.frame(m), 100.0, 30.0);
        }
        for &id in &self.shown {
            let mn = &self.menus[id];
            let hovered = self.hover.iter().rev().find(|h| h.0 == id).and_then(|h| h.1);
            for (i, en) in mn.entries.iter().enumerate() {
                let per = mn.per_column(m).max(1) as usize;
                let [x, y] = mn.text_at(m, i);
                let mut c = if hovered == Some(i) { mn.hicol } else { mn.col } as i32;
                let lists = [menu::MONEY, menu::HOSTAGE, menu::PLANETS, menu::FOR_HOSTAGE, menu::FOR_PLANETS, menu::INFO, menu::FOR_INFO, menu::TALK_EMPIRES, menu::ALLIES];
                let sub = en.sub;
                let listy = sub >= 0
                    && (lists.contains(&(sub as usize)) || (menu::TECHS as i32..=0x10).contains(&sub) || (menu::FOR_TECHS as i32..=0x24).contains(&sub) || sub == 0x32);
                if listy && self.menus[sub as usize].entries.is_empty() || (id == menu::MAIN && i == 5) {
                    c = 1;
                }
                let text = if t.hungarian { a2az(&en.text) } else { en.text.clone() };
                font.text(f, vv, &text, x, y, 0, mn.font, Align::Left, col(c, self.row_bright(i % per)), None);
            }
        }
        // The texts: own at (172, 395 + 14i), the other side's at (MenukKezdX − 15, InfonevY + 14i).
        for (i, l) in self.own_text.iter().enumerate() {
            let l = if t.hungarian { a2az(l) } else { l.clone() };
            font.text(f, vv, &l, 0xac as f32, (0x18b + 14 * i) as f32, 0x50, 2, Align::Left, col(2, self.own_fade / 127.0), None);
        }
        for (i, l) in self.other_text.iter().enumerate() {
            let l = if t.hungarian { a2az(l) } else { l.clone() };
            font.text(f, vv, &l, mx - 15.0, iy + (14 * i) as f32, 0x50, 2, Align::Left, col(2, self.other_fade / 127.0), None);
        }
        // Names under the heads (and "(dead)").
        for k in 0..v.slots.len().min(HEADS) {
            let (Some(h), Some(&x)) = (d.heads.get(k), d.name_x.get(k)) else { continue };
            let e = v.empire(k);
            let y = h[1] + 240.0 + 35.0;
            let b = self.row_bright(k);
            let name = names.get(e.clamp(0, 18) as usize).cloned().unwrap_or_default();
            font.text(f, vv, &name, x as f32, y, 0x14, d.name_font as usize, Align::Centre, col(d.name_colour, b), None);
            if !v.alive_slot(k) {
                font.text(f, vv, t.phrase(6), x as f32, y + 16.0, 0, d.name_font as usize, Align::Centre, col(6, b), None);
            }
        }
        // The info panel of the selected empire.
        if self.selected >= 0 {
            let k = self.selected as usize;
            let e = v.empire(k).clamp(0, 18) as usize;
            let [c1, c2] = d.info_colours;
            let font3 = d.info_font as usize;
            let b = |r: usize| self.row_bright(r);
            let name = names.get(e).cloned().unwrap_or_default();
            font.text(f, vv, &name, ix - 3.0, iy, 0x14, font3, Align::Left, col(c1, b(0)), None);
            let label = |i: u16| [ui.get(6, 10, i).as_slice(), b":"].concat();
            font.text(f, vv, &label(9), ix - 3.0, iy + 20.0, 0xf, 2, Align::Left, col(c2, b(1)), None);
            let known = known_planets(state, v.en, e);
            font.int(f, vv, known as i64, ix + 94.0, iy + 20.0, 5, 2, Align::Left, col(c1, b(1)));
            font.text(f, vv, &label(10), ix - 3.0, iy + 40.0, 0x37, 2, Align::Left, col(c2, b(3)), None);
            let trade = v.rel(v.en, e).trade != 0;
            font.text(f, vv, &ui.get(6, 10, if trade { 11 } else { 12 }), ix + 57.0, iy + 40.0, 0x19, 2, Align::Left, col(c1, b(3)), None);
            font.text(f, vv, &label(13), ix - 3.0, iy + 50.0, 0xf, 2, Align::Left, col(c2, b(4)), None);
            let lvl = v.rel(v.en, e).level as usize;
            let status = t.status.get(lvl).cloned().unwrap_or_default();
            font.text(f, vv, &status, ix + 57.0, iy + 50.0, 0x19, 2, Align::Left, col(c1, b(4)), None);
        }
    }
}


/// `sprintf(tpl, s)` for the one `%s`.
fn sprintf_s(tpl: &[u8], s: &[u8]) -> Vec<u8> {
    match tpl.windows(2).position(|w| w == b"%s") {
        Some(p) => [&tpl[..p], s, &tpl[p + 2..]].concat(),
        None => tpl.to_vec(),
    }
}

fn planet_name(v: &View, p: i32) -> Vec<u8> {
    v.planet_names.get(p.max(0) as usize).cloned().unwrap_or_default()
}

/// `FadeKerettelKerekit` on this screen: the fill textured by `fadertexture` (`textures/design/screen/dt.bmp`, loaded
/// by `Diplomacy3DInit`; texel = virtual pixel from the panel's corner [inf]) [obs: the panels' fills are half as bright
/// as the plain fader and show the texture], then the one-pixel frame.
#[allow(clippy::too_many_arguments)]
fn design_fader(f: &mut Frame, design: Option<TexId>, white: TexId, vv: &Virtual, r: [f32; 4], fill: [f32; 3], ft: f32, edge: [f32; 3], et: f32) {
    let Some(tex) = design else {
        crate::starmap_panels::fader(f, white, vv, r, fill, ft, edge, et);
        return;
    };
    let [x, y, w, h] = r;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let a = (255.0 - ft.clamp(0.0, 255.0)) / 255.0;
    // The texture is sampled in linear light (its mean 0.5 reads as 0.22): ×1.5 brings the fill to the exe's half
    // brightness over black [obs].
    let c = crate::starmap_hud::srgb_blend(fill, a);
    let c = [c[0] * 1.5, c[1] * 1.5, c[2] * 1.5, c[3]];
    let mut q = crate::render::Instance::new(crate::render::kind::SPRITE, vv.px(x + w / 2.0, y + h / 2.0), [w * vv.k / 2.0, h * vv.k / 2.0], c);
    q.uv = [0.0, 0.0, (w / 256.0).min(1.0), (h / 256.0).min(1.0)];
    f.push(tex, crate::render::Blend::Alpha, q);
    crate::starmap_panels::fader(f, white, vv, r, [0.0; 3], 255.0, edge, et);
}

/// `AjanlatFeltetelekOK` (ELF 0x51ce50): a demanded tech the human doesn't have, or an offered tech the sender no
/// longer has, voids the terms.
fn terms_hold(v: &View, state: &SaveState, k: usize) -> bool {
    let s = v.slots[k];
    let m = s.letter.msg;
    let has = |t: i32, e: i32| state.mem.u8(va::TECHS + (t.clamp(0, TECHS as i32 - 1) as usize * TECH_SIZE) as u32 + 0x45 + e.clamp(0, 18) as u32) == 5;
    let mut ok = true;
    if m.cat == 2 && m.sub == 3 {
        ok = has(m.value, s.letter.to);
    }
    if m.cat == 1 && m.sub2 == 3 && !has(m.value2, s.letter.to) {
        ok = false;
    }
    ok
}

/// `GetKnownPlanets`: the planets `e` owns that the human sees (`+0xd8 + 2·EN > 0`).
fn known_planets(state: &SaveState, en: usize, e: usize) -> usize {
    let m = &state.mem;
    let n = m.i32(va::PLANET_COUNT).max(0) as usize;
    (1..=n)
        .filter(|&p| {
            let r = m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE);
            r[0x20] as usize == e && i16::from_le_bytes([r[0xd8 + 2 * en], r[0xd9 + 2 * en]]) > 0
        })
        .count()
}

/// The open screen: its state, texts and 3D frame.
pub struct Open {
    pub screen: Screen,
    pub texts: Texts,
    pub scene: crate::diplomacy_scene::DiplomacyScene,
    /// `fadertexture` (`dt.bmp`).
    pub design: Option<TexId>,
}

impl Open {
    pub fn new(r: &mut crate::render::Renderer, install: &Install, rules: &ig2_rules::Rules, ui: &UiTexts, state: &SaveState) -> Self {
        let v = View::read(state);
        let design = install.resolve_texture("textures/design/screen/", "dt.bmp").and_then(|p| crate::assets::Image::load(install, &p).ok()).map(|img| r.add_texture(&img));
        Self { screen: Screen::new(rules, ui, &v), texts: Texts::load(install, ui), scene: crate::diplomacy_scene::DiplomacyScene::load(r, install, rules), design }
    }

    /// One frame's input and dialogue: the commands for the lockstep queue.
    pub fn input(&mut self, state: &SaveState, hud: &crate::ui::Hud, rules: &ig2_rules::Rules, inp: &Input) -> Vec<Command> {
        let v = View::read(state);
        self.screen.frame(&v, state, &hud.texts, &self.texts, &hud.empire_names, &rules.diplomacy_screen.heads, inp)
    }

    /// Draw the screen (3D frame, then the 2D layer) into `f`; the HUD's top bar goes over it afterwards.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, f: &mut Frame, r: &mut crate::render::Renderer, install: &Install, rules: &ig2_rules::Rules, hud: &crate::ui::Hud, state: &SaveState, size: [f32; 2], time: f32) {
        let Some(font) = &hud.font else { return };
        let v = View::read(state);
        let vv = Virtual::new(size);
        self.scene.draw(f, r, install, rules, &self.screen, &v, &vv, size, time);
        self.screen.draw(f, font, hud.white, self.design, &vv, &hud.palette, &v, &self.texts, &hud.texts, &hud.empire_names, rules, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_are_found_joined_substituted_and_wrapped() {
        let file = b"; comment\r\n#000/001/002 ; note\r\n- Hello #1#, we offer\r\n- #2# today.\r\n#000/001/002\r\n- Second.\r\n";
        let mut first = |_: usize| 0;
        let l = get_szoveg(file, [0, 1, 2, -1], [b"Godan", b"peace", b""], false, -1, &mut first);
        assert_eq!(l, vec![b"Hello Godan, we offer peace today.".to_vec()]);
        let l = get_szoveg(file, [0, 1, 2, -1], [b"", b"", b""], false, 6000, &mut first);
        assert_eq!(l, vec![b"Second.".to_vec()], "seeded: n·seed/10001 = 1");
        let none = get_szoveg(file, [9, 9, 9, -1], [b"", b"", b""], false, -1, &mut first);
        assert!(none[0].starts_with(b"Radio engineers"));
    }

    #[test]
    fn long_texts_wrap_under_seventy_columns() {
        let words = "word ".repeat(40);
        let l = wrap(words.as_bytes());
        assert!(l.len() >= 3 && l.iter().all(|x| x.len() < 70), "{l:?}");
    }

    #[test]
    fn hungarian_articles_follow_the_next_word() {
        assert_eq!(a2az(b"a\x95 Ember"), b"az Ember".to_vec());
        assert_eq!(a2az(b"a\x95 Godan"), b"a Godan".to_vec());
        assert_eq!(a2az(b"A\xB0 orsz\xE1g!"), b"AZ orsz\xE1g!".to_vec());
    }

    /// quick-1.sav: the human (6) at war with the Shinari (3). Negotiate → cease-fire, send it, the AI's answer
    /// arrives and is acknowledged; every step through the lockstep commands. Skips without install and save.
    #[test]
    fn quick1_a_cease_fire_is_proposed_sent_and_answered() {
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let mut world = crate::world::World::load(&save, &rules, &defaults, &volatile);
        let ui = UiTexts::load(&install);
        let names = crate::ui::empire_names(&crate::ui::hungarian_txt17(&install));
        let t = Texts::load(&install, &ui);
        let heads = tables.diplomacy_screen.heads.clone();
        let mut s = Screen::new(&tables, &ui, &View::read(&world.game.state));
        assert_eq!(s.selected, 0);
        let step = |s: &mut Screen, w: &crate::world::World, inp: Input| s.frame(&View::read(&w.game.state), &w.game.state, &ui, &t, &names, &heads, &inp);
        let at = |s: &Screen, id: usize, row: usize| {
            let p = s.menus[id].text_at(&s.metrics, row);
            [p[0] + 3.0, p[1] + 3.0]
        };
        // Lay the menus out, then press "Negotiate", drag into its sub-menu and release on its first entry.
        assert!(step(&mut s, &world, Input::default()).is_empty());
        let p = at(&s, menu::MAIN, 0);
        step(&mut s, &world, Input { at: p, pressed: true, held: true, ..Default::default() });
        let q = at(&s, menu::NEGOTIATE, 0);
        step(&mut s, &world, Input { at: q, held: true, ..Default::default() });
        assert!(step(&mut s, &world, Input { at: q, released: true, ..Default::default() }).is_empty());
        let prop = s.proposal.clone().expect("a proposal");
        assert_eq!((prop.terms.cat, prop.terms.sub, prop.declaration), (0, 0, true));
        assert!(!prop.lines.is_empty());
        // "Send it now!".
        step(&mut s, &world, Input::default());
        let send = at(&s, menu::SEND, 0);
        let out = step(&mut s, &world, Input { at: send, pressed: true, held: true, ..Default::default() });
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(matches!(out[1], Command::DiplomacyMessage { to: 3, declaration: true, terms: Terms { cat: 0, sub: 0, .. } }));
        for c in out {
            world.send(c);
        }
        world.pump();
        let v = View::read(&world.game.state);
        let slot = v.slots[0];
        assert_eq!((slot.asked, slot.sent_at), (0, v.ido), "the AI answered at once (FeldolgozAjanlat), clearing +0x16");
        assert_eq!(slot.answered, 1);
        // The answer's text and its OK.
        step(&mut s, &world, Input::default());
        assert!(!s.other_text.is_empty() && s.shown.contains(&menu::OK));
        let ok = at(&s, menu::OK, 0);
        for c in step(&mut s, &world, Input { at: ok, pressed: true, held: true, ..Default::default() }) {
            world.send(c);
        }
        world.pump();
        assert_eq!(View::read(&world.game.state).slots[0].answered, 0);
    }

    /// quick-1.sav: the Shinari (3) offer a cease-fire; the screen shows their text with "we agree / forget it", and
    /// agreeing answers yes through the queue (the waiting flag cleared first). Skips without install and save.
    #[test]
    fn quick1_an_ai_offer_is_shown_and_accepted() {
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let mut world = crate::world::World::load(&save, &rules, &defaults, &volatile);
        let ui = UiTexts::load(&install);
        let names = crate::ui::empire_names(&crate::ui::hungarian_txt17(&install));
        let t = Texts::load(&install, &ui);
        let heads = tables.diplomacy_screen.heads.clone();
        // The AI's message to the human (`FeldolgozAjanlat` stores it in the human's slot, §5.4).
        Command::DiplomacyMessage { to: 6, declaration: false, terms: Terms { cat: 0, sub: 0, value: -1, sub2: -1, value2: -1 } }.apply(&mut world.game, 3);
        let v = View::read(&world.game.state);
        assert_eq!(v.slots[0].waiting, 1);
        let mut s = Screen::new(&tables, &ui, &v);
        let step = |s: &mut Screen, w: &crate::world::World, inp: Input| s.frame(&View::read(&w.game.state), &w.game.state, &ui, &t, &names, &heads, &inp);
        assert!(step(&mut s, &world, Input::default()).is_empty());
        assert!(!s.other_text.is_empty());
        assert!(s.shown.contains(&menu::COUNTER_REPLY), "{:?}", s.shown);
        let p = s.menus[menu::COUNTER_REPLY].text_at(&s.metrics, 0);
        let out = step(&mut s, &world, Input { at: [p[0] + 3.0, p[1] + 3.0], pressed: true, held: true, ..Default::default() });
        assert!(matches!(out[0], Command::DiplomacyScreen { empire: 3, op: DiplomacyScreenOp::Answered }));
        assert!(matches!(out[1], Command::DiplomacyAnswer { to: 3, counter: false, reply: false, yes: true, .. }), "{out:?}");
        for c in out {
            world.send(c);
        }
        world.pump();
        let v = View::read(&world.game.state);
        assert_eq!(v.slots[0].waiting, 0);
        assert_eq!(v.rel(6, 3).level, 2, "the cease-fire holds");
    }

    #[test]
    fn sub_menus_sit_beside_their_entry_and_stay_on_screen() {
        let m = Metrics { width: [0, 6, 6, 8, 12], height: [0, 5, 7, 10, 12], rows: 37, col_gap: 5 };
        let mut menus = vec![Menu::default(); menu::COUNT];
        menus[0] = Menu { font: 3, gap: 6, x: 25, y: 371, entries: (0..6).map(|i| Entry::new(b"Negotiate".to_vec(), 0, if i == 1 { 0x13 } else { -1 })).collect(), ..Menu::default() };
        menus[0].fit(&m);
        assert_eq!(menus[0].y, 0x1c1 - 16 * 6, "six rows of 16 end at 449");
        menus[0x13].entries = (0..3).map(|_| Entry::new(b"Insult".to_vec(), 0, -1)).collect();
        place(&mut menus, &m, 0x13, 0);
        assert_eq!(menus[0x13].x, 8 * 9 + 5 - 5 + 25 + 5);
        assert_eq!(menus[0].frame(&m), [21.0, 349.0, 8.0 * 9.0 + 7.0, 101.0]);
        assert_eq!(menus[0].row_at(&m, [30.0, 353.0 + 16.0]), Some(1));
    }
}

