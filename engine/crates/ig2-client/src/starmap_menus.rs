//! The starmap's remaining input UI (re/screens/starmap.md §14–§17): the radial icon menu ("rop" icons:
//! `Starmap_Main` ELF 0x6f50a0 / exe 0x5b2b70, `Starmap_MouseInit2` ELF 0x7024b0, `StarmapGfx2D_3` exe 0x5aaba0), the
//! planet's long info (`StarmapPlanetInfoMod`, `StarmapHosszuInfoKiir` ELF 0x708440, `GFX_Starmap` 0x6f36c0) with
//! its tax, auto-build and priority settings, the select menus (`StarmapGfx2D_2` 0x6de1b0 boxes, `StarmapGfx2D_4`
//! 0x6d6b20 texts) and the production view's planet list. Every order goes out as a `MULTIGET_Starmap` message.

use std::cell::Cell;

use ig2_sim::game::StarmapOrder;
use ig2_sim::save::{va, SaveState, PLANET_SIZE, TECH_SIZE};

use crate::font::Align;
use crate::orders::Selection;
use crate::render::Frame;
use crate::kit::fader::fader;
use crate::starmap_panels::{Action, Draw2d, Panels};

/// `Starmap_RopIkonX/Y` (exe 0x9270d4/0x9270e8): the places' hit centres from `LastMouseX/Y`, places 1..=4.
const HIT: [[i32; 2]; 4] = [[29, 86], [81, 125], [132, 86], [81, 49]];
/// `Starmap_RopIkonX2/Y2` (exe 0x9270fc/0x927110): the icons' top-left from `LastMouseX/Y`.
const ICON_AT: [[i32; 2]; 4] = [[5, 62], [56, 100], [107, 62], [56, 24]];
/// `LastMouseX/Y = Mouse − (82, 84)`.
const ORIGIN: [i32; 2] = [82, 84];
/// `Starmap_RopIkonKonvert` (exe 0x927088): an icon's frame of `StarmapBrushok` 9.
pub const ICON_FRAME: [i32; 19] = [0, 13, 9, 10, 8, 1, 2, 6, 7, 5, 0, 0, 0, 3, 4, 0, 11, 12, 14];

/// One drawn frame of `GFX_Starmap`'s zoom loops (the display's 60 Hz [inf], like the panels).
const DRAWN_FRAME: f32 = crate::starmap_panels::PANEL_FRAME;

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

fn design(state: &SaveState, e: u8, d: usize) -> Option<&[u8]> {
    state.designs.get(e as usize)?.get(d).map(|r| &r[..])
}

/// What the screen does besides the panels' actions.
#[derive(Debug, Clone, PartialEq)]
pub enum Do {
    Act(Action),
    /// The colony screen of the selected planet (icon 4, the info box's right click).
    Colony,
    /// The production view on/off (icon 2, `AktRopIkon` 2).
    Production,
    /// The tanks button (icon 1).
    Tanks,
    /// The transfer with the nearest own fleet (icon 0xe).
    Join,
    /// The vehicle info panel (icon 9, re/screens/starmap.md §20).
    TanksInfo,
    /// The fleet's info panel (`AktRopIkon` 0xf in fleet mode, §19).
    FleetInfo,
}

/// The radial menu while the button is held (`StarmapRopIkonMod`): the press point, `RopIkonFrame` (UI frames, up to
/// 50), the four places' icons (`StarmapRopIkonok`) and the one under the pointer (`AktRopIkon2`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rop {
    pub at: [i32; 2],
    pub frame: i32,
    pub places: [i32; 4],
    pub hover: i32,
}

impl Rop {
    /// `LastMouseX/Y`.
    fn last(&self) -> [i32; 2] {
        [self.at[0] - ORIGIN[0], self.at[1] - ORIGIN[1]]
    }

    /// From frame 50: the icon of the last place within 30 pixels of the pointer (`AktRopIkon2`).
    fn hover_at(&mut self, at: [f32; 2]) {
        self.hover = 0;
        if self.frame < 50 {
            return;
        }
        let [lx, ly] = self.last();
        let (dx, dy) = (at[0] as i32 - lx, at[1] as i32 - ly);
        for (k, c) in HIT.iter().enumerate() {
            if (c[0] - dx) * (c[0] - dx) + (c[1] - dy) * (c[1] - dy) <= 900 {
                self.hover = self.places[k];
            }
        }
    }
}

/// The long info's camera phase (`StarmapPlanetInfoZoomMod`): flying in, up, flying back out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InfoPhase {
    Entering,
    Up,
    /// Leaving; `true` = then open the production view (the production button).
    Leaving(bool),
}

/// The planet's long info (`StarmapPlanetInfoMod`): the planet, the camera to come back to
/// (`BackupStarmapXStart/YStart/Zoom`) and where the fly started (`Starmap_Main::sx/sy/sz`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanetInfo {
    pub planet: usize,
    pub backup: [f32; 3],
    pub start: [f32; 3],
    pub phase: InfoPhase,
}

impl PlanetInfo {
    /// `L = (int)max(540, BackupZoom)`.
    fn floor(&self) -> f32 {
        self.backup[2].max(540.0).trunc()
    }

    /// `q = (Zoom − L)/(600 − L)`, `None` while the panel isn't shown (`Zoom ≤ L`).
    fn q(&self, zoom: f32) -> Option<f32> {
        let l = self.floor();
        (zoom > l).then(|| (zoom - l) / (600.0 - l))
    }
}

/// The select menus (§16, §17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Tax,
    AutoBuild,
    Priority,
    /// The sale of station `i` (1-based, hot-spot 0x64 + i).
    Sale(i32),
    Planets,
}

/// An open select menu: `smSelectX/Y`, `smSelected` and the lines it lists.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectMenu {
    pub kind: MenuKind,
    pub x: i32,
    pub y: i32,
    pub hovered: i32,
    /// The items' values (tax levels, auto-build modes, priorities 1..=3, the sale's 1, planets).
    pub items: Vec<i32>,
    /// The sale's line, or the planets' production texts.
    pub texts: Vec<Vec<u8>>,
    /// The planet list's columns: name width `a`, text width `b` (characters), rows per column.
    pub a: i32,
    pub b: i32,
    pub rows: i32,
    pub cols: i32,
}

impl SelectMenu {
    fn hover(&mut self, at: [f32; 2]) {
        let (mx, my) = (at[0] as i32, at[1] as i32);
        let (x, y) = (self.x, self.y);
        self.hovered = -1;
        match self.kind {
            MenuKind::Tax | MenuKind::AutoBuild => {
                for (i, &v) in self.items.iter().enumerate() {
                    let i = i as i32;
                    if mx >= x && mx <= x + 120 && my >= y + 13 * i && my < y + 13 * (i + 1) {
                        self.hovered = v;
                    }
                }
            }
            MenuKind::Priority => {
                for k in 1..4 {
                    if mx >= x && mx <= x + 120 && my >= y + 13 * (k - 1) && my < y + 13 * k {
                        self.hovered = k;
                    }
                }
            }
            MenuKind::Sale(_) => {
                let len = self.texts.first().map_or(0, |t| t.len()) as i32;
                if mx >= x && mx <= x + 6 * len && my >= y && my < y + 13 {
                    self.hovered = 1;
                }
            }
            MenuKind::Planets => {
                let w = 6 * (self.a + self.b + 1);
                for (i, &p) in self.items.iter().enumerate() {
                    let (c, r) = (i as i32 / self.rows, i as i32 % self.rows);
                    if mx >= x + w * c && mx <= x + w * c + w && my >= y + 13 * r && my < y + 13 * (r + 1) {
                        self.hovered = p;
                    }
                }
            }
        }
    }
}

/// The long info's hot-spots (`Starmap_MouseInit` exe 0x5b99d0, re/screens/starmap.md §15.3).
const INFO_SPOTS: &[(u32, [i32; 4])] = &[
    (0x5a, [400, 289, 100, 16]),
    (0x5b, [506, 289, 30, 16]),
    (0x5c, [542, 289, 20, 16]),
    (0x5d, [568, 289, 20, 16]),
    (0x5e, [400, 311, 100, 16]),
    (0x5f, [506, 311, 30, 16]),
    (0x60, [400, 333, 100, 16]),
    (0x61, [506, 333, 30, 16]),
    (0x62, [542, 333, 20, 16]),
    (0x63, [568, 333, 20, 16]),
    (100, [320, 34, 300, 324]),
];

/// Hot-spot 9, the exit button (445, 380, 164, 60).
pub fn exit_spot(at: [f32; 2]) -> bool {
    inside([445, 380, 164, 60], at)
}

use crate::kit::inside;

/// The screen's menus and modes (the original's screen globals).
#[derive(Debug, Clone, Default)]
pub struct Menus {
    pub rop: Option<Rop>,
    /// `StarmapAtiranyitasMode`: the next press picks the rally planet.
    pub redirect: bool,
    /// `StarmapSatelliteElkuldve`, `StarmapKolonizaloElkuldve`, `StarmapTransferOnElkuldve`: an order of the kind is
    /// in the queue (cleared once the handler ran, the next frame's message pass).
    pub sat_sent: bool,
    pub col_sent: bool,
    pub split_sent: bool,
    pub info: Option<PlanetInfo>,
    pub menu: Option<SelectMenu>,
    /// The pointer (virtual) and whether a button is held (`Mouse_Gomb`).
    pub pointer: [f32; 2],
    pub held: bool,
    /// The last drawn-frame time the zoom loops advanced to.
    clock: Option<f32>,
    /// `StarmapHosszuInfoKiir`'s statics: the planet, `oldcounter`, `hanyadik`.
    special: Cell<(usize, i32, usize)>,
}

/// `IndultEKolonizalo(e, p)`: a fleet of `e` ordered to planet `p` colonising or terraforming (flags 0x8, 0x10).
fn colony_ship_heading(state: &SaveState, e: u8, p: usize) -> bool {
    state.fleets.iter().skip(1).any(|r| {
        r[0x15] == e && f32_at(r, 0x44) == p as f32 && i16_at(r, 0x40) == 1 && i16_at(r, 0x42) == 1 && i16_at(r, 0x52) as u16 & 0x18 != 0
    })
}

/// `BolygoInfoIs(p, info)`: one of the planet's info slots 1..=5 (`+0x16a`) holds `info`.
fn info_is(r: &[u8], info: i16) -> bool {
    (0..5).any(|h| i16_at(r, 0x16a + 2 * h) == info)
}

/// The researched status (5) of tech `t` for `e` (`Talalmanyok + 0x45 + e`).
fn researched(state: &SaveState, t: usize, e: u8) -> bool {
    state.mem.u8(va::TECHS + (t * TECH_SIZE) as u32 + 0x45 + e as u32) == 5
}

/// `CsokkentettTervAr(e, d)` (`ai-empire.md` §5.3, the sim's `ai::resale`): a design's price over
/// `GetTervEladasOszto` (2, fighters 10, the story's cheap sale 0.5, story designs nothing).
pub fn resale(state: &SaveState, e: u8, d: usize) -> i32 {
    let Some(r) = design(state, e, d) else { return 0 };
    let mut f: f32 = if r[0x36] & 1 != 0 { 10.0 } else { 2.0 };
    let story = i32_at(&state.mem.read(va::STORY_ADAT + 0xac, 4), 0) != 0;
    if story && i32_at(r, 0x44) == 5 && i16_at(r, 0x40) as i32 == e as i32 {
        f = 0.5;
    }
    let id = i32_at(r, 0x80);
    if (0x6a5..3000).contains(&id) && ![0x8fc, 0x8fd, 0x906].contains(&id) && !(0x924..=0x937).contains(&id) {
        return 0;
    }
    (i32_at(r, 0x24) as f32 / f) as i32
}

/// `Starmap_MouseInit2`'s four places (re/screens/starmap.md §14.3); `blink` = `AktIdo100` bit 0. The story icons
/// (0x10, 0x12) aren't offered (not ported).
pub fn rop_places(state: &SaveState, sel: &Selection, panels: &Panels, redirect: bool, sent: (bool, bool), blink: bool) -> [i32; 4] {
    let mut pl = [0; 4];
    if panels.active() {
        return pl;
    }
    let e = en(state);
    if sel.mode == 1 {
        if sel.planet < 1 || sel.planet > state.planet_count() {
            return pl;
        }
        let r = planet(state, sel.planet);
        let owner = r[0x20];
        pl[1] = 4;
        if owner == e {
            pl[3] = 9;
        } else {
            let sats = r[0xc4 + e as usize];
            if sats == 0 && !sent.0 && researched(state, 15, e) {
                pl[0] = 5;
            }
            if sats < 2 && !sent.0 && researched(state, 16, e) {
                pl[0] = 6;
            }
        }
        if owner == 0 {
            let pool = |a: u32| state.mem.i32(a + 4 * e as u32);
            let (ships, terra) = (pool(va::COLONY_SHIP_POOL), pool(va::TERRAFORMER_POOL));
            if !sent.1 && (ships > 0 || terra > 0) && !colony_ship_heading(state, e, sel.planet) && !info_is(&r, -0x2094) {
                if ships > 0 {
                    pl[3] = 8;
                }
                if terra > 0 {
                    pl[2] = 7;
                }
            }
        } else if owner == e {
            pl[0] = 2;
            pl[2] = 3;
        }
        if redirect && owner == e && blink {
            pl[1] = 3;
        }
    } else if sel.mode == 2 {
        let Some(f) = state.fleets.get(sel.fleet) else { return pl };
        if f[0x15] == e {
            if i32_at(f, 0x54) > 1 {
                pl[0] = 0xd;
            }
            if crate::starmap_panels::nearest_own_fleet(state, sel.fleet as i32) > 0 {
                pl[1] = 0xe;
            }
            if crate::starmap_panels::near_planet(state, sel.fleet as i32) > 0 {
                pl[2] = 1;
            }
            if i16_at(f, 0x52) as u16 & 0x2000 != 0 {
                pl[2] = 0x11;
            }
        }
        if i16_at(f, 0x52) & 1 != 0 {
            for (k, x) in [(0, 0xd), (1, 0xe), (2, 1)] {
                if pl[k] == x {
                    pl[k] = 0;
                }
            }
        }
    }
    pl
}

impl Menus {
    /// The long info is on (`StarmapPlanetInfoMod`).
    pub fn info_on(&self) -> bool {
        self.info.is_some()
    }

    /// The handlers of the frame's message pass have run: the "sent" flags clear.
    pub fn sent_done(&mut self) {
        (self.sat_sent, self.col_sent, self.split_sent) = (false, false, false);
    }

    /// The left button went down on the map with `pick` under it (`SelectNearestUnit`, or `SelectNearestPlanet` with
    /// the production view up): the pick becomes the selection and the menu opens at the press.
    pub fn open_rop(&mut self, at: [f32; 2], places: [i32; 4]) {
        self.rop = Some(Rop { at: [at[0] as i32, at[1] as i32], frame: 0, places, hover: 0 });
    }

    /// One 10 ms UI frame (`FRISSIT_Starmap2`): the menu's frame runs up to 50.
    pub fn ui_frame(&mut self) {
        if let Some(r) = &mut self.rop {
            r.frame = (r.frame + 1).min(50);
            r.hover_at(self.pointer);
        }
    }

    /// The pointer moved (virtual pixels).
    pub fn hover(&mut self, at: [f32; 2]) {
        self.pointer = at;
        if let Some(r) = &mut self.rop {
            r.hover_at(at);
        }
        if let Some(m) = &mut self.menu {
            m.hover(at);
        }
    }

    /// The left button's release with the menu open: its icon (`AktRopIkon = AktRopIkon2`), 0 for none.
    pub fn release_rop(&mut self) -> i32 {
        self.rop.take().map_or(0, |r| r.hover)
    }

    /// `Starmap_Main`'s `AktRopIkon` block (re/screens/starmap.md §14.5).
    pub fn fire(&mut self, icon: i32, state: &SaveState, sel: &Selection) -> Vec<Do> {
        let p = sel.planet as i32;
        let mut out = Vec::new();
        // Every icon but the colony's clicks first (`Starmap_Main`'s `AktRopIkon` arms, exe 0x5b54e8..0x5b5e2b).
        let click = Do::Act(Action::Sound(crate::audio::CLICK));
        if matches!(icon, 1..=3 | 5..=8 | 0xe) || (icon == 0xd && !self.split_sent) {
            out.push(click);
        }
        match icon {
            1 => out.push(Do::Tanks),
            2 => out.push(Do::Production),
            3 => self.redirect = !self.redirect,
            4 => out.push(Do::Colony),
            5 | 6 => {
                out.push(Do::Act(Action::Send(StarmapOrder::Satellite { planet: p, kind: icon - 4 })));
                self.sat_sent = true;
            }
            7 | 8 => {
                out.push(Do::Act(Action::Send(StarmapOrder::Colonise { planet: p, kind: 9 - icon })));
                self.col_sent = true;
            }
            0xd if !self.split_sent => {
                if let Some(f) = state.fleets.get(sel.fleet) {
                    out.push(Do::Act(Action::ZoomIn { x: f32_at(f, 0x30), y: f32_at(f, 0x34), zoom: 590.0 }));
                    out.push(Do::Act(Action::Send(StarmapOrder::Split { fleet: sel.fleet as i32 })));
                    self.split_sent = true;
                }
            }
            9 => out.push(Do::TanksInfo),
            0xe => out.push(Do::Join),
            0xf if sel.mode == 2 => out.push(Do::FleetInfo),
            _ => {}
        }
        out
    }

    /// The redirect mode's pick (or a right press on the player's planet): `0x400(AktPlanet, target)` for the
    /// player's planet under the pointer (0 when it is the planet itself); the mode ends.
    pub fn redirect_to(&mut self, state: &SaveState, sel: &Selection, target: Option<usize>) -> Vec<Do> {
        self.redirect = false;
        match target {
            Some(t) if sel.mode == 1 && planet(state, sel.planet)[0x20] == en(state) => {
                let t = if t == sel.planet { 0 } else { t as i32 };
                vec![Do::Act(Action::Send(StarmapOrder::Rally { planet: sel.planet as i32, target: t }))]
            }
            _ => Vec::new(),
        }
    }

    /// The info box left-clicked in planet mode: the long info flies in from the camera `cam` (x, y, zoom).
    pub fn enter_info(&mut self, planet: usize, cam: [f32; 3]) {
        if self.info.is_none() {
            self.info = Some(PlanetInfo { planet, backup: cam, start: cam, phase: InfoPhase::Entering });
            self.menu = None;
        }
    }

    /// Leave the long info (hot-spot 8, 9, or 11 with `production`).
    pub fn leave_info(&mut self, cam: [f32; 3], production: bool) {
        if let Some(i) = &mut self.info {
            if !matches!(i.phase, InfoPhase::Leaving(_)) {
                i.phase = InfoPhase::Leaving(production);
                i.start = cam;
            }
            self.menu = None;
        }
    }

    /// The long info's hot-spot at `at` (the lowest id wins); `None` when it is off.
    pub fn info_spot(&self, state: &SaveState, at: [f32; 2]) -> Option<u32> {
        let i = self.info?;
        let own = planet(state, i.planet)[0x20] == en(state);
        let stations = i32_at(&planet(state, i.planet), 0x114).clamp(0, 10) as u32;
        let station = (0..stations).find(|&k| inside([0, 40 + 30 * k as i32, 100, 20], at)).map(|k| 0x65 + k);
        let spot = INFO_SPOTS.iter().filter(|s| (own || s.0 == 100) && inside(s.1, at)).map(|s| s.0).min();
        [station, spot].into_iter().flatten().min()
    }

    /// A press on the long info's hot-spots: the buttons send, the menus open. `None` = not theirs.
    pub fn press_info(&mut self, state: &SaveState, at: [f32; 2]) -> Option<Vec<Do>> {
        let id = self.info_spot(state, at)?;
        let p = self.info?.planet as i32;
        let click = Do::Act(Action::Sound(crate::audio::CLICK));
        let set = |what: i32| vec![click.clone(), Do::Act(Action::Send(StarmapOrder::Setting { planet: p, what, value: 0 }))];
        Some(match id {
            0x5c => set(1),
            0x5d => set(2),
            0x62 => set(3),
            0x63 => set(4),
            0x5b => set(5),
            0x5f => set(6),
            0x61 => set(7),
            0x5a => self.open_menu(state, MenuKind::Tax, at),
            0x5e => self.open_menu(state, MenuKind::AutoBuild, at),
            0x60 => self.open_menu(state, MenuKind::Priority, at),
            // The station sale menu clicks as it opens (exe 0x5b85b2); the others only on their pick.
            0x65..=0x6e => [vec![click], self.open_menu(state, MenuKind::Sale(id as i32 - 0x64), at)].concat(),
            _ => Vec::new(),
        })
    }

    /// Open a select menu at the pointer (§16, §17).
    pub fn open_menu(&mut self, state: &SaveState, kind: MenuKind, at: [f32; 2]) -> Vec<Do> {
        let (mx, my) = (at[0] as i32, at[1] as i32);
        let e = en(state);
        let p = self.info.map_or(0, |i| i.planet);
        let mut m = SelectMenu { kind, x: mx + 12, y: my - 20, hovered: -1, items: Vec::new(), texts: Vec::new(), a: 0, b: 0, rows: 0, cols: 1 };
        match kind {
            MenuKind::Tax => m.items = (0..8).collect(),
            MenuKind::AutoBuild => {
                let race5 = state.races().get(e as usize).copied() == Some(5);
                m.items = (0..=if race5 { 6 } else { 8 }).collect();
            }
            MenuKind::Priority => m.items = (1..4).collect(),
            MenuKind::Sale(i) => {
                (m.x, m.y) = ((mx - 12).max(6), my + 16);
                let d = i16_at(&planet(state, p), 0x100 + 2 * (i as usize - 1)).max(0) as usize;
                let name = design(state, e, d).map(|r| cstr(&r[..0x14]).to_vec()).unwrap_or_default();
                m.items = vec![1];
                m.texts = vec![name, resale(state, e, d).to_string().into_bytes()];
            }
            MenuKind::Planets => self.fill_planets(state, &mut m, at),
        }
        m.hover(at);
        self.menu = Some(m);
        Vec::new()
    }

    /// The planet list (§17): the player's planets, their ship production, the columns, the clamped place.
    fn fill_planets(&self, state: &SaveState, m: &mut SelectMenu, at: [f32; 2]) {
        let e = en(state);
        let (mut a, mut b) = (0usize, 0usize);
        for p in 1..=state.planet_count() {
            let r = planet(state, p);
            if r[0x20] != e {
                continue;
            }
            let name = cstr(&r[..0x18]).to_vec();
            a = a.max(name.len());
            let mut text = Vec::new();
            let head = i16_at(&r, 0x78);
            if head > 0 {
                let time = design(state, e, head as usize).map_or(1, |d| i32_at(d, 0x20));
                let pct = if time != 0 { (100 - i32_at(&r, 0xa0).wrapping_mul(100) / time).min(99) } else { 99 };
                let s = format!("{pct:>2}%-");
                text = [s.as_bytes(), design(state, e, head as usize).map_or(&[][..], |d| cstr(&d[..0x14]))].concat();
                b = b.max(text.len());
            }
            m.items.push(p as i32);
            m.texts.push([name, text].concat());
        }
        // `texts` holds name + production; split them back by the name's length when drawing.
        let n = m.items.len() as i32;
        m.cols = ((n + 31) / 32).max(1);
        m.rows = ((n + m.cols - 1) / m.cols).max(1);
        (m.a, m.b) = (a as i32, b as i32);
        let w = 6 * (m.a + m.b + 1) * m.cols;
        let (mut x, mut y) = (at[0] as i32 + 12, at[1] as i32 + 6);
        if x - 6 < 0 {
            x = 6;
        }
        if y - 22 < 0 {
            y = 22;
        }
        if x + w - 3 > 640 {
            x = 643 - w;
        }
        if y + m.rows * 13 + 2 > 480 {
            y = 478 - m.rows * 13;
        }
        (m.x, m.y) = (x, y);
    }

    /// The button went up over an open menu: its order (or the planet list's jump).
    pub fn release_menu(&mut self, state: &SaveState, sel: &Selection) -> Vec<Do> {
        let Some(m) = self.menu.take() else { return Vec::new() };
        if m.hovered < 0 || (m.kind == MenuKind::Planets && m.hovered < 1) {
            return Vec::new();
        }
        let p = self.info.map_or(sel.planet, |i| i.planet) as i32;
        // A pick clicks (exe 0x5b80ca, 0x5b8121, 0x5b817c, 0x5b85e9); the planet list zooms (`StarmapBeZoomol`'s
        // sound, the mode forced back to 0 after it).
        let send = |o| vec![Do::Act(Action::Sound(crate::audio::CLICK)), Do::Act(Action::Send(o))];
        match m.kind {
            MenuKind::Tax => send(StarmapOrder::Setting { planet: p, what: 8, value: m.hovered }),
            MenuKind::AutoBuild => send(StarmapOrder::Setting { planet: p, what: 9, value: m.hovered }),
            MenuKind::Priority => send(StarmapOrder::Setting { planet: p, what: 10, value: m.hovered }),
            MenuKind::Sale(i) => send(StarmapOrder::SellStation { planet: p, index: i }),
            MenuKind::Planets => {
                let r = planet(state, m.hovered as usize);
                vec![
                    Do::Act(Action::Sound(crate::audio::SoundId::global(5))),
                    Do::Act(Action::Select(Selection { mode: 1, planet: m.hovered as usize, ..*sel })),
                    Do::Act(Action::Centre { x: f32_at(&r, 0x18), y: f32_at(&r, 0x1c) }),
                ]
            }
        }
    }

    /// The long info's zoom loops and pin per drawn frame up to `time` (`Starmap_Main`'s fly in/out, `GFX_Starmap`'s
    /// pin to the orbiting planet `orbit` at the camera's turn): returns the camera to show, the star scale of the
    /// planet (`StarSize` over its backup) and, once out, whether the production view opens.
    pub fn advance_info(&mut self, cam: [f32; 4], orbit: impl Fn(usize) -> [f32; 2], time: f32) -> (Option<[f32; 3]>, f32, Option<bool>) {
        let last = self.clock.unwrap_or(time);
        let frames = ((time - last) / DRAWN_FRAME).floor().clamp(0.0, 120.0) as i32;
        if frames > 0 || self.clock.is_none() {
            self.clock = Some(last + frames as f32 * DRAWN_FRAME);
        }
        let Some(mut i) = self.info else { return (None, 1.0, None) };
        let [_, _, mut zoom, rotate] = cam;
        let target = |p: usize| info_pivot(orbit(p), rotate);
        let mut shown = [cam[0], cam[1], zoom];
        let mut star = 0.0;
        for _ in 0..frames.max(1) {
            match i.phase {
                InfoPhase::Entering => {
                    let [sx, sy, sz] = i.start;
                    if frames > 0 {
                        zoom = (zoom + (600.0 - zoom) / 2.0).min(599.0);
                    }
                    let t = target(i.planet);
                    let k = if 599.0 - sz != 0.0 { (zoom - sz) / (599.0 - sz) } else { 1.0 };
                    shown = [sx + (t[0] - sx) * k, sy + (t[1] - sy) * k, zoom];
                    star = 1.0 - k;
                    if zoom >= 599.0 {
                        i.phase = InfoPhase::Up;
                    }
                }
                InfoPhase::Up => {
                    let t = target(i.planet);
                    shown = [t[0], t[1], 599.0];
                    zoom = 599.0;
                    star = 0.0;
                }
                InfoPhase::Leaving(prod) => {
                    let [sx, sy, sz] = i.start;
                    let [bx, by, bz] = i.backup;
                    if frames > 0 {
                        zoom = (zoom - (600.0 - zoom) / 2.0).max(bz);
                    }
                    let k = if bz - sz != 0.0 { (zoom - sz) / (bz - sz) } else { 1.0 };
                    shown = [sx + (bx - sx) * k, sy + (by - sy) * k, zoom];
                    star = k;
                    if zoom <= bz {
                        self.info = None;
                        return (Some(i.backup), 1.0, Some(prod));
                    }
                }
            }
            if frames == 0 {
                break;
            }
        }
        if self.info.is_some() {
            self.info = Some(i);
        }
        (Some(shown), star, None)
    }
}

/// Menus already open for a snapshot (`--planet-info`, `--rop-menu X,Y`, `--menu KIND@X,Y`, `--mouse X,Y`), like
/// `screen.js` stages them in the exe: the long info up on the selected planet (the camera the save holds is its
/// pin; the backup zoom isn't saved, 400 is taken), the radial menu at frame 50 with the button held, a menu's box at
/// X,Y. Returns the menus and the camera `[XStart, YStart, Zoom]` to show.
#[allow(clippy::too_many_arguments)]
pub fn staged(state: &SaveState, sel: &Selection, panels: &Panels, cam: [f32; 4], info: bool, rop: Option<[f32; 2]>, menu: Option<&str>, mouse: Option<[f32; 2]>, time: f32) -> Result<(Menus, Option<[f32; 3]>), String> {
    let mut m = Menus { pointer: mouse.unwrap_or([320.0, 240.0]), ..Default::default() };
    let mut shown = None;
    if info {
        let backup = [cam[0], cam[1], if cam[2] < 599.0 { cam[2] } else { 400.0 }];
        m.info = Some(PlanetInfo { planet: sel.planet, backup, start: backup, phase: InfoPhase::Up });
        shown = Some([cam[0], cam[1], 599.0]);
    }
    if let Some(at) = rop {
        let blink = (time * 10.0) as i64 % 2 == 1;
        let places = rop_places(state, sel, panels, false, (false, false), blink);
        m.rop = Some(Rop { at: [at[0] as i32, at[1] as i32], frame: 50, places, hover: 0 });
        m.held = true;
    }
    if let Some(spec) = menu {
        let (kind, xy) = spec.split_once('@').ok_or("--menu KIND@X,Y")?;
        let (x, y) = xy.split_once(',').ok_or("--menu KIND@X,Y")?;
        let num = |v: &str| v.trim().parse::<i32>().map_err(|e| format!("--menu: {e}"));
        let (x, y) = (num(x)?, num(y)?);
        let kind = match kind {
            "tax" => MenuKind::Tax,
            "autobuild" => MenuKind::AutoBuild,
            "priority" => MenuKind::Priority,
            "planets" => MenuKind::Planets,
            k => MenuKind::Sale(k.strip_prefix("sale:").and_then(|n| n.parse().ok()).ok_or(format!("--menu {k}"))?),
        };
        if m.info.is_none() && kind != MenuKind::Planets {
            m.info = Some(PlanetInfo { planet: sel.planet, backup: [cam[0], cam[1], 400.0], start: [cam[0], cam[1], 400.0], phase: InfoPhase::Up });
        }
        m.open_menu(state, kind, [x as f32, y as f32]);
        if let Some(menu) = &mut m.menu {
            (menu.x, menu.y) = (x, y);
        }
        m.held = true;
        let p = m.pointer;
        m.hover(p);
    }
    Ok((m, shown))
}

/// `GFX_Starmap`'s pin of the long info (re/screens/starmap.md §15.1): the pivot `[XStart, YStart]` for the
/// orbiting planet at `q` with the camera turned `rotate`, offset towards the screen's left.
pub fn info_pivot(q: [f32; 2], rotate: f32) -> [f32; 2] {
    let (s, c) = ((rotate as f64).sin(), (rotate as f64).cos());
    [q[0] - ((c * 0.35) / 1.3 - (s * 0.12) / 1.3) as f32, -q[1] - ((c * 0.12) / 1.3 + (s * 0.35) / 1.3) as f32]
}

/// The name tables `StringReadAll` fills from the Hungarian `txt17.txt` and the texts the long info needs.
pub struct MenuTexts<'a> {
    pub texts: &'a crate::ui::UiTexts,
    pub hu17: &'a ig2_formats::strings::UiStrings,
    pub empire_names: &'a [Vec<u8>],
    pub infostr: &'a [(i16, Vec<u8>)],
}

impl MenuTexts<'_> {
    fn hu(&self, section: u16, index: u16) -> Vec<u8> {
        crate::ui::hu_bytes(self.hu17.get(0x11, section, index))
    }
    fn tax(&self, k: i32) -> Vec<u8> {
        self.hu(5, (22 + k).clamp(0, 52) as u16)
    }
    fn autobuild(&self, k: i32) -> Vec<u8> {
        self.hu(5, (30 + k).clamp(0, 52) as u16)
    }
    fn priority(&self, k: i32) -> Vec<u8> {
        self.hu(5, (41 + k).clamp(0, 52) as u16)
    }
}

/// `KirakFader(…)` in the `starf` look (re/screens/starmap.md §12.2).
fn starf(f: &mut Frame, d: &Draw2d, r: [f32; 4]) {
    let s = 0.9;
    crate::kit::fader::kirak_fader(f, d.white, &d.v, r, [0.0, 54.0 * s, 90.0 * s], 100.0, [0.0, 90.0 * s, 130.0 * s], 70.0);
}

/// `FadeKerettelKerekit(x − 6, y − 2, w, h, 0, 10, 0, 60, 0, 5, 0, 30)`: a select menu's box.
fn menu_box(f: &mut Frame, d: &Draw2d, r: [f32; 4]) {
    fader(f, d.white, &d.v, r, [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
}

impl Menus {
    /// Draw the long info's faders (`StarmapGfx2D_1`), the select menus' boxes (`StarmapGfx2D_2`), then the texts
    /// (`StarmapHosszuInfoKiir`, `StarmapGfx2D_4`) and the radial icons (`StarmapGfx2D_3`), at camera zoom `zoom`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, d: &Draw2d, t: &MenuTexts, state: &SaveState, rules: &ig2_rules::Rules, pics: &crate::hud::Pictures, brushes: &[ig2_rules::Brush], zoom: f32, counter: i32) {
        let e = en(state);
        if let Some(i) = self.info {
            if let Some(q) = i.q(zoom) {
                starf(f, d, [319.0, 33.0, 302.0, 326.0]);
                let own = planet(state, i.planet)[0x20] == e;
                if own {
                    let (ft, et) = ((255.0 - q * 100.0).trunc(), (255.0 - q * 180.0).trunc());
                    for r in 0..3 {
                        let y = (288 + 22 * r) as f32;
                        let mut boxes = vec![[399.0, y, 102.0, 18.0], [505.0, y, 32.0, 18.0]];
                        if r != 1 {
                            boxes.extend([[541.0, y, 22.0, 18.0], [567.0, y, 22.0, 18.0]]);
                        }
                        for b in boxes {
                            fader(f, d.white, &d.v, b, [0.0; 3], ft, [0.0; 3], et);
                        }
                    }
                }
                self.draw_long_info(f, d, t, state, rules, i.planet, q, counter);
            }
            // The exit button comes in with `(Zoom − BackupZoom)/(600 − BackupZoom)` (`addo` 0: 176 wide).
            let h = ((zoom - i.backup[2]) / (600.0 - i.backup[2])).clamp(0.0, 1.0);
            if h > 0.0 {
                let x = ((1.0 - ((h as f64) * std::f64::consts::FRAC_PI_2).sin()) * 200.0 + 445.0) as f32;
                starf(f, d, [x, 380.0, 176.0, 30.0]);
                let x = ((1.0 - ((h as f64) * std::f64::consts::FRAC_PI_2).sin()) * 200.0 + 500.0 - 25.0) as f32;
                d.text(f, &t.texts.get(2, 4, 4), x.trunc(), 390.0, 13, 3, 0xd, 0x25);
            }
        }
        if let Some(m) = &self.menu {
            self.draw_menu(f, d, t, m);
        }
        if let Some(r) = &self.rop {
            if r.frame >= 30 && self.held {
                let [lx, ly] = r.last();
                if let Some(b) = brushes.get(9) {
                    for (k, &icon) in r.places.iter().enumerate() {
                        if icon > 0 {
                            let at = [(lx + ICON_AT[k][0]) as f32, (ly + ICON_AT[k][1]) as f32];
                            pics.brush(f, &d.v, b, ICON_FRAME[icon as usize % ICON_FRAME.len()], Some(at), [1.0; 4]);
                        }
                    }
                }
            }
        }
    }

    fn draw_menu(&self, f: &mut Frame, d: &Draw2d, t: &MenuTexts, m: &SelectMenu) {
        let (x, y) = (m.x as f32, m.y as f32);
        let col = |v: i32| if v == m.hovered { 6 } else { 3 };
        match m.kind {
            MenuKind::Tax | MenuKind::AutoBuild => {
                let n = m.items.len() as f32;
                let h = if m.kind == MenuKind::Tax { 108.0 } else { n * 13.0 + 4.0 };
                menu_box(f, d, [x - 6.0, y - 2.0, 130.0, h]);
                for (i, &v) in m.items.iter().enumerate() {
                    let s = if m.kind == MenuKind::Tax { t.tax(v) } else { t.autobuild(v) };
                    d.text(f, &s, x, y + 3.0 + 13.0 * i as f32, 20, 2, 4, col(v));
                }
            }
            MenuKind::Priority => {
                menu_box(f, d, [x - 6.0, y - 2.0, 130.0, 43.0]);
                for k in 1..4 {
                    d.text(f, &t.priority(k), x, y + 13.0 * k as f32 - 10.0, 20, 2, 4, col(k));
                }
            }
            MenuKind::Sale(_) => {
                // `RemoveSpaceBase_Nev[1]` ("Sell %s. Credits: $%d") with the design's name and its resale price.
                let fmt = t.hu(5, 45);
                let s = fill_format(&fmt, &m.texts[0], &m.texts[1]);
                menu_box(f, d, [x - 6.0, y - 2.0, (s.len() * 6 + 10) as f32, 17.0]);
                d.text(f, &s, x, y + 3.0, s.len(), 2, 4, col(1));
            }
            MenuKind::Planets => {
                let w = 6 * (m.a + m.b + 1);
                menu_box(f, d, [x - 6.0, y - 2.0, (w * m.cols + 3) as f32, (13 * m.rows + 4) as f32]);
                for (i, (&p, s)) in m.items.iter().zip(&m.texts).enumerate() {
                    let (c, r) = (i as i32 / m.rows, i as i32 % m.rows);
                    let cx = (m.x + w * c) as f32;
                    let (name, text) = split_planet_item(s);
                    d.text(f, name, cx, y + 3.0 + 13.0 * r as f32, m.a as usize, 2, 4, col(p));
                    if !text.is_empty() {
                        d.text(f, text, cx + 2.0 + 6.0 * m.a as f32, y + 4.0 + 13.0 * r as f32, m.b as usize, 1, 4, 1);
                    }
                }
            }
        }
    }

    /// `StarmapHosszuInfoKiir(320, 34)` (re/screens/starmap.md §15.2) at brightness `80q² + 20` %.
    #[allow(clippy::too_many_arguments)]
    fn draw_long_info(&self, f: &mut Frame, d: &Draw2d, t: &MenuTexts, state: &SaveState, rules: &ig2_rules::Rules, p: usize, q: f32, counter: i32) {
        let (x0, y0) = (320.0, 34.0);
        let bright = (q * 80.0 * q + 20.0).trunc() / 100.0;
        let txt = |f: &mut Frame, s: &[u8], x: f32, y: f32, len: usize, size: usize, align: Align, c: usize| {
            let rgb = d.palette.get(c).copied().unwrap_or([1.0; 3]);
            d.font.text(f, &d.v, s, x, y, len, size, align, [rgb[0] * bright, rgb[1] * bright, rgb[2] * bright], None);
        };
        let label = |f: &mut Frame, n: u16, y: f32| txt(f, &t.texts.get(2, 2, n), x0 + 10.0, y0 + y, 8, 3, Align::Left, 1);
        let value = |f: &mut Frame, s: &[u8], x: f32, y: f32, c: usize| txt(f, s, x0 + x, y0 + y, 0x1a, 3, Align::Left, c);
        let e = en(state);
        let r = planet(state, p);
        let owner = r[0x20];
        let debug = state.mem.u8(va::DEBUGX) != 0;
        let sats = r[0xc4 + e as usize];
        let seen = owner == e || sats != 0 || debug;
        let mut cc = 10.0;
        label(f, 1, cc);
        value(f, cstr(&r[..0x18]), 0x4e as f32, cc, 3);
        cc += 16.0;
        label(f, 2, cc);
        let mut s = t.hu(5, r[0x22] as u16);
        if seen {
            let rich = crate::starmap_hud::richness(state, rules, p, e);
            s = [&s[..], b" (", rich.to_string().as_bytes(), b"% ", &t.hu(5, 20), b")"].concat();
        }
        value(f, &s, 0x4e as f32, cc, 0x21);
        cc += 16.0;
        if owner != 0 {
            label(f, 3, cc);
            value(f, &t.empire_names.get(owner as usize).cloned().unwrap_or_default(), 0x4e as f32, cc, 0x21);
            cc += 16.0;
        }
        label(f, 4, cc);
        let race = if owner == 0 { e as usize } else { r[0x21] as usize };
        let mut s = if owner != 0 { [t.hu(1, (r[0x21] as u16).saturating_add(5)), b" ".to_vec()].concat() } else { Vec::new() };
        if seen {
            let aff = rules.race_surface_affinity.get(race).and_then(|v| v.get(r[0x22] as usize)).copied().unwrap_or(0);
            s = [&s[..], b"(", aff.to_string().as_bytes(), b"% ", &t.hu(5, 21), b")"].concat();
        }
        value(f, &s, 0x4e as f32, cc, 0x21);
        cc += 16.0;
        if owner != 0 && seen {
            label(f, 5, cc);
            value(f, format!("\x01{} ({:+})", i32_at(&r, 0x28), i32_at(&r, 0x2c)).as_bytes(), 0x4e as f32, cc, 0x21);
            cc += 16.0;
        }
        if owner != 0 && (owner == e || debug) {
            label(f, 6, cc);
            let morale = (i32_at(&r, 0x30) / 100) as f32 as i32;
            value(f, &t.hu(4, (41 + morale).clamp(0, 52) as u16), 0x4e as f32, cc, 0x21);
            cc += 16.0;
            label(f, 7, cc);
            value(f, format!("${}", i32_at(&r, 0x44)).as_bytes(), 0x4e as f32, cc, 0x21);
            cc += 16.0;
            for (n, a, b, mark) in [(8u16, 0x28usize, 0x50usize, b'\x01'), (10, 0x48, 0x4c, b'\x02')] {
                let c = if i32_at(&r, a) < i32_at(&r, b) { 6 } else { 0x21 };
                label(f, n, cc);
                value(f, &[&[mark][..], i32_at(&r, a).to_string().as_bytes()].concat(), 0x4e as f32, cc, c);
                txt(f, &t.texts.get(2, 2, n + 1), x0 + 0x8c as f32, y0 + cc, 8, 3, Align::Left, 1);
                value(f, &[&[mark][..], i32_at(&r, b).to_string().as_bytes()].concat(), 0xd0 as f32, cc, c);
                cc += 16.0;
            }
        }
        if owner != 0 && (owner == e || sats > 1 || debug) {
            label(f, 12, cc);
            txt(f, i32_at(&r, 0x58).to_string().as_bytes(), x0 + 0x4e as f32, y0 + cc, 8, 3, Align::Left, 0x21);
            cc += 16.0;
        }
        label(f, 13, cc);
        // The info texts in turn, one every 30 `Counter` frames.
        let (mut sp, mut old, mut h) = self.special.get();
        if sp != p {
            (sp, old, h) = (p, counter, 1);
        }
        while counter - old > 0x1d {
            old += 0x1e;
            let start = h;
            loop {
                h = h % 5 + 1;
                if i16_at(&r, 0x168 + 2 * h) != 0 || h == start {
                    break;
                }
            }
        }
        self.special.set((sp, old, h));
        let code = i16_at(&r, 0x168 + 2 * h);
        if code != 0 {
            if let Some((_, s)) = t.infostr.iter().find(|s| s.0 == code) {
                value(f, s, 0x4e as f32, cc, 6);
            }
        }
        cc += 16.0;
        if owner != 0 && (owner == e || debug) {
            txt(f, &t.texts.get(2, 2, 14), x0 + 10.0, y0 + cc, 9, 3, Align::Left, 1);
            for pr in crate::starmap_hud::problem_order(&r, debug) {
                let v = i32_at(&r, 0x118 + 4 * pr) / 10000;
                txt(f, v.to_string().as_bytes(), x0 + 0x56 as f32, y0 + cc + 2.0, 2, 2, Align::Right, 6);
                txt(f, b"%", x0 + 99.0, y0 + cc + 2.0, 1, 2, Align::Left, 6);
                txt(f, &t.hu(4, (21 + pr) as u16), x0 + 0x70 as f32, y0 + cc + 2.0, 0x20, 2, Align::Left, 0x21);
                cc += 12.0;
            }
        }
        if owner == e {
            let values = [t.tax(r[0x34] as i32), t.autobuild(r[0x6c] as i32), t.priority(r[0x6d] as i32)];
            for (k, v) in values.iter().enumerate() {
                let y = 0x104 as f32 + 22.0 * k as f32;
                txt(f, &t.texts.get(2, 2, 15 + k as u16), x0 + 10.0, y0 + y, 0x20, 2, Align::Left, 1);
                txt(f, v, x0 + 0x52 as f32, y0 + y, 0x10, 2, Align::Field, 0x21);
                let [mx, my] = self.pointer;
                let pressed = self.held && mx >= x0 + 0xba as f32 && mx <= x0 + 0xd8 as f32 && my >= y0 + y - 5.0 && my <= y0 + y + 11.0;
                txt(f, &t.texts.get(2, 2, 18), x0 + 0xba as f32, y0 + y, 5, 2, Align::Field, if pressed { 2 } else { 1 });
                if k != 1 {
                    txt(f, b"-", x0 + 0xe4 as f32, y0 + y - 2.0, 1, 3, Align::Left, 1);
                    txt(f, b"+", x0 + 0xfe as f32, y0 + y - 2.0, 1, 3, Align::Left, 1);
                }
            }
        }
    }
}

/// The planet list's item: the name, then the production text (`pct%-design`), split where the stored text began.
fn split_planet_item(s: &[u8]) -> (&[u8], &[u8]) {
    match s.windows(2).position(|w| w == b"%-") {
        Some(i) if i >= 2 => s.split_at(i - 2),
        _ => (s, &[]),
    }
}

/// `stringhelper::format(fmt, name, price)` for `"…%s…%d…"`.
fn fill_format(fmt: &[u8], name: &[u8], number: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i] == b'%' && i + 1 < fmt.len() && (fmt[i + 1] == b's' || fmt[i + 1] == b'd') {
            out.extend_from_slice(if fmt[i + 1] == b's' { name } else { number });
            i += 2;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_answer_within_thirty_pixels_from_frame_fifty() {
        let mut r = Rop { at: [320, 240], frame: 49, places: [2, 4, 3, 9], hover: 0 };
        // Place 1's centre: LastMouse + (29, 86) = (267, 242).
        r.hover_at([267.0, 242.0]);
        assert_eq!(r.hover, 0);
        r.frame = 50;
        r.hover_at([267.0, 242.0]);
        assert_eq!(r.hover, 2);
        r.hover_at([267.0 + 21.0, 242.0 + 21.0]);
        assert_eq!(r.hover, 2, "21² + 21² = 882 ≤ 900");
        r.hover_at([267.0 + 22.0, 242.0 + 22.0]);
        assert_eq!(r.hover, 0);
    }

    /// The long info's buttons and menus and the radial icons drive the sim on `quick-1.sav` (IG2_INSTALL_DIR and
    /// IG2_SAVE_DIR; skipped without them).
    #[test]
    fn menus_reach_the_sim() {
        use ig2_net::{Command, StarmapCmd};
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let mut w = crate::world::World::load(&save, &rules, &defaults, &volatile);
        let run = |w: &mut crate::world::World, dos: Vec<Do>| {
            for d in dos {
                if let Do::Act(Action::Send(o)) = d {
                    w.send(Command::Starmap { order: StarmapCmd::from(o) });
                }
            }
            w.pump();
        };
        let e = en(&w.game.state);
        let p = (1..=w.game.state.planet_count()).find(|&p| planet(&w.game.state, p)[0x20] == e).unwrap();
        let sel = Selection { mode: 1, planet: p, fleet: 0 };
        // The player's planet: production left, colony below, redirect right, tank info above.
        assert_eq!(rop_places(&w.game.state, &sel, &Panels::default(), false, (false, false), false), [2, 4, 3, 9]);
        let mut m = Menus::default();
        m.enter_info(p, [0.0, 0.0, 400.0]);
        // The tax menu (hot-spot 0x5a): row 3 under the pointer on the release.
        assert_eq!(m.info_spot(&w.game.state, [410.0, 295.0]), Some(0x5a));
        assert_eq!(m.press_info(&w.game.state, [410.0, 295.0]), Some(vec![]));
        let menu = m.menu.clone().unwrap();
        assert_eq!((menu.kind, menu.x, menu.y), (MenuKind::Tax, 422, 275));
        m.hover([430.0, (menu.y + 13 * 3 + 5) as f32]);
        let dos = m.release_menu(&w.game.state, &sel);
        run(&mut w, dos);
        assert_eq!(planet(&w.game.state, p)[0x34], 3);
        // Tax + (0x5d), the priority menu's second item, the auto-build menu's first.
        let dos = m.press_info(&w.game.state, [575.0, 295.0]).unwrap();
        run(&mut w, dos);
        assert_eq!(planet(&w.game.state, p)[0x34], 4);
        m.press_info(&w.game.state, [410.0, 340.0]);
        let y = m.menu.as_ref().unwrap().y;
        m.hover([430.0, (y + 13 + 5) as f32]);
        let dos = m.release_menu(&w.game.state, &sel);
        run(&mut w, dos);
        assert_eq!(planet(&w.game.state, p)[0x6d], 2);
        m.press_info(&w.game.state, [410.0, 318.0]);
        let y = m.menu.as_ref().unwrap().y;
        m.hover([430.0, (y + 13 + 5) as f32]);
        let dos = m.release_menu(&w.game.state, &sel);
        run(&mut w, dos);
        assert_eq!(planet(&w.game.state, p)[0x6c], 1);
        // The redirect icon, then a pick of the planet itself: no rally (0x400 with 0).
        m.fire(3, &w.game.state, &sel);
        assert!(m.redirect);
        let dos = m.redirect_to(&w.game.state, &sel, Some(p));
        run(&mut w, dos);
        assert!(!m.redirect);
        assert_eq!(i16_at(&planet(&w.game.state, p), 0xac), 0);
        // A satellite (icon 5) over the AI's planet.
        let q = (1..=w.game.state.planet_count()).find(|&q| ![0, e].contains(&planet(&w.game.state, q)[0x20])).unwrap();
        let other = Selection { mode: 1, planet: q, fleet: 0 };
        let dos = m.fire(5, &w.game.state, &other);
        assert!(m.sat_sent);
        run(&mut w, dos);
        assert_eq!(planet(&w.game.state, q)[0xc4 + e as usize], 1);
    }

    #[test]
    fn sale_line_fills_the_format() {
        assert_eq!(fill_format(b"Sell %s. Credits: $%d", b"Base", b"3400"), b"Sell Base. Credits: $3400".to_vec());
        assert_eq!(split_planet_item(b"Earth 45%-Destroyer"), (&b"Earth "[..], &b"45%-Destroyer"[..]));
    }
}
