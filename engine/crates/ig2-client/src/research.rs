//! The research screen (`AktKep` 4, re/screens/research.md): the screen's own state (`Research_Belep` ELF 0x5e8780 /
//! exe 0x523540, the per-frame `Research3DUpdate` ELF 0x5e4e10 / exe 0x51fd10 and `FRISSIT_Research2` ELF 0x5eb780),
//! its input (the hit-mask picture `RESE_MSK.RAW`, `Research_Main` ELF 0x5ea1c0 / exe 0x523c10, the tech list and
//! the models under the pointer) sending `MULTIGET_Research`'s orders (`ig2_net::Command::Research`), the 2D pass
//! (`ResearchGfx2D_2` ELF 0x5df030 / exe 0x5221a0: the tech's texts, the lists, the labs, the running project) and the
//! 3D scene (`resbck.xvb` console, the `focilabda.xvb` ball, the category's tech models) rendered into a texture
//! over the virtual 640 × 480 screen like the starmap's panels.
//!
//! Shown in its open state: the opening and closing animations (`resframe` 0..60, the console flying in, the lasers
//! of `ResearchGfx2D_1`), the buttons' press animations (`resSetGombFrame`), sounds and tooltips are not drawn.

use std::collections::HashMap;

use ig2_formats::Install;
use ig2_net::{Command, ResearchCmd};
use ig2_sim::colony_side::TechTable;
use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Virtual};
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Look, Model, Placement, M3, V3};
use crate::starmap_panels::{fader, Draw2d};

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
/// The ball's centre (`MoveObj(FociMesh, 0, −35, −950)`).
const BALL: V3 = [0.0, -35.0, -950.0];

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

/// The screen's state (one peer's UI; the original keeps it in globals, `Terulet_Akt` and the selection are saved).
#[derive(Debug, Clone)]
pub struct Research {
    /// `ResearchTerulet` 1..=12, `Terulet_Akt[area]` (the remembered field per category, −1 none), `ResearchMezo`,
    /// `ResearchTalalmany`.
    pub area: usize,
    pub fields: [i32; 13],
    pub field: i32,
    pub tech: i32,
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
    /// `ButtonPressed`: an order is on its way (`MULTIGET_Research` clears it for the issuer).
    pub button_pressed: bool,
    /// `Double_Lenyomva`.
    pub urgent: bool,
    /// The 10 ms frames since the screen opened (the texts' and models' pace).
    pub frames: u64,
}

impl Research {
    /// `Research_Belep` on the state's saved selection.
    pub fn open(state: &SaveState, rules: &ig2_rules::Rules) -> Self {
        let m = &state.mem;
        let en = en(state);
        let mut fields = [0; 13];
        for (k, f) in fields.iter_mut().enumerate() {
            *f = m.i32(at::FIELDS + 4 * k as u32);
        }
        let s = &rules.research_screen;
        let mut r = Self {
            area: m.i32(at::AREA).clamp(1, 12) as usize,
            fields,
            field: m.i32(at::FIELD),
            tech: m.i32(at::TECH),
            slot_area: s.wheel_area,
            slot_kep: s.wheel_kep,
            turn: 0.0,
            turn_to: 0.0,
            turn_left: false,
            reveal: std::array::from_fn(|i| -((i % 8) as f32)),
            info_reveal: [0.0; 4],
            info_tech: -1,
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
            urgent: m.u8(va::URGENT + en as u32) & 1 != 0,
            frames: 0,
        };
        r.turn_wheel(m.i32(at::FRONT));
        (r.turn, r.turn_to) = (0.0, 0.0);
        r.slot_kep = s.wheel_kep;
        let techs = state.tech();
        for a in 1..=12 {
            r.fix_field(&techs, en, a);
        }
        r.field = r.fields[r.area];
        r.placed = r.area;
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
        if s == 0 {
            return;
        }
        self.turn_left = s == 1 || s == 2;
        self.slot_area.rotate_right(5 - s);
        self.turn_to += ((5 - s) * 10) as f32;
        if self.turn_to >= 50.0 {
            self.turn_to -= 50.0;
        }
    }

    /// `TT_*` (ELF 0x5e9430..): category `n` (`ResearchTerulet`), its remembered field.
    fn choose_area(&mut self, n: usize) {
        self.area = n;
        self.turn_wheel(n as i32);
        self.field = self.fields[n];
        self.tech = self.field + (n as i32 - 1) * 16;
    }

    /// `TalalmanyKivalasztva(k)` (ELF 0x5e24a0): field `k` of the category; its model moves in front of the ball.
    fn choose_field(&mut self, k: i32) {
        self.field = k;
        self.fields[self.area] = k;
        self.tech = k + (self.area as i32 - 1) * 16;
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

    /// `ResearchAtvaltTalalmanyra(t)`: the running project's category and field.
    fn jump_to(&mut self, t: i32) {
        let n = ((t - 1).max(0) / 16 + 1).clamp(1, 12) as usize;
        self.choose_area(n);
        self.field = t - (n as i32 - 1) * 16;
        self.tech = t;
        self.fields[n] = self.field;
    }

    /// The hot-spot under `at` (`Mouse_Darabol` ELF 0x5b7a40: the mask picture's colour, else the rectangles), with
    /// `Research_MouseInit2`'s and `Research3DUpdate`'s on/off states.
    pub fn spot(&self, state: &SaveState, mask: &[u8], at: [f32; 2]) -> Option<u32> {
        let (x, y) = (at[0].clamp(0.0, 639.0) as usize, at[1].clamp(0.0, 479.0) as usize);
        let c = mask.get(y * 640 + x).copied().unwrap_or(0);
        let en = en(state);
        let current = state.mem.i32(va::RESEARCHING + 4 * en as u32);
        let categories = self.turn == self.turn_to && self.dir == 0;
        let on = |id: u32| match id {
            1 => current > 0 || self.can_start(state),
            3 | 0x19 => current > 0,
            5..=0x10 => categories,
            _ => true,
        };
        let mut hit = None;
        for &(id, kep, r) in STATIC_SPOTS.iter().rev() {
            let inside = if kep == 0 { (r[0]..r[0] + r[2]).contains(&(x as i32)) && (r[1]..r[1] + r[3]).contains(&(y as i32)) } else { c == kep };
            if inside && on(id) {
                hit = Some(id);
            }
        }
        for (k, &id) in WHEEL_SPOTS.iter().enumerate() {
            // Hot-spot `id` is category `WHEEL_AREA[k]`; its colour is the one of the slot that category is in.
            let area = rules_wheel_area(k);
            if let Some(slot) = self.slot_area.iter().position(|&a| a == area) {
                if c as i32 == self.slot_kep[slot] && on(id) {
                    hit = Some(hit.map_or(id, |h: u32| h.min(id)));
                }
            }
        }
        hit
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

    /// A left press at virtual `at`: `Research_Main` for the hot-spot under it, else a list row (`ResearchGfx2D_2`)
    /// or a model (`CheckIfTalalmanyokFolottVan`).
    pub fn press(&mut self, state: &SaveState, rules: &ig2_rules::Rules, mask: &[u8], at: [f32; 2], models: &[(usize, [f32; 2])]) -> Vec<Act> {
        let mut out = Vec::new();
        let en = en(state);
        match self.spot(state, mask, at) {
            Some(1) => out = self.start_stop(state),
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
            None => {
                if let Some(k) = self.list_row(state, at) {
                    self.choose_field(k);
                } else if self.zoom == ZOOM_MIN {
                    // A model's projected centre within 30 pixels.
                    if let Some(&(k, _)) = models.iter().find(|(k, c)| self.near_dir[*k] == 0 && (c[0] - at[0]).hypot(c[1] - at[1]) < 30.0) {
                        self.choose_field(k as i32);
                    }
                }
            }
        }
        let _ = rules;
        out
    }

    /// The list row under `at` (`ResearchGfx2D_2`: rows of 21 cells, 10 lines apart, the human's known techs).
    fn list_row(&self, state: &SaveState, at: [f32; 2]) -> Option<i32> {
        let en = en(state);
        let techs = state.tech();
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

    /// `ResearchKeyboardHandler` (ELF 0x5e9aa0): ↓/↑ the next/previous known field, →/← the other column (±8),
    /// Enter `ResearchStartStop`, D `ResearchDoubleGomb`, PgUp/PgDn the previous/next category in
    /// `Terulet_Melyik`'s order (exe 0x919234).
    pub fn key(&mut self, state: &SaveState, k: Key) -> Vec<Act> {
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
            Key::Enter => return self.start_stop(state),
            Key::Double => return self.double(),
            Key::PageUp | Key::PageDown if !self.button_pressed => {
                let at = AREA_ORDER.iter().position(|&a| a == self.area).unwrap_or(0);
                let next = if k == Key::PageUp { (at + 11) % 12 } else { (at + 1) % 12 };
                self.choose_area(AREA_ORDER[next]);
            }
            _ => {}
        }
        if let Some(m) = moved {
            self.choose_field(m);
        }
        Vec::new()
    }

    /// `ResearchStartStop` (ELF 0x5e9190): start the shown tech when nothing runs and it can start, stop the running
    /// project otherwise.
    fn start_stop(&mut self, state: &SaveState) -> Vec<Act> {
        let mut out = Vec::new();
        if self.button_pressed {
            return out;
        }
        let current = state.mem.i32(va::RESEARCHING + 4 * en(state) as u32);
        if current == 0 && self.can_start(state) {
            self.button_pressed = true;
            out.push(Act::Send(Command::Research { order: ResearchCmd::Start { tech: self.tech } }));
        }
        if current > 0 {
            self.button_pressed = true;
            out.push(Act::Send(Command::Research { order: ResearchCmd::Stop { tech: current } }));
        }
        out
    }

    /// `ResearchDoubleGomb` (ELF 0x5e93c0): the urgent button's look flips at once, the order toggles the flag.
    fn double(&mut self) -> Vec<Act> {
        if self.button_pressed {
            return Vec::new();
        }
        self.urgent = !self.urgent;
        vec![Act::Send(Command::Research { order: ResearchCmd::Urgent })]
    }

    /// One 10 ms frame (`FRISSIT_Research2` + `Research3DUpdate`'s per-frame steps).
    pub fn step(&mut self, state: &SaveState) {
        self.frames += 1;
        let en = en(state);
        let techs = state.tech();
        // The category changed: the models fly out (`TalalmanyDir` 1, `AddZoom` up by 0.01), are reloaded and fly
        // back in (−1, down by 0.15) to the remembered field.
        if self.placed != self.area && self.dir != 1 {
            self.dir = 1;
        }
        if self.dir == 1 {
            self.zoom += 0.01;
            if self.zoom > ZOOM_MAX {
                self.zoom = ZOOM_MAX;
                self.placed = self.area;
                self.near = [0.0; 17];
                self.near_dir = [0; 17];
                self.dir = -1;
            }
        }
        if self.fields[self.area] == -1 {
            self.fix_field(&techs, en, self.area);
            if self.fields[self.area] != -1 {
                self.field = self.fields[self.area];
                self.tech = self.field + (self.area as i32 - 1) * 16;
            }
        }
        for r in &mut self.reveal {
            if *r < 20.0 {
                *r += 1.8;
            }
        }
        if self.dir == -1 {
            self.zoom -= 0.15;
            if self.zoom <= ZOOM_MIN {
                self.zoom = ZOOM_MIN;
                self.dir = 0;
                if self.field > 0 {
                    self.choose_field(self.field);
                }
            }
        }
        self.wire += 0.25;
        if self.wire >= 10.0 {
            self.wire = 0.0;
        }
        if self.turn != self.turn_to && self.dir != 1 {
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
        }
        // `KellMozgatni`: the chosen model comes forward by 80 a frame (to 700), the others go back.
        for k in 1..17 {
            if self.near_dir[k] == 1 && self.near[k] < 700.0 {
                self.near[k] = (self.near[k] + 80.0).min(700.0);
            }
            if self.near_dir[k] == -1 && self.near[k] > 0.0 {
                self.near[k] -= 80.0;
                if self.near[k] <= 0.0 {
                    self.near[k] = 0.0;
                    self.near_dir[k] = 0;
                }
            }
        }
        // The ball turns 0.002 a frame (`matrixforgat(m, 0.002, 0)`), the models 0.02 about their own axis.
        self.ball = scene::mul(&scene::axis_matrix([0.0, 1.0, 0.0], 0.002), &self.ball);
        for s in &mut self.spin {
            *s += 0.02;
        }
        self.keringeto += 0.0006;
        self.res_xy += 0.02;
        // The lab cones fade in (0.02 a frame) for the labs the shown tech needs, out for the others.
        let need = techs.labs_needed(self.tech.clamp(0, 192) as usize);
        for (c, &n) in self.cones.iter_mut().zip(&need) {
            *c = if n != 0 { (*c - 0.02).max(0.0) } else { (*c + 0.02).min(1.0) };
        }
        // The info lines come out 1.4 characters a frame.
        if self.info_tech != self.tech {
            self.info_tech = self.tech;
            self.info_reveal = [0.0; 4];
        }
        for r in &mut self.info_reveal {
            *r += 1.4;
        }
        self.button_pressed = false;
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
    Double,
    PageUp,
    PageDown,
}

/// `Terulet_Melyik` (exe 0x919234) inverted: the categories in the order PgUp/PgDn step through them.
const AREA_ORDER: [usize; 12] = [10, 11, 3, 1, 5, 4, 2, 12, 6, 9, 7, 8];

/// `forgastabla2i[k]`: the category of the middle wheel's hot-spot `WHEEL_SPOTS[k]` (radar, weapons, shields, bombs,
/// ships).
fn rules_wheel_area(k: usize) -> i32 {
    [3, 2, 4, 5, 1][k]
}

fn en(state: &SaveState) -> usize {
    state.mem.i32(va::EN).clamp(0, 8) as usize
}

/// `LaborSzint[e][1..=3]`.
fn labs(state: &SaveState, e: usize) -> [i32; 4] {
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
fn race_mults(state: &SaveState, rules: &ig2_rules::Rules, en: usize) -> (f32, f32) {
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

/// The 2D pass under and over the models.
pub struct Texts<'a> {
    pub infos: &'a [[Vec<u8>; 3]],
}

impl Research {
    /// The faders under the models (`Research3DUpdate`'s `KirakFader` 0..=4, open: full height): the two side
    /// panels, their bottom bars, the running project's box.
    pub fn draw_under(&self, f: &mut Frame, d: &Draw2d, state: &SaveState) {
        let (white, v) = (d.white, &d.v);
        let s = 0.9;
        let kirak = |f: &mut Frame, r: [f32; 4]| fader(f, white, v, r, [0.0, 54.0 * s, 90.0 * s], 105.0, [0.0, 90.0 * s, 130.0 * s], 50.0);
        let (y, h) = (124.0 - 43.0, 106.0);
        kirak(f, [3.0, y, 135.0, h]);
        kirak(f, [500.0, y, 135.0, h]);
        kirak(f, [5.0, y + h - 15.0, 131.0, 13.0]);
        kirak(f, [502.0, y + h - 15.0, 131.0, 13.0]);
        if state.mem.i32(va::RESEARCHING + 4 * en(state) as u32) != 0 {
            kirak(f, [1.0, 26.0, 108.0, 42.0]);
        }
    }

    /// The models' texture, then `ResearchGfx2D_2`'s texts.
    pub fn draw_over(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, rules: &ig2_rules::Rules, t: &Texts, models: Option<TexId>) {
        let v = &d.v;
        if let Some(tex) = models {
            f.push(tex, Blend::Alpha, Instance::new(kind::SPRITE, v.px(320.0, 240.0), [320.0 * v.k, 240.0 * v.k], [1.0; 4]));
        }
        let en = en(state);
        let techs = state.tech();
        if self.field == -1 {
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
        // The chosen row's frame (`DesignKeret1(x − 5, y − 2, 20 cells + 10, h + 4)`).
        let k = (self.field - 1).clamp(0, 15) as usize;
        let (x, y) = row_xy(k);
        frame_box(f, d, [x - 5.0, y - 2.0, d.font.advance(2) * 20.0 + 10.0, d.font.height(2) + 4.0]);
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
            let s1 = printf_d(&d.texts.get(4, 2, 3), cost);
            d.font.text(f, v, &s1, 4.0, 0x2a as f32, s1.len(), 2, Align::Left, pc, None);
            let s2 = printf_d(&d.texts.get(4, 2, 4), days);
            d.font.text(f, v, &s2, 4.0, 0x36 as f32, s2.len(), 2, Align::Left, pc, None);
        }
        let _ = money_mult;
        // The shown tech's days and daily cost under the panels, unless owned.
        if status < 5 {
            let (days, cost) = days_and_cost(state, rules, &techs, tech, en);
            let s1 = printf_d(&d.texts.get(4, 2, 5), days);
            d.font.text(f, v, &s1, 0x1fb as f32, 0xaf as f32, s1.len(), 2, Align::Left, c22, None);
            let s2 = printf_d(&d.texts.get(4, 2, 6), cost);
            d.font.text(f, v, &s2, 10.0, 0xaf as f32, 0x10, 2, Align::Left, c22, None);
        }
    }
}

/// `sprintf(fmt, n)` for the one `%d` the research strings hold.
fn printf_d(fmt: &[u8], n: u32) -> Vec<u8> {
    let s = String::from_utf8_lossy(fmt);
    s.replacen("%d", &n.to_string(), 1).replace("%%", "%").into_bytes()
}

/// `FillToScreen(x, y, w, h, 0xc)`: the typing cursor.
fn cursor(f: &mut Frame, d: &Draw2d, r: [f32; 4]) {
    let c = d.palette.get(0xc).copied().unwrap_or([1.0; 3]);
    let v = &d.v;
    f.push(d.white, Blend::Alpha, Instance::new(kind::SPRITE, v.px(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0), [r[2] * v.k / 2.0, r[3] * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
}

/// `DesignKeret1(x, y, w, h, 1)`: the chosen row's frame [approximation: the original's animated rounded frame is
/// drawn as a one-pixel box in the panels' edge colour].
fn frame_box(f: &mut Frame, d: &Draw2d, r: [f32; 4]) {
    let v = &d.v;
    let c = crate::starmap_hud::srgb_blend([0.0, 112.0, 227.0], 0.9);
    let [x, y, w, h] = r;
    for q in [[x, y, w, 1.0], [x, y + h - 1.0, w, 1.0], [x, y, 1.0, h], [x + w - 1.0, y, 1.0, h]] {
        f.push(d.white, Blend::Alpha, Instance::new(kind::SPRITE, v.px(q[0] + q[2] / 2.0, q[1] + q[3] / 2.0), [q[2] * v.k / 2.0, q[3] * v.k / 2.0], c));
    }
}

/// The screen's models on the GPU side: the console posed at the open frame and at the wheel's frames, the ball,
/// the categories' tech models, their textures, the hit mask, and the texture they are drawn into.
pub struct ResearchModels {
    console: Option<Model>,
    wheel: HashMap<i32, Model>,
    ball: Option<Model>,
    techs: HashMap<usize, Option<Model>>,
    textures: HashMap<String, TexId>,
    white: TexId,
    target: Option<Target>,
    /// `RESE_MSK.RAW` (kep 0x58): 640 × 480 hot-spot colours.
    pub mask: Vec<u8>,
    /// The tech models' projected centres of the last frame (for clicks).
    pub centres: Vec<(usize, [f32; 2])>,
}

/// The camera: `vx = 600·x/w + 320`, `vy = 600·y/w + 250` (w = −z), like the starmap's panels.
fn project(p: V3) -> Option<[f32; 2]> {
    let w = -p[2];
    (w > 1.0).then(|| [600.0 * p[0] / w + 320.0, 600.0 * p[1] / w + 250.0])
}

impl ResearchModels {
    pub fn new(r: &mut Renderer, install: &Install) -> Self {
        let white = r.add_texture(&crate::assets::Image::white());
        let mask = Self::mask(install);
        Self { console: None, wheel: HashMap::new(), ball: None, techs: HashMap::new(), textures: HashMap::new(), white, target: None, mask, centres: Vec::new() }
    }

    /// `RESE_MSK.RAW` (kep 0x58): a 32-byte `mhwanh` header (640 × 480), a 768-byte palette, then the pixels.
    pub fn mask(install: &Install) -> Vec<u8> {
        install.read("dataraw/screen/rese_msk.raw").ok().and_then(|b| b.get(800..800 + 640 * 480).map(|x| x.to_vec())).unwrap_or_default()
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

    /// Render the screen's models into its texture. `size` = the window in pixels.
    pub fn render(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, s: &Research, state: &SaveState, size: [f32; 2]) -> (Option<TexId>, Vec<Instance>) {
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let mut lines: Vec<Instance> = Vec::new();
        let v = Virtual::new(size);
        let en = en(state);
        let techs = state.tech();
        if self.console.is_none() {
            self.console = Model::load_at(install, "vector/research/screen/", "resbck.xvb", "textures/research/screen/", Some(60)).map_err(|e| eprintln!("ig2-client: {e}")).ok();
        }
        let wf = s.wheel_frame() as i32;
        if let std::collections::hash_map::Entry::Vacant(slot) = self.wheel.entry(wf) {
            if let Ok(m) = Model::load_at(install, "vector/research/screen/", "resbck.xvb", "textures/research/screen/", Some(wf as usize)) {
                slot.insert(m);
            }
        }
        if self.ball.is_none() {
            self.ball = Model::load(install, "vector/research/screen/", "focilabda.xvb", "textures/research/screen/").map_err(|e| eprintln!("ig2-client: {e}")).ok();
        }
        // The console: `AddToScene(resbck, 0.95)` at (0, 3, −650) (`resmozgato` at the open frame), frame 60; the
        // wheel `Group133` (child 0xd) at frame `kezeloframe + 70`; hidden: the closed wheel `Group115` (0xb), the
        // starmap console `Kezelo_radar` (0x10), the civil and military cones (3, 4, 8, 9); the wire sphere
        // `Group66` (0xe) is drawn as lines.
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
        let hidden = ["/Group115/", "/Kezelo_radar/", "/Group106/", "/Group107/", "/Group111/", "/Group112/", "/Group133/", "/Group66/"];
        let place = |m: &Model| Placement::add_to_scene(m.origin, 0.95).world((0.0, 0.0, 0.0), [0.0, 3.0, -650.0]);
        if let Some(m) = self.console.clone() {
            let t = place(&m);
            for (pi, p) in m.parts.iter().enumerate() {
                if hidden.iter().any(|h| p.path.starts_with(h)) {
                    if p.path.starts_with("/Group66/") {
                        wire(&mut lines, &v, p, &t.matrix, t.translation, srgb([5.0, 18.0, 30.0]));
                    }
                    continue;
                }
                // `KELLVILLOGAS`: the info box's glass (`Box01`, child 0xf → 0 → 0) at transparency
                // `resatlatszosag = sin(res_xy)·0.04 + 0.85`.
                // The flat blue glass faces the camera: drawn in 2D over its projected box (`srgb_blend`, as the
                // original blends on the sRGB values).
                if p.path.starts_with("/Group85/Group84/") {
                    let a = 1.0 - (((s.res_xy as f64).sin() * 0.04) as f32 + 0.85);
                    let pts: Vec<[f32; 2]> = p.verts.iter().filter_map(|x| {
                        let w = scene::apply(&t.matrix, x.pos);
                        project([w[0] + t.translation[0], w[1] + t.translation[1], w[2] + t.translation[2]])
                    }).collect();
                    if let (Some(x0), Some(x1), Some(y0), Some(y1)) = (
                        pts.iter().map(|q| q[0]).reduce(f32::min),
                        pts.iter().map(|q| q[0]).reduce(f32::max),
                        pts.iter().map(|q| q[1]).reduce(f32::min),
                        pts.iter().map(|q| q[1]).reduce(f32::max),
                    ) {
                        let c = crate::starmap_hud::srgb_blend([48.0, 149.0, 242.0], a);
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
                bake_part(p, &t.matrix, t.translation, tex, alpha, &mut dynamic, &mut draws);
                let _ = pi;
            }
        }
        if let Some(m) = self.wheel.get(&wf).cloned() {
            let t = place(&m);
            for p in m.parts.iter().filter(|p| p.path.starts_with("/Group133/")) {
                let tex = self.texture(r, install, p.texture.as_deref());
                bake_part(p, &t.matrix, t.translation, tex, 1.0, &mut dynamic, &mut draws);
            }
        }
        // The ball: `AddToScene(focilabda, 0.22)` at (0, −35, −950), wireframe (property 0x40), turning.
        if let Some(m) = self.ball.clone() {
            let pl = Placement::add_to_scene(m.origin, 0.22);
            let mm = scene::mul(&s.ball, &pl.matrix).map(|x| x * 1024.0);
            for p in &m.parts {
                wire(&mut lines, &v, p, &mm, BALL, srgb([49.0, 154.0, 255.0]));
            }
        }
        // The category's tech models (status ≥ 2): owned ones solid, the others as wireframes in their light's
        // colour (`FenyHozzarendel`: seen (0, 32, 32), researchable (0, 64, 64), researching (64, 64, 0)).
        self.centres.clear();
        let race = state.mem.u8(at::RACES + en as u32);
        let sr = &rules.research_screen;
        if s.dir != 1 {
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
                let pl = Placement::add_to_scene(m.origin, size_mult * 0.04).scaled([grow; 3]);
                // `ForgatObjectek`: `RotateObj(0, 0, spin)` then `RotateObj2((1, 0, 0), 1.3)`.
                let rot = scene::mul(&scene::axis_matrix([1.0, 0.0, 0.0], 1.3), &scene::create_matrix(0.0, 0.0, s.spin[k]));
                let mm = scene::mul(&rot, &pl.matrix).map(|x| x * 1024.0);
                if let Some(c) = project(at) {
                    self.centres.push((k, c));
                }
                if st == 5 {
                    for p in &m.parts {
                        let tex = self.texture(r, install, p.texture.as_deref());
                        bake_part(p, &mm, at, tex, 1.0, &mut dynamic, &mut draws);
                    }
                } else {
                    let c = match st {
                        2 => [0.25, 0.5, 0.5, 1.0],
                        3 => [0.4, 0.9, 0.9, 1.0],
                        _ => [0.9, 0.9, 0.4, 1.0],
                    };
                    for p in &m.parts {
                        wire(&mut lines, &v, p, &mm, at, c);
                    }
                }
            }
        }
        if dynamic.is_empty() {
            return (None, lines);
        }
        let k = size[1] / 480.0;
        let px = ((640.0 * k).round().max(1.0) as u32, size[1].round().max(1.0) as u32);
        let light = scene::normalise([0.0, -0.35, 1.0]);
        let (a, b) = (1.0 / (1.0 / 300.0 - 1.0 / 2400.0) / 300.0, 1.0 / (1.0 / 300.0 - 1.0 / 2400.0));
        let rows4 = [[1200.0 / 640.0, 0.0, 0.0, 0.0], [0.0, -1200.0 / 480.0, -(1.0 - 500.0 / 480.0), 0.0], [0.0, 0.0, -a, -b], [0.0, 0.0, -1.0, 0.0]];
        let view = std::array::from_fn(|c| std::array::from_fn(|k| rows4[k][c]));
        let mut f = Frame::new([0.0, 0.0, 0.0, 0.0]);
        f.scene = Some(Scene3d {
            uniforms: SceneUniforms {
                view,
                eye: [0.0; 4],
                light: [light[0], light[1], light[2], 0.0],
                // `CreateLightAmbient(0x202020)`, `CreateLightParalell(0x808080, (0, 0, −10))` and the two moving
                // side lights (orange 0xffba00, blue 0x009cff) folded into one.
                ambient: [0.25, 0.25, 0.25, 0.0],
                sun: [0.75, 0.72, 0.68, 0.0],
                fog: [0.0; 4],
                fog_range: [1e9, 2e9, 0.0, 1e9],
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

/// An sRGB colour (0..255) as the frame's linear colour.
fn srgb(c: [f32; 3]) -> [f32; 4] {
    let lin = |v: f32| {
        let v = v / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    [lin(c[0]), lin(c[1]), lin(c[2]), 1.0]
}

/// A part as a wireframe (`wirekirak` ELF 0x6083f0: the triangle edges its mask keeps), projected to lines.
fn wire(out: &mut Vec<Instance>, v: &Virtual, p: &scene::Part, m: &M3, at: V3, c: [f32; 4]) {
    for (ti, tri) in p.verts.chunks_exact(3).enumerate() {
        let mask = p.edges.get(ti).copied().unwrap_or(7);
        let q: Vec<Option<[f32; 2]>> = tri
            .iter()
            .map(|x| {
                let w = scene::apply(m, x.pos);
                project([w[0] + at[0], w[1] + at[1], w[2] + at[2]])
            })
            .collect();
        for (bit, (i, j)) in [(1u8, (2, 0)), (2, (0, 1)), (4, (1, 2))] {
            if mask & bit != 0 {
                if let (Some(a), Some(b)) = (q[i], q[j]) {
                    out.push(Instance::line(v.px(a[0], a[1]), v.px(b[0], b[1]), v.k.max(1.0), c));
                }
            }
        }
    }
}

/// Bake one camera-fixed part at `at` with matrix `m` (`alpha` < 1 draws it as glass).
fn bake_part(p: &scene::Part, m: &M3, at: V3, tex: TexId, alpha: f32, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let start = out.len() as u32;
    let a = match p.look {
        Look::Glass(g) => g * alpha,
        _ => alpha,
    };
    for x in &p.verts {
        let q = scene::apply(m, x.pos);
        out.push(MeshVertex { pos: [q[0] + at[0], q[1] + at[1], q[2] + at[2]], normal: scene::normalise(scene::apply(m, x.normal)), uv: x.uv, color: [1.0, 1.0, 1.0, a] });
    }
    draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0 });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Research {
        Research {
            area: 3,
            fields: [1; 13],
            field: 1,
            tech: 33,
            slot_area: [3, 2, 4, 5, 1],
            slot_kep: [9, 16, 11, 5, 12],
            turn: 0.0,
            turn_to: 0.0,
            turn_left: false,
            reveal: [0.0; 16],
            info_reveal: [0.0; 4],
            info_tech: -1,
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
            urgent: false,
            frames: 0,
        }
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
        let mut r = Research::open(&w.game.state, &tables);
        let mask = ResearchModels::mask(&install);
        assert_eq!(mask.len(), 640 * 480);
        // Stop whatever runs, then find a researchable tech the labs allow and start it with Enter.
        let current = w.game.state.mem.i32(va::RESEARCHING + 4 * e as u32);
        if current > 0 {
            let acts = r.key(&w.game.state, Key::Enter);
            send(&mut w, acts);
            r.button_pressed = false;
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
        let acts = r.press(&w.game.state, &tables, &mask, at, &[]);
        send(&mut w, acts);
        assert_ne!(w.game.state.mem.u8(va::URGENT + e as u32), before);
        // A middle-wheel category through its slot's mask colour turns the wheel (once it stands still: the
        // category hot-spots are off while it turns).
        r.dir = 0;
        r.turn = r.turn_to;
        let kep = r.slot_kep[3] as u8;
        let area = r.slot_area[3];
        let at = (0..640 * 480).find(|&i| mask[i] == kep).map(|i| [(i % 640) as f32, (i / 640) as f32]).unwrap();
        r.press(&w.game.state, &tables, &mask, at, &[]);
        assert_eq!(r.area as i32, area);
        assert_eq!(r.slot_area[0], area);
    }

    #[test]
    fn printf_fills_the_one_number() {
        assert_eq!(printf_d(b"Cost: %d/day", 12), b"Cost: 12/day".to_vec());
    }
}
