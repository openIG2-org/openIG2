//! The spy screen's 3D pass (re/screens/spy.md §2): `Spying3DInit` (ELF 0x6bad60, exe 0x58fd10) loads the frame
//! `spybck2.xvb` with its wireframe sphere, the roster carousel `spybck2a.xvb` (twelve pedestals, each with a
//! billboard of the spy's race), the ring `spybck2b.xvb`, the head-video disc `kor.xvb` and twelve status icons
//! `negyzet0..b.xvb`; `Spying3DUpdate` (ELF 0x6c4b10, exe 0x590410) and `BeallitKemekAtlatszosaga` (ELF 0x6ba0e0,
//! exe 0x583cb0) pose them every frame.
//!
//! The engine's camera here is the identity at the origin looking down −z, `fov` 600, centre (320, 240) on the
//! 640 × 480 virtual screen (`forgaspont`/`kamerapont`/angles all 0 in `Spying3DUpdate`): a world point `(x, y, z)`
//! lands at `(600·x/−z + 320, 600·y/−z + 240)`, +y down.

use ig2_formats::xv::{Archive, ChildContent, Node};
use ig2_formats::Install;

use crate::lighting;
use crate::render::{Renderer, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, frame_keys, Model, Part, Placement, V3};

const VECTORS: &str = "vector/spy/";
const TEXTURES: &str = "textures/spy/";
/// `KemTexturaNevek` (ELF 0x119a250): the billboard of race `r` is `<r>_a.bmp`.
const RACE_TEXTURES: usize = 16;
/// `Spy_Status_Texturak` (ELF 0x119a0d0): 0 at home, 1 travelling, 2 captured, 3.. the 32 frames of the mission
/// hourglass.
const STATUS_TEXTURES: [&str; 3] = ["spy_status_0.bmp", "spy_status_2.bmp", "spy_status_3.bmp"];
const HOURGLASS: usize = 32;
/// `KemFejNevek` (ELF 0x119a040): the head videos by race − 1 (16: none). `keresk.ani` is not in the install
/// (the file is `keresek.ani`): races 9 and 10 show no video in the original either [inf].
pub const HEADS: [&str; 17] = [
    "human.ani", "ufo.ani", "politikus.ani", "radioaktiv.ani", "brutal.ani", "dyno.ani", "lamer.ani", "tremor.ani", "keresk.ani",
    "keresk.ani", "kaloz.ani", "scorpus.ani", "scorpus.ani", "b5.ani", "worm", "", "ures.ani",
];

/// The camera's focal length and the virtual screen's centre.
const FOV: f32 = 600.0;
const CENTRE: [f32; 2] = [320.0, 240.0];

/// A world point on the virtual screen (`TransformPoint`), `None` behind the camera.
pub fn project(p: V3) -> Option<[f32; 2]> {
    (p[2] < -1.0).then(|| [FOV * p[0] / -p[2] + CENTRE[0], FOV * p[1] / -p[2] + CENTRE[1]])
}

/// Triangles of one material in a model's root space, with their texture.
#[derive(Clone)]
struct Piece {
    verts: Vec<scene::Vert>,
    tex: TexId,
}

/// One pedestal of the carousel (`SpyPalast` child `i`).
#[derive(Clone, Default)]
struct Pedestal {
    /// The child's origin in the model (`RotateObj(child, 0, −rot, 0)` turns about it).
    origin: V3,
    /// The pedestal (`ap`, child 0) and the billboard (`lap`, child 1's child 0: `KepernyonKemek[i]`).
    base: Vec<Piece>,
    sheet: Vec<Piece>,
}

/// Where a pedestal is on screen this frame (`BeallitKemekAtlatszosaga`, `KemekFeliratai`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Spot {
    /// The group's centre in world units and on the virtual screen.
    pub centre: V3,
    pub screen: [f32; 2],
    /// The pedestal's centre on the virtual screen (the name plate).
    pub plate: [f32; 2],
    /// `KepernyonKemek[i].+0x24`: 0 in front, 1 out of sight.
    pub transparency: f32,
}

/// Everything `Spying3DUpdate` needs from the screen's state.
#[derive(Debug, Clone, Default)]
pub struct Pose {
    /// `spyframe` 0..=50 (the opening, back to 0 when closing), `spywireframe` 0..10.
    pub frame: f32,
    pub wire: f32,
    /// `spyKELLKINYITAS` (the screen opens and closes animated), `spyKINYITVA` (fully open once), `spyBECSUKAS`
    /// (closing).
    pub animated: bool,
    pub opened: bool,
    pub closing: bool,
    /// `SpyPalastRotation`.
    pub rotation: f64,
    /// `spyFrameValtozo`, `spyAtfordulas`, `spyAtvaneFordulva`, `spykeringeto`.
    pub spin: i32,
    pub flip: f32,
    pub flipped: f32,
    pub orbit: f32,
    /// `KepernyonKemek[i].+0x20`: the roster slot on pedestal `i` (−1 none) and its race and state.
    pub slots: [Option<(u8, i16)>; 12],
    /// `KikeresKem(AktKem)`: the selected pedestal.
    pub selected: Option<usize>,
    /// The head video's frame on the disc.
    pub head: Option<TexId>,
}

/// A part of the frame `spybck2`: its look and whether it belongs to the telescopes (child 1, `Group18`).
#[derive(Clone)]
struct FramePart {
    piece: Piece,
    glass: f32,
    telescope: bool,
}

pub struct SpyScene {
    /// The frame's parts posed at the integer frames 0..=50 (`SetFrame(SpyMainMesh, spyframe)`; the same parts in
    /// the same order at every frame).
    frame: Vec<Vec<FramePart>>,
    /// The sphere `Gomb` at frames 0..=60: its triangles (points in `Gomb`'s own space: the model's minus the
    /// child's translation) with their edge masks.
    wire: Vec<Vec<([V3; 3], u8)>>,
    /// `Gomb`'s translation in the model (replaced by `MoveObj` while the screen is animated).
    gomb: V3,
    pedestals: Vec<Pedestal>,
    ring: Vec<Piece>,
    disc: Vec<Piece>,
    icon: Vec<Vec<Piece>>,
    race_tex: Vec<TexId>,
    status_tex: Vec<TexId>,
    white: TexId,
    /// The head video's texture slot.
    pub head_tex: TexId,
    frame_origin: V3,
}

fn texture(r: &mut Renderer, install: &Install, name: &str, cache: &mut std::collections::HashMap<String, TexId>, white: TexId) -> TexId {
    let Some(path) = install.resolve_texture(TEXTURES, name).or_else(|| install.resolve_model_texture("spy", name)) else { return white };
    if let Some(&t) = cache.get(&path) {
        return t;
    }
    let t = crate::assets::Image::load(install, &path).map(|img| r.add_texture(&img)).unwrap_or(white);
    cache.insert(path, t);
    t
}

fn pieces(r: &mut Renderer, install: &Install, parts: &[&Part], cache: &mut std::collections::HashMap<String, TexId>, white: TexId) -> Vec<Piece> {
    parts
        .iter()
        .map(|p| {
            let tex = p.texture.as_ref().map_or(white, |t| match crate::assets::Image::load(install, t) {
                Ok(img) => *cache.entry(t.clone()).or_insert_with(|| r.add_texture(&img)),
                Err(_) => white,
            });
            Piece { verts: p.verts.clone(), tex }
        })
        .collect()
}

/// The first path segment of a part (`/Group134/...` → `Group134`).
fn top(path: &str) -> &str {
    path.trim_start_matches('/').split('/').next().unwrap_or("")
}

/// The root node's children (name, translation) of a model file.
fn children(install: &Install, file: &str) -> Vec<(String, V3)> {
    let path = format!("dataraw/{VECTORS}{file}.xv");
    let Ok(bytes) = install.read(&path) else { return Vec::new() };
    let Ok(a) = Archive::parse(&bytes) else { return Vec::new() };
    match a.scene(Some(file)) {
        Ok(Some(n)) => n.children.iter().map(|(c, _)| (c.name.clone(), c.translation())).collect(),
        _ => Vec::new(),
    }
}

/// The edge mask of every face of the sphere `Gomb/Sphere01` (`wirekirak`: bit 0 c–a, 1 a–b, 2 b–c), in face order.
fn sphere_masks(install: &Install) -> Vec<u8> {
    let path = format!("dataraw/{VECTORS}spybck2.xv");
    let Ok(bytes) = install.read(&path) else { return Vec::new() };
    let Ok(a) = Archive::parse(&bytes) else { return Vec::new() };
    fn find(n: &Node) -> Option<Vec<u8>> {
        for (_, c) in &n.children {
            match c {
                ChildContent::Mesh(m) => {
                    return Some((0..m.faces.len()).map(|i| m.face_flags.get(i).copied().unwrap_or(7)).collect());
                }
                ChildContent::Node(sub) => {
                    if let Some(x) = find(sub) {
                        return Some(x);
                    }
                }
                ChildContent::Missing => {}
            }
        }
        None
    }
    match a.scene(Some("spybck2")) {
        Ok(Some(n)) => n.children.iter().find(|(c, _)| c.name == "Gomb").and_then(|(_, c)| if let ChildContent::Node(g) = c { find(g) } else { None }).unwrap_or_default(),
        _ => Vec::new(),
    }
}

impl SpyScene {
    pub fn load(r: &mut Renderer, install: &Install) -> Result<Self, String> {
        let mut cache = std::collections::HashMap::new();
        let white = r.add_texture(&crate::assets::Image::white());
        // `SpyMainMesh` at rest (`SetFrame(spyframe = 50)`): child 1 (the telescopes) is hidden once the screen
        // is open, child 0 is the wireframe sphere (`PropertiesOn(0x40)`), drawn below; the panels' fills
        // (`Group59/Object05`, `Group60/Object06`) are `SetObjTransparency(0.8)`.
        // The telescopes' children 4 and 5 (`Group16`, `Group17`) are hidden for good (`Spying3DInit`); the rest of
        // child 1 shows while the screen opens (before frame 25) and while it closes (`Spying3DUpdate`).
        let gomb = children(install, "spybck2").first().map_or([0.0; 3], |c| c.1);
        let masks = sphere_masks(install);
        let mut frame = Vec::new();
        let mut wire = Vec::new();
        let mut origin = [1024.0; 3];
        for f in 0..=60 {
            let m = Model::load_at(install, VECTORS, "spybck2.xvb", TEXTURES, Some(f))?;
            origin = m.origin;
            if f <= 50 {
                let mut parts = Vec::new();
                for p in &m.parts {
                    let t = top(&p.path);
                    if t == "Gomb" || (t == "Group18" && matches!(p.index.get(1), Some(4 | 5))) {
                        continue;
                    }
                    let glass = if p.path.ends_with("/Object05") || p.path.ends_with("/Object06") { 0.8 } else { -1.0 };
                    for piece in pieces(r, install, &[p], &mut cache, white) {
                        parts.push(FramePart { piece, glass, telescope: t == "Group18" });
                    }
                }
                frame.push(parts);
            }
            let mut tris = Vec::new();
            let local = |v: V3| -> V3 { std::array::from_fn(|k| v[k] - gomb[k]) };
            for p in m.parts.iter().filter(|p| top(&p.path) == "Gomb") {
                for (i, t) in p.verts.chunks_exact(3).enumerate() {
                    tris.push(([local(t[0].pos), local(t[1].pos), local(t[2].pos)], masks.get(i).copied().unwrap_or(7)));
                }
            }
            wire.push(tris);
        }
        let palast = Model::load(install, VECTORS, "spybck2a.xvb", TEXTURES)?;
        let kids = children(install, "spybck2a");
        let mut pedestals = vec![Pedestal::default(); 12];
        for (i, (name, origin)) in kids.iter().enumerate().take(12) {
            let mine: Vec<&Part> = palast.parts.iter().filter(|p| top(&p.path) == name).collect();
            let (sheet, base): (Vec<&Part>, Vec<&Part>) = mine.into_iter().partition(|p| p.path.rsplit('/').next().is_some_and(|n| n.starts_with("lap")));
            pedestals[i] = Pedestal { origin: *origin, base: pieces(r, install, &base, &mut cache, white), sheet: pieces(r, install, &sheet, &mut cache, white) };
        }
        // `SpyKozep`: children 2 and 3 are hidden (`Spying3DInit`, `Spying3DUpdate`).
        let kozep = Model::load(install, VECTORS, "spybck2b.xvb", TEXTURES)?;
        let kz = children(install, "spybck2b");
        let hidden: Vec<&str> = kz.iter().skip(2).map(|(n, _)| n.as_str()).collect();
        let parts: Vec<&Part> = kozep.parts.iter().filter(|p| !hidden.contains(&top(&p.path))).collect();
        let ring = pieces(r, install, &parts, &mut cache, white);
        let kor = Model::load(install, VECTORS, "kor.xvb", TEXTURES)?;
        let disc = pieces(r, install, &kor.parts.iter().collect::<Vec<_>>(), &mut cache, white);
        let mut icon = Vec::new();
        for i in 0..12 {
            let name = format!("negyzet{}.xvb", char::from_digit(i, 16).unwrap_or('0'));
            let m = Model::load(install, VECTORS, &name, TEXTURES)?;
            icon.push(pieces(r, install, &m.parts.iter().collect::<Vec<_>>(), &mut cache, white));
        }
        let race_tex = (1..=RACE_TEXTURES).map(|k| texture(r, install, &format!("{k}_a.bmp"), &mut cache, white)).collect();
        let mut status_tex: Vec<TexId> = STATUS_TEXTURES.iter().map(|n| texture(r, install, n, &mut cache, white)).collect();
        status_tex.extend((0..HOURGLASS).map(|k| texture(r, install, &format!("{:06}.bmp", 10000 + k), &mut cache, white)));
        let head_tex = r.add_texture(&crate::assets::Image { width: 1, height: 1, rgba: vec![0, 0, 0, 255] });
        Ok(Self { frame, wire, gomb, pedestals, ring, disc, icon, race_tex, status_tex, white, head_tex, frame_origin: origin })
    }

    /// The pedestals' places for pose `p` (`BeallitKemekAtlatszosaga`: the carousel turned by `rotation`, each
    /// pedestal turned back to face the camera; the transparency by its place on the ring).
    pub fn spots(&self, p: &Pose) -> [Spot; 12] {
        let (palast, rot) = self.palast(p);
        std::array::from_fn(|i| {
            let Some(ped) = self.pedestals.get(i) else { return Spot::default() };
            let at = |v: V3| palast.point(Self::turned(v, ped.origin, rot));
            let centre = bbox_centre(ped.base.iter().chain(&ped.sheet).flat_map(|x| x.verts.iter().map(|v| at(v.pos))));
            let plate = bbox_centre(ped.base.iter().flat_map(|x| x.verts.iter().map(|v| at(v.pos))));
            // `fmod((rot/2π)·12 + i − 5, 12)`: three places either side of the front fade out.
            let f = ((p.rotation / std::f64::consts::TAU) * 12.0 + i as f64 - 5.0) % 12.0;
            let f = f as f32;
            let mut t = if f <= 3.0 {
                f / 3.0
            } else if f >= 9.0 {
                1.0 - (f - 9.0) / 3.0
            } else {
                1.0
            };
            t = if t <= 0.7 { 0.0 } else { (t - 0.7) * 3.3333 };
            Spot { centre, screen: project(centre).unwrap_or([-1000.0; 2]), plate: project(plate).unwrap_or([-1000.0; 2]), transparency: t }
        })
    }

    /// `SpyPalast`'s transform: `MoveObj(0, −125 − (1 − f)·150, −1000·f)` with `f = spyframe/50`, and its turn
    /// (`RotateObj(0, rot, 0)`, plus `−sin((1 − f)·π)` while opening).
    fn palast(&self, p: &Pose) -> (scene::Transform, f32) {
        let f = p.frame / 50.0;
        let mut rot = p.rotation as f32;
        if p.frame < 50.0 {
            rot += -(((1.0 - f) as f64 * std::f64::consts::PI).sin() as f32);
        }
        let place = Placement::add_to_scene([1024.0; 3], 0.75).world((0.0, 0.0, 0.0), [0.0, -125.0 - (1.0 - f) * 150.0, f * -1000.0]);
        (place, rot)
    }

    /// A vertex of the pedestal with origin `o` after the carousel turned by `rot` and the pedestal turned back.
    fn turned(v: V3, o: V3, rot: f32) -> V3 {
        let m = scene::create_matrix(0.0, rot, 0.0);
        let q = scene::apply(&m, o);
        [q[0] + v[0] - o[0], q[1] + v[1] - o[1], q[2] + v[2] - o[2]]
    }

    /// The 3D pass for this frame, sized `size` physical pixels.
    pub fn build(&self, p: &Pose, size: [f32; 2]) -> Scene3d {
        let mut b = Builder::default();
        let lights = lights(p.orbit);
        // The frame (`SpyMainMesh`, scale 0.45 at (0, 2, −650)), posed at `spyframe` (`Kocka` interpolates the keys
        // in 1/256 frame steps; here the posed vertices of the two integer frames around it).
        let main = Placement::add_to_scene(self.frame_origin, 0.45).world((0.0, 0.0, 0.0), [0.0, 2.0, -650.0]);
        let (f0, f1, ft) = frame_keys(p.frame, self.frame.len());
        // `Spying3DUpdate`: the telescopes show (and the ring hides) while opening before frame 25 and while closing.
        let telescopes = (!p.opened && p.frame < 25.0) || p.closing;
        if let (Some(a), Some(c)) = (self.frame.get(f0), self.frame.get(f1)) {
            for (x, y) in a.iter().zip(c) {
                if x.telescope && !telescopes {
                    continue;
                }
                let look = if x.glass >= 0.0 { Look::Glass(1.0 - x.glass) } else { Look::Lit(0) };
                b.piece(&lerp_piece(&x.piece, &y.piece, ft), look, &lights, |v| main.point(v));
            }
        }
        // The wireframe sphere (`SetFrame(spyframe + spywireframe)`): its edges at the texture's colour, unlit
        // but for the ambient light [obs: (1, 8, 10) = Racsszin (8, 101, 132) × 0x14/0xff]. While the screen is
        // animated `Gomb` is moved to `(spyInitPos1 = 0, (1 − f)·200, −(1 − f)·1000)` (`MoveObj` replaces the
        // child's translation) and turned `(sin((1 − f)·2)·3, sin((1 − f)·2.1)·2, 0)`, `f = spyframe/50`; its
        // transparency is `1.35 − 2·spyframe/50` up to frame 25 (`spySetObjTransparencyH`, clamped to
        // 1e−6..0.9999999), else 1e−6.
        let (w0, w1, t) = frame_keys(p.frame + p.wire, self.wire.len());
        let (offset, turn) = if p.animated {
            let g = 1.0 - p.frame / 50.0;
            let a = ((g * 2.0) as f64).sin() as f32 * 3.0;
            let b2 = (g as f64 * 2.1).sin() as f32 * 2.0;
            ([0.0, g * 200.0 / 1024.0, -(g * 1000.0) / 1024.0], scene::create_matrix(a, b2, 0.0))
        } else {
            (self.gomb, scene::create_matrix(0.0, 0.0, 0.0))
        };
        let tr = if p.frame <= 25.0 { (1.35 - p.frame * 2.0 / 50.0).clamp(1e-6, 0.9999999) } else { 1e-6 };
        if let (Some(a), Some(c)) = (self.wire.get(w0), self.wire.get(w1)) {
            // Transparent, the lines are the texture's colour × the opacity in the frame's gamma space [obs: closing
            // frame 20, opacity 0.45: (4, 45, 59) on a line, (6, 71, 92) where two cross; opening frame 12: (2, 24,
            // 32) at crossings]; opaque, ambient-lit.
            let (colour, opacity) = if tr < 0.0001 {
                ([8.0, 101.0, 132.0].map(|c: f32| c / 255.0 * AMBIENT), 1.0)
            } else {
                let o = 1.0 - tr;
                ([8.0, 101.0, 132.0].map(|c: f32| c / 255.0), o)
            };
            let at = |v: V3| {
                let q = scene::apply(&turn, v);
                main.point([q[0] + offset[0], q[1] + offset[1], q[2] + offset[2]])
            };
            // Only the triangles facing the camera in the model's winding draw their edges (the engine's back-face
            // test) [obs: closing frame 20, the sphere seen from outside shows one side's grid; inside, at rest,
            // every triangle faces the camera].
            for ((x, mask), (y, _)) in a.iter().zip(c) {
                let lerp = |i: usize| at(std::array::from_fn(|j| x[i][j] + (y[i][j] - x[i][j]) * t));
                let (pa, pb, pc) = (lerp(0), lerp(1), lerp(2));
                let (u, w) = (sub(pb, pa), sub(pc, pa));
                let n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                if n[0] * pa[0] + n[1] * pa[1] + n[2] * pa[2] > 0.0 {
                    continue;
                }
                for (bit, e0, e1) in [(2, pa, pb), (4, pb, pc), (1, pc, pa)] {
                    if mask & bit != 0 {
                        b.line(e0, e1, colour, opacity, size, self.white);
                    }
                }
            }
        }
        // The carousel.
        let (palast, rot) = self.palast(p);
        let spots = self.spots(p);
        for (i, ped) in self.pedestals.iter().enumerate() {
            let Some((race, state)) = p.slots.get(i).copied().flatten() else {
                // An empty pedestal: the sheet is hidden (`RarakKemekTexturai`), the base and its "---" stay.
                for x in &ped.base {
                    b.piece(x, glass_or_lit(spots[i].transparency, 0), &lights, |v| palast.point(Self::turned(v, ped.origin, rot)));
                }
                continue;
            };
            // `SpyRarakFeny`: the selected pedestal's own ambient light, `(sin(90k) + 1)·127` grey.
            let extra = if p.selected == Some(i) { ((((p.orbit * 90.0) as f64).sin() as f32 + 1.0) * 127.0) as u8 } else { 0 };
            let at = |v: V3| palast.point(Self::turned(v, ped.origin, rot));
            for x in &ped.base {
                b.piece(x, glass_or_lit(spots[i].transparency, extra), &lights, at);
            }
            let tex = self.race_tex.get((race as usize).wrapping_sub(1)).copied().unwrap_or(self.white);
            for x in &ped.sheet {
                b.piece(&Piece { tex, ..x.clone() }, glass_or_lit(spots[i].transparency, extra), &lights, at);
            }
            // The status icon (`Spy_Status_Idk[i]`, scale 0.012) at the group's centre + (30, 47, 0).
            let s = match state {
                2 => 1,
                3 => (p.orbit * 240.0) as usize % HOURGLASS + 3,
                4 => 2,
                _ => 0,
            };
            let c = spots[i].centre;
            let place = Placement::add_to_scene([1024.0; 3], 0.012).world((0.0, 0.0, 0.0), [c[0] + 30.0, c[1] + 47.0, c[2]]);
            let tex = self.status_tex.get(s).copied().unwrap_or(self.white);
            for x in self.icon.get(i).into_iter().flatten() {
                b.piece(&Piece { tex, ..x.clone() }, Look::Glass((1.0 - spots[i].transparency).min(1.0)), &lights, |v| place.point(v));
            }
        }
        if !telescopes {
            // `SpyKozep` (scale 0.303) and the disc `kor` (0.173) at (0, 182, −1000): turned about z by
            // `spyFrameValtozo/220`, about y by `spyAtvaneFordulva`, and flipping over `spyAtfordulas` 0..30 (moved
            // towards the camera and back).
            let a = p.flip;
            let (y, z) = if a as i32 == 30 {
                (182.0, -1000.0)
            } else if (a as i32) < 16 {
                (182.0 - a * 2.0, a * 40.0 - 1000.0)
            } else {
                (182.0 - (29.0 - a) * 2.0, (29.0 - a) * 40.0 - 1000.0)
            };
            // The ring spins (`RotateObj(SpyKozep, 0, 0, spyFrameValtozo/220)`); the disc doesn't (`RotateObj(spyKorId,
            // 0, 0, 0)`).
            let turn = |v: V3, spin: f32| {
                let mut q = scene::apply(&scene::create_matrix(0.0, 0.0, spin), v);
                let y_axis = scene::axis_matrix([0.0, 1.0, 0.0], p.flipped);
                q = scene::apply(&y_axis, q);
                if a as i32 != 30 {
                    q = scene::apply(&scene::axis_matrix([0.0, 1.0, 0.0], a * 0.10471976), q);
                }
                q
            };
            let ring = Placement::add_to_scene([1024.0; 3], 0.303).world((0.0, 0.0, 0.0), [0.0, y, z]);
            for x in &self.ring {
                b.piece(x, Look::Lit(0), &lights, |v| ring.point(turn(v, p.spin as f32 / 220.0)));
            }
            self.draw_disc(&mut b, p, [0.0, y, z], &|v| turn(v, 0.0), Look::Flat, &lights);
        } else if p.closing {
            // Closing: the ring is hidden; the disc stays turned as it was, fades (`1 − f`, clamped like
            // `spySetObjTransparencyH`) and drops back to `(0, (1 − f)·70 + 182, −1000 − (1 − f)·700)`.
            let g = 1.0 - p.frame / 50.0;
            let turn = |v: V3| scene::apply(&scene::axis_matrix([0.0, 1.0, 0.0], p.flipped), v);
            let o = 1.0 - g.clamp(1e-6, 0.9999999);
            self.draw_disc(&mut b, p, [0.0, g * 70.0 + 182.0, -1000.0 - g * 700.0], &turn, Look::Glass(o), &lights);
        }
        b.finish(size)
    }

    /// The head-video disc `kor` (scale 0.173) at `at`, shown once its video started (`UnhideObj(spyKorId)` past
    /// the flip's frame 15).
    fn draw_disc(&self, b: &mut Builder, p: &Pose, at: V3, turn: &dyn Fn(V3) -> V3, look: Look, lights: &lighting::Lights) {
        let Some(tex) = p.head else { return };
        let disc = Placement::add_to_scene([1024.0; 3], 0.173).world((0.0, 0.0, 0.0), at);
        for x in &self.disc {
            b.piece(&Piece { tex, ..x.clone() }, look, lights, |v| disc.point(turn(v)));
        }
    }
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A frame part between two posed frames.
fn lerp_piece(a: &Piece, b: &Piece, t: f32) -> Piece {
    if t == 0.0 || a.verts.len() != b.verts.len() {
        return a.clone();
    }
    let l = |x: V3, y: V3| -> V3 { std::array::from_fn(|k| x[k] + (y[k] - x[k]) * t) };
    let verts = a.verts.iter().zip(&b.verts).map(|(x, y)| scene::Vert { pos: l(x.pos, y.pos), normal: scene::normalise(l(x.normal, y.normal)), uv: x.uv, lnormal: l(x.lnormal, y.lnormal) }).collect();
    Piece { verts, tex: a.tex }
}

fn glass_or_lit(transparency: f32, extra: u8) -> Look {
    if transparency < 0.0001 {
        Look::Lit(extra)
    } else {
        Look::Glass(1.0 - transparency.min(0.9999999))
    }
}

fn bbox_centre(points: impl Iterator<Item = V3>) -> V3 {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if lo[0] > hi[0] {
        return [0.0; 3];
    }
    std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0)
}

/// The screen's lights (`Spying3DInit`, `spyOldalsoFenyekBe` ELF 0x6b9c80, `spyMozgatMainFenyek` 0x6b9a60): the
/// ambient 0x141414, a white parallel light 0x808080 from (0, 0, −10) and two coloured ones (0x7f5d00, 0x004e7f)
/// moved to `(sin(20k)·300, cos(20k)·200, −1000)` and the same at +1.57075 (`k = spykeringeto`), lit by the engine's
/// model (`lighting`, re/screens/research.md §2.1).
/// `Spying3DInit`'s ambient 0x141414.
const AMBIENT: f32 = 0x14 as f32 / 255.0;

fn lights(orbit: f32) -> lighting::Lights {
    let side = |a: f32| [((a as f64).sin() * 300.0) as f32, ((a as f64).cos() * 200.0) as f32, -1000.0];
    let k = orbit * 20.0;
    lighting::Lights::default().ambient(0x141414).parallel(0x808080, [0.0, 0.0, -10.0]).parallel(0x7f5d00, side(k)).parallel(0x004e7f, side(k + 1.57075))
}

#[derive(Clone, Copy)]
enum Look {
    /// Lit opaque (alpha-tested), with an extra ambient term (the selected spy's pulse).
    Lit(u8),
    /// Lit at this opacity (`SetObjTransparency`: the texture's colour × the light × opacity [obs: the panels' (2,
    /// 20, 26) = Racsszin × 0.2 with the lights about saturated]).
    Glass(f32),
    /// Unlit and opaque (the video disc).
    Flat,
}

#[derive(Default)]
struct Builder {
    dynamic: Vec<MeshVertex>,
    opaque: Vec<MeshDraw>,
    glass: Vec<MeshDraw>,
}

impl Builder {
    fn piece(&mut self, x: &Piece, look: Look, lights: &lighting::Lights, at: impl Fn(V3) -> V3) {
        let start = self.dynamic.len() as u32;
        for v in &x.verts {
            // The engine lights and blends in the frame's gamma space: the gamma pass (re/screens/render.md §2).
            let p = at(v.pos);
            let (rgb, a) = match look {
                Look::Lit(e) => {
                    let w = lighting::through(&at, v.pos, v.lnormal);
                    let c = if e > 0 { lights.clone().ambient(u32::from_le_bytes([e, e, e, 0])).rgb(w) } else { lights.rgb(w) };
                    (c, 0.0)
                }
                // Transparent parts are lit too (`fenyekszamolasa` with property 8 keeps the colour and puts the
                // opacity in alpha) [obs: opening 30's right panel 1.74 → 1.16].
                Look::Glass(o) => {
                    let c = lights.rgb(lighting::through(&at, v.pos, v.lnormal));
                    (c, o)
                }
                Look::Flat => ([1.0; 3], 0.0),
            };
            self.dynamic.push(MeshVertex { pos: p, normal: v.normal, uv: v.uv, color: [rgb[0], rgb[1], rgb[2], a] });
        }
        // The opaque video disc takes the remaster's `VIDEO` material.
        let video = matches!(look, Look::Flat);
        let d = MeshDraw { mesh: None, range: start..self.dynamic.len() as u32, tex: x.tex, glass: matches!(look, Look::Glass(_)), cloud: false, shadow: crate::render3d::Shadow::None, video, additive: false };
        if d.glass {
            self.glass.push(d);
        } else {
            self.opaque.push(d);
        }
    }

    /// A one-pixel line between two world points, as a thin quad facing the camera, `rgb` blended at opacity `o`.
    #[allow(clippy::too_many_arguments)]
    fn line(&mut self, a: V3, b: V3, rgb: [f32; 3], o: f32, size: [f32; 2], white: TexId) {
        if a[2] > -1.0 || b[2] > -1.0 {
            return;
        }
        // Half a virtual pixel at each end's depth, across the line on screen.
        let (pa, pb) = (project(a).unwrap_or_default(), project(b).unwrap_or_default());
        let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
        let l = (dx * dx + dy * dy).sqrt();
        if l < 1e-4 {
            return;
        }
        let px = 480.0 / size[1].max(1.0);
        let (nx, ny) = (-dy / l * px * 0.5, dx / l * px * 0.5);
        let off = |p: V3, s: f32| -> V3 { [p[0] + s * nx * -p[2] / FOV, p[1] + s * ny * -p[2] / FOV, p[2]] };
        let start = self.dynamic.len() as u32;
        let n = [0.0, 0.0, 1.0];
        for q in [off(a, 1.0), off(a, -1.0), off(b, 1.0), off(b, -1.0), off(b, 1.0), off(a, -1.0)] {
            self.dynamic.push(MeshVertex { pos: q, normal: n, uv: [0.0, 0.0], color: [rgb[0], rgb[1], rgb[2], o] });
        }
        self.glass.push(MeshDraw { mesh: None, range: start..self.dynamic.len() as u32, tex: white, glass: true, cloud: false, shadow: crate::render3d::Shadow::None, video: false, additive: false });
    }

    fn finish(mut self, size: [f32; 2]) -> Scene3d {
        let (w, h) = (size[0].max(1.0), size[1].max(1.0));
        let (near, far) = (50.0f32, 6000.0f32);
        let b = near * far / (near - far);
        let a = far / (near - far);
        // Camera space = world: clip x = 2.5·(H/W)·x, clip y = −2.5·y, w = −z (re/screens/spy.md §2).
        let k = FOV / 240.0;
        let rows = [[k * h / w, 0.0, 0.0, 0.0], [0.0, -k, 0.0, 0.0], [0.0, 0.0, a, b], [0.0, 0.0, -1.0, 0.0]];
        let view = std::array::from_fn(|c| std::array::from_fn(|r| rows[r][c]));
        let mut draws = std::mem::take(&mut self.opaque);
        draws.extend(self.glass);
        Scene3d {
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
            dynamic: self.dynamic,
        }
    }
}

/// The head video of the selected spy (`MakeAnimName` ELF 0x6ba040: `anim/fejek/<KemFejNevek[race − 1]>`, race 0 →
/// `ures.ani`; `InitAnim(.., 25 fps)`, `PlayAnimVector` onto the disc's texture), decoded once per spy.
pub struct HeadVideo {
    frames: Vec<crate::assets::Image>,
    shown: Option<usize>,
}

impl HeadVideo {
    pub fn load(install: &Install, race: u8) -> Self {
        let file = HEADS.get(if race == 0 { 16 } else { race as usize - 1 }).copied().unwrap_or("");
        let name = format!("anim/fejek/{file}");
        let frames: Vec<crate::assets::Image> = install
            .read(&name)
            .ok()
            .and_then(|b| {
                let ani = ig2_formats::ani::Ani::parse(&b).ok()?;
                let (w, h) = (ani.width as u32, ani.height as u32);
                // An opaque texture: pixels no frame wrote yet are black. `PlayAnimVector` (ELF 0x45ea70) uploads the
                // frames as they are; the stripes are the `VIDEO` material's (re/screens/render.md §3).
                let opaque = |mut rgba: Vec<u8>| {
                    for p in rgba.chunks_exact_mut(4) {
                        p[3] = 255;
                    }
                    rgba
                };
                Some(ani.frames().ok()?.into_iter().map(|f| crate::assets::Image { width: w, height: h, rgba: opaque(f.rgba) }).collect())
            })
            .unwrap_or_default();
        Self { frames, shown: None }
    }

    /// Put the frame for `seconds` into `tex` (25 frames a second, looping).
    pub fn show(&mut self, r: &mut Renderer, tex: TexId, seconds: f32) {
        if self.frames.is_empty() {
            return;
        }
        let k = ((seconds.max(0.0) * 25.0) as usize) % self.frames.len();
        if self.shown != Some(k) {
            r.replace_texture(tex, &self.frames[k]);
            self.shown = Some(k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::frame_keys;

    /// `SetFrame` keeps `frame·256`: the opening's thirds land on 1/256 steps, the last frame stays put.
    #[test]
    fn frames_blend_in_256ths() {
        assert_eq!(frame_keys(0.0, 51), (0, 1, 0.0));
        assert_eq!(frame_keys(12.5, 51), (12, 13, 0.5));
        assert_eq!(frame_keys(1.0 / 3.0, 51), (0, 1, 85.0 / 256.0));
        assert_eq!(frame_keys(50.0, 51), (50, 50, 0.0));
        assert_eq!(frame_keys(59.75, 61), (59, 60, 0.75));
        assert_eq!(frame_keys(-1.0, 51), (0, 1, 0.0));
    }
}
