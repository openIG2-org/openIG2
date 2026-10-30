//! The colony screen (re/screens/colony.md): the planet's terrain object, its buildings and the ground war's
//! units and shots, seen through the original's colony camera, from the sim's state each tick.

use std::collections::HashMap;

use ig2_formats::{colony_map, text, Install};
use ig2_rules::UnitModels;
use ig2_sim::ground::{flag, Field, GroundMap};
use ig2_sim::save::SaveState;

use crate::assets::Image;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};
use crate::render3d::{MeshDraw, MeshId, MeshVertex, Scene3d, SceneUniforms, Shadow, ShadowMap};
use crate::scene::{self, Look, Model, Placement, Transform, V3, M3};

/// `TEREPXSIZE`/`TEREPYSIZE` after `Init3dColony`: the terrain spans ±4096 world units in x and y.
pub const MAP_SIZE: f32 = 8192.0;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

/// `Grid2Real` (ELF 0x45f950): 200 × 200 grid → world `(x, y)` (grid x runs against world x).
pub fn grid_to_real(gx: f32, gy: f32) -> (f32, f32) {
    let s = MAP_SIZE;
    (-((gx / 2.0 / 99.0 * 100.0 * s) / 99.0 - s / 2.0), (gy / 2.0 / 99.0 * 100.0 * s) / 99.0 - s / 2.0)
}

/// World `(x, y)` → height-grid cells (`ShowColony`: `49 − ((x + S/2)/S·50·49)/50`, `((y + S/2)/S·50·49)/50`).
pub fn height_cell(x: f32, y: f32) -> (f32, f32) {
    let s = MAP_SIZE;
    (49.0 - ((x + s / 2.0) / s * 50.0 * 49.0) / 50.0, ((y + s / 2.0) / s * 50.0 * 49.0) / 50.0)
}

/// `GetHeightCam` (ELF 0x4be590): the height under a camera point, cells clamped to 0.6..48.9.
fn height_cam(map: &GroundMap, x: f32, y: f32) -> f32 {
    let (fx, fy) = height_cell(x, y);
    map.height(fx.clamp(0.6, 48.9), fy.clamp(0.6, 48.9))
}

/// `GetHeight` (ELF 0x46bff0): 0 past the grid (the exe reads the next record inside it; clamped here).
pub fn height(map: &GroundMap, x: f32, y: f32) -> f32 {
    let (fx, fy) = height_cell(x, y);
    if fx > 200.0 || fy > 200.0 {
        return 0.0;
    }
    map.height(fx.clamp(0.0, 48.99), fy.clamp(0.0, 48.99))
}

// ---------------------------------------------------------------------------------------------------------
// State read from the sim

/// A building as the colony screen draws it (`Epuletek[p]` slot, spec §3).
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingView {
    pub slot: usize,
    pub kind: u8,
    /// `+0xc/+0x10`: grid position.
    pub x: f32,
    pub y: f32,
    /// `+8`: construction work left (0 = finished).
    pub work_left: u16,
    /// `+4`: damage (the repair button shows while it is positive).
    pub damage: i32,
    /// `+0x19`: bit 0 working, bit 4 blinking (`ShowColony`), bit 5 switched off.
    pub flags: u8,
    /// `+0x18`: output factor (%); `+0x1e`: efficiency (256 = full).
    pub yield_pct: u8,
    pub efficiency: i16,
}

/// A unit (`WF_Tank`) as the colony screen draws it (spec §4).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitView {
    pub slot: usize,
    pub owner: i32,
    pub hull: i32,
    pub flags: u32,
    pub has_gun: bool,
    /// `+0x15c`: the design (the pod tech for a defence turret).
    pub design: i32,
    /// `+0x198`: whose design table the design is in.
    pub side: i32,
    /// `(h, x, y)`: `+0x1dc`, `+0x1d4`, `+0x1d8`.
    pub pos: V3,
    pub body: M3,
    pub turret: M3,
    pub turret_angle: f32,
    /// `+0x1c0`: the body's heading (the blob shadow's turn).
    pub angle: f32,
    /// The weapon pod the turret model shows (`TankTorony` index), `None` without one.
    pub gun: Option<usize>,
    /// What the war's buttons and markers read (spec §11): `+0x1d0` weapons, `+0x288` air-strike units, `+0x250`
    /// mine laying, `+0x278` dropship bits, `+0x23c` target (with flag 0x100), the waypoints still to drive
    /// (cursor `+0x230` .. count `+0x22c`) and the members with hit points (`AliveTankNum`).
    pub weapons: u32,
    pub air: i32,
    pub laying: bool,
    pub transport: u32,
    pub target: i32,
    pub route: Vec<(i32, i32)>,
    pub members: i32,
    /// Riding its dropship (`+0x278` bit 3): the model is drawn `(h, x, y)` = (`+0x284`, `+0x27c`, `+0x280`) off
    /// its place (spec §4); the rings stay on the ground.
    pub ride: Option<V3>,
    /// `+0x238`: a defence turret's building slot.
    pub building: i32,
    /// The dropship (`WF_Hordozo`, `+0x264..+0x26c`) while it is shown (state `+0x278` bit 0 clear, not a turret or
    /// a 0x20000 unit): its offset `(h, x, y)` from the unit.
    pub dropship: Option<V3>,
}

/// A shot in flight: `(h, x, y)`, the list it is in (0 shells, 1 missiles/debris, 2 bolts) and its kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotView {
    pub pos: V3,
    pub list: u8,
    pub kind: i32,
}

/// The colony screen's camera as `Colony_Belep` reads it from the battlefield (`WF+0x1a8..+0x1b8`; ELF
/// `+0x2a8..+0x2b8`: `ColonyXStart`, `ColonyYStart`, `ColonyZoom`, `ColonyPitch`, `ColonyRotate`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SavedCamera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub pitch: f32,
    pub rotate: f32,
    /// `WF+0x184` (ELF `+0x26c`): centre on a building when the screen opens (`HaVanEpuletAkkorAzLegyenKozepen`).
    pub centre: bool,
}

impl SavedCamera {
    /// `InitNewGame`'s values (ELF 0x2a8..: −517.55, 499.84, −1100, −π/4, 0).
    pub const NEW_GAME: Self = Self { x: -517.5548, y: 499.8431, zoom: -1100.0, pitch: -std::f32::consts::FRAC_PI_4, rotate: 0.0, centre: true };

    fn read(f: &Field) -> Self {
        let v = f.tail.f1a8.map(f32::from_bits);
        // `Colony_Belep`: |x|, |y| > 5000 → 0.
        let sane = |a: f32| if a.abs() > 5000.0 || a.is_nan() { 0.0 } else { a };
        Self { x: sane(v[0]), y: sane(v[1]), zoom: v[2], pitch: v[3], rotate: v[4], centre: f.tail.f184 != 0 }
    }
}

/// The colony screen at one sim tick.
#[derive(Debug, Clone, PartialEq)]
pub struct ColonySnap {
    pub planet: usize,
    pub tick: i32,
    pub owner: u8,
    /// The owner's race (`PlayerFaj[owner]`): race 4 builds in two thirds of the work ([`cut_height`]).
    pub owner_race: u8,
    pub surface: u8,
    pub map: usize,
    pub buildings: Vec<BuildingView>,
    pub units: Vec<UnitView>,
    pub shots: Vec<ShotView>,
    pub camera: SavedCamera,
    /// `WF+0xa00`-equivalent weather and `+0xa04` background (`InitWeather`).
    pub weather: i32,
    pub background: i32,
    pub at_war: bool,
    /// The battlefield's war state (spec §11): `+0x7fc/+0x800 + 8(e − 1)` the call-in timers, `+0x89f + s − 1` the
    /// retreat flags, `+0x8b1 + s` side `s` has units on the field; the planet's `+0x64` bit 0 (a war on it).
    pub timers: [(i32, i32); 18],
    pub retreat: [u8; 18],
    pub present: [u8; 19],
    pub war_flag: bool,
    /// `MultiPlayerMode` (exe 0xBAE2C0), and the MP ground war (`GrWar_MultiCsata_folyamatban` 0xBAD13C) runs on this
    /// planet (`GrWar_MultiCsata_bolygo` 0xBAD140): the gates of `Colony_MouseInit2` and `ColonyGfx2D_1` (spec §11.1).
    pub multiplayer: bool,
    pub mp_war_here: bool,
    /// Alive units still to land (flag 0x8000000), of any side: `(owner, design)` in slot order.
    pub to_land: Vec<(i32, i32)>,
    /// The attacking fleet's owner (`Flottak[WF+0x338]+0x15`, 0 none) and the planet's name (`+0`).
    pub attacker: i32,
    pub planet_name: Vec<u8>,
    /// The retreat button's test (`Colony_Main` 0x10): no alive unit still to land and every one with `+0x278` bit 0.
    pub all_landed: bool,
    /// `WF::UsedPosition`'s view (spec §11.7): every alive unit not on board (flag 0x8000000 clear), in slot order:
    /// `(slot, owner, cell)` (`+0x1a0/+0x1a4`).
    pub placed: Vec<(usize, i32, (i32, i32))>,
    /// `WF+0x334`: the self-destruct countdown (−1 idle; spec §11.8).
    pub self_destruct: i32,
    /// The human sees inside (`WF+0xa1b`, set by `ShowColony`): the owner, or satellites there
    /// (`BolygoTipus::pMuholdakMennyi(EN)` = planet `+0xc4 + EN`; empire 7 counts an empty entry as one). Else
    /// the buildings, roads and units are hidden and only red footprints show (`DrawBuildingBases`).
    pub visible: bool,
}

impl ColonySnap {
    pub fn read(state: &SaveState, planet: usize) -> Option<Self> {
        let colony = state.colony();
        let p = colony.planets.get(planet)?;
        let field = state.ground_field(planet).unwrap_or_default();
        let buildings = p
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind != 0)
            .map(|(i, b)| BuildingView { slot: i + 1, kind: b.kind, x: b.x, y: b.y, work_left: b.work_left, damage: b.damage, flags: b.flags, yield_pct: b.yield_pct, efficiency: b.efficiency })
            .collect();
        let units = field
            .units
            .iter()
            .enumerate()
            .filter(|(i, u)| field.alive.get(*i).copied().unwrap_or(false) && u.flags & (flag::NOT_PLACED | flag::MINE) == 0)
            .map(|(i, u)| {
                let (body, turret) = u.rt.orientation();
                let turret_unit = u.flags & flag::TURRET != 0;
                let gun = if turret_unit {
                    usize::try_from(u.design - 0x98).ok()
                } else if u.has_gun {
                    let rec = usize::try_from(u.side).ok().and_then(|s| state.designs.get(s)).and_then(|d| d.get(usize::try_from(u.design).ok()?));
                    rec.map(|r| match (r[0x51], r[0x52]) {
                        (0, 0) => 0x11,
                        (0, b) => (b as usize).wrapping_sub(0xa1),
                        (a, _) => (a as usize).wrapping_sub(0xa1),
                    })
                    .filter(|&k| k < 19)
                } else {
                    None
                };
                UnitView {
                    slot: i,
                    owner: u.owner,
                    hull: u.hull,
                    flags: u.flags,
                    has_gun: u.has_gun,
                    design: u.design,
                    side: u.side,
                    pos: [u.rt.z, u.pos.0, u.pos.1],
                    body,
                    turret,
                    turret_angle: u.rt.turret_angle,
                    angle: u.rt.angle,
                    gun,
                    weapons: u.weapons,
                    air: u.air,
                    laying: u.rt.laying != 0,
                    transport: u.transport,
                    target: u.target,
                    route: u.rt.waypoints.iter().skip(u.rt.wp_cursor.max(0) as usize).copied().collect(),
                    members: u.hp.iter().filter(|&&h| h > 0).count() as i32,
                    ride: (u.transport & 8 != 0).then_some([u.rt.drop[2], u.rt.drop[0], u.rt.drop[1]]),
                    building: u.building,
                    dropship: (u.flags & 0x4020000 == 0 && u.transport & 1 == 0).then_some([u.rt.drop[2], u.rt.drop[0], u.rt.drop[1]]),
                }
            })
            .collect();
        let l = &field.lists;
        let shots = l
            .shells
            .iter()
            .flatten()
            .map(|s| ShotView { pos: s.pos, list: 0, kind: 0 })
            .chain(l.missiles.iter().flatten().map(|m| ShotView { pos: m.pos, list: 1, kind: m.kind }))
            .chain(l.bolts.iter().flatten().map(|b| ShotView { pos: b.pos, list: 2, kind: 0 }))
            .chain(l.fighters.iter().filter(|_| l.fighters_on).map(|f| ShotView { pos: f.pos, list: 3, kind: 0 }))
            .collect();
        let en = state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
        // `WF+0x8b1 + s` as `GetStatus` (exe 0x47be90) sets it: alive units counted on the field, by side (the saved
        // bytes are stale until the next status call).
        let mut present = [0u8; 19];
        for (u, _) in field.units.iter().zip(&field.alive).filter(|(u, a)| {
            **a && !(u.flags & flag::NOT_PLACED != 0 && u.flags & flag::NOT_COUNTED != 0) && u.flags2 & 4 == 0 && u.flags & flag::MINE == 0
        }) {
            if let Some(p) = present.get_mut(u.side as usize) {
                *p = 1;
            }
        }
        let sats = if en == 7 && p.spies[7] == 0 { 1 } else { p.spies[en] };
        // `DrawBuildingBases` only when none of these: the owner, satellites, or side EN has units on the field
        // (`WF+0x8b1 + EN`, colony.md §11.4: a lander sees the colony).
        let visible = p.owner as usize == en || sats != 0 || present[en] != 0;
        let camera = if field.units.is_empty() && field.tail.f1a8 == [0; 5] { SavedCamera::NEW_GAME } else { SavedCamera::read(&field) };
        Some(Self {
            planet,
            tick: state.mem.i32(ig2_sim::save::va::IDO2),
            owner: p.owner,
            owner_race: state.races().get(p.owner as usize).copied().unwrap_or(0),
            surface: p.surface,
            map: p.colony_map as usize,
            buildings,
            units,
            shots,
            camera,
            weather: field.weather,
            background: field.background,
            at_war: field.units.iter().zip(&field.alive).any(|(u, &a)| a && u.flags & flag::TURRET == 0 && u.owner != p.owner as i32),
            timers: field.timers,
            retreat: field.retreat,
            present,
            war_flag: state.mem.u8(ig2_sim::save::va::PLANETS + (planet * ig2_sim::save::PLANET_SIZE) as u32 + 0x64) & 1 != 0,
            multiplayer: state.mem.u8(ig2_sim::save::va::MULTIPLAYER) & 1 != 0,
            mp_war_here: state.mem.i32(0xBAD13C) != 0 && state.mem.i32(0xBAD140) == planet as i32,
            attacker: usize::try_from(field.attack_fleet).ok().filter(|&f| f > 0).and_then(|f| state.fleets.get(f)).map_or(0, |r| r[0x15] as i32),
            planet_name: state.mem.read(ig2_sim::save::va::PLANETS + (planet * ig2_sim::save::PLANET_SIZE) as u32, 0x14).into_iter().take_while(|&b| b != 0).collect(),
            all_landed: field.units.iter().zip(&field.alive).all(|(u, &a)| !a || (u.flags & flag::LANDED == 0 && u.transport & 1 != 0)),
            to_land: field.units.iter().zip(&field.alive).filter(|(u, a)| **a && u.flags & flag::LANDED != 0).map(|(u, _)| (u.owner, u.design)).collect(),
            placed: field.units.iter().zip(&field.alive).enumerate().filter(|(_, (u, a))| **a && u.flags & flag::LANDED == 0).map(|(i, (u, _))| (i, u.owner, u.cell)).collect(),
            self_destruct: field.self_destruct,
            visible,
        })
    }

    /// The units between `prev` (alpha 0) and `self` (alpha 1): positions glide; a new unit or one that jumped
    /// (a reused slot, a landing) is drawn where it is now.
    pub fn units_at(&self, prev: &ColonySnap, alpha: f32) -> Vec<UnitView> {
        let a = alpha.clamp(0.0, 1.0);
        self.units
            .iter()
            .map(|u| {
                let mut out = u.clone();
                if let Some(b) = prev.units.iter().find(|b| b.slot == u.slot && b.owner == u.owner && b.hull == u.hull) {
                    let d: V3 = std::array::from_fn(|k| u.pos[k] - b.pos[k]);
                    if d[1] * d[1] + d[2] * d[2] < 200.0 * 200.0 {
                        out.pos = std::array::from_fn(|k| b.pos[k] + d[k] * a);
                        out.body = lerp_matrix(&b.body, &u.body, a);
                        out.turret = lerp_matrix(&b.turret, &u.turret, a);
                    }
                }
                out
            })
            .collect()
    }
}

/// Rows lerped and renormalised (orientations change little per tick).
fn lerp_matrix(a: &M3, b: &M3, t: f32) -> M3 {
    let mut m: M3 = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
    for c in 0..3 {
        let l = (m[c] * m[c] + m[3 + c] * m[3 + c] + m[6 + c] * m[6 + c]).sqrt();
        if l > 1e-6 {
            for r in 0..3 {
                m[r * 3 + c] /= l;
            }
        }
    }
    m
}

// ---------------------------------------------------------------------------------------------------------
// The camera

/// The colony camera (`ShowColony` ELF 0x4ced80, spec §1): a look-at camera (`renderscene(1)` →
/// `CameraRotate(eye, target)`), fov 600 on the 480-line virtual screen centred at (320, 240).
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub pitch: f32,
    pub rotate: f32,
    pub size: [f32; 2],
    /// An earthquake's jolt added to the pivot after the clamp (`ShowColony` exe 0x49f130: `rand() % 31 − 15` for x,
    /// then for y, while `Colony_foldrenges` is the shown planet); zero otherwise.
    pub shake: [f32; 2],
    /// `TankView`'s eye and target this frame ([`CameraKeys::tank`]): they replace the block above.
    pub tank: Option<Eye>,
}

/// Where the camera stands and looks, `(h, x, y)`.
#[derive(Debug, Clone, Copy)]
pub struct Eye {
    pub eye: V3,
    pub target: V3,
}

/// The view basis of `CameraRotate` (ELF 0x5fcf90): `d` = forward, `a = d × (1, 0, 0)`, `b = d × a`; camera
/// space `e = (a·q, −b·q, −d·q)`, `q = p − eye`.
#[derive(Debug, Clone, Copy)]
pub struct ViewBasis {
    pub eye: V3,
    pub rows: [V3; 3],
}

const NEAR: f32 = 26.25;
const FAR: f32 = 60000.0;

fn cross(p: V3, q: V3) -> V3 {
    [p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0]]
}

fn dot(p: V3, q: V3) -> f32 {
    p[0] * q[0] + p[1] * q[1] + p[2] * q[2]
}

impl Camera {
    pub fn from_saved(s: &SavedCamera, size: [f32; 2]) -> Self {
        Self { x: s.x, y: s.y, zoom: s.zoom, pitch: s.pitch, rotate: s.rotate, size, shake: [0.0; 2], tank: None }
    }

    /// `ShowColony`'s camera block: pivot `(0, XStart, −YStart)`, eye at `Zoom − 800` from it along pitch and
    /// rotation, clamped to ±3900 (`MaximizeCamera`), kept 100 above the ground under it.
    pub fn eye(&self, map: &GroundMap) -> Eye {
        if let Some(t) = self.tank {
            return t;
        }
        let x = self.x.clamp(-3800.0, 3800.0);
        let y = self.y.clamp(-3800.0, 3800.0);
        let mut target = [0.0, x + self.shake[0], self.shake[1] - y];
        let r = self.zoom - 800.0;
        let (sp, cp) = ((self.pitch as f64).sin(), (self.pitch as f64).cos());
        let h = (r as f64 * cp) as f32;
        let s = (r as f64 * sp) as f32;
        let (sr, cr) = ((self.rotate as f64).sin(), (self.rotate as f64).cos());
        let mut eye = [target[0] + h, target[1] + (sr * s as f64) as f32, target[2] + (cr * s as f64) as f32];
        maximize(&mut eye, &mut target, 3900.0);
        let g = height_cam(map, eye[1], eye[2]);
        if g - 100.0 < eye[0] {
            target[0] = -(eye[0] - (g - 100.0));
            eye[0] = g - 100.0;
        }
        Eye { eye, target }
    }

    pub fn basis(&self, map: &GroundMap) -> ViewBasis {
        let Eye { eye, target } = self.eye(map);
        let d = scene::normalise([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
        let a = scene::normalise(cross(d, [1.0, 0.0, 0.0]));
        let b = scene::normalise(cross(d, a));
        ViewBasis { eye, rows: [a, [-b[0], -b[1], -b[2]], [-d[0], -d[1], -d[2]]] }
    }

    /// Virtual pixels → window pixels: 480 lines high, centred.
    pub fn k(&self) -> f32 {
        crate::font::Virtual::new(self.size).k
    }

    /// World → window pixels, `None` behind the near plane.
    pub fn project(&self, v: &ViewBasis, p: V3) -> Option<[f32; 2]> {
        let q = [p[0] - v.eye[0], p[1] - v.eye[1], p[2] - v.eye[2]];
        let e = v.rows.map(|r| dot(r, q));
        if e[2] > -NEAR {
            return None;
        }
        let f = 600.0 / -e[2];
        let (vx, vy) = (e[0] * f + 320.0, e[1] * f + 240.0);
        Some(crate::font::Virtual::new(self.size).px(vx, vy))
    }

    /// World → virtual pixels and the depth `−z` in front of the eye (`TransformPoint`), `None` behind the near plane.
    pub fn project_virtual(&self, v: &ViewBasis, p: V3) -> Option<([f32; 2], f32)> {
        let q = [p[0] - v.eye[0], p[1] - v.eye[1], p[2] - v.eye[2]];
        let e = v.rows.map(|r| dot(r, q));
        if e[2] > -NEAR {
            return None;
        }
        let f = 600.0 / -e[2];
        Some(([e[0] * f + 320.0, e[1] * f + 240.0], -e[2]))
    }

    /// The same projection as a clip-space matrix (column-major for WGSL).
    pub fn clip_matrix(&self, v: &ViewBasis) -> [[f32; 4]; 4] {
        let sx = 2.5 * self.size[1] / self.size[0];
        let sy = -2.5;
        let zf = FAR / (FAR - NEAR);
        let [a, b, d] = v.rows;
        // clip = R·(p − eye): x = sx·a·q, y = sy·b·q, z = zf·(−d·q) − NEAR·zf, w = −d·q.
        let row = |r: V3, s: f32, c: f32| [r[0] * s, r[1] * s, r[2] * s, -dot(r, v.eye) * s + c];
        let rows = [row(a, sx, 0.0), row(b, sy, 0.0), row(d, -zf, -NEAR * zf), row(d, -1.0, 0.0)];
        std::array::from_fn(|c| std::array::from_fn(|r| rows[r][c]))
    }

    /// `Colony_Move(dx, dy)` (exe 0x4688f0; ELF `Colony_Move0` 0x4e2cb0): the start point slides by `(dx, dy)`
    /// pixels turned by the rotation, x then y, scaled `((600 − Zoom) + 200)/600·0.85` (f32, the angle's cos/sin in f64
    /// rounded to f32; spec §11.9). The `MaximizeCamera` correction it first takes is the clamp of [`Camera::basis`].
    pub fn slide(&mut self, dx: i32, dy: i32) {
        let (c, s) = ((self.rotate as f64).cos() as f32, (self.rotate as f64).sin() as f32);
        for (dx, dy) in [(dx, 0), (0, dy)] {
            if dx == 0 && dy == 0 {
                continue;
            }
            let k = (600.0 - self.zoom) + 200.0;
            let (fx, fy) = (dx as f32, dy as f32);
            self.x -= (((fx * c - fy * s) * k) / 600.0) * 0.85;
            self.y -= (((fy * c + fx * s) * k) / 600.0) * 0.85;
        }
        // `ShowColony` clamps the start point every frame.
        self.x = self.x.clamp(-3800.0, 3800.0);
        self.y = self.y.clamp(-3800.0, 3800.0);
    }

    pub fn zoom_steps(&mut self, steps: f32) {
        self.zoom = (self.zoom + steps * 60.0).clamp(-2500.0, 700.0);
    }
}

/// `WF::LoadMesh`'s shadow textures by hull type (`WF+0x1a8 + 8·type`).
const SHADOW_TEXTURES: [&str; 5] = ["kerekes.bmp", "kozepes.bmp", "lebego.bmp", "mega.bmp", "aknarako.bmp"];

/// `WF_Tank::DrawShadow` (ELF 0x481d80, exe ≈0x473400; spec §11.10): a square of half size `TankShadowSize[hull]`
/// around the unit's map point shifted `TankShadowTransl[hull]` on both axes, 2 above its height, the texture turned
/// with the heading (`uv = (cos, sin)(−angle + π/4 + φ)/2 + 0.5` in f64, φ a quarter turn per corner). Riding its
/// dropship: the colour `dc = (int)(h/200·255 + 255)` (0 at h ≤ −200), the square grown `(255 − dc)/4` and moved with
/// the dropship's offset. Triangles (−+, ++, +−), (−+, −−, +−) (x, y signs).
fn blob_shadow(um: &ig2_rules::UnitModels, u: &UnitView) -> [MeshVertex; 6] {
    let t = u.hull.clamp(0, 4) as usize;
    let shift = um.shadow_shift.get(t).copied().unwrap_or(0.0);
    let (mut cx, mut cy, mut s) = (u.pos[1] + shift, u.pos[2] + shift, um.shadow_size.get(t).copied().unwrap_or(0.0));
    let mut dc = 255;
    if let Some([h, dx, dy]) = u.ride {
        if h <= -200.0 {
            dc = 0;
        } else {
            dc = ((h as f64 / 200.0) * 255.0 + 255.0) as i32;
            s += (255 - dc) as f32 / 4.0;
            cx += dx;
            cy += dy;
        }
    }
    let a = -(u.angle as f64) + std::f64::consts::FRAC_PI_4;
    let uv = |q: f64| {
        let p = a + q * std::f64::consts::FRAC_PI_2;
        [(p.cos() * 0.5 + 0.5) as f32, (p.sin() * 0.5 + 0.5) as f32]
    };
    let k = dc as f32 / 255.0;
    let h = u.pos[0] - 2.0;
    let v = |x: f32, y: f32, q: f64| MeshVertex { pos: [h, x, y], normal: [0.0; 3], uv: uv(q), color: [k, k, k, 1.0] };
    [v(cx - s, cy + s, 2.0), v(cx + s, cy + s, 1.0), v(cx + s, cy - s, 0.0), v(cx - s, cy + s, 2.0), v(cx - s, cy - s, 3.0), v(cx + s, cy - s, 0.0)]
}

/// `LandingZone::GenerateKorvonal` (exe 0x478f80, ELF 0x46acd0): the outline of the cells closed to landing, the
/// zone (`LandingZone::Generate`, `ig2_sim::placement::landing_zone`) or `MapAlloc` (the terrain and every
/// building with its frame ring), over x, y in 4..=191: an "in" cell's eight neighbours make a mask (bit 0 (x, y − 1),
/// then counter-clockwise: (x − 1, y − 1), (x − 1, y), (x − 1, y + 1), (x, y + 1), (x + 1, y + 1), (x + 1, y),
/// (x + 1, y − 1)) that picks one of the 14 edge tiles; inner cells and the other masks get none. Read at
/// `y·200 + x` (the zone itself is written a row up). `(x, y, tile − 1)` in the exe's order (x outer).
pub fn landing_outline(gm: &GroundMap, buildings: &[(u8, f32, f32)]) -> Vec<(i32, i32, usize)> {
    use ig2_sim::placement::{landing_zone, Grid};
    let zone = landing_zone(buildings, &gm.footprints);
    let mut alloc = Grid::from_terrain(&gm.terrain.mask);
    for &(kind, x, y) in buildings.iter().filter(|b| b.0 != 0) {
        let (w, h) = gm.footprints.get(kind as usize).copied().unwrap_or((0, 0));
        alloc.allocate(x as i32, y as i32, w, h, true);
    }
    let inside = |x: i32, y: i32| {
        let i = (y * 200 + x) as usize;
        zone.get(i).is_some_and(|&z| z != 0) || alloc.cells().get(i).is_some_and(|&a| a != 0)
    };
    const NEIGHBOURS: [(i32, i32); 8] = [(0, -1), (-1, -1), (-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1)];
    let mut out = Vec::new();
    for x in 4..192 {
        for y in 4..192 {
            if !inside(x, y) {
                continue;
            }
            let mask = NEIGHBOURS.iter().enumerate().fold(0u8, |m, (k, &(dx, dy))| m | ((inside(x + dx, y + dy) as u8) << k));
            let tile = match mask {
                0x7c | 0x7e | 0xfc | 0xfe => 1,
                0x70 | 0x72 | 0x73 | 0x78 | 0x7a | 0xf0 | 0xf2 | 0xf8 => 2,
                0xf1 | 0xf3 | 0xf9 | 0xfb => 3,
                0xc1 | 0xc3 | 0xc9 | 0xcd | 0xe1 => 4,
                0xc7 | 0xcf | 0xe7 | 0xef => 5,
                0x07 | 0x0f | 0x27 | 0x2f | 0x37 | 0x87 | 0x8f | 0xa7 => 6,
                0x1f | 0x3f | 0x9f | 0xbf => 7,
                0x1c | 0x1e | 0x3c | 0x9c | 0xdc => 8,
                0x7f => 9,
                0xfd => 10,
                0xf7 => 11,
                0xdf => 12,
                0x77 => 13,
                0xdd => 14,
                _ => continue,
            };
            out.push((x, y, tile - 1));
        }
    }
    out
}

/// `LandingZone::GenerateOneSquareVertex` (exe 0x479d90, f32): cell (x, y)'s square, ±20.89 around
/// `Grid2Real(x + 0.5, y + 0.5)`, each corner 3 above `GetHeight` of its height cell (grid / 4), uv 0.015..0.985;
/// triangles (P0, P1, P2), (P3, P1, P2).
fn landing_square(gm: &GroundMap, x: i32, y: i32) -> [MeshVertex; 6] {
    let (rx, ry) = grid_to_real(x as f32 + 0.5, y as f32 + 0.5);
    let h = |fx: f32, fy: f32| if fx > 200.0 || fy > 200.0 { 0.0 } else { gm.height(fx.clamp(0.0, 48.99), fy.clamp(0.0, 48.99)) };
    let (x0, x1, y0, y1) = (x as f32 * 0.25, (x + 1) as f32 * 0.25, y as f32 * 0.25, (y + 1) as f32 * 0.25);
    let c = [1.0, 0.0, 0.0, 128.0 / 255.0];
    let v = |hh: f32, px: f32, py: f32, u: f32, w: f32| MeshVertex { pos: [hh - 3.0, px, py], normal: [-1.0, 0.0, 0.0], uv: [u, w], color: c };
    let (lo, hi) = (0.015, 0.985);
    let p0 = v(h(x1, y0), rx - 20.89, ry - 20.89, lo, lo);
    let p1 = v(h(x0, y0), rx + 20.89, ry - 20.89, hi, lo);
    let p2 = v(h(x1, y1), rx - 20.89, ry + 20.89, lo, hi);
    let p3 = v(h(x0, y1), rx + 20.89, ry + 20.89, hi, hi);
    [p0, p1, p2, p3, p1, p2]
}

/// The camera keys' state (spec §11.9): the held flags of `ColonyKeyboardHandler`, the speeds `Frissit2_Colony`
/// integrates, the `c` presets' tween and the edge scroll's speeds.
#[derive(Debug, Clone, Default)]
pub struct CameraKeys {
    /// `ColonyTurnLeft/Right`, `ColonyTurnUp/Down` (↓ / ↑), `ColonyZoomIn/Out` (numpad + / −).
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub zoom_in: bool,
    pub zoom_out: bool,
    /// Exe 0x997c64, 0x996e94, 0x997c68.
    rotate_speed: f32,
    zoom_speed: f32,
    pitch_speed: f32,
    /// `NextCameraMode` (0x998248); mode 3 sets `TankView` (0x9963a3), which the first frame with nothing to follow
    /// clears (the mode stays 3).
    pub mode: i32,
    pub tank_view: bool,
    /// `TankViewLastAngle` (0x8ec9f0, −1000 at start = `None`, never reset).
    tank_last: Option<f32>,
    /// `ColonyTargetZoom/Pitch` and `TransfRotFrame` while `TransfRot`.
    tween: Option<(f32, f32, i32)>,
    /// `Colony_Autoscroll`'s `xSpeed/ySpeed` (0xa3eb68/6c).
    scroll: [i32; 2],
    /// `balclickx/y` (virtual pixels), `Colony_LastRotate`, `Colony_LastZoom` (the pitch) of the first press;
    /// `None` = `ujclick` (the next press takes them).
    turn_from: Option<([f32; 2], f32, f32)>,
}

/// The `c` presets (exe 0x8f1d44 zoom, 0x8f1e44 pitch; the rotate table 0x8f1dc4 is never read).
const PRESETS: [(f32, f32); 3] = [(-240.0, -1.113946), (-666.0, -0.356804), (-1100.0, -0.05)];

impl CameraKeys {
    /// `ShowColony` (exe 0x49f130, outside the pick list): a press of either button while `ujclick` keeps the pointer,
    /// Rotate and Pitch.
    pub fn mouse_press(&mut self, cam: &Camera, at: [f32; 2]) {
        if self.turn_from.is_none() {
            self.turn_from = Some(([at[0].trunc(), at[1].trunc()], cam.rotate, cam.pitch));
        }
    }

    /// A release of either button: `ujclick` again.
    pub fn mouse_release(&mut self) {
        self.turn_from = None;
    }

    /// Both buttons held: `Rotate = last − (x0 − x)/200`, `Pitch = last + (y0 − y)/200` clamped −1.4922565..−0.05
    /// (`at` in virtual pixels, from the first button's press).
    pub fn mouse_turn(&self, cam: &mut Camera, at: [f32; 2]) {
        if let Some((p, r, q)) = self.turn_from {
            cam.rotate = (r as f64 - (p[0] - at[0].trunc()) as f64 / 200.0) as f32;
            cam.pitch = ((q as f64 + (p[1] - at[1].trunc()) as f64 / 200.0) as f32).clamp(-1.4922565, -0.05);
        }
    }

    /// `c`: the next preset; zoom and pitch glide there over 15 drawn frames.
    pub fn preset(&mut self) {
        self.mode = if self.mode + 1 > 3 { 0 } else { self.mode + 1 };
        self.tank_view = self.mode == 3;
        if let Some(&(z, p)) = PRESETS.get(self.mode as usize) {
            self.tween = Some((z, p, 0));
        }
    }

    /// `TransformRotation` (ELF 0x4cc070), once per drawn frame: a quarter of the way to the preset.
    pub fn frame(&mut self, cam: &mut Camera) {
        if let Some((z, p, n)) = &mut self.tween {
            cam.zoom += (*z - cam.zoom) / 4.0;
            cam.pitch += (*p - cam.pitch) / 4.0;
            *n += 1;
            if *n >= 15 {
                self.tween = None;
            }
        }
    }

    /// `ShowColony`'s tank camera (exe 0x49f130, asm 0x4a097e..0x4a0be4), once per drawn frame: while `TankView`, the
    /// first selected unit by slot (not `NOT_PLACED`, not `NOT_COUNTED`) is followed from 130 behind and 60 above,
    /// its heading turned at most 0.025 rad a frame; the normal block, its clamps and the quake's jolt are skipped,
    /// keys and edge scroll still move the hidden camera. No selected unit: `TankView` off, the normal camera.
    pub fn tank(&mut self, cam: &mut Camera, units: &[UnitView], selected: &std::collections::BTreeSet<usize>) {
        cam.tank = None;
        if !self.tank_view {
            return;
        }
        let ok = |u: &&UnitView| selected.contains(&u.slot) && u.flags & flag::NOT_PLACED == 0;
        let mut chosen: Vec<&UnitView> = units.iter().filter(ok).collect();
        chosen.sort_by_key(|u| u.slot);
        let Some(u) = chosen.into_iter().find(|u| u.flags & flag::NOT_COUNTED == 0) else {
            self.tank_view = false;
            return;
        };
        // `a = (f32)(−heading − π/2)` in f64; a step of `d/|d|·0.025` (f64) while `|d| ≥ 0.025`.
        let want = (-(u.angle as f64) - std::f64::consts::FRAC_PI_2) as f32;
        let a = match self.tank_last {
            Some(last) => {
                let d = want - last;
                let ad = (d as f64).abs() as f32;
                if ad as f64 >= 0.025 { (last as f64 + (d / ad) as f64 * 0.025) as f32 } else { want }
            }
            None => want,
        };
        self.tank_last = Some(a);
        let [h, x, y] = u.pos;
        let (sa, ca) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
        cam.tank = Some(Eye { eye: [h - 90.0, sa * 130.0 + x, ca * 130.0 + y], target: [h - 30.0, x, y] });
    }

    /// `Frissit2_Colony`'s camera part (exe 0x477dc0), one 100 Hz UI tick, f32.
    pub fn tick(&mut self, cam: &mut Camera) {
        let f = |b: bool| b as i32 as f32;
        let cap = |v: f32, m: f32| if v.abs() > m { v / v.abs() * m } else { v };
        let rs = cap(self.rotate_speed - f(self.left) * 0.0035 + f(self.right) * 0.0035, 0.02);
        let zs = cap(f(self.zoom_in) * 1.7 + self.zoom_speed - f(self.zoom_out) * 1.7, 17.0);
        let ps = cap(self.pitch_speed - f(self.down) * 0.0025 + f(self.up) * 0.0025, 0.02);
        cam.rotate += rs;
        cam.pitch += ps;
        cam.zoom += zs;
        self.rotate_speed = rs * 0.9;
        self.zoom_speed = zs * 0.85;
        self.pitch_speed = ps * 0.79;
        cam.pitch = cam.pitch.clamp(-1.4922565, -0.05);
        cam.zoom = cam.zoom.clamp(-1100.0, 240.0);
    }

    /// `Colony_Autoscroll` (exe 0x465940), one UI tick: the window pointer `at` within 8 pixels of an edge of
    /// `size` (no button held) aims at ±30; the speeds step 5 towards it; `Colony_Move(speed / 5)`.
    pub fn autoscroll(&mut self, cam: &mut Camera, at: [i32; 2], size: [u32; 2], held: bool) {
        let (mut tx, mut ty) = (0i32, 0i32);
        if !held {
            if at[0] < 8 {
                tx = -30;
            }
            if size[0].wrapping_sub(8) < at[0] as u32 {
                tx = 30;
            }
            if at[1] < 8 {
                ty = -30;
            }
            if size[1].wrapping_sub(8) < at[1] as u32 {
                ty = 30;
            }
        }
        // Normalised in f32; the second axis sees the first's new value [original quirk].
        let n = |tx: i32, ty: i32| ((tx as f32 * tx as f32 + (ty * ty) as f32) as f64).sqrt() as f32;
        if tx * tx + ty * ty != 0 {
            tx = ((tx * 30) as f32 / n(tx, ty)) as i32;
        }
        if tx * tx + ty * ty != 0 {
            ty = ((ty * 30) as f32 / n(tx, ty)) as i32;
        }
        for (s, t) in [(0, tx), (1, ty)] {
            if self.scroll[s] < t {
                self.scroll[s] += 5;
            }
            if t < self.scroll[s] {
                self.scroll[s] -= 5;
            }
        }
        if self.scroll != [0, 0] {
            cam.slide((self.scroll[0] as f64 / 5.0) as i32, (self.scroll[1] as f64 / 5.0) as i32);
        }
    }

    /// Off the colony screen: the scroll speeds stop.
    pub fn stop_scroll(&mut self) {
        self.scroll = [0, 0];
    }
}

/// `MaximizeCamera` (ELF 0x4ce5e0): pull the eye back inside ±`m` along the eye–pivot line and move the pivot
/// by the same amount.
fn maximize(eye: &mut V3, pivot: &mut V3, m: f32) {
    let (dx, dy) = (eye[1] - pivot[1], eye[2] - pivot[2]);
    let (ox, oy) = (eye[1], eye[2]);
    if m < eye[1] {
        eye[1] = m;
        eye[2] = pivot[2] + dy * ((m - pivot[1]) / dx);
    }
    if m < eye[2] {
        eye[2] = m;
        eye[1] = pivot[1] + dx * ((m - pivot[2]) / dy);
    }
    if eye[1] < -m {
        eye[1] = -m;
        eye[2] = pivot[2] + dy * (-(m + pivot[1]) / dx);
    }
    if eye[2] < -m {
        eye[2] = -m;
        eye[1] = pivot[1] + dx * (-(m + pivot[2]) / dy);
    }
    pivot[1] += eye[1] - ox;
    pivot[2] += eye[2] - oy;
}

// ---------------------------------------------------------------------------------------------------------
// The scene on the GPU

struct Drawn {
    mesh: MeshId,
    draws: Vec<(std::ops::Range<u32>, TexId, bool, Shadow)>,
    /// Per draw, the building slot and level of detail it belongs to (empty: always drawn; [`ColonyScene::lods`]).
    lods: Vec<Option<(usize, usize)>>,
    /// Drawn only when the human sees inside (`Some(true)`) or only when not (`Some(false)`).
    when: Option<bool>,
}

/// An environment-mapped part (property 0x4000 with 0x8000: `Kocka` hands it to `vagashatarkirak`, ELF 0x606c90 /
/// exe 0x5395d0; re/screens/render.md §2.5): its baked, lit world vertices with their file normals and the inverse of
/// its world matrix; [`scene::env_uv`] gives the texcoords from the eye every frame. One pass, the texture bound by
/// [`env_texture`], no shadow-receive and no self-illumination pass.
/// Parts of a building under construction come this way too, environment-mapped or not (`env`), cut every frame at
/// [`cut_height`] (`cut`: the building's slot and bottom height).
struct EnvPart {
    verts: Vec<MeshVertex>,
    normals: Vec<V3>,
    inv: M3,
    tex: TexId,
    glass: bool,
    shadow: Shadow,
    when: Option<bool>,
    env: bool,
    cut: Option<(usize, f32)>,
    /// A blinking building's slot ([`blinks`]): its transparency follows the counter.
    blink: Option<usize>,
    /// The building slot and level of detail it belongs to ([`ColonyScene::lods`]).
    lod: Option<(usize, usize)>,
}

/// `ShowColony` (exe 0x49f130, asm 0x49f3a0..0x49f45b) for a building under construction (`+8` work left > 0), every
/// frame: `frac = 1 − left/total` (f32, not below 0; `total = EpuletekLeiro[kind]+4 · 50`, `/1.5` truncated for race
/// 4), and the cut `bottom − (f32)(apos·frac)·2.1 + apos` in f64, kept as f32 (`setvagashatar` exe 0x4542e0), `apos` =
/// `allvanypoziciok[kind − 1]`, `bottom` the slot's `+0x14`. On the height axis (more negative = higher): at frac 0
/// the cut lies `apos` below the bottom, at 1 `1.1·apos` above it. `None` for a finished building.
pub fn cut_height(rules: &ig2_rules::Rules, race: u8, kind: u8, left: u16, bottom: f32) -> Option<f32> {
    let frac = built_fraction(rules, race, kind, left)?;
    let apos = rules.scaffold_heights.get((kind as usize).wrapping_sub(1)).copied().unwrap_or(0.0);
    Some((bottom as f64 - (apos * frac) as f64 * 2.1 + apos as f64) as f32)
}

/// `SetLangPlaces` (exe 0x49bd50) for one placed building (`verts` its baked triangles): the chimneys, the child
/// meshes `fust1`, `fust2`, … up to the first missing one, at their nodes' world positions (`GetObjWorldSpacePos`),
/// and the three damage points: where a vertical line (from −1500 down to 15000) through (gx + 30, gz − 25),
/// (gx − 30, gz + 30) and (gx − 30, gz − 30) first meets the building (`vonalmetszes`), `(gx, gz)` its slot's
/// `Grid2Real` without the odd-size half cells.
fn smoke_places(m: &Model, xf: &Transform, b: &BuildingView, verts: &[MeshVertex], out: &mut crate::colony_smoke::Emitters) {
    for k in 1.. {
        let name = format!("fust{k}");
        let Some(n) = m.nodes.iter().find(|n| n.path.rsplit('/').next().is_some_and(|c| c.eq_ignore_ascii_case(&name))) else { break };
        out.chimneys.push((b.slot, xf.point(n.at)));
    }
    let (gx, gz) = grid_to_real(b.x, b.y);
    let hit = |y: f32, z: f32| -> Option<V3> {
        let mut best: Option<f32> = None;
        for t in verts.chunks_exact(3) {
            let (a, b, c) = (t[0].pos, t[1].pos, t[2].pos);
            let d = (b[1] - a[1]) * (c[2] - a[2]) - (c[1] - a[1]) * (b[2] - a[2]);
            if d.abs() < 1e-9 {
                continue;
            }
            let u = ((y - a[1]) * (c[2] - a[2]) - (c[1] - a[1]) * (z - a[2])) / d;
            let v = ((b[1] - a[1]) * (z - a[2]) - (y - a[1]) * (b[2] - a[2])) / d;
            if u < 0.0 || v < 0.0 || u + v > 1.0 {
                continue;
            }
            let h = a[0] + u * (b[0] - a[0]) + v * (c[0] - a[0]);
            if (-1500.0..=15000.0).contains(&h) && best.is_none_or(|x| h < x) {
                best = Some(h);
            }
        }
        best.map(|h| [h, y, z])
    };
    out.damage.push((b.slot, [hit(gx + 30.0, gz - 25.0), hit(gx - 30.0, gz + 30.0), hit(gx - 30.0, gz - 30.0)]));
}

/// [`cut_height`]'s `frac`: the share of the work done, `None` once finished.
fn built_fraction(rules: &ig2_rules::Rules, race: u8, kind: u8, left: u16) -> Option<f32> {
    if left == 0 {
        return None;
    }
    let mut total = rules.buildings.get(kind as usize).map_or(0, |b| b.build_work) * 50;
    if race == 4 {
        total = (total as f64 / 1.5) as i32;
    }
    Some((1.0 - left as f32 / total as f32).max(0.0))
}

/// The scaffolding of a building under construction (`EpuletAllvanyok::AddAllvanyok` exe 0x4578c0, `ModifyAllvany`
/// exe 0x48b390): `step = (int)(frac·10)`; one box a footprint cell (rows from `(int)(y − h/2 + oy)`, columns from
/// `(int)(x − w/2 + ox)` while below the far edge, `ox`/`oy` the odd-size half cells) at level
/// `min(step, magas[kind − 1][row·10 + col])`, placed at `(allvanypoziciok[kind − 1] + (int)bottom, Grid2Real(cx + 0.5,
/// cy + 0.5))`. The exe adds them as the work goes on and never lowers a level, which is the same as recomputing.
/// Returns `(level, position)`; the box is `allvany<level + 1>.xvb` ([`ColonyScene::scaffold_mesh`]).
pub fn scaffolds(rules: &ig2_rules::Rules, race: u8, b: &BuildingView, bottom: f32) -> Vec<(usize, V3)> {
    let Some(frac) = built_fraction(rules, race, b.kind, b.work_left) else { return Vec::new() };
    let step = (frac * 10.0) as usize;
    let k = b.kind as usize;
    let (Some(&(w, h)), Some(mask), Some(&apos)) = (rules.building_footprints.get(k), rules.scaffold_masks.get(k.wrapping_sub(1)), rules.scaffold_heights.get(k.wrapping_sub(1))) else {
        return Vec::new();
    };
    let ox = if w % 2 != 0 { 0.5 } else { 0.0 };
    let oy = if h % 2 != 0 { -0.5 } else { 0.0 };
    let (x, y) = (b.x as f64, b.y as f64);
    let (x0, x1) = ((x - w as f64 / 2.0 + ox) as i32, x + w as f64 / 2.0 + ox);
    let (y0, y1) = ((y - h as f64 / 2.0 + oy) as i32, y + h as f64 / 2.0 + oy);
    let height = apos + (bottom as i32) as f32;
    let mut out = Vec::new();
    for (row, cy) in (y0..).take_while(|&c| (c as f64) < y1).enumerate() {
        for (col, cx) in (x0..).take_while(|&c| (c as f64) < x1).enumerate() {
            let level = step.min(mask.get(row * 10 + col).copied().unwrap_or(0) as usize);
            let (gx, gz) = grid_to_real(cx as f32 + 0.5, cy as f32 + 0.5);
            out.push((level, [height, gx, gz]));
        }
    }
    out
}

/// `clippolygonvagashatar` (exe 0x52e410, `cliptlvertexvh` 0x52ea60): the part of a triangle list at or below the
/// cut (`h ≥ cut` on the height axis) kept, the crossing triangles split at `t = (cut − h_a)/(h_b − h_a)` with the
/// points, texcoords and colours lerped; no cap over the opening.
fn clip_below(tris: &[MeshVertex], cut: f32, out: &mut Vec<MeshVertex>) {
    let lerp = |a: &MeshVertex, b: &MeshVertex| {
        let t = (cut - a.pos[0]) / (b.pos[0] - a.pos[0]);
        let l = |x: f32, y: f32| x + (y - x) * t;
        MeshVertex {
            pos: std::array::from_fn(|k| l(a.pos[k], b.pos[k])),
            normal: std::array::from_fn(|k| l(a.normal[k], b.normal[k])),
            uv: std::array::from_fn(|k| l(a.uv[k], b.uv[k])),
            color: std::array::from_fn(|k| l(a.color[k], b.color[k])),
        }
    };
    for t in tris.chunks_exact(3) {
        let inside: Vec<bool> = t.iter().map(|v| v.pos[0] >= cut).collect();
        match inside.iter().filter(|&&x| x).count() {
            3 => out.extend_from_slice(t),
            0 => {}
            n => {
                // Walk the triangle's edges (Sutherland–Hodgman) and fan the polygon left.
                let mut poly = Vec::with_capacity(4);
                for i in 0..3 {
                    let (a, b) = (&t[i], &t[(i + 1) % 3]);
                    if inside[i] {
                        poly.push(*a);
                    }
                    if inside[i] != inside[(i + 1) % 3] {
                        poly.push(lerp(a, b));
                    }
                }
                debug_assert_eq!(poly.len(), if n == 2 { 4 } else { 3 });
                for k in 1..poly.len() - 1 {
                    out.extend([poly[0], poly[k], poly[k + 1]]);
                }
            }
        }
    }
}

/// `Init3dColony` (`PropertiesOn(…, 0xc000)`, exe 0x47e11e/0x47e229) and `ShowColony` (the panel's and the build
/// cursor's copies, exe 0x49fa3b..0x49fdeb): a building's `_B_` and `_C_` children are environment-mapped.
pub(crate) fn building_env(p: &scene::Part) -> bool {
    let l = p.path.to_ascii_lowercase();
    l.contains("_b_") || l.contains("_c_")
}

/// Property 0x100 without 1 (the terrain's trees, the Park): `fenyekszamolasa` (exe 0x52f190) lights the vertices flat,
/// without normals: the ambient lights' sum plus each parallel light's colour · `shade[1024]` (205) >> 8; on the colony
/// (`SetEnvironment`: ambient 0x1e1e1e, parallel 0xc8c8c8) 0x1e + (0xc8·205 >> 8) = 190 a channel.
const FLAT_LIGHT: f32 = (0x1e + ((0xc8 * 205) >> 8)) as f32 / 255.0;

/// A building's scaffolding as drawn: its slot, grid position and boxes (level, base).
type Scaffolding = (usize, (f32, f32), Vec<(usize, V3)>);
/// A scaffolding box to draw: level, base, and the falling one's axis and angle.
type ScaffoldBox = (usize, V3, Option<(V3, f32)>);

/// A scaffolding box falling over (`EpuletAllvany` after `DoljLe`): level, base, axis, the angle and its speed, the
/// frames still to wait, the angle drawn this frame.
struct Falling {
    level: usize,
    at: V3,
    axis: V3,
    angle: f32,
    vel: f32,
    delay: i32,
    drawn: f32,
}

impl Falling {
    /// One UI tick of `EpuletAllvany::Refresh`; `false` once the box is gone.
    fn tick(&mut self) -> bool {
        if self.delay > 0 {
            self.delay -= 1;
            return true;
        }
        self.drawn = self.angle;
        self.angle -= self.vel;
        self.vel += 0.005;
        self.angle > -std::f32::consts::FRAC_PI_2
    }
}

/// The texture an environment-mapped part shows: `changeaktmatarialself` (ELF 0x610ff0 / exe 0x52dda0) binds the batch
/// material's `texture2`, which `loadmaterial` (exe 0x534470) swaps for the sky's (`HAmp`'s first material's texture)
/// when it is `reflec.bmp` or `fenyek_2.bmp`; without one the texture bound before stays: for `_B_`/`_C_` the sky's
/// too (`ChangeTexture`, exe 0x451af0), else the part's own.
pub(crate) fn env_texture(install: &Install, p: &scene::Part, sky: Option<&str>, building: bool) -> Option<String> {
    match p.texture2_name.as_str() {
        "reflec.bmp" | "fenyek_2.bmp" => sky.map(str::to_owned),
        "" if building => sky.map(str::to_owned),
        "" => p.texture.clone(),
        t => install.resolve_texture("textures/colony/", t).or_else(|| p.texture.clone()),
    }
}

/// [`scene::env_uv`] over a part's baked vertices (`out`, in the order of `p.verts`), its root placed by `world`.
pub(crate) fn env_map(p: &scene::Part, world: &M3, eye: V3, out: &mut [MeshVertex]) {
    let Some(inv) = scene::inverse(&scene::mul(world, &p.matrix)) else { return };
    for (o, v) in out.iter_mut().zip(&p.verts) {
        o.uv = scene::env_uv(&inv, eye, o.pos, v.fnormal);
    }
}

/// One of the buildings' particles (`ParticleOwner`) as drawn: centre, half size (world units), colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub at: V3,
    pub size: f32,
    pub rgb: [f32; 3],
    pub flare: bool,
}

/// A stand-in for the `rand()` draws of the particles (the exe takes them from the shared libc stream).
fn pick(seed: u32, n: u32) -> u32 {
    let mut x = seed.wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    x % n
}

/// `AddBuildingSpecEffect` (exe 0x457dd0, ELF 0x4c1380; with `OptionsKellSpecFX`) and their step in `WF::Refresh2`
/// (exe 0x48ce10, ELF 0x489170), drawn by `DrawParticles` (exe 0x472800) through `ParticleOwner::Draw` (exe 0x526f80):
/// screen-aligned quads at the building's centre `(H, gx, gz)` (`H` its bottom height `+0x14`), added (`ONE`/`ONE`),
/// depth-tested without writing it, unfogged; the colour `c·(255 − fade) >> 8`. Only for finished, working buildings
/// (`+0x19` bit 0 set, bit 5 clear) on a planet the human sees inside. The positions step every fifth UI tick
/// (`(int)ColonyCounter % 5 == 0`, `ColonyCounter` = `counter`).
/// - kind 53 (the solar plant): a `flare.bmp` (owner B, type 0xc) at `H − 30`, half size `20 + rand % 10` drawn anew
///   each step, white; and 16 `rcloud.bmp` (owner A, type 0xd, colour (0x12, 0x63, 0x8b) fade 0x40, half size
///   `5 + rand % 2`) circling it: `f = sin(i/31)·0.15 + 1`, angle `i + counter/(f·60)`, radius `f·15`;
/// - kind 52: 32 `rcloud.bmp` (type 0xb, colour `(255 − rand % 100, 163 − rand % 130, rand % 50)`, half size
///   `(rand % 10)/2·2 + 10`) at `H − 10` swinging `sin(i + counter/60)·45` across.
///
/// `i` counts owner A's particles in the order the buildings were added. The `rand()` draws are stand-ins.
pub fn building_particles(buildings: &[BuildingView], at: &[(usize, V3)], counter: f32) -> Vec<Particle> {
    let cc = ((counter as i32).div_euclid(5) * 5) as f32;
    let step = (counter as i32).div_euclid(5) as u32;
    let mut out = Vec::new();
    let mut i = 0u32;
    for b in buildings.iter().filter(|b| b.kind == 52 || b.kind == 53) {
        let Some(&(_, t)) = at.iter().find(|(s, _)| *s == b.slot) else { continue };
        let (h, gx, gz) = (t[0] + 1.0, t[1], t[2]);
        let on = b.work_left == 0 && b.flags & 1 != 0 && b.flags & 0x20 == 0;
        let seed = b.slot as u32 * 64;
        if b.kind == 53 {
            if on {
                out.push(Particle { at: [h - 30.0, gx, gz], size: (20 + pick(seed ^ step.wrapping_mul(7919), 10)) as f32, rgb: [254.0 / 255.0; 3], flare: true });
            }
            for k in 0..16 {
                let f = ((i as f64 / 31.0).sin() * 0.15 + 1.0) as f32;
                let ang = (i as f64 + (cc / (f * 60.0)) as f64) as f32;
                if on {
                    let r = f * 15.0;
                    let size = (5 + pick(seed + k, 2)) as f32;
                    out.push(Particle { at: [h - 30.0, gx + ang.sin() * r, gz + ang.cos() * r], size, rgb: [13.0 / 255.0, 73.0 / 255.0, 103.0 / 255.0], flare: false });
                }
                i += 1;
            }
        } else {
            for k in 0..32 {
                if on {
                    let s = seed + k * 8;
                    let rgb = [(255 - pick(s, 100)) as f32, (163 - pick(s + 1, 130)) as f32, pick(s + 2, 50) as f32].map(|c| (c * 255.0 / 256.0).floor() / 255.0);
                    let size = (pick(s + 3, 10) / 2 * 2 + 10) as f32;
                    let x = (i as f32 + cc / 60.0).sin() * 45.0;
                    out.push(Particle { at: [h - 10.0, gx + x, gz], size, rgb, flare: false });
                }
                i += 1;
            }
        }
    }
    out
}

/// A quad facing the screen (the camera's right and up rows) around `at`, half size `h`: two triangles, prelit.
fn billboard(basis: &ViewBasis, at: V3, h: f32, rgb: [f32; 3]) -> [MeshVertex; 6] {
    let (a, b) = (basis.rows[0], basis.rows[1]);
    let q = |sx: f32, sy: f32, u: f32, v: f32| MeshVertex {
        pos: std::array::from_fn(|k| at[k] + (a[k] * sx + b[k] * sy) * h),
        normal: [0.0; 3],
        uv: [u, v],
        color: [rgb[0], rgb[1], rgb[2], 1.0],
    };
    [q(-1.0, 1.0, 0.0, 0.0), q(1.0, 1.0, 1.0, 0.0), q(1.0, -1.0, 1.0, 1.0), q(-1.0, 1.0, 0.0, 0.0), q(1.0, -1.0, 1.0, 1.0), q(-1.0, -1.0, 0.0, 1.0)]
}

/// A model's parts with their textures resolved, for per-frame placement (units).
struct UnitMesh {
    model: Model,
    texs: Vec<TexId>,
}

/// One planet's colony scene: what `Init3dColony` loads, uploaded once.
pub struct ColonyScene {
    statics: Vec<Drawn>,
    /// The environment-mapped parts, drawn every frame with texcoords from the eye ([`EnvPart`]).
    env: Vec<EnvPart>,
    /// The buildings' smoke ([`crate::colony_smoke`]) and where it comes from.
    smoke: crate::colony_smoke::Smoke,
    emitters: crate::colony_smoke::Emitters,
    /// Snow or rain ([`crate::colony_weather`]), moved once a drawn frame.
    weather: Option<crate::colony_weather::Weather>,
    /// The scaffolding drawn last frame by slot (the building's grid position and its boxes), and the boxes of
    /// finished or demolished buildings still falling ([`ColonyScene::carry_from`]).
    last_scaffolds: Vec<Scaffolding>,
    falling: Vec<Falling>,
    /// The `ColonyCounter` the falling boxes were last stepped to, and their `rand()` stand-in.
    fall_counter: Option<f32>,
    fall_rand: crate::colony_weather::CRand,
    /// Per building: slot, `|M·(1, 1, 1)|²` of its placement (world units), its position and its number of levels.
    lods: Vec<(usize, f32, V3, usize)>,
    /// Per building: slot and its world triangles by level of detail, the faces the click test sees
    /// ([`View::building_at`]).
    faces: Vec<(usize, Vec<Vec<[V3; 3]>>)>,
    /// The buildings' finest level of detail by kind, which `getbottomvertex` measures ([`bottom_vertex`]).
    finest: HashMap<u8, Option<Model>>,
    /// `allvany1..10.xvb` (`EpuletAllvanyok::Init` exe 0x47f330), loaded on first use.
    scaffold_models: Vec<Option<Option<UnitMesh>>>,
    /// The sky's texture (`HAmp`'s first material's `texture`, resolved): what `reflec.bmp`/`fenyek_2.bmp` become.
    pub sky: Option<String>,
    textures: HashMap<String, TexId>,
    white: TexId,
    units: HashMap<(usize, i32), Option<UnitMesh>>,
    turrets: HashMap<(usize, i32), Option<UnitMesh>>,
    fog: [f32; 3],
    /// The buildings' world transforms by slot (for markers).
    pub building_at: Vec<(usize, V3)>,
    /// The radar's picture: the terrain seen from straight above (re/screens/hud.md §3.1).
    pub radar: Option<TexId>,
    /// What the baked buildings were made from: `(slot, kind, x, y, finished)` ([`ColonyScene::stale`]).
    key: Vec<(usize, u8, i32, i32, bool, bool)>,
    /// Building models for the build cursor (`tempepulet`), by kind.
    buildings: HashMap<u8, Option<UnitMesh>>,
    /// `hordozo.xvb` (`WF::LoadMesh` exe 0x483f70), loaded on the first dropship.
    dropship: Option<Option<UnitMesh>>,
    /// The drawn units' `GetObjCenter` (their posed hull's box centre, a defence turret's gun), by slot: the hit test's
    /// point (`WF_Tank::MouseOver` exe 0x48bc90).
    pub unit_centres: Vec<(usize, V3)>,
    /// The unit info's turning model's render target ([`ColonyScene::render_info_model`]).
    info_target: Option<crate::render::Target>,
    /// The info box's planet globes `b<k>0.xvb` ([`ColonyScene::render_info_globe`]).
    globes: HashMap<usize, Option<UnitMesh>>,
}

fn texture(r: &mut Renderer, install: &Install, cache: &mut HashMap<String, TexId>, white: TexId, path: Option<&str>) -> TexId {
    let Some(path) = path else { return white };
    if let Some(&t) = cache.get(path) {
        return t;
    }
    let t = match Image::load(install, path) {
        Ok(img) => r.add_texture(&img),
        Err(e) => {
            eprintln!("ig2-client: {e}");
            white
        }
    };
    cache.insert(path.to_owned(), t);
    t
}

/// Bake a placed model into world-space vertices; `fog` = 0 keeps a part out of the fog (the background dome).
fn bake(model: &Model, xf: &Transform, tint: [f32; 3], fog: f32, out: &mut Vec<MeshVertex>, ranges: &mut Vec<(std::ops::Range<u32>, usize, Look)>) {
    let nm = xf.matrix;
    for (pi, p) in model.parts.iter().enumerate() {
        let start = out.len() as u32;
        let alpha = match p.look {
            Look::Glass(a) => a,
            _ => fog,
        };
        for v in &p.verts {
            out.push(MeshVertex {
                pos: xf.point(v.pos),
                normal: scene::normalise(scene::apply(&nm, v.normal)),
                uv: v.uv,
                color: [tint[0], tint[1], tint[2], alpha],
            });
        }
        ranges.push((start..out.len() as u32, pi, p.look));
    }
}

impl ColonyScene {
    pub fn load(r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, maps: &[GroundMap], s: &ColonySnap) -> Result<Self, String> {
        r.clear_meshes();
        let white = r.add_texture(&Image::white());
        let mut textures = HashMap::new();
        let gm = maps.get(s.map).ok_or(format!("no colony map {}", s.map))?;
        let raw = install.read("dataraw/colony/terepleiro.txt").map_err(|e| e.to_string())?;
        let txt = text::decode(&raw, install.lang());
        let surfaces = colony_map::parse_surfaces(&txt).map_err(|e| e.to_string())?;
        let backgrounds = colony_map::parse_backgrounds(&txt).map_err(|e| e.to_string())?;
        let mut statics = Vec::new();
        let upload = |r: &mut Renderer, textures: &mut HashMap<String, TexId>, model: &Model, verts: Vec<MeshVertex>, ranges: Vec<(std::ops::Range<u32>, usize, Look)>, role: &dyn Fn(&str) -> Shadow| {
            let mesh = r.add_mesh(&verts);
            let draws = ranges
                .into_iter()
                .map(|(rg, pi, look)| {
                    let t = texture(r, install, textures, white, model.parts[pi].texture.as_deref());
                    (rg, t, matches!(look, Look::Glass(_)), role(&model.parts[pi].path.to_ascii_lowercase()))
                })
                .collect();
            Drawn { mesh, draws, when: None, lods: Vec::new() }
        };

        // The background dome (`Hatterek[WF+0xa04]`, `AddToScene(…, 11)`, scaled (1, 1, 1.1), moved to h = −3000).
        let bg = backgrounds.get(s.background.max(0) as usize).or(backgrounds.first());
        let fog = bg.map_or([0.0; 3], |b| std::array::from_fn(|k| b.fog.get(k).copied().unwrap_or(0) as f32 / 255.0));
        // `HAmp` (exe 0xde0298), loaded before the buildings: its first material's texture is the environment map.
        let mut sky_name = String::new();
        let mut sky = None;
        if let Some(b) = bg {
            match Model::load(install, "vector/colony/", &b.model, "textures/colony/") {
                Ok(m) => {
                    if let Some(p) = m.parts.first() {
                        sky_name = p.texture_name.clone();
                        sky = p.texture.clone();
                    }
                    let xf = Placement::add_to_scene(m.origin, 11.0).scaled([1.0, 1.0, 1.1]).world((0.0, HALF_PI, 0.0), [-3000.0, 0.0, 0.0]);
                    let (mut v, mut rg) = (Vec::new(), Vec::new());
                    bake(&m, &xf, [1.0; 3], 0.0, &mut v, &mut rg);
                    statics.push(upload(r, &mut textures, &m, v, rg, &|_| Shadow::None));
                }
                Err(e) => eprintln!("ig2-client: background: {e}"),
            }
        }

        // The terrain object (spec §2.2), its lakes and lava glassy.
        let surf = surfaces.get(s.map).ok_or(format!("no colony map {}", s.map))?;
        let mut terrain = Model::load(install, "vector/colony/terrainobj/", &surf.model, &format!("textures/colony/terrainmap/{}/", s.surface))?;
        let lake = if s.surface == 1 || s.surface == 5 { 0.72 } else { 0.6 };
        terrain.set_look("tavak", Look::Glass(lake));
        terrain.set_look("viz", Look::Glass(0.37));
        terrain.set_look("fak", Look::Cutout);
        let rec = Placement::add_to_scene(terrain.origin, 1.0).scaled([4.0; 3]);
        let first = rec.world((0.0, HALF_PI, 0.0), [gm.base, 0.0, 0.0]);
        let raw_max_x = terrain.positions().map(|v| first.point(v)[1]).fold(0.0f32, f32::max);
        let xf = rec.scaled([4096.0 / raw_max_x; 3]).world((0.0, HALF_PI, 0.0), [gm.base, 0.0, 0.0]);
        let (mut v, mut rg) = (Vec::new(), Vec::new());
        bake(&terrain, &xf, [1.0; 3], 1.0, &mut v, &mut rg);
        // The trees (`fak*`: `Init3dColony` clears property 1 and sets 0x100, 0x200, 0x20, 0x10000): lit flat.
        let flat = FLAT_LIGHT;
        for (range, pi, _) in &rg {
            if terrain.parts[*pi].path.to_ascii_lowercase().contains("fak") {
                for x in &mut v[range.start as usize..range.end as usize] {
                    x.normal = [0.0; 3];
                    x.color = [flat, flat, flat, x.color[3]];
                }
            }
        }
        // The shadow map (re/screens/render.md §4): the terrain receives (`PropertiesOn(terep, 0x800000)`), its objects
        // `tt` and trees `fak` cast instead (`Init3dColony`, with `OptionsKellArnyek`, `lehetarnyek` and
        // `kelltereptargyarnyek` on).
        // Environment-mapped (`Init3dColony`): the ponds `tavak` on planet types 1 and 5 (0xc000, transparency 0.28,
        // `ChangeSelfTexture` exe 0x451ab0: the sky's texture name with its first letter `v`), the water `VIZ` (0xc008,
        // transparency 0.63, its own `texture2`'s swap).
        let mut env = Vec::new();
        let pond = (s.surface == 1 || s.surface == 5).then(|| {
            let mut n = sky_name.clone();
            n.replace_range(..n.chars().next().map_or(0, char::len_utf8), "v");
            install.resolve_texture("textures/colony/", &n)
        });
        let terrain_env: Vec<(usize, Option<String>)> = rg
            .iter()
            .enumerate()
            .filter_map(|(i, (_, pi, _))| {
                let p = &terrain.parts[*pi];
                let l = p.path.to_ascii_lowercase();
                match &pond {
                    Some(t) if l.contains("tavak") => Some((i, t.clone())),
                    _ if l.contains("viz") => Some((i, env_texture(install, p, sky.as_deref(), false))),
                    _ => None,
                }
            })
            .collect();
        for (i, tex) in &terrain_env {
            let (range, pi, look) = &rg[*i];
            let p = &terrain.parts[*pi];
            let inv = scene::inverse(&scene::mul(&xf.matrix, &p.matrix)).unwrap_or(scene::IDENTITY);
            let tex = texture(r, install, &mut textures, white, tex.as_deref());
            env.push(EnvPart { verts: v[range.start as usize..range.end as usize].to_vec(), normals: p.verts.iter().map(|x| x.fnormal).collect(), inv, tex, glass: matches!(look, Look::Glass(_)), shadow: Shadow::None, when: None, env: true, cut: None, blink: None, lod: None });
        }
        statics.push(upload(r, &mut textures, &terrain, v, rg, &|path| if path.contains("tt") || path.contains("fak") { Shadow::Cast } else { Shadow::Receive }));
        let radar = radar_picture(r, statics.last().unwrap());
        // The radar's picture keeps the water; the scene draws it through `env`.
        let td = &mut statics.last_mut().unwrap().draws;
        for (i, _) in terrain_env.iter().rev() {
            td.remove(*i);
        }

        // The buildings (spec §3): `AddToSceneGenesis(model, scale/1024/1000)`, turned a quarter, on the ground.
        let mut models: HashMap<u8, Option<Vec<Model>>> = HashMap::new();
        let mut lods = Vec::new();
        let mut finest: HashMap<u8, Option<Model>> = HashMap::new();
        let (mut v, mut rg, mut owners, mut worlds, mut cuts) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut blink_slots = Vec::new();
        let mut range_lods: Vec<(usize, usize)> = Vec::new();
        let mut building_at = Vec::new();
        let mut faces = Vec::new();
        let mut emitters = crate::colony_smoke::Emitters::default();
        for b in &s.buildings {
            let Some(bm) = rules.building_models.get(b.kind as usize) else { continue };
            let levels = models.entry(b.kind).or_insert_with(|| building_levels(install, &bm.model));
            let Some(levels) = levels.as_ref() else { continue };
            let m = &levels[0];
            let (w, h) = rules.building_footprints.get(b.kind as usize).copied().unwrap_or((0, 0));
            let (ox, oy) = (if w % 2 != 0 { 0.5 } else { 0.0 }, if h % 2 != 0 { -0.5 } else { 0.0 });
            let (wx, wy) = grid_to_real(b.x + ox, b.y + oy);
            let rec = Placement::genesis(m.origin, bm.scale / 1024.0 / 1000.0);
            let rel = rec.world((0.0, HALF_PI, 0.0), [0.0; 3]);
            let fm = finest.entry(b.kind).or_insert_with(|| load_lod_finest(install, "vector/colony/", &bm.model, "textures/colony/"));
            let bottom = bottom_vertex(fm.as_ref().unwrap_or(m), &rel, gm, wx, wy);
            let xf = Transform { translation: [bottom - 1.0, wx, wy], ..rel };
            building_at.push((b.slot, xf.translation));
            // Under construction: cut every frame at the built height ([`cut_height`], property 0x1000000).
            let (before, vstart) = (rg.len(), v.len());
            // Every level of detail baked; each frame draws the one `Kocka` picks ([`lod_pick`]).
            let mut level_of = Vec::new();
            let mut level0_end = 0;
            for (k, lm) in levels.iter().enumerate() {
                let n = rg.len();
                bake(lm, &xf, [1.0; 3], 1.0, &mut v, &mut rg);
                level_of.extend(std::iter::repeat_n(k, rg.len() - n));
                if k == 0 {
                    level0_end = v.len();
                }
            }
            let mut tris = vec![Vec::new(); levels.len()];
            for ((range, _, _), &k) in rg[before..].iter().zip(&level_of) {
                tris[k].extend(v[range.start as usize..range.end as usize].chunks_exact(3).map(|t| [t[0].pos, t[1].pos, t[2].pos]));
            }
            faces.push((b.slot, tris));
            // The head's record matrix is `R·diag(e)·s`, `e` the reach of the chain's vertices per axis when
            // `BoxUjraszamol` ran (it rewrites the half-extents; the drawn size stays `origin·s`), else the header's [obs: probes/lod_colony.js:
            // `npv0` (3040, 3040, 3040) records (100.32, 100.32, 31.25), its z reach 947]; `R` drops out of the length.
            let k = bm.scale / 1024.0 / 1000.0 * 1024.0;
            // (the recount runs only when every level allows it: `kemv2`'s ten frames keep `kemv0`'s header).
            let e = if levels.iter().all(|m| m.recounted) { levels.iter().map(Model::reach).fold([0.0f32; 3], |a, r| std::array::from_fn(|i| a[i].max(r[i]))) } else { levels[0].origin };
            let m1 = e.iter().map(|x| (x * k) * (x * k)).sum::<f32>();
            lods.push((b.slot, m1, xf.translation, levels.len()));
            // The Park (kind 10): `Init3dColony` sets 0x100 and 0x200 and clears 1 on the whole model, lit flat as the
            // terrain's trees ([`FLAT_LIGHT`]).
            if b.kind == 10 {
                for x in &mut v[vstart..] {
                    x.normal = [0.0; 3];
                    x.color = [FLAT_LIGHT, FLAT_LIGHT, FLAT_LIGHT, x.color[3]];
                }
            }
            // The slot's `+0x14` is `getbottomvertex`'s height, one below the model's placement.
            cuts.extend(std::iter::repeat_n((b.work_left > 0).then_some((b.slot, xf.translation[0] + 1.0)), rg.len() - before));
            smoke_places(m, &xf, b, &v[vstart..level0_end], &mut emitters);
            owners.extend(std::iter::repeat_n(b.kind, rg.len() - before));
            blink_slots.extend(std::iter::repeat_n(blinks(b).then_some(b.slot), rg.len() - before));
            worlds.extend(std::iter::repeat_n(xf.matrix, rg.len() - before));
            range_lods.extend(level_of.into_iter().map(|k| (b.slot, k)));
        }
        if !v.is_empty() {
            let mesh = r.add_mesh(&v);
            let (mut draws, mut draw_lods) = (Vec::new(), Vec::new());
            for ((((((range, pi, look), kind), world), cut), blink), lod) in rg.into_iter().zip(owners).zip(worlds).zip(cuts).zip(blink_slots).zip(range_lods) {
                let m = &models[&kind].as_ref().unwrap()[lod.1];
                let p = &m.parts[pi];
                // A building casts (0x400000), its `+r` parts (the ground plates) receive instead: `findboxxvb2` (exe
                // 0x453970) searches the root's own nodes only, so a `+r` mesh inside a child archive (the Park's
                // `fa` trees) keeps casting.
                let top = p.path.split('/').find(|c| !c.is_empty()).unwrap_or("").to_ascii_lowercase();
                let role = if top.contains("+r") { Shadow::Receive } else { Shadow::Cast };
                let is_env = building_env(p);
                if is_env || cut.is_some() || blink.is_some() {
                    let path = if is_env { env_texture(install, p, sky.as_deref(), true) } else { p.texture.clone() };
                    let t = texture(r, install, &mut textures, white, path.as_deref());
                    let inv = scene::inverse(&scene::mul(&world, &p.matrix)).unwrap_or(scene::IDENTITY);
                    let verts = v[range.start as usize..range.end as usize].to_vec();
                    env.push(EnvPart { verts, normals: p.verts.iter().map(|x| x.fnormal).collect(), inv, tex: t, glass: matches!(look, Look::Glass(_)), shadow: role, when: Some(true), env: is_env, cut, blink, lod: Some(lod) });
                    continue;
                }
                let t = texture(r, install, &mut textures, white, p.texture.as_deref());
                draws.push((range, t, matches!(look, Look::Glass(_)), role));
                draw_lods.push(Some(lod));
            }
            statics.push(Drawn { mesh, draws, when: Some(true), lods: draw_lods });
        }
        // What the human sees of a planet without satellites: `DrawBuildingBases` (ELF 0x4d2150), a quad of
        // `hazalap.bmp` over each footprint (41 world units per cell), red at half opacity.
        let base_tex = texture(r, install, &mut textures, white, install.resolve_texture("textures/colony/", "hazalap.bmp").as_deref());
        let mut v = Vec::new();
        for b in &s.buildings {
            let (w, h) = rules.building_footprints.get(b.kind as usize).copied().unwrap_or((0, 0));
            let (ox, oy) = (if w % 2 != 0 { 0.5 } else { 0.0 }, if h % 2 != 0 { -0.5 } else { 0.0 });
            let (cx, cy) = grid_to_real(b.x + ox, b.y + oy);
            let (hx, hy) = ((w * 41 / 2) as f32, (h * 41 / 2) as f32);
            let z = gm.height(b.x / 4.0, b.y / 4.0) - 3.0;
            let c = [1.0, 0.0, 0.0, 0.5];
            let q = |x: f32, y: f32, u: f32, vv: f32| MeshVertex { pos: [z, cx + x, cy + y], normal: [-0.57735, -0.57735, 0.57735], uv: [u, vv], color: c };
            v.extend([q(-hx, hy, 0.0, 1.0), q(hx, hy, 1.0, 1.0), q(hx, -hy, 1.0, 0.0), q(-hx, hy, 0.0, 1.0), q(-hx, -hy, 0.0, 0.0), q(hx, -hy, 1.0, 0.0)]);
        }
        if !v.is_empty() {
            let n = v.len() as u32;
            statics.push(Drawn { mesh: r.add_mesh(&v), draws: vec![(0..n, base_tex, true, Shadow::None)], when: Some(false), lods: Vec::new() });
        }
        // The roads (spec §3.3): `ut<n>.xvb` strips along the buildings' frame cells.
        let mut grid = road_grid(gm, &s.buildings, &rules.building_footprints);
        let mut pieces = Vec::new();
        road_pieces(&mut grid, &mut pieces);
        road_heights(&mut pieces, gm);
        let mut road_models: HashMap<usize, Option<Model>> = HashMap::new();
        let (mut v, mut rg, mut which) = (Vec::new(), Vec::new(), Vec::new());
        for p in &pieces {
            let Some(&scale) = rules.road_scale.get(p.n.wrapping_sub(1)) else { continue };
            let m = road_models
                .entry(p.n)
                .or_insert_with(|| Model::load(install, "vector/colony/", &format!("ut{}.xvb", p.n), "textures/colony/").map_err(|e| eprintln!("ig2-client: road: {e}")).ok());
            let Some(m) = m.as_ref() else { continue };
            let rec = Placement::add_to_scene(m.origin, scale);
            let mut rot = scene::mul(&scene::create_matrix(0.0, HALF_PI, 0.0), &rec.matrix);
            if p.across {
                rot = scene::mul(&axis_x(HALF_PI), &rot);
            }
            let xf = Transform { matrix: rot.map(|x| x * 1024.0), translation: [p.height - 2.5, p.at.0, p.at.1] };
            let before = rg.len();
            bake(m, &xf, [1.0; 3], 1.0, &mut v, &mut rg);
            which.extend(std::iter::repeat_n(p.n, rg.len() - before));
        }
        if !v.is_empty() {
            let mesh = r.add_mesh(&v);
            let draws = rg
                .into_iter()
                .zip(which)
                .map(|((range, pi, _), n)| {
                    let t = texture(r, install, &mut textures, white, road_models[&n].as_ref().unwrap().parts[pi].texture.as_deref());
                    // `AddUtDarab`: roads receive the shadows (0x800000 with `OptionsKellArnyek`).
                    (range, t, false, Shadow::Receive)
                })
                .collect();
            statics.push(Drawn { mesh, draws, when: Some(true), lods: Vec::new() });
        }
        Ok(Self { statics, env, finest, lods, faces, last_scaffolds: Vec::new(), falling: Vec::new(), fall_counter: None, fall_rand: crate::colony_weather::CRand(1), smoke: Default::default(), emitters, weather: crate::colony_weather::Weather::new(s.weather), scaffold_models: (0..10).map(|_| None).collect(), sky, textures, white, units: HashMap::new(), turrets: HashMap::new(), fog, building_at, radar: Some(radar), key: scene_key(s), buildings: HashMap::new(), info_target: None, globes: HashMap::new(), dropship: None, unit_centres: Vec::new() })
    }

    /// The background's fog colour (`Hatterek+0x20..+0x28`, 0..1).
    pub fn fog(&self) -> [f32; 3] {
        self.fog
    }

    /// The buildings changed since the scene was baked (built, demolished, finished): `Init3dColony`'s building
    /// loop must run again (the original adds and removes them one by one: `Build3dBuilding`, `Remove3dBuilding`).
    pub fn stale(&self, s: &ColonySnap) -> bool {
        self.key != scene_key(s)
    }

    /// A building kind's model with its textures (`LoadMaxLevelBox(Building[k − 1])`), for the build cursor.
    fn building_mesh(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, kind: u8) -> Option<&UnitMesh> {
        let (textures, white, sky) = (&mut self.textures, self.white, &self.sky);
        self.buildings
            .entry(kind)
            .or_insert_with(|| {
                let bm = rules.building_models.get(kind as usize)?;
                let mut model = Model::load(install, "vector/colony/", &bm.model, "textures/colony/").map_err(|e| eprintln!("ig2-client: building {kind}: {e}")).ok()?;
                model.set_look("_a_", Look::Glass(0.4));
                model.set_look("_c_", Look::Glass(0.4));
                let texs = model
                    .parts
                    .iter()
                    .map(|p| {
                        let path = if building_env(p) { env_texture(install, p, sky.as_deref(), true) } else { p.texture.clone() };
                        texture(r, install, textures, white, path.as_deref())
                    })
                    .collect();
                Some(UnitMesh { model, texs })
            })
            .as_ref()
    }

    /// The scene reloaded after the buildings changed (`stale`): what runs on keeps going (the smoke, the weather, the
    /// falling boxes), and the scaffolding of a building that was finished or demolished since falls over
    /// (`EpuletFelepult` exe 0x4741a0 / `Remove3dBuilding` 0x497760 → `DoljLe` 0x470e50).
    pub fn carry_from(&mut self, old: ColonyScene) {
        self.smoke = old.smoke;
        self.weather = old.weather;
        self.falling = old.falling;
        self.fall_counter = old.fall_counter;
        self.fall_rand = old.fall_rand;
        for (slot, (x, y), boxes) in old.last_scaffolds {
            if self.key.iter().any(|k| k.0 == slot && !k.4) {
                continue;
            }
            // `EpuletAllvany::Refresh`'s first call: the axis `(0, dz, −dy)/len` from the building's centre
            // (`Grid2Real(x, y)`, no half cells) to the box, a delay of `rand() % 30` frames.
            let (cx, cz) = grid_to_real(x, y);
            for (level, at) in boxes {
                let (dy, dz) = (cx - at[1], cz - at[2]);
                let len = ((dy * dy + dz * dz) as f64).sqrt() as f32;
                let len = if len == 0.0 { 1.0 } else { len };
                let delay = self.fall_rand.below(30) as i32;
                self.falling.push(Falling { level, at, axis: [0.0, dz / len, -dy / len], angle: 0.0, vel: 0.0, delay, drawn: 0.0 });
            }
        }
    }

    /// `EpuletAllvany::Refresh` (exe 0x48f920) once a UI tick up to `counter`: the delay runs down, then each tick
    /// draws the box at the angle and turns it `vel` further, `vel` growing 0.005 a tick; gone at −π/2 ([`Falling::tick`]).
    fn step_falling(&mut self, counter: f32) {
        let last = self.fall_counter.unwrap_or(counter);
        let ticks = (counter - last).max(0.0) as i32;
        self.fall_counter = Some(counter);
        for _ in 0..ticks {
            self.falling.retain_mut(Falling::tick);
        }
    }

    /// Scaffolding box `allvany<level + 1>.xvb`: one `Box01` with `allvany.bmp`, alpha-tested (`atlatszodolgok`,
    /// property 0x200).
    fn scaffold_mesh(&mut self, r: &mut Renderer, install: &Install, level: usize) -> Option<&UnitMesh> {
        let (textures, white) = (&mut self.textures, self.white);
        self.scaffold_models.get_mut(level)?
            .get_or_insert_with(|| {
                let mut model = Model::load(install, "vector/colony/", &format!("allvany{}.xvb", level + 1), "textures/colony/").map_err(|e| eprintln!("ig2-client: scaffolding: {e}")).ok()?;
                for p in &mut model.parts {
                    p.look = Look::Cutout;
                }
                let texs = model.parts.iter().map(|p| texture(r, install, textures, white, p.texture.as_deref())).collect();
                Some(UnitMesh { model, texs })
            })
            .as_ref()
    }

    /// `epkij.bmp` (`epuletkijelolestextura`, `Init3dColony`): the selected building's marker.
    /// A texture of `textures/colony/`, loaded once.
    fn colony_texture(&mut self, r: &mut Renderer, install: &Install, name: &str) -> TexId {
        let path = install.resolve_texture("textures/colony/", name);
        texture(r, install, &mut self.textures, self.white, path.as_deref())
    }

    fn marker_texture(&mut self, r: &mut Renderer, install: &Install) -> TexId {
        let path = install.resolve_texture("textures/colony/", "epkij.bmp");
        texture(r, install, &mut self.textures, self.white, path.as_deref())
    }

    fn unit_mesh(&mut self, r: &mut Renderer, install: &Install, um: &UnitModels, u: &UnitView) -> Option<&UnitMesh> {
        let hull = usize::try_from(u.hull).ok()?;
        let owner = u.owner.clamp(0, 10);
        let (textures, white) = (&mut self.textures, self.white);
        self.units
            .entry((hull, owner))
            .or_insert_with(|| {
                let h = um.hulls.get(hull)?;
                let (vdir, tdir) = model_dirs(h.dir);
                let model = Model::load(install, vdir, &h.name, tdir).map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
                // `ChangeTexture` on the first sub-mesh: `tankok/<hull><owner>.bmp`.
                let skin = ["kerekes", "kozep", "lebeg", "lanc", "aknarak"].get(hull).map(|n| format!("tankok/{n}{owner}.bmp"));
                let skin = skin.and_then(|s| install.resolve_texture("textures/colony/", &s));
                let first = model.parts.first().map(|p| p.path.split('/').nth(1).unwrap_or("").to_owned()).unwrap_or_default();
                let texs = model
                    .parts
                    .iter()
                    .map(|p| {
                        let tex = if p.path.split('/').nth(1) == Some(first.as_str()) && skin.is_some() { skin.as_deref() } else { p.texture.as_deref() };
                        texture(r, install, textures, white, tex)
                    })
                    .collect();
                Some(UnitMesh { model, texs })
            })
            .as_ref()
    }

    /// `HandleKisInfoObject` (exe 0x47d000, ELF 0x4ce840; spec §11.10) with `ColonyInfoMod` 1: the first selected
    /// unit's model (`GetSelectedMeshPointer` exe 0x47bc20: the hull's, a defence turret's gun) added to `mainxvb5` at
    /// scale 0.025, stood up (`RotateObj(π/2, 0, 0)`), at (−130, 657, −1100) of the panel's pass (fixed to its camera),
    /// turning −0.01 a UI frame about (0, 1, 0) (`Frissit2_Colony`'s `RotateObj2`, `turn` frames). Rendered into a
    /// texture of the window's size; the caller draws the info box's part of it.
    #[allow(clippy::too_many_arguments)]
    pub fn render_info_model(&mut self, r: &mut Renderer, install: &Install, um: &UnitModels, u: &UnitView, turn: f32, light: V3, size: [f32; 2]) -> Option<TexId> {
        let mesh = if u.flags & flag::TURRET != 0 { self.turret_mesh(r, install, um, u.gun?)? } else { self.unit_mesh(r, install, um, u)? };
        let (model, texs) = (mesh.model.clone(), mesh.texs.clone());
        Some(self.render_kis_info(r, &model, &texs, turn, light, size))
    }

    /// `HandleKisInfoObject` with `ColonyInfoMod` 0 (nothing selected; `GetSelectedMeshPointer` exe 0x47bc20): the
    /// planet's globe `vector/starmap/b<(surface − 1) % 8>0.xvb` (`LoadBox`, not cloned; the finest level of its detail
    /// chain, property 0x4000000, scaled by the chain head's extents), at scale 0.025 as the unit's.
    pub fn render_info_globe(&mut self, r: &mut Renderer, install: &Install, surface: u8, turn: f32, light: V3, size: [f32; 2]) -> Option<TexId> {
        let k = (surface.max(1) as usize - 1) % 8;
        if !self.globes.contains_key(&k) {
            let loaded = load_lod_finest(install, "vector/starmap/", &format!("b{k}0.xvb"), "textures/starmap/").map(|mut model| {
                // The remaster's `ig2_vertex.shader` scales the UVs by the texture's `uv_mul` (starmap.md §3.3).
                for p in &mut model.parts {
                    let k = p.texture.as_deref().map_or([1.0; 2], |t| crate::assets::Image::uv_mul(install, t));
                    for v in &mut p.verts {
                        v.uv = [v.uv[0] * k[0], v.uv[1] * k[1]];
                    }
                }
                let texs = model.parts.iter().map(|p| texture(r, install, &mut self.textures, self.white, p.texture.as_deref())).collect();
                UnitMesh { model, texs }
            });
            self.globes.insert(k, loaded);
        }
        let mesh = self.globes.get(&k)?.as_ref()?;
        let (model, texs) = (mesh.model.clone(), mesh.texs.clone());
        Some(self.render_kis_info(r, &model, &texs, turn, light, size))
    }

    /// The info box's model at scale 0.025, stood up, at [`KIS_INFO_AT`] of the panel's pass, turned `turn` frames,
    /// lit per vertex by [`kis_info_lights`].
    fn render_kis_info(&mut self, r: &mut Renderer, model: &Model, texs: &[TexId], turn: f32, light: V3, size: [f32; 2]) -> TexId {
        let rot = scene::mul(&scene::axis_matrix([0.0, 1.0, 0.0], -0.01 * turn), &scene::create_matrix(HALF_PI, 0.0, 0.0));
        let m: M3 = scene::mul(&rot, &Placement::add_to_scene(model.origin, 0.025).matrix).map(|x| x * 1024.0);
        let at = KIS_INFO_AT;
        let lights = kis_info_lights(light);
        let (mut dynamic, mut draws) = (Vec::new(), Vec::new());
        for (pi, p) in model.parts.iter().enumerate() {
            let start = dynamic.len() as u32;
            for v in &p.verts {
                let q = scene::apply(&m, v.pos);
                let c = lights.rgb(crate::lighting::normal(&m, v.lnormal));
                dynamic.push(MeshVertex { pos: [q[0] + at[0], q[1] + at[1], q[2] + at[2]], normal: [0.0; 3], uv: v.uv, color: [c[0], c[1], c[2], 1.0] });
            }
            draws.push(MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex: texs.get(pi).copied().unwrap_or(self.white), glass: false, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
        }
        let (f, px) = crate::colony_ui::panel_pass(size, (500.0, 2000.0), draws, dynamic);
        let target = match &mut self.info_target {
            Some(t) if t.size == px => t,
            _ => self.info_target.insert(r.target(px)),
        };
        r.draw_into(&f, target, px, 0.0);
        target.tex
    }

    /// The dropship's model with its own textures (no per-owner skin, no tint).
    fn dropship_mesh(&mut self, r: &mut Renderer, install: &Install) -> Option<&UnitMesh> {
        let (textures, white) = (&mut self.textures, self.white);
        self.dropship
            .get_or_insert_with(|| {
                let (vdir, tdir) = model_dirs(3);
                let model = Model::load(install, vdir, "hordozo", tdir).map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
                let texs = model.parts.iter().map(|p| texture(r, install, textures, white, p.texture.as_deref())).collect();
                Some(UnitMesh { model, texs })
            })
            .as_ref()
    }

    fn turret_mesh(&mut self, r: &mut Renderer, install: &Install, um: &UnitModels, k: usize) -> Option<&UnitMesh> {
        let (textures, white) = (&mut self.textures, self.white);
        self.turrets
            .entry((k, 0))
            .or_insert_with(|| {
                let t = um.turrets.get(k)?;
                let (vdir, tdir) = model_dirs(t.dir);
                let model = Model::load(install, vdir, &t.name, tdir).map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
                let texs = model.parts.iter().map(|p| texture(r, install, textures, white, p.texture.as_deref())).collect();
                Some(UnitMesh { model, texs })
            })
            .as_ref()
    }
}

/// `HandleKisInfoObject`'s `MoveObj(−130, 657, −1100)`: the info box's model, fixed to the panel pass's camera.
pub const KIS_INFO_AT: V3 = [-130.0, 657.0, -1100.0];

/// The info box's models' lights (`mainmeshid5` under `mainmeshid3`, `Init3dColony`): `light5` 0xffffff
/// ([`View::panel_light`]) and the node's own ambient 0x4b4b4b [obs: `probes/colony_lights.js`].
pub fn kis_info_lights(light: V3) -> crate::lighting::Lights {
    crate::lighting::Lights::default().parallel(0xffffff, light).ambient(0x4b4b4b)
}

/// `LoadBox(name)` as the info box draws it (property 0x4000000): the last level of the detail chain
/// (`lod_archive`), with the chain head's extents, which `AddToScene` scales by.
fn load_lod_finest(install: &Install, vdir: &str, name: &str, tdir: &str) -> Option<Model> {
    let root = Model::load(install, vdir, name, tdir).map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
    let mut stem = name.trim_end_matches(".xvb").to_string();
    for _ in 0..8 {
        let next = install.read(&format!("dataraw/{vdir}{stem}.xv")).ok().and_then(|b| {
            let ar = ig2_formats::xv::Archive::parse(&b).ok()?;
            let h = ig2_formats::xv::NodeHeader::parse(ar.root(Some(&stem))?.data).ok()?;
            (!h.lod_archive.is_empty()).then(|| h.lod_archive.trim_end_matches(".xvb").to_string())
        });
        match next {
            Some(n) => stem = n,
            None => break,
        }
    }
    let mut last = Model::load(install, vdir, &format!("{stem}.xvb"), tdir).ok()?;
    last.origin = root.origin;
    Some(last)
}

/// `TankAlap`/`TankTorony` `+0x34`: the model's vector and texture directories.
pub fn model_dirs(dir: i32) -> (&'static str, &'static str) {
    match dir {
        0 => ("vector/starmap/transfer/", "textures/starmap/transfer/"),
        1 => ("vector/spacewar/", "textures/spacewar/"),
        2 => ("vector/talalmanyok/", "textures/talalmanyok/"),
        _ => ("vector/colony/", "textures/colony/"),
    }
}

/// The colony screen's `MapAlloc` (`Init3dColony`): the map's build mask as 4 × 4 blocks (`Decode2LargeMap`),
/// then every building's interior (1) and frame ring (+0x20 per building) by `Allocate` in slot order.
fn road_grid(gm: &GroundMap, buildings: &[BuildingView], footprints: &[(i32, i32)]) -> Vec<u8> {
    let mut g = ig2_sim::placement::Grid::from_terrain(&gm.terrain.mask);
    for b in buildings {
        let (w, h) = footprints.get(b.kind as usize).copied().unwrap_or((0, 0));
        g.allocate(b.x as i32, b.y as i32, w, h, true);
    }
    (0..200 * 200).map(|i| g.get(i % 200, i / 200)).collect()
}

/// One road piece (`AddUtDarab`): `ut<n>.xvb` centred at `at`, `across` = along the grid's x.
#[derive(Debug, Clone, PartialEq)]
struct RoadPiece {
    n: usize,
    at: (f32, f32),
    height: f32,
    across: bool,
    /// The cells it covers: line, from, to (for the height).
    line: i32,
    from: i32,
    to: i32,
}

/// `RecreateRoadSystem` (ELF 0x4bee90): runs of frame cells (≥ 0x20) along each grid row longer than 3 become
/// road pieces (a run's ends step in by one where both neighbouring rows are frames too), then the runs along
/// each column that no row piece covered (bit 8). The heights are filled in by [`road_heights`].
fn road_pieces(g: &mut [u8], out: &mut Vec<RoadPiece>) {
    let at = |g: &[u8], y: i32, x: i32| g[(y * 200 + x) as usize];
    let frame = |g: &[u8], y: i32, x: i32| (0..200).contains(&y) && (0..200).contains(&x) && at(g, y, x) > 0x1f;
    for y in 0..200 {
        let (mut run, mut len, mut start) = (false, 0, 0);
        for x in 0..=200 {
            let end_of_run = x == 200 || at(g, y, x) < 0x20;
            if end_of_run {
                if run && len > 3 {
                    let both = |c: i32| (y > 0 && frame(g, y - 1, c)) as u8 + (y < 199 && frame(g, y + 1, c)) as u8;
                    let to = if both(x - 1) == 2 { x - 1 } else { x };
                    let from = if both(start) == 2 { start + 1 } else { start };
                    add_road(g, y, from, to, false, out);
                }
                run = false;
            } else if run {
                len += 1;
            } else {
                (len, start, run) = (1, x, true);
            }
        }
    }
    for x in 0..200 {
        let (mut run, mut start) = (false, 0);
        for y in 0..=200 {
            if y == 200 || at(g, y, x) < 0x20 || at(g, y, x) & 8 != 0 {
                if run {
                    add_road(g, x, start, y, true, out);
                }
                run = false;
            } else if !run {
                (start, run) = (y, true);
            }
        }
    }
}

/// `AddUtDarab(line, from, to, dir)` (ELF 0x4be960): pieces of at most 9 cells; a row piece marks its cells
/// (bit 8, unless bit 0x80) so the column pass skips them.
fn add_road(g: &mut [u8], line: i32, from: i32, to: i32, column: bool, out: &mut Vec<RoadPiece>) {
    if to - from >= 10 {
        let mut s = from;
        for _ in 0..(to - from - 1) / 9 + 1 {
            let e = if to - s < 10 { to } else { s + 9 };
            add_road(g, line, s, e, column, out);
            s = e;
        }
        return;
    }
    if !column {
        for x in from..to {
            let c = &mut g[(line * 200 + x) as usize];
            if *c & 0x80 == 0 {
                *c |= 8;
            }
        }
    }
    let n = (to - from).min(9) as usize;
    let mid = (from + to) as f32 / 2.0;
    // A row piece runs along grid x and is turned a quarter about the height axis; a column piece isn't.
    let at = if column { grid_to_real(line as f32 + 0.5, mid) } else { grid_to_real(mid, line as f32 + 0.5) };
    out.push(RoadPiece { n, at, height: 0.0, across: !column, line, from, to });
}

/// The pieces' heights: `GetHeight((from + to)/8, line/4)` for row pieces, `(line/4, (from + to)/8)` for columns.
fn road_heights(pieces: &mut [RoadPiece], gm: &GroundMap) {
    for p in pieces {
        let (a, b) = ((p.from + p.to) as f32 / 8.0, p.line as f32 / 4.0);
        p.height = if p.across { gm.height(a, b) } else { gm.height(b, a) };
    }
}

/// `getbottomvertex` (ELF 0x47c750): over the model's lowest vertices (within 0.5 % of its height range), the
/// largest `ground − vertex height`: how far down the model goes so it stands on the terrain.
/// A building model's level-of-detail chain (`LoadMaxLevelBox`: the head and its `lod_archive` successors), every
/// level placed by the head's extents (re/formats/xv.md, LOD), with the glass of `_A_`/`_C_` children.
fn building_levels(install: &Install, name: &str) -> Option<Vec<Model>> {
    let chain = scene::lod_chain(install, "vector/colony/", name);
    let mut levels = Vec::new();
    for stem in &chain {
        let Ok(mut m) = Model::load(install, "vector/colony/", &format!("{stem}.xvb"), "textures/colony/").map_err(|e| eprintln!("ig2-client: building {name}: {e}")) else { break };
        m.set_look("_a_", Look::Glass(0.4));
        m.set_look("_c_", Look::Glass(0.4));
        levels.push(m);
    }
    let origin = levels.first()?.origin;
    for m in &mut levels {
        m.origin = origin;
    }
    Some(levels)
}

/// `Kocka`'s level-of-detail step (exe 0x5281f0, re/formats/xv.md) for a building: `size = |M·(1, 1, 1)|²·(fov/−z)²`
/// (`m1` = the first factor in world units, `z` the depth of its position, fov 600), and the level climbs while
/// `valtashatar·30^level < 4·size` with `ShowColony`'s `valtashatar` 5625 [obs: `probes/lod_colony.js`, grwar:17:
/// kind 26 at depth 2361 has size 3075.7 = 47628·(600/2361)², level 1].
fn lod_pick(basis: &ViewBasis, m1: f32, at: V3, levels: usize) -> usize {
    let q = [at[0] - basis.eye[0], at[1] - basis.eye[1], at[2] - basis.eye[2]];
    let z = dot(basis.rows[2], q).min(-0.001);
    let size = m1 * (600.0 / -z) * (600.0 / -z);
    let (mut level, mut thr) = (0, 5625.0f32);
    while level + 1 < levels && level < 10 && thr < size * 4.0 {
        level += 1;
        thr *= 30.0;
    }
    level
}

/// `getbottomvertex` (exe 0x4a89a0, ELF 0x47c750) over `dumpvertexlist`'s vertices of the model placed by `rel` at the
/// origin: `Kocka2` at depth zero walks the level-of-detail chain to its end, so `m` is the finest level
/// ([`load_lod_finest`]) [obs: `probes/bottom.js`: kind 26 `kemv0` is measured on `kemv2`, bottom at 106.98].
fn bottom_vertex(m: &Model, rel: &Transform, map: &GroundMap, x: f32, y: f32) -> f32 {
    let pts: Vec<V3> = m.positions().map(|v| rel.point(v)).collect();
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    for p in &pts {
        if p[0] < lo {
            lo = p[0];
        }
        if hi < p[0] {
            hi = p[0];
        }
    }
    if pts.is_empty() {
        return 0.0;
    }
    let mut best = -100000.0f32;
    for p in &pts {
        if (p[0] - lo) / (hi - lo) > 0.995 {
            best = best.max(height(map, x + p[1], y + p[2]) - p[0]);
        }
    }
    best
}

// ---------------------------------------------------------------------------------------------------------
// The view

pub struct View {
    pub camera: Camera,
    pub scale: f32,
    /// What the colony screen's input adds to the 3D pass (`crate::colony_ui`).
    pub overlay: Overlay,
}

/// The build cursor and the selection markers, drawn with the scene.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay {
    /// The ground war's rings and target markers (`WF_Tank::DrawSelection`/`DrawTarget`, spec §11.4).
    pub marks: Vec<Mark>,
    /// `tempepulet` (`ShowColony`): the build menu's building at the spot found for it, half transparent.
    pub ghost: Option<Ghost>,
    /// `DrawBuildingSelection` (ELF 0x482ca0): building slot `slot`'s marker, `size` = `selectionsize`, `alpha` =
    /// `selectionatlatszosag` (0..=255).
    pub selection: Option<(usize, f32, i32)>,
    /// `LandingZone::Draw` (spec §11.10): the landing zone's outline.
    pub landing: bool,
    /// The route crosses' cells and half length ([`crate::colony_war::WarUi::routes`]).
    pub routes: (Vec<(i32, i32)>, f32),
    /// `UpdateRain`'s lightning lights this frame (`villamid`).
    pub flash: bool,
    /// `ColonyCounter` (exe 0x99823c): the colony screen's UI ticks ([`building_particles`]).
    pub counter: f32,
}

/// A flat textured square on the ground (two triangles, `polygons3d`), alpha `alpha` (0..=255).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark {
    pub tex: MarkTex,
    pub at: MarkAt,
    pub alpha: i32,
}

/// `WF+0x200/+0x208/+0x210/+0x218` (`WF::LoadMesh`): `friend.bmp`, `enemy.bmp`, `nyilak.bmp`, `epuletkij.bmp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkTex {
    Friend,
    Enemy,
    Arrow,
    Building,
}

impl MarkTex {
    fn file(self) -> &'static str {
        match self {
            MarkTex::Friend => "friend.bmp",
            MarkTex::Enemy => "enemy.bmp",
            MarkTex::Arrow => "nyilak.bmp",
            MarkTex::Building => "epuletkij.bmp",
        }
    }
}

/// Where a mark lies: around unit slot `slot` (half size `half`, `lift` above its height), or over building slot
/// `slot` grown by `grow` (`DrawTarget`'s building case: the selection marker's size with `(size − 45)·2`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkAt {
    Unit { slot: usize, half: f32, lift: f32 },
    Building { slot: usize, grow: f32 },
}

/// The build cursor: building `kind` at grid cell `at` (`AktBGridXSarga/YSarga`); `blocked` lights it red
/// (`SetLightColor(Tlight, 0xff)` when `CheckAllocation` reports the spot taken).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ghost {
    pub kind: u8,
    pub at: (i32, i32),
    pub blocked: bool,
}

impl View {
    pub fn new(s: &ColonySnap, size: (u32, u32), scale: f32, scene: &ColonyScene) -> Self {
        let mut camera = Camera::from_saved(&s.camera, [size.0 as f32, size.1 as f32]);
        // `HaVanEpuletAkkorAzLegyenKozepen`: glide to the first building (`SelectBuildingRelative(1)`): half-way
        // per frame for `ColTFrame` = 10 frames, so it stops 1/1024 short (seen in the exe: 375.6696 for 376.5427).
        if s.camera.centre {
            if let Some((_, at)) = scene.building_at.first() {
                let (tx, ty) = (at[1], -at[2]);
                for _ in 0..10 {
                    camera.x += (tx - camera.x) * 0.5;
                    camera.y += (ty - camera.y) * 0.5;
                }
            }
        }
        Self { camera, scale, overlay: Overlay::default() }
    }

    /// The panel pass's parallel light (`light5x` / `light5`, both 0xffffff at (10, 10, −10), `Init3dColony`) in that
    /// pass's camera space: `Kocka` turns it by the colony camera's rotation, though the pass's models are fixed to
    /// its own camera [obs: `probes/colony_lights.js`, colony 42 at pitch −π/4: (−0.5774, 0, −0.8165)].
    pub fn panel_light(&self, maps: &[GroundMap], map: usize) -> V3 {
        let Some(gm) = maps.get(map) else { return scene::normalise([-1.0, 0.0, -2f32.sqrt()]) };
        let rows = self.camera.basis(gm).rows;
        let l = scene::normalise([10.0, 10.0, -10.0]);
        rows.map(|w| w[0] * l[0] + w[1] * l[1] + w[2] * l[2])
    }

    pub fn resize(&mut self, size: (u32, u32), scale: f32) {
        self.camera.size = [size.0 as f32, size.1 as f32];
        self.scale = scale;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &self,
        r: &mut Renderer,
        install: &Install,
        rules: &ig2_rules::Rules,
        maps: &[GroundMap],
        scene: &mut ColonyScene,
        prev: &ColonySnap,
        curr: &ColonySnap,
        alpha: f32,
        time: f32,
    ) -> Frame {
        let fog = scene.fog;
        let mut f = Frame::new([fog[0] as f64, fog[1] as f64, fog[2] as f64, 1.0]);
        let Some(gm) = maps.get(curr.map) else { return f };
        let basis = self.camera.basis(gm);
        let mut draws: Vec<MeshDraw> = Vec::new();
        let picked: Vec<(usize, usize)> = scene.lods.iter().map(|&(slot, m1, at, n)| (slot, lod_pick(&basis, m1, at, n))).collect();
        let shown = |lod: Option<(usize, usize)>| lod.is_none_or(|(slot, k)| picked.iter().any(|&(s, l)| s == slot && l == k));
        for d in scene.statics.iter().filter(|d| d.when.is_none_or(|w| w == curr.visible)) {
            for (i, (range, tex, glass, shadow)) in d.draws.iter().enumerate() {
                if !shown(d.lods.get(i).copied().flatten()) {
                    continue;
                }
                draws.push(MeshDraw { mesh: Some(d.mesh), range: range.clone(), tex: *tex, glass: *glass, cloud: false, shadow: *shadow, video: false, additive: false, depth_write: false });
            }
        }
        // Units (spec §4): the hull at `P + body·(0,0,pivot)` with matrix `body·S`, the weapon on top.
        let mut dynamic = Vec::new();
        for e in scene.env.iter().filter(|e| e.when.is_none_or(|w| w == curr.visible) && shown(e.lod)) {
            let base = dynamic.len() as u32;
            let verts = e.verts.iter().zip(&e.normals).map(|(v, n)| if e.env { MeshVertex { uv: scene::env_uv(&e.inv, basis.eye, v.pos, *n), ..*v } } else { *v });
            let cut = e.cut.and_then(|(slot, bottom)| {
                let b = curr.buildings.iter().find(|b| b.slot == slot)?;
                cut_height(rules, curr.owner_race, b.kind, b.work_left, bottom)
            });
            let mut shadow = e.shadow;
            match cut {
                Some(c) => {
                    let all: Vec<MeshVertex> = verts.collect();
                    if shadow == Shadow::Cast {
                        dynamic.extend_from_slice(&all);
                        draws.push(MeshDraw { mesh: None, range: base..dynamic.len() as u32, tex: e.tex, glass: e.glass, cloud: false, shadow: Shadow::CastOnly, video: false, additive: false, depth_write: false });
                        shadow = Shadow::None;
                    }
                    let start = dynamic.len() as u32;
                    clip_below(&all, c, &mut dynamic);
                    draws.push(MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex: e.tex, glass: e.glass, cloud: false, shadow, video: false, additive: false, depth_write: false });
                    continue;
                }
                None => dynamic.extend(verts),
            }
            // A blinking building: `SetObjTransparency(t)` on the whole model (blended, the depth still written).
            let blink = e.blink.map(|k| 1.0 - blink_transparency(self.overlay.counter, k));
            if let Some(a) = blink {
                for x in &mut dynamic[base as usize..] {
                    x.color[3] = if e.glass { x.color[3].min(a) } else { a };
                }
            }
            draws.push(MeshDraw { mesh: None, range: base..dynamic.len() as u32, tex: e.tex, glass: e.glass || blink.is_some(), cloud: false, shadow, video: false, additive: false, depth_write: blink.is_some() });
        }
        scene.last_scaffolds.clear();
        for b in curr.buildings.iter().filter(|b| b.work_left > 0) {
            let Some(&(_, t)) = scene.building_at.iter().find(|(s, _)| *s == b.slot) else { continue };
            let boxes = scaffolds(rules, curr.owner_race, b, t[0] + 1.0);
            scene.last_scaffolds.push((b.slot, (b.x, b.y), boxes));
        }
        scene.step_falling(self.overlay.counter);
        if curr.visible {
            let mut boxes: Vec<ScaffoldBox> = scene.last_scaffolds.iter().flat_map(|(_, _, v)| v.iter().map(|&(l, at)| (l, at, None))).collect();
            boxes.extend(scene.falling.iter().map(|f| (f.level, f.at, Some((f.axis, f.drawn)))));
            for (level, at, fall) in boxes {
                let scale = rules.scaffold_scales.get(level).copied().unwrap_or(0.0);
                let Some(mesh) = scene.scaffold_mesh(r, install, level) else { continue };
                let rec = Placement::add_to_scene(mesh.model.origin, (scale as f64 * 0.038) as f32);
                let Some((axis, angle)) = fall else {
                    push_model(&mesh.model, &mesh.texs, &rec.world((0.0, HALF_PI, 0.0), at), &mut dynamic, &mut draws);
                    continue;
                };
                // `EpuletAllvany::Refresh`: `RotateObj(0, π/2, 0)` then `RotateObj2(axis, angle)`; transparent from −π/4.
                let m = scene::mul(&scene::axis_matrix(axis, angle), &rec.rotated(0.0, HALF_PI, 0.0));
                let xf = Transform { matrix: m.map(|x| x * 1024.0), translation: at };
                let t = if angle <= -std::f32::consts::FRAC_PI_4 { (-angle - std::f32::consts::FRAC_PI_4) / std::f32::consts::FRAC_PI_4 } else { 0.0 };
                let (mut v, mut rg) = (Vec::new(), Vec::new());
                bake(&mesh.model, &xf, [1.0; 3], 1.0, &mut v, &mut rg);
                let base = dynamic.len() as u32;
                for x in &mut v {
                    x.color[3] = 1.0 - t;
                }
                dynamic.extend(v);
                for (range, pi, _) in rg {
                    draws.push(MeshDraw { mesh: None, range: range.start + base..range.end + base, tex: mesh.texs[pi], glass: t > 0.0, cloud: false, shadow: Shadow::None, video: false, additive: false, depth_write: false });
                }
            }
        }
        // Snow and rain (`UpdateSnow`/`UpdateRain` once a drawn frame) around the pivot, inside and outside.
        if let Some(w) = scene.weather.as_mut() {
            w.step(self.overlay.counter);
        }
        if let Some(w) = scene.weather.as_ref() {
            let target = self.camera.eye(gm).target;
            let pivot = [target[1], target[2]];
            let base = dynamic.len() as u32;
            if w.snow() {
                // `hopehely.bmp`, colour 0x80ffffff: `rgb·(255 − 0x80)/256` = 126, added (the set's 100.0).
                let g = 126.0 / 255.0;
                for (at, size) in w.flakes(pivot) {
                    dynamic.extend(billboard(&basis, at, size, [g; 3]));
                }
                let tex = scene.colony_texture(r, install, "hopehely.bmp");
                draws.push(MeshDraw { mesh: None, range: base..dynamic.len() as u32, tex, glass: true, cloud: false, shadow: Shadow::None, video: false, additive: true, depth_write: false });
            } else {
                // `line3d2`'s colour 0xc09696 (`probes/rain.js`: words 0x80c09696, 0x10c09696), the drop's end at alpha 0x10 and the
                // streak's lower end at 0x80 [obs: the exe's streaks are brightest at the bottom].
                let px = 480.0 / self.camera.size[1].max(1.0) / 600.0;
                let c = [150.0 / 255.0, 150.0 / 255.0, 192.0 / 255.0];
                let (ca, cb) = ([c[0], c[1], c[2], 16.0 / 255.0], [c[0], c[1], c[2], 128.0 / 255.0]);
                for (a, b) in w.streaks(pivot) {
                    dynamic.extend(crate::colony_weather::line_quad(basis.eye, px, a, b, ca, cb));
                }
                draws.push(MeshDraw { mesh: None, range: base..dynamic.len() as u32, tex: scene.white, glass: true, cloud: false, shadow: Shadow::None, video: false, additive: false, depth_write: false });
            }
        }
        // The smoke, stepped to this frame's `ColonyCounter`, drawn like the buildings' particles.
        {
            let at = &scene.building_at;
            let bottom = |slot: usize| at.iter().find(|(s, _)| *s == slot).map(|(_, t)| t[0] + 1.0);
            let hp = |kind: u8| rules.buildings.get(kind as usize).map_or(0, |b| b.max_hp);
            scene.smoke.advance(self.overlay.counter, &curr.buildings, &hp, &bottom, &scene.emitters);
        }
        if !scene.smoke.puffs().is_empty() {
            let rcloud = scene.colony_texture(r, install, "rcloud.bmp");
            let base = dynamic.len() as u32;
            for q in scene.smoke.puffs() {
                dynamic.extend(billboard(&basis, q.p, q.size, q.colour()));
            }
            draws.push(MeshDraw { mesh: None, range: base..dynamic.len() as u32, tex: rcloud, glass: true, cloud: false, shadow: Shadow::None, video: false, additive: true, depth_write: false });
        }
        let parts = building_particles(&curr.buildings, &scene.building_at, self.overlay.counter);
        if curr.visible && !parts.is_empty() {
            let rcloud = scene.colony_texture(r, install, "rcloud.bmp");
            let flare = scene.colony_texture(r, install, "flare.bmp");
            for p in parts {
                let base = dynamic.len() as u32;
                dynamic.extend(billboard(&basis, p.at, p.size, p.rgb));
                let tex = if p.flare { flare } else { rcloud };
                draws.push(MeshDraw { mesh: None, range: base..base + 6, tex, glass: true, cloud: false, shadow: Shadow::None, video: false, additive: true, depth_write: false });
            }
        }
        let um = &rules.unit_models;
        scene.unit_centres.clear();
        // `WF_Tank::Draw3d`/`Update` hide the units too when the human doesn't see inside.
        for mut u in curr.units_at(prev, alpha).into_iter().filter(|_| curr.visible) {
            if u.flags & flag::TURRET == 0 {
                let tex = scene.colony_texture(r, install, SHADOW_TEXTURES[u.hull.clamp(0, 4) as usize]);
                let base = dynamic.len() as u32;
                dynamic.extend(blob_shadow(um, &u));
                draws.push(MeshDraw { mesh: None, range: base..base + 6, tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::Blob, video: false, additive: false, depth_write: false });
            }
            // `WF_Hordozo::Update` (exe 0x4a5ab0): `MoveObj((h + off_h) − 20, x + off0, y + off1)`; `WF_Tank::LoadMesh`
            // (exe 0x4847a0) added it at `AddToScene` scale 0.04 turned (0, π/2, 0) once, so it never turns.
            if let Some(d) = u.dropship {
                if let Some(mesh) = scene.dropship_mesh(r, install) {
                    let at = [u.pos[0] + d[0] - 20.0, u.pos[1] + d[1], u.pos[2] + d[2]];
                    let xf = Placement::add_to_scene(mesh.model.origin, 0.04).world((0.0, HALF_PI, 0.0), at);
                    push_model(&mesh.model, &mesh.texs, &xf, &mut dynamic, &mut draws);
                }
            }
            if let Some(d) = u.ride {
                u.pos = std::array::from_fn(|k| u.pos[k] + d[k]);
            }
            let turret_unit = u.flags & flag::TURRET != 0;
            let hull = u.hull.clamp(0, 4) as usize;
            let piv = um.pivot.get(hull).copied().unwrap_or(0.0);
            let col2 = [u.body[2], u.body[5], u.body[8]];
            let base: V3 = std::array::from_fn(|k| u.pos[k] + col2[k] * piv);
            if !turret_unit {
                if let Some(mesh) = scene.unit_mesh(r, install, um, &u) {
                    let s = um.hull_scale.get(hull).copied().unwrap_or(1.0) / 1024.0 / 10.0 / 5.0;
                    let rec = Placement::genesis(mesh.model.origin, s);
                    let xf = Transform { matrix: scene::mul(&u.body, &rec.matrix).map(|x| x * 1024.0), translation: base };
                    push_model(&mesh.model, &mesh.texs, &xf, &mut dynamic, &mut draws);
                    if let Some(c) = box_centre(&mesh.model, &xf) {
                        scene.unit_centres.push((u.slot, c));
                    }
                }
            }
            let Some(k) = u.gun else { continue };
            let Some(mesh) = scene.turret_mesh(r, install, um, k) else { continue };
            let (s, xf) = if turret_unit {
                let s = um.defence_scale.get(k.wrapping_sub(9)).copied().unwrap_or(0.2) / 1024.0 / 10.0;
                let rot = scene::mul(&axis_x(u.turret_angle), &scene::create_matrix(0.0, HALF_PI, 0.0));
                (s, Some((rot, u.pos)))
            } else {
                let s = match k {
                    0x11 => 1.953125e-05,
                    0x12 => 8.789063e-05,
                    k => um.turret_scale.get(hull * 9 + k).copied().unwrap_or(0.1) / 1024.0 / 10.0,
                };
                let off = scene::apply(&u.body, [0.0, -10.0, 12.0]);
                (s, Some((u.turret, std::array::from_fn(|i| base[i] + off[i]))))
            };
            if let Some((m, at)) = xf {
                let rec = Placement::genesis(mesh.model.origin, s);
                let xf = Transform { matrix: scene::mul(&m, &rec.matrix).map(|x| x * 1024.0), translation: at };
                push_model(&mesh.model, &mesh.texs, &xf, &mut dynamic, &mut draws);
                if let Some(c) = box_centre(&mesh.model, &xf).filter(|_| turret_unit) {
                    scene.unit_centres.push((u.slot, c));
                }
            }
        }
        self.overlay_models(r, install, rules, gm, scene, curr, &mut dynamic, &mut draws);
        // `light2` from (2, 2, −2) (`SetEnvironment`); `fenyekszamolasa` takes `|n' − L|` with the file normal's model z
        // negated, which for the quarter-turned models (`RotateObj(0, π/2, 0)`, model z = −h) flips h: the shader's
        // file normals see the light at (−2, 2, −2) [obs: colony view 18.3 → 11.7 against the other seven signs].
        let l = scene::normalise([-1.0, 1.0, -1.0]);
        // `rendershadowmap` (ELF 0x604f50): `CameraRotate(0, (2, 2, 2))`, its first two rows.
        let d = scene::normalise([2.0, 2.0, 2.0]);
        let a = scene::normalise(cross(d, [1.0, 0.0, 0.0]));
        let b = scene::normalise(cross(d, a));
        f.scene = Some(Scene3d {
            gamma: true,
            shadow: Some(ShadowMap { rows: [a, [-b[0], -b[1], -b[2]]] }),
            uniforms: SceneUniforms {
                view: self.camera.clip_matrix(&basis),
                eye: [basis.eye[0], basis.eye[1], basis.eye[2], 0.0],
                light: [l[0], l[1], l[2], 0.0],
                // `SetEnvironment` (ELF 0x4caa10) every frame: ambient 0x1e1e1e, the parallel light 0xc8c8c8 from
                // (2, 2, −2); `setfogrange(2000, 2200)` = fog from 2000 over (2200 − 2000)/0.03. The remaster's GL
                // renderer shows no distance fog (reference captures), so only the border fog is on (spec §2.5).
                ambient: [0.118, 0.118, 0.118, 0.0],
                sun: [0.784, 0.784, 0.784, 0.0],
                fog: [fog[0], fog[1], fog[2], 0.0],
                fog_range: [2000.0, 2000.0 + 200.0 / 0.03, 0.0, MAP_SIZE / 2.0],
                // `villamid` (`Init3dColony`: `CreateLightParalell(0, (1, 1, 0))` on `mainmeshid1`), `UpdateRain`'s
                // 0x806060 for the thunder's frame, else black; seen at (−1, 1, 0) as `light2` above. Its specular
                // flag (`SetLightSpecular`) adds nothing here: the client has no material specular tables.
                flash: [if self.overlay.flash { [0x60 as f32 / 255.0, 0x60 as f32 / 255.0, 0x80 as f32 / 255.0, 0.0] } else { [0.0; 4] }, {
                    let f = scene::normalise([-1.0, 1.0, 0.0]);
                    [f[0], f[1], f[2], 0.0]
                }],
            },
            draws,
            dynamic,
        });
        // Shots: glowing points (the original draws them as particles and small meshes, spec §5).
        let k = self.camera.k();
        for s in &curr.shots {
            if let Some(p) = self.camera.project(&basis, s.pos) {
                let (c, sz) = match s.list {
                    0 => ([1.0, 0.85, 0.4, 1.0], 3.0),
                    1 if s.kind == 7 => ([0.6, 0.5, 0.4, 0.6], 1.5),
                    1 => ([1.0, 0.5, 0.2, 1.0], 4.0),
                    2 => ([0.4, 0.8, 1.0, 1.0], 3.5),
                    _ => ([1.0, 1.0, 1.0, 1.0], 3.0),
                };
                f.push(scene.white, Blend::Additive, Instance::new(kind::GLOW, p, [sz * k * 2.0; 2], c));
            }
        }
        let _ = time;
        f
    }
}

impl View {
    /// The build cursor (`ShowColony`'s `tempepulet` block) and the selection marker (`DrawBuildingSelection`).
    #[allow(clippy::too_many_arguments)]
    fn overlay_models(&self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, gm: &GroundMap, scene: &mut ColonyScene, curr: &ColonySnap, dynamic: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
        if let Some(g) = self.overlay.ghost {
            let (w, h) = rules.building_footprints.get(g.kind as usize).copied().unwrap_or((0, 0));
            let scale = rules.building_models.get(g.kind as usize).map_or(0.0, |b| b.scale);
            // `getbottomvertex` measures the finest level of detail ([`bottom_vertex`]).
            let finest_bottom = rules.building_models.get(g.kind as usize).and_then(|bm| {
                let fm = scene.finest.entry(g.kind).or_insert_with(|| load_lod_finest(install, "vector/colony/", &bm.model, "textures/colony/")).as_ref()?;
                let (ox, oy) = (if w % 2 != 0 { 0.5 } else { 0.0 }, if h % 2 != 0 { -0.5 } else { 0.0 });
                let (wx, wy) = grid_to_real(g.at.0 as f32 + ox, g.at.1 as f32 + oy);
                let rel = Placement::genesis(fm.origin, scale / 1024.0 / 1000.0).world((0.0, HALF_PI, 0.0), [0.0; 3]);
                Some(bottom_vertex(fm, &rel, gm, wx, wy))
            });
            if let Some(mesh) = scene.building_mesh(r, install, rules, g.kind) {
                // The model's centre: the cell, odd sizes shifted half a cell (`Grid2Real` of the frame box's corners).
                let (ox, oy) = (if w % 2 != 0 { 0.5 } else { 0.0 }, if h % 2 != 0 { -0.5 } else { 0.0 });
                let (wx, wy) = grid_to_real(g.at.0 as f32 + ox, g.at.1 as f32 + oy);
                let rec = Placement::genesis(mesh.model.origin, scale / 1024.0 / 1000.0);
                let rel = rec.world((0.0, HALF_PI, 0.0), [0.0; 3]);
                let bottom = finest_bottom.unwrap_or_else(|| bottom_vertex(&mesh.model, &rel, gm, wx, wy));
                let xf = Transform { translation: [bottom - 1.0, wx, wy], ..rel };
                let tint = if g.blocked { [1.0, 0.25, 0.25] } else { [1.0; 3] };
                let (mut v, mut rg) = (Vec::new(), Vec::new());
                bake(&mesh.model, &xf, tint, 1.0, &mut v, &mut rg);
                let eye = self.camera.basis(gm).eye;
                for (range, pi, _) in &rg {
                    let p = &mesh.model.parts[*pi];
                    if building_env(p) {
                        env_map(p, &xf.matrix, eye, &mut v[range.start as usize..range.end as usize]);
                    }
                }
                let base = dynamic.len() as u32;
                // `SetObjTransparency(tempepulet, 0.5)`.
                for x in &mut v {
                    x.color[3] *= 0.5;
                }
                dynamic.extend(v);
                for (range, pi, _) in rg {
                    draws.push(MeshDraw { mesh: None, range: range.start + base..range.end + base, tex: mesh.texs[pi], glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
                }
            }
        }
        // `LandingZone::Draw` (exe 0x4712e0): per tile, two triangles a cell, `polygons3d(v, 3, 3)`: additive, the vertex
        // colour 0x800000ff (red, alpha 128: `RGB·A/256`).
        if self.overlay.landing && std::env::var_os("IG2_NO_LAND").is_none() {
            let buildings: Vec<(u8, f32, f32)> = curr.buildings.iter().map(|b| (b.kind, b.x, b.y)).collect();
            for (x, y, tile) in landing_outline(gm, &buildings) {
                let tex = scene.colony_texture(r, install, &format!("landz{}.bmp", tile + 1));
                let base = dynamic.len() as u32;
                dynamic.extend(landing_square(gm, x, y));
                draws.push(MeshDraw { mesh: None, range: base..base + 6, tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: true, depth_write: false });
            }
        }
        // Route crosses: two `line3d(0x80be8882)` a waypoint along the map axes, 10 above the ground: the engine's
        // `Lines` (re/screens/render.md §2.3), 1 pixel wide, depth-tested and added at alpha 0x80 (thin quads facing
        // the eye).
        let (cells, half) = &self.overlay.routes;
        if !cells.is_empty() {
            let basis = self.camera.basis(gm);
            let px = 480.0 / self.camera.size[1].max(1.0) / 600.0;
            let c = [0xbe as f32 / 255.0, 0x88 as f32 / 255.0, 0x82 as f32 / 255.0, 0x80 as f32 / 255.0];
            let base = dynamic.len() as u32;
            for &(gx, gy) in cells {
                let (x, y) = grid_to_real(gx as f32, gy as f32);
                let h = height(gm, x, y) - 10.0;
                for (a, b) in [([h, x - half, y], [h, x + half, y]), ([h, x, y - half], [h, x, y + half])] {
                    let mid: V3 = std::array::from_fn(|k| (a[k] + b[k]) / 2.0);
                    let to_eye: V3 = std::array::from_fn(|k| basis.eye[k] - mid[k]);
                    let dist = dot(to_eye, to_eye).sqrt();
                    let side = scene::normalise(cross(std::array::from_fn(|k| b[k] - a[k]), to_eye));
                    let w = dist * px / 2.0;
                    let off: V3 = side.map(|s| s * w);
                    let q = |p: V3, s: f32| MeshVertex { pos: std::array::from_fn(|k| p[k] + off[k] * s), normal: [0.0; 3], uv: [0.0; 2], color: c };
                    dynamic.extend([q(a, -1.0), q(b, -1.0), q(b, 1.0), q(a, -1.0), q(b, 1.0), q(a, 1.0)]);
                }
            }
            draws.push(MeshDraw { mesh: None, range: base..dynamic.len() as u32, tex: scene.white, glass: false, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: true, depth_write: false });
        }
        // A building's marker square: centre, half sizes (`w·40·select + grow`) and height.
        let building_at = scene.building_at.clone();
        let building = |slot: usize, grow: f32| {
            let b = curr.buildings.iter().find(|b| b.slot == slot)?;
            let &(_, at) = building_at.iter().find(|(s, _)| *s == slot)?;
            let (w, h) = rules.building_footprints.get(b.kind as usize).copied().unwrap_or((0, 0));
            let k = rules.building_models.get(b.kind as usize).map_or(0.0, |m| m.select);
            let lift = rules.scaffold_heights.get((b.kind as usize).wrapping_sub(1)).copied().unwrap_or(0.0) - 20.0;
            // Flat at the building's placed height (`+0x14`) plus `allvanypoziciok[k − 1] − 20` (up is −h).
            Some(([at[0] + lift, at[1], at[2]], (w * 0x28) as f32 * k + grow, (h * 0x28) as f32 * k + grow))
        };
        // `polygons3d(v, 3, 3)`: additive `RGB·A/256` (black is clear) [obs: the exe's rings over snow], the building
        // marker's too (`DrawBuildingSelection`).
        let mut square = |tex: TexId, at: V3, hx: f32, hy: f32, alpha: i32, additive: bool| {
            let c = [1.0, 1.0, 1.0, alpha.clamp(0, 255) as f32 / 255.0];
            let q = |x: f32, y: f32, u: f32, vv: f32| MeshVertex { pos: [at[0], at[1] + x, at[2] + y], normal: [-1.0, 0.0, 0.0], uv: [u, vv], color: c };
            let base = dynamic.len() as u32;
            dynamic.extend([q(-hx, hy, 0.0, 1.0), q(hx, hy, 1.0, 1.0), q(hx, -hy, 1.0, 0.0), q(-hx, hy, 0.0, 1.0), q(-hx, -hy, 0.0, 0.0), q(hx, -hy, 1.0, 0.0)]);
            draws.push(MeshDraw { mesh: None, range: base..base + 6, tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive, depth_write: false });
        };
        if let Some((slot, size, alpha)) = self.overlay.selection {
            if let Some((at, hx, hy)) = building(slot, size * 2.0) {
                let tex = scene.marker_texture(r, install);
                square(tex, at, hx, hy, alpha, true);
            }
        }
        for m in &self.overlay.marks {
            let tex = scene.colony_texture(r, install, m.tex.file());
            match m.at {
                MarkAt::Unit { slot, half, lift } => {
                    // The unit's map position, `lift` above its height (up is −h).
                    if let Some(u) = curr.units.iter().find(|u| u.slot == slot) {
                        square(tex, [u.pos[0] - lift, u.pos[1], u.pos[2]], half, half, m.alpha, true);
                    }
                }
                MarkAt::Building { slot, grow } => {
                    if let Some((at, hx, hy)) = building(slot, grow) {
                        square(tex, at, hx, hy, m.alpha, true);
                    }
                }
            }
        }
    }

    /// The ground point under window pixel `p` (`Update3D`'s click test against the terrain): the eye's ray
    /// marched over the heights until it goes below the ground; world `(x, y)`.
    pub fn ground_at(&self, gm: &GroundMap, p: [f32; 2]) -> Option<(f32, f32)> {
        let v = self.camera.basis(gm);
        ground_hit(gm, &v, self.pointer_ray(&v, p)).map(|(_, at)| at)
    }

    /// The eye's unit ray through window pixel `p`.
    fn pointer_ray(&self, v: &ViewBasis, p: [f32; 2]) -> V3 {
        let [vx, vy] = crate::font::Virtual::new(self.camera.size).virtual_f(p);
        let c = [(vx - 320.0) / 600.0, (vy - 240.0) / 600.0, -1.0];
        let r = v.rows;
        scene::normalise(std::array::from_fn(|j| r[0][j] * c[0] + r[1][j] * c[1] + r[2][j] * c[2]))
    }

    /// The building whose model is hit first under window pixel `p`: `ShowColony` (exe 0x49f130) reads the object of
    /// the nearest face under the click point (`pontteszt` ELF 0x5f96b0, run on every drawn triangle of the click pass)
    /// and walks it up to the building whose model holds it (`selectedepulet` exe 0x8ec994, the slot − 1). The drawn
    /// level of detail is tested; the terrain in front hides a building. Hidden buildings (a planet not seen into)
    /// can't be hit.
    /// shortcut: units in front don't hide a building and an unfinished building's cut-away top still counts;
    /// test their faces too if a player notices.
    pub fn building_at(&self, gm: &GroundMap, scene: &ColonyScene, visible: bool, p: [f32; 2]) -> Option<usize> {
        if !visible {
            return None;
        }
        let v = self.camera.basis(gm);
        let n = self.pointer_ray(&v, p);
        let ground = ground_hit(gm, &v, n).map_or(f32::INFINITY, |(t, _)| t);
        let mut best: Option<(f32, usize)> = None;
        for (slot, levels) in &scene.faces {
            let Some(&(_, m1, at, count)) = scene.lods.iter().find(|l| l.0 == *slot) else { continue };
            let Some(tris) = levels.get(lod_pick(&v, m1, at, count)) else { continue };
            for t in tris {
                if let Some(d) = ray_triangle(v.eye, n, t)
                    && d < ground
                    && best.is_none_or(|(b, _)| d < b)
                {
                    best = Some((d, *slot));
                }
            }
        }
        best.map(|(_, slot)| slot)
    }
}

/// Where the eye's ray `n` meets the ground: its distance and world `(x, y)` (`Update3D`'s click test against the
/// terrain), marched over the heights until it goes below the ground.
fn ground_hit(gm: &GroundMap, v: &ViewBasis, n: V3) -> Option<(f32, (f32, f32))> {
    let half = MAP_SIZE / 2.0;
    let mut t = 0.0f32;
    let mut last = v.eye;
    while t < 20000.0 {
        t += 8.0;
        let q = [v.eye[0] + n[0] * t, v.eye[1] + n[1] * t, v.eye[2] + n[2] * t];
        if q[1].abs() > half * 1.5 || q[2].abs() > half * 1.5 {
            return None;
        }
        // Up is −h: under the ground once h passes the terrain's height there.
        if q[0] >= height(gm, q[1], q[2]) {
            let mid = ((q[1] + last[1]) / 2.0, (q[2] + last[2]) / 2.0);
            return (mid.0.abs() <= half && mid.1.abs() <= half).then_some((t - 4.0, mid));
        }
        last = q;
    }
    None
}

/// The distance along the unit ray `n` from `o` to triangle `t`, either side (Möller–Trumbore).
fn ray_triangle(o: V3, n: V3, t: &[V3; 3]) -> Option<f32> {
    let sub = |a: V3, b: V3| -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] };
    let (e1, e2) = (sub(t[1], t[0]), sub(t[2], t[0]));
    let p = cross(n, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-9 {
        return None;
    }
    let s = sub(o, t[0]);
    let u = dot(s, p) / det;
    let q = cross(s, e1);
    let w = dot(n, q) / det;
    let d = dot(e2, q) / det;
    (u >= 0.0 && w >= 0.0 && u + w <= 1.0 && d > 0.0).then_some(d)
}

/// `Real2Grid` (ELF 0x4717e0): world `(x, y)` → the grid cell, `floor((S/2 − x)·99/S/100·99·2)` and
/// `floor((y + S/2)·99/S/100·99·2)` (f32, floored in f64).
pub fn grid_cell(x: f32, y: f32) -> (i32, i32) {
    let s = MAP_SIZE;
    let fy = ((y + s / 2.0) * 99.0) / s;
    let gx = (((-x + s / 2.0) * 99.0) / s) / 100.0 * 99.0 * 2.0;
    ((gx as f64).floor() as i32, ((fy / 100.0 * 99.0 * 2.0) as f64).floor() as i32)
}

/// The radar's picture (re/screens/hud.md §3.1): the terrain rendered from straight above, the radar's left edge
/// at world x = +S/2 (grid x 0) and its top at world y = −S/2. The exe draws kep 1 here; the GOG build shows the
/// planet's own ground [obs].
fn radar_picture(r: &mut Renderer, terrain: &Drawn) -> TexId {
    let s = 2.0 / MAP_SIZE;
    // clip = M·(h, x, y, 1): x → −x·s, y → −y·s, depth from the height (up is −h).
    let rows = [[0.0, -s, 0.0, 0.0], [0.0, 0.0, -s, 0.0], [1.0 / 20000.0, 0.0, 0.0, 0.5], [0.0, 0.0, 0.0, 1.0]];
    let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
    let l = scene::normalise([-1.0, -1.0, 1.0]);
    let draws = terrain.draws.iter().map(|(range, tex, glass, _)| MeshDraw { mesh: Some(terrain.mesh), range: range.clone(), tex: *tex, glass: *glass, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false }).collect();
    let mut f = Frame::new([0.0, 0.0, 0.0, 1.0]);
    f.scene = Some(Scene3d {
        gamma: false,
        shadow: None,
        uniforms: SceneUniforms {
            view,
            eye: [-20000.0, 0.0, 0.0, 0.0],
            light: [l[0], l[1], l[2], 0.0],
            ambient: [0.118, 0.118, 0.118, 0.0],
            sun: [0.784, 0.784, 0.784, 0.0],
            fog: [0.0; 4],
            fog_range: [1e9, 2e9, 0.0, 1e9],
            flash: [[0.0; 4]; 2],
        },
        draws,
        dynamic: Vec::new(),
    });
    r.render_texture(&f, (200, 200))
}

/// `Grid2Real`'s inverse: world `(x, y)` → grid (`WF_Tank::GetGPos`).
pub fn real_to_grid(x: f32, y: f32) -> (f32, f32) {
    let s = MAP_SIZE;
    ((s / 2.0 - x) * 99.0 * 99.0 * 2.0 / (100.0 * s), (y + s / 2.0) * 99.0 * 99.0 * 2.0 / (100.0 * s))
}

impl View {
    /// `get_cradarpos(sx, sy)` (ELF 0x4cb1a0): the ground point (h = 0) under the virtual screen point
    /// `(320 + sx, 240 + sy)`, `None` above the horizon.
    fn ground_under(&self, v: &ViewBasis, sx: f32, sy: f32) -> Option<(f32, f32)> {
        let c = [sx / 600.0, sy / 600.0, -1.0];
        let r = v.rows;
        let d: V3 = std::array::from_fn(|j| r[0][j] * c[0] + r[1][j] * c[1] + r[2][j] * c[2]);
        if d[0].abs() < 1e-6 {
            return None;
        }
        let t = -v.eye[0] / d[0];
        (t > 0.0).then(|| (v.eye[1] + t * d[1], v.eye[2] + t * d[2]))
    }

    /// The radar's view outline (`DrawRadar`): the screen corners (−320, −220), (320, −220), (−320, 240),
    /// (320, 240) on the ground, in radar pixels.
    pub fn radar_view(&self, gm: &GroundMap) -> Option<[[f32; 2]; 4]> {
        let basis = self.camera.basis(gm);
        let mut out = [[0.0; 2]; 4];
        for (k, (sx, sy)) in [(-320.0, -220.0), (320.0, -220.0), (-320.0, 240.0), (320.0, 240.0)].into_iter().enumerate() {
            let (x, y) = self.ground_under(&basis, sx, sy)?;
            let (cx, cy) = height_cell(x, y);
            out[k] = [cx * 2.0 + 522.0, cy * 2.0 + 362.0];
        }
        Some(out)
    }

    /// `ShowBuildings` (ELF 0x4c9220, re/screens/hud.md §4.1): each building's name at its placed point, faded with
    /// depth from 2300 to 2800, size 2, colour 2, centred 4 pixels left of the point.
    #[allow(clippy::too_many_arguments)]
    pub fn names(&self, f: &mut Frame, font: &crate::font::Font, palette: &[[f32; 3]], name: &dyn Fn(u8) -> Vec<u8>, scene: &ColonyScene, maps: &[GroundMap], curr: &ColonySnap) {
        let Some(gm) = maps.get(curr.map) else { return };
        if !curr.visible {
            return;
        }
        let basis = self.camera.basis(gm);
        let v = crate::font::Virtual::new(self.camera.size);
        let c = palette.get(2).copied().unwrap_or([1.0; 3]);
        for (slot, at) in &scene.building_at {
            let Some(b) = curr.buildings.iter().find(|b| b.slot == *slot) else { continue };
            let q = [at[0] - basis.eye[0], at[1] - basis.eye[1], at[2] - basis.eye[2]];
            let z = dot(basis.rows[2], q);
            let Some(p) = self.camera.project(&basis, *at) else { continue };
            // `KiirStrSzinSzorzo`: 100 nearer than 2300, 0 beyond 2800, `((z + 2300)/500 + 1)·100` between.
            let pct = if z > -2300.0 {
                100
            } else if z < -2800.0 {
                0
            } else {
                (((z + 2300.0) / 500.0 + 1.0) * 100.0) as i32
            };
            if pct == 0 {
                continue;
            }
            let fade = pct as f32 / 100.0;
            let (x, y) = ((p[0] - v.x0) / v.k, p[1] / v.k);
            let text = name(b.kind);
            font.text(f, &v, &text, (x - 4.0).trunc(), y.trunc(), 0, 2, crate::font::Align::Centre, c.map(|k| k * fade), None);
        }
    }
}

/// The colony HUD's state (re/screens/hud.md §3) for the shown planet, with the build menu on `build_kind`.
#[allow(clippy::too_many_arguments)]
/// `ShowColony` while EN has units to land (`CheckPlacing(EN)` false, `GetFirstUnPlacedTank(EN)` exe 0x47ac10;
/// spec §11.7): `AktEpuletTipus = −10 − design`, so `buildshown` is that unit's hull (`Tervek[EN][design] + 0x44 − 0x13`) in
/// the colour `min(EN, 10)`.
pub fn landing_unit(state: &SaveState, curr: &ColonySnap) -> Option<(usize, i32)> {
    let en = state.mem.i32(ig2_sim::save::va::EN);
    let &(_, design) = curr.to_land.iter().find(|&&(o, _)| o == en)?;
    let rec = state.designs.get(usize::try_from(en).ok()?)?.get(usize::try_from(design).ok()?)?;
    let hull = (i32::from_le_bytes(rec[0x44..0x48].try_into().ok()?) - 0x13).clamp(0, 4) as usize;
    Some((hull, en.min(10)))
}

#[allow(clippy::too_many_arguments)]
pub fn hud_state(state: &SaveState, tables: &ig2_rules::Rules, name: &dyn Fn(usize) -> Vec<u8>, curr: &ColonySnap, view: &View, maps: &[GroundMap], build_kind: usize, selected: usize) -> crate::hud::ColonyHud {
    use ig2_sim::save::{va, PLANET_SIZE};
    let en = state.mem.i32(va::EN).clamp(0, 18) as usize;
    let r = state.mem.read(va::PLANETS + (curr.planet * PLANET_SIZE) as u32, PLANET_SIZE);
    let i = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    let own = r[0x20] as usize == en && r[0x64] & 1 == 0;
    let build = tables.buildings.get(build_kind).map(|b| {
        let race = state.races()[en];
        let price = ig2_sim::colony_side::price(&state.tech(), race, b.tech as usize);
        (name(b.tech as usize), price, b.energy, b.workers)
    });
    let buildings = curr
        .buildings
        .iter()
        .filter(|b| b.work_left != 0 || !(0x20..=0x23).contains(&b.kind))
        .map(|b| (b.x, b.y, if b.flags & 1 != 0 { 2 } else { 1 }))
        .collect();
    let units = if curr.visible {
        curr.units.iter().map(|u| {
            let (gx, gy) = real_to_grid(u.pos[1], u.pos[2]);
            (gx, gy, u.owner as usize == en)
        }).collect()
    } else {
        Vec::new()
    };
    // `AktEpulet` (`crate::colony_ui`).
    let selected = curr.buildings.iter().find(|b| b.slot == selected).map(|b| (b.damage, b.flags));
    crate::hud::ColonyHud {
        own,
        // `GFX_Colony`/`ColonyGfx2D_3`: the planet panel and its faders when the planet is EN's and not at war, or
        // while EN still has units to land (`!CheckPlacing(EN)`, colony.md §11.1).
        panel: own || curr.to_land.iter().any(|&(o, _)| o == en as i32),
        selected,
        workers: i(0x28) - i(0x50),
        energy: i(0x48) - i(0x4c),
        build,
        buildings,
        units,
        view: maps.get(curr.map).and_then(|gm| view.radar_view(gm)),
        panel_model: None,
        info: None,
        // `ColonyGfx2D_3`: the first unit of any side still on board, its design's name (spec §11.7).
        landing_name: curr.to_land.first().filter(|_| curr.to_land.iter().any(|&(o, _)| o == en as i32)).and_then(|&(o, d)| {
            let rec = state.designs.get(usize::try_from(o).ok()?)?.get(usize::try_from(d).ok()?)?;
            Some(rec[..0x14].iter().copied().take_while(|&b| b != 0).collect())
        }),
    }
}

fn scene_key(s: &ColonySnap) -> Vec<(usize, u8, i32, i32, bool, bool)> {
    s.buildings.iter().map(|b| (b.slot, b.kind, b.x as i32, b.y as i32, b.work_left == 0, blinks(b))).collect()
}

/// `ShowColony` (exe asm 0x49f4c3..0x49f51a): a finished building with `+0x19` bit 4 (story 806's police strike,
/// re/rules/campaign/elem.md) gets the transparency `(f32)(sin(ColonyCounter/15 + k)·0.25 + 0.26)`, `k` its 1-based slot.
fn blinks(b: &BuildingView) -> bool {
    b.work_left == 0 && b.flags & 0x10 != 0
}

fn blink_transparency(counter: f32, slot: usize) -> f32 {
    ((counter as f64 / 15.0 + slot as f64).sin() * 0.25 + 0.26) as f32
}

fn axis_x(angle: f32) -> M3 {
    let (s, c) = ((angle as f64).sin() as f32, (angle as f64).cos() as f32);
    [1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c]
}

/// `GetObjCenter`: the middle of the posed model's extent.
fn box_centre(model: &Model, xf: &Transform) -> Option<V3> {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in model.parts.iter().flat_map(|p| &p.verts) {
        let p = xf.point(v.pos);
        for j in 0..3 {
            lo[j] = lo[j].min(p[j]);
            hi[j] = hi[j].max(p[j]);
        }
    }
    (lo[0] <= hi[0]).then(|| std::array::from_fn(|j| (lo[j] + hi[j]) / 2.0))
}

fn push_model(model: &Model, texs: &[TexId], xf: &Transform, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let (mut v, mut rg) = (Vec::new(), Vec::new());
    bake(model, xf, [1.0; 3], 1.0, &mut v, &mut rg);
    let base = out.len() as u32;
    out.extend(v);
    for (range, pi, look) in rg {
        draws.push(MeshDraw { mesh: None, range: range.start + base..range.end + base, tex: texs[pi], glass: matches!(look, Look::Glass(_)), cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
    }
}

/// `--colony-check`: how well the terrain object's vertices sit on the colony map's heights.
pub fn check(install: &Install, rules: &ig2_sim::game::GameRules, map: usize, surface: u8) -> Result<(), String> {
    let gm = rules.ground_maps.get(map).ok_or("no such map")?;
    let raw = install.read("dataraw/colony/terepleiro.txt").map_err(|e| e.to_string())?;
    let surfaces = colony_map::parse_surfaces(&text::decode(&raw, install.lang())).map_err(|e| e.to_string())?;
    let s = surfaces.get(map).ok_or(format!("no colony map {map}"))?;
    let model = Model::load(install, "vector/colony/terrainobj/", &s.model, &format!("textures/colony/terrainmap/{surface}/"))?;
    let rec = Placement::add_to_scene(model.origin, 1.0).scaled([4.0; 3]);
    let first = rec.world((0.0, HALF_PI, 0.0), [gm.base, 0.0, 0.0]);
    let raw_max_x = model.positions().map(|v| first.point(v)[1]).fold(0.0f32, f32::max);
    let xf = rec.scaled([4096.0 / raw_max_x; 3]).world((0.0, HALF_PI, 0.0), [gm.base, 0.0, 0.0]);
    let mut d: Vec<f32> = model
        .positions()
        .map(|v| xf.point(v))
        .filter_map(|p| {
            let (fx, fy) = height_cell(p[1], p[2]);
            ((0.6..48.9).contains(&fx) && (0.6..48.9).contains(&fy)).then(|| p[0] - gm.height(fx, fy))
        })
        .collect();
    d.sort_by(f32::total_cmp);
    if d.is_empty() {
        return Err("no vertex over the map".into());
    }
    println!(
        "map {map}: raw max x {raw_max_x}, base {}: mesh h − map height over {} verts: p10 {} median {} p90 {}",
        gm.base,
        d.len(),
        d[d.len() / 10],
        d[d.len() / 2],
        d[d.len() * 9 / 10]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exe's camera at each grab of `tools/trace/scripts/colony-camera.json` (quick-1.sav, planet 42; ← held
    /// ticks 20..60, numpad + 100..130, ↑ 180..205, → and numpad − 240..260; the injected pointer leaves the
    /// window's pointer at (0, 0): the corner's edge scroll runs throughout): `(tick, [x, y, zoom, pitch, rotate])`.
    #[allow(clippy::excessive_precision)]
    const CAMERA_TRACE: [(i32, [f32; 5]); 17] = [
        (10, [388.0970458984375, -318.9864501953125, -1100.0, -0.7853981852531433, 0.0]),
        (20, [510.1741638183594, -198.59678649902344, -1100.0, -0.7853981852531433, 0.0]),
        (30, [652.6380004882812, -72.67979431152344, -1100.0, -0.7853981852531433, -0.1405971646308899]),
        (40, [816.1217651367188, 24.165470123291016, -1100.0, -0.7853981852531433, -0.3405971825122833]),
        (60, [1183.879150390625, 112.13786315917969, -1100.0, -0.7853981852531433, -0.7405970096588135]),
        (80, [1563.42724609375, 87.79256439208984, -1100.0, -0.7853981852531433, -0.8987132906913757]),
        (100, [1941.0377197265625, 39.74980545043945, -1100.0, -0.7853981852531433, -0.9179365634918213]),
        (115, [2216.74072265625, 2.6050467491149902, -988.6122436523438, -0.7853981852531433, -0.9200493693351746]),
        (130, [2470.102783203125, -31.791284561157227, -823.732421875, -0.7853981852531433, -0.9204843640327454]),
        (160, [2938.1953125, -95.4498519897461, -760.486572265625, -0.7853981852531433, -0.9205923676490784]),
        (180, [3247.91064453125, -137.57948303222656, -760.01904296875, -0.7853981852531433, -0.9205964803695679]),
        (195, [3480.177978515625, -169.174560546875, -760.0018310546875, -0.6503065824508667, -0.9205968976020813]),
        (210, [3712.443603515625, -200.7694091796875, -760.0003662109375, -0.5015217065811157, -0.9205968976020813]),
        (240, [3800.0, -263.9591064453125, -760.0003662109375, -0.48779091238975525, -0.9205968976020813]),
        (250, [3800.0, -275.55426025390625, -821.7551879882812, -0.48778036236763, -0.779999852180481]),
        (260, [3800.0, -256.06982421875, -924.9340209960938, -0.4877793788909912, -0.5800000429153442]),
        (290, [3800.0, -81.33546447753906, -986.1958618164062, -0.48777931928634644, -0.4076303541660309]),
    ];

    /// Frissit2_Colony's integration and Colony_Autoscroll against the trace: the keys reach the camera one tick
    /// after their event, exactly (f32); the edge scroll's slide from the t20 grab on (the first grab's ten ticks hold
    /// `Colony_Move`'s one-shot `MaximizeCamera` correction), its speeds' phase found by search.
    /// `ShowColony`'s both-button turn against ig2.exe (`tools/trace/scripts/colony-turn.json`, quick-1.sav planet 42):
    /// left down at (300, 200), right down, the pointer 40 right → rotate 0 → 0.2, then 20 up → pitch −0.7854 →
    /// −0.6854; a right drag alone turns nothing.
    #[test]
    fn both_buttons_turn_the_camera_as_the_exe() {
        let mut cam = Camera { x: 0.0, y: 0.0, zoom: -1100.0, pitch: -std::f32::consts::FRAC_PI_4, rotate: 0.0, size: [640.0, 480.0], shake: [0.0; 2], tank: None };
        let mut k = CameraKeys::default();
        k.mouse_press(&cam, [300.0, 200.0]);
        k.mouse_press(&cam, [300.0, 200.0]);
        k.mouse_turn(&mut cam, [340.0, 200.0]);
        assert_eq!((cam.rotate, cam.pitch), (0.2, -std::f32::consts::FRAC_PI_4));
        k.mouse_turn(&mut cam, [340.0, 180.0]);
        assert!((cam.pitch - -0.6854).abs() < 1e-4 && cam.rotate == 0.2);
        k.mouse_release();
        k.mouse_press(&cam, [340.0, 180.0]);
        k.mouse_turn(&mut cam, [340.0, 180.0]);
        assert_eq!(cam.rotate, 0.2);
    }

    #[test]
    fn camera_keys_and_edge_scroll_follow_the_exe() {
        let held = |t: i32, from: i32, n: i32| (from..from + n).contains(&t);
        let run = |sx: i32, sy: i32| {
            let [x, y, zoom, pitch, rotate] = CAMERA_TRACE[1].1;
            let mut cam = Camera { x, y, zoom, pitch, rotate, size: [640.0, 480.0], shake: [0.0; 2], tank: None };
            let mut k = CameraKeys { scroll: [sx, sy], ..CameraKeys::default() };
            let (mut pos, mut rest) = (0.0f32, 0.0f32);
            for t in 21..=290 {
                let e = t - 1;
                (k.left, k.zoom_in, k.up) = (held(e, 20, 40), held(e, 100, 30), held(e, 180, 25));
                (k.right, k.zoom_out) = (held(e, 240, 20), held(e, 240, 20));
                k.tick(&mut cam);
                k.autoscroll(&mut cam, [0, 0], [800, 600], false);
                if let Some((_, want)) = CAMERA_TRACE.iter().find(|(g, _)| *g == t) {
                    pos = pos.max((cam.x - want[0]).abs()).max((cam.y - want[1]).abs());
                    rest = rest.max(((cam.zoom - want[2]) / want[2]).abs()).max(((cam.pitch - want[3]) / want[3]).abs()).max(((cam.rotate - want[4]) / want[4].abs().max(0.1)).abs());
                }
            }
            (pos, rest)
        };
        let best = (0..36).map(|p| run(-5 * (p % 6), -5 * (p / 6))).min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        assert!(best.0 < 0.01 && best.1 < 1e-6, "{best:?}");
    }

    #[test]
    fn grid_and_height_cells_agree_with_the_map_corners() {
        // Grid (0, 0) is world (+4096·100/99, −4096·…): grid x runs against world x.
        let (x, y) = grid_to_real(0.0, 0.0);
        assert!((x - 4096.0).abs() < 1e-3 && (y + 4096.0).abs() < 1e-3, "{x} {y}");
        let (fx, fy) = height_cell(4096.0, -4096.0);
        assert!(fx.abs() < 1e-4 && fy.abs() < 1e-4);
    }

    #[test]
    fn maximize_keeps_the_eye_inside_and_moves_the_pivot_along() {
        let mut eye = [0.0, 4000.0, 100.0];
        let mut pivot = [0.0, 3500.0, 100.0];
        maximize(&mut eye, &mut pivot, 3900.0);
        assert_eq!(eye[1], 3900.0);
        assert_eq!(pivot[1], 3400.0);
    }

    /// quick-1.sav, planet 42, as ig2.exe shows it after entering (`tools/trace/run_screen.py`, re/screens/colony.md
    /// §8): pivot (0, 375.6696, 333.1013), eye (−1343.5028, 375.6696, 1676.6041). Skips without install and save.
    #[test]
    fn building_lod_steps_like_kocka() {
        // `probes/lod_colony.js` on grwar:17: slot 18 (`kemv`, |M·1|² 47628) at depth 2361.1 has size 3075.7 and draws
        // level 1; slot 16 (`npv`, recounted extents: size 1118.05 at depth 2606.8) stays at level 0.
        let basis = ViewBasis { eye: [0.0; 3], rows: [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]] };
        assert_eq!(lod_pick(&basis, 47628.0, [-2361.0972, 0.0, 0.0], 3), 1);
        assert_eq!(lod_pick(&basis, 1118.0518 * (2606.828f32 / 600.0).powi(2), [-2606.828, 0.0, 0.0], 3), 0);
        // Close up: the chain's end; one level: always 0.
        assert_eq!(lod_pick(&basis, 47628.0, [-200.0, 0.0, 0.0], 3), 2);
        assert_eq!(lod_pick(&basis, 47628.0, [-200.0, 0.0, 0.0], 1), 0);
    }

    #[test]
    fn blink_stays_between_001_and_051() {
        let b = BuildingView { slot: 3, kind: 7, x: 0.0, y: 0.0, work_left: 0, damage: 0, flags: 0x11, yield_pct: 100, efficiency: 256 };
        assert!(blinks(&b) && !blinks(&BuildingView { work_left: 5, ..b.clone() }) && !blinks(&BuildingView { flags: 1, ..b.clone() }));
        // sin(0/15 + 3)·0.25 + 0.26, and the extremes over a period.
        assert!((blink_transparency(0.0, 3) - (3.0f64.sin() * 0.25 + 0.26) as f32).abs() < 1e-7);
        let ts: Vec<f32> = (0..95).map(|c| blink_transparency(c as f32, 1)).collect();
        assert!(ts.iter().all(|t| (0.0099..=0.5101).contains(t)));
    }

    #[test]
    fn falling_scaffold_lasts_26_ticks_after_its_delay() {
        // `EpuletAllvany::Refresh`: the angle falls by 0, 0.005, 0.010, … a tick: past −π/2 on the 26th turning tick.
        let mut f = vec![Falling { level: 0, at: [0.0; 3], axis: [0.0, 1.0, 0.0], angle: 0.0, vel: 0.0, delay: 3, drawn: 0.0 }];
        let mut ticks = 0;
        while !f.is_empty() {
            ticks += 1;
            f.retain_mut(Falling::tick);
        }
        assert_eq!(ticks, 3 + 26);
    }

    #[test]
    fn quick1_planet17_construction_matches_the_exe() {
        // `probes/scaffold.js` on grwar:17 of quick-1.sav (2026-10-25): slot 18 (kind 26, 21400 work left, `+0x14`
        // = −15.129608) is cut at 44.16639 (`setvagashatar`), its 6 × 6 boxes at height 70 with these levels by row
        // (`ModifyAllvany`'s cy 78..83, cx 99..104) and `Grid2Real` centres.
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (sim, defaults) = crate::rules::load(&install).unwrap();
        let snap = ColonySnap::read(&SaveState::from_save(&save, &defaults), 17).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let b = snap.buildings.iter().find(|b| b.slot == 18).unwrap();
        assert_eq!((b.kind, b.work_left), (26, 21400));
        let bottom = -15.129608;
        // `getbottomvertex` on the finest level (`kemv2`, probes/bottom.js: the four bottom corners at 106.98).
        let bm = &tables.building_models[26];
        let fm = load_lod_finest(&install, "vector/colony/", &bm.model, "textures/colony/").unwrap();
        let (w, h) = tables.building_footprints[26];
        let (wx, wy) = grid_to_real(b.x + if w % 2 != 0 { 0.5 } else { 0.0 }, b.y + if h % 2 != 0 { -0.5 } else { 0.0 });
        let rel = Placement::genesis(fm.origin, bm.scale / 1024.0 / 1000.0).world((0.0, HALF_PI, 0.0), [0.0; 3]);
        let got = bottom_vertex(&fm, &rel, &sim.ground_maps[snap.map], wx, wy);
        assert!((got - bottom).abs() < 1e-4, "{got}");
        let cut = cut_height(&tables, snap.owner_race, b.kind, b.work_left, bottom).unwrap();
        assert!((cut - 44.16639).abs() < 1e-4, "{cut}");
        let boxes = scaffolds(&tables, snap.owner_race, b, bottom);
        let levels: String = boxes.iter().map(|(l, _)| char::from(b'0' + *l as u8)).collect();
        assert_eq!(levels, "001100011110111111111111011110001100");
        assert!(boxes.iter().all(|(_, at)| at[0] == 70.0));
        let (first, last) = (boxes[0].1, boxes[35].1);
        assert!((first[1] - -62.27).abs() < 1e-2 && (first[2] - -815.355).abs() < 1e-2, "{first:?}");
        assert!((last[1] - -271.228).abs() < 1e-2 && (last[2] - -606.397).abs() < 1e-2, "{last:?}");
    }

    /// `probes/colony_pick.js` on colony:42 of quick-1.sav (2026-10-30): the exe's `selectedepulet` for an 8-pixel grid
    /// of click points over the virtual frame, camera (375.67, −333.10, −1100, −π/4, 0); 660 of the 4800 points hit
    /// one of 8 buildings (the slot is `selectedepulet + 1`). Ours agrees at 4793: the other 7 lie on a model's
    /// outline, where the exe's screen-space triangle test and our ray differ by under a pixel.
    #[test]
    fn quick1_planet42_building_pick_matches_the_exe() {
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let traces = std::env::var("IG2_TRACE_DIR").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../_local/traces").into());
        let Ok(text) = std::fs::read_to_string(format!("{traces}/colony-pick/picks.jsonl")) else {
            eprintln!("skipped: no colony-pick/picks.jsonl");
            return;
        };
        let Ok(mut r) = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let snap = ColonySnap::read(&SaveState::from_save(&save, &defaults), 42).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let scene = ColonyScene::load(&mut r, &install, &tables, &rules.ground_maps, &snap).unwrap();
        let gm = &rules.ground_maps[snap.map];
        let mut view = View { camera: Camera::from_saved(&snap.camera, [640.0, 480.0]), scale: 1.0, overlay: Overlay::default() };
        let (mut n, mut hits, mut same) = (0, 0, 0);
        let mut wrong = Vec::new();
        for line in text.lines() {
            let rec: serde_json::Value = serde_json::from_str(line).unwrap();
            if rec["k"] == "pickcam" {
                let c: Vec<f32> = rec["cam"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect();
                view.camera = Camera { x: c[0], y: c[1], zoom: c[2], pitch: c[3], rotate: c[4], ..view.camera };
                continue;
            }
            let (x, y) = (rec["x"].as_f64().unwrap() as f32, rec["y"].as_f64().unwrap() as f32);
            let exe = rec["sel"].as_i64().unwrap();
            let ours = view.building_at(gm, &scene, snap.visible, [x, y]).map_or(-1, |s| s as i64 - 1);
            n += 1;
            hits += (exe >= 0) as usize;
            if ours == exe {
                same += 1;
            } else {
                wrong.push((x, y, exe, ours));
            }
        }
        assert_eq!((n, hits), (4800, 660));
        assert!(wrong.len() <= 7, "{same}/{n} agree; differing (x, y, exe, ours): {wrong:?}");
    }

    #[test]
    fn quick1_planet42_camera_matches_the_exe() {
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let state = SaveState::from_save(&save, &defaults);
        let snap = ColonySnap::read(&state, 42).unwrap();
        assert!(snap.visible && snap.camera.centre);
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        let gm = &rules.ground_maps[snap.map];
        // The first building's centre (the glide's target) without loading the scene.
        let b = &snap.buildings[0];
        let (w, h) = tables.building_footprints[b.kind as usize];
        let (tx, ty) = grid_to_real(b.x + if w % 2 != 0 { 0.5 } else { 0.0 }, b.y + if h % 2 != 0 { -0.5 } else { 0.0 });
        let mut cam = Camera::from_saved(&snap.camera, [800.0, 600.0]);
        for _ in 0..10 {
            cam.x += (tx - cam.x) * 0.5;
            cam.y += (-ty - cam.y) * 0.5;
        }
        let e = cam.eye(gm);
        let close = |a: f32, b: f32| (a - b).abs() < 2e-3;
        assert!(close(e.target[1], 375.6696) && close(e.target[2], 333.1013), "{:?}", e.target);
        assert!(close(e.eye[0], -1343.5028) && close(e.eye[1], 375.6696) && close(e.eye[2], 1676.6041), "{:?}", e.eye);
    }

    #[test]
    fn the_camera_looks_at_its_pivot() {
        let cam = Camera { x: 100.0, y: -200.0, zoom: -1100.0, pitch: -std::f32::consts::FRAC_PI_4, rotate: 0.3, size: [640.0, 480.0], shake: [0.0; 2], tank: None };
        let terrain = ig2_sim::placement::Terrain { mask: [0; 650], kind: 0, start: (0, 0) };
        let map = GroundMap { terrain, heights: vec![0; 2500], base: 0.0, footprints: Vec::new() };
        let b = cam.basis(&map);
        let Eye { target, .. } = cam.eye(&map);
        let p = cam.project(&b, target).unwrap();
        assert!((p[0] - 320.0).abs() < 1e-2 && (p[1] - 240.0).abs() < 1e-2, "{p:?}");
        // Up on screen is −h.
        let up = cam.project(&b, [target[0] - 50.0, target[1], target[2]]).unwrap();
        assert!(up[1] < p[1]);
    }

    /// The panel pass's parallel light in its camera space against ig2.exe (`probes/colony_lights.js`, quick-1.sav
    /// planet 42, the camera at pitch −π/4, rotate 0): `vec` (−0.5774, 0, −0.8165) on every object of the pass.
    #[test]
    fn panel_light_turns_with_the_colony_camera() {
        let camera = Camera { x: 375.67, y: -333.1, zoom: -1100.0, pitch: -std::f32::consts::FRAC_PI_4, rotate: 0.0, size: [640.0, 480.0], shake: [0.0; 2], tank: None };
        let terrain = ig2_sim::placement::Terrain { mask: [0; 650], kind: 0, start: (0, 0) };
        let map = GroundMap { terrain, heights: vec![0; 2500], base: 0.0, footprints: Vec::new() };
        let l = View { camera, scale: 1.0, overlay: Overlay::default() }.panel_light(&[map], 0);
        let exe = [-0.5774, 0.0, -0.8165];
        assert!((0..3).all(|i| (l[i] - exe[i]).abs() < 1e-4), "{l:?}");
    }
}
