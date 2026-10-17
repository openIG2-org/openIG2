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

use crate::render::{Renderer, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Part, Placement, V3};

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
    /// `spyframe` 0..=50 (the opening), `spywireframe` 0..10.
    pub frame: f32,
    pub wire: f32,
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

pub struct SpyScene {
    frame: Vec<(Piece, f32)>,
    /// The sphere `Gomb` at frames 50..=60: edges (pairs of model-space points).
    wire: Vec<Vec<[V3; 2]>>,
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
        let main = Model::load_at(install, VECTORS, "spybck2.xvb", TEXTURES, Some(50))?;
        let mut frame = Vec::new();
        for p in &main.parts {
            let t = top(&p.path);
            if t == "Gomb" || t == "Group18" {
                continue;
            }
            let glass = if p.path.ends_with("/Object05") || p.path.ends_with("/Object06") { 0.8 } else { -1.0 };
            for piece in pieces(r, install, &[p], &mut cache, white) {
                frame.push((piece, glass));
            }
        }
        let masks = sphere_masks(install);
        let mut wire = Vec::new();
        for f in 50..=60 {
            let m = Model::load_at(install, VECTORS, "spybck2.xvb", TEXTURES, Some(f))?;
            let mut edges = Vec::new();
            for p in m.parts.iter().filter(|p| top(&p.path) == "Gomb") {
                for (i, t) in p.verts.chunks_exact(3).enumerate() {
                    let mask = masks.get(i).copied().unwrap_or(7);
                    let (a, b, c) = (t[0].pos, t[1].pos, t[2].pos);
                    if mask & 2 != 0 {
                        edges.push([a, b]);
                    }
                    if mask & 4 != 0 {
                        edges.push([b, c]);
                    }
                    if mask & 1 != 0 {
                        edges.push([c, a]);
                    }
                }
            }
            wire.push(edges);
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
        Ok(Self { frame, wire, pedestals, ring, disc, icon, race_tex, status_tex, white, head_tex, frame_origin: main.origin })
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
        let lights = Lights::new(p.orbit);
        // The frame (`SpyMainMesh`, scale 0.45 at (0, 2, −650)).
        let main = Placement::add_to_scene(self.frame_origin, 0.45).world((0.0, 0.0, 0.0), [0.0, 2.0, -650.0]);
        for (piece, glass) in &self.frame {
            let look = if *glass >= 0.0 { Look::Glass(1.0 - glass) } else { Look::Lit(0.0) };
            b.piece(piece, look, &lights, |v| main.point(v));
        }
        // The wireframe sphere (`SetFrame(spyframe + spywireframe)`): its edges at the texture's colour, unlit
        // but for the ambient light [obs: (1, 8, 10) = Racsszin (8, 101, 132) × 0x14/0xff].
        let k = p.wire.clamp(0.0, 10.0);
        let (f0, t) = (k.floor() as usize, k.fract());
        if let (Some(a), Some(c)) = (self.wire.get(f0), self.wire.get((f0 + 1).min(self.wire.len().saturating_sub(1)))) {
            let colour = [8.0, 101.0, 132.0].map(|c: f32| linear(c / 255.0 * AMBIENT));
            for (e0, e1) in a.iter().zip(c) {
                let lerp = |x: V3, y: V3| std::array::from_fn(|j| x[j] + (y[j] - x[j]) * t);
                b.line(main.point(lerp(e0[0], e1[0])), main.point(lerp(e0[1], e1[1])), colour, size, self.white);
            }
        }
        // The carousel.
        let (palast, rot) = self.palast(p);
        let spots = self.spots(p);
        for (i, ped) in self.pedestals.iter().enumerate() {
            let Some((race, state)) = p.slots.get(i).copied().flatten() else {
                // An empty pedestal: the sheet is hidden (`RarakKemekTexturai`), the base and its "---" stay.
                for x in &ped.base {
                    b.piece(x, glass_or_lit(spots[i].transparency, 0.0), &lights, |v| palast.point(Self::turned(v, ped.origin, rot)));
                }
                continue;
            };
            let extra = if p.selected == Some(i) { (((p.orbit * 90.0) as f64).sin() as f32 + 1.0) * 127.0 / 255.0 } else { 0.0 };
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
        if p.frame >= 25.0 {
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
                b.piece(x, Look::Lit(0.0), &lights, |v| ring.point(turn(v, p.spin as f32 / 220.0)));
            }
            let disc = Placement::add_to_scene([1024.0; 3], 0.173).world((0.0, 0.0, 0.0), [0.0, y, z]);
            let tex = p.head.unwrap_or(self.head_tex);
            for x in &self.disc {
                b.piece(&Piece { tex, ..x.clone() }, Look::Flat, &lights, |v| disc.point(turn(v, 0.0)));
            }
        }
        b.finish(size)
    }
}

/// The linear-light factor that scales a mid-tone texel (sRGB 0.4) as the engine's factor `x` scales it in sRGB.
fn gamma(x: f32) -> f32 {
    linear(0.4 * x.max(0.0)) / linear(0.4)
}

/// An sRGB value (0..1) in linear light.
fn linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn glass_or_lit(transparency: f32, extra: f32) -> Look {
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

/// `Spying3DInit`'s ambient 0x141414.
const AMBIENT: f32 = 0x14 as f32 / 255.0;

/// The screen's lights (`Spying3DInit`, `spyOldalsoFenyekBe` ELF 0x6b9c80, `spyMozgatMainFenyek` 0x6b9a60): the
/// ambient 0x141414, a white light 0x808080 at (0, 0, −10), and two coloured ones (0x7f5d00 and 0x004e7f) circling
/// at (±300, ±200, −1000) with `spykeringeto`, in the ring's plane. Shaded per vertex here (the engine's Gouraud pass)
/// as lights at those points, one-sided [inf: the capture's ring takes the colours on its rims only, on the side
/// the lights are; the white light's face-on value matches the frame's brackets and figures]. The colour words are
/// read as `0x00RRGGBB` [inf].
struct Lights {
    points: [(V3, [f32; 3]); 3],
}

impl Lights {
    fn new(orbit: f32) -> Self {
        // 0x80 is full light [inf: the capture's ring is drawn at its texture's own brightness, face-on].
        let rgb = |c: u32| [((c >> 16) & 0xff) as f32 / 128.0, ((c >> 8) & 0xff) as f32 / 128.0, (c & 0xff) as f32 / 128.0];
        let a = (orbit * 20.0) as f64;
        let l3 = [(a.sin() * 300.0) as f32, (a.cos() * 200.0) as f32, -1000.0];
        let l4 = [((a + 1.57075).sin() * 300.0) as f32, ((a + 1.57075).cos() * 200.0) as f32, -1000.0];
        Self { points: [([0.0, 0.0, -10.0], rgb(0x808080)), (l3, rgb(0x7f5d00)), (l4, rgb(0x004e7f))] }
    }
    /// The light on a vertex at world `p` with world normal `n`.
    fn shade(&self, p: V3, n: V3, extra: f32) -> [f32; 3] {
        let mut c = [AMBIENT + extra; 3];
        for (i, (at, col)) in self.points.iter().enumerate() {
            let l = scene::normalise([at[0] - p[0], at[1] - p[1], at[2] - p[2]]);
            let d = n[0] * l[0] + n[1] * l[1] + n[2] * l[2];
            let d = if i == 0 { d.abs() } else { d.max(0.0) };
            for k in 0..3 {
                c[k] += col[k] * d;
            }
        }
        c.map(|x| x.min(1.3))
    }
}

#[derive(Clone, Copy)]
enum Look {
    /// Lit opaque (alpha-tested), with an extra ambient term (the selected spy's pulse).
    Lit(f32),
    /// Unlit at this opacity (`SetObjTransparency`: transparent parts take the texture's colour × opacity
    /// [obs: the panels' (2, 20, 26) = Racsszin × 0.2]).
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
    fn piece(&mut self, x: &Piece, look: Look, lights: &Lights, at: impl Fn(V3) -> V3) {
        let start = self.dynamic.len() as u32;
        for v in &x.verts {
            // The engine lights and blends in the frame's gamma space; this frame is sRGB, so the factors go
            // through the transfer curve ([`gamma`]) to land on the same values.
            let p = at(v.pos);
            let (rgb, a) = match look {
                Look::Lit(e) => {
                    // The normal through the same transform (rotation and uniform scale).
                    let q = at([v.pos[0] + v.normal[0], v.pos[1] + v.normal[1], v.pos[2] + v.normal[2]]);
                    let n = scene::normalise([q[0] - p[0], q[1] - p[1], q[2] - p[2]]);
                    (lights.shade(p, n, e).map(gamma), 0.0)
                }
                Look::Glass(o) => ([gamma(o) / o.max(1e-6); 3], o),
                Look::Flat => ([1.0; 3], 0.0),
            };
            self.dynamic.push(MeshVertex { pos: p, normal: v.normal, uv: v.uv, color: [rgb[0], rgb[1], rgb[2], a] });
        }
        let d = MeshDraw { mesh: None, range: start..self.dynamic.len() as u32, tex: x.tex, glass: matches!(look, Look::Glass(_)) };
        if d.glass {
            self.glass.push(d);
        } else {
            self.opaque.push(d);
        }
    }

    /// A one-pixel line between two world points, as a thin quad facing the camera.
    fn line(&mut self, a: V3, b: V3, rgb: [f32; 3], size: [f32; 2], white: TexId) {
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
            self.dynamic.push(MeshVertex { pos: q, normal: n, uv: [0.0, 0.0], color: [rgb[0], rgb[1], rgb[2], 1.0] });
        }
        self.glass.push(MeshDraw { mesh: None, range: start..self.dynamic.len() as u32, tex: white, glass: true });
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
                // An opaque texture: pixels no frame wrote yet are black.
                // `DrawAnimInterlace`: every other row at half brightness [obs: the capture's stripes, 27 vs 54].
                let opaque = |mut rgba: Vec<u8>| {
                    for (i, p) in rgba.chunks_exact_mut(4).enumerate() {
                        if (i / w as usize) % 2 == 1 {
                            p[..3].iter_mut().for_each(|c| *c /= 2);
                        }
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
