//! The starmap's production view and transfer panel (re/screens/starmap.md §12–§13): the original's state machine
//! (`StarmapProductMode`/`StarmapPModeIn`/`StarmapPDir`/`StarmapPFrame`, `StarmapTransferMode` and its fleets), the
//! hot-spots of `Starmap_MouseInit` (exe 0x5b99d0) and the orders `Starmap_Main` (ELF 0x6f50a0, exe 0x5b2b70) sends
//! from them (0x1d production edits, 0x1c transfers, 0x15 stops, 0x14 splits), and the drawing: the faders of
//! `StarmapGfx2D_2` (ELF 0x6de1b0) and `ShowStarmap` (ELF 0x6eca50), the texts of `StarmapGfx2D_3` (exe 0x5aaba0)
//! and `StarmapGfx2D_4`, and the camera-fixed models (`gyartas.xvb`, `ProductGFX` ELF 0x6e8870, `TransferObjs`)
//! rendered into a texture over the virtual 640 × 480 screen.

use std::collections::HashMap;

use ig2_formats::Install;
use ig2_sim::game::StarmapOrder;
use ig2_sim::save::{va, SaveState, PLANET_SIZE};

use crate::font::{Align, Font, Virtual};
use crate::kit::fader::{fader, kirak_fader};
use crate::kit::FaderDesign;
use crate::lighting;
use crate::orders::Selection;
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement, M3, V3};

/// `Starmap_MouseInit`'s hot-spots the panels use (`Mouse_Szoveg_Set(id, …, x, y, w, h)`, virtual pixels).
const SPOTS: &[(u32, [i32; 4])] = &[
    (8, [0, 424, 166, 56]),
    (9, [445, 380, 164, 60]),
    (10, [174, 432, 44, 44]),
    (11, [422, 432, 44, 44]),
    (0xc, [254, 37, 22, 37]),
    (0xd, [363, 37, 22, 37]),
    (0xe, [153, 164, 21, 24]),
    (0xf, [153, 238, 21, 21]),
    (0x10, [153, 293, 21, 21]),
    (0x11, [153, 349, 21, 21]),
    (0x12, [468, 164, 21, 24]),
    (0x13, [468, 238, 21, 21]),
    (0x14, [468, 293, 21, 21]),
    (0x15, [468, 349, 21, 21]),
    (0x16, [469, 134, 19, 13]),
    (0x17, [469, 208, 19, 14]),
    (0x18, [469, 263, 19, 14]),
    (0x19, [469, 318, 19, 14]),
    (0x1a, [135, 132, 28, 51]),
    (0x1b, [135, 205, 28, 51]),
    (0x1c, [135, 259, 28, 51]),
    (0x1d, [135, 314, 28, 51]),
    (0x1e, [450, 132, 28, 51]),
    (0x1f, [450, 205, 28, 51]),
    (0x20, [450, 259, 28, 51]),
    (0x21, [450, 314, 28, 51]),
    (0x22, [163, 132, 28, 51]),
    (0x23, [163, 205, 28, 51]),
    (0x24, [163, 259, 28, 51]),
    (0x25, [163, 314, 28, 51]),
    (0x26, [478, 132, 28, 51]),
    (0x27, [478, 205, 28, 51]),
    (0x28, [478, 259, 28, 51]),
    (0x29, [478, 314, 28, 51]),
    (0x2a, [11, 125, 120, 60]),
    (0x2b, [11, 206, 120, 50]),
    (0x2c, [11, 261, 120, 50]),
    (0x2d, [11, 316, 120, 50]),
    (0x2e, [511, 125, 120, 60]),
    (0x2f, [511, 206, 120, 50]),
    (0x30, [511, 261, 120, 50]),
    (0x31, [511, 316, 120, 50]),
    (0x32, [276, 32, 87, 47]),
    (0x33, [0, 80, 195, 292]),
    (0x34, [445, 80, 195, 292]),
    (0x43, [28, 60, 124, 70]),
    (0x44, [338, 60, 139, 70]),
    (0x45, [152, 60, 139, 70]),
    (0x46, [487, 60, 121, 70]),
    (0x47, [28, 130, 124, 70]),
    (0x48, [338, 130, 139, 70]),
    (0x49, [152, 130, 139, 70]),
    (0x4a, [487, 130, 121, 70]),
    (0x4b, [28, 200, 124, 70]),
    (0x4c, [338, 200, 139, 70]),
    (0x4d, [152, 200, 139, 70]),
    (0x4e, [487, 200, 121, 70]),
    (0x4f, [28, 270, 124, 70]),
    (0x50, [338, 270, 139, 70]),
    (0x51, [152, 270, 139, 70]),
    (0x52, [487, 270, 121, 70]),
    (0x57, [307, 30, 26, 329]),
    // `Starmap_MouseInit2` moves 0x58 over both transfer panels (`Mouse_Csak_Pos_Set(0x58, 24, 24, 592, 339)`).
    (0x58, [24, 24, 592, 339]),
];

/// The queue slots' rows (`StarmapGfx2D_3`: `local_1d8`).
const SLOT_Y: [f32; 4] = [141.0, 214.0, 271.0, 326.0];

/// `GetProductionPanelSubMeshIDs` (ELF 0x6d4010): the `gyartas.xvb` group of slot `k + 4L`'s minus and plus
/// buttons, "into a fleet" lamp and repeat lamp (`prsmesh[r][0..4]`).
const BUTTON_GROUPS: [[u32; 4]; 8] =
    [[9, 8, 11, 10], [17, 16, 19, 18], [25, 24, 27, 26], [33, 32, 35, 34], [13, 12, 15, 14], [21, 20, 23, 22], [29, 28, 31, 30], [37, 36, 39, 38]];

/// One drawn frame of the original's panels animation (`ShowStarmap` steps `StarmapPFrame` once per drawn frame;
/// the GOG build draws at the display's 60 Hz [inf]).
pub const PANEL_FRAME: f32 = 1.0 / 60.0;

/// What the screen does in reply to an input: an order for the lockstep queue, or a change of the local view.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Send(StarmapOrder),
    Select(Selection),
    /// `StarmapBeZoomol(x, y, zoom)`: remember the view, fly to the galaxy point at `zoom`.
    ZoomIn { x: f32, y: f32, zoom: f32 },
    /// `StarmapKiZoomol`: back to the remembered view.
    ZoomOut,
    /// `StarmapKozepreAllit(x, y)`: centre the galaxy point.
    Centre { x: f32, y: f32 },
    /// A sound (`PlaySoundGlobal`, re/screens/audio.md §6.1).
    Sound(crate::audio::SoundId),
    /// A sale menu's order (`MULTIGET_Design` 0x34 / 0x35).
    Design(crate::starmap_info::DesignOrder),
    /// A story hero's design sent back from the sale menu (`Story_Special(0x19, fleet, design)`, single player).
    HeroBack { fleet: i32, design: i32 },
    /// The radial menu's story icon `icon` on fleet or planet `target` (`Starmap_Main`: 0x10 and 0x11 in single
    /// player only, after a click when `click`; 0x12 always, silent).
    StoryIcon { icon: i32, target: i32, click: bool },
    /// `TutorialCheckEvent(id)` (re/rules/tutorial.md §4).
    Tutorial(i32),
}

/// The tutorial's starmap gates: `TutorialProductionDisabled` (the production button, hot-spot 11, and the radial
/// icon 2), `TutorialExitGombDisabled` (the production view's exit, hot-spot 9: `Starmap_Main` ELF line 1545) and
/// `TutorialSplitDisabled` (the radial split icon 0xd).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TutorialLocks {
    pub production: bool,
    pub exit: bool,
    pub split: bool,
}

/// `PlaySoundGlobal(0)`.
const CLICK: Action = Action::Sound(crate::audio::CLICK);

/// The transfer panel (`StarmapTransferMode`): `StarmapTransferFlotta1` on the left, `…Flotta2` (a fleet, or −p
/// with `StarmapTransferTankMode`) on the right, the first row shown (`StarmapTransferKezdet`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub f1: i32,
    pub f2: i32,
    pub tank_mode: bool,
    pub first: i32,
}

/// A held button: the hot-spot and `Starmap_Main::KezdoIdo` (its 10 ms frames so far), plus the mouse button.
#[derive(Debug, Clone, Copy)]
struct Hold {
    spot: u32,
    frames: i32,
    left: bool,
}

/// The design list of a queue slot (`StarmapProductSelectMode`, `smSelect*`).
#[derive(Debug, Clone, PartialEq)]
pub struct DesignList {
    pub line: usize,
    /// `StarmapProductSelectHely` and the slot's UI byte (`re`).
    pub slot: usize,
    ui: u8,
    pub x: i32,
    pub y: i32,
    pub per_col: i32,
    pub cols: i32,
    /// The listed designs (0 = remove), `smSelected` (−1 none).
    pub designs: Vec<usize>,
    pub hovered: i32,
}

/// The panels' state (the original's screen globals).
#[derive(Debug, Clone, Default)]
pub struct Panels {
    /// `StarmapPModeIn`, `StarmapPDir` (opening), `StarmapPFrame`, `StarmapProductMode`.
    pub p_moving: bool,
    pub p_opening: bool,
    pub p_frame: i32,
    pub product: bool,
    pub transfer: Option<Transfer>,
    /// The fleet's or the vehicle info panel (re/screens/starmap.md §19–§20).
    pub info: Option<crate::starmap_info::InfoPanel>,
    /// The info panels' clicks to play (`PlaySoundGlobal(0)`; the zooms' sound 5 comes with `Action::ZoomIn/ZoomOut`).
    pub sounds: Vec<crate::audio::SoundId>,
    pub list: Option<DesignList>,
    hold: Option<Hold>,
    /// The tutorial's gates on the panels (re/rules/tutorial.md §5), set by the window every frame.
    pub locks: TutorialLocks,
    /// `Tutorial_Split_Ok`: a transfer button (0x44..=0x52) was pressed while the tutorial awaited 5 (`Starmap_Main`);
    /// the panel's closing then raises it (`TransferOff` exe 0x5bbcda). Never cleared, as in the exe.
    split_ok: bool,
    /// `StarmapGfx2D_2::addo`: the exit button's width change (+11 production, −10 transfer).
    addo: i32,
    /// Drawn frames since the transfer panel's models were made (their spin) and the head units' spin.
    pub spin_frames: f32,
    pub head_frames: f32,
    /// The last drawn-frame time the animations advanced to.
    clock: Option<f32>,
    /// `starf`, `fifo`: the faders' shine and drift.
    pub faders: StarmapFaders,
}

#[cfg(test)]
fn spot_rect(id: u32) -> Option<[i32; 4]> {
    SPOTS.iter().find(|s| s.0 == id).map(|s| s.1)
}

use crate::kit::inside;

/// The planet record `p`.
fn planet(state: &SaveState, p: usize) -> Vec<u8> {
    state.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE)
}

fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

fn en(state: &SaveState) -> u8 {
    state.mem.i32(va::EN).clamp(0, 18) as u8
}

/// `Tervek[e][d]`.
fn design(state: &SaveState, e: u8, d: usize) -> Option<&[u8]> {
    state.designs.get(e as usize)?.get(d).map(|r| &r[..])
}

fn fleet(state: &SaveState, f: i32) -> Option<&[u8]> {
    usize::try_from(f).ok().and_then(|f| state.fleets.get(f)).map(|r| &r[..])
}

/// `StarmapKeyboardHandler`'s PgUp / PgDn (VK 0x21 / 0x22, re/screens/starmap.md §10.6): in planet mode the previous /
/// next planet the human owns and knows (any known one with Shift; `DEBUGX` lifts "known"), when it owns one or Shift
/// is held; in fleet mode the previous / next own fleet, when it has one. The new selection and the point to centre
/// (`StarmapKozepreAllit`, galaxy coordinates).
pub fn step_selection(state: &SaveState, sel: &Selection, next: bool, shift: bool) -> Option<(Selection, [f32; 2])> {
    let e = en(state);
    match sel.mode {
        1 => {
            let n = state.planet_count();
            let own = (1..=n).filter(|&p| planet(state, p)[0x20] == e).count();
            if n == 0 || (own == 0 && !shift) {
                return None;
            }
            let debug = state.mem.u8(va::DEBUGX) != 0;
            let mut p = sel.planet.clamp(1, n);
            for _ in 0..n {
                p = if next { p % n + 1 } else { (p + n - 2) % n + 1 };
                let r = planet(state, p);
                if (shift || r[0x20] == e) && (i16_at(&r, 0xd8 + 2 * e as usize) >= 1 || debug) {
                    return Some((Selection { mode: 1, planet: p, ..*sel }, [f32_at(&r, 0x18), f32_at(&r, 0x1c)]));
                }
            }
            None
        }
        2 => {
            let n = state.fleets.len().saturating_sub(1);
            if !state.fleets.iter().skip(1).any(|f| f[0x15] == e) {
                return None;
            }
            let (mut f, m) = (sel.fleet as i64, n as i64);
            for _ in 0..n {
                f = if next { (f + m) % m + 1 } else { (f + m - 2).rem_euclid(m) + 1 };
                let r = &state.fleets[f as usize];
                if r[0x15] == e {
                    return Some((Selection { mode: 2, fleet: f as usize, ..*sel }, [f32_at(r, 0x30), f32_at(r, 0x34)]));
                }
            }
            None
        }
        _ => None,
    }
}

/// `SearchBolygohozKozeliFlotta(p, 20, true, false, false, true, 0)` (exe 0x540f50, `production-research.md` §4): the
/// owner's fleet nearest to planet `p` (Manhattan < 20) that isn't colonising, terraforming or in battle, an idle one
/// preferred; 0 if none.
pub fn near_fleet(state: &SaveState, p: usize) -> i32 {
    let r = planet(state, p);
    let (px, py, owner) = (f32_at(&r, 0x18), f32_at(&r, 0x1c), r[0x20]);
    let (mut best, mut lim) = (0usize, 20.0f32);
    for (g, f) in state.fleets.iter().enumerate().skip(1) {
        let flags = i16_at(f, 0x52) as u16;
        let ok = f[0x15] != 0xff && f[0x15] != 0 && f[0x15] == owner && flags & 0x18 == 0 && flags & 1 == 0;
        if ok && (best == 0 || i16_at(f, 0x40) == 0) {
            let d = (px - f32_at(f, 0x30)).abs() + (py - f32_at(f, 0x34)).abs();
            if d < lim {
                (best, lim) = (g, d);
            }
        }
    }
    best as i32
}

/// `SearchFlottahozKozeliBolygo(f, 20, true, true)` (`ai-empire.md` §12.4): the fleet owner's planet nearest to it
/// (Manhattan < 20), none if that one is protected (`+0x64`).
pub fn near_planet(state: &SaveState, f: i32) -> i32 {
    let Some(r) = fleet(state, f) else { return 0 };
    let (fx, fy, owner) = (f32_at(r, 0x30), f32_at(r, 0x34), r[0x15]);
    if owner == 0 {
        return 0;
    }
    let (mut best, mut pick) = (20.0f32, 0usize);
    for p in 1..=state.planet_count() {
        let q = planet(state, p);
        if q[0x20] != owner {
            continue;
        }
        let d = (fy - f32_at(&q, 0x1c)).abs() + (fx - f32_at(&q, 0x18)).abs();
        if d < best {
            (best, pick) = (d, p);
        }
    }
    if pick > 0 && planet(state, pick)[0x64] != 0 {
        pick = 0;
    }
    pick as i32
}

/// `StarmapAktFleethezKozeliSajatFlotta` (`Starmap_MouseInit2`): the player's fleet nearest to `f` that isn't in
/// battle, `(int)` Manhattan distance below 20; 0 if none.
pub fn nearest_own_fleet(state: &SaveState, f: i32) -> i32 {
    let Some(me) = fleet(state, f) else { return 0 };
    let e = en(state);
    let (mut best, mut pick) = (20i32, 0usize);
    for (g, r) in state.fleets.iter().enumerate().skip(1) {
        if r[0x15] == e && i16_at(r, 0x52) & 1 == 0 && g as i32 != f {
            let d = ((f32_at(me, 0x30) - f32_at(r, 0x30)).abs() + (f32_at(me, 0x34) - f32_at(r, 0x34)).abs()) as i32;
            if d < best {
                (best, pick) = (d, g);
            }
        }
    }
    pick as i32
}

/// A queue slot: design, name, shown count (`+0x88 + 4L + k`, slot 0 less `+0xa8 + L`, ×5 for fighters), total price.
pub type Slot = (usize, Vec<u8>, i32, i32);

/// One line of a queue as `StarmapGfx2D_3` shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueueLine {
    /// `+0x70 + 4L`: the planet's capacity for the line (shipyards, factories); < 1 shows "none".
    pub capacity: i32,
    pub slots: [Option<Slot>; 4],
    /// The head unit's bar: `76 − left·76/time` of 76.
    pub progress: Option<i32>,
    pub repeat: [bool; 4],
    pub into_fleet: [bool; 4],
}

/// The queue of `planet` (`+0x78..+0xb8`; re/screens/starmap.md §12.3).
pub fn queue(state: &SaveState, p: usize) -> [QueueLine; 2] {
    let r = planet(state, p);
    let owner = r[0x20];
    let e = en(state);
    std::array::from_fn(|l| {
        let mut q = QueueLine { capacity: i32_at(&r, 0x70 + 4 * l), ..Default::default() };
        for k in 0..4 {
            q.repeat[k] = r[0x90 + 4 * l + k] != 0;
            q.into_fleet[k] = r[0xad + 4 * l + k] != 0;
            let d = i16_at(&r, 0x78 + 8 * l + 2 * k);
            if d <= 0 {
                continue;
            }
            let Some(rec) = design(state, owner, d as usize) else { continue };
            // The fighters' ×5 reads the player's own designs (`Tervek[EN]`).
            let mult = if design(state, e, d as usize).is_some_and(|x| x[0x36] != 0) { 5 } else { 1 };
            let mut c = r[0x88 + 4 * l + k] as i32;
            if k == 0 {
                c -= r[0xa8 + l] as i32;
            }
            q.slots[k] = Some((d as usize, cstr(&rec[..0x14]).to_vec(), c * mult, c.wrapping_mul(i32_at(rec, 0x24))));
        }
        let head = i16_at(&r, 0x78 + 8 * l);
        if head > 0 {
            if let Some(rec) = design(state, owner, head as usize) {
                let time = i32_at(rec, 0x20);
                if time != 0 {
                    q.progress = Some(76 - i32_at(&r, 0xa0 + 4 * l).wrapping_mul(76) / time);
                }
            }
        }
        q
    })
}

/// One row of the transfer panel: a design and how many of it each side holds (ships, or the vehicles' counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub design: usize,
    /// 1 ships (not with `StarmapTransferTankMode`), 2 fighters (neither), 3 vehicles.
    pub group: u8,
    pub left: i32,
    pub right: i32,
}

/// `StarmapTransferDarab` and its twin (ELF 0x171ad90/0x171ad98): what each side holds of each of the player's
/// designs (`Hajok` entries by holder, `Tankok` counts by holder), then the rows in the original's order: groups 1,
/// 2, 3, designs ascending, only designs either side holds (re/screens/starmap.md §13.2).
pub fn rows(state: &SaveState, t: &Transfer) -> Vec<Row> {
    let e = en(state);
    let n = state.designs.get(e as usize).map_or(0, |d| d.len().saturating_sub(1));
    let mut left = vec![0i32; n + 1];
    let mut right = vec![0i32; n + 1];
    let mut add = |holder: i32, d: i16, c: i32| {
        if d == 0x7fff || d < 0 || d as usize > n {
            return;
        }
        if holder == t.f1 {
            left[d as usize] += c;
        }
        if holder == t.f2 {
            right[d as usize] += c;
        }
    };
    for s in state.ships.iter().skip(1) {
        add(i16_at(s, 0) as i32, i16_at(s, 2), 1);
    }
    let nv = state.mem.i32(va::VEHICLE_COUNT).max(0) as usize;
    let tanks = state.mem.read(va::VEHICLES, 8 * (nv + 1));
    for v in tanks.chunks_exact(8).skip(1) {
        add(i16_at(v, 0) as i32, i16_at(v, 2), i32_at(v, 4));
    }
    let mut out = Vec::new();
    for group in 1..=3u8 {
        for d in 1..=n {
            let Some(rec) = design(state, e, d) else { continue };
            let class = i16_at(rec, 0x16);
            let fits = match group {
                1 => class == 1 && rec[0x36] == 0 && !t.tank_mode,
                2 => class == 1 && rec[0x36] != 0 && !t.tank_mode,
                _ => class == 2,
            };
            if fits && (left[d] > 0 || right[d] > 0) {
                out.push(Row { design: d, group, left: left[d], right: right[d] });
            }
        }
    }
    out
}

impl Panels {
    /// A panel already open and at rest (`--panel production` or `--panel transfer:F1,F2[,tank]`).
    pub fn opened(spec: &str) -> Result<Self, String> {
        let mut p = Self::default();
        if spec == "production" {
            (p.product, p.p_frame, p.addo) = (true, 0xc, 0xb);
        } else if let Some(v) = spec.strip_prefix("transfer:") {
            let parts: Vec<&str> = v.split(',').collect();
            let num = |i: usize| parts.get(i).and_then(|x| x.trim().parse::<i32>().ok()).ok_or(format!("--panel transfer:F1,F2[,tank]: {v}"));
            p.transfer = Some(Transfer { f1: num(0)?, f2: num(1)?, tank_mode: parts.get(2) == Some(&"tank"), first: 0 });
            p.addo = -10;
        } else if spec == "fleet" || spec == "tanks" {
            use crate::starmap_info::{InfoPanel, Kind};
            let kind = if spec == "fleet" { Kind::Fleet } else { Kind::Tanks };
            p.info = Some(InfoPanel::new(kind, Default::default(), 590.0));
            p.addo = if kind == Kind::Fleet { -10 } else { 0 };
        } else {
            return Err(format!("--panel {spec}: production, transfer:F1,F2[,tank], fleet or tanks"));
        }
        Ok(p)
    }

    /// The production view is up or moving (`StarmapProductMode || (StarmapPModeIn && StarmapPDir)`).
    pub fn production_open(&self) -> bool {
        self.product || (self.p_moving && self.p_opening)
    }

    /// Something the panels draw is on screen.
    pub fn active(&self) -> bool {
        self.product || self.p_moving || self.transfer.is_some() || self.info.is_some()
    }

    /// `StarmapFleetMode`: the info box clicked with the left button in fleet mode on the player's fleet (not
    /// `fSpec`, flags 0x4400): the panel toggles; a transfer panel up gives way first (re/screens/starmap.md §19.1).
    /// `zoom_from` = the zoom before any panel's fly (`StarmapInfoOffLastZoom`).
    pub fn info_box_fleet(&mut self, state: &SaveState, sel: &Selection, names: crate::starmap_info::SaleNames, zoom_from: f32) -> Vec<Action> {
        use crate::starmap_info::{InfoPanel, Kind};
        let Some(r) = fleet(state, sel.fleet as i32) else { return Vec::new() };
        if i16_at(r, 0x52) as u16 & 0x4400 != 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        self.sounds.push(crate::audio::CLICK);
        if self.info.as_ref().is_some_and(|i| i.kind == Kind::Tanks) {
            self.info = None;
        }
        if self.info.is_some() {
            self.info = None;
            out.push(Action::ZoomOut);
            return out;
        }
        let closed = self.transfer.is_some();
        if let Some(t) = self.transfer {
            self.sounds.push(crate::audio::CLICK);
            out.extend(self.close_transfer(state, sel, t.tank_mode));
        }
        if r[0x15] == en(state) {
            if !closed {
                out.push(Action::ZoomIn { x: f32_at(r, 0x30), y: f32_at(r, 0x34), zoom: 590.0 });
            }
            self.info = Some(InfoPanel::new(Kind::Fleet, names, zoom_from));
        }
        out
    }

    /// `AktRopIkon == 9`: the vehicle info panel of the selected planet (no zoom).
    pub fn open_tanks_info(&mut self, names: crate::starmap_info::SaleNames) {
        self.sounds.push(crate::audio::CLICK);
        self.info = Some(crate::starmap_info::InfoPanel::new(crate::starmap_info::Kind::Tanks, names, 590.0));
    }

    /// The minimap (hot-spot 7) is off: under the production view, the transfer panel and the fleet's zoomed info
    /// panel (`Starmap_MouseInit2` turns 1..=7 off with `StarmapZoomMode`); the vehicle panel leaves it on.
    pub fn minimap_off(&self) -> bool {
        self.product || self.p_moving || self.transfer.is_some() || self.fleet_info()
    }

    /// The fleet's info panel is up.
    pub fn fleet_info(&self) -> bool {
        self.info.as_ref().is_some_and(|i| i.kind == crate::starmap_info::Kind::Fleet)
    }

    /// The info panel closes (hot-spot 9; the fleet's flies back out).
    pub fn close_info(&mut self) -> Vec<Action> {
        match self.info.take() {
            Some(i) if i.kind == crate::starmap_info::Kind::Fleet => {
                self.sounds.push(crate::audio::CLICK);
                vec![Action::ZoomOut]
            }
            _ => Vec::new(),
        }
    }

    /// `AktRopIkon == 2` (the production button, hot-spot 11): open, or close when open.
    pub fn toggle_production(&mut self, state: &SaveState, sel: &Selection) -> Vec<Action> {
        self.info = None;
        let out = self.close_transfer(state, sel, true);
        if !self.production_open() {
            self.p_opening = true;
            self.p_frame = 0;
        } else {
            self.p_opening = false;
            self.p_frame = 0x11;
            self.product = false;
        }
        self.p_moving = true;
        self.list = None;
        out
    }

    /// Close the production view (hot-spot 9, the transfer button, a planet that isn't the player's).
    pub fn close_production(&mut self) {
        if self.production_open() {
            self.p_moving = true;
            self.p_opening = false;
            self.p_frame = 0x11;
            self.product = false;
            self.list = None;
        }
    }

    /// `StarmapKiZoomol` + `TransferOff` (ELF 0x6f2810): the panel closes; an emptied selected fleet hands the
    /// selection to the other side's fleet. The original's single player deletes the player's emptied fleets of the
    /// panel here; the port leaves that to the sim's 0x1b (one frame later, as in multiplayer).
    pub fn close_transfer(&mut self, state: &SaveState, sel: &Selection, zoom_out: bool) -> Vec<Action> {
        let Some(t) = self.transfer.take() else { return Vec::new() };
        let mut out = Vec::new();
        if zoom_out {
            out.push(Action::ZoomOut);
        }
        if !t.tank_mode && self.split_ok {
            out.push(Action::Tutorial(5));
        }
        let empty = |f: i32| fleet(state, f).is_none_or(|r| i32_at(r, 0x54) < 1);
        let mut f = sel.fleet as i32;
        if empty(f) && f == t.f2 && t.f1 > 0 {
            f = t.f1;
        }
        if empty(f) && f == t.f1 && t.f2 > 0 {
            f = t.f2;
        }
        if f != sel.fleet as i32 {
            out.push(Action::Select(Selection { fleet: f as usize, ..*sel }));
        }
        out
    }

    /// Open the transfer between `f1` and `f2` (the split's handler, `AktRopIkon` 1 or 0xe).
    pub fn open_transfer(&mut self, f1: i32, f2: i32, tank_mode: bool) {
        self.close_production();
        self.transfer = Some(Transfer { f1, f2, tank_mode, first: 0 });
        self.spin_frames = 0.0;
        self.hold = None;
    }

    /// `AktRopIkon == 1` (the tanks button, hot-spot 10, `Starmap_Main`): the selected fleet and the player's planet
    /// it orbits, or the selected planet and the player's fleet near it, in vehicle mode (`StarmapTransferTankMode`),
    /// zoomed in to 590 and the fleet stopped (0x15); pressed again it closes; a fleet-to-fleet panel gives way.
    pub fn tanks_button(&mut self, state: &SaveState, sel: &Selection) -> Vec<Action> {
        let mut out = Vec::new();
        self.close_production();
        // The fleet's info panel goes without flying out; the zoom stays (no `StarmapBeZoomol`).
        let zoomed = self.fleet_info();
        self.info = None;
        match self.transfer {
            Some(t) if t.tank_mode => return self.close_transfer(state, sel, true),
            Some(_) => out.extend(self.close_transfer(state, sel, false)),
            None if zoomed => {}
            None => {
                let at = if sel.mode == 2 {
                    fleet(state, sel.fleet as i32).map(|r| (f32_at(r, 0x30), f32_at(r, 0x34)))
                } else {
                    let r = planet(state, sel.planet);
                    Some((f32_at(&r, 0x18), f32_at(&r, 0x1c)))
                };
                if let Some((x, y)) = at {
                    out.push(Action::ZoomIn { x, y, zoom: 590.0 });
                }
            }
        }
        let (f1, p) = if sel.mode == 2 { (sel.fleet as i32, near_planet(state, sel.fleet as i32)) } else { (near_fleet(state, sel.planet), sel.planet as i32) };
        self.open_transfer(f1, -p, true);
        out.push(Action::Send(StarmapOrder::Move { fleet: f1, order: 0, kind: 0, x: 0.0, y: 0.0 }));
        out
    }

    /// `AktRopIkon == 0xe` (the radial menu's transfer icon): the selected fleet (right) and the player's nearest
    /// fleet (`StarmapAktFleethezKozeliSajatFlotta`, left), both stopped, zoomed in to 590.
    pub fn fleet_transfer(&mut self, state: &SaveState, sel: &Selection) -> Vec<Action> {
        let Some(me) = fleet(state, sel.fleet as i32).filter(|r| sel.mode == 2 && r[0x15] == en(state) && i16_at(r, 0x52) & 1 == 0) else {
            return Vec::new();
        };
        let near = nearest_own_fleet(state, sel.fleet as i32);
        if near < 1 {
            return Vec::new();
        }
        let mut out = vec![Action::ZoomIn { x: f32_at(me, 0x30), y: f32_at(me, 0x34), zoom: 590.0 }];
        self.open_transfer(near, sel.fleet as i32, false);
        for f in [sel.fleet as i32, near] {
            out.push(Action::Send(StarmapOrder::Move { fleet: f, order: 0, kind: 0, x: 0.0, y: 0.0 }));
        }
        out
    }

    /// `StarmapTanksGomb` / `StarmapProductGomb` (`Starmap_MouseInit2` ELF 0x7024b0): which of the two buttons the
    /// selection shows.
    pub fn buttons(state: &SaveState, sel: &Selection) -> (bool, bool) {
        let e = en(state);
        if sel.mode == 1 {
            let r = planet(state, sel.planet);
            if r[0x20] != e {
                return (false, false);
            }
            (near_fleet(state, sel.planet) > 0 && r[0x64] & 1 == 0, true)
        } else if sel.mode == 2 {
            let mine = fleet(state, sel.fleet as i32).is_some_and(|r| r[0x15] == e && i16_at(r, 0x52) & 1 == 0);
            let p = near_planet(state, sel.fleet as i32);
            (mine && p > 0 && planet(state, p as usize)[0x20] == e, false)
        } else {
            (false, false)
        }
    }

    /// The hot-spots on at `at` (`Starmap_MouseInit2`'s switches for the panels' modes); the lowest id wins [inf].
    pub fn spot(&self, state: &SaveState, sel: &Selection, at: [f32; 2]) -> Option<u32> {
        if let Some(i) = &self.info {
            if let Some(id) = i.spot(state, sel, at) {
                return Some(id);
            }
            return SPOTS.iter().filter(|s| matches!(s.0, 8 | 10 | 11) && inside(s.1, at)).map(|s| s.0).min();
        }
        let on = |id: u32| -> bool {
            if let Some(t) = &self.transfer {
                let shown = rows(state, t).len().min(8) as u32;
                return matches!(id, 8..=11) || (0x43..0x43 + 2 * shown).contains(&id) || (0x57..=0x58).contains(&id);
            }
            if self.product {
                if id == 8 || id == 9 || id == 10 || id == 11 {
                    return true;
                }
                if !(0xc..=0x36).contains(&id) {
                    return false;
                }
                // Empty slots have no buttons (`Starmap_MouseInit2`).
                let r = planet(state, sel.planet);
                let empty = |l: usize, k: usize| i16_at(&r, 0x78 + 8 * l + 2 * k) == 0;
                return match id {
                    0xe..=0x11 => !empty(0, (id - 0xe) as usize),
                    0x12..=0x15 => !empty(1, (id - 0x12) as usize),
                    0x16..=0x19 => !empty(1, (id - 0x16) as usize),
                    0x1a..=0x1d | 0x22..=0x25 => !empty(0, ((id - 0x1a) % 8) as usize),
                    0x1e..=0x21 | 0x26..=0x29 => !empty(1, ((id - 0x1a) % 8 - 4) as usize),
                    _ => true,
                };
            }
            id == 8 || id == 10 || id == 11 || (id == 9 && self.p_moving && self.p_opening)
        };
        SPOTS.iter().filter(|s| on(s.0) && inside(s.1, at)).map(|s| s.0).min()
    }

    /// A press on the screen (`Mouse_Lenyom`): the panels' hot-spots. `None` = not theirs.
    pub fn press(&mut self, state: &SaveState, sel: &Selection, at: [f32; 2], left: bool) -> Option<Vec<Action>> {
        let id = self.spot(state, sel, at)?;
        self.press_id(state, sel, id, at, left)
    }

    /// Hot-spot `id` pressed (also `AktIkonKell`'s presses without the pointer: Esc's 9). `None` = not theirs.
    pub fn press_id(&mut self, state: &SaveState, sel: &Selection, id: u32, at: [f32; 2], left: bool) -> Option<Vec<Action>> {
        if self.transfer.is_some() && (0x44..=0x52).contains(&id) && state.mem.i32(ig2_sim::save::va::TUTORIAL_EVENT) == 5 {
            self.split_ok = true;
        }
        let mut out = Vec::new();
        let (tanks, product) = Self::buttons(state, sel);
        // `Starmap_Main` (exe 0x5b2b70): the click of each button on its press. The tanks and production buttons set
        // `AktRopIkon` and run the radial icon's arm in the same pass, which clicks again.
        if let Some(i) = &mut self.info {
            if matches!(id, 9 | 0x43..=0x58) {
                if i.press(state, sel, id, at) {
                    out = self.close_info();
                }
                if (0x43..=0x52).contains(&id) {
                    self.sounds.push(crate::audio::CLICK);
                }
                return Some(out);
            }
        }
        match id {
            8 => return None,
            10 if tanks => out = [vec![CLICK, CLICK], self.tanks_button(state, sel)].concat(),
            11 if product && !self.locks.production => out = [vec![CLICK, CLICK], self.toggle_production(state, sel)].concat(),
            9 if self.transfer.is_none() && self.locks.exit => {}
            10 | 11 => return None,
            9 => {
                out.push(CLICK);
                if self.transfer.is_some() {
                    out = self.close_transfer(state, sel, true);
                } else {
                    self.close_production();
                }
            }
            0xc | 0xd => {
                let n = state.planet_count();
                let e = en(state);
                let mut p = sel.planet;
                for _ in 0..n {
                    p = if id == 0xc { (p + n - 2) % n + 1 } else { p % n + 1 };
                    if planet(state, p)[0x20] == e {
                        break;
                    }
                }
                let r = planet(state, p);
                out.push(CLICK);
                out.push(Action::Select(Selection { mode: 1, planet: p, ..*sel }));
                out.push(Action::Centre { x: f32_at(&r, 0x18), y: f32_at(&r, 0x1c) });
            }
            0xe..=0x15 => {
                let (l, k) = (((id - 0xe) / 4) as usize, ((id - 0xe) % 4) as usize);
                out.push(CLICK);
                out.push(self.edit(state, sel, 3, l, k, 0));
            }
            0x16..=0x19 => out.extend([CLICK, self.edit(state, sel, 5, 1, (id - 0x16) as usize, 0)]),
            0x1a..=0x29 | 0x43..=0x52 => {
                // The count buttons click on the press edge only; the transfer cells don't.
                if id <= 0x29 {
                    out.push(CLICK);
                }
                self.hold = Some(Hold { spot: id, frames: 0, left });
                out.extend(self.hold_frame(state, sel, at));
            }
            0x2a..=0x31 => self.open_list(state, sel, id, at),
            // The scroll bar follows the pointer every frame while held (`Mouse_Megfogva`).
            0x57 => {
                self.hold = Some(Hold { spot: id, frames: 0, left });
                self.scroll(state, at);
            }
            _ => {}
        }
        Some(out)
    }

    /// A 0x1d edit of line `l`'s slot `k` (by its UI byte `+0x98 + 4L + k`).
    fn edit(&self, state: &SaveState, sel: &Selection, op: i32, l: usize, k: usize, value: i32) -> Action {
        let r = planet(state, sel.planet);
        let ui = r[0x98 + 4 * l + k] as i32;
        Action::Send(StarmapOrder::Production { op, planet: sel.planet as i32, line: l as i32, slot: ui, value })
    }

    /// One 10 ms UI frame while a button is held (`Starmap_Main::KezdoIdo`): the first frame and every frame after
    /// the seventh send, `(KezdoIdo − 7)/8 + 1` at a time (at most 5 for the counts; a right button moves everything).
    pub fn hold_frame(&mut self, state: &SaveState, sel: &Selection, at: [f32; 2]) -> Vec<Action> {
        if let Some(i) = &mut self.info {
            i.hold_frame(state, sel, at);
        }
        let Some(h) = self.hold else { return Vec::new() };
        if h.spot == 0x57 {
            self.scroll(state, at);
            return Vec::new();
        }
        let k = h.frames;
        self.hold = Some(Hold { frames: k + 1, ..h });
        if k != 0 && k < 7 {
            return Vec::new();
        }
        let step = (k - 7) / 8 + 1;
        let mut out = Vec::new();
        match h.spot {
            0x1a..=0x29 => {
                let i = (h.spot - 0x1a) % 8;
                let (l, slot) = ((i / 4) as usize, (i % 4) as usize);
                let r = planet(state, sel.planet);
                let d = i16_at(&r, 0x78 + 8 * l + 2 * slot);
                // Stations have no counts (`+0x37`).
                if design(state, en(state), d.max(0) as usize).is_some_and(|x| x[0x37] != 0) {
                    return out;
                }
                // The pair's left half lowers the count (`Mouse_x` < 164 / 479).
                let op = if (l == 0 && at[0] < 164.0) || (l == 1 && at[0] < 479.0) { 1 } else { 2 };
                out.push(self.edit(state, sel, op, l, slot, step.min(5)));
            }
            0x43..=0x52 => {
                let Some(t) = self.transfer else { return out };
                let item = ((h.spot - 0x43) / 2) as i32 + t.first;
                let to_left = (h.spot - 0x43) % 2 == 1;
                let (from, to) = if to_left { (t.f2, t.f1) } else { (t.f1, t.f2) };
                if let Some(row) = rows(state, &t).get(item as usize) {
                    let count = if h.left { step } else { 1_000_000 };
                    out.push(Action::Send(StarmapOrder::Transfer { from, to, design: row.design as i32, ship: 0, count }));
                }
            }
            _ => {}
        }
        out
    }

    /// The button went up.
    pub fn release(&mut self, state: &SaveState, sel: &Selection) -> Vec<Action> {
        self.hold = None;
        match self.info.as_mut().and_then(|i| i.release()) {
            Some(crate::starmap_info::SaleOrder::Design(o)) => return vec![Action::Design(o)],
            Some(crate::starmap_info::SaleOrder::HeroBack { fleet, design }) => return vec![Action::HeroBack { fleet, design }],
            None => {}
        }
        let Some(list) = self.list.take() else { return Vec::new() };
        let r = planet(state, sel.planet);
        let l = list.line;
        let e = en(state);
        let station = |d: usize| design(state, e, d).is_some_and(|x| x[0x37] != 0);
        // `+0x114` stations built plus the line's queued ones: an eighth can't be queued.
        let stations = i32_at(&r, 0x114) + (0..4).filter(|&k| i16_at(&r, 0x78 + 8 * l + 2 * k) > 0 && station(i16_at(&r, 0x78 + 8 * l + 2 * k) as usize)).count() as i32;
        let current = i16_at(&r, 0x78 + 8 * l + 2 * list.slot) as i32;
        match usize::try_from(list.hovered).ok().and_then(|i| list.designs.get(i).copied()) {
            Some(d) if (stations < 8 || !station(d) || d == 0) && current != d as i32 => vec![CLICK, Action::Send(StarmapOrder::Production {
                op: 4,
                planet: sel.planet as i32,
                line: l as i32,
                slot: list.ui as i32,
                value: d as i32,
            })],
            _ => Vec::new(),
        }
    }

    /// The pointer moved (the open design list follows it).
    pub fn hover(&mut self, at: [f32; 2]) {
        if let Some(i) = &mut self.info {
            i.hover(at);
        }
        if let Some(l) = &mut self.list {
            l.hovered = -1;
            for i in 0..l.designs.len() as i32 {
                let (x, y) = (l.x + 126 * (i / l.per_col), l.y + 13 * (i % l.per_col));
                if at[0] >= x as f32 && at[0] <= (x + 120) as f32 && at[1] >= y as f32 && at[1] < (y + 13) as f32 {
                    l.hovered = i;
                }
            }
        }
    }

    /// Hot-spots 0x2a..=0x31: the slot's design list at the pointer (`smSelect*`): design 0 (remove) and the
    /// player's visible designs of the line's class (`+0x16 == L + 1`, `+0x18`), 32 to a column.
    fn open_list(&mut self, state: &SaveState, sel: &Selection, id: u32, at: [f32; 2]) {
        let l = ((id - 0x2a) / 4) as usize;
        let k = ((id - 0x2a) % 4) as usize;
        let e = en(state);
        let all = state.designs.get(e as usize).map_or(0, |d| d.len());
        let designs: Vec<usize> = (0..all)
            .filter(|&d| d == 0 || design(state, e, d).is_some_and(|x| i16_at(x, 0x16) == l as i16 + 1 && x[0x18] & 1 != 0))
            .collect();
        let n = designs.len() as i32;
        let cols = ((n + 31) / 32).max(1);
        let per_col = (n + cols - 1) / cols;
        let (mx, my) = (at[0] as i32, at[1] as i32);
        let mut y = my - 14;
        let mut x = if l == 0 { mx + 12 } else { mx - 120 * cols - 8 };
        if x - 6 < 0 {
            x = 6;
        }
        if my - 16 < 0 {
            y = 2;
        }
        if x + cols * 126 - 3 > 640 {
            x = 643 - cols * 126;
        }
        if y + per_col * 13 + 2 > 450 {
            y = 448 - per_col * 13;
        }
        let r = planet(state, sel.planet);
        let ui = r[0x98 + 4 * l + k];
        // `StarmapProductSelectHely`: the last slot with that UI byte.
        let slot = (0..4).rev().find(|&j| r[0x98 + 4 * l + j] == ui).unwrap_or(k);
        self.list = Some(DesignList { line: l, slot, ui, x, y, per_col: per_col.max(1), cols, designs, hovered: -1 });
        self.hover(at);
    }

    /// Hot-spot 0x57: the scroll bar sets the first row, 2632/n pixels a thumb, even rows only.
    fn scroll(&mut self, state: &SaveState, at: [f32; 2]) {
        let Some(t) = &mut self.transfer else { return };
        let n = rows(state, t).len() as i32;
        if n < 9 {
            t.first = 0;
            return;
        }
        let h = 2632 / n;
        let mut first = ((at[1] as i32 - 30 - h / 2) * (n - 8) + (329 - h) / 2) / (329 - h);
        first = first.clamp(0, n - 8);
        t.first = (first as f64 / 2.0).ceil() as i32 * 2;
    }

    /// The per-drawn-frame steps of `ShowStarmap` and `StarmapGfx2D_2` (production frame, exit button width, spins)
    /// up to `time`, and the closes of `FRISSIT_Starmap2` (the shown planet isn't the player's).
    pub fn advance(&mut self, state: &SaveState, sel: &Selection, time: f32) -> Vec<Action> {
        let mut out = Vec::new();
        // A planet turning protected (`+0x64`) ends the panel.
        if self.transfer.is_some() && planet(state, sel.planet)[0x64] & 1 != 0 {
            out = self.close_transfer(state, sel, true);
        }
        let last = self.clock.unwrap_or(time);
        let frames = ((time - last) / PANEL_FRAME).floor().clamp(0.0, 120.0) as i32;
        if frames > 0 || self.clock.is_none() {
            self.clock = Some(last + frames as f32 * PANEL_FRAME);
        }
        if self.production_open() && (sel.mode != 1 || planet(state, sel.planet)[0x20] != en(state)) {
            self.close_production();
        }
        for _ in 0..frames {
            self.step_frame();
            let fleet_info = self.info.as_ref().is_some_and(|i| i.kind == crate::starmap_info::Kind::Fleet);
            let target = if self.product { 0xb } else if self.transfer.is_some() || fleet_info { -10 } else { 0 };
            self.addo += (target - self.addo).signum();
            self.spin_frames += 1.0;
            self.head_frames += 1.0;
        }
        if let Some(t) = &mut self.transfer {
            let n = rows(state, t).len() as i32;
            t.first = t.first.clamp(0, (n - 8).max(0));
        }
        out
    }

    /// `ShowStarmap`'s step of the production view's animation (`StarmapPFrame` 0..=12 opening, 17..=30 closing).
    fn step_frame(&mut self) {
        if self.p_moving {
            if !self.p_opening {
                if self.p_frame < 0x1e {
                    self.p_frame += 1;
                } else {
                    self.p_moving = false;
                    self.product = false;
                }
            } else if self.p_frame < 0xc {
                self.p_frame += 1;
            } else {
                self.p_moving = false;
                self.product = true;
            }
        }
    }

    /// `StarmapInfoOffHoltart`: how far the exit button is in (1 = in place).
    fn exit_in(&self) -> f32 {
        if self.p_moving {
            let f = if self.p_frame > 0xc { 0x1e - self.p_frame } else { self.p_frame };
            return f as f32 / 12.0;
        }
        if let Some(i) = &self.info {
            return i.holtart;
        }
        if self.product || self.transfer.is_some() {
            1.0
        } else {
            0.0
        }
    }

    /// `FRISSIT_Starmap2`'s `FaderDesignFrissito`s (ELF 0x719070): `fifo` while the transfer, the fleet info or the
    /// tanks info is up, `starf` always.
    pub fn ui_tick(&mut self) {
        if self.transfer.is_some() || self.info.is_some() {
            self.faders.fifo.tick();
        }
        self.faders.starf.tick();
    }

    /// `StarmapGfx2D_2`'s `starf` count at the exit button (ELF 0x6de1b0: from 6, one a box drawn: the slide-in's
    /// ten, then the production view's ten).
    pub fn exit_fader(&self) -> usize {
        6 + 10 * (self.p_moving as usize + self.product as usize)
    }
}

/// The starmap's two `faderstruktura`s (re/screens/hud.md §1.2.1): `starf` (the HUD's, the production view's and
/// the exit button's faders) and `fifo` (the transfer, fleet info and tanks info panels).
#[derive(Debug, Clone)]
pub struct StarmapFaders {
    pub starf: FaderDesign,
    pub fifo: FaderDesign,
    /// The research screen's `resf` (exe 0xDC4758, `FRISSIT_Research2` 0x51db4b) when the capture had it.
    pub resf: Option<FaderDesign>,
}

impl StarmapFaders {
    /// `Starmap_Belep`: `InitFaderDesignStruktura(fifo)`, then `starf`.
    pub fn enter() -> Self {
        let fifo = FaderDesign::new(1);
        Self { starf: fifo.next(), fifo, resf: None }
    }
}

impl StarmapFaders {
    /// The exe's `fifo`, `starf` (and `resf`) at a capture's grab (`--faders`: the `ui.json` beside the state save, hex strings
    /// of 0x5cc bytes each, run_screen.py).
    pub fn from_ui_json(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let j: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let one = |k: &str| -> Result<FaderDesign, String> {
            let h = j["faders"][k].as_str().ok_or_else(|| format!("{}: no faders.{k}", path.display()))?;
            let b: Vec<u8> = (0..h.len() / 2).filter_map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()).collect();
            FaderDesign::from_exe(&b).ok_or_else(|| format!("{}: faders.{k} too short", path.display()))
        };
        Ok(Self { fifo: one("fifo")?, starf: one("starf")?, resf: one("resf").ok() })
    }
}

impl Default for StarmapFaders {
    fn default() -> Self {
        Self::enter()
    }
}

/// `KirakFader(i, starf, …, 0, 54s, 90s, 100, 0, 90s, 130s, 70)`, `s` and the texture's drift of `starf`'s fader `i`.
pub(crate) fn starf(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4], fd: &FaderDesign, i: usize) {
    let s = fd.s(i);
    kirak_fader(f, white, v, r, [0.0, 54.0 * s, 90.0 * s], 100.0, [0.0, 90.0 * s, 130.0 * s], 70.0, fd.offset(i));
}

/// `KirakFader(i, fifo, …, 0, 54s, 90s, 100, 0, 60s, 110s, 70)`: the info panels' look.
pub(crate) fn fifo(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4], fd: &FaderDesign, i: usize) {
    let s = fd.s(i);
    kirak_fader(f, white, v, r, [0.0, 54.0 * s, 90.0 * s], 100.0, [0.0, 60.0 * s, 110.0 * s], 70.0, fd.offset(i));
}

/// `FadeKerettelKerekit(x, 80, w, 292, 0, 20, 20, 200, 0, 15, 15, 150)`: the production view's big panels.
fn big_panel(f: &mut Frame, white: TexId, v: &Virtual, x: f32, w: f32) {
    fader(f, white, v, [x, 80.0, w, 292.0], [0.0, 20.0, 20.0], 200.0, [0.0, 15.0, 15.0], 150.0);
}

/// `FillToScreen(x, y, w, h, c)`: a solid `DDColor` rectangle.
fn fill(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4], c: [f32; 3]) {
    let [x, y, w, h] = r;
    if w > 0.0 && h > 0.0 {
        f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
    }
}

/// What the 2D pass needs besides the state.
pub struct Draw2d<'a> {
    pub font: &'a Font,
    pub white: TexId,
    pub v: Virtual,
    pub palette: &'a [[f32; 3]],
    pub texts: &'a crate::ui::UiTexts,
    pub time: f32,
}

impl Draw2d<'_> {
    fn col(&self, i: usize) -> [f32; 3] {
        self.palette.get(i).copied().unwrap_or([1.0; 3])
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn text(&self, f: &mut Frame, s: &[u8], x: f32, y: f32, len: usize, size: usize, align: u32, col: usize) {
        let a = match align & 3 {
            0 => Align::Left,
            1 => Align::Field,
            2 => Align::Right,
            _ => Align::Centre,
        };
        self.font.text(f, &self.v, s, x, y, len, size, a, self.col(col), None);
    }
}

impl Panels {
    /// The faders under the models (`StarmapGfx2D_2`'s and `ShowStarmap`'s), in the original's order.
    pub fn draw_under(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, sel: &Selection) {
        let (white, v) = (d.white, &d.v);
        // `StarmapGfx2D_2`'s count (`local_2c`), one a drawn box.
        let (fd, mut n) = (&self.faders.starf, 6);
        let mut starf = |f: &mut Frame, r: [f32; 4]| {
            starf(f, white, v, r, fd, n);
            n += 1;
        };
        if self.p_moving {
            // The opening/closing slide (`StarmapGfx2D_2`, `StarmapPModeIn`).
            let fr = if self.p_frame > 0xc { 0x1e - self.p_frame } else { self.p_frame } as f32;
            let slide = |o: f32, cut: f32| {
                let x = ((((fr - o) / 6.0) as f64 * std::f64::consts::FRAC_PI_2).cos() * 130.0) as f32;
                if fr - o < 0.0 {
                    130.0
                } else if fr > cut {
                    0.0
                } else {
                    x
                }
            };
            let s = [slide(0.0, 6.0), slide(2.0, 8.0), slide(4.0, 10.0), slide(6.0, 12.0)];
            for (i, (y, h)) in [(125.0, 60.0), (206.0, 50.0), (261.0, 50.0), (316.0, 50.0)].into_iter().enumerate() {
                starf(f, [11.0 - s[i], y, 120.0, h]);
            }
            for (i, (y, h)) in [(125.0, 60.0), (206.0, 50.0), (261.0, 50.0), (316.0, 50.0)].into_iter().enumerate() {
                starf(f, [s[i] + 511.0, y, 120.0, h]);
            }
            let g = fr + 1.0;
            for cx in [70.0, 570.0] {
                starf(f, [cx - g * 38.0 / 13.0, 103.0 - g * 17.0 / 13.0, g * 76.0 / 13.0, g * 35.0 / 13.0]);
            }
            let k = (1.0 - ((fr / 12.0) as f64 * std::f64::consts::FRAC_PI_2).sin()) as f32 * 200.0;
            big_panel(f, white, v, (5.0 - k).trunc(), 190.0);
            big_panel(f, white, v, (k + 445.0).trunc(), 191.0);
        }
        if self.product {
            big_panel(f, white, v, 5.0, 190.0);
            big_panel(f, white, v, 445.0, 191.0);
            for x in [11.0, 511.0] {
                for (y, h) in [(125.0, 60.0), (206.0, 50.0), (261.0, 50.0), (316.0, 50.0)] {
                    starf(f, [x, y, 120.0, h]);
                }
            }
            starf(f, [32.0, 86.0, 76.0, 35.0]);
            starf(f, [532.0, 86.0, 76.0, 35.0]);
        }
        if let Some(t) = &self.transfer {
            // `ShowStarmap`'s transfer faders (`fifo`): fill (0, 54s, 90s) at 100, frame (0, 60s, 110s) at 70.
            // Faders 0..5, the scroll bar's track and thumb both 6.
            let fifo = |f: &mut Frame, r: [f32; 4], i: usize| fifo(f, white, v, r, &self.faders.fifo, i);
            for (i, r) in [[28.0, 27.0, 273.0, 334.0], [338.0, 27.0, 273.0, 334.0], [30.0, 29.0, 269.0, 17.0], [30.0, 342.0, 269.0, 17.0], [340.0, 29.0, 269.0, 17.0], [340.0, 342.0, 269.0, 17.0]].into_iter().enumerate() {
                fifo(f, r, i);
            }
            let n = rows(state, t).len() as i32;
            let (mut h, mut top) = if n < 9 { (0x14b, 0) } else { (0xa58 / n, t.first * (0x14b - 0xa58 / n) / (n - 8)) };
            if top + h > 0x14b {
                let over = top + h - 0x14b;
                h -= over / 2;
                top -= over / 2;
            }
            fifo(f, [306.0, 29.0, 28.0, 333.0], 6);
            fifo(f, [310.0, (top + 0x22) as f32, 20.0, (h - 8) as f32], 6);
        }
        if let Some(i) = &self.info {
            i.draw_under(f, d, state, sel, &self.faders.fifo);
        }
        // The exit button (`StarmapGfx2D_2`, `StarmapInfoOffHoltart` > 0). The tanks info leaves the holtart at 0
        // (only the product, transfer and fleet modes set it) and draws its own box below instead.
        let tanks = self.info.as_ref().is_some_and(|i| i.kind == crate::starmap_info::Kind::Tanks);
        let k = self.exit_in();
        if k > 0.0 && !tanks {
            let x = ((1.0 - ((k as f64) * std::f64::consts::FRAC_PI_2).sin()) * 200.0 + 445.0) as f32;
            starf(f, [x, 380.0, (self.addo + 0xb0) as f32, 30.0]);
        }
        // The tanks info's box (`StarmapTanksInfo`), the next count.
        if tanks {
            starf(f, [445.0, 380.0, 176.0, 30.0]);
        }
    }

    /// The panels' 3D texture, then their texts (`StarmapGfx2D_3`, `StarmapGfx2D_4`'s design list).
    pub fn draw_over(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, sel: &Selection, rules: &ig2_rules::Rules, models: Option<TexId>) {
        let v = &d.v;
        if let Some(tex) = models {
            f.push(tex, Blend::Alpha, Instance::new(kind::SPRITE, v.px(320.0, 240.0), crate::kit::Wide::new(v).half(v), [1.0; 4]));
        }
        if let Some(t) = &self.transfer {
            self.draw_transfer_texts(f, d, state, t);
        }
        if self.product {
            self.draw_production_texts(f, d, state, sel);
        }
        if let Some(i) = &self.info {
            i.draw_over(f, d, state, sel, rules);
        }
        let k = self.exit_in();
        if k > 0.0 {
            let x = ((1.0 - ((k as f64) * std::f64::consts::FRAC_PI_2).sin()) * 200.0 + 500.0 - 25.0) as f32;
            d.text(f, &d.texts.get(2, 4, 4), x.trunc(), 390.0, 13, 3, 0xd, 0x25);
        }
    }

    fn draw_production_texts(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, sel: &Selection) {
        let r = planet(state, sel.planet);
        let name = cstr(&r[..0x18]);
        if name.len() < 13 {
            d.text(f, name, 320.0, 48.0, 12, 3, 7, 1);
        } else {
            d.text(f, name, 320.0, 50.0, 20, 2, 7, 1);
        }
        let q = queue(state, sel.planet);
        for (l, line) in q.iter().enumerate() {
            let lx = l as f32;
            if line.capacity < 1 {
                d.text(f, &d.texts.get(2, 4, 2), lx * 500.0 + 69.0, 105.0, 8, 3, 7, 1);
            } else {
                d.text(f, line.capacity.to_string().as_bytes(), lx * 500.0 + 69.0, 105.0, 8, 3, 7, 1);
            }
            d.text(f, &d.texts.get(2, 4, 3), lx * 500.0 + 41.0, 94.0, 10, 2, 5, 1);
            for (k, slot) in line.slots.iter().enumerate() {
                let Some((_, name, count, price)) = slot else { continue };
                let selected = self.list.as_ref().is_some_and(|s| s.line == l && s.slot == k);
                let y = SLOT_Y[k];
                d.text(f, name, lx * 501.0 + 31.0, y + 2.0, 18, 2, 4, if selected { 6 } else { 3 });
                d.text(f, count.to_string().as_bytes(), lx * 315.0 + 163.0, y + 12.0, 3, 2, 7, 1);
                let money = [b"$".as_slice(), price.to_string().as_bytes()].concat();
                d.text(f, &money, lx * 501.0 + 31.0, y + 20.0 + if l == 0 { 2.0 } else { 0.0 }, 10, 2, 4, 1);
            }
            if let Some(w) = line.progress {
                let (bar, rest) = (d.col(3), d.col(6));
                fill(f, d.white, &d.v, [lx * 500.0 + 32.0, 152.0, w as f32, 4.0], bar);
                // The rest blinks (`KiirStrSzinSzorzo = AktIdo300Szazalek`) without capacity, else it is black.
                let m = if line.capacity < 1 { crate::hud::blink(d.time, 300.0) } else { 0.0 };
                fill(f, d.white, &d.v, [lx * 500.0 + 32.0 + w as f32, 152.0, (76 - w) as f32, 4.0], rest.map(|c| c * m));
            }
        }
        if let Some(list) = &self.list {
            // `FaderHanyadikMenet = 2`: the list's box (0, 10, 0) at 60, frame (0, 5, 0) at 30, then the names.
            let w = (20 + 1) * 6 * list.cols + 3;
            fader(f, d.white, &d.v, [(list.x - 6) as f32, (list.y - 2) as f32, w as f32, (list.per_col * 13 + 4) as f32], [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
            let e = en(state);
            let stations = i32_at(&r, 0x114)
                + (0..4).filter(|&k| i16_at(&r, 0x78 + 8 * list.line + 2 * k) > 0 && design(state, e, i16_at(&r, 0x78 + 8 * list.line + 2 * k) as usize).is_some_and(|x| x[0x37] != 0)).count() as i32;
            for (i, &dd) in list.designs.iter().enumerate() {
                let i = i as i32;
                let Some(rec) = design(state, e, dd) else { continue };
                let mut col = if i == list.hovered { 6 } else { 3 };
                if stations > 7 && rec[0x37] != 0 {
                    col = 1;
                }
                d.text(f, cstr(&rec[..0x14]), (list.x + 126 * (i / list.per_col)) as f32, (list.y + 3 + 13 * (i % list.per_col)) as f32, 0x12, 2, 4, col);
            }
        }
    }

    fn draw_transfer_texts(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, t: &Transfer) {
        let name = |h: i32| -> Vec<u8> {
            if h < 1 {
                cstr(&planet(state, (-h).max(0) as usize)[..0x18]).to_vec()
            } else {
                fleet(state, h).map(|r| cstr(&r[..0x14]).to_vec()).unwrap_or_default()
            }
        };
        d.text(f, &name(t.f1), 156.0, 32.0, 0x13, 3, 7, 3);
        d.text(f, &name(t.f2), 484.0, 32.0, 0x13, 3, 7, 3);
        // `ShowFlottaTankCap(110, 347, f1)` and (420, 347) for the right: "<label> %d/%d" or "<label><none>".
        let cap = |h: i32| -> Vec<u8> {
            if h < 1 {
                let r = planet(state, (-h).max(0) as usize);
                [d.texts.get(2, 5, 4), format!(" {}", i32_at(&r, 0x58)).into_bytes()].concat()
            } else {
                let r = fleet(state, h).unwrap_or(&[0u8; 0x80][..]);
                if i32_at(r, 0x78) == 0 {
                    [d.texts.get(2, 5, 2), d.texts.get(2, 5, 3)].concat()
                } else {
                    [d.texts.get(2, 5, 2), format!(" {}/{}", i32_at(r, 0x5c), i32_at(r, 0x78)).into_bytes()].concat()
                }
            }
        };
        d.text(f, &cap(t.f1), 164.0, 347.0, 0x28, 2, 7, 1);
        d.text(f, &cap(t.f2), 474.0, 347.0, 0x28, 2, 7, 1);
        let e = en(state);
        let full = |h: i32| h > 0 && fleet(state, h).is_some_and(|r| i32_at(r, 0x78) <= i32_at(r, 0x5c));
        for (i, row) in rows(state, t).iter().enumerate() {
            let i = i as i32 - t.first;
            if !(0..8).contains(&i) {
                continue;
            }
            let Some(rec) = design(state, e, row.design) else { continue };
            let mut label = cstr(&rec[..0x14]).to_vec();
            // An outdated design (`+0x1c` upgrade target): "name (<obsolete>)".
            if i32_at(rec, 0x1c) > 0 {
                label.extend_from_slice(b" (");
                label.extend(d.texts.get(2, 4, 1));
                label.extend_from_slice(b")");
            }
            let col = match row.group {
                1 => 3,
                2 => 7,
                _ => 1,
            };
            let (cx, cy) = ((i % 2 * 0x81) as f32, (i / 2 * 0x46) as f32);
            d.text(f, &label, cx + 91.0, cy + 124.0, 0x19, 2, 7, col);
            d.text(f, &label, cx + 419.0, cy + 124.0, 0x19, 2, 7, col);
            let lc = if row.group == 3 && full(t.f1) { 6 } else { 1 };
            let rc = if row.group == 3 && full(t.f2) { 6 } else { 1 };
            d.text(f, row.left.to_string().as_bytes(), cx + 91.0, cy + 112.0, 0xe, 2, 7, lc);
            d.text(f, row.right.to_string().as_bytes(), cx + 419.0, cy + 112.0, 0xe, 2, 7, rc);
        }
    }
}

/// A model with its textures, ready to bake.
struct Loaded {
    model: Model,
    texs: Vec<TexId>,
    /// Per part, its self-illumination texture (`Part::self_illum`).
    glows: Vec<Option<TexId>>,
}

/// The panels' models on the GPU side: `gyartas.xvb` by animation frame, the hull models, their textures, and the
/// texture they are drawn into.
pub struct PanelModels {
    prod: HashMap<usize, Model>,
    units: HashMap<(usize, u8, u8, u32, u32), Option<Loaded>>,
    textures: HashMap<String, TexId>,
    white: TexId,
    target: Option<Target>,
    /// The info box's globe (`vector/info/bolygo_1.xvb`, loaded once; its sub-mesh 0's UVs scaled per surface) and
    /// its own target.
    globe: Option<Option<Model>>,
    globes: HashMap<u8, Model>,
    globe_target: Option<Target>,
    /// The fleet picture `flag<race>0.xvb` per race, and the fleet it was last loaded for with the frame it was
    /// (`SInfoOldFleet`: a new fleet starts the turn again).
    flags: HashMap<u8, Option<Loaded>>,
    flag_fleet: Option<(usize, f32)>,
}

/// `TransfAlap` `+0x34`: where the hull's model and its textures are (`ShowStarmap`, `ProductGFX`).
fn transfer_dirs(dir: i32) -> (&'static str, &'static str) {
    match dir {
        1 => ("vector/spacewar/", "textures/spacewar/"),
        3 => ("vector/colony/", "textures/colony/"),
        _ => ("vector/talalmanyok/", "textures/talalmanyok/"),
    }
}

impl PanelModels {
    pub fn new(r: &mut Renderer) -> Self {
        let white = r.add_texture(&crate::assets::Image::white());
        Self { prod: HashMap::new(), units: HashMap::new(), textures: HashMap::new(), white, target: None, globe: None, globes: HashMap::new(), globe_target: None, flags: HashMap::new(), flag_fleet: None }
    }

    /// The info box's planet picture (`StarmapLoadInfoBolygo` ELF 0x6d3950, `StarmapFrissitBolygoUrhajok` 0x6d3d40,
    /// re/screens/starmap.md §11): in planet mode `bolygo_1.xvb` at `AddToScene` scale 0.029, camera-fixed at (−545,
    /// 365, −1100) (virtual ≈ (23, 449)), stood up by `RotateObj(π/2, 0, 0)` and turned by `matrixforgat(M, M, 0.05,
    /// 0)` every drawn frame (`frames` of them), sub-mesh 0 skinned with `BolygoTexturak[surface − 1]`; lit like the
    /// panels' models. In fleet mode the fleet picture instead (`render_flag`). The caller draws it over
    /// `StarmapGfx2D_1`'s fader. `None` in neither mode.
    #[allow(clippy::too_many_arguments)]
    pub fn render_globe(&mut self, r: &mut Renderer, install: &Install, state: &SaveState, sel: &Selection, cam: &[[f32; 3]; 3], size: [f32; 2], frames: f32) -> Option<TexId> {
        if sel.mode == 2 {
            return self.render_flag(r, install, state, sel, cam, size, frames);
        }
        if sel.mode != 1 || sel.planet == 0 || sel.planet > state.planet_count() {
            return None;
        }
        let surface = state.mem.u8(va::PLANETS + (sel.planet * PLANET_SIZE) as u32 + 0x22);
        // `BolygoTexturak[surface − 1]` = `textures/starmap/p<surface>.bmp` (the ninth is p1 again).
        let n = if (1..=8).contains(&surface) { surface } else { 1 };
        let base = self.globe.get_or_insert_with(|| Model::load(install, "vector/info/", "bolygo_1.xvb", "textures/info/").map_err(|e| eprintln!("ig2-client: {e}")).ok()).clone()?;
        let skin = install.resolve_texture("textures/starmap/", &format!("p{n}.bmp"));
        let model = self
            .globes
            .entry(n)
            .or_insert_with(|| {
                // The remaster's `ig2_vertex.shader` scales the UVs by the texture's `uv_mul` (§3.3).
                let k = skin.as_deref().map_or([1.0; 2], |t| crate::assets::Image::uv_mul(install, t));
                let mut m = base;
                for p in m.parts.iter_mut().filter(|p| p.index.first() == Some(&0)) {
                    for v in &mut p.verts {
                        v.uv = [v.uv[0] * k[0], v.uv[1] * k[1]];
                    }
                }
                m
            })
            .clone();
        let texs: Vec<TexId> = model
            .parts
            .iter()
            .map(|p| {
                let path = if p.index.first() == Some(&0) { skin.clone() } else { p.texture.clone() };
                self.texture(r, install, path.as_deref())
            })
            .collect();
        Some(self.render_box_model(r, &model, &texs, 0.029, [-545.0, 365.0, -1100.0], cam, size, frames))
    }

    /// The info box's fleet picture (`StarmapLoadInfoFlottak` ELF 0x6d3b10, re/screens/starmap.md §11): in fleet mode,
    /// not for a special fleet (`fSpec`, flags 0x4400) or an ownerless one, `vector/spacewar/flag<race>0.xvb` of the
    /// owner's race (`PlayerFaj`; above 8 the sixth) at scale 0.03, camera-fixed at (−535, 365, −1100), stood up and
    /// turned as the globe from the frame its fleet was selected.
    #[allow(clippy::too_many_arguments)]
    fn render_flag(&mut self, r: &mut Renderer, install: &Install, state: &SaveState, sel: &Selection, cam: &[[f32; 3]; 3], size: [f32; 2], frames: f32) -> Option<TexId> {
        let f = state.fleets.get(sel.fleet).filter(|_| sel.fleet > 0)?;
        if u16::from_le_bytes([f[0x52], f[0x53]]) & 0x4400 != 0 || f[0x15] == 0xff {
            return None;
        }
        let race = state.races().get(f[0x15] as usize).copied().unwrap_or(0);
        let d = if race > 8 { 6 } else { race };
        if d == 0 {
            return None;
        }
        // The first fleet seen turns from frame 0 (a still's clock); a later change restarts the turn.
        let since = match self.flag_fleet {
            Some((k, at)) if k == sel.fleet => at,
            Some(_) => frames,
            None => 0.0,
        };
        self.flag_fleet = Some((sel.fleet, since));
        if !self.flags.contains_key(&d) {
            let loaded = Model::load(install, "vector/spacewar/", &format!("flag{d}0.xvb"), "textures/spacewar/").map_err(|e| eprintln!("ig2-client: {e}")).ok().map(|model| {
                let paths: Vec<Option<String>> = model.parts.iter().map(|p| p.texture.clone()).collect();
                let texs = paths.iter().map(|p| self.texture(r, install, p.as_deref())).collect();
                Loaded { model, texs, glows: Vec::new() }
            });
            self.flags.insert(d, loaded);
        }
        let (model, texs) = self.flags.get(&d)?.as_ref().map(|l| (l.model.clone(), l.texs.clone()))?;
        Some(self.render_box_model(r, &model, &texs, 0.03, [-535.0, 365.0, -1100.0], cam, size, frames - since))
    }

    /// One of the info box's camera-fixed pictures: `RotateObj(π/2, 0, 0)`, turned by `matrixforgat(M, M, 0.05, 0)`
    /// a drawn frame, lit like the panels' models.
    #[allow(clippy::too_many_arguments)]
    fn render_box_model(&mut self, r: &mut Renderer, model: &Model, texs: &[TexId], scale: f32, at: V3, cam: &[[f32; 3]; 3], size: [f32; 2], frames: f32) -> TexId {
        let l = [10.0, 10.0, -10.0];
        let lights = lighting::Lights::default().ambient(0x404040).parallel(0x808080, std::array::from_fn(|i| cam[i][0] * l[0] + cam[i][1] * l[1] + cam[i][2] * l[2]));
        let spin = scene::axis_matrix([0.0, -0.05, 0.0], 0.05 * frames.floor());
        let rot = scene::mul(&spin, &scene::create_matrix(std::f32::consts::FRAC_PI_2, 0.0, 0.0));
        let m = scene::mul(&rot, &Placement::add_to_scene(model.origin, scale).matrix).map(|x| x * 1024.0);
        let (mut dynamic, mut draws) = (Vec::new(), Vec::new());
        bake(model, &m, at, texs, &[], 1.0, &lights, &mut dynamic, &mut draws);
        finish(r, &mut self.globe_target, dynamic, draws, size)
    }

    fn texture(&mut self, r: &mut Renderer, install: &Install, path: Option<&str>) -> TexId {
        let Some(path) = path else { return self.white };
        if let Some(&t) = self.textures.get(path) {
            return t;
        }
        let t = match crate::assets::Image::load(install, path) {
            Ok(img) => r.add_texture(&img),
            Err(e) => {
                eprintln!("ig2-client: {e}");
                self.white
            }
        };
        self.textures.insert(path.to_owned(), t);
        t
    }

    /// The model of hull `hull` (`TransfAlap`) with race digit `digit` in a spacewar hull's name (hulls < 12), and
    /// a ground hull's skin `tankok/<kind><owner>.bmp` on its first sub-mesh (owner capped at 10), at the level of
    /// detail `Kocka` draws it at for `AddToScene(…, s)` at depth `z` (`Starmap_Belep`: `valtashatar` 1600).
    #[allow(clippy::too_many_arguments)]
    fn unit(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, hull: usize, digit: u8, owner: u8, s: f32, z: f32) -> Option<&Loaded> {
        let key = (hull, digit, owner, s.to_bits(), z.to_bits());
        if !self.units.contains_key(&key) {
            let loaded = (|| {
                let m = rules.unit_models.transfer.get(hull)?;
                if m.name.is_empty() {
                    return None;
                }
                let (vdir, tdir) = transfer_dirs(m.dir);
                let mut name = m.name.clone().into_bytes();
                if m.dir == 1 && hull < 12 && name.len() > 4 {
                    name[4] = (b'0' + digit).min(b'8');
                }
                let name = String::from_utf8_lossy(&name).into_owned();
                let model = Model::load_lod(install, vdir, &name, tdir, s, z, 1600).map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
                let skin = (hull > 0x12)
                    .then(|| ["kerekes", "kozep", "lebeg", "lanc", "aknarak"].get(hull - 0x13).map(|k| format!("tankok/{k}{}.bmp", owner.min(10))))
                    .flatten()
                    .and_then(|s| install.resolve_texture("textures/colony/", &s));
                let first = model.parts.first().map(|p| p.path.split('/').nth(1).unwrap_or("").to_owned()).unwrap_or_default();
                let paths: Vec<Option<String>> = model
                    .parts
                    .iter()
                    .map(|p| if skin.is_some() && p.path.split('/').nth(1) == Some(first.as_str()) { skin.clone() } else { p.texture.clone() })
                    .collect();
                let texs = paths.iter().map(|p| self.texture(r, install, p.as_deref())).collect();
                let glows = model.parts.iter().map(|p| p.self_illum.as_deref().map(|t| self.texture(r, install, Some(t)))).collect();
                Some(Loaded { model, texs, glows })
            })();
            self.units.insert(key, loaded);
        }
        self.units.get(&key).and_then(|l| l.as_ref())
    }

    /// Render the panels' models into their texture (`None` when nothing is shown). `size` = the window in pixels,
    /// `cam` the starmap camera's rotation (its rows), which turns the scene's light.
    #[allow(clippy::too_many_arguments)]
    pub fn render(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, p: &Panels, state: &SaveState, sel: &Selection, cam: &[[f32; 3]; 3], size: [f32; 2]) -> Option<TexId> {
        let show_prod = p.p_frame != 0 && (p.p_moving || p.product);
        if !show_prod && p.transfer.is_none() && p.info.is_none() {
            return None;
        }
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let e = en(state);
        let races = state.races();
        // Every panel model is lit per vertex by the starmap scene's lights (`Init3dStarmap`: ambient 0x404040, the
        // parallel 0x808080 at (10, 10, −10)), the light turned into camera space by the camera's rotation; the
        // console's two side lights don't reach them [obs: light probe on the fleet panel].
        let l = [10.0, 10.0, -10.0];
        let lights = lighting::Lights::default().ambient(0x404040).parallel(0x808080, std::array::from_fn(|i| cam[i][0] * l[0] + cam[i][1] * l[1] + cam[i][2] * l[2]));
        if show_prod {
            let frame = p.p_frame.clamp(0, 30) as usize;
            if let std::collections::hash_map::Entry::Vacant(slot) = self.prod.entry(frame) {
                match Model::load_at(install, "vector/starmap/", "gyartas.xvb", "textures/starmap/", Some(frame)) {
                    Ok(m) => {
                        slot.insert(m);
                    }
                    Err(e) => eprintln!("ig2-client: {e}"),
                }
            }
            let lamp = |n: &str| install.resolve_texture("textures/starmap/", n);
            let (off, fleet_off, on) = (lamp("gyart_gr.bmp"), lamp("gyart_gr_2.bmp"), lamp("gyart_gr_3.bmp"));
            let q = queue(state, sel.planet);
            if let Some(m) = self.prod.get(&frame).cloned() {
                let place = Placement::add_to_scene(m.origin, 0.6336).world((0.0, 0.0, 0.0), [0.0, -50.0, -600.0]);
                let texs: Vec<TexId> = m
                    .parts
                    .iter()
                    .map(|part| {
                        // `ProductGFX`: the repeat lamps (`+0x90 + 4L + k`) and line 1's "into a fleet" lamps
                        // (`+0xad + 4 + k`) take `gyart_gr_3` when on.
                        let mut tex = part.texture.clone();
                        for (row, g) in BUTTON_GROUPS.iter().enumerate() {
                            let (l, k) = (row / 4, row % 4);
                            let has = |n: u32| part.path.split('/').any(|s| s == format!("Group{n:02}"));
                            if has(g[3]) {
                                tex = if q[l].repeat[k] { on.clone() } else { off.clone() };
                            }
                            if l == 1 && has(g[2]) {
                                tex = if q[l].into_fleet[k] { on.clone() } else { fleet_off.clone() };
                            }
                        }
                        self.texture(r, install, tex.as_deref())
                    })
                    .collect();
                // `Init3dStarmap` gives `gyartas` an ambient 0x0a0a0a of its own, after the scene's lights.
                bake(&m, &place.matrix, place.translation, &texs, &[], 1.0, &lights.clone().ambient(0x0a0a0a), &mut dynamic, &mut draws);
            }
            // The head units (`ProductGFX`): the ship line's at (−540, −350, −1600) ×0.375, the vehicles' at
            // (540, −350, −1600) ×0.28, turning 0.05 a frame, as solid as the unit is built (5..95 %).
            if p.product {
                let r0 = planet(state, sel.planet);
                let owner = r0[0x20];
                let spin = scene::mul(&scene::axis_matrix([0.0, (-0.5f32).cos(), (-0.5f32).sin()], 0.05 * p.head_frames), &tilt());
                for (l, at, s) in [(0usize, [-540.0, -350.0, -1600.0], 0.375f32), (1, [540.0, -350.0, -1600.0], 0.28)] {
                    let d = i16_at(&r0, 0x78 + 8 * l);
                    let Some(rec) = (d > 0).then(|| design(state, owner, d as usize)).flatten() else { continue };
                    let hull = i32_at(rec, 0x44).max(0) as usize;
                    let digit = if l == 0 { races.get(i16_at(rec, 0x40).clamp(0, 18) as usize).copied().unwrap_or(0) } else { races.get(e as usize).copied().unwrap_or(0) };
                    let t = q[l].progress.map_or(0.05, |w| (w as f32 / 76.0).clamp(0.05, 0.95));
                    if let Some(u) = self.unit(r, install, rules, hull, digit, owner, s, at[2]) {
                        let m = scene::mul(&spin, &Placement::add_to_scene(u.model.origin, s).matrix).map(|x| x * 1024.0);
                        let (model, texs, glows) = (u.model.clone(), u.texs.clone(), u.glows.clone());
                        bake(&model, &m, at, &texs, &glows, t, &lights, &mut dynamic, &mut draws);
                    }
                }
            }
        }
        if let Some(t) = &p.transfer {
            let owner = fleet(state, state.mem.i32(va::AKT_FLEET)).map_or(e, |r| r[0x15]);
            for (i, row) in rows(state, t).iter().enumerate() {
                let k = i as i32 - t.first;
                if !(0..8).contains(&k) {
                    continue;
                }
                let Some(rec) = design(state, e, row.design) else { continue };
                let hull = i32_at(rec, 0x44).max(0) as usize;
                let digit = i16_at(rec, 0x40).clamp(0, 9) as u8;
                let spin = scene::mul(&scene::axis_matrix([0.0, (-0.5f32).cos(), (-0.5f32).sin()], (i % 3 + 1) as f32 * 0.03 * p.spin_frames), &tilt());
                let (x, y) = ((k % 2 * 0x81) as f32, (k / 2 * 0x46 - 0x9c) as f32);
                if let Some(u) = self.unit(r, install, rules, hull, digit, owner, 0.04, -600.0) {
                    let m = scene::mul(&spin, &Placement::add_to_scene(u.model.origin, 0.04).matrix).map(|x| x * 1024.0);
                    let (model, texs, glows) = (u.model.clone(), u.texs.clone(), u.glows.clone());
                    // An empty side's model is three quarters transparent.
                    let a = |n: i32| if n == 0 { 0.25 } else { 1.0 };
                    bake(&model, &m, [x - 229.0, y, -600.0], &texs, &glows, a(row.left), &lights, &mut dynamic, &mut draws);
                    bake(&model, &m, [x + 99.0, y, -600.0], &texs, &glows, a(row.right), &lights, &mut dynamic, &mut draws);
                }
            }
        }
        if let Some(i) = &p.info {
            for pose in i.poses(state, sel) {
                if let Some(u) = self.unit(r, install, rules, pose.hull, pose.digit, pose.owner, pose.scale, pose.at[2]) {
                    let m = scene::mul(&pose.rot, &Placement::add_to_scene(u.model.origin, pose.scale).matrix).map(|x| x * 1024.0);
                    let (model, texs, glows) = (u.model.clone(), u.texs.clone(), u.glows.clone());
                    bake(&model, &m, pose.at, &texs, &glows, 1.0, &lights, &mut dynamic, &mut draws);
                }
            }
        }
        if dynamic.is_empty() {
            return None;
        }
        Some(finish(r, &mut self.target, dynamic, draws, size))
    }
}

/// Draw baked camera-fixed models into `target` (made at the window's size on first use) and give its texture.
fn finish(r: &mut Renderer, target: &mut Option<Target>, dynamic: Vec<MeshVertex>, draws: Vec<MeshDraw>, size: [f32; 2]) -> TexId {
    {
        let wide = crate::kit::Wide::new(&crate::font::Virtual::new(size));
        let px = wide.px;
        let light = scene::normalise([0.0, -0.35, 1.0]);
        // Camera space → clip over the virtual 640 × 480: vx = 600·x/w + 320, vy = 600·y/w + 250 (w = −z); depth
        // 0..1 over w = 400..2100.
        let (a, b) = (1.0 / (1.0 / 400.0 - 1.0 / 2100.0) / 400.0, 1.0 / (1.0 / 400.0 - 1.0 / 2100.0));
        let rows4 = [[1200.0 / 640.0, 0.0, 0.0, 0.0], [0.0, -1200.0 / 480.0, -(1.0 - 500.0 / 480.0), 0.0], [0.0, 0.0, -a, -b], [0.0, 0.0, -1.0, 0.0]];
        let rows4 = wide.rows(rows4);
        let view = std::array::from_fn(|c| std::array::from_fn(|k| rows4[k][c]));
        let mut f = Frame::new([0.0, 0.0, 0.0, 0.0]);
        f.scene = Some(Scene3d {
            gamma: true,
            shadow: None,
            uniforms: SceneUniforms {
                view,
                eye: [0.0; 4],
                light: [light[0], light[1], light[2], 0.0],
                ambient: [0.251, 0.251, 0.251, 0.0],
                sun: [1.0, 0.808, 0.502, 0.0],
                fog: [0.0; 4],
                fog_range: [1e9, 2e9, 0.0, 1e9],
                flash: [[0.0; 4]; 2],
            },
            draws,
            dynamic,
        });
        let target = match target {
            Some(t) => t,
            None => target.insert(r.target(px)),
        };
        r.draw_into(&f, target, px, 0.0);
        target.tex
    }
}

/// `RotateObj(π/2, 0, 0)` then `RotateObj2((1, 0, 0), −0.5)`: the panels' models stand up and lean towards the eye.
pub(crate) fn tilt() -> M3 {
    scene::mul(&scene::axis_matrix([1.0, 0.0, 0.0], -0.5), &scene::create_matrix(std::f32::consts::FRAC_PI_2, 0.0, 0.0))
}

/// Bake a camera-fixed model (`SetRelative(false)`) at `at` with matrix `m`; `alpha` < 1 draws it as glass.
/// Bake a camera-fixed model with the engine's vertex colours (the shader's pre-lit path: no normal), then each
/// part's self-illumination (`selfillumkirak`: an additive unlit pass in the grey `255 − t·255`, `t` the object's
/// transparency, else the material's; re/screens/design.md §12c).
#[allow(clippy::too_many_arguments)]
fn bake(model: &Model, m: &M3, at: V3, texs: &[TexId], glows: &[Option<TexId>], alpha: f32, lights: &lighting::Lights, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    for (pi, p) in model.parts.iter().enumerate() {
        let start = out.len() as u32;
        let a = p.alpha(alpha);
        for v in &p.verts {
            let q = scene::apply(m, v.pos);
            let c = lights.rgb(lighting::normal(m, v.lnormal));
            out.push(MeshVertex { pos: [q[0] + at[0], q[1] + at[1], q[2] + at[2]], normal: [0.0; 3], uv: v.uv, color: [c[0], c[1], c[2], a] });
        }
        let tex = texs.get(pi).copied().unwrap_or(texs.first().copied().unwrap_or(TexId(0)));
        draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: p.material_glass(alpha) });
        if let Some(&Some(g)) = glows.get(pi) {
            let t = if alpha < 1.0 { 1.0 - alpha } else { p.transparency };
            crate::design_scene::glow(p, &scene::Transform { matrix: *m, translation: at }, g, t, out, draws);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starmap_faders_follow_the_exe_counts() {
        // `Starmap_Belep`: `fifo` takes the stream's first 60 draws, `starf` the next.
        let f = StarmapFaders::enter();
        let mut fifo = FaderDesign::new(1);
        let mut starf = fifo.next();
        let mut p = Panels::default();
        // `FRISSIT_Starmap2`: `starf` always, `fifo` only with a transfer or info panel up.
        p.ui_tick();
        starf.tick();
        assert_eq!((p.faders.starf.s(0), p.faders.starf.offset(3)), (starf.s(0), starf.offset(3)));
        assert_eq!((p.faders.fifo.s(0), p.faders.fifo.offset(0)), (0.0, [0.0; 2]));
        assert_eq!(f.fifo.offset(0), [0.0; 2]);
        p.transfer = Panels::opened("transfer:0,1").unwrap().transfer;
        p.ui_tick();
        fifo.tick();
        assert_eq!((p.faders.fifo.s(5), p.faders.fifo.offset(5)), (fifo.s(5), fifo.offset(5)));
        // `StarmapGfx2D_2` counts from 6: the exit button after the slide-in's and the production view's ten.
        assert_eq!(Panels::default().exit_fader(), 6);
        assert_eq!(Panels::opened("production").unwrap().exit_fader(), 16);
    }

    #[test]
    fn hot_spots_follow_the_exe_table() {
        // The minus half of slot 0's pair and its repeat lamp (exe 0x5b99d0).
        assert_eq!(spot_rect(0x1a), Some([135, 132, 28, 51]));
        assert_eq!(spot_rect(0xe), Some([153, 164, 21, 24]));
        assert!(inside(spot_rect(0x2e).unwrap(), [600.0, 130.0]));
        assert!(!inside(spot_rect(0x2e).unwrap(), [632.0, 130.0]));
    }

    #[test]
    fn production_opens_in_twelve_frames_and_closes_from_seventeen() {
        let mut p = Panels { p_moving: true, p_opening: true, ..Default::default() };
        assert!(p.p_moving && p.p_opening && p.p_frame == 0);
        for _ in 0..13 {
            p.step_frame();
        }
        assert!(p.product && !p.p_moving && p.p_frame == 12);
        p.close_production();
        assert_eq!(p.p_frame, 0x11);
        for _ in 0..14 {
            p.step_frame();
        }
        assert!(!p.product && !p.p_moving && p.p_frame == 30);
    }

    /// The panels' clicks as lockstep commands on `quick-1.sav`: a design picked from slot 0's list, its count
    /// raised, the unit finished, the new fleet split and its ship moved across the transfer panel (skips without the
    /// install and the save).
    #[test]
    fn panel_clicks_reach_the_sim() {
        use ig2_net::{Command, StarmapCmd};
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let mut w = crate::world::World::load(&save, &rules, &defaults, &volatile);
        // The starmap on view (the window's `set_screen` each frame) over a save that holds another `AktKep`.
        w.game.state.mem.set_i32(va::AKT_KEP, 0);
        w.game.set_screen(2);
        let send = |w: &mut crate::world::World, acts: Vec<Action>| {
            for a in acts {
                if let Action::Send(o) = a {
                    w.send(Command::Starmap { order: StarmapCmd::from(o) });
                }
            }
            w.pump();
        };
        let e = en(&w.game.state);
        let p = (1..=w.game.state.planet_count()).find(|&p| planet(&w.game.state, p)[0x20] == e).unwrap();
        let sel = Selection { mode: 1, planet: p, fleet: 0 };
        let mut panels = Panels::opened("production").unwrap();
        // Slot 0's list (hot-spot 0x2a): a plain warship (no station, no fighter).
        assert_eq!(panels.press(&w.game.state, &sel, [50.0, 150.0], true), Some(vec![]));
        let list = panels.list.clone().unwrap();
        let state = &w.game.state;
        let (i, d) = list.designs.iter().copied().enumerate().find(|&(_, d)| d > 0 && design(state, e, d).is_some_and(|x| x[0x36] == 0 && x[0x37] == 0)).map(|(i, d)| (i as i32, d)).unwrap();
        panels.hover([(list.x + 126 * (i / list.per_col) + 5) as f32, (list.y + 13 * (i % list.per_col) + 5) as f32]);
        let acts = panels.release(&w.game.state, &sel);
        send(&mut w, acts);
        assert_eq!(queue(&w.game.state, p)[0].slots[0].as_ref().map(|s| s.0), Some(d));
        // The pair's right half (hot-spot 0x22) raises the count once on the press.
        let acts = panels.press(&w.game.state, &sel, [170.0, 150.0], true).unwrap();
        send(&mut w, acts);
        panels.release(&w.game.state, &sel);
        assert_eq!(queue(&w.game.state, p)[0].slots[0].as_ref().map(|s| s.2), Some(2));
        // Finish it: a fleet of the player's appears beside the planet.
        w.send(Command::Starmap { order: StarmapCmd::from(StarmapOrder::FinishHeads { planet: p as i32 }) });
        w.pump();
        w.tick();
        let f = near_fleet(&w.game.state, p);
        assert!(f > 0, "no fleet built");
        // Split it; the transfer opens between the two and the ship crosses (hot-spot 0x43: row 0, left to right).
        w.send(Command::Starmap { order: StarmapCmd::from(StarmapOrder::Split { fleet: f }) });
        w.pump();
        let new = w
            .order_events
            .iter()
            .find_map(|x| match x {
                ig2_sim::game::OrderEvent::Split { new, .. } => Some(*new as i32),
                _ => None,
            })
            .unwrap();
        let mut panels = Panels::default();
        panels.open_transfer(f, new, false);
        w.game.set_transfer(Some((f, new)));
        // The empty new fleet outlives the next ticks while the panel shows it (issue #2: it went at once).
        for _ in 0..2 {
            w.tick();
            w.pump();
        }
        assert!(fleet(&w.game.state, new).is_some_and(|r| r[0x15] == e), "the split's new fleet was deleted");
        let t = panels.transfer.unwrap();
        assert_eq!(rows(&w.game.state, &t), vec![Row { design: d, group: 1, left: 1, right: 0 }]);
        let sel = Selection { mode: 2, planet: p, fleet: new as usize };
        let acts = panels.press(&w.game.state, &sel, [50.0, 100.0], true).unwrap();
        send(&mut w, acts);
        assert_eq!(rows(&w.game.state, &t), vec![Row { design: d, group: 1, left: 0, right: 1 }]);
        // The emptied fleet stays while the panel shows it (no 0x1b).
        w.tick();
        w.pump();
        assert!(fleet(&w.game.state, f).is_some_and(|r| r[0x15] == e), "the panel's emptied fleet was deleted");
    }

    #[test]
    fn axis_rotation_turns_like_creatematrixfromaxis() {
        let m = scene::axis_matrix([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2);
        let p = scene::apply(&m, [1.0, 0.0, 0.0]);
        assert!((p[0]).abs() < 1e-6 && (p[1] - 1.0).abs() < 1e-6, "{p:?}");
    }
}
