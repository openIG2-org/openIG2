//! The design screen (`AktKep` 5, re/screens/design.md), part 1: viewing. `Design_Belep` (ELF 0x5074e0 / exe
//! 0x4b1be0) and `Design_Kilep` (0x507bb0 / exe 0x4b1f20) with the opening and closing (`desframe`), the per-tick
//! `FRISSIT_Design2` (0x50d890) and the state half of `Design3DUpdate` (0x503ea0 / exe 0x4ad8e0: the hull sliding in
//! and spinning), the 2D pass `DesignGfx2D_1` (0x4fb840 / exe 0x4af440: the design list, its frame, the data panels,
//! the pod rows), the faders and the scroll bar (`GFX_Design` 0x507ed0), and the viewing input of `Design_Main`
//! (0x509140 / exe 0x4b2190: the list, the scroll bar, dragging the hull) and `DesignKeyboardHandler` (0x508dc0: up
//! and down). The 3D scene is [`crate::design_scene`].
//!
//! The editing half (the modes 1–3, the popups, the name input, the orders) is [`crate::design_edit`].
//!
//! Part 3 (the 3D buttons' press frames, the pods' boxes, lights and pick, the tech models opening out of a pod, the
//! pod and tech rows' `DesignKeret2` frames) is [`crate::design_pods`]; the opening's lasers are below.

use ig2_formats::Install;
use ig2_rules::sounds::Table;
use ig2_sim::colony_side::TechTable;
use ig2_sim::save::{va, SaveState};

use crate::audio::{SoundId, CLICK};
use crate::design_edit::{Ctx, Names, NameEntry, Popup};
use crate::font::Align;
use crate::design_pods::Pods;
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
pub(crate) const KORD2Y: [f32; 3] = [375.0, 390.0, 405.0];
pub(crate) const KORD3X: f32 = 62.0;
const CSUSZKA: [i32; 2] = [137, 92];
pub(crate) const POD: [f32; 2] = [508.0, 80.0];
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

/// `Design_MouseInit`'s hot-spots (exe 0x4b3810, `Mouse_Szoveg_Set(id, colour, x, y, w, h, GetString(5, 1, tip))`):
/// `(id, mask colour, rect, tip)`, in id order (the first hit wins). Colour 0 = the whole rectangle. The pod cells
/// 0x35..=0x52 (row k: `0x33 + 2k` −, `0x34 + 2k` +) come between 10 and 0x54 ([`cell_rect`]).
pub const SPOTS: [(u32, u8, [i32; 4], u16); 12] = [
    (1, 2, [0, 0x128, 0x42, 0x30], 1),
    (2, 3, [0x41, 0x127, 0x43, 0x31], 2),
    (3, 4, [0, 0x157, 0x42, 0x31], 3),
    (4, 5, [0x41, 0x157, 0x44, 0x32], 4),
    (5, 2, [0, 0x128, 0x42, 0x30], 5),
    (6, 3, [0x41, 0x127, 0x43, 0x31], 6),
    (7, 4, [0, 0x157, 0x42, 0x31], 7),
    (8, 5, [0x41, 0x157, 0x44, 0x32], 8),
    (9, 0, [5, 0x5d, 0x7b, 0xe9], 9),
    (10, 0, [CSUSZKA[0], CSUSZKA[1], 10, 0xea], 10),
    (0x54, 0, [0x1f2, 0x5c, 0x8a, 0xea], 0xb),
    (0x56, 0, [KORD3X as i32 - 3, KORD2Y[0] as i32 - 3, 0x66, 0xe], 0xc),
];

/// Pod cell `id`'s rectangle (`0x33 + 2k`: `(podkezdx + 0x55, podkezdy + 1 + 15k, 0x14, 0xe)`, `+ 0x6e` for the plus).
pub fn cell_rect(id: u32) -> [i32; 4] {
    let k = ((id - 0x33) / 2) as i32;
    let x = if (id - 0x33) % 2 == 0 { 0x55 } else { 0x6e };
    [POD[0] as i32 + x, POD[1] as i32 + 1 + 15 * k, 0x14, 0xe]
}

/// The tooltip texts by hot-spot id (`GetString(5, 1, tip)`; the cells and the 3D buttons `0xd` −, `0xe` +).
pub fn tips(texts: &crate::ui::UiTexts) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(); 0x57];
    for &(id, _, _, k) in &SPOTS {
        out[id as usize] = texts.get(5, 1, k);
    }
    // The 3D minus and plus (`Design_MouseInit2`).
    out[0x20] = texts.get(5, 1, 0xd);
    out[0x1f] = texts.get(5, 1, 0xe);
    for id in 0x33..=0x52u32 {
        out[id as usize] = texts.get(5, 1, if (id - 0x33) % 2 == 0 { 0xd } else { 0xe });
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
    /// `desMessagesUtaniLegelsoBejoves` (exe 0xa3f0c0): back from the messages, help or options screen, until the
    /// end of the first `Design3DUpdate` (no hull drag starts on that frame).
    back: bool,
    /// `NemkellTexturaCsere` (exe 0xa3f6ec): `Design_Main` chose another design (not an order's event or a key);
    /// the hull slides out at the next `Design3DUpdate` even before `AktAlap` follows; off when it is all out.
    pub(crate) retexture: bool,
    /// The left button's input waiting for this tick's `Design_Main` (the exe runs it after the frame's 3D update and
    /// draw; the edges `KirakFader` and the pick read are taken at once).
    main: Vec<MainInput>,
    /// The orders `Design_Main` sent, with the tick they reach `MULTIGET_Design`: the exe's own messages come back
    /// at the fourth frame's start [obs: the OK's 0x33 sent at 521 changes `AktTerv` at 525, the sale's 0x34 at 631
    /// at 635; probes/design_zoom.js].
    in_flight: Vec<(u64, ig2_sim::game::DesignOrder)>,
    /// `DesignAktPod` (0 = none) and the design it belongs to (a new design clears it, `Design_Main`).
    pub pod: usize,
    pod_of: usize,
    /// The pointer and the hot-spot the left button holds (`Mouse_Megfogva`).
    pub mouse: [f32; 2],
    pub(crate) held: Option<u32>,
    /// `desf`: the faders' shine, drift and hover glow (`FaderDesignFrissito` in `FRISSIT_Design2`), and the left
    /// button (down, this frame's press and release edges) as `KirakFader` reads it.
    faders: crate::kit::FaderDesign,
    buttons: (bool, bool, bool),
    pub list_frame: DesignFrame,
    pub tip: Tooltip,
    /// The text of the tooltip 0x0c (the tech model's name).
    pub tip_text: Vec<u8>,
    /// UI ticks since the screen opened.
    pub frames: u64,
    pub sounds: Vec<SoundId>,
    /// `TutorialCheckEvent` ids the screen raised (0xb: laser 2.0 on a hull of tech 5; 0xc: OK on hull model 6 with
    /// a pod fitted), for the window to carry out.
    pub tutorial: Vec<i32>,
    /// `DesignTervezesMod` (0 view, 1 new, 2 redesign, 3 modify), the scratch `DesignTervTervezett`, `CanModify`.
    pub mode: u8,
    pub scratch: [u8; 0x84],
    pub can_modify: bool,
    /// The open popup and its row under the pointer (`dSelected`, −1 none).
    pub popup: Option<Popup>,
    pub selected: i32,
    /// `Design_Main::KezdoIdo` (the held −/+).
    pub(crate) repeat: u32,
    /// The name input (`IOMode`, `IOAzonosito` 0x33).
    pub entry: Option<NameEntry>,
    /// The orders the screen sends (0x33, 0x34), taken by the window.
    pub orders: Vec<ig2_sim::game::DesignOrder>,
    pub names: Names,
    /// `DesMelyikGombot` (the 3D button pressed last, −1 none).
    pub button: i32,
    /// `SubZoomDes` (the tech models opening out of a pod, 0..=1000).
    pub sub_zoom: f32,
    /// Part 3: the pods' boxes, lights and pick, the tech models, the button frames ([`crate::design_pods`]).
    pub pods: Pods,
    /// `DesignKeret2`'s frame (the pod and tech rows).
    pub pod_frame: DesignFrame,
    /// `DesignGfx2D_1`'s opening lasers and sparks, and their glow texture (`laservago`, from the scene).
    pub lasers: crate::kit::Lasers,
    pub laser_glow: Option<crate::render::TexId>,
    /// `TutorialPaused` (delete is off).
    pub tutorial_paused: bool,
    /// Order events for the next frame (`MultiSendPackets` before its `Design3DUpdate`: the tick's `Design_Main`
    /// sent them).
    pub(crate) events: Vec<ig2_sim::game::OrderEvent>,
}

/// Frames from the `Design_Main` that sends an order to the one whose start applies it.
const ORDER_FRAMES: u64 = 4;

/// The left button's input as `Design_Main` takes it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum MainInput {
    Press([f32; 2]),
    /// The right button's press: `Design_Main` acts on it like a left one but for spots 9 and 0x54, which test
    /// `Mouse_Bal` (exe 0x509140; FX3).
    PressRight([f32; 2]),
    Move([f32; 2]),
    Release,
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
            back: false,
            retexture: false,
            main: Vec::new(),
            in_flight: Vec::new(),
            pod: 0,
            pod_of: 0,
            mouse: [320.0, 240.0],
            held: None,
            faders: crate::kit::FaderDesign::new(1),
            buttons: (false, false, false),
            list_frame: DesignFrame::default(),
            tip: Tooltip::default(),
            tip_text: Vec::new(),
            frames: 0,
            sounds: Vec::new(),
            tutorial: Vec::new(),
            mode: 0,
            scratch: [0; 0x84],
            can_modify: false,
            popup: None,
            selected: -1,
            repeat: 0,
            entry: None,
            orders: Vec::new(),
            names: Names::default(),
            button: -1,
            sub_zoom: 0.0,
            pods: Pods::new(),
            pod_frame: DesignFrame::second(),
            lasers: crate::kit::Lasers::default(),
            laser_glow: None,
            tutorial_paused: false,
            events: Vec::new(),
        };
        s.scroll_to_pointer();
        s.back = !animated;
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
    pub fn step(&mut self, c: &Ctx, mask: &[u8]) {
        let (state, rules) = (c.state, c.rules);
        self.frames += 1;
        crate::design_scene::track_boxes(c, self);
        self.apply_events(c.state);
        if self.current() != self.pod_of {
            self.pod = 0;
            self.pod_of = self.current();
        }
        // `FRISSIT_Design2`.
        self.faders.tick();
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
        if self.loaded != self.model || self.retexture {
            self.add_dir = 1;
        }
        self.pods_begin(c);
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
                    self.pods_hull_loaded();
                    self.add_dir = -1;
                }
            }
        }
        // Step 13 (`AddZoomDes == ADDZOOMMAXDES`): `NemkellTexturaCsere = 0`.
        if self.add_zoom == ADD_ZOOM_MAX {
            self.retexture = false;
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
        self.back = false;
        // `DesignGfx2D_1` (drawn inside `Update3D`, after the above): `AktAlap = Alapok[d.+0x44]+8`.
        if let Some(d) = self.edited(state) {
            let hull = i32_at(d, field::HULL).clamp(0, 24) as usize;
            self.model = rules.hulls.get(hull).map_or(-1, |h| h.id);
        }
        self.pods_end(c, mask);
        // `DesignGfx2D_1` while opening (`!desKINYITVA`, `desframe/60` not 0 or 1): `DrawLaser(fader, 0x4c, 0x4c, 0x4c,
        // f/60)` round the five faders from the corners `LaserPos` (F3 with F0's y and height).
        self.lasers.tick();
        self.lasers.clear();
        if !self.closing && !self.opened && self.frame != 0.0 && self.frame != 60.0 {
            let from = [[0, 0], [639, 479], [0, 479], [639, 0], [320, 0]];
            for (r, at) in self.faders().into_iter().zip(from) {
                self.lasers.shoot(r.map(|q| q as i32), at, self.frame / 60.0, [0x4c; 3]);
            }
        }
        self.design_main(c, mask);
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
    pub(crate) fn scroll_to_pointer(&mut self) {
        if self.pointer <= self.first && self.pointer > 0 {
            self.first = self.pointer - 1;
        }
        if self.first + ROWS <= self.pointer {
            self.first = self.pointer - ROWS;
        }
    }

    /// The hot-spot under `at` (`AktIkon2`, `Mouse_Darabol` ELF 0x5b7a40): the lowest id that is on and either has
    /// colour 0 and its rectangle holds the pointer, or has the colour of the mask under it (its rectangle aside).
    pub fn spot(&self, c: &Ctx, mask: &[u8], at: [f32; 2]) -> u32 {
        let (x, y) = (at[0].clamp(0.0, 639.0) as i32, at[1].clamp(0.0, 479.0) as i32);
        let colour = mask.get((y * 640 + x) as usize).copied().unwrap_or(0);
        let p = [x as f32, y as f32];
        let hit = |id: u32, col: u8, r: [i32; 4]| self.spot_on(c, id) && if col == 0 { crate::kit::inside(r, p) } else { col == colour };
        let cells = (0x35..=0x52).map(|id| (id, 0, cell_rect(id)));
        let buttons = [(0x1f, 6, [0, 0, 0x20, 9]), (0x20, 1, [0, 0, 0x20, 9])];
        let mut all: Vec<(u32, u8, [i32; 4])> = SPOTS.iter().map(|&(id, col, r, _)| (id, col, r)).chain(buttons).chain(cells).collect();
        all.sort_by_key(|s| s.0);
        all.into_iter().find(|&(id, col, r)| hit(id, col, r)).map_or(0, |s| s.0)
    }

    /// `Design_MouseInit2`'s state, with 0x54 waiting for the screen to be open (`desKINYITVA && !desBECSUKAS`);
    /// the others are live while it opens and closes; every one is off while the tech models are out
    /// (`SubZoomDes != 0`: `Mouse_Szoveg_Off_Range(1, ..)`).
    pub fn spot_on(&self, c: &Ctx, id: u32) -> bool {
        self.sub_zoom == 0.0 && (id != 0x54 || (self.opened && !self.closing)) && self.spot_state(c.state, c.techs, c.rules, id)
    }

    /// `Design_MouseInit2`'s on/off state for `id`; the 3D buttons keep the last state it set while the screen
    /// closes (`FrissitGombTexturak` isn't rerun). Viewing: 1, 9 on; 2..=4 with a design whose hull tech isn't
    /// obsolete and the design itself isn't; 10 with more than 15 designs (`GFX_Design`). Editing: 5..=8; the 3D
    /// −/+ (0x20, 0x1f) on the chosen pod while `CanModify`, nothing slides and it takes more than one; the cells
    /// of the fitted pods taking more than one (`DesignGfx2D_1`). 0x54 when the hull has pods; 0x56 always.
    pub fn spot_state(&self, state: &SaveState, techs: &TechTable, rules: &ig2_rules::Rules, id: u32) -> bool {
        let editing = self.mode != 0;
        let d = self.edited(state);
        let fits = |k: usize| {
            d.is_some_and(|d| {
                let (race, hull, tech) = (i16_at(d, field::RACE), i32_at(d, field::HULL), d[0x50 + k] as i32);
                k >= 1 && k <= pod_count(rules, race, hull) && tech != 0 && max_fitted(rules, techs, race, hull, k, tech) > 1
            })
        };
        match id {
            1 | 9 => !editing,
            2..=4 => {
                !editing
                    && self.record(state).is_some_and(|d| {
                        let hull = i32_at(d, field::HULL).clamp(0, 24) as usize;
                        let tech = rules.hulls.get(hull).map_or(0, |h| h.tech).max(0) as usize;
                        techs.byte(tech, 0x26) & 1 == 0 && d[field::OBSOLETE] == 0
                    })
            }
            5..=8 => editing,
            10 => !editing && self.count() > ROWS,
            0x1f | 0x20 => editing && self.sub_zoom == 0.0 && self.add_zoom == 0.0 && self.pod != 0 && self.can_modify && fits(self.pod),
            0x35..=0x52 => editing && self.opened && !self.closing && fits(((id - 0x33) / 2) as usize),
            0x54 => d.is_some_and(|d| pod_count(rules, i16_at(d, field::RACE), i32_at(d, field::HULL)) > 0),
            0x56 => true,
            _ => false,
        }
    }

    /// This tick's `Design_Main` (exe 0x4b2190, after the frame's `Design3DUpdate` and draw): the held hot-spot's
    /// frame (none on the release's), then the input since the last tick; another design chosen sets
    /// `NemkellTexturaCsere`.
    fn design_main(&mut self, c: &Ctx, mask: &[u8]) {
        let old = self.current();
        if !self.main.iter().any(|m| matches!(m, MainInput::Release)) {
            self.held_pod(c);
            self.edit_held(c);
        }
        for m in std::mem::take(&mut self.main) {
            match m {
                MainInput::Press(at) => self.main_press(c, mask, at),
                MainInput::PressRight(at) if !matches!(self.spot(c, mask, at), 9 | 0x54) => self.main_press(c, mask, at),
                MainInput::PressRight(_) => {}
                MainInput::Move(at) => self.main_moved(at),
                MainInput::Release => self.main_release(c),
            }
        }
        if self.current() != old {
            self.retexture = true;
        }
        let due = self.frames + ORDER_FRAMES - 1;
        self.in_flight.extend(self.orders.drain(..).map(|o| (due, o)));
        let frames = self.frames;
        let (now, later) = std::mem::take(&mut self.in_flight).into_iter().partition(|o| o.0 <= frames);
        self.in_flight = later;
        self.orders = now.into_iter().map(|o: (u64, _)| o.1).collect();
    }

    /// The orders not handed out yet (the screen goes).
    pub fn flush_orders(&mut self) -> Vec<ig2_sim::game::DesignOrder> {
        let mut out = std::mem::take(&mut self.orders);
        out.extend(self.in_flight.drain(..).map(|o| o.1));
        out
    }

    /// The left button went down at `at` (`Mouse_Lenyom`; its hot-spot acts in the tick's `Design_Main`).
    pub fn press(&mut self, _c: &Ctx, _mask: &[u8], at: [f32; 2]) {
        self.mouse = at;
        (self.buttons.0, self.buttons.1) = (true, true);
        self.pod_edges(true);
        self.main.push(MainInput::Press(at));
    }

    /// `Design_Main` on `Mouse_Lenyom`: the hot-spot under `at` acts.
    fn main_press(&mut self, c: &Ctx, mask: &[u8], at: [f32; 2]) {
        let id = self.spot(c, mask, at);
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
                self.sounds.push(CLICK);
                self.held = Some(0x54);
                self.selected = -1;
                if self.pick_pod(c.state, c.rules, at) {
                    self.pod_row_press(c);
                }
            }
            0x56 => self.name_press(c),
            _ => {
                self.edit_press(c, id, at);
            }
        }
    }

    /// Spot 0x54: the pod row under the pointer (`(podkezdx, +0x73) × (podkezdy + 15k, +15]`) becomes
    /// `DesignAktPod`.
    fn pick_pod(&mut self, state: &SaveState, rules: &ig2_rules::Rules, at: [f32; 2]) -> bool {
        let Some(d) = self.edited(state) else { return false };
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
        self.pods.right = true;
        if self.opened && !self.closing && self.over_hull(at) {
            self.drag = Some(at);
        }
        self.main.push(MainInput::PressRight(at));
    }

    /// The pointer moved (held: the list, the scroll bar and the popups follow it).
    pub fn moved(&mut self, at: [f32; 2]) {
        self.mouse = at;
        self.tip.moved(at);
        self.main.push(MainInput::Move(at));
    }

    /// `Design_Main` with the pointer at `at`: the held list, scroll bar or popup follows it.
    fn main_moved(&mut self, at: [f32; 2]) {
        match self.held {
            Some(9) => self.pick_row(at),
            Some(10) => self.scroll_bar(at),
            _ => {}
        }
        if self.popup.is_some() {
            self.hover_popup(at);
        }
    }

    /// Held on the pod panel without `CanModify`, the pointer picks the row it is on (`Design_Main` each frame).
    pub fn held_pod(&mut self, c: &Ctx) {
        if self.held == Some(0x54) && !self.can_modify {
            let at = self.mouse;
            self.pick_pod(c.state, c.rules, at);
        }
    }

    /// The left button went up (`Mouse_Felenged`).
    pub fn release(&mut self, _c: &Ctx) {
        (self.buttons.0, self.buttons.2) = (false, true);
        self.pod_edges(false);
        self.drag = None;
        self.main.push(MainInput::Release);
    }

    /// `Design_Main` on `Mouse_Felenged`: the held popup acts and the hold ends.
    fn main_release(&mut self, c: &Ctx) {
        self.held = None;
        self.edit_release(c);
    }

    /// The right button went up.
    pub fn release_right(&mut self) {
        self.pods.right = false;
        self.drag = None;
        // `Mouse_Felenged` is either button's: the held popup acts and the hold ends.
        self.main.push(MainInput::Release);
    }

    /// `AlapfolottLenyomva` (ELF 0x503d20 / exe 0x4acbc0): a hull is loaded and slid in, not on the first frame
    /// back from another screen, and the pointer is within 160 pixels of its projected centre (`GetObjCenter` +
    /// `TransformPoint`, ·639/639; f32 sum, double root). The re-measured root's box is centred on its pivot, so the
    /// centre is the hull's place (`UnHideAlap`: `SubZoomDes` moves it back while the tech models are out).
    fn over_hull(&self, at: [f32; 2]) -> bool {
        if self.loaded == -1 || self.add_zoom != 0.0 || self.back {
            return false;
        }
        let Some(c) = self.screen(self.hull_at()) else { return false };
        let d = (((c[0] - at[0]) * (c[0] - at[0]) + (c[1] - at[1]) * (c[1] - at[1])) as f64).sqrt() as f32;
        d < 160.0
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
    pub fn tooltip(&mut self, c: &Ctx, mask: &[u8], tips: &[Vec<u8>], now: f64) {
        let mut under = self.spot(c, mask, self.tip.mouse);
        let mut len = tips.get(under as usize).map_or(0, |t| t.len());
        // The tech models' spot 0x0c (`KijelolTalalmany`) where no other hot-spot is.
        let m = self.tip.mouse;
        if let (0, Some((t, o))) = (under, self.pods.tip) {
            if o[0] <= m[0] && m[0] < o[0] + 30.0 && o[1] <= m[1] && m[1] < o[1] + 30.0 {
                self.tip_text = c.texts.map_or_else(Vec::new, |x| crate::hud::tech_name(x, t));
                (under, len) = (0x0c, self.tip_text.len());
            }
        }
        // Any left press holds the tooltip back (the 3D pod pick too, which holds no hot-spot) [obs: design-pods.json t400].
        self.tip.update(now, under, len, self.held.is_some() || self.buttons.0);
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
    /// (`GFX_Design`): fill (0, 54, 90)·`s` at `a·155 + 100`, edge (0, 90, 130)·`s` at `a·225 + 30`, `a = 1 − f/60`
    /// (0 closing), `s` the fader's shine. Under them (drawn at once, the faders queued): each panel's scan band at
    /// `fmod(clock·k, 1)` and its hover glow in (0, s·54, s·90) (re/screens/hud.md §1.2.1).
    pub fn draw_faders(&mut self, f: &mut Frame, d: &Draw2d) {
        use crate::kit::fader_design;
        let a = if self.closing { 0.0 } else { 1.0 - self.frame / 60.0 };
        let (ft, et) = (a * 155.0 + 100.0, a * 225.0 + 30.0);
        let boxes = self.faders();
        let blink = fader_design::blink(d.time as f64 * 1000.0);
        let clock = self.faders.clock();
        let bands = [
            (clock as f64).rem_euclid(1.0),
            ((clock / 2.0) as f64).rem_euclid(1.0),
            (clock as f64 / 1.5).rem_euclid(1.0),
            (clock as f64 * 1.8).rem_euclid(1.0),
            (clock as f64 * 1.2).rem_euclid(1.0),
        ];
        let c = |s: f32| [0, (s * 54.0) as i32, (s * 90.0) as i32];
        for (i, r) in boxes.iter().enumerate() {
            let r = r.map(|q| q as i32);
            fader_design::glow(f, d.white, &d.v, r, c(self.faders.s(i)), bands[i] as f32, 0, blink);
        }
        for (i, r) in boxes.iter().enumerate() {
            if let Some((t, style)) = self.faders.glow(i) {
                fader_design::glow(f, d.white, &d.v, r.map(|q| q as i32), c(self.faders.s(i)), t, style, blink);
            }
        }
        if self.frame > 30.0 || !self.closing {
            let (down, pressed, released) = self.buttons;
            let free = (!down && !released) || pressed;
            for (i, r) in boxes.into_iter().enumerate() {
                let s = self.faders.s(i);
                let (fill, edge) = ([0.0, 54.0 * s, 90.0 * s], [0.0, 90.0 * s, 130.0 * s]);
                crate::kit::fader::kirak_fader(f, d.white, &d.v, r, fill, ft, edge, et, self.faders.offset(i));
                self.faders.enter(i, r, self.mouse, free);
            }
        }
        (self.buttons.1, self.buttons.2) = (false, false);
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
        // The lasers while opening, the sparks until frame 60.
        if let Some(glow) = self.laser_glow {
            self.lasers.draw(f, d.white, glow, &d.v, !self.opened, self.frame != 60.0);
        }
        // While editing the list is dimmed (`KiirStrSzinSzorzo` 50).
        let list_pct = |n: usize| if self.mode != 0 { 50 } else { pct(n) };
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
            text(f, name(r), KORD1[0], KORD1[1] + row as f32 * 15.0 - 15.0, 16, c, list_pct(i - self.first - 1));
        }
        if self.mode == 0 && self.opened && self.pointer > self.first && self.pointer <= n.min(self.first + ROWS) {
            self.list_frame.draw(f, d.white, &d.v, col(0x22, 100));
        }
        let Some(r) = self.edited(state) else { return };
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
        // The tank capacity of the stored `Tervek[EN][AktTerv]`, also while editing.
        if let Some(cap) = self.record(state).map(|d| d[field::TANKS]).filter(|&c| c != 0) {
            text(f, &label(9), 327.0, 57.0, 19, 1, pct(6));
            int(f, cap as i64, 445.0, 57.0, 4, true, 2, pct(7));
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
        // `DesignKeret2` (one draw per call this frame) and `CheckIfTalalmanyfolottvan`'s `Keret(…, 3)` outlines
        // (`FillToScreen` ×4, no corners).
        for &c in &self.pods.frames2 {
            self.pod_frame.draw(f, d.white, &d.v, col(c as usize, 100));
        }
        for &[x, y, w, h] in &self.pods.outlines {
            let c = col(3, 100);
            for (rx, ry, rw, rh) in [(x + 1, y, w - 2, 1), (x + 1, y + h - 1, w - 2, 1), (x, y + 1, 1, h - 2), (x + w - 1, y + 1, 1, h - 2)] {
                let (cx, cy) = (rx as f32 + rw as f32 / 2.0, ry as f32 + rh as f32 / 2.0);
                f.push(d.white, crate::render::Blend::GammaAlpha, crate::render::Instance::new(crate::render::kind::SPRITE, d.v.px(cx, cy), [rw as f32 * d.v.k / 2.0, rh as f32 * d.v.k / 2.0], [c[0], c[1], c[2], 1.0]));
            }
        }
        // The tech models are out (`SubZoomDes != 0`): their rows by index, then the "clear" row after the last shown.
        if self.sub_zoom != 0.0 {
            let mut last = -2;
            for (i, e) in self.pods.techs.iter().enumerate().filter(|(_, e)| e.shown) {
                let y = POD[1] + (i as f32 + 1.0) * 15.0;
                let n = tech_name(e.tech as i32);
                text(f, &n, POD[0], y + 5.0, n.len(), 2, 100);
                last = i as i32;
            }
            text(f, &d.texts.get(5, 2, 0xf), POD[0], POD[1] + 45.0 + last as f32 * 15.0 + 5.0, 14, 2, 100);
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
                // The minus and plus cells (`BrushToScreenXY`): live while editing a pod taking more than one,
                // else greyed.
                let brushes = &rules.design_screen.brushes;
                let live = usize::from(!(self.mode != 0 && max > 1)) * 2;
                if z < 0.8 {
                    if let Some(b) = brushes.get(live) {
                        pics.brush(f, &d.v, b, 1, Some([POD[0] + 96.0 - 4.0 - 35.0 * z, y + 4.0]), [1.0; 4]);
                    }
                }
                if z < 0.2 {
                    if let Some(b) = brushes.get(live + 1) {
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
pub(crate) fn max_fitted(rules: &ig2_rules::Rules, techs: &TechTable, race: i16, hull: i32, k: usize, tech: i32) -> i32 {
    rules.design_screen.pod_max(race.max(0) as usize, hull.max(0) as usize, k).min(techs.byte(tech.clamp(0, 192) as usize, 0x24) as i32)
}

/// `PodTipusok[KeresPod(class)]`'s name (`NEWGAME_Design`): an empty slot's class.
pub(crate) fn pod_type_name(texts: &crate::ui::UiTexts, rules: &ig2_rules::Rules, race: i16, hull: i32, k: usize) -> Vec<u8> {
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
    fn a_full_name_takes_a_key_at_the_cursor_and_drops_its_last() {
        use crate::design_edit::{EntryKey, NameEntry};
        let entry = |text: &[u8], cursor, clear| Some(NameEntry { text: text.to_vec(), cursor, clear, stored: Some(4) });
        let mut s = Design { entry: entry(b"ABCDEFGHIJKLMNOP", 3, false), ..blank() };
        assert_eq!(s.entry_key(EntryKey::Char(b'x')), Some((4, b"ABCxDEFGHIJKLMNO".to_vec())));
        // The cursor at `IOMaxLen`: nothing goes in.
        s.entry = entry(b"ABCDEFGHIJKLMNOP", 16, false);
        assert_eq!(s.entry_key(EntryKey::Char(b'x')), Some((4, b"ABCDEFGHIJKLMNOP".to_vec())));
        // The first key replaces the text, the cursor keys only drop the flag.
        s.entry = entry(b"ABCDEFGHIJKLMNOP", 16, true);
        assert_eq!(s.entry_key(EntryKey::Char(b'x')), Some((4, b"x".to_vec())));
        s.entry = entry(b"Hawk", 4, true);
        s.entry_key(EntryKey::Home);
        assert_eq!(s.entry_key(EntryKey::Char(b'x')), Some((4, b"xHawk".to_vec())));
    }

    #[test]
    fn esc_folds_the_open_tech_models() {
        let mut s = blank();
        s.pods.sub_dir = 1;
        s.pods.picked = -1;
        s.pods_escape();
        assert_eq!(s.pods.sub_dir, -1);
        // A picked model flies into its pod once they are all out.
        s.pods.sub_dir = 1;
        s.pods.picked = 0;
        s.sub_zoom = 1000.0;
        s.pods_escape();
        assert_eq!((s.pods.sub_dir, s.pods.sub2), (1, 1000.0));
    }

    #[test]
    fn the_hull_drag_starts_near_the_hull_and_not_on_the_frame_back() {
        let mut s = Design { loaded: 5, add_zoom: 0.0, opened: true, ..blank() };
        s.press_right([320.0, 380.0]);
        assert!(s.drag.is_some());
        s.drag = None;
        s.press_right([320.0, 400.0]);
        assert!(s.drag.is_none());
        // The tech models out push the hull back (z −2400): its centre moves up.
        s.sub_zoom = 1000.0;
        s.press_right([320.0, 236.25 + 159.0]);
        assert!(s.drag.is_some());
        s.drag = None;
        s.back = true;
        s.press_right([320.0, 240.0]);
        assert!(s.drag.is_none());
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
            back: false,
            retexture: false,
            main: Vec::new(),
            in_flight: Vec::new(),
            pod: 0,
            pod_of: 0,
            mouse: [0.0; 2],
            held: None,
            faders: crate::kit::FaderDesign::new(1),
            buttons: (false, false, false),
            list_frame: DesignFrame::default(),
            tip: Tooltip::default(),
            tip_text: Vec::new(),
            frames: 0,
            sounds: Vec::new(),
            tutorial: Vec::new(),
            mode: 0,
            scratch: [0; 0x84],
            can_modify: false,
            popup: None,
            selected: -1,
            repeat: 0,
            entry: None,
            orders: Vec::new(),
            names: Names::default(),
            button: -1,
            sub_zoom: 0.0,
            pods: Pods::new(),
            pod_frame: DesignFrame::second(),
            lasers: crate::kit::Lasers::default(),
            laser_glow: None,
            tutorial_paused: false,
            events: Vec::new(),
        }
    }
}
