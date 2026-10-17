//! The 3D frames of the info and messages screens, rendered into a texture over the virtual 640 × 480 screen
//! (re/screens/info.md §1.1, re/screens/messages.md §4.1).
//!
//! Both screens put a camera-fixed model (`SetRelative(false)`) in front of the eye: `Infoing3DInit` (ELF
//! 0x555b50) `info.xvb` ×1.22, `RotateObj(π/2, 0, 0)`, at (0, 15, −650), posed at `infoframe` (open:
//! `infoMAXFRAMES` = 45), with the race heads, the chart button's blinking marker `negyzet1.xvb` and the money
//! triangle's knob `hszog0.xvb`; `Messages3DInit` (ELF 0x5a19a0) `messbck.xvb` ×0.9 at (33, 10, −650), posed at
//! `mesframe` (15 = the list, 30 = a message open). The projection is the screens' `xcenter` 320, `ycenter` 240
//! with the focal 600: `vx = 600·x/w + 320`, `vy = 600·y/w + 240` (w = −z).

use std::collections::HashMap;

use ig2_formats::Install;

use crate::render::{Frame, Renderer, Target, TexId};
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
}

/// The models and the texture they are drawn into.
pub struct FrameModels {
    white: TexId,
    textures: HashMap<String, TexId>,
    info: HashMap<usize, Option<Model>>,
    marker: Option<Option<Model>>,
    knob: Option<Option<Model>>,
    messages: HashMap<usize, Option<Model>>,
    target: Option<Target>,
}

fn load(install: &Install, dir: &str, name: &str, tex: &str, frame: Option<usize>) -> Option<Model> {
    Model::load_at(install, dir, name, tex, frame).map_err(|e| eprintln!("ig2-client: {e}")).ok()
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

    /// Bake `model`'s parts that `keep` accepts, with matrix `m` at `at`; `skin` may replace a part's texture.
    #[allow(clippy::too_many_arguments)]
    fn bake(&mut self, r: &mut Renderer, install: &Install, model: &Model, m: &M3, at: V3, alpha: f32, keep: &dyn Fn(&[u16]) -> bool, skin: &dyn Fn(&[u16]) -> Option<String>, out: &mut Vec<MeshVertex>, draws: &mut Vec<MeshDraw>) {
        for p in &model.parts {
            if !keep(&p.index) {
                continue;
            }
            let a = match p.look {
                Look::Glass(g) => g * alpha,
                _ => alpha,
            };
            let start = out.len() as u32;
            for v in &p.verts {
                let q = scene::apply(m, v.pos);
                out.push(MeshVertex {
                    pos: [q[0] + at[0], q[1] + at[1], q[2] + at[2]],
                    normal: scene::normalise(scene::apply(m, v.normal)),
                    uv: v.uv,
                    color: [1.0, 1.0, 1.0, a],
                });
            }
            let tex = skin(&p.index).or_else(|| p.texture.clone());
            let tex = self.texture(r, install, tex.as_deref());
            draws.push(MeshDraw { mesh: None, range: start..out.len() as u32, tex, glass: a < 1.0 });
        }
    }

    /// Draw a scene over the virtual screen into the target: `ambient` and `sun` grey levels (0..1) of the
    /// screen's two white lights (the coloured side lights `…OldalsoFenyekBe` are not drawn [inf]).
    fn finish(&mut self, r: &mut Renderer, dynamic: Vec<MeshVertex>, draws: Vec<MeshDraw>, ambient: f32, sun: f32, size: [f32; 2]) -> Option<TexId> {
        if dynamic.is_empty() {
            return None;
        }
        let k = size[1] / 480.0;
        let px = ((640.0 * k).round().max(1.0) as u32, size[1].round().max(1.0) as u32);
        // Camera space → clip over the virtual 640 × 480: vx = 600·x/w + 320, vy = 600·y/w + 240 (w = −z); depth
        // 0..1 over w = 300..3000.
        let (n, f) = (300.0f32, 3000.0f32);
        let (a, b) = (1.0 / (1.0 / n - 1.0 / f) / n, 1.0 / (1.0 / n - 1.0 / f));
        let rows = [[1200.0 / 640.0, 0.0, 0.0, 0.0], [0.0, -1200.0 / 480.0, 0.0, 0.0], [0.0, 0.0, -a, -b], [0.0, 0.0, -1.0, 0.0]];
        let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
        // `CreateLightParalell(…, (0, 0, −10))`: the light shines away from the eye, onto the faces towards it.
        let mut frame = Frame::new([0.0, 0.0, 0.0, 0.0]);
        frame.scene = Some(Scene3d {
            uniforms: SceneUniforms {
                view,
                eye: [0.0; 4],
                light: [0.0, 0.0, 1.0, 1.0],
                ambient: [ambient, ambient, ambient, 0.0],
                sun: [sun, sun, sun, 0.0],
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
            if ix.len() >= 3 && ix[0] == 3 && ix[2] == 1 {
                if let Some(k) = HEAD_RING.iter().position(|&h| h == ix[1]) {
                    return !s.known[k];
                }
            }
            false
        };
        // `ChangeTexture(3/ring/1/0, InfoTexturaDescriptorok[race − 1])`: `textures/diplomac/<race>a.bmp`.
        let heads: Vec<Option<String>> = s.races.iter().map(|&race| install.resolve_texture("textures/diplomac/", &format!("{}a.bmp", race.max(1)))).collect();
        let skin = |ix: &[u16]| -> Option<String> {
            if ix.len() >= 4 && ix[0] == 3 && ix[2] == 1 && ix[3] == 0 {
                let k = HEAD_RING.iter().position(|&h| h == ix[1])?;
                return heads[k].clone();
            }
            None
        };
        self.bake(r, install, &info, &m, at, 1.0, &|ix| !hidden(ix), &skin, &mut dynamic, &mut draws);
        // `InfoShowSelectedGomb`: the marker at the selected chart button's centre (−1, −3), its transparency
        // `(sin(InfoVillogtato) + 1)/4`.
        if let (Some(c), Some(mk)) = (s.chart, &marker) {
            let b = BUTTON_MESH[if c > 4 { c + 1 } else { c }];
            let prefix: Vec<u16> = b.iter().take_while(|&&i| i >= 0).map(|&i| i as u16).collect();
            if let Some(p) = centre(&info, &prefix, &m, at) {
                let mm = Placement::add_to_scene(mk.origin, 0.031).rotated(0.0, 0.0, 0.0).map(|x| x * 1024.0);
                let alpha = 1.0 - ((s.blink.sin() + 1.0) / 4.0);
                self.bake(r, install, mk, &mm, [p[0] - 1.0, p[1] - 3.0, p[2]], alpha, &|_| true, &|_| None, &mut dynamic, &mut draws);
            }
        }
        // The money triangle's knob (`MoveObj(InfoHId, InfoTriX·4.5 − 1438, InfoTriY·4.5 − 1050, −2650)`).
        if let Some(k) = &knob {
            let mm = Placement::add_to_scene(k.origin, 0.03).rotated(0.0, 0.0, 0.0).map(|x| x * 1024.0);
            let at = [s.tri[0] * 4.5 - 1438.0, s.tri[1] * 4.5 - 1050.0, -2650.0];
            self.bake(r, install, k, &mm, at, 1.0, &|_| true, &|_| None, &mut dynamic, &mut draws);
        }
        // `Infoing3DInit`: ambient 0x1e1e1e, the parallel light 0x9e9e9e.
        self.finish(r, dynamic, draws, 0x1e as f32 / 255.0, 0x9e as f32 / 255.0, size)
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
        self.bake(r, install, &model, &m, [33.0, 10.0, -650.0], 1.0, &|_| true, &|_| None, &mut dynamic, &mut draws);
        // `Messages3DInit`: ambient 0x141414, the parallel light 0x808080.
        self.finish(r, dynamic, draws, 0x14 as f32 / 255.0, 0x80 as f32 / 255.0, size)
    }
}

/// Draw a frame texture over the virtual screen (the target covers 640 × 480 virtual pixels).
pub fn draw(f: &mut Frame, tex: TexId, v: &crate::font::Virtual) {
    use crate::render::{kind, Blend, Instance};
    let c = v.px(320.0, 240.0);
    f.push(tex, Blend::Alpha, Instance::new(kind::SPRITE, c, [320.0 * v.k, 240.0 * v.k], [1.0; 4]));
}
