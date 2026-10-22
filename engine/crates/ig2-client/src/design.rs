//! The design screen (`AktKep` 5, re/screens/design.md), part 1: viewing. `Design_Belep` (ELF 0x5074e0 / exe
//! 0x4b1be0) and `Design_Kilep` (0x507bb0 / exe 0x4b1f20) with the opening and closing (`desframe`), the per-tick
//! `FRISSIT_Design2` (0x50d890) and the state half of `Design3DUpdate` (0x503ea0 / exe 0x4ad8e0: the hull sliding in
//! and spinning), the 2D pass `DesignGfx2D_1` (0x4fb840 / exe 0x4af440: the design list, its frame, the data panels,
//! the pod rows), the faders and the scroll bar (`GFX_Design` 0x507ed0), and the viewing input of `Design_Main`
//! (0x509140 / exe 0x4b2190: the list, the scroll bar, dragging the hull) and `DesignKeyboardHandler` (0x508dc0: up
//! and down). The 3D scene is [`crate::design_scene`].
//!
//! Not yet (re/screens/design.md, the roadmap's "Design screen part 2"): the editing modes (new, redesign, modify,
//! delete; the pod popups, the name input) and their orders (`Multi_Send` 0x33/0x34, `MULTIGET_Design` in the sim),
//! the 3D buttons' press animation, the pods' lights (`SzinezPodok`), the 3D pod pick (`PodKivalaszto`), the tech
//! models opening out of a pod, the fader scan bands (`FadeKerettelKerekitDesign`), the lasers and sparks.

use ig2_formats::Install;
use ig2_rules::sounds::Table;
use ig2_sim::colony_side::TechTable;
use ig2_sim::save::{va, SaveState};

use crate::audio::{SoundId, CLICK};
use crate::font::Align;
use crate::kit::{DesignFrame, Tooltip};
use crate::render::Frame;
use crate::scene::{self, M3};
use crate::starmap_panels::Draw2d;

/// Design record fields (`Tervek[e][i]`, 0x84 bytes; re/screens/design.md §2).
pub mod field {
    pub const DELETED: usize = 0x16;
    pub const LISTED: usize = 0x18;
    pub const OBSOLETE: usize = 0x19;
    pub const SUCCESSOR: usize = 0x1c;
    pub const EFFORT: usize = 0x20;
    pub const PRICE: usize = 0x24;
    pub const TANKS: usize = 0x28;
    pub const FIREPOWER: usize = 0x2c;
    pub const HP: usize = 0x30;
    pub const NO_SPEED: usize = 0x37;
    pub const QUARTER_SPEED: usize = 0x38;
    pub const SPEED: usize = 0x3c;
    pub const RACE: usize = 0x40;
    pub const HULL: usize = 0x44;
    pub const HULL_TECH: usize = 0x48;
}

/// `AktTervMutato` (saved): the chosen design's rank in the sorted list.
const AKT_TERV_MUTATO: u32 = 0xB9AAEC;
/// `Hajok` (6 bytes each: fleet i16, design i16), `Tankok` (8: fleet or −planet i16, design i16, count i32).
const NO_DESIGN: i16 = 0x7fff;
/// Planet record fields: owner (+0x20), the stations (`+0xfe + 2k`, k = 1..=`+0x114`).
const PLANET_SIZE: u32 = 0x188;

/// `Kord1X/Y` (the list), `Kord2X`, `Kord2Y/Y2/Y3`, `Kord3X` (the bottom-left panel), `CsuszkaX/Y` (the scroll bar),
/// `podkezdx/y` (the pod rows), read from the ELF image (exe 0x902ee4..).
const KORD1: [f32; 2] = [15.0, 100.0];
const KORD2X: f32 = 15.0;
const KORD2Y: [f32; 3] = [375.0, 390.0, 405.0];
const KORD3X: f32 = 62.0;
const CSUSZKA: [i32; 2] = [137, 92];
const POD: [f32; 2] = [508.0, 80.0];
/// Rows the list shows.
const ROWS: usize = 15;
/// `ADDZOOMMAXDES` (exe 0x902f5c): 321.0.
pub const ADD_ZOOM_MAX: f32 = 321.0;
/// The hull's start orientation (`LoadAlapVektor`: `RotateObj(1.0471667, 0.785375, 0)`).
const HULL_TURN: (f32, f32, f32) = (1.047_166_7, 0.785_375, 0.0);

/// `Des_Hangok` (`InitSounds(Des_Hangok, 4)`): screen21 (opening), be1 (a hull loaded), ki1 (the list), screen23
/// (closing).
fn des(i: u16) -> SoundId {
    SoundId::new(Table::Des, i)
}

/// `Design_MouseInit`'s hot-spots in the viewing mode (exe 0x4b3810, `Mouse_Szoveg_Set(id, colour, x, y, w, h,
/// GetString(5, 1, tip))`): `(id, mask colour, rect, tip)`. Colour 0 = the whole rectangle.
pub const SPOTS: [(u32, u8, [i32; 4], u16); 7] = [
    (1, 2, [0, 0x128, 0x42, 0x30], 1),
    (2, 3, [0x41, 0x127, 0x43, 0x31], 2),
    (3, 4, [0, 0x157, 0x42, 0x31], 3),
    (4, 5, [0x41, 0x157, 0x44, 0x32], 4),
    (9, 0, [5, 0x5d, 0x7b, 0xe9], 9),
    (10, 0, [CSUSZKA[0], CSUSZKA[1], 10, 0xea], 10),
    (0x54, 0, [0x1f2, 0x5c, 0x8a, 0xea], 0xb),
];

/// The tooltip texts by hot-spot id (`GetString(5, 1, tip)`).
pub fn tips(texts: &crate::ui::UiTexts) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(); 0x57];
    for &(id, _, _, k) in &SPOTS {
        out[id as usize] = texts.get(5, 1, k);
    }
    out
}

/// `DES_MSK.RAW` (kep 0x5f): a 32-byte header, a 768-byte palette, 640 × 480 hot-spot colours.
pub fn mask(install: &Install) -> Vec<u8> {
    install.read("dataraw/screen/des_msk.raw").ok().and_then(|b| b.get(800..800 + 640 * 480).map(|x| x.to_vec())).unwrap_or_default()
}

fn en(state: &SaveState) -> usize {
    state.mem.i32(va::EN).clamp(0, 18) as usize
}

fn i16_at(r: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([r[o], r[o + 1]])
}

fn i32_at(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}

/// The designs of empire `e` (`Tervek[e][0..=OsszHanyTerv[e]]`).
fn designs(state: &SaveState, e: usize) -> &[[u8; ig2_sim::save::DESIGN_SIZE]] {
    state.designs.get(e).map_or(&[], |d| d.as_slice())
}

/// `Design_UjraRendez` (ELF 0x5078d0 / exe 0x4b3cb0): the listed designs (`+0x16 ≥ 0`, `+0x18` set) by hull
/// (`+0x44`), as its pass moves each element from the end forward past larger hulls; `[0] = 0`.
pub fn sorted(state: &SaveState, e: usize) -> Vec<usize> {
    let d = designs(state, e);
    let mut order = vec![0];
    order.extend((1..d.len()).filter(|&i| i16_at(&d[i], field::DELETED) >= 0 && d[i][field::LISTED] & 1 != 0));
    let n = order.len() - 1;
    let hull = |i: usize| i32_at(&d[i], field::HULL);
    for c in (2..=n).rev() {
        let mut j = c - 1;
        while j < n && hull(order[j + 1]) < hull(order[j]) {
            order.swap(j, j + 1);
            j += 1;
        }
    }
    order
}

/// `HanyHajoVanATervbol(e, fleet = 0, design)` (ELF 0x506520): the ships, vehicles (by count) and planet stations
/// of design `t` or of a design it replaced (`+0x1c == t`).
pub fn units_of(state: &SaveState, e: usize, t: usize) -> i32 {
    let d = designs(state, e);
    let of = |k: i16| k == t as i16 || (k >= 0 && d.get(k as usize).is_some_and(|r| i32_at(r, field::SUCCESSOR) == t as i32));
    let fleet_owner = |f: i16| state.fleets.get(f.max(0) as usize).map_or(0, |r| r[0x15]);
    let planet_owner = |p: u32| state.mem.u8(va::PLANETS + p * PLANET_SIZE + 0x20);
    let mut n = 0;
    for s in state.ships.iter().skip(1) {
        let (f, k) = (i16_at(s, 0), i16_at(s, 2));
        if k != NO_DESIGN && fleet_owner(f) as usize == e && of(k) {
            n += 1;
        }
    }
    let tanks = state.mem.i32(va::VEHICLE_COUNT).max(0) as u32;
    for i in 1..=tanks {
        let at = va::VEHICLES + 8 * i;
        let (f, k) = (state.mem.i16(at), state.mem.i16(at + 2));
        if k == NO_DESIGN {
            continue;
        }
        let owner = if f < 1 { planet_owner((-(f as i32)) as u32) } else { fleet_owner(f) };
        if owner as usize == e && of(k) {
            n += state.mem.i32(at + 4);
        }
    }
    let planets = state.mem.i32(va::PLANET_COUNT).max(0) as u32;
    for p in 1..=planets {
        if planet_owner(p) as usize != e {
            continue;
        }
        let base = va::PLANETS + p * PLANET_SIZE;
        for k in 1..=state.mem.i32(base + 0x114).max(0) as u32 {
            if of(state.mem.i16(base + 0xfe + 2 * k)) {
                n += 1;
            }
        }
    }
    n
}

/// Keys of `DesignKeyboardHandler` that work while viewing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
}

/// The screen's state.
#[derive(Debug, Clone)]
pub struct Design {
    /// `desframe` (0..=60), `desKINYITVA` (open), `desBECSUKAS` (closing), `desKELLKINYITAS` (animated),
    /// `desELINDULT` (the first frame was drawn).
    pub frame: f32,
    pub opened: bool,
    pub closing: bool,
    /// `Design_Kilep` was asked for: the main loop calls it after this tick's frame, so the closing starts with the
    /// next tick [obs: the exe's panels fold one tick later].
    leaving: bool,
    pub animated: bool,
    started: bool,
    /// `deskeringeto` (the side lights' angle), `des_xy`, `deswireframe` (the sphere's frame past 60).
    pub keringeto: f32,
    des_xy: f32,
    pub wire: f32,
    /// `DesignSzinallitoDesignTablazat`: each list row's text brightness in percent (−10·i → 100).
    fade: [i32; ROWS],
    /// `DesignRendezo` (`[0] = 0`), `DesignTervKezdet` (rows scrolled away), `AktTervMutato`.
    pub order: Vec<usize>,
    pub first: usize,
    pub pointer: usize,
    /// `AddZoomDes`, `AddZoomDirDes`, `AktAlap` (the chosen design's hull model), `OldAlap` (the one loaded).
    pub add_zoom: f32,
    add_dir: i32,
    pub model: i32,
    pub loaded: i32,
    /// `LEGELSOBEJOVES`: the first slide-in finished (the hull's z follows `AddZoomDes` from then on).
    pub settled: bool,
    /// The hull's rotation (`AlapokTsc[m]+0x14` without its scale), `desGyX/Y`, the drag (`desLenyomta`, the
    /// pointer's last place `desMouseOldX/Y`).
    pub turn: M3,
    gy: [f32; 2],
    drag: Option<[f32; 2]>,
    /// `DesignAktPod` (0 = none) and the design it belongs to (a new design clears it, `Design_Main`).
    pub pod: usize,
    pod_of: usize,
    /// The pointer and the hot-spot the left button holds (`Mouse_Megfogva`).
    pub mouse: [f32; 2],
    held: Option<u32>,
    pub list_frame: DesignFrame,
    pub tip: Tooltip,
    /// UI ticks since the screen opened.
    pub frames: u64,
    pub sounds: Vec<SoundId>,
}

impl Design {
    /// `Design_Belep`: `animated` = `desKELLKINYITAS` (`KELLKINYITASDEFAULT`, not coming back from the messages,
    /// help or options screen).
    pub fn open(state: &SaveState, animated: bool) -> Self {
        let e = en(state);
        let order = sorted(state, e);
        let n = order.len() - 1;
        let pointer = if n < 1 { 0 } else { (state.mem.i32(AKT_TERV_MUTATO).max(0) as usize).min(n) };
        let mut s = Self {
            frame: if animated { 0.0 } else { 60.0 },
            opened: !animated,
            closing: false,
            leaving: false,
            animated,
            // The first frame is drawn (`desELINDULT`) before the first UI tick.
            started: true,
            keringeto: 0.0,
            des_xy: 0.0,
            wire: 0.0,
            fade: std::array::from_fn(|i| -10 * i as i32),
            order,
            first: 0,
            pointer,
            add_zoom: ADD_ZOOM_MAX,
            add_dir: -1,
            model: -1,
            loaded: -1,
            settled: false,
            turn: scene::create_matrix(HULL_TURN.0, HULL_TURN.1, HULL_TURN.2),
            gy: [0.0; 2],
            drag: None,
            pod: 0,
            pod_of: 0,
            mouse: [320.0, 240.0],
            held: None,
            list_frame: DesignFrame::default(),
            tip: Tooltip::default(),
            frames: 0,
            sounds: Vec::new(),
        };
        s.scroll_to_pointer();
        if animated {
            s.sounds.push(des(0));
        }
        s
    }

    /// `AktTerv`: the chosen design (0 = none).
    pub fn current(&self) -> usize {
        self.order.get(self.pointer).copied().unwrap_or(0)
    }

    /// `AktHanyTerv`.
    pub fn count(&self) -> usize {
        self.order.len() - 1
    }

    /// The chosen design's record (the view mode's `d`).
    pub fn record<'a>(&self, state: &'a SaveState) -> Option<&'a [u8; ig2_sim::save::DESIGN_SIZE]> {
        designs(state, en(state)).get(self.current()).filter(|_| self.current() > 0)
    }

    /// `Design_Kilep` towards another game screen: the closing runs out (`desframe` down by 1.2 a tick) before the
    /// screen goes; true = gone now. (Towards the messages, help or options screen it goes at once, silently.)
    pub fn leave(&mut self) -> bool {
        if !self.animated {
            return true;
        }
        if !self.closing && !self.leaving {
            self.leaving = true;
            self.sounds.push(des(3));
        }
        false
    }

    /// The closing has finished.
    pub fn closed(&self) -> bool {
        self.closing && self.frame <= 0.0
    }

    /// Hold the screen at rest (snapshots).
    pub fn hold_open(&mut self) {
        self.frame = 60.0;
        self.opened = true;
        self.started = true;
    }

    /// One UI tick: `FRISSIT_Design2`, then `Design3DUpdate`'s state (the exe runs that per drawn frame; one frame a
    /// tick here, as the captures run).
    pub fn step(&mut self, state: &SaveState, rules: &ig2_rules::Rules) {
        self.frames += 1;
        self.held_pod(state, rules);
        if self.current() != self.pod_of {
            self.pod = 0;
            self.pod_of = self.current();
        }
        // `FRISSIT_Design2`.
        self.keringeto += 0.0006;
        self.des_xy += 0.02;
        if self.animated && !self.closing && self.started && !self.opened {
            self.frame = (self.frame + 0.8).min(60.0);
        }
        if self.animated && self.closing {
            self.frame = (self.frame - 1.2).max(0.0);
        }
        for f in &mut self.fade {
            *f = (*f + 1).min(100);
        }
        // `DesignKeret1(Kord1X − 5, Kord1Y + row·15 − 0x12, 0x72, 0xd)` around the chosen row, asked every frame.
        if self.opened && !self.closing && self.pointer > self.first && self.pointer <= self.count().min(self.first + ROWS) {
            let y = KORD1[1] as i32 + (self.pointer - self.first) as i32 * 15 - 0x12;
            self.list_frame.ask([KORD1[0] as i32 - 5, y, 0x72, 0xd]);
        }
        self.list_frame.tick();
        // `Design3DUpdate` (`AktAlap` as the last frame's `DesignGfx2D_1` left it).
        if self.loaded != self.model {
            self.add_dir = 1;
        }
        if self.opened {
            if self.add_dir == -1 {
                self.add_zoom -= 40.0;
                if self.add_zoom <= 0.0 {
                    self.add_zoom = 0.0;
                    self.add_dir = 0;
                    self.settled = true;
                }
            }
            if self.add_dir == 1 {
                self.add_zoom += 40.0;
                // Capped also while closing (the cap comes before the `desBECSUKAS` test).
                if self.add_zoom >= ADD_ZOOM_MAX {
                    self.add_zoom = ADD_ZOOM_MAX;
                }
                if self.add_zoom >= ADD_ZOOM_MAX && !self.closing {
                    // `HideAlap(OldAlap); OldAlap = AktAlap; LoadAlapVektor(AktAlap)`: a new hull, turned to the start
                    // orientation, and its sound.
                    self.loaded = self.model;
                    self.turn = scene::create_matrix(HULL_TURN.0, HULL_TURN.1, HULL_TURN.2);
                    if self.model > 0 {
                        self.sounds.push(des(1));
                    }
                    self.add_dir = -1;
                }
            }
        }
        self.started = true;
        if self.frame == 60.0 {
            self.wire += 0.25;
            if self.wire >= 10.0 {
                self.wire = 0.0;
            }
        }
        if (!self.opened || self.closing) && self.frame >= 60.0 {
            self.opened = true;
        }
        self.gy = self.gy.map(|g| g * 0.8);
        // The spin: a thrown hull keeps turning by `desGyX/Y` (decaying), dragged it follows the pointer, and an idle
        // one turns 0.002 a frame about the vertical.
        if self.opened && self.loaded != -1 {
            match self.drag {
                None => {
                    self.rotate(self.gy[0], self.gy[1]);
                    self.rotate(0.002, 0.0);
                }
                Some(old) => {
                    self.rotate(self.gy[0], self.gy[1]);
                    self.gy[0] = self.gy[0] * 0.9 + (old[0] - self.mouse[0]) / 30.0 * 0.1;
                    self.gy[1] = self.gy[1] * 0.9 + (old[1] - self.mouse[1]) / 30.0 * 0.1;
                    self.drag = Some(self.mouse);
                }
            }
        }
        // `DesignGfx2D_1` (drawn inside `Update3D`, after the above): `AktAlap = Alapok[d.+0x44]+8`.
        if let Some(d) = self.record(state) {
            let hull = i32_at(d, field::HULL).clamp(0, 24) as usize;
            self.model = rules.hulls.get(hull).map_or(-1, |h| h.id);
        }
        if self.leaving {
            self.leaving = false;
            self.closing = true;
            self.add_dir = 1;
        }
    }

    /// `matrixforgat(M, out, x, y)` (ELF 0x4523b0): turned by `√(x² + y²)` about `(y, −x, 0)` (`(1, 0, 0)` when
    /// that is zero), after the old rotation.
    fn rotate(&mut self, x: f32, y: f32) {
        let a = (((y * y + x * x) as f64).sqrt()) as f32;
        let axis = if x == 0.0 && y == 0.0 { [1.0, 0.0, 0.0] } else { [y, -x, 0.0] };
        self.turn = scene::mul(&scene::axis_matrix(axis, a), &self.turn);
    }

    /// The end of `Design_Main`: the chosen row kept in the window.
    fn scroll_to_pointer(&mut self) {
        if self.pointer <= self.first && self.pointer > 0 {
            self.first = self.pointer - 1;
        }
        if self.first + ROWS <= self.pointer {
            self.first = self.pointer - ROWS;
        }
    }

    /// The hot-spot under `at` (`AktIkon2`): the viewing mode's spots that are on (`Design_MouseInit2`).
    pub fn spot(&self, state: &SaveState, techs: &TechTable, rules: &ig2_rules::Rules, mask: &[u8], at: [f32; 2]) -> u32 {
        let (x, y) = (at[0] as i32, at[1] as i32);
        let colour = if (0..640).contains(&x) && (0..480).contains(&y) { mask.get((y * 640 + x) as usize).copied().unwrap_or(0) } else { 0 };
        for &(id, c, r, _) in &SPOTS {
            if !self.spot_on(state, techs, rules, id) || !crate::kit::inside(r, at) {
                continue;
            }
            if c == 0 || c == colour {
                return id;
            }
        }
        0
    }

    /// `Design_MouseInit2` in the viewing mode: 1 always; 2..=4 with a design whose hull tech isn't obsolete and the
    /// design itself isn't; 9; 10 with more than 15 designs (`GFX_Design`); 0x54 when the hull has pods.
    /// Only 0x54 waits for the screen to be open (`desKINYITVA && !desBECSUKAS`); the others are live while it opens
    /// and closes.
    pub fn spot_on(&self, state: &SaveState, techs: &TechTable, rules: &ig2_rules::Rules, id: u32) -> bool {
        (id != 0x54 || (self.opened && !self.closing)) && self.spot_state(state, techs, rules, id)
    }

    /// `Design_MouseInit2`'s on/off state for `id`; the 3D buttons keep the last state it set while the screen
    /// closes (`FrissitGombTexturak` isn't rerun).
    pub fn spot_state(&self, state: &SaveState, techs: &TechTable, rules: &ig2_rules::Rules, id: u32) -> bool {
        match id {
            1 | 9 => true,
            2..=4 => self.record(state).is_some_and(|d| {
                let hull = i32_at(d, field::HULL).clamp(0, 24) as usize;
                let tech = rules.hulls.get(hull).map_or(0, |h| h.tech).max(0) as usize;
                techs.byte(tech, 0x26) & 1 == 0 && d[field::OBSOLETE] == 0
            }),
            10 => self.count() > ROWS,
            0x54 => self.record(state).is_some_and(|d| pod_count(rules, i16_at(d, field::RACE), i32_at(d, field::HULL)) > 0),
            _ => false,
        }
    }

    /// The left button went down at `at` (`Mouse_Lenyom` in `Design_Main`; `AlapfolottLenyomva` starts a drag).
    pub fn press(&mut self, state: &SaveState, techs: &TechTable, rules: &ig2_rules::Rules, mask: &[u8], at: [f32; 2]) {
        self.mouse = at;
        let id = self.spot(state, techs, rules, mask, at);
        match id {
            9 => {
                self.sounds.push(CLICK);
                self.sounds.push(des(2));
                self.held = Some(9);
                self.pick_row(at);
            }
            // The scroll bar has no sound.
            10 => {
                self.held = Some(10);
                self.scroll_bar(at);
            }
            0x54 => {
                self.held = Some(0x54);
                if self.pick_pod(state, rules, at) {
                    self.sounds.push(CLICK);
                }
            }
            // New and delete sound on the press; redesign and modify act on the release (`onRelease`), where
            // `RedesignPlan`/`ModifyPlan` click only on a press: silent. What they start is part 2 (editing).
            1 | 4 => self.sounds.push(CLICK),
            _ => {}
        }
    }

    /// Spot 0x54 while viewing: the pod row under the pointer (`(podkezdx, +0x73) × (podkezdy + 15k, +15]`) becomes
    /// `DesignAktPod`.
    fn pick_pod(&mut self, state: &SaveState, rules: &ig2_rules::Rules, at: [f32; 2]) -> bool {
        let Some(d) = self.record(state) else { return false };
        let n = pod_count(rules, i16_at(d, field::RACE), i32_at(d, field::HULL));
        for k in 1..=n {
            let y = POD[1] + 15.0 * k as f32;
            if at[0] > POD[0] && at[0] < POD[0] + 115.0 && at[1] > y && at[1] <= y + 15.0 {
                self.pod = k;
                return true;
            }
        }
        false
    }

    /// The right button went down: `AlapfolottLenyomva` (ELF 0x503d20) starts turning the hull by hand.
    pub fn press_right(&mut self, at: [f32; 2]) {
        self.mouse = at;
        if self.opened && !self.closing && self.over_hull(at) {
            self.drag = Some(at);
        }
    }

    /// The pointer moved (held: the list and the scroll bar follow it).
    pub fn moved(&mut self, at: [f32; 2]) {
        self.mouse = at;
        self.tip.moved(at);
        match self.held {
            Some(9) => self.pick_row(at),
            Some(10) => self.scroll_bar(at),
            _ => {}
        }
    }

    /// Held on the pod panel, the pointer picks the row it is on (`Design_Main` each frame).
    pub fn held_pod(&mut self, state: &SaveState, rules: &ig2_rules::Rules) {
        if self.held == Some(0x54) {
            let at = self.mouse;
            self.pick_pod(state, rules, at);
        }
    }

    /// A button went up (`Mouse_Felenged`).
    pub fn release(&mut self) {
        self.held = None;
        self.drag = None;
    }

    /// `AlapfolottLenyomva` (ELF 0x503d20): a hull is loaded and slid in, and the pointer is within 160 pixels of
    /// its projected centre (`GetObjCenter` + `TransformPoint`; here its place (0, −15, −1400)).
    fn over_hull(&self, at: [f32; 2]) -> bool {
        let c = [320.0, 600.0 * -15.0 / 1400.0 + 240.0];
        let d = (((c[0] - at[0]) * (c[0] - at[0]) + (c[1] - at[1]) * (c[1] - at[1])) as f64).sqrt();
        self.loaded != -1 && self.add_zoom == 0.0 && d < 160.0
    }

    /// Hot-spot 9: `AktTervMutato = (my − Kord1Y + 5)/15 + Kezdet + 1`, kept on a shown row.
    fn pick_row(&mut self, at: [f32; 2]) {
        let row = ((at[1] as i32 - KORD1[1] as i32 + 5) / 15).max(0) as usize;
        let n = self.count();
        let m = (row + self.first + 1).min(n).min(self.first + ROWS).max(self.first + 1);
        if m <= n {
            self.pointer = m;
        }
        self.scroll_to_pointer();
    }

    /// Hot-spot 10: the thumb follows the pointer (`h = 3585 / n` here, the drawing uses 3510).
    fn scroll_bar(&mut self, at: [f32; 2]) {
        let n = self.count() as i64;
        if n < 16 {
            self.first = 0;
        } else {
            let h = 3585 / n;
            let span = 0xef - h;
            let k = (((at[1] as i64 - 0x22) - h / 2) * (n - 15) + span / 2) / span;
            self.first = k.clamp(0, n - 15) as usize;
            self.pointer = self.pointer.clamp(self.first + 1, self.first + ROWS);
        }
        self.scroll_to_pointer();
    }

    /// `DesignKeyboardHandler`'s up and down.
    pub fn key(&mut self, k: Key) {
        match k {
            Key::Up if self.pointer > 1 => {
                let m = self.pointer - 1;
                if m <= self.first {
                    self.first = self.pointer.saturating_sub(2);
                }
                self.pointer = m;
            }
            Key::Down if self.pointer < self.count() => {
                self.pointer += 1;
                if self.first + ROWS < self.pointer {
                    self.first += 1;
                }
            }
            _ => {}
        }
    }

    /// The tooltip: the hot-spot under the pointer once it rested (`UpdateFrame`).
    pub fn tooltip(&mut self, state: &SaveState, techs: &TechTable, rules: &ig2_rules::Rules, mask: &[u8], tips: &[Vec<u8>], now: f64) {
        let under = self.spot(state, techs, rules, mask, self.tip.mouse);
        let len = tips.get(under as usize).map_or(0, |t| t.len());
        self.tip.update(now, under, len, self.held.is_some());
    }

    /// `DesignGetFaderCoordinates` (ELF 0x4ff870): the five panels `[x, y, w, h]` (`t` = 1, closing `2f/60 − 1`).
    fn faders(&self) -> [[f32; 4]; 5] {
        let t = if self.closing { self.frame / 60.0 * 2.0 - 1.0 } else { 1.0 };
        let (y1, h1) = (209.0 - t * 117.0, t * 234.0);
        let (y2, h2) = (398.0 - t * 33.0, t * 72.0);
        [
            [5.0, y1, 124.0, h1],
            [471.0, y2, 166.0, h2],
            [5.0, y2, 174.0, h2],
            [499.0, y1, 136.0, h1],
            [320.0 - t * 160.0, 51.0 - t * 29.0, t * 320.0, t * 57.0],
        ]
    }

    /// The faders (`Design3DUpdate`'s `KirakFader` 0..=4, while past frame 30 or not closing) and the scroll bar
    /// (`GFX_Design`): fill (0, 54, 90)·0.9 at `s·155 + 100`, edge (0, 90, 130)·0.9 at `s·225 + 30`, `s = 1 − f/60`
    /// (0 closing).
    pub fn draw_faders(&self, f: &mut Frame, d: &Draw2d) {
        let s = if self.closing { 0.0 } else { 1.0 - self.frame / 60.0 };
        let (ft, et) = (s * 155.0 + 100.0, s * 225.0 + 30.0);
        let b = 0.9;
        let (fill, edge) = ([0.0, 54.0 * b, 90.0 * b], [0.0, 90.0 * b, 130.0 * b]);
        if self.frame > 30.0 || !self.closing {
            for r in self.faders() {
                crate::kit::fader::kirak_fader(f, d.white, &d.v, r, fill, ft, edge, et);
            }
        }
        if !self.opened || self.closing {
            return;
        }
        let n = self.count() as i64;
        let (h, off) = if n < 16 { (0xea, 0) } else { let h = 3510 / n; (h, self.first as i64 * (0xea - h) / (n - 15)) };
        if h < 0xea {
            let [x, y] = CSUSZKA.map(|c| c as f32);
            let fill = [0.0, 54.0, 90.0];
            let edge = [0.0, 90.0, 130.0];
            crate::kit::fader::fader(f, d.white, &d.v, [x, y, 10.0, 234.0], fill, ft, edge, et);
            crate::kit::fader::fader(f, d.white, &d.v, [x + 1.0, y + 1.0 + off as f32, 8.0, h as f32 - 2.0], fill, ft, edge, et);
        }
    }

    /// `DesignGfx2D_1` (only while not closing): the list, its frame, the data panels and the pod rows.
    pub fn draw_texts(&self, f: &mut Frame, d: &Draw2d, pics: &crate::hud::Pictures, state: &SaveState, techs: &TechTable, rules: &ig2_rules::Rules) {
        if self.closing {
            return;
        }
        let e = en(state);
        let all = designs(state, e);
        let label = |k: u16| {
            let mut s = d.texts.get(5, 2, k);
            s.push(b':');
            s
        };
        let col = |c: usize, pct: i32| d.palette.get(c).copied().unwrap_or([1.0; 3]).map(|x| x * pct as f32 / 100.0);
        let text = |f: &mut Frame, s: &[u8], x: f32, y: f32, len: usize, c: usize, pct: i32| d.font.text(f, &d.v, s, x, y, len, 2, Align::Left, col(c, pct), None);
        let int = |f: &mut Frame, n: i64, x: f32, y: f32, len: usize, right: bool, c: usize, pct: i32| {
            d.font.int(f, &d.v, n, x, y, len, 2, if right { Align::Right } else { Align::Left }, col(c, pct))
        };
        let pct = |n: usize| if n < ROWS { self.fade[n].max(0) } else { 100 };
        let hull_obsolete = |r: &[u8]| {
            let t = i32_at(r, field::HULL_TECH).clamp(0, 192) as usize;
            techs.byte(t, 0x26) & 1 != 0
        };
        // The list.
        let n = self.count();
        for i in self.first + 1..=n.min(self.first + ROWS) {
            let Some(r) = all.get(self.order[i]) else { continue };
            let c = if hull_obsolete(r) || r[field::OBSOLETE] != 0 { 0xc } else { 1 };
            let row = i - self.first;
            text(f, name(r), KORD1[0], KORD1[1] + row as f32 * 15.0 - 15.0, 16, c, pct(i - self.first - 1));
        }
        if self.opened && self.pointer > self.first && self.pointer <= n.min(self.first + ROWS) {
            self.list_frame.draw(f, d.white, &d.v, col(0x22, 100));
        }
        let Some(r) = self.record(state) else { return };
        let tech_name = |t: i32| crate::hud::tech_name(d.texts, t.clamp(0, 192) as usize);
        // The bottom-left panel: name, speed, firepower, hit points.
        text(f, &label(1), KORD2X, KORD2Y[0], 7, 1, pct(8));
        text(f, name(r), KORD3X, KORD2Y[0], 16, 2, pct(9));
        text(f, &label(2), KORD2X, KORD2Y[1], 7, 1, pct(10));
        if r[field::NO_SPEED] != 0 {
            text(f, b"-", KORD3X, KORD2Y[1], 4, 2, pct(11));
        } else {
            let v = f32::from_le_bytes(r[field::SPEED..field::SPEED + 4].try_into().unwrap());
            let speed = if r[field::QUARTER_SPEED] != 0 { ((v * 100.0 + 0.5) as i32 as f64 / 4.0) as i64 } else { (v * 100.0 + 0.5) as i64 };
            int(f, speed, KORD3X, KORD2Y[1], 4, false, 2, pct(11));
        }
        text(f, &label(3), KORD2X, KORD2Y[1] + 15.0, 10, 1, pct(12));
        int(f, i32_at(r, field::FIREPOWER) as i64, KORD2X + 70.0, KORD2Y[1] + 15.0, 5, false, 2, pct(13));
        text(f, &label(4), KORD2X, KORD2Y[1] + 30.0, 10, 1, pct(12));
        int(f, i32_at(r, field::HP) as i64, KORD2X + 70.0, KORD2Y[1] + 30.0, 5, false, 2, pct(13));
        // The top panel: hull, production time, price, units, tank capacity.
        text(f, &label(5), 168.0, 27.0, 6, 1, pct(0));
        text(f, &tech_name(i32_at(r, field::HULL_TECH)), 215.0, 27.0, 16, 2, pct(1));
        text(f, &label(6), 327.0, 27.0, 16, 1, pct(2));
        let time = ((i32_at(r, field::EFFORT) as f64 / 0.03) / 1000.0 / 50.0) as i64;
        int(f, time, 423.0, 27.0, 8, true, 2, pct(3));
        text(f, &label(7), 168.0, 42.0, 14, 1, pct(4));
        int(f, i32_at(r, field::PRICE) as i64, 215.0, 42.0, 10, false, 2, pct(5));
        text(f, &label(8), 327.0, 42.0, 19, 1, pct(6));
        int(f, units_of(state, e, self.current()) as i64, 445.0, 42.0, 4, true, 2, pct(7));
        if r[field::TANKS] != 0 {
            text(f, &label(9), 327.0, 57.0, 19, 1, pct(6));
            int(f, r[field::TANKS] as i64, 445.0, 57.0, 4, true, 2, pct(7));
        }
        // The bottom-right panel: with no pod chosen (`DesignAktPod < 1`) "POD: ---" and the hull's name; else the
        // pod's class, its tech (or "---"), the count fitted and how many it takes.
        let (race, hull) = (i16_at(r, field::RACE), i32_at(r, field::HULL));
        if self.pod < 1 {
            text(f, &label(10), 490.0, KORD2Y[0], 4, 1, pct(7));
            text(f, b"---", 520.0, KORD2Y[0], 16, 2, pct(8));
            text(f, &label(0xb), 490.0, KORD2Y[1], 5, 1, pct(9));
            text(f, &tech_name(i32_at(r, field::HULL_TECH)), 520.0, KORD2Y[1], 16, 2, pct(10));
        } else {
            let k = self.pod;
            text(f, &label(10), 490.0, KORD2Y[0], 6, 1, pct(7));
            text(f, &pod_type_name(d.texts, rules, race, hull, k), 520.0, KORD2Y[0], 16, 2, pct(8));
            let tech = r[0x50 + k] as i32;
            text(f, &label(0xb), 490.0, KORD2Y[1], 5, 1, pct(9));
            text(f, &label(0xc), 490.0, KORD2Y[2], 6, 1, pct(11));
            text(f, &label(0xd), 560.0, KORD2Y[2], 4, 1, pct(13));
            if tech == 0 {
                text(f, b"---", 520.0, KORD2Y[1], 16, 2, pct(10));
                text(f, b"---", 530.0, KORD2Y[2], 2, 2, pct(12));
                text(f, b"---", 591.0, KORD2Y[2], 3, 2, pct(14));
            } else {
                text(f, &tech_name(tech), 520.0, KORD2Y[1], 16, 2, pct(10));
                int(f, r[0x5d + k] as i64, 530.0, KORD2Y[2], 2, false, 2, pct(12));
                int(f, max_fitted(rules, techs, race, hull, k, tech) as i64, 591.0, KORD2Y[2], 2, false, 2, pct(14));
            }
        }
        if !self.opened {
            return;
        }
        // The pod rows (`SubZoomDes == 0`), squeezed by `z = AddZoomDes / ADDZOOMMAXDES` while the hull slides.
        let z = self.add_zoom / ADD_ZOOM_MAX;
        for k in 1..=pod_count(rules, race, hull) {
            let y = POD[1] + 15.0 * k as f32;
            let tech = r[0x50 + k] as i32;
            let count = r[0x5d + k] as i64;
            if tech == 0 {
                let n1 = ((1.0 - z) * 13.0) as usize;
                if n1 >= 1 {
                    text(f, &pod_type_name(d.texts, rules, race, hull, k), POD[0] + z * 50.0 - 4.0, y + 5.0, n1, 0xc, 100);
                }
                let n2 = ((1.0 - z) * 5.0) as usize;
                if n2 >= 1 {
                    text(f, &d.texts.get(5, 2, 0xe), POD[0] - 6.0 + 96.0 - 35.0 * z + 4.0, y + 5.0, n2, 2, 100);
                }
            } else {
                let n = ((1.0 - z) * 16.0) as usize;
                text(f, &tech_name(tech), POD[0] + 50.0 * z - 4.0, y + 5.0, n, 1, 100);
                let max = max_fitted(rules, techs, race, hull, k, tech) as i64;
                if z < 0.6 {
                    let cells = if count < 10 { 1.0 } else { 2.0 };
                    let x = POD[0] + 96.0 + 12.0 - (cells * 6.0) / 2.0 - 35.0 * z;
                    if count == 0 {
                        text(f, b"-", x, y + 5.0, 2, 1, 100);
                    } else {
                        int(f, count, x, y + 5.0, 2, false, if count >= max { 0x22 } else { 1 }, 100);
                    }
                }
                // The minus and plus cells (`BrushToScreenXY`): greyed while viewing (editing with more than one
                // fitting: live, part 2).
                let brushes = &rules.design_screen.brushes;
                if z < 0.8 {
                    if let Some(b) = brushes.get(2) {
                        pics.brush(f, &d.v, b, 1, Some([POD[0] + 96.0 - 4.0 - 35.0 * z, y + 4.0]), [1.0; 4]);
                    }
                }
                if z < 0.2 {
                    if let Some(b) = brushes.get(3) {
                        pics.brush(f, &d.v, b, 1, Some([POD[0] + 96.0 + 19.0 - 35.0 * z, y + 4.0]), [1.0; 4]);
                    }
                }
            }
        }
    }
}

/// The record's name (`+0`, up to 0x16 bytes, NUL-ended).
pub fn name(r: &[u8]) -> &[u8] {
    let s = &r[..0x16];
    &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())]
}

/// `GetMaxPodok(race, hull)` (ELF 0x4fb390): the hull's pod slots for the race's pod table.
pub fn pod_count(rules: &ig2_rules::Rules, race: i16, hull: i32) -> usize {
    pod_slots(rules, race, hull).iter().filter(|&&c| c > 0).count()
}

/// `PodFajok[table − 1][hull][0..12]` (`HanyasFajPodTablazatatHasznalja[race]`): each slot's pod class.
fn pod_slots(rules: &ig2_rules::Rules, race: i16, hull: i32) -> Vec<i32> {
    rules.design_screen.pod_slots(race.max(0) as usize, hull.max(0) as usize)
}

/// `min(GetMaxRarakhato(race, hull, k), Talalmanyok[tech].+0x24)`: how many of `tech` pod `k` takes.
fn max_fitted(rules: &ig2_rules::Rules, techs: &TechTable, race: i16, hull: i32, k: usize, tech: i32) -> i32 {
    rules.design_screen.pod_max(race.max(0) as usize, hull.max(0) as usize, k).min(techs.byte(tech.clamp(0, 192) as usize, 0x24) as i32)
}

/// `PodTipusok[KeresPod(class)]`'s name (`NEWGAME_Design`): an empty slot's class.
fn pod_type_name(texts: &crate::ui::UiTexts, rules: &ig2_rules::Rules, race: i16, hull: i32, k: usize) -> Vec<u8> {
    let class = pod_slots(rules, race, hull).get(k - 1).copied().unwrap_or(0);
    let mut s = rules.design_screen.pod_type_text(class).map(|(sec, i)| texts.get(0x13, sec, i)).unwrap_or_default();
    s.truncate(0xe);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fader_rects_fold_to_the_middle_closing() {
        let mut s = Design { closing: false, ..blank() };
        assert_eq!(s.faders()[0], [5.0, 92.0, 124.0, 234.0]);
        assert_eq!(s.faders()[4], [160.0, 22.0, 320.0, 57.0]);
        s.closing = true;
        s.frame = 30.0;
        assert_eq!(s.faders()[0], [5.0, 209.0, 124.0, 0.0]);
    }

    #[test]
    fn up_and_down_keep_the_pointer_in_the_window() {
        let mut s = Design { order: (0..=20).collect(), pointer: 15, opened: true, ..blank() };
        s.key(Key::Down);
        assert_eq!((s.pointer, s.first), (16, 1));
        for _ in 0..20 {
            s.key(Key::Down);
        }
        assert_eq!((s.pointer, s.first), (20, 5));
        // Up scrolls only once the pointer passes the window's top row (`Kezdet = AktTervMutato − 2`).
        for _ in 0..5 {
            s.key(Key::Up);
        }
        assert_eq!((s.pointer, s.first), (15, 5));
        for _ in 0..10 {
            s.key(Key::Up);
        }
        assert_eq!((s.pointer, s.first), (5, 4));
    }

    #[test]
    fn the_idle_hull_turns_about_the_vertical() {
        let mut s = blank();
        s.turn = scene::IDENTITY;
        s.rotate(0.002, 0.0);
        // About (0, −1, 0) by 0.002: x leans to +z.
        let v = scene::apply(&s.turn, [1.0, 0.0, 0.0]);
        assert!((v[2] - 0.002).abs() < 1e-5 && v[1].abs() < 1e-7, "{v:?}");
    }

    fn blank() -> Design {
        Design {
            frame: 60.0,
            opened: true,
            closing: false,
            leaving: false,
            animated: true,
            started: true,
            keringeto: 0.0,
            des_xy: 0.0,
            wire: 0.0,
            fade: [100; ROWS],
            order: vec![0],
            first: 0,
            pointer: 0,
            add_zoom: 0.0,
            add_dir: 0,
            model: -1,
            loaded: -1,
            settled: true,
            turn: scene::IDENTITY,
            gy: [0.0; 2],
            drag: None,
            pod: 0,
            pod_of: 0,
            mouse: [0.0; 2],
            held: None,
            list_frame: DesignFrame::default(),
            tip: Tooltip::default(),
            frames: 0,
            sounds: Vec::new(),
        }
    }
}
