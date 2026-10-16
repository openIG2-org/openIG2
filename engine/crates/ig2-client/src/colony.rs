//! The colony screen (re/screens/colony.md): the planet's terrain object, its buildings and the ground war's
//! units and shots, seen through the original's colony camera, from the sim's state each tick.

use std::collections::HashMap;

use ig2_formats::{colony_map, text, Install};
use ig2_rules::UnitModels;
use ig2_sim::ground::{flag, Field, GroundMap};
use ig2_sim::save::SaveState;

use crate::assets::Image;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};
use crate::render3d::{MeshDraw, MeshId, MeshVertex, Scene3d, SceneUniforms};
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
fn height(map: &GroundMap, x: f32, y: f32) -> f32 {
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
    /// `+0x19` bit 4: blinking (`ShowColony`).
    pub flags: u8,
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
    /// The weapon pod the turret model shows (`TankTorony` index), `None` without one.
    pub gun: Option<usize>,
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
            .map(|(i, b)| BuildingView { slot: i + 1, kind: b.kind, x: b.x, y: b.y, work_left: b.work_left, damage: b.damage, flags: b.flags })
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
                    gun,
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
        let sats = if en == 7 && p.spies[7] == 0 { 1 } else { p.spies[en] };
        let visible = p.owner as usize == en || sats != 0;
        let camera = if field.units.is_empty() && field.tail.f1a8 == [0; 5] { SavedCamera::NEW_GAME } else { SavedCamera::read(&field) };
        Some(Self {
            planet,
            tick: state.mem.i32(ig2_sim::save::va::IDO2),
            owner: p.owner,
            surface: p.surface,
            map: p.colony_map as usize,
            buildings,
            units,
            shots,
            camera,
            weather: field.weather,
            background: field.background,
            at_war: field.units.iter().zip(&field.alive).any(|(u, &a)| a && u.flags & flag::TURRET == 0 && u.owner != p.owner as i32),
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
        Self { x: s.x, y: s.y, zoom: s.zoom, pitch: s.pitch, rotate: s.rotate, size }
    }

    /// `ShowColony`'s camera block: pivot `(0, XStart, −YStart)`, eye at `Zoom − 800` from it along pitch and
    /// rotation, clamped to ±3900 (`MaximizeCamera`), kept 100 above the ground under it.
    pub fn eye(&self, map: &GroundMap) -> Eye {
        let x = self.x.clamp(-3800.0, 3800.0);
        let y = self.y.clamp(-3800.0, 3800.0);
        let mut target = [0.0, x, -y];
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
    fn k(&self) -> f32 {
        self.size[1] / 480.0
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
        let k = self.k();
        Some([self.size[0] / 2.0 + (vx - 320.0) * k, vy * k])
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

    /// The original's mouse controls (`ShowColony`: both buttons drag rotate/pitch by 1/200 per pixel, pitch
    /// clamped −1.4922565..−0.05).
    pub fn turn(&mut self, dx: f32, dy: f32) {
        self.rotate -= dx / 200.0;
        self.pitch = (self.pitch + dy / 200.0).clamp(-1.4922565, -0.05);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (s, c) = (self.rotate.sin(), self.rotate.cos());
        let k = (800.0 - self.zoom).max(100.0) / 400.0;
        self.x = (self.x + (c * dx - s * dy) * k).clamp(-3800.0, 3800.0);
        self.y = (self.y + (s * dx + c * dy) * k).clamp(-3800.0, 3800.0);
    }

    pub fn zoom_steps(&mut self, steps: f32) {
        self.zoom = (self.zoom + steps * 60.0).clamp(-2500.0, 700.0);
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
    draws: Vec<(std::ops::Range<u32>, TexId, bool)>,
    /// Drawn only when the human sees inside (`Some(true)`) or only when not (`Some(false)`).
    when: Option<bool>,
}

/// A model's parts with their textures resolved, for per-frame placement (units).
struct UnitMesh {
    model: Model,
    texs: Vec<TexId>,
}

/// One planet's colony scene: what `Init3dColony` loads, uploaded once.
pub struct ColonyScene {
    statics: Vec<Drawn>,
    textures: HashMap<String, TexId>,
    white: TexId,
    units: HashMap<(usize, i32), Option<UnitMesh>>,
    turrets: HashMap<(usize, i32), Option<UnitMesh>>,
    fog: [f32; 3],
    /// The buildings' world transforms by slot (for markers).
    pub building_at: Vec<(usize, V3)>,
    /// The radar's picture: the terrain seen from straight above (re/screens/hud.md §3.1).
    pub radar: Option<TexId>,
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
        let upload = |r: &mut Renderer, textures: &mut HashMap<String, TexId>, model: &Model, verts: Vec<MeshVertex>, ranges: Vec<(std::ops::Range<u32>, usize, Look)>| {
            let mesh = r.add_mesh(&verts);
            let draws = ranges
                .into_iter()
                .map(|(rg, pi, look)| {
                    let t = texture(r, install, textures, white, model.parts[pi].texture.as_deref());
                    (rg, t, matches!(look, Look::Glass(_)))
                })
                .collect();
            Drawn { mesh, draws, when: None }
        };

        // The background dome (`Hatterek[WF+0xa04]`, `AddToScene(…, 11)`, scaled (1, 1, 1.1), moved to h = −3000).
        let bg = backgrounds.get(s.background.max(0) as usize).or(backgrounds.first());
        let fog = bg.map_or([0.0; 3], |b| std::array::from_fn(|k| b.fog.get(k).copied().unwrap_or(0) as f32 / 255.0));
        if let Some(b) = bg {
            match Model::load(install, "vector/colony/", &b.model, "textures/colony/") {
                Ok(m) => {
                    let xf = Placement::add_to_scene(m.origin, 11.0).scaled([1.0, 1.0, 1.1]).world((0.0, HALF_PI, 0.0), [-3000.0, 0.0, 0.0]);
                    let (mut v, mut rg) = (Vec::new(), Vec::new());
                    bake(&m, &xf, [1.0; 3], 0.0, &mut v, &mut rg);
                    statics.push(upload(r, &mut textures, &m, v, rg));
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
        statics.push(upload(r, &mut textures, &terrain, v, rg));
        let radar = radar_picture(r, statics.last().unwrap());

        // The buildings (spec §3): `AddToSceneGenesis(model, scale/1024/1000)`, turned a quarter, on the ground.
        let mut models: HashMap<u8, Option<Model>> = HashMap::new();
        let (mut v, mut rg, mut owners) = (Vec::new(), Vec::new(), Vec::new());
        let mut building_at = Vec::new();
        for b in &s.buildings {
            let Some(bm) = rules.building_models.get(b.kind as usize) else { continue };
            let m = models.entry(b.kind).or_insert_with(|| {
                Model::load(install, "vector/colony/", &bm.model, "textures/colony/")
                    .map(|mut m| {
                        m.set_look("_a_", Look::Glass(0.4));
                        m.set_look("_c_", Look::Glass(0.4));
                        m
                    })
                    .map_err(|e| eprintln!("ig2-client: building {}: {e}", b.kind))
                    .ok()
            });
            let Some(m) = m.as_ref() else { continue };
            let (w, h) = rules.building_footprints.get(b.kind as usize).copied().unwrap_or((0, 0));
            let (ox, oy) = (if w % 2 != 0 { 0.5 } else { 0.0 }, if h % 2 != 0 { -0.5 } else { 0.0 });
            let (wx, wy) = grid_to_real(b.x + ox, b.y + oy);
            let rec = Placement::genesis(m.origin, bm.scale / 1024.0 / 1000.0);
            let rel = rec.world((0.0, HALF_PI, 0.0), [0.0; 3]);
            let bottom = bottom_vertex(m, &rel, gm, wx, wy);
            let xf = Transform { translation: [bottom - 1.0, wx, wy], ..rel };
            building_at.push((b.slot, xf.translation));
            // Under construction: drawn see-through until finished (the original cuts the model at the built height
            // and puts up scaffolding: `SetEpuletVagas`, `EpuletAllvanyok`).
            let tint = if b.work_left > 0 { [0.6, 0.75, 1.0] } else { [1.0; 3] };
            let before = rg.len();
            bake(m, &xf, tint, 1.0, &mut v, &mut rg);
            if b.work_left > 0 {
                for x in &mut rg[before..] {
                    x.2 = Look::Glass(0.45);
                }
            }
            owners.extend(std::iter::repeat_n(b.kind, rg.len() - before));
        }
        if !v.is_empty() {
            let mesh = r.add_mesh(&v);
            let mut draws = Vec::new();
            for ((range, pi, look), kind) in rg.into_iter().zip(owners) {
                let m = models[&kind].as_ref().unwrap();
                let t = texture(r, install, &mut textures, white, m.parts[pi].texture.as_deref());
                draws.push((range, t, matches!(look, Look::Glass(_))));
            }
            statics.push(Drawn { mesh, draws, when: Some(true) });
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
            statics.push(Drawn { mesh: r.add_mesh(&v), draws: vec![(0..n, base_tex, true)], when: Some(false) });
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
                    (range, t, false)
                })
                .collect();
            statics.push(Drawn { mesh, draws, when: Some(true) });
        }
        Ok(Self { statics, textures, white, units: HashMap::new(), turrets: HashMap::new(), fog, building_at, radar: Some(radar) })
    }

    /// The background's fog colour (`Hatterek+0x20..+0x28`, 0..1).
    pub fn fog(&self) -> [f32; 3] {
        self.fog
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

/// `TankAlap`/`TankTorony` `+0x34`: the model's vector and texture directories.
fn model_dirs(dir: i32) -> (&'static str, &'static str) {
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
        Self { camera, scale }
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
        for d in scene.statics.iter().filter(|d| d.when.is_none_or(|w| w == curr.visible)) {
            for (range, tex, glass) in &d.draws {
                draws.push(MeshDraw { mesh: Some(d.mesh), range: range.clone(), tex: *tex, glass: *glass });
            }
        }
        // Units (spec §4): the hull at `P + body·(0,0,pivot)` with matrix `body·S`, the weapon on top.
        let mut dynamic = Vec::new();
        let um = &rules.unit_models;
        // `WF_Tank::Draw3d`/`Update` hide the units too when the human doesn't see inside.
        for u in curr.units_at(prev, alpha).into_iter().filter(|_| curr.visible) {
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
            }
        }
        let l = scene::normalise([-1.0, -1.0, 1.0]);
        f.scene = Some(Scene3d {
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
            },
            draws,
            dynamic,
        });
        // Shots: glowing points (the original draws them as particles and small meshes, spec §5).
        let k = self.camera.size[1] / 480.0;
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

/// The radar's picture (re/screens/hud.md §3.1): the terrain rendered from straight above, the radar's left edge
/// at world x = +S/2 (grid x 0) and its top at world y = −S/2. The exe draws kep 1 here; the GOG build shows the
/// planet's own ground [obs].
fn radar_picture(r: &mut Renderer, terrain: &Drawn) -> TexId {
    let s = 2.0 / MAP_SIZE;
    // clip = M·(h, x, y, 1): x → −x·s, y → −y·s, depth from the height (up is −h).
    let rows = [[0.0, -s, 0.0, 0.0], [0.0, 0.0, -s, 0.0], [1.0 / 20000.0, 0.0, 0.0, 0.5], [0.0, 0.0, 0.0, 1.0]];
    let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
    let l = scene::normalise([-1.0, -1.0, 1.0]);
    let draws = terrain.draws.iter().map(|(range, tex, glass)| MeshDraw { mesh: Some(terrain.mesh), range: range.clone(), tex: *tex, glass: *glass }).collect();
    let mut f = Frame::new([0.0, 0.0, 0.0, 1.0]);
    f.scene = Some(Scene3d {
        uniforms: SceneUniforms {
            view,
            eye: [-20000.0, 0.0, 0.0, 0.0],
            light: [l[0], l[1], l[2], 0.0],
            ambient: [0.118, 0.118, 0.118, 0.0],
            sun: [0.784, 0.784, 0.784, 0.0],
            fog: [0.0; 4],
            fog_range: [1e9, 2e9, 0.0, 1e9],
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
pub fn hud_state(state: &SaveState, tables: &ig2_rules::Rules, name: &dyn Fn(usize) -> Vec<u8>, curr: &ColonySnap, view: &View, maps: &[GroundMap], build_kind: usize) -> crate::hud::ColonyHud {
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
    // `AktEpulet`: entering with `WF+0x184` selects the first building (`SelectBuildingRelative(1)`).
    let selected = curr.buildings.first().filter(|_| curr.camera.centre).map(|b| (b.damage, b.flags));
    crate::hud::ColonyHud {
        own,
        selected,
        workers: i(0x28) - i(0x50),
        energy: i(0x48) - i(0x4c),
        build,
        buildings,
        units,
        view: maps.get(curr.map).and_then(|gm| view.radar_view(gm)),
    }
}

fn axis_x(angle: f32) -> M3 {
    let (s, c) = ((angle as f64).sin() as f32, (angle as f64).cos() as f32);
    [1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c]
}

fn push_model(model: &Model, texs: &[TexId], xf: &Transform, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let (mut v, mut rg) = (Vec::new(), Vec::new());
    bake(model, xf, [1.0; 3], 1.0, &mut v, &mut rg);
    let base = out.len() as u32;
    out.extend(v);
    for (range, pi, look) in rg {
        draws.push(MeshDraw { mesh: None, range: range.start + base..range.end + base, tex: texs[pi], glass: matches!(look, Look::Glass(_)) });
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
        let cam = Camera { x: 100.0, y: -200.0, zoom: -1100.0, pitch: -std::f32::consts::FRAC_PI_4, rotate: 0.3, size: [640.0, 480.0] };
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
}
