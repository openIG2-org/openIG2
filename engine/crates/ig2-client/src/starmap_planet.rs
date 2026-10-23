//! The starmap's planets as meshes (re/screens/starmap.md §3.3): `Init3dStarmap`'s `PlanetMeshes` `b{k}0.xvb` and the
//! cloud mesh `felhoo_0.xvb` at the last level of their detail chains, placed by `rotate_stars`, turned by
//! `ShowStarmap` and lit per vertex by the starmap's light, with the GOG remaster's `uv_mul` and its procedural
//! `CLOUD` material (`render/ig2_fragment_cloud.shader`) for the long info planet's clouds.

use ig2_formats::Install;

use crate::assets::Image;
use crate::render::{Renderer, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d};
use crate::scene::{normalise, Model, Placement, Vert};

/// `Init3dStarmap`'s ambient light 0x404040 and parallel light 0x808080 (gamma-space levels).
const AMBIENT: f32 = 0x40 as f32 / 255.0;
const SUN: f32 = 0x80 as f32 / 255.0;

/// A model at the last level of its detail chain, its parts in the root's space with their textures.
struct Mesh {
    /// The chain head's half-extents (`AddToScene` scales by them).
    origin: [f32; 3],
    parts: Vec<Part>,
}

struct Part {
    verts: Vec<Vert>,
    tex: TexId,
    /// The texture's `uv_mul` (`ig2_vertex.shader`: `var_uv = sg3d_texcoord0 * uv_mul.xy`).
    uv_mul: [f32; 2],
}

/// Where the camera looks from (spec §2.2): its rows `M` (`CreateMatrix(α, β, π/2)`) and the eye's world point.
#[derive(Debug, Clone, Copy)]
pub struct Eye {
    pub matrix: [[f32; 3]; 3],
    pub at: [f32; 3],
}

/// The planet meshes `b{k}0.xvb` (k = (surface − 1) % 8), the cloud mesh and the cloud material's noise.
pub struct Planets {
    meshes: [Option<Mesh>; 8],
    cloud: Option<Mesh>,
    noise: TexId,
    /// The battle screen's planet `b{k}2.xvb` (re/screens/spacewar.md §1.1 step 7), loaded on demand.
    battle: Vec<Option<Mesh>>,
}

impl Planets {
    pub fn load(r: &mut Renderer, install: &Install) -> Self {
        let meshes = std::array::from_fn(|k| Mesh::load(r, install, &format!("b{k}0.xvb")));
        let cloud = Mesh::load(r, install, "felhoo_0.xvb");
        let noise = r.add_texture_linear(&Image { width: PLASMA, height: PLASMA, rgba: plasma().iter().flat_map(|&v| [v, v, v, 255]).collect() });
        Self { meshes, cloud, noise, battle: Vec::new() }
    }

    /// Load the battle screen's planet meshes `b{k}2.xvb`.
    pub fn load_battle(&mut self, r: &mut Renderer, install: &Install) {
        self.battle = (0..8).map(|k| Mesh::load(r, install, &format!("b{k}2.xvb"))).collect();
    }

    /// The battle planet (`initgfx`: `AddToScene(…, R·1.16)`, `RotateObj(rot)`, `MoveObj(at)`), `scale` and `at` in
    /// the starmap's world, lit from the world direction `light` (towards the light).
    #[allow(clippy::too_many_arguments)]
    pub fn add_battle_planet(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, surface: u8, scale: f32, rot: (f32, f32, f32), at: [f32; 3], eye: &Eye, light: [f32; 3]) {
        if let Some(Some(m)) = self.battle.get((surface.max(1) as usize - 1) % 8) {
            m.add(scene, draws, scale, rot, at, eye, None, normalise(light));
        }
    }

    pub fn ready(&self, surface: u8) -> bool {
        self.meshes[(surface.max(1) as usize - 1) % 8].is_some()
    }

    /// Planet `surface` of size `bolygomeret` at the world point `at` (h, x, y), turned by `RotateObj(0, 0, spin)`
    /// (`ShowStarmap`: `PlanetRotate · SpinSpeed/100 · 6`), into `draws` (opaque).
    #[allow(clippy::too_many_arguments)]
    pub fn add_planet(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, surface: u8, size: u8, spin: f32, at: [f32; 3], eye: &Eye) {
        if let Some(m) = &self.meshes[(surface.max(1) as usize - 1) % 8] {
            m.add(scene, draws, scale(size) / 8.0, (0.0, 0.0, spin), at, eye, None, eye.matrix[2]);
        }
    }

    /// The planet's clouds (`HideClouds` shows them on the long info's planet only): `felhoo_0.xvb` at
    /// `(scale + 0.0001)/8`, turned by `RotateObj(−PlanetRotate · spin · 2, 5, 0)`; the second cloud mesh is added at
    /// scale 0 and never shows. Drawn with the remaster's `CLOUD` material: the mesh's texture `ethcldbp.bmp` is only
    /// its key (`command_glLoadTexture` ELF 0x426e30), the clouds come from the noise (`cloud_shape`).
    #[allow(clippy::too_many_arguments)]
    pub fn add_clouds(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, size: u8, turn: f32, at: [f32; 3], eye: &Eye) {
        if let Some(m) = &self.cloud {
            m.add(scene, draws, (scale(size) + 0.0001) / 8.0, (turn, 5.0, 0.0), at, eye, Some(self.noise), eye.matrix[2]);
        }
    }
}

/// `Init3dStarmap`: `bolygomeret/200000 + 0.002`.
fn scale(size: u8) -> f32 {
    size as f32 / 200000.0 + 0.002
}

impl Mesh {
    /// `LoadBox(name)` at the end of its level-of-detail chain (`lod_archive`: `b00` → `b01` → `b02` → `b03`, a
    /// tetrahedron, 48, 168 and 528 faces). The renderer (`Kocka` ELF 0x5f6a40) steps down the chain while
    /// `valtashatar·30^level < size·lod²` (`lod` 5 in every level, `size` the object's projected extent): the long
    /// info's planet, a screen wide, is always at the last level. The client draws that level everywhere; the
    /// coarser ones would only show on planets a few pixels across.
    fn load(r: &mut Renderer, install: &Install, name: &str) -> Option<Self> {
        let load = |n: &str| Model::load(install, "vector/starmap/", n, "textures/starmap/");
        let root = load(name).map_err(|e| eprintln!("ig2-client: {e} (no starmap mesh)")).ok()?;
        let mut stem = name.trim_end_matches(".xvb").to_string();
        for _ in 0..8 {
            let next = install.read(&format!("dataraw/vector/starmap/{stem}.xv")).ok().and_then(|b| {
                let ar = ig2_formats::xv::Archive::parse(&b).ok()?;
                let h = ig2_formats::xv::NodeHeader::parse(ar.root(Some(&stem))?.data).ok()?;
                (!h.lod_archive.is_empty()).then(|| h.lod_archive.trim_end_matches(".xvb").to_string())
            });
            match next {
                Some(n) => stem = n,
                None => break,
            }
        }
        let parts = load(&format!("{stem}.xvb"))
            .ok()?
            .parts
            .into_iter()
            .filter_map(|p| {
                let path = p.texture.as_deref()?;
                let t = Image::load(install, path).ok()?;
                Some(Part { uv_mul: Image::uv_mul(install, path), verts: p.verts, tex: r.add_texture(&t) })
            })
            .collect();
        Some(Self { origin: root.origin, parts })
    }

    /// Its triangles at `AddToScene` scale `scale`, turned by `RotateObj(rot)` and moved to `at`, the faces turned
    /// away from the eye left out (the 1999 engine sorts and culls them itself), lit per vertex in gamma space:
    /// ambient plus the parallel light along `(fxn, fyn, fzn)` = `ShowStarmap`'s `StmapLightX/Y/Z` = (0, 0, −10),
    /// which lights the faces turned towards the eye (camera space). The normals are the directions from the centre:
    /// the meshes' own are mirrored in x and y, and the captures match outward ones [obs].
    /// `cloud`: the noise texture of the `CLOUD` material, drawn over the planet in gamma space.
    #[allow(clippy::too_many_arguments)]
    fn add(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, scale: f32, rot: (f32, f32, f32), at: [f32; 3], eye: &Eye, cloud: Option<TexId>, light: [f32; 3]) {
        let xf = Placement::add_to_scene(self.origin, scale).world(rot, at);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let towards_eye = light;
        for part in &self.parts {
            let start = scene.dynamic.len() as u32;
            for tri in part.verts.chunks_exact(3) {
                let pos: [[f32; 3]; 3] = std::array::from_fn(|k| xf.point(tri[k].pos));
                let centre: [f32; 3] = std::array::from_fn(|j| (pos[0][j] + pos[1][j] + pos[2][j]) / 3.0);
                let out: [f32; 3] = std::array::from_fn(|j| centre[j] - at[j]);
                let view: [f32; 3] = std::array::from_fn(|j| centre[j] - eye.at[j]);
                if dot(out, view) >= 0.0 {
                    continue;
                }
                for k in 0..3 {
                    let n = normalise(std::array::from_fn(|j| pos[k][j] - at[j]));
                    // Gamma-space like the remaster (re/screens/render.md §2).
                    let lit = (AMBIENT + SUN * dot(n, towards_eye).max(0.0)).min(1.0);
                    let uv = [tri[k].uv[0] * part.uv_mul[0], tri[k].uv[1] * part.uv_mul[1]];
                    scene.dynamic.push(MeshVertex { pos: pos[k], normal: [0.0; 3], uv, color: [lit, lit, lit, 1.0] });
                }
            }
            let range = start..scene.dynamic.len() as u32;
            draws.push(MeshDraw { mesh: None, range, tex: cloud.unwrap_or(part.tex), glass: false, cloud: cloud.is_some(), shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: false });
        }
    }
}

/// `sg3d::create_plasma(7, 5, …)` (ELF 0x87dbb0, made by `ig2screen::initialise`): the cloud material's
/// `SG3D_TEXTURE_NOISE`, 128 × 128.
const PLASMA: u32 = 128;

/// sg3d's own `srand` (ELF 0x7d2a40, really a generator): Park–Miller's minimal standard `seed·16807 mod 2³¹ − 1`
/// in Carta's form, `rand_seed` (ELF 0x119dfb0) starting at 1.
fn park_miller(seed: &mut u32) -> u32 {
    let hi = (*seed >> 16).wrapping_mul(16807);
    let mut lo = (hi >> 15).wrapping_add((hi & 0x7fff) << 16).wrapping_add((*seed & 0xffff).wrapping_mul(16807));
    if lo > 0x7fff_ffff {
        lo = lo.wrapping_add(0x8000_0001);
    }
    *seed = lo;
    lo
}

/// `create_plasma(7, 5)`: `create_noise_optim(5)` fills 32 × 32 bytes with the generator's low bytes (row by row);
/// each of the 128 × 128 texels sums `7 − 5 + 1` = 3 octaves `(2·sample − 1)·amp` (amp 0.5, 0.25, 0.125) of
/// `sample_optim` (ELF 0x870920, level 0, smoothstep weights, wrapping) at `((x + ½)/128, (y + ½)/128)·2^octave`,
/// stored as `(int)(sum·127.5 + 127.5)`. All f32 like the original.
fn plasma() -> Vec<u8> {
    const N: usize = 32;
    let mut seed = 1;
    let noise: Vec<u8> = (0..N * N).map(|_| park_miller(&mut seed) as u8).collect();
    let at = |x: usize, y: usize| noise[(y & (N - 1)) * N + (x & (N - 1))] as f32;
    let lerp = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    // `texture_sampling_params_t` (ELF 0x874150) + `sample_optim`'s 8-bit branch.
    let sample = |u: f32, v: f32| {
        let half = 0.5 / N as f32;
        let (pu, pv) = (u - half, v - half);
        let (fu, fv) = ((pu - pu.floor()) * N as f32, (pv - pv.floor()) * N as f32);
        let (cu, cv) = (fu.floor(), fv.floor());
        let (x, y) = (cu as i64 as usize, cv as i64 as usize);
        let (wx, wy) = (smooth(fu - cu), smooth(fv - cv));
        let a = lerp(at(x, y), at(x + 1, y), wx);
        let b = lerp(at(x, y + 1), at(x + 1, y + 1), wx);
        lerp(a, b, wy) / 255.0
    };
    let size = PLASMA as usize;
    let mut out = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            let (mut u, mut v) = ((x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32);
            let (mut sum, mut amp) = (0.0f32, 0.5f32);
            for _ in 0..3 {
                let s = sample(u, v);
                sum += (s + s - 1.0) * amp;
                (u, v) = (u * 2.0, v * 2.0);
                amp *= 0.5;
            }
            out.push((sum * 127.5 + 127.5) as i32 as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_is_park_miller() {
        let mut s = 1;
        assert_eq!([park_miller(&mut s), park_miller(&mut s), park_miller(&mut s)], [16807, 282475249, 1622650073]);
        let mut s = 1;
        for _ in 0..10000 {
            park_miller(&mut s);
        }
        assert_eq!(s, 1043618065);
    }

    #[test]
    fn the_plasma_wraps_and_spans_the_bytes() {
        let p = plasma();
        assert_eq!(p.len(), 128 * 128);
        let (lo, hi) = (p.iter().min().unwrap(), p.iter().max().unwrap());
        assert!(*lo < 64 && *hi > 192, "{lo}..{hi}");
        // Every octave's period divides 128 texels: across the wrap the texels join like neighbours inside.
        let step = |a: usize, b: usize| (0..128).map(|y| (p[y * 128 + a] as i32 - p[y * 128 + b] as i32).abs()).max().unwrap();
        let inside = (0..127).map(|x| step(x, x + 1)).max().unwrap();
        assert!(step(127, 0) <= inside, "{} > {inside}", step(127, 0));
    }
}
