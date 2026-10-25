//! The info screen (`AktKep` 9, re/screens/info.md): its UI state (`Info_Belep` ELF 0x563fc0, `FRISSIT_Info2`
//! 0x5686f0), hot-spots (`Info_MouseInit` 0x5667b0) and input (`Info_Main` 0x5647f0), and the 2D passes beside
//! the chart: the empire panel (`KirakBirodalomInfo` 0x559df0), the spending limit (`PutLimitCucc` 0x5683b0), the
//! money triangle's texts (`PutHaromszog` 0x55a8b0) and the lines (`MeghuzIzeVonalak` 0x55c7c0).

use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Font, Virtual};
use crate::render::{Blend, Frame, Instance, TexId};
use crate::ui::{UiSnap, UiTexts};

/// `sqrtf(2700)`: the money triangle's height is `√2700` (side 60): corners A (319, 50 + h), B (271.9, 50),
/// C (366.1, 50).
fn tri_h() -> f32 {
    2700f32.sqrt()
}

/// What the panels show, read from the sim after each tick.
#[derive(Debug, Clone, Default)]
pub struct InfoData {
    /// Per empire: the planets the human sees (`+0xd8 + 2·EN` > 0) and the fleets it sees (`+0x19 + EN` & 1).
    pub known_planets: [u32; 19],
    pub known_fleets: [u32; 19],
    /// The human's `TradeIncomeLastYear`, `TaxIncomeLastYear`, its planets' `+0x44` summed, `MoneyHatar[EN][0]`,
    /// `MoneyTeruletSzorzo[EN][1..=3]`.
    pub trade_last: i32,
    pub tax_last: i32,
    pub month_income: i32,
    pub limit: f64,
    pub shares: [f64; 3],
    /// The list's candidates (`FeltoltBolygoFlottaTomb`, re/screens/info.md §6.1): every planet and fleet, in index
    /// order, with what its row shows.
    pub planets: Vec<ListPlanet>,
    pub fleets: Vec<ListFleet>,
    /// `GetENRadarKifejlesztve`: the human's radar level (a foreign fleet's ships from 2, its speed from 3).
    pub radar: i32,
    /// `TutorialActive` or `TutorialPaused`: the list shows every planet and fleet of the empire.
    pub tutorial: bool,
    pub debug: bool,
    /// `MultiPlayerMode` (no info-text richness bonus in MP).
    pub multiplayer: bool,
    /// `PlayerFaj[EN]` (race 5 has two auto-build modes fewer).
    pub race: u8,
}

/// A planet's row (`KirakBolygoInfo` exe 0x4eb840).
#[derive(Debug, Clone, Default)]
pub struct ListPlanet {
    pub owner: u8,
    /// `+0xd8 + 2·EN` > 0: the human sees it.
    pub seen: bool,
    /// `+0x22`: the surface (the globe's texture `p<surface>.bmp`).
    pub surface: u8,
    pub name: Vec<u8>,
    /// The population line: the human's planet, race 7 and no `+0xcb`, a satellite (`+0xc4 + EN`) or `DEBUGX`.
    pub details: bool,
    pub pop: i32,
    pub growth: i32,
    /// `+0x30`.
    pub morale: i32,
    /// `+0x118 + 4i`, i = 0..=19.
    pub problems: [i32; 20],
    /// The production line when `(Counter / 30) % 2` is odd / even (`StarmapRovidInfoKiir`'s choice).
    pub queue: [Option<Vec<u8>>; 2],
    /// The whole record (`Bolygok[p]`, 0x188 bytes): the detail mode's fields (re/screens/info.md §6.6).
    pub rec: Vec<u8>,
}

/// A fleet's row (`KirakFlottaInfo` exe 0x4ebfc0).
#[derive(Debug, Clone, Default)]
pub struct ListFleet {
    pub owner: u8,
    /// `+0x52`.
    pub flags: u16,
    /// `+0x19 + EN` ≠ 0.
    pub seen: bool,
    pub name: Vec<u8>,
    /// `+0x40` the order (0 idle, 1 move, 2 attack), `+0x42` the target kind (0 a point, 1 a planet, 2 a fleet)
    /// and the target's name.
    pub order: i16,
    pub kind: i16,
    pub target: Vec<u8>,
    /// `+0x54` ships with fighters, `+0x58` fighters, `+0x5c` vehicles, `+0x78` capacity, `+0x38` speed.
    pub ships: i32,
    pub fighters: i32,
    pub ground: i32,
    pub capacity: i32,
    pub speed: f32,
    /// `CalcSpacePower(f)` and the detail mode's design list (`InfoNagyKiirFlotta`: the ships' designs in order of
    /// first appearance with their counts, then the vehicles' with their summed counts; names from the owner's
    /// `Tervek`).
    pub power: i32,
    pub designs: Vec<(Vec<u8>, i32)>,
}

/// One row of `BolygoFlottaTomb`: kind 0 a planet, 1 a fleet, and the index into [`InfoData::planets`] or
/// [`InfoData::fleets`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Planet(usize),
    Fleet(usize),
}

fn cstr(b: &[u8]) -> Vec<u8> {
    b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()
}

/// `InfoNagyKiirFlotta`'s design list of fleet `f` (exe 0x4e6ea0): the ships' designs (`Hajok`, design ≠ 0x7fff)
/// in order of first appearance with their counts, then the vehicles' (`Tankok`, deduplicated among the vehicles
/// only) with their summed counts, named from `Tervek[owner]`.
fn fleet_designs(state: &SaveState, w: &ig2_sim::info::InfoWorld, f: usize, owner: u8) -> Vec<(Vec<u8>, i32)> {
    let mut ids: Vec<(i16, i32)> = Vec::new();
    for s in w.ships.iter().skip(1).filter(|s| s.fleet as i32 == f as i32 && s.design != 0x7fff) {
        match ids.iter_mut().find(|x| x.0 == s.design) {
            Some(x) => x.1 += 1,
            None => ids.push((s.design, 1)),
        }
    }
    let ships = ids.len();
    for v in w.vehicles.iter().skip(1).filter(|v| v.fleet as i32 == f as i32 && v.design != 0x7fff) {
        match ids[ships..].iter_mut().find(|x| x.0 == v.design) {
            Some(x) => x.1 = x.1.wrapping_add(v.count),
            None => ids.push((v.design, v.count)),
        }
    }
    let designs = state.designs.get(owner as usize);
    ids.into_iter().map(|(d, n)| (designs.and_then(|t| t.get(d as usize)).map(|r| cstr(&r[..0x14])).unwrap_or_default(), n)).collect()
}

impl InfoData {
    pub fn read(state: &SaveState) -> Self {
        use ig2_sim::save::{PLANET_SIZE, TECH_SIZE};
        let m = &state.mem;
        let en = m.i32(va::EN).clamp(0, 18) as usize;
        let mut d = Self { debug: m.u8(va::DEBUGX) != 0, multiplayer: m.u8(va::MULTIPLAYER) != 0, tutorial: m.u8(va::TUTORIAL_ACTIVE) != 0 || m.u8(va::TUTORIAL_PAUSED) != 0, ..Self::default() };
        for p in 1..=state.planet_count() {
            let base = va::PLANETS + (p * PLANET_SIZE) as u32;
            let owner = m.u8(base + 0x20) as usize;
            if owner < 19 && m.i16(base + 0xd8 + 2 * en as u32) > 0 {
                d.known_planets[owner] += 1;
            }
            if owner == en {
                d.month_income = d.month_income.wrapping_add(m.i32(base + 0x44));
            }
            let r = m.read(base, PLANET_SIZE);
            let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            let owner = owner as u8;
            let details = owner as usize == en || (en == 7 && r[0xcb] == 0) || r[0xc4 + en] != 0 || d.debug;
            let mine = owner as usize == en || (d.debug && owner != 0);
            let queue = |c: i32| if mine { crate::starmap_hud::production_line(state, &r, owner, c) } else { None };
            d.planets.push(ListPlanet {
                owner,
                seen: m.i16(base + 0xd8 + 2 * en as u32) > 0,
                surface: r[0x22],
                name: cstr(&r[..0x18]),
                details,
                pop: i32_at(0x28),
                growth: i32_at(0x2c),
                morale: i32_at(0x30),
                problems: std::array::from_fn(|i| i32_at(0x118 + 4 * i)),
                queue: [queue(30), queue(0)],
                rec: r.to_vec(),
            });
        }
        let name_of = |f: &[u8]| cstr(&f[..0x14]);
        let nf = m.i32(va::FLEET_COUNT).max(0) as usize;
        let c = state.combat();
        let w = ig2_sim::info::InfoWorld { fleet_owners: state.fleets.iter().map(|r| r[0x15]).collect(), ships: c.ships, vehicles: c.vehicles, designs: c.designs, ..Default::default() };
        for (fi, f) in state.fleets.iter().enumerate().skip(1).take(nf) {
            let owner = f[0x15] as usize;
            if owner < 19 && f[0x19 + en] & 1 != 0 {
                d.known_fleets[owner] += 1;
            }
            let i16_at = |o: usize| i16::from_le_bytes([f[o], f[o + 1]]);
            let i32_at = |o: usize| i32::from_le_bytes(f[o..o + 4].try_into().unwrap());
            let (order, kind) = (i16_at(0x40), i16_at(0x42));
            let tx = f32::from_le_bytes(f[0x44..0x48].try_into().unwrap()) as i32;
            let target = match kind {
                1 if tx >= 0 => cstr(&m.read(va::PLANETS + (tx as usize * PLANET_SIZE) as u32, 0x18)),
                2 => state.fleets.get(tx.max(0) as usize).map(|g| name_of(g)).unwrap_or_default(),
                _ => Vec::new(),
            };
            d.fleets.push(ListFleet {
                owner: f[0x15],
                flags: u16::from_le_bytes([f[0x52], f[0x53]]),
                seen: f[0x19 + en] != 0,
                name: name_of(f),
                order,
                kind,
                target,
                ships: i32_at(0x54),
                fighters: i32_at(0x58),
                ground: i32_at(0x5c),
                capacity: i32_at(0x78),
                speed: f32::from_le_bytes(f[0x38..0x3c].try_into().unwrap()),
                power: w.space_power(fi),
                designs: fleet_designs(state, &w, fi, f[0x15]),
            });
        }
        // `GetENRadarKifejlesztve` (re/rules/fleet-movement.md §6.2): the best developed radar tech 0x21..=0x24, one
        // more for empire 2, one more with the story's bonus (`StoryAdat+0x38`) outside MP.
        let status = |k: usize| m.u8(va::TECHS + (k * TECH_SIZE) as u32 + 0x45 + en as u32);
        let tech = (1..=4).rev().find(|&c| status(0x20 + c) == 5).unwrap_or(0) as i32;
        d.radar = if tech > 0 { tech + (en == 2) as i32 } else { 0 };
        if m.u8(va::MULTIPLAYER) == 0 && m.i32(va::STORY_ADAT + 0x38) != 0 {
            d.radar += 1;
        }
        d.trade_last = m.i32(va::TRADE_LAST + 4 * en as u32);
        d.tax_last = m.i32(va::TAX_LAST + 4 * en as u32);
        let t = state.treasuries()[en];
        d.limit = t.reserve[0];
        d.shares = [t.share[1], t.share[2], t.share[3]];
        d.race = m.u8(va::PLAYER_FAJ + en as u32);
        d
    }

    /// `FeltoltBolygoFlottaTomb` (exe 0x4e5a20, every frame in `GFX_Info`): the planets of `selected`, then its
    /// fleets without flag 0x20; another empire's only where the human sees them (or during the tutorial).
    pub fn list(&self, selected: usize, human: usize) -> Vec<Entry> {
        let all = selected == human || self.tutorial;
        let planets = self.planets.iter().enumerate().filter(|(_, p)| p.owner as usize == selected && (all || p.seen)).map(|(i, _)| Entry::Planet(i));
        let fleets = self.fleets.iter().enumerate().filter(|(_, f)| f.owner as usize == selected && f.flags & 0x20 == 0 && (all || f.seen)).map(|(i, _)| Entry::Fleet(i));
        planets.chain(fleets).collect()
    }

    /// `InfoProblemaRendez(p)` (exe 0x4e75c0) on `rendezo` (`InfoProblemaRendezo[0..=4]`, shared by every call):
    /// the problems at 100000 or more, the four worst first into slots 1..; slots past the count keep the earlier
    /// planet's entries. Returns the count (`InfoProblemaHanyKiirt`).
    fn sort_problems(p: &ListPlanet, debug: bool, rendezo: &mut [usize; 5]) -> usize {
        let v = |i: usize| p.problems[i];
        let mut n = 0;
        rendezo[0] = 0;
        for i in 1..0x14 {
            if v(i) > 99999 || debug {
                let mut at = n;
                while at > 0 && v(rendezo[at]) < v(i) {
                    at -= 1;
                }
                if at + 1 < 5 {
                    n = (n + 1).min(4);
                    for s in ((at + 2)..=n).rev() {
                        rendezo[s] = rendezo[s - 1];
                    }
                    rendezo[at + 1] = i;
                }
            }
        }
        n
    }
}

/// The screen's UI state.
#[derive(Debug, Clone)]
pub struct InfoScreen {
    /// The sounds the screen started since the window last took them (re/screens/audio.md §6.3).
    pub sounds: Vec<crate::audio::SoundId>,
    /// `InfoSelected`: the empire the panels show (the human on entry).
    pub selected: usize,
    /// `Info_SelectedChart` 0..=9.
    pub chart: usize,
    /// `Info_StarLogMode` (else `Info_ChartMode`).
    pub star_log: bool,
    /// `infoframe` (45 open; the star log opens it on to 75 at 0.3 a frame).
    pub frame: f32,
    /// `InfoVillogtato` (+0.04 a frame).
    pub blink: f32,
    /// `infokeringeto` (+0.0006 a frame) and `InfoChartSzamlaloAdderek` (`rand() % 400 + 200` on entry).
    pub turn: f32,
    pub adders: [f32; 10],
    /// The heads' fader clock `InfoFejF+0x4d8` (`InitFaderDesignStruktura` on entry, +0.003 a frame in
    /// `FaderDesignFrissito`): the selected head's pulsing light.
    pub head_clock: f32,
    /// `infof`: `PutFaders`' two `KirakFader` boxes' shine and drift (their colours aren't × `s`; no glow is drawn).
    pub faders: crate::kit::FaderDesign,
    /// The heads that carry `RarakFenyekFejekre`'s second light (slot 1): every head that was not selected in a
    /// frame since entry (the slot is set, never removed while the screen is open).
    pub head_lit: [bool; 9],
    /// `InfoTriX`, `InfoTriY` (`Info_InitializeTriangleToMidPosition` ELF 0x55e5d0: (319, 50 + 2h/5.6)).
    pub tri: [f32; 2],
    /// `BolygoFlottaPointer` (exe 0xB82984): the list's first row shown.
    pub pointer: usize,
    /// `Counter` (exe 0xBAE05C, +1 per 50 ms tick, also while paused) and `KirakBolygoInfo`'s statics: the problem
    /// step `h` (0xB85E88) and the `Counter` it last stepped at (0xB85E84).
    pub counter: i32,
    pub problem_step: usize,
    pub problem_counter: i32,
    /// Per row the `InfoProblemaRendezo` slot its problem line shows (0 = the morale) and whether it carries the red
    /// light (`VanNagyProblema`), as the last frame left them.
    pub problem_rows: [(usize, usize, bool); 4],
    /// `InfoBF` (exe 0xB85B90, 0xB0 a slot): per row slot the planet's and the fleet's rotation (`GetObjMatrix`, the
    /// matrix after the scale) and the spin `+0x78`.
    pub globes: [Globe; 4],
    /// The detail mode (`Info_NagyInfoMode` exe 0xB79FB1, re/screens/info.md §6.6) and its way back
    /// (`Info_KilepesNagyinforol` 0xB85258).
    pub detail: bool,
    pub leaving: bool,
    /// `InfoBolygoFlottaSelected` (the list entry shown), `InfoNKivalasztott` (its row slot) and the entry it held
    /// (`Info_NagyInfoKivalasztott`/`…Tip`).
    pub sel_entry: usize,
    pub sel_slot: usize,
    pub sel_pick: Option<Entry>,
    /// `InfoMaxTervekPtr`, `InfoMaxTervek`: the fleet's design list.
    pub designs_ptr: usize,
    pub designs_n: usize,
    /// `InfoNagyKiirBolygo`'s statics: the info text slot `hanyadik` (0x90B8A0), the `Counter` it last stepped at
    /// (0xB85E94) and its planet (0xB85E98).
    pub texts: (usize, i32, usize),
    /// The star log (§6.8): `StarLog`'s 200 records (colour, text; the newest last), `StarLogPointer` and
    /// `StarlogHanysorfeltoltve`. Unsaved: a load starts it empty here.
    pub log: Vec<(i32, Vec<u8>)>,
    pub log_pointer: usize,
    pub log_filled: usize,
    /// The planet-settings select menus (re/screens/info.md §7.1): the one open (`InfoSelectTax`/`…AutoBuild`/
    /// `…Priority` exe 0xB79F8A/0xB79F8B/0xB79FB0, by its hot-spot 26..=28), where (`infoSelectX/Y` 0x90B6FC/0x90B700)
    /// and the item under the pointer (`infosmSelected` 0xB84C14, −1 none; a global the menus share).
    pub select: Option<u32>,
    pub select_at: [i32; 2],
    pub select_item: i32,
    /// `Mouse_Megfogva` (exe 0xDB466A): the hot-spot a held button keeps (`AktIkon` isn't re-picked).
    pub held: Option<u32>,
    /// `InfoHaromszogMegfogva` (exe 0xB85259) and `InfoHaromszogNovelo` (+1 a UI frame up to 100; the shares blink
    /// below it, 0 after a drag).
    pub tri_held: bool,
    pub tri_novelo: f32,
    /// `infoFelfadeles` (exe 0xB85264): 0 at an animated `Info_Belep`, +1 a UI frame once open up to 100
    /// (`FRISSIT_Info2` exe 0x4e5570); 100 at once without the opening, which the client doesn't have yet.
    pub fade_in: f32,
    /// The last shares sent (the exe sends them every frame of a drag; the port only when they change).
    tri_sent: Option<[f64; 3]>,
    /// UI frames since entry (`Counter` steps every fifth).
    frames: u32,
    /// The buttons' press animation (`LekezelGombok`).
    pub buttons: Buttons3d,
}

/// `LekezelGombok` (ELF 0x559850, from `Infoing3DUpdate`): one button `GombMeshek[which]` at a time sinks in by
/// `InfoGombFrame` 0, 7, 10 and comes out 3, 0. `InfoGombNew` (set by a press, or every frame a button is held on the
/// list or star-log arrows) restarts the sink, so a held arrow stays in at 10. The frame is shared: a button left
/// while another starts keeps the depth it had (`infoSetGombFrame` moves only `which`'s mesh).
#[derive(Debug, Clone)]
pub struct Buttons3d {
    /// `InfoGombNew`, `InfoMelyikGombot` (−1 none, `Infoing3DInit`), `InfoGombFrame`, `InfoGombFrameCsokkent`.
    pub new: bool,
    pub which: i32,
    pub frame: i32,
    pub falling: bool,
    /// The frame each `GombMeshek` button was last put at.
    pub depth: [i32; 19],
}

impl Default for Buttons3d {
    fn default() -> Self {
        Self { new: false, which: -1, frame: 0, falling: false, depth: [0; 19] }
    }
}

impl Buttons3d {
    /// `InfoGombNew = 1; InfoMelyikGombot = which`.
    pub fn start(&mut self, which: i32) {
        (self.new, self.which) = (true, which);
    }

    /// One frame of `LekezelGombok` (the screen is open: the client has no opening or closing animation).
    pub fn step(&mut self) {
        if self.new {
            (self.new, self.falling) = (false, false);
        }
        if self.which < 0 {
            return;
        }
        if !self.falling {
            self.frame = (self.frame + 7).min(10);
            self.falling = self.frame == 10;
        } else if self.frame != 0 {
            self.frame = (self.frame - 7).max(0);
        } else {
            return;
        }
        self.depth[self.which as usize] = self.frame;
    }
}

/// The pointer's state in a frame of `Info_Main`: the press and release edges (`Mouse_Lenyom`, `Mouse_Felenged`),
/// the button held (`Mouse_Gomb`) and which (`Mouse_Bal`, `Mouse_Jobb`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Buttons {
    pub press: bool,
    pub release: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

/// The money triangle (`Info_Main` exe 0x4e81c0, hot-spot 29, re/screens/info.md §7.2) in the exe's f32: `h = √2700`,
/// A = (319, top), B = (271.9, base), C = (366.1, base) with `top = h + 50`, `base = top − h`. The signed distances
/// from the edges (`PontEgyenesTavolsaga`, inlined): positive inside for AB and BC, negative inside for AC.
struct Tri {
    h: f32,
    top: f32,
    base: f32,
}

/// `319 − 271.9` and `366.1 − 271.9` in f32 (the exe's constants 47.100006 and 94.20001) and their squares
/// (2218.4106, 8873.643).
const AB_DX: f32 = 47.100006;
const AB_DX2: f32 = 2218.4106;
const BC_DX: f32 = 94.20001;
const BC_DX2: f32 = 8873.643;

impl Tri {
    fn new() -> Self {
        let h = tri_h();
        let top = h + 50.0;
        Self { h, top, base: top - h }
    }
    /// `(f32)sqrt((f64)(a·a + d²))` for the edge's `a`.
    fn len(a: f32, d2: f32) -> f32 {
        ((a * a + d2) as f64).sqrt() as f32
    }
    fn ab(&self, x: f32, y: i32) -> f32 {
        let a = -(self.top - self.base);
        ((self.top - y as f32) * AB_DX + (319.0 - x) * a) / Self::len(a, AB_DX2)
    }
    fn ac(&self, x: f32, y: i32) -> f32 {
        let a = -(self.top - self.base);
        ((319.0 - x) * a - (self.top - y as f32) * AB_DX) / Self::len(a, AB_DX2)
    }
    fn bc(&self, x: f32, y: i32) -> f32 {
        // `−(base − base)`: −0.
        let a = -0.0f32;
        ((271.9 - x) * a - (self.base - y as f32) * BC_DX) / Self::len(a, BC_DX2)
    }
    /// The three shares 0x5b sends for the knob at (x, y): BC/h, −AC/h/1.25, AB/h/1.25 in f64, each below 0.03 → 0.
    fn shares(&self, x: i32, y: i32) -> [f64; 3] {
        let (h, x) = (self.h as f64, x as f32);
        let cut = |v: f64| if v < 0.03 { 0.0 } else { v };
        [cut(self.bc(x, y) as f64 / h), cut(-self.ac(x, y) as f64 / h / 1.25), cut(self.ab(x, y) as f64 / h / 1.25)]
    }
}

/// `Info_Main`'s hot-spot 29 for the pointer at `m` with the knob at `old`: where the knob goes and the shares to
/// send, or `None` when the move is refused (a press or a right click outside the triangle: the knob stays, the
/// grab ends). A drag outside is pulled back along x in 0.1 steps; a right click inside resets it to the middle.
fn tri_drag(old: [i32; 2], m: [i32; 2], press: bool, right: bool) -> Option<([i32; 2], [f64; 3])> {
    let t = Tri::new();
    let (mut x, mut y) = (m[0], m[1]);
    if t.top < y as f32 {
        y = t.top as i32;
    }
    if y as f32 <= t.base {
        y = (t.base + 1.0) as i32;
    }
    let mut refused = false;
    if t.ab(x as f32, y) < 0.0 {
        if !press && !right {
            let mut xf = x as f32;
            while t.ab(xf, y) < 0.0 {
                xf += 0.1;
            }
            x = xf as i32;
        } else {
            ([x, y], refused) = (old, true);
        }
    }
    if t.ac(x as f32, y) > 0.0 {
        if !press && !right {
            let mut xf = x as f32;
            while t.ac(xf, y) > 0.0 {
                xf -= 0.1;
            }
            x = xf as i32;
        } else {
            return None;
        }
    }
    if refused {
        return None;
    }
    if right {
        (x, y) = (319, ((t.h * 2.0) / 5.6 + t.base) as i32);
    }
    Some(([x, y], t.shares(x, y)))
}

/// The spending limit for the pointer at virtual x `mx` over hot-spot 32: `v = clamp((mx − 280)·100/100, 0, 100)`,
/// `(exp(v·2000·3/200000) − 1)·10500` truncated to hundreds, above 15,000 to thousands.
pub fn limit_at(mx: i32) -> f64 {
    let v = ((((mx - 0x118) * 100) / 100) as f64).clamp(0.0, 100.0);
    let mut l = (((((v * 2000.0 * 3.0) / 200000.0).exp() - 1.0) * 10500.0) as i32 / 100) * 100;
    if l > 15000 {
        l = (l / 1000) * 1000;
    }
    l as f64
}

/// A select menu's items: tax 0..=7, auto-build 0..=8 (0..=6 for race 5), priority 1..=3 (`Info_Main`).
fn select_items(spot: u32, race: u8) -> std::ops::RangeInclusive<i32> {
    match spot {
        26 => 0..=7,
        27 => 0..=if race == 5 { 6 } else { 8 },
        _ => 1..=3,
    }
}

/// The selected globe's place in the detail mode (`Info_Main`: 0xc22c0000, 0x41e00000, 0xc3160000).
pub const DETAIL_AT: [f32; 3] = [-43.0, 28.0, -150.0];

/// A list slot's models (`LoadBolygoUrhajok`, `InfoBF` exe 0xB85B90 + 0xB0·k).
#[derive(Debug, Clone, Copy)]
pub struct Globe {
    pub planet: crate::scene::M3,
    pub fleet: crate::scene::M3,
    pub spin: f32,
    /// `+0x7c` position, `+0x88` home (−310, 59k + 40, −650), `+0x94` where the detail mode sent it, `+0xa0` target,
    /// `+0xac` step per UI frame.
    pub pos: [f32; 3],
    pub home: [f32; 3],
    pub start: [f32; 3],
    pub target: [f32; 3],
    pub step: [f32; 3],
}

impl Globe {
    fn home(k: usize) -> [f32; 3] {
        [-310.0, (59 * k + 40) as f32, -650.0]
    }

    /// Head for `target` in 40 UI frames (`(target − position)/40` per axis).
    fn send(&mut self, target: [f32; 3]) {
        self.target = target;
        self.step = std::array::from_fn(|i| (self.target[i] - self.pos[i]) / 40.0);
    }

    /// `RotateBolygoFlottak`'s fade of the detail mode's globe: `(x − home)/(target − home)` going out, `1 − (start −
    /// x)/(start − home)` coming back (0 when the target lies left of home).
    fn fade(&self) -> f32 {
        let (x, home, target, start) = (self.pos[0], self.home[0], self.target[0], self.start[0]);
        if target <= home {
            if target == home { 1.0 - (start - x) / (start - home) } else { 0.0 }
        } else {
            (x - home) / (target - home)
        }
    }
}

/// One axis of `FRISSIT_Info2`'s motion: towards `target` by `step`, stopping there; true when a position above its
/// target arrives (on the x axis that ends the detail mode).
fn approach(pos: &mut f32, target: f32, step: f32) -> bool {
    if target <= *pos {
        if *pos != target {
            *pos += step;
            if *pos <= target {
                *pos = target;
                return true;
            }
        }
    } else {
        *pos += step;
        if target <= *pos {
            *pos = target;
        }
    }
    false
}

impl InfoScreen {
    /// `Info_Belep`: the human's empire, chart 0, chart mode. The ten sweep speeds are UI-only libc draws: a fixed
    /// MSVC `rand()` sequence here [inf: the seed is the game's].
    pub fn enter(human: usize) -> Self {
        let mut seed = 1u32;
        let mut rand = || {
            seed = seed.wrapping_mul(214013).wrapping_add(2531011);
            ((seed >> 16) & 0x7fff) as i32
        };
        let adders = std::array::from_fn(|_| (rand() % 400 + 200) as f32);
        // `LoadBolygoUrhajok`: `RotateObj(π/2, 0, 0)`, the spin `((rand() % 10) + 1)/100 + 0.01` [inf: drawn after
        // the sweeps here; the exe draws them first, in `Infoing3DInit`].
        let rot = crate::scene::create_matrix(std::f32::consts::FRAC_PI_2, 0.0, 0.0);
        let globes = std::array::from_fn(|k| {
            let home = Globe::home(k);
            Globe { planet: rot, fleet: rot, spin: ((rand() % 10) as f32 + 1.0) / 100.0 + 0.01, pos: home, home, start: home, target: home, step: [0.0; 3] }
        });
        Self {
            sounds: Vec::new(),
            selected: human,
            chart: 0,
            star_log: false,
            frame: crate::screen_frames::INFO_MAX_FRAMES,
            blink: 0.0,
            turn: 0.0,
            adders,
            head_clock: 0.0,
            faders: crate::kit::FaderDesign::new(1),
            head_lit: std::array::from_fn(|e| e != human),
            tri: [319.0, ((50.0 + tri_h()) - tri_h() + tri_h() * 2.0 / 5.6).trunc()],
            pointer: 0,
            counter: 0,
            problem_step: 0,
            problem_counter: 0,
            problem_rows: [(0, 0, false); 4],
            globes,
            detail: false,
            leaving: false,
            sel_entry: 0,
            sel_slot: 0,
            sel_pick: None,
            designs_ptr: 0,
            designs_n: 0,
            texts: (1, 0, 0),
            log: vec![(0, Vec::new()); 200],
            log_pointer: 0,
            log_filled: 0,
            select: None,
            select_at: [0; 2],
            select_item: -1,
            held: None,
            tri_held: false,
            tri_novelo: 100.0,
            fade_in: 100.0,
            tri_sent: None,
            frames: 0,
            buttons: Buttons3d::default(),
        }
    }

    /// The entry the detail mode shows (`BolygoFlottaTomb[InfoBolygoFlottaSelected]`).
    pub fn detail_entry(&self, d: &InfoData, human: usize) -> Option<Entry> {
        self.detail.then(|| d.list(self.selected, human).get(self.sel_entry).copied()).flatten()
    }

    /// `AtteszMatrixokat(down)` (ELF 0x558f10, re/screens/info.md §6.7): the slots' matrices and spins move with
    /// their entries (down: slot k takes slot k + 1's, slot 3 slot 0's; up the other way).
    pub fn swap_globes(&mut self, down: bool) {
        let take = |g: &Globe| (g.planet, g.fleet, g.spin);
        let all: Vec<_> = self.globes.iter().map(take).collect();
        for k in 0..4 {
            let from = if down { (k + 1) % 4 } else { (k + 3) % 4 };
            (self.globes[k].planet, self.globes[k].fleet, self.globes[k].spin) = all[from];
        }
    }

    /// The rows shown: up to four entries of the list from `pointer`.
    pub fn rows(&self, d: &InfoData, human: usize) -> Vec<Entry> {
        d.list(self.selected, human).into_iter().skip(self.pointer).take(4).collect()
    }

    /// `CheckMelyikFolottVan` (exe 0x4e5320): a shown row's globe (`GetObjPos`, projected by `TransformPoint`: the
    /// screen's camera, `vx = 600·x/w + 320`, `vy = 600·y/w + 240`) strictly inside a ±30 pixel box around `at`.
    /// Only a planet row (kind 0, its point scaled `·639/639`, `·479/479` in f32) answers yes; a fleet row's hit
    /// (kind 1) ends the search with no (0x4e54f6 jumps to the `xor al,al` exit: an original bug).
    fn over_globe(&self, d: &InfoData, human: usize, at: [f32; 2]) -> bool {
        let inside = |x: f32, y: f32| x - 30.0 < at[0] && at[0] < x + 30.0 && y - 30.0 < at[1] && at[1] < y + 30.0;
        for (k, e) in self.rows(d, human).iter().enumerate() {
            let p = self.globes[k].pos;
            let w = -p[2];
            let (x, y) = (600.0 * p[0] / w + 320.0, 600.0 * p[1] / w + 240.0);
            match e {
                Entry::Planet(_) if inside(x * 639.0 / 639.0, y * 479.0 / 479.0) => return true,
                Entry::Fleet(_) if inside(x, y) => return false,
                _ => {}
            }
        }
        false
    }

    /// One frame of `KirakBolygoFlottakat`'s problem bookkeeping (re/screens/info.md §6.2): per planet row
    /// `VanNagyProblema` then `KirakBolygoInfo`'s step `h` (every 30 `Counter`, wrapping after the planet's count;
    /// shared by the rows, so a planet with fewer problems resets it for the rows below). `InfoProblemaRendezo`'s
    /// stale slots start as the last row's planet left them (the frame before).
    pub fn settle(&mut self, d: &InfoData, human: usize) {
        // The detail mode draws `InfoNagyKiirBolygo`/`…Flotta` instead (re/screens/info.md §6.6): no red lights
        // change; the planet's info text steps every 30 `Counter`, the fleet's design count is kept.
        if self.detail {
            match self.detail_entry(d, human) {
                Some(Entry::Planet(i)) => {
                    let r = &d.planets[i].rec;
                    let code = |h: usize| r.get(0x168 + 2 * h..0x16a + 2 * h).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
                    let (mut h, mut old, mut planet) = self.texts;
                    if i + 1 != planet {
                        (h, old, planet) = (1, self.counter, i + 1);
                    }
                    while self.counter - old > 0x1d {
                        old += 0x1e;
                        let start = h;
                        loop {
                            h = h % 5 + 1;
                            if code(h) != 0 || h == start {
                                break;
                            }
                        }
                    }
                    self.texts = (h, old, planet);
                }
                Some(Entry::Fleet(i)) if d.fleets[i].owner as usize == human => self.designs_n = d.fleets[i].designs.len(),
                _ => {}
            }
            return;
        }
        let rows = self.rows(d, human);
        let planet = |e: &Entry| match *e {
            Entry::Planet(i) => d.planets.get(i),
            Entry::Fleet(_) => None,
        };
        let mut rendezo = [0usize; 5];
        if let Some(p) = rows.iter().rev().find_map(planet) {
            InfoData::sort_problems(p, d.debug, &mut rendezo);
        }
        self.problem_rows = [(0, 0, false); 4];
        for (k, e) in rows.iter().enumerate() {
            let Some(p) = planet(e) else { continue };
            InfoData::sort_problems(p, d.debug, &mut rendezo);
            let big = p.problems[rendezo[1]] / 10000 > 0x32 || p.morale / 100 < 2;
            let n = InfoData::sort_problems(p, d.debug, &mut rendezo);
            if p.owner as usize == human || (d.debug && p.owner != 0) {
                while self.counter - self.problem_counter > 0x1d {
                    self.problem_counter += 0x1e;
                    self.problem_step += 1;
                    if self.problem_step > n {
                        self.problem_step = 0;
                    }
                }
                if self.problem_step > n {
                    self.problem_step = 0;
                }
            }
            self.problem_rows[k] = (self.problem_step, rendezo[self.problem_step], big);
        }
    }

    /// One 10 ms UI frame (`FRISSIT_Info2`): the blink, the sweeps, the star log's opening; with a button `down` over
    /// hot-spot `spot` at virtual `y`, `Info_Main`'s per-frame spots before it, and `LekezelGombok` after it (the exe
    /// runs both once a drawn frame; once a UI tick here, ADR 0016 §1).
    pub fn ui_frame(&mut self, d: &InfoData, human: usize, spot: Option<u32>, y: f32, down: bool) {
        if down {
            self.held_frame(spot, y, d, human);
        }
        self.frames += 1;
        if self.frames % 5 == 0 {
            self.counter += 1;
        }
        // `FRISSIT_Info2`: the detail mode ends at once when its entry changed under it (the slot snaps home).
        if self.detail && self.detail_entry(d, human) != self.sel_pick {
            (self.detail, self.leaving) = (false, false);
            let g = &mut self.globes[self.sel_slot];
            (g.target, g.pos) = (g.home, g.home);
        }
        // `RotateBolygoFlottak2`: `matrixforgat(m, m, spin/8, 0)` for each shown row's model.
        for (k, e) in self.rows(d, human).into_iter().enumerate() {
            let g = &mut self.globes[k];
            let turn = crate::scene::axis_matrix([0.0, -1.0, 0.0], g.spin / 8.0);
            match e {
                Entry::Planet(_) => g.planet = crate::scene::mul(&turn, &g.planet),
                Entry::Fleet(_) => g.fleet = crate::scene::mul(&turn, &g.fleet),
            }
        }
        // Every slot moves towards its target; the x axis arriving from above (the globe home again) ends the
        // detail mode.
        for g in &mut self.globes {
            for i in 0..3 {
                if approach(&mut g.pos[i], g.target[i], g.step[i]) && i == 0 {
                    (self.detail, self.leaving) = (false, false);
                }
            }
        }
        self.settle(d, human);
        if self.tri_novelo < 100.0 {
            self.tri_novelo = (self.tri_novelo + 1.0).min(100.0);
        }
        if self.fade_in < 100.0 {
            self.fade_in = (self.fade_in + 1.0).min(100.0);
        }
        self.blink += 0.04;
        self.turn += 0.0006;
        self.head_clock += 0.003;
        self.faders.tick();
        for (e, l) in self.head_lit.iter_mut().enumerate() {
            *l |= e != self.selected;
        }
        let max = crate::screen_frames::INFO_MAX_FRAMES;
        if self.star_log && self.frame < max + 30.0 {
            self.frame = (self.frame + 0.3).min(max + 30.0);
        }
        if !self.star_log && max < self.frame {
            self.frame = (self.frame - 0.3).max(max);
        }
        self.buttons.step();
    }

    /// `InfoChartSzamlalok[k]` = `((int)(infokeringeto·adder[k]) % 55 << 1) − 27`.
    pub fn sweep(&self, k: usize) -> f32 {
        ((((self.turn * self.adders[k]) as i32) % 0x37) << 1) as f32 - 27.0
    }

    /// The frame model's view of this state.
    pub fn frame_view(&self, s: &UiSnap) -> crate::screen_frames::InfoFrame {
        crate::screen_frames::InfoFrame {
            known: std::array::from_fn(|k| s.known[k + 1]),
            races: std::array::from_fn(|k| s.races[k + 1]),
            chart: (!self.star_log).then_some(self.chart),
            blink: self.blink,
            tri: self.tri,
            frame: self.frame,
            turn: self.turn,
            selected: self.selected,
            alive: std::array::from_fn(|k| s.alive[k + 1]),
            head_clock: self.head_clock,
            head_lit: std::array::from_fn(|k| self.head_lit[k + 1] || k + 1 != self.selected),
            globes: self.globe_views(&s.info, s.human),
            star_log: self.star_log,
            buttons: self.buttons.depth,
        }
    }

    /// `RotateBolygoFlottak`: the shown rows' models while open (not closing, no detail mode); a planet with a big
    /// problem carries the red light `((int)((sin(InfoVillogtato) + 1)·150·0.5 + 14), 14, 14)`.
    fn globe_views(&self, d: &InfoData, human: usize) -> [Option<crate::screen_frames::GlobeView>; 4] {
        let mut out = [None; 4];
        if self.frame < crate::screen_frames::INFO_MAX_FRAMES {
            return out;
        }
        let red = (((self.blink as f64).sin() as f32 + 1.0) * 150.0 * 0.5 + 14.0) as i32 as u32 & 0xff;
        let fade = self.globes[self.sel_slot].fade();
        for (k, e) in self.rows(d, human).into_iter().enumerate() {
            let g = &self.globes[k];
            // The detail mode: the selected globe `fade − 0.3` transparent (at least 0.01), the others `fade` and
            // hidden from 0.99.
            let mut t = 0.0;
            if self.detail && k == self.sel_slot {
                t = (fade - 0.3).max(0.01);
            } else if self.detail {
                t = fade;
                if k + self.pointer != self.sel_entry && t >= 0.99 {
                    continue;
                }
            }
            out[k] = Some(match e {
                Entry::Planet(i) => crate::screen_frames::GlobeView {
                    fleet: false,
                    rotation: g.planet,
                    surface: d.planets[i].surface,
                    red: self.problem_rows[k].2.then_some(red | 14 << 8 | 14 << 16),
                    at: g.pos,
                    alpha: 1.0 - t,
                },
                Entry::Fleet(_) => crate::screen_frames::GlobeView { fleet: true, rotation: g.fleet, surface: 0, red: None, at: g.pos, alpha: 1.0 - t },
            });
        }
        out
    }

    /// The clocks as after one visit with `infokeringeto` = `turn` (`--info-turn`, the exe's at a capture): the
    /// heads' fader ran as many frames (`turn / 0.0006`).
    pub fn at_turn(&mut self, turn: f32) {
        self.turn = turn;
        self.head_clock = turn / 0.0006 * 0.003;
    }

    /// The mouse over hot-spot `id` (`Info_Main`): 1..=8 pick the empire as soon as the mouse is over its column.
    /// Another empire also starts its list from the top (`BolygoFlottaPointer = 0`).
    /// Another empire also leaves the detail mode and reloads the slots (`LoadBolygoUrhajok`: home, unturned).
    /// The pointer over hot-spot `id`: a race column shows that empire. Not while a held button keeps another spot
    /// (`Mouse_Megfogva`: `IG2_gameUpdate` exe 0x4e0440 doesn't re-pick `AktIkon` during a menu, triangle or limit drag).
    pub fn hover(&mut self, id: Option<u32>) {
        if self.held.is_some() {
            return;
        }
        if let Some(e @ 1..=8) = id {
            if self.selected != e as usize {
                self.pointer = 0;
                self.detail = false;
                let rot = crate::scene::create_matrix(std::f32::consts::FRAC_PI_2, 0.0, 0.0);
                for g in &mut self.globes {
                    (g.planet, g.fleet, g.pos, g.target) = (rot, rot, g.home, g.home);
                }
            }
            self.selected = e as usize;
        }
    }

    /// A left press on hot-spot `id` (`Info_Main`, re/screens/info.md §5, §6.5–§6.8) with `n` list entries: 14 the
    /// charts, 15 the star log, 16..=25 a chart, 9..=12 a row's detail, 37 back from it, 30/31 the list up/down
    /// (`AtteszMatrixokat`; the design list in detail mode), 34 the scroll bar at virtual `y`, 35/36 the star log.
    pub fn press(&mut self, id: Option<u32>, d: &InfoData, human: usize, y: f32) {
        let n = d.list(self.selected, human).len();
        // `Info_Main` (exe 0x4e81c0): the mode and chart buttons, the list and star log arrows click on every press,
        // whether or not anything moves.
        if matches!(id, Some(14..=25 | 30 | 31 | 35 | 36)) {
            self.sounds.push(crate::audio::CLICK);
        }
        // `InfoMelyikGombot`: 14 → `GombMeshek[0xb]`, 15 → 5, a chart `c` → `c`, past 4 → `c + 1`.
        match id {
            Some(14) => self.buttons.start(0xb),
            Some(15) => self.buttons.start(5),
            Some(c @ 16..=25) => self.buttons.start(if c - 16 > 4 { c as i32 - 0xf } else { c as i32 - 16 }),
            _ => {}
        }
        match id {
            Some(14) => self.star_log = false,
            Some(15) => self.star_log = true,
            Some(c @ 16..=25) => self.chart = (c - 16) as usize,
            Some(k @ 9..=12) if !self.leaving => {
                let s = (k - 9) as usize;
                (self.detail, self.designs_ptr, self.sel_entry, self.sel_slot) = (true, 0, s + self.pointer, s);
                self.sel_pick = d.list(self.selected, human).get(s + self.pointer).copied();
                let g = &mut self.globes[s];
                g.send(DETAIL_AT);
                g.start = g.target;
            }
            Some(37) if !self.leaving => {
                self.leaving = true;
                let g = &mut self.globes[self.sel_slot];
                g.send(g.home);
            }
            Some(13) => self.detail = false,
            Some(30) if self.detail => self.designs_ptr = self.designs_ptr.saturating_sub(1),
            Some(31) if self.detail => {
                if (self.designs_ptr as i64) < self.designs_n as i64 - 9 {
                    self.designs_ptr += 1;
                }
            }
            Some(30) if self.pointer > 0 => {
                self.pointer -= 1;
                self.swap_globes(false);
            }
            Some(31) if (self.pointer as i64) < n as i64 - 4 => {
                self.pointer += 1;
                self.swap_globes(true);
            }
            // `Mouse_Megfogva = 1`: the bar keeps following the pointer while held ([`InfoScreen::ui_frame`]).
            Some(34) if !self.detail => {
                self.pointer = scroll_to(n, y);
                self.held = Some(34);
            }
            // 35/36 step every frame a button is held on them ([`InfoScreen::ui_frame`]); the press only clicks.
            _ => {}
        }
    }

    /// The release over hot-spot `id`: `IG2_gameUpdate` calls `Info_Main` once more over the spot the button was held
    /// on (a release flag 0 spot; 34's `Mouse_Megfogva` is already clear) [obs: `fx3-rest/infohold`, 36 held 15
    /// frames steps the log 16 times].
    pub fn release(&mut self, id: Option<u32>, d: &InfoData, human: usize) {
        if id != Some(34) {
            self.held_frame(id, 0.0, d, human);
        }
    }

    /// `Info_Main`'s per-frame spots (exe 0x4e81c0, its tail) on a frame a button is held on hot-spot `id` at virtual
    /// `y` (or on the spot `Mouse_Megfogva` keeps). `IG2_gameUpdate` calls `Info_Main` only while a button is down
    /// (and once on its release), never for the pointer alone [obs: `fx3-rest/infohold`, the pointer resting on 35
    /// 40 frames: no call, no step]. The scroll bar 34 held in list mode follows `y`, past its edges too; the
    /// star-log arrows 35/36 step the log; the list and star-log arrows (30/31, 35/36) keep their buttons sunk in.
    fn held_frame(&mut self, id: Option<u32>, y: f32, d: &InfoData, human: usize) {
        match self.held.or(id) {
            Some(34) if self.held == Some(34) && !self.detail => self.pointer = scroll_to(d.list(self.selected, human).len(), y),
            Some(30) => self.buttons.start(0xc),
            Some(31) => self.buttons.start(0xd),
            Some(35) => {
                if self.log_filled > 16 && self.log_pointer < self.log_filled - 16 {
                    self.log_pointer += 1;
                }
                self.buttons.start(0x11);
            }
            Some(36) => {
                if self.log_filled > 16 && self.log_pointer > 0 {
                    self.log_pointer -= 1;
                }
                self.buttons.start(0x12);
            }
            _ => {}
        }
    }

    /// `Info_Main`'s orders (re/screens/info.md §7) for the pointer at virtual `at` over hot-spot `id` this frame:
    /// the planet settings menus (26..=28 → the starmap's 0x19 on release over an item), the money triangle (29 →
    /// 0x5b) and the spending limit (32 → 0x5c while the pointer is over it, no button needed).
    pub fn orders(&mut self, id: Option<u32>, at: [f32; 2], b: Buttons, d: &InfoData, human: usize) -> Vec<ig2_net::Command> {
        use ig2_net::{Command, StarmapCmd};
        let mut out = Vec::new();
        if b.release && self.tri_held {
            (self.tri_held, self.tri_novelo) = (false, 0.0);
        }
        let id = match self.held {
            Some(h) if b.down || b.release => Some(h),
            _ => id,
        };
        let m = [at[0] as i32, at[1] as i32];
        let mut setting = None;
        if let Some(k @ 26..=28) = id {
            if self.select == Some(k) || b.press {
                if !b.release {
                    if self.select != Some(k) {
                        self.select_at = [m[0] + 0xc, m[1] - 0x14];
                    }
                    let [x, y] = self.select_at;
                    let first = if k == 28 { 1 } else { 0 };
                    self.select_item = -1;
                    for i in select_items(k, d.race) {
                        let top = y + (i - first) * 0xd;
                        if x <= m[0] && m[0] <= x + 0x78 && top <= m[1] && m[1] < top + 0xd {
                            self.select_item = i;
                        }
                    }
                    (self.select, self.held) = (Some(k), Some(k));
                } else {
                    if self.select_item >= 0 {
                        // A pick of the planet settings menus clicks (exe 0x4e85bf, 0x4e87b9, 0x4e880f).
                        self.sounds.push(crate::audio::CLICK);
                        setting = Some((k as i32 - 18, self.select_item));
                    }
                    self.select = None;
                }
            }
        }
        if id == Some(29) {
            if b.press && b.left {
                (self.held, self.tri_held) = (Some(29), true);
            }
            let right = b.press && b.right;
            if self.tri_held || right {
                match tri_drag([self.tri[0] as i32, self.tri[1] as i32], m, b.press, right) {
                    Some((knob, shares)) => {
                        self.tri = [knob[0] as f32, knob[1] as f32];
                        if self.tri_sent != Some(shares) {
                            self.tri_sent = Some(shares);
                            out.push(ig2_sim::game::InfoOrder::Shares { sliders: shares }.into());
                        }
                    }
                    None => (self.held, self.tri_held) = (None, false),
                }
            }
        }
        if id == Some(32) && self.selected == human {
            let l = limit_at(m[0]);
            if l != d.limit {
                out.push(ig2_sim::game::InfoOrder::Limit { limit: l }.into());
            }
            if b.down {
                self.held = Some(32);
            }
        }
        if let (Some((what, value)), Some(Entry::Planet(i))) = (setting, self.detail_entry(d, human)) {
            out.push(Command::Starmap { order: StarmapCmd::Setting { planet: i as i32 + 1, what, value } });
        }
        if !b.down {
            self.held = None;
        }
        out
    }

    /// `Info_MouseInit2` (exe 0x4e9530): whether hot-spot `id` is on now. 13 never; 9 + k for the rows shown, 33
    /// outside the detail mode and 37 only in it; 26..=28 in detail mode on the human's planet; the race columns for the empires alive
    /// and met; the chart buttons and 15 in chart mode, 35/36 in star-log mode; 14 and 15 only with a
    /// `MainScenarioDescriptor` (0xbbd140, a game started as a scenario).
    pub fn spot_on(&self, id: u32, s: &UiSnap) -> bool {
        let n = s.info.list(self.selected, s.human).len();
        match id {
            1..=8 => s.alive[id as usize] && s.known[id as usize],
            9..=12 => !self.detail && (id as usize - 9) + self.pointer < n,
            13 => false,
            14 => s.scenario,
            15 => s.scenario && !self.star_log,
            16..=25 => !self.star_log,
            33 => !self.detail,
            26..=28 => self.detail && self.selected == s.human && matches!(self.detail_entry(&s.info, s.human), Some(Entry::Planet(_))),
            35 | 36 => self.star_log,
            37 => self.detail,
            _ => true,
        }
    }

    /// The first hot-spot on now under the virtual point `at` (14 sits on the star-log button in star-log mode).
    pub fn spot_at(&self, name_x: &[i32], at: [f32; 2], s: &UiSnap) -> Option<u32> {
        // 33 is `56·min(HanyBolygoFlotta, 4)` high, set again every frame.
        let n = s.info.list(self.selected, s.human).len().min(4) as i32;
        spots(name_x)
            .into_iter()
            .map(|(id, r)| if id == 14 && self.star_log { (id, [0x15e, r[1], r[2], r[3]]) } else { (id, r) })
            .map(|(id, r)| if id == 33 { (id, [r[0], r[1], r[2], 0x38 * n]) } else { (id, r) })
            .find(|(id, [x, y, w, h])| self.spot_on(*id, s) && at[0] >= *x as f32 && at[0] < (x + w) as f32 && at[1] >= *y as f32 && at[1] < (y + h) as f32)
            .map(|(id, _)| id)
    }
}

impl InfoScreen {
    /// The hot-spot's tooltip (`Info_MouseInit`'s `Mouse_Szoveg_Set` texts, exe 0x4e9900): the race columns their
    /// empire's name, the others `GetString(9, 1, k)` (re/screens/info.md §5); the pointer at virtual `at`.
    pub fn tip_text(&self, id: u32, texts: &crate::ui::UiTexts, names: &[Vec<u8>], s: &UiSnap, at: [f32; 2]) -> Vec<u8> {
        let k = match id {
            1..=8 => return names.get(id as usize).cloned().unwrap_or_default(),
            9..=12 => 1,
            13 => 2,
            // `Info_MouseInit2` sets 15 again in chart mode with text 0x1a.
            15 if !self.star_log => 0x1a,
            14..=31 => id - 11,
            32 => return Vec::new(),
            // `Info_MouseInit2` sets 33 again every frame: 21 over a row's globe (`CheckMelyikFolottVan`), else 27.
            33 if self.over_globe(&s.info, s.human, at) => 21,
            33 => 0x1b,
            34..=37 => id - 12,
            _ => return Vec::new(),
        };
        texts.get(9, 1, k as u16)
    }
}

/// The scroll bar held at virtual `y` (`Info_Main`, hot-spot 34): `((y − 266 − s/2)·(n − 4) + (180 − s)/2) / (180 − s)`
/// with `s = 720/n`, clamped to 0..=n − 4; 0 for up to four entries.
pub fn scroll_to(n: usize, y: f32) -> usize {
    if n < 5 {
        return 0;
    }
    let (n, s) = (n as i32, 0x2d0 / n as i32);
    let p = (((y as i32 - 0x10a) - s / 2) * (n - 4) + (0xb4 - s) / 2) / (0xb4 - s);
    p.clamp(0, n - 4) as usize
}

/// `Info_MouseInit`'s hot-spots in id order (`Mouse_Szoveg_Set(id, 0, x, y, w, h, …)`, virtual pixels): 1..=8
/// the race columns, 9..=12 the planet/fleet rows, 13 the list, 14 charts, 15 star log, 16..=25 the chart buttons,
/// 26..=28 the tax/auto-build/priority selectors, 29 the triangle, 30/31 list up/down, 32 the spending limit, 33
/// the list's left edge, 34 its scroll bar, 35/36 star log up/down, 37 back from the detail (13 and 37 are 224
/// high; 33 here too, [`InfoScreen::spot_at`] cuts it to the list's rows).
pub fn spots(name_x: &[i32]) -> Vec<(u32, [i32; 4])> {
    let h = tri_h();
    let mut v = Vec::new();
    for e in 1..=8u32 {
        v.push((e, [name_x.get(e as usize - 1).copied().unwrap_or(40) - 2, 0x30, 0x32, 0x4f]));
    }
    for k in 0..4 {
        v.push((9 + k as u32, [0x3c, 0xf9 + 0x38 * k, 200, 0x35]));
    }
    v.push((13, [0x3c, 0xf9, 200, 4 * 0x38]));
    v.push((14, [0x24c, 0x1ae, 0x2d, 0x14]));
    v.push((15, [0x15e, 0x1ae, 0x2d, 0x14]));
    for (k, y) in [0x11b, 0x136, 0x151, 0x16c, 0x187].into_iter().enumerate() {
        v.push((16 + k as u32, [0x15e, y, 0x2d, 0x14]));
    }
    for (k, y) in [0x11b, 0x136, 0x151, 0x16c, 0x187].into_iter().enumerate() {
        v.push((21 + k as u32, [0x24c, y, 0x2d, 0x14]));
    }
    v.push((26, [0x87, 0x162, 100, 0xd]));
    v.push((27, [0x87, 0x170, 100, 0xd]));
    v.push((28, [0x87, 0x17e, 100, 0xd]));
    let top = (h + 50.0) - h;
    v.push((29, [0x10a, top as i32, 0x66, (((h + 50.0) - top) + 10.0) as i32]));
    v.push((30, [0x122, 0xf7, 0x12, 0x13]));
    v.push((31, [0x122, 0x1c2, 0x12, 0x13]));
    v.push((32, [0x118, 0xd1, 100, 8]));
    v.push((33, [0, 0xf9, 0x37, 4 * 0x38]));
    v.push((34, [0x122, 0x10a, 0x12, 0xcb]));
    v.push((35, [0x26d, 0x112, 0x12, 0x13]));
    v.push((36, [0x26d, 0x1af, 0x12, 0x13]));
    v.push((37, [0x3c, 0xf9, 200, 4 * 0x38]));
    v
}

/// The first hot-spot under the virtual point `at` (`Mouse_Darabol` [inf: the lowest id wins]), whether on or not.
#[cfg(test)]
fn spot_at(name_x: &[i32], at: [f32; 2]) -> Option<u32> {
    spots(name_x).into_iter().find(|(_, [x, y, w, h])| at[0] >= *x as f32 && at[0] < (x + w) as f32 && at[1] >= *y as f32 && at[1] < (y + h) as f32).map(|(id, _)| id)
}

#[allow(clippy::too_many_arguments)]
fn fill(f: &mut Frame, white: TexId, v: &Virtual, x: f32, y: f32, w: f32, h: f32, c: [f32; 3]) {
    use crate::render::kind;
    let p = v.px(x + w / 2.0, y + h / 2.0);
    f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, p, [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
}

/// `InfoTscVonal` + `line2d`: a horizontal line from `a` to `b` at `a`'s y, one screen pixel high, its vertex colours
/// interpolated along it in the remaster's gamma pass (one strip a virtual pixel; the capture shows the colour bytes
/// as given, on screen row `⌊y·k⌋`).
fn line(f: &mut Frame, white: TexId, v: &Virtual, a: [f32; 2], b: [f32; 2], ca: [f32; 3], cb: [f32; 3]) {
    use crate::render::kind;
    let n = ((b[0] - a[0]).abs().round() as usize).max(1);
    for i in 0..n {
        let tm = (i as f32 + 0.5) / n as f32;
        let c: [f32; 3] = std::array::from_fn(|k| ca[k] + (cb[k] - ca[k]) * tm);
        let p = v.px(a[0] + (b[0] - a[0]) * tm, a[1]);
        let w = (b[0] - a[0]).abs() / n as f32;
        f.push(white, Blend::GammaAdditive, Instance::new(kind::SPRITE, p, [w * v.k / 2.0, 0.5], [c[0], c[1], c[2], 1.0]));
    }
}

/// `PutFaders` (exe 0x4ed410, `GFX_Info`): the textured fills under the frame (re/screens/info.md §6.3): the
/// chart's, the four list rows', the empire panel's, the scroll bar and its thumb, the human's spending limit. The
/// texts are faded in (`infoFelfadeles` 100: `t` = 0, opaque); the detail mode's shapes aren't drawn.
pub fn draw_faders(f: &mut Frame, white: TexId, v: &Virtual, s: &UiSnap, ui: &InfoScreen) {
    use crate::kit::fader::{fade_kerekit, kirak_fader};
    let (max, fr) = (crate::screen_frames::INFO_MAX_FRAMES, ui.frame);
    if fr < max - 15.0 {
        return;
    }
    let (mut slide, mut open) = (0, 0.0f32);
    if max + 15.0 <= fr {
        slide = ((1.0 - ((fr - max) - 15.0) / 15.0) * 200.0) as i32;
    } else if max < fr {
        slide = (((fr - max) * 200.0) / 15.0) as i32;
    }
    if fr < max && max - 15.0 <= fr {
        open = 1.0 - (fr - 30.0) / 15.0;
        slide = (open * 200.0) as i32;
    }
    // `PutFaders` (exe 0x4ed410): the rows' fill `(int)((1 − infoFelfadeles/100)·255)`; the limit box `KirakFader(1,
    // infof, …)` fades in with `F = 1 − infoFelfadeles/100`: fill `F·155 + 100`, edge `F·225 + 30` (`(int)` in
    // `KirakFader`).
    let fade = 1.0 - ui.fade_in / 100.0;
    let t = ((fade * 255.0) as i32) as f32;
    let dark = [0.0, 34.0, 50.0];
    kirak_fader(f, white, v, [387.0, 271.0, (0xe2 - slide) as f32, 183.0], [0.0, 54.0, 90.0], 100.0, [0.0, 90.0, 130.0], 30.0, ui.faders.offset(0));
    // `infof+0x4d8`, the fader clock (`Info_Belep` → +0.003 a frame, as the heads').
    let c = ui.head_clock;
    let x0 = (open + 15.0) as i32 as f32;
    for k in 0..4 {
        let y0 = (k * 0x37) as f32;
        if ui.detail {
            // The detail mode: the rows' boxes 58 high (the last 53, at y 413) join into one panel; offsets (20c, 0).
            let h = if k == 3 { 53.0 } else { 58.0 };
            fade_kerekit(f, white, v, [x0, y0 + 248.0, 271.0, h], dark, t, [c * 20.0, 0.0]);
            fade_kerekit(f, white, v, [x0 - 8.0, y0 + 252.0, 10.0, 45.0], dark, t, [c * 20.0, 0.0]);
            fade_kerekit(f, white, v, [x0 - 15.0, y0 + 262.0, 9.0, 25.0], dark, t, [c * 20.0, 0.0]);
        } else {
            fade_kerekit(f, white, v, [x0, y0 + 248.0, 271.0, 53.0], dark, t, [0.0, c * 20.0]);
            fade_kerekit(f, white, v, [x0 - 8.0, y0 + 252.0, 10.0, 45.0], dark, t, [c * 20.0; 2]);
            fade_kerekit(f, white, v, [x0 - 15.0, y0 + 262.0, 9.0, 25.0], dark, t, [c * 20.0; 2]);
        }
    }
    for r in [[157.0, 146.0, 330.0, 83.0], [135.0, 161.0, 26.0, 62.0], [483.0, 161.0, 19.0, 62.0], [498.0, 166.0, 8.0, 52.0]] {
        fade_kerekit(f, white, v, r, dark, t, [0.0, c * 18.0]);
    }
    let (bar, edge) = ([0.0, 54.0, 90.0], [0.0, 90.0, 130.0]);
    kirak_fader(f, white, v, [295.0, 267.0, 10.0, 180.0], bar, 100.0, edge, 30.0, [0.0; 2]);
    let n = s.info.list(ui.selected, s.human).len();
    let (a, b) = if n < 5 { (1.0, 178.0) } else { (180.0 / n as f32 * ui.pointer as f32 + 1.0, 4.0 / n as f32 * 180.0) };
    kirak_fader(f, white, v, [296.0, ((a + 267.0) as i32) as f32, 8.0, (b as i32) as f32], bar, 100.0, edge, 30.0, [0.0; 2]);
    // The selectors' boxes on the human's planet in detail mode.
    if ui.selected == s.human && matches!(ui.detail_entry(&s.info, s.human), Some(Entry::Planet(_))) {
        for y in [354.0, 368.0, 382.0] {
            kirak_fader(f, white, v, [133.0, y, 100.0, 13.0], [0.0, 20.0, 30.0], 60.0, [0.0, 15.0, 25.0], 30.0, [0.0; 2]);
        }
    }
    if ui.selected == s.human {
        let (ft, et) = (((fade * 155.0 + 100.0) as i32) as f32, ((fade * 225.0 + 30.0) as i32) as f32);
        kirak_fader(f, white, v, [280.0, 209.0, 100.0, 8.0], dark, ft, [0.0, 60.0, 100.0], et, ui.faders.offset(1));
    }
}

/// `GetBolygoGazdagsag` (exe 0x4aca20) of a planet record: richness `+0x24` × the owner's (else the human's) race
/// production multiplier, plus the info texts' bonus outside MP (re/rules/colony-economy.md).
fn richness(r: &[u8], s: &UiSnap, rules: &ig2_rules::Rules, mp: bool) -> i32 {
    let i16_at = |o: usize| i16::from_le_bytes([r[o], r[o + 1]]);
    let events: Vec<i16> = (1..=5).map(|h| i16_at(0x168 + 2 * h)).collect();
    let mut bonus = 0;
    if !mp && events.iter().any(|&e| e != 0) {
        if events.contains(&0xc3a) || events.contains(&0x1f41) {
            bonus = 20;
        }
        if events.contains(&0x1f42) {
            bonus = 10;
        }
        if events.contains(&0x91b) {
            bonus = 15;
        }
    }
    let e = if r[0x20] == 0 { s.human } else { r[0x20] as usize };
    let race = s.races.get(e).copied().unwrap_or(0) as usize;
    let rich = i32::from_le_bytes(r[0x24..0x28].try_into().unwrap());
    (rich as f32 * rules.race_production_mult.get(race).copied().unwrap_or(1.0) + bonus as f32) as i32
}

/// The detail mode's panel (re/screens/info.md §6.6): `InfoNagyKiirBolygo(61, 253, p)` (exe 0x4e6500) or
/// `InfoNagyKiirFlotta(61, 253, f)` (exe 0x4e6ea0), rows 13 apart, size 2.
#[allow(clippy::too_many_arguments)]
fn draw_detail(f: &mut Frame, font: &Font, v: &Virtual, s: &UiSnap, d: &InfoData, texts: &UiTexts, infostr: &[(i16, Vec<u8>)], txt17: &crate::ui::UiTexts, rules: &ig2_rules::Rules, col: &dyn Fn(usize) -> [f32; 3], ui: &InfoScreen) {
    let (x, y0) = (61.0, 253.0);
    let put = |f: &mut Frame, t: &[u8], x: f32, y: f32, len: usize, align: Align, c: usize| font.text(f, v, t, x, y, len, 2, align, col(c), None);
    let hu = |section: u16, k: i32| txt17.get(0x11, section, k.clamp(0, 99) as u16);
    let en = s.human;
    match ui.detail_entry(d, en) {
        Some(Entry::Planet(i)) => {
            let p = &d.planets[i];
            let r = &p.rec;
            let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            let label = |f: &mut Frame, k: u16, y: f32, len: usize| put(f, &texts.get(2, 2, k), x + 10.0, y0 + y, len, Align::Left, 1);
            let (owner, mine) = (r[0x20] as usize, r[0x20] as usize == en || d.debug);
            let mut y = 0.0;
            label(f, 1, y, 8);
            put(f, &p.name, x + 80.0, y0 + y, 0x1a, Align::Left, 3);
            y += 13.0;
            label(f, 2, y, 0xb);
            let surface = [hu(5, r[0x22] as i32), format!(" ({}% ", richness(r, s, rules, d.multiplayer)).into_bytes(), hu(5, 20), b")".to_vec()].concat();
            put(f, &surface, x + 80.0, y0 + y, 0x1a, Align::Left, 0x21);
            y += 13.0;
            if owner != 0 && (owner == en || (en == 7 && r[0xcb] == 0) || r[0xc4 + en] != 0 || d.debug) {
                label(f, 5, y, 0xb);
                put(f, format!("{} ({:+})", i32_at(0x28), i32_at(0x2c)).as_bytes(), x + 80.0, y0 + y, 0x1a, Align::Left, 0x21);
                y += 13.0;
            }
            if owner != 0 && mine {
                label(f, 6, y, 0xb);
                put(f, &hu(4, 41 + i32_at(0x30) / 100), x + 80.0, y0 + y, 0x1a, Align::Left, 0x21);
                y += 13.0;
                label(f, 7, y, 0xb);
                put(f, i32_at(0x44).to_string().as_bytes(), x + 80.0, y0 + y, 0x1a, Align::Left, 0x21);
                y += 13.0;
                for (k, a, b) in [(8u16, 0x28, 0x50), (10, 0x48, 0x4c)] {
                    let c = if i32_at(a) < i32_at(b) { 6 } else { 0x21 };
                    label(f, k, y, 0xb);
                    put(f, i32_at(a).to_string().as_bytes(), x + 80.0, y0 + y, 0x1a, Align::Left, c);
                    put(f, &texts.get(2, 2, k + 1), x + 115.0, y0 + y, 0xb, Align::Left, 1);
                    put(f, i32_at(b).to_string().as_bytes(), x + 168.0, y0 + y, 0x1a, Align::Left, c);
                    y += 13.0;
                }
            }
            if owner != 0 && (owner == en || ((en != 7 || r[0xcb] != 0) && r[0xc4 + en] > 1) || d.debug) {
                label(f, 12, y, 0xb);
                put(f, i32_at(0x58).to_string().as_bytes(), x + 80.0, y0 + y, 0xb, Align::Left, 0x21);
                y += 13.0;
            }
            if owner != 0 && mine {
                // `AdoMertek_Nev`, `AutoBuild_Nev`, `BolygoPrioritas_Nev` (the selectors 26..=28 lie over them).
                for (k, (base, off)) in [(22, 0x34usize), (30, 0x6c), (41, 0x6d)].into_iter().enumerate() {
                    label(f, 15 + k as u16, y + 13.0 * k as f32, 0x20);
                    put(f, &hu(5, base + r[off] as i32), x + 80.0, y0 + y + 13.0 * k as f32, 0x10, Align::Left, 0x21);
                }
                y += 39.0;
            }
            label(f, 13, y, 0xb);
            let h = ui.texts.0.clamp(1, 5);
            let code = i16::from_le_bytes([r[0x168 + 2 * h], r[0x169 + 2 * h]]);
            if code != 0 {
                if let Some((_, t)) = infostr.iter().find(|t| t.0 == code) {
                    put(f, t, x + 80.0, y0 + y, 0x1a, Align::Left, 6);
                }
            }
            if owner != 0 && mine {
                y += 13.0;
                label(f, 14, y, 0xb);
                let mut rendezo = [0usize; 5];
                let n = InfoData::sort_problems(p, d.debug, &mut rendezo);
                for &pr in &rendezo[1..=n] {
                    put(f, (p.problems[pr] / 10000).to_string().as_bytes(), x + 80.0, y0 + y, 2, Align::Right, 6);
                    put(f, b"%", x + 93.0, y0 + y, 1, Align::Left, 6);
                    put(f, &hu(4, 21 + pr as i32), x + 106.0, y0 + y, 0x20, Align::Left, 0x21);
                    y += 13.0;
                }
            }
        }
        Some(Entry::Fleet(i)) => {
            let fl = &d.fleets[i];
            let label = |f: &mut Frame, k: u16, y: f32| put(f, &texts.get(9, 2, k), x, y0 + y, 0xc, Align::Left, 1);
            let value = |f: &mut Frame, t: &[u8], y: f32, len: usize| put(f, t, x + 76.0, y0 + y, len, Align::Left, 2);
            let mine = fl.owner as usize == en;
            let mut y = 0.0;
            label(f, 1, y);
            value(f, &fl.name, y, 0x14);
            y += 13.0;
            if mine || d.debug || d.radar > 1 {
                label(f, 2, y);
                value(f, fl.ships.to_string().as_bytes(), y, 10);
                y += 13.0;
                label(f, 3, y);
                value(f, fl.ground.to_string().as_bytes(), y, 10);
                y += 13.0;
            }
            if mine || d.debug || d.radar > 2 {
                label(f, 4, y);
                value(f, (((fl.speed as f64) * 100.0 + 0.5) as i32).to_string().as_bytes(), y, 10);
                y += 13.0;
            }
            if mine {
                label(f, 5, y);
                value(f, fl.power.to_string().as_bytes(), y, 10);
                y += 13.0;
                if fl.order != 0 {
                    // `sprintf((9,2,8|9|10), …)`: "Space", "Planet %s", "Fleet %s".
                    let fmt = texts.get(9, 2, 8 + fl.kind.clamp(0, 2) as u16);
                    let target = if fl.kind == 0 { crate::ui::sprintf(&fmt, &[0]) } else { replace_s(&fmt, &fl.target) };
                    label(f, 6, y);
                    value(f, &target, y, 0x1e);
                    y += 13.0;
                }
                label(f, 7, y);
                // From entry 0 while `i + InfoMaxTervekPtr < n` [quirk: the pointer shortens the list].
                for (k, (name, count)) in fl.designs.iter().enumerate().take(9) {
                    if k + ui.designs_ptr >= fl.designs.len() {
                        break;
                    }
                    let t = [&name[..], format!(" ({count})").as_bytes()].concat();
                    value(f, &t, y, 0x18);
                    y += 13.0;
                }
            }
        }
        None => {}
    }
}

/// The first `%s` of `fmt` replaced by `s`.
fn replace_s(fmt: &[u8], s: &[u8]) -> Vec<u8> {
    match fmt.windows(2).position(|w| w == b"%s") {
        Some(i) => [&fmt[..i], s, &fmt[i + 2..]].concat(),
        None => fmt.to_vec(),
    }
}

/// `KirakBolygoFlottakat(70, 249)` (exe 0x4eb700): the rows from `BolygoFlottaPointer`, 56 apart
/// (re/screens/info.md §6.2), each line `KiirStrAlap(s, 70, y + dy, 0x13, size, 0xc, colour)`.
#[allow(clippy::too_many_arguments)]
fn draw_rows(f: &mut Frame, font: &Font, v: &Virtual, s: &UiSnap, d: &InfoData, texts: &UiTexts, txt17: &crate::ui::UiTexts, names: &[Vec<u8>], col: &dyn Fn(usize) -> [f32; 3], ui: &InfoScreen) {
    let cat = |parts: &[&[u8]]| parts.concat();
    let put = |f: &mut Frame, t: &[u8], y: f32, size: usize, c: usize| font.text(f, v, t, 70.0, y, 0x13, size, Align::Left, col(c), None);
    let empire = |e: u8| names.get(e as usize).cloned().unwrap_or_default();
    let en = s.human;
    if ui.detail {
        return;
    }
    for (k, e) in ui.rows(d, en).into_iter().enumerate() {
        let y = (0xf9 + 0x38 * k) as f32;
        match e {
            Entry::Planet(i) => {
                let p = &d.planets[i];
                put(f, &p.name, y + 3.0, 2, 0x22);
                put(f, &empire(p.owner), y + 15.0, 1, 0x23);
                if p.owner != 0 && p.details {
                    let line = format!(": {} ({:+})", p.pop, p.growth);
                    put(f, &cat(&[&texts.get(2, 3, 1), line.as_bytes()]), y + 24.0, 1, 0x23);
                }
                if p.owner as usize == en || (d.debug && p.owner != 0) {
                    let (step, slot, _) = ui.problem_rows[k];
                    let text = if step < 1 {
                        let morale = p.morale / 100;
                        cat(&[&texts.get(2, 3, 2), b": ", &txt17.get(0x11, 4, (41 + morale).clamp(0, 99) as u16)])
                    } else {
                        cat(&[format!("{}% ", p.problems[slot] / 10000).as_bytes(), &txt17.get(0x11, 4, (21 + slot) as u16)])
                    };
                    put(f, &text, y + 33.0, 1, 0x23);
                    if let Some(q) = &p.queue[((ui.counter / 30) % 2 == 0) as usize] {
                        put(f, q, y + 42.0, 1, 1);
                    }
                }
            }
            Entry::Fleet(i) => {
                let fl = &d.fleets[i];
                let (name_col, c) = if fl.flags & 0x400 != 0 { (6, 6) } else if fl.flags & 0x4000 != 0 { (1, 1) } else { (3, 0x21) };
                put(f, &fl.name, y + 3.0, 2, name_col);
                put(f, &empire(fl.owner), y + 15.0, 1, c);
                let mine = fl.owner as usize == en || d.debug;
                if mine {
                    if fl.order == 0 {
                        put(f, &texts.get(2, 3, 3), y + 24.0, 1, c);
                    } else if fl.order == 1 || fl.order == 2 {
                        let target = if fl.kind == 0 { texts.get(2, 3, 4) } else { fl.target.clone() };
                        put(f, &cat(&[&texts.get(2, 3, 4 + fl.order as u16), b":", &target]), y + 24.0, 1, c);
                    }
                }
                if mine || d.radar > 1 {
                    let (ships, fighters) = (fl.ships - fl.fighters, fl.fighters);
                    let line = |a: u16, cap: bool| {
                        let tail = if cap { format!(":{}/{}", fl.ground, fl.capacity) } else { format!(":{}", fl.ground) };
                        cat(&[&texts.get(2, 3, a), format!(":{ships} ").as_bytes(), &texts.get(2, 3, a + 1), format!(":{fighters} ").as_bytes(), &texts.get(2, 3, a + 2), tail.as_bytes()])
                    };
                    let mut t = line(7, true);
                    if t.len() > 0x13 {
                        t = line(10, true);
                    }
                    if t.len() > 0x13 {
                        t = line(10, false);
                    }
                    put(f, &t, y + 33.0, 1, c);
                }
                if mine || d.radar > 2 {
                    let speed = ((fl.speed as f64) * 100.0 + 0.5) as i32;
                    put(f, &cat(&[&texts.get(2, 3, 13), format!(": {speed}").as_bytes()]), y + 42.0, 1, c);
                }
            }
        }
    }
}

/// `Info_PutStarLog` (ELF 0x5645b0, re/screens/info.md §6.8): from `infoframe` 60 the 16 lines of records `184 + c −
/// StarLogPointer` at (410, 275 + 11c), size 2, in the record's colour, at most `(f − 60)·33/15` characters and
/// `(f − 60)·100/15` % bright (the texts faded in).
fn draw_star_log(f: &mut Frame, font: &Font, v: &Virtual, col: &dyn Fn(usize) -> [f32; 3], ui: &InfoScreen) {
    // The screen is open here, so nothing shows below 60 (the closing's full-length case isn't drawn).
    let max = crate::screen_frames::INFO_MAX_FRAMES;
    if ui.frame < max + 15.0 {
        return;
    }
    let len = ((ui.frame - max - 15.0) * 33.0 / 15.0) as i32;
    let bright = (((ui.frame - max - 15.0) * 100.0 / 15.0) as i32) as f32 / 100.0;
    if len <= 0 {
        return;
    }
    for c in 0..16usize {
        let Some((colour, text)) = (c + 0xb8).checked_sub(ui.log_pointer).and_then(|i| ui.log.get(i)) else { continue };
        let rgb = col(*colour as usize).map(|x| x * bright);
        font.text(f, v, text, 410.0, (275 + 11 * c) as f32, len as usize, 2, Align::Left, rgb, None);
    }
}

/// The 2D passes beside the chart (`InfoGfx2D_2`, `_3`), drawn over the frame.
#[allow(clippy::too_many_arguments)]
pub fn draw(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, s: &UiSnap, d: &InfoData, texts: &UiTexts, infostr: &[(i16, Vec<u8>)], txt17: &crate::ui::UiTexts, rules: &ig2_rules::Rules, names: &[Vec<u8>], palette: &[[f32; 3]], ui: &InfoScreen) {
    let name_x = &rules.race_name_x[..];
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    draw_rows(f, font, v, s, d, texts, txt17, names, &col, ui);
    if ui.detail {
        draw_detail(f, font, v, s, d, texts, infostr, txt17, rules, &col, ui);
    }
    draw_star_log(f, font, v, &col, ui);
    let label = |k: u16| {
        let mut t = texts.get(9, 3, k);
        t.push(b':');
        t
    };
    // `MeghuzIzeVonalak`: the four list rows' top and bottom lines in 0x322200, which `line2d` takes as 0x00BBGGRR
    // (DDColor's order too): (0, 34, 50) on the capture…
    let row = [0.0, 34.0 / 255.0, 50.0 / 255.0];
    let rows: &[f32] = if ui.detail { &[249.0, 466.0] } else { &[249.0, 301.0, 304.0, 356.0, 359.0, 411.0, 414.0, 466.0] };
    for &y in rows {
        line(f, white, v, [0x1d as f32, y], [0x104 as f32, y], row, row);
    }
    // …and under each race column a line fading in and out of the race's colour `DDColor[e + 15]` at y 107.
    for e in 1..=8 {
        let x = name_x.get(e - 1).copied().unwrap_or(40) as f32;
        let c = col(e + 15);
        line(f, white, v, [x, 107.0], [x + 27.0, 107.0], [0.0; 3], c);
        line(f, white, v, [x + 27.0, 107.0], [x + 54.0, 107.0], c, [0.0; 3]);
    }
    // `KirakBirodalomInfo` from (180, 150), rows 15 apart, values at x 240: the name; the planets and fleets the
    // human sees; for the human also last year's trade and tax income and last month's.
    let e = ui.selected.min(18);
    let (x, y) = (0xb4 as f32, 0x96 as f32);
    font.text(f, v, &label(2), x, y, 10, 2, Align::Left, col(1), None);
    font.text(f, v, names.get(e).map_or(&[][..], |n| &n[..]), x + 60.0, y, 0x14, 2, Align::Left, col(2), None);
    let known = crate::ui::sprintf(&texts.get(9, 3, 9), &[d.known_planets[e] as i64, d.known_fleets[e] as i64]);
    font.text(f, v, &label(5), x, y + 15.0, 10, 2, Align::Left, col(1), None);
    font.text(f, v, &known, x + 60.0, y + 15.0, known.len(), 2, Align::Left, col(2), None);
    if e == s.human {
        let mut fmt = texts.get(9, 3, 6);
        fmt.extend_from_slice(b": %d - ");
        fmt.extend(texts.get(9, 3, 7));
        fmt.extend_from_slice(b", %d - ");
        fmt.extend(texts.get(9, 3, 8));
        let income = crate::ui::sprintf(&fmt, &[d.trade_last as i64, d.tax_last as i64]);
        font.text(f, v, &label(3), x, y + 30.0, 10, 2, Align::Left, col(1), None);
        font.text(f, v, &income, x + 60.0, y + 30.0, income.len(), 2, Align::Left, col(2), None);
        let mut fmt = texts.get(9, 3, 4);
        fmt.extend_from_slice(b": %d");
        let month = crate::ui::sprintf(&fmt, &[d.month_income as i64]);
        font.text(f, v, &month, x + 60.0, y + 45.0, month.len(), 2, Align::Left, col(2), None);
        // `PutLimitCucc`: "Spending limit:" at (180, 210), "$" and the limit at (386, 210), the bar of 48 ticks
        // at x 283..377 (y 210..215, the ends 2 shorter), lit (0x25) up to `ln(limit/10500 + 1)/3`.
        font.text(f, v, &label(1), 180.0, 210.0, 0x12, 2, Align::Left, col(1), None);
        // "$" + the limit (`operator+("$", CString(limit))`; the capture shows "$0").
        let mut lim = b"$".to_vec();
        lim.extend((d.limit as i32).to_string().bytes());
        font.text(f, v, &lim, 386.0, 210.0, 8, 2, Align::Left, col(2), None);
        let lit = ((((d.limit / 10500.0 + 1.0).ln() / 3.0 * 200000.0) as i32 / 2000) * 100) / 100;
        for c in (1..0x61).step_by(2) {
            let o = if c == 1 || c == 0x5f { 2 } else { 0 };
            let colour = col(if c < lit { 0x25 } else { 0xc });
            fill(f, white, v, (c + 0x11a) as f32, (0xd2 + o) as f32, 1.0, (0xd7 - 2 * o - 0xd2 + 1) as f32, colour);
        }
        // `PutHaromszog`: the triangle's corners and their income shares in per cent.
        font.text(f, v, &texts.get(9, 3, 10), 290.0, 127.0, 0x14, 1, Align::Left, col(2), None);
        font.text(f, v, &texts.get(9, 3, 11), 269.0, 40.0, 0x14, 1, Align::Left, col(2), None);
        font.text(f, v, &texts.get(9, 3, 12), 325.0, 40.0, 0x14, 1, Align::Left, col(2), None);
        let pct = |x: f64| format!("{}%", (x * 100.0) as i32).into_bytes();
        // They blink (`KiirStrSzinSzorzo = AktIdo300Szazalek`) while `InfoHaromszogNovelo` < 100 (after a drag).
        let k = if ui.tri_novelo != 100.0 { crate::hud::blink(ui.frames as f32 / 100.0, 300.0) } else { 1.0 };
        let c = col(2).map(|x| x * k);
        font.text(f, v, &pct(d.shares[0]), 310.0, 118.0, 5, 1, Align::Left, c, None);
        font.text(f, v, &pct(d.shares[1]), 243.0, 40.0, 0x14, 1, Align::Left, c, None);
        font.text(f, v, &pct(d.shares[2]), 380.0, 40.0, 0x14, 1, Align::Left, c, None);
    }
    draw_select(f, font, white, v, d, txt17, &col, ui);
}

/// An open planet-settings menu (re/screens/info.md §7.1): `PutFaders`' box on fader pass 2 (after `InfoGfx2D_2`,
/// `FadeKerettelKerekit(x − 6, y − 2, 130, h, (0, 10, 0) 60, (0, 5, 0) 30)` untextured, h = 108 for the taxes, 13 per
/// auto-build mode + 4, 43 for the priorities), then `InfoGfx2D_3`'s items `KiirStr(name, x, y + 13i + 3, 20, 2, …,
/// colour 3, 6 under the pointer)`: `AdoMertek_Nev`, `AutoBuild_Nev`, `BolygoPrioritas_Nev` (the Hungarian `txt17`).
#[allow(clippy::too_many_arguments)]
fn draw_select(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, d: &InfoData, txt17: &crate::ui::UiTexts, col: &dyn Fn(usize) -> [f32; 3], ui: &InfoScreen) {
    let Some(k) = ui.select else { return };
    let [x, y] = ui.select_at.map(|c| c as f32);
    let items = select_items(k, d.race);
    let h = match k {
        26 => 0x6c as f32,
        27 => ((items.end() + 1) * 0xd + 4) as f32,
        _ => 0x2b as f32,
    };
    crate::kit::fader::fader(f, white, v, [x - 6.0, y - 2.0, 0x82 as f32, h], [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
    let (base, first) = match k {
        26 => (22, 0),
        27 => (30, 0),
        _ => (41, 1),
    };
    for i in items {
        let s = txt17.get(0x11, 5, (base + i) as u16);
        let c = if i == ui.select_item { 6 } else { 3 };
        font.text(f, v, &s, x, y + ((i - first) * 0xd + 3) as f32, 0x14, 2, Align::Left, col(c), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Info_MouseInit2` (exe 0x4e9530): 14 and 15 only with a `MainScenarioDescriptor` [obs: fx3-rest `infohold`
    /// on quick-1.sav, none]; 33's text 21 over a row's globe (`CheckMelyikFolottVan` exe 0x4e5320: the first row's
    /// home (−310, 40, −650) projects to (33.8, 276.9), a ±30 box), else 0x1b.
    #[test]
    fn scenario_spots_and_the_globe_test() {
        let s = InfoScreen::enter(6);
        let mut ui = UiSnap { human: 6, ..Default::default() };
        ui.info.planets = vec![ListPlanet { owner: 6, seen: true, ..Default::default() }];
        assert!(!s.spot_on(14, &ui) && !s.spot_on(15, &ui) && s.spot_on(16, &ui));
        ui.scenario = true;
        assert!(s.spot_on(14, &ui) && s.spot_on(15, &ui));
        assert!(s.over_globe(&ui.info, 6, [34.0, 277.0]));
        assert!(s.over_globe(&ui.info, 6, [63.0, 300.0]));
        assert!(!s.over_globe(&ui.info, 6, [64.0, 277.0]));
        // The second row's globe has no entry to show.
        assert!(!s.over_globe(&ui.info, 6, [34.0, 331.0]));
    }

    #[test]
    fn chart_buttons_follow_info_mouse_init() {
        let x = [41, 97, 157, 214, 382, 439, 496, 552];
        assert_eq!(spot_at(&x, [360.0, 290.0]), Some(16));
        assert_eq!(spot_at(&x, [600.0, 400.0]), Some(25));
        assert_eq!(spot_at(&x, [600.0, 435.0]), Some(14));
        assert_eq!(spot_at(&x, [360.0, 435.0]), Some(15));
        assert_eq!(spot_at(&x, [160.0, 60.0]), Some(3));
        let mut s = InfoScreen::enter(6);
        let d = InfoData::default();
        s.press(Some(23), &d, 6, 0.0);
        assert_eq!(s.chart, 7);
        s.press(Some(15), &d, 6, 0.0);
        assert!(s.star_log);
        s.hover(Some(3));
        assert_eq!(s.selected, 3);
    }

    #[test]
    fn list_scrolls_like_info_main() {
        assert_eq!(scroll_to(4, 400.0), 0);
        // n = 10: s = 72, ((300 − 266 − 36)·6 + 54)/108 = 0; at y 440 → (138·6 + 54)/108 = 8 → clamped to 6.
        assert_eq!(scroll_to(10, 300.0), 0);
        assert_eq!(scroll_to(10, 440.0), 6);
        assert_eq!(scroll_to(10, 360.0), 3);
        let mut s = InfoScreen::enter(6);
        let d = InfoData { fleets: vec![ListFleet { owner: 6, ..Default::default() }; 6], ..Default::default() };
        // `AtteszMatrixokat`: the slots' spins move with their entries.
        let spins: Vec<f32> = s.globes.iter().map(|g| g.spin).collect();
        s.press(Some(31), &d, 6, 0.0);
        assert_eq!(s.globes.iter().map(|g| g.spin).collect::<Vec<_>>(), [spins[1], spins[2], spins[3], spins[0]]);
        s.press(Some(31), &d, 6, 0.0);
        s.press(Some(31), &d, 6, 0.0);
        assert_eq!(s.pointer, 2);
        s.press(Some(30), &d, 6, 0.0);
        assert_eq!(s.pointer, 1);
        assert_eq!(s.globes.iter().map(|g| g.spin).collect::<Vec<_>>(), [spins[1], spins[2], spins[3], spins[0]]);
        s.hover(Some(3));
        assert_eq!(s.pointer, 0);
    }

    /// `fx3-rest/infohold` (quick-1.sav, five entries): the bar pressed at y 280 and dragged down and off it follows
    /// `Mouse_y` every held frame (pointer 0 at y 348, 1 from 357); the release at 450 moves nothing.
    #[test]
    fn scroll_bar_follows_the_held_pointer() {
        let mut s = InfoScreen::enter(6);
        let d = InfoData { fleets: vec![ListFleet { owner: 6, ..Default::default() }; 5], ..Default::default() };
        s.press(Some(34), &d, 6, 280.0);
        assert_eq!((s.pointer, s.held), (0, Some(34)));
        for (y, p) in [(348.0, 0), (357.0, 1), (442.0, 1)] {
            s.ui_frame(&d, 6, None, y, true);
            assert_eq!(s.pointer, p);
        }
        s.release(Some(34), &d, 6);
        // The pointer alone never moves it.
        s.held = None;
        s.ui_frame(&d, 6, Some(34), 266.0, false);
        assert_eq!(s.pointer, 1);
    }

    /// `fx3-rest/infohold` (40 star-log lines): resting on 35 steps nothing; 35 held 30 frames (+ the release frame)
    /// stops at 24 = 40 − 16, 36 held 15 frames with the right button steps 16 times (24 → 8). The button sinks 7, 10,
    /// stays at 10 while held, comes out 10, 3, 0 after the release (`InfoGombFrame` 0xB8531C).
    #[test]
    fn star_log_arrows_step_while_held() {
        let mut s = InfoScreen::enter(6);
        let d = InfoData::default();
        (s.star_log, s.log_filled) = (true, 40);
        for _ in 0..40 {
            s.ui_frame(&d, 6, Some(35), 0.0, false);
        }
        assert_eq!((s.log_pointer, s.buttons.frame), (0, 0));
        s.press(Some(35), &d, 6, 0.0);
        let mut frames = Vec::new();
        for _ in 0..30 {
            s.ui_frame(&d, 6, Some(35), 0.0, true);
            frames.push(s.buttons.frame);
        }
        s.release(Some(35), &d, 6);
        for _ in 0..3 {
            s.ui_frame(&d, 6, Some(35), 0.0, false);
            frames.push(s.buttons.frame);
        }
        assert_eq!(s.log_pointer, 24);
        assert_eq!(&frames[..3], [7, 10, 10]);
        assert_eq!(&frames[29..], [10, 10, 3, 0]);
        assert_eq!(s.buttons.depth[0x11], 0);
        for _ in 0..15 {
            s.ui_frame(&d, 6, Some(36), 0.0, true);
        }
        s.release(Some(36), &d, 6);
        assert_eq!(s.log_pointer, 8);
        assert_eq!(s.buttons.which, 0x12);
    }

    /// A chart button's press (`fx3-rest/infohold`, chart 17 → `GombMeshek[1]`): 7, 10, 3, 0, once.
    #[test]
    fn chart_button_dips_once() {
        let mut s = InfoScreen::enter(6);
        let d = InfoData::default();
        s.press(Some(17), &d, 6, 0.0);
        let frames: Vec<i32> = (0..5).map(|_| {
            s.ui_frame(&d, 6, None, 0.0, false);
            s.buttons.frame
        }).collect();
        assert_eq!((s.buttons.which, frames), (1, vec![7, 10, 3, 0, 0]));
    }

    fn planet(owner: u8, problems: &[(usize, i32)], morale: i32) -> ListPlanet {
        let mut p = ListPlanet { owner, seen: true, morale, ..Default::default() };
        for &(i, v) in problems {
            p.problems[i] = v;
        }
        p
    }

    #[test]
    fn rows_share_the_problem_step() {
        let d = InfoData {
            planets: vec![planet(6, &[(3, 200000), (5, 600000)], 300), planet(6, &[(2, 150000)], 300), planet(6, &[(4, 300000), (7, 120000)], 100), planet(3, &[], 300)],
            fleets: vec![ListFleet { owner: 6, ..Default::default() }, ListFleet { owner: 6, flags: 0x20, ..Default::default() }],
            ..Default::default()
        };
        assert_eq!(d.list(6, 6), vec![Entry::Planet(0), Entry::Planet(1), Entry::Planet(2), Entry::Fleet(0)]);
        let mut s = InfoScreen::enter(6);
        (s.problem_step, s.counter, s.problem_counter) = (2, 100, 100);
        s.settle(&d, 6);
        // Row 0 shows its second problem; row 1 has one, so the shared step falls back to the morale there and below.
        assert_eq!(s.problem_rows[0], (2, 3, true));
        assert_eq!(s.problem_rows[1], (0, 0, false));
        // Row 2: morale 1 is a big problem.
        assert_eq!(s.problem_rows[2], (0, 0, true));
        // 30 `Counter` later the step moves on: 1 → row 0's worst problem.
        s.counter += 30;
        s.settle(&d, 6);
        assert_eq!(s.problem_rows[0].0, 1);
        assert_eq!(s.problem_rows[0].1, 5);
    }

    #[test]
    fn detail_mode_slides_the_globe_out_and_back() {
        let d = InfoData { planets: vec![planet(6, &[], 300); 3], ..Default::default() };
        let mut s = InfoScreen::enter(6);
        s.press(Some(10), &d, 6, 0.0);
        assert!(s.detail);
        assert_eq!((s.sel_entry, s.sel_slot, s.sel_pick), (1, 1, Some(Entry::Planet(1))));
        // 40 UI frames to (−43, 28, −150); the globe 70 % transparent there, the others hidden.
        for _ in 0..41 {
            s.ui_frame(&d, 6, None, 0.0, false);
        }
        assert_eq!(s.globes[1].pos, DETAIL_AT);
        let v = s.globe_views(&d, 6);
        assert!(v[0].is_none() && v[2].is_none());
        assert!((v[1].unwrap().alpha - 0.3).abs() < 1e-5);
        // Back (37): home again in 40 frames, which ends the mode.
        s.press(Some(37), &d, 6, 0.0);
        assert!(s.leaving);
        for _ in 0..41 {
            s.ui_frame(&d, 6, None, 0.0, false);
        }
        assert!(!s.detail && !s.leaving);
        assert_eq!(s.globes[1].pos, Globe::home(1));
    }

    #[test]
    fn triangle_follows_info_main() {
        assert_eq!((319.0f32 - 271.9f32, 366.1f32 - 271.9f32), (AB_DX, BC_DX));
        assert_eq!((AB_DX * AB_DX, BC_DX * BC_DX), (AB_DX2, BC_DX2));
        let mid = [319, 68];
        // Inside: the knob goes there; the shares are the edge distances (BC/h, −AC/h/1.25, AB/h/1.25).
        let (k, sh) = tri_drag(mid, [319, 80], false, false).unwrap();
        assert_eq!(k, [319, 80]);
        assert!((sh[0] - 30.0 / 51.96).abs() < 1e-3 && (sh[1] - sh[2]).abs() < 1e-6);
        // A drag past AB is pulled back inside along x; a press there is refused.
        let (k, _) = tri_drag(mid, [200, 90], false, false).unwrap();
        assert!(k[0] > 300 && k[1] == 90);
        assert_eq!(tri_drag(mid, [200, 90], true, false), None);
        // Below the base: y clamps to base + 1 (51), above the apex to 101; a corner share under 0.03 is cut to 0.
        let (k, sh) = tri_drag(mid, [319, 40], false, false).unwrap();
        assert_eq!((k[1], sh[0]), (51, 0.0));
        assert_eq!(tri_drag(mid, [319, 200], false, false).unwrap().0, [319, 101]);
        // A right click inside resets it to the middle.
        assert_eq!(tri_drag(mid, [330, 70], true, true).unwrap().0, mid);
    }

    #[test]
    fn limit_follows_the_bar() {
        assert_eq!(limit_at(250), 0.0);
        assert_eq!(limit_at(300), ((((20.0f64 * 2000.0 * 3.0 / 200000.0).exp() - 1.0) * 10500.0) as i32 / 100 * 100) as f64);
        assert_eq!(limit_at(380), 200000.0);
        assert_eq!(limit_at(500), 200000.0);
    }

    #[test]
    fn select_menus_send_the_planet_settings() {
        let mut d = InfoData { planets: vec![planet(6, &[], 300); 3], ..Default::default() };
        d.planets[1].rec = vec![0; 0x188];
        let mut s = InfoScreen::enter(6);
        s.press(Some(10), &d, 6, 0.0);
        let b = |press, release, down| Buttons { press, release, down, left: true, right: false };
        // Pressed on the auto-build selector at (150, 370): the menu opens at (162, 350), right of the pointer, and
        // follows it.
        assert!(s.orders(Some(27), [150.0, 370.0], b(true, false, true), &d, 6).is_empty());
        assert_eq!((s.select, s.select_at, s.select_item), (Some(27), [162, 350], -1));
        assert!(s.orders(None, [170.0, 395.0], b(false, false, true), &d, 6).is_empty());
        assert_eq!(s.select_item, 3);
        let out = s.orders(None, [170.0, 395.0], b(false, true, false), &d, 6);
        assert_eq!(out, vec![ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Setting { planet: 2, what: 9, value: 3 } }]);
        assert_eq!(s.select, None);
        // Released off the items: nothing is sent.
        s.orders(Some(28), [150.0, 384.0], b(true, false, true), &d, 6);
        assert!(s.orders(None, [400.0, 384.0], b(false, true, false), &d, 6).is_empty());
    }

    #[test]
    fn detail_mode_ends_when_its_entry_goes() {
        let d = InfoData { planets: vec![planet(6, &[], 300); 4], ..Default::default() };
        let mut s = InfoScreen::enter(6);
        s.press(Some(12), &d, 6, 0.0);
        assert!(s.detail);
        let fewer = InfoData { planets: vec![planet(6, &[], 300); 3], ..Default::default() };
        s.ui_frame(&fewer, 6, None, 0.0, false);
        assert!(!s.detail);
        assert_eq!(s.globes[3].pos, Globe::home(3));
    }

    #[test]
    fn sweeps_wrap_like_the_exe() {
        let mut s = InfoScreen::enter(6);
        s.adders = [300.0; 10];
        s.turn = 0.5;
        // (int)(150) % 55 = 40 → 80 − 27.
        assert_eq!(s.sweep(0), 53.0);
    }

    #[test]
    fn spots_13_and_37_are_224_high() {
        assert_eq!(spots(&[40; 8]).iter().find(|(id, _)| *id == 37).unwrap().1[3], 224);
    }
}
