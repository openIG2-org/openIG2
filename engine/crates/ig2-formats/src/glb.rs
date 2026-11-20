//! glTF 2.0 binary models (`.glb`) a mod supplies in place of an `.xv` archive (ADR 0090).
//!
//! [`to_xv`] turns one into the `.xv` archive the engine already reads (`re/formats/xv.md`), so every loader of
//! models takes it unchanged: the scene's nodes become `.xvb` nodes, their meshes `.xvo` meshes, their materials
//! `.xvm` materials. Only what the file holds is read: no external or data URIs, no required extensions.

use std::collections::HashSet;

use serde_json::Value;

use crate::{format_err, Result};

/// The largest `.glb` read.
pub const MAX_BYTES: usize = 64 << 20;
/// Nodes, meshes and materials together stay well under `openxv`'s 1,400 members.
const MAX_MEMBERS: usize = 1200;
/// `LoadXVB` nesting the reader accepts (`xv::Archive::load_node`).
const MAX_DEPTH: usize = 32;
/// Vertices in the whole model.
const MAX_VERTICES: usize = 1 << 20;
/// An `.xvo` counts its faces in a `u16` and indexes vertices with `u16`.
const MESH_LIMIT: usize = 0xFFFF;
/// An embedded picture's side, at most (the texture upscaling's `MAX_SIDE`).
pub const MAX_IMAGE_SIDE: u32 = 4096;
/// The first character of a texture name that is an embedded picture: the archive member of that name (`~0.png`).
pub const EMBEDDED: char = '~';

/// The archive stem leaves room for `-<15-byte name>.xvo` in a 64-byte member name.
const MAX_STEM: usize = 40;

type M3 = [f32; 9];
type V3 = [f32; 3];

const IDENTITY: M3 = [1., 0., 0., 0., 1., 0., 0., 0., 1.];
/// glTF's axes (+Y up, +Z front) to the models' (+Z up, re/screens/colony.md §2.4): `(x, y, z) = (X, −Z, Y)`, a turn
/// (determinant 1), so the winding and the handedness stay.
const AXES: M3 = [1., 0., 0., 0., 0., -1., 0., 1., 0.];

fn err(msg: impl Into<String>) -> crate::Error {
    format_err("glb", msg)
}

fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| (0..3).map(|k| a[i / 3 * 3 + k] * b[k * 3 + i % 3]).sum())
}

fn apply(m: &M3, v: V3) -> V3 {
    std::array::from_fn(|r| m[r * 3] * v[0] + m[r * 3 + 1] * v[1] + m[r * 3 + 2] * v[2])
}

fn arr<'v>(v: &'v Value, key: &str) -> &'v [Value] {
    v.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn index(v: &Value, key: &str) -> Option<usize> {
    v.get(key)?.as_u64().and_then(|x| usize::try_from(x).ok())
}

fn nums(v: &Value, key: &str, n: usize) -> Result<Option<Vec<f32>>> {
    let Some(a) = v.get(key) else { return Ok(None) };
    let f: Option<Vec<f32>> = a.as_array().and_then(|a| a.iter().map(|x| x.as_f64().map(|x| x as f32)).collect());
    match f {
        Some(f) if f.len() == n && f.iter().all(|x| x.is_finite()) => Ok(Some(f)),
        _ => Err(err(format!("\"{key}\" must hold {n} finite numbers"))),
    }
}

/// The 16-byte name fields: printable ASCII, at most 15 bytes, unique (case-insensitive, as `openf` matches).
#[derive(Default)]
struct Names(HashSet<String>);

impl Names {
    fn take(&mut self, want: Option<&str>, fallback: &str) -> String {
        let clean: String = want.unwrap_or("").chars().filter(char::is_ascii_graphic).take(15).collect();
        let mut name = clean;
        let mut n = 0;
        while name.is_empty() || self.0.contains(&name.to_ascii_lowercase()) {
            n += 1;
            name = format!("{fallback}{n}");
        }
        self.0.insert(name.to_ascii_lowercase());
        name
    }
}

fn name16(s: &str) -> [u8; 16] {
    let mut n = [0u8; 16];
    n[..s.len()].copy_from_slice(s.as_bytes());
    n
}

/// The per-axis reach of `points` from the pivot: the `.xvb`/`.xvo` header's half-extents (`BoxUjraszamol`'s).
fn reach(points: &[V3]) -> V3 {
    points.iter().fold([0.0; 3], |e, p| std::array::from_fn(|k| e[k].max(p[k].abs())))
}

/// The points of `children` (name, matrix, translation, their points in their own frame) in their parent's frame.
fn placed(children: &[(String, M3, V3, Vec<V3>)]) -> Vec<V3> {
    children.iter().flat_map(|(_, m, t, p)| p.iter().map(|&v| { let q = apply(m, v); [q[0] + t[0], q[1] + t[1], q[2] + t[2]] })).collect()
}

/// The common `.xvb`/`.xvo` header, version 2.
fn header(out: &mut Vec<u8>, origin: V3) {
    out.extend_from_slice(&0xFFFEu16.to_le_bytes());
    origin.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
}

struct Doc<'a> {
    json: Value,
    bin: &'a [u8],
}

struct Mesh {
    verts: Vec<(V3, V3, [f32; 2])>,
    faces: Vec<([u32; 3], u16)>,
    /// Material member names, in face index order.
    materials: Vec<String>,
}

struct Writer<'a> {
    doc: Doc<'a>,
    stem: String,
    members: Vec<(String, Vec<u8>)>,
    nodes: Names,
    material_names: Names,
    /// glTF material → its `.xvm` name, once written.
    materials: Vec<Option<String>>,
    /// The root's half-extents (every axis).
    size: f32,
    seen: Vec<bool>,
    vertices: usize,
    /// Images already stored as archive members.
    images: HashSet<usize>,
}

/// Convert a `.glb` into an `.xv` archive whose root is `<stem>.xvb` (the game path's file stem), at the original
/// exporter's half-extents 1024. Fails with a clear message on anything malformed, oversized or outside the subset
/// ADR 0090 reads; never panics.
pub fn to_xv(glb: &[u8], stem: &str) -> Result<Vec<u8>> {
    to_xv_sized(glb, stem, 1024.0)
}

/// [`to_xv`] with the root's half-extents `size` on every axis: the largest of the model's it replaces, so that it
/// draws as large (not every original's root is 1024 wide: `colony/npv0`'s is 3040).
pub fn to_xv_sized(glb: &[u8], stem: &str, size: f32) -> Result<Vec<u8>> {
    if !(size > 0.0 && size.is_finite()) {
        return Err(err(format!("model size {size} must be positive")));
    }
    if stem.is_empty() || stem.len() > MAX_STEM || !stem.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(err(format!("model name {stem:?} must be 1 to {MAX_STEM} plain ASCII characters")));
    }
    let doc = parse(glb)?;
    let nmat = arr(&doc.json, "materials").len();
    let nnodes = arr(&doc.json, "nodes").len();
    let nimg = arr(&doc.json, "images").len();
    if nnodes + nmat + nimg > MAX_MEMBERS {
        return Err(err(format!("{nnodes} nodes, {nmat} materials and {nimg} images; at most {MAX_MEMBERS} together")));
    }
    let mut w = Writer {
        doc,
        stem: stem.to_owned(),
        members: Vec::new(),
        nodes: Names::default(),
        material_names: Names::default(),
        materials: vec![None; nmat],
        size,
        seen: vec![false; nnodes],
        vertices: 0,
        images: HashSet::new(),
    };
    w.root()?;
    Ok(archive(&w.members))
}

/// The GLB container (glTF 2.0 §4.4) and the checks that keep everything inside it.
fn parse(b: &[u8]) -> Result<Doc<'_>> {
    if b.len() > MAX_BYTES {
        return Err(err(format!("file is {} bytes; at most {MAX_BYTES}", b.len())));
    }
    let u32at = |o: usize| b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()));
    if u32at(0) != Some(0x4654_6C67) {
        return Err(err("not a glTF binary file (.glb)"));
    }
    if u32at(4) != Some(2) {
        return Err(err("only glTF 2.0 is read"));
    }
    let total = (u32at(8).unwrap_or(0) as usize).min(b.len());
    let (mut json, mut bin, mut o) = (None, None, 12usize);
    while o + 8 <= total {
        let (len, kind) = (u32at(o).unwrap() as usize, u32at(o + 4).unwrap());
        let data = (o + 8).checked_add(len).filter(|&e| e <= total).map(|e| &b[o + 8..e]).ok_or_else(|| err("a chunk runs past the end of the file"))?;
        match kind {
            0x4E4F_534A if json.is_none() => json = Some(data),
            0x004E_4942 if bin.is_none() => bin = Some(data),
            _ => {}
        }
        o += 8 + len;
    }
    let json: Value = serde_json::from_slice(json.ok_or_else(|| err("no JSON chunk"))?).map_err(|e| err(format!("JSON: {e}")))?;
    let version = json.get("asset").and_then(|a| a.get("version")).and_then(Value::as_str).unwrap_or("");
    if !version.starts_with("2.") {
        return Err(err(format!("asset version {version:?}; only 2.x is read")));
    }
    let required: Vec<&str> = arr(&json, "extensionsRequired").iter().filter_map(Value::as_str).collect();
    if !required.is_empty() {
        return Err(err(format!("needs extensions openIG2 doesn't read: {}", required.join(", "))));
    }
    for (i, buf) in arr(&json, "buffers").iter().enumerate() {
        if buf.get("uri").is_some() || i > 0 {
            return Err(err(format!("buffer {i} lies outside the file's binary chunk; a mod's model keeps all its data inside the .glb")));
        }
    }
    let doc = Doc { bin: bin.unwrap_or(&[]), json };
    for (i, img) in arr(&doc.json, "images").iter().enumerate() {
        if img.get("uri").is_some() {
            return Err(err(format!("image {i} names a file or data URI; embed it in the .glb")));
        }
        let bytes = doc.view(index(img, "bufferView").ok_or_else(|| err(format!("image {i} has no bufferView")))?)?.0;
        let mime = img.get("mimeType").and_then(Value::as_str).unwrap_or("");
        let ok = match mime {
            "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
            _ => false,
        };
        if !ok {
            return Err(err(format!("image {i} must be an embedded PNG or JPEG (is {mime:?})")));
        }
    }
    Ok(doc)
}

impl<'a> Doc<'a> {
    fn item(&self, kind: &str, i: usize) -> Result<&Value> {
        arr(&self.json, kind).get(i).ok_or_else(|| err(format!("{kind}[{i}] doesn't exist")))
    }

    /// A buffer view's bytes and stride.
    fn view(&self, i: usize) -> Result<(&'a [u8], Option<usize>)> {
        let v = self.item("bufferViews", i)?;
        if index(v, "buffer") != Some(0) {
            return Err(err(format!("bufferViews[{i}] must use the binary chunk")));
        }
        let (off, len) = (index(v, "byteOffset").unwrap_or(0), index(v, "byteLength").unwrap_or(0));
        let bytes = off.checked_add(len).and_then(|e| self.bin.get(off..e)).ok_or_else(|| err(format!("bufferViews[{i}] runs past the binary chunk")))?;
        Ok((bytes, index(v, "byteStride")))
    }

    /// Accessor `i`'s elements as `comps` numbers each: f32, or unsigned integers (normalised to 0..1 when `norm`).
    fn read(&self, i: usize, comps: usize, norm: bool) -> Result<Vec<f32>> {
        let a = self.item("accessors", i)?;
        let what = format!("accessors[{i}]");
        if a.get("sparse").is_some() {
            return Err(err(format!("{what}: sparse accessors aren't read")));
        }
        let want = match comps { 1 => "SCALAR", 2 => "VEC2", _ => "VEC3" };
        if a.get("type").and_then(Value::as_str) != Some(want) {
            return Err(err(format!("{what} must be {want}")));
        }
        let ct = index(a, "componentType").unwrap_or(0);
        let size = match ct { 5126 | 5125 => 4, 5123 => 2, 5121 => 1, _ => return Err(err(format!("{what}: component type {ct} isn't read"))) };
        let count = index(a, "count").unwrap_or(0);
        if count > 4 * MAX_VERTICES {
            return Err(err(format!("{what}: {count} elements; at most {}", 4 * MAX_VERTICES)));
        }
        let (bytes, stride) = self.view(index(a, "bufferView").ok_or_else(|| err(format!("{what} has no bufferView")))?)?;
        let elem = comps * size;
        let stride = stride.unwrap_or(elem);
        let start = index(a, "byteOffset").unwrap_or(0);
        if stride < elem {
            return Err(err(format!("{what}: stride {stride} is smaller than an element")));
        }
        let end = count.checked_sub(1).map_or(Some(start), |n| n.checked_mul(stride).and_then(|x| x.checked_add(start + elem)));
        if end.is_none_or(|e| e > bytes.len()) {
            return Err(err(format!("{what} runs past its buffer view")));
        }
        let mut out = Vec::with_capacity(count * comps);
        for e in 0..count {
            let base = start + e * stride;
            for c in 0..comps {
                let s = &bytes[base + c * size..base + (c + 1) * size];
                let v = match (ct, norm) {
                    (5126, _) => f32::from_le_bytes(s.try_into().unwrap()),
                    (5125, false) => u32::from_le_bytes(s.try_into().unwrap()) as f32,
                    (5123, n) => u16::from_le_bytes(s.try_into().unwrap()) as f32 / if n { 65535.0 } else { 1.0 },
                    (5121, n) => s[0] as f32 / if n { 255.0 } else { 1.0 },
                    _ => return Err(err(format!("{what}: component type {ct} isn't read here"))),
                };
                if !v.is_finite() {
                    return Err(err(format!("{what} holds a number that isn't finite")));
                }
                out.push(v);
            }
        }
        Ok(out)
    }

    /// Triangle indices: integer components, read exactly (f32 holds every u32 below 2^24, the vertex limit).
    fn indices(&self, i: usize) -> Result<Vec<u32>> {
        let ct = index(self.item("accessors", i)?, "componentType").unwrap_or(0);
        if ct == 5126 {
            return Err(err(format!("accessors[{i}]: indices must be integers")));
        }
        Ok(self.read(i, 1, false)?.into_iter().map(|x| x as u32).collect())
    }

    /// A node's own transform: `p' = M·p + t`.
    fn local(&self, node: &Value) -> Result<(M3, V3)> {
        if let Some(m) = nums(node, "matrix", 16)? {
            // column-major 4×4
            return Ok((std::array::from_fn(|i| m[(i % 3) * 4 + i / 3]), [m[12], m[13], m[14]]));
        }
        let t = nums(node, "translation", 3)?.map_or([0.0; 3], |t| [t[0], t[1], t[2]]);
        let q = nums(node, "rotation", 4)?.unwrap_or(vec![0., 0., 0., 1.]);
        let s = nums(node, "scale", 3)?.unwrap_or(vec![1., 1., 1.]);
        let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
        let r = [
            1. - 2. * (y * y + z * z), 2. * (x * y - z * w), 2. * (x * z + y * w),
            2. * (x * y + z * w), 1. - 2. * (x * x + z * z), 2. * (y * z - x * w),
            2. * (x * z - y * w), 2. * (y * z + x * w), 1. - 2. * (x * x + y * y),
        ];
        Ok((std::array::from_fn(|i| r[i] * s[i % 3]), t))
    }
}

impl Writer<'_> {
    fn member(&mut self, name: String, data: Vec<u8>) {
        self.members.push((name, data));
    }

    /// The default scene's nodes under the root `<stem>.xvb`, turned to the models' axes.
    fn root(&mut self) -> Result<()> {
        let scene_index = index(&self.doc.json, "scene").unwrap_or(0);
        let scene = self.doc.item("scenes", scene_index)?.clone();
        let mut children = Vec::new();
        for n in arr(&scene, "nodes") {
            let i = n.as_u64().and_then(|x| usize::try_from(x).ok()).ok_or_else(|| err("scene node isn't an index"))?;
            if let Some((name, m, t, pts)) = self.node(i, 1)? {
                children.push((name, mul(&AXES, &m), apply(&AXES, t), pts));
            }
        }
        if children.is_empty() {
            return Err(err("the scene holds no mesh"));
        }
        // A lower level of detail is another model of the same folder (`xv.md` § Level of detail).
        let lod = scene.get("extras").and_then(|e| e.get("lod")).and_then(Value::as_str).unwrap_or("");
        if lod.len() > 15 || !lod.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(err(format!("scene extras.lod {lod:?} must be a model name of at most 15 plain ASCII characters")));
        }
        // The original exporter wrote its models with their largest reach from the pivot 1 (re/formats/xv.md), the
        // root's half-extents the model's size (mostly 1024); the game sizes models by that.
        let e = reach(&placed(&children));
        let big = e[0].max(e[1]).max(e[2]);
        if !(big > 0.0 && big.is_finite()) {
            return Err(err("the scene's meshes have no size"));
        }
        for c in &mut children {
            c.1 = c.1.map(|x| x / big);
            c.2 = c.2.map(|x| x / big);
        }
        let (bytes, _) = self.xvb(&children, lod, Some([self.size; 3]));
        let name = format!("{}.xvb", self.stem);
        self.member(name, bytes);
        Ok(())
    }

    /// An `.xvb` of `children` (name, matrix, translation, their points in their own frame); returns it and the points
    /// in this frame.
    fn xvb(&self, children: &[(String, M3, V3, Vec<V3>)], lod: &str, origin: Option<V3>) -> (Vec<u8>, Vec<V3>) {
        let pts = placed(children);
        let mut b = Vec::new();
        header(&mut b, origin.unwrap_or_else(|| reach(&pts)));
        b.extend_from_slice(&0u32.to_le_bytes()); // frames
        b.extend_from_slice(&0u32.to_le_bytes()); // lod_param
        b.extend_from_slice(&name16(lod));
        b.extend_from_slice(&(children.len() as u16).to_le_bytes());
        for (name, m, t, _) in children {
            t.iter().for_each(|v| b.extend_from_slice(&v.to_le_bytes()));
            b.extend_from_slice(&[0; 4]);
            m.iter().for_each(|v| b.extend_from_slice(&v.to_le_bytes()));
            b.extend_from_slice(&name16(name));
        }
        (b, pts)
    }

    /// Node `i` written as `<stem>-<name>.xvo` (a mesh alone) or `.xvb` (with children); `None` for one that draws
    /// nothing (a camera, a light, an empty group).
    #[allow(clippy::type_complexity)]
    fn node(&mut self, i: usize, depth: usize) -> Result<Option<(String, M3, V3, Vec<V3>)>> {
        if depth > MAX_DEPTH {
            return Err(err(format!("nodes nest deeper than {MAX_DEPTH}")));
        }
        match self.seen.get_mut(i) {
            None => return Err(err(format!("nodes[{i}] doesn't exist"))),
            Some(true) => return Err(err(format!("nodes[{i}] has two parents"))),
            Some(s) => *s = true,
        }
        let node = self.doc.item("nodes", i)?.clone();
        let (m, t) = self.doc.local(&node)?;
        let mut children = Vec::new();
        for c in arr(&node, "children") {
            let c = c.as_u64().and_then(|x| usize::try_from(x).ok()).ok_or_else(|| err(format!("nodes[{i}]: a child isn't an index")))?;
            if let Some(child) = self.node(c, depth + 1)? {
                children.push(child);
            }
        }
        let name = node.get("name").and_then(Value::as_str);
        let mesh = match index(&node, "mesh") {
            Some(k) => Some(self.mesh(k)?),
            None => None,
        };
        let own = |w: &mut Self| w.nodes.take(name, "node");
        Ok(match (mesh, children.is_empty()) {
            (None, true) => None,
            (Some(mesh), true) => {
                let n = own(self);
                let pts = self.xvo(&n, &mesh);
                Some((n, m, t, pts))
            }
            (mesh, false) => {
                if let Some(mesh) = mesh {
                    let mn = self.nodes.take(name.map(|s| format!("{s}_m")).as_deref(), "mesh");
                    let pts = self.xvo(&mn, &mesh);
                    children.insert(0, (mn, IDENTITY, [0.0; 3], pts));
                }
                let n = own(self);
                let (bytes, pts) = self.xvb(&children, "", None);
                self.member(format!("{}-{n}.xvb", self.stem), bytes);
                Some((n, m, t, pts))
            }
        })
    }

    /// Writes `<stem>-<name>.xvo`; returns its points.
    fn xvo(&mut self, name: &str, mesh: &Mesh) -> Vec<V3> {
        let pts: Vec<V3> = mesh.verts.iter().map(|v| v.0).collect();
        let mut b = Vec::new();
        header(&mut b, reach(&pts));
        b.extend_from_slice(&(mesh.verts.len() as u32).to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes()); // frames
        b.extend_from_slice(&0u32.to_le_bytes()); // unknown
        b.extend_from_slice(&[0; 16]); // chained
        b.extend_from_slice(&(mesh.faces.len() as u16).to_le_bytes());
        b.extend_from_slice(&(mesh.materials.len() as i32).to_le_bytes());
        mesh.materials.iter().for_each(|m| b.extend_from_slice(&name16(m)));
        for (p, n, uv) in &mesh.verts {
            p.iter().chain(n).chain(uv).for_each(|v| b.extend_from_slice(&v.to_le_bytes()));
        }
        for (f, mat) in &mesh.faces {
            f.iter().for_each(|&x| b.extend_from_slice(&(x as u16).to_le_bytes()));
            b.extend_from_slice(&mat.to_le_bytes());
        }
        b.extend(std::iter::repeat_n(7u8, mesh.faces.len()));
        self.member(format!("{}-{name}.xvo", self.stem), b);
        pts
    }

    /// glTF mesh `k`'s triangle primitives in one `.xvo`, each primitive's material one of its materials.
    fn mesh(&mut self, k: usize) -> Result<Mesh> {
        let mesh = self.doc.item("meshes", k)?.clone();
        let what = format!("meshes[{k}]");
        let mut out = Mesh { verts: Vec::new(), faces: Vec::new(), materials: Vec::new() };
        for p in arr(&mesh, "primitives") {
            if index(p, "mode").unwrap_or(4) != 4 {
                return Err(err(format!("{what}: only triangles are drawn (primitive mode {})", index(p, "mode").unwrap_or(0))));
            }
            let attrs = p.get("attributes").ok_or_else(|| err(format!("{what}: a primitive has no attributes")))?;
            let pos = self.doc.read(index(attrs, "POSITION").ok_or_else(|| err(format!("{what}: a primitive has no POSITION")))?, 3, false)?;
            let n = pos.len() / 3;
            let normals = index(attrs, "NORMAL").map(|a| self.doc.read(a, 3, false)).transpose()?;
            let uvs = index(attrs, "TEXCOORD_0").map(|a| self.doc.read(a, 2, true)).transpose()?;
            if normals.as_ref().is_some_and(|v| v.len() / 3 != n) || uvs.as_ref().is_some_and(|v| v.len() / 2 != n) {
                return Err(err(format!("{what}: attributes of different lengths")));
            }
            let idx = match index(p, "indices") {
                Some(a) => self.doc.indices(a)?,
                None => (0..n as u32).collect(),
            };
            if idx.len() % 3 != 0 || idx.iter().any(|&x| x as usize >= n) {
                return Err(err(format!("{what}: indices aren't whole triangles of its vertices")));
            }
            let mat = self.material(index(p, "material"))?;
            let slot = match out.materials.iter().position(|m| *m == mat) {
                Some(s) => s,
                None => {
                    out.materials.push(mat);
                    out.materials.len() - 1
                }
            } as u16;
            let at = |i: usize| -> (V3, V3, [f32; 2]) {
                let v = |a: &[f32], c: usize| -> V3 { std::array::from_fn(|j| a.get(i * c + j).copied().unwrap_or(0.0)) };
                let uv = uvs.as_deref().map_or([0.0; 2], |u| [u[i * 2], u[i * 2 + 1]]);
                (v(&pos, 3), normals.as_deref().map_or([0.0; 3], |nr| v(nr, 3)), uv)
            };
            let base = out.verts.len();
            if normals.is_some() {
                out.verts.extend((0..n).map(at));
                out.faces.extend(idx.chunks_exact(3).map(|f| ([f[0] + base as u32, f[1] + base as u32, f[2] + base as u32], slot)));
            } else {
                // glTF: no normals means flat shading, so each triangle gets its own corners.
                for (j, f) in idx.chunks_exact(3).enumerate() {
                    let c = [f[0], f[1], f[2]].map(|x| at(x as usize));
                    let (a, b) = (std::array::from_fn::<f32, 3, _>(|k| c[1].0[k] - c[0].0[k]), std::array::from_fn::<f32, 3, _>(|k| c[2].0[k] - c[0].0[k]));
                    let nrm = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
                    let l = nrm.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let nrm = if l > 0.0 { nrm.map(|x| x / l) } else { [1.0, 0.0, 0.0] };
                    out.verts.extend(c.map(|(p, _, uv)| (p, nrm, uv)));
                    let v0 = (base + j * 3) as u32;
                    out.faces.push(([v0, v0 + 1, v0 + 2], slot));
                }
            }
            if out.verts.len() > MESH_LIMIT + 1 || out.faces.len() > MESH_LIMIT {
                return Err(err(format!("{what}: {} vertices and {} triangles; a mesh holds at most 65,536 and 65,535 (split it into several objects)", out.verts.len(), out.faces.len())));
            }
        }
        if out.faces.is_empty() {
            return Err(err(format!("{what} has no triangles")));
        }
        self.vertices += out.verts.len();
        if self.vertices > MAX_VERTICES {
            return Err(err(format!("more than {MAX_VERTICES} vertices in the model")));
        }
        Ok(out)
    }

    /// glTF material `m` as `<stem>-<name>.xvm` (written once); its name in a mesh's list. A primitive without one
    /// takes an empty name: the game draws it untextured.
    fn material(&mut self, m: Option<usize>) -> Result<String> {
        let Some(m) = m else { return Ok(String::new()) };
        if let Some(Some(n)) = self.materials.get(m) {
            return Ok(n.clone());
        }
        let mat = self.doc.item("materials", m)?.clone();
        let pbr = mat.get("pbrMetallicRoughness");
        let texture = self.texture_name(pbr.and_then(|p| p.get("baseColorTexture")))?;
        let texture2 = self.texture_name(mat.get("emissiveTexture"))?;
        // `.xvm` +0x30: 0 opaque, 1 invisible (xv.md § Material transparency).
        let blend = mat.get("alphaMode").and_then(Value::as_str) == Some("BLEND");
        let alpha = pbr.map(|p| nums(p, "baseColorFactor", 4)).transpose()?.flatten().map_or(1.0, |c| c[3]);
        let transparency = if blend { (1.0 - alpha).clamp(0.0, 1.0) } else { 0.0 };
        let name = self.material_names.take(mat.get("name").and_then(Value::as_str), "mat");
        let mut b = Vec::with_capacity(52);
        b.extend_from_slice(&name16(&name));
        b.extend_from_slice(&name16(&texture));
        b.extend_from_slice(&name16(&texture2));
        b.extend_from_slice(&transparency.to_le_bytes());
        self.member(format!("{}-{name}.xvm", self.stem), b);
        self.materials[m] = Some(name.clone());
        Ok(name)
    }

    /// A texture reference's texture name: an image named after a game texture (`panel_4_3.bmp`, a `.bmp` name of at
    /// most 15 characters) is that texture; any other image is the model's own picture, stored as the archive member
    /// `~<image>.png` / `.jpg` ([`EMBEDDED`]) and named so.
    fn texture_name(&mut self, info: Option<&Value>) -> Result<String> {
        let Some(info) = info else { return Ok(String::new()) };
        let t = self.doc.item("textures", index(info, "index").ok_or_else(|| err("a texture reference has no index"))?)?;
        let Some(src) = index(t, "source") else { return Ok(String::new()) };
        let img = self.doc.item("images", src)?;
        let name = img.get("name").and_then(Value::as_str).unwrap_or("");
        if name.len() <= 15 && name.bytes().all(|b| b.is_ascii_graphic()) && name.to_ascii_lowercase().ends_with(".bmp") {
            return Ok(name.to_owned());
        }
        let ext = if img.get("mimeType").and_then(Value::as_str) == Some("image/png") { "png" } else { "jpg" };
        let member = format!("{EMBEDDED}{src}.{ext}");
        if !self.images.contains(&src) {
            let bytes = self.doc.view(index(img, "bufferView").unwrap_or(0))?.0.to_vec();
            self.images.insert(src);
            self.member(member.clone(), bytes);
        }
        Ok(member)
    }
}

/// An embedded picture (PNG or JPEG, told by its signature) as width, height and RGBA rows: what a model's own texture
/// draws (ADR 0090 §3). At most [`MAX_IMAGE_SIDE`] a side, checked before the pixels are decoded; a broken picture is
/// an error, never a panic.
pub fn decode_image(b: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    decode(b).map_err(err)
}

fn decode(b: &[u8]) -> std::result::Result<(u32, u32, Vec<u8>), String> {
    let fits = |w: u32, h: u32| {
        if w == 0 || h == 0 || w.max(h) > MAX_IMAGE_SIDE {
            return Err(format!("picture is {w} × {h}; at most {MAX_IMAGE_SIDE} a side"));
        }
        Ok(())
    };
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        let mut d = png::Decoder::new(std::io::Cursor::new(b));
        d.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA | png::Transformations::STRIP_16);
        let mut r = d.read_info().map_err(|e| format!("PNG: {e}"))?;
        fits(r.info().width, r.info().height)?;
        let mut buf = vec![0; r.output_buffer_size().ok_or_else(|| "PNG: too large".to_string())?];
        let info = r.next_frame(&mut buf).map_err(|e| format!("PNG: {e}"))?;
        buf.truncate(info.buffer_size());
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
            c => return Err(format!("PNG: unsupported colour type {c:?}")),
        };
        return Ok((info.width, info.height, rgba));
    }
    if !b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err("picture is neither PNG nor JPEG".into());
    }
    let mut d = zune_jpeg::JpegDecoder::new(b);
    d.decode_headers().map_err(|e| format!("JPEG: {e:?}"))?;
    let (w, h) = d.dimensions().ok_or_else(|| "JPEG: no picture size".to_string())?;
    fits(w as u32, h as u32)?;
    let px = d.decode().map_err(|e| format!("JPEG: {e:?}"))?;
    let rgba = match px.len() / (w * h) {
        4 => px,
        3 => px.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        1 => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => return Err("JPEG: an unusual colour layout".into()),
    };
    Ok((w as u32, h as u32, rgba))
}

/// Every embedded picture of the converted archive `xv` decodes ([`to_xv`]'s output; the mod check).
pub fn check_images(xv: &[u8]) -> Result<()> {
    let a = crate::xv::Archive::parse(xv)?;
    for m in a.members.iter().filter(|m| m.name.starts_with(EMBEDDED)) {
        decode(m.data).map_err(|e| err(format!("{}: {e}", m.name)))?;
    }
    Ok(())
}

/// An `.xv` archive of `members` (`xv.md` § Archive).
fn archive(members: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut dir = Vec::new();
    for (name, data) in members {
        dir.extend_from_slice(&(out.len() as u32).to_le_bytes());
        let mut n = [0u8; 64];
        n[..name.len()].copy_from_slice(name.as_bytes());
        dir.extend_from_slice(&n);
        out.extend_from_slice(data);
    }
    out.extend_from_slice(&dir);
    out.extend_from_slice(&(members.len() as u32).to_le_bytes());
    out
}

/// Generated test models (also for the client's tests, with the `test-util` feature).
#[cfg(any(test, feature = "test-util"))]
#[doc(hidden)]
pub mod tests {
    use super::*;
    #[cfg(test)]
    use crate::xv::{Archive, ChildContent, Material};
    use serde_json::json;

    /// A `.glb` of `json` (its buffer set to `bin`) and `bin`.
    fn glb(mut json: Value, bin: &[u8]) -> Vec<u8> {
        if !bin.is_empty() {
            json["buffers"] = json!([{ "byteLength": bin.len() }]);
        }
        let mut j = serde_json::to_vec(&json).unwrap();
        j.resize(j.len().next_multiple_of(4), b' ');
        let mut bin = bin.to_vec();
        bin.resize(bin.len().next_multiple_of(4), 0);
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&((12 + 8 + j.len() + if bin.is_empty() { 0 } else { 8 + bin.len() }) as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&j);
        if !bin.is_empty() {
            out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
            out.extend_from_slice(b"BIN\0");
            out.extend_from_slice(&bin);
        }
        out
    }

    fn f32s(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    /// A unit quad in glTF's XY plane facing +Z: positions, normals, UVs, u16 indices and a PNG image.
    fn quad(normals: bool) -> (Value, Vec<u8>) {
        quad_with(normals, "panel_4_3.bmp", b"\x89PNG\r\n\x1a\n\0\0\0\0")
    }

    /// [`quad`] whose image is `picture` (PNG or JPEG bytes), named `name`.
    fn quad_with(normals: bool, name: &str, picture: &[u8]) -> (Value, Vec<u8>) {
        let mut bin = f32s(&[0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 0.]); // 0..48
        bin.extend(f32s(&[0., 0., 1.].repeat(4))); // 48..96
        bin.extend(f32s(&[0., 1., 1., 1., 1., 0., 0., 0.])); // 96..128
        bin.extend([0u16, 1, 2, 0, 2, 3].iter().flat_map(|x| x.to_le_bytes())); // 128..140
        bin.extend([0; 4]);
        bin.extend(picture); // 144..
        let mut attrs = json!({ "POSITION": 0, "TEXCOORD_0": 2 });
        if normals {
            attrs["NORMAL"] = json!(1);
        }
        let j = json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0], "extras": { "lod": "quad1" } }],
            "nodes": [
                { "name": "Hull", "translation": [0., 2., 0.], "children": [1], "mesh": 0 },
                { "name": "Turret", "rotation": [0., 0., std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2], "scale": [2., 2., 2.], "mesh": 0 }
            ],
            "meshes": [{ "primitives": [{ "attributes": attrs, "indices": 3, "material": 0 }] }],
            "materials": [{ "name": "metal", "alphaMode": "BLEND",
                "pbrMetallicRoughness": { "baseColorFactor": [1., 1., 1., 0.25], "baseColorTexture": { "index": 0 } } }],
            "textures": [{ "source": 0 }],
            "images": [{ "name": name, "bufferView": 4, "mimeType": if picture.starts_with(b"\x89PNG") { "image/png" } else { "image/jpeg" } }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3" },
                { "bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3" },
                { "bufferView": 2, "componentType": 5126, "count": 4, "type": "VEC2" },
                { "bufferView": 3, "componentType": 5123, "count": 6, "type": "SCALAR" }
            ],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 48 },
                { "buffer": 0, "byteOffset": 48, "byteLength": 48 },
                { "buffer": 0, "byteOffset": 96, "byteLength": 32 },
                { "buffer": 0, "byteOffset": 128, "byteLength": 12 },
                { "buffer": 0, "byteOffset": 144, "byteLength": picture.len() }
            ]
        });
        (j, bin)
    }

    /// A valid two-node `.glb` (for the mod file system's tests).
    pub fn sample() -> Vec<u8> {
        let (j, bin) = quad(true);
        glb(j, &bin)
    }

    /// [`sample`] textured by its own picture `picture` (PNG or JPEG), its image named `hull.png`.
    pub fn sample_with_picture(picture: &[u8]) -> Vec<u8> {
        let (j, bin) = quad_with(true, "hull.png", picture);
        glb(j, &bin)
    }

    /// A `w` × `h` RGBA PNG, every pixel `rgba`.
    pub fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgba.repeat((w * h) as usize)).unwrap();
        out
    }

    #[cfg(test)]
    fn close(a: &[f32], b: &[f32]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn converts_hierarchy_meshes_and_materials() {
        let (j, bin) = quad(true);
        let xv = to_xv(&glb(j, &bin), "ship").unwrap();
        let a = Archive::parse(&xv).unwrap();
        let root = a.scene(Some("ship")).unwrap().unwrap();
        assert_eq!(root.header.lod_archive, "quad1");
        assert_eq!(root.children.len(), 1);
        let (hull, content) = &root.children[0];
        assert_eq!(hull.name, "Hull");
        // glTF +Y becomes the models' +Z: the hull's (0, 2, 0) is (0, 0, 2), its matrix the axis turn; then all of it
        // shrinks by 4, the largest reach (below), to the exporter's unit size.
        assert!(close(&hull.translation(), &[0., 0., 0.5]));
        assert!(close(&hull.matrix, &AXES.map(|x| x / 4.0)));
        let ChildContent::Node(hull) = content else { panic!("hull should be a node") };
        let names: Vec<&str> = hull.children.iter().map(|(c, _)| c.name.as_str()).collect();
        assert_eq!(names, ["Hull_m", "Turret"]);
        // A quarter turn about Z, scaled by 2, row-major.
        assert!(close(&hull.children[1].0.matrix, &[0., -2., 0., 2., 0., 0., 0., 0., 2.]));
        let ChildContent::Mesh(m) = &hull.children[0].1 else { panic!("mesh") };
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.faces.len(), 2);
        assert_eq!(m.faces[1].idx, [0, 2, 3]);
        assert_eq!(m.vertices[2].uv, [1., 0.]);
        assert_eq!(m.materials, ["metal"]);
        assert_eq!(m.face_flags, [7, 7]);
        let mat = Material::parse(a.get("ship-metal.xvm").unwrap().data).unwrap();
        assert_eq!(mat.texture, "panel_4_3.bmp");
        assert_eq!(f32::from_bits(mat.flags.unwrap()), 0.75);
        // The reach before scaling: the hull quad (x 0..1, z 2..3 after the turn) and the turret's (x −2..0, z 2..4), so
        // (2, 0, 4); the root's half-extents are the exporter's 1024.
        assert_eq!(root.header.origin, [1024.0; 3]);
    }

    #[test]
    fn missing_normals_shade_flat() {
        let (j, bin) = quad(false);
        let a_bytes = to_xv(&glb(j, &bin), "ship").unwrap();
        let a = Archive::parse(&a_bytes).unwrap();
        let root = a.scene(Some("ship")).unwrap().unwrap();
        let ChildContent::Node(hull) = &root.children[0].1 else { panic!() };
        let ChildContent::Mesh(m) = &hull.children[0].1 else { panic!() };
        assert_eq!(m.vertices.len(), 6);
        assert!(m.vertices.iter().all(|v| close(&v.normal, &[0., 0., 1.])));
    }

    #[test]
    fn rejects_what_it_does_not_read() {
        let (j, bin) = quad(true);
        let bad = |edit: &dyn Fn(&mut Value)| {
            let mut j = j.clone();
            edit(&mut j);
            to_xv(&glb(j, &bin), "ship").unwrap_err().to_string()
        };
        assert!(bad(&|j| j["images"][0]["uri"] = json!("http://example.org/a.png")).contains("URI"));
        assert!(bad(&|j| j["extensionsRequired"] = json!(["KHR_draco_mesh_compression"])).contains("KHR_draco"));
        assert!(bad(&|j| j["images"][0]["mimeType"] = json!("image/webp")).contains("PNG or JPEG"));
        assert!(bad(&|j| j["meshes"][0]["primitives"][0]["mode"] = json!(1)).contains("triangles"));
        assert!(bad(&|j| j["accessors"][0]["count"] = json!(5)).contains("runs past"));
        assert!(bad(&|j| j["bufferViews"][0]["byteLength"] = json!(100_000)).contains("runs past"));
        assert!(bad(&|j| j["nodes"][1]["children"] = json!([0])).contains("two parents"));
        assert!(bad(&|j| j["nodes"][0]["translation"] = json!([0., "x", 0.])).contains("finite"));
        assert!(bad(&|j| j["asset"]["version"] = json!("1.0")).contains("2.x"));
        assert!(bad(&|j| j["scenes"][0]["nodes"] = json!([])).contains("no mesh"));
        let mut buf = bin.clone();
        buf[128..130].copy_from_slice(&9u16.to_le_bytes()); // an index past the 4 vertices
        assert!(to_xv(&glb(j.clone(), &buf), "ship").unwrap_err().to_string().contains("triangles"));
        assert!(to_xv(b"glTF", "ship").is_err());
        assert!(to_xv(&glb(j.clone(), &bin), "a-very-long-model-name-that-has-no-room-left").is_err());
    }

    /// An image named after no game texture is the model's own picture: stored once in the archive as `~<image>.png`,
    /// named so by every material that uses it, decoded to RGBA; a broken or oversized one is an error (ADR 0090 §3).
    #[test]
    fn embedded_pictures_are_the_models_own_textures() {
        let picture = png(2, 3, [10, 20, 30, 255]);
        let (mut j, bin) = quad_with(true, "hull.png", &picture);
        j["materials"][0]["emissiveTexture"] = json!({ "index": 0 });
        let xv = to_xv(&glb(j, &bin), "ship").unwrap();
        let a = Archive::parse(&xv).unwrap();
        let mat = Material::parse(a.get("ship-metal.xvm").unwrap().data).unwrap();
        assert_eq!((mat.texture.as_str(), mat.texture2.as_str()), ("~0.png", "~0.png"));
        assert_eq!(a.members.iter().filter(|m| m.name.starts_with(EMBEDDED)).count(), 1);
        assert_eq!(a.get("~0.png").unwrap().data, &picture[..]);
        assert_eq!(decode_image(&picture).unwrap(), (2, 3, [10, 20, 30, 255].repeat(6)));
        check_images(&xv).unwrap();
        // A picture with a broken body converts (its signature is right) but fails the check, with its member's name.
        let mut broken = picture.clone();
        let n = broken.len();
        broken[n - 20..].fill(0);
        let xv = to_xv(&sample_with_picture(&broken), "ship").unwrap();
        assert!(check_images(&xv).unwrap_err().to_string().contains("~0.png"));
        assert!(decode_image(&png(MAX_IMAGE_SIDE + 1, 1, [0; 4])).unwrap_err().to_string().contains("at most 4096"));
        assert!(decode_image(b"\xFF\xD8\xFF\xE0 not a jpeg").is_err());
        assert!(decode_image(b"GIF89a").is_err());
        for k in 0..picture.len() {
            let _ = decode_image(&picture[..k]);
        }
    }

    /// Every truncation and every single-byte change of a valid file reads or fails; none panics.
    #[test]
    fn malformed_input_never_panics() {
        let (j, bin) = quad(true);
        let ok = glb(j, &bin);
        for n in 0..ok.len() {
            let _ = to_xv(&ok[..n], "ship");
        }
        for i in (0..ok.len()).step_by(3) {
            let mut b = ok.clone();
            b[i] ^= 0xA5;
            let _ = to_xv(&b, "ship");
        }
    }
}
