//! The design screen's 3D scene (re/screens/design.md §3, §5.1): `Design3DInit` (ELF 0x5035b0 / exe 0x4ad360) and
//! `Design3DUpdate` (0x503ea0 / exe 0x4ad8e0). The console `desbck.xvb` at `SetFrame(desframe)` with its wire sphere
//! (child 0, `SetFrame(desframe + deswireframe)`) and its six buttons (child 6, their material's texture swapped by
//! `FrissitGombTexturak`), and the chosen design's hull (`LoadAlapVektor` ELF 0x5006c0) sliding in and turning.
//! Lit on the CPU like the original's engine (ambient 0x141414, the white parallel light, the two coloured side
//! lights of `desOldalsoFenyekBe`, moved by `desMozgatMainFenyek`); wireframes are drawn as lines in that light.

use std::collections::HashMap;

use ig2_formats::Install;
use ig2_sim::colony_side::TechTable;
use ig2_sim::save::SaveState;

use crate::assets::Image;
use crate::design::{Design, ADD_ZOOM_MAX};
use crate::font::Virtual;
use crate::lighting;
use crate::render::{kind, Blend, Frame, Instance, Renderer, Target, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement, V3};

const VECTORS: &str = "vector/design/screen/";
const TEXTURES: &str = "textures/design/screen/";
const HULL_VECTORS: &str = "vector/design/alapok/";
/// `desbck`'s children (`GetSubMeshid(DesMainMesh, …, i)`): the wire sphere, the buttons, the console's cover
/// (`desMeshKiBe(7, …)`, hidden while open).
const WIRE: u16 = 0;
const BUTTONS: u16 = 6;
const COVER: u16 = 7;
/// Which hot-spot lights each 3D button (`FrissitGombTexturak`: `dGombok[i]`, viewing ids 1..=4, the pod count
/// buttons 0x1f/0x20).
const BUTTON_SPOT: [u32; 6] = [1, 0x1f, 0x20, 2, 3, 4];

/// A texture on the GPU and on the CPU (the wireframes sample it per vertex).
struct Tex {
    id: TexId,
    img: Image,
}

/// The screen's 3D resources.
pub struct DesignScene {
    models: HashMap<String, Option<Model>>,
    textures: HashMap<String, Tex>,
    white: TexId,
    target: Option<Target>,
    /// `DES_MSK.RAW`: the hot-spots' colours.
    pub mask: Vec<u8>,
}

impl DesignScene {
    pub fn new(r: &mut Renderer, install: &Install) -> Self {
        Self { models: HashMap::new(), textures: HashMap::new(), white: r.add_texture(&Image::white()), target: None, mask: crate::design::mask(install) }
    }

    fn model(&mut self, r: &mut Renderer, install: &Install, vdir: &str, name: &str, tdir: &str, frame: Option<usize>) -> Option<Model> {
        let key = format!("{vdir}{name}@{tdir}@{frame:?}");
        if !self.models.contains_key(&key) {
            let m = Model::load_at(install, vdir, name, tdir, frame).map_err(|e| eprintln!("ig2-client: {e}")).ok();
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

    /// `desbck` at `frame` (`SetFrame` keeps 1/256 steps between two keys).
    fn console(&mut self, r: &mut Renderer, install: &Install, frame: f32) -> Option<Model> {
        let (f0, f1, t) = scene::frame_keys(frame, 121);
        let a = self.model(r, install, VECTORS, "desbck.xvb", TEXTURES, Some(f0))?;
        if t == 0.0 {
            return Some(a);
        }
        let b = self.model(r, install, VECTORS, "desbck.xvb", TEXTURES, Some(f1))?;
        Some(a.lerp(&b, t))
    }

    /// `LoadAlapVektor(m)`: `AlapNevek[m]` from `vector/design/alapok/` (models 5..=11 take the race digit at [4]),
    /// textured from `talalmanyok` (m < 5), `spacewar` (5..=14), `urbazis` (15..=18), `tankalapok` (19..=24) or
    /// `alapok`.
    fn hull(&mut self, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, m: usize, race: i16) -> Option<Model> {
        let name = rules.design_screen.models.get(m).filter(|n| !n.is_empty())?;
        let mut name = name.clone().into_bytes();
        if (5..12).contains(&m) && name.len() > 4 {
            name[4] = b'0'.wrapping_add(race as u8);
        }
        let name = String::from_utf8_lossy(&name).into_owned();
        let tdir = match m {
            0..=4 => "textures/talalmanyok/",
            5..=14 => "textures/spacewar/",
            15..=18 => "textures/design/urbazis/",
            19..=24 => "textures/design/tankalapok/",
            _ => "textures/design/alapok/",
        };
        self.model(r, install, HULL_VECTORS, &name, tdir, None)
    }

    /// Draw the scene behind the 2D layer: the wire sphere, the solid models (offscreen, depth-tested).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, f: &mut Frame, r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules, s: &Design, state: &SaveState, techs: &TechTable, en: usize, size: [f32; 2]) {
        let vv = Virtual::new(size);
        // The camera: `ycenter = 250 − desframe·10/60` while animated, else 240.
        let yc = if s.animated { 250.0 - s.frame * 10.0 / 60.0 } else { 240.0 };
        let project = |p: V3| -> Option<[f32; 2]> {
            let w = -p[2];
            (w > 1.0).then(|| [600.0 * p[0] / w + 320.0, 600.0 * p[1] / w + yc])
        };
        // Lights (a node sees its parents' and its own, `lighting`): `Design3DInit`'s ambient 0x141414 and white
        // parallel light from (0, 0, −10) on the scene; `desOldalsoFenyekBe` (ELF 0x4fff30) puts the blue 0x7f5d00 on
        // the buttons (child 6) only and the orange 0x004e7f on the scene; `desMozgatMainFenyek` (0x500010) moves them to
        // `(sin(20k)·300, cos(20k)·200, −350)` and the same at +1.57075 (`k = deskeringeto`).
        let k = s.keringeto * 20.0;
        let side = |a: f32| [((a as f64).sin() * 300.0) as f32, ((a as f64).cos() * 200.0) as f32, -350.0];
        let lights = lighting::Lights::default().ambient(0x141414).parallel(0x808080, [0.0, 0.0, -10.0]).parallel(0x004e7f, side(k + 1.57075));
        let button_lights = lights.clone().parallel(0x7f5d00, side(k));
        // Only the ambient light reaches the wire sphere [obs: its lines (13, 17, 19) = the texture × 0x14].
        let ambient = lighting::Lights::default().ambient(0x141414);
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let mut grid: Vec<(V3, V3, [f32; 4])> = Vec::new();
        // The console: `AddToScene(desbck, 1.1)` at (0, 0, −650), once the first slide-in is done at
        // `AddZoomDes − 650`.
        let z = if s.opened && !s.closing && s.settled { s.add_zoom - 650.0 } else { -650.0 };
        let open = s.opened && !s.closing;
        let panels = [1, 2, 3].map(|i| install.resolve_texture(TEXTURES, &format!("des_panel{i}.bmp")));
        if let Some(m) = self.console(r, install, s.frame) {
            let t = Placement::add_to_scene(m.origin, 1.1).world((0.0, 0.0, 0.0), [0.0, 0.0, z]);
            for p in &m.parts {
                match p.index.first().copied() {
                    Some(WIRE | 1..=5) => continue,
                    Some(COVER) if open => continue,
                    _ => {}
                }
                // `FrissitGombTexturak`: a button whose hot-spot is on shows `des_panel1` (viewing), else `des_panel3`.
                let tex = if p.index.first() == Some(&BUTTONS) {
                    let i = p.index.get(1).copied().unwrap_or(0) as usize;
                    let on = BUTTON_SPOT.get(i).is_some_and(|&id| s.spot_state(state, techs, rules, id));
                    panels[if on { 0 } else { 2 }].clone().or(p.texture.clone())
                } else {
                    p.texture.clone()
                };
                let id = tex.and_then(|t| self.texture(r, install, &t).map(|x| x.id)).unwrap_or(self.white);
                let l = if p.index.first() == Some(&BUTTONS) { &button_lights } else { &lights };
                bake(p, &t, l, id, 1.0, true, &mut dynamic, &mut draws);
            }
        }
        // The wire sphere: child 0 at `SetFrame(desframe + deswireframe)`, while animated moved to
        // `(0, 0, 200 + (1 − f/60)·1000)` (`MoveObj` replaces the child's translation) at transparency `1 − f/60`
        // (`desSetObjTransparencyH`: at least 0.01, above 0.95 gone).
        let tr = if s.animated { 1.0 - s.frame / 60.0 } else if s.frame <= 30.0 { 1.35 - s.frame * 2.0 / 60.0 } else { 0.0 };
        let tr = if tr > 0.95 { 1.0 } else { tr.max(0.01) };
        if tr < 1.0 {
            if let Some(mut m) = self.console(r, install, s.frame + s.wire) {
                if s.animated {
                    m.move_child(&[WIRE], Some([0.0, 0.0, (200.0 + (1.0 - s.frame / 60.0) * 1000.0) / 1024.0]), None);
                }
                let t = Placement::add_to_scene(m.origin, 1.1).world((0.0, 0.0, 0.0), [0.0, 0.0, z]);
                for p in m.below(&[WIRE]) {
                    self.wire(p, &t, &ambient, 1.0 - tr, &mut grid);
                }
            }
        }
        // The hull: `AddToScene(AlapAranyok[m]·0.5)` (smaller for empire 1's models 5..=10), at
        // `(0, −15, AddZoomDes − 1400)` once the screen is open, fading with the slide (`AddZoomDes / ADDZOOMMAXDES`;
        // closing `1 − f/60`).
        if s.loaded >= 0 && s.opened {
            let m = s.loaded as usize;
            let race = s.record(state).map_or(0, |d| i16::from_le_bytes([d[crate::design::field::RACE], d[crate::design::field::RACE + 1]]));
            if let Some(model) = self.hull(r, install, rules, m, race) {
                let mut k = rules.design_screen.scale.get(m).copied().unwrap_or(1.0) * 0.5;
                if en == 1 {
                    k *= match m {
                        5 | 6 => 0.5,
                        7 | 8 => 0.6,
                        9 | 10 => 0.8,
                        _ => 1.0,
                    };
                }
                let pl = Placement::add_to_scene(model.origin, k);
                let t = scene::Transform { matrix: scene::mul(&s.turn, &pl.matrix).map(|x| x * 1024.0), translation: [0.0, -15.0, s.add_zoom - 1400.0] };
                let tr = if s.closing { 1.0 - s.frame / 60.0 } else { s.add_zoom / ADD_ZOOM_MAX };
                let tr = if tr > 0.95 { 1.0 } else { tr.max(0.01) };
                if tr < 1.0 {
                    for p in &model.parts {
                        let id = p.texture.as_deref().and_then(|t| self.texture(r, install, t).map(|x| x.id)).unwrap_or(self.white);
                        bake(p, &t, &lights, id, 1.0 - tr, true, &mut dynamic, &mut draws);
                    }
                }
            }
        }
        // The sphere is the far background: under the solid models.
        for (a, b, c) in grid {
            if let (Some(pa), Some(pb)) = (project(a), project(b)) {
                f.push(self.white, Blend::GammaAdditive, Instance::line(vv.px(pa[0], pa[1]), vv.px(pb[0], pb[1]), vv.k.max(1.0), c));
            }
        }
        if dynamic.is_empty() {
            return;
        }
        let wide = crate::kit::Wide::new(&vv);
        let px = wide.px;
        let mut fr = Frame::new([0.0, 0.0, 0.0, 0.0]);
        let mut u = uniforms(yc);
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

/// The mesh pass's constants: the clip mapping of the projection (`fov2` 600, `xcenter` 320, `ycenter` `yc`), vertex
/// colours carry the light (ambient 1, no sun).
fn uniforms(yc: f32) -> SceneUniforms {
    let (a, b) = (1.0 / (1.0 / 200.0 - 1.0 / 3000.0) / 200.0, 1.0 / (1.0 / 200.0 - 1.0 / 3000.0));
    let rows4 = [[1200.0 / 640.0, 0.0, 0.0, 0.0], [0.0, -1200.0 / 480.0, -(1.0 - 2.0 * yc / 480.0), 0.0], [0.0, 0.0, -a, -b], [0.0, 0.0, -1.0, 0.0]];
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

/// Bake a camera-fixed part with CPU-lit vertex colours; `cull` drops the triangles turned away (the engine's
/// back-face test).
#[allow(clippy::too_many_arguments)]
fn bake(p: &scene::Part, t: &scene::Transform, lights: &lighting::Lights, tex: TexId, alpha: f32, cull: bool, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
    let start = out.len() as u32;
    let a = match p.look {
        scene::Look::Glass(g) => g * alpha,
        _ => alpha,
    };
    for tri in p.verts.chunks_exact(3) {
        let w: [V3; 3] = std::array::from_fn(|i| t.point(tri[i].pos));
        if cull && crate::research::faces_away(&w) {
            continue;
        }
        for (v, q) in tri.iter().zip(w) {
            let n = scene::normalise(scene::apply(&t.matrix, v.normal));
            let c = lights.rgb(lighting::normal(&t.matrix, v.lnormal));
            // The colour bytes multiply the texture in the gamma pass (re/screens/render.md §2).
            out.push(MeshVertex { pos: q, normal: n, uv: v.uv, color: [c[0], c[1], c[2], a] });
        }
    }
    draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false });
}
