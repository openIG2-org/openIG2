//! The 3D frames of the info and messages screens, rendered into a texture over the virtual 640 × 480 screen
//! (re/screens/info.md §1.1, re/screens/messages.md §4.1).
//!
//! Both screens put a camera-fixed model (`SetRelative(false)`) in front of the eye: `Infoing3DInit` (ELF
//! 0x555b50) `info.xvb` ×1.22, `RotateObj(π/2, 0, 0)`, at (0, 15, −650), posed at `infoframe` (open:
//! `infoMAXFRAMES` = 45), with the race heads, the chart button's blinking marker `negyzet1.xvb` and the money
//! triangle's knob `hszog0.xvb`; `Messages3DInit` (ELF 0x5a19a0) `messbck.xvb` ×0.9 at (33, 10, −650), posed at
//! `mesframe` (15 = the list, 30 = a message open). The projection is the screens' `xcenter` 320, `ycenter` 240
//! with the focal 600: `vx = 600·x/w + 320`, `vy = 600·y/w + 240` (w = −z). Vertices are lit on the CPU like the
//! original's engine (`lighting`: the screens' ambient and white parallel light, the two coloured side lights, the
//! info heads' marks) and drawn in the remaster's gamma-space mesh pass (re/screens/render.md §2, §5).

use std::collections::HashMap;

use ig2_formats::Install;

use crate::render::{Frame, Renderer, Target, TexId};
use crate::lighting::Lights;
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Look, Model, Placement, M3, V3};

/// `infoMAXFRAMES` (ELF 0x117abac).
pub const INFO_MAX_FRAMES: f32 = 45.0;

/// `InfoMeshSorrend` (ELF 0x117abb0): the head ring of empire `k + 1` under the frame's node 3.
const HEAD_RING: [u16; 8] = [7, 5, 4, 2, 3, 1, 6, 8];

/// `GombMeshek` (ELF 0x117aca0, 19 × 4 i32, −1 = no deeper level): the chart buttons' meshes (index
/// `Info_SelectedChart`, +1 above 4) and the star-log buttons.
const BUTTON_MESH: [[i32; 4]; 19] = [
    [1, 1, -1, -1],
    [1, 2, -1, -1],
    [1, 3, -1, -1],
    [1, 4, -1, -1],
    [1, 5, -1, -1],
    [1, 6, -1, -1],
    [1, 7, 0, 6],
    [1, 7, 0, 1],
    [1, 7, 0, 2],
    [1, 7, 0, 3],
    [1, 7, 0, 4],
    [1, 7, 0, 5],
    [1, 9, -1, -1],
    [1, 0, -1, -1],
    [1, 0x11, -1, -1],
    [1, 0xf, -1, -1],
    [1, 0x10, -1, -1],
    [1, 7, 1, 1],
    [1, 7, 1, 2],
];

/// What the info frame shows.
#[derive(Debug, Clone, Default)]
pub struct InfoFrame {
    /// `BirodalomIsmeri[EN][e]` for `e` = 1..=8: the heads shown.
    pub known: [bool; 8],
    /// `PlayerFaj[e]` for `e` = 1..=8: the heads' textures `textures/diplomac/<race>a.bmp`.
    pub races: [u8; 8],
    /// `Info_SelectedChart`; `None` in star-log mode (the marker is hidden).
    pub chart: Option<usize>,
    /// `InfoVillogtato` (+0.04 per 10 ms UI frame).
    pub blink: f32,
    /// `InfoTriX`, `InfoTriY`.
    pub tri: [f32; 2],
    /// `infoframe`: 45 open, up to 75 with the star log.
    pub frame: f32,
    /// `infokeringeto`: the side lights' clock.
    pub turn: f32,
    /// `InfoSelected` (1..=8).
    pub selected: usize,
    /// `BirodalomEl[e]` for `e` = 1..=8.
    pub alive: [bool; 8],
    /// The heads' fader clock `InfoFejF+0x4d8`.
    pub head_clock: f32,
    /// The heads carrying `RarakFenyekFejekre`'s slot-1 light.
    pub head_lit: [bool; 8],
    /// The list's models by row slot (`RotateBolygoFlottak`, re/screens/info.md §6.4).
    pub globes: [Option<GlobeView>; 4],
    /// `Info_StarLogMode`: the left buttons wear `info_ures.bmp` / `info_j6.bmp` (`SetUresTexturak`, §6.8).
    pub star_log: bool,
}

/// A list slot's model: a planet `vector/info/bolygo_k.xvb` with `textures/starmap/p<surface>.bmp`, or the fleet
/// `vector/spacewar/flag10.xvb` (`LoadBolygoUrhajok(1)`), turned by `rotation` (the matrix after the scale).
#[derive(Debug, Clone, Copy)]
pub struct GlobeView {
    pub fleet: bool,
    pub rotation: M3,
    pub surface: u8,
    /// `RarakPirosFenytBolygora`'s ambient light (`InfoVillogtato`'s red), if the planet has a big problem.
    pub red: Option<u32>,
    /// `MoveObj` to `InfoBF+0x7c` and the opacity (1 − `SetObjTransparency`'s; the detail mode fades them).
    pub at: V3,
    pub alpha: f32,
}

/// `infoMozgatMainFenyek` (ELF 0x554e80, inlined at exe 0x4ef450 into `Infoing3DUpdate`): the side light at angle
/// `a` = `infokeringeto·20` (+1.57075 for the second), `(sin a·300, cos a·200, −650)`.
fn info_side(a: f32) -> V3 {
    [((a as f64).sin() as f32) * 300.0, ((a as f64).cos() as f32) * 200.0, -650.0]
}

/// The lights of the info screen's nodes (`Infoing3DInit` ELF 0x555b50 / exe 0x4ea8a0): on `mainmeshid` the ambient
/// 0x1e1e1e, the parallel 0x9e9e9e from (0, 0, −10) and the side light 0x004e7f (`InfoMainLight4`); on `InfoMainMesh`
/// (the frame only) the side light 0x7f5d00 (`InfoMainLight3`).
fn info_lights(s: &InfoFrame) -> (Lights, Lights) {
    let k = s.turn * 20.0;
    let root = Lights::default().ambient(0x1e1e1e).parallel(0x9e9e9e, [0.0, 0.0, -10.0]).parallel(0x004e7f, info_side(k + 1.57075));
    let frame = root.clone().parallel(0x7f5d00, info_side(k));
    (root, frame)
}

/// `RarakFenyekFejekre` (ELF 0x554790): the ambient lights on head `e`'s node `InfoFejek[e]` (3/ring/1/0). Slot 0 is
/// the selected head's, grey `(int)(sin(t·20)·127/2 + 192)` with `t` the heads' fader clock; slot 1 (kept once set)
/// red (200, 50, 50) for a dead empire, else `(100 − x, 100 − x/2, 100 + x)` with `x = (int)(s·100) − 89` clamped
/// to 0..100 and `s` = the fader's `+0x168 + 4e` = 0.9 (without its random shine) [inf: x = 0, the ELF's f64 product].
fn head_lights(s: &InfoFrame, k: usize, mut l: Lights) -> Lights {
    let e = k + 1;
    if e == s.selected {
        let p = (((s.head_clock as f64) * 20.0).sin() * 127.0) as f32;
        let g = (p / 2.0 + 192.0) as i32 as u32 & 0xff;
        l = l.ambient(g | g << 8 | g << 16);
    }
    if s.head_lit[k] {
        let rgb = |r: u32, g: u32, b: u32| r | g << 8 | b << 16;
        l = l.ambient(if !s.alive[k] {
            rgb(200, 50, 50)
        } else {
            let x = (((0.9f32 as f64) * 100.0) as i32 - 89).clamp(0, 100) as u32;
            rgb(100 - x, 100 - x / 2, 100 + x)
        });
    }
    l
}

/// The models and the texture they are drawn into.
pub struct FrameModels {
    white: TexId,
    textures: HashMap<String, TexId>,
    info: HashMap<usize, Option<Model>>,
    globes: HashMap<usize, Option<Model>>,
    flag: Option<Option<Model>>,
    marker: Option<Option<Model>>,
    knob: Option<Option<Model>>,
    messages: HashMap<usize, Option<Model>>,
    target: Option<Target>,
}

fn load(install: &Install, dir: &str, name: &str, tex: &str, frame: Option<usize>) -> Option<Model> {
    Model::load_at(install, dir, name, tex, frame).map_err(|e| eprintln!("ig2-client: {e}")).ok()
}

/// `LoadBox(name)` drawn at the end of its level-of-detail chain (the root's `lod_archive`: `flag10` → `flag11` →
/// `flag12`; re/screens/starmap.md §3.3: the renderer steps down the chain for objects more than a few pixels across),
/// placed by the chain head's half-extents (`AddToScene`).
fn load_last(install: &Install, dir: &str, name: &str, tex: &str) -> Option<Model> {
    let head = load(install, dir, name, tex, None)?;
    let mut stem = name.trim_end_matches(".xvb").to_string();
    for _ in 0..8 {
        let next = install.read(&format!("dataraw/{dir}{stem}.xv")).ok().and_then(|b| {
            let ar = ig2_formats::xv::Archive::parse(&b).ok()?;
            let h = ig2_formats::xv::NodeHeader::parse(ar.root(Some(&stem))?.data).ok()?;
            (!h.lod_archive.is_empty()).then(|| h.lod_archive.trim_end_matches(".xvb").to_string())
        });
        match next {
            Some(n) => stem = n,
            None => break,
        }
    }
    let mut last = load(install, dir, &format!("{stem}.xvb"), tex, None)?;
    last.origin = head.origin;
    Some(last)
}

/// The bounding-box centre of the parts under `prefix` (`GetObjCenter`), in the placed model's space.
fn centre(model: &Model, prefix: &[u16], m: &M3, at: V3) -> Option<V3> {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in model.parts.iter().filter(|p| p.index.starts_with(prefix)) {
        for v in &p.verts {
            let q = scene::apply(m, v.pos);
            for k in 0..3 {
                lo[k] = lo[k].min(q[k] + at[k]);
                hi[k] = hi[k].max(q[k] + at[k]);
            }
        }
    }
    (lo[0] <= hi[0]).then(|| std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0))
}

impl FrameModels {
    pub fn new(r: &mut Renderer) -> Self {
        Self {
            white: r.add_texture(&crate::assets::Image::white()),
            textures: HashMap::new(),
            info: HashMap::new(),
            globes: HashMap::new(),
            flag: None,
            marker: None,
            knob: None,
            messages: HashMap::new(),
            target: None,
        }
    }

    fn texture(&mut self, r: &mut Renderer, install: &Install, path: Option<&str>) -> TexId {
        let Some(path) = path else { return self.white };
        if let Some(&t) = self.textures.get(path) {
            return t;
        }
        let t = match crate::assets::Image::load(install, path) {
            Ok(img) => r.add_texture(&img),
            Err(e) => {
                eprintln!("ig2-client: {e}");
                self.white
            }
        };
        self.textures.insert(path.to_owned(), t);
        t
    }

    /// Bake `model`'s parts that `keep` accepts, with matrix `m` at `at`, lit per vertex by the part's `lights`
    /// (`lighting::normal` of its `lnormal`); `skin` may replace a part's texture.
    #[allow(clippy::too_many_arguments)]
    fn bake(&mut self, r: &mut Renderer, install: &Install, model: &Model, m: &M3, at: V3, alpha: f32, keep: &dyn Fn(&[u16]) -> bool, skin: &dyn Fn(&[u16]) -> Option<String>, lights: &dyn Fn(&[u16]) -> Lights, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
        for p in &model.parts {
            if !keep(&p.index) {
                continue;
            }
            let a = match p.look {
                Look::Glass(g) => g * alpha,
                _ => alpha,
            };
            let start = out.len() as u32;
            let l = lights(&p.index);
            for tri in p.verts.chunks_exact(3) {
                // The engine culls the triangles facing away (re/screens/research.md §2): a translucent globe shows
                // no back hemisphere.
                let w: [V3; 3] = std::array::from_fn(|i| {
                    let q = scene::apply(m, tri[i].pos);
                    [q[0] + at[0], q[1] + at[1], q[2] + at[2]]
                });
                if crate::research::faces_away(&w) {
                    continue;
                }
                for (v, q) in tri.iter().zip(w) {
                    // The colour bytes multiply the texture in the gamma pass (re/screens/render.md §2); transparent
                    // parts are lit too, their opacity in alpha.
                    // The lighting normal carries the child record's scale (`flag12`'s 2.84, `bolygo_k`'s 0.85): the
                    // engine lights by its direction [obs: the ship lit like the exe's only once normalised].
                    let c = l.rgb(crate::lighting::normal(m, scene::normalise(v.lnormal)));
                    out.push(MeshVertex { pos: q, normal: scene::normalise(scene::apply(m, v.normal)), uv: v.uv, color: [c[0], c[1], c[2], a] });
                }
            }
            let tex = skin(&p.index).or_else(|| p.texture.clone());
            let tex = self.texture(r, install, tex.as_deref());
            draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
        }
    }

    /// Draw a scene over the virtual screen into the target, in the gamma pass (the vertex colours carry the light).
    fn finish(&mut self, r: &mut Renderer, dynamic: Vec<MeshVertex>, draws: Vec<MeshDraw>, size: [f32; 2]) -> Option<TexId> {
        if dynamic.is_empty() {
            return None;
        }
        let wide = crate::kit::Wide::frame(&crate::font::Virtual::new(size));
        let px = wide.px;
        // Camera space → clip over the virtual 640 × 480: vx = 600·x/w + 320, vy = 600·y/w + 240 (w = −z); depth
        // 0..1 over w = 50..3000 (the info screen's detail globe comes to w = 150).
        let (n, f) = (50.0f32, 3000.0f32);
        let (a, b) = (1.0 / (1.0 / n - 1.0 / f) / n, 1.0 / (1.0 / n - 1.0 / f));
        let rows = [[1200.0 / 640.0, 0.0, 0.0, 0.0], [0.0, -1200.0 / 480.0, 0.0, 0.0], [0.0, 0.0, -a, -b], [0.0, 0.0, -1.0, 0.0]];
        let rows = wide.rows(rows);
        let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
        let mut frame = Frame::new([0.0, 0.0, 0.0, 0.0]);
        frame.scene = Some(Scene3d {
            gamma: true,
            shadow: None,
            uniforms: SceneUniforms {
                view,
                eye: [0.0; 4],
                light: [0.0, 0.0, 1.0, 0.0],
                ambient: [1.0, 1.0, 1.0, 0.0],
                sun: [0.0; 4],
                fog: [0.0; 4],
                fog_range: [1e9, 2e9, 0.0, 1e9],
            },
            draws,
            dynamic,
        });
        let target = match &mut self.target {
            Some(t) => t,
            None => self.target.insert(r.target(px)),
        };
        r.draw_into(&frame, target, px, 0.0);
        Some(target.tex)
    }

    /// The info screen's frame at `infoframe` (`SetFrame(InfoMainMesh, infoframe)`, whole frames [inf]).
    pub fn render_info(&mut self, r: &mut Renderer, install: &Install, s: &InfoFrame, size: [f32; 2]) -> Option<TexId> {
        let pose = s.frame.max(0.0) as usize;
        let info = self.info.entry(pose).or_insert_with(|| load(install, "vector/info/", "info.xvb", "textures/info/", Some(pose))).clone()?;
        let marker = self.marker.get_or_insert_with(|| load(install, "vector/info/", "negyzet1.xvb", "textures/info/", None)).clone();
        let knob = self.knob.get_or_insert_with(|| load(install, "vector/info/", "hszog0.xvb", "textures/info/", None)).clone();
        let (mut dynamic, mut draws) = (Vec::new(), Vec::new());
        let place = Placement::add_to_scene(info.origin, 1.22);
        let m = place.rotated(std::f32::consts::FRAC_PI_2, 0.0, 0.0).map(|x| x * 1024.0);
        let at = [0.0, 15.0, -650.0];
        // `Infoing3DInit` hides node 1's children 0xe..=0x13, `InfoShowSelectedGomb` `GombMeshek[11]`, `UnhideIsmertFajFejeket` the heads (3/ring/1) of the empires not met.
        let hidden = |ix: &[u16]| -> bool {
            if ix.len() >= 2 && ix[0] == 1 && (0xe..=0x13).contains(&ix[1]) {
                return true;
            }
            if ix.starts_with(&[1, 7, 0, 5]) {
                return true;
            }
            // `KisGolyokAtlatszosaga`: the list discs 1/10..=13/0 under the globes are hidden once open (transparency
            // 1 from `infoMAXFRAMES`; 0.7 before, not drawn translucent here).
            if s.frame >= INFO_MAX_FRAMES && ix.len() >= 3 && ix[0] == 1 && (10..=13).contains(&ix[1]) && ix[2] == 0 {
                return true;
            }
            if ix.len() >= 3 && ix[0] == 3 && ix[2] == 1 {
                if let Some(k) = HEAD_RING.iter().position(|&h| h == ix[1]) {
                    return !s.known[k];
                }
            }
            false
        };
        // `ChangeTexture(3/ring/1/0, InfoTexturaDescriptorok[race − 1])`: `textures/diplomac/<race>a.bmp`.
        let heads: Vec<Option<String>> = s.races.iter().map(|&race| install.resolve_texture("textures/diplomac/", &format!("{}a.bmp", race.max(1)))).collect();
        // `SetUresTexturak` (star-log mode): the left buttons `GombMeshek[0..5]`' child 0 wear `info_ures.bmp`, the
        // sixth `info_j6.bmp` (`BaloldaliGombok` ELF 0x117ac60).
        let empty = [install.resolve_texture("textures/info/", "info_ures.bmp"), install.resolve_texture("textures/info/", "info_j6.bmp")];
        let skin = |ix: &[u16]| -> Option<String> {
            if ix.len() >= 4 && ix[0] == 3 && ix[2] == 1 && ix[3] == 0 {
                let k = HEAD_RING.iter().position(|&h| h == ix[1])?;
                return heads[k].clone();
            }
            if s.star_log && ix.len() >= 3 && ix[0] == 1 && (1..=6).contains(&ix[1]) && ix[2] == 0 {
                return empty[(ix[1] == 6) as usize].clone();
            }
            None
        };
        // `SetJobbAtl((infoframe − 45)/30)` while the star log opens or closes: the right buttons `GombMeshek[6..11]`
        // (1/7/0/1..=6) that transparent.
        let right = |ix: &[u16]| ix.len() >= 4 && ix[..3] == [1, 7, 0] && (1..=6).contains(&ix[3]);
        let fade = ((s.frame - INFO_MAX_FRAMES) / 30.0).clamp(0.0, 1.0);
        let (root, framel) = info_lights(s);
        let heads_l: Vec<Lights> = (0..8).map(|k| head_lights(s, k, framel.clone())).collect();
        let lights = |ix: &[u16]| -> Lights {
            if ix.len() >= 4 && ix[0] == 3 && ix[2] == 1 && ix[3] == 0 {
                if let Some(k) = HEAD_RING.iter().position(|&h| h == ix[1]) {
                    return heads_l[k].clone();
                }
            }
            framel.clone()
        };
        self.bake(r, install, &info, &m, at, 1.0, &|ix| !hidden(ix) && !(fade > 0.0 && right(ix)), &skin, &lights, &mut dynamic, &mut draws);
        if fade > 0.0 {
            self.bake(r, install, &info, &m, at, 1.0 - fade, &|ix| !hidden(ix) && right(ix), &skin, &lights, &mut dynamic, &mut draws);
        }
        // `InfoShowSelectedGomb`: the marker at the selected chart button's centre (−1, −3), its transparency
        // `(sin(InfoVillogtato) + 1)/4`.
        if let (Some(c), Some(mk)) = (s.chart, &marker) {
            let b = BUTTON_MESH[if c > 4 { c + 1 } else { c }];
            let prefix: Vec<u16> = b.iter().take_while(|&&i| i >= 0).map(|&i| i as u16).collect();
            if let Some(p) = centre(&info, &prefix, &m, at) {
                let mm = Placement::add_to_scene(mk.origin, 0.031).rotated(0.0, 0.0, 0.0).map(|x| x * 1024.0);
                let alpha = 1.0 - ((s.blink.sin() + 1.0) / 4.0);
                let n = draws.len();
                self.bake(r, install, mk, &mm, [p[0] - 1.0, p[1] - 3.0, p[2]], alpha, &|_| true, &|_| None, &|_| root.clone(), &mut dynamic, &mut draws);
                // `PropertiesOff(InfoKijelolesId, 8 | 4 | 2)` (`Infoing3DInit`): additive, so `fa.bmp`'s black inside
                // adds nothing and only its blue rim shows.
                for d in &mut draws[n..] {
                    (d.glass, d.additive) = (true, true);
                }
            }
        }
        // `RotateBolygoFlottak`: the list's models at their `InfoBF+0x7c` (home (−310, 59k + 40, −650)), on
        // `mainmeshid`'s lights (+ the red ambient light of a planet with a big problem).
        for (k, g) in s.globes.iter().enumerate() {
            let Some(g) = g else { continue };
            let model = if g.fleet {
                self.flag.get_or_insert_with(|| load_last(install, "vector/spacewar/", "flag10.xvb", "textures/spacewar/")).clone()
            } else {
                self.globes.entry(k).or_insert_with(|| load(install, "vector/info/", &format!("bolygo_{k}.xvb"), "textures/info/", None)).clone()
            };
            let Some(model) = model else { continue };
            let place = Placement::add_to_scene(model.origin, if g.fleet { 0.025 } else { 0.029 });
            let mm = scene::mul(&g.rotation, &place.matrix).map(|x| x * 1024.0);
            let at = g.at;
            // `BolygoTexturak[surface − 1]` = `textures/starmap/p<surface>.bmp` (the ninth is p1 again).
            let n = match g.surface {
                1..=8 => g.surface,
                _ => 1,
            };
            let tex = (!g.fleet).then(|| install.resolve_texture("textures/starmap/", &format!("p{n}.bmp"))).flatten();
            let skin = |ix: &[u16]| if ix.first() == Some(&0) { tex.clone() } else { None };
            // The remaster's `ig2_vertex.shader` scales the UVs by the texture's `uv_mul` (re/screens/starmap.md §3.3).
            let mut model = model;
            if let Some(t) = &tex {
                let k = crate::assets::Image::uv_mul(install, t);
                for p in model.parts.iter_mut().filter(|p| p.index.first() == Some(&0)) {
                    for v in &mut p.verts {
                        v.uv = [v.uv[0] * k[0], v.uv[1] * k[1]];
                    }
                }
            }
            let l = match g.red {
                Some(c) => root.clone().ambient(c),
                None => root.clone(),
            };
            self.bake(r, install, &model, &mm, at, g.alpha, &|_| true, &skin, &|_| l.clone(), &mut dynamic, &mut draws);
        }
        // The money triangle's knob (`MoveObj(InfoHId, InfoTriX·4.5 − 1438, InfoTriY·4.5 − 1050, −2650)`).
        if let Some(k) = &knob {
            let mm = Placement::add_to_scene(k.origin, 0.03).rotated(0.0, 0.0, 0.0).map(|x| x * 1024.0);
            let at = [s.tri[0] * 4.5 - 1438.0, s.tri[1] * 4.5 - 1050.0, -2650.0];
            self.bake(r, install, k, &mm, at, 1.0, &|_| true, &|_| None, &|_| root.clone(), &mut dynamic, &mut draws);
        }
        self.finish(r, dynamic, draws, size)
    }

    /// The messages screen's frame at `mesframe` = `frame` (15 = the list, 30 = a message open).
    pub fn render_messages(&mut self, r: &mut Renderer, install: &Install, frame: usize, size: [f32; 2]) -> Option<TexId> {
        let model = self.messages.entry(frame).or_insert_with(|| load(install, "vector/messages/", "messbck.xvb", "textures/messages/", Some(frame))).clone()?;
        let (mut dynamic, mut draws) = (Vec::new(), Vec::new());
        let m = Placement::add_to_scene(model.origin, 0.9).rotated(0.0, 0.0, 0.0).map(|x| x * 1024.0);
        // `Messages3DUpdate`: sub-meshes 1/6, 2/7 and 4/0 are 40 % transparent.
        let glass = |ix: &[u16]| ix.starts_with(&[1, 6]) || ix.starts_with(&[2, 7]) || ix.starts_with(&[4, 0]);
        let mut model = model;
        for p in &mut model.parts {
            if glass(&p.index) {
                p.look = Look::Glass(0.6);
            }
        }
        // `Messages3DInit` (ELF 0x5a19a0 / exe 0x506970): ambient 0x141414, the parallel 0x808080 from (0, 0, −10) and
        // the side lights 0x7f5d00 / 0x004e7f from (200, 0, −350) (the exe's 0x50a000, ported as `resOldalsoFenyekBe`),
        // all on `mainmeshid`; `mesMozgatMainFenyek` is never called: they stay there.
        let l = Lights::default().ambient(0x141414).parallel(0x808080, [0.0, 0.0, -10.0]).parallel(0x7f5d00, [200.0, 0.0, -350.0]).parallel(0x004e7f, [200.0, 0.0, -350.0]);
        self.bake(r, install, &model, &m, [33.0, 10.0, -650.0], 1.0, &|_| true, &|_| None, &|_| l.clone(), &mut dynamic, &mut draws);
        self.finish(r, dynamic, draws, size)
    }
}

/// Draw a frame texture over the virtual screen (the target covers the 4:3 frame, [`crate::kit::Wide::frame`]).
pub fn draw(f: &mut Frame, tex: TexId, v: &crate::font::Virtual) {
    use crate::render::{kind, Blend, Instance};
    let c = v.px(320.0, 240.0);
    f.push(tex, Blend::GammaPremul, Instance::new(kind::SPRITE, c, crate::kit::Wide::frame(v).half(v), [1.0; 4]));
}

