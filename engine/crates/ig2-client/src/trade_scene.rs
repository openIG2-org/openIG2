//! The trade screen's 3D scene (re/screens/trade.md §3.2, §4): `Trade3DInit` (ELF 0x7aa0e0 / exe 0x608030) and the
//! frame half of `Trade3DUpdate` (0x7abfc0 / exe 0x608340). The frame `tozs1bck.xvb` at `SetFrame(traframe)` with its
//! wire sphere (child 0, `SetFrame(traframe + trawireframe)`, moved, turned and faded while the screen opens), its
//! parts 2 and 4 shown by the frame, and its built-in buttons hidden (`EltuntetGombok`). Lit on the CPU like the
//! original's engine: ambient 0x141414, the white parallel light, the two coloured side lights of
//! `traOldalsoFenyekBe` circling (`traMozgatMainFenyek`).

use std::collections::HashMap;

use ig2_formats::Install;

use crate::assets::Image;
use crate::font::Virtual;
use crate::lighting;
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement, V3};
use crate::trade::Trade;

const VECTORS: &str = "vector/trade/";
const TEXTURES: &str = "textures/trade/";
/// `tozs1bck`'s children: the wire sphere (`TraWireGomb`, submesh 0), the parts shown late in the opening (2) and
/// early (4), and the button pairs `EltuntetGombok` hides (exe 0x928de0: child 1's 3, 2, 1, 0, 8, 9, 10, 11).
const WIRE: u16 = 0;
const LATE: u16 = 2;
const EARLY: u16 = 4;
const BUTTONS: [u16; 8] = [3, 2, 1, 0, 8, 9, 10, 11];

/// A texture on the GPU and on the CPU (the wireframes sample it per vertex).
struct Tex {
    id: TexId,
    img: Image,
}

/// The screen's 3D resources.
pub struct TradeScene {
    models: HashMap<String, Option<Model>>,
    textures: HashMap<String, Tex>,
    white: TexId,
    target: Option<Target>,
}

impl TradeScene {
    pub fn new(r: &mut Renderer) -> Self {
        Self { models: HashMap::new(), textures: HashMap::new(), white: r.add_texture(&Image::white()), target: None }
    }

    fn model(&mut self, r: &mut Renderer, install: &Install, frame: usize) -> Option<Model> {
        let key = format!("tozs1bck@{frame}");
        if !self.models.contains_key(&key) {
            let m = Model::load_at(install, VECTORS, "tozs1bck.xvb", TEXTURES, Some(frame)).map_err(|e| eprintln!("ig2-client: {e}")).ok();
            for p in m.iter().flat_map(|m| &m.parts) {
                if let Some(t) = &p.texture {
                    self.texture(r, install, t);
                }
            }
            self.models.insert(key.clone(), m);
        }
        self.models.get(&key).cloned().flatten()
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

    /// `tozs1bck` at `frame` (`SetFrame` keeps 1/256 steps between two keys).
    fn frame_model(&mut self, r: &mut Renderer, install: &Install, frame: f32) -> Option<Model> {
        let (f0, f1, t) = scene::frame_keys(frame, 121);
        let a = self.model(r, install, f0)?;
        if t == 0.0 {
            return Some(a);
        }
        let b = self.model(r, install, f1)?;
        Some(a.lerp(&b, t))
    }

    /// `TradeLoadAlap`'s model (`LoadBox` with its `VectorDir`/`TextureDir`), cached by file.
    fn hull(&mut self, r: &mut Renderer, install: &Install, vdir: &str, name: &str, tdir: &str) -> Option<Model> {
        let key = format!("{vdir}{name}");
        if !self.models.contains_key(&key) {
            let m = Model::load(install, vdir, name, tdir).map_err(|e| eprintln!("ig2-client: {e}")).ok();
            for p in m.iter().flat_map(|m| &m.parts) {
                if let Some(t) = &p.texture {
                    self.texture(r, install, t);
                }
            }
            self.models.insert(key.clone(), m);
        }
        self.models.get(&key).cloned().flatten()
    }

    /// Draw the scene behind the 2D layer: the wire sphere, then the frame and the chosen offer's hull (offscreen,
    /// depth-tested).
    pub fn draw(&mut self, f: &mut Frame, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, s: &Trade, size: [f32; 2]) {
        let vv = Virtual::new(size);
        // The camera: `kamerapont (0, 0, 600)`, `xcenter 320`, `ycenter 240` (`Trade3DUpdate` step 5).
        let project = |p: V3| -> Option<[f32; 2]> {
            let w = -p[2];
            (w > 1.0).then(|| [600.0 * p[0] / w + 320.0, 600.0 * p[1] / w + 240.0])
        };
        // `Trade3DInit`'s ambient 0x141414 and white parallel light from (0, 0, −10); `traOldalsoFenyekBe`'s amber
        // 0x7f5d00 and blue 0x004e7f, moved by `traMozgatMainFenyek` (exe 0x60b450) to `(sin k·300, cos k·200, −850)`
        // and the same at `k + 1.57075` (`k = trakeringeto·20`; the first in f32, the second in f64).
        let k = s.keringeto * 20.0;
        let first = [((k as f64).sin() as f32) * 300.0, ((k as f64).cos() as f32) * 200.0, -850.0];
        let k2 = k as f64 + 1.57075;
        let second = [(k2.sin() * 300.0) as f32, (k2.cos() * 200.0) as f32, -850.0];
        let lights = lighting::Lights::default().ambient(0x141414).parallel(0x808080, [0.0, 0.0, -10.0]).parallel(0x7f5d00, first).parallel(0x004e7f, second);
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let mut grid: Vec<(V3, V3, [f32; 4])> = Vec::new();
        // `AddToScene(tozs1bck, 1.15)` at (0, 15, −650).
        let at = [0.0, 15.0, -650.0];
        // Part 4 is hidden while the info screen is the neighbour (`Trade3DUpdate` step 7 overrides step 3); part 2
        // shows from frame 35 on.
        let hide_early = s.hides_early();
        if let Some(m) = self.frame_model(r, install, s.frame) {
            let t = Placement::add_to_scene(m.origin, 1.15).world((0.0, 0.0, 0.0), at);
            for p in &m.parts {
                let i = &p.index;
                match i.first().copied() {
                    Some(WIRE) => continue,
                    Some(LATE) if s.frame < 35.0 => continue,
                    Some(EARLY) if hide_early => continue,
                    Some(1) if i.get(1).is_some_and(|c| BUTTONS.contains(c)) => continue,
                    _ => {}
                }
                let id = p.texture.clone().and_then(|t| self.texture(r, install, &t).map(|x| x.id)).unwrap_or(self.white);
                // The panels' and the backdrop's material is 0.65 transparent (`Racsszin`, `tozs_hk`; ig2.exe draws them
                // at opacity 89/255). The panels lie over black and hide the backdrop behind them [obs: a flat
                // racsszin × 0.35 there]: drawn opaque at that opacity, they keep the depth the glass pass doesn't write.
                let o = 1.0 - p.transparency.clamp(0.0, 1.0);
                let panel = p.texture.as_deref().is_some_and(|t| t.ends_with("racsszin_bmp.pvr"));
                if panel && o < 1.0 {
                    bake(p, &t, &lights, id, 1.0, o, &mut dynamic, &mut draws);
                } else {
                    bake(p, &t, &lights, id, o, 1.0, &mut dynamic, &mut draws);
                }
            }
        }
        // The hull (`ForgatTradeAlap` exe 0x6068a0): `AddToScene(k·0.08)`, turned by its own matrix, at
        // `(TRADExa − 190 + s, 145, −650)`, `SetObjTransparency(s/s0)`.
        let clear = 1.0 - s.last_transparency;
        if let Some((base, race)) = s.hull_model.filter(|_| s.hull_shown() && clear > 0.0) {
            if let Some((name, vdir, tdir, k)) = crate::trade::hull_file(rules, base, race) {
                if let Some(m) = self.hull(r, install, vdir, &name, tdir) {
                    let pl = Placement::add_to_scene(m.origin, k * 0.08);
                    let t = scene::Transform { matrix: scene::mul(&s.hull_turn, &pl.matrix).map(|x| x * 1024.0), translation: [s.hull_x, 145.0, -650.0] };
                    for p in &m.parts {
                        let id = p.texture.clone().and_then(|t| self.texture(r, install, &t).map(|x| x.id)).unwrap_or(self.white);
                        bake(p, &t, &lights, id, clear, 1.0, &mut dynamic, &mut draws);
                        // `SetObjTransparency` only turns blending on: the hull keeps its depth write; then its lit
                        // windows (`selfillumkirak`, design_scene's `glow`).
                        if let Some(d) = draws.last_mut() {
                            d.depth_write = true;
                        }
                        if let Some(id) = p.self_illum.as_deref().and_then(|t| self.texture(r, install, t).map(|x| x.id)) {
                            crate::design_scene::glow(p, &t, id, 1.0 - clear, &mut dynamic, &mut draws);
                        }
                    }
                }
            }
        }
        // The wire sphere: child 0 at `SetFrame(traframe + trawireframe)`; while animated moved to
        // `(0, x·200, 200 − x·1000)` and turned by `(sin(2x)·3, sin(2.1x)·2, 0)` (`x = 1 − f/50`), else at
        // (0, 0, 200) unturned; transparency `1.35 − 2f/50` up to frame 25 (clamped 1e-6..0.9999999), else 1e-6.
        let v = s.wire_transparency();
        if v < 1.0 {
            if let Some(mut m) = self.frame_model(r, install, s.frame + s.wire) {
                let (to, rot) = s.wire_pose();
                m.move_child(&[WIRE], Some(to.map(|c| c / 1024.0)), Some(&rot));
                let t = Placement::add_to_scene(m.origin, 1.15).world((0.0, 0.0, 0.0), at);
                // `wirekirak`: each end lit by the scene's lights times the texture, the faces turned away culled (as
                // the research screen's sphere) [the exe's lines are ~3x brighter and wider: deviations].
                for p in m.below(&[WIRE]) {
                    self.wire(p, &t, &lights, 1.0 - v, &mut grid);
                }
            }
        }
        for (a, b, c) in grid {
            if let (Some(pa), Some(pb)) = (project(a), project(b)) {
                f.push(self.white, Blend::GammaAdditive, Instance::line(vv.px(pa[0], pa[1]), vv.px(pb[0], pb[1]), 1.0, c));
            }
        }
        if dynamic.is_empty() {
            return;
        }
        let wide = crate::kit::Wide::new(&vv);
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
        let half = wide.half(&vv);
        f.push(tex, Blend::GammaPremul, Instance::new(kind::SPRITE, vv.px(320.0, 240.0), half, [1.0; 4]));
    }

    /// `wirekirak`: the part's flagged triangle edges as lines, each end in its vertex's lit colour times the
    /// texture's there, times the opacity.
    fn wire(&self, p: &scene::Part, t: &scene::Transform, lights: &lighting::Lights, opacity: f32, out: &mut Vec<(V3, V3, [f32; 4])>) {
        let img = p.texture.as_deref().and_then(|t| self.textures.get(t)).map(|t| &t.img);
        let colour = |v: &scene::Vert| -> [f32; 3] {
            let l = lights.rgb(lighting::normal(&t.matrix, v.lnormal));
            let tc = img.map_or([1.0; 3], |im| sample(im, v.uv));
            [l[0] * tc[0] * opacity, l[1] * tc[1] * opacity, l[2] * tc[2] * opacity]
        };
        for (ti, tri) in p.verts.chunks_exact(3).enumerate() {
            let flags = p.edges.get(ti).copied().unwrap_or(7);
            let pts: Vec<V3> = tri.iter().map(|v| t.point(v.pos)).collect();
            // The engine culls the triangles facing away, wireframes too (as the research screen's sphere).
            if crate::research::faces_away(&[pts[0], pts[1], pts[2]]) {
                continue;
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

/// The mesh pass's constants: the clip mapping of the projection (`fov2` 600, `xcenter` 320, `ycenter` 240), vertex
/// colours carry the light (ambient 1, no sun).
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

/// Bake a camera-fixed part with CPU-lit vertex colours (× `k`), the triangles turned away dropped (the engine's
/// back-face test).
#[allow(clippy::too_many_arguments)]
fn bake(p: &scene::Part, t: &scene::Transform, lights: &lighting::Lights, tex: TexId, alpha: f32, k: f32, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {

    let start = out.len() as u32;
    let a = match p.look {
        scene::Look::Glass(g) => g * alpha,
        _ => alpha,
    };
    for tri in p.verts.chunks_exact(3) {
        let w: [V3; 3] = std::array::from_fn(|i| t.point(tri[i].pos));
        if crate::research::faces_away(&w) {
            continue;
        }
        for (v, q) in tri.iter().zip(w) {
            let n = scene::normalise(scene::apply(&t.matrix, v.normal));
            let c = lights.rgb(lighting::normal(&t.matrix, v.lnormal));
            out.push(MeshVertex { pos: q, normal: n, uv: v.uv, color: [c[0] * k, c[1] * k, c[2] * k, a] });
        }
    }
    draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
}
