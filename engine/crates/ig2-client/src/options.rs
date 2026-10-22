//! The Options screen (`AktKep` 13, re/screens/options.md): the sound and game pages (the music/sound sliders, the
//! option rows), the 24 save slots, load, save with its name entry, and quit with its question. Its state is the
//! exe's globals (`OptionsMelyikMod`, `OptCsuszkak`, `SaveAktivSlot`, `SaveLeiro`, `OptionsCsakLoadLehet`, …), its
//! input `Options_Main` (exe 0x51a320) over the hot-spots of `Options_MouseInit`/`MouseInit2` (exe 0x51ace0 /
//! 0x51a8a0), its clock `Frissit2_Options` (ELF 0x5da990) and its draw `OptionsGfx2D_1` (ELF 0x5d49a0, the faders and
//! lasers) and `OptionsGfx2D_2` (exe 0x518a30, the texts, slots and slider tracks).
//!
//! Every action fires on the press: a box set with the "release" flag only waits for the button to come up
//! (`IG2_gameUpdate`'s `Mouse_FelengedesMajdMegvar`). The hot-spot under the pointer is the lowest id that holds it
//! (`Mouse_Darabol`). The tutorial layout (wider quit, "continue tutorial") waits for the tutorial overlay.
//!
//! Slots: the original's `save<n>.sav` in its save folder (`GetSavePath`, the folder of `options.dat`) are listed and
//! loaded; the remake saves its own format (ADR 0007, `.ig2s`) as `save<n>.ig2s` beside them with the slot's title
//! after the state, and lists that one when both exist (re/deviations.md).

use std::path::{Path, PathBuf};

use crate::font::{Align, Font};
use crate::kit::{fader, inside, DesignFrame, Lasers, Tooltip, Virtual};
use crate::render::{kind, Blend, Frame, Instance, TexId};
use crate::ui::UiTexts;

/// The slots (`1..=0x18`); 23 and 24 can't be saved to (24 is the autosave's).
pub const SLOTS: usize = 24;
/// The option rows' table in the exe (`0x9160a0`, 17 × 0x22c).
const ROWS_VA: u32 = 0x9160a0;
const ROW: usize = 0x22c;
const ROWS: usize = 17;
/// `DrawLaser(…, 0x3c, 0x50, 0xfa, …)`.
const LASER: [i32; 3] = [0x3c, 0x50, 0xfa];

/// An option row (`KirakMenuPontok` exe 0x5171f0's table).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub name: Vec<u8>,
    pub key: String,
    pub enabled: bool,
    pub value: i32,
    pub states: i32,
    pub page: i32,
    pub id: i32,
    pub dep: i32,
    pub mp_ok: bool,
}

/// The rows from the user's exe, named as `Options_LoadStrings` (exe 0x51a2c0) names them: row k gets
/// `GetString(13, 2, k + 1)`, except rows 5 and 6, which keep the exe's own text.
pub fn read_rows(exe: &ig2_rules::PeImage, texts: &UiTexts) -> Vec<Row> {
    let Ok(b) = exe.read(ROWS_VA, ROW * ROWS) else { return Vec::new() };
    let cstr = |s: &[u8]| s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())].to_vec();
    let i32_at = |r: &[u8], o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    b.chunks_exact(ROW)
        .enumerate()
        .map(|(k, r)| Row {
            name: if k == 5 || k == 6 { cstr(&r[..0x100]) } else { texts.get(13, 2, k as u16 + 1) },
            key: String::from_utf8_lossy(&cstr(&r[0x100..0x200])).into_owned(),
            enabled: r[0x200] != 0,
            value: i32_at(r, 0x204),
            states: i32_at(r, 0x208),
            page: i32_at(r, 0x20c),
            id: i32_at(r, 0x210),
            dep: i32_at(r, 0x214),
            mp_ok: r[0x228] != 0,
        })
        .collect()
}

/// `CheckOptionsDependencies` (exe 0x516e10): a row is on with no dependency, with a positive one while that row is
/// on, with a negative one while it is off; rows not for multiplayer are off there. An unknown id reads row 0.
pub fn check_dependencies(rows: &mut [Row], multiplayer: bool) {
    let values: Vec<(i32, i32)> = rows.iter().map(|r| (r.id, r.value)).collect();
    let value_of = |id: i32| values.iter().find(|(i, _)| *i == id).or(values.first()).map_or(0, |v| v.1);
    for r in rows.iter_mut() {
        r.enabled = match r.dep {
            0 => true,
            d if d > 0 => value_of(d) != 0,
            d => value_of(-d) == 0,
        };
        if !r.mp_ok && multiplayer {
            r.enabled = false;
        }
    }
}

/// A slider (`OptCsuszkak`, 0x2a0 bytes each).
#[derive(Debug, Clone, PartialEq)]
pub struct Slider {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub value: i32,
    /// `"- "` + the six level texts, and the title.
    pub levels: [Vec<u8>; 6],
    pub title: Vec<u8>,
    pub visible: bool,
    pub enabled: bool,
}

/// A save slot's title (`SaveLeiro[n]`, `LoadTitle` exe 0x553590) and the file it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub name: Vec<u8>,
    pub date: [i32; 3],
    pub file: PathBuf,
    /// The remake's own format (`.ig2s`) rather than an original save.
    pub ours: bool,
}

/// The save folder (`GetSavePath` exe 0x5579c0: the folder of `options.dat`); `IG2_SLOT_DIR` overrides it.
pub fn slot_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("IG2_SLOT_DIR") {
        return Some(PathBuf::from(d));
    }
    ig2_formats::options::Options::path().and_then(|p| p.parent().map(Path::to_path_buf))
}

/// The slot title after the state in our `save<n>.ig2s`: this magic, the name (31 bytes, NUL padded) and the date.
const SLOT_MAGIC: &[u8; 8] = b"IG2SLOT1";
const SLOT_TRAILER: usize = 8 + 31 + 12;

/// The remake's slot file: the snapshot file (ADR 0007 §4), then the slot title.
pub fn slot_file(snapshot_file: Vec<u8>, name: &[u8], date: [i32; 3]) -> Vec<u8> {
    let mut out = snapshot_file;
    out.extend_from_slice(SLOT_MAGIC);
    let mut n = [0u8; 31];
    let len = name.len().min(30);
    n[..len].copy_from_slice(&name[..len]);
    out.extend_from_slice(&n);
    for v in date {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// The title of our slot file, if it carries one.
fn our_title(b: &[u8]) -> Option<(Vec<u8>, [i32; 3])> {
    let t = b.get(b.len().checked_sub(SLOT_TRAILER)?..)?;
    if &t[..8] != SLOT_MAGIC {
        return None;
    }
    let name = t[8..39].iter().take_while(|&&c| c != 0).copied().collect();
    let i = |o: usize| i32::from_le_bytes(t[o..o + 4].try_into().unwrap());
    Some((name, [i(39), i(43), i(47)]))
}

/// `OptionsRefreshSlots` (exe 0x5196c0): the titles of slots 1..=24 (index 0 unused).
pub fn read_slots(dir: Option<&Path>) -> Vec<Option<Slot>> {
    let mut out = vec![None; SLOTS + 1];
    let Some(dir) = dir else { return out };
    for (n, slot) in out.iter_mut().enumerate().skip(1) {
        let ours = dir.join(format!("save{n}.ig2s"));
        if let Some((name, date)) = std::fs::read(&ours).ok().and_then(|b| our_title(&b)) {
            *slot = Some(Slot { name, date, file: ours, ours: true });
            continue;
        }
        let orig = dir.join(format!("save{n}.sav"));
        let head = std::fs::File::open(&orig).ok().and_then(|mut f| {
            use std::io::Read;
            let mut b = vec![0u8; 80];
            let k = f.read(&mut b).ok()?;
            b.truncate(k);
            ig2_formats::save::Header::read(&b).ok()
        });
        if let Some(h) = head {
            let name = h.name[..30].iter().take_while(|&&c| c != 0).copied().collect();
            *slot = Some(Slot { name, date: [h.year, h.month, h.day], file: orig, ours: false });
        }
    }
    out
}

/// What the screen asks of the window once it has faded out (`Options_Kilep`'s loop), or at once.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Back to `OptionsElottiKep` (the screen it was opened from).
    Back,
    /// `AktKep = 0xf`: the main menu (from load only: the selection screen; else the quit question's YES).
    MainMenu,
    /// `LoadSave(slot, "", 1)`, then the starmap.
    Load(usize),
    /// `Options_EnterPressed`: `LoadSave(slot, name, 2)`, then back.
    Save(usize, Vec<u8>),
}

/// The save-name entry (`IOMode` with `IOAzonosito` 0x83).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub text: Vec<u8>,
    pub cursor: usize,
    /// `IOGombnyomasraTorles`: the first key clears the text.
    pub clear: bool,
}

/// The screen's hot-spot ids (`Mouse_Szoveg_Set`; the tutorial's shifted ids aren't used).
mod id {
    pub const LOAD: u32 = 1;
    pub const SAVE: u32 = 2;
    pub const QUIT: u32 = 3;
    pub const SLOTS: u32 = 4;
    pub const SLIDER: u32 = 5;
    pub const SOUND: u32 = 10;
    pub const GAME: u32 = 11;
    pub const PANEL: u32 = 17;
    pub const BACK: u32 = 18;
}

/// The settings the screen starts from (the exe's globals as `options.dat` and its image leave them).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub music: i32,
    pub sound: i32,
    pub gamma: i32,
    pub gamma_enabled: bool,
    pub page: i32,
    pub slot: usize,
    pub gfx_detail: i32,
}

#[derive(Debug, Clone)]
pub struct OptionsScreen {
    pub page: i32,
    pub sliders: [Slider; 3],
    pub slot: usize,
    pub slots: Vec<Option<Slot>>,
    pub rows: Vec<Row>,
    pub load_only: bool,
    /// `OptionsHarcFolyik`: a battle runs (no saving).
    pub battle: bool,
    pub multiplayer: bool,
    /// `OptionsAreYouSureToQuitMode`.
    pub sure: bool,
    pub entry: Option<Entry>,
    /// `OptionsFelfadeles` (0..100, +0.6 a tick once drawn), `OptionsVoltElsoframe`.
    pub fade: f32,
    drawn: bool,
    /// `OptionsKilepesValtozo` while leaving (−2 a tick), the frames it has been 0, and where to.
    leaving: Option<(i32, u32, Act)>,
    /// `OptionsGraphicsDetail` (3 = custom after a row press).
    pub gfx_detail: i32,
    /// `OptionsFelfadelesV`, `OptionsBlokk1/2Atlatszosag` (0..100) as the last draw left them.
    fade_v: f32,
    pub blokk1: f32,
    pub blokk2: f32,
    /// The slider grabbed (`Mouse_Megfogva`): its index, `CsuszXStart`, `StartBarPos`.
    grab: Option<(usize, i32, i32)>,
    held: bool,
    pub at: [f32; 2],
    lasers: Lasers,
    design: DesignFrame,
    pub tip: Tooltip,
    /// The settings changed since the last write (`SaveGlobalRegistrySettings`).
    pub dirty: bool,
    ui: u64,
}

fn level_texts(texts: &UiTexts, idx: [u16; 6]) -> [Vec<u8>; 6] {
    idx.map(|i| {
        let mut s = b"- ".to_vec();
        s.extend_from_slice(&texts.get(13, 4, i));
        s
    })
}

impl OptionsScreen {
    /// `Options_Belep` (exe 0x519790): the sliders from the settings, the slots read, the rows' dependencies.
    pub fn enter(texts: &UiTexts, mut rows: Vec<Row>, s: &Settings, load_only: bool, multiplayer: bool, slots: Vec<Option<Slot>>) -> Self {
        let slider = |y: i32, value: i32, levels: [u16; 6], title: u16| Slider {
            x: 30,
            y,
            width: 0xdc,
            value,
            levels: level_texts(texts, levels),
            title: texts.get(13, 4, title),
            visible: false,
            enabled: true,
        };
        check_dependencies(&mut rows, multiplayer);
        let mut o = Self {
            page: s.page,
            sliders: [
                slider(0x6e, s.music, [1, 2, 3, 4, 5, 6], 7),
                slider(0x96, s.sound, [1, 2, 3, 4, 5, 6], 8),
                slider(0x6e, s.gamma, [9, 2, 3, 4, 5, 6], 10),
            ],
            slot: s.slot.clamp(1, SLOTS),
            slots,
            rows,
            load_only,
            battle: false,
            multiplayer,
            sure: false,
            entry: None,
            fade: 0.0,
            drawn: false,
            leaving: None,
            gfx_detail: s.gfx_detail,
            fade_v: 0.0,
            blokk1: 0.0,
            blokk2: 0.0,
            grab: None,
            held: false,
            at: [320.0, 240.0],
            lasers: Lasers::default(),
            design: DesignFrame::default(),
            tip: Tooltip::default(),
            dirty: false,
            ui: 0,
        };
        o.show_sliders(s.gamma_enabled);
        o
    }

    /// `OptionsCsuszkakKiBe` (ELF 0x5d48c0): page 0 shows music and sound, page 1 gamma (enabled as the hardware
    /// allows), page 2 none.
    fn show_sliders(&mut self, gamma_enabled: bool) {
        for (i, s) in self.sliders.iter_mut().enumerate() {
            s.visible = matches!((self.page, i), (0, 0 | 1) | (1, 2));
            s.enabled = i != 2 || gamma_enabled;
        }
    }

    pub fn settings(&self) -> Settings {
        Settings {
            music: self.sliders[0].value,
            sound: self.sliders[1].value,
            gamma: self.sliders[2].value,
            gamma_enabled: self.sliders[2].enabled,
            page: self.page,
            slot: self.slot,
            gfx_detail: self.gfx_detail,
        }
    }

    fn filled(&self, n: usize) -> bool {
        self.slots.get(n).is_some_and(Option::is_some)
    }

    /// `Options_MouseInit2` (exe 0x51a8a0): whether hot-spot `id` is on.
    fn spot_on(&self, id: u32) -> bool {
        match id {
            id::LOAD => self.filled(self.slot) && !self.sure && !self.multiplayer,
            id::SAVE => (!self.load_only && !self.battle && !self.multiplayer && self.slot < 23) || self.sure,
            id::SLOTS => self.entry.is_none(),
            5..=7 => self.sliders[(id - 5) as usize].visible,
            _ => true,
        }
    }

    /// The hot-spot rectangles (`Options_MouseInit` exe 0x51ace0; the sliders' set each `Gfx2D_2`).
    fn spot_rect(&self, id: u32) -> Option<[i32; 4]> {
        Some(match id {
            id::LOAD => [0x16e, 0x3e, 0x4e, 0x25],
            id::SAVE => [0x1c4, 0x3e, 0x4e, 0x25],
            id::QUIT => [0x21d, 0x3e, 0x4e, 0x25],
            id::SLOTS => [0x166, 0x71, 0x10d, 0x150],
            5..=7 => {
                let s = &self.sliders[(id - 5) as usize];
                [s.x - 10, s.y - 5, s.width + 10, 0x1e]
            }
            id::SOUND => [0xd, 0x42, 0x5a, 0x14],
            id::GAME => [0x6c, 0x42, 0x5a, 0x14],
            id::PANEL => [9, 0x3e, 0x120, 0x180],
            id::BACK => [0, 0, 0x159, 0x1df],
            _ => return None,
        })
    }

    /// `Mouse_Darabol` (ELF 0x5b7a40): the lowest id on whose box the pointer (clamped to the screen) rests. The
    /// boxes take `x ≤ px ≤ x + w − 1`.
    pub fn spot_at(&self, at: [f32; 2]) -> u32 {
        let p = [(at[0] as i32).clamp(0, 0x27f) as f32, (at[1] as i32).clamp(0, 0x1df) as f32];
        (1..=id::BACK).find(|&i| self.spot_on(i) && self.spot_rect(i).is_some_and(|r| inside(r, p))).unwrap_or(0)
    }

    /// The tooltip of hot-spot `id` (`GetString(13, 1, n)`; the sliders show their title; the panel none).
    pub fn tip_text(&self, texts: &UiTexts, id: u32) -> Vec<u8> {
        match id {
            id::LOAD => texts.get(13, 1, 1),
            id::SAVE => texts.get(13, 1, 2),
            id::QUIT if self.load_only => texts.get(13, 1, 13),
            id::QUIT => texts.get(13, 1, 3),
            id::SLOTS => texts.get(13, 1, 4),
            5..=7 => self.sliders[(id - 5) as usize].title.clone(),
            id::SOUND => texts.get(13, 1, 5),
            id::GAME => texts.get(13, 1, 7),
            id::BACK => texts.get(13, 1, 12),
            _ => Vec::new(),
        }
    }

    /// Leave for `act` through the fade-out (`Options_Kilep`: `KilepesValtozo = (int)OptionsFelfadeles`).
    fn leave(&mut self, act: Act) {
        if self.leaving.is_none() {
            self.leaving = Some((self.fade as i32, 0, act));
            self.dirty = true;
        }
    }

    #[cfg(test)]
    pub fn leaving(&self) -> bool {
        self.leaving.is_some()
    }

    /// One 10 ms UI tick (`Frissit2_Options`, `UpdateSparks`, `Frissit2_Grafika`'s row frame).
    pub fn tick(&mut self) {
        self.ui += 1;
        if self.drawn && self.fade < 100.0 {
            self.fade = (self.fade + 0.6).min(100.0);
        }
        if let Some((v, _, _)) = &mut self.leaving {
            *v = (*v - 2).max(0);
        }
        self.lasers.tick();
        self.design.tick();
    }

    /// The frame's end: once the fade-out has been 0 for two frames the screen is done (`Options_Kilep`'s loop).
    pub fn done(&mut self) -> Option<Act> {
        let (v, frames, _) = self.leaving.as_mut()?;
        if *v == 0 {
            *frames += 1;
        }
        if *frames > 2 {
            return self.leaving.take().map(|l| l.2);
        }
        None
    }

    /// The pointer moved (`Mouse_x/y`): the hover-only slot pick and a grabbed slider's drag (`Options_Main`).
    pub fn hover(&mut self, at: [f32; 2]) {
        self.at = at;
        self.tip.moved(at);
        if self.leaving.is_some() || self.sure {
            return;
        }
        if let Some((i, start, bar)) = self.grab {
            // `trunc((f64)((f32)(x − start) / (f32)width) · 100.0 + (f64)StartBarPos)`, then 0..=100 (asm 0x51a6c0).
            let s = &mut self.sliders[i];
            let d = (at[0] as i32 - start) as f32 / s.width as f32;
            s.value = ((d as f64) * 100.0 + bar as f64) as i32;
            s.value = s.value.clamp(0, 100);
            return;
        }
        if self.spot_at(at) == id::SLOTS {
            // `(Mouse_y − 0x82) / 0xc + 1` (C division), taken if filled or below 23, clamped to 1..=24. Rows 25..=27
            // (the box's bottom) read past the titles in the exe: not taken.
            let n = (at[1] as i32 - 0x82) / 0xc + 1;
            if n <= SLOTS as i32 && (self.filled(n.max(0) as usize) || n < 0x17) {
                self.slot = n.clamp(1, SLOTS as i32) as usize;
            }
        }
    }

    /// A left press (`Mouse_Lenyom`): `Options_Main`'s actions, the rows' toggles (`KirakMenuPontok`). With the name
    /// entry up only the slot list is off (a save press starts the entry again).
    pub fn press(&mut self, at: [f32; 2], sound: &mut dyn FnMut(crate::audio::SoundId)) -> Option<Act> {
        self.hover(at);
        self.held = true;
        if self.leaving.is_some() {
            return None;
        }
        // The rows of the page (`y..y+15`, `31 ≤ x ≤ 279`, nothing held): tested in the draw, so also under the quit
        // question.
        if self.grab.is_none() {
            if let Some(k) = self.row_at(at) {
                let r = &mut self.rows[k];
                r.value += 1;
                if r.value > r.states {
                    r.value = 0;
                }
                self.gfx_detail = 3;
                self.dirty = true;
                check_dependencies(&mut self.rows, self.multiplayer);
            }
        }
        let spot = self.spot_at(at);
        match spot {
            id::QUIT => {
                if self.sure {
                    self.sure = false;
                    return None;
                }
                sound(crate::audio::CLICK);
                if self.load_only {
                    self.leave(Act::MainMenu);
                } else {
                    self.sure = true;
                }
                return None;
            }
            id::SAVE if self.sure => {
                if !self.load_only {
                    self.leave(Act::MainMenu);
                }
                return None;
            }
            id::SAVE => {
                // The slot's name (at most 32 bytes) or "Save Game %02d"; the first key clears it.
                let text = match self.slots.get(self.slot).and_then(Option::as_ref) {
                    Some(s) => s.name.iter().take(0x20).copied().collect(),
                    None => format!("Save Game {:02}", self.slot).into_bytes(),
                };
                self.entry = Some(Entry { cursor: text.len(), text, clear: true });
                return None;
            }
            _ if self.sure => return None,
            id::LOAD => {
                self.leave(Act::Load(self.slot));
                return None;
            }
            5..=7 => {
                let i = (spot - 5) as usize;
                if self.sliders[i].enabled {
                    self.grab = Some((i, at[0] as i32, self.sliders[i].value));
                }
            }
            // `AktKep = OptionsElottiKep`: the main menu when load only.
            id::BACK if self.fade == 100.0 => self.leave(if self.load_only { Act::MainMenu } else { Act::Back }),
            id::SOUND => self.set_page(0),
            id::GAME => self.set_page(2),
            _ => {}
        }
        None
    }

    fn set_page(&mut self, p: i32) {
        self.page = p;
        let g = self.sliders[2].enabled;
        self.show_sliders(g);
    }

    /// The row of the page under `at` that a press toggles (enabled only).
    fn row_at(&self, at: [f32; 2]) -> Option<usize> {
        for (n, (k, r)) in self.rows.iter().enumerate().filter(|(_, r)| r.page == self.page).enumerate() {
            let y = self.row_y(n as i32);
            let (x, py) = (at[0] as i32, at[1] as i32);
            if r.enabled && py >= y && py < y + 0xf && (0x1f..=0x117).contains(&x) {
                return Some(k);
            }
        }
        None
    }

    fn row_y(&self, n: i32) -> i32 {
        n * 0xf + 0x5c + if self.page == 1 { 100 } else { 0 }
    }

    pub fn release(&mut self) {
        self.held = false;
        self.grab = None;
    }

    /// Esc (`ESC_Pressed` ELF 0x581440 on screen 13: back to `OptionsElottiKep`, the main menu when load only; in
    /// the name entry `Options_EscPressed`: the entry ends).
    pub fn escape(&mut self) {
        if self.entry.take().is_some() {
            return;
        }
        if self.leaving.is_none() {
            self.leave(if self.load_only { Act::MainMenu } else { Act::Back });
        }
    }

    /// A key in the name entry (`DoKeyboard` ELF 0x57fd20 with `IOMode`): a character goes in at the cursor (at the full
    /// 32 the last one drops), Backspace and Delete (the first key with `IOGombnyomasraTorles` clears the text
    /// instead), the arrows, Home and End; Enter saves (`Options_EnterPressed` exe 0x51a090: `LoadSave(slot,
    /// IOString, 2)`, then back).
    pub fn entry_key(&mut self, k: EntryKey) -> Option<Act> {
        let e = self.entry.as_mut()?;
        let clear = std::mem::take(&mut e.clear);
        if clear && matches!(k, EntryKey::Char(_) | EntryKey::Back | EntryKey::Delete) {
            e.text.clear();
            e.cursor = 0;
            if !matches!(k, EntryKey::Char(_)) {
                return None;
            }
        }
        match k {
            EntryKey::Char(c) => {
                if e.cursor < 0x20 {
                    e.text.insert(e.cursor.min(e.text.len()), c);
                    e.text.truncate(0x20);
                    e.cursor += 1;
                }
            }
            EntryKey::Back if e.cursor > 0 => {
                e.cursor -= 1;
                e.text.remove(e.cursor);
            }
            EntryKey::Delete if e.cursor < e.text.len() => {
                e.text.remove(e.cursor);
            }
            EntryKey::Left => e.cursor = e.cursor.saturating_sub(1),
            EntryKey::Right => e.cursor = (e.cursor + 1).min(e.text.len()),
            EntryKey::Home => e.cursor = 0,
            EntryKey::End => e.cursor = e.text.len(),
            EntryKey::Enter => {
                let e = self.entry.take()?;
                return Some(Act::Save(self.slot, e.text));
            }
            _ => {}
        }
        None
    }

    /// After a save: the slots again, and back to the screen before (`Options_EnterPressed`).
    pub fn saved(&mut self, slots: Vec<Option<Slot>>) {
        self.slots = slots;
        self.leave(Act::Back);
    }

    /// `Options_EnterPressed` when `LoadSave` failed: the slots again, the screen stays.
    pub fn refresh(&mut self, slots: Vec<Option<Slot>>) {
        self.slots = slots;
    }

    /// The option rows' values as `SaveOptions` writes them, and the screen's own keys
    /// (`SaveGlobalRegistrySettings` exe 0x51b4e0).
    pub fn store(&self, o: &mut ig2_formats::options::Options) {
        let s = self.settings();
        for (k, v) in [
            ("Sound Volume", s.sound),
            ("Music Volume", s.music),
            ("Gamma Correction", s.gamma),
            ("Save slot", s.slot as i32),
            ("OptionsMelyikMod", s.page),
            ("OptionsGfxDetail", s.gfx_detail),
        ] {
            o.set(k, &v.to_string());
        }
        for r in &self.rows {
            o.set(&r.key, &r.value.to_string());
        }
    }
}

/// A key for the name entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKey {
    Char(u8),
    Back,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
}

/// What a draw needs.
pub struct Draw<'a> {
    pub font: &'a Font,
    pub white: TexId,
    pub glow: TexId,
    pub v: Virtual,
    pub palette: &'a [[f32; 3]],
    pub texts: &'a UiTexts,
    /// `AktIdo` (ms), for the tooltip's wait and the cursor's blink.
    pub now_ms: f64,
}

impl Draw<'_> {
    fn col(&self, i: usize, mult: f32) -> [f32; 3] {
        self.palette.get(i).copied().unwrap_or([1.0; 3]).map(|c| c * mult / 100.0)
    }
}

/// The exe's ARGB colour as a gamma-space RGB.
fn rgb(c: [i32; 3]) -> [f32; 4] {
    [c[0].clamp(0, 255) as f32 / 255.0, c[1].clamp(0, 255) as f32 / 255.0, c[2].clamp(0, 255) as f32 / 255.0, 1.0]
}

/// `OptionsVonal` + `line2d(…, false, true)`: a one-pixel line, its colour running from `ca` to `cb`, additive.
fn line(f: &mut Frame, d: &Draw, a: [i32; 2], b: [i32; 2], ca: [i32; 3], cb: [i32; 3]) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let n = dx.abs().max(dy.abs()).max(1);
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let p = [a[0] as f32 + dx as f32 * t, a[1] as f32 + dy as f32 * t];
        let c: [i32; 3] = std::array::from_fn(|i| (ca[i] as f32 + (cb[i] - ca[i]) as f32 * t) as i32);
        f.push(d.white, Blend::GammaAdditive, Instance::new(kind::SPRITE, d.v.px(p[0] + 0.5, p[1] + 0.5), [d.v.k / 2.0, d.v.k / 2.0], rgb(c)));
    }
}

/// `FillToScreen(x, y, w, h, colour)`.
fn fill(f: &mut Frame, d: &Draw, r: [f32; 4], c: [f32; 3]) {
    let v = &d.v;
    f.push(d.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0), [r[2] * v.k / 2.0, r[3] * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
}

impl OptionsScreen {
    /// One drawn frame: `OptionsGfx2D_1` (faders, lasers, slider boxes, sparks), `OptionsGfx2D_2` (texts, slots,
    /// tracks), the tooltip. The 3D behind (the main menu's scene when load only, else black) is the window's.
    pub fn draw(&mut self, f: &mut Frame, d: &Draw) {
        self.drawn = true;
        self.draw_faders(f, d);
        self.draw_texts(f, d);
        let under = if self.held { 0 } else { self.spot_at(self.tip.mouse) };
        let text = self.tip_text(d.texts, under);
        self.tip.update(d.now_ms, if text.is_empty() { 0 } else { under }, text.len(), self.held);
        let shown = self.tip_text(d.texts, self.tip.shown);
        self.tip.draw(f, d.font, d.white, &d.v, d.palette, &shown);
    }

    /// `OptionsGfx2D_1` (ELF 0x5d49a0): each block fades in over its window of `OptionsFelfadeles` with a laser round
    /// it while it does; leaving, all fade by `OptionsKilepesValtozo`.
    fn draw_faders(&mut self, f: &mut Frame, d: &Draw) {
        let (white, v) = (d.white, &d.v);
        let fd = self.fade;
        let leaving = self.leaving.as_ref().map(|l| l.0 as f32);
        let s = fader::S;
        self.lasers.clear();
        let kirak = |f: &mut Frame, r: [f32; 4], vv: f32| {
            let k = 1.0 - vv / 100.0;
            fader::kirak_fader(f, white, v, r, [20.0, 54.0 * s, 100.0 * s], k * 195.0 + 60.0, [0.0, 90.0 * s, 130.0 * s], k * 205.0 + 50.0);
        };
        let mut vv = self.fade_v;
        // The load button: `V = 3F` below 33.
        if self.filled(self.slot) && !self.multiplayer && !self.sure {
            if fd >= 33.0 {
                vv = 100.0;
            } else {
                vv = fd * 3.0;
                if leaving.is_none() {
                    self.lasers.shoot([0x16e, 0x3e, 0x4e, 0x21], [0x27f, 0], vv / 100.0, LASER);
                }
            }
            if let Some(l) = leaving {
                vv = l;
            }
            kirak(f, [366.0, 62.0, 78.0, 33.0], vv);
        }
        // The save (or YES) button: `V = 3(F − 33)` between 33 and 66.
        if (!self.multiplayer || self.sure) && ((!self.load_only && !self.battle && (self.sure || self.slot < 0x17)) || self.sure) {
            if fd <= 33.0 || fd >= 66.0 {
                vv = if fd >= 33.0 { 100.0 } else { 0.0 };
            } else {
                vv = (fd - 33.0) * 3.0;
                if leaving.is_none() {
                    self.lasers.shoot([0x1c4, 0x3e, 0x4e, 0x21], [0x27f, 0], vv / 100.0, LASER);
                }
            }
            if let Some(l) = leaving {
                vv = l;
            }
            kirak(f, [452.0, 62.0, 78.0, 33.0], vv);
        }
        // The quit button, always: `V = 3(F − 66)` past 66 (102 at F 100, as the exe has it).
        if fd > 66.0 {
            vv = (fd - 66.0) * 3.0;
            if fd != 100.0 && leaving.is_none() {
                self.lasers.shoot([0x21d, 0x3e, 0x4e, 0x21], [0x27f, 0], vv / 100.0, LASER);
            }
        } else if fd < 66.0 {
            vv = 0.0;
        }
        if let Some(l) = leaving {
            vv = l;
        }
        kirak(f, [541.0, 62.0, 78.0, 33.0], vv);
        // The slot list: `V = 2F` below 50, its laser from the bottom right corner.
        if fd >= 50.0 {
            vv = 100.0;
        } else {
            vv = fd * 2.0;
            if leaving.is_none() {
                self.lasers.shoot([0x169, 0x74, 0x109, 0x14a], [0x27f, 0x1df], vv / 100.0, LASER);
            }
        }
        if let Some(l) = leaving {
            vv = l;
        }
        kirak(f, [361.0, 116.0, 265.0, 330.0], vv);
        self.blokk1 = vv;
        // The left panel: `V = 2(F − 50)` between 50 and 100, its laser from the top left corner.
        if fd <= 50.0 || fd >= 100.0 {
            if fd < 50.0 {
                vv = 0.0;
            } else if fd == 100.0 {
                vv = 100.0;
            }
        } else {
            vv = (fd - 50.0) * 2.0;
            if leaving.is_none() {
                self.lasers.shoot([9, 0x3e, 0x120, 0x180], [0, 0], vv / 100.0, LASER);
            }
        }
        if let Some(l) = leaving {
            vv = l;
        }
        kirak(f, [9.0, 62.0, 288.0, 384.0], vv);
        self.blokk2 = vv;
        // The sliders' boxes (`FadeKerettelKerekit`), brighter under the pointer.
        let k = 1.0 - vv / 100.0;
        let hover = self.spot_at(self.at);
        for (i, sl) in self.sliders.iter().enumerate().filter(|(_, s)| s.visible) {
            let r = [sl.x as f32, (sl.y - 3) as f32, sl.width as f32, 10.0];
            if hover == id::SLIDER + i as u32 {
                fader::fader(f, white, v, r, [20.0, 54.0, 150.0], k * 245.0 + 10.0, [230.0; 3], k * 245.0 + 10.0);
            } else {
                fader::fader(f, white, v, r, [20.0, 54.0, 100.0], k * 235.0 + 20.0, [230.0; 3], k * 225.0 + 30.0);
            }
        }
        // The tabs.
        kirak(f, [13.0, 66.0, 90.0, 20.0], vv);
        kirak(f, [108.0, 66.0, 90.0, 20.0], vv);
        self.fade_v = vv;
        self.lasers.draw(f, white, d.glow, v, true, true);
    }

    /// `OptionsGfx2D_2` (exe 0x518a30).
    fn draw_texts(&mut self, f: &mut Frame, d: &Draw) {
        let v = &d.v;
        let (b1, b2) = (self.blokk1, self.blokk2);
        let hover = if self.held && self.grab.is_none() { 0 } else { self.spot_at(self.at) };
        let text = |f: &mut Frame, s: &[u8], x: f32, y: f32, len: usize, size: usize, a: Align, c: [f32; 3]| d.font.text(f, v, s, x, y, len, size, a, c, None);
        let cw = d.font.advance(2);
        // The sliders: the title (colour 2, 1 when it can't be changed) and the level (`value / 20`), or (13,4,11).
        for s in self.sliders.iter().filter(|s| s.visible) {
            let (x, y) = (s.x as f32, (s.y + 0xb) as f32);
            text(f, &s.title, x, y, s.title.len(), 2, Align::Left, d.col(if s.enabled { 2 } else { 1 }, b2));
            let after = x + cw * s.title.len() as f32 + 10.0;
            if s.enabled {
                let l = &s.levels[(s.value / 0x14).clamp(0, 5) as usize];
                text(f, l, after, y, l.len(), 2, Align::Left, d.col(0x25, b2));
            } else {
                text(f, &d.texts.get(13, 4, 11), after, y, 0x14, 2, Align::Left, d.col(0x25, b2));
            }
        }
        // The page's rows (`KirakMenuPontok`).
        for (n, r) in self.rows.iter().filter(|r| r.page == self.page).enumerate() {
            let y = self.row_y(n as i32) as f32;
            let (px, py) = (self.at[0] as i32, self.at[1] as i32);
            let over = py >= y as i32 && py < y as i32 + 0xf && (0x1f..=0x117).contains(&px) && self.grab.is_none();
            let (cn, cv) = if !r.enabled { (1, 1) } else if over { (2, 2) } else { (0x22, 0x25) };
            text(f, &r.name, 0x1e as f32, y, r.name.len(), 2, Align::Left, d.col(cn, b2));
            let (val, c) = match (r.states, r.value) {
                (1, 1) => (d.texts.get(13, 3, 1), cv),
                (1, _) => (d.texts.get(13, 3, 2), cv),
                (2, 0..=2) => (d.texts.get(13, 3, 3 + r.value as u16), 2),
                _ => (Vec::new(), cv),
            };
            text(f, &val, 0xe6 as f32, y, 10, 2, Align::Left, d.col(c, b2));
        }
        // The buttons' labels (font 3, centred in 9 cells), colour 2 under the pointer.
        let lab = |f: &mut Frame, s: u16, x: i32, spot: u32| {
            text(f, &d.texts.get(13, 5, s), x as f32, 0x48 as f32, 9, 3, Align::Centre, d.col(if hover == spot { 2 } else { 1 }, b2));
        };
        if self.filled(self.slot) && !self.sure && !self.multiplayer {
            lab(f, 1, 0x195, id::LOAD);
        }
        if !self.load_only {
            if self.sure {
                lab(f, 2, 0x1eb, id::SAVE);
            } else if self.slot <= 0x16 && !self.battle && !self.multiplayer {
                lab(f, 3, 0x1eb, id::SAVE);
            }
        }
        match (self.load_only, self.sure) {
            (false, true) => lab(f, 5, 0x249, id::QUIT),
            (false, false) => lab(f, 6, 0x244, id::QUIT),
            (true, _) => lab(f, 4, 0x244, id::QUIT),
        }
        // The slot list (`KiirStrSzinSzorzo = Blokk1`): names at x 380, the selected one framed.
        for i in 1..=SLOTS {
            let y = 0x83 + (i as i32 - 1) * 0xc;
            let c = if i == self.slot { 2 } else { 1 };
            if i == self.slot {
                self.design.ask([0x178, y - 4, 0xbb, 0xc]);
            }
            match &self.slots[i] {
                None if i < 0x17 => text(f, b"------------------------------", 0x17c as f32, (y - 1) as f32, 0x1e, 2, Align::Left, d.col(c, b1)),
                None => {}
                Some(s) => {
                    text(f, &s.name, 0x17c as f32, (y - 1) as f32, 0x1e, 2, Align::Left, d.col(c, b1));
                    for (k, (x, len)) in [(0x238, 4), (0x253, 2), (0x262, 2)].into_iter().enumerate() {
                        d.font.int(f, v, s.date[k] as i64, x as f32, y as f32, len, 1, Align::Right, d.col(c, b1));
                    }
                }
            }
        }
        self.design.draw(f, d.white, v, d.col(0x22, b1));
        // The tabs (colour 2 under the pointer or for their page; the second tests page 1, as the exe does).
        let tab = |f: &mut Frame, s: u16, x: i32, on: bool| text(f, &d.texts.get(13, 5, s), x as f32, 0x46 as f32, 0xf, 3, Align::Centre, d.col(if on { 2 } else { 1 }, b2));
        tab(f, 9, 0x3a, hover == id::SOUND || self.page == 0);
        tab(f, 11, 0x99, hover == id::GAME || self.page == 1);
        // The name entry: its box (colour 0x27 until the first key, else 0), the text, the blinking cursor.
        if let Some(e) = &self.entry {
            let (x, y) = (0x17c as f32, (self.slot as i32 * 0xc + 0x76) as f32);
            let (w, h) = (d.font.advance(2), d.font.height(2));
            fill(f, d, [x - 1.0, y - 1.0, w * e.text.len() as f32 + 2.0, h + 2.0], d.col(if e.clear { 0x27 } else { 0 }, 100.0));
            text(f, &e.text, x, y, 0x20, 2, Align::Left, d.col(2, b2));
            if (d.now_ms / 200.0) as i64 % 2 == 0 {
                let c = e.cursor.min(0x1f) as f32;
                fill(f, d, [x + w * c, y, w, h], d.col(1, 100.0));
            }
        }
        // The tracks: per visible slider `value + 10` columns, two pixels apart, the colour rising along them; the
        // thumb's five grey strokes.
        for s in self.sliders.iter().filter(|s| s.visible) {
            let n = (s.width / 100) * s.value / 2 + 10;
            for j in 0..n {
                let x = s.x + j * 2;
                let dd = if j == n - 1 { 3 } else if j == n - 2 { 1 } else { 0 };
                let t = (j as f32 / s.width as f32) * 2.0 * b2 / 100.0;
                // `line2d` takes 0xAABBGGRR: the 200 is blue.
                let top = [(t * 10.0) as i32, (t * 10.0) as i32, (t * 200.0) as i32];
                let bottom = [(t * 210.0) as i32, (t * 210.0) as i32, (t * 200.0) as i32];
                line(f, d, [x, s.y - 2 + dd], [x, s.y + 6 - dd], top, bottom);
            }
            let g = ((b2 * 140.0) / 100.0) as i32;
            let grey = [g; 3];
            let x14 = (s.width / 100) * s.value + 0x12 + s.x;
            let x12 = x14 - 1;
            let y = s.y;
            for (a, b) in [([x14 - 3, y + 4], [x12, y + 4]), ([x12, y + 2], [x12, y + 4]), ([x14, y], [x14, y + 3]), ([x12, y - 1], [x12, y]), ([x14 - 3, y - 2], [x12, y - 2])] {
                line(f, d, a, b, grey, grey);
            }
        }
    }
}

/// The settings `Options_Belep` starts from: the audio's volumes and the settings file (`options.dat` with the
/// remake's own over it).
pub fn start_settings(file: &ig2_formats::options::Options) -> Settings {
    Settings {
        music: file.int("Music Volume").unwrap_or(100).clamp(0, 100),
        sound: file.int("Sound Volume").unwrap_or(100).clamp(0, 100),
        gamma: file.int("Gamma Correction").unwrap_or(50),
        gamma_enabled: false,
        page: file.int("OptionsMelyikMod").unwrap_or(0),
        slot: file.int("Save slot").unwrap_or(1).clamp(1, SLOTS as i32) as usize,
        gfx_detail: file.int("OptionsGfxDetail").unwrap_or(0),
    }
}

/// What a headless Options needs: the HUD's font, palette and texts, the lasers' glow, the rows.
pub struct Headless {
    pub hud: crate::ui::Hud,
    pub glow: TexId,
}

impl Headless {
    pub fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install, tables: &ig2_rules::Rules) -> Self {
        let hud = crate::ui::Hud::load(r, install, tables);
        let glow = install.resolve_texture("textures/starmap/", "laservago.bmp").and_then(|p| crate::assets::Image::load(install, &p).ok()).map_or(hud.white, |img| r.add_texture(&img));
        Self { hud, glow }
    }

    /// `Options_Belep` with the user's exe rows and settings file.
    pub fn enter(&self, install: &ig2_formats::Install, load_only: bool) -> OptionsScreen {
        let file = ig2_formats::options::Options::load();
        let exe = install.read_raw("ig2.exe").unwrap_or_default();
        let mut rows = ig2_rules::PeImage::parse(&exe).map(|img| read_rows(&img, &self.hud.texts)).unwrap_or_default();
        for r in &mut rows {
            if let Some(v) = file.int(&r.key) {
                r.value = v;
            }
        }
        OptionsScreen::enter(&self.hud.texts, rows, &start_settings(&file), load_only, false, read_slots(slot_dir().as_deref()))
    }

    pub fn draw(&self, s: &mut OptionsScreen, f: &mut Frame, size: [f32; 2], now_ms: f64) {
        let Some(font) = &self.hud.font else { return };
        let d = Draw { font, white: self.hud.white, glow: self.glow, v: Virtual::new(size), palette: &self.hud.palette, texts: &self.hud.texts, now_ms };
        s.draw(f, &d);
    }
}

/// `ig2-client --snapshot OUT --screen options [--ui-frames N] [--mouse X,Y] [--script S]`: the Options screen
/// opened on the save's game (black behind it, the top bar over it) after N UI ticks of 10 ms, one drawn frame each;
/// with a script one grab per `t` (re/screens/sound.md §3: the events at t, the tick, the frame).
pub fn snapshot(o: &crate::Options, out: &Path) -> Result<(), String> {
    use crate::script::Input;
    let l = crate::load(o)?;
    let world = crate::world::World::load(&l.save, &l.rules, &l.tech_defaults, &l.volatile);
    let mut r = crate::render::Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut h = Headless::load(&mut r, &l.install, &l.tables);
    let mut s = h.enter(&l.install, false);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let script = o.script.as_deref().map(crate::script::Script::load).transpose()?;
    let (end, grabs) = match &script {
        Some(sc) => (sc.end(), sc.grabs.clone()),
        None => (o.ui_frames, vec![o.ui_frames]),
    };
    // The pointer rests where the capture leaves it (the screen's centre) unless told otherwise.
    s.hover(o.mouse.unwrap_or([320.0, 240.0]));
    let mut played = Vec::new();
    for t in 0..=end {
        for e in script.iter().flat_map(|sc| sc.at(t)) {
            match &e.input {
                Input::Move(at) => s.hover(*at),
                Input::Button { button: 1, down: true, at } => {
                    if let Some(a) = s.press(*at, &mut |id| played.push((t, id))) {
                        println!("t {t}: {a:?}");
                    }
                }
                Input::Button { button: 1, down: false, .. } => s.release(),
                Input::Key { name, down: true } if name == "escape" => s.escape(),
                _ => {}
            }
        }
        if t > 0 {
            s.tick();
        }
        if let Some(a) = s.done() {
            println!("t {t}: {a:?}");
        }
        // Each t a drawn frame (the fade starts with the first, `OptionsVoltElsoframe`).
        let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
        h.draw(&mut s, &mut frame, size, t as f64 * 10.0);
        if grabs.contains(&t) {
            h.hud.paused = true;
            h.hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Options, 0.0);
            let rgba = r.draw_to_image(&frame, o.size, 0.0);
            let path = if script.is_some() { crate::script::grab_path(out, t) } else { out.to_path_buf() };
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {t} | page {} slot {} fade {} sure {} entry {:?}", path.display(), s.page, s.slot, s.fade, s.sure, s.entry.as_ref().map(|e| String::from_utf8_lossy(&e.text).into_owned()));
        }
    }
    for (t, id) in played {
        println!("sound t {t}: {id:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> OptionsScreen {
        let texts = UiTexts { strings: Default::default(), kinds: Vec::new(), lang: ig2_formats::text::Lang::En };
        let rows = vec![
            Row { name: b"Auto Save".to_vec(), key: "OptionsKellAutoSave".into(), enabled: true, value: 1, states: 1, page: 2, id: 101, dep: 0, mp_ok: true },
            Row { name: b"GB sim".to_vec(), key: "DebugData_grBattle".into(), enabled: true, value: 0, states: 1, page: 2, id: 102, dep: 0, mp_ok: true },
            Row { name: b"GB overrun".to_vec(), key: "DebugData_GrNoOverrun".into(), enabled: true, value: 1, states: 1, page: 2, id: 103, dep: -102, mp_ok: true },
        ];
        let s = Settings { music: 100, sound: 50, gamma: 0, gamma_enabled: false, page: 0, slot: 1, gfx_detail: 0 };
        let mut slots = vec![None; SLOTS + 1];
        slots[24] = Some(Slot { name: b"Autosave 2328 1 1".to_vec(), date: [2328, 1, 1], file: PathBuf::new(), ours: false });
        OptionsScreen::enter(&texts, rows, &s, false, false, slots)
    }

    #[test]
    fn hot_spots_lowest_id_wins() {
        let o = screen();
        // The left panel (17) lies over the back area (18); the save button is on, load off (slot 1 empty).
        assert_eq!(o.spot_at([100.0, 300.0]), id::PANEL);
        assert_eq!(o.spot_at([320.0, 300.0]), id::BACK);
        assert_eq!(o.spot_at([400.0, 70.0]), 0);
        assert_eq!(o.spot_at([480.0, 70.0]), id::SAVE);
        // The music slider's box (x − 10, y − 5, w + 10, 30) is over the panel.
        assert_eq!(o.spot_at([25.0, 110.0]), id::SLIDER);
        assert_eq!(o.spot_at([700.0, 900.0]), 0, "clamped to (639, 479): nothing there");
    }

    #[test]
    fn slot_pick_by_hover() {
        let mut o = screen();
        o.hover([400.0, (0x82 + 12 * 4 + 3) as f32]);
        assert_eq!(o.slot, 5);
        // Row 23 is empty: not taken; row 24 holds the autosave.
        o.hover([400.0, (0x82 + 12 * 22 + 3) as f32]);
        assert_eq!(o.slot, 5);
        o.hover([400.0, (0x82 + 12 * 23 + 3) as f32]);
        assert_eq!(o.slot, 24);
        // Slot 24 can't be saved to.
        assert!(!o.spot_on(id::SAVE));
    }

    #[test]
    fn slider_drag_is_relative() {
        let mut o = screen();
        o.press([100.0, 150.0], &mut |_| {});
        assert!(o.grab.is_some(), "the sound slider (y 150)");
        // `trunc(f64(f32(−37) / 220) · 100 + 50)` = 33.
        o.hover([63.0, 400.0]);
        assert_eq!(o.sliders[1].value, 33);
        o.hover([600.0, 0.0]);
        assert_eq!(o.sliders[1].value, 100);
        o.release();
        o.hover([0.0, 150.0]);
        assert_eq!(o.sliders[1].value, 100);
    }

    #[test]
    fn quit_asks_first() {
        let mut o = screen();
        let mut sounds = Vec::new();
        assert_eq!(o.press([580.0, 70.0], &mut |s| sounds.push(s)), None);
        assert!(o.sure && sounds.len() == 1);
        // NO: the question closes without a sound.
        o.press([580.0, 70.0], &mut |s| sounds.push(s));
        assert!(!o.sure && sounds.len() == 1);
        // YES: to the main menu through the fade-out.
        o.press([580.0, 70.0], &mut |s| sounds.push(s));
        o.press([480.0, 70.0], &mut |s| sounds.push(s));
        assert!(o.leaving());
        let mut act = None;
        for _ in 0..200 {
            o.tick();
            act = act.or(o.done());
        }
        assert_eq!(act, Some(Act::MainMenu));
    }

    #[test]
    fn rows_toggle_and_depend() {
        let mut o = screen();
        o.press([150.0, 75.0], &mut |_| {});
        assert_eq!(o.page, 2);
        assert!(o.rows[2].enabled, "row 103 needs row 102 off");
        // Row 1 (102) at y 0x5c + 15.
        o.press([40.0, (0x5c + 15 + 2) as f32], &mut |_| {});
        assert_eq!(o.rows[1].value, 1);
        assert!(!o.rows[2].enabled && o.gfx_detail == 3);
        o.press([40.0, (0x5c + 15 + 2) as f32], &mut |_| {});
        assert!(o.rows[2].enabled);
        let mut f = ig2_formats::options::Options::default();
        o.store(&mut f);
        assert_eq!((f.int("Music Volume"), f.int("OptionsMelyikMod"), f.int("DebugData_grBattle")), (Some(100), Some(2), Some(0)));
    }

    #[test]
    fn save_name_entry() {
        let mut o = screen();
        o.press([480.0, 70.0], &mut |_| {});
        assert_eq!(o.entry.as_ref().map(|e| e.text.clone()), Some(b"Save Game 01".to_vec()));
        // The first key clears the text.
        for c in *b"ab" {
            o.entry_key(EntryKey::Char(c));
        }
        o.entry_key(EntryKey::Back);
        assert_eq!(o.entry_key(EntryKey::Enter), Some(Act::Save(1, b"a".to_vec())));
    }

    #[test]
    fn audit_fixes() {
        // The first Backspace clears the whole text; at 32 a character still goes in and the last drops; Home, End.
        let mut o = screen();
        o.press([480.0, 70.0], &mut |_| {});
        o.entry_key(EntryKey::Back);
        assert_eq!(o.entry.as_ref().map(|e| e.text.len()), Some(0));
        for _ in 0..0x20 {
            o.entry_key(EntryKey::Char(b'x'));
        }
        o.entry_key(EntryKey::Home);
        o.entry_key(EntryKey::Char(b'y'));
        let e = o.entry.clone().unwrap();
        assert_eq!((e.text.len(), e.text[0], e.cursor), (0x20, b'y', 1));
        o.entry_key(EntryKey::End);
        o.entry_key(EntryKey::Delete);
        assert_eq!(o.entry.as_ref().map(|e| (e.text.len(), e.cursor)), Some((0x20, 0x20)));
        // The slot list's bottom rows (25..=27) pick nothing.
        o.entry = None;
        o.hover([400.0, (0x82 + 12 * 24 + 3) as f32]);
        assert_eq!(o.slot, 1);
        // Load only, the back area goes to the main menu.
        let mut o = screen();
        o.load_only = true;
        o.fade = 100.0;
        o.press([320.0, 300.0], &mut |_| {});
        let mut act = None;
        for _ in 0..300 {
            o.tick();
            act = act.or(o.done());
        }
        assert_eq!(act, Some(Act::MainMenu));
    }

    #[test]
    fn slot_file_title() {
        let b = slot_file(vec![1, 2, 3], b"My game", [2327, 10, 23]);
        assert_eq!(our_title(&b), Some((b"My game".to_vec(), [2327, 10, 23])));
        assert_eq!(our_title(&[0; 10]), None);
    }
}
