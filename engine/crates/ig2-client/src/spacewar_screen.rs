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
        tables.ship_types.get((u.race * 12 + u.kind).max(0) as usize).map(|t| (t.name.clone(), t.scale))
    }
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
            units_raw,
            erosseg: v["erosseg"].as_f64().map(|x| x as f32),
            types,
        })
    }

    fn f(&self, o: usize) -> f32 {
        raw::f32_at(&self.obj, o)
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
    Look { matrix: look_matrix(eye, target), eye: std::array::from_fn(|i| v.origin[i] + eye[i] / k), near: zbm * 1.05 / k }
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
    clock: HashMap<usize, Option<Mesh>>,
}

impl Panel {
    pub fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install) -> Self {
        let icons = (0..10).map(|i| Mesh::load(r, install, "vector/spacewar/", &format!("ikon{i}.xvb"), "textures/spacewar/")).collect();
        Self { icons, no: Mesh::load(r, install, "vector/spacewar/", "no.xvb", "textures/spacewar/"), clock: HashMap::new() }
    }

    fn clock(&mut self, r: &mut crate::render::Renderer, install: &ig2_formats::Install, frame: usize) {
        self.clock.entry(frame).or_insert_with(|| {
            let m = crate::scene::Model::load_at(install, "vector/spacewar/", "ora.xvb", "textures/spacewar/", Some(frame)).ok()?;
            let parts = m.parts.into_iter().filter_map(|p| Some((p.verts, r.add_texture(&crate::assets::Image::load(install, p.texture.as_deref()?).ok()?)))).collect();
            Some(Mesh { origin: m.origin, parts })
        });
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
        let at = [e[0] * k + 320.0, e[1] * k + 250.0];
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
    pub fn load(r: &mut crate::render::Renderer, install: &ig2_formats::Install, tables: &ig2_rules::Rules, d: &Dump) -> Self {
        let mut ships = HashMap::new();
        for (i, u) in d.battle.units.iter().enumerate() {
            let Some((name, _)) = d.unit_type(i, tables) else { continue };
            let key = (name, u.kind >= 8);
            if let std::collections::hash_map::Entry::Vacant(e) = ships.entry(key) {
                let m = Self::mesh(r, install, &e.key().0, e.key().1);
                e.insert(m);
            }
        }
        Self { ships }
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
    tables: &'a ig2_rules::Rules,
    /// The battle planet's surface (`Bolygok +0x22`), for a planet battle.
    surface: Option<u8>,
    /// The exe's units as dumped (600 bytes each): the drawn pose `+0xe4` (the node's matrix, `seturhajopos`).
    units_raw: &'a [u8],
    dump: &'a Dump,
}

/// Grid constants (`InitSpaceWar`, spec §1.5): cells of 3, 128 × 128, bounds ±192, the plane at h = 18.
const CELL: f32 = 3.0;
const CELLS: i32 = 128;
const BOUND: f32 = 192.0;
const PLANE_H: f32 = 18.0;

/// A vertex colour word `A<<24 | 0xBBGGRR`: red in the low byte, `A` a transparency (255 = clear; spec §6.1.1, §9.1), also
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
}

/// Draws camera-fixed models into a texture over the virtual rectangle `rect` (the way the starmap's panel is drawn,
/// `starmap_hud::Console`), to lay over the scene: they sit in front of everything [inf]. Lit by their own ambient
/// light 0x808080 and a front light [inf].
fn fixed_texture(r: &mut crate::render::Renderer, items: &[Fixed], rect: [f32; 4]) -> Option<crate::render::TexId> {
    if items.is_empty() {
        return None;
    }
    let mut dynamic = Vec::new();
    let mut draws = Vec::new();
    for it in items {
        let place = crate::scene::Placement::add_to_scene(it.mesh.origin, it.size);
        let rot = crate::scene::create_matrix(it.rot.0, it.rot.1, it.rot.2);
        let model = crate::scene::mul(&rot, &place.matrix);
        for (verts, tex) in &it.mesh.parts {
            let start = dynamic.len() as u32;
            for v in verts {
                let e = crate::scene::apply(&model, v.pos).map(|x| x * 1024.0);
                dynamic.push(crate::render3d::MeshVertex {
                    pos: [e[0] + it.at[0], e[1] + it.at[1], e[2] + it.at[2]],
                    normal: crate::scene::normalise(crate::scene::apply(&rot, v.normal)),
                    uv: v.uv,
                    color: [1.0, 1.0, 1.0, it.opacity],
                });
            }
            draws.push(crate::render3d::MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex: *tex, glass: it.opacity < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false });
        }
    }
    let [x0, y0, x1, y1] = rect;
    let (w, h) = (x1 - x0, y1 - y0);
    // Camera space → clip for the virtual window: vx = 600·x/−z + 320, vy = 600·y/−z + 250; depth 0..1 over −z =
    // 100..2000.
    let (a, b) = (-2000.0 / 1900.0, -100.0 * 2000.0 / 1900.0);
    let rows = [
        [1200.0 / w, 0.0, -(2.0 * (320.0 - x0) / w - 1.0), 0.0],
        [0.0, -1200.0 / h, -(1.0 - 2.0 * (250.0 - y0) / h), 0.0],
        [0.0, 0.0, a, b],
        [0.0, 0.0, -1.0, 0.0],
    ];
    let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
    let light = crate::scene::normalise([0.0, -0.35, 1.0]);
    let mut f = crate::render::Frame::new([0.0, 0.0, 0.0, 0.0]);
    f.scene = Some(crate::render3d::Scene3d {
        gamma: false,
        shadow: None,
        uniforms: crate::render3d::SceneUniforms {
            view,
            eye: [0.0; 4],
            light: [light[0], light[1], light[2], 0.0],
            ambient: [0.502, 0.502, 0.502, 0.0],
            sun: [0.25, 0.25, 0.25, 0.0],
            fog: [0.0; 4],
            fog_range: [1e9, 2e9, 0.0, 1e9],
        },
        draws,
        dynamic,
    });
    Some(r.render_texture(&f, ((w * 4.0) as u32, (h * 4.0) as u32)))
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
    /// A battle point in the starmap's world.
    fn world(&self, q: [f32; 3]) -> [f32; 3] {
        let k = (self.view.scale * SCENE_SCALE).max(1e-6);
        std::array::from_fn(|i| self.view.origin[i] + q[i] / k)
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
        // eye-space segment widened across the view ray by half a virtual pixel at each end, back to the world.
        let Some(look) = cam.look else { return };
        let d = [eb[0] - ea[0], eb[1] - ea[1], eb[2] - ea[2]];
        let across = |e: [f32; 3]| {
            let n = crate::scene::normalise(cross(d, e));
            let hw = 0.5 * -e[2] / 600.0;
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
        late.push(crate::render3d::MeshDraw { mesh: None, range: start..scene.dynamic.len() as u32, tex: white, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false });
    }

    /// The grid (`initracs`, spec §1.5): 128 lines each way at h = 18, each in three pieces (the outer ends clear),
    /// alpha `255 − 63t` (`t = (64 − |i − 64|)/32`, 0xC0 beyond t = 1), RGB 0x402000.
    fn grid(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, white: crate::render::TexId) {
        for i in 0..CELLS {
            let t = (64 - (i - 64).abs()) as f32 / 32.0;
            let a = if t <= 1.0 { (255.0 - 63.0 * t) as i32 as u32 } else { 0xc0 };
            let on = word_color(a << 24 | 0x40_2000);
            let off = word_color(0x40_2000);
            let c = (i - 64) as f32 * CELL + 1.5;
            for along_y in [true, false] {
                let at = |v: f32| if along_y { [PLANE_H, c, v] } else { [PLANE_H, v, c] };
                let (lo, hi) = (-BOUND, BOUND);
                self.line(f, cam, white, at(lo), at(lo / 2.0), off, on);
                self.line(f, cam, white, at(lo / 2.0), at(hi / 2.0), on, on);
                self.line(f, cam, white, at(hi / 2.0), at(hi), on, off);
            }
        }
    }

    /// The drop lines (`SpaceWarhajovonalkirajzol`, spec §1.6): from each live unit to the grid plane, alpha 200,
    /// green for the viewer's own, 0x0000E0 for allies, 0x00C0E0 for the rest.
    fn drop_lines(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, white: crate::render::TexId, b: &Battle) {
        for u in b.units.iter().filter(|u| u.alive && u.owner != 0 && (u.owner == b.human || !u.cloaked)) {
            let rgb = if u.owner == b.human {
                0x00_c000
            } else if b.hostile.get(b.human.max(0) as usize).and_then(|r| r.get(u.owner.max(0) as usize)).copied() == Some(1) {
                0x00_c0e0
            } else {
                0x00_00e0
            };
            let c = word_color(200 << 24 | rgb);
            self.line(f, cam, white, [PLANE_H, u.pos.y, u.pos.z], [u.pos.x, u.pos.y, u.pos.z], c, c);
        }
    }

    /// Unit `i`'s model and pose: the node's matrix (exe unit `+0xe4`, `seturhajopos`) and the `AddToScene` scale
    /// `tipusnevek.scale`, at its position.
    fn pose(&self, i: usize, u: &ig2_sim::spacebattle::Unit) -> Option<(&Mesh, crate::scene::M3, crate::scene::Transform)> {
        let k = (self.view.scale * SCENE_SCALE).max(1e-6);
        let (name, scale) = self.dump.unit_type(i, self.tables)?;
        let mesh = self.models.get(&name, u.kind >= 8)?;
        let m: crate::scene::M3 = match self.units_raw.get(i * 600..(i + 1) * 600) {
            Some(r) => std::array::from_fn(|i| raw::f32_at(r, 0xe4 + 4 * i)),
            None => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let place = crate::scene::Placement::add_to_scene(mesh.origin, scale / k);
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
            if !u.alive || (u.cloaked && u.owner != b.human) {
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
                late.push(crate::render3d::MeshDraw { mesh: None, range, tex: *tex, glass: alpha < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false });
            }
        }
    }

    /// `lovedekekkirak`'s pass B (ELF 0x6823c0, exe 0x578d20; spec §6.1): per live unit with its selection timers
    /// (exe unit `+0x184` grow T1, `+0x188` fade F, `+0x18c` attacked T3), the hit-point bar, the corner brackets while
    /// selected, the attack ticks, and the ship count of a selected group; additive, in virtual pixels. `c = 1 −
    /// starmaperosseg` (0 at the battle's zoom).
    fn overlays(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, hud: &crate::ui::Hud, b: &Battle, erosseg: f32) {
        use crate::render::{kind, Blend, Instance};
        let v = crate::font::Virtual::new(cam.size);
        let k = (self.view.scale * SCENE_SCALE).max(1e-6);
        let c = 1.0 - erosseg;
        let (ab, ar) = ((255.0 - c * 127.0) as i32 as u32, (255.0 - c * 191.0) as i32 as u32);
        let additive = |w: u32| {
            let rgb = word_color(w);
            let a = (255 - (w >> 24)) as f32 / 256.0;
            [rgb[0] * a, rgb[1] * a, rgb[2] * a, 1.0]
        };
        let (atk, dmg, ok) = (additive(ab << 24 | 0x00_00ff), additive(ab << 24 | 0x40_40ff), additive(ab << 24 | 0x40_ff40));
        let (foe, own) = (additive(ar << 24 | 0x40_40ff), additive(ar << 24 | 0xff_ffff));
        // `line3`'s quads: screen-aligned rectangles between two corners.
        let quad = |f: &mut crate::render::Frame, x0: f32, y0: f32, x1: f32, y1: f32, col: [f32; 4]| {
            let (lx, hx, ly, hy) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
            if hx > lx && hy > ly {
                f.push(self.assets.white, Blend::GammaAdditive, Instance::new(kind::SPRITE, v.px((lx + hx) / 2.0, (ly + hy) / 2.0), [(hx - lx) * v.k / 2.0, (hy - ly) * v.k / 2.0], col));
            }
        };
        let m = cam.matrix();
        let font = hud.font.as_ref();
        let pal = |i: usize| hud.palette.get(i).copied().unwrap_or([1.0; 3]);
        for (i, u) in b.units.iter().enumerate() {
            let Some(r) = self.units_raw.get(i * 600..(i + 1) * 600) else { continue };
            if !u.alive {
                continue;
            }
            let (t1, fd, t3) = (raw::i32_at(r, 0x184) as f32, raw::i32_at(r, 0x188) as f32, raw::i32_at(r, 0x18c) as f32);
            let selected = r[0x181] != 0;
            let brackets = t1 != 0.0 && fd < 250.0;
            if !brackets && t3 <= 0.0 {
                continue;
            }
            let w = self.centre(i, u).unwrap_or_else(|| self.world([u.pos.x, u.pos.y, u.pos.z]));
            let e = cam.eye(&m, w[0], [w[1], w[2]]);
            let Some((p, px)) = cam.project_eye(e) else { continue };
            let [x, y] = v.virtual_of(p);
            let scale = self.dump.unit_type(i, self.tables).map_or(0.0, |t| t.1);
            let rr0 = scale * 1000.0 * self.view.scale / k * px / v.k;
            let (g, fa) = if brackets { ((t1 / 250.0).min(1.0), (250.0 - fd) / 250.0) } else { ((t3 / 250.0).min(1.0), 1.0) };
            // The bar: `r·f/5` high, `1.2·r·f + 1` above the centre (f64 in the exe), red for the damage, green.
            let q = u.damage as f32 / u.hp as f32;
            let yt = (y as f64 - (rr0 * fa) as f64 * 1.2 - 1.0) as f32;
            let yb = (y as f64 - ((rr0 * fa) as f64 * 1.2 - (rr0 * fa) as f64 / 5.0) - 1.0) as f32;
            let x0 = x - rr0 * g;
            let xm = x0 + rr0 * q * 2.0 * g;
            quad(f, x0, yt, xm, yb, dmg);
            quad(f, xm, yt, x + rr0 * g, yb, ok);
            if u.shield_used < u.shield {
                let cc = u.shield_used as f32 / u.shield as f32;
                quad(f, x0 + rr0 * cc * 2.0 * g, yt, x + rr0 * g, yb, dmg);
            }
            let mut rr = rr0;
            if brackets {
                let col = if u.owner == b.human { own } else { foe };
                let r2 = 9000.0 / t1 - 9.0 + rr0;
                let la = r2 * (250.0 - fd) / 500.0;
                for (cx, cy, sx, sy) in [(-1.0, -1.0, 1.0, 1.0), (1.0, 1.0, -1.0, -1.0), (1.0, -1.0, -1.0, 1.0), (-1.0, 1.0, 1.0, -1.0)] {
                    let (px0, py0) = (x + cx * r2, y + cy * r2);
                    quad(f, px0, py0, px0 + sx * la, py0 - sy, col);
                    quad(f, px0, py0, px0 - sx, py0 + sy * la, col);
                }
                rr = r2;
            }
            if t3 > 0.0 {
                let r3 = 9000.0 / t3 - 9.0 + rr0;
                let lt = r3 * 250.0 / 500.0;
                quad(f, x + r3, y, x + r3 + lt, y + 1.0, atk);
                quad(f, x - r3, y, x - r3 - lt, y + 1.0, atk);
                quad(f, x, y + r3, x + 1.0, y + r3 + lt, atk);
                quad(f, x, y - r3, x + 1.0, y - r3 - lt, atk);
                rr = r3;
            }
            if let (true, Some(font)) = (selected && u.ship_count > 1, font) {
                font.int(f, &v, u.ship_count as i64, (x + rr - 5.0).trunc(), (y + rr - 7.0).trunc(), 5, 2, crate::font::Align::Left, pal(3));
            }
        }
    }

    /// The weapon column (spec §5.1): per shown slot (`c` ≠ 0) the button at 0.02 turned (π/2, 0, 0), the reload
    /// clock and the overlay at 0.014 (not on the retreat button 9); the overlay is 0.75 opaque when some of the
    /// group's mounts are off, 0.5 when all are, hidden when all are on. Lit by its own ambient light 0x808080 [inf].
    fn weapons<'p>(&self, items: &mut Vec<Fixed<'p>>, panel: &'p Panel) {
        let half = std::f32::consts::FRAC_PI_2;
        for i in 0..10 {
            let sl = Slot::from_slot(&self.dump.obj, i);
            if sl.counter == 0 {
                continue;
            }
            let at = sl.place(i, self.view.xcenter);
            if let Some(Some(m)) = panel.icons.get(i) {
                items.push(Fixed { mesh: m, at, size: 0.02, rot: (half, 0.0, 0.0), opacity: 1.0 });
            }
            if i == 9 {
                continue;
            }
            // The clock shows only while the group reloads [obs: none at a full 0xaa].
            if let (true, Some(Some(m))) = (sl.reload < 0xaa, panel.clock.get(&clock_frame(sl.reload))) {
                items.push(Fixed { mesh: m, at, size: 0.014, rot: (0.0, 0.0, 0.0), opacity: 1.0 });
            }
            let overlay = match sl.state {
                3 => Some(0.75),
                6 => Some(0.5),
                _ => None,
            };
            if let (Some(a), Some(m)) = (overlay, &panel.no) {
                items.push(Fixed { mesh: m, at, size: 0.014, rot: (0.0, 0.0, 0.0), opacity: a });
            }
        }
    }

    /// The info box's model (`kisinfogfxfrissit` ELF 0x6860c0, exe 0x5778c0; spec §5.2): the selection's highest
    /// `race·12 + type` (`selectkisinfourhajo`) at 0.03, turned π/2 about x, at (−545, 365, −1100), spinning 0.05 a
    /// frame (the phase isn't dumped: `spin`).
    fn info_model(&self, items: &mut Vec<Fixed<'a>>, b: &Battle, spin: f32) {
        let mut best: Option<(i32, usize)> = None;
        for (i, u) in b.units.iter().enumerate() {
            let sel = self.units_raw.get(i * 600 + 0x181).is_some_and(|&x| x != 0);
            if u.alive && sel && best.is_none_or(|(t, _)| u.race * 12 + u.kind > t) {
                best = Some((u.race * 12 + u.kind, i));
            }
        }
        let Some((_, i)) = best else { return };
        let Some((name, _)) = self.dump.unit_type(i, self.tables) else { return };
        if let Some(m) = self.models.get(&name, b.units[i].kind >= 8) {
            items.push(Fixed { mesh: m, at: [-545.0, 365.0, -1100.0], size: 0.03, rot: (std::f32::consts::FRAC_PI_2, spin, 0.0), opacity: 1.0 });
        }
    }

    fn draw(&self, f: &mut crate::render::Frame, cam: &crate::starmap::Camera, b: &Battle, eye: &crate::starmap_planet::Eye) {
        let k = (self.view.scale * SCENE_SCALE).max(1e-6);
        // The planet of a planet battle (step 7): `b{(type − 1) % 8}2.xvb` at R·1.16, `RotateObj(π/2, π/2, 0)`, at P.
        if let (Some(surface), true) = (self.surface, b.planet_owner >= 0 || b.planet > 0) {
            let r = b.planet_size as f32;
            let p = b.planet_pos.map(|x| x as f32);
            let half = std::f32::consts::FRAC_PI_2;
            let crate::render::Frame { scene, late, .. } = f;
            if let Some(scene) = scene.as_mut() {
                self.assets.planet_meshes.add_battle_planet(scene, late, surface, r * 1.16 / k, (half, half, 0.0), self.world(p), eye, planet_light());
            }
        }
        self.ships(f, b, eye);
        self.grid(f, cam, self.assets.white);
        self.drop_lines(f, cam, self.assets.white, b);
    }
}

/// The starmap scene's parallel light (`Init3dStarmap`: 0x808080 along (10, 10, −10), the direction it shines in):
/// towards it is (−10, −10, 10) [obs: of the eight sign choices the capture matches this one, mean diff 7.3 vs ≥ 17.6].
fn planet_light() -> [f32; 3] {
    [-10.0, -10.0, 10.0]
}

/// `ora.xvb`'s frame for a reload `0..0xaa`: `SetFrame((reload/10)·1.65)` (int division, the frame truncated).
fn clock_frame(reload: i32) -> usize {
    ((reload / 10) as f32 * 1.65) as usize
}

/// `--screen spacewar --battle FILE`: the battle a capture dumped, drawn at the dump's moment.
pub fn snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut crate::render::Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let path = o.battle.as_deref().ok_or("--screen spacewar needs --battle FILE")?;
    let d = Dump::load(path)?;
    let v = d.view();
    let (eye, target) = match d.cam {
        Some(c) if !v.restoring => ([c[0], c[1], c[2]], [c[4], c[5], c[6]]),
        _ => v.eye_target(),
    };
    let t = world.curr.tick as f32 * 0.05;
    let mut assets = crate::starmap::Assets::load(r, &l.install);
    assets.planet_meshes.load_battle(r, &l.install);
    let mut view = crate::starmap::View::new(&world.curr, &world.view, o.size, o.scale);
    view.set_clouds(o.clouds);
    view.futofeny = o.futofeny.or(d.futofeny);
    view.acttime = o.acttime.or(d.acttime.map(|a| a as f32 / 1000.0));
    view.camera.look = Some(starmap_look(&v, eye, target));
    star_sprites(assets.white, &mut view.backdrop, &view.camera, &star_field());
    let mut frame = view.build(&assets, &world.prev, &world.curr, o.alpha, t);
    let models = Models::load(r, &l.install, &l.tables, &d);
    let scene = Scene {
        view: &v,
        assets: &assets,
        models: &models,
        tables: &l.tables,
        surface: world.curr.planets.iter().find(|p| p.index as i32 == d.battle.planet).map(|p| p.surface),
        units_raw: &d.units_raw,
        dump: &d,
    };
    scene.draw(&mut frame, &view.camera, &d.battle, &view.camera.eye_world());
    scene.overlays(&mut frame, &view.camera, hud, &d.battle, d.erosseg.unwrap_or(0.0));
    let mut panel = Panel::load(r, &l.install);
    for i in 0..9 {
        panel.clock(r, &l.install, clock_frame(Slot::from_slot(&d.obj, i).reload));
    }
    let virt = crate::font::Virtual::new([o.size.0 as f32, o.size.1 as f32]);
    let mut column = Vec::new();
    scene.weapons(&mut column, &panel);
    put_fixed(&mut frame, &virt, fixed_texture(r, &column, COLUMN_RECT), COLUMN_RECT);
    let mut info = Vec::new();
    scene.info_model(&mut info, &d.battle, 0.0);
    let info_tex = fixed_texture(r, &info, INFO_MODEL_RECT);
    // The panel under the ambient light alone [obs: console diff 11.4 and 10.1 in two battles against 19.4 and 16.3
    // with the starmap's lights; the exe's panel changes colour with the side lights `smMozgatMainFenyek` circles].
    let console = crate::starmap_hud::Console::load_lit(r, &l.install, [0.0; 3]).map_err(|e| eprintln!("ig2-client: kezel.xvb: {e}")).ok();
    crate::spacewar_hud::draw(&mut frame, hud, console.as_ref(), &virt, &d, eye, target, 1.0);
    put_fixed(&mut frame, &virt, info_tex, INFO_MODEL_RECT);
    hud.draw(&mut frame, [o.size.0 as f32, o.size.1 as f32], &world.ui, &l.tables, crate::ui::Screen::Starmap, t);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    let b = &d.battle;
    println!(
        "{}: space battle | tick {} units {} fighters {} | pivot {:?} yaw {} tilt {} zoom {} | eye {eye:?} target {target:?} | origin {:?}",
        out.display(),
        b.tick,
        b.units.len(),
        b.fighters.len(),
        v.pivot,
        v.yaw,
        v.tilt,
        v.zoom,
        v.origin
    );
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
