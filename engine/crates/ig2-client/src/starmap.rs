//! The starmap screen's view (ADR 0006), drawn the way the original draws it (`re/screens/starmap.md`): its 3D
//! camera (`StarmapXStart/YStart/Rotate/Pitch/Zoom`, fov 600 on a 480-line virtual screen), a star glow
//! `n1..n8` per system, planets orbiting their star, fleets as the 2D `screen/star_pl` sprites in their empire's
//! column, routes, the selection ring and the generated background, in the exe's palette (`DDColor`).
//!
//! Names are drawn by [`View::names`] (re/screens/hud.md §4). Not drawn yet: the HUD panels, the minimap, radar
//! rings (spec §9).

use ig2_formats::Install;

use crate::assets::Image;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};
use crate::world::{FleetView, PlanetView, SavedView, Snapshot};

/// Surface types with a planet map `p<n>`.
const SURFACES: usize = 10;

/// `DDColor` in the user's exe (spec §7): 43 × `0x00BBGGRR`.
const DD_COLOR: u32 = 0x8F_8710;
const DD_COLORS: usize = 43;

/// A sprite sheet of equal cells (a brush set, spec §4.2).
#[derive(Debug, Clone, Copy)]
struct Sheet {
    tex: TexId,
    cell: [f32; 2],
    size: [f32; 2],
}

impl Sheet {
    fn uv(&self, col: u32, row: u32) -> [f32; 4] {
        let (w, h) = (self.cell[0] / self.size[0], self.cell[1] / self.size[1]);
        [col as f32 * w, row as f32 * h, w, h]
    }
}

/// The starmap's textures and the exe's palette (each texture falls back to a plain shape when missing).
pub struct Assets {
    pub white: TexId,
    select: Option<TexId>,
    planets: [Option<TexId>; SURFACES],
    /// `textures/starmap/n1..n8`: the system stars.
    stars: [Option<TexId>; 8],
    /// `felho3`, `felho2`: the galaxy clouds.
    clouds: [Option<TexId>; 2],
    /// `radar.bmp`: the radar ranges' dots; the discs' colours, `rrange.bmp` (planets) and `rrange2.bmp` (fleets).
    radar_dot: Option<TexId>,
    radar_fill: [[f32; 3]; 2],
    /// `gomb.xvb`: the sky sphere around the galaxy (spec §6.5), baked into world space, per texture.
    sky: Vec<(Vec<crate::render3d::MeshVertex>, TexId)>,
    /// The planet and cloud meshes (spec §3.3).
    pub planet_meshes: crate::starmap_planet::Planets,
    /// `screen/star_pl` (30 × 22 cells, 17 columns) and `screen/star_pl2` (15 × 11, 16 columns).
    fleets: Option<Sheet>,
    small_fleets: Option<Sheet>,
    palette: Vec<[f32; 3]>,
}

impl Assets {
    pub fn load(r: &mut Renderer, install: &Install) -> Self {
        let img = |path: &str| match Image::load(install, path) {
            Ok(i) => Some(i),
            Err(e) => {
                eprintln!("ig2-client: {e} (drawing a plain shape instead)");
                None
            }
        };
        let mut tex = |path: &str| img(path).map(|i| r.add_texture(&i));
        let select = tex("dataraw/textures/starmap/select_bmp.pvr");
        let planets = std::array::from_fn(|s| if s > 0 { tex(&format!("dataraw/textures/starmap/p{s}_bmp.pvr")) } else { None });
        let stars = std::array::from_fn(|i| tex(&format!("dataraw/textures/starmap/n{}_bmp.pvr", i + 1)));
        let clouds = [tex("dataraw/textures/starmap/felho3_bmp.pvr"), tex("dataraw/textures/starmap/felho2_bmp.pvr")];
        let radar_dot = tex("dataraw/textures/starmap/radar_bmp.pvr");
        let sky = sky_sphere(r, install);
        let planet_meshes = crate::starmap_planet::Planets::load(r, install);
        // The disc textures are one colour (rrange2 with faint stripes): their mean.
        let mean = |path: &str, or: [f32; 3]| {
            img(path).map_or(or, |i| {
                let n = (i.rgba.len() / 4).max(1) as f32;
                let sum = i.rgba.chunks_exact(4).fold([0.0f32; 3], |a, p| [a[0] + p[0] as f32, a[1] + p[1] as f32, a[2] + p[2] as f32]);
                sum.map(|c| c / n / 255.0)
            })
        };
        let radar_fill = [mean("dataraw/textures/starmap/rrange_bmp.pvr", [1.0, 0.0, 0.0]), mean("dataraw/textures/starmap/rrange2_bmp.pvr", [0.27, 0.0, 1.0])];
        let mut sheet = |path: &str, cell: [f32; 2]| {
            img(path).map(|i| Sheet { size: [i.width as f32, i.height as f32], tex: r.add_texture(&i), cell })
        };
        let fleets = sheet("dataraw/screen/star_pl_bmp.pvr", [30.0, 22.0]);
        let small_fleets = sheet("dataraw/screen/star_pl2_bmp.pvr", [15.0, 11.0]);
        let white = r.add_texture(&Image::white());
        let palette = read_palette(install).unwrap_or_else(|e| {
            eprintln!("ig2-client: no palette from ig2.exe ({e}); drawing everything grey");
            vec![[0.6; 3]; DD_COLORS]
        });
        Self { white, select, planets, stars, clouds, radar_dot, radar_fill, sky, planet_meshes, fleets, small_fleets, palette }
    }

    /// Empire `e`'s colour, `DDColor[e + 15]` (spec §7).
    fn empire(&self, owner: u8, a: f32) -> [f32; 4] {
        let [r, g, b] = self.palette.get(owner as usize + 15).copied().unwrap_or([0.6; 3]);
        [r, g, b, a]
    }
}

/// `Init3dStarmap`'s `gomb.xvb` (`AddToScene(…, mainxvb, 2.0)`, not moved or turned): a sphere of radius ≈ 2048
/// around the galaxy's origin with the nebula textures `texcso3.bmp` and `ujtext.bmp` on the inside.
fn sky_sphere(r: &mut Renderer, install: &Install) -> Vec<(Vec<crate::render3d::MeshVertex>, TexId)> {
    use crate::scene::{Model, Placement};
    let model = match Model::load(install, "vector/starmap/", "gomb.xvb", "textures/starmap/") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("ig2-client: {e} (no sky sphere)");
            return Vec::new();
        }
    };
    let xf = Placement::add_to_scene(model.origin, 2.0).world((0.0, 0.0, 0.0), [0.0; 3]);
    model
        .parts
        .iter()
        .filter_map(|p| {
            let t = Image::load(install, p.texture.as_deref()?).ok()?;
            let tex = r.add_texture(&t);
            let verts = p
                .verts
                .iter()
                .map(|v| crate::render3d::MeshVertex {
                    pos: xf.point(v.pos),
                    normal: crate::scene::normalise(crate::scene::apply(&xf.matrix, v.normal)),
                    uv: v.uv,
                    color: [1.0, 1.0, 1.0, 0.0],
                })
                .collect();
            Some((verts, tex))
        })
        .collect()
}

/// `DDColor` from the user's `ig2.exe` (never shipped: ADR 0004).
pub(crate) fn read_palette(install: &Install) -> Result<Vec<[f32; 3]>, String> {
    let exe = std::fs::read(install.root().join("ig2.exe")).map_err(|e| e.to_string())?;
    let img = ig2_rules::PeImage::parse(&exe).map_err(|e| e.to_string())?;
    let b = img.read(DD_COLOR, 4 * DD_COLORS).map_err(|e| e.to_string())?;
    Ok(b.chunks_exact(4).map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]).collect())
}

/// A `0xTTBBGGRR` particle colour: RGB × (255 − TT)/256 (`Particle` ELF 0x5f4ff0).
pub(crate) fn particle_color(c: u32) -> [f32; 4] {
    let k = (255 - (c >> 24)) as f32 / 256.0 / 255.0;
    [(c & 0xff) as f32 * k, ((c >> 8) & 0xff) as f32 * k, ((c >> 16) & 0xff) as f32 * k, 1.0]
}

/// `fov` (`calculateclipplanes`: `zoom · 600`, zoom 1).
const FOV: f32 = 600.0;
/// `xcenter`, `ycenter` as `ShowStarmap` sets them.
const CENTER: [f32; 2] = [320.0, 250.0];
/// The virtual screen's height (`HatarY2 + 1`).
const VIRTUAL_H: f32 = 480.0;
/// Particles closer than this are clipped (`zclip = −1.05 · zbm`, `zbm` 0.4).
const ZCLIP: f32 = -0.42;
/// The pan square (`Starmap_Move0`).
const PAN_LIMIT: f32 = 300.0;
/// UI ticks per second (`FRISSIT_Starmap2` runs every 10 ms).
const UI_HZ: f32 = 100.0;

/// The original's starmap camera (spec §2): it looks at the pivot `(x, −y)` from `600 − zoom` units away, turned
/// by `rotate` (yaw) and `pitch`. Screen positions are physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// `StarmapXStart`, `StarmapYStart` (the pivot's galaxy y is `−y`).
    pub x: f32,
    pub y: f32,
    pub rotate: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub size: [f32; 2],
    /// The battle screen's look-at camera instead (re/screens/spacewar.md §2): the battle is drawn inside the
    /// starmap's scene, so the galaxy behind it is this view of the starmap.
    pub look: Option<Look>,
}

/// A look-at camera in the starmap's world `(h, x, y)` (`CameraRotate` ELF 0x5fcf90: the view rows `c`, `−b`, `−d`
/// from `CreateMatrix2v`), its eye, the near plane `zbm · 1.05` and the projection's x centre (`xcenter`, 320; 160 in the
/// info view: `TransformPoint` adds it to every point of the scene, the starmap's too).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub matrix: [[f32; 3]; 3],
    pub eye: [f32; 3],
    pub near: f32,
    pub xcenter: f32,
}

/// `CreateMatrix(α, β, γ)` (ELF 0x451ee0), f32 sines and cosines like the original.
pub(crate) fn create_matrix(a: f32, b: f32, c: f32) -> [[f32; 3]; 3] {
    let (s1, c1) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
    let (s2, c2) = ((b as f64).sin() as f32, (b as f64).cos() as f32);
    let (s3, c3) = ((c as f64).sin() as f32, (c as f64).cos() as f32);
    [
        [c3 * c2, -c3 * s2 * s1 - s3 * c1, -c3 * s2 * c1 + s3 * s1],
        [s3 * c2, -s3 * s2 * s1 + c3 * c1, -s3 * s2 * c1 - c3 * s1],
        [s2, c2 * s1, c2 * c1],
    ]
}

impl Camera {
    /// The view a new game starts with (`StarmapSetSideView` ELF 0x6fde80) over the planets the human sees.
    pub fn side_view(s: &Snapshot, size: [f32; 2]) -> Self {
        let (mut hi, mut lo) = ([-300.0f32; 2], [300.0f32; 2]);
        let mut n = 0;
        for p in s.planets.iter().filter(|p| p.known) {
            for k in 0..2 {
                hi[k] = hi[k].max(p.pos[k]);
                lo[k] = lo[k].min(p.pos[k]);
            }
            n += 1;
        }
        let mut c = Self { x: 0.0, y: 0.0, rotate: 0.0, pitch: -0.95, zoom: 2.0, size, look: None };
        if n > 0 {
            c.x = (hi[0] + lo[0]) / 2.0;
            c.y = -(hi[1] + lo[1]) / 2.0;
            let zx = ((hi[0] - lo[0]).abs() / 2.0 * 600.0 / 320.0 - 600.0).abs();
            let zy = ((hi[1] - lo[1]).abs() / 2.0 * 600.0 / 240.0 - 600.0).abs();
            c.zoom = zx.min(zy) / 1.5;
        }
        c
    }

    /// The camera a save was left with, or the side view when it holds none.
    pub fn from_save(v: &SavedView, s: &Snapshot, size: [f32; 2]) -> Self {
        if v.zoom == 0.0 && v.pitch == 0.0 {
            return Self::side_view(s, size);
        }
        let rotate = v.rotate.rem_euclid(std::f32::consts::TAU);
        Self { x: v.x, y: v.y, rotate, pitch: v.pitch, zoom: v.zoom, size, look: None }
    }

    pub(crate) fn matrix(&self) -> [[f32; 3]; 3] {
        if let Some(l) = &self.look {
            return l.matrix;
        }
        create_matrix(self.rotate, self.pitch, std::f32::consts::FRAC_PI_2)
    }
    fn distance(&self) -> f32 {
        600.0 - self.zoom
    }
    /// The projection's centre on the virtual screen.
    fn center(&self) -> [f32; 2] {
        [self.look.map_or(CENTER[0], |l| l.xcenter), CENTER[1]]
    }
    /// Physical pixels per virtual pixel.
    pub fn k(&self) -> f32 {
        self.virt().k
    }

    /// The virtual screen in this view ([`crate::kit::Layout`]).
    pub fn virt(&self) -> crate::font::Virtual {
        crate::font::Virtual::new(self.size)
    }

    /// Camera space of the galaxy point `p` at height `h` (`smRotatePoint`).
    pub(crate) fn eye(&self, m: &[[f32; 3]; 3], h: f32, p: [f32; 2]) -> [f32; 3] {
        let (d, back) = match &self.look {
            Some(l) => ([h - l.eye[0], p[0] - l.eye[1], p[1] - l.eye[2]], 0.0),
            None => ([h, p[0] - self.x, p[1] + self.y], self.distance()),
        };
        let r = |i: usize| m[i][0] * d[0] + m[i][1] * d[1] + m[i][2] * d[2];
        [r(0), r(1), r(2) - back]
    }

    /// World `(h, x, y)` → clip space for the mesh pass: the particles' projection (`e = M·(p − pivot) − (0, 0, D)`,
    /// `fov` 600, centre (320, 250) of the 480-line virtual screen) as a matrix. The near plane is `ShowStarmap`'s
    /// `zbm` (0.4): the long info's eye sits about one unit from its planet.
    fn clip_matrix(&self) -> [[f32; 4]; 4] {
        const FAR: f32 = 10000.0;
        let m = self.matrix();
        let (pivot, back, near) = match &self.look {
            Some(l) => (l.eye, 0.0, l.near),
            None => ([0.0, self.x, -self.y], self.distance(), 0.4),
        };
        let t = |i: usize| -(m[i][0] * pivot[0] + m[i][1] * pivot[1] + m[i][2] * pivot[2]);
        // e = M·p + (t0, t1, t2 − D); w = −e.z.
        let e = [[m[0][0], m[0][1], m[0][2], t(0)], [m[1][0], m[1][1], m[1][2], t(1)], [m[2][0], m[2][1], m[2][2], t(2) - back]];
        let w = e[2].map(|x| -x);
        let sx = 2.0 * FOV / VIRTUAL_H * self.size[1] / self.size[0];
        let sy = -2.0 * FOV / VIRTUAL_H;
        let cy = 1.0 - 2.0 * CENTER[1] / VIRTUAL_H;
        let cx = 2.0 * (self.center()[0] - CENTER[0]) / VIRTUAL_H * self.size[1] / self.size[0];
        let zf = FAR / (FAR - near);
        let rows = [
            std::array::from_fn(|c| e[0][c] * sx + w[c] * cx),
            std::array::from_fn(|c| e[1][c] * sy + w[c] * cy),
            std::array::from_fn(|c| w[c] * zf - if c == 3 { near * zf } else { 0.0 }),
            w,
        ];
        std::array::from_fn(|c| std::array::from_fn(|r| rows[r][c]))
    }

    /// The eye (spec §2.2): the camera's rows and its world point (h, x, y), where `e = 0` in [`Camera::eye`].
    pub fn eye_world(&self) -> crate::starmap_planet::Eye {
        let m = self.matrix();
        if let Some(l) = &self.look {
            return crate::starmap_planet::Eye { matrix: m, at: l.eye };
        }
        let d = self.distance();
        crate::starmap_planet::Eye { matrix: m, at: [m[2][0] * d, m[2][1] * d + self.x, m[2][2] * d - self.y] }
    }

    /// Virtual → physical pixels.
    fn phys(&self, v: [f32; 2]) -> [f32; 2] {
        self.virt().px(v[0], v[1])
    }

    /// Screen position and physical pixels per galaxy unit there, when in front of the camera.
    pub(crate) fn project_eye(&self, e: [f32; 3]) -> Option<([f32; 2], f32)> {
        if e[2] >= 0.0 {
            return None;
        }
        let f = FOV / -e[2];
        let c = self.center();
        Some((self.phys([e[0] * f + c[0], e[1] * f + c[1]]), f * self.k()))
    }

    pub fn project(&self, p: [f32; 2]) -> Option<([f32; 2], f32)> {
        self.project_eye(self.eye(&self.matrix(), 0.0, p))
    }

    pub fn to_screen(self, p: [f32; 2]) -> Option<[f32; 2]> {
        self.project(p).map(|(s, _)| s)
    }

    /// The galaxy point (height 0) under the screen point `s`, if the ray meets the galaxy plane.
    pub fn to_world(self, s: [f32; 2]) -> Option<[f32; 2]> {
        let v = self.virt().virtual_f(s);
        let c = self.center();
        let dir = [(v[0] - c[0]) / FOV, (v[1] - c[1]) / FOV, -1.0];
        let m = self.matrix();
        // d = Mᵀ·(e + (0, 0, D)): the eye sits at e = 0.
        let back = |e: [f32; 3]| -> [f32; 3] { std::array::from_fn(|j| m[0][j] * e[0] + m[1][j] * e[1] + m[2][j] * e[2]) };
        let (o, at) = match &self.look {
            Some(l) => (l.eye, [0.0, 0.0]),
            None => (back([0.0, 0.0, self.distance()]), [self.x, -self.y]),
        };
        let r = back(dir);
        if r[0].abs() < 1e-9 {
            return None;
        }
        let t = -o[0] / r[0];
        (t > 0.0).then(|| [o[1] + t * r[1] + at[0], o[2] + t * r[2] + at[1]])
    }

    /// A galaxy point on the 640 × 480 virtual screen (`smRotatePoint` + `TransformPoint`), when in front.
    pub fn virtual_point(&self, p: [f32; 2]) -> Option<[f32; 2]> {
        let e = self.eye(&self.matrix(), 0.0, p);
        if e[2] >= 0.0 {
            return None;
        }
        let (f, c) = (FOV / -e[2], self.center());
        Some([e[0] * f + c[0], e[1] * f + c[1]])
    }

    /// Physical → virtual pixels.
    pub fn virtual_of(&self, s: [f32; 2]) -> [f32; 2] {
        self.virt().virtual_of(s)
    }

    /// `get_pos` / `SikMetszespont`: the galaxy point under the virtual point `v`.
    pub fn virtual_to_world(&self, v: [f32; 2]) -> Option<[f32; 2]> {
        self.to_world(self.phys(v))
    }

    /// Drag the galaxy so the point under `from` ends under `to`; the pivot stays in the pan square.
    pub fn pan(&mut self, from: [f32; 2], to: [f32; 2]) {
        if let (Some(a), Some(b)) = (self.to_world(from), self.to_world(to)) {
            self.x = (self.x + a[0] - b[0]).clamp(-PAN_LIMIT, PAN_LIMIT);
            self.y = (self.y - (a[1] - b[1])).clamp(-PAN_LIMIT, PAN_LIMIT);
        }
    }

    /// `steps` zoom key presses (`StarmapKeyboardDoModifies`: `Zoom ± (640 − Zoom)/50`, clamp [2, 580]).
    pub fn zoom_steps(&mut self, steps: f32) {
        for _ in 0..steps.abs().round().max(1.0) as u32 {
            let d = (640.0 - self.zoom) / 10.0 / 5.0;
            self.zoom = (self.zoom + d * steps.signum()).clamp(2.0, 580.0);
        }
    }

    /// Yaw by `frames` keyboard frames (±0.02 each).
    pub fn turn(&mut self, frames: f32) {
        self.rotate = (self.rotate + 0.02 * frames).rem_euclid(std::f32::consts::TAU);
    }

    /// Pitch by `frames` keyboard frames (±0.00628318 each, clamp [−1.560795, −0.2]).
    pub fn tilt(&mut self, frames: f32) {
        self.pitch = (self.pitch + 0.00628318 * frames).clamp(-1.560795, -0.2);
    }
}

/// Something on the map under the cursor or selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Planet(usize),
    Fleet(usize),
}

/// The starmap view state: camera, UI scale, hover and selection.
pub struct View {
    pub camera: Camera,
    /// Logical → physical pixels (the window's scale factor).
    pub scale: f32,
    pub hover: Option<Pick>,
    pub selected: Option<Pick>,
    /// Draw every fleet and planet, not only those the human sees (the original's `DEBUGX` view).
    pub reveal: bool,
    /// `StarSize[p − 1]` over its backup while the planet's long info flies in or out (re/screens/starmap.md §15.1).
    pub star_scale: Option<(usize, f32)>,
    /// Quads drawn right after the background, under the planets (the battle screen's star field).
    pub backdrop: Vec<(TexId, Blend, Instance)>,
    background: Vec<Particle>,
    /// The background as generated, before any drift.
    generated: Vec<Particle>,
    /// `GomolygasCounter`: UI ticks the starmap has run (the clouds' drift clock).
    clouds: i32,
    /// `HANYFELHOPARTICLE = OptionsFogResolution · 50`: how many clouds of each set are drawn and drift.
    fog: usize,
    twinkle: Twinkle,
    /// `futofeny`, the starmap's UI clock (+1 per UI tick) when pinned (`--futofeny`, the exe's value at a capture);
    /// else `time · 100`.
    pub futofeny: Option<f32>,
    /// The remaster's clock `game::acttime` (exe 0xFD1520, ms) in seconds, `sg3d_time.x` of the cloud material, when
    /// pinned (`--acttime`, the exe's value at a capture); else the frame time.
    pub acttime: Option<f32>,
    /// The battle screen's `starmaperosseg` (`starmapszarok`, spacewar.md §3.3, §7.4): the systems' glows × e, the
    /// planets and fleets hidden at 0 (`starmaphideflottak`, `starmaphalvanyit`).
    pub battle_fade: Option<f32>,
}

/// The background's layout: the far stars, `Clouds1`, `Clouds2`, `KodPamacsok`.
const FAR_STARS: usize = 2500;
const CLOUDS: usize = 105;
const CLOUDS1: usize = FAR_STARS;
const CLOUDS2: usize = CLOUDS1 + CLOUDS;
const FOG: usize = CLOUDS2 + CLOUDS;
/// `KodPamacsok`' members that keep a colour (spec §6.1).
const FOG_LIT: usize = 30;

/// `PulzaljKisCsillag` (exe 0x5a05a0, ELF 0x6e9d50): the far stars twinkle. Its tables, built at the first call.
struct Twinkle {
    rnd1: Vec<i32>,
    rnd2: Vec<i32>,
    sine: [i32; 256],
}

impl Twinkle {
    /// The exe's own approximations of π (3.1415, 3.141592) are part of the tables.
    #[allow(clippy::approx_constant)]
    fn new() -> Self {
        let sin = |x: f64| x.sin();
        let rnd1 = (0..FAR_STARS).map(|i| (((sin((i as f32 * 82.0) as f64) as f32 / 3.0 + 1.0) * 0.25 * 180.0) as f64 / 3.1415) as i32).collect();
        let rnd2 = (0..FAR_STARS).map(|i| (sin((i as f32 * 415.0) as f64) as f32 * 30.0 + 120.0) as i32).collect();
        let sine = std::array::from_fn(|j| (sin((j as f32 * 0.0078125) as f64 * 3.141592) * 90.0) as i32);
        Self { rnd1, rnd2, sine }
    }

    /// Far star `i`'s colour at UI clock `futofeny` (`starmaperosseg` 1): `0x646464` with the transparency
    /// `sine[((i + ⌊futofeny/10⌋)·rnd1[i]) & 0xff] + rnd2[i]`. The exe updates a quarter of the stars per frame
    /// (`melyikkiscsill`), so right after `⌊futofeny/10⌋` changes the other quarters still show the previous value;
    /// the port gives every star the current one.
    fn color(&self, i: usize, futofeny: f32) -> u32 {
        let k = (futofeny / 10.0) as i32;
        let t = self.sine[(self.rnd1[i].wrapping_mul(i as i32 + k) & 0xff) as usize] + self.rnd2[i];
        (t.wrapping_mul(0x100_0000) as u32) | 0x64_6464
    }
}

/// `PulzaljNagyCsillag` (exe 0x5a0810, ELF 0x6ea0e0): a seen system star's size `StarSize + sin(c + futofeny/4/13)/2.5`
/// (c = the planet's 0-based index), in f64, every frame; its colour is white (`0xffffff`), not the planet's `szin`.
pub fn star_pulse(index: usize, star_size: f32, futofeny: f32) -> f32 {
    (((futofeny as f64 * 0.25) / 13.0 + (index as f64 - 1.0)).sin() / 2.5 + star_size as f64) as f32
}

/// `OptionsFogResolution` from the user's options (2 when the file has none).
fn fog_particles() -> usize {
    let res = ig2_formats::options::Options::load().int("OptionsFogResolution").unwrap_or(2);
    (res.max(0) as usize * 50).min(CLOUDS)
}

/// `GalaxisGomolygas` (ELF 0x6d8ac0, exe 0x5999c0) at drift clock `clock`: every fifth UI tick the first `n` clouds
/// of both sets move and grow a little, in f64 written back to f32.
fn drift(bg: &mut [Particle], clock: i32, n: usize) {
    if clock % 5 != 0 {
        return;
    }
    let t = clock as f64;
    let step = |v: &mut f32, w: f64, k: f64| *v = (w / k + *v as f64) as f32;
    for i in 0..n.min(CLOUDS) {
        let k = i as f64;
        let p = &mut bg[CLOUDS1 + i];
        step(&mut p.h, (t / (k + 319.0) + k).cos(), 40.0);
        step(&mut p.p[0], (t / (k + 210.0) + k).sin(), 40.0);
        step(&mut p.p[1], (t / (k + 250.0) + k).sin(), 40.0);
        step(&mut p.size, (t / (k + 400.0) + k).cos(), 70.0);
    }
    for i in 0..n.min(CLOUDS) {
        let k = i as f64;
        let p = &mut bg[CLOUDS2 + i];
        step(&mut p.h, (t / (k + 279.0) + k).sin(), 9.0);
        step(&mut p.p[0], (t / (k + 210.0) + k).cos(), 9.0);
        step(&mut p.p[1], (t / (k + 150.0) + k).sin(), 9.0);
        step(&mut p.size, (t / (k + 220.0) + k).sin(), 20.0);
    }
}

/// A background particle (spec §6): galaxy point, height, size (galaxy units; 0 = a point), colour, texture set.
#[derive(Debug, Clone, Copy)]
struct Particle {
    h: f32,
    p: [f32; 2],
    size: f32,
    color: u32,
    /// 0 = point star, 1 = `felho3`, 2 = `felho2`.
    set: u8,
    /// The index in its set (the clouds past `HANYFELHOPARTICLE` aren't drawn).
    n: u16,
}

/// MSVC's `rand()` from `srand(seed)` (the exe's libc).
#[derive(Debug, Clone)]
pub(crate) struct MsvcRand(pub(crate) u32);

impl MsvcRand {
    pub(crate) fn rand(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
    /// `random(n)` (ELF 0x4f0200).
    pub(crate) fn random(&mut self, n: i32) -> i32 {
        if n > 0 {
            self.rand() % n
        } else {
            0
        }
    }
}

/// A point on the galaxy's spiral arms: (height, x, y) (`GenerateStarParticles`' arm recipe); `mid` draws what
/// the original draws between the position and the arm's turn.
/// The exe (0x599cf0) rounds the square root and the sine and cosine to f32 and mixes f32 and f64 steps (asm; the ELF
/// works in f64).
fn spiral<T>(r: &mut MsvcRand, mid: impl FnOnce(&mut MsvcRand) -> T) -> (f32, [f32; 2], T) {
    let (ax, ay) = if r.random(2) == 0 { (1.0f32, 0.0f32) } else { (0.0, 1.0) };
    let h0 = (r.random(30) - 15) as f32;
    let (a, b) = (r.random(600), r.random(50));
    let x = (b as f32 + (a - 300) as f32 * ax) - 25.0;
    let (a, b) = (r.random(600), r.random(50));
    let y = (b as f32 + (a - 300) as f32 * ay) - 25.0;
    let t = mid(r);
    let dist = ((y * y + x * x) as f64).sqrt() as f32;
    let turn0 = dist / 100.0;
    let h = (((1.0 - dist / (180000f64.sqrt() as f32)) as f64 + 0.3) * h0 as f64) as f32;
    let turn = ((r.random((turn0 * 100.0) as i32) as f64 / 100.0 - (turn0 * 0.5) as f64) / 2.5 + turn0 as f64) as f32;
    let (s, c) = ((turn as f64).sin() as f32, (turn as f64).cos() as f32);
    (h, [c * x - s * y, s * x + c * y], t)
}

/// `AddColor` (exe 0x590f60, ELF 0x6d8650): adds `rgb · (1 − d/√720900)^power` to every cloud of both sets at
/// distance `d` from `at` (h, x, y), each channel capped at 255 (f32 with the root and the power in f64).
fn add_color(clouds: &mut [Particle], at: [f32; 3], power: f32, rgb: [i32; 3]) {
    let far = 720900f64.sqrt() as f32;
    for p in clouds {
        let d = [at[0] - p.h, at[1] - p.p[0], at[2] - p.p[1]];
        let dist = ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f64).sqrt() as f32;
        let k = ((1.0 - dist / far) as f64).powf(power as f64) as f32;
        let ch = |shift: u32, add: i32| (((p.color >> shift) & 0xff) as f32 + add as f32 * k) as u32;
        let (r, g, b) = (ch(0, rgb[0]).min(255), ch(8, rgb[1]).min(255), ch(16, rgb[2]).min(255));
        p.color = 0xc800_0000 | b << 16 | g << 8 | r;
    }
}

/// `GenerateStarParticles` (ELF 0x6d8e90): the far stars, the clouds and the fog clumps from `srand(1)`.
fn background() -> Vec<Particle> {
    let mut r = MsvcRand(1);
    let grey = |v: i32| (v as u32) * 0x010101;
    let mut stars = vec![Particle { h: 0.0, p: [0.0; 2], size: 0.0, color: 0, set: 0, n: 0 }; FAR_STARS];
    for s in stars.iter_mut().take(1750) {
        let (h, p, v) = spiral(&mut r, |r| r.random(100));
        *s = Particle { h, p, color: grey(v), ..*s };
    }
    let ring = |r: &mut MsvcRand, rad: f32| {
        // In f64 (asm; the double literal is π itself).
        let a = ((r.random(400) as f64 / 400.0) * std::f64::consts::PI * 2.0) as f32;
        let (s, c) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
        [c * 0.0 - s * rad, c * rad + s * 0.0]
    };
    for s in stars.iter_mut().skip(1500) {
        let rad = (r.random(600) - 300) as f32;
        let p = ring(&mut r, rad);
        *s = Particle { h: 0.0, p, color: grey(r.random(100)), ..*s };
    }
    for s in stars.iter_mut().skip(1750) {
        let rad = (r.random(1800) - 900) as f32;
        let p = ring(&mut r, rad);
        let h = (r.random(600) - 300) as f32;
        *s = Particle { h, p, color: grey(r.random(100)), ..*s };
    }
    for s in stars.iter_mut().skip(2000) {
        let mut v = [(r.random(100) - 50) as f32, (r.random(100) - 50) as f32, (r.random(100) - 50) as f32];
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if len > 0.0 {
            v = v.map(|x| x / len);
        }
        let d = (r.random(500) + 900) as f32;
        *s = Particle { h: v[0] * d, p: [v[1] * d, v[2] * d], color: grey(r.random(100)), ..*s };
    }
    let mut out = stars;
    // `Clouds1` (felho3, sizes 5..10) and `Clouds2` (felho2, 12..31).
    for (set, base, span) in [(1u8, 5, 6), (2, 12, 20)] {
        for n in 0..CLOUDS as u16 {
            let (h, p, tint) = spiral(&mut r, |r| r.random(50) + 50);
            let size = (r.random(span) + base) as f32;
            let b = (r.random(100) + tint + 0x37) as u32;
            let (g, rr) = ((r.random(50) + 50) as u32, (r.random(50) + 50) as u32);
            out.push(Particle { h, p, size, color: 0xc800_0000 | b << 16 | g << 8 | rr, set, n });
        }
    }
    // The first five `Clouds2` move to new spiral points, each tinting every cloud near it (`AddColor`). The exe
    // passes the three draws right to left: the first is blue (the ELF's first is red).
    for i in 0..5 {
        let (h, p, ()) = spiral(&mut r, |_| ());
        out[CLOUDS2 + i].h = h;
        out[CLOUDS2 + i].p = p;
        let b = r.random(50);
        let g = r.random(50);
        let rr = r.random(50);
        add_color(&mut out[CLOUDS1..FOG], [h, p[0], p[1]], 25.0, [rr, g, b]);
    }
    // `KodPamacsok`: 30 fog clumps of 10 (felho2, sizes 25..49), far out of the galaxy plane: the ±675 spreads
    // are the height and x, the ±450 one y.
    for _ in 0..30 {
        let ch = (r.random(900) - 450) as f32 * 1.5;
        let cx = (r.random(900) - 450) as f32 * 1.5;
        let mut cy = r.random(300) as f32 * 1.5;
        if r.random(2) != 0 {
            cy = -cy;
        }
        let (br, bg, bb) = (r.random(100) + 50, r.random(100) + 50, r.random(100) + 50);
        for _ in 0..10 {
            let h = ch + r.random(100) as f32 - 50.0;
            let x = cx + r.random(100) as f32 - 50.0;
            let y = cy + r.random(100) as f32 - 50.0;
            let size = (r.random(25) + 25) as f32;
            let b = (bb + r.random(50)) as u32;
            let g = (bg + r.random(50)) as u32;
            let rr = (br + r.random(50)) as u32;
            let n = (out.len() - FOG) as u16;
            out.push(Particle { h, p: [x, y], size, color: 0xc800_0000 | b << 16 | g << 8 | rr, set: 2, n });
        }
    }
    // Both binaries copy the colour to the backup at `+0x48` of the clump's index, not the member's, and
    // `Init3dStarmap` then restores every colour from the backup: only members 0..29 keep theirs, the other 270 are
    // black (invisible in the additive pass; the exe's live records confirm it).
    for p in &mut out[FOG + FOG_LIT..] {
        p.color = 0;
    }
    out
}

/// Where a planet is drawn: its star's position plus its orbit (`rotate_stars` ELF 0x6e1a00, spec §3.3).
pub fn orbit_position(p: &PlanetView, clock: f32) -> [f32; 2] {
    let r = p.look.dist as f32 / 20.0 + 2.0;
    let t = p.look.speed as f32 / 100.0 + 0.1;
    let a = (clock / t) as f64;
    [p.pos[0] + (-(a.sin()) * r as f64) as f32, p.pos[1] + (a.cos() * r as f64) as f32]
}

impl View {
    pub fn new(s: &Snapshot, saved: &SavedView, size: (u32, u32), scale: f32) -> Self {
        let camera = Camera::from_save(saved, s, [size.0 as f32, size.1 as f32]);
        let generated = background();
        Self { camera, scale, hover: None, selected: None, reveal: false, star_scale: None, backdrop: Vec::new(), background: generated.clone(), generated, clouds: 0, fog: fog_particles(), twinkle: Twinkle::new(), futofeny: None, acttime: None, battle_fade: None }
    }

    /// One UI tick of the starmap (`FRISSIT_Starmap2` → `GalaxisGomolygas`).
    pub fn ui_tick(&mut self) {
        self.clouds += 1;
        drift(&mut self.background, self.clouds, self.fog);
    }

    /// The clouds as after `clock` UI ticks on the starmap (the exe's `GomolygasCounter`, `--clouds`).
    pub fn set_clouds(&mut self, clock: i32) {
        if clock < self.clouds {
            self.background.clone_from(&self.generated);
            self.clouds = 0;
        }
        while self.clouds < clock {
            self.ui_tick();
        }
    }

    pub fn resize(&mut self, size: (u32, u32), scale: f32) {
        self.camera.size = [size.0 as f32, size.1 as f32];
        self.scale = scale;
    }

    fn planets<'a>(&self, s: &'a Snapshot) -> impl Iterator<Item = &'a PlanetView> + use<'a> {
        let reveal = self.reveal;
        s.planets.iter().filter(move |p| p.known || (reveal && p.on_map()))
    }

    fn shown<'a>(&self, fleets: &'a [FleetView]) -> impl Iterator<Item = &'a FleetView> + use<'a> {
        let reveal = self.reveal;
        fleets.iter().filter(move |f| reveal || f.visible)
    }

    /// A fleet sprite's centre and half size (physical px), `None` behind the camera (spec §4.2).
    fn fleet_box(&self, f: &FleetView) -> Option<([f32; 2], [f32; 2])> {
        let (c, _) = self.camera.project(f.pos)?;
        let k = self.camera.k();
        Some(if f.flags & 0x20 != 0 {
            ([c[0] + 0.5 * k, c[1] + 0.5 * k], [7.5 * k, 5.5 * k])
        } else {
            ([c[0] + k, c[1] + k], [15.0 * k, 11.0 * k])
        })
    }

    /// A star glow's centre and half size (physical px).
    fn star_box(&self, p: &PlanetView) -> Option<([f32; 2], f32)> {
        let e = self.camera.eye(&self.camera.matrix(), 0.0, p.pos);
        if e[2] >= ZCLIP {
            return None;
        }
        let (c, px) = self.camera.project_eye(e)?;
        Some((c, (p.look.star_size >> 1) as f32 * px))
    }

    /// What's under the screen point `at` (fleets above planets, nearest first).
    pub fn pick(&self, prev: &Snapshot, curr: &Snapshot, alpha: f32, at: [f32; 2]) -> Option<Pick> {
        let fleets = curr.fleets_at(prev, alpha);
        let d2 = |c: [f32; 2]| (c[0] - at[0]).powi(2) + (c[1] - at[1]).powi(2);
        let fleet = self
            .shown(&fleets)
            .filter_map(|f| self.fleet_box(f).map(|b| (b, f.slot)))
            .filter(|((c, h), _)| (at[0] - c[0]).abs() <= h[0] && (at[1] - c[1]).abs() <= h[1])
            .map(|((c, _), slot)| (d2(c), Pick::Fleet(slot)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let planet = || {
            self.planets(curr)
                .filter_map(|p| self.star_box(p).map(|(c, r)| (c, r, p.index)))
                .filter(|&(c, r, _)| d2(c) <= (r * 0.5).max(8.0 * self.scale).powi(2))
                .map(|(c, _, i)| (d2(c), Pick::Planet(i)))
                .min_by(|a, b| a.0.total_cmp(&b.0))
        };
        fleet.or_else(planet).map(|(_, p)| p)
    }

    /// The frame for the state `alpha` of the way from `prev` to `curr`; `time` (seconds) drives the UI-tick
    /// animations (planet spin, fleet sprites, the selection light).
    pub fn build(&self, a: &Assets, prev: &Snapshot, curr: &Snapshot, alpha: f32, time: f32) -> Frame {
        let mut f = Frame::new([0.0, 0.0, 0.0, 1.0]);
        let cam = &self.camera;
        let m = cam.matrix();
        let k = cam.k();
        let ui = self.futofeny.unwrap_or(time * UI_HZ);
        self.sky(&mut f, a, time);
        self.background(&mut f, a, &m, ui);
        for &(t, b, i) in &self.backdrop {
            f.push(t, b, i);
        }
        self.radar(&mut f, a, &m, &curr.radar);

        // System stars: `n{(surface − 1) % 8 + 1}` glows in the star colour (spec §3.2).
        for p in self.planets(curr) {
            let Some((c, _)) = self.star_box(p) else { continue };
            let px = cam.project(p.pos).map_or(0.0, |(_, px)| px);
            let mut half = star_pulse(p.index, (p.look.star_size >> 1) as f32, ui) * px;
            if let Some((_, k)) = self.star_scale.filter(|s| s.0 == p.index) {
                half *= k;
                if half <= 0.0 {
                    continue;
                }
            }
            let tex = a.stars[(p.surface.max(1) as usize - 1) % 8].unwrap_or(a.white);
            let kind = if tex == a.white { kind::GLOW } else { kind::SPRITE };
            let e = self.battle_fade.unwrap_or(1.0);
            if e <= 0.0 {
                continue;
            }
            let col = particle_color(0xff_ffff).map(|x| x * e);
            f.push(tex, Blend::GammaAdditive, Instance::new(kind, c, [half; 2], [col[0], col[1], col[2], 1.0]));
        }

        // Planets on their orbits as meshes, spinning (`ShowStarmap`: `RotateObj(0, 0, PlanetRotate · spin · 6)`,
        // `PlanetRotate` +1/150 per UI tick like `futofeny`; spec §3.3), the long info's planet with its clouds
        // (`HideClouds`). The particle pass (`DrawStarmapParticles`, a scene callback) comes first: the planets
        // cover it.
        let clock = prev.orbit + (curr.orbit - prev.orbit) * alpha.clamp(0.0, 1.0);
        let planet_rotate = ui / 150.0;
        let clouds = self.star_scale.map(|s| s.0);
        let eye = cam.eye_world();
        f.mark_late();
        let hidden = self.battle_fade.is_some_and(|e| e <= 0.0);
        for p in self.planets(curr).filter(|_| !hidden) {
            let at = orbit_position(p, clock);
            let spin = p.look.spin as f32 / 100.0;
            let Frame { scene, late, .. } = &mut f;
            if let (true, Some(scene)) = (a.planet_meshes.ready(p.surface), scene.as_mut()) {
                a.planet_meshes.add_planet(scene, late, p.surface, p.look.size, planet_rotate * spin * 6.0, [0.0, at[0], at[1]], &eye);
                if clouds == Some(p.index) {
                    a.planet_meshes.add_clouds(scene, late, p.look.size, -planet_rotate * spin * 2.0, [0.0, at[0], at[1]], &eye);
                }
                continue;
            }
            // Without the meshes: a shaded sprite of the surface map.
            let Some((c, px)) = cam.project(at) else { continue };
            let half = (1024.0 * 0.846 * (p.look.size as f32 / 200000.0 + 0.002) / 8.0 * px).max(0.75 * self.scale);
            if c[0] < -half || c[1] < -half || c[0] > cam.size[0] + half || c[1] > cam.size[1] + half {
                continue;
            }
            let turn = planet_rotate * spin * 6.0 / std::f32::consts::TAU;
            match a.planets.get(p.surface as usize).copied().flatten() {
                Some(t) => f.push(
                    t,
                    Blend::Alpha,
                    Instance { param: [turn, 0.6], uv: [0.0, 0.0, 1.0, 0.5], ..Instance::new(kind::SPHERE, c, [half; 2], [1.0; 4]) },
                ),
                None => f.push(a.white, Blend::Alpha, Instance::new(kind::RING, c, [half; 2], [0.5, 0.5, 0.6, 1.0])),
            }
        }

        // Routes of the human's fleets (`Show_Flottak3d`, spec §4.3).
        let fleets = if hidden { Vec::new() } else { curr.fleets_at(prev, alpha) };
        for fl in self.shown(&fleets).filter(|f| f.order > 0 && f.flags & 0x20 == 0 && (self.reveal || f.owner == curr.human)) {
            let (Some(t), Some(p)) = (fl.target, cam.to_screen(fl.pos)) else { continue };
            let Some(q) = cam.to_screen(t) else { continue };
            let color = if fl.order == 2 { [1.0, 0.0, 0.0, 0.5] } else { [0x46 as f32 / 255.0, 0x4b as f32 / 255.0, 0x5f as f32 / 255.0, 0.5] };
            f.push(a.white, Blend::Alpha, Instance::line(p, q, 3.0 * k, color));
            let rot = (q[1] - p[1]).atan2(q[0] - p[0]);
            f.push(a.white, Blend::Alpha, Instance { rot, ..Instance::new(kind::MARKER, q, [5.0 * k; 2], color) });
        }

        // The selection ring (spec §5).
        if let Some(sel) = self.selected {
            let (centre, color) = match sel {
                Pick::Planet(i) => (curr.planets.iter().find(|p| p.index == i).map(|p| p.pos), 0xff8020u32),
                Pick::Fleet(i) => (fleets.iter().find(|p| p.slot == i).map(|p| p.pos), 0x00ffff),
            };
            if let (Some(c), Some(t)) = (centre, a.select) {
                let d = cam.distance();
                let radius = 10.0 / (FOV / d).sqrt();
                let size_div = (0.3 * FOV / d).sqrt();
                let head = (ui / 3.0) as i32;
                for i in 0..40 {
                    let lag = (head - i).rem_euclid(20);
                    let size = (2.0 - 0.2 * lag as f32).max(0.4) / size_div;
                    let ang = i as f32 * 0.15707964;
                    let e = cam.eye(&m, 0.0, [c[0] + ang.cos() * radius, c[1] + ang.sin() * radius]);
                    if e[2] >= ZCLIP {
                        continue;
                    }
                    if let Some((s, px)) = cam.project_eye(e) {
                        f.push(t, Blend::GammaAdditive, Instance::new(kind::SPRITE, s, [size * px; 2], particle_color(color)));
                    }
                }
            }
        }

        // Fleets: the `star_pl` ship of the owner's column, turning (spec §4.2).
        for fl in self.shown(&fleets) {
            let Some((c, half)) = self.fleet_box(fl) else { continue };
            let row = (ui as i64 / 5 + fl.slot as i64 * 5).rem_euclid(48) as u32;
            let small = fl.flags & 0x20 != 0;
            let sheet = if small { a.small_fleets } else { a.fleets };
            let cols = if small { 16 } else { 17 };
            let dim = if fl.visible { 1.0 } else { 0.5 };
            match sheet {
                Some(s) => {
                    let frame = if fl.flags & 0x4000 != 0 && !small { 17 } else { fl.owner.max(1) as u32 } + row * cols;
                    let (col, row) = ((frame - 1) % cols, ((frame - 1) / cols) % 48);
                    f.push(s.tex, Blend::GammaAlpha, Instance { uv: s.uv(col, row), ..Instance::new(kind::SPRITE, c, half, [dim, dim, dim, 1.0]) });
                }
                None => f.push(a.white, Blend::Alpha, Instance::new(kind::MARKER, c, [half[1]; 2], a.empire(fl.owner, dim))),
            }
        }

        // Hover (the client's own cue; the original has none).
        if let Some(h) = self.hover.filter(|h| Some(*h) != self.selected) {
            let at = match h {
                Pick::Planet(i) => curr.planets.iter().find(|q| q.index == i).and_then(|p| self.star_box(p)).map(|(c, r)| (c, (r * 0.5).max(8.0 * self.scale))),
                Pick::Fleet(i) => fleets.iter().find(|q| q.slot == i).and_then(|q| self.fleet_box(q)).map(|(c, h)| (c, h[0])),
            };
            if let Some((c, r)) = at {
                let ring = r + 3.0 * self.scale;
                f.push(a.white, Blend::Alpha, Instance { param: [(ring - 1.0 * self.scale) / ring, 0.0], ..Instance::new(kind::RING, c, [ring; 2], [1.0, 1.0, 1.0, 0.6]) });
            }
        }
        f
    }

    /// `Show_Stars2d3d` and `Show_Flottak2d`'s names (re/screens/hud.md §4.2, §4.3): an owned planet's name under
    /// its system, `sqrt(Zoom/5)` lower, faded from zoom 590, blinking with `+0xba`; a fleet's name (`+0x52` bit
    /// 11) 9 below it, both size 1, centred, in the owner's colour `DDColor[owner + 15]`.
    #[allow(clippy::too_many_arguments)]
    pub fn names(&self, f: &mut Frame, font: &crate::font::Font, a: &Assets, prev: &Snapshot, curr: &Snapshot, alpha: f32, time: f32) {
        let cam = &self.camera;
        let v = crate::font::Virtual::new(cam.size);
        let virt = |p: [f32; 2]| ((p[0] - v.x0) / v.k, p[1] / v.k);
        let col = |owner: u8| a.palette.get(owner as usize + 15).copied().unwrap_or([1.0; 3]);
        let zoom_fade = if cam.zoom >= 590.0 { (100.0 - (cam.zoom - 590.0) / 9.1 * 100.0) as i32 as f32 / 100.0 } else { 1.0 };
        let below = (cam.zoom / 5.0).sqrt() as i32 as f32;
        for p in self.planets(curr).filter(|p| p.on_map() && (p.owner != 0 || p.blink)) {
            let Some((c, _)) = cam.project(p.pos) else { continue };
            let (x, y) = virt(c);
            let k = zoom_fade.max(0.0) * if p.blink { crate::hud::blink(time, 300.0) } else { 1.0 };
            let name: Vec<u8> = p.name.chars().map(|c| c as u32 as u8).collect();
            font.text(f, &v, &name, x.trunc(), (y + below).trunc(), 0x14, 1, crate::font::Align::Centre, col(p.owner).map(|c| c * k), None);
        }
        for fl in self.shown(&curr.fleets_at(prev, alpha)).filter(|fl| fl.flags & 0x800 != 0) {
            let Some((c, _)) = cam.project(fl.pos) else { continue };
            let (x, y) = virt(c);
            // The shadow in colour 0 first (black: nothing to add), then the name.
            font.text(f, &v, &fl.name, (x + 1.0).trunc(), (y + 9.0).trunc(), 0x14, 1, crate::font::Align::Centre, col(fl.owner), None);
        }
    }

    /// The generated galaxy: point stars, clouds and fog clumps, additive (spec §6).
    fn background(&self, f: &mut Frame, a: &Assets, m: &[[f32; 3]; 3], futofeny: f32) {
        let cam = &self.camera;
        for (i, p) in self.background.iter().enumerate() {
            // Black particles add nothing (`Particle` skips textured ones with colour 0); `Init3dStarmap` hands the
            // cloud sets `HANYFELHOPARTICLE` as their counts.
            if p.color == 0 || ((CLOUDS1..FOG).contains(&i) && p.n as usize >= self.fog) {
                continue;
            }
            let e = cam.eye(m, p.h, p.p);
            if e[2] >= ZCLIP {
                continue;
            }
            let Some((c, px)) = cam.project_eye(e) else { continue };
            let color = particle_color(if i < FAR_STARS { self.twinkle.color(i, futofeny) } else { p.color });
            match p.set {
                0 => {
                    if c[0] < 0.0 || c[1] < 0.0 || c[0] > cam.size[0] || c[1] > cam.size[1] {
                        continue;
                    }
                    f.push(a.white, Blend::GammaAdditive, Instance::new(kind::SPRITE, c, [0.5 * cam.k(); 2], color));
                }
                s => {
                    let half = p.size * px;
                    if c[0] < -half || c[1] < -half || c[0] > cam.size[0] + half || c[1] > cam.size[1] + half {
                        continue;
                    }
                    if let Some(t) = a.clouds[s as usize - 1] {
                        f.push(t, Blend::GammaAdditive, Instance::new(kind::SPRITE, c, [half; 2], color));
                    }
                }
            }
        }
    }

    /// The sky sphere (spec §6.5) through the mesh pass, lit by the scene's ambient 0x404040 and parallel light
    /// 0x808080 from (10, 10, −10), in the gamma pass like the remaster's (re/screens/render.md §2).
    fn sky(&self, f: &mut Frame, a: &Assets, time: f32) {
        if a.sky.is_empty() {
            return;
        }
        let (amb, sun) = (0x40 as f32 / 255.0, 0x80 as f32 / 255.0);
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        for (verts, tex) in &a.sky {
            let start = dynamic.len() as u32;
            dynamic.extend_from_slice(verts);
            draws.push(crate::render3d::MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex: *tex, glass: false, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
        }
        let l = crate::scene::normalise([-1.0, -1.0, 1.0]);
        f.scene = Some(crate::render3d::Scene3d {
            gamma: true,
            shadow: None,
            uniforms: crate::render3d::SceneUniforms {
                view: self.camera.clip_matrix(),
                // `w`: `sg3d_time.x` for the planets' cloud material.
                eye: [0.0, 0.0, 0.0, self.acttime.unwrap_or(time)],
                light: [l[0], l[1], l[2], 0.0],
                ambient: [amb; 4],
                sun: [sun; 4],
                fog: [0.0; 4],
                fog_range: [1.0e9, 1.0e9, 0.0, 1.0e9],
            },
            draws,
            dynamic,
        });
    }

    /// The radar ranges (spec §6.3): the discs' union in `rrange.bmp`'s colour (planets) or `rrange2.bmp`'s
    /// (fleets), added at 10 % in gamma space (`SetObjTransparency(1 − 0.1·starmaperosseg·fade)`, the fade from
    /// zoom 580 to 590), then the `radar.bmp` dots (`FStars` set 2, half size 1.3, red `0xff` or blue `0xc04000`).
    fn radar(&self, f: &mut Frame, a: &Assets, m: &[[f32; 3]; 3], rings: &crate::starmap_radar::Rings) {
        let cam = &self.camera;
        let fade = (1.0 - (cam.zoom - 580.0) / 10.0).clamp(0.0, 1.0);
        let full = |f: &mut Frame, blend: Blend, color: [f32; 4]| {
            f.push(a.white, blend, Instance::new(kind::SPRITE, [cam.size[0] / 2.0, cam.size[1] / 2.0], [cam.size[0] / 2.0 + 1.0, cam.size[1] / 2.0 + 1.0], color));
        };
        // `rrange.xvb`: a fan of 15 triangles around a unit disc's centre.
        for fleet in [false, true] {
            let discs: Vec<_> = rings.discs.iter().filter(|d| d.2 == fleet && d.1 > 0.0).collect();
            if discs.is_empty() || fade <= 0.0 {
                continue;
            }
            full(f, Blend::MaskClear, [0.0; 4]);
            for &&(centre, r, _) in &discs {
                let eye = |k: usize| {
                    let t = k as f32 * std::f32::consts::TAU / 15.0;
                    let (sn, co) = ((t as f64).sin() as f32, (t as f64).cos() as f32);
                    cam.eye(m, 0.0, [centre[0] + co * r, centre[1] + sn * r])
                };
                let mid = cam.eye(m, 0.0, centre);
                let Some((c0, _)) = (mid[2] < ZCLIP).then(|| cam.project_eye(mid)).flatten() else { continue };
                for k in 0..15 {
                    let (e1, e2) = (eye(k), eye(k + 1));
                    if e1[2] >= ZCLIP || e2[2] >= ZCLIP {
                        continue;
                    }
                    if let (Some((c1, _)), Some((c2, _))) = (cam.project_eye(e1), cam.project_eye(e2)) {
                        f.push(a.white, Blend::MaskMax, Instance::tri(c0, c1, c2, [1.0, 1.0, 1.0, 0.1 * fade]));
                    }
                }
            }
            let [r, g, b] = a.radar_fill[fleet as usize];
            full(f, Blend::MaskAdd, [r, g, b, 1.0]);
            full(f, Blend::MaskRestore, [1.0; 4]);
        }
        let Some(t) = a.radar_dot else { return };
        for d in &rings.dots {
            let e = cam.eye(m, 0.0, d.pos);
            if e[2] >= ZCLIP {
                continue;
            }
            let Some((c, px)) = cam.project_eye(e) else { continue };
            let color = if d.fleet { particle_color(0xc04000) } else { particle_color(0xff) };
            f.push(t, Blend::GammaAdditive, Instance::new(kind::SPRITE, c, [1.3 * px; 2], color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::PlanetLook;

    fn planet(index: usize, x: f32, y: f32) -> PlanetView {
        PlanetView { index, pos: [x, y], owner: 0, surface: 1, name: String::new(), known: true, blink: false, friendly: false, look: PlanetLook { star_size: 8, ..Default::default() } }
    }

    fn cam(rotate: f32, pitch: f32) -> Camera {
        Camera { x: 10.0, y: -20.0, rotate, pitch, zoom: 300.0, size: [1920.0, 1080.0], look: None }
    }

    #[test]
    fn the_pivot_is_at_the_virtual_centre_and_galaxy_x_points_left() {
        // Top view, no yaw: the pivot (x, −y) lands on virtual (320, 250); +x goes left, +y down (spec §2.2).
        let c = cam(0.0, -std::f32::consts::FRAC_PI_2);
        let (p, px) = c.project([10.0, 20.0]).unwrap();
        assert!((p[0] - 960.0).abs() < 1e-3 && (p[1] - 250.0 * 1080.0 / 480.0).abs() < 1e-2);
        assert!((px - 600.0 / 300.0 * 1080.0 / 480.0).abs() < 1e-3);
        let right = c.to_screen([11.0, 20.0]).unwrap();
        let down = c.to_screen([10.0, 21.0]).unwrap();
        assert!(right[0] < p[0] && (right[1] - p[1]).abs() < 1e-2);
        assert!(down[1] > p[1] && (down[0] - p[0]).abs() < 1e-2);
    }

    #[test]
    fn unprojection_inverts_projection() {
        for (r, p) in [(0.0, -0.95), (1.3, -0.4), (4.0, -1.5)] {
            let c = cam(r, p);
            for w in [[10.0, 20.0], [-40.0, 70.0], [100.0, -5.0]] {
                let s = c.to_screen(w).unwrap();
                let back = c.to_world(s).unwrap();
                assert!((back[0] - w[0]).abs() < 1e-2 && (back[1] - w[1]).abs() < 1e-2, "{r} {p} {w:?} → {back:?}");
            }
        }
    }

    #[test]
    fn pan_keeps_the_grabbed_point_and_the_pan_square() {
        let mut c = cam(0.7, -0.95);
        let (from, to) = ([900.0, 500.0], [1000.0, 560.0]);
        let w = c.to_world(from).unwrap();
        c.pan(from, to);
        let after = c.to_world(to).unwrap();
        assert!((after[0] - w[0]).abs() < 1e-2 && (after[1] - w[1]).abs() < 1e-2);
        c.pan([0.0, 1000.0], [1900.0, 0.0]);
        assert!(c.x.abs() <= PAN_LIMIT && c.y.abs() <= PAN_LIMIT);
    }

    #[test]
    fn zoom_steps_follow_the_keyboard_and_clamp() {
        let mut c = cam(0.0, -0.95);
        c.zoom_steps(1.0);
        assert_eq!(c.zoom, 300.0 + 340.0 / 50.0);
        c.zoom_steps(-1000.0);
        assert_eq!(c.zoom, 2.0);
        c.tilt(1000.0);
        assert_eq!(c.pitch, -0.2);
    }

    #[test]
    fn side_view_frames_the_known_planets() {
        // `StarmapSetSideView`: centre of the box, zoom from the half extents.
        let s = Snapshot { planets: vec![planet(1, -100.0, -60.0), planet(2, 100.0, 60.0)], ..Default::default() };
        let c = Camera::side_view(&s, [1920.0, 1080.0]);
        assert_eq!((c.x, c.y, c.pitch), (0.0, 0.0, -0.95));
        let zx = (100.0f32 * 600.0 / 320.0 - 600.0).abs();
        let zy = (60.0f32 * 600.0 / 240.0 - 600.0).abs();
        assert_eq!(c.zoom, zx.min(zy) / 1.5);
    }

    #[test]
    fn planets_orbit_their_star() {
        let mut p = planet(1, 50.0, 50.0);
        p.look.dist = 60;
        p.look.speed = 90;
        // r = 60/20 + 2 = 5; at K = 0 the planet sits at +y.
        assert_eq!(orbit_position(&p, 0.0), [50.0, 55.0]);
        let q = orbit_position(&p, 1.0);
        assert!(((q[0] - 50.0).powi(2) + (q[1] - 50.0).powi(2) - 25.0).abs() < 1e-3);
    }

    #[test]
    fn particle_colours_use_the_inverted_alpha() {
        assert_eq!(particle_color(0x0000_00ff)[0], 255.0 / 256.0);
        let c = particle_color(0xc800_0000 | 100);
        assert!((c[0] - 100.0 * 55.0 / 256.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn the_background_is_the_originals_count() {
        let b = background();
        assert_eq!(b.len(), 2500 + 210 + 300);
        assert!(b[..2500].iter().all(|p| p.set == 0 && p.color & 0xff < 100));
        // Shell stars sit 900..1400 out.
        let r = |p: &Particle| (p.h * p.h + p.p[0] * p.p[0] + p.p[1] * p.p[1]).sqrt();
        assert!(b[2000..2500].iter().all(|p| (899.0..1400.0).contains(&r(p))));
    }

    /// The exe's live particle records (`run_screen.py --particles`, `_local/traces/starmap-particles.json`): the
    /// clouds after its drift clock and the fog clumps' colours, bit for bit.
    #[test]
    fn the_background_matches_the_exes_records() {
        let dir = std::env::var("IG2_TRACE_DIR").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../_local/traces").into());
        let Ok(text) = std::fs::read_to_string(format!("{dir}/starmap-particles.json")) else {
            eprintln!("skipped: no starmap-particles.json");
            return;
        };
        let d: serde_json::Value = serde_json::from_str(&text).unwrap();
        let clock = d["clouds"].as_i64().unwrap() as i32;
        let fog = d["hany"].as_i64().unwrap() as usize;
        let mut bg = background();
        for t in 1..=clock {
            drift(&mut bg, t, fog);
        }
        let hex = |s: &str| (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect::<Vec<u8>>();
        let f = |r: &[u8], o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        // The quarter `melyikkiscsill − 1` was updated at the grab's `futofeny`, the others up to three frames earlier.
        let futofeny = d["futofeny"].as_f64().unwrap() as f32;
        let kis = d["kis"].as_i64().unwrap() as usize;
        let stars = hex(d["fstars"][0]["recs"].as_str().unwrap());
        let tw = Twinkle::new();
        for (i, p) in bg.iter_mut().enumerate().take(FAR_STARS) {
            let exe = u32::from_le_bytes(stars[i * 0x4c + 0xc..i * 0x4c + 0x10].try_into().unwrap());
            let back = (kis + 3 - i % 4) % 4;
            p.color = (0..=back as i32 * 2).map(|d| tw.color(i, futofeny - d as f32)).find(|&c| c == exe).unwrap_or(tw.color(i, futofeny));
        }
        let mut bad = 0;
        for (set, first, n) in [(0, 0, FAR_STARS), (8, CLOUDS1, fog), (9, CLOUDS2, fog), (10, FOG, 300)] {
            let recs = hex(d["fstars"][set]["recs"].as_str().unwrap());
            for (i, r) in recs.chunks_exact(0x4c).take(n).enumerate() {
                let p = &bg[first + i];
                let (want, got) = ([f(r, 0), f(r, 4), f(r, 8), f(r, 0x14)], [p.h, p.p[0], p.p[1], p.size]);
                let color = u32::from_le_bytes(r[0xc..0x10].try_into().unwrap());
                if want.map(f32::to_bits) != got.map(f32::to_bits) || color != p.color {
                    if bad < 10 {
                        eprintln!("set {set} #{i}: exe {want:?} {color:08x}, port {got:?} {:08x}", p.color);
                    }
                    bad += 1;
                }
            }
        }
        assert_eq!(bad, 0);
    }

    #[test]
    fn fleets_are_picked_before_planets() {
        let mut s = Snapshot { planets: vec![planet(1, 0.0, 0.0), planet(2, 50.0, 0.0)], ..Default::default() };
        s.fleets.push(FleetView { slot: 4, pos: [0.5, 0.0], owner: 6, visible: true, target: None, order: 0, flags: 0, name: Vec::new(), ships: 1, story_id: 0, kind: 0, target_index: 0 });
        let saved = SavedView { x: 25.0, y: 0.0, rotate: 0.0, pitch: -1.5, zoom: 400.0, orbit: 0.0 };
        let v = View::new(&s, &saved, (1000, 1000), 1.0);
        let at = |p: [f32; 2]| v.pick(&s, &s, 1.0, v.camera.to_screen(p).unwrap());
        assert_eq!(at([0.5, 0.0]), Some(Pick::Fleet(4)));
        assert_eq!(at([50.0, 0.0]), Some(Pick::Planet(2)));
        assert_eq!(at([25.0, 30.0]), None);
    }
}


