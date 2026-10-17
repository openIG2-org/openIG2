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
    white: TexId,
    select: Option<TexId>,
    planets: [Option<TexId>; SURFACES],
    /// `textures/starmap/n1..n8`: the system stars.
    stars: [Option<TexId>; 8],
    /// `felho3`, `felho2`: the galaxy clouds.
    clouds: [Option<TexId>; 2],
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
        Self { white, select, planets, stars, clouds, fleets, small_fleets, palette }
    }

    /// Empire `e`'s colour, `DDColor[e + 15]` (spec §7).
    fn empire(&self, owner: u8, a: f32) -> [f32; 4] {
        let [r, g, b] = self.palette.get(owner as usize + 15).copied().unwrap_or([0.6; 3]);
        [r, g, b, a]
    }
}

/// `DDColor` from the user's `ig2.exe` (never shipped: ADR 0004).
pub(crate) fn read_palette(install: &Install) -> Result<Vec<[f32; 3]>, String> {
    let exe = std::fs::read(install.root().join("ig2.exe")).map_err(|e| e.to_string())?;
    let img = ig2_rules::PeImage::parse(&exe).map_err(|e| e.to_string())?;
    let b = img.read(DD_COLOR, 4 * DD_COLORS).map_err(|e| e.to_string())?;
    Ok(b.chunks_exact(4).map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]).collect())
}

/// A `0xTTBBGGRR` particle colour: RGB × (255 − TT)/256 (`Particle` ELF 0x5f4ff0).
fn particle_color(c: u32) -> [f32; 4] {
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
}

/// `CreateMatrix(α, β, γ)` (ELF 0x451ee0), f32 sines and cosines like the original.
fn create_matrix(a: f32, b: f32, c: f32) -> [[f32; 3]; 3] {
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
        let mut c = Self { x: 0.0, y: 0.0, rotate: 0.0, pitch: -0.95, zoom: 2.0, size };
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
        Self { x: v.x, y: v.y, rotate, pitch: v.pitch, zoom: v.zoom, size }
    }

    fn matrix(&self) -> [[f32; 3]; 3] {
        create_matrix(self.rotate, self.pitch, std::f32::consts::FRAC_PI_2)
    }
    fn distance(&self) -> f32 {
        600.0 - self.zoom
    }
    /// Physical pixels per virtual pixel.
    pub fn k(&self) -> f32 {
        self.size[1] / VIRTUAL_H
    }

    /// Camera space of the galaxy point `p` at height `h` (`smRotatePoint`).
    fn eye(&self, m: &[[f32; 3]; 3], h: f32, p: [f32; 2]) -> [f32; 3] {
        let d = [h, p[0] - self.x, p[1] + self.y];
        let r = |i: usize| m[i][0] * d[0] + m[i][1] * d[1] + m[i][2] * d[2];
        [r(0), r(1), r(2) - self.distance()]
    }

    /// Virtual → physical pixels.
    fn phys(&self, v: [f32; 2]) -> [f32; 2] {
        let k = self.k();
        [self.size[0] / 2.0 + (v[0] - CENTER[0]) * k, v[1] * k]
    }

    /// Screen position and physical pixels per galaxy unit there, when in front of the camera.
    fn project_eye(&self, e: [f32; 3]) -> Option<([f32; 2], f32)> {
        if e[2] >= 0.0 {
            return None;
        }
        let f = FOV / -e[2];
        Some((self.phys([e[0] * f + CENTER[0], e[1] * f + CENTER[1]]), f * self.k()))
    }

    pub fn project(&self, p: [f32; 2]) -> Option<([f32; 2], f32)> {
        self.project_eye(self.eye(&self.matrix(), 0.0, p))
    }

    pub fn to_screen(self, p: [f32; 2]) -> Option<[f32; 2]> {
        self.project(p).map(|(s, _)| s)
    }

    /// The galaxy point (height 0) under the screen point `s`, if the ray meets the galaxy plane.
    pub fn to_world(self, s: [f32; 2]) -> Option<[f32; 2]> {
        let k = self.k();
        let v = [(s[0] - self.size[0] / 2.0) / k + CENTER[0], s[1] / k];
        let dir = [(v[0] - CENTER[0]) / FOV, (v[1] - CENTER[1]) / FOV, -1.0];
        let m = self.matrix();
        // d = Mᵀ·(e + (0, 0, D)): the eye sits at e = 0.
        let back = |e: [f32; 3]| -> [f32; 3] { std::array::from_fn(|j| m[0][j] * e[0] + m[1][j] * e[1] + m[2][j] * e[2]) };
        let o = back([0.0, 0.0, self.distance()]);
        let r = back(dir);
        if r[0].abs() < 1e-9 {
            return None;
        }
        let t = -o[0] / r[0];
        (t > 0.0).then(|| [o[1] + t * r[1] + self.x, o[2] + t * r[2] - self.y])
    }

    /// A galaxy point on the 640 × 480 virtual screen (`smRotatePoint` + `TransformPoint`), when in front.
    pub fn virtual_point(&self, p: [f32; 2]) -> Option<[f32; 2]> {
        let e = self.eye(&self.matrix(), 0.0, p);
        if e[2] >= 0.0 {
            return None;
        }
        let f = FOV / -e[2];
        Some([e[0] * f + CENTER[0], e[1] * f + CENTER[1]])
    }

    /// Physical → virtual pixels.
    pub fn virtual_of(&self, s: [f32; 2]) -> [f32; 2] {
        let k = self.k();
        [(s[0] - self.size[0] / 2.0) / k + CENTER[0], s[1] / k]
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
    background: Vec<Particle>,
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
}

/// MSVC's `rand()` from `srand(seed)` (the exe's libc).
struct MsvcRand(u32);

impl MsvcRand {
    fn rand(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
    /// `random(n)` (ELF 0x4f0200).
    fn random(&mut self, n: i32) -> i32 {
        if n > 0 {
            self.rand() % n
        } else {
            0
        }
    }
}

/// A point on the galaxy's spiral arms: (height, x, y) (`GenerateStarParticles`' arm recipe); `mid` draws what
/// the original draws between the position and the arm's turn.
fn spiral<T>(r: &mut MsvcRand, mid: impl FnOnce(&mut MsvcRand) -> T) -> (f32, [f32; 2], T) {
    let (ax, ay) = if r.random(2) == 0 { (1.0f32, 0.0f32) } else { (0.0, 1.0) };
    let h0 = (r.random(30) - 15) as f32;
    let (a, b) = (r.random(600), r.random(50));
    let x = (ax * (a - 300) as f32 + b as f32) - 25.0;
    let (a, b) = (r.random(600), r.random(50));
    let y = (ay * (a - 300) as f32 + b as f32) - 25.0;
    let t = mid(r);
    let dist = ((x * x + y * y) as f64).sqrt();
    let turn0 = (dist / 100.0) as f32;
    let h = (h0 as f64 * ((1.0 - dist / 180000f64.sqrt()) + 0.3)) as f32;
    let turn = turn0 + (r.random((turn0 * 100.0) as i32) as f32 / 100.0 - turn0 / 2.0) / 2.5;
    let (s, c) = ((turn as f64).sin(), (turn as f64).cos());
    (h, [(c * x as f64 - s * y as f64) as f32, (c * y as f64 + s * x as f64) as f32], t)
}

/// `GenerateStarParticles` (ELF 0x6d8e90): the far stars, the clouds and the fog clumps from `srand(1)`.
fn background() -> Vec<Particle> {
    let mut r = MsvcRand(1);
    let grey = |v: i32| (v as u32) * 0x010101;
    let mut stars = vec![Particle { h: 0.0, p: [0.0; 2], size: 0.0, color: 0, set: 0 }; 2500];
    for s in stars.iter_mut().take(1750) {
        let (h, p, v) = spiral(&mut r, |r| r.random(100));
        *s = Particle { h, p, color: grey(v), ..*s };
    }
    let ring = |r: &mut MsvcRand, rad: f32| {
        let a = (r.random(400) as f32 / 400.0) * std::f32::consts::PI * 2.0;
        let (s, c) = ((a as f64).sin(), (a as f64).cos());
        [(-s * rad as f64) as f32, (c * rad as f64) as f32]
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
        for _ in 0..105 {
            let (h, p, tint) = spiral(&mut r, |r| r.random(50) + 50);
            let size = (r.random(span) + base) as f32;
            let b = (r.random(100) + tint + 0x37) as u32;
            let (g, rr) = ((r.random(50) + 50) as u32, (r.random(50) + 50) as u32);
            out.push(Particle { h, p, size, color: 0xc800_0000 | b << 16 | g << 8 | rr, set });
        }
    }
    // The first five `Clouds2` are moved again for `AddColor` (not drawn differently here).
    let first = out.len() - 105;
    for i in 0..5 {
        let (h, p, ()) = spiral(&mut r, |_| ());
        out[first + i].h = h;
        out[first + i].p = p;
        for _ in 0..3 {
            r.random(50);
        }
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
            out.push(Particle { h, p: [x, y], size, color: 0xc800_0000 | b << 16 | g << 8 | rr, set: 2 });
        }
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

/// A planet sphere's radius in galaxy units [inf] (spec §3.3).
fn planet_radius(p: &PlanetView) -> f32 {
    1.1327 * (p.look.size as f32 / 1600.0 + 0.25)
}

impl View {
    pub fn new(s: &Snapshot, saved: &SavedView, size: (u32, u32), scale: f32) -> Self {
        let camera = Camera::from_save(saved, s, [size.0 as f32, size.1 as f32]);
        Self { camera, scale, hover: None, selected: None, reveal: false, background: background() }
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
        let ui = time * UI_HZ;
        self.background(&mut f, a, &m);

        // System stars: `n{(surface − 1) % 8 + 1}` glows in the star colour (spec §3.2).
        for p in self.planets(curr) {
            let Some((c, half)) = self.star_box(p) else { continue };
            let tex = a.stars[(p.surface.max(1) as usize - 1) % 8].unwrap_or(a.white);
            let kind = if tex == a.white { kind::GLOW } else { kind::SPRITE };
            f.push(tex, Blend::Additive, Instance::new(kind, c, [half; 2], particle_color(p.look.color)));
        }

        // Planets on their orbits, spinning (`ShowStarmap`: angle `PlanetRotate · spin · 6`, `PlanetRotate` +1/150
        // per UI tick).
        let clock = prev.orbit + (curr.orbit - prev.orbit) * alpha.clamp(0.0, 1.0);
        let planet_rotate = ui / 150.0;
        for p in self.planets(curr) {
            let Some((c, px)) = cam.project(orbit_position(p, clock)) else { continue };
            let half = (planet_radius(p) * px).max(0.75 * self.scale);
            if c[0] < -half || c[1] < -half || c[0] > cam.size[0] + half || c[1] > cam.size[1] + half {
                continue;
            }
            let turn = planet_rotate * p.look.spin as f32 / 100.0 * 6.0 / std::f32::consts::TAU;
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
        let fleets = curr.fleets_at(prev, alpha);
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
                        f.push(t, Blend::Additive, Instance::new(kind::SPRITE, s, [size * px; 2], particle_color(color)));
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
                    f.push(s.tex, Blend::Alpha, Instance { uv: s.uv(col, row), ..Instance::new(kind::SPRITE, c, half, [dim, dim, dim, 1.0]) });
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
    fn background(&self, f: &mut Frame, a: &Assets, m: &[[f32; 3]; 3]) {
        let cam = &self.camera;
        for p in &self.background {
            let e = cam.eye(m, p.h, p.p);
            if e[2] >= ZCLIP {
                continue;
            }
            let Some((c, px)) = cam.project_eye(e) else { continue };
            let color = particle_color(p.color);
            match p.set {
                0 => {
                    if c[0] < 0.0 || c[1] < 0.0 || c[0] > cam.size[0] || c[1] > cam.size[1] {
                        continue;
                    }
                    f.push(a.white, Blend::Additive, Instance::new(kind::GLOW, c, [1.5 * self.scale; 2], color));
                }
                s => {
                    let half = p.size * px;
                    if c[0] < -half || c[1] < -half || c[0] > cam.size[0] + half || c[1] > cam.size[1] + half {
                        continue;
                    }
                    if let Some(t) = a.clouds[s as usize - 1] {
                        f.push(t, Blend::Additive, Instance::new(kind::SPRITE, c, [half; 2], color));
                    }
                }
            }
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
        Camera { x: 10.0, y: -20.0, rotate, pitch, zoom: 300.0, size: [1920.0, 1080.0] }
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
