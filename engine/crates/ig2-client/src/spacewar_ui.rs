//! The battle screen's own state on a live battle (re/screens/spacewar.md): what `SpwC` keeps for the screen and the
//! sim never reads. The camera and its animations (`kamerabeallit` §2.2, `SpaceWarFrissit2` §2.4, `NezetVisszaAllitSet`
//! / `NezetVisszaAllitas` §2.5, `rescalescene`), the drawn ship poses (`seturhajopos` §6.5), the selection and its
//! timers (`kijelol*`, `kijelolesanim` §3.6), the input (`SpwC::main` §7.1.1, `SpaceWarMouse` §7.1.2, `BalClick` /
//! `JobbClick` §7.1.3, `SpaceWarKeyboardHandler` §7.1.5), the weapon column (§5.1), the retreat window (§5.4) and
//! `infoszamol`'s statistics (§5.2). Orders leave as [`Order`]s for the lockstep queue (`Multi_Send`, also in SP).

use ig2_sim::spacebattle::{Battle, Order, Unit};

use crate::spacewar_screen::View;

const HALF_PI: f32 = 1.5707964;
/// The tilt's limits (0xbfc7c82d, 0xbe4ccccd) and its default (0xbf4ccccd).
const TILT_MIN: f32 = -1.5607964;
const TILT_MAX: f32 = -0.2;
const TILT_DEFAULT: f32 = -0.8;
/// `+0x2514` before the opening zoom ends (`initgfx` step 2): one battle unit per 2500 starmap units.
pub const SCALE_START: f32 = 0.0004;
/// The grid plane's height (`(10/2 + 1) · 3`, `initracs`; `+0x928 · scale + +0x24f0`).
pub const PLANE_H: f32 = 18.0;

pub type M3 = [f32; 9];

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}
/// `tav` (exe 0x583bf0): the distance, `sqrt` in f64.
fn tav(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub(a, b);
    ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f64).sqrt() as f32
}
/// `normalizevektor` (ELF 0x4522d0): f64 `sqrt` of the f32 sum; below 1e-5 the vector is left as it is.
fn normalise(v: [f32; 3]) -> [f32; 3] {
    let l = ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) as f64).sqrt() as f32;
    if l < 1e-5 {
        v
    } else {
        v.map(|x| x / l)
    }
}
/// `vektorszog` (ELF 0x4515b0): `2·asin(min(|a − b|, 2)/2)` (f64 asin).
fn angle(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = len(sub(a, b)).min(2.0);
    ((d / 2.0) as f64).asin() as f32 * 2.0
}
/// `MultipleMatrix(a, b, out)` (ELF 0x4518f0): `out = b · a` (row-major).
fn multiple(a: &M3, b: &M3) -> M3 {
    crate::scene::mul(b, a)
}
/// `CreateMatrix(α, β, γ)` as 9 floats.
fn create_matrix(a: f32, b: f32, c: f32) -> M3 {
    let m = crate::starmap::create_matrix(a, b, c);
    [m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2]]
}
/// `CreateMatrix2v(a, v, 0)` (ELF 0x453690): `c = normalise(a × v)`, `b = normalise(a × c)`; the columns are `−a`,
/// `−b`, `−c`.
fn create_matrix2v(a: [f32; 3], v: [f32; 3]) -> M3 {
    let c = normalise(cross(a, v));
    let b = normalise(cross(a, c));
    [-a[0], -b[0], -c[0], -a[1], -b[1], -c[1], -a[2], -b[2], -c[2]]
}
/// `creatematrixfromaxis(axis, angle)` (ELF 0x4520a0): the quaternion form, f32 but for the f64 sine and cosine.
fn axis_matrix(v: [f32; 3], a: f32) -> M3 {
    let c = ((a / 2.0) as f64).cos() as f32;
    let s = ((a / 2.0) as f64).sin() as f32;
    let (x, y) = (v[0] * s, v[1] * s);
    let y2 = y * 2.0;
    let z2 = v[2] * s * 2.0;
    let cx2 = c * x * 2.0;
    let xx2 = x * x * 2.0;
    let zz = v[2] * s * z2;
    [
        1.0 - (y * y2 + zz),
        x * y2 - c * z2,
        x * z2 + c * y2,
        x * y2 + c * z2,
        1.0 - (xx2 + zz),
        y * z2 - cx2,
        x * z2 - c * y2,
        y * z2 + cx2,
        1.0 - (xx2 + y * y2),
    ]
}

/// What the screen keeps per unit (`SpwC`'s unit records: the drawn pose and the selection's timers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitGfx {
    /// ELF `+0xe8` (exe `+0xe4`): the node's matrix (`seturhajopos`).
    pub matrix: M3,
    /// ELF `+0x25c..+0x264`: the model's own turn (asteroids spin, `frissit2`).
    pub turn: [f32; 3],
    /// ELF `+0x189` / `+0x18a` (exe `+0x181`): selected, and as the last `kijelolesanim` saw it.
    pub selected: bool,
    pub was_selected: bool,
    /// ELF `+0x18c` grow, `+0x190` fade, `+0x194` attacked (ms, capped at 1000; exe `+0x184..+0x18c`).
    pub grow: i32,
    pub fade: i32,
    pub attacked: i32,
    /// ELF `+0x1c4` (exe `+0x1b8`): the number-key group.
    pub group: i32,
    /// The box select's snapshot of the selection (`+0x21b8[i]`).
    pub snapshot: bool,
}

impl Default for UnitGfx {
    fn default() -> Self {
        Self { matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], turn: [0.0; 3], selected: false, was_selected: false, grow: 0, fade: 0, attacked: 0, group: 0, snapshot: false }
    }
}

/// `seturhajopos(i)`'s orientation part (ELF 0x686aa0, exe 0x580540; spec §6.5): the matrix from the heading, the
/// previous matrix's third column as the up vector, the model's own turn, and the bank eased in by at most 0.05 a call.
/// The heading's own easing is the sim's (`space-combat.md` §7.2).
pub fn pose(u: &Unit, g: &mut UnitGfx) {
    if u.owner == 0 {
        // Scenery: `RotateObj(spin.x, spin.y + π, spin.z)`.
        g.matrix = create_matrix(u.spin[0], u.spin[1] + std::f32::consts::PI, u.spin[2]);
        return;
    }
    let e = u.turn * u.speed * 20.0 * 2.5;
    let h = [u.heading.x, u.heading.y, u.heading.z];
    let up = [-g.matrix[2], -g.matrix[5], -g.matrix[8]];
    let m1 = create_matrix2v(h, up);
    let m2 = create_matrix(g.turn[0] + HALF_PI, g.turn[1] + HALF_PI, g.turn[2]);
    g.matrix = multiple(&m2, &m1);
    if (g.matrix[0].abs() >= 0.01 || e.abs() >= 0.01) && u.ship_type.turn != 0.0 {
        let m1 = create_matrix2v(h, [1.0, 0.0, 0.0]);
        let mut mt = multiple(&create_matrix(HALF_PI, HALF_PI, 0.0), &m1);
        if e.abs() >= 0.01 {
            let col2 = [g.matrix[2], g.matrix[5], g.matrix[8]];
            let axis = normalise(cross([u.mid_heading.x, u.mid_heading.y, u.mid_heading.z], col2));
            mt = multiple(&mt, &axis_matrix(axis, -e));
        }
        let want = [mt[0], mt[3], mt[6]];
        let cur = [g.matrix[0], g.matrix[3], g.matrix[6]];
        let mut a = angle(want, cur);
        if a.abs() > 0.01 {
            let mut axis = cross(want, cur);
            if axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2] <= 0.0 {
                axis[0] = 1.0;
            }
            let axis = normalise(axis);
            let step = if a / 6.0 < 0.05 { a / 6.0 } else { 0.05 };
            if step < a {
                a = step;
            }
            g.matrix = multiple(&g.matrix, &axis_matrix(axis, -a));
        }
    }
}

/// A view as the camera fields hold it (spec §2.5's origin `O` and target `T`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pose {
    pub yaw: f32,
    pub tilt: f32,
    pub zoom: f32,
    pub xcenter: f32,
    pub pan: [f32; 3],
}

/// The designs `infoszamol` values the selection by (`Tervek`, `Hajok`).
pub struct Plans<'a> {
    pub designs: &'a [Vec<ig2_sim::spacewar::Design>],
    pub ships: &'a [ig2_sim::spacewar::Ship],
    /// Per gun, its building's tech (`EpuletekLeiro[type].tech`; −1 none), and `TalalmanyokrEro`.
    pub gun_tech: Vec<i32>,
    pub firepower: &'a [i32],
    /// `Flottak` (0x80 each: the name first, the owner at `+0x15`).
    pub fleets: &'a [[u8; ig2_sim::save::FLEET_SIZE]],
}

impl Plans<'_> {
    /// `Tervek[e][d]`'s name (the record's first bytes).
    fn plan_name(&self, e: i32, d: i32) -> Vec<u8> {
        let Some(p) = self.designs.get(e.max(0) as usize).and_then(|x| x.get(d.max(0) as usize)) else { return Vec::new() };
        p.raw[..p.raw.iter().position(|&c| c == 0).unwrap_or(0x14).min(0x14)].to_vec()
    }

    /// A fighter group's firepower (`+0x9c`): its design's attack × the count.
    fn fighter_value(&self, g: &ig2_sim::spacebattle::Fighter) -> i32 {
        self.designs.get(g.owner.max(0) as usize).and_then(|e| e.get(g.design.max(0) as usize)).map_or(0, |d| d.attack).wrapping_mul(g.count)
    }

    /// A unit's value: a plan-less unit's own design (`+0xc`), else the sum over its ships' designs, by the plan owner
    /// (`+0x1d4`, the original owner).
    fn unit_value(&self, u: &Unit) -> i32 {
        let fp = |d: i32| self.designs.get(u.original_owner.max(0) as usize).and_then(|e| e.get(d.max(0) as usize)).map_or(0, |d| d.attack);
        match u.ships.first() {
            Some(&s) if s >= 0 => u.ships.iter().take(u.ship_count.max(0) as usize).filter(|&&s| s >= 0).map(|&s| self.ships.get(s as usize).map_or(0, |x| fp(x.design as i32))).sum(),
            _ => fp(u.design),
        }
    }
}

/// What a frame or tick of the screen asks the window for.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// An order for the lockstep queue (`Multi_Send(.., EN, ..)`).
    Order(Order),
    /// `PlaySoundGlobal(0)`.
    Click,
    /// `SetIdoSpeed(1, false); SetIdoPause(false, false)`: the formation phase's start button.
    Start,
    /// The zoom-out ended (`AktKepKell = starmaphovaleptovabb or 2`): leave for the starmap.
    Left,
    /// A battle sound (`PlaySound(&SpWar_Hangok[i])` after the volume write).
    Sound(crate::spacewar_fx::Sound),
    /// A weapon loop's start, volume or end (`lovedeksound`).
    Loop(crate::spacewar_fx::Loop),
}

/// The battle screen's state for battle slot `slot`.
pub struct SpaceWarUi {
    pub slot: usize,
    pub units: Vec<UnitGfx>,
    /// The planet guns' selection (`+0x89` and the box snapshot `+0x8b`).
    pub guns: Vec<(bool, bool)>,
    // Camera fields (§2.1, exe `+0xc5c..`): the pivot and its wanted point, yaw, tilt, zoom, the quarter-turn yaw
    // of the radar and keys (`+0xd6c`), the zoom/yaw/tilt rates (`+0x2300..+0x2308`).
    pub pivot: [f32; 3],
    pub pan_target: [f32; 3],
    pub yaw: f32,
    pub tilt: f32,
    pub zoom: f32,
    pub xcenter: f32,
    pub snap_yaw: f32,
    pub zoom_rate: f32,
    pub yaw_rate: f32,
    pub tilt_rate: f32,
    /// `SpaceWar_Autoscroll`'s pan speeds (exe globals 0xE5DFD8/DC, ×5).
    pub scroll: [i32; 2],
    /// `+0x2514` and `starmapeltolas` (`+0x2500`, exe `+0x1f74`).
    pub scale: f32,
    pub origin: [f32; 3],
    /// The default view (`+0x24d4..+0x24e8`) and the starmap camera saved at entry (`+0x2528..+0x2540`).
    pub defaults: Pose,
    pub starmap: Pose,
    /// `spwkamerapont` / `spwforgaspont` (the smoothed eye and target, §2.2) and `initkamera`.
    pub eye: [f32; 3],
    pub target: [f32; 3],
    init_camera: bool,
    // The view animation (§2.5): origin O, target T, start (ms), length in frames (`+0x2520`),
    // `visszaallitasidoszorzo`, and its flags.
    o: Pose,
    t: Pose,
    anim_start: f32,
    anim_frames: f32,
    anim_mult: f32,
    /// `+0x229e`: an animation runs (or the info view is held); `+0x229c` settled; `+0x229d` back; `+0x22ec` info
    /// mode; `+0x22e8` the followed unit.
    pub restoring: bool,
    pub settled: bool,
    back: bool,
    info_mode: bool,
    follow: i32,
    /// `+0x2545` entering, `+0x2544` leaving, `+0x2524` the first frame, `+0x2547` the big info box (0, 1 in, 2 out).
    pub entering: bool,
    pub leaving: bool,
    first_frame: bool,
    pub big_info: u8,
    /// `spacewarelfade` (exe 0xE72E14): 0.001 → 1 over the opening zoom, back over the closing one.
    pub fade: f32,
    /// `spwradarcounter` (0..100).
    pub radar: i32,
    // Ticks and frames.
    pub ticks: u32,
    last_frame_ms: u32,
    /// `spwvillogas`, `acttick2`.
    pub blink: i32,
    // The mouse (§7.1.2): held, press and release events, the press points, the state flags.
    left: bool,
    right: bool,
    left_press: bool,
    right_press: bool,
    left_release: bool,
    right_release: bool,
    left_at: [f32; 2],
    right_at: [f32; 2],
    suppress: bool,
    rotating: bool,
    height: bool,
    pub boxing: bool,
    box_spoke: bool,
    prev_mouse: [f32; 2],
    pub mouse: [f32; 2],
    /// `+0x2230` hovered index (−1 none), `+0x2234` its kind (1 unit, 2 gun); `+0x2244` the pointer is on the plane.
    pub hovered: i32,
    /// `Mouse_Bal`/`Mouse_Jobb` and this frame's `Mouse_Lenyom`/`Mouse_Felenged`.
    held: (bool, bool),
    latched: Option<u32>,
    press: bool,
    release: bool,
    pub hover_kind: i32,
    on_plane: bool,
    plane_point: [f32; 3],
    /// The height mode's point (`+0x228c..+0x2294`) and the pole's ground point (`+0x2284/88`).
    pub height_point: [f32; 3],
    pub pole: [f32; 2],
    height_press: [f32; 2],
    /// `MousePointerType` (0 normal, 2 own, 3 attack, 4 fire), the saved one (`+0x24c4`) and the armed group
    /// (`+0x24c8`, `+0x24cc`).
    pub pointer: i32,
    saved_pointer: i32,
    pub armed: i32,
    armed_marker: i32,
    // The weapon column (§5.1) and `infoszamol`'s groups: counters 0..50, then per group the barrels, the enabled
    // barrels, the slowest reload (0..0xaa) and the state (−1 none, 3 some off, 6 all off, 7 all on).
    pub column: [i32; 10],
    /// The formation phase's end (`+0x2d86`, ELF `+0x342e`): set by the start button, the phase (`+0x2d85`) is over
    /// once a column slot has slid out. The exe clears both bytes in the screen's tick; the sim's
    /// `Battle::formation_choice` keeps the start value, so the screen keeps the end here.
    pub formation_leaving: bool,
    pub formation_done: bool,
    /// `+0x3430` / `+0x3434` (exe `+0x2d88` / `+0x2d8c`): the formation chosen in the phase and the one loaded
    /// (0: `felallas1`, the start's; −1 without the phase). [`SpaceWarUi::formation_due`] places the side anew.
    pub formation_pick: i32,
    pub formation_loaded: i32,
    /// The voices (`SpeakingDirector::Play(&SpaceWarSpeaker, line, kind)`, §7.4) due, and the throttle
    /// `beszedlasttick` / `acttick2` (+1 a UI tick with the gfx up; 30 ticks between lines).
    pub voices: Vec<(i32, i32)>,
    speech_last: i64,
    acttick2: i64,
    /// The fighter chase camera (§2.2, §7.4): `spwcameramode` (key C), `vadaszlinked`, `aktvadasz` (the followed group,
    /// not drawn; −1 none), `elozovadasz`, and this frame's exact follow (eye, target).
    pub chase: bool,
    linked: bool,
    pub chased: i32,
    prev_chased: i32,
    chase_view: Option<([f32; 3], [f32; 3])>,
    /// The guns' marker timers (gun `+0x10`: `kijelolesanim` adds the frame's ms up to 1000; §7.4).
    pub gun_t: Vec<i32>,
    /// The guns' selection timers (`+0x8c` grow, `+0x90` fade, `+0x94` attacked; §3.6) and the previous selection.
    pub gun_timers: Vec<[i32; 3]>,
    gun_was: Vec<bool>,
    /// The small box's title (`infoszamol`'s snapshot): one unit's plan name and the tech under it; the big box's.
    title: Vec<u8>,
    title_tech: i32,
    pub big: crate::spacewar_hud::BigInfo,
    /// `MousePointerType` (0 normal, 2 own unit, 3 attack, 4 the armed fire cursor) and `spwckezdotimer` (ms).
    pub start_ms: Option<u32>,
    pub group_count: [i32; 10],
    pub group_enabled: [i32; 10],
    pub group_reload: [i32; 10],
    pub group_state: [i32; 10],
    infoszamol_timer: i32,
    pub stats: crate::spacewar_hud::Stats,
    /// `SPWRetreatWindow`, and the opening click not yet released.
    pub retreat_window: bool,
    retreat_opening: bool,
    /// `gfxvadaszdefendattack` (0..100).
    pub stance: i32,
    /// The camera cluster's button animation (`MelyikGombot`, `GombFazis`, `KezeloGombFrame`).
    pub button: i32,
    pub button_phase: i32,
    pub button_frame: f32,
    /// The info model's turn (`matrixforgat(.., 0.05)` a frame).
    pub spin: f32,
    /// The weapon-group masks (`fegyvernevek[g]` record `+0x20`: weapon kind bits).
    pub masks: [u32; 10],
}

/// The weapon groups' masks of the ELF's table (spec §7.1.3; the exe image has zeros there [u: the exe fills them
/// from somewhere not read]): lasers, guns, paralyser, ship manipulator, anti-matter ray, destructor ray, torpedo,
/// bomb, virus bomb, retreat.
pub const GROUP_MASKS: [u32; 10] = [0x1f, 0x1e0, 0x200, 0x400, 0x800, 0x1000, 0xe000, 0x30000, 0x40000, 0x80000];

impl SpaceWarUi {
    /// `SpaceWar_Belep` → `initgfx` (spec §1.1): the battle starts from the starmap camera (`cam`: pivot `(0, XStart,
    /// −YStart)`, yaw `StarmapRotate`, tilt `StarmapPitch`, zoom `600 − Zoom`) shifted by the battle's origin, at scene
    /// scale 0.0004; every own unit selected (`flottafelallit`); the default yaw (step 23).
    pub fn enter(slot: usize, b: &Battle, cam: &crate::starmap::Camera, now_ms: u32) -> Self {
        let origin = [0.0, b.map_pos[0], -b.map_pos[1]];
        // `initgfx` (exe 0x575050): the starmap camera's pan minus the slot's raw `starmapeltolas` (+0x1f74: h, x,
        // y as stored, y not negated as the drawing origin's is).
        let pivot = [0.0, cam.x - b.map_pos[0], -cam.y - b.map_pos[1]];
        let starmap = Pose { yaw: cam.rotate, tilt: cam.pitch, zoom: 600.0 - cam.zoom, xcenter: 320.0, pan: pivot };
        let mut s = Self {
            slot,
            units: vec![UnitGfx::default(); b.units.len()],
            guns: vec![(false, false); b.guns.len()],
            pivot,
            pan_target: pivot,
            yaw: starmap.yaw,
            tilt: starmap.tilt,
            zoom: starmap.zoom,
            xcenter: 320.0,
            snap_yaw: 0.0,
            zoom_rate: 0.0,
            yaw_rate: 0.0,
            tilt_rate: 0.0,
            scroll: [0; 2],
            scale: SCALE_START,
            origin,
            defaults: Pose::default(),
            starmap,
            eye: [0.0; 3],
            target: [0.0; 3],
            init_camera: true,
            o: Pose::default(),
            t: Pose::default(),
            anim_start: now_ms as f32,
            anim_frames: 80.0,
            anim_mult: 1.0,
            restoring: false,
            settled: false,
            back: false,
            info_mode: false,
            follow: -1,
            entering: false,
            leaving: false,
            first_frame: true,
            big_info: 0,
            fade: 0.001,
            radar: 0,
            ticks: 0,
            last_frame_ms: now_ms,
            blink: 0,
            left: false,
            right: false,
            left_press: false,
            right_press: false,
            left_release: false,
            right_release: false,
            left_at: [0.0; 2],
            right_at: [0.0; 2],
            suppress: false,
            rotating: false,
            height: false,
            boxing: false,
            box_spoke: false,
            prev_mouse: [0.0; 2],
            mouse: [0.0; 2],
            hovered: -1,
            held: (false, false),
            latched: None,
            press: false,
            release: false,
            hover_kind: 0,
            on_plane: false,
            plane_point: [0.0; 3],
            height_point: [0.0; 3],
            pole: [0.0; 2],
            height_press: [0.0; 2],
            pointer: 0,
            saved_pointer: 0,
            armed: -1,
            armed_marker: -1,
            column: [0; 10],
            formation_leaving: false,
            formation_done: false,
            formation_pick: if b.formation_choice { 0 } else { -1 },
            formation_loaded: if b.formation_choice { 0 } else { -1 },
            voices: Vec::new(),
            speech_last: 0,
            acttick2: 0,
            chase: false,
            linked: false,
            chased: -1,
            prev_chased: -1,
            chase_view: None,
            gun_t: b.guns.iter().map(|g| g.hits).collect(),
            gun_timers: vec![[0; 3]; b.guns.len()],
            gun_was: vec![false; b.guns.len()],
            title: Vec::new(),
            title_tech: -1,
            big: Default::default(),
            start_ms: None,
            group_count: [0; 10],
            group_enabled: [0; 10],
            group_reload: [0; 10],
            group_state: [-1; 10],
            infoszamol_timer: 0,
            stats: Default::default(),
            retreat_window: false,
            retreat_opening: false,
            stance: b.sides.get(b.human.max(0) as usize).map_or(0, |s| s.fighter_aggression),
            button: 0,
            button_phase: 0,
            button_frame: 0.0,
            spin: 0.0,
            masks: GROUP_MASKS,
        };
        s.defaults.yaw = Self::default_yaw(b);
        s.select_fleet(b);
        for (u, g) in b.units.iter().zip(&mut s.units) {
            pose(u, g);
        }
        s
    }

    /// `initgfx` step 23: a planet battle looks from π; else from the own units' centre `c`: `v = normalise(0, c.x,
    /// −c.y)`, `π − asin(v.x)` when `v.y ≥ 0`, else `asin(v.x)` (f64 `asin`); no own unit: 0.
    fn default_yaw(b: &Battle) -> f32 {
        if b.planet >= 0 && b.planet_owner >= 0 {
            return std::f32::consts::PI;
        }
        let own: Vec<&Unit> = b.units.iter().filter(|u| u.alive && u.owner == b.human).collect();
        if own.is_empty() {
            return 0.0;
        }
        let n = own.len() as f32;
        let c = [own.iter().map(|u| u.pos.y).sum::<f32>() / n, own.iter().map(|u| u.pos.z).sum::<f32>() / n];
        let v = normalise([0.0, c[0], -c[1]]);
        if v[2] >= 0.0 {
            (std::f64::consts::PI - (v[1] as f64).asin()) as f32
        } else {
            (v[1] as f64).asin() as f32
        }
    }

    /// The camera fields as the renderer reads them.
    pub fn view(&self) -> View {
        View { pivot: self.pivot, yaw: self.yaw, tilt: self.tilt, zoom: self.zoom, restoring: self.restoring, xcenter: self.xcenter, origin: self.origin, scale: self.scale }
    }

    /// The frame's eye and target (§2.3): the smoothed pair, or straight from the fields during a view animation.
    pub fn eye_target(&self) -> ([f32; 3], [f32; 3]) {
        if self.restoring {
            self.view().eye_target()
        } else {
            self.chase_view.unwrap_or((self.eye, self.target))
        }
    }

    /// `starmaperosseg` (§3.3): `log(zoom / starmapszorzo)/3 + 0.4` clamped to 0..1 (`starmapszorzo` = scale · 2500).
    pub fn erosseg(&self) -> f32 {
        let k = self.scale * crate::spacewar_screen::SCENE_SCALE;
        (((self.zoom / k) as f64).ln() / 3.0 + 0.4).clamp(0.0, 1.0) as f32
    }

    // ------------------------------------------------------------------------------------------- camera

    /// `kamerabeallit` (spec §2.2): the targets from the fields, or the first live fighter group in chase mode (eye
    /// at the group, looking 10 along its heading), then the smoothing (×0.85 a tick) until the chase links.
    fn kamerabeallit(&mut self, b: &Battle) {
        if self.chase && self.restoring {
            self.chase = false;
        }
        if (self.tilt as f64) < -1.5607963267948965 {
            self.tilt = TILT_MIN;
        }
        let m = crate::starmap::create_matrix(self.tilt, -self.yaw, HALF_PI);
        let mut p = self.pivot;
        let mut te: [f32; 3] = std::array::from_fn(|i| m[i][2] * self.zoom + p[i]);
        if !self.chase {
            (self.linked, self.chased) = (false, -1);
        } else if let Some(g) = b.fighters.iter().position(|f| f.alive) {
            if self.prev_chased != g as i32 {
                (self.prev_chased, self.linked, self.chased) = (g as i32, false, -1);
            }
            if self.linked {
                self.chased = g as i32;
            }
            let f = &b.fighters[g];
            te = [f.pos.x, f.pos.y, f.pos.z];
            p = [f.pos.x + f.dir.x * 10.0, f.pos.y + f.dir.y * 10.0, f.pos.z + f.dir.z * 10.0];
        }
        if self.init_camera {
            self.init_camera = false;
            (self.eye, self.target) = (te, p);
            return;
        }
        let d2: f32 = (0..3).map(|i| (self.eye[i] - te[i]) * (self.eye[i] - te[i])).sum();
        if self.chase && !self.linked && d2.sqrt() < 1.0 {
            self.linked = true;
        }
        if !self.linked && !self.restoring {
            for i in 0..3 {
                self.eye[i] = te[i] + (self.eye[i] - te[i]) * 0.85;
                self.target[i] = p[i] + (self.target[i] - p[i]) * 0.85;
            }
        } else {
            (self.eye, self.target) = (te, p);
        }
    }

    /// `rescalescene(k)` (spec §2.5): the camera's lengths and the scene scale times `k`; the wanted pan := the pan.
    fn rescale(&mut self, k: f32) {
        self.eye = self.eye.map(|x| x * k);
        self.target = self.target.map(|x| x * k);
        self.scale *= k;
        self.zoom *= k;
        self.pivot = self.pivot.map(|x| x * k);
        self.pan_target = self.pivot;
    }

    fn current(&self) -> Pose {
        Pose { yaw: self.yaw, tilt: self.tilt, zoom: self.zoom, xcenter: self.xcenter, pan: self.pivot }
    }

    /// `NezetVisszaAllitSet()` (spec §2.5): animate from the current view to the defaults (A), keeping the yaw in
    /// info mode (B), or to the selection (C).
    fn anim_set(&mut self, b: &Battle, now_ms: u32) {
        self.anim_mult = 1.0;
        self.restoring = true;
        let two_pi = 6.2831855f32;
        let y = self.yaw as f64 / std::f64::consts::TAU;
        self.o = Pose { yaw: ((y - y.floor()) * std::f64::consts::TAU) as f32, ..self.current() };
        self.t = Pose { xcenter: 320.0, ..self.defaults };
        if !self.info_mode || self.stats.total == 0 {
            if self.info_mode {
                self.t.yaw = self.o.yaw;
            }
            while self.t.yaw <= 0.0 {
                self.t.yaw += two_pi;
            }
            while self.o.yaw <= 0.0 {
                self.o.yaw += two_pi;
            }
            let d = self.t.yaw - self.o.yaw;
            if d > std::f32::consts::PI {
                self.t.yaw -= two_pi;
            } else if d < -std::f32::consts::PI {
                self.t.yaw += two_pi;
            }
            self.follow = -1;
        } else {
            self.follow = self.units.iter().position(|g| g.selected).map_or(-1, |i| i as i32);
            let (pan, zoom, _) = self.selection_centre(b);
            (self.t.pan, self.t.zoom) = (pan, zoom);
            self.t.xcenter = 160.0;
            self.t.yaw = self.o.yaw;
            self.t.tilt = TILT_DEFAULT;
        }
        self.anim_start = now_ms as f32;
    }

    /// `kijelolescenter` (0x6b1a40, spec §2.5): no selection → (0, 40); one → its position and `scale · 1024 · 3`;
    /// more → the centroid and 2.5 × the largest distance from it. The flag: no selected unit is alive any more.
    fn selection_centre(&self, b: &Battle) -> ([f32; 3], f32, bool) {
        let sel: Vec<usize> = (0..self.units.len()).filter(|&i| self.units[i].selected).collect();
        let gone = !sel.iter().any(|&i| b.units[i].alive);
        let p = |i: usize| [b.units[i].pos.x, b.units[i].pos.y, b.units[i].pos.z];
        match sel.len() {
            0 => ([0.0; 3], 40.0, gone),
            1 => (p(sel[0]), b.units[sel[0]].ship_type.scale * 1024.0 * 3.0, gone),
            n => {
                let mut c = [0.0f32; 3];
                for &i in &sel {
                    let q = p(i);
                    for k in 0..3 {
                        c[k] += q[k];
                    }
                }
                let c = c.map(|x| x / n as f32);
                let far = sel.iter().map(|&i| tav(p(i), c)).fold(0.0f32, f32::max);
                (c, far * 2.5, gone)
            }
        }
    }

    /// `gorbe2(x)`: `x²` below 1, else `2x − 1`.
    fn gorbe2(x: f32) -> f32 {
        if x < 1.0 {
            x * x
        } else {
            2.0 * x - 1.0
        }
    }

    /// `NezetVisszaAllitas()` (spec §2.5), every frame while an animation runs. It reads the frame time `+0x22f0`
    /// before `screenfrissites` stores this frame's (exe 0x57f800, ELF 0x6a7780: the call precedes `SDL_GetTicks`),
    /// so the curve runs on the previous frame's clock (`prev_ms`).
    fn anim_frame(&mut self, b: &Battle, prev_ms: u32, now_ms: u32, out: &mut Vec<Out>) {
        if !self.restoring {
            return;
        }
        let t = (prev_ms as f32 - self.anim_start) / self.anim_frames * self.anim_mult / 50.0;
        if self.follow != -1 && !self.back && self.info_mode {
            let (pan, zoom, _) = self.selection_centre(b);
            (self.t.pan, self.t.zoom) = (pan, zoom);
        }
        let g = crate::spacewar_screen::gorbe(t);
        // The big box slides with the view animation (`+0x2548`; inverted on the way back).
        if self.big_info != 0 {
            self.big.f = if self.back { 1.0 - g } else { g }.clamp(0.0, 1.0);
        }
        let (o, tt) = (self.o, self.t);
        self.yaw = if !self.back && self.info_mode { o.yaw + Self::gorbe2(t / 2.0) } else { o.yaw + (tt.yaw - o.yaw) * g };
        self.tilt = o.tilt + (tt.tilt - o.tilt) * g;
        self.xcenter = o.xcenter + (tt.xcenter - o.xcenter) * g;
        let l = ((o.zoom / tt.zoom) as f64).ln();
        self.zoom = (o.zoom as f64 / ((g * l as f32) as f64).exp()) as f32;
        let mut p = g;
        if (l as f32).abs() > 0.1 {
            p = 1.0 - (tt.zoom - self.zoom) / (tt.zoom - o.zoom);
        }
        self.pivot = std::array::from_fn(|i| o.pan[i] + (tt.pan[i] - o.pan[i]) * p);
        if self.entering {
            self.fade = g;
        }
        if self.leaving {
            self.fade = 1.0 - g;
        }
        if self.fade == 0.0 {
            self.fade = 0.001;
        }
        if g >= 1.0 {
            if !self.back {
                if !self.info_mode {
                    self.restoring = false;
                }
                self.settled = true;
            } else {
                self.restoring = false;
                self.settled = true;
                self.big_info = 0;
            }
            if self.leaving {
                self.restoring = false;
                self.fade = 0.001;
                out.push(Out::Left);
            }
            if self.entering {
                self.rescale(crate::spacewar_screen::SCENE_SCALE);
                self.entering = false;
                self.fade = 1.0;
                // `spwckezdotimer`: the MP banner's clock starts here.
                self.start_ms = Some(now_ms);
            }
        }
        self.pan_target = self.pivot;
        let (_, _, gone) = self.selection_centre(b);
        if self.info_mode && self.follow != -1 && gone {
            self.info_toggle(b, now_ms);
        }
    }

    /// The first frame's block (end of `screenfrissites`, spec §2.5): the opening zoom to the default view.
    fn first_frame_block(&mut self, b: &Battle, now_ms: u32, from_options: bool) {
        self.anim_frames = 80.0;
        self.defaults.tilt = TILT_DEFAULT;
        self.defaults.zoom = Self::bezoomolasztav(b) * self.scale;
        self.defaults.pan = [0.0; 3];
        self.info_mode = false;
        self.anim_set(b, now_ms);
        self.back = false;
        self.settled = false;
        self.first_frame = false;
        self.entering = true;
        self.snap_yaw = ((self.defaults.yaw / HALF_PI) as f64 + 0.5) as i32 as f32 * HALF_PI;
        self.anim_mult = if from_options { 5.0 } else { 1.0 };
    }

    /// `bezoomolasztav` (0x6a9ed0): 2.5 × the farthest live unit from the origin, 200 with none.
    fn bezoomolasztav(b: &Battle) -> f32 {
        let far = b.units.iter().filter(|u| u.alive && u.disabled == 0).map(|u| tav([0.0; 3], [u.pos.x, u.pos.y, u.pos.z])).fold(0.0f32, f32::max);
        if far == 0.0 {
            200.0
        } else {
            far * 2.5
        }
    }

    /// `SpwC::Starmapvisszaallit` (0x6b4020, spec §4.1.2): the closing zoom back to the saved starmap camera.
    pub fn leave(&mut self, b: &Battle, now_ms: u32) {
        if self.leaving {
            return;
        }
        self.rescale(SCALE_START);
        self.leaving = true;
        self.anim_frames = 80.0;
        self.defaults = self.starmap;
        self.info_mode = false;
        self.anim_set(b, now_ms);
    }

    /// `infomodeonoff` (spec §7.1.4): the camera flies to the selection and the big box slides in, or back.
    fn info_toggle(&mut self, b: &Battle, now_ms: u32) {
        if !self.settled {
            return;
        }
        if !self.restoring && self.stats.sel_ships != 0 {
            self.big_info = 1;
            self.anim_frames = 20.0;
            self.defaults = Pose { yaw: 0.0, tilt: TILT_DEFAULT, zoom: 200.0, ..self.defaults };
            self.info_mode = true;
            self.anim_set(b, now_ms);
            self.back = false;
            self.settled = false;
        } else if self.restoring {
            self.big_info = 2;
            self.anim_frames = 20.0;
            // `NezetVisszaAllitSet2`: back to where the info view started from.
            let back = self.o;
            self.o = self.current();
            self.t = back;
            self.anim_start = now_ms as f32;
            self.back = true;
            self.settled = false;
        }
    }

    // ------------------------------------------------------------------------- the UI tick (100 Hz, §7.2)

    /// `FRISSIT_SpaceWar2` → `SpaceWarFrissit2` (spec §2.4, §7.2), 30 times a second.
    pub fn tick(&mut self, b: &Battle, plans: &Plans, out: &mut Vec<Out>) {
        self.ticks += 1;
        self.acttick2 += 1;
        if !self.first_frame && !self.entering {
            self.radar = if self.leaving { (self.radar - 1).max(0) } else { (self.radar + 1).min(100) };
        }
        self.blink += 1;
        if !self.restoring {
            self.zoom -= self.zoom / 25.0 * self.zoom_rate;
            if self.zoom <= 5.0 {
                self.zoom = 5.0;
            }
            if self.zoom_rate < 0.0 && self.zoom > 400.0 {
                self.zoom = 400.0;
            }
            self.clamp_pan(b);
            let d = sub(self.pan_target, self.pivot);
            if d.iter().any(|x| x.abs() >= 0.01) {
                let mut d = d.map(|x| x / 4.0);
                let l = len(d) / 4.0;
                if l > 1.5 {
                    d = d.map(|x| x * 1.5 / l);
                }
                for (p, d) in self.pivot.iter_mut().zip(d) {
                    *p += d;
                }
            } else {
                self.pivot = self.pan_target;
            }
            self.yaw += self.yaw_rate / 60.0;
            self.tilt += self.tilt_rate / 250.0;
            self.tilt = self.tilt.clamp(TILT_MIN, TILT_MAX);
        }
        if self.button_phase == 1 {
            self.button_frame = (self.button_frame + 1.0).min(10.0);
        } else if self.button_phase == 2 {
            self.button_frame = (self.button_frame - 1.0).max(0.0);
        }
        self.infoszamol_timer -= 1;
        if self.infoszamol_timer < 0 {
            self.infoszamol(b, plans, out);
            self.infoszamol_timer = 5;
        }
        if !self.first_frame && !self.entering {
            self.column_tick(b);
        }
        if !self.first_frame {
            self.kamerabeallit(b);
        }
        self.zoom_rate = 0.0;
        self.yaw_rate = 0.0;
    }

    /// The pivot clamp (§2.4): two cells inside the arena on each axis.
    /// `SpaceWar_Autoscroll` (ELF 0x67dac0 / exe 0x5624c0), every UI tick (re/screens/spacewar.md §2.6): the
    /// pointer within 8 pixels of the window's edge (`win` in window pixels, the window `size`) pans the view, unless
    /// a mouse button is held; the speed ramps by 5 towards ±30 per axis.
    pub fn autoscroll(&mut self, win: [i32; 2], size: [i32; 2], button: bool) {
        let edge = |p: i32, n: i32| {
            let mut v = 0;
            if p < 8 {
                v = -30;
            }
            // An unsigned compare: a pointer left of (above) the window counts as past the far edge.
            if p as u32 > n.wrapping_sub(8) as u32 {
                v = 30;
            }
            v
        };
        let (mut x, mut y) = if button { (0, 0) } else { (edge(win[0], size[0]), edge(win[1], size[1])) };
        // The length to the unit circle of 30, y's with x's new value (f32, the root in f64).
        let y2 = y * y;
        let norm = |a: i32, b2: i32| ((30 * a) as f32 / (((a as f32) * (a as f32) + b2 as f32) as f64).sqrt() as f32) as i32;
        if x * x + y2 != 0 {
            x = norm(x, y2);
        }
        if x * x + y2 != 0 {
            y = norm(y, x * x);
        }
        for (v, t) in self.scroll.iter_mut().zip([x, y]) {
            if t > *v {
                *v += 5;
            }
            if t < *v {
                *v -= 5;
            }
        }
        if self.scroll != [0, 0] {
            self.move_view((self.scroll[0] as f64 / 5.0) as f32, (self.scroll[1] as f64 / 5.0) as f32);
        }
    }

    /// `SpaceWar_Move` (exe 0x563450; the ELF's 0x67c5e0 shrinks a refused step, the exe has no such loop): the pivot
    /// moves `(dx, dy)` screen steps turned by the yaw, `(scale · plane + zoom) / 600 · 0.6` each, and the wanted
    /// point with it.
    fn move_view(&mut self, dx: f32, dy: f32) {
        let k = self.scale * PLANE_H + self.zoom;
        let (c, s) = ((self.yaw as f64).cos() as f32, (self.yaw as f64).sin() as f32);
        let mut step = |a: f32, b: f32| {
            self.pivot[1] = (self.pivot[1] as f64 - (a * k / 600.0) as f64 * 0.6) as f32;
            self.pivot[2] = ((b * k / 600.0) as f64 * 0.6 + self.pivot[2] as f64) as f32;
        };
        if dx.abs() > 0.0 {
            step(c * dx, s * dx);
        }
        if dy.abs() > 0.0 {
            step(-(s * dy), c * dy);
        }
        if dx.abs() > 0.0 || dy.abs() > 0.0 {
            self.pan_target = self.pivot;
        }
    }

    fn clamp_pan(&mut self, b: &Battle) {
        let (lo, hi) = (|k: usize, c: f32| b.bounds[k] + 2.0 * c, |k: usize, c: f32| b.bounds[k] - 2.0 * c);
        self.pivot[0] = self.pivot[0].clamp(lo(4, b.cell[2]), hi(5, b.cell[2]).max(lo(4, b.cell[2])));
        self.pivot[1] = self.pivot[1].clamp(lo(0, b.cell[0]), hi(1, b.cell[0]).max(lo(0, b.cell[0])));
        self.pivot[2] = self.pivot[2].clamp(lo(2, b.cell[1]), hi(3, b.cell[1]).max(lo(2, b.cell[1])));
    }

    /// The column's slide counters (§5.1): a group's button shows while own ships are selected that carry it; the
    /// retreat button while the side may retreat; all of them in the formation phase.
    fn column_tick(&mut self, b: &Battle) {
        let may_retreat = b.sides.get(b.human.max(0) as usize).is_some_and(|s| s.may_retreat);
        for i in 0..10 {
            let formation = self.formation(b);
            let on = if self.leaving || self.formation_leaving {
                false
            } else if i == 9 {
                formation || may_retreat
            } else {
                formation || (self.group_count[i] != 0 && self.stats.own != 0 && self.stats.sel_ships != 0)
            };
            let c = self.column[i] + if on { 1 } else { -1 };
            // A slot slid out while the phase ends: the formation phase is over (exe 0x56d870, `+0x2d85` as a u16 = 0).
            if c < 0 && self.formation_leaving {
                (self.formation_leaving, self.formation_done) = (false, true);
            }
            self.column[i] = c.clamp(0, 50);
        }
    }

    /// The formation phase (`+0x2d85`): the human picks a formation before the battle starts (§7.1.3).
    pub fn formation(&self, b: &Battle) -> bool {
        b.formation_choice && !self.formation_done
    }

    /// `SpaceWarFrissit2`'s formation step (exe 0x56d870, after the column; not on the first frame or while
    /// entering, `+0x1f98` / `+0x1fb9`): a pick that differs from the loaded formation is loaded (`loadfelallas`,
    /// the caller's `Game::pick_formation`), which re-places the human's side.
    pub fn formation_due(&mut self) -> Option<i32> {
        if self.first_frame || self.entering || self.formation_pick == self.formation_loaded {
            return None;
        }
        self.formation_loaded = self.formation_pick;
        Some(self.formation_pick)
    }

    /// The units' poses after a re-placement (no battle tick runs in the paused phase).
    pub fn placed(&mut self, b: &Battle) {
        if self.units.len() < b.units.len() {
            self.units.resize(b.units.len(), UnitGfx::default());
        }
        for (u, g) in b.units.iter().zip(&mut self.units) {
            if u.alive {
                pose(u, g);
            }
        }
    }

    // ------------------------------------------------------------------------------------------ frames

    /// One frame of `screenfrissites` (spec §3): the view animation, the selection timers, the first frame's block.
    pub fn frame(&mut self, b: &Battle, now_ms: u32, out: &mut Vec<Out>) {
        if self.first_frame {
            self.first_frame = false;
            self.kamerabeallit(b);
            self.first_frame = true;
        }
        self.anim_frame(b, self.last_frame_ms, now_ms, out);
        // `screenfrissites` (§2.3): a linked chase follows the live group exactly.
        self.chase_view = None;
        if let (true, true, Some(f)) = (self.chase, self.linked, usize::try_from(self.chased).ok().and_then(|g| b.fighters.get(g))) {
            self.chase_view = Some(([f.pos.x, f.pos.y, f.pos.z], [f.pos.x + f.dir.x * 10.0, f.pos.y + f.dir.y * 10.0, f.pos.z + f.dir.z * 10.0]));
        }
        let dt = now_ms.saturating_sub(self.last_frame_ms) as i32;
        self.last_frame_ms = now_ms;
        self.kijelolesanim(b, dt);
        self.spin += 0.05;
        if self.first_frame {
            self.first_frame_block(b, now_ms, false);
        }
    }

    /// After every battle tick: the drawn poses (`seturhajopos` from `Mozgatas`) and the asteroids' turn
    /// (`frissit2`, §6.6).
    pub fn battle_ticked(&mut self, b: &Battle) {
        if self.units.len() < b.units.len() {
            self.units.resize(b.units.len(), UnitGfx::default());
        }
        if self.guns.len() < b.guns.len() {
            self.guns.resize(b.guns.len(), (false, false));
        }
        for (u, g) in b.units.iter().zip(&mut self.units) {
            if u.alive {
                match u.kind {
                    8 => g.turn[1] = (g.turn[1] as f64 + 0.007) as f32,
                    9 => g.turn[1] = (g.turn[1] as f64 + 0.005) as f32,
                    10 => g.turn[1] = (g.turn[1] as f64 + 0.01) as f32,
                    _ => {}
                }
                pose(u, g);
            }
        }
    }

    /// `kijelolesanim(dt)` (spec §3.6): the grow and fade timers per unit, and the attacked mark of the selected
    /// units' targets.
    fn kijelolesanim(&mut self, b: &Battle, dt: i32) {
        let mut marked = vec![false; self.units.len()];
        for (i, u) in b.units.iter().enumerate() {
            let Some(g) = self.units.get_mut(i) else { continue };
            if !u.alive {
                continue;
            }
            if g.selected != g.was_selected {
                if g.selected {
                    g.grow = 0;
                }
                g.fade = 0;
                g.was_selected = g.selected;
            }
            g.grow = (g.grow + dt).min(1000);
            if !g.selected {
                g.fade = (g.fade + dt).min(1000);
            }
            if u.order == ig2_sim::spacebattle::order::ATTACK && g.selected && u.target_kind == ig2_sim::spacebattle::target_kind::UNIT {
                if let Some(m) = u.target.and_then(|t| marked.get_mut(t)) {
                    *m = true;
                }
            }
        }
        for (g, m) in self.units.iter_mut().zip(marked) {
            g.attacked = if m { (g.attacked + dt).min(1000) } else { 0 };
        }
        // The guns as the units (`+0x89` selected, `+0x8a` previous): grow, fade, attacked while a selected unit's attack
        // order names the gun.
        let n = b.guns.len();
        self.gun_timers.resize(n, [0; 3]);
        self.gun_was.resize(n, false);
        let mut hit = vec![false; n];
        for (i, u) in b.units.iter().enumerate() {
            if u.alive && u.order == ig2_sim::spacebattle::order::ATTACK && self.units.get(i).is_some_and(|g| g.selected) && u.target_kind == ig2_sim::spacebattle::target_kind::GUN {
                if let Some(h) = u.target.and_then(|t| hit.get_mut(t)) {
                    *h = true;
                }
            }
        }
        for (k, (gun, &h)) in b.guns.iter().zip(&hit).enumerate() {
            if gun.building == 0 {
                continue;
            }
            let sel = self.guns.get(k).is_some_and(|g| g.0);
            let t = &mut self.gun_timers[k];
            if sel != self.gun_was[k] {
                if sel {
                    t[0] = 0;
                }
                t[1] = 0;
                self.gun_was[k] = sel;
            }
            t[0] = (t[0] + dt).min(1000);
            if !sel {
                t[1] = (t[1] + dt).min(1000);
            }
            t[2] = if h { (t[2] + dt).min(1000) } else { 0 };
        }
        // The guns' marker timers: while set (1 for a foreign planet's, 1000 for the human's), + dt up to 1000.
        self.gun_t.resize(b.guns.len(), 0);
        for (t, g) in self.gun_t.iter_mut().zip(&b.guns) {
            if *t == 0 {
                *t = g.hits.min(1);
            }
            if *t != 0 {
                *t = (*t + dt).min(1000);
            }
        }
    }

    // ------------------------------------------------------------------------------------------ selection

    fn selectable(b: &Battle, i: usize) -> bool {
        b.units.get(i).is_some_and(|u| u.alive && u.owner == b.human && u.owner != 0 && u.disabled == 0)
    }

    /// `SpeakingDirector::Play(&SpaceWarSpeaker, line, kind)` behind the screen's throttle (`beszedlasttick + 30 <
    /// acttick2`): unit `i`'s voice (`i % 3 + 1`) when it is a capital ship (type below 8).
    pub fn voice(&mut self, b: &Battle, i: usize, kind: i32) {
        if b.units.get(i).is_some_and(|u| u.kind < 8) && self.speech_last + 30 < self.acttick2 {
            self.speech_last = self.acttick2;
            self.voices.push((i as i32 % 3 + 1, kind));
        }
    }

    /// The battle's own voices (`dechp`, `retreat`): the dying and the dead (6 the human's dying; 7 / 9 the human's / the
    /// others' dead) and a retreat (3 the human's, 10 the others'; line 1).
    pub fn battle_voice(&mut self, b: &Battle, e: &ig2_sim::spacebattle::BattleEvent) {
        use ig2_sim::spacebattle::BattleEvent;
        match *e {
            // `dechp`'s line is the unit's id (`+0x1a8`) % 3 + 1.
            BattleEvent::Voice { unit, kind } if self.speech_last + 30 < self.acttick2 => {
                self.speech_last = self.acttick2;
                self.voices.push((b.units.get(unit).map_or(0, |u| u.id) % 3 + 1, kind));
            }
            BattleEvent::Retreat { empire } if self.speech_last + 30 < self.acttick2 => {
                self.speech_last = self.acttick2;
                self.voices.push((1, if empire == b.human { 3 } else { 10 }));
            }
            _ => {}
        }
    }

    /// `kijelolesleszed(false)`: nothing selected.
    fn deselect(&mut self) {
        for g in &mut self.units {
            g.selected = false;
        }
        for g in &mut self.guns {
            g.0 = false;
        }
    }

    /// `flottafelallit` (spec §1.1 step 11, key A): every own unit, and the guns of an own planet.
    fn select_fleet(&mut self, b: &Battle) {
        self.deselect();
        for i in 0..self.units.len() {
            if Self::selectable(b, i) {
                self.units[i].selected = true;
            }
        }
        if b.planet_owner == b.human {
            for g in &mut self.guns {
                g.0 = true;
            }
        }
    }

    // ------------------------------------------------------------------------------------------ infoszamol

    /// `infoszamol` (ELF 0x6a4080, exe 0x574040; spec §5.2, §7.1.3): the selection's statistics and the weapon groups
    /// (both only while something is selected; the groups keep their last values otherwise), the fighter stance's
    /// message while the slider differs from the side's. `plans` gives a design's firepower (`Tervek[e][d] +0x2c`) and a
    /// ship's design (`Hajok[s] +2`). Not ported: its writes to the sim (the sides' outmatched flag and the `Hajok +4`
    /// counter of the last ship, `re/deviations.md`).
    pub fn infoszamol(&mut self, b: &Battle, plans: &Plans, out: &mut Vec<Out>) {
        let me = b.human;
        let mut st = crate::spacewar_hud::Stats { rank_sum: -1, ..Default::default() };
        // The overview (exe 0x574040, the `+0x1f2c` block): own, enemy (the relation byte 1) and other ships,
        // fighters and firepower; the guns count by their building's tech.
        let side = |o: i32| if o == me { 0 } else if b.hostile.get(me.max(0) as usize).and_then(|r| r.get(o.max(0) as usize)).copied() == Some(1) { 1 } else { 2 };
        for u in b.units.iter().filter(|u| u.alive && u.owner != 0) {
            let k = side(u.owner);
            st.sides[0][k] += u.ship_count;
            st.sides[2][k] += plans.unit_value(u);
        }
        for g in b.fighters.iter().filter(|g| g.alive) {
            let k = side(g.owner);
            st.sides[1][k] += g.count;
            st.sides[2][k] += plans.fighter_value(g);
        }
        for (k, g) in b.guns.iter().enumerate() {
            let tech = plans.gun_tech.get(k).copied().unwrap_or(-1);
            if g.building != 0 && g.weapon >= 0 && tech > 0 {
                st.sides[2][side(b.planet_owner)] += plans.firepower.get(tech as usize).copied().unwrap_or(0);
            }
        }
        // The counters (`kijelolesujraszamol`'s count of the live selection).
        for (i, u) in b.units.iter().enumerate() {
            if u.alive && self.units.get(i).is_some_and(|g| g.selected) {
                (st.total, st.sel_ships, st.last, st.last_kind) = (st.total + 1, st.sel_ships + 1, i as i32, 1);
                st.own += (u.owner == me) as i32;
            }
        }
        for (k, g) in self.guns.iter().enumerate() {
            if g.0 && b.guns.get(k).is_some_and(|x| x.building != 0) {
                (st.total, st.sel_guns, st.last, st.last_kind) = (st.total + 1, st.sel_guns + 1, k as i32, 2);
                st.own += (b.planet_owner == me) as i32;
            }
        }
        // The title for one unit: a ship's plan name and hull (when it has an owner and a ship list), a gun's building.
        self.title.clear();
        self.title_tech = -1;
        if st.total == 1 && st.sel_ships == 1 {
            if let Some(u) = b.units.get(st.last.max(0) as usize).filter(|u| u.owner != 0 && u.ships.first().is_some_and(|&s| s != 0)) {
                self.title = plans.plan_name(u.owner, u.design);
                self.title_tech = u.model;
            }
        } else if st.total == 1 && st.sel_guns == 1 {
            self.title_tech = plans.gun_tech.get(st.last.max(0) as usize).copied().unwrap_or(-1);
        }
        // The big box's ship rows (one live ship) and weapon list (one own ship with a design).
        self.big.ship = None;
        self.big.weapons.clear();
        if st.total == 1 && st.last_kind == 1 {
            if let Some(u) = b.units.get(st.last.max(0) as usize).filter(|u| u.alive && u.owner != 0) {
                let fleet = u.ships.first().filter(|&&s| s >= 0).and_then(|&s| plans.ships.get(s as usize)).map(|s| s.fleet as i32).unwrap_or(-1);
                let rec = (fleet >= 0).then(|| plans.fleets.get(fleet as usize)).flatten();
                let owner = rec.map_or(b.planet_owner, |r| r[0x15] as i32);
                let name = rec.map(|r| r[..r.iter().position(|&c| c == 0).unwrap_or(0x14).min(0x14)].to_vec());
                self.big.ship = Some(crate::spacewar_hud::BigShip { plan: plans.plan_name(u.original_owner, u.design), tech: u.model, owner, fleet: name });
                if u.design != 0 && st.own != 0 {
                    if let Some(p) = plans.designs.get(u.original_owner.max(0) as usize).and_then(|e| e.get(u.design.max(0) as usize)) {
                        let n = i32::from_le_bytes(p.raw[0x4c..0x50].try_into().unwrap()).clamp(0, 12) as usize;
                        for k in 1..=n {
                            if p.raw[0x5d + k] != 0 {
                                self.big.weapons.push((p.raw[0x50 + k] as i32, p.raw[0x5d + k] as i32));
                            }
                        }
                    }
                }
            }
        }
        if st.total != 0 {
            let mut sum = [0i32; 32];
            let mut en = [0i32; 32];
            let mut maxr = [0i32; 32];
            // A mount without a set timer keeps the last value (the exe starts it as a pointer: clamped, 0xaa).
            let mut v = 0xaa;
            for (i, u) in b.units.iter().enumerate() {
                if !u.alive || u.owner == 0 || !self.units.get(i).is_some_and(|g| g.selected) {
                    continue;
                }
                st.ships += 1;
                st.damage += u.damage;
                st.max_hp += u.hp;
                st.shield_used += u.shield_used;
                st.shield += u.shield;
                st.value += plans.unit_value(u);
                if u.experience >= 0 && u.ship_count != 0 {
                    let q = u.experience / u.ship_count;
                    st.rank_sum = if st.rank_sum < 0 { q } else { st.rank_sum + q };
                }
                if u.owner != me {
                    continue;
                }
                for m in u.mounts.iter().filter(|m| m.weapon >= 0) {
                    let k = (m.kind & 31) as usize;
                    sum[k] += m.count;
                    if (u.weapon_mask >> k) & 1 != 0 {
                        en[k] += m.count;
                    }
                    if m.timer_set != 0 {
                        v = 0xa9 - m.timer.wrapping_mul(0xaa) / m.timer_set;
                    }
                    maxr[k] = maxr[k].max(v.clamp(0, 0xaa));
                }
            }
            for (k, g) in self.guns.iter().enumerate() {
                if g.0 && b.guns.get(k).is_some_and(|x| x.building != 0) {
                    st.ships += 1;
                }
            }
            for g in 0..10 {
                let (mut c, mut e, mut r) = (0, 0, 0);
                for k in 0..0x14 {
                    if (self.masks[g] >> k) & 1 != 0 {
                        c += sum[k];
                        e += en[k];
                        r = r.max(maxr[k]);
                    }
                }
                self.group_count[g] = c;
                self.group_enabled[g] = e;
                self.group_reload[g] = r;
                self.group_state[g] = if c == 0 {
                    -1
                } else if e == 0 {
                    6
                } else if e == c {
                    7
                } else {
                    3
                };
            }
        }
        self.stats = st;
        let side = me.max(0) as usize;
        if b.sides.get(side).is_some_and(|s| s.fighter_aggression != self.stance) {
            out.push(Out::Order(Order::FighterStance { side, value: self.stance }));
        }
    }

    // ------------------------------------------------------------------------------------------ input

    /// `Mouse_Darabol`'s `AktIkon` with `Mouse_Megfogva`: while a button stays down the hot-spot it went down on stays
    /// the one under the pointer (`SpwC::main` latches 0x1f, 0x47, 0x48, 0x4a, 0x4b, 0x4c; the others act once).
    /// The release frame still sees the latched spot.
    pub fn latch(&mut self, raw: Option<u32>) -> Option<u32> {
        if !(self.held.0 || self.held.1) {
            self.latched = raw;
        }
        self.latched
    }

    /// The mouse buttons as `Mouse_Lekerdez` reports them (`left`/`right` held now): the edges for the hot-spots
    /// (`Mouse_Lenyom`, `Mouse_Felenged`) and, in `SpwC::main`, the battlefield's own flags: releases always, presses
    /// only on the battlefield (`AktIkon` 0x4c, spec §7.1.0).
    pub fn buttons(&mut self, left: bool, right: bool, on_field: bool) {
        self.press = (left && !self.held.0) || (right && !self.held.1);
        self.release = (!left && self.held.0) || (!right && self.held.1);
        if self.held.0 && !left {
            self.left_release = true;
            self.left = false;
        }
        if self.held.1 && !right {
            self.right_release = true;
            self.right = false;
        }
        if on_field && left && !self.held.0 {
            self.left_press = true;
            self.left = true;
            self.left_at = self.mouse;
        }
        if on_field && right && !self.held.1 {
            self.right_press = true;
            self.right = true;
            self.right_at = self.mouse;
        }
        self.held = (left, right);
    }
}

/// Where a unit or gun is on the screen (`GetObjCenter` + `TransformPoint`, virtual pixels) and its pick radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projected {
    pub at: [f32; 2],
    pub radius: f32,
}

impl SpaceWarUi {
    /// `SpaceWarMouse` (spec §7.1.2) and `SpaceWarMouseMove`, after the frame is drawn: `units` and `guns` are the
    /// projected centres; `plane` the grid plane's point under the pointer (`SikMetszespont`), if the ray meets it
    /// inside the arena.
    #[allow(clippy::too_many_arguments)]
    pub fn mouse_frame(&mut self, b: &Battle, units: &[Option<Projected>], guns: &[Option<Projected>], plane: Option<[f32; 3]>, pole: impl Fn([f32; 2], f32) -> f32, shift: bool, field: bool, out: &mut Vec<Out>) {
        self.on_plane = plane.is_some();
        if let Some(p) = plane {
            self.plane_point = p;
        }
        let (l, r) = (self.left, self.right);
        let (lp, rp, lr, rr) = (self.left_press, self.right_press, self.left_release, self.right_release);
        let m = self.mouse;
        if !self.boxing {
            if !self.height && !self.rotating && !rr {
                if !self.suppress && lr && !r && field {
                    self.bal_click(b, shift, out);
                } else if !self.suppress && ((lp && r) || (rp && l)) {
                    self.rotating = true;
                } else if !self.suppress && !l && r && self.own_selected(b) && self.hovered == -1 {
                    let movable = (0..self.units.len()).any(|i| self.units[i].selected && b.units[i].alive && b.units[i].owner == b.human && b.units[i].ship_type.top_speed > 0.01);
                    if movable && (self.right_at[1] - m[1]).abs() > 5.0 {
                        self.height = true;
                        self.height_press = self.right_at;
                        self.pole = [self.plane_point[1], self.plane_point[2]];
                    }
                } else if !self.suppress && l && ((self.left_at[1] - m[1]).abs() > 5.0 || (self.left_at[0] - m[0]).abs() > 5.0) && !self.restoring {
                    self.boxing = true;
                    self.box_spoke = false;
                    for g in &mut self.units {
                        g.snapshot = g.selected;
                    }
                    for g in &mut self.guns {
                        g.1 = g.0;
                    }
                }
            } else if !self.height && !self.rotating && rr {
                if field {
                    self.jobb_click(b, out);
                }
            } else if self.rotating {
                if !l && !r {
                    self.rotating = false;
                }
            } else if self.height {
                if self.height {
                    // `magassagkirak`: the height under the pointer's row on the pole, snapped to a grid cell.
                    let h = pole(self.pole, m[1]);
                    let cell = b.cell[2].max(1e-6);
                    let c = ((h - b.bounds[4]) / cell).floor().clamp(1.0, (b.grid[2] - 2).max(1) as f32);
                    self.height_point = [b.bounds[4] + (c + 0.5) * cell, self.pole[0], self.pole[1]];
                }
                if !rr && lp {
                    self.height = false;
                    self.suppress = true;
                    self.rotating = true;
                } else if rr {
                    if field {
                        self.jobb_click(b, out);
                    }
                    self.height = false;
                }
            }
        } else if self.right_press {
            for g in &mut self.units {
                g.selected = g.snapshot;
            }
            for g in &mut self.guns {
                g.0 = g.1;
            }
            self.boxing = false;
            self.suppress = true;
            self.rotating = true;
        } else if self.left_release || !l {
            self.boxing = false;
            if self.pointer == 4 {
                self.pointer = self.saved_pointer;
                self.armed = -1;
                self.armed_marker = -1;
            }
        } else {
            let (x0, x1) = (self.left_at[0].min(m[0]), self.left_at[0].max(m[0]));
            let (y0, y1) = (self.left_at[1].min(m[1]), self.left_at[1].max(m[1]));
            let inside = |p: [f32; 2]| p[0] > x0 && p[0] < x1 && p[1] > y0 && p[1] < y1;
            for (i, g) in self.units.iter_mut().enumerate() {
                g.selected = shift && g.snapshot;
                if !g.selected && Self::selectable(b, i) && units.get(i).copied().flatten().is_some_and(|p| inside(p.at)) {
                    g.selected = true;
                }
            }
            // A box that takes its first own unit speaks, during the drag [obs: ui5, the exe at t703 of a drag from
            // t690; the unit: inf, the first of them].
            if !self.box_spoke {
                if let Some(i) = (0..self.units.len()).find(|&i| self.units[i].selected && b.units.get(i).is_some_and(|u| u.alive && u.owner == b.human)) {
                    self.box_spoke = true;
                    self.voice(b, i, 1);
                }
            }
            let mine = b.planet_owner == b.human;
            for (k, g) in self.guns.iter_mut().enumerate() {
                g.0 = shift && g.1;
                if mine && !g.0 && b.guns.get(k).is_some_and(|x| x.building != 0) && guns.get(k).copied().flatten().is_some_and(|p| inside(p.at)) {
                    g.0 = true;
                }
            }
        }
        if !self.left && !self.right {
            self.suppress = false;
        }
        if self.rotating && !self.restoring {
            self.yaw -= (self.prev_mouse[0] - m[0]) / 100.0;
            self.tilt -= (self.prev_mouse[1] - m[1]) / 200.0;
            self.tilt = self.tilt.clamp(TILT_MIN, TILT_MAX);
        }
        if !self.rotating && !self.height && !self.boxing {
            self.mouse_move(b, units, guns);
        }
        self.prev_mouse = m;
        self.left_press = false;
        self.right_press = false;
        self.left_release = false;
        self.right_release = false;
    }

    fn own_selected(&self, b: &Battle) -> bool {
        (0..self.units.len()).any(|i| self.units[i].selected && b.units[i].owner == b.human)
    }

    /// `SpaceWarMouseMove` (ELF 0x6b2c90, exe 0x56fa60; spec §7.1.2 as corrected): the nearest unit (own or not
    /// cloaked) whose projected centre is closer than its pick radius (at least 10 pixels) and than 400 pixels; a gun
    /// (radius 10) wins only when nearer. The cursor: own → 2; a foreign unit or gun → 3 unless nothing is selected or
    /// a selected live unit is the human's enemy.
    fn mouse_move(&mut self, b: &Battle, units: &[Option<Projected>], guns: &[Option<Projected>]) {
        let m = self.mouse;
        let dist = |p: [f32; 2]| (((p[0] - m[0]) * (p[0] - m[0]) + (p[1] - m[1]) * (p[1] - m[1])) as f64).sqrt() as f32;
        let (mut best, mut idx, mut kind) = (400.0f32, -1i32, 0);
        for (i, u) in b.units.iter().enumerate() {
            if !u.alive || (u.owner != b.human && u.cloaked) {
                continue;
            }
            let Some(Some(p)) = units.get(i) else { continue };
            let d = dist(p.at);
            if best > d && p.radius.max(10.0) > d {
                (best, idx, kind) = (d, i as i32, 1);
            }
        }
        for (k, g) in b.guns.iter().enumerate() {
            if g.building == 0 {
                continue;
            }
            let Some(Some(p)) = guns.get(k) else { continue };
            let d = dist(p.at);
            if best > d && 10.0 > d {
                (best, idx, kind) = (d, k as i32, 2);
            }
        }
        (self.hovered, self.hover_kind) = if kind != 0 { (idx, kind) } else { (-1, 0) };
        if self.pointer < 4 {
            let me = b.human.max(0) as usize;
            let enemy_selected = (0..self.units.len()).any(|i| self.units[i].selected && b.units[i].alive && b.hostile[me][b.units[i].owner.max(0) as usize] == 1);
            let total = self.units.iter().any(|g| g.selected) || self.guns.iter().any(|g| g.0);
            self.pointer = match self.hover_kind {
                0 => 0,
                1 if b.units[self.hovered as usize].owner == b.human => 2,
                2 if b.planet_owner == b.human => 2,
                _ if !total || enemy_selected => 2,
                _ => 3,
            };
        }
    }

    /// `BalClick` (ELF 0x6b13c0, exe 0x566810; spec §7.1.3): select.
    fn bal_click(&mut self, b: &Battle, shift: bool, out: &mut Vec<Out>) {
        if b.ended || self.restoring {
            return;
        }
        out.push(Out::Click);
        if self.pointer == 4 {
            self.pointer = 0;
            self.saved_pointer = 0;
            self.armed = -1;
            self.armed_marker = -1;
        }
        let h = self.hovered;
        match self.hover_kind {
            0 => self.deselect(),
            1 if b.units[h as usize].disabled == 0 => {
                let i = h as usize;
                if !shift {
                    self.deselect();
                    self.units[i].selected = true;
                    // `BalClick` (ELF 0x6b13c0): an own live unit selected alone speaks.
                    if b.units[i].alive && b.units[i].owner == b.human {
                        self.voice(b, i, 1);
                    }
                } else {
                    if !self.own_selected(b) {
                        self.deselect();
                    }
                    self.units[i].selected = !self.units[i].selected;
                }
            }
            2 => {
                let k = h as usize;
                if !shift {
                    self.deselect();
                    self.guns[k].0 = true;
                } else {
                    self.guns[k].0 = !self.guns[k].0;
                }
            }
            _ => {}
        }
    }

    /// `JobbClick` (ELF 0x6b0210, exe 0x569190; spec §7.1.3): orders for the selected own units.
    fn jobb_click(&mut self, b: &Battle, out: &mut Vec<Out>) {
        if b.ended || self.restoring || self.formation(b) {
            return;
        }
        out.push(Out::Click);
        let mine: Vec<usize> = (0..self.units.len()).filter(|&i| self.units[i].selected && b.units[i].alive && b.units[i].owner == b.human).collect();
        let h = self.hovered;
        match (self.pointer, self.hover_kind) {
            (3, 1) => {
                for &i in &mine {
                    out.push(Out::Order(Order::Attack { unit: i, target: h as usize }));
                    self.voice(b, i, 2);
                }
                if b.planet_owner == b.human {
                    for (k, g) in self.guns.iter().enumerate() {
                        if g.0 && b.guns.get(k).is_some_and(|x| x.building != 0) {
                            out.push(Out::Order(Order::GunTarget { gun: k, unit: h as usize }));
                        }
                    }
                }
            }
            (3, 2) => {
                for &i in &mine {
                    out.push(Out::Order(Order::AttackGun { unit: i, gun: h as usize }));
                }
            }
            (4, 1) if self.armed >= 0 => {
                let mask = self.masks[self.armed as usize];
                let t = &b.units[h as usize];
                let mut sent = false;
                for &i in &mine {
                    let u = &b.units[i];
                    let d = tav([u.pos.x, u.pos.y, u.pos.z], [t.pos.x, t.pos.y, t.pos.z]);
                    for (k, m) in u.mounts.iter().enumerate() {
                        if m.weapon >= 0 && mask & (1 << m.kind) != 0 && m.kind != 2 && d <= m.range as f32 && m.timer < 0 {
                            out.push(Out::Order(Order::DirectFire { unit: i, target: h as usize, mount: k }));
                            sent = true;
                        }
                    }
                }
                if sent && self.armed_marker == -1 {
                    self.pointer = self.saved_pointer;
                    self.armed = -1;
                }
            }
            (0 | 2, _) => {
                for &i in &mine {
                    let (x, y, z) = if self.height {
                        let p = self.height_point;
                        b.screen_point(p[0], p[1], p[2])
                    } else if self.on_plane {
                        let p = self.plane_point;
                        b.screen_point(b.units[i].pos.x, p[1], p[2])
                    } else {
                        continue;
                    };
                    out.push(Out::Order(Order::Move { unit: i, x, y, z }));
                    self.voice(b, i, 0);
                }
            }
            _ => {}
        }
    }

    /// `SpwC::main`'s hot-spots (spec §7.1.1): `spot` = the `AktIkon` under the pointer (`None` = nothing), with the
    /// frame's button state. Returns whether the battlefield (0x4c) has the pointer.
    pub fn hot_spots(&mut self, b: &Battle, spot: Option<u32>, now_ms: u32, out: &mut Vec<Out>) -> bool {
        let (l, r) = self.held;
        let (press, release) = (self.press, self.release);
        let any = l || r;
        if !any {
            self.button_phase = 2;
        }
        match spot {
            Some(0x1f) => {
                let (dx, dy) = (self.mouse[0] - 321.0, self.mouse[1] - 421.0);
                let th = self.snap_yaw as f64;
                let (s, c) = (th.sin() as f32, th.cos() as f32);
                // Every call, hovering too (exe `SpwC::main` 0x57b9e0 has no button test; a button only latches the
                // hot-spot through `Mouse_Megfogva`).
                self.pan_target[1] = -((c * dx + s * dy) / (94.0 / (b.bounds[1] - b.bounds[0])));
                self.pan_target[2] = (c * dy - s * dx) / (67.0 / (b.bounds[3] - b.bounds[2]));
            }
            Some(id @ (0x46 | 0x49)) if press => {
                out.push(Out::Click);
                if !self.restoring {
                    self.anim_frames = 40.0;
                    self.defaults = Pose { yaw: 0.0, tilt: if id == 0x46 { TILT_MIN } else { TILT_DEFAULT }, zoom: self.scale * 200.0, pan: [0.0; 3], xcenter: 320.0 };
                    self.info_mode = false;
                    self.anim_set(b, now_ms);
                    self.back = false;
                    self.settled = false;
                }
                (self.button_frame, self.button_phase, self.button) = (0.0, 1, if id == 0x46 { 4 } else { 1 });
            }
            Some(id @ (0x47 | 0x48)) if any && !release => {
                if press {
                    out.push(Out::Click);
                }
                let s = if id == 0x47 { -1.0 } else { 1.0 };
                self.yaw += if l { 0.1 * s } else { -0.1 * s };
                (self.button_phase, self.button) = (1, if id == 0x47 { 5 } else { 2 });
            }
            Some(0x4a) if any && !release => {
                if press {
                    out.push(Out::Click);
                }
                self.zoom_rate = if l { 1.0 } else { -1.0 };
                (self.button_phase, self.button) = (1, 0);
            }
            Some(0x4b) if any && !release => {
                if press {
                    out.push(Out::Click);
                }
                self.tilt = (self.tilt + if l { 0.0314159 } else { -0.0314159 }).clamp(TILT_MIN, TILT_MAX);
                (self.button_phase, self.button) = (1, 3);
            }
            Some(0x42) if press => self.info_toggle(b, now_ms),
            Some(0x37) if any && !release => self.stance = (((self.mouse[0] - 50.0) as i32 * 100) / 0x71).clamp(0, 100),
            Some(id @ 0x21..=0x2a) if press => self.weapon_button(b, (id - 0x21) as usize, l, out),
            _ => {}
        }
        spot == Some(0x4c)
    }

    /// The weapon column's buttons (spec §7.1.3): right arms the fire cursor, left toggles the group's mounts, the
    /// tenth opens the retreat window; in the formation phase the tenth starts the battle.
    fn weapon_button(&mut self, b: &Battle, k: usize, left: bool, out: &mut Vec<Out>) {
        if self.formation(b) {
            // `+0x342e = 1; SetIdoSpeed(1); SetIdoPause(false)`: the column slides out, then the phase ends.
            if left && k == 9 && !self.formation_leaving {
                self.formation_leaving = true;
                out.push(Out::Start);
            } else if left && k < 9 {
                self.formation_pick = k as i32;
            }
            return;
        }
        if !left {
            if k < 9 && self.saved_pointer != 4 {
                if self.armed == k as i32 {
                    (self.pointer, self.armed, self.armed_marker, self.saved_pointer) = (0, -1, -1, 0);
                } else {
                    (self.armed, self.armed_marker, self.saved_pointer, self.pointer) = (k as i32, -1, 0, 4);
                }
            }
            return;
        }
        if k == 9 {
            self.retreat_window = true;
            self.retreat_opening = true;
            return;
        }
        if self.group_state[k] < 0 {
            return;
        }
        let mask = self.masks[k];
        let (keep, set) = if self.group_state[k] == 7 { (!mask, 0) } else { (u32::MAX, mask) };
        for i in 0..self.units.len() {
            if self.units[i].selected && b.units[i].alive && b.units[i].owner == b.human {
                out.push(Out::Order(Order::Weapons { unit: i, keep, set }));
            }
        }
    }

    /// `HandleRetreatWindow` (spec §5.4): "Yes" retreats; the click closes the window either way. Taken on the next
    /// left release after the one of the opening click [inf: the spec's "every frame without the left button" would
    /// close it the frame after it opened].
    pub fn retreat_window_frame(&mut self, yes_hovered: bool, tutorial_paused: bool, out: &mut Vec<Out>) {
        if !self.retreat_window || !(self.release && !self.held.0) {
            return;
        }
        if self.retreat_opening {
            self.retreat_opening = false;
            return;
        }
        if yes_hovered && !tutorial_paused {
            out.push(Out::Order(Order::Retreat));
        }
        self.retreat_window = false;
    }

    /// `SpaceWarKeyboardHandler` (spec §7.1.5): `key` down or up.
    #[allow(clippy::too_many_arguments)]
    pub fn key(&mut self, b: &Battle, k: Key, down: bool, ctrl: bool, debug: bool, tutorial_paused: bool, out: &mut Vec<Out>) {
        match k {
            Key::Left => self.yaw_rate = if down { -1.0 } else { 0.0 },
            Key::Right => self.yaw_rate = if down { 1.0 } else { 0.0 },
            Key::Up => self.tilt_rate = if down { -1.0 } else { 0.0 },
            Key::Down => self.tilt_rate = if down { 1.0 } else { 0.0 },
            Key::ZoomIn => self.zoom_rate = if down { 1.0 } else { 0.0 },
            Key::ZoomOut => self.zoom_rate = if down { -1.0 } else { 0.0 },
            _ if !down => {}
            Key::All => self.select_fleet(b),
            Key::Camera => self.chase = !self.chase,
            Key::Group(n) if ctrl => {
                if self.own_selected(b) {
                    for g in self.units.iter_mut().filter(|g| g.selected) {
                        g.group = n;
                    }
                }
            }
            Key::Group(n) => {
                if self.pointer == 4 {
                    (self.pointer, self.armed, self.armed_marker) = (self.saved_pointer, -1, -1);
                }
                self.deselect();
                for i in 0..self.units.len() {
                    if Self::selectable(b, i) && self.units[i].group == n {
                        self.units[i].selected = true;
                    }
                }
            }
            Key::Retreat if debug && !tutorial_paused => out.push(Out::Order(Order::Retreat)),
            Key::Explode if debug => {
                for i in 0..self.units.len() {
                    if self.units[i].selected {
                        out.push(Out::Order(Order::Explode { unit: i }));
                    }
                }
            }
            _ => {}
        }
    }
}

impl SpaceWarUi {
    /// The frame as the renderer reads it. `types` names each unit's model (`tipusnevek[race·12 + type]`).
    pub fn shown<'a>(&self, b: &'a Battle, tables: &ig2_rules::Rules) -> crate::spacewar_screen::Shown<'a> {
        let (eye, target) = self.eye_target();
        let types = b
            .units
            .iter()
            .map(|u| tables.ship_types.get((u.race * 12 + u.kind).max(0) as usize).map(|t| (t.name.clone(), u.ship_type.scale)))
            .collect();
        let blink_off = self.pointer == 4 && self.armed >= 0 && self.blink % 50 >= 26;
        crate::spacewar_screen::Shown {
            battle: b,
            view: self.view(),
            eye,
            target,
            units: self.units.clone(),
            types,
            slots: std::array::from_fn(|i| crate::spacewar_screen::Slot { counter: self.column[i], state: self.group_state[i], reload: self.group_reload[i] }),
            stats: self.stats,
            snap_yaw: self.snap_yaw,
            erosseg: self.erosseg(),
            fade: self.fade,
            radar: self.radar,
            spin: self.spin,
            hidden_button: blink_off.then_some(self.armed as usize),
            boxed: self.boxing.then_some([self.left_at[0], self.left_at[1], self.mouse[0], self.mouse[1]]),
            fx: None,
            chased: usize::try_from(self.chased).ok(),
            gun_t: self.gun_t.clone(),
            gun_timers: self.gun_timers.clone(),
            gun_damage: Vec::new(),
            title: self.title.clone(),
            title_tech: self.title_tech,
            stance: self.stance,
            big: (self.big_info != 0).then(|| self.big.clone()),
            pointer: self.pointer,
            formation: self.formation(b).then_some(self.formation_pick),
        }
    }

    /// `Mouse_Darabol` over `SpaceWar_MouseInit`'s hot-spots (spec §4.2): the lowest id under the pointer wins; the
    /// camera cluster's buttons are colours of the mask `STAR_M.RAW` (kep 0x1f). Returns `AktIkon` (id + 0x1e).
    pub fn spot(&self, mask: &[u8], at: [f32; 2]) -> Option<u32> {
        let (x, y) = (at[0], at[1]);
        let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        let colour = mask.get(y.clamp(0.0, 479.0) as usize * 640 + x.clamp(0.0, 639.0) as usize).copied().unwrap_or(0);
        if inside([274.0, 388.0, 94.0, 67.0]) {
            return Some(0x1f);
        }
        for i in 0..10 {
            if self.column[i] == 0 {
                continue;
            }
            let sl = crate::spacewar_screen::Slot { counter: self.column[i], state: 0, reload: 0 };
            let p = sl.place(i, self.xcenter);
            let (hx, hy) = ((p[0] - 20.0 + 320.0) as i32 as f32, (p[1] - 20.0 + 250.0) as i32 as f32);
            if inside([hx, hy, 40.0, 40.0]) {
                return Some(0x21 + i as u32);
            }
        }
        if self.stats.total == 0 && inside([50.0, 461.0, 113.0, 9.0]) {
            return Some(0x37);
        }
        if inside([0.0, 424.0, 166.0, 56.0]) {
            return Some(0x42);
        }
        for (id, c) in [(0x46, 0xf), (0x47, 0x10), (0x48, 0x11), (0x49, 0x12), (0x4a, 0xb), (0x4b, 0xc)] {
            // A colour spot is the mask's colour alone (the rectangle isn't tested [obs: 0xb lies partly outside its 212,377,40,32]).
            if colour == c {
                return Some(id);
            }
        }
        inside([0.0, 0.0, 640.0, 480.0]).then_some(0x4c)
    }
}

/// The keys `SpaceWarKeyboardHandler` handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    /// Numpad + / −.
    ZoomIn,
    ZoomOut,
    /// `a`: `flottafelallit`.
    All,
    /// 1..9 (top row): group `n − 1`.
    Group(i32),
    /// DEBUGX `r` / Backspace.
    Retreat,
    Explode,
    /// `c`: the fighter chase camera on or off (`spwcameramode`).
    Camera,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SpaceWar_Autoscroll` with the pointer in the window's top-left corner, against the exe (`run_spwc.py --script`
    /// with the real cursor there, per-frame `cam` records of UI ticks 420 and 421): the targets (−21, −24) settle
    /// the ramp at (−25, −25), and one UI tick moves the pivot by `5 · (zoom + scale · 18) / 600 · 0.6` along both axes.
    #[test]
    fn autoscroll_matches_the_exe() {
        let b = Battle::slot();
        let mut ui = SpaceWarUi::enter(0, &b, &crate::starmap::Camera::side_view(&Default::default(), [640.0, 480.0]), 0);
        (ui.yaw, ui.zoom, ui.scale) = (std::f32::consts::PI, 436.35565, 1.0);
        ui.pivot = [0.0, -45.435562, 45.435566];
        ui.scroll = [-25, -25];
        ui.autoscroll([0, 0], [800, 600], false);
        assert_eq!(ui.scroll, [-25, -25]);
        assert_eq!(ui.pivot, [0.0, -47.70734, 47.707344]);
        assert_eq!(ui.pan_target, ui.pivot);
        // From rest the ramp climbs 5 a tick; a held button stops it again.
        let mut ui = SpaceWarUi::enter(0, &b, &crate::starmap::Camera::side_view(&Default::default(), [640.0, 480.0]), 0);
        for want in [[-5, -5], [-10, -10], [-15, -15], [-20, -20], [-25, -25], [-25, -25]] {
            ui.autoscroll([3, 5], [800, 600], false);
            assert_eq!(ui.scroll, want);
        }
        ui.autoscroll([3, 5], [800, 600], true);
        assert_eq!(ui.scroll, [-20, -20]);
        // Left of the window counts as the right edge (an unsigned compare).
        let mut ui = SpaceWarUi::enter(0, &b, &crate::starmap::Camera::side_view(&Default::default(), [640.0, 480.0]), 0);
        ui.autoscroll([-4, 300], [800, 600], false);
        assert_eq!(ui.scroll, [5, 0]);
    }

    /// `creatematrixfromaxis` is a rotation by the angle about the axis (det 1, the axis fixed).
    #[test]
    fn axis_matrix_rotates_about_its_axis() {
        let a = normalise([0.3, -0.5, 0.8]);
        let m = axis_matrix(a, 0.7);
        let r = crate::scene::apply(&m, a);
        assert!(len(sub(r, a)) < 1e-5);
        let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6]) + m[2] * (m[3] * m[7] - m[4] * m[6]);
        assert!((det - 1.0).abs() < 1e-5);
    }

    /// `seturhajopos` on the exe's own state: a capture's dump (`run_spwc.py --grab-ticks`, in `IG2_GRAB_DIR`) has
    /// each unit's drawn matrix (exe unit `+0xe4`) and its own turn (exe `+0x234..+0x23c`); for a ship at rest one more
    /// call keeps it (its fixed point), or moves it one easing step.
    #[test]
    fn pose_keeps_the_exe_matrix_of_a_ship_at_rest() {
        let Some(dir) = std::env::var_os("IG2_GRAB_DIR") else { return };
        let mut checked = 0;
        for name in ["grab3-t600.json", "grab3-t1200.json", "grab4-t300.json", "grab4-t800.json"] {
            let Ok(d) = crate::spacewar_screen::Dump::load(&std::path::Path::new(&dir).join(name).to_string_lossy()) else { continue };
            for (i, u) in d.battle.units.iter().enumerate() {
                if !u.alive || u.owner == 0 || u.speed != 0.0 {
                    continue;
                }
                let r = &d.units_raw[i * 600..(i + 1) * 600];
                let m: M3 = std::array::from_fn(|k| ig2_sim::spacebattle_raw::f32_at(r, 0xe4 + 4 * k));
                let turn = std::array::from_fn(|k| ig2_sim::spacebattle_raw::f32_at(r, 0x234 + 4 * k));
                let mut g = UnitGfx { matrix: m, turn, ..Default::default() };
                pose(u, &mut g);
                let err = (0..9).map(|k| (g.matrix[k] - m[k]).abs()).fold(0.0f32, f32::max);
                // Still turning towards the heading (`|m₀₀| ≥ 0.01`): one step of at most 0.05.
                let tol = if m[0].abs() < 0.01 { 1e-3 } else { 0.05 };
                assert!(err < tol, "{name} unit {i}: {m:?} -> {:?}", g.matrix);
                checked += 1;
            }
        }
        eprintln!("{checked} resting ships checked");
    }

    /// `infoszamol` on the exe's own selection: a dump's slot has what the exe's last call left (exe `+0x1fc8..+0x1ffc`
    /// statistics, the groups' barrels `+0x2a30`, slowest reload `+0x2a58`, enabled barrels `+0x2a80`, state `+0x2aa8`),
    /// with the masks of the ELF's table. The value needs `Hajok`, which the dump hasn't: not compared.
    #[test]
    fn infoszamol_matches_the_exe() {
        use ig2_sim::spacebattle_raw::i32_at;
        let Some(dir) = std::env::var_os("IG2_GRAB_DIR") else { return };
        let mut checked = 0;
        for name in ["grab3-t600.json", "grab3-t1200.json", "grab4-t300.json", "grab4-t800.json"] {
            let Ok(d) = crate::spacewar_screen::Dump::load(&std::path::Path::new(&dir).join(name).to_string_lossy()) else { continue };
            let b = &d.battle;
            let mut ui = SpaceWarUi::enter(0, b, &crate::starmap::Camera::side_view(&Default::default(), [640.0, 480.0]), 0);
            for (i, g) in ui.units.iter_mut().enumerate() {
                g.selected = d.units_raw[i * 600 + 0x181] != 0;
            }
            for g in &mut ui.guns {
                g.0 = false;
            }
            let mut out = Vec::new();
            ui.infoszamol(b, &Plans { designs: &[], ships: &[], gun_tech: Vec::new(), firepower: &[], fleets: &[] }, &mut out);
            let o = &d.obj;
            let st = ui.stats;
            let want = crate::spacewar_hud::Stats::from_slot(o);
            assert_eq!((st.ships, st.damage, st.max_hp, st.rank_sum), (want.ships, want.damage, want.max_hp, want.rank_sum), "{name}");
            assert_eq!((st.total, st.sel_ships, st.sel_guns, st.last, st.last_kind, st.own), (want.total, want.sel_ships, want.sel_guns, want.last, want.last_kind, want.own), "{name}");
            for g in 0..10 {
                let exe = (i32_at(o, 0x2a30 + 4 * g), i32_at(o, 0x2a80 + 4 * g), i32_at(o, 0x2a58 + 4 * g), i32_at(o, 0x2aa8 + 4 * g));
                assert_eq!((ui.group_count[g], ui.group_enabled[g], ui.group_reload[g], ui.group_state[g]), exe, "{name} group {g}");
            }
            checked += 1;
        }
        eprintln!("{checked} dumps checked");
    }

    /// `kamerabeallit` on the exe's settled camera: from the dumped fields (pivot, yaw, tilt, zoom) the targets are
    /// the smoothed eye and target the exe left (`spwkamerapont`/`spwforgaspont`) once it has stopped easing, and a
    /// tick from there keeps them;
    /// `bezoomolasztav` gave the default zoom (`+0x1f50` = it × 0.0004) only if the units haven't moved since the zoom
    /// [obs: printed, not asserted].
    #[test]
    fn kamerabeallit_matches_the_exe_camera() {
        let Some(dir) = std::env::var_os("IG2_GRAB_DIR") else { return };
        let mut checked = 0;
        for name in ["grab3-t600.json", "grab3-t1200.json", "grab4-t300.json", "grab4-t800.json"] {
            let Ok(d) = crate::spacewar_screen::Dump::load(&std::path::Path::new(&dir).join(name).to_string_lossy()) else { continue };
            let Some(c) = d.cam else { continue };
            let v = d.view();
            let mut ui = SpaceWarUi::enter(0, &d.battle, &crate::starmap::Camera::side_view(&Default::default(), [640.0, 480.0]), 0);
            (ui.pivot, ui.yaw, ui.tilt, ui.zoom, ui.scale) = (v.pivot, v.yaw, v.tilt, v.zoom, v.scale);
            (ui.eye, ui.target, ui.init_camera, ui.restoring) = ([c[0], c[1], c[2]], [c[4], c[5], c[6]], false, false);
            if (0..3).any(|k| (c[4 + k] - v.pivot[k]).abs() > 1e-3) {
                // Still easing when the capture froze (the grabs ran with the real pointer off the window: `SpaceWar_Autoscroll`).
                eprintln!("{name}: the exe's camera is still easing: target {:?}, pivot {:?}", &c[4..7], v.pivot);
                continue;
            }
            ui.kamerabeallit(&d.battle);
            for k in 0..3 {
                assert!((ui.eye[k] - c[k]).abs() < 1e-3 && (ui.target[k] - c[4 + k]).abs() < 1e-3, "{name}: {:?} {:?} vs {c:?}", ui.eye, ui.target);
            }
            let dz = ig2_sim::spacebattle_raw::f32_at(&d.obj, 0x1f50);
            eprintln!("{name}: default zoom {dz} (bezoomolasztav now × 0.0004: {})", SpaceWarUi::bezoomolasztav(&d.battle) * SCALE_START);
            checked += 1;
        }
        eprintln!("{checked} dumps checked");
    }

    /// `gorbe` eases from 0 to 1 symmetrically.
    #[test]
    fn view_animation_curve() {
        let g = crate::spacewar_screen::gorbe;
        assert_eq!(g(0.0), 0.0);
        assert_eq!(g(0.5), 0.5);
        assert_eq!(g(2.0), 1.0);
        assert!((g(0.25) + g(0.75) - 1.0).abs() < 1e-6);
    }
}
