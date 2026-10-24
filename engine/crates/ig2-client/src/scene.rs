//! `.xv` models as the original's 3D engine places them (re/screens/colony.md §2): the scene tree walked like
//! `Kocka2` (ELF 0x5fac80) into triangle lists in the model's root space, plus the object transforms of
//! `AddToScene`/`AddToSceneGenesis`/`ScaleObj`/`RotateObj`/`MoveObj` to put a model into the world.
//!
//! World coordinates are the engine's `(h, x, y)`: `h` is the height axis (up is −h), `x`, `y` the map plane.

use ig2_formats::xv::{Archive, ChildContent, Material, Node};
use ig2_formats::Install;

pub type V3 = [f32; 3];
/// A row-major 3×3 matrix, applied to column vectors (`MultipleMatrix(a, b)` = `b·a`).
pub type M3 = [f32; 9];

pub const IDENTITY: M3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|k| {
        let (r, c) = (k / 3, k % 3);
        (0..3).map(|i| a[r * 3 + i] * b[i * 3 + c]).sum()
    })
}

pub fn apply(m: &M3, v: V3) -> V3 {
    std::array::from_fn(|r| m[r * 3] * v[0] + m[r * 3 + 1] * v[1] + m[r * 3 + 2] * v[2])
}

pub fn scale(s: V3) -> M3 {
    [s[0], 0.0, 0.0, 0.0, s[1], 0.0, 0.0, 0.0, s[2]]
}

/// `CreateMatrix(a, b, c)` (ELF 0x451ee0), f32 sines and cosines.
pub fn create_matrix(a: f32, b: f32, c: f32) -> M3 {
    let (sa, ca) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
    let (sb, cb) = ((b as f64).sin() as f32, (b as f64).cos() as f32);
    let (sc, cc) = ((c as f64).sin() as f32, (c as f64).cos() as f32);
    [
        cc * cb,
        -cc * sb * sa - sc * ca,
        -cc * sb * ca + sc * sa,
        sc * cb,
        -sc * sb * sa + cc * ca,
        -sc * sb * ca - cc * sa,
        sb,
        cb * sa,
        cb * ca,
    ]
}

/// `creatematrixfromaxis(axis, θ)` (ELF 0x4520a0): the rotation by `θ` about the unit `axis`, from the quaternion
/// `(cos θ/2, sin θ/2·axis)`. `RotateObj2(axis, θ)` makes an object's rotation `axis(θ)·R`.
pub fn axis_matrix(axis: V3, a: f32) -> M3 {
    let n = normalise(axis);
    let (w, s) = ((a / 2.0).cos(), (a / 2.0).sin());
    let (x, y, z) = (n[0] * s, n[1] * s, n[2] * s);
    [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - w * z),
        2.0 * (x * z + w * y),
        2.0 * (x * y + w * z),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - w * x),
        2.0 * (x * z - w * y),
        2.0 * (y * z + w * x),
        1.0 - 2.0 * (x * x + y * y),
    ]
}

/// One vertex in a model's root space.
#[derive(Debug, Clone, Copy)]
pub struct Vert {
    pub pos: V3,
    pub normal: V3,
    pub uv: [f32; 2],
    /// The lighting normal (`lighting`): the file normal with z negated, turned by the nodes' rotation chain
    /// (`spsorttype+0x24`: animated nodes' key rotations; a record's matrix never enters it, `LoadXVB` resets the
    /// child's `+100` to the identity; no `AddToScene` scale).
    pub lnormal: V3,
}

/// How a part is blended (`SetObjTransparency`, property bits; re/screens/colony.md §2.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Look {
    Opaque,
    /// Alpha-tested foliage (`fak`: properties 0x100/0x200).
    Cutout,
    /// Transparent with the given opacity (`SetObjTransparency(t)` → opacity `1 − t`).
    Glass(f32),
}

/// The triangles of one material of one mesh, in the model's root space.
#[derive(Debug, Clone)]
pub struct Part {
    /// The `.pvr` the material's texture resolves to (`TextureDir` rule), if any.
    pub texture: Option<String>,
    /// The chain of child names from the root (`findboxxvb2` matches parts by name).
    pub path: String,
    /// The same chain as child indices (`GetSubMeshid(parent, child, i)`).
    pub index: Vec<u16>,
    pub verts: Vec<Vert>,
    pub look: Look,
    /// Per triangle, the wireframe edge mask (`wirekirak`: bit 0 = c–a, bit 1 = a–b, bit 2 = b–c).
    pub edges: Vec<u8>,
    /// The material's last word (`.xvm` +0x30, re/formats/xv.md) as an f32: its transparency (0 = opaque) [obs: the
    /// trade screen's `Racsszin`/`tozs_hk` hold 0.65 and ig2.exe draws them at opacity 89/255; trade.md §4.1].
    pub transparency: f32,
    /// The self-illumination pass (`selfillumkirak` ELF 0x613da0): when the mesh's first material has a second
    /// texture (`.xvm` `texture2`, `loadmaterial`'s +0x50), every batch is drawn again additively and unlit with its
    /// material's second texture (or, without one, its first: `changeaktmatarialself` binds nothing new). The ship
    /// hulls' lit windows (`6desttsi.bmp`).
    pub self_illum: Option<String>,
}

/// A loaded model: its root node's half-extents (`XVBStruct+4..+0xc`), its parts and the pose of every child.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub origin: V3,
    pub parts: Vec<Part>,
    pub nodes: Vec<NodePose>,
}

/// Where a child (`GetSubMeshid` chain `index`) was placed: its parent's matrix, its record translation, its matrix
/// (`P·M`) and translation in the root's space.
#[derive(Debug, Clone)]
pub struct NodePose {
    pub index: Vec<u16>,
    pub parent: M3,
    pub record: V3,
    pub matrix: M3,
    pub at: V3,
    /// The rotation chain to this child (lighting).
    pub light: M3,
}

impl Model {
    /// `LoadBox(name)` with the screen's `VectorDir`/`TextureDir`: `<vector_dir><stem>.xv`, textures by the
    /// game's lookup in `texture_dir`.
    pub fn load(install: &Install, vector_dir: &str, name: &str, texture_dir: &str) -> Result<Self, String> {
        Self::load_at(install, vector_dir, name, texture_dir, None)
    }

    /// [`Model::load`] posed at animation frame `frame` (`SetFrame`): every node with more than two frames places
    /// its children by their keys (`Kocka` ELF 0x5f6a40: the key's matrix, the record's translation moved by the
    /// key's change since frame 0), and every mesh with more than two frames takes that frame's vertex positions
    /// (`kirak` 0x60e160; the shipped ones: `spacewar/ora`, `spacewar/karika`, `colony/teszt`).
    pub fn load_at(install: &Install, vector_dir: &str, name: &str, texture_dir: &str, frame: Option<usize>) -> Result<Self, String> {
        let stem = name.trim_end_matches(".xvb").trim_end_matches(".XVB");
        let path = format!("dataraw/{vector_dir}{stem}.xv").to_ascii_lowercase();
        let bytes = install.read(&path).map_err(|e| format!("{path}: {e}"))?;
        let archive = Archive::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let scene = archive.scene(Some(stem)).map_err(|e| format!("{path}: {e}"))?.ok_or(format!("{path}: no root node"))?;
        let root = archive.root(Some(stem)).map(|m| m.name.clone()).unwrap_or_default();
        let mut w = Walk {
            archive: &archive,
            install,
            texture_dir,
            prefix: format!("{}-", root.split('.').next().unwrap_or_default()),
            parts: Vec::new(),
            nodes: Vec::new(),
            frame,
        };
        w.node(&scene, &IDENTITY, &IDENTITY, [0.0; 3], "", &[]);
        let mut model = Self { origin: scene.header.origin, parts: w.parts, nodes: w.nodes };
        if !skips_box_recount(&scene) {
            model.box_recount();
        }
        Ok(model)
    }

    /// `LoadXVBx`'s `BoxUjraszamol` (ELF 0x602750; `kilogasvizsgalat` 0x5ecb10, `boxujraszamol` 0x5ed9f0): the root's
    /// half-extents become the vertices' reach from its pivot per axis (`e`), and `+0x4c` = `max(e)/1024` when the
    /// file's largest half-extent is exactly 1024 (else it stays 1.0). `AddToScene` scales by `e/max(e)·s·+0x4c`, so the
    /// model as drawn is the file's scaled by `max(origin)/max(e)` for a root that isn't 1024 wide, unchanged otherwise
    /// (re/screens/design.md §14.3).
    fn box_recount(&mut self) {
        let mx = self.origin[0].max(self.origin[1]).max(self.origin[2]);
        if mx == 1024.0 {
            return;
        }
        let mut e = [0.0f32; 3];
        for p in self.positions() {
            for k in 0..3 {
                e[k] = e[k].max(p[k].abs() * self.origin[k]);
            }
        }
        let me = e[0].max(e[1]).max(e[2]);
        if me <= 0.0 || !me.is_finite() {
            return;
        }
        let g = mx / me;
        for p in &mut self.parts {
            for v in &mut p.verts {
                v.pos = v.pos.map(|x| x * g);
            }
        }
        for n in &mut self.nodes {
            n.parent = n.parent.map(|x| x * g);
            n.matrix = n.matrix.map(|x| x * g);
            n.at = n.at.map(|x| x * g);
        }
    }

    /// `MoveObj`/`RotateObj` on the child `index` (re/screens/spy.md §2.2): its record translation replaced by `to`
    /// (model units, `v/1024`) and its matrix turned by `rot` (`P·K·R`), moving every part below it.
    pub fn move_child(&mut self, index: &[u16], to: Option<V3>, rot: Option<&M3>) {
        let Some(n) = self.nodes.iter().find(|n| n.index == index).cloned() else { return };
        let shift = to.map_or([0.0; 3], |t| apply(&n.parent, [t[0] - n.record[0], t[1] - n.record[1], t[2] - n.record[2]]));
        // v' = at' + M·R·M⁻¹·(v − at); a collapsed (zero-scale) child keeps its points.
        let turn = rot.and_then(|r| inverse(&n.matrix).map(|inv| mul(&mul(&n.matrix, r), &inv)));
        let nturn = turn.map(|t| normal_matrix(&t));
        let lturn = rot.and_then(|r| inverse(&n.light).map(|inv| mul(&mul(&n.light, r), &inv)));
        for p in self.parts.iter_mut().filter(|p| p.index.starts_with(index)) {
            for v in &mut p.verts {
                let mut q = v.pos;
                if let (Some(t), Some(nt)) = (&turn, &nturn) {
                    let d = apply(t, [q[0] - n.at[0], q[1] - n.at[1], q[2] - n.at[2]]);
                    q = [d[0] + n.at[0], d[1] + n.at[1], d[2] + n.at[2]];
                    v.normal = normalise(apply(nt, v.normal));
                }
                if let Some(lt) = &lturn {
                    v.lnormal = apply(lt, v.lnormal);
                }
                v.pos = [q[0] + shift[0], q[1] + shift[1], q[2] + shift[2]];
            }
        }
    }

    /// The parts below child `index`.
    pub fn below<'a>(&'a self, index: &'a [u16]) -> impl Iterator<Item = &'a Part> + 'a {
        self.parts.iter().filter(move |p| p.index.starts_with(index))
    }

    /// This model's points moved `t` of the way to `other`'s (the same model at the next frame: the same parts in
    /// the same order; `SetFrame` between two keys).
    pub fn lerp(&self, other: &Model, t: f32) -> Model {
        if t == 0.0 {
            return self.clone();
        }
        let l = |x: V3, y: V3| -> V3 { std::array::from_fn(|k| x[k] + (y[k] - x[k]) * t) };
        let mut out = self.clone();
        for (p, q) in out.parts.iter_mut().zip(&other.parts) {
            if p.verts.len() == q.verts.len() {
                for (v, w) in p.verts.iter_mut().zip(&q.verts) {
                    v.pos = l(v.pos, w.pos);
                    v.normal = normalise(l(v.normal, w.normal));
                    // `Kocka` blends the keys' rotation matrices linearly (no renormalising).
                    v.lnormal = l(v.lnormal, w.lnormal);
                }
            }
        }
        out
    }

    /// Every vertex position (root space).
    pub fn positions(&self) -> impl Iterator<Item = V3> + '_ {
        self.parts.iter().flat_map(|p| p.verts.iter().map(|v| v.pos))
    }

    /// Set the look of the parts whose path contains `name` (`findboxxvb2` + `SetObjTransparency`).
    pub fn set_look(&mut self, name: &str, look: Look) {
        let n = name.to_ascii_lowercase();
        for p in &mut self.parts {
            if p.path.to_ascii_lowercase().contains(&n) {
                p.look = look;
            }
        }
    }
}

struct Walk<'a, 'b> {
    archive: &'a Archive<'b>,
    install: &'a Install,
    texture_dir: &'a str,
    prefix: String,
    parts: Vec<Part>,
    nodes: Vec<NodePose>,
    frame: Option<usize>,
}

impl Walk<'_, '_> {
    fn texture(&self, material: &str) -> Option<String> {
        let m = self.archive.get(&format!("{}{material}.xvm", self.prefix))?;
        let mat = Material::parse(m.data).ok()?;
        if mat.texture.is_empty() {
            return None;
        }
        self.install.resolve_texture(self.texture_dir, &mat.texture).or_else(|| {
            let screen = self.texture_dir.trim_start_matches("textures/").split('/').next().unwrap_or("");
            self.install.resolve_model_texture(screen, &mat.texture)
        })
    }

    /// The material's second texture (`texture2`), resolved like the first; `fenyek_2.bmp`/`reflec.bmp` become a
    /// runtime texture in `loadmaterial` (not loaded here).
    fn texture2(&self, material: &str) -> Option<String> {
        let m = self.archive.get(&format!("{}{material}.xvm", self.prefix))?;
        let mat = Material::parse(m.data).ok()?;
        if mat.texture2.is_empty() || mat.texture2.eq_ignore_ascii_case("fenyek_2.bmp") || mat.texture2.eq_ignore_ascii_case("reflec.bmp") {
            return None;
        }
        self.install.resolve_texture(self.texture_dir, &mat.texture2).or_else(|| {
            let screen = self.texture_dir.trim_start_matches("textures/").split('/').next().unwrap_or("");
            self.install.resolve_model_texture(screen, &mat.texture2)
        })
    }

    /// The material's transparency word (an f32; absent or not finite = 0).
    fn transparency(&self, material: &str) -> f32 {
        let m = self.archive.get(&format!("{}{material}.xvm", self.prefix));
        let t = m.and_then(|m| Material::parse(m.data).ok()).and_then(|m| m.flags).map_or(0.0, f32::from_bits);
        if t.is_finite() { t } else { 0.0 }
    }

    /// `Kocka2`: a child sits at `P·t + T` with matrix `P·M` (the child record's matrix; `RotateObj` is never
    /// applied inside a model).
    fn node(&mut self, n: &Node, p: &M3, pl: &M3, t: V3, path: &str, index: &[u16]) {
        let frames = n.header.frames() as usize;
        for (i, (c, content)) in n.children.iter().enumerate() {
            let mut ct = c.translation().map(|v| if v.is_finite() { v } else { 0.0 });
            let mut cm = c.matrix;
            // The lighting chain: `LoadXVB` copies the record's matrix to the child's `+100`, then resets `+100` to
            // the identity (only `+0x88` = record·`+100` keeps it), so an un-keyed child turns no normal (row 134).
            let mut cl = IDENTITY;
            if let (Some(f), true) = (self.frame, frames > 2) {
                if let (Some(k), Some(k0)) = (n.header.key(f.min(frames - 1), i), n.header.key(0, i)) {
                    cm = k.matrix;
                    cl = k.rotation;
                    ct = std::array::from_fn(|j| ct[j] + k.translation[j] - k0.translation[j]);
                }
            }
            let pt = apply(p, ct);
            let t2 = [pt[0] + t[0], pt[1] + t[1], pt[2] + t[2]];
            let m = mul(p, &cm);
            let ml = mul(pl, &cl);
            let path = format!("{path}/{}", c.name);
            let index: Vec<u16> = index.iter().copied().chain([i as u16]).collect();
            let record = c.translation().map(|v| if v.is_finite() { v } else { 0.0 });
            self.nodes.push(NodePose { index: index.clone(), parent: *p, record, matrix: m, at: t2, light: ml });
            match content {
                ChildContent::Node(sub) => self.node(sub, &m, &ml, t2, &path, &index),
                ChildContent::Mesh(mesh) => {
                    let texs: Vec<Option<String>> = mesh.materials.iter().map(|x| self.texture(x)).collect();
                    let trs: Vec<f32> = mesh.materials.iter().map(|x| self.transparency(x)).collect();
                    let tex2: Vec<Option<String>> = mesh.materials.iter().map(|x| self.texture2(x)).collect();
                    let lit = tex2.first().is_some_and(Option::is_some);
                    let nm = normal_matrix(&m);
                    let vf = self.frame.filter(|_| mesh.is_vertex_animated()).map(|f| f.min(mesh.frames() as usize - 1));
                    let mut by_mat: Vec<Vec<Vert>> = vec![Vec::new(); texs.len().max(1)];
                    let mut edges: Vec<Vec<u8>> = vec![Vec::new(); by_mat.len()];
                    for (fi, f) in mesh.faces.iter().enumerate() {
                        let k = (f.material as usize).min(by_mat.len() - 1);
                        if f.idx.iter().all(|&i| (i as usize) < mesh.vertices.len()) {
                            edges[k].push(mesh.face_flags.get(fi).copied().unwrap_or(7));
                        }
                        for &i in &f.idx {
                            let Some(v) = mesh.vertices.get(i as usize) else { continue };
                            let pos = vf.and_then(|f| mesh.frame_vertex(f, i as usize)).map_or(v.pos, |x| x.0);
                            let q = apply(&m, pos);
                            by_mat[k].push(Vert {
                                pos: [q[0] + t2[0], q[1] + t2[1], q[2] + t2[2]],
                                normal: normalise(apply(&nm, v.normal)),
                                uv: v.uv,
                                // `LoadObject` normalises the file normal (a zero one becomes (1, 0, 0)).
                                lnormal: apply(&ml, {
                                    let q = if v.normal == [0.0; 3] { [1.0, 0.0, 0.0] } else { normalise(v.normal) };
                                    [q[0], q[1], -q[2]]
                                }),
                            });
                        }
                    }
                    for (k, (verts, edges)) in by_mat.into_iter().zip(edges).enumerate() {
                        if !verts.is_empty() {
                            self.parts.push(Part { texture: texs.get(k).cloned().flatten(), path: path.clone(), index: index.clone(), verts, look: Look::Opaque, edges, transparency: trs.get(k).copied().unwrap_or(0.0), self_illum: if lit { tex2.get(k).cloned().flatten().or_else(|| texs.get(k).cloned().flatten()) } else { None } });
                        }
                    }
                }
                ChildContent::Missing => {}
            }
        }
    }
}

/// `BoxUjraszamol`'s guard: no recount for a root with more than one child whose first is a node (`XVBStruct` byte 0
/// clear), nor for one with more than two animation frames (the chain through `+0x18` is the LOD archive, not loaded
/// here).
fn skips_box_recount(root: &Node) -> bool {
    let first_is_node = matches!(root.children.first(), Some((_, ChildContent::Node(_))));
    (root.children.len() > 1 && first_is_node) || root.header.frames() > 2
}

/// The two integer frames around `frame` (within `n`) and the weight of the second: `SetFrame` keeps
/// `floor(frame)·256 + fract·256` as an integer, so the fraction goes in 1/256 steps.
pub fn frame_keys(frame: f32, n: usize) -> (usize, usize, f32) {
    let last = n.saturating_sub(1);
    let f = frame.max(0.0) as f64;
    let k = (f.floor() * 256.0 + (f - f.floor()) * 256.0) as i32;
    let f0 = (k >> 8) as usize;
    if f0 >= last {
        return (last, last, 0.0);
    }
    (f0, f0 + 1, (k & 0xff) as f32 / 256.0)
}

/// The inverse of `m`, if it isn't singular.
pub fn inverse(m: &M3) -> Option<M3> {
    let c = |r: usize, k: usize| m[r * 3 + k];
    let det = c(0, 0) * (c(1, 1) * c(2, 2) - c(1, 2) * c(2, 1)) - c(0, 1) * (c(1, 0) * c(2, 2) - c(1, 2) * c(2, 0)) + c(0, 2) * (c(1, 0) * c(2, 1) - c(1, 1) * c(2, 0));
    if det.abs() < 1e-30 || !det.is_finite() {
        return None;
    }
    let i = 1.0 / det;
    Some([
        (c(1, 1) * c(2, 2) - c(1, 2) * c(2, 1)) * i,
        (c(0, 2) * c(2, 1) - c(0, 1) * c(2, 2)) * i,
        (c(0, 1) * c(1, 2) - c(0, 2) * c(1, 1)) * i,
        (c(1, 2) * c(2, 0) - c(1, 0) * c(2, 2)) * i,
        (c(0, 0) * c(2, 2) - c(0, 2) * c(2, 0)) * i,
        (c(0, 2) * c(1, 0) - c(0, 0) * c(1, 2)) * i,
        (c(1, 0) * c(2, 1) - c(1, 1) * c(2, 0)) * i,
        (c(0, 1) * c(2, 0) - c(0, 0) * c(2, 1)) * i,
        (c(0, 0) * c(1, 1) - c(0, 1) * c(1, 0)) * i,
    ])
}

/// The inverse transpose of `m` (normals).
fn normal_matrix(m: &M3) -> M3 {
    let c = |r: usize, k: usize| m[r * 3 + k];
    let cof: M3 = [
        c(1, 1) * c(2, 2) - c(1, 2) * c(2, 1),
        c(1, 2) * c(2, 0) - c(1, 0) * c(2, 2),
        c(1, 0) * c(2, 1) - c(1, 1) * c(2, 0),
        c(0, 2) * c(2, 1) - c(0, 1) * c(2, 2),
        c(0, 0) * c(2, 2) - c(0, 2) * c(2, 0),
        c(0, 1) * c(2, 0) - c(0, 0) * c(2, 1),
        c(0, 1) * c(1, 2) - c(0, 2) * c(1, 1),
        c(0, 2) * c(1, 0) - c(0, 0) * c(1, 2),
        c(0, 0) * c(1, 1) - c(0, 1) * c(1, 0),
    ];
    // The cofactor matrix is the inverse transpose up to the determinant's scale (normals are renormalised).
    cof
}

pub fn normalise(v: V3) -> V3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-12 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [1.0, 0.0, 0.0]
    }
}

/// Where a model instance sits: `world = 1024·(R·S)·v + T` (the engine's scene unit is 1/1024 of a world unit:
/// `MoveObj` divides by 1024 and `dumpvertexlist`/`renderscene` multiply the scene by 1024).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub matrix: M3,
    pub translation: V3,
}

impl Placement {
    /// `AddToScene(xvb, parent, s)` (ELF 0x4551d0): the root's half-extents over their maximum, times `s`.
    pub fn add_to_scene(origin: V3, s: f32) -> Self {
        let mx = origin[0].max(origin[1]).max(origin[2]);
        let d = origin.map(|o| o / mx * s);
        Self { matrix: scale(d), translation: [0.0; 3] }
    }

    /// `AddToSceneGenesis(xvb, parent, s)` (ELF 0x455320): the root's half-extents times `s`.
    pub fn genesis(origin: V3, s: f32) -> Self {
        Self { matrix: scale(origin.map(|o| o * s)), translation: [0.0; 3] }
    }

    /// `ScaleObj(id, a, b, c)`: scales the record's matrix rows.
    pub fn scaled(mut self, s: V3) -> Self {
        self.matrix = mul(&scale(s), &self.matrix);
        self
    }

    /// `RotateObj(id, a, b, c)` after the scale: `CreateMatrix(a, b, c)·S` (`MultipleMatrix(S, R)`).
    pub fn rotated(&self, a: f32, b: f32, c: f32) -> M3 {
        mul(&create_matrix(a, b, c), &self.matrix)
    }

    /// The world transform of a model whose record has scale `self`, rotation `(a, b, c)`, moved to `world`
    /// (`MoveObj` in world units).
    pub fn world(&self, rot: (f32, f32, f32), world: V3) -> Transform {
        let m = self.rotated(rot.0, rot.1, rot.2);
        Transform { matrix: m.map(|x| x * 1024.0), translation: world }
    }
}

/// A finished world transform: `world = matrix·v + translation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub matrix: M3,
    pub translation: V3,
}

impl Transform {
    pub fn point(&self, v: V3) -> V3 {
        let q = apply(&self.matrix, v);
        [q[0] + self.translation[0], q[1] + self.translation[1], q[2] + self.translation[2]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_quarter_turn_maps_model_axes() {
        // `RotateObj(0, π/2, 0)` as `Init3dColony` uses it: (x, y, z) → (−z, y, x).
        let r = create_matrix(0.0, std::f32::consts::FRAC_PI_2, 0.0);
        let p = apply(&r, [1.0, 2.0, 3.0]);
        assert!((p[0] + 3.0).abs() < 1e-6 && (p[1] - 2.0).abs() < 1e-6 && (p[2] - 1.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn matrix_products_compose_like_multiple_matrix() {
        let a = scale([2.0, 3.0, 4.0]);
        let r = create_matrix(0.3, 0.2, 0.1);
        let v = [1.0, -1.0, 0.5];
        let both = apply(&mul(&r, &a), v);
        let step = apply(&r, apply(&a, v));
        for k in 0..3 {
            assert!((both[k] - step[k]).abs() < 1e-5);
        }
    }

    /// `BoxUjraszamol` (IG2_INSTALL_DIR; skipped without it): the light tank hull `ltankp` (root 6065.9 wide, its body
    /// reaching 1.65× that) is drawn with its furthest vertex at the `AddToScene` scale, 0.61× the file's size [obs:
    /// the design screen's tank at t500..t820, re/screens/design.md §14.3]; the 1024-wide Destroyer keeps its size and
    /// its shell's windows come from `MatJ`'s second texture.
    #[test]
    fn roots_are_remeasured_like_box_ujraszamol() {
        let Some(inst) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let inst = inst.expect("valid install");
        let reach = |m: &Model| {
            let mx = m.origin[0].max(m.origin[1]).max(m.origin[2]);
            m.positions().flat_map(|p| (0..3).map(move |k| (p[k] * m.origin[k] / mx).abs())).fold(0.0f32, f32::max)
        };
        let tank = Model::load(&inst, "vector/design/alapok/", "ltankp", "textures/design/tankalapok/").unwrap();
        assert!((tank.origin[0] - 6065.9).abs() < 0.1 && tank.origin[0] == tank.origin[1], "{:?}", tank.origin);
        assert!((reach(&tank) - 1.0).abs() < 1e-5, "{}", reach(&tank));
        let ship = Model::load(&inst, "vector/design/alapok/", "romb6_2", "textures/spacewar/").unwrap();
        assert!((reach(&ship) - 1.0).abs() < 1e-3, "{}", reach(&ship));
        let lit = |name: &str| ship.parts.iter().find(|p| p.path.ends_with(name)).and_then(|p| p.self_illum.clone());
        assert!(lit("Object02").is_some_and(|t| t.contains("6desttsi")));
        assert!(lit("Object05").is_some_and(|t| t.contains("6destsi1")));
        assert!(lit("Object08").is_none());
    }

    /// The lighting normals of `talalmanyok/vad1` (one child scaled diag(0.85, 1.90, 0.18)) are the file normals with
    /// z negated, the record's matrix left out: every normal the exe lit on the trade screen's hull [obs:
    /// `light-trade.jsonl` record 5, 305 of them, re/screens/trade.md §13 part 5] is one of the client's.
    #[test]
    fn lighting_normals_skip_the_record_matrix() {
        let Some(inst) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let inst = inst.expect("valid install");
        let dir = std::env::var_os("IG2_TRACE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
        let Ok(text) = std::fs::read_to_string(dir.join("light-trade.jsonl")) else { return };
        let hull = Model::load(&inst, "vector/talalmanyok/", "vad1", "textures/talalmanyok/").unwrap();
        let ln: Vec<V3> = hull.parts.iter().flat_map(|p| p.verts.iter().map(|v| v.lnormal)).collect();
        let r: serde_json::Value = text.lines().filter_map(|l| serde_json::from_str(l).ok()).find(|r: &serde_json::Value| r["verts"].as_array().is_some_and(|v| v.len() == 305)).unwrap();
        for v in r["verts"].as_array().unwrap() {
            let n: V3 = std::array::from_fn(|i| v[i].as_f64().unwrap() as f32);
            assert!(ln.iter().any(|l| (l[0] - n[0]).abs() < 1e-5 && (l[1] - n[1]).abs() < 1e-5 && (l[2] + n[2]).abs() < 1e-5), "{n:?}");
        }
    }
}
