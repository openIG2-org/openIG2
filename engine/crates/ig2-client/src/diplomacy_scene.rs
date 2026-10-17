//! The diplomacy screen's 3D frame (re/screens/diplomacy.md §3), at rest: the background picture, the wire grid of
//! `dipbck8.xvb`, the six head cages (`golya.xvb`, wireframe) with the contacts' portraits (`negyzetN.xvb`), and the
//! round frame (`tube.xvb`) with the selected leader's video on `kor.xvb`. Vertices are lit on the CPU like the
//! original's engine (ambient, the white parallel light, the two coloured side lights, the heads' marks); wireframes
//! are drawn as textured lines in that light.

use std::collections::HashMap;

use ig2_formats::Install;

use crate::assets::Image;
use crate::diplomacy::{Screen, View, HEADS};
use crate::font::Virtual;
use crate::lighting;
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement, V3};

const VECTORS: &str = "vector/diplomac/";
const TEXTURES: &str = "textures/diplomac/";
/// The scene's depth: everything sits at z = −650 (`GombPoziciok`, `MoveObj(DipKozep, 0, 0, −650)`).
const Z: f32 = -650.0;

/// A texture on the GPU and on the CPU (the wireframes sample it per vertex).
struct Tex {
    id: TexId,
    img: Image,
}

/// The screen's 3D resources.
pub struct DiplomacyScene {
    models: HashMap<String, Model>,
    textures: HashMap<String, Tex>,
    white: TexId,
    /// The background picture (`Kepek[0x34]`, `_ESIG_U2.BMP`).
    background: Option<TexId>,
    /// The leaders' videos by race: one texture per frame.
    anims: HashMap<u8, Vec<TexId>>,
    target: Option<Target>,
    /// The cages' start orientations (`Diplomacy3DInit`: 3 × `rand() % 314 / 20`, cosmetic).
    spin0: [[f32; 3]; HEADS],
    /// The remaster's clock in seconds (`sg3d_time`: the video's scan lines), when pinned (`--acttime`); else the
    /// frame time.
    pub acttime: Option<f32>,
}

impl DiplomacyScene {
    pub fn load(r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules) -> Self {
        let mut s = Self {
            models: HashMap::new(),
            textures: HashMap::new(),
            white: r.add_texture(&Image::white()),
            background: None,
            anims: HashMap::new(),
            target: None,
            spin0: [[0.0; 3]; HEADS],
            acttime: None,
        };
        let mut seed = 0x2545_f491u32;
        for o in &mut s.spin0 {
            *o = std::array::from_fn(|_| {
                seed = seed.wrapping_mul(214013).wrapping_add(2531011);
                (((seed >> 16) & 0x7fff) % 0x13a) as f32 / 20.0
            });
        }
        let names = &rules.pictures;
        if let Some(n) = names.get(0x34) {
            let path = format!("dataraw/screen/{}.pvr", n.to_ascii_lowercase().replace('.', "_"));
            // The GOG install doesn't ship it: `Copy3hatterkep` then draws nothing (the picture's loaded flag is clear).
            if let Ok(img) = Image::load(install, &path) {
                s.background = Some(r.add_texture(&img));
            }
        }
        s
    }

    fn model(&mut self, r: &mut Renderer, install: &Install, name: &str, frame: Option<usize>) -> Option<Model> {
        let key = format!("{name}@{frame:?}");
        if !self.models.contains_key(&key) {
            match Model::load_at(install, VECTORS, name, TEXTURES, frame) {
                Ok(m) => {
                    for p in &m.parts {
                        if let Some(t) = &p.texture {
                            self.texture(r, install, t);
                        }
                        if std::env::var_os("IG2_DIP_DEBUG").is_some() {
                            let mean = p.texture.as_deref().and_then(|t| self.textures.get(t)).map(|t| {
                                let n = (t.img.rgba.len() / 4).max(1) as f32;
                                let s = |c: usize| t.img.rgba.iter().skip(c).step_by(4).map(|&x| x as f32).sum::<f32>() / n;
                                [s(0), s(1), s(2)]
                            });
                            eprintln!("{key}: {} tris {} tex {:?} mean {mean:?} look {:?}", p.path, p.verts.len() / 3, p.texture, p.look);
                        }
                    }
                    self.models.insert(key.clone(), m);
                }
                Err(e) => {
                    eprintln!("ig2-client: {e}");
                    self.models.insert(key.clone(), Model::default());
                }
            }
        }
        self.models.get(&key).cloned()
    }

    fn texture(&mut self, r: &mut Renderer, install: &Install, path: &str) -> Option<&Tex> {
        if !self.textures.contains_key(path) {
            match Image::load(install, path) {
                Ok(img) => {
                    let id = r.add_texture(&img);
                    self.textures.insert(path.to_owned(), Tex { id, img });
                }
                Err(e) => {
                    eprintln!("ig2-client: {e}");
                    return None;
                }
            }
        }
        self.textures.get(path)
    }

    /// The leader's video of `race` (`anim/fejek/` + `KemFejNevek[race]`, `formats/ani.md`), decoded once.
    fn anim(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, race: u8) -> &[TexId] {
        self.anims.entry(race).or_insert_with(|| {
            let Some(name) = rules.diplomacy_screen.head_anims.get(race as usize).filter(|n| n.ends_with(".ani")) else { return Vec::new() };
            let path = format!("anim/fejek/{name}");
            let Ok(bytes) = install.read(&path) else {
                eprintln!("ig2-client: {path}: missing");
                return Vec::new();
            };
            let Ok(ani) = ig2_formats::ani::Ani::parse(&bytes) else { return Vec::new() };
            let (w, h) = (ani.width as u32, ani.height as u32);
            let frames = ani.frames().unwrap_or_default();
            if let Some(dir) = std::env::var_os("IG2_DIP_DUMP") {
                for (i, f) in frames.iter().enumerate().filter(|(i, _)| i % 25 == 0) {
                    let _ = crate::assets::write_png(&std::path::Path::new(&dir).join(format!("ani-{race}-{i}.png")), w, h, &f.rgba);
                }
            }
            // `PlayAnimVector` (ELF 0x45ea70) uploads the frames as they are; the scan lines are the `VIDEO` material's
            // (re/screens/render.md §3).
            frames.into_iter().map(|f| r.add_texture(&Image { width: w, height: h, rgba: f.rgba }))
                .collect()
        })
    }

    /// Draw the frame behind the 2D layer: background, the solid models (offscreen, depth-tested), the wireframes.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, f: &mut Frame, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, scr: &Screen, v: &View, vv: &Virtual, size: [f32; 2], time: f32) {
        // `Copy3hatterkep(0x34, grey 127)`: the picture over the viewport at half brightness.
        if let Some(bg) = self.background {
            let w = 640.0 * vv.k;
            f.push(bg, Blend::GammaAlpha, Instance::new(kind::SPRITE, vv.px(320.0, 240.0), [w.max(size[0]) / 2.0, size[1] / 2.0], [0.5, 0.5, 0.5, 1.0]));
        }
        let d = rules.diplomacy_screen.clone();
        // Lights (§3): ambient 0x141414, the white parallel light from (0, 0, −10), the two side lights
        // (`dipOldalsoFenyekBe` ELF 0x5100d0) moved by `dipMozgatMainFenyek` (0x50f5b0) to `(sin(20k)·300,
        // cos(20k)·200, −350)` and the same at +1.57075 (`k = dipkeringeto`); the engine's model (`lighting`).
        let k = 0.0006 * scr.ui_ticks as f32 * 20.0;
        let side = |a: f32| [((a as f64).sin() * 300.0) as f32, ((a as f64).cos() * 200.0) as f32, -350.0];
        let base = lighting::Lights::default().ambient(0x141414).parallel(0x808080, [0.0, 0.0, -10.0]).parallel(0x7f5d00, side(k)).parallel(0x004e7f, side(k + 1.57075));
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let mut grid: Vec<(V3, V3, [f32; 4])> = Vec::new();
        let mut wires: Vec<(V3, V3, [f32; 4])> = Vec::new();
        let frames = time * 60.0;
        // The background grid: `dipbck8.xvb` child 0 (`DipWireGomb`), animated over frames 80..90.
        let wf = 80 + ((frames * 0.25) as usize % 10);
        if let Some(m) = self.model(r, install, "dipbck8.xvb", Some(wf)) {
            let first = m.parts.first().map(|p| p.path.split('/').nth(1).unwrap_or("").to_owned()).unwrap_or_default();
            let t = Placement::add_to_scene(m.origin, 1.1).world((0.0, 0.0, 0.0), [0.0, 0.0, Z]);
            // Only the ambient light reaches the grid [obs: a uniform `racsszin` × 0x14 on the capture].
            for p in m.parts.iter().filter(|p| p.path.split('/').nth(1) == Some(first.as_str())) {
                self.wire(p, &t, &base, false, &mut grid);
            }
        }
        // The cages and portraits.
        let selected = scr.selected;
        for (i, h) in d.heads.iter().enumerate().take(HEADS) {
            let alive = v.alive_slot(i);
            let spin = if alive { 0.02 * frames } else { -0.005 * frames };
            let o = self.spin0[i];
            let rot = if alive { (o[0] + spin, o[1] + spin, o[2] + spin) } else { (o[0] + spin, o[1], o[2]) };
            let mut lights = base.clone();
            if i as i32 == selected {
                // `JelolFej`: the selected head's blue mark.
                lights = lights.ambient(0x860E0E);
            }
            if let Some(s) = v.slots.get(i) {
                let blink = (time * 1000.0 / 200.0) as i64 % 2 == 0;
                if s.open == 0 || (s.asked != 0 && blink) {
                    lights = lights.ambient(0x0E0E86);
                }
                if s.waiting != 0 && blink && alive {
                    lights = lights.ambient(0x0E860E);
                }
            }
            if let Some(m) = self.model(r, install, "golya.xvb", None) {
                let t = Placement::add_to_scene(m.origin, 0.04).world(rot, *h);
                for p in &m.parts {
                    self.wire(p, &t, &lights, true, &mut wires);
                }
            }
            if i >= v.slots.len() {
                continue;
            }
            // The portrait square: sub-mesh 0 takes `TexturaNevek[2(race − 1) + blocked]`.
            let e = v.empire(i).clamp(0, 18) as usize;
            let race = v.races[e].max(1) as usize;
            let blocked = v.raw.get(i).and_then(|b| b.get(0x15)).copied().unwrap_or(0) != 0;
            let portrait = d.portraits.get(2 * (race - 1) + blocked as usize).and_then(|n| install.resolve_texture(TEXTURES, n));
            if let Some(m) = self.model(r, install, &format!("negyzet{}.xvb", i + 1), None) {
                let t = Placement::add_to_scene(m.origin, 0.04).world((0.0, 0.0, 0.0), *h);
                let first = m.parts.first().map(|p| p.path.split('/').nth(1).unwrap_or("").to_owned()).unwrap_or_default();
                for p in &m.parts {
                    let tex = if p.path.split('/').nth(1) == Some(first.as_str()) { portrait.clone().or(p.texture.clone()) } else { p.texture.clone() };
                    let id = tex.and_then(|t| self.texture(r, install, &t).map(|x| x.id)).unwrap_or(self.white);
                    bake(p, &t, &base, id, 1.0, &mut dynamic, &mut draws);
                }
            }
        }
        // The round frame and the leader's video, turned half round (`dipAtvaneFordulva = π`) once at rest.
        if selected >= 0 {
            let half = (std::f32::consts::PI, 0.0);
            let turn = if scr.turn >= 30.0 { half.0 } else { scr.turn * 0.104_719_76 };
            let push = if scr.turn >= 30.0 { 0.0 } else if scr.turn >= 16.0 { (29.0 - scr.turn) * 30.0 } else { scr.turn * 30.0 };
            let ry = scene::axis_matrix([0.0, 1.0, 0.0], turn);
            if let Some(m) = self.model(r, install, "tube.xvb", None) {
                let p0 = Placement::add_to_scene(m.origin, 0.148);
                let t = scene::Transform { matrix: scene::mul(&ry, &p0.matrix).map(|x| x * 1024.0), translation: [0.0, 0.0, Z + push] };
                for p in &m.parts {
                    let id = p.texture.as_deref().and_then(|t| self.texture(r, install, t).map(|x| x.id)).unwrap_or(self.white);
                    bake(p, &t, &base, id, 1.0, &mut dynamic, &mut draws);
                }
            }
            let e = v.empire(selected as usize).clamp(0, 18) as usize;
            let race = v.races[e];
            let frame = {
                let a = self.anim(r, install, rules, race);
                (!a.is_empty() && scr.turn > 14.0).then(|| a[((time * 25.0) as usize) % a.len()])
            };
            if let (Some(m), Some(tex)) = (self.model(r, install, "kor.xvb", None), frame) {
                let p0 = Placement::add_to_scene(m.origin, 0.105);
                let t = scene::Transform { matrix: scene::mul(&ry, &p0.matrix).map(|x| x * 1024.0), translation: [0.0, 0.0, Z + push] };
                let white = lighting::Lights::default().ambient(0xffffff);
                for p in &m.parts {
                    bake(p, &t, &white, tex, 1.0, &mut dynamic, &mut draws);
                    // The video's texture takes the remaster's `VIDEO` material.
                    if let Some(d) = draws.last_mut() {
                        d.video = true;
                    }
                }
            }
        }
        let line = |f: &mut Frame, list: Vec<(V3, V3, [f32; 4])>| {
            for (a, b, c) in list {
                if let (Some(pa), Some(pb)) = (project(a), project(b)) {
                    f.push(self.white, Blend::Alpha, Instance::line(vv.px(pa[0], pa[1]), vv.px(pb[0], pb[1]), vv.k.max(1.0), c));
                }
            }
        };
        // The grid is the far background: under the solid models.
        line(f, grid);
        if !dynamic.is_empty() {
            let px = ((640.0 * vv.k).round().max(1.0) as u32, size[1].round().max(1.0) as u32);
            let mut fr = Frame::new([0.0, 0.0, 0.0, 0.0]);
            let mut u = uniforms();
            u.eye[3] = self.acttime.unwrap_or(time);
            fr.scene = Some(Scene3d { uniforms: u, gamma: true, shadow: None, draws, dynamic });
            let target = match &mut self.target {
                Some(t) => t,
                None => self.target.insert(r.target(px)),
            };
            r.draw_into(&fr, target, px, 0.0);
            let tex = target.tex;
            let half = [320.0 * vv.k, size[1] / 2.0];
            f.push(tex, Blend::GammaPremul, Instance::new(kind::SPRITE, vv.px(320.0, 240.0), half, [1.0; 4]));
        }
        line(f, wires);
    }

    /// `wirekirak`: the part's flagged triangle edges as lines, each end in its vertex's lit colour times the
    /// texture's there; `cull` drops the faces turned away (hidden lines).
    fn wire(&self, p: &scene::Part, t: &scene::Transform, lights: &lighting::Lights, cull: bool, out: &mut Vec<(V3, V3, [f32; 4])>) {
        let img = p.texture.as_deref().and_then(|t| self.textures.get(t)).map(|t| &t.img);
        let colour = |v: &scene::Vert| -> [f32; 3] {
            let l = lights.rgb(lighting::normal(&t.matrix, v.lnormal));
            let tc = img.map_or([1.0; 3], |im| sample(im, v.uv));
            [l[0] * tc[0], l[1] * tc[1], l[2] * tc[2]]
        };
        for (ti, tri) in p.verts.chunks_exact(3).enumerate() {
            let flags = p.edges.get(ti).copied().unwrap_or(7);
            let pts: Vec<V3> = tri.iter().map(|v| t.point(v.pos)).collect();
            if cull {
                let (Some(a), Some(b), Some(c)) = (project(pts[0]), project(pts[1]), project(pts[2])) else { continue };
                if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.0 {
                    continue;
                }
            }
            let cols: Vec<[f32; 3]> = tri.iter().map(colour).collect();
            for (bit, (a, b)) in [(0, (2, 0)), (1, (0, 1)), (2, (1, 2))] {
                if flags & (1 << bit) != 0 {
                    let c = [(cols[a][0] + cols[b][0]) / 2.0, (cols[a][1] + cols[b][1]) / 2.0, (cols[a][2] + cols[b][2]) / 2.0, 1.0];
                    out.push((pts[a], pts[b], c));
                }
            }
        }
    }
}

/// Camera space → the virtual screen (`fov2` 600, `xcenter` 320, `ycenter` 240; y grows downwards).
fn project(p: V3) -> Option<[f32; 2]> {
    let w = -p[2];
    (w > 1.0).then(|| [600.0 * p[0] / w + 320.0, 600.0 * p[1] / w + 240.0])
}

/// The mesh pass's constants: the clip mapping of [`project`], vertex colours carry the light (ambient 1, no sun).
fn uniforms() -> SceneUniforms {
    let (a, b) = (1.0 / (1.0 / 200.0 - 1.0 / 3000.0) / 200.0, 1.0 / (1.0 / 200.0 - 1.0 / 3000.0));
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
    }
}

/// The texture's colour at `uv` (repeating), 0..1 RGB.
fn sample(img: &Image, uv: [f32; 2]) -> [f32; 3] {
    let (w, h) = (img.width.max(1) as f32, img.height.max(1) as f32);
    let x = ((uv[0].rem_euclid(1.0)) * w).min(w - 1.0) as usize;
    let y = ((uv[1].rem_euclid(1.0)) * h).min(h - 1.0) as usize;
    let i = (y * img.width as usize + x) * 4;
    img.rgba.get(i..i + 3).map_or([1.0; 3], |c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0])
}

/// Bake a camera-fixed part with CPU-lit vertex colours.
fn bake(p: &scene::Part, t: &scene::Transform, lights: &lighting::Lights, tex: TexId, alpha: f32, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let start = out.len() as u32;
    let a = match p.look {
        scene::Look::Glass(g) => g * alpha,
        _ => alpha,
    };
    for v in &p.verts {
        let n = scene::normalise(scene::apply(&t.matrix, v.normal));
        let c = lights.rgb(lighting::normal(&t.matrix, v.lnormal));
        // The colour bytes multiply the texture in the gamma pass (re/screens/render.md §2).
        out.push(MeshVertex { pos: t.point(v.pos), normal: n, uv: v.uv, color: [c[0], c[1], c[2], a] });
    }
    draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false });
}
