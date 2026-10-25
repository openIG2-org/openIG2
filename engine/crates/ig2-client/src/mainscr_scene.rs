//! The main menu's 3D scene (re/screens/mainscr.md §3–§4): `Main3DInit` (exe 0x4f96a0) builds it once,
//! `Mainscreen_Update3D` (exe 0x500270) poses it every drawn frame.
//!
//! Two kinds of objects: the camera-fixed ones (`SetRelative(false)`: the rings, the meteors) sit in camera space as
//! `MoveObj` puts them, the others (the galaxy mesh, its four particle clouds, the background pictures, the dust) are
//! seen through the moving camera `e = M·p − kamerapont`, `M = CreateMatrix(elforgatas, 0, 0)` (`CameraRotate2`,
//! `forgaspont` 0). Both project with fov 600 about (320, 240) (`TransformPoint`). Particles are the engine's
//! `0xTTBBGGRR` sprites (`starmap::particle_color`), half-size `size·600/−z`, added; the meshes go through the 3D
//! pass offscreen and are laid over them.

use std::collections::HashMap;

use ig2_formats::Install;

use crate::assets::Image;
use crate::font::Virtual;
use crate::lighting;
use crate::mainscr::{MainScr, METKOZ, METMAX};
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement, M3, V3};
use crate::starmap::MsvcRand;

const VECTORS: &str = "vector/main/";
const TEXTURES: &str = "textures/main/";
/// The label textures (`eng` is a literal in the exe; the install's localisation copies hold the language's).
const UNSELECTED: &str = "textures/main/eng/unselected/";
const SELECTED: &str = "textures/main/eng/selected/";
const FOV: f32 = 600.0;

/// One particle (`PARTICLETYPE`: position, colour word, size).
#[derive(Debug, Clone, Copy)]
struct Particle {
    p: V3,
    color: u32,
    size: f32,
}

/// A particle system on its null box: the texture (`None` = points), the particles.
struct Cloud {
    tex: Option<TexId>,
    parts: Vec<Particle>,
}

/// The scene's resources and its generated particle sets.
pub struct MainScene {
    models: HashMap<String, Model>,
    textures: HashMap<String, TexId>,
    white: TexId,
    /// `GenerateGalaxy` ×4 (`n2` 3500, `felho2` 3500, `n2` 3500, `n2` 3300).
    galaxy: Vec<Cloud>,
    /// `GenerateKod` (3500 points), `GenerateKod2` ×2 (`torpedo`, `n2`, 1750 each).
    dust: Vec<Cloud>,
    /// The three background pictures (`texcsor` and `galaxy_1` twice): position, size, texture.
    pictures: Vec<(V3, f32, Option<TexId>)>,
    /// The labels the rings wore last (kept while they fly out, `Update3D` step 10).
    labels: Option<&'static [&'static str]>,
    target: Option<Target>,
    /// `kezdo_.xv` posed at the opening's integer frames (`SetFrame`), loaded as needed.
    race_frames: HashMap<usize, Model>,
    /// `FajVInitPosok` (exe 0x914088), from the user's exe.
    race_starts: crate::mainscr_race::Starts,
}

/// `GenerateGalaxy` (ELF 0x58cde0, spec §3.2): a twisted bar in the y–z plane, its thickness along x.
fn galaxy(r: &mut MsvcRand, n: usize, alpha: u32, s: f32) -> Vec<Particle> {
    (0..n)
        .map(|_| {
            let (kx, ky) = if r.random(2) == 0 { (1.0, 0.0) } else { (0.0, 1.0) };
            let h = r.random(30) as f32;
            let (u1, v1) = (r.random(600), r.random(50));
            let fx = kx * (u1 - 300) as f32 + (v1 - 25) as f32;
            let (u2, v2) = (r.random(600), r.random(50));
            let fy = ky * (u2 - 300) as f32 + (v2 - 25) as f32;
            let d = (fx * fx + fy * fy).sqrt();
            let t = d / 100.0;
            let q = r.random((t * 100.0) as i32) as f32;
            let ang = (t as f64 + (q as f64 / 100.0 - t as f64 / 2.0) / 2.5) as f32;
            let x = s * ((h - 15.0) * ((1.0 - d / 180000f32.sqrt()) + 0.3));
            let (sa, ca) = (ang.sin(), ang.cos());
            let y = s * (ca * fx - sa * fy);
            let z = s * (ca * fy + sa * fx);
            let size = (r.random(8) + 12) as f32;
            for _ in 0..3 {
                r.random(20);
            }
            Particle { p: [x, y, z], color: alpha << 24, size }
        })
        .collect()
}

/// The four clouds of `Main3DInit` step 5: texture, count, alpha, scale.
const CLOUDS: [(&str, usize, u32, f32); 4] = [("n2.bmp", 3500, 200, 1.4), ("felho2.bmp", 3500, 100, 2.0), ("n2.bmp", 3500, 200, 3.0), ("n2.bmp", 3300, 200, 0.8)];

/// The clouds from the libc stream as the booting exe has it: the CRT's default seed (1), 60 draws in [obs: the first
/// particles of all four clouds, two boots alike, spec §13].
fn clouds() -> Vec<Vec<Particle>> {
    let mut rng = MsvcRand(1);
    for _ in 0..60 {
        rng.rand();
    }
    CLOUDS.iter().map(|&(_, n, alpha, scale)| galaxy(&mut rng, n, alpha, scale)).collect()
}

/// `ChangeGalaxyColors` (exe 0x4f6040, all f32): the tint by distance from the centre, faded by `GlobalTransparency`.
fn tint(c: &mut [Particle], gt: f32, rgb: [f32; 3], a: f32) {
    let alpha = (gt * a) as u32;
    for p in c {
        let f = ((p.p[0] * p.p[0] + p.p[1] * p.p[1] + p.p[2] * p.p[2]) / 100000.0 + 0.2).min(1.0);
        let ch = |k: f32| (gt * k * 255.0 * f) as u32 & 0xff;
        p.color = alpha << 24 | ch(rgb[2]) << 16 | ch(rgb[1]) << 8 | ch(rgb[0]);
    }
}

/// `GenerateKod` (`srand(400)`): a 3000-unit cube of points from `o`, colour from three draws.
fn kod(n: usize, o: V3, s: f32) -> Vec<Particle> {
    let mut r = MsvcRand(400);
    (0..n)
        .map(|_| {
            let p = [o[0] + s * r.random(100) as f32, o[1] + s * r.random(100) as f32, o[2] + s * r.random(100) as f32];
            for _ in 0..3 {
                r.random(127);
            }
            Particle { p, color: 0, size: 1.0 }
        })
        .collect()
}

/// `GenerateKod2` (`srand(400)`): a 4500-unit cube, sizes 30..59.
fn kod2(n: usize, o: V3, s: f32) -> Vec<Particle> {
    let mut r = MsvcRand(400);
    (0..n)
        .map(|_| {
            let p = [o[0] + s * r.random(150) as f32, o[1] + s * r.random(150) as f32, o[2] + s * r.random(150) as f32];
            let size = (r.random(30) + 30) as f32;
            Particle { p, color: 0x3278_7878, size }
        })
        .collect()
}

/// `DeGenerateKod` (every frame, `srand(400)`): brightness 128..254 per particle and channel × the tint × `GT`, alpha
/// `GT·a`.
fn dekod(c: &mut [Particle], gt: f32, rgb: [f32; 3], a: f32) {
    let mut r = MsvcRand(400);
    let alpha = (gt * a) as u32;
    for p in c {
        let b = ((r.random(127) + 128) as f32 * rgb[2] * gt) as u32 & 0xff;
        let g = ((r.random(127) + 128) as f32 * rgb[1] * gt) as u32 & 0xff;
        let rr = ((r.random(127) + 128) as f32 * rgb[0] * gt) as u32 & 0xff;
        p.color = alpha << 24 | b << 16 | g << 8 | rr;
    }
}

/// Camera space → the virtual screen and the pixels per world unit there.
fn project(e: V3) -> Option<([f32; 2], f32)> {
    (e[2] < -1.0).then(|| {
        let k = FOV / -e[2];
        ([e[0] * k + 320.0, e[1] * k + 240.0], k)
    })
}

impl MainScene {
    pub fn load(r: &mut Renderer, install: &Install) -> Self {
        let mut s = Self {
            models: HashMap::new(),
            textures: HashMap::new(),
            white: r.add_texture(&Image::white()),
            galaxy: Vec::new(),
            dust: Vec::new(),
            pictures: Vec::new(),
            labels: None,
            target: None,
            race_frames: HashMap::new(),
            race_starts: install.read_raw("ig2.exe").ok().and_then(|b| ig2_rules::PeImage::parse(&b).ok().map(|pe| crate::mainscr_race::read_starts(&pe))).unwrap_or_default(),
        };
        for ((tex, ..), parts) in CLOUDS.iter().zip(clouds()) {
            let tex = s.texture(r, install, TEXTURES, tex);
            s.galaxy.push(Cloud { tex, parts });
        }
        for (tex, at, size) in [("texcsor.bmp", [-400.0, -400.0, -100.0], 1350.0), ("galaxy_1.bmp", [-500.0, 1500.0, -3000.0], 300.0), ("galaxy_1.bmp", [-900.0, 1900.0, -3000.0], 200.0)] {
            let tex = s.texture(r, install, TEXTURES, tex);
            s.pictures.push((at, size, tex));
        }
        let o = [-1000.0, -800.0, -1800.0];
        s.dust.push(Cloud { tex: None, parts: kod(3500, o, 30.0) });
        let torpedo = s.texture(r, install, TEXTURES, "torpedo.bmp");
        s.dust.push(Cloud { tex: torpedo, parts: kod2(1750, o, 30.0) });
        let n2 = s.texture(r, install, TEXTURES, "n2.bmp");
        s.dust.push(Cloud { tex: n2, parts: kod2(1750, o, 30.0) });
        s
    }

    fn texture(&mut self, r: &mut Renderer, install: &Install, dir: &str, name: &str) -> Option<TexId> {
        let path = install.resolve_texture(dir, name)?;
        self.texture_path(r, install, &path)
    }

    fn texture_path(&mut self, r: &mut Renderer, install: &Install, path: &str) -> Option<TexId> {
        if let Some(t) = self.textures.get(path) {
            return Some(*t);
        }
        match Image::load(install, path) {
            Ok(img) => {
                let id = r.add_texture(&img);
                self.textures.insert(path.to_owned(), id);
                Some(id)
            }
            Err(e) => {
                eprintln!("ig2-client: {e}");
                None
            }
        }
    }

    fn model(&mut self, install: &Install, dir: &str, tex: &str, name: &str) -> Model {
        self.models
            .entry(name.to_owned())
            .or_insert_with(|| {
                Model::load(install, dir, name, tex).unwrap_or_else(|e| {
                    eprintln!("ig2-client: {e}");
                    Model::default()
                })
            })
            .clone()
    }

    /// The frame (`Mainscreen_Update3D`) behind the 2D layer; returns the race boxes' centres (`ForgatFajvalasztasDolgai`)
    /// for the menu's hit-tests.
    pub fn draw(&mut self, f: &mut Frame, r: &mut Renderer, install: &Install, s: &MainScr, vv: &Virtual) -> [Option<[f32; 2]>; 3] {
        let (cam, a) = s.cam;
        let m = scene::create_matrix(a, 0.0, 0.0);
        // `forgaspont` (the turn's centre; `q·5` along x while leaving): `e = M·(p − f) − kamerapont` (`CameraRotate2`
        // ELF 0x5fd340).
        let pivot = [s.exit_q().map_or(0.0, |q| q * 5.0), 0.0, 0.0];
        let mf = scene::apply(&m, pivot);
        let cam = [cam[0] + mf[0], cam[1] + mf[1], cam[2] + mf[2]];
        let eye = |p: V3| -> V3 {
            let q = scene::apply(&m, p);
            [q[0] - cam[0], q[1] - cam[1], q[2] - cam[2]]
        };
        let gt = s.gt;
        let sprite = |f: &mut Frame, tex: Option<TexId>, white: TexId, e: V3, size: f32, color: u32| {
            let Some((c, k)) = project(e) else { return };
            let half = (size * k).max(0.5) * vv.k;
            f.push(tex.unwrap_or(white), Blend::GammaAdditive, Instance::new(kind::SPRITE, vv.px(c[0], c[1]), [half; 2], crate::starmap::particle_color(color)));
        };
        // Step 4: the pictures at `kamerapont.z/1.2 − 3000`, `A = GT·140·GT`, grey `GT·255` (only the first two move).
        let grey = (gt * 255.0) as u32 & 0xff;
        let pcol = ((gt * 140.0 * gt) as u32) << 24 | grey << 16 | grey << 8 | grey;
        for (i, (at, size, tex)) in self.pictures.iter().enumerate() {
            // `MoveObj` puts the null box at (x, y, z′) while its one particle keeps the record's own (x, y, z) inside
            // it (exe 0x500270 / `Main3DInit` 0x4f96a0 step 6): the picture sits at the sum. The third box never moves.
            let p = if i < 2 { [2.0 * at[0], 2.0 * at[1], cam[2] / 1.2 - 3000.0 + at[2]] } else { *at };
            sprite(f, *tex, self.white, eye(p), *size, if i < 2 { pcol } else { 0x8cff_ffff });
        }
        // Step 2: the dust, recoloured every frame.
        for (c, rgb) in self.dust.iter_mut().zip([[0.5; 3], [0.0; 3], [0.5; 3]]) {
            dekod(&mut c.parts, gt, rgb, 0.0);
        }
        for c in &self.dust {
            for p in &c.parts {
                sprite(f, c.tex, self.white, eye(p.p), p.size, p.color);
            }
        }
        // Step 5: the clouds turn (`RotateObj(0, π/2, roll)` then `RotateObj2((1, 0, 0), 1.0)`), recoloured.
        let clouds: [(f32, [f32; 3], f32); 4] = [(-20.0, [0.6, 0.6, 0.9], 130.0), (-30.0, [0.2, 0.4, 0.6], 170.0), (-40.0, [0.2, 0.3, 0.5], 150.0), (-10.0, [0.7, 0.7, 0.7], 100.0)];
        for (c, (div, rgb, al)) in self.galaxy.iter_mut().zip(clouds) {
            tint(&mut c.parts, gt, rgb, al);
            let rot = scene::mul(&scene::axis_matrix([1.0, 0.0, 0.0], 1.0), &scene::create_matrix(0.0, std::f32::consts::FRAC_PI_2, s.m / div * 0.25));
            for p in &c.parts {
                sprite(f, c.tex, self.white, eye(scene::apply(&rot, p.p)), p.size, p.color);
            }
        }
        self.meshes(f, r, install, s, vv, &m, cam)
    }

    /// The race scene this frame: `kezdo_.xv` posed (the opening's frame, then `MozgatKor` per race), its placement, and
    /// the root children's transparencies (`SetObjTransparency`; above 0.99999 hidden).
    fn race_posed(&mut self, install: &Install, rc: &crate::mainscr_race::RaceChoice) -> (Model, scene::Transform, HashMap<u16, f32>) {
        let mut model = self.race_model(install, rc.frame);
        let place = Placement::add_to_scene(model.origin, 0.611).world((0.0, 0.0, 0.0), [0.0, 0.0, -800.0]);
        let mut alpha: HashMap<u16, f32> = HashMap::new();
        if rc.opened {
            for k in 3..9u16 {
                alpha.insert(k, 1.0);
            }
            let poses = rc.poses(&self.race_starts);
            for (k, pose) in poses.iter().enumerate() {
                let (a, b, c) = RACE_SUBS[k];
                let Some(p) = pose else {
                    for sub in [a, b, c] {
                        alpha.insert(sub, 1.0);
                    }
                    continue;
                };
                for sub in [a, b, c] {
                    let z = if sub == a { p.at[2] + 100.0 } else { p.at[2] };
                    let to = [p.at[0] / 1024.0, (rc.y + p.at[1]) / 1024.0, z / 1024.0];
                    // `RotateObjr(sub, 0, 0, ±0.02)` on the rings, added up.
                    let roll = RING_ROLL.iter().find(|(n, _)| *n == sub).map(|(_, d)| d * rc.roll);
                    let rot = roll.map(|c| scene::create_matrix(0.0, 0.0, c));
                    model.move_child(&[sub], Some(to), rot.as_ref());
                    // The rings' faces towards the camera take the scene's front light 0xdcc8c8 in the exe (lit stone
                    // with a faint blue top and warm bottom); here the lighting normals of the drawn faces point the
                    // other way along z, so only the side lights tinted them. Mirrored in z they match the capture
                    // [obs: `probes/race_rings.js`: the exe's per-vertex colours are `fenyekszamolasa`'s exactly from
                    // the same file normals and chains as these; the averages agree, the drawn side doesn't; cause
                    // open, deviation 148; mainscr-race t 1010-1250 frame 5.3-6.7, rings neutral].
                    if sub != a {
                        for p in model.parts.iter_mut().filter(|p| p.index.first() == Some(&sub)) {
                            for v in &mut p.verts {
                                v.lnormal[2] = -v.lnormal[2];
                            }
                        }
                    }
                    alpha.insert(sub, p.a);
                }
            }
        }
        (model, place, alpha)
    }

    /// The race boxes' centres this frame without drawing (the menu's hit-tests; `ForgatFajvalasztasDolgai`).
    pub fn race_boxes(&mut self, install: &Install, s: &MainScr) -> [Option<[f32; 2]>; 3] {
        match &s.race {
            Some(rc) if rc.boxes_on() => {
                let (model, place, _) = self.race_posed(install, rc);
                race_centres(rc, &model, &place)
            }
            _ => [None; 3],
        }
    }

    /// `kezdo_.xv` at the opening's `frame` (between two integer frames, `SetFrame` keeps fractions).
    fn race_model(&mut self, install: &Install, frame: f32) -> Model {
        let (a, b, w) = scene::frame_keys(frame, 91);
        for k in [a, b] {
            self.race_frames.entry(k).or_insert_with(|| {
                Model::load_at(install, VECTORS, "kezdo_.xvb", TEXTURES, Some(k)).unwrap_or_else(|e| {
                    eprintln!("ig2-client: {e}");
                    Model::default()
                })
            });
        }
        self.race_frames[&a].lerp(&self.race_frames[&b], w)
    }

    /// The meshes: the galaxy, the meteors, the rings with their labels; drawn offscreen and laid over the particles.
    #[allow(clippy::too_many_arguments)]
    fn meshes(&mut self, f: &mut Frame, r: &mut Renderer, install: &Install, s: &MainScr, vv: &Virtual, m: &M3, cam: V3) -> [Option<[f32; 2]>; 3] {
        let mut centres = [None; 3];
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let unlit = lighting::Lights::default().ambient(0xffffff);
        // The scene's side lights (`mainOldalsoFenyekBe` exe 0x5047d0, ported as `mesOldalsoFenyekBe`; on `mainmeshid`,
        // the scene's root, so every lit model sees them first, `Kocka`): colour words 0xc86464 from (200, −1000, 500)
        // and 0x829cff from (0, 1400, 500). Each ship adds its own parallel light (0x323238 from (−1000, 0, 2000),
        // `Main3DInit` step 9), the race scene its 0xdcc8c8 from (0, 0, −1000) (`LoadFajValasztasVektor` 0x4f9410).
        // The meteors (`LoadBox(.., 0)`, no light of their own) draw near black in the exe [obs: lit by the side lights
        // they come out grey; re/deviations.md 150]: a dim light only.
        let dim = lighting::Lights::default().parallel(0x323238, [-1000.0, 0.0, 2000.0]);
        let side = lighting::Lights::default().parallel(0xc86464, [200.0, -1000.0, 500.0]).parallel(0x829cff, [0.0, 1400.0, 500.0]);
        let ship = side.clone().parallel(0x323238, [-1000.0, 0.0, 2000.0]);
        let relative = |t: scene::Transform| -> scene::Transform {
            let mm = scene::mul(m, &t.matrix);
            let q = scene::apply(m, t.translation);
            scene::Transform { matrix: mm, translation: [q[0] - cam[0], q[1] - cam[1], q[2] - cam[2]] }
        };
        // The galaxy (`galaxy.xvb` 0.6, transparency 0.5, additive): `RotateObj(0, 0, −M/25·0.5)`, then the axis (1, 0, 0) by π/3.
        let g = self.model(install, VECTORS, TEXTURES, "galaxy.xvb");
        let rot = scene::mul(&scene::axis_matrix([1.0, 0.0, 0.0], std::f32::consts::FRAC_PI_3), &scene::create_matrix(0.0, 0.0, s.m / -25.0 * 0.5));
        let p0 = Placement::add_to_scene(g.origin, 0.6);
        let t = relative(scene::Transform { matrix: scene::mul(&rot, &p0.matrix).map(|x| x * 1024.0), translation: [0.0; 3] });
        for p in &g.parts {
            let tex = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x)).unwrap_or(self.white);
            bake(p, &t, &unlit, tex, 0.5, true, &mut dynamic, &mut draws);
        }
        // Step 6: the meteors, camera-fixed, faded in between `METMAX` and `METKOZ`.
        let met = self.model(install, VECTORS, TEXTURES, "meteor.xvb");
        for mt in &s.meteors {
            let z = mt.pos[2];
            if z <= METMAX {
                continue;
            }
            let tr = if z >= METKOZ { 1.0 - s.gt } else { ((z - METKOZ) / (METMAX - METKOZ)).clamp(0.0, 1.0) * s.gt + (1.0 - s.gt) };
            let t = Placement::add_to_scene(met.origin, mt.scale).world((mt.angles[0], mt.angles[1], mt.angles[2]), mt.pos);
            for p in &met.parts {
                let tex = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x)).unwrap_or(self.white);
                bake(p, &t, &dim, tex, 1.0 - tr, false, &mut dynamic, &mut draws);
            }
        }
        // Step 7: the ships shown (`flag{i+1}0.xvb` 0.5, camera-fixed, lit by their own parallel light 0x323238 from
        // (−1000, 0, 2000) and the scene's), turned to their course (`CreateMatrix2(d, ang)·CreateMatrix(0, 0, r2)`).
        for (i, sh) in s.ships.iter().enumerate().filter(|(_, sh)| sh.shown) {
            let model = self.model(install, "vector/spacewar/", "textures/spacewar/", &format!("flag{}0.xvb", i + 1));
            let d = scene::normalise([sh.target[0] - sh.start[0], sh.target[1] - sh.start[1], sh.target[2] - sh.start[2]]);
            let (spin, spin2) = (i == 2, i == 1);
            let ang = if spin { s.m / 20.0 + 1.570_796_4 } else { ((sh.roll as f64).sin() + std::f64::consts::FRAC_PI_2) as f32 };
            let a = create_matrix2(d, ang);
            let b = scene::create_matrix(0.0, 0.0, if spin2 { s.m / 20.0 } else { 0.0 } + 1.570_796_4);
            let p0 = Placement::add_to_scene(model.origin, 0.5);
            let t = scene::Transform { matrix: scene::mul(&scene::mul(&a, &b), &p0.matrix).map(|x| x * 1024.0), translation: sh.pos };
            let lights = ship.clone();
            for p in &model.parts {
                let tex = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x)).unwrap_or(self.white);
                bake(p, &t, &lights, tex, 1.0 - (sh.alpha * s.gt + (1.0 - s.gt)), false, &mut dynamic, &mut draws);
            }
        }
        // The race scene (§9.2): `kezdo_.xv` 0.611 at (0, 0, −800), camera-fixed; its opening by `SetFrame`, then each
        // race's head (A), rings (B, C) moved by `MozgatKor`, the telescopes (subs 3..8) hidden.
        if let Some(rc) = &s.race {
            let (model, place, alpha) = self.race_posed(install, rc);
            // The scene's side lights, then its own parallel light.
            let base = side.clone().parallel(0xdcc8c8, [0.0, 0.0, -1000.0]);
            for p in &model.parts {
                let top = p.index.first().copied().unwrap_or(u16::MAX);
                let a = alpha.get(&top).copied().unwrap_or(0.0);
                if a as f64 > 0.99999 {
                    continue;
                }
                // `FejFenyekBeallitasa`: each head's own ambient light, white for the highlighted race.
                // The heads take their own ambient light only (`PropertiesOn(0x40000)`) [obs: the others dim].
                let lights = match RACE_SUBS.iter().position(|r| r.0 == top) {
                    Some(k) => lighting::Lights::default().ambient(if rc.last == Some(k) { 0xffffff } else { 0x646464 }),
                    None => base.clone(),
                };
                let tex = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x)).unwrap_or(self.white);
                bake(p, &place, &lights, tex, 1.0 - a, false, &mut dynamic, &mut draws);
            }
            centres = race_centres(rc, &model, &place);
        }
        // Steps 8 and 10: the rings (`korke1` 0.7 turned, `szrke{i}` 0.3 with the label on child 0), transparency 0.3,
        // additive, unlit; after the race scene's opaque models, as the engine draws its transparent objects last
        // [obs: the difficulty labels and their flares over the race ring and the head; deviation 305].
        if let Some(l) = s.labels() {
            self.labels = Some(l);
        }
        let ring = self.model(install, VECTORS, TEXTURES, "korke1.xvb");
        for i in 0..s.rings {
            let at = s.ring_pos(i);
            let t = Placement::add_to_scene(ring.origin, 0.7).world(s.ring_rot(i), at);
            for p in &ring.parts {
                let tex = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x)).unwrap_or(self.white);
                bake(p, &t, &unlit, tex, 0.7, true, &mut dynamic, &mut draws);
            }
            let face = self.model(install, VECTORS, TEXTURES, &format!("szrke{i}.xvb"));
            let label = self.labels.and_then(|l| l.get(i)).and_then(|n| self.texture(r, install, if s.hovered == Some(i) { SELECTED } else { UNSELECTED }, n));
            let t = Placement::add_to_scene(face.origin, 0.3).world((0.0, 0.0, 0.0), at);
            for p in &face.parts {
                let own = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x));
                let tex = if p.index.first() == Some(&0) { label.or(own) } else { own }.unwrap_or(self.white);
                bake(p, &t, &unlit, tex, 0.7, true, &mut dynamic, &mut draws);
            }
        }
        // `HandleLoadingMesh` (§2.2): in the exit animation's last 100 ticks the loading plate (`loadi1.xv` 0.21,
        // camera-fixed, unlit, additive) turns in at z −1400 with the start's picture `load<n>`.
        if let (Some(k), Some((_, st))) = (s.exit_k(), &s.leaving) {
            if k < 50.0 {
                let t = (k as f64 / 50.0) as f32;
                let plate = self.model(install, VECTORS, TEXTURES, "loadi1.xvb");
                let pic = st.picture(|_| 0);
                let tex = self.texture(r, install, TEXTURES, &format!("load{}.bmp", pic + 1));
                let rot = ((t as f64) * std::f64::consts::FRAC_PI_2) as f32;
                let tr = Placement::add_to_scene(plate.origin, 0.21).world((0.0, rot, 0.0), [0.0, -(t * 100.0), -1400.0]);
                for p in &plate.parts {
                    let own = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x));
                    bake(p, &tr, &unlit, tex.or(own).unwrap_or(self.white), 1.0 - t, true, &mut dynamic, &mut draws);
                }
                // Plate 2 (`loadi2.xv`, its own picture: the "LOADING" line) turns the other way [obs: shown with plate 1].
                let plate2 = self.model(install, VECTORS, TEXTURES, "loadi2.xvb");
                let tr = Placement::add_to_scene(plate2.origin, 0.21).world((0.0, -rot, 0.0), [0.0, t * 100.0, -1400.0]);
                for p in &plate2.parts {
                    let own = p.texture.as_deref().and_then(|x| self.texture_path(r, install, x)).unwrap_or(self.white);
                    bake(p, &tr, &unlit, own, 1.0 - t, true, &mut dynamic, &mut draws);
                }
            }
        }
        if dynamic.is_empty() {
            return centres;
        }
        let wide = crate::kit::Wide::new(vv);
        let px = wide.px;
        let mut fr = Frame::new([0.0, 0.0, 0.0, 0.0]);
        let mut u = uniforms();
        let rows = wide.rows(std::array::from_fn(|r| std::array::from_fn(|c| u.view[c][r])));
        u.view = std::array::from_fn(|c| std::array::from_fn(|r| rows[r][c]));
        fr.scene = Some(Scene3d { uniforms: u, gamma: true, shadow: None, draws, dynamic });
        let target = match &mut self.target {
            Some(t) => t,
            None => self.target.insert(r.target(px)),
        };
        r.draw_into(&fr, target, px, 0.0);
        let tex = target.tex;
        f.push(tex, Blend::GammaPremul, Instance::new(kind::SPRITE, vv.px(320.0, 240.0), wide.half(vv), [1.0; 4]));
        centres
    }
}

/// The race boxes' centres: ring C's centre (`GetObjCenter`) through `TransformPoint`, while the boxes exist.
fn race_centres(rc: &crate::mainscr_race::RaceChoice, model: &Model, place: &scene::Transform) -> [Option<[f32; 2]>; 3] {
    if !rc.boxes_on() {
        return [None; 3];
    }
    std::array::from_fn(|k| {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in model.below(&[RACE_SUBS[k].2]).flat_map(|p| p.verts.iter()) {
            let q = place.point(v.pos);
            for i in 0..3 {
                lo[i] = lo[i].min(q[i]);
                hi[i] = hi[i].max(q[i]);
            }
        }
        (lo[0] <= hi[0]).then(|| project(std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0)).map(|(p, _)| p)).flatten()
    })
}

/// The menu's 2D (`MainScrGfx2D_1/_2`, `KiirFajInfoSzoveg`): the race blurb, the scenario list, the name entry.
#[allow(clippy::too_many_arguments)]
pub fn draw_2d(s: &mut MainScr, assets: &mut Option<crate::mainscr_scenario::Assets>, h: &crate::ui::Hud, r: &mut Renderer, install: &Install, frame: &mut Frame, size: [f32; 2], now_ms: f64) {
    let Some(font) = &h.font else { return };
    let d = crate::options::Draw { font, white: h.white, glow: h.white, v: Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms };
    let blink = crate::kit::fader_design::blink(now_ms);
    if let Some(rc) = &s.race {
        rc.draw_blurb(frame, &d, blink);
    }
    if let (Some(sc), Some(a)) = (&mut s.scenario, assets) {
        a.race_pictures(r, install);
        if let Some(e) = sc.entry() {
            let pic = e.pic.clone();
            a.picture(r, install, &pic);
        }
        sc.draw(frame, &d, &a.pics, &a.tables);
    }
    if let Some(n) = &s.name {
        crate::mainscr_scenario::draw_name(frame, &d, n, blink);
    }
    if let Some(reason) = s.disconnected {
        crate::mainscr_scenario::draw_disconnected(frame, &d, reason, s.at);
    }
}

/// What a ring's request needs from the install before the menu goes on (shared by the window and the script run):
/// the scenario list's entries, the race blurbs.
pub fn prepare(s: &mut MainScr, a: crate::mainscr::Act, assets: &mut Option<crate::mainscr_scenario::Assets>, install: &Install, texts: &crate::ui::UiTexts) {
    use crate::mainscr::Act;
    match a {
        Act::OpenScenarios => {
            let a = assets.get_or_insert_with(|| crate::mainscr_scenario::Assets::load(install, texts));
            s.open_scenarios(a.entries.clone());
        }
        Act::Campaign => s.blurbs = crate::mainscr_race::load_blurbs(install, texts),
        _ => {}
    }
}

/// Each race's sub-meshes of `kezdo_.xv` (`MozgatKor`): head A, ring B, ring C (the box anchor); race 0 Human, 1
/// Politikus, 2 Predator.
const RACE_SUBS: [(u16, u16, u16); 3] = [(2, 9, 10), (0, 11, 12), (1, 13, 14)];
/// The rings' roll per tick (`RotateObjr(sub, 0, 0, ±0.02)`), as a sign.
const RING_ROLL: [(u16, f32); 6] = [(9, 1.0), (10, -1.0), (11, 1.0), (12, -1.0), (13, -1.0), (14, 1.0)];

/// `CreateMatrix2(d, θ)` (ELF 0x4534e0): the columns `−d`, `−c2`, `−c1` with `c1 = |d × (sin θ, cos θ, 0)|`,
/// `c2 = |d × c1|` (`vektormul`, `normalizevektor`).
fn create_matrix2(d: V3, a: f32) -> M3 {
    let cross = |a: V3, b: V3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let u = [((a as f64).sin()) as f32, ((a as f64).cos()) as f32, 0.0];
    let c1 = scene::normalise(cross(d, u));
    let c2 = scene::normalise(cross(d, c1));
    [-d[0], -c2[0], -c1[0], -d[1], -c2[1], -c1[1], -d[2], -c2[2], -c1[2]]
}

/// The 3D pass's constants: camera space → clip as [`project`] (fov 600 on 640 × 480), depth 10..60000.
fn uniforms() -> SceneUniforms {
    let (n, fa) = (10.0f32, 60000.0f32);
    let (a, b) = (fa / (fa - n), fa * n / (fa - n));
    let rows4 = [[1200.0 / 640.0, 0.0, 0.0, 0.0], [0.0, -1200.0 / 480.0, 0.0, 0.0], [0.0, 0.0, -a, -b], [0.0, 0.0, -1.0, 0.0]];
    let view = std::array::from_fn(|c| std::array::from_fn(|k| rows4[k][c]));
    SceneUniforms {
        view,
        eye: [0.0; 4],
        light: [0.0, 0.0, 1.0, 0.0],
        ambient: [1.0, 1.0, 1.0, 0.0],
        sun: [0.0; 4],
        fog: [0.0; 4],
        fog_range: [1e9, 2e9, 0.0, 1e9],
        flash: [[0.0; 4]; 2],
    }
}

/// Bake a part in camera space with CPU-lit vertex colours; `additive` for the engine's property-8-off objects.
#[allow(clippy::too_many_arguments)]
fn bake(p: &scene::Part, t: &scene::Transform, lights: &lighting::Lights, tex: TexId, opacity: f32, additive: bool, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let start = out.len() as u32;
    for v in &p.verts {
        let c = lights.rgb(lighting::normal(&t.matrix, v.lnormal));
        out.push(MeshVertex { pos: t.point(v.pos), normal: [0.0; 3], uv: v.uv, color: [c[0], c[1], c[2], opacity] });
    }
    draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: opacity < 1.0 || additive, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive, depth_write: false });
}

/// `ig2-client --snapshot OUT --screen main [--ui-frames N] [--mouse X,Y]`: the main menu after N UI ticks of 10 ms
/// (one drawn frame each, as the exe in a capture script), no save needed.
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    if let Some(path) = &o.script {
        return sequence(o, out, &crate::script::Script::load(path)?);
    }
    let install = crate::open_install(o)?;
    let mut r = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut scene = MainScene::load(&mut r, &install);
    let mut s = MainScr::enter();
    for _ in 0..o.ui_frames {
        s.frame();
        s.tick();
    }
    s.frame();
    let size = [o.size.0 as f32, o.size.1 as f32];
    let vv = Virtual::new(size);
    if let Some(m) = o.mouse {
        s.hover(m);
    }
    let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
    scene.draw(&mut frame, &mut r, &install, &s, &vv);
    let rgba = r.draw_to_image(&frame, o.size, 0.0);
    crate::assets::write_snapshot(out, o, &rgba)
}

/// `ig2-client --snapshot OUT --screen main --script S`: the input script frame by frame (re/screens/sound.md §3), one
/// UI tick and one drawn frame per t: the events at t, the tick, the frame (`Update3D`: the camera, the hover, the
/// presses), the grab `OUT-t<T>.png`.
pub fn sequence(o: &crate::Options, out: &std::path::Path, script: &crate::script::Script) -> Result<(), String> {
    use crate::script::Input;
    let install = crate::open_install(o)?;
    let mut r = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut scene = MainScene::load(&mut r, &install);
    let mut s = MainScr::enter();
    let size = [o.size.0 as f32, o.size.1 as f32];
    let vv = Virtual::new(size);
    let mut mouse = [320.0, 240.0];
    // A script's input reaches `Update3D`'s press test two UI ticks after it went in [obs: the fly-out of the t 350
    // press fits best 2 ticks late].
    let lag = 2;
    // The load ring opens Options in load-only mode over the scene (re/screens/options.md); input goes to it then.
    let mut options: Option<(crate::options::Headless, crate::options::OptionsScreen)> = None;
    let tables = ig2_rules::Rules::from_install(&install)?;
    let hud = crate::ui::Hud::load(&mut r, &install, &tables);
    let mut assets = None;
    // Our MP screens (ADR 0014), offline: hosting works on a listener nobody reaches, nothing touches the network.
    let mut mp: Option<crate::mp_lobby::MpScreen> = None;
    let mut net = crate::mp_lobby::OfflineNet;
    // The row frame's globals (`--dkm`, else the exe's boot values), carried by the scenario list's frame (row 362).
    let [m0, c0, n0] = o.dkm.unwrap_or([0.0, std::f32::consts::FRAC_PI_2, 0.0]);
    let (mut dk, mut dk_owner) = (crate::kit::DesignFrame::with_phase(m0, c0, n0), 0u8);
    for t in 0..=script.end() {
        let mut press = None;
        let mut release = None;
        let mut right = None;
        let mut keys = Vec::new();
        for e in script.at(t.saturating_sub(lag)).filter(|_| t >= lag) {
            match &e.input {
                Input::Key { name, down: true } => keys.push(name.clone()),
                Input::Move(at) => mouse = *at,
                Input::Button { button: 1, down, at } => {
                    let (down, at) = (*down, *at);
                    mouse = at;
                    if down {
                        press = Some(at);
                    } else {
                        release = Some(at);
                    }
                }
                Input::Button { button: 3, down: true, at } => {
                    mouse = *at;
                    right = Some(*at);
                }
                Input::Button { button: 3, down: false, .. } => s.release_right(),
                _ => {}
            }
        }
        // The screen's first frame comes two UI ticks after the script's start (`MainElindult` gates the ticks until
        // it) [obs: `Update3D`'s first call at t 2, M 0].
        let mut due = None;
        if t > 2 {
            s.tick();
            due = s.due();
        }
        s.frame();
        let list = s.scenario.as_mut().map(|sc| &mut sc.frame);
        crate::window::carry_row_frame(&mut dk, &mut dk_owner, if list.is_some() { 6 } else { 0 }, list, 1);
        s.hover(mouse);
        s.race_boxes = scene.race_boxes(&install, &s);
        if let Some(st) = s.take_started() {
            println!("t {t}: start {st:?}");
        }
        if let Some(m) = &mut mp {
            use crate::mp_lobby::Out;
            m.hover(mouse);
            let mut outs: Vec<Out> = m.tick(&mut net).into_iter().collect();
            for (p, rt) in [(press, false), (right, true)] {
                if let Some(p) = p {
                    outs.extend(m.press(p, rt, &mut net));
                }
            }
            for k in &keys {
                use crate::options::EntryKey;
                let lower = k.to_ascii_lowercase();
                let key = match lower.as_str() {
                    "enter" | "return" => Some(EntryKey::Enter),
                    "backspace" => Some(EntryKey::Back),
                    "space" => Some(EntryKey::Char(b' ')),
                    c if c.len() == 1 => Some(EntryKey::Char(k.as_bytes()[0])),
                    _ => None,
                };
                outs.extend(m.key(key, matches!(lower.as_str(), "escape" | "esc"), &mut net));
            }
            for o in outs {
                match o {
                    Out::Click => println!("t {t}: mp click"),
                    Out::Close => {
                        println!("t {t}: mp close");
                        mp = None;
                        s.close_mp();
                        break;
                    }
                    Out::Start(_) => println!("t {t}: mp start"),
                }
            }
            keys.clear();
            press = None;
            release = None;
            right = None;
        }
        for k in &keys {
            use crate::options::EntryKey;
            let key = match k.to_ascii_lowercase().as_str() {
                "escape" | "esc" => {
                    s.escape();
                    None
                }
                "enter" | "return" => Some(EntryKey::Enter),
                "backspace" => Some(EntryKey::Back),
                "space" => Some(EntryKey::Char(b' ')),
                c if c.len() == 1 => Some(EntryKey::Char(k.as_bytes()[0])),
                _ => None,
            };
            if let Some(a) = key.and_then(|key| s.name_key(key)) {
                println!("t {t}: {a:?}");
            }
        }
        if let Some((_, os)) = &mut options {
            os.hover(mouse);
            if let Some(p) = press {
                os.press(p, &mut |id| println!("t {t}: sound {id:?}"));
            }
            if release.is_some() {
                os.release();
            }
            os.tick();
            if let Some(a) = os.done() {
                println!("t {t}: options {a:?}");
            }
        } else {
            let acts = [due, press.and_then(|p| s.press(p)), right.and_then(|p| s.press_right(p)), release.and_then(|p| s.release(p))];
            for a in acts.into_iter().flatten() {
                println!("t {t}: {a:?}");
                prepare(&mut s, a, &mut assets, &install, &hud.texts);
                if matches!(a, crate::mainscr::Act::MultiServer | crate::mainscr::Act::MultiMatchmaking) {
                    let name = String::from_utf8_lossy(&s.player).into_owned();
                    mp = Some(crate::mp_lobby::MpScreen::open(a == crate::mainscr::Act::MultiMatchmaking, &name, [0; 32]));
                }
                if a == crate::mainscr::Act::LoadGame {
                    let tables = ig2_rules::Rules::from_install(&install)?;
                    let h = crate::options::Headless::load(&mut r, &install, &tables);
                    let os = h.enter(&install, true);
                    options = Some((h, os));
                }
            }
        }
        if !script.grabs.contains(&t) {
            // The exe draws every tick: the list's row frame is asked for on the frames this run doesn't draw too.
            if let (Some(sc), Some(_)) = (&mut s.scenario, &assets) {
                sc.ask_frame();
            }
        }
        if script.grabs.contains(&t) {
            let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
            scene.draw(&mut frame, &mut r, &install, &s, &vv);
            draw_2d(&mut s, &mut assets, &hud, &mut r, &install, &mut frame, size, t as f64 * 10.0);
            if let (Some(m), Some(font)) = (&mp, &hud.font) {
                let d = crate::options::Draw { font, white: hud.white, glow: hud.white, v: Virtual::new(size), palette: &hud.palette, texts: &hud.texts, now_ms: t as f64 * 10.0 };
                m.draw(&mut frame, &d, crate::kit::fader_design::blink(t as f64 * 10.0));
            }
            if let Some((h, os)) = &mut options {
                h.draw(os, &mut frame, size, t as f64 * 10.0);
            } else if s.leaving.is_none() {
                // `UpdateFrame`'s pointer at the script's point, the hand over a ring or a box (`MainScr_Kilep`'s loop
                // hides it: −2; phase 0: deviation 277).
                hud.draw_pointer(&mut frame, size, [mouse[0] as i32, mouse[1] as i32], s.hand as i32);
            }
            let rgba = r.draw_to_image(&frame, o.size, 0.0);
            let path = crate::script::grab_path(out, t);
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {t} | menu {:?} M {} tav {} cel {} gt {} hovered {:?} hand {} race {:?}", path.display(), s.menu, s.m, s.tav, s.cel, s.gt, s.hovered, s.hand, s.race_boxes);
        } else if let Some((h, os)) = &mut options {
            // Options draws (and so fades) every frame, the grab or not.
            h.draw(os, &mut Frame::new([0.0; 4]), size, t as f64 * 10.0);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;



    /// The first three particles of each cloud as the exe built them (`tools/trace/probes/mainscr.js`, `--boot`).
    #[test]
    fn clouds_match_the_exe() {
        let exe: [[[f32; 4]; 3]; 4] = [
            [[-8.050135, -422.48572, 83.3152, 18.0], [8.581916, -18.848612, 39.715107, 13.0], [-15.337398, -6.2327566, -180.49785, 16.0]],
            [[-27.74932, -112.43204, -47.36069, 18.0], [-21.151478, -349.087, -65.987015, 17.0], [29.706547, -52.13919, 8.336974, 12.0]],
            [[14.01583, 97.34572, -136.9555, 18.0], [29.65425, -508.4148, -50.14414, 13.0], [-4.631025, -516.2883, -430.5281, 13.0]],
            [[-0.9293775, -214.7889, 115.9426, 14.0], [9.430235, 21.56529, 35.0391, 12.0], [-10.13706, 14.53478, -48.10593, 13.0]],
        ];
        let c = clouds();
        for (k, (cloud, want)) in c.iter().zip(exe).enumerate() {
            for (p, w) in cloud.iter().zip(want) {
                let d = (0..3).map(|i| (p.p[i] - w[i]).abs()).fold(0.0, f32::max);
                assert!(d < 0.01 && p.size == w[3], "cloud {k}: {:?} {} vs {w:?}", p.p, p.size);
            }
        }
    }
}
