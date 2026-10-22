//! The colony screen's input and the 2D parts it owns (re/screens/colony.md §9–§10): the hot-spots of
//! `Colony_MouseInit` (ELF 0x4e0b80), `Colony_Main` (ELF 0x4dadf0 / exe 0x4666d0) and `ColonyKeyboardHandler`
//! (ELF 0x4dcd10): building selection (`AktEpulet`), the build menu's pick list (`ColonySelectEpuletMode`), the
//! build cursor (`ColonyBuildMode`), the repair and demolish windows, and the planet panel's turning building
//! (`buildshown`). Orders leave as `ig2_net::Command::Colony` (`MULTIGET_Colony`'s messages, `ig2_sim::game::ColonyOrder`).

use std::collections::HashMap;

use ig2_formats::Install;
use ig2_net::Command;
use ig2_sim::game::{ColonyOrder, ColonyOrderEvent};
use ig2_sim::ground::GroundMap;
use ig2_sim::save::{va, SaveState, PLANET_SIZE};

use crate::audio::{Rand, SoundId, CLICK};
use crate::colony::{ColonySnap, Ghost, View};
use ig2_rules::sounds::Table;
use crate::font::{Align, Font, Virtual};
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement, M3, V3};

/// `Mouse_Szoveg_Set(id, 0, x, y, w, h, …)` of `Colony_MouseInit`: the hot-spots the building screen uses, in the
/// order `Mouse_Darabol` tries them (the 3D view 0x14 last).
const SPOTS: [(u32, [f32; 4]); 9] = [
    (1, [179.0, 443.0, 31.0, 31.0]),
    (2, [212.0, 443.0, 31.0, 31.0]),
    (4, [0.0, 424.0, 166.0, 56.0]),
    (5, [522.0, 362.0, 100.0, 100.0]),
    (6, [25.0, 39.0, 145.0, 99.0]),
    (0x11, [245.0, 443.0, 31.0, 31.0]),
    (0x13, [63.0, 148.0, 85.0, 17.0]),
    (0x14, [0.0, 20.0, 640.0, 460.0]),
    (0x20, [0.0, 0.0, 640.0, 480.0]),
];

fn colony(idx: i32) -> SoundId {
    SoundId::new(Table::Colony, idx as u16)
}

/// `Colony_Belep`'s sounds (exe 0x465b40) for planet type `surface` (`Bolygok+0x22`) and the battlefield's
/// `weather`: the type's wind, and rain on a rainy field (a test of its own: a desert planet in rain gets both)
/// [`OptionsKellIdojaras` taken as on].
fn enter_ambient(r: &mut Rand, surface: u8, weather: i32) -> Vec<SoundId> {
    let mut out = Vec::new();
    if matches!(surface, 3 | 6) {
        out.push(colony(12 + r.below(2)));
    }
    if weather == 2 {
        out.push(colony(37 + r.below(3)));
    }
    match surface {
        2 | 9 | 8 | 1 | 5 => out.push(colony(53 + r.below(3))),
        4 => out.push(colony(56 + r.below(2))),
        7 => out.push(colony(43)),
        _ => {}
    }
    out
}

/// One check of `Frissit2_Colony`'s ambients (exe 0x477dc0): a 4-in-1000 draw, then the planet type's birds,
/// crickets or cracks (nothing on type 4 or the others).
fn ambient_pick(r: &mut Rand, surface: u8) -> Option<SoundId> {
    let hit = |r: &mut Rand| r.rand() % 1000 < 4;
    let id = match surface {
        1 | 5 if hit(r) => {
            if r.rand() & 1 == 0 { 17 + r.below(10) } else { 14 + r.below(3) }
        }
        2 | 8 | 9 if hit(r) => 49 + r.below(4),
        3 | 6 if hit(r) => {
            if r.rand() & 1 == 0 { 9 + r.below(3) } else { 8 }
        }
        7 if hit(r) => {
            if r.rand() & 1 == 0 { 44 + r.below(3) } else { 3 + r.below(5) }
        }
        _ => return None,
    };
    Some(colony(id))
}

/// `AktEpuletTipus` (exe 0xB9C0C4, saved).
const AKT_EPULET_TIPUS: u32 = 0xB9C0C4;

use crate::kit::inside_f as inside;

/// The repair and demolish windows (`ColonyRepairWindow`, `ColonyDemolishWindow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Repair,
    Demolish,
}

impl Confirm {
    /// `ShowRepairWindow`/`ShowDemolishWindow` (ELF 0x4bae90, 0x4ba980): the window's left edge, top and height;
    /// its width is `6 × (strlen(title) + 2)`.
    fn frame(self) -> (f32, f32, f32) {
        match self {
            Confirm::Repair => (222.0, 408.0, 56.0),
            Confirm::Demolish => (239.0, 420.0, 44.0),
        }
    }

    /// `HandleRepairWindow`/`HandleDemolishWindow` (ELF 0x4d8970, 0x4d87e0): row 1 (yes) for y in 434..=446, row 2
    /// (no) for 448..=460, inside the window's columns.
    fn row(self, cells: usize, p: [f32; 2]) -> u8 {
        let (x, _, _) = self.frame();
        let (mx, my) = (p[0].floor(), p[1].floor());
        if mx < x || mx >= x + (cells * 6) as f32 {
            return 0;
        }
        if my > 433.0 && my < 447.0 {
            1
        } else if my > 447.0 && my < 461.0 {
            2
        } else {
            0
        }
    }
}

/// The build menu's pick list while it is held open (`colSelectX/Y`, `SelectBuildingMikorVoltLenyomas`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Picker {
    at: [f32; 2],
    pressed_ms: u32,
    /// `colSelected`: the tech under the pointer, −1 none.
    hovered: i32,
}

/// What a click asks of the window beyond commands.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub commands: Vec<Command>,
    /// `AktKepKell = 2`: back to the starmap.
    pub starmap: bool,
    /// `AktKepKell = 3`, `AktPlanetKell`: another planet's colony screen.
    pub colony: Option<usize>,
    /// `ScrollHibaUzenet`: a red line on the message ticker.
    pub error: Option<Vec<u8>>,
    /// The press went to the 3D view outside the build cursor: a left drag pans the camera [deviation: the original
    /// scrolls at the window's edges; in a war the left drag is the box, §11].
    pub pan: bool,
    /// `PlaySoundGlobal(0)`.
    pub click: bool,
}

/// The planet as the input code reads it.
pub struct Ctx<'a> {
    pub state: &'a SaveState,
    pub snap: &'a ColonySnap,
    pub rules: &'a ig2_rules::Rules,
    pub sim: &'a ig2_sim::game::GameRules,
    pub texts: &'a crate::ui::UiTexts,
    /// `Talalmanyok[t]`'s localised name.
    pub tech_name: &'a dyn Fn(usize) -> Vec<u8>,
}

impl Ctx<'_> {
    fn en(&self) -> usize {
        self.state.mem.i32(va::EN).clamp(0, 18) as usize
    }
    fn planet_rec(&self, p: usize) -> Vec<u8> {
        self.state.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE)
    }
    /// The human's planet, not at war (`+0x20 == EN`, `+0x64` bit 0 clear).
    fn own(&self) -> bool {
        let r = self.planet_rec(self.snap.planet);
        r[0x20] as usize == self.en() && r[0x64] & 1 == 0
    }
    /// `SearchBuilding(p, 1, true)` (ELF 0x616720) > 0: a finished, working colony hub.
    fn has_hub(&self) -> bool {
        self.snap.buildings.iter().any(|b| b.kind == 1 && b.work_left == 0 && b.flags & 1 != 0)
    }
    fn building(&self, slot: usize) -> Option<&crate::colony::BuildingView> {
        self.snap.buildings.iter().find(|b| b.slot == slot)
    }
    /// The techs the pick list offers: 0x51..=0x90 the human owns (status 5), less 0x69, 0x81..=0x83 for race 5.
    fn offered(&self) -> Vec<usize> {
        let tech = self.state.tech();
        let en = self.en();
        let race = self.state.races()[en];
        (0x51..0x91).filter(|&t| tech.status(t, en) == 5 && (race != 5 || !matches!(t, 0x69 | 0x81..=0x83))).collect()
    }
    /// `Talalmanyok[t] + 0x27`: the building kind tech `t` gives.
    fn kind_of(&self, t: usize) -> usize {
        self.state.tech().byte(t, 0x27) as usize
    }
    fn text(&self, section: u16, index: u16) -> Vec<u8> {
        self.texts.get(3, section, index)
    }
    /// The war input's view of the planet (spec §11).
    fn war(&self) -> crate::colony_war::WarCtx<'_> {
        crate::colony_war::WarCtx { snap: self.snap, en: self.en() as i32, tutorial_paused: false }
    }
}

/// `sprintf(GetString(…), name)`: the first `%s` replaced.
fn format_s(f: &[u8], arg: &[u8]) -> Vec<u8> {
    match f.windows(2).position(|w| w == b"%s") {
        Some(i) => [&f[..i], arg, &f[i + 2..]].concat(),
        None => f.to_vec(),
    }
}

/// The colony screen's input state.
pub struct ColonyUi {
    /// `AktEpulet`: the selected building slot (0 = none).
    pub selected: usize,
    /// `AktEpuletTipus`: the build menu's building kind (1 = the colony hub).
    pub kind: usize,
    /// `ColonyBuildMode`: the build cursor is up.
    pub build_mode: bool,
    /// `CanModifyAktEpulet`: the next building this screen builds gets selected.
    can_modify: bool,
    /// `NemSzabadEpiteni`: build orders on their way (the pick list stays shut meanwhile).
    pub pending: i32,
    picker: Option<Picker>,
    window: Option<Confirm>,
    /// The hot-spot a button press captured (`Mouse_Megfogva`).
    held: Option<u32>,
    /// `AktBGridX/Y`: the grid cell under the pointer; the build cursor's spot (`AktBGridXSarga/YSarga`).
    pub cell: Option<(i32, i32)>,
    pub ghost: Option<Ghost>,
    /// `selectionsize`, `selectionatlatszosag`, `selectionzoomirany`, `OldAktEpulet` (`Frissit2_Colony`).
    sel_size: f32,
    sel_alpha: i32,
    sel_settled: bool,
    old_selected: usize,
    /// `ColonyCounter`: the screen's 100 Hz frame counter.
    counter: f32,
    /// `ColTargetX/Y`, `ColTFrame`: a radar click's glide.
    glide: Option<([f32; 2], i32)>,
    /// `buildshown`'s spin (frames turned).
    spin: f32,
    /// The ground war's input (spec §11).
    pub war: crate::colony_war::WarUi,
    /// The sounds the screen started since the window last took them (re/screens/audio.md §6.2).
    pub sounds: Vec<SoundId>,
    /// The libc `rand()` of the sound picks (a cosmetic stream, [`Rand`]).
    rand: Rand,
    /// `VoltHarc` (exe 0x99646a): the shown planet's ground war as `Frissit2_Colony` saw it last.
    volt_harc: bool,
}

impl Default for ColonyUi {
    fn default() -> Self {
        Self {
            selected: 0,
            kind: 1,
            build_mode: false,
            can_modify: false,
            pending: 0,
            picker: None,
            window: None,
            held: None,
            cell: None,
            ghost: None,
            sel_size: 300.0,
            sel_alpha: 0,
            sel_settled: false,
            old_selected: 0,
            counter: 0.0,
            glide: None,
            spin: 0.0,
            war: Default::default(),
            sounds: Vec::new(),
            rand: Rand(1),
            volt_harc: false,
        }
    }
}

impl ColonyUi {
    /// Entering the screen (`Colony_Belep`): with `WF+0x184` set the first building is selected
    /// (`HaVanEpuletAkkorAzLegyenKozepen`); on a foreign planet the selection goes back to 0.
    pub fn enter(s: &ColonySnap, own: bool, state: &SaveState) -> Self {
        // `AktEpuletTipus` is saved (exe 0xB9C0C4).
        let mut ui = Self { kind: state.mem.i32(AKT_EPULET_TIPUS).max(1) as usize, rand: Rand::seeded(), ..Self::default() };
        if s.camera.centre && own {
            ui.selected = s.buildings.first().map_or(0, |b| b.slot);
        }
        ui.enter_sounds(s);
        ui
    }


    /// `Colony_Belep` (exe 0x465b40) after `Init3dColony`: the planet type's wind (`Bolygok+0x22`), and rain on a
    /// rainy battlefield (a test of its own: a desert planet in rain gets both) [`OptionsKellIdojaras` taken as on].
    fn enter_sounds(&mut self, s: &ColonySnap) {
        let e = enter_ambient(&mut self.rand, s.surface, s.weather);
        self.sounds.extend(e);
    }

    /// `Frissit2_Colony`'s ambients (exe 0x477dc0), on the colony screen every fifth UI tick (`ColonyCounter`,
    /// already counted): a 4-in-1000 draw, then the planet type's birds, crickets or cracks (none on type 4).
    fn ambient(&mut self, surface: u8) {
        if (self.counter as i64) % 5 == 0
            && let Some(id) = ambient_pick(&mut self.rand, surface)
        {
            self.sounds.push(id);
        }
    }

    /// `Frissit2_Colony`'s music switch (exe 0x477dc0, on the colony screen, not in a self-playing game): a ground war
    /// starting on the shown planet → `PlayMusicGrWar(1, 0)`, ending → `PlayMusicMain(0, 0)`.
    pub fn war_music(&mut self, war: bool) -> Option<crate::audio::Music> {
        let m = match (self.volt_harc, war) {
            (false, true) => Some(crate::audio::Music::GrWar),
            (true, false) => Some(crate::audio::Music::Main),
            _ => None,
        };
        self.volt_harc = war;
        m
    }

    /// The enabled hot-spot under `p` (virtual pixels), per `Colony_MouseInit2` (ELF 0x4e1d90).
    fn spot(&self, c: &Ctx, p: [f32; 2]) -> Option<u32> {
        let own = c.own();
        let sel = c.building(self.selected).filter(|b| own && b.flags & 0x10 == 0);
        SPOTS.iter().find(|(id, r)| {
            inside(*r, p)
                && match id {
                    1 => sel.is_some_and(|b| b.damage > 0),
                    2 | 0x11 => sel.is_some(),
                    6 => own,
                    _ => true,
                }
        }).map(|&(id, _)| id)
    }

    /// `ColonyRemoveBuilding`: the build cursor goes.
    fn drop_build(&mut self) {
        self.build_mode = false;
        self.ghost = None;
    }

    /// A button pressed at `p` (virtual pixels): `left` or right.
    pub fn press(&mut self, c: &Ctx, p: [f32; 2], left: bool, now_ms: u32) -> Outcome {
        let mut out = Outcome::default();
        let spot = self.spot(c, p);
        // The war's buttons and its view input come first (`Colony_Main`'s 0xb..=0x12, 0x14 in a war).
        if c.snap.war_flag || !self.war.selected.is_empty() {
            let w = self.war.press(&c.war(), p, left, spot == Some(0x14));
            out.commands = w.commands;
            out.click = w.click;
            if w.drop_building {
                self.selected = 0;
            }
            if w.taken {
                self.held = spot;
                return out;
            }
        }
        let Some(id) = spot else { return out };
        self.held = Some(id);
        match id {
            1 if left => {
                self.window = Some(Confirm::Repair);
                self.drop_build();
            }
            2 if left => {
                self.window = Some(Confirm::Demolish);
                self.drop_build();
            }
            0x11 if left => {
                self.sounds.push(CLICK);
                out.commands.push(ColonyOrder::Priority { planet: c.snap.planet as i32, slot: self.selected as i32 }.into());
            }
            5 if left => {
                self.sounds.push(CLICK);
                self.radar(p);
            }
            6 if left && self.pending == 0 => {
                self.sounds.push(CLICK);
                self.picker = Some(Picker { at: [p[0] + 12.0, p[1] - 17.0], pressed_ms: now_ms, hovered: -1 });
                self.hover_picker(c, p);
            }
            0x13 if left => self.start_build(c, &mut out),
            0x14 => {
                out.pan = left && !self.build_mode && !c.snap.war_flag;
                self.view_press(c, left, &mut out);
            }
            _ => {}
        }
        out
    }

    /// The pointer moved to `p` with a button held (the frame's `Colony_Main` pass while captured).
    pub fn drag(&mut self, c: &Ctx, p: [f32; 2]) {
        if self.held == Some(0x14) && self.war.drag(&c.war(), p) {
            self.selected = 0;
        }
        match self.held {
            Some(5) => self.radar(p),
            Some(6) => self.hover_picker(c, p),
            _ => {}
        }
    }

    /// A button released at `p`.
    pub fn release(&mut self, c: &Ctx, p: [f32; 2], left: bool, now_ms: u32) -> Outcome {
        let mut out = Outcome::default();
        let held = self.held.take();
        let w = self.war.release(&c.war(), left, self.spot(c, p) == Some(0x14));
        out.commands = w.commands;
        // `ColonyGfx2D_3`: with the left button up an open window takes its answer and shuts.
        if let (true, Some(w)) = (left, self.window.take()) {
            let cells = self.window_cells(c, w);
            if w.row(cells, p) == 1 {
                // `HandleRepairWindow` sounds for any "yes" (exe 0x47d170); `HandleDemolishWindow` (0x47ced0) only
                // with the building there, after its order.
                if w == Confirm::Repair {
                    let r = self.rand.below(2);
                    self.sounds.push(colony(r));
                }
                if let (Confirm::Demolish, Some(_)) = (w, c.building(self.selected)) {
                    let r = self.rand.below(4);
                    self.sounds.push(SoundId::new(Table::Gr, 13 + r as u16));
                }
                if let Some(b) = c.building(self.selected) {
                    let (planet, slot) = (c.snap.planet as i32, b.slot as i32);
                    out.commands.push(match w {
                        Confirm::Repair => ColonyOrder::Repair { planet, slot },
                        Confirm::Demolish => ColonyOrder::Demolish { planet, slot },
                    }.into());
                    if w == Confirm::Demolish {
                        self.selected = 0;
                    }
                }
            }
        }
        match held {
            Some(6) => {
                if let Some(pk) = self.picker.take() {
                    self.pick(c, pk, now_ms);
                }
            }
            // Hot-spot 4 (the short info) released with the right button: the starmap; not while the war runs here.
            Some(4) if !left && !c.snap.war_flag && self.spot(c, p) == Some(4) => out.starmap = true,
            _ => {}
        }
        out
    }

    /// A key (`ColonyKeyboardHandler`): `E` the priority switch, Shift+Delete demolishes at once, PageUp/PageDown
    /// the previous/next own planet's colony.
    pub fn key(&mut self, c: &Ctx, k: Key) -> Outcome {
        let mut out = Outcome::default();
        let planet = c.snap.planet as i32;
        match k {
            Key::Priority if c.own() => {
                if self.selected != 0 {
                    out.commands.push(ColonyOrder::Priority { planet, slot: self.selected as i32 }.into());
                }
                // `ColonyKeyboardHandler` (exe 0x462a7e): the click after the (maybe empty) send.
                self.sounds.push(CLICK);
            }
            Key::Demolish if c.own() && c.building(self.selected).is_some() => {
                out.commands.push(ColonyOrder::Demolish { planet, slot: self.selected as i32 }.into());
                let r = self.rand.rand() & 3;
                self.sounds.push(SoundId::new(Table::Gr, 13 + r as u16));
                self.selected = 0;
            }
            Key::Prev | Key::Next if !c.snap.war_flag => out.colony = next_planet(c, k == Key::Next),
            Key::War(wk) => {
                let w = self.war.key(&c.war(), wk);
                out.commands = w.commands;
                if w.drop_building {
                    self.selected = 0;
                }
            }
            _ => {}
        }
        out
    }

    /// `colSelected` for the pointer at `p`: the offered tech whose row is under it.
    fn hover_picker(&mut self, c: &Ctx, p: [f32; 2]) {
        let Some(mut pk) = self.picker else { return };
        pk.hovered = -1;
        let hub = c.has_hub();
        for (t, col, row) in layout(&c.offered()) {
            let (x, y) = (pk.at[0] + (col * 0x78) as f32, pk.at[1] + (row * 0xd + 0xe) as f32);
            if p[0] >= x && p[0] <= x + 114.0 && p[1] >= y && p[1] < y + 13.0 && (hub || t == 0x51) {
                pk.hovered = t as i32;
                // Hovering an entry drops the build cursor (`ColonyRemoveBuilding`).
                if self.build_mode {
                    self.drop_build();
                }
            }
        }
        // Too far right: the list moves to x 163.
        if pk.at[0] + 477.0 > 640.0 {
            pk.at[0] = 163.0;
        }
        self.picker = Some(pk);
    }

    /// The pick list released (`Colony_Main`, hot-spot 6): a short press (under 301 ms) toggles the build cursor for
    /// the current kind; else the entry under the pointer becomes the kind with the cursor up.
    fn pick(&mut self, c: &Ctx, pk: Picker, now_ms: u32) {
        let sel = if now_ms.wrapping_sub(pk.pressed_ms) < 301 { 0 } else { pk.hovered };
        if !(c.has_hub() || sel == 0x51 || (self.kind == 1 && sel == -1)) {
            return;
        }
        if sel < 1 {
            self.build_mode = !self.build_mode;
            if !self.build_mode {
                self.ghost = None;
            }
        } else {
            self.kind = c.kind_of(sel as usize);
            self.build_mode = true;
        }
    }

    /// The 3D view pressed (hot-spot 0x14): select the building under the pointer, or build at the cursor.
    fn view_press(&mut self, c: &Ctx, left: bool, out: &mut Outcome) {
        if !self.build_mode {
            if left && c.snap.visible && c.snap.owner as usize == c.en() {
                let under = self.building_under(c);
                // `Colony_Main` hot-spot 0x14: the click when another building becomes the selection.
                if under != self.selected {
                    self.sounds.push(CLICK);
                }
                self.selected = under;
                if let Some(b) = c.building(self.selected) {
                    self.kind = b.kind as usize;
                    self.can_modify = false;
                }
            }
            return;
        }
        if !left {
            self.sounds.push(CLICK);
            self.drop_build();
            return;
        }
        if self.kind == 1 || c.has_hub() {
            if self.ghost.is_none_or(|g| g.blocked) {
                let tech = c.rules.buildings.get(self.kind).map_or(0, |b| b.tech as usize);
                out.error = Some(format_s(&c.text(2, 2), &(c.tech_name)(tech)));
            } else {
                self.start_build(c, out);
            }
        } else {
            out.error = Some(c.text(2, 1));
        }
    }

    /// `GetNearestBuilding`'s building (`selectedepulet + 1`): the one whose footprint holds the pointer's cell
    /// [deviation: the original picks the model hit by the click]; defence posts (kinds 0x20..=0x23, finished)
    /// select their turrets instead and give 0.
    fn building_under(&self, c: &Ctx) -> usize {
        let Some((gx, gy)) = self.cell else { return 0 };
        let hit = c.snap.buildings.iter().find(|b| {
            let (w, h) = c.rules.building_footprints.get(b.kind as usize).copied().unwrap_or((0, 0));
            let (x, y) = (b.x as i32, b.y as i32);
            gx >= x - w / 2 && gx < x + w / 2 + w % 2 && gy >= y - h / 2 - h % 2 && gy < y + h / 2
        });
        match hit {
            Some(b) if (0x20..0x24).contains(&b.kind) && b.work_left == 0 => 0,
            Some(b) => b.slot,
            None => 0,
        }
    }

    /// `colony_start_build` (ELF 0x4dcbb0): with the cursor on a free spot, 0x1f for the kind at the spot.
    fn start_build(&mut self, c: &Ctx, out: &mut Outcome) {
        let Some(g) = self.ghost.filter(|g| self.build_mode && !g.blocked) else { return };
        // `Build-Building1` (exe 0x4a88a0), before `Multi_Send(0x1f)`.
        let r = self.rand.below(2);
        self.sounds.push(colony(r));
        self.pending += 1;
        out.commands.push(ColonyOrder::Build { planet: c.snap.planet as i32, kind: self.kind as i32, x: g.at.0, y: g.at.1 }.into());
        self.can_modify = true;
        self.drop_build();
    }

    /// `Colony_Main` hot-spot 5: the camera glides to the radar point (`ColTargetX/Y`, 10 frames).
    fn radar(&mut self, p: [f32; 2]) {
        let s = crate::colony::MAP_SIZE;
        let tx = (-(p[0].floor() - 572.0) / 50.0) * s / 2.0;
        let ty = (-(p[1].floor() - 412.0) / 50.0) * s / 2.0;
        self.glide = Some(([tx, ty], 10));
    }

    /// The pointer over the 3D view at grid cell `cell` (`ShowColony`): the build cursor's spot is the first free
    /// one for the kind searched from there (`LocateActBuildingSuitableLocation`, the same search the order runs).
    pub fn hover(&mut self, c: &Ctx, cell: Option<(i32, i32)>) {
        self.war.pointer(cell);
        if cell == self.cell && (self.ghost.is_some() || !self.build_mode) {
            return;
        }
        self.cell = cell;
        self.ghost = None;
        let (Some(at), true) = (cell, self.build_mode) else { return };
        let world = c.state.colony();
        let Some(planet) = world.planets.get(c.snap.planet) else { return };
        let cells = c.state.vehicle_cells(c.snap.planet);
        if let Some(spot) = c.sim.placement.locate(planet, self.kind as u8, &cells, Some(at)) {
            self.ghost = Some(Ghost { kind: self.kind as u8, at: spot, blocked: false });
        }
    }

    /// What the orders did (`MULTIGET_Colony` for this player's screen on planet `planet`).
    pub fn events(&mut self, events: &[ColonyOrderEvent], human: u8, planet: usize) {
        for e in events {
            match *e {
                ColonyOrderEvent::Built { empire, planet: p, slot } if empire == human => {
                    // `Build3dBuilding` (exe 0x49c780, from `EpuletNew` on the shown planet): `Cracking1`.
                    if p == planet {
                        self.sounds.push(colony(3));
                    }
                    self.pending = (self.pending - 1).max(0);
                    if p == planet && self.can_modify && slot > 0 {
                        self.selected = slot;
                    }
                }
                ColonyOrderEvent::BuildRefused { empire, .. } if empire == human => {
                    self.pending = (self.pending - 1).max(0);
                    self.drop_build();
                }
                ColonyOrderEvent::Demolished { planet: p, .. } if p == planet => {
                    // `Remove3dBuilding` (exe 0x497760, from `EpuletDelete`): `Cracking2`.
                    self.sounds.push(colony(4));
                    self.selected = 0;
                }
                _ => {}
            }
        }
    }

    /// One 100 Hz UI frame (`Frissit2_Colony`): the selection marker's zoom-in and pulse, the panel's spin, the radar
    /// glide; and the selection kept on a building that still stands.
    pub fn ui_frame(&mut self, s: &ColonySnap, en: i32, cam: &mut crate::colony::Camera) {
        self.war.ui_frame(&crate::colony_war::WarCtx { snap: s, en, tutorial_paused: false });
        self.counter += 1.0;
        self.ambient(s.surface);
        self.spin += 1.0;
        if !s.buildings.iter().any(|b| b.slot == self.selected) {
            self.selected = 0;
        }
        if self.old_selected != self.selected {
            self.sel_size = 300.0;
            self.sel_settled = false;
            self.sel_alpha = 0;
            self.old_selected = self.selected;
        }
        let wave = ((self.counter as f64 / 30.0).sin() * 3.0 + 3.0) as f32;
        if !self.sel_settled {
            self.sel_size *= 0.95;
            self.sel_alpha = (((255.0 - (self.sel_size - 3.0)) / 3.0) as i32).clamp(0, 255);
            if self.sel_size <= wave {
                self.sel_settled = true;
            }
        }
        if self.sel_settled {
            self.sel_size = wave;
            self.sel_alpha = (-(self.counter as f64 / 30.0).sin() * 64.0 + 128.0) as i32;
        }
        if let Some((t, n)) = self.glide {
            cam.x += (t[0] - cam.x) / 2.0;
            cam.y += (t[1] - cam.y) / 2.0;
            self.glide = (n > 1).then_some((t, n - 1));
        }
    }

    /// The 3D pass's additions.
    pub fn overlay(&self, s: &ColonySnap, en: i32) -> crate::colony::Overlay {
        crate::colony::Overlay {
            marks: self.war.marks(&crate::colony_war::WarCtx { snap: s, en, tutorial_paused: false }),
            ghost: self.ghost.filter(|_| self.build_mode),
            selection: (self.selected > 0).then_some((self.selected, self.sel_size, self.sel_alpha)),
        }
    }

    fn window_cells(&self, c: &Ctx, w: Confirm) -> usize {
        let title = c.text(4, if w == Confirm::Repair { 4 } else { 1 });
        title.iter().take_while(|&&b| b != 0).count() + 2
    }

    /// `ColonyGfx2D_1`/`_4`'s pick list and `ColonyGfx2D_3`/`_4`'s windows (faders, then texts), at pointer `p`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, c: &Ctx, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], hu17: &ig2_formats::strings::UiStrings, p: [f32; 2], now_ms: u32) {
        let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
        if let Some(pk) = self.picker.filter(|pk| now_ms.wrapping_sub(pk.pressed_ms) > 300) {
            let offered = c.offered();
            let rows = layout(&offered).iter().map(|&(_, _, r)| r + 1).max().unwrap_or(0);
            crate::kit::fader::fader(f, white, v, [pk.at[0] - 6.0, pk.at[1] - 2.0, (4 * 0x78 + 3) as f32, (rows * 0xd + 0x12) as f32], [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
            // `Epulet_Terulet_Nev[1..=4]` (txt17 §3, 0x22 + k, the Hungarian file's `StringReadAll`).
            for k in 1..=4u16 {
                let name = crate::ui::hu_bytes(hu17.get(0x11, 3, 0x22 + k));
                font.text(f, v, &name, pk.at[0] + ((k - 1) * 0x78 + 0x36) as f32, pk.at[1] + 3.0, 0x12, 2, Align::Centre, col(6), None);
            }
            let hub = c.has_hub();
            let mut count: HashMap<u8, i32> = HashMap::new();
            for b in &c.snap.buildings {
                *count.entry(b.kind).or_default() += 1;
            }
            for (t, colm, row) in layout(&offered) {
                let (x, y) = (pk.at[0] + (colm * 0x78) as f32, pk.at[1] + (row * 0xd) as f32);
                let mut colour = if !hub && t != 0x51 { 1 } else { 3 };
                if t as i32 == pk.hovered {
                    colour = 6;
                }
                let n = count.get(&(c.kind_of(t) as u8)).copied().unwrap_or(0);
                if n > 0 {
                    font.int(f, v, n as i64, x, y + 18.0, 2, 1, Align::Centre, col(0xc));
                }
                font.text(f, v, &(c.tech_name)(t), x + 8.0, y + 17.0, 0x12, 2, Align::Left, col(colour), None);
            }
        }
        let Some(w) = self.window else { return };
        let (x, y, h) = w.frame();
        let cells = self.window_cells(c, w);
        let width = (cells * 6) as f32;
        crate::kit::fader::fader(f, white, v, [x, y, width, h], [0.0, 20.0, 30.0], 60.0, [0.0, 15.0, 25.0], 30.0);
        let row = w.row(cells, p);
        match w {
            Confirm::Repair => {
                font.text(f, v, &c.text(4, 4), x, 411.0, cells, 2, Align::Field, col(2), None);
                let cost = c.building(self.selected).map_or(0, |b| {
                    let k = c.rules.buildings.get(b.kind as usize);
                    let price = k.map_or(0, |k| c.state.tech().price(k.tech as usize));
                    ig2_sim::game::repair_price(price, b.damage, k.map_or(1, |k| k.max_hp))
                });
                let mut line = c.text(4, 5);
                line.extend(cost.to_string().bytes());
                font.text(f, v, &line, x, 423.0, cells, 2, Align::Field, col(2), None);
            }
            Confirm::Demolish => font.text(f, v, &c.text(4, 1), x, 423.0, cells, 2, Align::Field, col(2), None),
        }
        let mid = x + (cells * 3) as f32;
        font.text(f, v, &c.text(4, 2), mid, 435.0, cells, 3, Align::Centre, col(if row == 1 { 2 } else { 1 }), None);
        font.text(f, v, &c.text(4, 3), mid, 449.0, cells, 3, Align::Centre, col(if row == 2 { 2 } else { 1 }), None);
    }
}

/// `DrawHpStripe(v, max, x, y, w, 255)` (ELF 0x45f6a0): a black 5-pixel bar, inside it `(w − 2)·v/max` pixels of
/// colour 7 above half, 3 above a quarter, else 6.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hp_stripe(f: &mut Frame, white: TexId, v: &Virtual, palette: &[[f32; 3]], value: i32, max: i32, x: f32, y: f32, w: f32) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    let fill = |f: &mut Frame, x: f32, y: f32, w: f32, h: f32, c: [f32; 3]| {
        if w > 0.0 {
            f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
        }
    };
    fill(f, x, y, w, 5.0, col(0));
    let r = value as f32 / max.max(1) as f32;
    let c = if r > 0.5 { 7 } else if r > 0.25 { 3 } else { 6 };
    fill(f, x + 1.0, y + 1.0, ((w - 2.0) * r).trunc(), 3.0, col(c));
}

/// The keys the screen takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Priority,
    Demolish,
    Prev,
    Next,
    /// The war's keys (spec §11.2).
    War(crate::colony_war::WarKey),
}

/// The pick list's places: each offered tech's column `(t − 0x51)/16` and its row in that column.
fn layout(offered: &[usize]) -> Vec<(usize, usize, usize)> {
    let mut rows = [0usize; 4];
    offered.iter().map(|&t| {
        let colm = (t - 0x51) / 16;
        let r = rows[colm];
        rows[colm] += 1;
        (t, colm, r)
    }).collect()
}

/// `ColonyKeyboardHandler` PageUp/PageDown: the previous/next planet the human owns and sees (`+0xd8 + 2·EN > 0`).
fn next_planet(c: &Ctx, forward: bool) -> Option<usize> {
    let n = c.state.planet_count();
    let en = c.en();
    let mut p = c.snap.planet;
    for _ in 0..n {
        p = if forward { p % n + 1 } else { (p + n - 2) % n + 1 };
        let r = c.planet_rec(p);
        let seen = i16::from_le_bytes([r[0xd8 + 2 * en], r[0xd9 + 2 * en]]) > 0;
        if r[0x20] as usize == en && seen {
            return (p != c.snap.planet).then_some(p);
        }
    }
    None
}

/// The planet panel's turning building (`buildshown`, `ShowColony` + `Frissit2_Colony`): the build menu's kind's
/// preview model (`Building[k − 1] + 0x28`, `AddToScene(…, 1.5 · preview scale)`), stood up and leant back 0.3,
/// turning 0.012 a frame about `(0, cos −0.3, sin −0.3)`, 30000 in front of a camera of its own centred on
/// (95, 101) and clipped to 18..173 × 36..167 (the second `Update3D`).
#[derive(Default)]
pub struct PanelBuilding {
    models: HashMap<usize, Option<(Model, Vec<TexId>)>>,
    textures: HashMap<String, TexId>,
    white: Option<TexId>,
    target: Option<Target>,
}

/// The panel's viewport (`HatarX1..Y2` = 0x12..0xad × 0x24..0xa7) and its centre (`xcenter`, `ycenter`).
pub const PANEL_VIEW: [f32; 4] = [18.0, 36.0, 174.0, 168.0];
const PANEL_CENTRE: [f32; 2] = [95.0, 101.0];

impl PanelBuilding {
    fn texture(&mut self, r: &mut Renderer, install: &Install, path: Option<&str>) -> TexId {
        let white = *self.white.get_or_insert_with(|| r.add_texture(&crate::assets::Image::white()));
        let Some(path) = path else { return white };
        if let Some(&t) = self.textures.get(path) {
            return t;
        }
        let t = crate::assets::Image::load(install, path).map(|img| r.add_texture(&img)).unwrap_or(white);
        self.textures.insert(path.to_owned(), t);
        t
    }

    /// Render kind `kind` into a texture of the window's size; the caller draws [`PANEL_VIEW`] of it.
    pub fn render(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, kind: usize, spin: f32, size: [f32; 2]) -> Option<TexId> {
        if !self.models.contains_key(&kind) {
            let loaded = rules.building_models.get(kind).filter(|b| !b.preview.is_empty()).and_then(|b| {
                Model::load(install, "vector/colony/", &b.preview, "textures/colony/").map_err(|e| eprintln!("ig2-client: preview {kind}: {e}")).ok()
            });
            let loaded = loaded.map(|mut m| {
                m.set_look("_a_", scene::Look::Glass(0.4));
                m.set_look("_c_", scene::Look::Glass(0.4));
                let texs = m.parts.iter().map(|p| p.texture.clone()).collect::<Vec<_>>().into_iter().map(|t| self.texture(r, install, t.as_deref())).collect();
                (m, texs)
            });
            self.models.insert(kind, loaded);
        }
        let (model, texs) = self.models.get(&kind)?.as_ref()?.clone();
        // `local_e0 = Building+0x54` (1.5 at most below `OptionsBuildingRes` 2, the remaster runs the highest), ×3/2.
        let s = rules.building_models.get(kind).map_or(1.0, |b| b.preview_scale) * 3.0 / 2.0;
        let tilt = scene::mul(&scene::axis_matrix([1.0, 0.0, 0.0], -0.3), &scene::create_matrix(std::f32::consts::FRAC_PI_2, 0.0, 0.0));
        let turn = scene::axis_matrix([0.0, (-0.3f32).cos(), (-0.3f32).sin()], 0.012 * spin);
        let m: M3 = scene::mul(&scene::mul(&turn, &tilt), &Placement::add_to_scene(model.origin, s).matrix).map(|x| x * 1024.0);
        let at: V3 = [0.0, 0.0, -30000.0];
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        for (pi, p) in model.parts.iter().enumerate() {
            let start = dynamic.len() as u32;
            let a = match p.look {
                scene::Look::Glass(g) => g,
                _ => 1.0,
            };
            for v in &p.verts {
                let q = scene::apply(&m, v.pos);
                dynamic.push(MeshVertex { pos: [q[0] + at[0], q[1] + at[1], q[2] + at[2]], normal: scene::normalise(scene::apply(&m, v.normal)), uv: v.uv, color: [1.0, 1.0, 1.0, a] });
            }
            draws.push(MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex: texs.get(pi).copied().unwrap_or(texs[0]), glass: a < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false });
        }
        let wide = crate::kit::Wide::new(&crate::font::Virtual::new(size));
        let px = wide.px;
        // Camera space → clip over the virtual 640 × 480 with the centre at (95, 101): vx = 600·x/w + 95.
        let (cx, cy) = (PANEL_CENTRE[0], PANEL_CENTRE[1]);
        let (near, far) = (20000.0f32, 40000.0f32);
        let (a, b) = (1.0 / (1.0 / near - 1.0 / far) / near, 1.0 / (1.0 / near - 1.0 / far));
        let rows4 = [[1200.0 / 640.0, 0.0, -(cx - 320.0) / 320.0, 0.0], [0.0, -1200.0 / 480.0, -(1.0 - 2.0 * cy / 480.0), 0.0], [0.0, 0.0, -a, -b], [0.0, 0.0, -1.0, 0.0]];
        let rows4 = wide.rows(rows4);
        let view = std::array::from_fn(|c| std::array::from_fn(|k| rows4[k][c]));
        // `light5x` from (10, 10, −10) with the h axis flipped for the file normals, as the view's (colony.rs).
        let light = scene::normalise([-1.0, 1.0, -1.0]);
        let mut f = Frame::new([0.0, 0.0, 0.0, 0.0]);
        f.scene = Some(Scene3d {
            gamma: true,
            shadow: None,
            uniforms: SceneUniforms {
                view,
                eye: [0.0; 4],
                light: [light[0], light[1], light[2], 0.0],
                // `mainmeshid2`'s ambient 0x404040 and parallel light `light5x` 0xffffff (`Init3dColony`;
                // `SetEnvironment` only recolours `light2`).
                ambient: [0.251, 0.251, 0.251, 0.0],
                sun: [1.0, 1.0, 1.0, 0.0],
                fog: [0.0; 4],
                fog_range: [1e9, 2e9, 0.0, 1e9],
            },
            draws,
            dynamic,
        });
        let target = match &mut self.target {
            Some(t) if t.size == px => t,
            _ => self.target.insert(r.target(px)),
        };
        r.draw_into(&f, target, px, 0.0);
        Some(target.tex)
    }

    /// Draw the panel's viewport of the rendered texture.
    pub fn draw(f: &mut Frame, v: &Virtual, tex: TexId) {
        let [x0, y0, x1, y1] = PANEL_VIEW;
        let (w, h) = (x1 - x0, y1 - y0);
        let mut i = Instance::new(kind::SPRITE, v.px(x0 + w / 2.0, y0 + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [1.0; 4]);
        let wide = crate::kit::Wide::new(v);
        i.uv = [wide.u(x0), y0 / 480.0, w / (640.0 * wide.sx), h / 480.0];
        f.push(tex, Blend::GammaPremul, i);
    }
}

impl ColonyUi {
    /// `ColonyKisInfo(50, 424)` (ELF 0x4edb40) with `ColonyInfoMod` 2 (`FRISSIT_Colony2`: a building selected): the
    /// building's name, its damage bar, working state and product; `false` when nothing is selected (the planet's
    /// short info then). The small turning copy of the model beside it (`HandleKisInfoObject`) is not drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_building_info(&self, f: &mut Frame, c: &Ctx, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]]) -> bool {
        let Some(b) = c.building(self.selected) else { return false };
        let Some(k) = c.rules.buildings.get(b.kind as usize) else { return false };
        let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
        let (x, y) = (50.0, 424.0);
        let line = |f: &mut Frame, s: &[u8], dx: f32, dy: f32, len: usize, size: usize, align: Align, colour: usize| {
            font.text(f, v, s, x + dx, y + dy, len, size, align, col(colour), None);
        };
        // The mafia's name for a flagged (`+0x19` bit 4) building of techs 0x52..=0x90 is not ported (U25).
        line(f, &(c.tech_name)(k.tech as usize), 0.0, 3.0, 0x13, 2, Align::Left, 0x22);
        hp_stripe(f, white, v, palette, k.max_hp - b.damage, k.max_hp, x + 48.0, y + 24.0, 50.0);
        line(f, &c.text(2, 11), 0.0, 24.0, 0x13, 1, Align::Left, 0x23);
        if b.flags & 1 == 0 {
            if b.work_left == 0 {
                line(f, &c.text(2, 14), 0.0, 15.0, 0x13, 1, Align::Left, 0x23);
                if k.b1 != 0 {
                    line(f, &c.text(2, 15), 0.0, 33.0, 0x13, 1, Align::Left, 0x23);
                }
            } else {
                line(f, &c.text(2, 16), 0.0, 15.0, 0x13, 1, Align::Left, 0x23);
                line(f, &c.text(2, 17), 0.0, 33.0, 0x13, 1, Align::Left, 0x23);
                hp_stripe(f, white, v, palette, k.build_work - b.work_left as i32 / 50, k.build_work, x + 48.0, y + 33.0, 50.0);
            }
            return true;
        }
        // "Working:   %" without its last '%', the percentage (`+0x1e · 100 / 256`) in the cells after it.
        let full = c.text(2, 12);
        let head = full.strip_suffix(b"%").unwrap_or(&full);
        line(f, head, 0.0, 15.0, 0, 1, Align::Left, 0x23);
        let eff = b.efficiency as i32 * 100 / 0x100;
        let mut pct = eff.to_string().into_bytes();
        pct.push(b'%');
        line(f, &pct, (full.len() * 6) as f32, 15.0, 4, 1, Align::Right, if eff == 100 { 0x23 } else { 6 });
        if k.b1 != 0 {
            let label = c.text(2, 13);
            line(f, &label, 0.0, 33.0, 0xf, 1, Align::Left, 0x23);
            let mut n = k.output * b.yield_pct as i32 * b.efficiency as i32 / 0x100;
            let r = c.planet_rec(c.snap.planet);
            if k.b1 == 3 {
                let world = c.state.colony();
                let ctx = ig2_sim::colony::EconomyContext {
                    multiplayer: c.state.mem.u8(va::MULTIPLAYER) != 0,
                    human_empire: c.en() as u8,
                    empire_race: c.state.races(),
                    ..Default::default()
                };
                if let Some(p) = world.planets.get(c.snap.planet) {
                    n = n * ig2_sim::colony::production_base(p, &c.sim.side.colony, &ctx) / 100;
                }
            }
            // `BolygoInfoIs(p, −0x215c)`: kinds 6 and 7 show 5000 and 7000.
            let story = (0..5).any(|i| i16::from_le_bytes([r[0x16a + 2 * i], r[0x16b + 2 * i]]) == -0x215c);
            if story && b.kind == 6 {
                n = 5000;
            }
            if story && b.kind == 7 {
                n = 7000;
            }
            let mut s = match k.b1 {
                1 => vec![1],
                2 => vec![2],
                3 => vec![3],
                _ => Vec::new(),
            };
            s.extend(n.to_string().bytes());
            line(f, &s, (label.len() * 6) as f32, 32.0, 7, 2, Align::Left, 0x23);
        }
        true
    }

    /// The panel's spin in frames.
    pub fn spin(&self) -> f32 {
        self.spin
    }
}

/// The colony map's grid cell under window pixel `p` (`AktBGridX/Y`), if the pointer is on the ground.
pub fn cell_under(view: &View, gm: &GroundMap, p: [f32; 2]) -> Option<(i32, i32)> {
    view.ground_at(gm, p).map(|(x, y)| crate::colony::grid_cell(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Observed (sound-id trace, quick-1.sav planet 42, surface 8, 2026-10-22): `Colony_Belep` played
    /// `Colony_Hangok[54]` (Rock-windatmos2) at the script's first frame, one of the type's three.
    #[test]
    fn entering_plays_the_planet_types_wind() {
        let mut r = Rand(7);
        let e = enter_ambient(&mut r, 8, 0);
        assert_eq!(e.len(), 1);
        assert!((53..=55).contains(&e[0].idx) && e[0].table == Table::Colony);
        // A desert planet in rain gets its wind and the rain; type 7 one fixed sound; type 0 none.
        let e = enter_ambient(&mut r, 3, 2);
        assert!(e.len() == 2 && (12..=13).contains(&e[0].idx) && (37..=39).contains(&e[1].idx));
        assert_eq!(enter_ambient(&mut r, 7, 0), vec![colony(43)]);
        assert!(enter_ambient(&mut r, 0, 0).is_empty());
    }

    /// The ambients' draws (exe 0x477dc0): about 4 checks in 1000 sound, within the type's groups; type 4 never.
    #[test]
    fn ambients_draw_four_in_a_thousand() {
        let mut r = Rand(1);
        let hits: Vec<SoundId> = (0..100_000).filter_map(|_| ambient_pick(&mut r, 1)).collect();
        assert!((300..500).contains(&hits.len()), "{}", hits.len());
        assert!(hits.iter().all(|h| (14..=26).contains(&h.idx)));
        assert!((0..10_000).all(|_| ambient_pick(&mut r, 4).is_none()));
    }

    #[test]
    fn the_pick_list_fills_four_columns_of_sixteen() {
        let l = layout(&[0x51, 0x52, 0x61, 0x75, 0x76, 0x90]);
        assert_eq!(l, vec![(0x51, 0, 0), (0x52, 0, 1), (0x61, 1, 0), (0x75, 2, 0), (0x76, 2, 1), (0x90, 3, 0)]);
    }

    #[test]
    fn window_rows_follow_handle_repair_window() {
        // 10 cells = 60 pixels from x 222.
        assert_eq!(Confirm::Repair.row(10, [230.0, 440.0]), 1);
        assert_eq!(Confirm::Repair.row(10, [230.0, 450.0]), 2);
        assert_eq!(Confirm::Repair.row(10, [230.0, 447.0]), 0);
        assert_eq!(Confirm::Repair.row(10, [282.0, 440.0]), 0);
        assert_eq!(Confirm::Demolish.row(10, [230.0, 440.0]), 0);
    }

    /// The whole chain on quick-1.sav's planet 42: the pick list, the build cursor, a click in the view → 0x1f → the
    /// building stands and is selected; the demolish window → 0x20 → it goes (skips without the install and save).
    #[test]
    fn building_through_the_screen_reaches_the_sim() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let texts = crate::ui::UiTexts::load(&install);
        let mut w = crate::world::World::load(&save, &rules, &defaults, &volatile);
        w.show_colony(Some(42));
        let names = |t: usize| crate::hud::tech_name(&texts, t);
        let snap = w.colony_curr.clone().unwrap();
        let before = snap.buildings.len();
        let mut ui = ColonyUi::enter(&snap, true, &w.game.state);
        let (t, at) = {
            let c = Ctx { state: &w.game.state, snap: &snap, rules: &tables, sim: &rules, texts: &texts, tech_name: &names };
            assert!(c.has_hub());
            // Held on the panel past 300 ms, released on the first offered entry that isn't the hub.
            ui.press(&c, [100.0, 80.0], true, 0);
            let (t, colm, row) = layout(&c.offered()).into_iter().find(|&(t, _, _)| t != 0x51).unwrap();
            let pk = ui.picker.unwrap();
            let at = [pk.at[0] + (colm * 0x78) as f32 + 20.0, pk.at[1] + (row * 0xd + 0xe) as f32 + 5.0];
            ui.drag(&c, at);
            assert_eq!(ui.picker.unwrap().hovered, t as i32);
            ui.release(&c, at, true, 1000);
            assert!(ui.build_mode);
            assert_eq!(ui.kind, c.kind_of(t));
            ui.hover(&c, Some((100, 100)));
            (t, ui.ghost.expect("a free spot").at)
        };
        let out = {
            let c = Ctx { state: &w.game.state, snap: &snap, rules: &tables, sim: &rules, texts: &texts, tech_name: &names };
            ui.press(&c, [320.0, 300.0], true, 2000)
        };
        assert_eq!(out.commands.len(), 1, "tech {t}");
        assert!(!ui.build_mode && ui.pending == 1);
        for cmd in out.commands {
            w.send(cmd);
        }
        w.pump();
        ui.events(&std::mem::take(&mut w.colony_events), w.curr.human, 42);
        let snap = w.colony_curr.clone().unwrap();
        assert_eq!(snap.buildings.len(), before + 1);
        let b = snap.buildings.iter().find(|b| b.slot == ui.selected).expect("the new building selected");
        assert_eq!(((b.x as i32), (b.y as i32)), at);
        assert!(b.work_left > 0 && ui.pending == 0);
        // The demolish window: press its button, release on "yes".
        let c = Ctx { state: &w.game.state, snap: &snap, rules: &tables, sim: &rules, texts: &texts, tech_name: &names };
        ui.press(&c, [220.0, 450.0], true, 3000);
        assert_eq!(ui.window, Some(Confirm::Demolish));
        let out = ui.release(&c, [245.0, 440.0], true, 3100);
        assert_eq!(out.commands, vec![Command::from(ColonyOrder::Demolish { planet: 42, slot: b.slot as i32 })]);
        for cmd in out.commands {
            w.send(cmd);
        }
        w.pump();
        assert_eq!(w.colony_curr.as_ref().unwrap().buildings.len(), before);
    }

    #[test]
    fn format_puts_the_name_in() {
        assert_eq!(format_s(b"No room for %s here", b"Farm"), b"No room for Farm here".to_vec());
        assert_eq!(format_s(b"plain", b"x"), b"plain".to_vec());
    }
}
