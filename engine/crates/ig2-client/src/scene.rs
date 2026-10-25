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
    /// The file normal as `LoadObject` keeps it (normalised; a zero one is (1, 0, 0)), in its mesh's own space: the
    /// environment mapping's `N` (`envszamol`, [`env_uv`]).
    pub fnormal: V3,
}

/// The environment mapping (`vagashatarkirak` ELF 0x606c90 / exe 0x5395d0, property 0x4000; `envszamol` ELF 0x6110e0,
/// inlined in the exe): the texcoords of a vertex at `pos` with the file normal `n`, seen from `eye` (world space).
/// `inv` = the inverse of the mesh's world matrix (its node's `P·M` and the placement): `e` is the eye direction in the
/// mesh's own space, `normalizevektor`'d (left as is when shorter than 1e-5), and `uv = (e.x − n.x, e.y − n.y)` with no
/// scale or offset (about −2..2 on the repeating texture). `n.z` is unused.
pub fn env_uv(inv: &M3, eye: V3, pos: V3, n: V3) -> [f32; 2] {
    let e = apply(inv, [eye[0] - pos[0], eye[1] - pos[1], eye[2] - pos[2]]);
    let l = ((e[0] * e[0] + e[1] * e[1] + e[2] * e[2]) as f64).sqrt() as f32;
    let e = if l < 1e-5 { e } else { [e[0] / l, e[1] / l, e[2] / l] };
    [e[0] - n[0], e[1] - n[1]]
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
    /// The mesh's FIRST material's last word (`.xvm` +0x30, re/formats/xv.md) as an f32: the transparency `Kocka`
    /// (exe 0x5281f0) falls back to when the object's own is 0 (0 = opaque) [obs: the trade screen's
    /// `Racsszin`/`tozs_hk` hold 0.65 and ig2.exe draws them at opacity 89/255; trade.md §4.1]. See [`Part::opacity`].
    pub transparency: f32,
    /// The self-illumination pass (`selfillumkirak` ELF 0x613da0): when the mesh's first material has a second
    /// texture (`.xvm` `texture2`, `loadmaterial`'s +0x50), every batch is drawn again additively and unlit with its
    /// material's second texture (or, without one, its first: `changeaktmatarialself` binds nothing new). The ship
    /// hulls' lit windows (`6desttsi.bmp`).
    pub self_illum: Option<String>,
    /// The batch material's `texture` name as stored (lower case).
    pub texture_name: String,
    /// The batch material's `texture2` name as stored (`reflec.bmp`, `fenyek_2.bmp`, …; empty = none): what an
    /// environment-mapped part binds (`changeaktmatarialself`).
    pub texture2_name: String,
    /// Its mesh's matrix in the root's space (the node's `P·M`): the environment mapping's object space.
    pub matrix: M3,
}

impl Part {
    /// `Kocka` (exe 0x5281f0): the drawn opacity for the object transparency `object` (`SetObjTransparency`, 0 = none):
    /// the object's own when non-zero, else the mesh's first material's ([`Part::transparency`]), as the byte
    /// `float2int(255 − t·255)` (the vertex alpha, `fenyekszamolasa`; below 255 `renderstateallitas` blends).
    pub fn opacity(&self, object: f32) -> f32 {
        let t = if object != 0.0 { object } else { self.transparency };
        crate::lighting::float2int(255.0 - t * 255.0).clamp(0, 255) as f32 / 255.0
    }

    /// The drawn opacity of this part at the screen's opacity `alpha` (`1 −` the object transparency) and its
    /// [`Look`]: opaque (no object transparency) falls back to the material's ([`Part::opacity`]).
    pub fn alpha(&self, alpha: f32) -> f32 {
        let a = match self.look {
            Look::Glass(g) => g * alpha,
            _ => alpha,
        };
        if a >= 1.0 { self.opacity(0.0) } else { a }
    }

    /// Whether only the material makes this part transparent (the exe's blend keeps the depth write).
    pub fn material_glass(&self, alpha: f32) -> bool {
        self.transparency != 0.0 && !matches!(self.look, Look::Glass(_)) && alpha >= 1.0
    }
}

/// A loaded model: its root node's half-extents (`XVBStruct+4..+0xc`), its parts and the pose of every child.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub origin: V3,
    pub parts: Vec<Part>,
    pub nodes: Vec<NodePose>,
    /// `BoxUjraszamol` ran ([`skips_box_recount`] false): the half-extents became the vertices' reach.
    pub recounted: bool,
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
    /// The chain of child names to it, as [`Part::path`] (`GetObjWorldSpacePos` finds children by name).
    pub path: String,
}

impl Model {
    /// `LoadBox(name)` with the screen's `VectorDir`/`TextureDir`: `<vector_dir><stem>.xv`, textures by the
    /// game's lookup in `texture_dir`.
    pub fn load(install: &Install, vector_dir: &str, name: &str, texture_dir: &str) -> Result<Self, String> {
        Self::load_at(install, vector_dir, name, texture_dir, None)
    }

    /// The level of `name`'s level-of-detail chain ([`lod_chain`]) that `Kocka` draws for the model added with
    /// `AddToScene(…, s)` at camera depth `z` under the screen's `valtashatar` ([`lod_level`]), loaded and placed by
    /// the chain head's half-extents: `AddToScene` sizes the head and `AddToXVB` hands that slot to every node of the
    /// chain (re/formats/xv.md, LOD).
    pub fn load_lod(install: &Install, vector_dir: &str, name: &str, texture_dir: &str, s: f32, z: f32, valtashatar: i32) -> Result<Self, String> {
        let mut chain = lod_chain(install, vector_dir, name)
            .iter()
            .map(|n| Self::load(install, vector_dir, &format!("{n}.xvb"), texture_dir))
            .collect::<Result<Vec<_>, _>>()?;
        let level = lod_level(lod_extent(&chain), s, z, valtashatar, chain.len());
        let origin = chain[0].origin;
        let mut model = chain.swap_remove(level);
        model.origin = origin;
        Ok(model)
    }

    /// The vertices' reach from the pivot per axis, in the file's units (`max |p|·origin`).
    pub fn reach(&self) -> V3 {
        let mut e = [0.0f32; 3];
        for p in self.positions() {
            for k in 0..3 {
                e[k] = e[k].max(p[k].abs() * self.origin[k]);
            }
        }
        e
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
        let recounted = !skips_box_recount(&scene);
        let mut model = Self { origin: scene.header.origin, parts: w.parts, nodes: w.nodes, recounted };
        if recounted {
            model.box_recount();
        }
        Ok(model)
    }

    /// `LoadXVBx`'s `BoxUjraszamol` (ELF 0x602750; `kilogasvizsgalat` 0x5ecb10, `boxujraszamol` 0x5ed9f0): the root's
    /// half-extents become the vertices' reach from its pivot per axis (`e`), and `+0x4c` = `max(e)/1024` when the
    /// file's largest half-extent is exactly 1024 (else it stays 1.0). `AddToScene` scales by `e/max(e)·s·+0x4c`, so the
    /// model as drawn is the file's scaled by `max(origin)/max(e)` for a root that isn't 1024 wide, unchanged otherwise
    /// (re/screens/design.md §12c).
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

    /// The material's `texture` and `texture2` names as stored, lower case (empty when absent).
    fn texture_names(&self, material: &str) -> (String, String) {
        let m = self.archive.get(&format!("{}{material}.xvm", self.prefix));
        m.and_then(|m| Material::parse(m.data).ok()).map_or_else(Default::default, |m| (m.texture.to_ascii_lowercase(), m.texture2.to_ascii_lowercase()))
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
            // the identity (only `+0x88` = record·`+100` keeps it), so an un-keyed child turns no normal.
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
            self.nodes.push(NodePose { index: index.clone(), parent: *p, record, matrix: m, at: t2, light: ml, path: path.clone() });
            match content {
                ChildContent::Node(sub) => self.node(sub, &m, &ml, t2, &path, &index),
                ChildContent::Mesh(mesh) => {
                    let texs: Vec<Option<String>> = mesh.materials.iter().map(|x| self.texture(x)).collect();
                    let trs: Vec<f32> = mesh.materials.iter().map(|x| self.transparency(x)).collect();
                    let tex2: Vec<Option<String>> = mesh.materials.iter().map(|x| self.texture2(x)).collect();
                    let names: Vec<(String, String)> = mesh.materials.iter().map(|x| self.texture_names(x)).collect();
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
                            // `LoadObject` normalises the file normal (a zero one becomes (1, 0, 0)).
                            let fnormal = if v.normal == [0.0; 3] { [1.0, 0.0, 0.0] } else { normalise(v.normal) };
                            by_mat[k].push(Vert {
                                pos: [q[0] + t2[0], q[1] + t2[1], q[2] + t2[2]],
                                normal: normalise(apply(&nm, v.normal)),
                                uv: v.uv,
                                lnormal: apply(&ml, [fnormal[0], fnormal[1], -fnormal[2]]),
                                fnormal,
                            });
                        }
                    }
                    for (k, (verts, edges)) in by_mat.into_iter().zip(edges).enumerate() {
                        if !verts.is_empty() {
                            self.parts.push(Part { texture: texs.get(k).cloned().flatten(), path: path.clone(), index: index.clone(), verts, look: Look::Opaque, edges, transparency: trs.first().copied().unwrap_or(0.0), self_illum: if lit { tex2.get(k).cloned().flatten().or_else(|| texs.get(k).cloned().flatten()) } else { None }, texture_name: names.get(k).map(|n| n.0.clone()).unwrap_or_default(), texture2_name: names.get(k).map(|n| n.1.clone()).unwrap_or_default(), matrix: m });
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

/// The stems of `name`'s level-of-detail chain, the head first: each root's `lod_archive` names the next level
/// (`LoadObject` ELF: `+0x18` = the next node, `+0x10` = 1.0 while one follows; `flag10` → `flag11` → `flag12`,
/// `romb60` → `romb61` → `romb62`).
pub fn lod_chain(install: &Install, vector_dir: &str, name: &str) -> Vec<String> {
    let mut chain = vec![name.trim_end_matches(".xvb").to_string()];
    while chain.len() < 16 {
        let stem = chain.last().unwrap();
        let next = install.read(&format!("dataraw/{vector_dir}{stem}.xv").to_ascii_lowercase()).ok().and_then(|b| {
            let ar = Archive::parse(&b).ok()?;
            let h = ig2_formats::xv::NodeHeader::parse(ar.root(Some(stem))?.data).ok()?;
            (!h.lod_archive.is_empty()).then(|| h.lod_archive.trim_end_matches(".xvb").to_string())
        });
        match next {
            Some(n) => chain.push(n),
            None => break,
        }
    }
    chain
}

/// `AddToScene`'s diagonal for a scale of 1, in world units, of a model and its level-of-detail chain (the head
/// first): the head's half-extents after `BoxUjraszamol` (`e`, the reach of every level's vertices: the recount runs
/// down the `+0x18` chain [obs: `romb60`'s box takes x and y from `romb60`, z from `romb61`/`romb62`]) over their
/// maximum, times `+0x4c` = `max(e)/1024` for a root 1024 wide (so `e` itself), times 1024.
pub fn lod_extent(chain: &[Model]) -> V3 {
    let mut e = [0.0f32; 3];
    for m in chain {
        let r = m.reach();
        for k in 0..3 {
            e[k] = e[k].max(r[k]);
        }
    }
    let me = e[0].max(e[1]).max(e[2]);
    let o = chain.first().map_or([0.0; 3], |m| m.origin);
    if o[0].max(o[1]).max(o[2]) == 1024.0 || me <= 0.0 {
        e
    } else {
        e.map(|x| x / me * 1024.0)
    }
}

/// `Kocka`'s level-of-detail step (ELF 0x5f6a40, exe 0x5281f0): the box's size is `|M·(1, 1, 1)|²·(fov/−z)²` (`M`
/// its matrix, set by its parent before the recursion; `fov` 600, `z` clamped to −0.001 in scene units), times 4 on
/// entry; the renderer steps to the next node of the chain while `valtashatar·30^level < 4·size·lod²` (`lod` 1.0
/// as `LoadObject` sets it on every node with a successor; re/formats/xv.md, Level of detail), up to 10 levels. `M`'s rotation drops out of the length, so `extent·s` (the
/// `AddToScene` diagonal in world units, [`lod_extent`]) stands in for it; `z` is in world units.
/// [obs: probes/lod.js on the fleet panel: `romb60` at s 0.04 has size 3119.55, at 0.08 12478.14; `valtashatar` 1600
/// (`Starmap_Belep`)].
pub fn lod_level(extent: V3, s: f32, z: f32, valtashatar: i32, levels: usize) -> usize {
    const FOV: f32 = 600.0;
    let z = z / 1024.0;
    let k = if z <= -0.001 { -(FOV / z) } else { FOV / -0.001 };
    let v = extent.map(|e| e * s / 1024.0);
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let size = len * len * k * k;
    let (mut level, mut thr) = (0, valtashatar as f32);
    while level + 1 < levels && level < 10 && thr < size * 4.0 {
        level += 1;
        thr *= 30.0;
    }
    level
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
    fn env_uv_matches_the_exe() {
        // `probes/envmap.js` on grwar:17 of quick-1.sav (2026-10-25): a `vagashatarkirak` call's camera-space record
        // (matrix, point; the eye at the origin), three corners' file points and normals and the texcoords the exe drew.
        let m = [0.0, -48.418583, 0.0, 34.23711, 0.0, -7.110858, 34.237106, 0.0, 7.110856];
        let at = [-63.22683, -637.3606, -2591.87];
        let inv = inverse(&m).unwrap();
        for (p, n, uv) in [
            ([0.111014, 1.0, 1.0], [-2.8887797e-7, 0.75874317, 0.65138996], [0.32597104, -0.7747188]),
            ([-0.10750656, 0.99999994, 1.0], [-4.349908e-8, 0.7587433, 0.65138984], [0.32732293, -0.774711]),
            ([0.25529337, 0.6433839, -0.9999991], [-4.349908e-8, 0.7587433, 0.65138984], [0.32087967, -0.77207816]),
        ] {
            let q = apply(&m, p);
            let got = env_uv(&inv, [0.0; 3], [q[0] + at[0], q[1] + at[1], q[2] + at[2]], n);
            assert!((got[0] - uv[0]).abs() < 2e-6 && (got[1] - uv[1]).abs() < 2e-6, "{got:?} vs {uv:?}");
        }
    }

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
    /// the design screen's tank at t500..t820, re/screens/design.md §12c]; the 1024-wide Destroyer keeps its size and
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

    /// The fleet panel's Destroyer (`romb60` → `romb61` → `romb62`): `AddToScene`'s diagonal is the head's re-measured
    /// box and `Kocka`'s sizes are the exe's [obs: probes/lod.js, fx13g: matrix column lengths (0.04, 0.03351,
    /// 0.01588) at s 0.04, sizes 3119.55 and 12478.14 at z −0.5859]; under the starmap's `valtashatar` 1600 the
    /// cells draw `romb61` and the big preview `romb62`.
    #[test]
    fn panel_hulls_step_down_their_lod_chain_like_kocka() {
        let Some(inst) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let inst = inst.expect("valid install");
        let chain: Vec<Model> = ["romb60", "romb61", "romb62"].iter().map(|n| Model::load(&inst, "vector/spacewar/", &format!("{n}.xvb"), "textures/spacewar/").unwrap()).collect();
        let (head, e) = (&chain[0], lod_extent(&chain));
        for (k, want) in [0.04f32, 0.03351, 0.01588].into_iter().enumerate() {
            assert!((e[k] * 0.04 / 1024.0 - want).abs() < 1e-5, "{e:?}");
        }
        assert_eq!(lod_chain(&inst, "vector/spacewar/", "romb60.xvb"), ["romb60", "romb61", "romb62"]);
        assert_eq!(lod_level(e, 0.04, -600.0, 1600, 3), 1);
        assert_eq!(lod_level(e, 0.08, -600.0, 1600, 3), 2);
        assert_eq!(lod_level(e, 0.08, -600.0, 1600, 2), 1);
        let cell = Model::load_lod(&inst, "vector/spacewar/", "romb60.xvb", "textures/spacewar/", 0.04, -600.0, 1600).unwrap();
        assert!(cell.parts.iter().any(|p| p.path.ends_with("Object13")) && cell.origin == head.origin);
    }

    /// The lighting normals of `talalmanyok/vad1` (one child scaled diag(0.85, 1.90, 0.18)) are the file normals with
    /// z negated, the record's matrix left out: every normal the exe lit on the trade screen's hull [obs:
    /// `light-trade.jsonl` record 5, 305 of them, re/screens/trade.md §4.7] is one of the client's.
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

    /// `Kocka`'s fallback (IG2_INSTALL_DIR; skipped without it): the trade screen's panels (`Racsszin`, material 0.65)
    /// come out at the opacity byte 89 [obs: `run_screen.py --kep 8 --light-probe`, trade.md §4.7], the sphere grid at
    /// 191 (0.25), and an object transparency replaces the material's.
    #[test]
    fn material_transparency_is_the_fallback() {
        let Some(inst) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let inst = inst.expect("valid install");
        let m = Model::load(&inst, "vector/trade/", "tozs1bck.xvb", "textures/trade/").unwrap();
        let byte = |p: &Part, t: f32| (p.opacity(t) * 255.0).round() as u32;
        let panel = m.parts.iter().find(|p| p.path.ends_with("/kep01")).unwrap();
        assert_eq!(byte(panel, 0.0), 89);
        assert_eq!(byte(panel, 0.8), 51);
        let grid = m.parts.iter().find(|p| p.path.ends_with("/Sphere01")).unwrap();
        assert_eq!(byte(grid, 0.0), 191);
        assert_eq!(byte(grid, 1e-6), 255);
        assert_eq!(panel.alpha(1.0), panel.opacity(0.0));
        assert!(panel.material_glass(1.0) && !panel.material_glass(0.5));
    }
}
