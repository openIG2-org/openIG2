//! The starmap's planets as meshes (re/screens/starmap.md §3.3): `Init3dStarmap`'s `PlanetMeshes` `b{k}0.xvb` and the
//! cloud mesh `felhoo_0.xvb` at the last level of their detail chains, placed by `rotate_stars`, turned by
//! `ShowStarmap` and lit per vertex by the scene root's lights (`engine_light`), with the GOG remaster's `uv_mul` and its procedural
//! `CLOUD` material (`render/ig2_fragment_cloud.shader`) for the long info planet's clouds.

use ig2_formats::Install;

use crate::assets::Image;
use crate::render::{Renderer, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d};
use crate::scene::{normalise, Model, Placement, Vert};


/// `Init3dStarmap` (ELF 0x6db470, exe 0x59c850): the parallel light's vector on the scene's root `mainmeshid`, in the
/// world (`MoveObj`) axes. Every planet and cloud inherits it with the ambient light (`Kocka`); `ShowStarmap`'s
/// `StmapLightX/Y/Z` → `fxn/fyn/fzn` are read only by `setlightnormal`, which nothing calls.
const LIGHT: [f32; 3] = [10.0, 10.0, -10.0];

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
    /// Its cloud shell `vector/spacewar/bolygo3.xvb` (`ETHCLDBP.bmp`, the `CLOUD` material), added twice.
    shell: Option<Mesh>,
}

impl Planets {
    pub fn load(r: &mut Renderer, install: &Install) -> Self {
        let meshes = std::array::from_fn(|k| Mesh::load(r, install, &format!("b{k}0.xvb")));
        let cloud = Mesh::load(r, install, "felhoo_0.xvb");
        let noise = r.add_texture_linear(&Image { width: PLASMA, height: PLASMA, rgba: plasma().iter().flat_map(|&v| [v, v, v, 255]).collect(), mips: Vec::new() });
        Self { meshes, cloud, noise, battle: Vec::new(), shell: None }
    }

    /// Load the battle screen's planet meshes `b{k}2.xvb`.
    pub fn load_battle(&mut self, r: &mut Renderer, install: &Install) {
        self.battle = (0..8).map(|k| Mesh::load(r, install, &format!("b{k}2.xvb"))).collect();
        self.shell = Mesh::load_in(r, install, "vector/spacewar/", "textures/spacewar/", "bolygo3.xvb");
    }

    /// One of the battle planet's two cloud shells (`initgfx` exe 0x575425..0x575572: `AddToScene(bolygo3, R·1.02 |
    /// R·1.04)`, `RotateObj`, `MoveObj(P)`, `SetObjTransparency(0.7)`), lit like the planet. The remaster draws its
    /// `CLOUD` material with the procedural clouds (the transparency's alpha unused), and the engine keeps its depth
    /// write: the shells, drawn after the planet and before the shield rings, hide the rings under them [obs: the
    /// shield capture's probe, re/screens/spacewar.md §1.1]. Here a depth-only pass among the translucent draws,
    /// then the clouds.
    #[allow(clippy::too_many_arguments)]
    pub fn add_battle_shell(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, scale: f32, rot: (f32, f32, f32), at: [f32; 3], eye: &Eye) {
        if let Some(m) = &self.shell {
            m.add_depth(scene, draws, scale, rot, at, eye);
            m.add(scene, draws, scale, rot, at, eye, Some(self.noise), &engine_light(rot));
        }
    }

    /// The battle planet (`initgfx`: `AddToScene(…, R·1.16)`, `RotateObj(rot)`, `MoveObj(at)`), `scale` and `at` in
    /// the starmap's world. Lit as the starmap's planets: the scene root's parallel light against the
    /// object-space normals, whatever the camera [obs: the shield and fx8 captures, 2026-10-25: view ~2.0 → ~1.4 and
    /// 3.7..4.8 → 2.6..3.6 against the old camera-turned light, which went dark as the end zoom-out turned the camera;
    /// re/screens/spacewar.md §1.0].
    #[allow(clippy::too_many_arguments)]
    pub fn add_battle_planet(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, surface: u8, scale: f32, rot: (f32, f32, f32), at: [f32; 3], eye: &Eye) {
        if let Some(Some(m)) = self.battle.get((surface.max(1) as usize - 1) % 8) {
            m.add(scene, draws, scale, rot, at, eye, None, &engine_light(rot));
        }
    }

    pub fn ready(&self, surface: u8) -> bool {
        self.meshes[(surface.max(1) as usize - 1) % 8].is_some()
    }

    /// Planet `surface` of size `bolygomeret` at the world point `at` (h, x, y), turned by `RotateObj(0, 0, spin)`
    /// (`ShowStarmap`: `PlanetRotate · SpinSpeed/100 · 6`), into `draws` (opaque).
    #[allow(clippy::too_many_arguments)]
    /// `opacity`: the alpha below 1 makes it translucent, still writing depth (the battle's zoom: `starmaphalvanyit`).
    #[allow(clippy::too_many_arguments)]
    pub fn add_planet(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, surface: u8, size: u8, spin: f32, at: [f32; 3], eye: &Eye, opacity: f32) {
        if let Some(m) = &self.meshes[(surface.max(1) as usize - 1) % 8] {
            let rot = (0.0, 0.0, spin);
            m.add_with(scene, draws, scale(size) / 8.0, rot, at, eye, None, &engine_light(rot), opacity);
        }
    }

    /// The planet's clouds (`HideClouds` shows them on the long info's planet only): `felhoo_0.xvb` at
    /// `(scale + 0.0001)/8`, turned by `RotateObj(−PlanetRotate · spin · 2, 5, 0)`; the second cloud mesh is added at
    /// scale 0 and never shows. Drawn with the remaster's `CLOUD` material: the mesh's texture `ethcldbp.bmp` is only
    /// its key (`command_glLoadTexture` ELF 0x426e30), the clouds come from the noise (`cloud_shape`).
    #[allow(clippy::too_many_arguments)]
    pub fn add_clouds(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, size: u8, turn: f32, at: [f32; 3], eye: &Eye) {
        if let Some(m) = &self.cloud {
            let rot = (turn, 5.0, 0.0);
            m.add(scene, draws, (scale(size) + 0.0001) / 8.0, rot, at, eye, Some(self.noise), &engine_light(rot));
        }
    }
}

/// A starmap planet's or cloud's vertex grey (`fenyekszamolasa` ELF 0x606530, exe 0x52f190; [`crate::lighting`]): the
/// root's ambient 0x404040 and parallel 0x808080 light, the latter in the object's space `Rᵀ·L` (`R` its `RotateObj`;
/// the camera's rotation cancels: `Kocka` turns `L` by it, `fenyekszamolasa` back by the node's chain), against the
/// file normal with z negated (`Vert::lnormal`; the meshes' normals are mirrored in x and y and stay so). Both the
/// planet (flags 0xf) and the cloud (0xb) have property 8: the bytes aren't scaled by the opacity, which goes in the
/// alpha (the remaster's `CLOUD` shader ignores it).
fn engine_light(rot: (f32, f32, f32)) -> impl Fn([f32; 3], &Vert) -> f32 {
    let lights = engine_lights(crate::scene::create_matrix(rot.0, rot.1, rot.2));
    move |_, v: &Vert| engine_grey(&lights, v.lnormal) as f32 / 255.0
}

/// The root's lights as an object turned by `r` (`RotateObj`) sees them: the parallel one at `Rᵀ·L`.
fn engine_lights(r: crate::scene::M3) -> crate::lighting::Lights {
    let l = normalise(LIGHT);
    let lo: [f32; 3] = std::array::from_fn(|i| r[i] * l[0] + r[3 + i] * l[1] + r[6 + i] * l[2]);
    crate::lighting::Lights::default().ambient(0x404040).parallel(0x808080, lo)
}

/// The vertex byte for the lighting normal `n` (the file normal, z negated).
fn engine_grey(lights: &crate::lighting::Lights, n: [f32; 3]) -> u32 {
    lights.shade(n)[0] as u32
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
        Self::load_in(r, install, "vector/starmap/", "textures/starmap/", name)
    }

    /// [`Mesh::load`] from another screen's directories.
    fn load_in(r: &mut Renderer, install: &Install, vector_dir: &str, texture_dir: &str, name: &str) -> Option<Self> {
        let load = |n: &str| Model::load(install, vector_dir, n, texture_dir);
        let root = load(name).map_err(|e| eprintln!("ig2-client: {e} (no starmap mesh)")).ok()?;
        let stem = crate::scene::lod_chain(install, vector_dir, name).pop()?;
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
    /// away from the eye left out (the 1999 engine sorts and culls them itself), each vertex's grey from `lit` (the
    /// direction from the centre and the vertex: [`engine_light`] on the starmap, the battle planet's own light).
    /// `cloud`: the noise texture of the `CLOUD` material, drawn over the planet in gamma space.
    #[allow(clippy::too_many_arguments)]
    fn add(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, scale: f32, rot: (f32, f32, f32), at: [f32; 3], eye: &Eye, cloud: Option<TexId>, lit: &dyn Fn([f32; 3], &Vert) -> f32) {
        self.add_with(scene, draws, scale, rot, at, eye, cloud, lit, 1.0);
    }

    /// Its front faces' depth only (no colour), among the translucent draws in push order.
    fn add_depth(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, scale: f32, rot: (f32, f32, f32), at: [f32; 3], eye: &Eye) {
        self.add_with(scene, draws, scale, rot, at, eye, None, &|_, _| 0.0, 0.0);
    }

    /// `alpha` below 1: blended at that alpha, the depth still written (the engine's transparency keeps the depth
    /// write, re/screens/render.md §2.3); 0 leaves only the depth.
    #[allow(clippy::too_many_arguments)]
    fn add_with(&self, scene: &mut Scene3d, draws: &mut Vec<MeshDraw>, scale: f32, rot: (f32, f32, f32), at: [f32; 3], eye: &Eye, cloud: Option<TexId>, lit: &dyn Fn([f32; 3], &Vert) -> f32, alpha: f32) {
        let glass = alpha < 1.0;
        let xf = Placement::add_to_scene(self.origin, scale).world(rot, at);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
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
                    let lit = lit(n, &tri[k]);
                    let uv = [tri[k].uv[0] * part.uv_mul[0], tri[k].uv[1] * part.uv_mul[1]];
                    scene.dynamic.push(MeshVertex { pos: pos[k], normal: [0.0; 3], uv, color: [lit, lit, lit, alpha] });
                }
            }
            let range = start..scene.dynamic.len() as u32;
            draws.push(MeshDraw { mesh: None, range, tex: cloud.unwrap_or(part.tex), glass, cloud: cloud.is_some(), shadow: crate::render3d::Shadow::None, video: false, additive: false, depth_write: glass });
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

    /// The battle planet's outer cloud shell (`bolygo3` at R·1.04, R = 1, about P; re/screens/spacewar.md §1.1) as the
    /// client places it, in battle units: the shield capture's young rings (537..540, 1029.2..1036.9 units from P,
    /// hidden in the exe at t3200) lie under it, the frame-30 ones (515 at 1094.1, 513 from 1099.4, shown) above it;
    /// the exe's shell is ~1.5 % smaller (its vertex 1028.4 from the inner node at scale 1041.45, the partial shell's
    /// box re-measure not modelled), 526 at 1065.4 lies between the two [obs:
    /// `run_spwc.py --records` kirak/static draws, `_local/traces/shield5`: the shells' node scales ~1041 and
    /// ~1061.5, `t` 0.7; the ring depths 0.93314..0.93383 behind the shell's 0.93411].
    #[test]
    fn the_outer_cloud_shell_covers_the_young_shield_rings() {
        let Some(inst) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let inst = inst.expect("valid install");
        let m = Model::load(&inst, "vector/spacewar/", "bolygo3", "textures/spacewar/").unwrap();
        let radius = |k: f64| {
            let xf = Placement::add_to_scene(m.origin, (1.0f64 * k) as f32).world((0.0, std::f32::consts::FRAC_PI_2, 0.0), [0.0; 3]);
            m.positions().map(|v| normalise_len(xf.point(v))).fold((f32::MAX, 0.0f32), |(lo, hi), r| (lo.min(r), hi.max(r)))
        };
        let (outer, inner) = (radius(1.04), radius(1.02));
        assert!(outer.0 > 1036.9 && outer.1 < 1094.1, "{outer:?}");
        assert!(inner.1 < outer.0, "{inner:?} {outer:?}");
    }

    fn normalise_len(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    /// The long info's planet and its clouds as ig2.exe lit them (`run_screen.py --kep 2 --planet 71 --rop 0xf
    /// --light-probe --jsonl _local/traces/light-planetinfo.jsonl`, quick-1.sav, 2026-10-25): per `fenyekszamolasa`
    /// call the node's chain `M` (camera · `RotateObj`), the camera-space light and each vertex's file normal and
    /// colour. `Mᵀ·L` is the root's (10, 10, −10) turned back by the object's own rotation alone (the planet's
    /// `(0, 0, θ)`, the clouds' `(a, 5, 0)`, solved here), and [`engine_grey`] gives every vertex byte within 1 (the
    /// exe turns the light by the camera and back, an ulp off ours; exact with the exe's own `Mᵀ·L`:
    /// `lighting::tests::starmap_long_info_probe_matches_the_exe`).
    #[test]
    fn long_info_planet_and_clouds_light_as_the_exe() {
        let dir = std::env::var_os("IG2_TRACE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
        let Ok(text) = std::fs::read_to_string(dir.join("light-planetinfo.jsonl")) else {
            eprintln!("skipped: no light-planetinfo.jsonl");
            return;
        };
        let f = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let (mut objects, mut verts, mut exact) = ([0; 2], 0, 0);
        for line in text.lines() {
            let r: serde_json::Value = serde_json::from_str(line).unwrap();
            let flags = r["flags"].as_u64().unwrap();
            let Some(par) = r["lights"].as_array().unwrap().iter().find(|l| l["type"].as_u64() == Some(1)) else { continue };
            // The planet (flags 0xf, opacity 255) and `CloudId` (0xb); `CloudId2` (0xf at opacity 128, scale 0) never shows.
            if !(flags == 15 && r["opacity"].as_u64() == Some(255) || flags == 11) {
                continue;
            }
            let cloud = flags == 11;
            let m: crate::scene::M3 = std::array::from_fn(|i| f(&r["m"][i]));
            let d: [f32; 3] = std::array::from_fn(|i| f(&par["vec"][i]));
            let lo: [f32; 3] = std::array::from_fn(|i| m[i] * d[0] + m[3 + i] * d[1] + m[6 + i] * d[2]);
            let rot = |t: f32| if cloud { crate::scene::create_matrix(t, 5.0, 0.0) } else { crate::scene::create_matrix(0.0, 0.0, t) };
            let miss = |t: f32| {
                let l = normalise(LIGHT);
                let r = rot(t);
                (0..3).map(|i| (r[i] * l[0] + r[3 + i] * l[1] + r[6 + i] * l[2] - lo[i]).powi(2)).sum::<f32>()
            };
            // The angle: the best of a 1/1000 grid, then narrowed.
            let mut t = (0..6284).map(|k| k as f32 / 1000.0).min_by(|a, b| miss(*a).total_cmp(&miss(*b))).unwrap();
            let mut step = 0.0005;
            while step > 1e-7 {
                t = [t - step, t, t + step].into_iter().min_by(|a, b| miss(*a).total_cmp(&miss(*b))).unwrap();
                step *= 0.5;
            }
            assert!(miss(t) < 1e-10, "{line:.80}: the light isn't the root's turned back by the object's rotation ({})", miss(t));
            let lights = engine_lights(rot(t));
            for v in r["verts"].as_array().unwrap() {
                let n: [f32; 3] = [f(&v[0]), f(&v[1]), -f(&v[2])];
                let (c, want) = (engine_grey(&lights, n) as i32, (v[3].as_u64().unwrap() & 0xff) as i32);
                assert!((c - want).abs() <= 1, "{line:.80}: {c} vs {want}");
                verts += 1;
                exact += (c == want) as usize;
            }
            objects[cloud as usize] += 1;
        }
        assert!(objects[0] > 0 && objects[1] > 0 && verts > 500 && exact * 100 > verts * 98, "{objects:?} objects, {verts} vertices, {exact} exact");
    }

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
