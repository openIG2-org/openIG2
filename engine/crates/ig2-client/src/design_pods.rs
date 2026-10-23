//! The design screen, part 3 (re/screens/design.md §5.1, §5.3, §12): the 3D buttons' press frames (`DesGombFrame`,
//! `desSetGombFrame` ELF 0x503bb0), the hull's `POD%02d` boxes and their lights (`SzinezPodok` 0x500510, `KijelolPod`),
//! the 3D pod pick (`PodKivalaszto` 0x502a60 / exe 0x4b6260), the tech models opening out of the chosen pod
//! (`InitiateDelay`/`DelayVar`, `SubZoomDes`/`SubZoom2Des`, `BetoltHozzavaloTalalmanyok` 0x501650,
//! `MegkeresHozzavaloTalalmanyok` 0x501cc0, `CheckIfTalalmanyfolottvan` 0x501380, `KijelolTalalmany` 0x500dc0,
//! `KiszamolErtek`, `PodKiurito`), their list rows with the `DesignKeret2` frames, and the opening's lasers.

use ig2_sim::ai::design::tech_fits;

use crate::design::{field, pod_count, Design, POD};
use crate::design_edit::Ctx;
use crate::scene::{self, V3};

/// A tech model out of the chosen pod (`AktpodCuccai[i]`, stride 0x28: shown +0, tech +0x24 (ELF 0x12c4134), the
/// mesh, its light (`+0x20` lit), loaded `+0x21`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PodTech {
    pub tech: usize,
    pub loaded: bool,
    pub shown: bool,
    /// `KijelolTalalmany`'s ambient light 0x0e0e86 on it.
    pub lit: bool,
    /// `MoveObj` (world, `SetRelative(false)`), the transparency (`desSetObjTransparencyH`), the rotation
    /// (`RotateObj(1.0471667, rand() % 314 / 200, 0)`, then `RotateObjr(0, 0.05, 0)` a frame).
    pub pos: V3,
    pub clear: f32,
    pub turn: [f32; 3],
}

/// Part 3's state.
#[derive(Debug, Clone, Default)]
pub struct Pods {
    /// `OldPod` (the 3D-picked pod, 0-based, −1 none).
    pub old_pod: i32,
    /// `InitiateDelay`, `DelayVar` (UI ticks the pick has been held).
    initiate: bool,
    delay: f32,
    /// `SubZoomDirDes`, `SubZoom2Des` (the picked tech flying into the pod), `SubZoom2Lecsokkentve`.
    pub sub_dir: i32,
    pub sub2: f32,
    sub2_done: bool,
    /// `AktpodCuccai[0..Osszhanycucc]`, `VannakBetoltveTalalmanyok`.
    pub techs: Vec<PodTech>,
    loaded: bool,
    /// `KivalasztottTalalmany` (−1 none), `VoltmarkijelolveEbbenaframeban`, `DesignPodKiuritomod`.
    pub picked: i32,
    marked: bool,
    clear_row: bool,
    /// The `POD%02d` boxes of the loaded hull: pod (0-based) and its centre after the `AddToScene` scale (×1024),
    /// before the hull's turn (`GetObjCenter`), for the model `boxes_of`; set by the scene.
    pub boxes: Vec<(usize, V3)>,
    pub boxes_of: i32,
    /// The left button's edges since the last tick (`Mouse_Lenyom`, `Mouse_Felenged`) and down (`Mouse_Bal`).
    pressed: bool,
    released: bool,
    /// The press the pick sees: the exe's pick sound comes a tick after the press's click [obs: design-pods.json, t 301].
    pick_press: bool,
    /// `Mouse_Bal`: the left button is down.
    down: bool,
    /// `DesGombFrame`, `DesGombFrameCsokkent`, `DesGombNew`, `DesMelyikGombotOld`.
    pub gomb_frame: i32,
    gomb_falling: bool,
    gomb_new: bool,
    pub gomb_old: i32,
    /// This frame's `DesignKeret2` calls (colour 1 or 2) and `Keret(…, 3)` outlines.
    pub frames2: Vec<u8>,
    pub outlines: Vec<[i32; 4]>,
    /// Per pod of the loaded hull: no tech fits it (`VannakTalalmanyokAPodon` false; `SzinezPodok`'s red light).
    pub no_techs: Vec<bool>,
    /// The cosmetic `rand()` of the models' turn.
    seed: u32,
}

impl Pods {
    pub fn new() -> Self {
        Self { old_pod: -1, picked: -1, gomb_old: -1, boxes_of: -1, seed: 1, ..Self::default() }
    }

    fn rand(&mut self) -> i32 {
        self.seed = self.seed.wrapping_mul(214013).wrapping_add(2531011);
        ((self.seed >> 16) & 0x7fff) as i32
    }
}

/// `desSetObjTransparencyH`: at least 0.01, above 0.95 gone (1).
pub fn transparency(t: f32) -> f32 {
    if t > 0.95 {
        1.0
    } else {
        t.max(0.01)
    }
}

/// `KijelolPod(i, …, noTechs, chosen)`'s ambient light on a pod box: red (no tech fits, or not editable) or green,
/// at 20 % or, on the chosen pod, pulsing `sin(deskeringeto·200)·50 + 50` %; `R | G << 8 | B << 16`.
pub fn pod_light(keringeto: f32, chosen: bool, no_techs: bool, can_modify: bool) -> u32 {
    // The exe rounds the sine to f32 before `·50 + 50` in f64 (exe 0x4b46a9).
    let i = if chosen { ((((keringeto * 200.0) as f64).sin() as f32) as f64 * 50.0 + 50.0) as f32 } else { 20.0 };
    let (hi, lo) = ((i * 255.0 / 100.0) as i32 as u8 as u32, (i * 32.0 / 100.0) as i32 as u8 as u32);
    if !can_modify || no_techs {
        hi | lo << 8 | lo << 16
    } else {
        lo | hi << 8 | lo << 16
    }
}

impl Design {
    /// A 3D button pressed (`DesMelyikGombot = b; DesGombNew = 1`).
    pub(crate) fn push_button(&mut self, b: i32) {
        self.button = b;
        self.pods.gomb_new = true;
    }

    /// The pressed button and its `SetFrame` (`desSetGombFrame(b, f)`: child 6 → b at `f + 60`).
    pub fn button_frame(&self) -> Option<(u16, f32)> {
        (self.button >= 0 && self.button < 100).then(|| (self.button as u16, (self.pods.gomb_frame + 60) as f32))
    }

    /// The left button went down or up (the pick reads the edges once, at the next tick).
    pub(crate) fn pod_edges(&mut self, down: bool) {
        self.pods.down = down;
        if down {
            self.pods.pressed = true;
        } else {
            self.pods.released = true;
        }
    }

    /// `VannakTalalmanyok` (ELF 0x501ad0): a tech the empire has fits the chosen pod.
    fn techs_fit(&self, c: &Ctx) -> bool {
        self.pod != 0 && self.techs_on(c, self.pod)
    }

    /// `VannakTalalmanyokAPodon(k)` (ELF 0x4ffb70 / exe 0x4b7530): `TalalmanyRarakhato(base, t, k, 5, EN, race)` for
    /// some t in 1..=0xc0.
    pub(crate) fn techs_on(&self, c: &Ctx, k: usize) -> bool {
        let Some(d) = self.edited(c.state) else { return false };
        let (hull, pt) = (rd_i32(d, field::HULL).max(0) as usize, rd_i16(d, field::RACE));
        k != 0 && (1..0xc1).any(|t| tech_fits(c.sim, c.techs, c.debug, hull, t, k, 5, c.en(), pt))
    }

    /// The techs the chosen pod takes at status ≥ `min` (`TalalmanyRarakhato(…, min, …)`, t = 1..=0xc0).
    fn pod_techs(&self, c: &Ctx, min: u8) -> Vec<usize> {
        let Some(d) = self.edited(c.state) else { return Vec::new() };
        let (hull, pt) = (rd_i32(d, field::HULL).max(0) as usize, rd_i16(d, field::RACE));
        (1..0xc1).filter(|&t| tech_fits(c.sim, c.techs, c.debug, hull, t, self.pod, min, c.en(), pt)).collect()
    }

    /// `BetoltHozzavaloTalalmanyok` (ELF 0x501650): every tech the pod takes that the empire has seen gets a model
    /// (scale 0.03), turned `(1.0471667, rand() % 314 / 200, 0)`, at (0, −15, −350), hidden.
    fn load_techs(&mut self, c: &Ctx) {
        if self.pod != 0 {
            let list = self.pod_techs(c, 1);
            self.pods.techs.clear();
            for t in list {
                let r = ((self.pods.rand() % 0x13a) / 200) as f32;
                self.pods.techs.push(PodTech { tech: t, loaded: true, pos: [0.0, -15.0, -350.0], clear: 0.01, turn: [1.047_166_7, r, 0.0], ..PodTech::default() });
                self.pods.loaded = true;
            }
        }
        self.pods.picked = -1;
    }

    /// `RemoveHozzavaloTalalmanyok` (ELF 0x501be0 / exe 0x4b6a70).
    fn remove_techs(&mut self) {
        if self.pods.loaded {
            self.pods.techs.clear();
        }
    }

    /// The hull's place (`UnHideAlap` ELF 0x503490): `(0, −15, AddZoomDes − 1400 − SubZoomDes)`.
    pub fn hull_at(&self) -> V3 {
        [0.0, -15.0, self.add_zoom - 1400.0 - self.sub_zoom]
    }

    /// The camera's `ycenter`.
    fn ycenter(&self) -> f32 {
        if self.animated {
            250.0 - self.frame * 10.0 / 60.0
        } else {
            240.0
        }
    }

    /// `TransformPoint` (fov 600, `xcenter` 320) of a world point, `·639/639`.
    pub fn screen(&self, p: V3) -> Option<[f32; 2]> {
        let w = -p[2];
        (w > 1.0).then(|| [(600.0 * p[0] / w + 320.0) * 639.0 / 639.0, (600.0 * p[1] / w + self.ycenter()) * 639.0 / 639.0])
    }

    /// Pod `k`'s box centre in the world (`GetObjCenter` of `POD%02d`).
    pub fn pod_centre(&self, k: usize) -> Option<V3> {
        if self.pods.boxes_of != self.loaded {
            return None;
        }
        let (_, c) = self.pods.boxes.iter().find(|b| b.0 == k)?;
        let q = scene::apply(&self.turn, *c);
        let t = self.hull_at();
        Some([q[0] + t[0], q[1] + t[1], q[2] + t[2]])
    }

    /// `Design3DUpdate`'s part-3 steps before the hull logic: a new hull folds the models (1), the held pick opens
    /// them after 30 ticks (2).
    pub(crate) fn pods_begin(&mut self, c: &Ctx) {
        if self.pods.initiate {
            self.pods.delay += 1.0;
        }
        if self.loaded != self.model {
            self.pods.sub_dir = 0;
            self.sub_zoom = 0.0;
        }
        if self.pods.delay >= 30.0 {
            self.pods.sub_dir = 1;
            self.pods.delay = 0.0;
            self.pods.initiate = false;
            if self.can_modify && self.techs_fit(c) {
                self.remove_techs();
                self.load_techs(c);
            }
        }
        if self.opened {
            if self.pods.sub_dir == -1 {
                self.sub_zoom -= 140.0;
                if self.sub_zoom <= 0.0 {
                    self.sub_zoom = 0.0;
                    self.pods.sub_dir = 0;
                }
            }
            if self.pods.sub_dir == 1 {
                self.sub_zoom = (self.sub_zoom + 140.0).min(1000.0);
            }
        }
    }

    /// The hull reloaded (`LoadAlapVektor` in `Design3DUpdate`): the models go, nothing is picked.
    pub(crate) fn pods_hull_loaded(&mut self) {
        self.remove_techs();
        self.pods.old_pod = -1;
    }

    /// `Design3DUpdate` steps 6, 10, 11 and the rest of the frame (`DesignGfx2D_1`'s rows, `PodKivalaszto`,
    /// `CheckIfTalalmanyfolottvan`).
    pub(crate) fn pods_end(&mut self, c: &Ctx, mask: &[u8]) {
        self.pods.frames2.clear();
        self.pods.outlines.clear();
        let n = self.edited(c.state).map_or(0, |d| pod_count(c.rules, rd_i16(d, field::RACE), self.loaded.max(0)));
        self.pods.no_techs = (0..n).map(|k| !self.techs_on(c, k + 1)).collect();
        // The 3D button: +7 a frame up to 10, then −7 back to 0 (the frame isn't reset by a new press).
        if self.opened && !self.closing && self.frame >= 60.0 {
            if self.pods.gomb_new {
                self.pods.gomb_new = false;
                self.pods.gomb_falling = false;
                self.pods.gomb_old = self.button;
            }
            if self.button != -1 {
                if !self.pods.gomb_falling {
                    self.pods.gomb_frame = (self.pods.gomb_frame + 7).min(10);
                    if self.pods.gomb_frame == 10 {
                        self.pods.gomb_falling = true;
                    }
                } else if self.pods.gomb_frame != 0 {
                    self.pods.gomb_frame = (self.pods.gomb_frame - 7).max(0);
                }
            }
        }
        let released = std::mem::take(&mut self.pods.released);
        // `Design_Main` zeroes `DesignPodKiuritomod` every frame before `Megkeres` reads it (exe 0x4b21cf): the clear
        // row never acts.
        self.pods.clear_row = false;
        let pressed = std::mem::take(&mut self.pods.pressed);
        if self.pods.sub2 > 0.0 {
            self.pods.sub2 -= 140.0;
            if self.pods.sub2 < 0.0 {
                self.pods.sub2 = 0.0;
                self.pods.sub2_done = true;
            }
        }
        if self.pods.sub2_done && self.pods.sub_dir == 1 && self.pods.picked != -1 {
            self.pods.sub_dir = -1;
            self.pods.sub2_done = false;
        }
        if released && self.pods.sub_dir == 1 && self.pods.picked == -1 {
            self.pods.sub_dir = -1;
        }
        if released && self.pods.sub_dir == 1 && self.pods.picked != -1 && self.pods.sub2 == 0.0 && self.sub_zoom == 1000.0 {
            self.pods.sub2 = 1000.0;
        }
        if released {
            self.pods.initiate = false;
        }
        if self.can_modify && self.techs_fit(c) {
            self.place_techs(c);
            self.hover_techs();
        }
        // `DesignGfx2D_1` (open, not closing).
        if self.opened && !self.closing {
            if self.sub_zoom != 0.0 {
                self.unmark_all();
                let mut last = -2;
                let m = self.mouse;
                for i in 0..self.pods.techs.len() {
                    if !self.pods.techs[i].shown {
                        continue;
                    }
                    let (x, y) = (POD[0] as i32, POD[1] as i32 + (i as i32 + 1) * 15);
                    if (y as f32) < m[1] && m[1] <= (y + 15) as f32 && (x as f32) < m[0] && m[0] < (x + 0x73) as f32 && !self.pods.marked && self.sub_zoom == 1000.0 && self.pods.sub2 == 0.0 {
                        self.mark(i);
                        self.pod_frame.ask([x - 3, y + 2, 0x73, 0xd]);
                        self.pods.frames2.push(1);
                    }
                    last = i as i32;
                }
                let (x, y) = (POD[0] as i32, POD[1] as i32 + 0x2d + last * 15);
                self.pods.clear_row = (y as f32) < m[1] && m[1] <= (y + 15) as f32 && (x as f32) < m[0] && m[0] < (x + 0x73) as f32;
                if self.pods.clear_row {
                    self.pod_frame.ask([x - 3, y + 2, 0x73, 0xd]);
                    self.pods.frames2.push(2);
                }
            } else if self.pod != 0 {
                // `!VoltPodEbbenaframeban` (always 0 here): the chosen pod's row.
                self.pod_frame.ask([POD[0] as i32 - 7, POD[1] as i32 + self.pod as i32 * 15 + 2, 0x84, 0xd]);
                self.pods.frames2.push(1);
            }
        }
        self.pod_frame.tick();
        // `GFX_Design`: `PodKivalaszto`, then `CheckIfTalalmanyfolottvan` again.
        if self.opened && !self.closing {
            // The pick needs `Lenyom && Bal` in the same call: a press already released doesn't pick.
            let late = std::mem::replace(&mut self.pods.pick_press, pressed);
            self.pick(c, mask, late && self.pods.down);
            if self.can_modify && self.techs_fit(c) {
                self.hover_techs();
            }
        }
    }

    /// `PodKivalaszto` (exe 0x4b6260, re/screens/design.md §12): with `CanModify` and nothing sliding, a press
    /// within `R = AlapAranyok·50` of a box's projected centre picks the nearest-to-camera box: `DesignAktPod`,
    /// `PlaySoundGlobal(2)`, and with techs to fit the 30-tick hold starts.
    fn pick(&mut self, c: &Ctx, mask: &[u8], pressed: bool) {
        if self.loaded < 0 || !self.can_modify {
            return;
        }
        let Some(d) = self.edited(c.state) else { return };
        let n = pod_count(c.rules, rd_i16(d, field::RACE), self.loaded);
        let r = c.rules.design_screen.scale.get(self.loaded as usize).copied().unwrap_or(0.0) * 50.0;
        if self.add_zoom != 0.0 || self.sub_zoom != 0.0 || self.pods.sub2 != 0.0 {
            return;
        }
        let free = self.spot(c, mask, self.mouse) == 0;
        let mut best: Option<(usize, f32)> = None;
        for k in 0..n {
            let Some(p) = self.pod_centre(k) else { continue };
            let Some(s) = self.screen(p) else { continue };
            let dist = (((s[0] - self.mouse[0]) * (s[0] - self.mouse[0]) + (s[1] - self.mouse[1]) * (s[1] - self.mouse[1])) as f64).sqrt() as f32;
            // The 0x55 tooltip is set and switched off again on the same frame (never shown).
            if dist < r && pressed && free && best.is_none_or(|b| p[2] > b.1) {
                best = Some((k, p[2]));
            }
        }
        let Some((k, _)) = best else { return };
        self.sounds.push(crate::audio::SoundId::global(2));
        if self.pods.old_pod != k as i32 {
            self.pod = k + 1;
        }
        if self.can_modify && self.techs_fit(c) {
            self.pods.initiate = true;
            self.pods.delay = 0.0;
            self.pods.sub2 = 0.0;
        }
        self.pods.old_pod = k as i32;
    }

    /// `MegkeresHozzavaloTalalmanyok` (ELF 0x501cc0): the models the empire has (status 5) on a ring
    /// `(sin a·90, cos a·70 − 15, 50 − SubZoomDes/1000·400)`, `a = i/n·6.283 + deskeringeto`, fading in; the picked one
    /// flies into the pod's box while `SubZoom2Des` runs down (`KiszamolErtek` fits it). Folded (`SubZoomDes` 0), the
    /// models hide and a pending clear empties the pod (`PodKiurito`).
    fn place_techs(&mut self, c: &Ctx) {
        if self.pod == 0 || self.pods.initiate {
            return;
        }
        let en = c.en();
        let have = |t: usize| c.techs.status(t, en) == 5;
        if self.sub_zoom != 0.0 {
            let n = self.pods.techs.iter().filter(|e| have(e.tech) && e.loaded).count();
            let mut v = 0;
            let centre = self.pod_centre(self.pod - 1).unwrap_or(self.hull_at());
            let k = self.keringeto as f64;
            for i in 0..self.pods.techs.len() {
                let e = self.pods.techs[i];
                if !(have(e.tech) && e.loaded) {
                    if e.shown && e.loaded {
                        self.pods.techs[i].shown = false;
                    }
                    continue;
                }
                let mut e = e;
                e.shown = true;
                let a = v as f32 / n as f32;
                // The exe's 6.283, not 2π.
                #[allow(clippy::approx_constant)]
                const TURN: f64 = 6.283;
                let (sin, cos) = ((a as f64 * TURN + k).sin(), (a as f64 * TURN + k).cos());
                if i as i32 == self.pods.picked && (self.pods.sub2 != 0.0 || self.pods.sub_dir == -1) {
                    if self.pods.sub2 != 0.0 {
                        self.fit(c);
                        let f = self.pods.sub2 / 1000.0;
                        e.pos = [
                            (((f * 90.0) as f64 * sin) * f as f64 + ((1.0 - f) * centre[0]) as f64) as f32,
                            ((((f * 70.0) as f64 * cos) - 15.0) * f as f64 + (centre[1] * (1.0 - f)) as f64) as f32,
                            centre[2] * (1.0 - f) + f * -350.0,
                        ];
                        e.clear = transparency(1.0 - self.pods.sub2 / 1000.0);
                    }
                } else {
                    e.clear = transparency(1.0 - self.sub_zoom / 1000.0);
                    e.pos = [(sin * 90.0) as f32, (cos * 70.0 - 15.0) as f32, 50.0 - (self.sub_zoom / 1000.0) * 400.0];
                }
                if self.frame < 60.0 {
                    e.clear = transparency(1.0 - self.frame / 60.0);
                }
                e.turn[1] += 0.05;
                self.pods.techs[i] = e;
                v += 1;
            }
        } else {
            for e in &mut self.pods.techs {
                if have(e.tech) && e.loaded && e.shown {
                    e.shown = false;
                }
            }
            if self.pods.clear_row {
                self.scratch[0x50 + self.pod] = 0;
                self.scratch[0x5d + self.pod] = 0;
                self.recompute(c);
            }
        }
    }

    /// `KiszamolErtek`: the picked tech fitted to the chosen pod, as many as it takes
    /// (`min(GetMaxRarakhato, Talalmanyok[t].+0x24)`).
    fn fit(&mut self, c: &Ctx) {
        let Some(e) = self.pods.techs.get(self.pods.picked as usize) else { return };
        if self.mode == 0 {
            return;
        }
        let k = self.pod;
        self.scratch[0x50 + k] = e.tech as u8;
        let max = self.scratch_max(c, k);
        self.scratch[0x5d + k] = max.clamp(0, 255) as u8;
        self.recompute(c);
    }

    /// `CheckIfTalalmanyfolottvan` (ELF 0x501380 / exe 0x4ad0d0): opened and still, the model within 50 pixels of
    /// the pointer is marked (`KijelolTalalmany`) with an outline on its row; none: no pick.
    fn hover_techs(&mut self) {
        if !(self.sub_zoom == 1000.0 && self.pods.sub2 == 0.0 && self.add_zoom == 0.0 && !self.pods.sub2_done && self.pods.sub_dir == 1) {
            return;
        }
        if self.pod != 0 && !self.pods.marked {
            for i in 0..self.pods.techs.len() {
                let e = self.pods.techs[i];
                if !(e.loaded && e.shown) {
                    continue;
                }
                let Some(s) = self.screen(e.pos) else { continue };
                let d = (((s[0] - self.mouse[0]) * (s[0] - self.mouse[0]) + (s[1] - self.mouse[1]) * (s[1] - self.mouse[1])) as f64).sqrt();
                if d >= 50.0 {
                    self.pods.techs[i].lit = false;
                } else if !self.pods.marked {
                    self.mark(i);
                    self.pods.outlines.push([POD[0] as i32 - 3, POD[1] as i32 + (i as i32 + 1) * 15 + 2, 0x73, 0xd]);
                }
            }
        }
        if !self.pods.marked && self.pods.sub2 == 0.0 && self.pods.sub_dir != -1 {
            self.pods.picked = -1;
        }
    }

    /// `KijelolTalalmany(i)` (ELF 0x500dc0): its light on, it is the pick.
    fn mark(&mut self, i: usize) {
        self.pods.techs[i].lit = true;
        self.pods.marked = true;
        self.pods.picked = i as i32;
    }

    /// `UnjelolMindet` (ELF 0x501310).
    fn unmark_all(&mut self) {
        for e in &mut self.pods.techs {
            e.lit = false;
        }
        self.pods.marked = false;
    }
}

fn rd_i32(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}

fn rd_i16(r: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([r[o], r[o + 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_lights_are_red_without_techs_and_green_with() {
        // 20 %: 255·0.2 = 51, 32·0.2 = 6.
        assert_eq!(pod_light(0.0, false, true, true), 51 | 6 << 8 | 6 << 16);
        assert_eq!(pod_light(0.0, false, false, true), 6 | 51 << 8 | 6 << 16);
        assert_eq!(pod_light(0.0, false, false, false), 51 | 6 << 8 | 6 << 16);
        // The chosen pod at sin 0: 50 %.
        assert_eq!(pod_light(0.0, true, false, true), 16 | 127 << 8 | 16 << 16);
    }

    #[test]
    fn transparency_clamps_like_des_set_obj_transparency_h() {
        assert_eq!(transparency(0.0), 0.01);
        assert_eq!(transparency(0.96), 1.0);
        assert_eq!(transparency(0.5), 0.5);
    }
}
