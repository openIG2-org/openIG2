//! The research screen (`AktKep` 4, re/screens/research.md): the screen's own state (`Research_Belep` ELF 0x5e8780 /
//! exe 0x523540, the per-frame `Research3DUpdate` ELF 0x5e4e10 / exe 0x51fd10 and `FRISSIT_Research2` ELF 0x5eb780),
//! its input (the hit-mask picture `RESE_MSK.RAW`, `Research_Main` ELF 0x5ea1c0 / exe 0x523c10, the tech list and
//! the models under the pointer) sending `MULTIGET_Research`'s orders (`ig2_net::Command::Research`), the 2D pass
//! (`ResearchGfx2D_2` ELF 0x5df030 / exe 0x5221a0: the tech's texts, the lists, the labs, the running project) and the
//! 3D scene (`resbck.xvb` console, the `focilabda.xvb` ball, the category's tech models) rendered into a texture
//! over the virtual 640 × 480 screen like the starmap's panels.
//!
//! The opening and closing (`resframe` 0..=60: the console flying in, the info box and the wire sphere moved by
//! `MoveObj`/`RotateObj`, the ball flying in), the buttons' press animations (`resSetGombFrame`) and the hot-spots'
//! tooltips are drawn; the lasers of `ResearchGfx2D_1` and sounds are not.

use std::collections::HashMap;

use ig2_formats::Install;
use ig2_net::{Command, ResearchCmd};
use ig2_sim::colony_side::TechTable;
use ig2_sim::save::{va, SaveState};

use crate::audio::{SoundId, CLICK};
use crate::font::{Align, Virtual};
use ig2_rules::sounds::Table;
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement, M3, V3};
use crate::lighting;
use crate::starmap_panels::Draw2d;

/// Exe globals the screen reads from the saved state (re/formats/save.md §3).
mod at {
    /// `ResearchTerulet`, `ResearchMezo`, `ResearchTalalmany`, `Terulet_Akt[13]`.
    pub const AREA: u32 = 0xB9A9B4;
    pub const FIELD: u32 = 0xB9AA04;
    pub const TECH: u32 = 0xB9AA08;
    pub const FIELDS: u32 = 0x919200;
    /// `StoryAdat+0x6c`: the story's research bonus in percent (SP).
    pub const STORY_BONUS: u32 = 0xF273F8 + 0x6c;
    /// `PlayerFaj`.
    pub const RACES: u32 = 0xBAE574;
    /// `ResearchOldForgato` (saved; exe 0x919598 starts at 3, the radars): the middle wheel's front category.
    pub const FRONT: u32 = 0x919598;
}

/// `ADDZOOMMAX` (exe 0x918bd0: 0x3f490e56, a little under π/4); `ADDZOOMMIN` is a zeroed global.
const ZOOM_MAX: f32 = f32::from_bits(0x3f49_0e56);
const ZOOM_MIN: f32 = 0.0;
/// The model tooltip's hot-spot (`CheckIfTalalmanyokFolottVan`).
const MODEL_TIP: u32 = 0x11;
/// `InfoSorok[i]+0xb8` (`NEWGAME_Research`): where each info line's typing starts over.
const INFO_START: [f32; 4] = [0.0, -20.0, -40.0, -60.0];
/// The ball's centre (`MoveObj(FociMesh, 0, −35, −950)`).
const BALL: V3 = [0.0, -35.0, -950.0];
/// `resInitPos1` (exe 0x9185b0) and `InitPos2` (exe 0x9185c0): where `Research3DUpdate` puts the wire sphere
/// (`Group66`) and the info box (`Group85`) at the open frame (`MoveObj`, v/1024 model units).
const WIRE_AT: V3 = [0.0, 0.0, 200.0];
/// The wire sphere's colour opaque and lit [obs: (46, 144, 239) on the cells facing the camera at frame 30, (5, 18,
/// 30) = it × 0x20/0x100 on the far side and at rest].
const WIRE_INK: [f32; 3] = [46.0, 144.0, 239.0];
/// The ball's wire colour (`focilabda.xvb`), lit per vertex like the sphere's [obs: the face-on edges (49, 154, 255),
/// the lights saturating].
const BALL_INK: [f32; 3] = [49.0, 154.0, 255.0];
/// The info box's glass (`Box01` "Kekes").
const GLASS: [f32; 3] = [48.0, 149.0, 242.0];
/// The mesh pass's own light is off (ambient 1): the parts come lit per vertex (`ResearchLights`).
const LIGHT: V3 = [0.0, 0.0, 1.0];
const LIT: [f32; 3] = [1.0, 1.0, 1.0];
const INFO_AT: V3 = [f32::from_bits(0x3d14_7012), f32::from_bits(0xc369_c4a2), f32::from_bits(0xc10a_948d)];

/// The tooltip texts of `Research_MouseInit` (exe 0x524080: `Mouse_Szoveg_Set(id, …, GetString(4, 1, k))`, `k` from
/// the asm's immediates; `Research_MouseInit2` exe 0x523e20 gives 1 the text 0x18 every frame): `(id, k)`.
pub const TIPS: [(u32, u16); 23] = [
    (1, 0x18),
    (3, 7),
    (4, 8),
    (6, 9),
    (7, 0xa),
    (8, 0xb),
    (10, 0xc),
    (0xd, 0xd),
    (0xe, 0xe),
    (0xf, 0xf),
    (0x12, 0x10),
    (0x13, 0x11),
    (0x14, 0x12),
    (0x15, 0x13),
    (0x16, 0x14),
    (0x17, 0x15),
    (0x18, 0x16),
    (0x19, 0x17),
    // `Forgotabla`'s middle-wheel spots.
    (9, 1),
    (0x10, 2),
    (0xb, 3),
    (5, 4),
    (0xc, 5),
];

/// The tooltip texts by hot-spot id (`GetString(4, 1, k)` of [`TIPS`]).
pub fn tips(texts: &crate::ui::UiTexts) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(); 0x1a];
    for &(id, k) in &TIPS {
        out[id as usize] = texts.get(4, 1, k);
    }
    out
}

/// The lab hot-spots 0x13..=0x18 (tooltips only: on while the lab's cone shows, `Research_MouseInit2`).
const LAB_SPOTS: [(u32, [i32; 4]); 6] = [
    (0x13, [10, 0xbd, 0x37, 0x32]),
    (0x14, [10, 0xf5, 0x37, 0x32]),
    (0x15, [10, 0x129, 0x37, 0x32]),
    (0x16, [0x23f, 0xbd, 0x37, 0x32]),
    (0x17, [0x23f, 0xf5, 0x37, 0x32]),
    (0x18, [0x23f, 0x129, 0x37, 0x32]),
];

/// Hot-spots (`Research_MouseInit` exe 0x524080): `(id, mask colour, rect)`; colour 0 = the rectangle.
/// The middle wheel's five (9, 0x10, 0xb, 5, 0xc) take their colour from the slot their category is in.
const STATIC_SPOTS: &[(u32, u8, [i32; 4])] = &[
    (1, 0x11, [10, 0x114, 0x50, 0x15]),
    (3, 0, [9, 0x13b, 0x50, 0x16]),
    (4, 0x12, [10, 0x15d, 0x49, 0x18]),
    (6, 6, [10, 0x179, 0x37, 0x17]),
    (7, 7, [10, 0x179, 0x37, 0x17]),
    (8, 8, [10, 0x179, 0x37, 0x17]),
    (10, 10, [10, 0x179, 0x37, 0x17]),
    (0xd, 0xd, [10, 0x179, 0x37, 0x17]),
    (0xe, 0xe, [10, 0x179, 0x37, 0x17]),
    (0xf, 0xf, [10, 0x179, 0x37, 0x17]),
    (0x12, 0, [0xaf, 0x1d, 0x12a, 0x32]),
    (0x19, 0, [4, 0x10, 0x68, 0x2e]),
];

/// `Research_Main`'s category hot-spots → `TT_*` → `ResearchTerulet` (1 ships, 2 weapons, 3 radars, 4 shields,
/// 5 bombs, 6 civil, 7 research, 8 military, 9 power, 10 tanks, 11 turrets, 12 special).
const SPOT_AREA: [(u32, usize); 12] = [(5, 5), (6, 6), (7, 9), (8, 8), (9, 3), (10, 7), (0xb, 4), (0xc, 1), (0xd, 12), (0xe, 10), (0xf, 11), (0x10, 2)];

/// The middle wheel's categories in hot-spot order (`Forgotabla`'s `Mouse_Szoveg_Set` for 9, 0x10, 0xb, 5, 0xc).
const WHEEL_SPOTS: [u32; 5] = [9, 0x10, 0xb, 5, 0xc];

/// What a press on the screen does.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// A lockstep order (`Multi_Send`).
    Send(Command),
}

/// The screen's place, kept between its visits (`ResearchTerulet`, `ResearchMezo`, `ResearchTalalmany`,
/// `Terulet_Akt[13]`, `ResearchOldForgato`): the exe's globals; the client keeps them per peer, off the shared state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub area: usize,
    pub field: i32,
    pub tech: i32,
    pub fields: [i32; 13],
    pub front: i32,
}

impl Place {
    /// The saved place (the game's start or a loaded save).
    pub fn read(state: &SaveState) -> Self {
        let m = &state.mem;
        Self {
            area: m.i32(at::AREA).clamp(1, 12) as usize,
            field: m.i32(at::FIELD),
            tech: m.i32(at::TECH),
            fields: std::array::from_fn(|k| m.i32(at::FIELDS + 4 * k as u32)),
            front: m.i32(at::FRONT),
        }
    }

    /// Into the state's globals, as the exe's save finds them (`ResearchTerulet`, `ResearchMezo`, `ResearchTalalmany`,
    /// `Terulet_Akt[13]`, `ResearchOldForgato`: `LoadSave(…, 2)` writes the live values).
    pub fn write(&self, state: &mut SaveState) {
        let m = &mut state.mem;
        m.set_i32(at::AREA, self.area as i32);
        m.set_i32(at::FIELD, self.field);
        m.set_i32(at::TECH, self.tech);
        m.set_i32s(at::FIELDS, &self.fields);
        m.set_i32(at::FRONT, self.front);
    }

    /// `ResearchAtvaltTalalmanyra(t)` (exe 0x521f20) with the screen down (the top bar's quick-research pick): the
    /// category's `TT_*` (a middle-wheel one turns to the front, `ResearchOldForgato = forgastabla2[0]`), then the
    /// tech, its field and the category's remembered field.
    pub fn jump_to(&mut self, t: i32, rules: &ig2_rules::Rules) {
        let n = ((t - 1).max(0) / 16 + 1).clamp(1, 12) as usize;
        self.area = n;
        if rules.research_screen.wheel_area.contains(&(n as i32)) {
            self.front = n as i32;
        }
        self.tech = t;
        self.field = t - (n as i32 - 1) * 16;
        self.fields[n] = self.field;
    }
}

/// The screen's state (one peer's UI; the original keeps it in globals, `Terulet_Akt` and the selection are saved).
#[derive(Debug, Clone)]
pub struct Research {
    /// `ResearchTerulet` 1..=12, `Terulet_Akt[area]` (the remembered field per category, −1 none), `ResearchMezo`,
    /// `ResearchTalalmany`.
    pub area: usize,
    pub fields: [i32; 13],
    pub field: i32,
    pub tech: i32,
    /// `TalalmanyKivalasztva` picked tech 0xa2 since the window last looked (its `TutorialCheckEvent(8)`, exe
    /// 0x524fdb).
    pub picked_turret: bool,
    /// The middle wheel per slot (front = 0): category (`forgastabla2`), button id (`forgastabla3`), line start
    /// (`forgastabla4x/4y`); the mask colour of each slot stays (`forgastabla` is the inverse permutation).
    slot_area: [i32; 5],
    slot_kep: [i32; 5],
    /// `kezeloframe` → `kezeloframeto` (the wheel's node animation, frames 70 + 0..50), `Balraforgat`.
    turn: f32,
    turn_to: f32,
    turn_left: bool,
    /// `maxhossz[16]`: how many characters of each list row are out; the info lines' (`InfoSorok[i]+0xb4`).
    reveal: [f32; 16],
    info_reveal: [f32; 4],
    info_tech: i32,
    /// `ResearchTalalmany` as the frame's `ResearchGfx2D_2` reads it for the info lines (as `drawn_area`: a mouse
    /// press's tech a frame later) [obs: research-left.json t 400 and 510, the lines typed 1.4 characters a frame
    /// from the frame after the press].
    drawn_tech: i32,
    /// `reswireframe`, the ball's matrix (`FociMesh`), `AddZoom`, `TalalmanyDir`, `Kozelites`/`KozelitesDirection`,
    /// `ResearchTalalmanyForgas`, `reskeringeto`, `GombocTransparencyk` (the lab cones' fade).
    wire: f32,
    pub ball: M3,
    zoom: f32,
    dir: i32,
    near: [f32; 17],
    near_dir: [i32; 17],
    spin: [f32; 17],
    keringeto: f32,
    /// `res_xy` (2 on entry, +0.02 a frame): the info box's glass breathes with it (`resatlatszosag`).
    res_xy: f32,
    cones: [f32; 3],
    /// The category whose models are placed (`OldResearchTerulet` for `LoadTalalmanyVectorok`).
    placed: usize,
    /// The sounds this screen started since the owner last took them (re/screens/audio.md §6): `Res_Hangok` and the
    /// clicks of `PlaySoundGlobal`.
    pub sounds: Vec<SoundId>,
    /// `ButtonPressed`: an order is on its way (`MULTIGET_Research` clears it for the issuer).
    pub button_pressed: bool,
    /// `ForgatasAlatt`: the middle wheel turned this frame (its category hot-spots are off).
    turning: bool,
    /// `Double_Lenyomva`.
    pub urgent: bool,
    /// The 10 ms frames since the screen opened (the texts' and models' pace).
    pub frames: u64,
    /// `resframe` (0..=60: +0.4 a UI frame while opening, −0.72 while closing), `resKELLKINYITAS` (the screen opens
    /// and closes animated), `resKINYITVA` (open), `resBECSUKAS` (closing), `resELINDULT` (a frame was drawn),
    /// `Research_Kilep::VoltNulla` (frame 0 was drawn once), and a frame held for a capture comparison.
    pub frame: f32,
    pub animated: bool,
    pub opened: bool,
    pub closing: bool,
    started: bool,
    closed_once: bool,
    pub pin: Option<f32>,
    /// `oldEnterlenyomva`/`oldDlenyomva` (exe 0xDDFFEE/0xDDFFED): the down flag of the last key event the handler
    /// saw, whatever the key (§4).
    key_old: bool,
    /// `OldResearchTerulet` (the category of the last drawn frame) and whether the category's models show
    /// (`LoadTalalmanyVectorok` hides them, `UnHideMeglevoTalalmanyok` shows the known ones, `HideAllTalalmany`).
    old_area: usize,
    /// `ResearchTerulet` as the frame's `Research3DUpdate` reads it: the exe draws before `Research_Main`, so a
    /// category chosen by a mouse press (not a key) shows in the next frame's pass (taken at the end of
    /// [`Research::step`]) [obs: research-left.json, the running project 0x19 pressed at t 450, its `Res_Hangok[1]`
    /// at t 460].
    drawn_area: usize,
    models_shown: bool,
    /// Tooltip 0x11 (`CheckIfTalalmanyokFolottVan`): the model's tech and the pointer it was set at.
    model_tip: Option<(usize, [f32; 2])>,
    /// The ball while it flies in (`MoveObj(FociMesh, …)` from frame 30; hidden before) and the wire sphere's
    /// transparency (`resSetObjTransparencyH(ResWireGomb, …)`, kept once open).
    ball_at: V3,
    ball_shown: bool,
    wire_fade: f32,
    /// The press animation: `ResMelyikGombot` (−1 none), `ResGombNew`, `ResGombFrame` 0..=10,
    /// `ResGombFrameCsokkent` (going back up), `KeszALenyomas` (the last press is over: the wheel may turn), the
    /// middle wheel's button per slot (`forgastabla3`), and each button's last frame (`resSetGombFrame` leaves it).
    button: i32,
    button_new: bool,
    button_frame: i32,
    button_up: bool,
    press_done: bool,
    slot_button: [i32; 5],
    /// `forgastabla4x/4y`: where each slot's button lines point (rotated with the other slot tables).
    slot_line: [[i32; 2]; 5],
    pub pressed: Vec<(i32, i32)>,
    /// The tooltip (`UpdateFrame`) and the left button held.
    pub tip: crate::kit::Tooltip,
    pub held: bool,
    /// `resf`: the faders' shine, drift and hover glow (`FaderDesignFrissito` in `FRISSIT_Research2`).
    faders: crate::kit::FaderDesign,
    /// The buttons down now (bit 0 left, bit 1 right: the window's events), at the last two `Mouse_Lekerdez` polls
    /// (one a frame, after the frame is drawn), and the flags the last drawn frame read from them.
    pub buttons: u8,
    poll: [u8; 2],
    edge: Edge,
    /// The drags (§4.1): `resGyX/resGyY` (exe 0xDC4728/0xDC472C, the ball's turn a frame), `MouseOldX/Y`
    /// (0xDC538C/0xDDF31C), `ResearchGombMegfogva` (0xDDF321, the ball held) and `Research_Talalmany_Megfogva`
    /// (0xDC471D, the chosen model held).
    gy: [f32; 2],
    mouse_old: [i32; 2],
    ball_grip: bool,
    model_grip: bool,
    /// The chosen row's frame (`DesignKeret1`, asked for by every drawn frame while open, stepped every UI frame)
    /// and the list font's cell (`CharXMeret[2]`, `CharYMeret[2]`).
    pub(crate) design: crate::kit::DesignFrame,
    cell: [i32; 2],
    /// `ResearchGfx2D_1`: the opening's lasers and sparks, and whether this frame draws them (`resELINDULT`, not
    /// closing; the lasers while opening, the sparks unless at frame 60).
    lasers: crate::kit::Lasers,
    laser_on: bool,
    sparks_on: bool,
    /// The button lines (`ResDrawVonalak`, §5.6) and where they point: `ResLastGombX/Y` (−1 before the first press).
    lines: Lines,
    last_button: [i32; 2],
}

impl Research {
    /// `Research_Belep` on the state's saved selection. `animated` = `resKELLKINYITAS` (`KELLKINYITASDEFAULT`,
    /// unless coming back from the messages, help or options screens): the screen opens from `resframe` 0.
    pub fn open(state: &SaveState, rules: &ig2_rules::Rules, animated: bool) -> Self {
        Self::open_at(state, rules, animated, Place::read(state))
    }

    /// `Research_Belep` at the place the screen was left (or the quick-research pick moved it to).
    pub fn open_at(state: &SaveState, rules: &ig2_rules::Rules, animated: bool, place: Place) -> Self {
        let m = &state.mem;
        let en = en(state);
        let s = &rules.research_screen;
        let mut r = Self {
            area: place.area,
            fields: place.fields,
            field: place.field,
            tech: place.tech,
            picked_turret: false,
            slot_area: s.wheel_area,
            slot_kep: s.wheel_kep,
            turn: 0.0,
            turn_to: 0.0,
            turn_left: false,
            reveal: std::array::from_fn(|i| -((i % 8) as f32)),
            info_reveal: INFO_START,
            info_tech: -1,
            drawn_tech: -1,
            wire: 0.0,
            ball: scene::IDENTITY,
            zoom: ZOOM_MAX,
            dir: -1,
            near: [0.0; 17],
            near_dir: [0; 17],
            spin: [0.0; 17],
            keringeto: 0.0,
            res_xy: 2.0,
            cones: [1.0; 3],
            placed: 0,
            button_pressed: false,
            turning: false,
            urgent: m.u8(va::URGENT + en as u32) & 1 != 0,
            frames: 0,
            frame: if animated { 0.0 } else { 60.0 },
            animated,
            opened: !animated,
            closing: false,
            // `resELINDULT`: `Research_Belep`'s frame is drawn before the first UI tick, so the opening moves from tick 1
            // [obs: `resframe` 4.0 at tick 10, open at tick 150; sequence diff 2026-10-22].
            started: true,
            closed_once: false,
            pin: None,
            key_old: false,
            old_area: 0,
            drawn_area: 0,
            models_shown: false,
            model_tip: None,
            ball_at: BALL,
            ball_shown: !animated,
            wire_fade: 0.0,
            button: -1,
            button_new: false,
            button_frame: 0,
            button_up: false,
            press_done: true,
            slot_button: s.wheel_button,
            slot_line: s.wheel_line,
            pressed: Vec::new(),
            sounds: Vec::new(),
            tip: crate::kit::Tooltip::default(),
            held: false,
            faders: crate::kit::FaderDesign::new(1),
            buttons: 0,
            poll: [0; 2],
            edge: Edge::default(),
            gy: [0.0; 2],
            mouse_old: [0; 2],
            ball_grip: false,
            model_grip: false,
            design: Default::default(),
            cell: [rules.charset.width.get(2).copied().unwrap_or(6), rules.charset.height.get(2).copied().unwrap_or(7)],
            lasers: crate::kit::Lasers::default(),
            laser_on: false,
            sparks_on: false,
            lines: Lines::default(),
            last_button: [-1, -1],
        };
        // `kezeloframe = kezeloframeto = 0`, then `Forgotabla(ResearchOldForgato)`: the wheel turns to the saved front
        // category once the screen is open.
        r.turn_wheel(place.front);
        r.slot_kep = s.wheel_kep;
        // `Research_Belep` exe 0x523540: the opening's sound (none without the opening: from the options screen the
        // exe plays only the field's `Res_Hangok[1]`, 31 ticks in; FX1 trace `fkeys`).
        if animated {
            r.sounds.push(SoundId::new(Table::Res, 0));
        }
        r.button = -1;
        (r.old_area, r.drawn_area, r.drawn_tech) = (r.area, r.area, r.tech);
        // `SetDoubleTexture`: the urgent button starts pressed in.
        if r.urgent {
            r.set_button_frame(0xc9, 10);
        }
        let techs = state.tech();
        for a in 1..=12 {
            r.fix_field(&techs, en, a);
        }
        r.field = r.fields[r.area];
        r.placed = r.area;
        // `LoadTalalmanyVectorok(ResearchTerulet)`: the lines fly in.
        r.lines.load();
        r
    }

    /// `IgazitTeruletAkt(a)`: the remembered field moves on to the first tech the human knows (status ≥ 2), −1 if
    /// none is left.
    fn fix_field(&mut self, techs: &TechTable, en: usize, a: usize) {
        let mut f = self.fields[a];
        while f < 0x11 && techs.status(((a as i32 - 1) * 16 + f).clamp(0, 192) as usize, en) < 2 {
            f += 1;
        }
        self.fields[a] = if f == 0x11 { -1 } else { f };
    }

    /// `Forgotabla(n)` (exe 0x51e360): category `n` in the middle wheel turns to the front slot (`FUN_00525740`
    /// rotates the tables right `5 − s` times; the mask colours stay with the slots), the wheel animates by
    /// `(5 − s)·10` frames.
    fn turn_wheel(&mut self, n: i32) {
        let Some(s) = self.slot_area.iter().position(|&a| a == n) else { return };
        self.turn_left = s == 1 || s == 2;
        // `ResMelyikGombot = forgastabla3[s]`: the slot's button is pressed; `ResLastGombX/Y = forgastabla4x/4y[s]`.
        self.button = self.slot_button[s];
        self.last_button = self.slot_line[s];
        if s == 0 {
            return;
        }
        self.slot_area.rotate_right(5 - s);
        self.slot_button.rotate_right(5 - s);
        self.slot_line.rotate_right(5 - s);
        self.turn_to += ((5 - s) * 10) as f32;
        if self.turn_to >= 50.0 {
            self.turn_to -= 50.0;
        }
    }

    /// `TT_*` (ELF 0x5e9430..): category `n` (`ResearchTerulet`), its remembered field. The middle wheel's categories
    /// (1..=5) go through `Forgotabla` (exe 0x51e360), which clicks while the screen is open (`resKINYITVA`); the
    /// others set theirs silently (exe 0x524a50..) [obs: research-right.json and its left run, t 380 power, t 450
    /// the running civil project].
    fn choose_area(&mut self, n: usize) {
        if self.opened && (1..=5).contains(&n) {
            self.sounds.push(CLICK);
        }
        self.area = n;
        // The categories off the middle wheel press their own button (`TT_*`: 1 tanks and 3 turrets on the left
        // wheel, 0x12d..=0x130 civil, military, research, power on the right one, 0x6a = the middle wheel's SPEC).
        self.button = match n {
            6 => 0x12d,
            8 => 0x12e,
            7 => 0x12f,
            9 => 0x130,
            10 => 1,
            11 => 3,
            12 => 0x6a,
            _ => self.button,
        };
        // `TT_*` (exe 0x524a50..): the off-wheel buttons' `ResLastGombX/Y` (§5.6).
        if let Some(&(_, xy)) = OFF_WHEEL_LINES.iter().find(|(a, _)| *a == n) {
            self.last_button = xy;
        }
        self.button_new = true;
        self.turn_wheel(n as i32);
        self.field = self.fields[n];
        self.tech = self.field + (n as i32 - 1) * 16;
    }

    /// `TalalmanyKivalasztva(k)` (ELF 0x5e24a0 / exe 0x524f60): field `k` of the category; its model moves in front of
    /// the ball. On a press (`Mouse_Lenyom`) a model not on its way grabs it (`Research_Talalmany_Megfogva`); every call
    /// takes `MouseOld` from the pointer.
    fn choose_field(&mut self, k: i32) {
        if self.edge.press && self.near_dir[k.clamp(0, 16) as usize] == 0 {
            self.model_grip = true;
        }
        self.mouse_old = self.pointer();
        self.field = k;
        self.fields[self.area] = k;
        self.tech = k + (self.area as i32 - 1) * 16;
        self.picked_turret |= self.tech == 0xa2;
        let k = k.clamp(0, 16) as usize;
        if self.near_dir[k] != 1 {
            for d in self.near_dir.iter_mut() {
                if *d == 1 {
                    *d = -1;
                }
            }
            self.near_dir[k] = 1;
        }
    }

    /// The exe's own `resf` at a capture's start (`--faders`): its `rand()`-driven shine and drift, which the
    /// screen's own generator can't replay.
    pub fn set_faders(&mut self, f: crate::kit::FaderDesign) {
        self.faders = f;
    }

    /// The top bar's quick-research pick (`MOUSE_SelectScreens`, re/screens/hud.md §2.1): `ResearchAtvaltTalalmanyra(t)`,
    /// then, the screen showing t, its start button goes in (`ResMelyikGombot = 0xcc`, `ResGombNew`, `ButtonPressed`;
    /// no sound).
    pub fn quick_start(&mut self, t: i32) {
        self.jump_to(t);
        self.button_pressed = true;
        (self.button, self.button_new) = (0xcc, true);
    }

    /// The place the screen leaves at (`Research_Kilep`: the globals stay as they are).
    pub fn place(&self) -> Place {
        Place { area: self.area, field: self.field, tech: self.tech, fields: self.fields, front: self.slot_area[0] }
    }

    /// `ResearchAtvaltTalalmanyra(t)`: the running project's category and field.
    fn jump_to(&mut self, t: i32) {
        let n = ((t - 1).max(0) / 16 + 1).clamp(1, 12) as usize;
        self.choose_area(n);
        self.field = t - (n as i32 - 1) * 16;
        self.tech = t;
        self.fields[n] = self.field;
    }

    /// The hot-spot under `at` (`Mouse_Darabol` ELF 0x5b7a40: the mask picture's colour, else the rectangles; the
    /// lowest id of several), with `Research_MouseInit2`'s and `Research3DUpdate`'s on/off states.
    pub fn spot(&self, state: &SaveState, mask: &[u8], at: [f32; 2]) -> Option<u32> {
        self.spot_in(state, mask, at, false)
    }

    /// [`Research::spot`] with the hot-spots that only carry tooltips (the labs 0x13..=0x18 while their cone shows).
    pub fn tip_spot(&self, state: &SaveState, mask: &[u8], at: [f32; 2]) -> Option<u32> {
        self.spot_in(state, mask, at, true)
    }

    fn spot_in(&self, state: &SaveState, mask: &[u8], at: [f32; 2], tips: bool) -> Option<u32> {
        let (x, y) = (at[0].clamp(0.0, 639.0) as usize, at[1].clamp(0.0, 479.0) as usize);
        let c = mask.get(y * 640 + x).copied().unwrap_or(0);
        let en = en(state);
        let current = state.mem.i32(va::RESEARCHING + 4 * en as u32);
        // `Research3DUpdate`: the category hot-spots are off while the wheel turns, the models fly, a press isn't
        // over (but for the urgent button's) or the screen isn't open.
        let categories = !self.turning && self.dir == 0 && (self.press_done || self.button == 0xc9) && self.opened && !self.closing;
        let on = |id: u32| match id {
            1 => current > 0 || self.can_start(state),
            3 | 0x19 => current > 0,
            5..=0x10 => categories,
            0x12 => self.opened && !self.closing,
            _ => true,
        };
        let inside = |r: [i32; 4]| (r[0]..r[0] + r[2]).contains(&(x as i32)) && (r[1]..r[1] + r[3]).contains(&(y as i32));
        let mut hits: Vec<u32> = Vec::new();
        for &(id, kep, r) in STATIC_SPOTS {
            if (if kep == 0 { inside(r) } else { c == kep }) && on(id) {
                hits.push(id);
            }
        }
        for (k, &id) in WHEEL_SPOTS.iter().enumerate() {
            // Hot-spot `id` is category `WHEEL_AREA[k]`; its colour is the one of the slot that category is in.
            let area = rules_wheel_area(k);
            if let Some(slot) = self.slot_area.iter().position(|&a| a == area) {
                if c as i32 == self.slot_kep[slot] && on(id) {
                    hits.push(id);
                }
            }
        }
        if tips {
            for (i, &(id, r)) in LAB_SPOTS.iter().enumerate() {
                if inside(r) && self.cones[i % 3] == 0.0 {
                    hits.push(id);
                }
            }
        }
        hits.into_iter().min()
    }

    /// `ResearchStartStop`'s first branch: nothing runs, the shown tech is researchable (3) and the labs suffice
    /// (or the research cheat 0xB7727C is on).
    fn can_start(&self, state: &SaveState) -> bool {
        let en = en(state);
        let t = self.tech.clamp(0, 192) as usize;
        let techs = state.tech();
        if techs.status(t, en) != 3 {
            return false;
        }
        let labs = labs(state, en);
        state.mem.i32(va::RESEARCH_CHEAT) != 0 || techs.labs_needed(t).iter().enumerate().all(|(i, &n)| n as i32 <= labs[i + 1])
    }

    /// A press at virtual `at` (`left`: `Mouse_Bal`): `Research_Main` for the hot-spot under it, either button
    /// (exe 0x523c10 tests no button), else with the left one a list row (`ResearchGfx2D_2`) or a model
    /// (`CheckIfTalalmanyokFolottVan`, which `Research3DUpdate` runs while closing too).
    pub fn press(&mut self, state: &SaveState, rules: &ig2_rules::Rules, mask: &[u8], at: [f32; 2], models: &[(usize, [f32; 2])], left: bool) -> Vec<Act> {
        let mut out = Vec::new();
        if self.closing {
            if left {
                self.pick_model(at, models);
            }
            return out;
        }
        let en = en(state);
        match self.spot(state, mask, at) {
            Some(1) => out = self.start_stop(state, true),
            Some(4) => out = self.double(),
            Some(0x12) => {
                // The info box: every line out at once.
                self.info_reveal = [200.0; 4];
            }
            Some(0x19) if !self.button_pressed => {
                let current = state.mem.i32(va::RESEARCHING + 4 * en as u32);
                self.jump_to(current);
            }
            Some(id) => {
                if let Some(&(_, a)) = SPOT_AREA.iter().find(|&&(s, _)| s == id) {
                    self.choose_area(a);
                }
            }
            // A known tech's row is `ResearchGfx2D_2`'s, in the next frame's draw (`draw_frame`).
            None if left && self.list_row(en, &state.tech(), at).is_none() => self.pick_model(at, models),
            None => {}
        }
        let _ = rules;
        out
    }

    /// `CheckIfTalalmanyokFolottVan`'s pick (exe 0x51d2d0): the first model under `at` plays `PlaySoundGlobal(2)` and
    /// is chosen.
    fn pick_model(&mut self, at: [f32; 2], models: &[(usize, [f32; 2])]) {
        let first = self.models_under(at, models).next();
        if let Some(k) = first {
            self.sounds.push(SoundId::global(2));
            self.choose_field(k as i32);
        }
    }

    /// The models `CheckIfTalalmanyokFolottVan` (exe 0x51d2d0) finds at `at`, in `k` order: it runs while the screen
    /// is open (`resKINYITVA` 0xddf322, closing too) with `AddZoom` at its minimum, for the loaded and shown models
    /// (`+0x2a`, `+0x28`) not flying (`KozelitesDirection`); `models` = their projected centres, those with
    /// `z·1024 > −1050` ([`ResearchModels::centres`]). The distance in f32, its root in f64, `< 30` strict.
    fn models_under<'a>(&'a self, at: [f32; 2], models: &'a [(usize, [f32; 2])]) -> impl Iterator<Item = usize> + 'a {
        let on = (self.opened || self.closing) && self.zoom == ZOOM_MIN;
        models.iter().filter(move |(k, c)| {
            let (dx, dy) = (c[0] - at[0], c[1] - at[1]);
            on && self.near_dir[*k] == 0 && (((dx * dx + dy * dy) as f64).sqrt() as f32) < 30.0
        }).map(|&(k, _)| k)
    }

    /// `Mouse_Tipus = 1` at `at`: a list row (`ResearchGfx2D_2` exe 0x5221a0) or a model under it
    /// (`CheckIfTalalmanyokFolottVan` exe 0x51d2d0).
    pub fn hand(&self, state: &SaveState, at: [f32; 2], models: &[(usize, [f32; 2])]) -> bool {
        self.list_row(en(state), &state.tech(), at).is_some() || self.models_under(at, models).next().is_some()
    }

    /// The list row under `at` (`ResearchGfx2D_2`: rows of 21 cells, 10 lines apart, the human's known techs), only
    /// while open and not closing (`DAT_00ddf322 != 0 && DAT_00ddf323 == 0`, exe 0x5221a0).
    fn list_row(&self, en: usize, techs: &TechTable, at: [f32; 2]) -> Option<i32> {
        if !self.opened || self.closing {
            return None;
        }
        (0..16).find_map(|k| {
            let t = (self.area - 1) * 16 + k + 1;
            if techs.status(t.min(192), en) < 2 {
                return None;
            }
            let (x, y) = row_xy(k);
            let inside = at[0] >= x && at[0] < x + 6.0 * 21.0 && at[1] >= y && at[1] < y + 7.0 + 3.0;
            inside.then_some(k as i32 + 1)
        })
    }

    /// `ResearchKeyboardHandler` (ELF 0x5e9aa0 / exe 0x523020) on a key down: ↓/↑ the next/previous known field, →/←
    /// the other column (±8), Enter `ResearchStartStop` unless the handler's last event was a down too
    /// (`oldEnterlenyomva`: a held Enter's repeats fire again after any key up), PgUp/PgDn the previous/next category
    /// in `Terulet_Melyik`'s order (exe 0x919234; `remapKeyCodes` swaps the keys, the window maps them). The exe's D
    /// (vk 0x44, `ResearchDoubleGomb`) never arrives: `remapKeyCodes` queues no key down for letters
    /// (re/screens/input.md §1).
    pub fn key(&mut self, state: &SaveState, k: Key) -> Vec<Act> {
        let old = std::mem::replace(&mut self.key_old, true);
        let en = en(state);
        let techs = state.tech();
        let known = |f: i32| f > 0 && f < 0x11 && techs.status(((self.area as i32 - 1) * 16 + f) as usize, en) > 1;
        let f = self.fields[self.area];
        let mut moved = None;
        match k {
            Key::Down if f < 0x10 => moved = (f + 1..0x11).find(|&x| known(x)),
            Key::Up if f > 1 => moved = (1..f).rev().find(|&x| known(x)),
            Key::Right if f != -1 && f < 9 && known(f + 8) => moved = Some(f + 8),
            Key::Left if f > 8 && known(f - 8) => moved = Some(f - 8),
            Key::Enter if old => {}
            Key::Enter => return self.start_stop(state, false),
            Key::PageUp | Key::PageDown if !self.button_pressed => {
                let at = AREA_ORDER.iter().position(|&a| a == self.area).unwrap_or(0);
                let next = if k == Key::PageUp { (at + 11) % 12 } else { (at + 1) % 12 };
                // `TT_*` (`Forgotabla`'s click for the wheel's categories) [obs: research-fx9.json t 170, 180].
                self.choose_area(AREA_ORDER[next]);
            }
            _ => {}
        }
        if let Some(m) = moved {
            self.choose_field(m);
        }
        // Keys come through the event filter before the frame's draw: its pass sees them at once [obs:
        // research-left.json, PgUp at t 180, the reload's `Res_Hangok[1]` at t 249].
        (self.drawn_area, self.drawn_tech) = (self.area, self.tech);
        Vec::new()
    }

    /// Any other event the handler sees (`FUN_004db040` passes every key up, every character as a down, and the key
    /// downs it has no case for): only `oldEnterlenyomva` follows it.
    pub fn key_seen(&mut self, down: bool) {
        self.key_old = down;
    }

    /// `ResearchStartStop` (ELF 0x5e9190): start the shown tech when nothing runs and it can start, stop the running
    /// project otherwise.
    /// It clicks only on a press (`Mouse_Lenyom`): Enter is silent [obs: research-fx9.json t 210, 245].
    fn start_stop(&mut self, state: &SaveState, press: bool) -> Vec<Act> {
        let mut out = Vec::new();
        if self.button_pressed {
            return out;
        }
        let current = state.mem.i32(va::RESEARCHING + 4 * en(state) as u32);
        // `ResMelyikGombot = 0xcc`, `ResGombNew`: the start/stop button goes in and out.
        if current == 0 && self.can_start(state) {
            self.button_pressed = true;
            (self.button, self.button_new) = (0xcc, true);
            if press {
                self.sounds.push(CLICK);
            }
            out.push(Act::Send(Command::Research { order: ResearchCmd::Start { tech: self.tech } }));
        }
        if current > 0 {
            self.button_pressed = true;
            (self.button, self.button_new) = (0xcc, true);
            if press {
                self.sounds.push(CLICK);
            }
            out.push(Act::Send(Command::Research { order: ResearchCmd::Stop { tech: current } }));
        }
        out
    }

    /// `ResearchDoubleGomb` (ELF 0x5e93c0): the urgent button's look flips at once, the order toggles the flag.
    pub fn double(&mut self) -> Vec<Act> {
        if self.button_pressed {
            return Vec::new();
        }
        // `SetDoubleTexture` / `DoubleGombFelenged`: the button is pressed in (frame 10) or let out at once.
        self.set_button_frame(0xc9, if self.urgent { 0 } else { 10 });
        self.urgent = !self.urgent;
        self.sounds.push(CLICK);
        vec![Act::Send(Command::Research { order: ResearchCmd::Urgent })]
    }

    /// `resSetGombFrame(id, f)` (ELF 0x5e4360): the pose button `id` is left in (`MoveObj` of a wheel's child back by
    /// 30·f, or `SetFrame(f + 60)` of the info box's buttons); see [`ResearchModels::render`].
    fn set_button_frame(&mut self, id: i32, f: i32) {
        match self.pressed.iter_mut().find(|(b, _)| *b == id) {
            Some(p) => p.1 = f,
            None => self.pressed.push((id, f)),
        }
    }

    /// One 10 ms frame: `FRISSIT_Research2` (ELF 0x5eb780, the 100 Hz UI tick), then a drawn frame
    /// (`Research3DUpdate` ELF 0x5e4e10 / exe 0x51fd10 and `ResearchGfx2D_2`'s per-frame steps).
    pub fn step(&mut self, state: &SaveState) {
        self.frames += 1;
        let en = en(state);
        let techs = state.tech();
        // `FRISSIT_Research2`.
        self.lines.tick();
        // `ResearchTalalmanyForgas`: the held model doesn't turn by itself.
        for (k, s) in self.spin.iter_mut().enumerate() {
            if !(self.model_grip && k as i32 == self.field) {
                *s += 0.02;
            }
        }
        self.res_xy += 0.02;
        self.keringeto += 0.0006;
        self.faders.tick();
        // `Frissit2_Grafika`: the sparks, the chosen row's frame.
        self.lasers.tick();
        self.design.tick();
        if self.animated && !self.closing && self.started {
            self.frame = (self.frame + 0.4).min(60.0);
        }
        if self.animated && self.closing {
            self.frame = (self.frame - 0.72).max(0.0);
        }
        if let Some(f) = self.pin {
            // Held as the tracer holds the exe (`run_screen.py --res-frame`): the side lights' clock
            // `reskeringeto` at 0 and the glass's `resatlatszosag` at 0.85 (`res_xy` 0).
            (self.frame, self.keringeto, self.res_xy) = (f, 0.0, 0.0);
        }
        // The lab cones fade in (0.02 a frame) for the labs the shown tech needs, out for the others.
        let need = techs.labs_needed(self.tech.clamp(0, 192) as usize);
        for (c, &n) in self.cones.iter_mut().zip(&need) {
            *c = if n != 0 { (*c - 0.02).max(0.0) } else { (*c + 0.02).min(1.0) };
        }
        // The models fly out (`AddZoom` +0.01 a frame, +0.02 while closing).
        if self.opened && !self.closing && self.dir == 1 && self.zoom <= ZOOM_MAX {
            self.zoom += 0.01;
        }
        if self.closing && self.dir == 1 && self.zoom < ZOOM_MAX {
            self.zoom += 0.02;
        }
        self.draw_frame(en, &techs);
        self.button_pressed = false;
        // `Mouse_Lekerdez` after `UpdateFrame`: the next frame reads these.
        self.poll = [self.buttons, self.poll[0]];
        (self.drawn_area, self.drawn_tech) = (self.area, self.tech);
    }

    /// The pointer as `Mouse_x`/`Mouse_y` (whole virtual pixels, 0..639 × 0..479).
    fn pointer(&self) -> [i32; 2] {
        [self.tip.mouse[0].clamp(0.0, 639.0) as i32, self.tip.mouse[1].clamp(0.0, 479.0) as i32]
    }

    /// `GombfolottLenyomva` (ELF 0x5e2190, inlined in exe 0x51fd10): the right button pressed (no button down before)
    /// within 160 pixels of the ball's projected centre.
    fn ball_pressed(&self) -> bool {
        let p = self.pointer();
        let near = project(BALL).is_some_and(|c| ((c[0] - p[0] as f32).powi(2) + (c[1] - p[1] as f32).powi(2)).sqrt() < 160.0);
        near && self.edge.press && self.edge.buttons & 2 != 0
    }

    /// `Research3DUpdate`'s state steps and `ResearchGfx2D_2`'s.
    fn draw_frame(&mut self, en: usize, techs: &TechTable) {
        self.started = true;
        self.edge = Edge::of(self.poll);
        // `Research3DUpdate` (exe 0x51fd10): the ball's drag fades ×0.8 a frame; a release lets the model go.
        self.gy = self.gy.map(|g| g * 0.8);
        if self.edge.release {
            self.model_grip = false;
        }
        if self.opened && self.ball_pressed() {
            self.mouse_old = self.pointer();
        }
        // `ResearchGfx2D_2` (exe 0x5221a0, l.281-301): a left press (`Mouse_Lenyom && Bal`) on a known tech's row
        // clicks and chooses it; drawn before the frame's `_Main`, it sees a press a frame after the hot-spots do [obs:
        // research-left.json, the row at t 520 clicks at t 521].
        if self.edge.press && self.edge.buttons & 1 != 0 {
            let p = self.pointer();
            if let Some(k) = self.list_row(en, techs, [p[0] as f32, p[1] as f32]) {
                self.sounds.push(CLICK);
                self.choose_field(k);
            }
        }
        let open_or_closing = self.opened || self.closing;
        // A new category: the models fly out, are reloaded at `ADDZOOMMAX` and fly back in.
        if self.old_area != self.drawn_area {
            // `ResEltuntetVonalkak`.
            self.lines.hide();
            if self.dir == 0 {
                (self.dir, self.zoom) = (1, ZOOM_MIN);
            }
            if self.dir == -1 {
                self.dir = 1;
            }
        }
        self.old_area = self.drawn_area;
        if open_or_closing && self.dir == 1 && ZOOM_MAX < self.zoom {
            self.zoom = ZOOM_MAX;
            self.models_shown = false;
            if !self.closing {
                // `LoadTalalmanyVectorok` exe 0x51ee14, the screen open: `Res_Hangok[1]` while the wheel is within
                // 14 frames of its target, else `[2]` (f32 difference, f64 abs).
                if self.opened {
                    let near = (((self.turn - self.turn_to) as f64).abs() as f32) < 14.0;
                    self.sounds.push(SoundId::new(Table::Res, if near { 1 } else { 2 }));
                }
                self.near = [0.0; 17];
                self.near_dir = [0; 17];
                // ... and every loaded model's `ResearchTalalmanyForgas[k] = 0` [obs: FX17b, the models' turn at t 560
                // of `scripts/research.json` counts from the reload].
                self.spin = [0.0; 17];
                self.lines.load();
                self.placed = self.drawn_area;
                self.models_shown = true;
                self.dir = -1;
            }
        }
        if self.opened && !self.closing {
            for r in &mut self.reveal {
                if *r < 20.0 {
                    *r += 1.8;
                }
            }
        }
        if open_or_closing && self.dir == -1 {
            self.zoom -= 0.15;
            if self.zoom <= ZOOM_MIN {
                self.zoom = ZOOM_MIN;
                self.dir = 0;
                if self.field != 0 && self.field != -1 {
                    self.choose_field(self.field);
                }
            }
        }
        if self.opened && self.dir == 0 {
            self.models_shown = true;
        }
        if self.frame == 60.0 {
            self.wire += 0.25;
            if self.wire >= 10.0 {
                self.wire = 0.0;
            }
        }
        // Opening and closing: the screen counts as open at frame 60; the ball is hidden before frame 30 while the
        // wire sphere fades in (`1.35 − 2f/60`; the exe clamps below 0.0001 to 0 and above 0.99 to 1), then flies in
        // from `g = (60 − f)·200`: at `(sin(g/1000)·200·g/2000, cos(g/1000)·150·g/2000 − 35, −950 − g/10)`, turned
        // `RotateObj(sin(g/1000), 0, 0)` and 0.002 on (its transparency `g/12000 − 0.6` is always clamped to 0).
        if !self.opened {
            if self.frame >= 60.0 && !self.closing {
                self.opened = true;
            }
            if self.frame < 30.0 {
                self.ball_shown = false;
                let t = 1.35 - (self.frame as f64 * 2.0) / 60.0;
                self.wire_fade = if t < 0.0001 { 0.0 } else if t > 0.99 { 1.0 } else { t as f32 };
            } else {
                self.ball_shown = true;
                let g = (60.0 - self.frame) * 100.0 * 2.0;
                let (sn, cs) = (((g / 1000.0) as f64).sin(), ((g / 1000.0) as f64).cos());
                let h = (g / 2000.0) as f64;
                self.ball_at = [(sn * 200.0 * h) as f32, (cs * 150.0 * h + -35.0) as f32, -950.0 - g / 10.0];
                self.ball = scene::mul(&scene::axis_matrix([0.0, 1.0, 0.0], -0.002), &scene::create_matrix(sn as f32, 0.0, 0.0));
                self.wire_fade = 0.0;
            }
        }
        // `ResearchGfx2D_1`: while opening, lasers run round the side panels' faders (`research_fader_*`).
        self.laser_on = !self.closing && self.started && !self.opened && self.frame != 0.0 && self.frame != 60.0;
        self.sparks_on = !self.closing && self.started && self.frame != 60.0;
        if self.laser_on {
            // Not closing: the faders are whole, (3, 81, 135, 106) and (500, 81, 135, 106).
            // `DrawLaser(…, 0, 0x70, 0xe3, frame/60)` round both side panels, from the top corners (0, 0) and (639, 0).
            self.lasers.clear();
            for (rect, from) in [[3, 81, 0x87, 106], [500, 81, 0x87, 106]].into_iter().zip([[0, 0], [0x27f, 0]]) {
                self.lasers.shoot(rect, from, self.frame / 60.0, RESEARCH_LASER);
            }
        }
        // The press animation: a new press holds the wheel (`KeszALenyomas`) until its button is back out.
        if self.button_new {
            self.press_done = false;
        }
        self.turning = false;
        if self.opened && self.turn != self.turn_to && self.button_frame == 0 && self.press_done && self.dir != 1 {
            if self.turn_left {
                self.turn -= 2.0;
                if self.turn == -2.0 {
                    self.turn = 48.0;
                }
            } else {
                self.turn += 2.0;
                if self.turn == 50.0 {
                    self.turn = 0.0;
                }
            }
            self.turning = true;
        }
        if self.frame >= 60.0 {
            if self.button_new {
                self.button_new = false;
                // The urgent button stays in while urgent (`ResGombFrameCsokkent = !Double_Lenyomva`).
                self.button_up = self.button == 0xc9 && !self.urgent;
            }
            if self.button != -1 {
                if !self.button_up {
                    self.button_frame = (self.button_frame + 7).min(10);
                    self.set_button_frame(self.button, self.button_frame);
                    if self.button_frame == 10 && self.button != 0xc9 {
                        self.button_up = true;
                    }
                } else if self.button_frame != 0 {
                    self.button_frame -= 7;
                    if self.button_frame < 1 {
                        self.button_frame = 0;
                        self.press_done = true;
                    }
                    self.set_button_frame(self.button, self.button_frame);
                }
            }
        }
        // `KellMozgatni`: the chosen model comes forward by 80 a frame (to 700), the others go back.
        if self.opened {
            for k in 1..17 {
                if self.near_dir[k] == 1 && self.near[k] < 700.0 {
                    self.near[k] = (self.near[k] + 80.0).min(700.0);
                }
                // The held model turns with the pointer: `(MouseOldX − Mouse_x)/100` from where it was grabbed.
                if self.near_dir[k] == 1 && self.model_grip {
                    self.spin[k] = (self.mouse_old[0] - self.pointer()[0]) as f32 / 100.0;
                }
                if self.near_dir[k] == -1 && self.near[k] > 0.0 {
                    self.near[k] -= 80.0;
                    if self.near[k] <= 0.0 {
                        self.near[k] = 0.0;
                        self.near_dir[k] = 0;
                    }
                }
            }
            // The drag's turn (`matrixforgat(m, resGyX, resGyY)`), then the grip: a release drops it, a right press near
            // the ball takes it; held, the pointer's move a frame feeds the turn; free, the ball turns 0.002 a frame.
            self.ball = turn(self.gy, &self.ball);
            if self.edge.release {
                self.ball_grip = false;
            }
            if self.ball_pressed() {
                self.ball_grip = true;
            }
            if !self.model_grip && self.ball_grip {
                let p = self.pointer();
                for ((g, old), now) in self.gy.iter_mut().zip(self.mouse_old).zip(p) {
                    *g = *g * 0.9 + (old - now) as f32 / 30.0 * 0.1;
                }
                self.mouse_old = p;
            }
            if !self.ball_grip {
                self.ball = turn([0.002, 0.0], &self.ball);
            }
        }
        // `ResearchGfx2D_2`: a category without a remembered field takes the first known one; the info lines start
        // over (at `InfoSorok[i]+0xb8`: 0, −20, −40, −60) for a new tech and come out 1.4 characters a frame.
        if self.fields[self.area] == -1 {
            self.fix_field(techs, en, self.area);
            if self.fields[self.area] != -1 {
                self.field = self.fields[self.area];
                self.tech = self.field + (self.area as i32 - 1) * 16;
            }
        }
        if self.field != -1 && self.opened && !self.closing {
            // `DesignKeret1(x − 5, y − 2, 20 cells + 10, h + 4, 1)` around the chosen row.
            let (x, y) = row_xy((self.field - 1).clamp(0, 15) as usize);
            self.design.ask([x as i32 - 5, y as i32 - 2, self.cell[0] * 20 + 10, self.cell[1] + 4]);
        }
        if self.field != -1 {
            if self.info_tech != self.drawn_tech {
                self.info_tech = self.drawn_tech;
                self.info_reveal = INFO_START;
            }
            if self.opened {
                for r in &mut self.info_reveal {
                    *r += 1.4;
                }
            }
        }
    }

    /// A capture's moment: `resframe` held at `f` while opening (or closing, `resBECSUKAS` with `resKINYITVA`
    /// cleared as `Research_Kilep` leaves them).
    pub fn hold(&mut self, f: f32, closing: bool) {
        (self.pin, self.frame) = (Some(f), f);
        // `Frissit2_Grafika` runs through the screen change before the first drawn frame (see `hold_open`) [obs:
        // held at 60 the exe shows the row frame].
        for _ in 0..150 {
            self.design.tick();
        }
        if closing {
            self.closing = true;
            self.opened = false;
            self.dir = 1;
        }
    }

    /// The screen as the opening left it (`resframe` 60, open; the row frame's morph as after the opening's 150 UI
    /// frames: asked for on the very first frame it would never show).
    pub fn hold_open(&mut self) {
        self.frame = 60.0;
        self.opened = true;
        self.started = true;
        for _ in 0..150 {
            self.design.tick();
        }
    }

    /// `Research_Kilep` (ELF 0x5e8e70 / exe 0x5239b0): leaving the screen. Animated, it closes first
    /// (`resBECSUKAS`: the closed middle wheel instead of the turning one, the models fly out, `resframe` 60 → 0);
    /// returns whether the screen can go now. The original loops `UpdateFrame` + `FRISSITES` until frame 0 was drawn
    /// once.
    pub fn leave(&mut self) -> bool {
        if !self.animated {
            return true;
        }
        if !self.closing {
            // `Research_Kilep` exe 0x5239b0: the closing's sound (`resKELLKINYITAS`).
            self.sounds.push(SoundId::new(Table::Res, 3));
            self.closing = true;
            self.opened = false;
            if self.dir == 0 {
                (self.dir, self.zoom) = (1, ZOOM_MIN);
            }
            if self.dir == -1 {
                self.dir = 1;
            }
        }
        self.closed()
    }

    /// The closing has drawn frame 0 (`Research_Kilep::VoltNulla`) and one more frame.
    pub fn closed(&mut self) -> bool {
        if !self.closing || self.frame != 0.0 {
            return false;
        }
        if self.closed_once {
            return true;
        }
        self.closed_once = true;
        false
    }

    /// The hot-spots' tooltip once per drawn frame (`UpdateFrame` with `Research_MouseInit2`'s on/off states); `now`
    /// in ms. `models` as for [`Research::press`]; `texts` give the model tooltip's tech name ([`Research::tip_text`]).
    pub fn tooltip(&mut self, state: &SaveState, mask: &[u8], tips: &[Vec<u8>], texts: &crate::ui::UiTexts, models: &[(usize, [f32; 2])], now: f64) {
        self.model_tip_step(models);
        let mut under = self.tip_spot(state, mask, self.tip.mouse).unwrap_or(0);
        // `Mouse_Szoveg_Set(0x11, 0, x − 15, y − 15, 30, 30, ..)` at the pointer of the frame it was set; of several
        // hot-spots the lowest id wins.
        let inside = |c: [f32; 2], m: [f32; 2]| (c[0] - 15.0..c[0] + 15.0).contains(&m[0]) && (c[1] - 15.0..c[1] + 15.0).contains(&m[1]);
        if self.model_tip.is_some_and(|(_, c)| inside(c, self.tip.mouse)) && (under == 0 || under > MODEL_TIP) {
            under = MODEL_TIP;
        }
        let len = self.tip_text(tips, texts, under).len();
        self.tip.update(now, under, len, self.held);
    }

    /// `CheckIfTalalmanyokFolottVan`'s tooltip 0x11 (exe 0x51d2d0, once a 3D update while open): on over a model (the
    /// last one found sets the text, the tech's name), off when none is under the pointer unless a model flies
    /// (`Vankinntalalmany` exe 0x525280: it keeps what it was).
    fn model_tip_step(&mut self, models: &[(usize, [f32; 2])]) {
        if !(self.opened || self.closing) {
            return;
        }
        if let Some(k) = self.models_under(self.tip.mouse, models).last() {
            self.model_tip = Some(((self.placed.max(1) - 1) * 16 + k, self.tip.mouse));
        } else if self.near_dir.iter().all(|&d| d == 0) {
            self.model_tip = None;
        }
    }

    /// The text of the tooltip `id` shows: the hot-spots' `Research_MouseInit` texts, 0x11 the model's tech name.
    pub fn tip_text(&self, tips: &[Vec<u8>], texts: &crate::ui::UiTexts, id: u32) -> Vec<u8> {
        let t = match (id, self.model_tip) {
            (MODEL_TIP, Some((t, _))) => crate::hud::tech_name(texts, t),
            _ => tips.get(id as usize).cloned().unwrap_or_default(),
        };
        t.split(|&b| b == 0).next().unwrap_or_default().to_vec()
    }

    /// Where model `k` (1..=16) of the shown category sits and how big it is (`ElforgatTalalmanyok` ELF 0x5e3270):
    /// its orbit offset ×2.2 turned with the ball, pushed out by `sin(AddZoom)·5 + 1`, moved towards
    /// (0, −50, −1000) and grown ×4 as it comes forward.
    pub fn model_at(&self, orbit: V3, k: usize) -> (V3, f32) {
        let p = scene::apply(&self.ball, orbit.map(|x| x * 2.2));
        let out = (self.zoom as f64).sin() as f32 * 5.0 + 1.0;
        let c = p.map(|x| x * out);
        let n = self.near[k];
        if n != 0.0 {
            let f = n / 700.0;
            ([(1.0 - f) * c[0], (1.0 - f) * (c[1] - 35.0) - 50.0 * f, (1.0 - f) * (c[2] - 950.0) - 1000.0 * f], f * 4.0)
        } else {
            ([c[0], c[1] - 35.0, c[2] - 950.0], 1.0)
        }
    }

    pub fn wheel_frame(&self) -> f32 {
        self.turn + 70.0
    }
}

/// The keys the screen takes (`ResearchKeyboardHandler`'s virtual-key codes 0x28, 0x26, 0x27, 0x25, 0xd, 'D', 0x21,
/// 0x22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Down,
    Up,
    Right,
    Left,
    Enter,
    PageUp,
    PageDown,
}

/// What a frame reads from the last `Mouse_Lekerdez` polls (ELF 0x5b7720): `Mouse_Bal`/`Mouse_Jobb`, `Mouse_Lenyom`
/// (any button down, none at the poll before) and `Mouse_Felenged` (none down, one before).
#[derive(Debug, Clone, Copy, Default)]
struct Edge {
    buttons: u8,
    press: bool,
    release: bool,
}

impl Edge {
    fn of(poll: [u8; 2]) -> Self {
        Self { buttons: poll[0], press: poll[0] != 0 && poll[1] == 0, release: poll[0] == 0 && poll[1] != 0 }
    }
}

/// `matrixforgat(m, a, b)` (exe 0x44fbc0, ELF 0x4523b0): `m` turned by `√(a² + b²)` about the screen axis across
/// (a, b), in the client's matrix order (`a` alone turns about y, as the idle spin: −a in `axis_matrix`'s sense [obs:
/// FX17b, the exe's ball chain at t 250 of `scripts/research.json` is (0.9797, 0, −0.2006 | 0, 1, 0 | 0.2006, 0,
/// 0.9797), 101 idle steps after the opening]).
fn turn(g: [f32; 2], m: &scene::M3) -> scene::M3 {
    let angle = (g[0] * g[0] + g[1] * g[1]).sqrt();
    if angle == 0.0 {
        return *m;
    }
    scene::mul(&scene::axis_matrix([-g[1], g[0], 0.0], -angle), m)
}

/// `Terulet_Melyik` (exe 0x919234) inverted: the categories in the order PgUp/PgDn step through them.
const AREA_ORDER: [usize; 12] = [10, 11, 3, 1, 5, 4, 2, 12, 6, 9, 7, 8];

/// `forgastabla2i[k]`: the category of the middle wheel's hot-spot `WHEEL_SPOTS[k]` (radar, weapons, shields, bombs,
/// ships).
fn rules_wheel_area(k: usize) -> i32 {
    [3, 2, 4, 5, 1][k]
}

pub(crate) fn en(state: &SaveState) -> usize {
    state.mem.i32(va::EN).clamp(0, 8) as usize
}

/// `LaborSzint[e][1..=3]`.
pub(crate) fn labs(state: &SaveState, e: usize) -> [i32; 4] {
    std::array::from_fn(|i| state.mem.i32(va::LABS + 16 * e as u32 + 4 * i as u32))
}

/// List row `k`'s place: 0..=7 at x 10, 8..=15 at x 507 (`local_44 − 8`), 10 lines apart from y 86.
fn row_xy(k: usize) -> (f32, f32) {
    if k > 7 {
        (507.0, ((k - 8) * 10 + 0x56) as f32)
    } else {
        (10.0, (k * 10 + 0x56) as f32)
    }
}

/// `res_info.txt` (`LoadTalalmanyInfok` ELF 0x5e7df0): per tech a `;` header line, then three info lines.
pub fn tech_infos(install: &Install) -> Vec<[Vec<u8>; 3]> {
    let raw = install.read("dataraw/text/res_info.txt").unwrap_or_default();
    let lines: Vec<&[u8]> = raw.split(|&b| b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)).collect();
    let mut out = vec![[Vec::new(), Vec::new(), Vec::new()]];
    for rec in lines.chunks(4) {
        if out.len() >= 0xbe {
            break;
        }
        let get = |i: usize| rec.get(i).map(|l| l.to_vec()).unwrap_or_default();
        out.push([get(1), get(2), get(3)]);
    }
    out
}

/// The race's research time and money multipliers (`FajSpeckoKutatasIdoSzorzo/MoneySzorzo`).
pub(crate) fn race_mults(state: &SaveState, rules: &ig2_rules::Rules, en: usize) -> (f32, f32) {
    let race = state.mem.u8(at::RACES + en as u32) as usize;
    (rules.race_research_time_mult.get(race).copied().unwrap_or(1.0), rules.race_research_money_mult.get(race).copied().unwrap_or(1.0))
}

/// `ResearchGfx2D_2`'s days left and daily cost of tech `t` for the human: the labs over the need (+5, /5, f64),
/// doubled when urgent, the story bonus in SP, at most the work left, at least 1; days = left / speed / 50; the
/// cost per day = price × money mult × left / total, doubled when urgent.
fn days_and_cost(state: &SaveState, rules: &ig2_rules::Rules, techs: &TechTable, t: usize, en: usize) -> (u32, u32) {
    let (time_mult, money_mult) = race_mults(state, rules, en);
    let labs = labs(state, en);
    let need = techs.labs_needed(t);
    let urgent = state.mem.u8(va::URGENT + en as u32) & 1 != 0;
    let over = |i: usize| if need[i] != 0 { (labs[i + 1] - need[i] as i32).max(0) } else { 0 };
    let mut speed = ((over(0) as f64 + 5.0 + over(1) as f64 + over(2) as f64) / 5.0) as i32;
    if urgent {
        speed <<= 1;
    }
    if state.mem.u8(va::MULTIPLAYER) & 1 == 0 {
        speed = (speed as f64 * ((state.mem.i32(at::STORY_BONUS) as f64 + 100.0) / 100.0)) as i32;
    }
    let total = techs.base_research_time(t) as f32 * time_mult;
    let left = total - techs.research_time(t, en) as f32;
    let speed = (if left <= speed as f32 { left } else { speed as f32 } as i32).max(1);
    let days = ((left as i32) / speed / 0x32) as u32;
    let mut cost = (techs.research_cost(t) as f32 * money_mult * left / total) as u32;
    if urgent {
        cost <<= 1;
    }
    (days, cost)
}

/// The research lasers' colour (`DrawLaser(…, 0, 0x70, 0xe3, …)`).
const RESEARCH_LASER: [i32; 3] = [0, 0x70, 0xe3];

/// The off-wheel categories' `TT_*` handlers (ELF 0x5e94b0.., exe 0x524a50..): `ResLastGombX/Y` per category.
const OFF_WHEEL_LINES: [(usize, [i32; 2]); 7] = [
    (6, [0x1ec, 0x172]),
    (9, [0x1c0, 0x199]),
    (8, [0x218, 0x19e]),
    (7, [0x1ed, 0x1c8]),
    (12, [0x140, 0x19b]),
    (10, [0x96, 0x172]),
    (11, [0x96, 0x1ca]),
];

/// The button lines (re/screens/research.md §5.6): ten arrowheads circling the last pressed category button, each
/// with a timer `ResVonalkak[i]` (in over 50 UI ticks, held 60, out over 40), `ResVonalakNovelo` (the ring's turn,
/// restarting every 501 ticks) and `ResKellVonalTavolodas` (after a category's models load, the arrows fly in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lines {
    pub v: [f32; 10],
    pub turn: f32,
    pub fly: bool,
}

impl Default for Lines {
    fn default() -> Self {
        Self { v: [0.0; 10], turn: 0.0, fly: false }
    }
}

/// One `DrawArrowAbsolute(p0, p1, colour, 2.0)` of the lines: from `p0` to the tip `p1`, ARGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrow {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub colour: u32,
}

impl Lines {
    /// `ResInitVonalkak` (ELF 0x5e1460): 100, 105, …, 145; no fly-in.
    pub fn init(&mut self) {
        for (i, v) in self.v.iter_mut().enumerate() {
            *v = ((i as f32 as f64 / 10.0) * 50.0 + 100.0) as f32;
        }
        self.fly = false;
    }

    /// `ResEltuntetVonalkak` (ELF 0x5e14d0): out of sight for good (until the next start).
    pub fn hide(&mut self) {
        self.v = [100000.0; 10];
        self.fly = false;
    }

    /// `LoadTalalmanyVectorok`'s start: the ring from its first turn, flying in.
    pub fn load(&mut self) {
        self.turn = 0.0;
        self.init();
        self.fly = true;
    }

    /// `FRISSIT_Research2` (every UI tick): `ResFrissitVonalkak`, then the turn (a restart at 500).
    pub fn tick(&mut self) {
        for v in &mut self.v {
            *v = if *v <= 0.0 { 0.0 } else { *v - 0.5 };
        }
        if 500.0 <= self.turn {
            self.turn = 0.0;
            self.init();
        } else {
            self.turn += 1.0;
        }
    }

    /// `ResDrawVonalak(c)` (ELF 0x5e4910, exe 0x51f340): the arrows of the lines alive (`v < 100`), with the exe's
    /// f32/f64 steps (and its 3.1415).
    #[allow(clippy::approx_constant)]
    pub fn arrows(&self, c: [f32; 2]) -> Vec<Arrow> {
        let (cx, cy) = (c[0] * 639.0 / 639.0, c[1] * 479.0 / 479.0);
        let spin = ((self.turn as f64 / 500.0) * 3.1415 * 2.0) as f32;
        let mut out = Vec::new();
        for (i, &v) in self.v.iter().enumerate() {
            if v >= 100.0 || v.is_nan() {
                continue;
            }
            let out_by = if self.fly { ((if v <= 50.0 { 0.0 } else { v - 50.0 }) as f64 / 12.0) as f32 } else { 0.0 };
            let a = if v > 50.0 {
                ((50.0 - (v - 50.0)) as f64 * 2.56) as f32
            } else if v > 20.0 {
                128.0
            } else {
                let a = (v as f64 * 6.4) as f32;
                if a <= 0.0 { 0.001 } else { a }
            };
            let th = spin as f64 + (i as f32 / 10.0) as f64 * 3.1415 * 2.0;
            let sx = (th.sin() * 20.0) as f32 * 639.0 / 639.0;
            let sy = (th.cos() * 20.0) as f32 * 479.0 / 479.0;
            let p1 = [cx + sx * (out_by + 1.0), cy + sy * (out_by + 1.0)];
            let p0 = [(cx as f64 + sx as f64 * (out_by as f64 + 1.5)) as f32, (cy as f64 + sy as f64 * (out_by as f64 + 1.5)) as f32];
            out.push(Arrow { p0, p1, colour: (a as i32 as u32) << 24 | 0x73_5f5f });
        }
        out
    }
}

/// `DrawArrowAbsolute`'s head (ELF 0x6efa10, exe 0x593cb0) as a turned square of `nyilhegy`: the triangle tip `p1`
/// (uv 0.5, 1), base `p1 ± 2.5n − 10d` (uv (0, 0) / (1, 0)), `n = (−d.y, d.x)·2`, is the texture's square laid
/// 10 × 10 from the base to the tip; the shaft has no area (|p1 − p0| = 10) and is left out. Colour premultiplied
/// by its alpha (`polygons(…, 1)`: `byte·A >> 8`), added.
fn draw_arrow(f: &mut Frame, v: &crate::font::Virtual, tex: TexId, a: &Arrow) {
    let (dx, dy) = (a.p1[0] - a.p0[0], a.p1[1] - a.p0[1]);
    let len = ((dx * dx + dy * dy) as f64).sqrt() as f32;
    if len == 0.0 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let al = a.colour >> 24;
    let ch = |s: u32| ((((a.colour >> s) & 0xff) * al) >> 8) as f32 / 255.0;
    let centre = v.px(a.p1[0] - 5.0 * ux, a.p1[1] - 5.0 * uy);
    let mut q = Instance::new(kind::SPRITE, centre, [5.0 * v.k, 5.0 * v.k], [ch(16), ch(8), ch(0), 1.0]);
    // The texture's u runs along −n, its v along d.
    q.rot = (-ux).atan2(uy);
    f.push(tex, Blend::GammaAdditive, q);
}

/// The 2D pass under and over the models.
pub struct Texts<'a> {
    pub infos: &'a [[Vec<u8>; 3]],
    /// The button lines' arrowhead texture.
    pub arrow: Option<TexId>,
}

impl Research {
    /// The lasers and the faders over the models (`Research3DUpdate`'s `KirakFader` 0..=4, open: full height): the
    /// two side panels, their bottom bars, the running project's box; the side panels' scan bands and glows
    /// (re/screens/hud.md §1.2.1), drawn at once and so under the faders, which are queued.
    pub fn draw_faders(&mut self, f: &mut Frame, d: &Draw2d, state: &SaveState, glow: Option<TexId>) {
        use crate::kit::fader_design;
        self.lasers.draw(f, d.white, glow.unwrap_or(d.white), &d.v, self.laser_on, self.sparks_on);
        let l = (self.frame / 60.0) * 2.0 - 1.0;
        let (grow, fade) = if self.closing { (l, 1.0) } else { (1.0, l) };
        let (white, v) = (d.white, &d.v);
        // `research_fader_*`: closing, the side panels fold up to y 124 (height `l·106`).
        let (y, h) = (124.0 - grow * 43.0, grow * 106.0);
        let sides = [[3, y as i32, 135, h as i32], [500, y as i32, 135, h as i32]];
        // `GLOBALKELLDESIGN`: the scan bands at `fmod(clock, 1)` and `fmod(clock/2, 1)`, then the glows of faders 0
        // and 1, all in (0, 54, 90).
        let blink = fader_design::blink(d.time as f64 * 1000.0);
        let c = [0, 54, 90];
        let clock = self.faders.clock() as f64;
        fader_design::glow(f, white, v, sides[0], c, clock.rem_euclid(1.0) as f32, 0, blink);
        fader_design::glow(f, white, v, sides[1], c, (clock / 2.0).rem_euclid(1.0) as f32, 0, blink);
        for (i, r) in sides.into_iter().enumerate() {
            if let Some((t, style)) = self.faders.glow(i) {
                fader_design::glow(f, white, v, r, c, t, style, blink);
            }
        }
        // Past frame 30 only. Opening, the panels are whole and fade from opaque (`l = f/30 − 1`: fill
        // `(1 − l)·150 + 105`, edge `(1 − l)·205 + 50`).
        if self.frame <= 30.0 {
            return;
        }
        let (ft, et) = ((1.0 - fade) * 150.0 + 105.0, (1.0 - fade) * 205.0 + 50.0);
        // `KirakFader` (exe 0x4d3880): `(!Mouse_Bal && !Mouse_Jobb && !Mouse_Felenged) || Mouse_Lenyom`, the last poll's.
        let e = self.edge;
        let (at, free) = (self.tip.mouse, (e.buttons == 0 && !e.release) || e.press);
        let faders = &mut self.faders;
        let mut kirak = |f: &mut Frame, i: usize, r: [f32; 4]| {
            let s = faders.s(i);
            crate::kit::fader::kirak_fader(f, white, v, r, [0.0, 54.0 * s, 90.0 * s], ft, [0.0, 90.0 * s, 130.0 * s], et, faders.offset(i));
            faders.enter(i, r, at, free);
        };
        kirak(f, 0, [3.0, y, 135.0, h]);
        kirak(f, 1, [500.0, y, 135.0, h]);
        kirak(f, 2, [5.0, y + h - 15.0, 131.0, 13.0]);
        kirak(f, 3, [502.0, y + h - 15.0, 131.0, 13.0]);
        if state.mem.i32(va::RESEARCHING + 4 * en(state) as u32) != 0 {
            kirak(f, 4, [1.0, 26.0, 108.0, 42.0]);
        }
    }

    /// The models' texture, then `ResearchGfx2D_2`'s texts.
    /// The models' texture (`Update3D`, before the 2D pass: the faders and lasers draw over it [obs: closing, the
    /// side panels cover the closed wheel's pods]).
    pub fn draw_models(&self, f: &mut Frame, d: &Draw2d, models: TexId) {
        let v = &d.v;
        f.push(models, Blend::GammaPremul, Instance::new(kind::SPRITE, v.px(320.0, 240.0), crate::kit::Wide::new(v).half(v), [1.0; 4]));
    }

    pub fn draw_over(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, rules: &ig2_rules::Rules, t: &Texts) {
        let v = &d.v;
        let en = en(state);
        let techs = state.tech();
        // `ResearchGfx2D_2` starts with the button lines (`ResLastGombX ≠ −1 && resKINYITVA`).
        if let Some(tex) = t.arrow.filter(|_| self.last_button[0] != -1 && self.opened && !self.closing) {
            for a in self.lines.arrows([self.last_button[0] as f32, self.last_button[1] as f32]) {
                draw_arrow(f, v, tex, &a);
            }
        }
        // `ResearchGfx2D_2` writes only while the screen is open (and not closing).
        if self.field == -1 || !self.opened || self.closing {
            return;
        }
        let tech = self.tech.clamp(0, 192) as usize;
        // The info box: three lines of `res_info.txt`, then "Kell: " and the prerequisites (or the "none" text).
        // `GetString(4, 2, 1) + ": "` (exe 0x8541c8).
        let mut head = d.texts.get(4, 2, 1);
        head.extend_from_slice(b": ");
        let mut need = head.clone();
        let pre = techs.prerequisites(tech);
        for i in 0..3 {
            if pre[i] != 0 {
                need.extend_from_slice(&crate::hud::tech_name(d.texts, pre[i]));
                if (i == 0 && pre[0] != 0 && (pre[1] != 0 || pre[2] != 0)) || (i == 1 && pre[1] != 0 && pre[2] != 0) {
                    need.extend_from_slice(b", ");
                }
            }
        }
        if need == head {
            need.extend_from_slice(&d.texts.get(4, 2, 2));
        }
        let empty = Vec::new();
        let info = t.infos.get(tech);
        for i in 0..4 {
            let line = if i < 3 { info.map_or(&empty, |l| &l[i]) } else { &need };
            let shown = self.info_reveal[i];
            let y = (i as f32) * 13.0 + 28.0;
            if shown >= 1.0 {
                d.font.text(f, v, line, 175.0, y, shown as usize, 2, Align::Left, d.palette.get(2).copied().unwrap_or([1.0; 3]), None);
            }
            if shown < line.len() as f32 && shown > 0.0 {
                let w = d.font.advance(2);
                cursor(f, d, [175.0 + (shown as i32 - 1) as f32 * w, y, w, d.font.height(2)]);
            }
        }
        // The lists: the category's known techs in their status colour (5 owned 2, 4 researching 3, 3 researchable
        // 1, 2 seen 0x27), with the progress percent.
        let (time_mult, money_mult) = race_mults(state, rules, en);
        for k in 0..16 {
            let tk = (self.area - 1) * 16 + k + 1;
            let st = techs.status(tk.min(192), en);
            if st < 2 {
                continue;
            }
            let col = match st {
                5 => 2,
                4 => 3,
                3 => 1,
                _ => 0x27,
            };
            let (x, y) = row_xy(k);
            if self.reveal[k] >= 0.0 {
                let pct = ((techs.research_time(tk, en) * 100) as f32 / (techs.base_research_time(tk) as f32 * time_mult) + 0.5) as u32;
                let name = crate::hud::tech_name(d.texts, tk);
                let mut row = name.clone();
                row.resize(row.len().max(16), b' ');
                if pct == 0 || pct == 100 {
                    row.extend_from_slice(b"    ");
                } else {
                    row.extend_from_slice(format!(" {pct:2}%").as_bytes());
                }
                d.font.text(f, v, &row, x, y, self.reveal[k] as usize, 2, Align::Left, d.palette.get(col).copied().unwrap_or([1.0; 3]), None);
                if (self.reveal[k] as i32) < 0x14 && (self.reveal[k] as usize) < name.len() {
                    let w = d.font.advance(2);
                    cursor(f, d, [x + (self.reveal[k] as i32) as f32 * w, y, w, d.font.height(2)]);
                }
            }
        }
        // The chosen row's frame (`DesignKeret1`, palette colour 1).
        self.design.draw(f, d.white, v, d.palette.get(1).copied().unwrap_or([1.0; 3]));
        // The labs: the tech's need (red when the human's level is below it) and the human's levels.
        let s = &rules.research_screen;
        let labs = labs(state, en);
        let need = techs.labs_needed(tech);
        let status = techs.status(tech, en);
        for i in 1..4 {
            if need[i - 1] != 0 {
                if status > 0 {
                    let c = if labs[i] < need[i - 1] as i32 { 1 } else { 2 };
                    d.font.int(f, v, need[i - 1] as i64, (s.lab_x2[i] - 6) as f32, s.lab_y[i] as f32, 3, 2, Align::Field, d.palette.get(c).copied().unwrap_or([1.0; 3]));
                }
                d.font.int(f, v, labs[i] as i64, (s.lab_x[i] - 6) as f32, s.lab_y[i] as f32, 3, 2, Align::Field, d.palette.get(2).copied().unwrap_or([1.0; 3]));
            }
        }
        // The running project, pulsing (`KiirStrSzinSzorzo = (sin(reskeringeto·100) + 1)·20 + 40`).
        let pulse = (((self.keringeto * 100.0) as f64).sin() as f32 + 1.0) * 20.0 + 40.0;
        let c22 = d.palette.get(0x22).copied().unwrap_or([1.0; 3]);
        let current = state.mem.i32(va::RESEARCHING + 4 * en as u32);
        if current > 0 {
            let ct = current.clamp(0, 192) as usize;
            let name = crate::hud::tech_name(d.texts, ct);
            let pc = c22.map(|x| x * pulse / 100.0);
            d.font.text(f, v, &name, 4.0, 0x1e as f32, name.len(), 2, Align::Left, pc, None);
            let (days, cost) = days_and_cost(state, rules, &techs, ct, en);
            let s1 = printf_d(&d.texts.get(4, 2, 3), b":%d", cost);
            d.font.text(f, v, &s1, 4.0, 0x2a as f32, s1.len(), 2, Align::Left, pc, None);
            let s2 = printf_d(&d.texts.get(4, 2, 4), b": %d%", days);
            d.font.text(f, v, &s2, 4.0, 0x36 as f32, s2.len(), 2, Align::Left, pc, None);
        }
        let _ = money_mult;
        // The shown tech's days and daily cost under the panels, unless owned.
        if status < 5 {
            let (days, cost) = days_and_cost(state, rules, &techs, tech, en);
            let s1 = printf_d(&d.texts.get(4, 2, 5), b": %d%", days);
            d.font.text(f, v, &s1, 0x1fb as f32, 0xaf as f32, s1.len(), 2, Align::Left, c22, None);
            let s2 = printf_d(&d.texts.get(4, 2, 6), b": %d", cost);
            d.font.text(f, v, &s2, 10.0, 0xaf as f32, 0x10, 2, Align::Left, c22, None);
        }
    }
}

/// `sprintf(GetString(..) + suffix, n)`: the texts hold no format; `ResearchGfx2D_2` (exe 0x5221a0) appends `":%d"`
/// (0x854cf0, money left), `": %d%"` (0x8585a8, the days) or `": %d"` (0x854c78, the cost) with `CString::operator+`
/// (exe 0x4a92a0). The text's own bytes stay (its code page); MSVC's `sprintf` prints nothing for the lone trailing
/// `%` [obs: research-left.json t 510, Hungarian: "Idő kell: 100"].
fn printf_d(text: &[u8], suffix: &[u8], n: u32) -> Vec<u8> {
    let fmt = [text, suffix].concat();
    let fmt = fmt.strip_suffix(b"%").filter(|f| !f.ends_with(b"%")).unwrap_or(&fmt);
    crate::ui::sprintf(fmt, &[n as i64])
}

/// `FillToScreen(x, y, w, h, 0xc)`: the typing cursor.
fn cursor(f: &mut Frame, d: &Draw2d, r: [f32; 4]) {
    let c = d.palette.get(0xc).copied().unwrap_or([1.0; 3]);
    let v = &d.v;
    f.push(d.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0), [r[2] * v.k / 2.0, r[3] * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
}

/// The screen's models on the GPU side: the console posed at its integer frames (the opening 0..=60, the buttons'
/// 60..=70, the wheel's 70..=120), the ball, the categories' tech models, their textures, the hit mask, and the
/// texture they are drawn into.
pub struct ResearchModels {
    frames: HashMap<usize, Option<Model>>,
    ball: Option<Model>,
    techs: HashMap<usize, Option<Model>>,
    textures: HashMap<String, TexId>,
    white: TexId,
    target: Option<Target>,
    /// `RESE_MSK.RAW` (kep 0x58): 640 × 480 hot-spot colours.
    pub mask: Vec<u8>,
    /// The tech models' projected centres of the last frame (for clicks, the hand and tooltip 0x11): the shown ones
    /// with `z·1024 > −1050` (`MoveObj`'s units; `CheckIfTalalmanyokFolottVan` exe 0x51d34d), however faded.
    pub centres: Vec<(usize, [f32; 2])>,
    /// The lasers' glow (`laservago`, `InitFader1`: `textures/starmap/laservago.bmp`).
    pub glow: Option<TexId>,
    /// The button lines' arrowhead (`LoadNyilszar` exe 0x59d8f0: `textures/starmap/nyilhegy.bmp`).
    pub arrow: Option<TexId>,
}

/// The camera: `vx = 600·x/w + 320`, `vy = 600·y/w + 250` (w = −z), like the starmap's panels.
fn project(p: V3) -> Option<[f32; 2]> {
    let w = -p[2];
    (w > 1.0).then(|| [600.0 * p[0] / w + 320.0, 600.0 * p[1] / w + 250.0])
}

/// `resbck`'s children (`GetSubMeshid(ResMainMesh, …, i)`).
mod child {
    pub const LEFT_WHEEL: u16 = 10;
    pub const CLOSED_WHEEL: u16 = 11;
    pub const RIGHT_WHEEL: u16 = 12;
    pub const WHEEL: u16 = 13;
    pub const WIRE: u16 = 14;
    pub const INFO: u16 = 15;
    pub const RADAR: u16 = 16;
}

/// `resSetGombFrame`'s depth of a left/right wheel button: `MoveObj` puts its z at `base − 30·f` (v units).
fn button_base(id: i32) -> Option<f32> {
    Some(match id {
        1 => -6000.0,
        3 => -5300.0,
        0x12d => -3250.0,
        0x12e => -3060.0,
        0x12f => -2950.0,
        0x130 => -3260.0,
        _ => return None,
    })
}

impl ResearchModels {
    pub fn new(r: &mut Renderer, install: &Install) -> Self {
        let white = r.add_texture(&crate::assets::Image::white());
        let mask = Self::mask(install);
        let mut load = |name| install.resolve_texture("textures/starmap/", name).and_then(|p| crate::assets::Image::load(install, &p).ok()).map(|img| r.add_texture(&img));
        let (glow, arrow) = (load("laservago.bmp"), load("nyilhegy.bmp"));
        Self { frames: HashMap::new(), ball: None, techs: HashMap::new(), textures: HashMap::new(), white, target: None, mask, centres: Vec::new(), glow, arrow }
    }

    /// `RESE_MSK.RAW` (kep 0x58): a 32-byte `mhwanh` header (640 × 480), a 768-byte palette, then the pixels.
    pub fn mask(install: &Install) -> Vec<u8> {
        install.read("dataraw/screen/rese_msk.raw").ok().and_then(|b| b.get(800..800 + 640 * 480).map(|x| x.to_vec())).unwrap_or_default()
    }

    /// `resbck` posed at integer frame `f` (`SetFrame`).
    fn posed(&mut self, install: &Install, f: usize) -> Option<&Model> {
        self.frames
            .entry(f.min(120))
            .or_insert_with(|| Model::load_at(install, "vector/research/screen/", "resbck.xvb", "textures/research/screen/", Some(f.min(120))).map_err(|e| eprintln!("ig2-client: {e}")).ok())
            .as_ref()
    }

    /// `resbck` at `frame` (`SetFrame` keeps 1/256 steps between two keys).
    fn posed_at(&mut self, install: &Install, frame: f32) -> Option<Model> {
        let (f0, f1, t) = scene::frame_keys(frame, 121);
        let a = self.posed(install, f0)?.clone();
        Some(if t == 0.0 { a } else { a.lerp(self.posed(install, f1)?, t) })
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

    /// `LoadTalalmanyVectorok(c)`'s model for field `k` of category `c` (ELF 0x5e1520): the ships' hulls 5..=11 from
    /// `spacewar` with the race digit, the buildings' (categories 6..=9, the military's preview), the turrets' fall
    /// back to `TankTorony`, else `TTeruletek` from `talalmanyok`.
    fn tech_model(&mut self, install: &Install, rules: &ig2_rules::Rules, techs: &TechTable, race: u8, c: usize, k: usize) -> Option<&Model> {
        let idx = (c - 1) * 16 + k - 1;
        if let std::collections::hash_map::Entry::Vacant(slot) = self.techs.entry(idx) {
            let s = &rules.research_screen;
            let name = s.models.get(idx).cloned().unwrap_or_default();
            let (name, vdir, tdir) = if c == 1 && k > 4 {
                let mut n = name.into_bytes();
                if k < 12 && n.len() > 4 {
                    n[4] = b'0' + race;
                }
                (String::from_utf8_lossy(&n).into_owned(), "vector/spacewar/", "textures/spacewar/")
            } else if (6..=9).contains(&c) {
                let b = techs.byte(idx + 1, 0x27) as usize;
                let m = rules.building_models.get(b);
                let n = if b == 0 { String::new() } else if c == 8 { m.map(|m| m.preview.clone()).unwrap_or_default() } else { m.map(|m| m.model.clone()).unwrap_or_default() };
                (n, "vector/colony/", "textures/colony/")
            } else if c == 11 && name.is_empty() {
                (rules.unit_models.turrets.get(k - 1).map(|m| m.name.clone()).unwrap_or_default(), "vector/colony/", "textures/colony/")
            } else {
                (name, "vector/talalmanyok/", "textures/talalmanyok/")
            };
            let model = (!name.is_empty()).then(|| Model::load(install, vdir, &name, tdir).map_err(|e| eprintln!("ig2-client: {e}")).ok()).flatten();
            slot.insert(model);
        }
        self.techs.get(&idx).and_then(|m| m.as_ref())
    }

    /// The console as `Research3DUpdate` poses it: `resbck` at `resframe`, the middle wheel at `kezeloframe + 70`,
    /// the pressed buttons (`resSetGombFrame`), and while animated the info box `Group85` moved by
    /// `MoveObj(InitPos2 + (0, g·700, g·1400))` and turned `RotateObj(sin(2g)·2, 0, 0)`, `g = 1 − f/60` (the record translation
    /// replaced, re/screens/spy.md §2.2).
    fn console(&mut self, install: &Install, s: &Research) -> Option<Model> {
        let mut m = self.posed_at(install, s.frame)?;
        let wheel = s.wheel_frame().round().max(0.0) as usize;
        if let Some(w) = self.posed(install, wheel) {
            graft(&mut m, w, &[child::WHEEL]);
        }
        for &(id, f) in &s.pressed {
            match id {
                // The info box's buttons: their own node animation at `f + 60`.
                200..=299 => {
                    let k = (id - 200) as u16;
                    if let Some(p) = self.posed(install, (f + 60).max(0) as usize).cloned() {
                        graft(&mut m, &p, &[child::INFO, k]);
                    }
                }
                // The middle wheel's buttons back by `30·f` from where `InitKozepso` found them.
                100..=199 => {
                    let at = [child::WHEEL, (id - 100) as u16];
                    if let Some(n) = m.nodes.iter().find(|n| n.index == at) {
                        let r = n.record;
                        m.move_child(&at, Some([r[0], r[1], r[2] - 30.0 * f as f32 / 1024.0]), None);
                    }
                }
                _ => {
                    let wheel = if id < 100 { child::LEFT_WHEEL } else { child::RIGHT_WHEEL };
                    let at = [wheel, (id % 100) as u16];
                    if let (Some(base), Some(n)) = (button_base(id), m.nodes.iter().find(|n| n.index == at)) {
                        let r = n.record;
                        m.move_child(&at, Some([r[0], r[1], (base - 30.0 * f as f32) / 1024.0]), None);
                    }
                }
            }
        }
        if s.animated {
            let g = 1.0 - s.frame / 60.0;
            let to = [INFO_AT[0], INFO_AT[1] + g * 700.0, INFO_AT[2] + g * 1400.0].map(|v| v / 1024.0);
            let rot = scene::create_matrix(((g * 2.0) as f64).sin() as f32 * 2.0, 0.0, 0.0);
            m.move_child(&[child::INFO], Some(to), Some(&rot));
        }
        Some(m)
    }

    /// The wire sphere `Group66` at `SetFrame(resframe + reswireframe)`, while animated moved to `resInitPos1 + (0,
    /// g·200, −g·1000)` and turned `(sin(2g)·3, sin(2g)·2, 0)`.
    fn wire_sphere(&mut self, install: &Install, s: &Research) -> Option<Model> {
        let mut m = self.posed_at(install, s.frame + s.wire)?;
        if s.animated {
            let g = 1.0 - s.frame / 60.0;
            let to = [WIRE_AT[0], WIRE_AT[1] + g * 200.0, WIRE_AT[2] - g * 1000.0].map(|v| v / 1024.0);
            let a = ((g * 2.0) as f64).sin() as f32;
            m.move_child(&[child::WIRE], Some(to), Some(&scene::create_matrix(a * 3.0, a * 2.0, 0.0)));
        }
        Some(m)
    }

    /// Render the screen's models into its texture. `size` = the window in pixels.
    pub fn render(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, s: &Research, state: &SaveState, size: [f32; 2]) -> (Option<TexId>, Vec<Instance>) {
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let mut lines: Vec<Instance> = Vec::new();
        let v = Virtual::new(size);
        let en = en(state);
        let techs = state.tech();
        if self.ball.is_none() {
            self.ball = Model::load(install, "vector/research/screen/", "focilabda.xvb", "textures/research/screen/").map_err(|e| eprintln!("ig2-client: {e}")).ok();
        }
        // The console: `AddToScene(resbck, 0.95)` at (0, (int)resmozgato, −650), `resmozgato = (1 − f/60)·26 + 3`.
        // Hidden: the civil and military cones (3, 4, 8, 9); the closed middle wheel `Group115` while open, the
        // turning one `Group133` unless open (`InitKozepso`, `Research_Kilep`); the starmap console `Kezelo_radar`
        // once open; the wire sphere `Group66` is drawn as lines.
        // `Research3DInit`: ambient 0x202020, `CreateLightParalell(0x808080, (0, 0, −10))`; `resOldalsoFenyekBe`
        // (ELF 0x5e0f90) adds 0xffba00 and 0x009cff, moved every frame by `resMozgatMainFenyek` (inlined in
        // `Research3DUpdate`, exe 0x51fd10) to `(sin(20k)·300, cos(20k)·200, −950)` and the same at +1.57075 (`k =
        // reskeringeto`, exe 0xDDF6BC). Lit per vertex by the engine's model (`lighting`, research.md §2.1).
        let side = |a: f32| [((a as f64).sin() * 300.0) as f32, ((a as f64).cos() * 200.0) as f32, -950.0];
        let k = s.keringeto * 20.0;
        let lights = lighting::Lights::default()
            .ambient(0x202020)
            .parallel(0x808080, [0.0, 0.0, -10.0])
            .parallel(0xffba00, side(k))
            .parallel(0x009cff, side(k + 1.57075));
        let current = state.mem.i32(va::RESEARCHING + 4 * en as u32);
        let start_tex = if current >= 1 { "fejl_stop.bmp" } else if s.can_start(state) { "fejl_start.bmp" } else { "fejl_emp.bmp" };
        let double_tex = if s.urgent { "fejl_double_a.bmp" } else { "fejl_double.bmp" };
        let swap = |path: &str| -> Option<&str> {
            if path.contains("/Group80/") {
                Some(start_tex)
            } else if path.contains("/Group82/") {
                Some(double_tex)
            } else {
                None
            }
        };
        let cone_alpha = |path: &str| -> Option<f32> {
            for (g, i) in [("/Group103/", 0), ("/Group108/", 0), ("/Group104/", 1), ("/Group109/", 1), ("/Group105/", 2), ("/Group110/", 2)] {
                if path.starts_with(g) {
                    return Some(1.0 - s.cones[i]);
                }
            }
            None
        };
        let open = s.opened && !s.closing;
        let hidden = |p: &scene::Part| match p.index.first().copied() {
            Some(3 | 4 | 8 | 9 | child::WIRE) => true,
            Some(child::CLOSED_WHEEL) => open,
            Some(child::WHEEL) => !open,
            Some(child::RADAR) => !(s.closing || !s.opened),
            _ => false,
        };
        let mozgato = ((1.0 - s.frame / 60.0) * 26.0 + 3.0) as i32 as f32;
        let place = |m: &Model| Placement::add_to_scene(m.origin, 0.95).world((0.0, 0.0, 0.0), [0.0, mozgato, -650.0]);
        if let Some(m) = self.console(install, s) {
            let t = place(&m);
            for p in &m.parts {
                if hidden(p) {
                    continue;
                }
                // `KELLVILLOGAS`: the info box's glass (`Box01`, child 0xf → 0 → 0) at transparency
                // `resatlatszosag = sin(res_xy)·0.04 + 0.85`.
                // The flat blue glass faces the camera: drawn in 2D over its projected box (`fade_color` on the
                // gamma blend, as the original blends on the stored values).
                if p.path.starts_with("/Group85/Group84/") && s.frame != 60.0 {
                    // Flying in, tilted: as glass in the scene, unlit and blended as the 2D pass blends (on the
                    // stored values) [obs: frame 55 (9, 28, 46) = (48, 149, 242) × 0.19 over black].
                    let a = 1.0 - (((s.res_xy as f64).sin() * 0.04) as f32 + 0.85);
                    let tex = self.texture(r, install, p.texture.as_deref());
                    bake_glass(p, &t.matrix, t.translation, tex, a, &mut dynamic, &mut draws);
                    continue;
                }
                if p.path.starts_with("/Group85/Group84/") {
                    let a = 1.0 - (((s.res_xy as f64).sin() * 0.04) as f32 + 0.85);
                    let pts: Vec<[f32; 2]> = p.verts.iter().filter_map(|x| project(t.point(x.pos))).collect();
                    if let (Some(x0), Some(x1), Some(y0), Some(y1)) = (
                        pts.iter().map(|q| q[0]).reduce(f32::min),
                        pts.iter().map(|q| q[0]).reduce(f32::max),
                        pts.iter().map(|q| q[1]).reduce(f32::min),
                        pts.iter().map(|q| q[1]).reduce(f32::max),
                    ) {
                        let c = crate::kit::fader::fade_color(GLASS, a);
                        lines.insert(0, Instance::new(kind::SPRITE, v.px((x0 + x1) / 2.0, (y0 + y1) / 2.0), [(x1 - x0) * v.k / 2.0, (y1 - y0) * v.k / 2.0], c));
                    }
                    continue;
                }
                let alpha = cone_alpha(&p.path).unwrap_or(1.0);
                if alpha <= 0.0 {
                    continue;
                }
                let tex_path = swap(&p.path).and_then(|n| install.resolve_texture("textures/research/screen/", n)).or_else(|| p.texture.clone());
                let tex = self.texture(r, install, tex_path.as_deref());
                bake_part(p, &t.matrix, t.translation, tex, alpha, &lights, &mut dynamic, &mut draws);
            }
        }
        // The wire sphere: its edges in the texture's colour lit per vertex when opaque (`wirekirak`; the far side
        // takes the ambient light only [obs: (5, 18, 30)]), else `colour × opacity` (`resSetObjTransparencyH`, re/screens/spy.md §2.2); hidden once the
        // closing reached frame 0. Every triangle draws its edges [obs: at rest the far side's grid shows; culling
        // the back faces as the spy screen's sphere changes nothing measurable].
        let fade = s.wire_fade;
        if fade < 1.0 && !(s.closing && s.frame == 0.0) {
            if let Some(m) = self.wire_sphere(install, s) {
                let t = place(&m);
                let c = if fade < 0.0001 { Ink::Lit(WIRE_INK, &lights) } else { Ink::Flat(rgb255([40.0, 143.0, 239.0].map(|x| x * (1.0 - fade)))) };
                for p in m.below(&[child::WIRE]) {
                    wire(&mut lines, &v, p, &t.matrix, t.translation, c, false);
                }
            }
        }
        // The ball: `AddToScene(focilabda, 0.22)`, wireframe (property 0x40), from frame 30 (at (0, −35, −950) once
        // open), turning. Every edge draws: the far side's in the ambient colour [obs: FX17b, the exe's ball vertices
        // are white (all four lights saturate) or 0x202020, and its far edges show dim].
        if let (Some(m), true) = (self.ball.clone(), s.ball_shown || s.opened) {
            let pl = Placement::add_to_scene(m.origin, 0.22);
            let mm = scene::mul(&s.ball, &pl.matrix).map(|x| x * 1024.0);
            let at = if s.opened { BALL } else { s.ball_at };
            for p in &m.parts {
                wire(&mut lines, &v, p, &mm, at, Ink::Lit(BALL_INK, &lights), false);
            }
        }
        // The category's tech models (status ≥ 2): owned ones solid, the others as wireframes in their light's
        // colour (`FenyHozzarendel`: seen (0, 32, 32), researchable (0, 64, 64), researching (64, 64, 0)); hidden
        // until the zoom-in after the opening ends (`LoadTalalmanyVectorok` hides them, `UnHideMeglevoTalalmanyok`).
        // Flying out after a new category they still draw, fading with `AddZoom` [obs: research-left.json t 400, the
        // civil models 20 ticks after the press on category 9].
        self.centres.clear();
        let race = state.mem.u8(at::RACES + en as u32);
        let sr = &rules.research_screen;
        if s.models_shown {
            for k in 1..17 {
                let t = (s.placed - 1) * 16 + k;
                let st = techs.status(t.min(192), en);
                if st < 2 {
                    continue;
                }
                let orbit = sr.orbit.get(k).copied().unwrap_or([0.0; 3]);
                let size_mult = rules.research_size_mult.get(t - 1).copied().unwrap_or(1.0);
                let Some(m) = self.tech_model(install, rules, &techs, race, s.placed, k).cloned() else { continue };
                let (at, grow) = s.model_at(orbit, k);
                if at[2] > -1050.0 {
                    if let Some(c) = project(at) {
                        self.centres.push((k, c));
                    }
                }
                // `SetTTransp` (exe 0x5248a0): the depth fades the models behind the ball's centre and hides the far ones.
                let tr = item_transparency(at[2], s.zoom);
                if tr > 0.99 {
                    continue;
                }
                let opacity = lighting::float2int(255.0 - tr * 255.0) as f32;
                let pl = Placement::add_to_scene(m.origin, size_mult * 0.04).scaled([grow; 3]);
                // `ForgatObjectek`: `RotateObj(0, 0, spin)` then `RotateObj2((1, 0, 0), 1.3)`.
                let rot = scene::mul(&scene::axis_matrix([1.0, 0.0, 0.0], 1.3), &scene::create_matrix(0.0, 0.0, s.spin[k]));
                let mm = scene::mul(&rot, &pl.matrix).map(|x| x * 1024.0);
                if st == 5 {
                    // Property 8: the opacity goes to alpha.
                    for p in &m.parts {
                        let tex = self.texture(r, install, p.texture.as_deref());
                        bake_part(p, &mm, at, tex, opacity / 255.0, &lights, &mut dynamic, &mut draws);
                    }
                } else {
                    // `UnHideMeglevoTalalmanyok`: properties 0x40 | 0x800, not 8, and the model's own ambient light
                    // (`FenyHozzarendel`, exe 0x525060) besides the screen's; each edge end takes its lit colour, which
                    // (without property 8) the opacity scales (`>> 8`, research.md §2.1), and no texture colour [obs:
                    // FX17b, the exe's edges are white where `fenyekszamolasa` gave 0xfbfbfb and teal where 0x5e5e1f;
                    // a texture's colour made them pink and dim, ball region 5.8 → 3.3 at t 250].
                    let own = match st {
                        2 => 0x202000,
                        3 => 0x404000,
                        _ => 0x004040,
                    };
                    let item_lights = lights.clone().ambient(own);
                    for p in &m.parts {
                        wire(&mut lines, &v, p, &mm, at, Ink::Shade(&item_lights, opacity / 256.0), false);
                    }
                }
            }
        }
        if dynamic.is_empty() {
            return (None, lines);
        }
        let wide = crate::kit::Wide::new(&crate::font::Virtual::new(size));
        let px = wide.px;
        let light = scene::normalise(LIGHT);
        // Depth from w = NEAR to 2400 [obs: closing, the info box's glass passes close to the camera and still shows].
        const NEAR: f32 = 10.0;
        let (a, b) = (1.0 / (1.0 / NEAR - 1.0 / 2400.0) / NEAR, 1.0 / (1.0 / NEAR - 1.0 / 2400.0));
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
                // Unlit here: `bake_part` lights the vertices.
                ambient: [LIT[0], LIT[1], LIT[2], 0.0],
                sun: [0.0; 4],
                fog: [0.0; 4],
                fog_range: [1e9, 2e9, 0.0, 1e9],
                flash: [[0.0; 4]; 2],
            },
            draws,
            dynamic,
        });
        let target = match &mut self.target {
            Some(t) => t,
            None => self.target.insert(r.target(px)),
        };
        r.draw_into(&f, target, px, 0.0);
        (Some(target.tex), lines)
    }
}

/// Copy the parts below child `index` from `from` (the same model at another frame: the same parts in the same
/// order) into `m`: that child's own frame (`SetFrame` on a sub-mesh).
fn graft(m: &mut Model, from: &Model, index: &[u16]) {
    for (p, q) in m.parts.iter_mut().zip(&from.parts) {
        if p.index.starts_with(index) && p.verts.len() == q.verts.len() {
            p.verts.clone_from(&q.verts);
        }
    }
}

/// A colour (0..255) as a tint of the 2D pass ([`Blend::GammaAdditive`]: `line2d` blends on the stored values).
fn rgb255(c: [f32; 3]) -> [f32; 4] {
    [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, 1.0]
}

/// How a wireframe's edges are coloured: one colour, or (`Lit`) an opaque wireframe's texture colour lit by the
/// screen's lights.
#[derive(Clone, Copy)]
enum Ink<'a> {
    Flat([f32; 4]),
    /// The texture's colour lit per vertex (`wirekirak`: each line end takes its vertex's colour).
    Lit([f32; 3], &'a lighting::Lights),
    /// The lit colour per vertex times a factor (the opacity of an object without property 8), untextured.
    Shade(&'a lighting::Lights, f32),
}

/// `SetTTransp` (exe 0x5248a0, every tech model at each `ElforgatTalalmanyok` step, exe 0x51d600) from the camera
/// depth `z` of the model's `MoveObj` point: 0.01 in front of the ball's centre (z > −950), rising to 1 at z = −1050,
/// 1 behind; plus `AddZoom / ADDZOOMMAX`, at most 1. Above 0.99 the model is hidden (`HideAlboxok`), else shown at
/// that transparency (below 0.0001 → 0, above 0.99 → 1). The ELF's version (0x5e2af0) adds to the old transparency
/// instead; the exe's is the one that runs [obs: opacity bytes 252 in front, 128 for the chosen model at z −1000].
fn item_transparency(z: f32, zoom: f32) -> f32 {
    let depth = if z > -950.0 {
        0.01
    } else if z >= -1050.0 {
        -(z + 950.0) / 100.0
    } else {
        1.0
    };
    let t = (depth + zoom / ZOOM_MAX).min(1.0);
    if t < 0.0001 {
        0.0
    } else if t > 0.99 {
        1.0
    } else {
        t
    }
}

/// A part as a wireframe (`wirekirak` ELF 0x6083f0: the triangle edges its mask keeps), projected to lines; `cull`
/// keeps only the triangles facing the camera (the engine's back-face test, re/screens/spy.md §2.2). Edges crossing
/// the camera plane are clipped to it [obs: the opening's wire sphere passes the camera, its near cells cross the
/// screen]. The engine draws them as aliased GL lines one output pixel wide at every window size [obs: 1 px at 800 × 600,
/// 1920 × 1080 and 3840 × 2130, re/screens/widescreen.md §6].
fn wire(out: &mut Vec<Instance>, v: &Virtual, p: &scene::Part, m: &M3, at: V3, ink: Ink, cull: bool) {
    // An edge two faces share draws once: the second line lies at the same depth and fails the engine's depth test
    // [obs: FX17b, the exe's ball edges are one even colour, not doubled].
    let mut drawn = std::collections::HashSet::new();
    let key = |a: V3| a.map(f32::to_bits);
    for (ti, tri) in p.verts.chunks_exact(3).enumerate() {
        let mask = p.edges.get(ti).copied().unwrap_or(7);
        let w: [V3; 3] = std::array::from_fn(|i| {
            let w = scene::apply(m, tri[i].pos);
            [w[0] + at[0], w[1] + at[1], w[2] + at[2]]
        });
        if cull && faces_away(&w) {
            continue;
        }
        // The ends' colours (one line instance takes their mean).
        let ends: [[f32; 3]; 3] = std::array::from_fn(|i| match ink {
            Ink::Flat(c) => [c[0], c[1], c[2]],
            Ink::Lit(tex, lights) => {
                let l = lights.shade(lighting::normal(m, tri[i].lnormal));
                std::array::from_fn(|k| (tex[k] * l[k] as f32 / 255.0).floor() / 255.0)
            }
            Ink::Shade(lights, f) => {
                let l = lights.shade(lighting::normal(m, tri[i].lnormal));
                std::array::from_fn(|k| (l[k] as f32 * f).floor() / 255.0)
            }
        });
        for (bit, (i, j)) in [(1u8, (2, 0)), (2, (0, 1)), (4, (1, 2))] {
            let (ka, kb) = (key(tri[i].pos), key(tri[j].pos));
            if mask & bit != 0 && drawn.insert(if ka <= kb { (ka, kb) } else { (kb, ka) }) {
                let c = match ink {
                    Ink::Flat(c) => c,
                    Ink::Lit(..) | Ink::Shade(..) => [(ends[i][0] + ends[j][0]) / 2.0, (ends[i][1] + ends[j][1]) / 2.0, (ends[i][2] + ends[j][2]) / 2.0, 1.0],
                };
                if let Some((a, b)) = clip_near(w[i], w[j]) {
                    out.push(Instance::hard_line(v.px(a[0], a[1]), v.px(b[0], b[1]), c));
                }
            }
        }
    }
}

/// The segment `a`–`b` projected, cut where it passes the plane `w = 1` before the camera.
fn clip_near(a: V3, b: V3) -> Option<([f32; 2], [f32; 2])> {
    let (a2, b2) = cut_near(a, b)?;
    Some((project(a2)?, project(b2)?))
}

/// The segment `a`–`b` (camera space) cut where it passes the plane `w = 1` before the camera; `None` when it lies
/// wholly in front of it.
pub(crate) fn cut_near(a: V3, b: V3) -> Option<(V3, V3)> {
    const NEAR: f32 = 1.0001;
    let (wa, wb) = (-a[2], -b[2]);
    if wa < NEAR && wb < NEAR {
        return None;
    }
    let cut = |p: V3, q: V3, wp: f32, wq: f32| {
        let t = (NEAR - wp) / (wq - wp);
        [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, -NEAR]
    };
    let a2 = if wa < NEAR { cut(a, b, wa, wb) } else { a };
    let b2 = if wb < NEAR { cut(b, a, wb, wa) } else { b };
    Some((a2, b2))
}

fn normal(w: &[V3; 3]) -> V3 {
    let (u, t) = (sub(w[1], w[0]), sub(w[2], w[0]));
    [u[1] * t[2] - u[2] * t[1], u[2] * t[0] - u[0] * t[2], u[0] * t[1] - u[1] * t[0]]
}

/// The engine's back-face test: the triangle's winding normal points away from the camera at the origin.
pub(crate) fn faces_away(w: &[V3; 3]) -> bool {
    let n = normal(w);
    n[0] * w[0][0] + n[1] * w[0][1] + n[2] * w[0][2] > 0.0
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Bake one camera-fixed part at `at` with matrix `m` (`alpha` < 1 draws it as glass). The engine culls the
/// triangles facing away [obs: mid-opening the lab counters and the starmap console turn their backs to the camera
/// and vanish in the exe's captures; frame 12 labs 6.9 → 0.0].
#[allow(clippy::too_many_arguments)]
fn bake_part(p: &scene::Part, m: &M3, at: V3, tex: TexId, alpha: f32, lights: &lighting::Lights, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let start = out.len() as u32;
    let a = p.alpha(alpha);
    for tri in p.verts.chunks_exact(3) {
        let w: [V3; 3] = std::array::from_fn(|i| {
            let q = scene::apply(m, tri[i].pos);
            [q[0] + at[0], q[1] + at[1], q[2] + at[2]]
        });
        if faces_away(&w) {
            continue;
        }
        for (x, q) in tri.iter().zip(w) {
            let n = scene::normalise(scene::apply(m, x.normal));
            // Lit per vertex by the engine's model (`lighting`, research.md §2.1); transparent parts are lit too
            // (property 8: the colour lit, the opacity in alpha). The colour bytes multiply the texture in the gamma
            // pass (re/screens/render.md §2).
            let rgb = lights.rgb(lighting::normal(m, x.lnormal));
            out.push(MeshVertex { pos: q, normal: n, uv: x.uv, color: [rgb[0], rgb[1], rgb[2], a] });
        }
    }
    draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: p.material_glass(alpha) });
}

/// Bake glass at opacity `a`: unlit (the normal along the light, the tint undoing `LIT`), blended in the gamma pass
/// and laid over the frame premultiplied ([`Blend::GammaPremul`]): the exe's texture colour × `a` over black (the
/// info box's [`GLASS`]).
#[allow(clippy::too_many_arguments)]
fn bake_glass(p: &scene::Part, m: &M3, at: V3, tex: TexId, a: f32, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let tint: [f32; 3] = std::array::from_fn(|k| 1.0 / LIT[k]);
    let start = out.len() as u32;
    let normal = scene::normalise(LIGHT);
    for tri in p.verts.chunks_exact(3) {
        let w: [V3; 3] = std::array::from_fn(|i| {
            let q = scene::apply(m, tri[i].pos);
            [q[0] + at[0], q[1] + at[1], q[2] + at[2]]
        });
        for (x, q) in tri.iter().zip(w) {
            out.push(MeshVertex { pos: q, normal, uv: x.uv, color: [tint[0], tint[1], tint[2], a] });
        }
    }
    draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SetTTransp` (exe 0x5248a0) against the opacity bytes the engine drew with (`fenyekszamolasa`'s argument,
    /// `probes/research_items.js` on `scripts/research.json`, t 250/400): 252 for the models in front, 128 for the
    /// chosen one pulled to z −1000; hidden behind −1050; the zoom-out adds `AddZoom / ADDZOOMMAX`.
    #[test]
    fn item_transparency_follows_depth() {
        let byte = |z: f32, zoom: f32| lighting::float2int(255.0 - item_transparency(z, zoom) * 255.0);
        assert_eq!(byte(-900.0, 0.0), 252);
        assert_eq!(byte(-1000.0, 0.0), 128);
        assert_eq!(item_transparency(-1049.5, 0.0), 1.0);
        assert!(item_transparency(-1100.0, 0.0) > 0.99);
        assert_eq!(item_transparency(-800.0, ZOOM_MAX), 1.0);
        assert!((item_transparency(-800.0, ZOOM_MAX / 2.0) - 0.51).abs() < 1e-6);
    }

    fn screen() -> Research {
        Research {
            area: 3,
            fields: [1; 13],
            field: 1,
            tech: 33,
            picked_turret: false,
            slot_area: [3, 2, 4, 5, 1],
            slot_kep: [9, 16, 11, 5, 12],
            turn: 0.0,
            turn_to: 0.0,
            turn_left: false,
            reveal: [0.0; 16],
            info_reveal: [0.0; 4],
            info_tech: -1,
            drawn_tech: -1,
            wire: 0.0,
            ball: scene::IDENTITY,
            zoom: 0.0,
            dir: 0,
            near: [0.0; 17],
            near_dir: [0; 17],
            spin: [0.0; 17],
            keringeto: 0.0,
            res_xy: 2.0,
            cones: [1.0; 3],
            placed: 3,
            button_pressed: false,
            turning: false,
            urgent: false,
            frames: 0,
            frame: 60.0,
            animated: true,
            opened: true,
            closing: false,
            started: true,
            closed_once: false,
            pin: None,
            key_old: false,
            old_area: 3,
            drawn_area: 3,
            models_shown: true,
            model_tip: None,
            ball_at: BALL,
            ball_shown: true,
            wire_fade: 0.0,
            button: -1,
            button_new: false,
            button_frame: 0,
            button_up: false,
            press_done: true,
            slot_button: [100, 101, 102, 103, 104],
            slot_line: [[322, 353]; 5],
            pressed: Vec::new(),
            sounds: Vec::new(),
            tip: crate::kit::Tooltip::default(),
            held: false,
            faders: crate::kit::FaderDesign::new(1),
            buttons: 0,
            poll: [0; 2],
            edge: Edge::default(),
            gy: [0.0; 2],
            mouse_old: [0; 2],
            ball_grip: false,
            model_grip: false,
            design: Default::default(),
            cell: [6, 7],
            lasers: crate::kit::Lasers::default(),
            laser_on: false,
            sparks_on: false,
            lines: Lines::default(),
            last_button: [-1, -1],
        }
    }

    /// The draw steps alone (no state save needed).
    fn frames(s: &mut Research, n: usize) {
        let techs = TechTable::default();
        for _ in 0..n {
            s.draw_frame(0, &techs);
        }
    }

    /// `CheckIfTalalmanyokFolottVan` (exe 0x51d2d0): a model within 30 pixels (strict) at the zoom's minimum and not
    /// flying is under the pointer; the left button picks it (a right press doesn't), tooltip 0x11 gets its tech.
    #[test]
    fn models_follow_the_exe_hit_test() {
        let mut s = screen();
        let models = [(4, [100.0, 100.0]), (5, [120.0, 100.0])];
        assert_eq!(s.models_under([125.0, 100.0], &models).collect::<Vec<_>>(), [4, 5]);
        assert_eq!(s.models_under([130.0, 100.0], &models).collect::<Vec<_>>(), [5]);
        assert_eq!(s.models_under([150.0, 100.0], &models).next(), None);
        s.tip.mouse = [110.0, 100.0];
        s.model_tip_step(&models);
        assert_eq!(s.model_tip, Some((2 * 16 + 5, [110.0, 100.0])));
        // While a model flies the tooltip keeps what it was.
        s.near_dir[4] = 1;
        s.tip.mouse = [300.0, 300.0];
        s.model_tip_step(&models);
        assert!(s.model_tip.is_some());
        s.near_dir[4] = 0;
        s.model_tip_step(&models);
        assert_eq!(s.model_tip, None);
        s.zoom = 0.5;
        assert_eq!(s.models_under([100.0, 100.0], &models).next(), None);
    }

    /// `Forgotabla` (exe 0x51e360) clicks for the middle wheel's categories; the off-wheel buttons' `TT_*` (exe
    /// 0x524a50..) don't [obs: research-left.json and research-right.json on quick-1.sav: power (9) at t 380 silent,
    /// the running civil project (1) through 0x19 at t 450 a click].
    #[test]
    fn only_the_wheel_categories_click() {
        let mut s = screen();
        s.choose_area(9);
        assert!(s.sounds.is_empty());
        s.choose_area(1);
        assert_eq!(s.sounds, [CLICK]);
    }

    #[test]
    fn a_press_goes_in_and_out_before_the_wheel_turns() {
        // Ships sit in slot 4: its button (forgastabla3 = 104) goes in 7, 10 and back out 3, 0 (`resSetGombFrame`),
        // then the wheel turns 2 frames a frame (`KeszALenyomas`).
        let mut s = screen();
        s.choose_area(1);
        assert_eq!((s.button, s.button_new), (104, true));
        // (The models' flight out and back, which also holds the wheel, is left out here.)
        (s.old_area, s.drawn_area) = (s.area, s.area);
        let mut seen = Vec::new();
        for _ in 0..4 {
            frames(&mut s, 1);
            seen.push((s.button_frame, s.turn));
        }
        assert_eq!(seen, [(7, 0.0), (10, 0.0), (3, 0.0), (0, 0.0)]);
        assert_eq!(s.pressed, [(104, 0)]);
        frames(&mut s, 1);
        assert_eq!(s.turn, 2.0);
        // A category off the wheel presses its own button (civil: the right wheel's 0x12d).
        s.choose_area(6);
        assert_eq!(s.button, 0x12d);
    }

    #[test]
    fn the_urgent_button_stays_in() {
        let mut s = screen();
        s.double();
        assert!(s.urgent);
        assert_eq!(s.pressed, [(0xc9, 10)]);
        s.double();
        assert_eq!(s.pressed, [(0xc9, 0)]);
    }

    #[test]
    fn the_opening_runs_to_frame_60_and_the_closing_back() {
        let mut s = screen();
        (s.frame, s.opened, s.started, s.ball_shown) = (0.0, false, false, false);
        let techs = TechTable::default();
        // `resELINDULT`: the first UI frame before a drawn one doesn't move it.
        let step = |s: &mut Research| {
            if s.animated && !s.closing && s.started {
                s.frame = (s.frame + 0.4).min(60.0);
            }
            if s.animated && s.closing {
                s.frame = (s.frame - 0.72).max(0.0);
            }
            s.draw_frame(0, &techs);
        };
        let mut n = 0;
        while !s.opened {
            step(&mut s);
            n += 1;
            if n == 80 {
                // Frame 31.6: the ball flies in.
                assert!(s.ball_shown && s.ball_at[2] < -950.0);
            }
        }
        assert_eq!(s.frame, 60.0);
        assert!((150..=152).contains(&n), "{n}");
        assert_eq!(s.ball_at, BALL);
        assert!(!s.leave());
        let mut n = 0;
        while !s.closed() {
            step(&mut s);
            n += 1;
        }
        assert_eq!(s.frame, 0.0);
        assert_eq!(n, 85);
    }

    #[test]
    fn the_wheel_turns_the_chosen_category_to_the_front() {
        // Ships sit in slot 4: the tables rotate right once, the wheel animates 10 frames forwards.
        let mut s = screen();
        s.choose_area(1);
        assert_eq!(s.slot_area, [1, 3, 2, 4, 5]);
        assert_eq!((s.turn_to, s.turn_left), (10.0, false));
        // Then weapons (slot 2 now): three rotations right, 30 frames on (left turn: slots 1 and 2 turn back).
        s.choose_area(2);
        assert_eq!(s.slot_area, [2, 4, 5, 1, 3]);
        assert_eq!((s.turn_to, s.turn_left), (40.0, true));
        // A category off the middle wheel leaves it alone.
        s.choose_area(7);
        assert_eq!(s.slot_area, [2, 4, 5, 1, 3]);
        assert_eq!(s.tech, 6 * 16 + 1);
    }

    #[test]
    fn list_rows_follow_the_exe_layout() {
        assert_eq!(row_xy(0), (10.0, 86.0));
        assert_eq!(row_xy(7), (10.0, 156.0));
        assert_eq!(row_xy(8), (507.0, 86.0));
        assert_eq!(row_xy(15), (507.0, 156.0));
    }

    #[test]
    fn the_chosen_model_comes_forward_and_grows() {
        let mut s = screen();
        s.choose_field(4);
        assert_eq!(s.near_dir[4], 1);
        for _ in 0..9 {
            for k in 1..17 {
                if s.near_dir[k] == 1 && s.near[k] < 700.0 {
                    s.near[k] = (s.near[k] + 80.0).min(700.0);
                }
            }
        }
        let (at, grow) = s.model_at([10.0, 0.0, 0.0], 4);
        assert_eq!(grow, 4.0);
        assert_eq!(at, [0.0, -50.0, -1000.0]);
        s.choose_field(5);
        assert_eq!((s.near_dir[4], s.near_dir[5]), (-1, 1));
    }

    /// The top bar's quick pick with the screen down moves the place it opens at (`ResearchAtvaltTalalmanyra` exe
    /// 0x521f20); the place the screen leaves at comes back on the next visit (needs the install and
    /// `quick-1.sav`).
    #[test]
    fn the_quick_pick_moves_the_place_the_screen_opens_at() {
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let w = crate::world::World::load(&save, &rules, &defaults, &volatile);
        let wheel = tables.research_screen.wheel_area;
        // A middle-wheel category turns to the front; one off the wheel (7, research) leaves the front alone.
        let mut p = Place::read(&w.game.state);
        let a = wheel[2];
        p.jump_to((a - 1) * 16 + 5, &tables);
        assert_eq!((p.area, p.field, p.tech, p.fields[a as usize], p.front), (a as usize, 5, (a - 1) * 16 + 5, 5, a));
        p.jump_to(6 * 16 + 2, &tables);
        assert_eq!((p.area, p.field, p.fields[7], p.front), (7, 2, 2, a));
        // The screen opens there and leaves at the same place (the wheel's front is its first slot).
        let r = Research::open_at(&w.game.state, &tables, false, p);
        assert_eq!((r.area, r.tech, r.slot_area[0]), (7, 6 * 16 + 2, a));
        let back = r.place();
        assert_eq!((back.area, back.tech, back.front, back.fields[a as usize]), (7, 6 * 16 + 2, a, r.fields[a as usize]));
        // A save writes the place into the globals and a load reads it back.
        let mut state = w.game.state.clone();
        back.write(&mut state);
        assert_eq!(Place::read(&state), back);
    }

    /// The screen's keys and hot-spots reach the sim through `MULTIGET_Research`'s commands (needs the install and
    /// `quick-1.sav`).
    #[test]
    fn screen_orders_reach_the_sim() {
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let mut w = crate::world::World::load(&save, &rules, &defaults, &volatile);
        let send = |w: &mut crate::world::World, acts: Vec<Act>| {
            for Act::Send(c) in acts {
                w.send(c);
            }
            w.pump();
        };
        let e = en(&w.game.state);
        let mut r = Research::open(&w.game.state, &tables, false);
        let mask = ResearchModels::mask(&install);
        assert_eq!(mask.len(), 640 * 480);
        // Stop whatever runs, then find a researchable tech the labs allow and start it with Enter.
        let current = w.game.state.mem.i32(va::RESEARCHING + 4 * e as u32);
        if current > 0 {
            let acts = r.key(&w.game.state, Key::Enter);
            send(&mut w, acts);
            r.button_pressed = false;
            r.key_seen(false);
            assert_eq!(w.game.state.mem.i32(va::RESEARCHING + 4 * e as u32), 0);
        }
        let t = (1..193).find(|&t| {
            r.tech = t;
            r.can_start(&w.game.state)
        });
        let Some(t) = t else { return };
        r.jump_to(t);
        let acts = r.key(&w.game.state, Key::Enter);
        send(&mut w, acts);
        assert_eq!(w.game.state.mem.i32(va::RESEARCHING + 4 * e as u32), t);
        assert_eq!(w.game.state.tech().status(t as usize, e), 4);
        // The urgent button through its mask colour (0x12).
        r.button_pressed = false;
        let at = (0..640 * 480).find(|&i| mask[i] == 0x12).map(|i| [(i % 640) as f32, (i / 640) as f32]).unwrap();
        let before = w.game.state.mem.u8(va::URGENT + e as u32);
        let acts = r.press(&w.game.state, &tables, &mask, at, &[], true);
        send(&mut w, acts);
        assert_ne!(w.game.state.mem.u8(va::URGENT + e as u32), before);
        // A middle-wheel category through its slot's mask colour turns the wheel (once it stands still: the
        // category hot-spots are off while it turns).
        r.dir = 0;
        r.turn = r.turn_to;
        let kep = r.slot_kep[3] as u8;
        let area = r.slot_area[3];
        let at = (0..640 * 480).find(|&i| mask[i] == kep).map(|i| [(i % 640) as f32, (i / 640) as f32]).unwrap();
        r.press(&w.game.state, &tables, &mask, at, &[], true);
        assert_eq!(r.area as i32, area);
        assert_eq!(r.slot_area[0], area);
    }

    /// `ResDrawVonalak`'s arrows as ig2.exe drew them (Frida, `probes/research_lines.js`, quick-1.sav, research-lines.json:
    /// ui 992 the ring after the opening, 1300 flying in at the tanks button after PgDn, 1150 faded out): `ResVonalkak`,
    /// `ResVonalakNovelo`, `ResKellVonalTavolodas`, the centre, then the first, second and last `DrawArrowAbsolute`
    /// (p0, p1 as f32 bits, colour). All 706 calls of the run matched bit for bit.
    #[test]
    fn button_lines_match_the_exe() {
        type Rec = ([u32; 10], u32, [f32; 2], usize, [[u32; 5]; 3]);
        let recs: [Rec; 3] = [
            (
                [0x41c80000, 0x41f00000, 0x420c0000, 0x42200000, 0x42340000, 0x42480000, 0x425c0000, 0x42700000, 0x42820000, 0x428c0000],
                0x43160000,
                [322.0, 353.0],
                10,
                [
                    [0x43af4420, 0x43abdd94, 0x43aa82c0, 0x43ad690d, 0x80735f5f],
                    [0x43a9d153, 0x43a45d8b, 0x43a6e0e2, 0x43a86907, 0x80735f5f],
                    [0x43bf1d57, 0x43ba4ad0, 0x43ba5c13, 0x43b8bf00, 0x4c735f5f],
                ],
            ),
            (
                [0x428e0000, 0x42980000, 0x42a20000, 0x42ac0000, 0x42b60000, 0x42c00000, 0x42ca0000, 0x42d40000, 0x42de0000, 0x42e80000],
                0x42680000,
                [150.0, 370.0],
                6,
                [
                    [0x43414a2c, 0x43d13e59, 0x433aa139, 0x43cd8387, 0x4a735f5f],
                    [0x435daa6c, 0x43c0c668, 0x4353e4a3, 0x43bfb6fd, 0x3d735f5f],
                    [0x429def85, 0x43913661, 0x42ab4111, 0x4394f148, 0x0a735f5f],
                ],
            ),
            (
                [0; 10],
                0x439a0000,
                [322.0, 353.0],
                10,
                [
                    [0x439702d7, 0x43a54f4b, 0x439a573a, 0x43a90a32, 0x00735f5f],
                    [0x43925766, 0x43ad5165, 0x43973a44, 0x43ae60ee, 0x00735f5f],
                    [0x439f7faf, 0x43a19348, 0x439fffca, 0x43a68cda, 0x00735f5f],
                ],
            ),
        ];
        for (v, turn, c, n, want) in recs {
            let l = Lines { v: v.map(f32::from_bits), turn: f32::from_bits(turn), fly: true };
            let got: Vec<[u32; 5]> = l.arrows(c).iter().map(|a| [a.p0[0].to_bits(), a.p0[1].to_bits(), a.p1[0].to_bits(), a.p1[1].to_bits(), a.colour]).collect();
            assert_eq!(got.len(), n);
            assert_eq!([got[0], got[1], got[n - 1]], want);
        }
    }

    #[test]
    fn button_lines_run_their_course() {
        let mut l = Lines::default();
        l.load();
        assert_eq!(l.v, [100.0, 105.0, 110.0, 115.0, 120.0, 125.0, 130.0, 135.0, 140.0, 145.0]);
        // The first arrow shows after one tick, the last after 91; all are gone (alpha 0) 200 ticks after their start.
        l.tick();
        assert_eq!(l.arrows([0.0, 0.0]).len(), 1);
        for _ in 0..90 {
            l.tick();
        }
        assert_eq!(l.arrows([0.0, 0.0]).len(), 10);
        // At 500 the turn restarts without the fly-in; a new category hides them all.
        for _ in 0..410 {
            l.tick();
        }
        assert!(!l.fly && l.turn == 0.0 && l.v[9] == 145.0);
        l.hide();
        l.tick();
        assert!(l.arrows([0.0, 0.0]).is_empty());
    }

    #[test]
    fn the_last_pressed_button_holds_the_lines() {
        let mut s = screen();
        s.choose_area(10);
        assert_eq!(s.last_button, [0x96, 0x172]);
        s.choose_area(2);
        assert_eq!(s.last_button, [322, 353]);
    }

    #[test]
    fn printf_fills_the_one_number() {
        assert_eq!(printf_d(b"Id\xf5 kell", b": %d%", 100), b"Id\xf5 kell: 100".to_vec());
        assert_eq!(printf_d(b"P\xe9nz kell", b":%d", 7), b"P\xe9nz kell:7".to_vec());
    }

    #[test]
    fn a_right_press_near_the_ball_drags_it() {
        let mut s = screen();
        let techs = TechTable::default();
        let frame = |s: &mut Research| {
            s.draw_frame(0, &techs);
            s.poll = [s.buttons, s.poll[0]];
        };
        let c = project(BALL).unwrap();
        s.tip.moved(c);
        frame(&mut s);
        // The press is seen a frame late (`Mouse_Lekerdez` after the draw); then the pointer's move feeds the turn.
        s.buttons = 2;
        frame(&mut s);
        frame(&mut s);
        assert!(s.ball_grip && s.gy == [0.0, 0.0]);
        s.tip.moved([c[0] + 30.0, c[1]]);
        frame(&mut s);
        assert!((s.gy[0] + 0.1).abs() < 1e-6 && s.gy[1] == 0.0);
        // A left press doesn't grab it; a release lets it go and the turn fades ×0.8 a frame.
        s.buttons = 0;
        frame(&mut s);
        frame(&mut s);
        assert!(!s.ball_grip);
        s.buttons = 1;
        frame(&mut s);
        frame(&mut s);
        assert!(!s.ball_grip);
    }
}
