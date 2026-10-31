//! The starmap screen's HUD (re/screens/starmap.md §11): the control panel `kezel.xvb` (a camera-fixed model of the
//! 3D pass), `StarmapGfx2D_1`'s fader box and the minimap's dark window, `StarmapGfx2D_3`'s short info of the
//! selected planet or fleet (`StarmapRovidInfoKiir`) and the minimap (`GFX_StarmapRadar`).

use ig2_formats::Install;
use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Font, Virtual};
use crate::kit::fader::fade_color;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement};
use crate::world::Snapshot;

/// The virtual rectangle the panel is rendered over (it spans about 214..426 × 366..474).
const CONSOLE_RECT: [f32; 4] = [200.0, 360.0, 440.0, 480.0];
/// Texture pixels per virtual pixel for the panel.
const CONSOLE_SCALE: f32 = 4.0;

/// The control panel: `kezel.xvb` (`Init3dStarmap` ELF 0x6db470: `AddToScene(…, 0.17)`, relative to the camera at
/// (0, 170, −600), not rotated), drawn into a texture over [`CONSOLE_RECT`], again when a button's pose changes.
pub struct Console {
    tex: TexId,
    /// The battle screen's console, lit again each frame with the circling side lights ([`Console::relight`]).
    sides: Option<Box<Sides>>,
    /// The starmap's console, posed again when a button sinks ([`Console::pose`]).
    still: Option<Box<Still>>,
}

/// The starmap console's frame kept for posing its buttons: the model as loaded, the button frames loaded so far,
/// the frame each button (`GetSubMeshid(KezeloId, i)`, i = 0..6) is drawn at.
struct Still {
    frame: Frame,
    target: crate::render::Target,
    size: (u32, u32),
    base: Model,
    keys: Vec<(usize, Model)>,
    depth: [usize; 6],
}

/// The console's button animation (`Starmap_Main` ELF 0x6f50a0 lines 163-257, `ShowStarmap` ELF 0x6eca50 lines
/// 379-381 and 465-473 / exe 0x5a3250): a console hot-spot sets `MelyikGombot` (its sub-mesh), `KezeloGombFrame = 0` and
/// `GombFazis = 1`; a button let go (`Mouse_Felenged`) turns `GombFazis` to 2; once a drawn frame (a UI tick here) the
/// frame steps +5 (to 10) or −5 (to 0) and `SetFrame`s that sub-mesh only, so a button left mid-way keeps its pose.
/// The held rotate, zoom and pitch spots (2, 3, 5, 6) set frame 0 again every frame, so they show frame 5; the views
/// (1, 4) act on the release frame, which also lets go: they never sink.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ConsoleButtons {
    /// `MelyikGombot`.
    pub which: usize,
    /// `GombFazis`: 0 idle, 1 sinking, 2 rising.
    phase: u8,
    /// `KezeloGombFrame`.
    frame: f32,
    /// `Mouse_Felenged` seen since the last step.
    released: bool,
    /// The frame each sub-mesh was last set to.
    pub depth: [f32; 6],
}

impl ConsoleButtons {
    /// The sub-mesh of console hot-spot `spot` (1 top view, 2 rotate right, 3 rotate left, 4 side view, 5 zoom,
    /// 6 pitch); the minimap (7) has none.
    pub fn sub_mesh(spot: u32) -> Option<usize> {
        Some(match spot {
            1 => 4,
            2 => 5,
            3 => 2,
            4 => 1,
            5 => 0,
            6 => 3,
            _ => return None,
        })
    }

    /// `Starmap_Main` on console hot-spot `spot`: `KezeloGombFrame = 0; GombFazis = 1; MelyikGombot = …`.
    pub fn press(&mut self, spot: u32) {
        if let Some(m) = Self::sub_mesh(spot) {
            (self.frame, self.phase, self.which) = (0.0, 1, m);
        }
    }

    /// A button let go (`Mouse_Felenged`).
    pub fn release(&mut self) {
        self.released = true;
    }

    /// `ShowStarmap`'s step.
    pub fn step(&mut self) {
        if std::mem::take(&mut self.released) {
            self.phase = 2;
        }
        match self.phase {
            1 => self.frame = (self.frame + 5.0).min(10.0),
            2 => self.frame = (self.frame - 5.0).max(0.0),
            _ => {}
        }
        self.depth[self.which] = self.frame;
    }
}

/// The panel's frame, its target size and the scene's parallel light.
type PanelFrame = (Frame, (u32, u32), [f32; 3]);

/// The battle screen's parallel lights in camera space for `smkeringeto` = `kering` (see [`Console::relight`]): the
/// starmap scene's 0x808080 at (10, 10, −10) and `smOldalsoFenyekBe`'s two side lights, each with its colour.
pub fn scene_lights(m: &[[f32; 3]; 3], kering: f32) -> [([f32; 3], [f32; 3]); 3] {
    let a = (kering * 20.0) as f64;
    // The engine compares the vertex normal with z negated (`n'`) against the light's vector, so the camera-space
    // light is used with z negated [obs fx8b: the console ring 2.0 against 10 without the lights].
    let cam = |d: [f32; 3]| {
        let l = scene::normalise(std::array::from_fn(|i| m[i][0] * d[0] + m[i][1] * d[1] + m[i][2] * d[2]));
        [l[0], l[1], -l[2]]
    };
    let side = |t: f64| cam([(t.sin() * 300.0) as f32, (t.cos() * 200.0) as f32, 0.0]);
    [(cam([10.0, 10.0, -10.0]), rgb(0x808080)), (side(a), rgb(0x7f5d00)), (side(a + 1.57075), rgb(0x004e7f))]
}

/// Colour words are the bytes [r, g, b] (`0x00BBGGRR`).
fn rgb(c: u32) -> [f32; 3] {
    [(c & 0xff) as f32 / 255.0, ((c >> 8) & 0xff) as f32 / 255.0, ((c >> 16) & 0xff) as f32 / 255.0]
}

/// One vertex's light as `fenyekszamolasa` sums it: the ambient 0x404040 plus each parallel light's
/// `shade[float2int(|n − L|²·2047)]` of `SetLights(2, 0.15)` (`sqrt(1 − i/2896)·255`, `>> 8`), saturating.
pub fn engine_light(lights: &[([f32; 3], [f32; 3])], n: [f32; 3]) -> [f32; 3] {
    let shade = |c: f32| {
        let i = ((2.0 - 2.0 * c) * 2047.0).round();
        if i < 2896.0 { ((1.0 - i / 2896.0).sqrt() * 255.0).floor() / 256.0 } else { 0.0 }
    };
    let mut c = rgb(0x404040);
    for (l, col) in lights {
        let d = shade(n[0] * l[0] + n[1] * l[1] + n[2] * l[2]);
        for k in 0..3 {
            c[k] += col[k] * d;
        }
    }
    c.map(|x| x.min(1.0))
}

/// The console's frame kept for lighting it on the CPU: the vertices' normals, the scene's lights, the target.
struct Sides {
    frame: Frame,
    normals: Vec<[f32; 3]>,
    target: crate::render::Target,
    size: (u32, u32),
}

impl Console {
    /// Lit as a still frame of its lights: the scene's ambient 0x404040 and parallel 0x808080, and the two
    /// coloured lights `smOldalsoFenyekBe` (ELF 0x6d81e0) gives it, 0x7f5d00 and 0x004e7f (`0x00BBGGRR`), which
    /// `smMozgatMainFenyek` circles in front of it [inf: one warm light from the front, the shader has one].
    pub fn load(r: &mut Renderer, install: &Install) -> Result<Self, String> {
        Self::load_lit(r, install, [0.502 + 0.498, 0.502 + 0.306, 0.502])
    }

    /// The panel under another parallel light colour `sun` (the battle screen's, re/screens/spacewar.md §5.1).
    pub fn load_lit(r: &mut Renderer, install: &Install, sun: [f32; 3]) -> Result<Self, String> {
        let (frame, size, _) = Self::frame(r, install, sun)?;
        let mut target = r.target(size);
        r.draw_into(&frame, &mut target, size, 0.0);
        let base = Self::model(install, None)?;
        Ok(Self { tex: target.tex, sides: None, still: Some(Box::new(Still { frame, target, size, base, keys: Vec::new(), depth: [0; 6] })) })
    }

    /// `kezel.xvb` at key `frame` (`None`: as loaded).
    fn model(install: &Install, frame: Option<usize>) -> Result<Model, String> {
        Model::load_at(install, "vector/starmap/", "kezel.xvb", "textures/starmap/", frame)
    }

    /// The buttons at `depth` (`SetFrame` of `GetSubMeshid(KezeloId, i)`, whole frames as the steps of 5 leave them):
    /// each sub-mesh's vertices from the model at its frame, drawn again when one changed.
    pub fn pose(&mut self, r: &mut Renderer, install: &Install, depth: &[f32; 6]) {
        let Some(st) = self.still.as_deref_mut() else { return };
        let depth = depth.map(|d| d.max(0.0) as usize);
        if depth == st.depth {
            return;
        }
        st.depth = depth;
        for &d in &depth {
            if d != 0 && !st.keys.iter().any(|(k, _)| *k == d) {
                match Self::model(install, Some(d)) {
                    Ok(m) => st.keys.push((d, m)),
                    Err(e) => eprintln!("ig2-client: kezel.xvb frame {d}: {e}"),
                }
            }
        }
        let place = Placement::add_to_scene(st.base.origin, 0.17).world((0.0, 0.0, 0.0), [0.0, 170.0, -600.0]);
        let Some(sc) = st.frame.scene.as_mut() else { return };
        let mut at = 0;
        for (k, p) in st.base.parts.iter().enumerate() {
            let d = p.index.first().and_then(|&i| depth.get(i as usize)).copied().unwrap_or(0);
            let key = st.keys.iter().find(|(f, _)| d != 0 && *f == d).and_then(|(_, m)| m.parts.get(k));
            let src = key.filter(|q| q.verts.len() == p.verts.len()).unwrap_or(p);
            for (v, dst) in src.verts.iter().zip(&mut sc.dynamic[at..at + p.verts.len()]) {
                dst.pos = place.point(v.pos);
                dst.normal = scene::normalise(scene::apply(&place.matrix, v.normal));
            }
            at += p.verts.len();
        }
        r.draw_into(&st.frame, &mut st.target, st.size, 0.0);
    }

    /// The panel with `smOldalsoFenyekBe`'s two parallel lights on it (ELF 0x6d81e0: colours 0x7f5d00 and 0x004e7f,
    /// `0x00BBGGRR`), which `smMozgatMainFenyek` (ELF 0x6d8290) moves every frame to `(sin a·300, cos a·200, 0)` and
    /// the same at `a + 1.57075`, `a = smkeringeto·20` (exe 0xF176DC); lit per vertex on the CPU ([`Console::relight`]).
    pub fn load_sides(r: &mut Renderer, install: &Install, kering: f32, m: &[[f32; 3]; 3]) -> Result<Self, String> {
        let (mut frame, size, _) = Self::frame(r, install, [0.0; 3])?;
        // The engine multiplies the texture by the vertex colour on the stored values (the remaster's gamma pass).
        if let Some(sc) = frame.scene.as_mut() {
            sc.gamma = true;
        }
        let normals = frame.scene.as_ref().map_or(Vec::new(), |s| s.dynamic.iter().map(|v| v.normal).collect());
        let target = r.target(size);
        let mut c = Self { tex: target.tex, sides: Some(Box::new(Sides { frame, normals, target, size })), still: None };
        c.relight(r, kering, m);
        Ok(c)
    }

    /// Light the side-lit panel for `smkeringeto` = `kering` as the engine's `fenyekszamolasa` does (re/screens/research.md
    /// §2.1, per vertex, saturating; the shader's pre-lit path, in gamma space): the starmap scene's ambient 0x404040 and
    /// parallel light 0x808080 at (10, 10, −10) (`Init3dStarmap`) and the two side lights, world vectors `(h, x, y)`
    /// turned into camera space by the camera's rows `m` (the root's rotation chain is the camera's `cmatrix`).
    pub fn relight(&mut self, r: &mut Renderer, kering: f32, m: &[[f32; 3]; 3]) {
        let Some(sd) = self.sides.as_deref_mut() else { return };
        let lights = scene_lights(m, kering);
        if let Some(sc) = sd.frame.scene.as_mut() {
            for (v, n) in sc.dynamic.iter_mut().zip(&sd.normals) {
                let c = engine_light(&lights, *n);
                v.normal = [0.0; 3];
                v.color = [c[0], c[1], c[2], 0.0];
            }
        }
        r.draw_into(&sd.frame, &mut sd.target, sd.size, 0.0);
    }

    /// The panel's frame: the model at its place under the scene's lights, and the target size.
    fn frame(r: &mut Renderer, install: &Install, sun: [f32; 3]) -> Result<PanelFrame, String> {
        let light = scene::normalise([0.0, -0.35, 1.0]);
        let model = Self::model(install, None)?;
        let place = Placement::add_to_scene(model.origin, 0.17).world((0.0, 0.0, 0.0), [0.0, 170.0, -600.0]);
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let white = r.add_texture(&crate::assets::Image::white());
        for p in &model.parts {
            let start = dynamic.len() as u32;
            for v in &p.verts {
                dynamic.push(MeshVertex {
                    pos: place.point(v.pos),
                    normal: scene::normalise(scene::apply(&place.matrix, v.normal)),
                    uv: v.uv,
                    color: [1.0, 1.0, 1.0, 0.0],
                });
            }
            // Drawn opaque: the window in the texture's middle is black where the exe draws it (no alpha test).
            let tex = match p.texture.as_deref().map(|t| crate::assets::Image::load(install, t)) {
                Some(Ok(mut img)) => {
                    img.rgba.chunks_exact_mut(4).for_each(|px| px[3] = 255);
                    r.add_texture(&img)
                }
                _ => white,
            };
            draws.push(MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex, glass: false, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
        }
        let [x0, y0, x1, y1] = CONSOLE_RECT;
        let (w, h) = (x1 - x0, y1 - y0);
        // Camera space → clip for the virtual window: vx = 600·x/w + 320, vy = 600·y/w + 250 (w = −z); depth
        // 0..1 over w = 500..700.
        let rows = [
            [1200.0 / w, 0.0, -(2.0 * (320.0 - x0) / w - 1.0), 0.0],
            [0.0, -1200.0 / h, -(1.0 - 2.0 * (250.0 - y0) / h), 0.0],
            [0.0, 0.0, -3.5, -1750.0],
            [0.0, 0.0, -1.0, 0.0],
        ];
        let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
        let mut f = Frame::new([0.0, 0.0, 0.0, 0.0]);
        f.scene = Some(Scene3d {
            gamma: false,
            shadow: None,
            uniforms: SceneUniforms {
                view,
                eye: [0.0; 4],
                light: [light[0], light[1], light[2], 0.0],
                // `Init3dStarmap`: ambient 0x404040, the parallel light 0x808080.
                ambient: [0.251, 0.251, 0.251, 0.0],
                sun: [sun[0], sun[1], sun[2], 0.0],
                fog: [0.0; 4],
                fog_range: [1e9, 2e9, 0.0, 1e9],
                flash: [[0.0; 4]; 2],
            },
            draws,
            dynamic,
        });
        Ok((f, ((w * CONSOLE_SCALE) as u32, (h * CONSOLE_SCALE) as u32), light))
    }

    pub(crate) fn draw(&self, f: &mut Frame, v: &Virtual) {
        let [x0, y0, x1, y1] = CONSOLE_RECT;
        let c = v.px((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        f.push(self.tex, Blend::Alpha, Instance::new(kind::SPRITE, c, [(x1 - x0) * v.k / 2.0, (y1 - y0) * v.k / 2.0], [1.0; 4]));
    }
}

/// The minimap's inside (`GFX_StarmapRadar` ELF 0x70e490: `HatarX1..Y2` = 274..365 × 388..452).
pub const MINIMAP: [f32; 4] = [274.0, 388.0, 365.0, 452.0];

/// A galaxy point on the minimap: `((±x/100/3)·92)/2 + 46 + 274`, `((y/100/3)·65)/2 + 32 + 388`.
pub fn minimap_point(p: [f32; 2]) -> [f32; 2] {
    [(((-p[0] / 100.0) / 3.0) * 92.0) / 2.0 + 46.0 + 274.0, (((p[1] / 100.0) / 3.0) * 65.0) / 2.0 + 32.0 + 388.0]
}

/// What the minimap shows.
#[derive(Debug, Clone, Default)]
pub struct Minimap {
    /// Seen planets: position, `DDColor` index (`owner + 15`, unowned 12), selected.
    pub planets: Vec<([f32; 2], usize, bool)>,
    pub fleets: Vec<([f32; 2], usize, bool)>,
    /// The view's footprint: the galaxy points under the virtual corners (0, 20), (640, 20), (0, 480), (640, 480).
    pub view: [Option<[f32; 2]>; 4],
}

impl Minimap {
    /// From the frame's snapshot (fleets interpolated) and the selection (`StarmapMode`, `AktPlanet`, `AktFleet`).
    pub fn new(s: &Snapshot, fleets: &[crate::world::FleetView], sel: &crate::orders::Selection, cam: &crate::starmap::Camera) -> Self {
        let colour = |o: u8| if o == 0 { 12 } else { o as usize + 15 };
        Self {
            planets: s.planets.iter().filter(|p| p.known).map(|p| (p.pos, colour(p.owner), sel.mode == 1 && sel.planet == p.index)).collect(),
            fleets: fleets.iter().filter(|f| f.visible).map(|f| (f.pos, f.owner as usize + 15, sel.mode == 2 && sel.fleet == f.slot)).collect(),
            view: [[0.0, 20.0], [640.0, 20.0], [0.0, 480.0], [640.0, 480.0]].map(|c| cam.virtual_to_world(c)),
        }
    }
}

/// The short info lines (`StarmapRovidInfoKiir(50, 424, 0)`): text, x, y, size, `DDColor` index; and the brushes
/// `StarmapGfx2D_3` puts beside them: (record, frame, x, y).
#[derive(Debug, Clone, Default)]
pub struct InfoBox {
    pub lines: Vec<(Vec<u8>, f32, f32, usize, usize)>,
    pub brushes: Vec<(usize, i32, f32, f32)>,
}

/// `Starmap_RopIkonKonvert` (exe 0x927088): the icon frames of `StarmapBrushok` 9 by button.
const ICON_FRAME: [i32; 20] = [0, 13, 9, 10, 8, 1, 2, 6, 7, 5, 0, 0, 0, 3, 4, 0, 11, 12, 14, 0];

/// `GetString` texts the box needs.
pub struct InfoTexts<'a> {
    pub texts: &'a crate::ui::UiTexts,
    /// `StringReadAll`'s source (`Moral_Nev`, `Problemak_Nev`).
    pub txt17: &'a crate::ui::UiTexts,
    pub empire_names: &'a [Vec<u8>],
}

impl InfoTexts<'_> {
    fn s(&self, file: u16, section: u16, index: u16) -> Vec<u8> {
        self.texts.get(file, section, index)
    }
    /// `txt17` §4 from the Hungarian file: `Problemak_Nev[p]` = 21 + p, `Moral_Nev[k]` = 41 + k.
    fn names(&self, index: u16) -> Vec<u8> {
        self.txt17.get(0x11, 4, index)
    }
    fn empire(&self, e: u8) -> Vec<u8> {
        self.empire_names.get(e as usize).cloned().unwrap_or_default()
    }
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

impl InfoBox {
    /// The exe's `StarmapRovidInfoKiir` (exe 0x5afc50) for planet mode (`StarmapMode` 1) or fleet mode, at
    /// (x, y) = (50, 424). `step` = how many 30-frame steps of `Counter` the planet has been shown (the morale line,
    /// then each problem in turn).
    pub fn new(state: &SaveState, rules: &ig2_rules::Rules, t: &InfoTexts, sel: &crate::orders::Selection, step: i32, counter: i32) -> Self {
        let (x, y) = (50.0, 424.0);
        let m = &state.mem;
        let en = m.i32(va::EN).clamp(0, 18) as u8;
        let debug = m.u8(va::DEBUGX) != 0;
        let mut lines = Vec::new();
        let mut brushes = Vec::new();
        let mut put = |s: Vec<u8>, dx: f32, dy: f32, size: usize, col: usize| lines.push((s, x + dx, y + dy, size, col));
        if sel.mode == 1 {
            let p = sel.planet;
            if p < 1 || p > state.planet_count() {
                return Self::default();
            }
            let r = m.read(va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, ig2_sim::save::PLANET_SIZE);
            let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            let owner = r[0x20];
            // `Starmap_MouseInit2` (ELF 0x7024b0): the tanks button (`StarmapTanksGomb`: the player's fleet near) and
            // the production button (`StarmapProductGomb`) on the player's planet.
            let (tanks, product) = crate::starmap_panels::Panels::buttons(state, sel);
            if tanks {
                brushes.push((9, ICON_FRAME[1], 171.0, 430.0));
            }
            if product || debug {
                brushes.push((9, ICON_FRAME[2], 419.0, 430.0));
            }
            put(cstr(&r[..0x18]).to_vec(), 0.0, 3.0, 2, 0x22);
            // Race 7 sees planets without a `+0xcb` flag; a satellite (`+0xc4 + EN`) shows the details.
            let seen = owner == en || (en == 7 && r[0xcb] == 0) || r[0xc4 + en as usize] != 0 || debug;
            if seen {
                put(b"\x03:".to_vec(), -42.0, 3.0, 2, 1);
                put(cat(&[richness(state, rules, p, en).to_string().as_bytes(), b"%"]), -32.0, 4.0, 1, 1);
                let race = if owner == 0 { en as usize } else { r[0x21] as usize };
                let growth = rules.race_surface_affinity.get(race).and_then(|v| v.get(r[0x22] as usize)).copied().unwrap_or(0);
                put(b"\x01:".to_vec(), -42.0, 42.0, 2, 1);
                put(cat(&[growth.to_string().as_bytes(), b"%"]), -32.0, 43.0, 1, 1);
            }
            if owner != en {
                let level = if en == 7 && r[0xcb] == 0 { 1 } else { r[0xc4 + en as usize] };
                if level != 0 || (en == 7 && r[0xcb] == 0) {
                    // `Talalmanyok[level + 0xe]`: the satellite's tech name.
                    put(crate::hud::tech_name(t.texts, level as usize + 0xe), 0.0, 33.0, 1, 0x23);
                }
            }
            put(t.empire(owner), 0.0, 15.0, 1, 0x23);
            if owner != 0 && seen {
                let line = format!(": {} ({:+})", i32_at(0x28), i32_at(0x2c));
                put(cat(&[&t.s(2, 3, 1), line.as_bytes()]), 0.0, 24.0, 1, 0x23);
            }
            if owner == en || (debug && owner != 0) {
                let order = problem_order(&r, debug);
                let k = step.max(0) as usize % (order.len() + 1);
                let text = if k < 1 {
                    let morale = (i32_at(0x30) / 100) as f32 as i32;
                    cat(&[&t.s(2, 3, 2), b": ", &t.names((41 + morale).clamp(0, 99) as u16)])
                } else {
                    let pr = order[k - 1];
                    let v = i32_at(0x118 + 4 * pr);
                    cat(&[format!("{}% ", v / 10000).as_bytes(), &t.names((21 + pr) as u16)])
                };
                put(text, 0.0, 33.0, 1, 0x23);
                if let Some(s) = production_line(state, &r, owner, counter) {
                    put(s, 0.0, 42.0, 1, 1);
                }
            }
        } else if sel.mode == 2 && sel.fleet > 0 {
            let map = state.starmap();
            let Some(fl) = map.fleets.get(sel.fleet).filter(|f| f.owner != ig2_sim::fleet::DELETED) else { return Self::default() };
            let totals = state.combat().fleets.get(sel.fleet).map(|c| c.totals).unwrap_or_default();
            let (name_col, col) = if fl.flags & 0x400 != 0 { (6, 6) } else if fl.flags & 0x4000 != 0 { (1, 1) } else { (3, 0x21) };
            // A protected fleet's mark (brush 11 at (6, 430)), else a wormhole's (brush 3 at (6, 429)).
            if fl.flags & 0x4000 != 0 {
                brushes.push((11, 1, 6.0, 430.0));
            } else if fl.flags & 0x400 != 0 {
                brushes.push((3, 1, 6.0, 429.0));
            }
            if crate::starmap_panels::Panels::buttons(state, sel).0 {
                brushes.push((9, ICON_FRAME[1], 171.0, 430.0));
            }
            put(cstr(&fl.name).to_vec(), 0.0, 3.0, 2, name_col);
            put(t.empire(fl.owner), 0.0, 15.0, 1, col);
            let mine = fl.owner == en || debug;
            if mine {
                if fl.order == 0 {
                    put(t.s(2, 3, 3), 0.0, 24.0, 1, col);
                } else {
                    let target = match fl.kind {
                        0 => t.s(2, 3, 4),
                        1 => {
                            let i = fl.tx as i32;
                            let r = m.read(va::PLANETS + (i.max(0) as usize * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
                            cstr(&r).to_vec()
                        }
                        _ => map.fleets.get((fl.tx as i32).max(0) as usize).map(|g| cstr(&g.name).to_vec()).unwrap_or_default(),
                    };
                    let head = if fl.order == 1 { t.s(2, 3, 5) } else { t.s(2, 3, 6) };
                    put(cat(&[&head, b":", &target]), 0.0, 24.0, 1, col);
                }
            }
            if mine && fl.flags & 0x4400 == 0 {
                let (ships, fighters, ground, cap) = (totals.ships - totals.fighters, totals.fighters, totals.ground, totals.capacity);
                // "SH:%d FI:%d TA:%d/%d", shortened to "S:… F:… T:…/…", then without the capacity.
                let line = |a: u16, with_cap: bool| {
                    let tail = if with_cap { format!(":{ground}/{cap}") } else { format!(":{ground}") };
                    cat(&[&t.s(2, 3, a), format!(":{ships} ").as_bytes(), &t.s(2, 3, a + 1), format!(":{fighters} ").as_bytes(), &t.s(2, 3, a + 2), tail.as_bytes()])
                };
                let mut s = line(7, true);
                if s.len() > 0x13 {
                    s = line(10, true);
                }
                if s.len() > 0x13 {
                    s = line(10, false);
                }
                put(s, 0.0, 33.0, 1, col);
            }
            if mine && fl.flags & 0x400 == 0 {
                put(cat(&[&t.s(2, 3, 13), format!(": {}", (fl.speed * 100.0 + 0.5) as i32).as_bytes()]), 0.0, 42.0, 1, col);
            }
        }
        Self { lines, brushes }
    }
}

/// `GetBolygoGazdagsag` (ELF 0x4fb180): richness × the owner's race production multiplier, plus the event bonus.
pub(crate) fn richness(state: &SaveState, rules: &ig2_rules::Rules, p: usize, en: u8) -> i32 {
    let c = state.colony();
    let Some(pl) = c.planets.get(p) else { return 0 };
    let mut bonus = 0;
    if state.mem.u8(va::MULTIPLAYER) == 0 && pl.any_event() {
        if pl.has_event(0xc3a) || pl.has_event(0x1f41) {
            bonus = 20;
        }
        if pl.has_event(0x1f42) {
            bonus = 10;
        }
        if pl.has_event(0x91b) {
            bonus = 15;
        }
    }
    let e = if pl.owner == 0 { en } else { pl.owner };
    let race = state.races().get(e as usize).copied().unwrap_or(0) as usize;
    (pl.richness as f32 * rules.race_production_mult.get(race).copied().unwrap_or(1.0) + bonus as f32) as i32
}

/// `StarmapProblemaRendez` (ELF 0x7082c0): the problems 1..=19 at 100000 or more (`+0x118 + 4p`), the four worst
/// first.
pub(crate) fn problem_order(r: &[u8], debug: bool) -> Vec<usize> {
    let v = |p: usize| i32::from_le_bytes(r[0x118 + 4 * p..0x11c + 4 * p].try_into().unwrap());
    let mut order: Vec<usize> = Vec::new();
    for c in 1..0x14 {
        if v(c) > 99999 || debug {
            let mut at = order.len();
            while at > 0 && v(order[at - 1]) < v(c) {
                at -= 1;
            }
            if at < 4 {
                order.insert(at, c);
                order.truncate(4);
            }
        }
    }
    order
}

/// `Counter` (exe 0xBAE05C) at UI time `time` (seconds): one step a 50 ms tick, the clock the info texts rotate on.
pub fn counter(time: f32) -> i32 {
    (time * 20.0) as i32
}

/// The production line: the head ship (line 0) or vehicle (line 1; every other 30 frames when both) and how much
/// of it is built.
pub(crate) fn production_line(state: &SaveState, r: &[u8], owner: u8, counter: i32) -> Option<Vec<u8>> {
    let head = |l: usize| i16::from_le_bytes([r[0x78 + 8 * l], r[0x79 + 8 * l]]);
    let mut line = if head(0) > 0 { Some(0) } else { None };
    if head(1) > 0 && ((counter / 30) % 2 == 0 || line.is_none()) {
        line = Some(1);
    }
    let l = line?;
    let designs = state.designs.get(owner as usize)?;
    let d = designs.get(head(l) as usize)?;
    let name = cstr(&d[..0x14]).to_vec();
    let time = i32::from_le_bytes(d[0x20..0x24].try_into().unwrap()).max(1);
    let left = i32::from_le_bytes(r[0xa0 + 4 * l..0xa4 + 4 * l].try_into().unwrap());
    let pct = 100 - left * 100 / time;
    let mut s = cat(&[&name, format!(" - {pct}%").as_bytes()]);
    if s.len() > 0x13 {
        s = cat(&[&name, format!("-{pct}%").as_bytes()]);
    }
    if s.len() > 0x13 {
        s = cat(&[&name, format!("{pct}%").as_bytes()]);
    }
    Some(s)
}

/// `KirakFader(i, …, x, y, w, h, 0, 54s, 90s, 155l + 100, 0, 90s, 130s, 185l + 70)` (ELF 0x547920 →
/// `FadeKerettelKerekit` 0x538330, [`crate::kit::fader::kirak_fader`]) with s = [`crate::kit::fader::S`] and
/// l = 0.
pub fn fader(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4]) {
    use crate::kit::fader::S;
    crate::kit::fader::kirak_fader(f, white, v, r, [0.0, 54.0 * S, 90.0 * S], 100.0, [0.0, 90.0 * S, 130.0 * S], 70.0, [0.0; 2]);
}

/// The HUD's resources.
pub struct StarmapHud {
    pub console: Option<Console>,
    /// `StarmapRovidInfoKiir`'s statics: the planet shown and the `Counter` its rotation started at.
    pub rotation: std::cell::Cell<(usize, i32)>,
}

impl StarmapHud {
    /// The problem line's step for `planet` at UI frame `counter` (restarting when the planet changes).
    pub fn step(&self, planet: usize, counter: i32) -> i32 {
        let (p, start) = self.rotation.get();
        if p != planet {
            self.rotation.set((planet, counter));
            return 0;
        }
        (counter - start) / 30
    }

    /// Draw the panel, the faders, the short info and the minimap (in the original's pass order).
    #[allow(clippy::too_many_arguments)]
    /// The fader behind the short info is `starf`'s `radar` (`fd`).
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], pics: &crate::hud::Pictures, brushes: &[ig2_rules::Brush], info: &InfoBox, mini: &Minimap, time: f32, fd: &crate::kit::FaderDesign, radar: usize, globe: Option<TexId>) {
        if let Some(c) = &self.console {
            c.draw(f, v);
        }
        // `StarmapGfx2D_1`: the fader behind the short info (`starf` 1 after the long info's box, else 0), then the
        // minimap's dark window (`FadeScreen(271, 385, 98, 71, black, 0x40)`; the value is the transparency [obs: the
        // window is dark in the captures]).
        crate::starmap_panels::starf(f, white, v, [5.0, 423.0, 162.0, 52.0], fd, radar);
        // The info box's globe (`StarmapFrissitBolygoUrhajok`): over the box's fader, as the exe draws it (its
        // `PropertiesOn(0x2000000)` [obs: the capture's globe is undimmed by the fill]).
        if let Some(tex) = globe {
            f.push(tex, Blend::Alpha, Instance::new(kind::SPRITE, v.px(320.0, 240.0), crate::kit::Wide::new(v).half(v), [1.0; 4]));
        }
        f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(271.0 + 49.0, 385.0 + 35.5), [49.0 * v.k, 35.5 * v.k], fade_color([0.0; 3], 1.0 - 0x40 as f32 / 255.0)));
        short_info(f, font, v, palette, pics, brushes, info, 1.0, 0.0);
        minimap(f, white, v, palette, mini, time, 0.0);
    }
}

/// `StarmapRovidInfoKiir`'s brushes and lines; the text colour times `mult` (`KiirStrSzinSzorzo`/100: the battle
/// screen's `(1 − spacewarelfade)·100`) and the tanks/production buttons (brush 9) `buttons_dy` lower (the battle's
/// `spacewarelfade·400`; re/screens/spacewar.md §5.2).
#[allow(clippy::too_many_arguments)]
pub fn short_info(f: &mut Frame, font: &Font, v: &Virtual, palette: &[[f32; 3]], pics: &crate::hud::Pictures, brushes: &[ig2_rules::Brush], info: &InfoBox, mult: f32, buttons_dy: f32) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    for &(b, n, x, y) in &info.brushes {
        let y = if b == 9 { (y + buttons_dy) as i32 as f32 } else { y };
        if let Some(b) = brushes.get(b) {
            pics.brush(f, v, b, n, Some([x, y]), [1.0; 4]);
        }
    }
    for (s, x, y, size, c) in &info.lines {
        font.text(f, v, s, *x, *y, 0x13, *size, Align::Left, col(*c).map(|x| x * mult), None);
    }
}

/// `GFX_StarmapRadar` (ELF 0x70e490): fleets as 2 × 1 dots, then planets 2 × 2, the selected one blinking; the view's
/// outline in colour 1, clipped to the window. On the battle screen (`spacewarelfade` = `fade` > 0) the galaxy
/// points are scaled by `fade·8 + 1` and everything is drawn at `(1 − fade)·255` (re/screens/spacewar.md §5.2).
pub fn minimap(f: &mut Frame, white: TexId, v: &Virtual, palette: &[[f32; 3]], mini: &Minimap, time: f32, fade: f32) {
    {
        let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
        let k = fade * 8.0 + 1.0;
        let opacity = ((1.0 - fade) * 255.0) as i32 as f32 / 255.0;
        let point = |p: [f32; 2]| minimap_point([p[0] * k, p[1] * k]);
        let inside = |q: [f32; 2]| q[0] > 274.0 && q[0] < 366.0 && q[1] > 388.0 && q[1] < 453.0;
        let dot = |f: &mut Frame, q: [f32; 2], w: f32, h: f32, c: [f32; 3], a: f32| {
            let (x, y) = ((q[0] - 1.0).trunc(), (q[1] - 1.0).trunc());
            f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], a]));
        };
        let blink = crate::hud::blink(time, 100.0);
        for &(p, c, sel) in &mini.fleets {
            let q = point(p);
            if inside(q) {
                dot(f, q, 2.0, 1.0, col(c), opacity * if sel { blink } else { 1.0 });
            }
        }
        for &(p, c, sel) in &mini.planets {
            let q = point(p);
            if inside(q) {
                dot(f, q, 2.0, 2.0, col(c), opacity * if sel { blink } else { 1.0 });
            }
        }
        let corner = |i: usize| mini.view[i].map(point);
        for (a, b) in [(0, 1), (0, 2), (2, 3), (1, 3)] {
            if let (Some(a), Some(b)) = (corner(a), corner(b)) {
                if let Some((a, b)) = crate::hud::clip_line(a, b, MINIMAP) {
                    let c = col(1);
                    f.push(white, Blend::GammaAlpha, Instance::line(v.px(a[0], a[1]), v.px(b[0], b[1]), v.k.max(1.0), [c[0], c[1], c[2], opacity]));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_lines_step_every_one_and_a_half_seconds() {
        // `StarmapRovidInfoKiir` (exe 0x5afc50) steps every 30 `Counter`, a 50 ms tick each (issue #34).
        let hud = StarmapHud { console: None, rotation: std::cell::Cell::new((0, 0)) };
        assert_eq!(hud.step(7, counter(10.0)), 0);
        assert_eq!(hud.step(7, counter(11.45)), 0);
        assert_eq!(hud.step(7, counter(11.5)), 1);
        assert_eq!(hud.step(7, counter(13.0)), 2);
        assert_eq!((counter(1.45) / 30, counter(1.5) / 30), (0, 1));
    }

    #[test]
    fn console_buttons_sink_while_held_and_views_never() {
        // A held zoom (5): `Starmap_Main` sets frame 0 every frame, `ShowStarmap` adds 5; let go, it comes back.
        let mut b = ConsoleButtons::default();
        for _ in 0..3 {
            b.press(5);
            b.step();
            assert_eq!((b.which, b.depth[0]), (0, 5.0));
        }
        b.release();
        b.step();
        assert_eq!(b.depth[0], 0.0);
        // The top view (1) acts on the release frame: pressed and let go in one step, it stays at 0.
        b.press(1);
        b.release();
        b.step();
        assert_eq!((b.which, b.depth[4]), (4, 0.0));
        // The minimap has no button.
        b.press(7);
        assert_eq!(b.which, 4);
        // Not set again, a sinking button goes on to 10 and holds there.
        let mut b = ConsoleButtons::default();
        b.press(6);
        for _ in 0..3 {
            b.step();
        }
        assert_eq!(b.depth[3], 10.0);
    }

    /// Each `GetSubMeshid(KezeloId, i)` sub-mesh moves between its frames 0 and 5, the pose a held button shows
    /// (skips without the install).
    #[test]
    fn console_buttons_have_key_frames() {
        let Ok(root) = std::env::var("IG2_INSTALL_DIR") else { return };
        let install = Install::open(root, ig2_formats::text::Lang::En).unwrap();
        let m0 = Console::model(&install, None).unwrap();
        let m5 = Console::model(&install, Some(5)).unwrap();
        for i in 0..6u16 {
            let parts = m0.parts.iter().zip(&m5.parts).filter(|(p, _)| p.index.first() == Some(&i));
            let moved = parts.flat_map(|(p, q)| p.verts.iter().zip(&q.verts)).any(|(a, b)| a.pos != b.pos);
            assert!(moved, "sub-mesh {i} keeps its pose at frame 5");
        }
    }

    #[test]
    fn minimap_maps_the_galaxy_mirrored_in_x() {
        assert_eq!(minimap_point([0.0, 0.0]), [320.0, 420.0]);
        let q = minimap_point([300.0, -300.0]);
        assert!((q[0] - 274.0).abs() < 1e-4 && (q[1] - 387.5).abs() < 1e-4);
    }

    #[test]
    fn problems_sort_worst_first_and_keep_four() {
        let mut r = vec![0u8; 0x188];
        for (p, v) in [(2, 150000i32), (5, 400000), (7, 200000), (9, 120000), (11, 900000), (3, 5)] {
            r[0x118 + 4 * p..0x11c + 4 * p].copy_from_slice(&v.to_le_bytes());
        }
        assert_eq!(problem_order(&r, false), vec![11, 5, 7, 2]);
    }
}
