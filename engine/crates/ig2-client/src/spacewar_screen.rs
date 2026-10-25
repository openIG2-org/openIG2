//! The real-time space battle's screen (`AktKep` 11, re/screens/spacewar.md): `GFX_SpaceWar` (ELF 0x67c710, exe
//! 0x55e850) → `SpwC::screenfrissites` (ELF 0x6a7780, exe 0x57f800) for the shown battle slot.
//!
//! The battle is drawn inside the starmap's own 3D scene (spec §1.0): `initgfx` shifts the galaxy by the battle's
//! origin (`sceneeltol`, exe slot `+0x1f74`) and, once the opening zoom ends, `rescalescene(2500)` scales it by 2500, so
//! the galaxy behind the battle is the starmap seen through the battle's look-at camera (`CameraRotate`, spec §2). The
//! battle's state is `ig2-sim`'s [`Battle`]; a capture's dump (`run_spwc.py --grab-ticks`) carries the exe's slot as
//! raw regions, decoded by [`ig2_sim::spacebattle_raw`].

use std::collections::HashMap;

use ig2_sim::spacebattle::{Battle, ShipType};
use ig2_sim::spacebattle_raw::{self as raw, Mem, Statics};
use serde_json::Value;

use crate::starmap::Look;

/// `rescalescene(2500)` at the end of the opening zoom: battle units per starmap unit (spec §2.5).
pub const SCENE_SCALE: f32 = 2500.0;

/// A battle as a capture left it: the decoded slot, its raw bytes (the screen's own fields, exe offsets), and the
/// screen's globals.
pub struct Dump {
    pub battle: Battle,
    /// The slot (`SpwC`, 0x2D9C bytes).
    pub obj: Vec<u8>,
    /// `spwkamerapont` / `spwforgaspont` (exe 0xE72DD4 / 0xE72DE4: h, x, y, w each), the per-tick smoothed eye and
    /// target (spec §2.2), when the capture has them.
    pub cam: Option<[f32; 8]>,
    pub acttime: Option<u32>,
    pub futofeny: Option<f32>,
    /// `smkeringeto` (exe 0xF176DC), the console's side lights' phase, when the capture has it.
    pub kering: Option<f32>,
    /// The units as dumped (600 bytes each, exe layout).
    pub units_raw: Vec<u8>,
    /// `starmaperosseg` (exe 0x927994): how much of the starmap shows (0 at the battle's zoom).
    pub erosseg: Option<f32>,
    /// The live ship-type records the units point at (`+0x214`): their model names and scales (the battle's setup
    /// rewrites the records; the station types' names differ from the exe file's table).
    pub types: HashMap<u64, (String, f32)>,
}

impl Dump {
    /// Unit `i`'s model name and scale (`tipusnevek` record `+0`, `+0x10`).
    pub fn unit_type(&self, i: usize, tables: &ig2_rules::Rules) -> Option<(String, f32)> {
        let r = self.units_raw.get(i * 600..(i + 1) * 600)?;
        if let Some(t) = self.types.get(&(raw::u32_at(r, 0x214) as u64)) {
            return Some(t.clone());
        }
        let u = self.battle.units.get(i)?;
        let t = tables.ship_types.get((u.race * 12 + u.kind).max(0) as usize)?;
        Some((type_model(tables, u.race, u.kind)?, t.scale))
    }
}

/// The model name of ship type `kind` of `race` (`tipusnevek[race·12 + kind]` `+0`). The planet stations' types 8..=11
/// hold "TSCtablazatabol" in the exe file: `terv2tipus` (exe 0x582010) copies the hull tech's research model
/// (`TTeruletek[tech − 1]`, tech `kind + 0x45`) into the record whenever it maps a station design, which `AddFleet`'s
/// sort does before any unit is added (spec §1.3).
pub fn type_model(tables: &ig2_rules::Rules, race: i32, kind: i32) -> Option<String> {
    if (8..=11).contains(&kind) {
        return tables.research_screen.models.get((kind + 0x45 - 1) as usize).cloned();
    }
    tables.ship_types.get((race * 12 + kind).max(0) as usize).map(|t| t.name.clone())
}

fn unhex(s: &str) -> Vec<u8> {
    raw::unhex(s)
}

/// The dump's static data (the shape `spwc.js`'s `statics()` writes).
fn statics(v: &Value) -> Statics {
    let mut types = HashMap::new();
    for (k, t) in v["types"].as_object().into_iter().flatten() {
        let b = unhex(t.as_str().unwrap_or(""));
        if b.len() < 0x30 {
            continue;
        }
        types.insert(raw::ptr(k), ShipType {
            scale: raw::f32_at(&b, 0x10),
            top_speed: raw::f32_at(&b, 0x14),
            accel: raw::f32_at(&b, 0x1c),
            turn: raw::f32_at(&b, 0x24),
            size_xy: raw::i32_at(&b, 0x28),
            size_z: raw::i32_at(&b, 0x2c),
        });
    }
    let mut designs = vec![Vec::new(); 19];
    for (k, d) in v["designs"].as_object().into_iter().flatten() {
        if let Ok(e) = k.parse::<usize>() {
            designs[e.min(18)] = unhex(d.as_str().unwrap_or("")).chunks(0x84).map(ig2_sim::spacewar::Design::from_bytes).collect();
        }
    }
    let building_hp = v.get("buildingHp").map_or(Vec::new(), |b| unhex(b.as_str().unwrap_or("")).chunks(0x24).map(|c| raw::i32_at(c, 0)).collect());
    Statics { weapons: unhex(v["weapons"].as_str().unwrap_or("")), building_hp, types, designs }
}

impl Dump {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("--battle {path}: {e}"))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("--battle {path}: {e}"))?;
        let mut mem = Mem::default();
        for (k, r) in v["regions"].as_object().ok_or("--battle: no regions")? {
            let (Some(p), Some(full)) = (r["p"].as_str(), r["full"].as_str()) else { continue };
            mem.r.insert(k.clone(), (raw::ptr(p), unhex(full)));
        }
        if !mem.r.contains_key("obj") || !mem.r.contains_key("units") {
            return Err(format!("--battle {path}: no battle slot"));
        }
        let st = statics(&v["statics"]);
        let mut types = HashMap::new();
        for (k, t) in v["statics"]["types"].as_object().into_iter().flatten() {
            let b = unhex(t.as_str().unwrap_or(""));
            if b.len() >= 0x14 {
                let name = String::from_utf8_lossy(b[..0x10].split(|&c| c == 0).next().unwrap_or(&[])).into_owned();
                types.insert(raw::ptr(k), (name, raw::f32_at(&b, 0x10)));
            }
        }
        let battle = raw::decode(&mem, &st);
        let obj = mem.get("obj").to_vec();
        let units_raw = mem.get("units").to_vec();
        let cam = v["cam"].as_str().map(unhex).filter(|b| b.len() >= 32).map(|b| std::array::from_fn(|i| raw::f32_at(&b, 4 * i)));
        Ok(Self {
            battle,
            obj,
            cam,
            acttime: v["acttime"].as_u64().map(|x| x as u32),
            futofeny: v["futofeny"].as_f64().map(|x| x as f32),
            kering: v["kering"].as_f64().map(|x| x as f32),
            units_raw,
            erosseg: v["erosseg"].as_f64().map(|x| x as f32),
            types,
        })
    }

    fn f(&self, o: usize) -> f32 {
        raw::f32_at(&self.obj, o)
    }

    /// The dump as the renderer reads it: the units' drawn poses and selection timers (exe unit `+0xe4`, `+0x234`,
    /// `+0x181`, `+0x184..+0x18c`), the weapon column, `infoszamol`'s statistics, the radar's yaw `+0xc7c`.
    pub fn shown(&self, tables: &ig2_rules::Rules) -> Shown<'_> {
        let v = self.view();
        let (eye, target) = match self.cam {
            Some(c) if !v.restoring => ([c[0], c[1], c[2]], [c[4], c[5], c[6]]),
            _ => v.eye_target(),
        };
        let units = (0..self.battle.units.len())
            .map(|i| {
                let r = &self.units_raw[i * 600..(i + 1) * 600];
                crate::spacewar_ui::UnitGfx {
                    matrix: std::array::from_fn(|k| raw::f32_at(r, 0xe4 + 4 * k)),
                    turn: std::array::from_fn(|k| raw::f32_at(r, 0x234 + 4 * k)),
                    selected: r[0x181] != 0,
                    was_selected: r[0x181] != 0,
                    grow: raw::i32_at(r, 0x184),
                    fade: raw::i32_at(r, 0x188),
                    attacked: raw::i32_at(r, 0x18c),
                    ..Default::default()
                }
            })
            .collect();
        Shown {
            battle: &self.battle,
            view: v,
            eye,
            target,
            units,
            types: (0..self.battle.units.len()).map(|i| self.unit_type(i, tables)).collect(),
            slots: std::array::from_fn(|i| Slot::from_slot(&self.obj, i)),
            stats: crate::spacewar_hud::Stats::from_slot(&self.obj),
            snap_yaw: self.f(0xc7c),
            erosseg: self.erosseg.unwrap_or(0.0),
            fade: 1.0,
            radar: 100,
            spin: 0.0,
            hidden_button: None,
            boxed: None,
            fx: None,
            chased: None,
            gun_t: self.battle.guns.iter().map(|g| g.hits).collect(),
            gun_timers: Vec::new(),
            gun_damage: Vec::new(),
            info_other: None,
            title: Vec::new(),
            title_tech: -1,
            stance: 0,
            big: None,
            pointer: 0,
            formation: None,
            kering: self.kering.unwrap_or(0.0),
        }
    }

    /// The starmap camera `initgfx` saved at entry (`StarmapXStart`, `StarmapYStart`, rotate, pitch, zoom): the pan
    /// `+0x1f9c` is the starmap pivot `(0, XStart, −YStart)` minus `starmapeltolas` `+0x1f74`; yaw `+0x1fac`, pitch
    /// `+0x1fb0`, distance `+0x1fb4` (`600 − StarmapZoom`).
    pub fn saved_starmap_camera(&self) -> [f32; 5] {
        let (o, p) = ([self.f(0x1f78), self.f(0x1f7c)], [self.f(0x1fa0), self.f(0x1fa4)]);
        [p[0] + o[0], -(p[1] + o[1]), self.f(0x1fac), self.f(0x1fb0), 600.0 - self.f(0x1fb4)]
    }

    /// The camera fields (exe offsets; spec §2.1): pivot `+0xc5c` (h, x, y), yaw `+0xc80`, tilt `+0xc84`, zoom
    /// `+0xc88`; the view animation runs while `+0x1d3e` is set; `xcenter` `+0x1fc0`.
    pub fn view(&self) -> View {
        View {
            pivot: [self.f(0xc5c), self.f(0xc60), self.f(0xc64)],
            yaw: self.f(0xc80),
            tilt: self.f(0xc84),
            zoom: self.f(0xc88),
            restoring: self.obj[0x1d3e] != 0,
            xcenter: self.f(0x1fc0),
            origin: [self.f(0x1f74), self.f(0x1f78), self.f(0x1f7c)],
            scale: self.f(0x1f88),
        }
    }
}

/// One frame of the battle screen, from a capture's dump or the live screen ([`crate::spacewar_ui::SpaceWarUi`]).
pub struct Shown<'a> {
    pub battle: &'a Battle,
    pub view: View,
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub units: Vec<crate::spacewar_ui::UnitGfx>,
    /// Each unit's model name and `AddToScene` scale (`tipusnevek[race·12 + type]`).
    pub types: Vec<Option<(String, f32)>>,
    pub slots: [Slot; 10],
    pub stats: crate::spacewar_hud::Stats,
    /// `+0xd6c` (exe `+0xc7c`): the radar's quarter-turn yaw.
    pub snap_yaw: f32,
    pub erosseg: f32,
    /// `spacewarelfade`, `spwradarcounter`, the info model's turn.
    pub fade: f32,
    pub radar: i32,
    pub spin: f32,
    /// The armed weapon group's button in its blink's off phase (§5.1).
    pub hidden_button: Option<usize>,
    /// The box select's corners (press point, pointer; virtual pixels) while it is drawn.
    pub boxed: Option<[f32; 4]>,
    /// The effects and the weapon table (the live screen only).
    pub fx: Option<(&'a crate::spacewar_fx::Fx, &'a [ig2_sim::spacebattle::WeaponRules])>,
    /// The fighter group the chase camera follows (`aktvadasz`: not drawn), the guns' marker timers (gun `+0x10`).
    pub chased: Option<usize>,
    pub gun_t: Vec<i32>,
    /// The guns' selection timers (grow, fade, attacked) and their buildings' damaged fraction (the live screen).
    pub gun_timers: Vec<[i32; 3]>,
    pub gun_damage: Vec<f32>,
    /// The info model when no ship is selected (`kisinfogfxfrissit`, spec §5.2): the model's name and whether it is
    /// from `talalmanyok` (else `spacewar`): `vad11.xvb` for type 5000, a selected gun's building's for 50000 + slot.
    pub info_other: Option<(String, bool)>,
    /// The small box's title for one unit (its plan's name), the tech under it (a ship's hull, a gun's building; −1
    /// none) and the fighters' stance `gfxvadaszdefendattack`.
    pub title: Vec<u8>,
    pub title_tech: i32,
    pub stance: i32,
    /// The big info box while the info view is on; `MousePointerType`.
    pub big: Option<crate::spacewar_hud::BigInfo>,
    pub pointer: i32,
    /// The formation phase's chosen formation (`+0x3430`).
    pub formation: Option<i32>,
    /// `smkeringeto` (exe 0xF176DC): the console's side lights circle with it (`smMozgatMainFenyek`).
    pub kering: f32,
}

/// The battle camera's state (spec §2.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub pivot: [f32; 3],
    pub yaw: f32,
    pub tilt: f32,
    pub zoom: f32,
    pub restoring: bool,
    pub xcenter: f32,
    /// `starmapeltolas` (exe `+0x1f74`): the battle's origin in the starmap's world (h, x, y).
    pub origin: [f32; 3],
    /// `+0x1f88`: the battle's scene scale (0.0004 during the opening zoom, 1 after it).
    pub scale: f32,
}

impl View {
    /// The eye and target from the fields (`kamerabeallit`'s targets, spec §2.2): `M = CreateMatrix(tilt, −yaw,
    /// π/2)` (f32), eye = `M·(0, 0, zoom) + pivot`.
    pub fn eye_target(&self) -> ([f32; 3], [f32; 3]) {
        let m = crate::starmap::create_matrix(self.tilt, -self.yaw, std::f32::consts::FRAC_PI_2);
        let p = self.pivot;
        (std::array::from_fn(|i| m[i][2] * self.zoom + p[i]), p)
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalise(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 {
        v
    } else {
        v.map(|x| x / l)
    }
}

/// `CameraRotate(eye, target)` (ELF 0x5fcf90): `d = normalise(target − eye)`, `c = normalise(d × (1, 0, 0))`,
/// `b = normalise(d × c)` (`CreateMatrix2v` 0x453690); the view rows are `c`, `−b`, `−d`, so a point ahead has
/// `e.z < 0` and `+y` is down on the screen.
pub fn look_matrix(eye: [f32; 3], target: [f32; 3]) -> [[f32; 3]; 3] {
    let d = normalise(sub(target, eye));
    let c = normalise(cross(d, [1.0, 0.0, 0.0]));
    let b = normalise(cross(d, c));
    [c, b.map(|x| -x), d.map(|x| -x)]
}

/// The look-at camera in the starmap's world: battle point `q` is starmap point `origin + q / 2500` after the
/// opening (spec §2.5); `zbm = |eye − target| / 6` in battle units, the near plane `zbm · 1.05` (`renderscene`).
pub fn starmap_look(v: &View, eye: [f32; 3], target: [f32; 3]) -> Look {
    let k = (v.scale * SCENE_SCALE).max(1e-6);
    let d = sub(eye, target);
    let zbm = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() / 6.0;
    Look { matrix: look_matrix(eye, target), eye: std::array::from_fn(|i| v.origin[i] + eye[i] / k), near: zbm * 1.05 / k, xcenter: v.xcenter }
}

/// A model's triangles in its root space with their textures, and the root's half-extents.
struct Mesh {
    origin: [f32; 3],
    parts: Vec<(Vec<crate::scene::Vert>, crate::render::TexId)>,
}

impl Mesh {
    fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install, vector_dir: &str, name: &str, texture_dir: &str) -> Option<Self> {
        let m = crate::scene::Model::load(install, vector_dir, name, texture_dir).map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
        let parts = m
            .parts
            .into_iter()
            .filter_map(|p| {
                let path = p.texture.as_deref()?;
                let t = crate::assets::Image::load(install, path).ok()?;
                Some((p.verts, r.add_texture(&t)))
            })
            .collect();
        Some(Self { origin: m.origin, parts })
    }
}

/// The ship models (`loadurhajogfx`, spec §1.3): `tipusnevek[race·12 + type]`'s model, from `vector/spacewar/` for
/// types below 8 and `vector/talalmanyok/` for the small craft.
pub struct Models {
    /// By (model name, small-craft directory).
    ships: HashMap<(String, bool), Option<Mesh>>,
}

/// The camera-fixed models (spec §1.1 step 21, §5.1, §5.2): the weapon column's `ikon<i>.xvb` buttons, the reload
/// clock `ora.xvb` (animated: its frame is the reload) and the "off" overlay `no.xvb`.
pub struct Panel {
    icons: Vec<Option<Mesh>>,
    no: Option<Mesh>,
    /// `ora.xvb` per `SetFrame` value (8.8 fixed point).
    clock: HashMap<i32, Option<Mesh>>,
    /// `felallastexture` (`final2x.bmp`): the buttons' first sub-mesh in the formation phase.
    final2x: Option<crate::render::TexId>,
}

impl Panel {
    pub fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install) -> Self {
        let icons = (0..10).map(|i| Mesh::load(r, install, "vector/spacewar/", &format!("ikon{i}.xvb"), "textures/spacewar/")).collect();
        let final2x = crate::assets::Image::load(install, "dataraw/textures/spacewar/final2x_bmp.pvr").map_err(|e| eprintln!("ig2-client: {e}")).ok().map(|t| r.add_texture(&t));
        Self { icons, no: Mesh::load(r, install, "vector/spacewar/", "no.xvb", "textures/spacewar/"), clock: HashMap::new(), final2x }
    }

    fn clock(&mut self, r: &mut crate::render::Renderer, install: &ig2_formats::Install, frame: f32) {
        self.clock.entry(fixed_frame(frame)).or_insert_with(|| Mesh::load_lerp(r, install, "ora.xvb", frame));
    }

    fn clock_at(&self, frame: f32) -> Option<&Mesh> {
        self.clock.get(&fixed_frame(frame)).and_then(Option::as_ref)
    }
}

impl Mesh {
    /// A `vector/spacewar/` model posed at animation frame `frame` (`SetFrame`).
    fn load_at(r: &mut crate::render::Renderer, install: &ig2_formats::Install, name: &str, frame: usize) -> Option<Self> {
        let m = crate::scene::Model::load_at(install, "vector/spacewar/", name, "textures/spacewar/", Some(frame)).ok()?;
        let parts = m.parts.into_iter().filter_map(|p| Some((p.verts, r.add_texture(&crate::assets::Image::load(install, p.texture.as_deref()?).ok()?)))).collect();
        Some(Mesh { origin: m.origin, parts })
    }

    /// A `vector/spacewar/` model at a fractional `SetFrame` value: the two frames around it blended (8.8 fixed point,
    /// as `kirak` 0x60e160 lerps the vertex frames).
    fn load_lerp(r: &mut crate::render::Renderer, install: &ig2_formats::Install, name: &str, frame: f32) -> Option<Self> {
        let at = |f| crate::scene::Model::load_at(install, "vector/spacewar/", name, "textures/spacewar/", Some(f)).ok();
        // `load_at` clamps a frame past the last one, so the key count only needs to be large here.
        let (f0, f1, w) = crate::scene::frame_keys(frame, usize::MAX);
        let mut m = at(f0)?;
        if w > 0.0 {
            m = m.lerp(&at(f1)?, w);
        }
        let parts = m.parts.into_iter().filter_map(|p| Some((p.verts, r.add_texture(&crate::assets::Image::load(install, p.texture.as_deref()?).ok()?)))).collect();
        Some(Mesh { origin: m.origin, parts })
    }

    /// The model's triangles at `xf` in one colour, into the 3D pass's late draws.
    fn push(&self, f: &mut crate::render::Frame, xf: &crate::scene::Transform, color: [f32; 4], additive: bool) {
        self.push_lerp(None, f, xf, color, additive);
    }

    /// [`Mesh::push`] with the vertex positions blended towards `next`'s frame by its weight (`kirak` 0x60e160's 8.8
    /// lerp, `SetFrame` exe 0x452f30), the textures this mesh's.
    fn push_lerp(&self, next: Option<(&Mesh, f32)>, f: &mut crate::render::Frame, xf: &crate::scene::Transform, color: [f32; 4], additive: bool) {
        let crate::render::Frame { scene, late, .. } = f;
        let Some(scene) = scene.as_mut() else { return };
        for (i, (verts, tex)) in self.parts.iter().enumerate() {
            let start = scene.dynamic.len() as u32;
            let to = next.and_then(|(m, w)| m.parts.get(i).filter(|p| p.0.len() == verts.len()).map(|p| (&p.0, w)));
            for (j, v) in verts.iter().enumerate() {
                let pos = match to {
                    Some((q, w)) => std::array::from_fn(|k| v.pos[k] + (q[j].pos[k] - v.pos[k]) * w),
                    None => v.pos,
                };
                scene.dynamic.push(crate::render3d::MeshVertex { pos: xf.point(pos), normal: [0.0; 3], uv: v.uv, color });
            }
            let range = start..scene.dynamic.len() as u32;
            late.push(crate::render3d::MeshDraw { mesh: None, range, tex: *tex, glass: color[3] < 1.0 || additive, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive, depth_write: false });
        }
    }
}

/// `CreateMatrix2(d, 0)` (ELF 0x4534e0; spec §6.7.2): `t = normalise(d × (0, 1, 0))`, `u = normalise(d × t)`, the
/// elements `[−d.x, −u.x, −t.x, −d.y, −u.y, −t.y, −d.z, −u.z, −t.z]`.
fn create_matrix2(d: [f32; 3]) -> crate::scene::M3 {
    let d = crate::scene::normalise(d);
    let t = crate::scene::normalise(cross(d, [0.0, 1.0, 0.0]));
    let u = crate::scene::normalise(cross(d, t));
    [-d[0], -u[0], -t[0], -d[1], -u[1], -t[1], -d[2], -u[2], -t[2]]
}

/// The battle's effect models and textures: the debris (`darab1..3.xvb`), the shield ring `karika.xvb` per frame,
/// the lightning's `villam.bmp` and `flare.bmp`.
pub struct FxGfx {
    debris: [Option<Mesh>; 3],
    rings: HashMap<usize, Option<Mesh>>,
    /// The rings' shared light ([`crate::spacewar_fx::ring_light`]): the battle tick it was taken at, the opacity byte.
    ring_light: Option<(i32, i32)>,
    fighter: Option<Mesh>,
    villam: Option<crate::render::TexId>,
    flare: Option<crate::render::TexId>,
}

impl FxGfx {
    fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install) -> Self {
        let debris = std::array::from_fn(|i| Mesh::load(r, install, "vector/spacewar/", &format!("darab{}.xvb", i + 1), "textures/spacewar/"));
        let tex = |r: &mut crate::render::Renderer, n: &str| crate::assets::Image::load(install, &format!("dataraw/textures/spacewar/{n}_bmp.pvr")).map_err(|e| eprintln!("ig2-client: {e}")).ok().map(|t| r.add_texture(&t));
        let fighter = Mesh::load(r, install, "vector/spacewar/", "vad10.xvb", "textures/spacewar/");
        Self { debris, rings: HashMap::new(), ring_light: None, fighter, villam: tex(r, "villam"), flare: tex(r, "flare") }
    }
}

/// The battle's star field (`initgfx` step 19, spec §1.1): `srand(0x66)`, then per star (1000) a grey `(rand() %
/// 255)/2`, three `rand() % 100 − 50` for (h, x, y), normalised (`spwnormalizevektor`) × 700, and a size `(rand() %
/// 100)/20.0 + 1`; the exe's libc is MSVC's (`seed·214013 + 2531011`, bits 16..30).
pub fn star_field() -> Vec<([f32; 3], f32, f32)> {
    let mut seed: u32 = 0x66;
    let mut rand = || {
        seed = seed.wrapping_mul(214013).wrapping_add(2531011);
        ((seed >> 16) & 0x7fff) as i32
    };
    (0..1000)
        .map(|_| {
            let g = (rand() % 255) / 2;
            let v = [(rand() % 100 - 50) as f32, (rand() % 100 - 50) as f32, (rand() % 100 - 50) as f32];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let p = if l == 0.0 { v } else { v.map(|x| x / l * 700.0) };
            let size = (rand() % 100) as f32 / 20.0 + 1.0;
            (p, g as f32 / 255.0, size)
        })
        .collect()
}

/// The star field (spec §1.1 step 19), a particle set of the sky sphere around the eye: only the camera's turn
/// moves it; each star a sprite of its size at 700 units, its grey additive [inf: `star.bmp`-like soft dot].
fn star_sprites(white: crate::render::TexId, out: &mut Vec<(crate::render::TexId, crate::render::Blend, crate::render::Instance)>, cam: &crate::starmap::Camera, stars: &[([f32; 3], f32, f32)]) {
    use crate::render::{kind, Blend, Instance};
    let Some(look) = cam.look else { return };
    let v = crate::font::Virtual::new(cam.size);
    let m = look.matrix;
    for &(p, g, size) in stars {
        let e: [f32; 3] = std::array::from_fn(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2]);
        if e[2] >= -1.0 {
            continue;
        }
        let k = 600.0 / -e[2];
        let at = [e[0] * k + look.xcenter, e[1] * k + 250.0];
        let half = size * k / 2.0;
        out.push((white, Blend::GammaAdditive, Instance::new(kind::GLOW, v.px(at[0], at[1]), [half * v.k; 2], [g, g, g, 1.0])));
    }
}

/// `gorbe(t)` (ELF 0x6b8a10, exe 0x5839a0): 1 past 1, `2t²` below ½, `1 − 2(1 − t)²` else.
pub fn gorbe(t: f32) -> f32 {
    if t > 1.0 {
        1.0
    } else if (t as f64) < 0.5 {
        2.0 * t * t
    } else {
        1.0 - 2.0 * (1.0 - t) * (1.0 - t)
    }
}

/// A weapon-column slot as `fegyverpanelfrissit` (ELF 0x6a7150, exe 0x5727d0) places it: the slide counter `c`
/// (exe slot `+0x2468 + 0x94·i + 0x28`, 0..50), the group's state `+0x2aa8 + 4i` (3 some mounts off, 6 all off, 7
/// all on, −1 none) and its reload `+0x2a58 + 4i` (0..0xaa).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub counter: i32,
    pub state: i32,
    pub reload: i32,
}

impl Slot {
    pub fn from_slot(obj: &[u8], i: usize) -> Self {
        Self { counter: raw::i32_at(obj, 0x2468 + 0x94 * i + 0x28), state: raw::i32_at(obj, 0x2aa8 + 4 * i), reload: raw::i32_at(obj, 0x2a58 + 4 * i) }
    }

    /// Camera space: `f = (1 − gorbe(1 − c/50))·50`, x 150 → 300, y `42i − 200` (+15 for the retreat button 9), z
    /// −200 → −600; x shifted by `320 − xcenter`.
    pub fn place(&self, i: usize, xcenter: f32) -> [f32; 3] {
        let f = (1.0 - gorbe(1.0 - self.counter as f32 / 50.0)) * 50.0;
        let y = 42.0 * i as f32 - 200.0 + if i == 9 { 15.0 } else { 0.0 };
        [f * 150.0 / 50.0 + 150.0 + 320.0 - xcenter * 639.0 / 639.0, y, -200.0 - f * 400.0 / 50.0]
    }
}

impl Models {
    pub fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install, s: &Shown) -> Self {
        let mut ships = HashMap::new();
        for (i, u) in s.battle.units.iter().enumerate() {
            let Some(Some((name, _))) = s.types.get(i).cloned() else { continue };
            let key = (name, u.kind >= 8);
            if let std::collections::hash_map::Entry::Vacant(e) = ships.entry(key) {
                let m = Self::mesh(r, install, &e.key().0, e.key().1);
                e.insert(m);
            }
        }
        Self { ships }
    }

    /// Load the models of units that came since (`AddUjUrhajo` mid-battle).
    pub fn top_up(&mut self, r: &mut crate::render::Renderer, install: &ig2_formats::Install, s: &Shown) {
        if let Some((name, small)) = &s.info_other {
            if let std::collections::hash_map::Entry::Vacant(e) = self.ships.entry((name.clone(), *small)) {
                e.insert(Self::mesh(r, install, name, *small));
            }
        }
        for (i, u) in s.battle.units.iter().enumerate() {
            let Some(Some((name, _))) = s.types.get(i).cloned() else { continue };
            let key = (name, u.kind >= 8);
            if let std::collections::hash_map::Entry::Vacant(e) = self.ships.entry(key) {
                let m = Self::mesh(r, install, &e.key().0, e.key().1);
                e.insert(m);
            }
        }
    }

    fn mesh(r: &mut crate::render::Renderer, install: &ig2_formats::Install, name: &str, small: bool) -> Option<Mesh> {
        let (vd, td) = if small { ("vector/talalmanyok/", "textures/talalmanyok/") } else { ("vector/spacewar/", "textures/spacewar/") };
        Mesh::load(r, install, vd, name, td)
    }

    fn get(&self, name: &str, small: bool) -> Option<&Mesh> {
        self.ships.get(&(name.to_string(), small)).and_then(|m| m.as_ref())
    }
}

/// The battle's own objects in the starmap's scene (spec §1.1, §3).
struct Scene<'a> {
    view: &'a View,
    assets: &'a crate::starmap::Assets,
    models: &'a Models,
    /// The battle planet's surface (`Bolygok +0x22`), for a planet battle.
    surface: Option<u8>,
    shown: &'a Shown<'a>,
}

/// Grid constants (`InitSpaceWar`, spec §1.5): cells of 3, 128 × 128, bounds ±192, the plane at h = 18.
const CELL: f32 = 3.0;
const CELLS: i32 = 128;
const BOUND: f32 = 192.0;
const PLANE_H: f32 = 18.0;

/// A vertex colour word `A<<24 | 0xBBGGRR`: red in the low byte, `A` a transparency (255 = clear; spec §6.1.1, §1.6), also
/// for the engine's `Lines` (the grid, the drop lines) [obs: frame diff 7.10 against 7.47 with A as the opacity].
fn word_color(w: u32) -> [f32; 4] {
    let c = |s: u32| ((w >> s) & 0xff) as f32 / 255.0;
    [c(0), c(8), c(16), 1.0 - c(24)]
}

/// A camera-fixed model (`SetRelative(false)`): its `AddToScene` scale `size` and rotation `rot` in camera space at
/// `at` (x right, y down, z ahead negative).
struct Fixed<'a> {
    mesh: &'a Mesh,
    at: [f32; 3],
    size: f32,
    rot: (f32, f32, f32),
    opacity: f32,
    /// The first sub-mesh's texture instead of its own (the formation phase's `final2x.bmp`).
    tex0: Option<crate::render::TexId>,
    /// Property bit 8 clear (`PropertiesOff(0xf)`: `ora.xvb`, `no.xvb`): added (`ONE`/`ONE`, RGB × the opacity) over
    /// what is under it (re/screens/render.md §2.3).
    additive: bool,
}

/// Draws camera-fixed models into a texture over the virtual rectangle `rect` (the way the starmap's panel is drawn,
/// `starmap_hud::Console`), to lay over the scene: they sit in front of everything [inf]. Lit by their own ambient
/// light 0x808080 and a front light [inf].
fn fixed_texture(r: &mut crate::render::Renderer, target: &mut Option<crate::render::Target>, items: &[Fixed], rect: [f32; 4], kering: f32, sides: bool, xcenter: f32) -> Option<crate::render::TexId> {
    if items.is_empty() {
        return None;
    }
    let lights = crate::starmap_hud::scene_lights(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], kering);
    // The info model takes the parallel light without the side lights [obs fx8b: its picture 5.63 -> 5.35, ambient
    // only 5.71].
    let lights = if sides { &lights[..] } else { &lights[..1] };
    let mut dynamic = Vec::new();
    let mut draws = Vec::new();
    for it in items {
        let place = crate::scene::Placement::add_to_scene(it.mesh.origin, it.size);
        let rot = crate::scene::create_matrix(it.rot.0, it.rot.1, it.rot.2);
        let model = crate::scene::mul(&rot, &place.matrix);
        for (pi, (verts, tex)) in it.mesh.parts.iter().enumerate() {
            let tex = if pi == 0 { it.tex0.as_ref().unwrap_or(tex) } else { tex };
            let start = dynamic.len() as u32;
            for v in verts {
                let e = crate::scene::apply(&model, v.pos).map(|x| x * 1024.0);
                // A camera-fixed record's lighting normal: its rotation (+0x64) applied to the file normal with z negated,
                // against the lights' own vectors (the chain stops at the record) [obs fx8b: weapons 2.6-4.4; turned by
                // the camera 4.3-8.6].
                let w = crate::scene::normalise(crate::scene::apply(&rot, [v.normal[0], v.normal[1], -v.normal[2]]));
                let c = crate::starmap_hud::engine_light(lights, [w[0], w[1], -w[2]]);
                dynamic.push(crate::render3d::MeshVertex {
                    pos: [e[0] + it.at[0], e[1] + it.at[1], e[2] + it.at[2]],
                    normal: [0.0; 3],
                    uv: v.uv,
                    color: if it.additive { [c[0] * it.opacity, c[1] * it.opacity, c[2] * it.opacity, 1.0] } else { [c[0], c[1], c[2], it.opacity] },
                });
            }
            draws.push(crate::render3d::MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex: *tex, glass: it.opacity < 1.0 || it.additive, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: it.additive, depth_write: false });
        }
    }
    let [x0, y0, x1, y1] = rect;
    let (w, h) = (x1 - x0, y1 - y0);
    // Camera space → clip for the virtual window: vx = 600·x/−z + xcenter, vy = 600·y/−z + 250; depth 0..1 over −z =
    // 100..2000.
    let (a, b) = (-2000.0 / 1900.0, -100.0 * 2000.0 / 1900.0);
    let rows = [
        [1200.0 / w, 0.0, -(2.0 * (xcenter - x0) / w - 1.0), 0.0],
        [0.0, -1200.0 / h, -(1.0 - 2.0 * (250.0 - y0) / h), 0.0],
        [0.0, 0.0, a, b],
        [0.0, 0.0, -1.0, 0.0],
    ];
    let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
    let mut f = crate::render::Frame::new([0.0, 0.0, 0.0, 0.0]);
    f.scene = Some(crate::render3d::Scene3d {
        gamma: true,
        shadow: None,
        uniforms: crate::render3d::SceneUniforms {
            view,
            eye: [0.0; 4],
            light: [0.0, 0.0, 1.0, 0.0],
            ambient: [0.0; 4],
            sun: [0.0; 4],
            fog: [0.0; 4],
            fog_range: [1e9, 2e9, 0.0, 1e9],
            flash: [[0.0; 4]; 2],
        },
        draws,
        dynamic,
    });
    let px = ((w * 4.0) as u32, (h * 4.0) as u32);
    let t = match target {
        Some(t) => t,
        None => target.insert(r.target(px)),
    };
    r.draw_into(&f, t, px, 0.0);
    Some(t.tex)
}

/// A texture of [`fixed_texture`] over its rectangle.
fn put_fixed(f: &mut crate::render::Frame, v: &crate::font::Virtual, tex: Option<crate::render::TexId>, rect: [f32; 4]) {
    let Some(tex) = tex else { return };
    let [x0, y0, x1, y1] = rect;
    f.push(tex, crate::render::Blend::Alpha, crate::render::Instance::new(crate::render::kind::SPRITE, v.px((x0 + x1) / 2.0, (y0 + y1) / 2.0), [(x1 - x0) * v.k / 2.0, (y1 - y0) * v.k / 2.0], [1.0; 4]));
}

/// The weapon column's and the info model's rectangles (virtual).
const COLUMN_RECT: [f32; 4] = [560.0, 20.0, 640.0, 480.0];
const INFO_MODEL_RECT: [f32; 4] = [0.0, 420.0, 60.0, 480.0];

impl<'a> Scene<'a> {
    /// A battle point in the starmap's world: `origin + q/2500` at any scene scale (`rescalescene` scales the battle
    /// and the starmap alike: the camera's units change, not where the battle sits in the starmap).
    fn world(&self, q: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| self.view.origin[i] + q[i] / SCENE_SCALE)
    }

    /// A 3D line in battle units, its colour fading linearly from `ca` to `cb` (drawn in pieces), clipped at the
    /// near plane.
    #[allow(clippy::too_many_arguments)]
    fn line(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, white: crate::render::TexId, a: [f32; 3], b: [f32; 3], ca: [f32; 4], cb: [f32; 4]) {
        let m = cam.matrix();
        let e = |q: [f32; 3]| {
            let w = self.world(q);
            cam.eye(&m, w[0], [w[1], w[2]])
        };
        let (mut ea, mut eb) = (e(a), e(b));
        let (mut ca, mut cb) = (ca, cb);
        let near = -cam.look.map_or(0.4, |l| l.near);
        if ea[2] > near && eb[2] > near {
            return;
        }
        let lerp = |p: [f32; 3], q: [f32; 3], t: f32| -> [f32; 3] { std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t) };
        let lerp4 = |p: [f32; 4], q: [f32; 4], t: f32| -> [f32; 4] { std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t) };
        if ea[2] > near {
            let t = (near - ea[2]) / (eb[2] - ea[2]);
            (ea, ca) = (lerp(ea, eb, t), lerp4(ca, cb, t));
        } else if eb[2] > near {
            let t = (near - eb[2]) / (ea[2] - eb[2]);
            (eb, cb) = (lerp(eb, ea, t), lerp4(cb, ca, t));
        }
        // A one-pixel band in the 3D pass, so the scene's depth hides it (the engine's `Lines` are depth tested): the
        // eye-space segment widened across the view ray by half a screen pixel at each end, back to the world.
        let Some(look) = cam.look else { return };
        let d = [eb[0] - ea[0], eb[1] - ea[1], eb[2] - ea[2]];
        let k = cam.k();
        let across = |e: [f32; 3]| {
            let n = crate::scene::normalise(cross(d, e));
            let hw = 0.5 * -e[2] / (600.0 * k);
            n.map(|x| x * hw)
        };
        let (na, nb) = (across(ea), across(eb));
        let m = look.matrix;
        let world = |e: [f32; 3]| -> [f32; 3] { std::array::from_fn(|j| look.eye[j] + m[0][j] * e[0] + m[1][j] * e[1] + m[2][j] * e[2]) };
        let add = |e: [f32; 3], n: [f32; 3], s: f32| world([e[0] + n[0] * s, e[1] + n[1] * s, e[2] + n[2] * s]);
        let crate::render::Frame { scene, late, .. } = f;
        let Some(scene) = scene.as_mut() else { return };
        let start = scene.dynamic.len() as u32;
        let vert = |pos: [f32; 3], c: [f32; 4]| crate::render3d::MeshVertex { pos, normal: [0.0; 3], uv: [0.5, 0.5], color: c };
        let quad = [vert(add(ea, na, -1.0), ca), vert(add(ea, na, 1.0), ca), vert(add(eb, nb, 1.0), cb), vert(add(eb, nb, -1.0), cb)];
        for k in [0, 1, 2, 0, 2, 3] {
            scene.dynamic.push(quad[k]);
        }
        // Added, not blended: the colour times its opacity onto the scene [obs: s4 t1100, a grid pixel (17, 35, 44) →
        // (17, 42, 59) = + 0x402000 × 0.25; blending would darken the red], writing depth: where the far lines crowd
        // the exe's grid stays one line bright [obs: s4 t720, the grid's far half].
        late.push(crate::render3d::MeshDraw { mesh: None, range: start..scene.dynamic.len() as u32, tex: white, glass: false, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: true, depth_write: false });
    }

    /// The grid (`initracs`, spec §1.5): 128 lines each way at h = 18, each in three pieces (the outer ends clear),
    /// alpha `255 − 63t` (`t = (64 − |i − 64|)/32`, 0xC0 beyond t = 1), RGB 0x402000.
    fn grid(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, white: crate::render::TexId) {
        let mut segs = Vec::new();
        for i in 0..CELLS {
            let t = (64 - (i - 64).abs()) as f32 / 32.0;
            let a = if t <= 1.0 { (255.0 - 63.0 * t) as i32 as u32 } else { 0xc0 };
            let on = word_color(a << 24 | 0x40_2000);
            // The outer ends fade out [obs: s4 t720, the exe's grid fades towards its edge; read as the word
            // 0x00402000 (A = 0, opaque) they would be the brightest part].
            let off = [0.0; 4];
            let c = (i - 64) as f32 * CELL + 1.5;
            for along_y in [true, false] {
                let at = |v: f32| if along_y { [PLANE_H, c, v] } else { [PLANE_H, v, c] };
                let (lo, hi) = (-BOUND, BOUND);
                segs.push((at(lo), at(lo / 2.0), off, on));
                segs.push((at(lo / 2.0), at(hi / 2.0), on, on));
                segs.push((at(hi / 2.0), at(hi), on, off));
            }
        }
        // Nearest first, so a crowded pixel keeps the nearest line (see `line`).
        let m = cam.matrix();
        let depth = |a: [f32; 3], b: [f32; 3]| {
            let w = self.world(std::array::from_fn(|j| (a[j] + b[j]) / 2.0));
            cam.eye(&m, w[0], [w[1], w[2]])[2]
        };
        segs.sort_by(|p, q| depth(q.0, q.1).total_cmp(&depth(p.0, p.1)));
        for (a, b, ca, cb) in segs {
            self.line(f, cam, white, a, b, ca, cb);
        }
    }

    /// The drop lines (`SpaceWarhajovonalkirajzol`, spec §1.6): from each live unit to the grid plane, alpha 200,
    /// green for the viewer's own, 0x0000E0 (red) for its enemies, 0x00C0E0 for the rest; added like the grid.
    fn drop_lines(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, white: crate::render::TexId, b: &Battle) {
        for u in b.units.iter().filter(|u| u.alive && u.owner != 0 && (u.owner == b.human || !u.cloaked)) {
            let rgb = if u.owner == b.human {
                0x00_c000
            } else if b.hostile.get(b.human.max(0) as usize).and_then(|r| r.get(u.owner.max(0) as usize)).copied() == Some(1) {
                // Red for an enemy [obs: s4fx t1100, an enemy's line adds (48, 0, 0) = 0xe0 × (255 − 200)/255].
                0x00_00e0
            } else {
                0x00_c0e0
            };
            let c = word_color(200 << 24 | rgb);
            self.line(f, cam, white, [PLANE_H, u.pos.y, u.pos.z], [u.pos.x, u.pos.y, u.pos.z], c, c);
        }
    }

    /// Unit `i`'s model and pose: the node's matrix (exe unit `+0xe4`, `seturhajopos`) and the `AddToScene` scale
    /// `tipusnevek.scale`, at its position.
    fn pose(&self, i: usize, u: &ig2_sim::spacebattle::Unit) -> Option<(&Mesh, crate::scene::M3, crate::scene::Transform)> {
        let (name, scale) = self.shown.types.get(i).cloned().flatten()?;
        let mesh = self.models.get(&name, u.kind >= 8)?;
        let m: crate::scene::M3 = self.shown.units.get(i).map_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], |g| g.matrix);
        let place = crate::scene::Placement::add_to_scene(mesh.origin, scale / SCENE_SCALE);
        let rs = crate::scene::mul(&m, &place.matrix);
        Some((mesh, m, crate::scene::Transform { matrix: rs.map(|x| x * 1024.0), translation: self.world([u.pos.x, u.pos.y, u.pos.z]) }))
    }

    /// `GetObjCenter`: the centre of the unit's posed model's box (where the overlays sit), in the starmap's world.
    fn centre(&self, i: usize, u: &ig2_sim::spacebattle::Unit) -> Option<[f32; 3]> {
        let (mesh, _, xf) = self.pose(i, u)?;
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in mesh.parts.iter().flat_map(|(vs, _)| vs) {
            let p = xf.point(v.pos);
            for j in 0..3 {
                lo[j] = lo[j].min(p[j]);
                hi[j] = hi[j].max(p[j]);
            }
        }
        (lo[0] <= hi[0]).then(|| std::array::from_fn(|j| (lo[j] + hi[j]) / 2.0))
    }

    /// The ships (`loadurhajogfx`, `seturhajopos`, spec §1.3, §3.1): the type's model at `AddToScene` scale
    /// `tipusnevek.scale`, posed by the node's matrix, lit like the starmap's planets.
    fn ships(&self, f: &mut crate::render::Frame, b: &Battle, eye: &crate::starmap_planet::Eye) {
        let crate::render::Frame { scene, late, .. } = f;
        let Some(scene) = scene.as_mut() else { return };
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let light = crate::scene::normalise(planet_light());
        let _ = eye;
        for (i, u) in b.units.iter().enumerate() {
            // A dead ship stays shown through its explosion's first 0x29 ticks unless it retreated (`seturhajopos`,
            // spec §6.5: ELF `+0x184`, exe `+0x17c`) [obs fx8 t2300: the exe's hull beside the blast].
            let shown = if u.alive { !(u.cloaked && u.owner != b.human) } else { u.owner != 0 && (1..=0x29).contains(&u.exploding) && !u.retreating };
            if !shown {
                continue;
            }
            let Some((mesh, m, xf)) = self.pose(i, u) else { continue };
            let alpha = if u.cloaked { 0.4 } else { 1.0 };
            for (verts, tex) in &mesh.parts {
                let start = scene.dynamic.len() as u32;
                for v in verts {
                    let n = crate::scene::normalise(crate::scene::apply(&m, v.normal));
                    let lit = (0x40 as f32 / 255.0 + 0x80 as f32 / 255.0 * dot(n, light).max(0.0)).min(1.0);
                    scene.dynamic.push(crate::render3d::MeshVertex { pos: xf.point(v.pos), normal: [0.0; 3], uv: v.uv, color: [lit, lit, lit, alpha] });
                }
                let range = start..scene.dynamic.len() as u32;
                late.push(crate::render3d::MeshDraw { mesh: None, range, tex: *tex, glass: alpha < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
            }
        }
    }

    /// `lovedekekkirak`'s pass B (ELF 0x6823c0, exe 0x578d20; spec §6.1): per live unit with its selection timers
    /// (exe unit `+0x184` grow T1, `+0x188` fade F, `+0x18c` attacked T3), the hit-point bar, the corner brackets while
    /// selected, the attack ticks, and the ship count of a selected group; additive, in virtual pixels. `c = 1 −
    /// starmaperosseg` (0 at the battle's zoom). The units' quads are depth tested (`line3(.., depth = true, ..)`, the
    /// vertices at `1 − zbm/−Z`): billboards of the mesh pass at the unit's eye depth; the guns' are not.
    fn overlays(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, hud: &crate::ui::Hud, b: &Battle, erosseg: f32, szar: Option<crate::render::TexId>) {
        use crate::render::{kind, Blend, Instance};
        let v = crate::font::Virtual::new(cam.size);
        let c = 1.0 - erosseg;
        let (ab, ar) = ((255.0 - c * 127.0) as i32 as u32, (255.0 - c * 191.0) as i32 as u32);
        let additive = |w: u32| {
            let rgb = word_color(w);
            let a = (255 - (w >> 24)) as f32 / 256.0;
            [rgb[0] * a, rgb[1] * a, rgb[2] * a, 1.0]
        };
        let (atk, dmg, ok) = (additive(ab << 24 | 0x00_00ff), additive(ab << 24 | 0x40_40ff), additive(ab << 24 | 0x40_ff40));
        let (foe, own) = (additive(ar << 24 | 0x40_40ff), additive(ar << 24 | 0xff_ffff));
        // `line3`'s quads (§6.1.1): screen-aligned rectangles between two corners, textured with `szar.bmp`, u from `u0`
        // to `u1` along x and v 0..1 along y; `swap` (a first vertex's tv ≥ 1) runs u along y and v along x.
        let tex = szar.unwrap_or(self.assets.white);
        let quad = |f: &mut crate::render::Frame, x0: f32, y0: f32, x1: f32, y1: f32, col: [f32; 4], u: (f32, f32), swap: bool| {
            let (lx, hx, ly, hy) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
            if hx > lx && hy > ly {
                let (c, w, h) = (v.px((lx + hx) / 2.0, (ly + hy) / 2.0), (hx - lx) * v.k / 2.0, (hy - ly) * v.k / 2.0);
                let uv = [u.0, 0.0, u.1 - u.0, 1.0];
                let i = if swap { Instance { rot: std::f32::consts::FRAC_PI_2, uv, ..Instance::new(kind::SPRITE, c, [h, w], col) } } else { Instance { uv, ..Instance::new(kind::SPRITE, c, [w, h], col) } };
                f.push(tex, Blend::GammaAdditive, i);
            }
        };
        let m = cam.matrix();
        // `line3`'s one-pixel arms are one screen pixel [obs: s4 t720, the exe's brackets are a pixel thin at 800 × 600].
        let one = 1.0 / v.k;
        // Pass A, the planet guns (spec §6.1 step 3): r 20 px; the bar, the brackets while selected, the attack ticks.
        for (k, gun) in b.guns.iter().enumerate() {
            let Some(&[t1, fd, t3]) = self.shown.gun_timers.get(k) else { continue };
            if gun.building == 0 {
                continue;
            }
            let (t1, fd, t3) = (t1 as f32, fd as f32, t3 as f32);
            let brackets = t1 != 0.0 && fd < 250.0;
            if !brackets && t3 <= 0.0 {
                continue;
            }
            let w = self.world([gun.pos.x, gun.pos.y, gun.pos.z]);
            let e = cam.eye(&m, w[0], [w[1], w[2]]);
            let Some((p, _)) = cam.project_eye(e) else { continue };
            let [x, y] = v.virtual_of(p);
            let r = 20.0f32;
            let (g, fa) = if brackets { ((t1 / 250.0).min(1.0), (250.0 - fd) / 250.0) } else { ((t3 / 250.0).min(1.0), 1.0) };
            let q = self.shown.gun_damage.get(k).copied().unwrap_or(0.0);
            let yt = (y as f64 - (r * fa) as f64 * 1.2 + 0.0 - 1.0) as f32;
            let yb = (y as f64 - ((r * fa) as f64 * 1.2 - (r * fa) as f64 / 5.0) - 1.0) as f32;
            let x0 = x - r * g;
            let xm = x0 + q * r * 2.0 * g;
            quad(f, x0, yt, xm, yb, dmg, (0.0, q), false);
            quad(f, xm, yt, x + r * g, yb, ok, (q, 1.0), false);
            if brackets {
                let col = if b.planet_owner == b.human { own } else { foe };
                let r2 = 9000.0 / t1 - 9.0 + r;
                let la = r2 * (250.0 - fd) / 500.0;
                for (cx, cy, sx, sy) in [(-1.0, -1.0, 1.0, 1.0), (1.0, 1.0, -1.0, -1.0), (1.0, -1.0, -1.0, 1.0), (-1.0, 1.0, 1.0, -1.0)] {
                    let (px0, py0) = (x + cx * r2, y + cy * r2);
                    quad(f, px0, py0, px0 + sx * la, py0 - sy * one, col, (0.5, 0.5), true);
                    quad(f, px0, py0, px0 - sx * one, py0 + sy * la, col, (0.5, 0.5), false);
                }
            }
            if t3 > 0.0 {
                let r3 = 9000.0 / t3 + 20.0 - 9.0;
                let lt = r3 * 250.0 / 500.0;
                quad(f, x + r3, y, x + r3 + lt, y + one, atk, (0.5, 0.5), true);
                quad(f, x - r3, y, x - r3 - lt, y + one, atk, (0.5, 0.5), true);
                quad(f, x, y + r3, x + one, y + r3 + lt, atk, (0.5, 0.5), false);
                quad(f, x, y - r3, x + one, y - r3 - lt, atk, (0.5, 0.5), false);
            }
        }
        let font = hud.font.as_ref();
        let pal = |i: usize| hud.palette.get(i).copied().unwrap_or([1.0; 3]);
        let mut depth: Vec<crate::render3d::MeshVertex> = Vec::new();
        for (i, u) in b.units.iter().enumerate() {
            let Some(g) = self.shown.units.get(i) else { continue };
            if !u.alive {
                continue;
            }
            let (t1, fd, t3) = (g.grow as f32, g.fade as f32, g.attacked as f32);
            let selected = g.selected;
            let brackets = t1 != 0.0 && fd < 250.0;
            if !brackets && t3 <= 0.0 {
                continue;
            }
            let w = self.centre(i, u).unwrap_or_else(|| self.world([u.pos.x, u.pos.y, u.pos.z]));
            let e = cam.eye(&m, w[0], [w[1], w[2]]);
            let Some((p, px)) = cam.project_eye(e) else { continue };
            let [x, y] = v.virtual_of(p);
            // `line3`'s rectangle in the plane through the unit's centre that faces the camera: a virtual pixel there
            // is `−Z/fov` eye units along the camera's rows (x right, y down).
            let unit = -e[2] / crate::starmap::FOV;
            let quad = |_: &mut crate::render::Frame, x0: f32, y0: f32, x1: f32, y1: f32, col: [f32; 4], u: (f32, f32), swap: bool| {
                let (lx, hx, ly, hy) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
                if !(hx > lx && hy > ly) {
                    return;
                }
                // u from `u.0` at x0 to `u.1` at x1 along x, v 0..1 from y0 to y1; `swap`: u along y, v 1..0 along x.
                let corner = |cx: f32, cy: f32| -> crate::render3d::MeshVertex {
                    let (a, bb) = ((cx - x0) / (x1 - x0), (cy - y0) / (y1 - y0));
                    let uv = if swap { [u.0 + bb * (u.1 - u.0), 1.0 - a] } else { [u.0 + a * (u.1 - u.0), bb] };
                    let (dx, dy) = ((cx - x) * unit, (cy - y) * unit);
                    let pos = std::array::from_fn(|j| w[j] + m[0][j] * dx + m[1][j] * dy);
                    crate::render3d::MeshVertex { pos, normal: [0.0; 3], uv, color: col }
                };
                for (cx, cy) in [(lx, ly), (hx, ly), (hx, hy), (lx, ly), (hx, hy), (lx, hy)] {
                    depth.push(corner(cx, cy));
                }
            };
            let mut quad = quad;
            let scale = self.shown.types.get(i).cloned().flatten().map_or(0.0, |t| t.1);
            let rr0 = scale * 1000.0 / SCENE_SCALE * px / v.k;
            let (g, fa) = if brackets { ((t1 / 250.0).min(1.0), (250.0 - fd) / 250.0) } else { ((t3 / 250.0).min(1.0), 1.0) };
            // The bar: `r·f/5` high, `1.2·r·f + 1` above the centre (f64 in the exe), red for the damage, green.
            let q = u.damage as f32 / u.hp as f32;
            let yt = (y as f64 - (rr0 * fa) as f64 * 1.2 - 1.0) as f32;
            let yb = (y as f64 - ((rr0 * fa) as f64 * 1.2 - (rr0 * fa) as f64 / 5.0) - 1.0) as f32;
            let x0 = x - rr0 * g;
            let xm = x0 + rr0 * q * 2.0 * g;
            quad(f, x0, yt, xm, yb, dmg, (0.0, q), false);
            quad(f, xm, yt, x + rr0 * g, yb, ok, (q, 1.0), false);
            if u.shield_used < u.shield {
                let cc = u.shield_used as f32 / u.shield as f32;
                quad(f, x0 + rr0 * cc * 2.0 * g, yt, x + rr0 * g, yb, dmg, (cc, 1.0), false);
            }
            let mut rr = rr0;
            if brackets {
                let col = if u.owner == b.human { own } else { foe };
                let r2 = 9000.0 / t1 - 9.0 + rr0;
                let la = r2 * (250.0 - fd) / 500.0;
                for (cx, cy, sx, sy) in [(-1.0, -1.0, 1.0, 1.0), (1.0, 1.0, -1.0, -1.0), (1.0, -1.0, -1.0, 1.0), (-1.0, 1.0, 1.0, -1.0)] {
                    let (px0, py0) = (x + cx * r2, y + cy * r2);
                    quad(f, px0, py0, px0 + sx * la, py0 - sy * one, col, (0.5, 0.5), true);
                    quad(f, px0, py0, px0 - sx * one, py0 + sy * la, col, (0.5, 0.5), false);
                }
                rr = r2;
            }
            if t3 > 0.0 {
                let r3 = 9000.0 / t3 - 9.0 + rr0;
                let lt = r3 * 250.0 / 500.0;
                quad(f, x + r3, y, x + r3 + lt, y + one, atk, (0.5, 0.5), true);
                quad(f, x - r3, y, x - r3 - lt, y + one, atk, (0.5, 0.5), true);
                quad(f, x, y + r3, x + one, y + r3 + lt, atk, (0.5, 0.5), false);
                quad(f, x, y - r3, x + one, y - r3 - lt, atk, (0.5, 0.5), false);
                rr = r3;
            }
            if let (true, Some(font)) = (selected && u.ship_count > 1, font) {
                font.int(f, &v, u.ship_count as i64, (x + rr - 5.0).trunc(), (y + rr - 7.0).trunc(), 5, 2, crate::font::Align::Left, pal(3));
            }
        }
        let crate::render::Frame { scene, late, .. } = f;
        if let (Some(scene), false) = (scene.as_mut(), depth.is_empty()) {
            let start = scene.dynamic.len() as u32;
            scene.dynamic.extend(depth);
            let range = start..scene.dynamic.len() as u32;
            late.push(crate::render3d::MeshDraw { mesh: None, range, tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: true, depth_write: false });
        }
    }

    /// The planet guns' markers (`agyukirak` ELF 0x6818c0 / exe 0x571480, spec §6.2; the outer shell's callback, so
    /// planet battles only): per gun in front of the camera with a building and a timer T, corner brackets of 10 px at
    /// `r = 9000/T + 2 − 9` (a 12-gon of radius 10 above armour 1), white round the human's own guns, red otherwise,
    /// alpha `255 − 191·(1 − e)`; added, untextured.
    fn gun_markers(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, b: &Battle, erosseg: f32) {
        use crate::render::{kind, Blend, Instance};
        if b.planet_owner < 0 && b.planet <= 0 {
            return;
        }
        let v = crate::font::Virtual::new(cam.size);
        let a = (255.0f32 - 191.0 * (1.0 - erosseg)) as i32 as u32;
        let rgb = if b.planet_owner == b.human { 0x00ff_ffff } else { 0x0000_00ff };
        let w = word_color(a << 24 | rgb);
        let t = (255 - a) as f32 / 256.0;
        let col = [w[0] * t, w[1] * t, w[2] * t, 1.0];
        let m = cam.matrix();
        let near = -cam.look.map_or(0.4, |l| l.near);
        let one = 1.0 / v.k;
        let quad = |f: &mut crate::render::Frame, x0: f32, y0: f32, x1: f32, y1: f32| {
            let (lx, hx, ly, hy) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
            let c = v.px((lx + hx) / 2.0, (ly + hy) / 2.0);
            f.push(self.assets.white, Blend::GammaAdditive, Instance::new(kind::SPRITE, c, [((hx - lx) * v.k / 2.0).max(0.5), ((hy - ly) * v.k / 2.0).max(0.5)], col));
        };
        for (k, g) in b.guns.iter().enumerate() {
            let tt = self.shown.gun_t.get(k).copied().unwrap_or(0);
            if g.building == 0 || tt == 0 {
                continue;
            }
            let wpos = self.world([g.pos.x, g.pos.y, g.pos.z]);
            let e = cam.eye(&m, wpos[0], [wpos[1], wpos[2]]);
            if e[2] >= near {
                continue;
            }
            let Some((p, _)) = cam.project_eye(e) else { continue };
            let [x, y] = v.virtual_of(p);
            if g.armour <= 1.0 {
                let r = (9000.0f32 / tt as f32 + 2.0) - 9.0;
                for (sx, sy) in [(1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                    let (cx, cy) = (x + sx * r, y + sy * r);
                    quad(f, cx, cy, cx + sx * 10.0, cy - sy * one);
                    quad(f, cx, cy, cx - sx * one, cy + sy * 10.0);
                }
            } else {
                // `kor(&(X, Y, Z), 10, 12, colour)`: a 12-gon of radius 10.
                let c = v.px(x, y);
                for i in 0..12 {
                    let (a0, a1) = (i as f32 * std::f32::consts::TAU / 12.0, (i + 1) as f32 * std::f32::consts::TAU / 12.0);
                    let p0 = [c[0] + a0.cos() * 10.0 * v.k, c[1] + a0.sin() * 10.0 * v.k];
                    let p1 = [c[0] + a1.cos() * 10.0 * v.k, c[1] + a1.sin() * 10.0 * v.k];
                    f.push(self.assets.white, Blend::GammaAdditive, Instance::line(p0, p1, 1.0, col));
                }
            }
        }
    }

    /// The fighters (`SpaceVadaszKirajzol` ELF 0x694e50 / exe 0x56d460, spec §3.2, §7.4): one `vad10.xvb` sprite per live
    /// group (but the chased one) at size 1, turned by `CreateMatrix2(dir, 0)`, in the particles' model mode; a point
    /// when 2 pixels or smaller.
    fn fighters(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, b: &Battle, mesh: Option<&Mesh>) {
        use crate::render::{kind, Blend, Instance};
        let m = cam.matrix();
        let near = -cam.look.map_or(0.4, |l| l.near);
        for (j, g) in b.fighters.iter().enumerate() {
            if !g.alive || self.shown.chased == Some(j) {
                continue;
            }
            let pos = [g.pos.x, g.pos.y, g.pos.z];
            let w = self.world(pos);
            let e = cam.eye(&m, w[0], [w[1], w[2]]);
            if e[2] >= near {
                continue;
            }
            let Some((c, px)) = cam.project_eye(e) else { continue };
            // Size 1 in the model mode; the exe's fighters at the default view are 1–2 px points of the set's word
            // 0x2c2c2c [obs: fx5 t1000, where a size-1 model would be 8 px wide]: the model at a quarter of it (a
            // calibration).
            let size = 0.25;
            let grey = 0x2c as f32 / 255.0;
            let (Some(mesh), true) = (mesh, size / SCENE_SCALE * px > 2.0) else {
                f.push(self.assets.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, c, [0.75; 2], [grey, grey, grey, 1.0]));
                continue;
            };
            let place = crate::scene::Placement::add_to_scene(mesh.origin, size / SCENE_SCALE);
            let xf = crate::scene::Transform { matrix: crate::scene::mul(&create_matrix2([g.dir.x, g.dir.y, g.dir.z]), &place.matrix), translation: w };
            let lit = (0x40 + 0x80) as f32 / 255.0 * 0.75;
            mesh.push(f, &xf, [lit, lit, lit, 1.0], false);
        }
    }

    /// The effect particles (`Particle` ELF 0x5f4ff0, textured mode; spec §6.7.2): screen-aligned squares of half-size
    /// `size·fov/−z`, the colour word's RGB × (255 − T)/256 added; the 25-frame sets step through their 5 × 5 sheet. Depth
    /// tested without a depth write (`ZENABLE`, no `ZWRITEENABLE`, `BLEND(1, 1)`): billboards of the mesh pass on the
    /// camera's axes, behind the hulls where the exe hides them.
    fn particles(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, fx: &crate::spacewar_fx::Fx, textures: &[Option<crate::render::TexId>; 5]) {
        let m = cam.matrix();
        let near = -cam.look.map_or(0.4, |l| l.near);
        let crate::render::Frame { scene, late, .. } = f;
        let Some(scene) = scene.as_mut() else { return };
        for (set, ps) in fx.sets.iter().enumerate() {
            let tex = textures[set].unwrap_or(self.assets.white);
            let frames = crate::spacewar_fx::SET_FRAMES[set];
            let start = scene.dynamic.len() as u32;
            for p in ps.iter().filter(|p| p.color != 0 && p.size > 0.0) {
                let w = self.world(p.pos);
                let e = cam.eye(&m, w[0], [w[1], w[2]]);
                if e[2] >= near {
                    continue;
                }
                let half = p.size / SCENE_SCALE;
                let uv = if frames > 0 {
                    let side = (frames as f32).sqrt() as i32;
                    let k = (p.frame as i32).clamp(0, frames as i32 - 1);
                    let d = 1.0 / side as f32;
                    [(k % side) as f32 * d, (k / side) as f32 * d, d, d]
                } else {
                    [0.0, 0.0, 1.0, 1.0]
                };
                let color = crate::starmap::particle_color(p.color);
                // The eye's x and y (down) axes in the world: the camera's rows.
                let corner = |sx: f32, sy: f32| -> crate::render3d::MeshVertex {
                    let pos = std::array::from_fn(|i| w[i] + (m[0][i] * sx + m[1][i] * sy) * half);
                    let u = [uv[0] + (sx + 1.0) / 2.0 * uv[2], uv[1] + (sy + 1.0) / 2.0 * uv[3]];
                    crate::render3d::MeshVertex { pos, normal: [0.0; 3], uv: u, color }
                };
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    scene.dynamic.push(corner(sx, sy));
                }
            }
            let range = start..scene.dynamic.len() as u32;
            if !range.is_empty() {
                late.push(crate::render3d::MeshDraw { mesh: None, range, tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: true, depth_write: false });
            }
        }
    }

    /// The lightning (`villamfrissit`, `villam::Update`, `kiralyvonalhuzo`; spec §6.8.4): each bolt's Hermite curve as
    /// a ribbon of half-width `W·fov/−z` px, `villam.bmp` scrolling along it, added; the flare a `flare.bmp` square.
    fn bolts(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, fx: &crate::spacewar_fx::Fx, g: &FxGfx) {
        use crate::render::{kind, Blend, Instance};
        let m = cam.matrix();
        let near = -cam.look.map_or(0.4, |l| l.near);
        let project = |q: [f32; 3]| {
            let w = self.world(q);
            let e = cam.eye(&m, w[0], [w[1], w[2]]);
            if e[2] >= near {
                return None;
            }
            cam.project_eye(e)
        };
        let all = fx.shot_bolts.values().flatten().chain(fx.unit_bolts.values()).chain(fx.fighter_bolts.values());
        for bolt in all {
            let pts = bolt.samples();
            let n = pts.len().saturating_sub(1).max(1) as f32;
            let tex = g.villam.unwrap_or(self.assets.white);
            for (i, w) in pts.windows(2).enumerate() {
                let (Some((a, pa)), Some((c, pc))) = (project(w[0].0), project(w[1].0)) else { continue };
                let (dx, dy) = (c[0] - a[0], c[1] - a[1]);
                let len = (dx * dx + dy * dy).sqrt();
                if len <= 0.0 {
                    continue;
                }
                let hw = bolt.width / SCENE_SCALE * (pa + pc) / 2.0;
                let v0 = i as f32 / n * bolt.texscale + bolt.tex;
                let v1 = (i + 1) as f32 / n * bolt.texscale + bolt.tex;
                let mid = [(a[0] + c[0]) / 2.0, (a[1] + c[1]) / 2.0];
                let col = crate::starmap::particle_color(w[0].1);
                f.push(tex, Blend::GammaAdditive, Instance { rot: (-dx).atan2(dy), uv: [0.0, v0, 1.0, v1 - v0], ..Instance::new(kind::SPRITE, mid, [hw, len / 2.0], col) });
            }
            if let (true, Some(&(q, _))) = (bolt.flare, bolt.cur.last())
                && let Some((c, px)) = project(q)
            {
                let half = bolt.flare_size / SCENE_SCALE * px;
                f.push(g.flare.unwrap_or(self.assets.white), Blend::GammaAdditive, Instance::new(kind::SPRITE, c, [half; 2], crate::starmap::particle_color(0x80ff_ffff)));
            }
        }
    }

    /// The shield rings (`karikafrissit`, spec §6.8.5): `karika.xvb` at 0.05, `CreateMatrix2(dir, 0)`, at the fractional
    /// frame `f` (`SetFrame`'s 8.8 lerp, each ring its own: `kirak` exe 0x531460 lerps per draw), unshaded,
    /// transparency `f/30`; `PropertiesOff(0xc)` (exe 0x576e40): added (`ONE`/`ONE`) with no depth write, the white
    /// vertices scaled by the shared opacity byte `>> 8` ([`crate::spacewar_fx::ring_light`]), the texture
    /// (`kekring`, no alpha) added as it is.
    fn rings(&self, f: &mut crate::render::Frame, r: &mut crate::render::Renderer, install: &ig2_formats::Install, b: &Battle, fx: &crate::spacewar_fx::Fx, g: &mut FxGfx) {
        if b.planet_owner < 0 {
            return;
        }
        if g.ring_light.is_none_or(|(t, _)| t != b.tick) {
            if let Some(o) = crate::spacewar_fx::ring_light(&fx.rings, b.tick) {
                g.ring_light = Some((b.tick, o));
            }
        }
        let o = g.ring_light.map_or(255, |x| x.1);
        for ring in fx.rings.iter().filter(|x| x.start != 0) {
            let fr = ring.frame(b.tick);
            if fr > 30.0 {
                continue;
            }
            let (f0, f1, w) = crate::scene::frame_keys(fr, usize::MAX);
            for k in [f0, f1] {
                g.rings.entry(k).or_insert_with(|| Mesh::load_at(r, install, "karika.xvb", k));
            }
            let Some(mesh) = g.rings[&f0].as_ref() else { continue };
            let next = g.rings[&f1].as_ref().filter(|_| w > 0.0).map(|m| (m, w));
            // The exe's ring records hold `diag(0.05)` in record space (battle / 1024): the model's box is ±1024, so
            // its vertex frames are already normalised (±0.007 at frame 0 growing to ±0.97 at 30) [obs 2026-10-25
            // shield capture, `--records` t3000: `s` 0.05 on every axis, `at` = the hit point / 1024].
            let s = 0.05 * 1024.0 / SCENE_SCALE;
            let xf = crate::scene::Transform { matrix: crate::scene::mul(&create_matrix2(ring.dir), &crate::scene::scale([s; 3])), translation: self.world(ring.at) };
            mesh.push_lerp(next, f, &xf, [1.0, 1.0, 1.0, o as f32 / 256.0], true);
        }
    }

    /// The debris (`Darabok`, spec §6.8.6): each shown piece's model at its size, turned by its spin, lit like the ships.
    fn debris(&self, f: &mut crate::render::Frame, fx: &crate::spacewar_fx::Fx, g: &FxGfx) {
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let light = crate::scene::normalise(planet_light());
        for d in fx.debris.iter().filter(|d| d.counter != 0) {
            let Some(mesh) = g.debris[d.model.min(2)].as_ref() else { continue };
            // `ScaleObja(size)` (ELF 0x457140) overwrites the record's scale matrix `+0x10` with `diag(size)`, dropping
            // `AddToScene`'s `diag(ext·s/1024)`; `BoxUjraszamol` (ELF 0x602750) left the vertices normalised per axis by
            // the box's extent, so each axis of the piece spans ±size: the flat plates become blocks. Same ×1024 as
            // `pose` [obs fx8: the exe's records hold diag(size), its darab boxes (851, 210, 1024) etc.].
            let s = d.size / SCENE_SCALE;
            let r = mesh_reach(mesh);
            let xf = crate::scene::Transform { matrix: crate::scene::mul(&d.turn, &crate::scene::scale(std::array::from_fn(|k| s / r[k]))).map(|x| x * 1024.0), translation: self.world(d.pos) };
            // Lit per vertex like the ships (ambient 0x40, the planet's parallel light 0x80), opaque until counter 30;
            // over the blasts' sprites, after them in the translucent pass [obs fx8 t2100: the exe's pieces stay dark in
            // front of the white core].
            let crate::render::Frame { scene, late, .. } = &mut *f;
            let Some(scene) = scene.as_mut() else { return };
            let alpha = 1.0 - d.transparency();
            for (verts, tex) in &mesh.parts {
                let start = scene.dynamic.len() as u32;
                for v in verts {
                    let n = crate::scene::normalise(crate::scene::apply(&d.turn, v.normal));
                    let lit = (0x40 as f32 / 255.0 + 0x80 as f32 / 255.0 * dot(n, light).max(0.0)).min(1.0);
                    scene.dynamic.push(crate::render3d::MeshVertex { pos: xf.point(v.pos), normal: [0.0; 3], uv: v.uv, color: [lit, lit, lit, alpha] });
                }
                let range = start..scene.dynamic.len() as u32;
                late.push(crate::render3d::MeshDraw { mesh: None, range, tex: *tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
            }
        }
    }

    /// The projectiles (`SpaceWarLovedekKirajzol` and `Particle`'s model mode, spec §6.3.1, §6.7.2): each live shot as its
    /// form's model turned by `CreateMatrix2(dir, 0)` at the shot's size, added with opacity `(255 − A)/256`; a point
    /// when it is 2 pixels or smaller.
    fn shots(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, b: &Battle, fx: &crate::spacewar_fx::Fx, weapons: &[ig2_sim::spacebattle::WeaponRules], meshes: &[Option<Mesh>]) {
        use crate::render::{kind, Blend, Instance};
        let m = cam.matrix();
        let near = -cam.look.map_or(0.4, |l| l.near);
        let forms = &fx.tables.forms;
        for (fi, pool) in b.pools.iter().enumerate() {
            let Some(form) = forms.get(fi) else { continue };
            for s in pool.shots.iter().filter(|s| s.shooter.is_some()) {
                let (word, size) = crate::spacewar_fx::shot_look(s, weapons, forms, form.size, b.tick);
                if size <= 0.0 {
                    continue;
                }
                let pos = [s.pos.x, s.pos.y, s.pos.z];
                let w = self.world(pos);
                let e = cam.eye(&m, w[0], [w[1], w[2]]);
                if e[2] >= near {
                    continue;
                }
                let Some((c, px)) = cam.project_eye(e) else { continue };
                let color = crate::starmap::particle_color(word);
                let mesh = meshes.get(fi).and_then(|x| x.as_ref());
                let (Some(mesh), true) = (mesh, size / SCENE_SCALE * px > 2.0) else {
                    f.push(self.assets.white, Blend::GammaAdditive, Instance::new(kind::SPRITE, c, [0.5; 2], color));
                    continue;
                };
                // The spinning sprites: the torpedo form 7 and weapon 11 (f64 sin/cos).
                let mut d = [s.dir.x, s.dir.y, s.dir.z];
                let spin = if fi == 7 { Some(3.0) } else if s.weapon == 11 { Some(10.0) } else { None };
                if let Some(k) = spin {
                    let a = s.age as f64 / k;
                    let (x, y) = (a.sin() as f32, a.cos() as f32);
                    let l = (x * x + y * y).sqrt();
                    (d[0], d[1]) = (x / l, y / l);
                }
                let d = crate::scene::normalise(d);
                let t = crate::scene::normalise(cross(d, [0.0, 1.0, 0.0]));
                let u = crate::scene::normalise(cross(d, t));
                let rot: crate::scene::M3 = [-d[0], -u[0], -t[0], -d[1], -u[1], -t[1], -d[2], -u[2], -t[2]];
                let place = crate::scene::Placement::add_to_scene(mesh.origin, size / SCENE_SCALE);
                let xf = crate::scene::Transform { matrix: crate::scene::mul(&rot, &place.matrix), translation: w };
                let a = (255 - (word >> 24)) as f32 / 256.0;
                let crate::render::Frame { scene, late, .. } = f;
                let Some(scene) = scene.as_mut() else { continue };
                for (verts, tex) in &mesh.parts {
                    let start = scene.dynamic.len() as u32;
                    for v in verts {
                        scene.dynamic.push(crate::render3d::MeshVertex { pos: xf.point(v.pos), normal: [0.0; 3], uv: v.uv, color: [1.0, 1.0, 1.0, a] });
                    }
                    let range = start..scene.dynamic.len() as u32;
                    late.push(crate::render3d::MeshDraw { mesh: None, range, tex: *tex, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: true, depth_write: false });
                }
            }
        }
    }

    /// The weapon column (spec §5.1): per shown slot (`c` ≠ 0) the button at 0.02 turned (π/2, 0, 0), the reload
    /// clock and the overlay at 0.014 (not on the retreat button 9); the overlay is 0.75 opaque when some of the
    /// group's mounts are off, 0.5 when all are, hidden when all are on. Lit by its own ambient light 0x808080 [inf].
    fn weapons<'p>(&self, items: &mut Vec<Fixed<'p>>, panel: &'p Panel) {
        let half = std::f32::consts::FRAC_PI_2;
        for i in 0..10 {
            let sl = self.shown.slots[i];
            if sl.counter == 0 {
                continue;
            }
            let at = sl.place(i, self.view.xcenter);
            if self.shown.hidden_button == Some(i) {
                continue;
            }
            let tex0 = self.shown.formation.and(panel.final2x);
            if let Some(Some(m)) = panel.icons.get(i) {
                items.push(Fixed { mesh: m, at, size: 0.02, rot: (half, 0.0, 0.0), opacity: 1.0, tex0, additive: false });
            }
            // The formation phase (`fegyverpanelfrissit`'s other branch): every button's clock at frame 0 on the
            // chosen formation (`+0x3430`), 32 on the others; no overlay.
            if let Some(pick) = self.shown.formation {
                let fr = if pick == i as i32 { 0.0 } else { 32.0 };
                if let Some(m) = panel.clock_at(fr) {
                    items.push(Fixed { mesh: m, at, size: 0.014, rot: (0.0, 0.0, 0.0), opacity: 1.0, tex0: None, additive: true });
                }
                continue;
            }
            if i == 9 {
                continue;
            }
            // The clock shows only while the group reloads [obs: none at a full 0xaa].
            if let (true, Some(m)) = (sl.reload < 0xaa, panel.clock_at(clock_frame(sl.reload))) {
                items.push(Fixed { mesh: m, at, size: 0.014, rot: (0.0, 0.0, 0.0), opacity: 1.0, tex0: None, additive: true });
            }
            let overlay = match sl.state {
                3 => Some(0.75),
                6 => Some(0.5),
                _ => None,
            };
            if let (Some(a), Some(m)) = (overlay, &panel.no) {
                items.push(Fixed { mesh: m, at, size: 0.014, rot: (0.0, 0.0, 0.0), opacity: a, tex0: None, additive: true });
            }
        }
    }

    /// The info box's model (`kisinfogfxfrissit` ELF 0x6860c0, exe 0x5778c0; spec §5.2): the selection's highest
    /// `race·12 + type` (`selectkisinfourhajo`) at 0.03, turned π/2 about x, at (−545, 365, −1100), spinning 0.05 a
    /// frame (the phase isn't dumped: `spin`); with no ship selected the fighter's or a gun's building's model.
    fn info_model(&self, items: &mut Vec<Fixed<'a>>, b: &Battle, spin: f32, clear: f32) {
        if let Some(m) = self.shown.info_other.as_ref().and_then(|(name, small)| self.models.get(name, *small)) {
            items.push(Fixed { mesh: m, at: [-545.0, 365.0, -1100.0], size: 0.03, rot: (std::f32::consts::FRAC_PI_2, spin, 0.0), opacity: 1.0 - clear, tex0: None, additive: false });
            return;
        }
        let mut best: Option<(i32, usize)> = None;
        for (i, u) in b.units.iter().enumerate() {
            let sel = self.shown.units.get(i).is_some_and(|g| g.selected);
            if u.alive && sel && best.is_none_or(|(t, _)| u.race * 12 + u.kind > t) {
                best = Some((u.race * 12 + u.kind, i));
            }
        }
        let Some((_, i)) = best else { return };
        let Some((name, _)) = self.shown.types.get(i).cloned().flatten() else { return };
        if let Some(m) = self.models.get(&name, b.units[i].kind >= 8) {
            items.push(Fixed { mesh: m, at: [-545.0, 365.0, -1100.0], size: 0.03, rot: (std::f32::consts::FRAC_PI_2, spin, 0.0), opacity: 1.0 - clear, tex0: None, additive: false });
        }
    }

    fn draw(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, b: &Battle, eye: &crate::starmap_planet::Eye) {
        // The planet of a planet battle (step 7): `b{(type − 1) % 8}2.xvb` at R·1.16, `RotateObj(π/2, π/2, 0)`, at P.
        if let (Some(surface), true) = (self.surface, b.planet_owner >= 0 || b.planet > 0) {
            let r = b.planet_size as f32;
            let p = b.planet_pos.map(|x| x as f32);
            let half = std::f32::consts::FRAC_PI_2;
            let crate::render::Frame { scene, late, .. } = f;
            if let Some(scene) = scene.as_mut() {
                self.assets.planet_meshes.add_battle_planet(scene, late, surface, r * 1.16 / SCENE_SCALE, (half, half, 0.0), self.world(p), eye);
                // The cloud shells (`bolygo3.xvb` at R·1.02 and R·1.04 in f64, `RotateObj(0, π/2, 0)`), which `frissit2`
                // (exe 0x573bff, 0x573c36) turns each battle step to `(tick/900, 0, 0)` and `(tick/−1200, π/2, 0)`.
                let t = b.tick as f32;
                for (k, rot) in [(1.02f64, (t / 900.0, 0.0, 0.0)), (1.04, (t / -1200.0, half, 0.0))] {
                    self.assets.planet_meshes.add_battle_shell(scene, late, (b.planet_size as f64 * k) as f32 / SCENE_SCALE, rot, self.world(p), eye);
                }
            }
        }
        self.ships(f, b, eye);
        self.grid(f, cam, self.assets.white);
        self.drop_lines(f, cam, self.assets.white, b);
    }
}

/// The starmap scene's parallel light (`Init3dStarmap`: 0x808080 along (10, 10, −10), the direction it shines in):
/// towards it is (−10, −10, 10) [obs: of the eight sign choices the capture matches this one, mean diff 7.3 vs ≥ 17.6].
/// A model's half-extent per axis from its pivot (`BoxUjraszamol`'s `ext / 1024` for a root 1024 wide), at least 1e-6.
fn mesh_reach(mesh: &Mesh) -> [f32; 3] {
    let mut r = [1e-6f32; 3];
    for v in mesh.parts.iter().flat_map(|(vs, _)| vs) {
        for (r, p) in r.iter_mut().zip(v.pos) {
            *r = r.max(p.abs());
        }
    }
    r
}

fn planet_light() -> [f32; 3] {
    [-10.0, -10.0, 10.0]
}

/// `ora.xvb`'s frame for a reload `0..0xaa`: `SetFrame((reload/10)·1.65)` (int division; the fraction blends the
/// next vertex frame).
fn clock_frame(reload: i32) -> f32 {
    (reload / 10) as f32 * 1.65
}

/// `SetFrame`'s 8.8 fixed-point value of a frame.
fn fixed_frame(frame: f32) -> i32 {
    let (f0, _, w) = crate::scene::frame_keys(frame, usize::MAX);
    (f0 as i32) * 256 + (w * 256.0) as i32
}

/// The battle screen's own models: the ships, the weapon column, the control panel under the ambient light, the star
/// field.
pub struct Gfx {
    models: Models,
    panel: Panel,
    console: Option<crate::starmap_hud::Console>,
    stars: Vec<([f32; 3], f32, f32)>,
    /// The effect sets' textures (`textures/spacewar/<set>.bmp`) and the projectile forms' models (`Lovedekload`).
    fx_textures: [Option<crate::render::TexId>; 5],
    forms: Vec<Option<Mesh>>,
    /// `szar.bmp` (`+0x1890`): the overlays' texture.
    szar: Option<crate::render::TexId>,
    fx: FxGfx,
    /// `MOUSE.BMP` (`dataraw/screen/mouse_bmp.pvr`) and its size.
    pub cursor: Option<(crate::render::TexId, [f32; 2])>,
    /// The weapon column's and the small info box's textures ([`fixed_texture`]), drawn into again each frame.
    column_target: Option<crate::render::Target>,
    info_target: Option<crate::render::Target>,
}

impl Gfx {
    pub fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install, s: &Shown) -> Self {
        // The panel under the ambient light alone [obs: console diff 11.4 and 10.1 in two battles against 19.4 and
        // 16.3 with the starmap's lights; the exe's panel changes colour with the side lights `smMozgatMainFenyek`
        // circles].
        let console = crate::starmap_hud::Console::load_sides(r, install, s.kering, &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).map_err(|e| eprintln!("ig2-client: kezel.xvb: {e}")).ok();
        let fx_textures = std::array::from_fn(|i| {
            let path = format!("dataraw/textures/spacewar/{}_bmp.pvr", crate::spacewar_fx::SET_TEXTURES[i]);
            crate::assets::Image::load(install, &path).map_err(|e| eprintln!("ig2-client: {e}")).ok().map(|t| r.add_texture(&t))
        });
        let forms = s.fx.map_or_else(Vec::new, |(fx, _)| fx.tables.forms.iter().map(|f| if f.name.is_empty() { None } else { Mesh::load(r, install, "vector/spacewar/", &f.name, "textures/spacewar/") }).collect());
        let szar = crate::assets::Image::load(install, "dataraw/textures/spacewar/szar_bmp.pvr").map_err(|e| eprintln!("ig2-client: {e}")).ok().map(|t| r.add_texture(&t));
        Self { models: Models::load(r, install, s), panel: Panel::load(r, install), console, stars: star_field(), fx_textures, forms, szar, fx: FxGfx::load(r, install), cursor: crate::assets::Image::load(install, "dataraw/screen/mouse_bmp.pvr").map_err(|e| eprintln!("ig2-client: {e}")).ok().map(|t| (r.add_texture(&t), [t.width as f32, t.height as f32])), column_target: None, info_target: None }
    }
}

/// The battle's camera on the starmap's view and its star field, before the view is built (spec §1.0, §9).
pub fn prepare(view: &mut crate::starmap::View, gfx: &Gfx, white: crate::render::TexId, s: &Shown) {
    view.camera.look = Some(starmap_look(&s.view, s.eye, s.target));
    view.backdrop.clear();
    star_sprites(white, &mut view.backdrop, &view.camera, &gfx.stars);
}

/// Everything the battle adds to the starmap's frame: its scene, the overlays, the weapon column, the panel, the radar
/// and the small info box (without the top bar).
#[allow(clippy::too_many_arguments)]
pub fn draw(f: &mut crate::render::Frame, r: &mut crate::render::Renderer, install: &ig2_formats::Install, gfx: &mut Gfx, assets: &crate::starmap::Assets, cam: &crate::starmap::Camera, hud: &crate::ui::Hud, s: &Shown, surface: Option<u8>, size: [f32; 2]) {
    gfx.models.top_up(r, install, s);
    let scene = Scene { view: &s.view, assets, models: &gfx.models, surface, shown: s };
    scene.draw(f, cam, s.battle, &cam.eye_world());
    scene.fighters(f, cam, s.battle, gfx.fx.fighter.as_ref());
    scene.gun_markers(f, cam, s.battle, s.erosseg);
    if let Some((fx, weapons)) = s.fx {
        scene.rings(f, r, install, s.battle, fx, &mut gfx.fx);
        scene.shots(f, cam, s.battle, fx, weapons, &gfx.forms);
        scene.particles(f, cam, fx, &gfx.fx_textures);
        scene.debris(f, fx, &gfx.fx);
        scene.bolts(f, cam, fx, &gfx.fx);
    }
    scene.overlays(f, cam, hud, s.battle, s.erosseg, gfx.szar);
    for sl in &s.slots[..9] {
        gfx.panel.clock(r, install, clock_frame(sl.reload));
    }
    if s.formation.is_some() {
        gfx.panel.clock(r, install, 0.0);
        gfx.panel.clock(r, install, 32.0);
    }
    let virt = crate::font::Virtual::new(size);
    // The column's projection follows `xcenter` (its place takes `320 − xcenter` back), the info model's stays at 320
    // [obs fx7 t1000..t1245 with the big box open: weapons 5.0-8.6 -> 2.8-6.0; the info model then left its box].
    let mut column = Vec::new();
    scene.weapons(&mut column, &gfx.panel);
    put_fixed(f, &virt, fixed_texture(r, &mut gfx.column_target, &column, COLUMN_RECT, s.kering, true, s.view.xcenter), COLUMN_RECT);
    let mut info = Vec::new();
    scene.info_model(&mut info, s.battle, s.spin, 1.0 - s.fade);
    let info_tex = fixed_texture(r, &mut gfx.info_target, &info, INFO_MODEL_RECT, s.kering, false, 320.0);
    if let Some(c) = gfx.console.as_mut() {
        c.relight(r, s.kering, &cam.matrix());
    }
    crate::spacewar_hud::draw(f, hud, gfx.console.as_ref(), &virt, s);
    put_fixed(f, &virt, info_tex, INFO_MODEL_RECT);
}

/// A battle point on the virtual screen and the projection's pixels per battle unit there.
fn to_virtual(cam: &crate::starmap::Camera, view: &View, q: [f32; 3]) -> Option<([f32; 2], f32)> {
    let w: [f32; 3] = std::array::from_fn(|i| view.origin[i] + q[i] / SCENE_SCALE);
    let e = cam.eye(&cam.matrix(), w[0], [w[1], w[2]]);
    let (p, px) = cam.project_eye(e)?;
    let v = crate::font::Virtual::new(cam.size);
    Some((v.virtual_of(p), px / v.k / SCENE_SCALE))
}

/// `SpaceWarMouseMove`'s projections (spec §7.1.2): each live unit's model centre (`GetObjCenter`) and its pick
/// radius `TransformTav(type scale · 1000, z)` (z in scene units: battle units × the scene scale); each gun's position
/// (virtual pixels).
pub fn pick(gfx: &Gfx, assets: &crate::starmap::Assets, s: &Shown, cam: &crate::starmap::Camera) -> (Vec<Option<crate::spacewar_ui::Projected>>, Vec<Option<crate::spacewar_ui::Projected>>) {
    let scene = Scene { view: &s.view, assets, models: &gfx.models, surface: None, shown: s };
    let units = s
        .battle
        .units
        .iter()
        .enumerate()
        .map(|(i, u)| {
            if !u.alive {
                return None;
            }
            let w = scene.centre(i, u)?;
            let q: [f32; 3] = std::array::from_fn(|j| (w[j] - s.view.origin[j]) * SCENE_SCALE);
            let (at, px) = to_virtual(cam, &s.view, q)?;
            // `TransformTav` takes the scene's depth: during the opening zoom (scene scale 0.0004) the radius is 2500
            // times the drawn size, so the nearest unit within 400 pixels is hovered (the exe's reticle at t300).
            let scale = s.types.get(i).cloned().flatten().map_or(0.0, |t| t.1);
            Some(crate::spacewar_ui::Projected { at, radius: scale * 1000.0 * px / s.view.scale.max(1e-6) })
        })
        .collect();
    let guns = s.battle.guns.iter().map(|g| to_virtual(cam, &s.view, [g.pos.x, g.pos.y, g.pos.z]).map(|(at, _)| crate::spacewar_ui::Projected { at, radius: 10.0 })).collect();
    (units, guns)
}

/// `SikMetszespont` (ELF 0x5ee770, exe 0x52ca10): the point of the plane at height `h` under the virtual point `at`,
/// along the view ray (its height step kept towards the plane); `None` unless it is strictly inside the arena (the
/// on-plane test `+0x2244`).
pub fn plane_point(s: &Shown, at: [f32; 2], h: f32) -> Option<[f32; 3]> {
    let m = look_matrix(s.eye, s.target);
    let e = [(at[0] - s.view.xcenter) / 600.0, (at[1] - 250.0) / 600.0, -1.0];
    let dir: [f32; 3] = std::array::from_fn(|j| m[0][j] * e[0] + m[1][j] * e[1] + m[2][j] * e[2]);
    let a: [f32; 3] = std::array::from_fn(|j| s.eye[j] + dir[j]);
    let bb: [f32; 3] = std::array::from_fn(|j| s.eye[j] + dir[j] * 10.0);
    // The exe's frame has the plane below the eye and clamps the height step to at most −0.01; in ours the eye is at
    // negative h (the battle's views look up at the grid plane at +18), so the step is at least +0.01.
    let mut dx = bb[0] - a[0];
    if (dx as f64) < 0.01 {
        dx = 0.01;
    }
    let q = dx / (h - a[0]);
    // The camera's units are the battle's times the scene scale.
    let p = [h, (bb[1] - a[1]) / q + a[1], (bb[2] - a[2]) / q + a[2]].map(|x| x / s.view.scale.max(1e-9));
    let b = s.battle;
    (b.bounds[0] < p[1] && p[1] < b.bounds[1] && b.bounds[2] < p[2] && p[2] < b.bounds[3]).then_some(p)
}

/// `magassagkirak` (spec §3.7): the height on the pole at the ground point `pole` whose screen row is `y`, between
/// its ends at −12.6 and 18.
pub fn pole_height(s: &Shown, cam: &crate::starmap::Camera, pole: [f32; 2], y: f32) -> f32 {
    let (h0, h1) = (-12.6f32, 18.0f32);
    match (to_virtual(cam, &s.view, [h0, pole[0], pole[1]]), to_virtual(cam, &s.view, [h1, pole[0], pole[1]])) {
        (Some((a, _)), Some((b, _))) if (a[1] - b[1]).abs() > 1e-3 => h0 + (h1 - h0) * ((a[1] - y) / (a[1] - b[1])),
        _ => h1,
    }
}

/// `--screen spacewar --battle FILE`: the battle a capture dumped, drawn at the dump's moment.
pub fn snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &mut crate::world::World, r: &mut crate::render::Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let path = o.battle.as_deref().ok_or("--screen spacewar needs --battle FILE")?;
    let d = Dump::load(path)?;
    if let Some(script) = &o.script {
        let cam = d.saved_starmap_camera();
        let kering = d.kering.unwrap_or(0.0);
        return live(o, out, l, world, r, hud, d.battle, cam, kering, &crate::script::Script::load(script)?);
    }
    let s = d.shown(&l.tables);
    let t = world.curr.tick as f32 * 0.05;
    let mut assets = crate::starmap::Assets::load(r, &l.install);
    assets.planet_meshes.load_battle(r, &l.install);
    let mut view = crate::starmap::View::new(&world.curr, &world.view, o.size, o.scale);
    view.set_clouds(o.clouds);
    view.futofeny = o.futofeny.or(d.futofeny);
    view.acttime = o.acttime.or(d.acttime.map(|a| a as f32 / 1000.0));
    let mut gfx = Gfx::load(r, &l.install, &s);
    prepare(&mut view, &gfx, assets.white, &s);
    let mut frame = view.build(&assets, &world.prev, &world.curr, o.alpha, t);
    let surface = world.curr.planets.iter().find(|p| p.index as i32 == d.battle.planet).map(|p| p.surface);
    let size = [o.size.0 as f32, o.size.1 as f32];
    draw(&mut frame, r, &l.install, &mut gfx, &assets, &view.camera, hud, &s, surface, size);
    // The speed buttons: `run_spwc.py --grab-ticks` grabs a running battle (the exe's bar shows the speed, not the
    // pause); `--paused` only holds the save's clock still here.
    hud.speed = (world.game.speed_level() - 1).clamp(0, 2) as usize;
    hud.paused = false;
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Starmap, t);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    let (b, v) = (&d.battle, &s.view);
    println!(
        "{}: space battle | tick {} units {} fighters {} | pivot {:?} yaw {} tilt {} zoom {} | eye {:?} target {:?} | origin {:?}",
        out.display(),
        b.tick,
        b.units.len(),
        b.fighters.len(),
        v.pivot,
        v.yaw,
        v.tilt,
        v.zoom,
        s.eye,
        s.target,
        v.origin
    );
    Ok(())
}

/// A dumped battle as the live battle in slot 0, the sim as `SpaceWarIndit` leaves it in SP (speed 1, paused).
pub fn go_live(world: &mut crate::world::World, battle: Battle) {
    let g = &mut world.game;
    g.realtime.slots = (0..10).map(|_| Battle::slot()).collect();
    let mut battle = battle;
    (battle.active, battle.shown) = (true, true);
    g.realtime.slots[0] = battle;
    g.realtime.types = g.rules.space.ship_types.clone();
    g.realtime.current = 0;
    g.sp_battles.live = true;
    g.set_speed(1, false);
    g.set_pause(true, false);
}

/// `--screen spacewar --battle FILE --script S`: the dumped battle as a live battle slot, entered from the save's
/// starmap camera and run by the script on the 10 ms UI clock (the sim's ticks every 50 ms while unpaused, on the
/// frames `t % 5 == --sim-phase`; `FRISSIT_SpaceWar2` every UI tick, a frame per UI tick); a PNG per grab, and a log
/// of what the screen sent (orders, clicks) on stdout for the (action → sound) table.
#[allow(clippy::too_many_arguments)]
fn live(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &mut crate::world::World, r: &mut crate::render::Renderer, hud: &mut crate::ui::Hud, battle: Battle, cam: [f32; 5], kering: f32, script: &crate::script::Script) -> Result<(), String> {
    use crate::script::Input;
    use crate::spacewar_ui::{Key, Out};
    go_live(world, battle);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let mut assets = crate::starmap::Assets::load(r, &l.install);
    assets.planet_meshes.load_battle(r, &l.install);
    let mut view = crate::starmap::View::new(&world.curr, &world.view, o.size, o.scale);
    view.set_clouds(o.clouds);
    // The starmap camera the exe entered with (the save's can lag behind it).
    let c = &mut view.camera;
    [c.x, c.y, c.rotate, c.pitch, c.zoom] = cam;
    let mut live = crate::spacewar_live::Live::enter(world, 0, &view.camera, &l.install, 0).ok_or("no battle slot")?;
    live.ui.kering = kering;
    let (mut mouse, mut next, mut shift) = ([320.0f32, 240.0], 0, false);
    for tick in 0..=script.end() {
        let now = tick * 10;
        let mut outs = Vec::new();
        while let Some(e) = script.events.get(next).filter(|e| e.t <= tick) {
            next += 1;
            match &e.input {
                Input::Move(at) => mouse = *at,
                Input::Button { button, down, at } => {
                    mouse = *at;
                    let first = !(live.left || live.right);
                    match button {
                        1 => live.left = *down,
                        3 => live.right = *down,
                        _ => {}
                    }
                    // The top bar's pause (spot 2, x 0..15, y < 20) on either button's first press, once the zoom is over.
                    if *down && first && matches!(button, 1 | 3) && live.clock_free() && at[0] < 15.0 && at[1] < 20.0 {
                        let paused = world.paused();
                        world.send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::from(ig2_sim::game::StarmapOrder::Pause { paused: !paused }) });
                    }
                }
                Input::Key { name, down } => {
                    let k = match name.as_str() {
                        "left" => Some(Key::Left),
                        "right" => Some(Key::Right),
                        "up" => Some(Key::Up),
                        "down" => Some(Key::Down),
                        "add" => Some(Key::ZoomIn),
                        "subtract" => Some(Key::ZoomOut),
                        "a" => Some(Key::All),
                        "c" => Some(Key::Camera),
                        // `DoKeyboard` keeps 1..3 (the speed); 4..9 reach the screen.
                        n if n.len() == 1 && ("4"..="9").contains(&n) => Some(Key::Group(n.as_bytes()[0] as i32 - b'1' as i32)),
                        "shift" => {
                            shift = *down;
                            None
                        }
                        _ => None,
                    };
                    if let (Some(k), Some(b)) = (k, world.game.realtime.slots.get(live.ui.slot)) {
                        live.ui.key(b, k, *down, false, false, false, &mut outs);
                    }
                }
            }
        }
        world.pump();
        for e in std::mem::take(&mut world.realtime) {
            if let ig2_sim::game::RealTimeEvent::SpaceOver { .. } = e {
                live.over(world, now);
                world.game.set_pause(true, false);
            }
        }
        if tick % 5 == o.sim_phase && !world.paused() {
            world.tick();
        }
        // The window pixel under the pointer (`MainCallBackEventHandler`'s mapping backwards, as the tracer's).
        live.pointer = [(mouse[0] * size[0] / 640.0).ceil() as i32, (mouse[1] * size[1] / 480.0).ceil() as i32];
        live.window = [o.size.0 as i32, o.size.1 as i32];
        outs.extend(live.update(world, now));
        let t = now as f32 / 1000.0;
        let (frame, more) = live.frame(world, r, &l.install, &assets, &mut view, hud, &l.tables, mouse, shift, now, 1.0, t, size);
        outs.extend(more);
        for (line, kind) in live.ui.voices.drain(..) {
            println!("t {tick}: Voice {{ line: {line}, kind: {kind} }}");
        }
        let mut left = false;
        for o in &outs {
            if !matches!(o, Out::Loop(crate::spacewar_fx::Loop::Volume { .. })) {
                println!("t {tick}: {o:?}");
            }
            for c in live.commands(o) {
                world.send(c);
            }
            left |= *o == Out::Left;
        }
        if let (Some(mut f), true) = (frame, script.grabs.binary_search(&tick).is_ok()) {
            hud.speed = (world.game.speed_level() - 1).clamp(0, 2) as usize;
            hud.paused = world.paused();
            hud.draw(&mut f, size, &world.ui, &l.tables, crate::ui::Screen::Starmap, t);
            let rgba = r.draw_to_image(&f, o.size, t);
            let path = crate::script::grab_path(out, tick);
            crate::assets::write_snapshot(&path, o, &rgba)?;
            let u = &live.ui;
            println!("{}: tick {tick} battle {} fade {} zoom {} yaw {} tilt {} pivot {:?} selected {}", path.display(), world.game.realtime.slots[0].tick, u.fade, u.zoom, u.yaw, u.tilt, u.pivot, u.stats.total);
        }
        if left {
            println!("t {tick}: left the battle screen");
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The look-at rows of a camera that looks down −h from above are a rotation (orthonormal, det 1).
    #[test]
    fn look_matrix_is_a_rotation() {
        let m = look_matrix([3.0, 1.0, 2.0], [0.0, 0.5, -1.0]);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for i in 0..3 {
            for j in 0..3 {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((dot(m[i], m[j]) - want).abs() < 1e-5, "{i} {j}");
            }
        }
        let det = dot(m[0], cross(m[1], m[2]));
        assert!((det - 1.0).abs() < 1e-5, "det {det}");
    }
}
