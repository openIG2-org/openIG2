//! `.xv` 3D model archives: the original 1999 vector-object library format.
//!
//! Spec: `re/formats/xv.md`. Evidence (ELF `ig2`): `openxv` @0x5fd8a0 (archive and directory),
//! `openf` (member lookup), `LoadXVB` @0x601c20 (scene node), `LoadObject` @0x600e30 (mesh),
//! `loadmaterial` @0x5ff760 (material), `LoadXVBx` @0x602ad0 (root entry point).
//!
//! An archive holds members named `<object>.xvb` (root node), `<object>-<child>.xvb|.xvo|.xvm`.
//! Children are resolved as `<prefix><name>.xvb`, falling back to `<prefix><name>.xvo`, where
//! the prefix is the root name up to its first `.` followed by `-`.

use crate::{format_err, Result};

const DIR_ENTRY: usize = 68;

fn err(msg: impl Into<String>) -> crate::Error {
    format_err("xv", msg)
}

/// Bounds-checked little-endian cursor.
struct Cur<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> Cur<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self.b.get(self.o..self.o + n).ok_or_else(|| err(format!("read of {n} at {} past end", self.o)))?;
        self.o += n;
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn name16(&mut self) -> Result<String> {
        Ok(cstr(self.bytes(16)?))
    }
    fn rest(&self) -> usize {
        self.b.len() - self.o
    }
}

/// NUL-terminated Latin-1 string. Bytes after the NUL are uninitialized tool memory.
fn cstr(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect()
}

#[derive(Debug, Clone)]
pub struct Archive<'a> {
    pub members: Vec<Member<'a>>,
}

#[derive(Debug, Clone)]
pub struct Member<'a> {
    pub name: String,
    pub data: &'a [u8],
}

impl<'a> Archive<'a> {
    pub fn parse(b: &'a [u8]) -> Result<Self> {
        if b.len() < 4 {
            return Err(err("archive too short"));
        }
        let n = u32::from_le_bytes(b[b.len() - 4..].try_into().unwrap()) as usize;
        if n >= 0x578 {
            // `openxv` asserts hanyfilename < 1400.
            return Err(err(format!("member count {n} exceeds engine limit")));
        }
        let dir = b.len().checked_sub(4 + n * DIR_ENTRY).ok_or_else(|| err("directory larger than file"))?;
        let mut entries: Vec<(usize, String)> = (0..n)
            .map(|i| {
                let e = &b[dir + i * DIR_ENTRY..dir + (i + 1) * DIR_ENTRY];
                (u32::from_le_bytes(e[..4].try_into().unwrap()) as usize, cstr(&e[4..]))
            })
            .collect();
        let order = entries.clone();
        entries.sort_by_key(|(o, _)| *o);
        let mut members = Vec::with_capacity(n);
        for (off, name) in &order {
            let next = entries.iter().map(|(o, _)| *o).find(|o| o > off).unwrap_or(dir);
            let data = b.get(*off..next).ok_or_else(|| err(format!("member {name} out of bounds")))?;
            members.push(Member { name: name.clone(), data });
        }
        Ok(Self { members })
    }

    /// Case-insensitive member lookup, as `openf` uppercases both sides.
    pub fn get(&self, name: &str) -> Option<&Member<'a>> {
        self.members.iter().find(|m| m.name.eq_ignore_ascii_case(name))
    }

    /// The root node. The game always names it (`LoadXVBx("<stem>.xvb")`), so prefer the
    /// archive's own stem, then fall back to a single dash-free `.xvb` member. Some archives hold
    /// only meshes (`spacewar/flag22.xv`); the game loads those with `LoadObject` directly.
    pub fn root(&self, archive_stem: Option<&str>) -> Option<&Member<'a>> {
        if let Some(m) = archive_stem.and_then(|s| self.get(&format!("{s}.xvb"))) {
            return Some(m);
        }
        let mut roots = self
            .members
            .iter()
            .filter(|m| m.name.to_ascii_lowercase().ends_with(".xvb") && !m.name.contains('-'));
        match (roots.next(), roots.next()) {
            (Some(r), None) => Some(r),
            _ => None,
        }
    }

    /// Load the scene tree under the root node, if the archive has one.
    pub fn scene(&self, archive_stem: Option<&str>) -> Result<Option<Node>> {
        let Some(root) = self.root(archive_stem) else { return Ok(None) };
        let stem = root.name.split('.').next().unwrap_or_default();
        self.load_node(root, &format!("{stem}-"), 0).map(Some)
    }

    fn load_node(&self, m: &Member<'a>, prefix: &str, depth: usize) -> Result<Node> {
        if depth > 32 {
            return Err(err("node nesting too deep"));
        }
        let header = NodeHeader::parse(m.data)?;
        let mut children = Vec::with_capacity(header.children.len());
        for c in &header.children {
            let content = if let Some(sub) = self.get(&format!("{prefix}{}.xvb", c.name)) {
                ChildContent::Node(Box::new(self.load_node(sub, prefix, depth + 1)?))
            } else if let Some(obj) = self.get(&format!("{prefix}{}.xvo", c.name)) {
                ChildContent::Mesh(Box::new(Mesh::parse(obj.data)?))
            } else {
                ChildContent::Missing
            };
            children.push((c.clone(), content));
        }
        Ok(Node { header, children })
    }
}

/// Shared header of `.xvb` and `.xvo`.
fn origin(c: &mut Cur) -> Result<(u8, [f32; 3])> {
    let marker = c.u16()?;
    Ok(match marker {
        0xFFFF | 0xFFFE => (if marker == 0xFFFE { 2 } else { 1 }, [c.f32()?, c.f32()?, c.f32()?]),
        x => (0, [x as f32, c.u16()? as f32, c.u16()? as f32]),
    })
}

#[derive(Debug, Clone)]
pub struct ChildRef {
    /// Bytes 0x00..0x10 of the child record: `f32[3]` translation ([`ChildRef::translation`]), then
    /// 4 bytes no code reads (often the exporter's `0xCDCDCDCD` fill).
    pub head: [u8; 16],
    /// 3×3 rotation/scale matrix, row-major (`+0x10`, copied to `+100` and fed to `MultipleMatrix`).
    pub matrix: [f32; 9],
    pub name: String,
}

impl ChildRef {
    /// The child's offset in the parent's frame: `p' = M·v + t` (`Kocka2` ELF 0x5fac80 and `Kocka`
    /// 0x5f6a40 read child record +0x00..+0x0c as three floats).
    pub fn translation(&self) -> [f32; 3] {
        std::array::from_fn(|i| f32::from_le_bytes(self.head[4 * i..4 * i + 4].try_into().unwrap()))
    }
}

/// One node animation key (21 f32), as `Kocka` (ELF 0x5f6a40) interpolates it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeKey {
    /// Full child transform (rotation and scale), replacing the child record's matrix while animated.
    pub matrix: [f32; 9],
    /// Rotation only: the chain used for normals/lighting (`spsorttype+0x24`).
    pub rotation: [f32; 9],
    /// Translation key. The child is placed at `child.translation + key.translation −
    /// key₀.translation` (key₀ = frame 0 of the same child).
    pub translation: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct NodeHeader {
    pub version: u8,
    pub origin: [f32; 3],
    /// Raw frame word; the high bit is a flag.
    pub frames_raw: u32,
    /// Stored to `XVB+0x10` when a LOD archive is chained [inf: switch distance].
    pub lod_param: u32,
    /// Lower-detail archive (`openxv(lod)`) [inf].
    pub lod_archive: String,
    pub children: Vec<ChildRef>,
    /// `frames × children` animation keys of 84 bytes (21 × f32), frame-major ([`NodeHeader::key`]).
    pub anim: Vec<f32>,
}

impl NodeHeader {
    pub fn frames(&self) -> u32 {
        self.frames_raw & 0x7FFF_FFFF
    }

    /// Key of `child` at `frame` (index `(frame · children + child) · 21`, `Kocka` ELF 0x5f6a40).
    /// The game only uses keys after `SetFrame` gave the child a frame (child `+0xe4` ≥ 0, 8.8 fixed
    /// point: `frame >> 8` with `(frame & 0xff) / 256` linear blending towards the next key, clamped
    /// at the last one). Many 1–2-frame nodes hold uninitialised fill here, so don't use their keys
    /// unless the game animates that child.
    pub fn key(&self, frame: usize, child: usize) -> Option<NodeKey> {
        let k = self.anim.get((frame * self.children.len() + child) * 21..)?.get(..21)?;
        Some(NodeKey {
            matrix: k[..9].try_into().unwrap(),
            rotation: k[9..18].try_into().unwrap(),
            translation: k[18..].try_into().unwrap(),
        })
    }

    pub fn parse(b: &[u8]) -> Result<Self> {
        let mut c = Cur { b, o: 0 };
        let (version, origin) = origin(&mut c)?;
        let frames_raw = c.u32()?;
        let lod_param = c.u32()?;
        let lod_archive = c.name16()?;
        let n = c.u16()? as usize;
        let mut children = Vec::with_capacity(n);
        for _ in 0..n {
            let rec = c.bytes(0x44)?;
            let f = |i: usize| f32::from_le_bytes(rec[0x10 + 4 * i..0x14 + 4 * i].try_into().unwrap());
            children.push(ChildRef {
                head: rec[..16].try_into().unwrap(),
                matrix: std::array::from_fn(f),
                name: cstr(&rec[0x34..0x44]),
            });
        }
        let frames = (frames_raw & 0x7FFF_FFFF) as usize;
        let anim = c.bytes(n * frames * 84)?.chunks_exact(4).map(|x| f32::from_le_bytes(x.try_into().unwrap())).collect();
        Ok(Self { version, origin, frames_raw, lod_param, lod_archive, children, anim })
    }
}

#[derive(Debug, Clone)]
pub enum ChildContent {
    Node(Box<Node>),
    Mesh(Box<Mesh>),
    /// Neither `.xvb` nor `.xvo` member exists. `LoadObject` returns NULL in the game too.
    Missing,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub header: NodeHeader,
    pub children: Vec<(ChildRef, ChildContent)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    /// As stored; the game normalises at load (zero → (1,0,0)).
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Face {
    pub idx: [u16; 3],
    /// Index into `Mesh::materials` (v2 only; clamped to the material count like the game).
    pub material: u16,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub version: u8,
    pub origin: [f32; 3],
    pub frames_raw: u32,
    /// Read into a local and dropped by `LoadObject` (ELF 0x600e30): no meaning in the game.
    pub unknown: u32,
    /// Chained object loaded recursively by `LoadObject` [inf: LOD or attachment].
    pub chained: String,
    /// Material member names (resolved as `<prefix><name>.xvm` by `loadmaterial`).
    pub materials: Vec<String>,
    pub vertices: Vec<Vertex>,
    pub faces: Vec<Face>,
    /// `frames × vertices × 6` f32: position + normal per vertex per frame ([`Mesh::frame_vertex`]).
    pub anim: Vec<f32>,
    /// Per-face wireframe edge mask (`wirekirak` ELF 0x6083f0): bit 0 = edge c–a, bit 1 = a–b,
    /// bit 2 = b–c (`idx = [a, b, c]`). Only the wireframe renderer reads it. Defaults to 7 (all edges)
    /// when absent (68 shipped v0 meshes omit the block).
    pub face_flags: Vec<u8>,
}

impl Mesh {
    pub fn frames(&self) -> u32 {
        self.frames_raw & 0x7FFF_FFFF
    }

    /// Whether the renderer blends vertex frames: `kirak` (ELF 0x60e160) needs at least 3 frames
    /// (and a frame set on the child); `LoadObject` runs `optimizeobject` on the others. The frame
    /// block of 1–2-frame meshes is often uninitialised fill.
    pub fn is_vertex_animated(&self) -> bool {
        self.frames() >= 3
    }

    /// Position and normal of `vertex` at `frame`. `kirak` lerps the positions of frame `f` and
    /// `f + 1` (clamped to the last frame) and transforms them like the base vertices; the normals
    /// aren't read by the shipped renderer (frame 0 equals the base vertices).
    pub fn frame_vertex(&self, frame: usize, vertex: usize) -> Option<([f32; 3], [f32; 3])> {
        let v = self.anim.get((frame * self.vertices.len() + vertex) * 6..)?.get(..6)?;
        Some((v[..3].try_into().unwrap(), v[3..].try_into().unwrap()))
    }

    pub fn parse(b: &[u8]) -> Result<Self> {
        let mut c = Cur { b, o: 0 };
        let (version, origin) = origin(&mut c)?;
        let nverts = c.u32()? as usize;
        let frames_raw = c.u32()?;
        let unknown = c.u32()?;
        let chained = c.name16()?;
        let nfaces = c.u16()? as usize;
        let (nmat, read_names) = if version == 2 {
            let n = c.u32()? as i32;
            if n < 1 { (1, false) } else { (n as usize, true) }
        } else {
            (1, true)
        };
        let materials = (0..nmat)
            .map(|_| if read_names { c.name16() } else { Ok(String::new()) })
            .collect::<Result<Vec<_>>>()?;
        let vb = c.bytes(nverts * 32)?;
        let fl = |s: &[u8], i: usize| f32::from_le_bytes(s[4 * i..4 * i + 4].try_into().unwrap());
        let vertices = vb
            .chunks_exact(32)
            .map(|v| Vertex {
                pos: [fl(v, 0), fl(v, 1), fl(v, 2)],
                normal: [fl(v, 3), fl(v, 4), fl(v, 5)],
                uv: [fl(v, 6), fl(v, 7)],
            })
            .collect();
        let fb = c.bytes(nfaces * 8)?;
        let faces = fb
            .chunks_exact(8)
            .map(|f| {
                let w = |i: usize| u16::from_le_bytes([f[2 * i], f[2 * i + 1]]);
                let material = if version == 2 { w(3).min(nmat as u16 - 1) } else { 0 };
                Face { idx: [w(0), w(1), w(2)], material }
            })
            .collect();
        let frames = (frames_raw & 0x7FFF_FFFF) as usize;
        let anim = c.bytes(frames * nverts * 24)?.chunks_exact(4).map(|x| f32::from_le_bytes(x.try_into().unwrap())).collect();
        let face_flags = if c.rest() >= nfaces { c.bytes(nfaces)?.to_vec() } else { vec![7; nfaces] };
        Ok(Self { version, origin, frames_raw, unknown, chained, materials, vertices, faces, anim, face_flags })
    }
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub texture: String,
    pub texture2: String,
    /// Absent in 5 shipped 48-byte members; the game would read the next member's bytes there.
    pub flags: Option<u32>,
}

impl Material {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let mut c = Cur { b, o: 0 };
        let name = c.name16()?;
        let texture = c.name16()?;
        let texture2 = c.name16()?;
        let flags = if c.rest() >= 4 { Some(c.u32()?) } else { None };
        Ok(Self { name, texture, texture2, flags })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
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

    fn name16(s: &str) -> [u8; 16] {
        let mut n = [0u8; 16];
        n[..s.len()].copy_from_slice(s.as_bytes());
        n
    }

    #[test]
    fn scene_resolves_children_with_prefix() {
        // v2 mesh: 3 verts, 1 face, 1 material.
        let mut mesh = vec![0xFE, 0xFF];
        for v in [0f32, 0., 0.] { mesh.extend_from_slice(&v.to_le_bytes()); }
        for v in [3u32, 0, 0] { mesh.extend_from_slice(&v.to_le_bytes()); }
        mesh.extend_from_slice(&[0; 16]);
        mesh.extend_from_slice(&1u16.to_le_bytes());
        mesh.extend_from_slice(&1u32.to_le_bytes());
        mesh.extend_from_slice(&name16("mat"));
        for i in 0..3 {
            for v in [i as f32, 0., 0., 0., 1., 0., 0., 0.] { mesh.extend_from_slice(&f32::to_le_bytes(v)); }
        }
        for w in [0u16, 1, 2, 5] { mesh.extend_from_slice(&w.to_le_bytes()); }
        mesh.push(3);
        // v2 node with one child "box".
        let mut node = vec![0xFE, 0xFF];
        for v in [0f32, 0., 0.] { node.extend_from_slice(&v.to_le_bytes()); }
        node.extend_from_slice(&0u32.to_le_bytes());
        node.extend_from_slice(&0u32.to_le_bytes());
        node.extend_from_slice(&[0; 16]);
        node.extend_from_slice(&1u16.to_le_bytes());
        let mut rec = [0u8; 0x44];
        rec[0x10..0x14].copy_from_slice(&1f32.to_le_bytes());
        rec[0x34..0x37].copy_from_slice(b"box");
        node.extend_from_slice(&rec);
        let mut mat = Vec::new();
        mat.extend_from_slice(&name16("mat"));
        mat.extend_from_slice(&name16("tex.bmp"));
        mat.extend_from_slice(&[0; 16]);
        mat.extend_from_slice(&0u32.to_le_bytes());

        let bytes = archive(&[("obj-mat.xvm", mat), ("obj.xvb", node), ("obj-box.xvo", mesh)]);
        let a = Archive::parse(&bytes).unwrap();
        assert_eq!(a.root(None).unwrap().name, "obj.xvb");
        let scene = a.scene(Some("obj")).unwrap().unwrap();
        let (child, content) = &scene.children[0];
        assert_eq!(child.name, "box");
        assert_eq!(child.matrix[0], 1.0);
        let ChildContent::Mesh(m) = content else { panic!("expected mesh") };
        assert_eq!(m.vertices.len(), 3);
        assert_eq!(m.faces[0], Face { idx: [0, 1, 2], material: 0 }); // 5 clamped to nmat-1
        assert_eq!(m.face_flags, [3]);
        let mat = Material::parse(a.get("OBJ-MAT.XVM").unwrap().data).unwrap();
        assert_eq!(mat.texture, "tex.bmp");
    }
}
