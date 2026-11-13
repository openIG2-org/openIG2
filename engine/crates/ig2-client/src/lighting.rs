//! The engine's per-vertex lighting (re/screens/research.md §2.1, verified on 6,096 vertices of ig2.exe's research
//! screen): `fenyekszamolasa` (ELF 0x606530, exe 0x52f190) colours every vertex of a lit object (property 1) on the
//! CPU from the lights its scene node inherits (`Kocka` ELF 0x5f6a40: the parents' lights, then the node's own, at
//! most 15). An ambient light (`CreateLightAmbient` ELF 0x4575c0, type 0) adds its colour; a parallel one
//! (`CreateLightParalell` 0x4577a0, type 1; `SetLightPos` 0x457c50 stores the normalised position, `Kocka` turns it
//! by the camera's rotation) adds `colour · shade[i] >> 8` per channel with `i = float2int(|n' − R⁻¹·L|²·2047)`:
//! `n'` the vertex's file normal with its z negated, `R` the node's rotation chain (`spsorttype+0x24`, transposed
//! by `InvertMatrix`), `shade` the table `SetLights(2, 0.15)` builds (ELF 0x602ea0: `sqrt(1 − i/2896)·255` up to
//! 2896, then 0). Sums saturate at 255 per channel (`addcolor` ELF 0x450c70). `CreateLightOmni` (type 2) lights
//! nothing here. Colour words are bytes `[r, g, b]` = `0xBBGGRR` [obs: the research screen's 0xffba00 tints the left
//! wheel cyan]; the shader multiplies the texture by them (`gl_FragColor = texture·var_color`, no doubling).

use std::cell::RefCell;
use std::sync::OnceLock;

use crate::scene::{self, M3, V3};

/// `SetLights` (ELF 0x602ea0) with `Init3DVariables`' arguments: the shade table, in the exe's f32 steps.
fn shade_table() -> &'static [u16; 8192] {
    static T: OnceLock<[u16; 8192]> = OnceLock::new();
    T.get_or_init(|| {
        let n = ((2.0f64).sqrt() / 4.0 * 8192.0) as i32;
        std::array::from_fn(|i| if (i as i32) < n { ((1.0 - i as f32 / n as f32).sqrt() * 255.0) as u16 } else { 0 })
    })
}

/// `float2int` (exe 0x542d80): `x + 2²³` in f32, the mantissa (round to nearest, ties to even).
pub(crate) fn float2int(x: f32) -> i32 {
    ((x + 8_388_608.0).to_bits() & 0x7f_ffff) as i32
}

/// The colour word's bytes.
fn bytes(c: u32) -> [u8; 3] {
    [c as u8, (c >> 8) as u8, (c >> 16) as u8]
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Light {
    Ambient([u8; 3]),
    /// The light vector in camera space (unit).
    Parallel(V3, [u8; 3]),
}

/// The lights a node sees, in `Kocka`'s order.
#[derive(Debug, Clone, Default)]
pub struct Lights(Vec<Light>);

impl Lights {
    /// `CreateLightAmbient(c)`.
    pub fn ambient(mut self, c: u32) -> Self {
        self.0.push(Light::Ambient(bytes(c)));
        self
    }

    /// `CreateLightParalell(c, pos)` / `SetLightPos(pos)` with the camera's rotation identity (the 3D-framed
    /// screens: `kamerapont` (0, 0, 600), angles 0).
    pub fn parallel(mut self, c: u32, pos: V3) -> Self {
        self.0.push(Light::Parallel(scene::normalise(pos), bytes(c)));
        self
    }

    /// The vertex colour bytes for the lighting normal `w` (camera space, [`normal`]).
    pub fn shade(&self, w: V3) -> [u8; 3] {
        self.accumulate(|d| [w[0] - d[0], w[1] - d[1], w[2] - d[2]])
    }

    /// The colours as `fenyekszamolasa` sums them, `e(L)` = the vertex normal minus the light vector `L` (in
    /// whichever space both are).
    fn accumulate(&self, e: impl Fn(V3) -> V3) -> [u8; 3] {
        let t = shade_table();
        let mut c = [0u8; 3];
        for l in &self.0 {
            let add = match *l {
                Light::Ambient(a) => a,
                Light::Parallel(d, a) => {
                    let e = e(d);
                    let i = float2int((e[0] * e[0] + e[1] * e[1] + e[2] * e[2]) * 2047.0);
                    let s = t.get(i.max(0) as usize).copied().unwrap_or(0) as u32;
                    a.map(|x| ((x as u32 * s) >> 8) as u8)
                }
            };
            for k in 0..3 {
                c[k] = c[k].saturating_add(add[k]);
            }
        }
        c
    }

    /// [`Lights::shade`] as 0..1 factors.
    pub fn rgb(&self, w: V3) -> [f32; 3] {
        self.shade(w).map(|x| x as f32 / 255.0)
    }
}

/// A node's lights as the mesh shader reads them (ADR 0088 §9): the ambient lights' summed colour bytes, then up to
/// four parallel lights, each its vector and its colour bytes.
pub type Set = [[f32; 4]; 9];

/// How many light sets one frame's scenes can name.
pub const SETS: usize = 16;

thread_local! {
    /// The light sets the frame's vertices name so far ([`Lights::vertex`]).
    static FRAME: RefCell<Vec<Set>> = const { RefCell::new(Vec::new()) };
}

/// The frame's light sets, for the scene made from the vertices baked so far ([`crate::render3d::Scene3d::lights`]).
pub fn sets() -> Vec<Set> {
    FRAME.with(|f| f.borrow().clone())
}

/// A new frame: forget the last one's light sets.
pub fn new_frame() {
    FRAME.with(|f| f.borrow_mut().clear());
}

impl Lights {
    fn set(&self) -> Option<Set> {
        let (mut s, mut ambient, mut k) = ([[0.0; 4]; 9], [0u8; 3], 0);
        for l in &self.0 {
            match *l {
                Light::Ambient(a) => ambient = std::array::from_fn(|i| ambient[i].saturating_add(a[i])),
                Light::Parallel(d, c) => {
                    if k == 4 {
                        return None;
                    }
                    s[1 + 2 * k] = [d[0], d[1], d[2], 0.0];
                    s[2 + 2 * k] = [c[0] as f32, c[1] as f32, c[2] as f32, 0.0];
                    k += 1;
                }
            }
        }
        s[0] = [ambient[0] as f32, ambient[1] as f32, ambient[2] as f32, 0.0];
        Some(s)
    }

    /// What a vertex with the lighting normal `w` carries: its normal attribute and its light colour. As the engine
    /// lights it (`normal` kept, the colour [`Lights::rgb`]); with "Per-pixel lighting" (ADR 0088 §9) the lighting
    /// normal `16·4^k` long, `k` these lights' place among the frame's sets, and white: the mesh shader then reads
    /// the shade table per fragment. Lights that don't fit a set, a frame with [`SETS`] sets already and a normal
    /// too far from unit length stay lit per vertex.
    pub fn vertex(&self, w: V3, normal: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let len = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
        if crate::render3d::lighting().per_pixel && (0.55..1.8).contains(&len) {
            let slot = self.set().and_then(|s| {
                FRAME.with(|f| {
                    let mut f = f.borrow_mut();
                    f.iter().rposition(|x| *x == s).or_else(|| (f.len() < SETS).then(|| {
                        f.push(s);
                        f.len() - 1
                    }))
                })
            });
            if let Some(k) = slot {
                let scale = 16.0 * 4f32.powi(k as i32);
                return (w.map(|x| x * scale), [1.0; 3]);
            }
        }
        (normal, self.rgb(w))
    }
}

/// The lighting normal in camera space: a part's [`scene::Vert::lnormal`] (the model's rotation chain applied to the
/// file normal with z negated) turned by the placement `m`'s rotation, `m = R·S` with `S` a scale along the model's
/// axes (`AddToScene`, `ScaleObj`; any uniform factor such as the scene's 1024): `m`'s columns normalised.
pub fn normal(m: &M3, lnormal: V3) -> V3 {
    let col = |j: usize| {
        let l = (m[j] * m[j] + m[3 + j] * m[3 + j] + m[6 + j] * m[6 + j]).sqrt();
        if l > 0.0 { 1.0 / l } else { 0.0 }
    };
    let s = [col(0), col(1), col(2)];
    std::array::from_fn(|i| m[3 * i] * s[0] * lnormal[0] + m[3 * i + 1] * s[1] * lnormal[1] + m[3 * i + 2] * s[2] * lnormal[2])
}

/// [`normal`] for a placement given as a point map `at` (model point → camera space) that scales last: `at = T·S·R`
/// with `S` a scale along the camera axes (`AddToScene`'s scale after the screen's own turns, no `RotateObj` on the
/// record): the linear part probed at `p`, its rows' lengths divided out.
pub fn through(at: impl Fn(V3) -> V3, p: V3, lnormal: V3) -> V3 {
    let o = at(p);
    let cols: [V3; 3] = std::array::from_fn(|j| {
        let mut q = p;
        q[j] += 1.0;
        let a = at(q);
        [a[0] - o[0], a[1] - o[1], a[2] - o[2]]
    });
    std::array::from_fn(|i| {
        let row = [cols[0][i], cols[1][i], cols[2][i]];
        let l = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
        if l > 0.0 { (row[0] * lnormal[0] + row[1] * lnormal[1] + row[2] * lnormal[2]) / l } else { 0.0 }
    })
}

/// A part's lighting tangent (`scene::Vert::ltangent`, ADR 0089 §6) turned like its lighting normal by [`normal`]:
/// the side flipped where `m` mirrors.
pub fn tangent(m: &M3, lt: [f32; 4]) -> [f32; 4] {
    let d = normal(m, [lt[0], lt[1], lt[2]]);
    if lt[3] == 0.0 { [0.0; 4] } else { [d[0], d[1], d[2], lt[3] * scene::det(m).signum()] }
}

/// [`tangent`] for a placement given as a point map ([`through`]).
pub fn through_tangent(at: impl Fn(V3) -> V3, p: V3, lt: [f32; 4]) -> [f32; 4] {
    let o = at(p);
    let m: M3 = std::array::from_fn(|k| {
        let (i, j) = (k / 3, k % 3);
        let mut q = p;
        q[j] += 1.0;
        at(q)[i] - o[i]
    });
    tangent(&m, lt)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SetLights`' table as ig2.exe holds it (read through `fenyekszamolasa`'s material, `run_screen.py
    /// --light-probe`): 255 at 0, the sqrt falloff to 2896, zero after.
    #[test]
    fn shade_table_matches_the_exe() {
        let t = shade_table();
        assert_eq!((t[0], t[1], t[1000], t[2000], t[2895], t[2896], t[8191]), (255, 254, 206, 141, 4, 0, 0));
    }

    /// The mesh shader's `set_light` (render3d.rs) in f32, as it reads a [`Set`].
    fn set_light(s: &Set, n: V3) -> [u8; 3] {
        let mut c = [s[0][0], s[0][1], s[0][2]];
        for j in 0..4 {
            let (l, col) = (s[1 + 2 * j], s[2 + 2 * j]);
            let i = ((n[0] - l[0]).powi(2) + (n[1] - l[1]).powi(2) + (n[2] - l[2]).powi(2)) * 2047.0;
            let i = i.round_ties_even();
            let shade = if i < 2896.0 { ((1.0 - i / 2896.0).sqrt() * 255.0).floor() } else { 0.0 };
            for k in 0..3 {
                c[k] += (col[k] * shade / 256.0).floor();
            }
        }
        c.map(|x| x.min(255.0) as u8)
    }

    /// "Per-pixel lighting" (ADR 0088 §9) gives a vertex the colour the engine's per-vertex light gives it: the
    /// spy screen's four lights over a sphere of normals, and lights past what a set holds stay per vertex.
    #[test]
    fn light_set_matches_the_vertex_light() {
        let l = Lights::default().ambient(0x141414).ambient(0xf0f0f0).parallel(0x808080, [0.0, 0.0, -10.0]).parallel(0x7f5d00, [200.0, 0.0, -350.0]).parallel(0x004e7f, [-200.0, 40.0, -350.0]);
        let s = l.set().unwrap();
        assert_eq!(s[0], [255.0, 255.0, 255.0, 0.0]);
        let l = Lights(l.0[1..].to_vec());
        let s = l.set().unwrap();
        for a in 0..40 {
            for b in 0..80 {
                let (t, p) = (a as f32 * 0.0785, b as f32 * 0.0785);
                let n = [t.sin() * p.cos(), t.sin() * p.sin(), t.cos()];
                assert_eq!(set_light(&s, n), l.shade(n), "{n:?}");
            }
        }
        let five = (0..5).fold(Lights::default(), |l, k| l.parallel(0x101010, [k as f32, 1.0, 0.0]));
        assert!(five.set().is_none());
        // Off (the default), a vertex keeps its normal and the engine's colour.
        assert_eq!(l.vertex([0.0, 0.0, 1.0], [0.0; 3]), ([0.0; 3], l.rgb([0.0, 0.0, 1.0])));
    }

    #[test]
    fn float2int_rounds_to_even() {
        assert_eq!((float2int(2.5), float2int(3.5), float2int(1023.4999), float2int(0.0)), (2, 4, 1023, 0));
    }

    /// Every vertex ig2.exe lit on the research screen's first frames (`run_screen.py --kep 4 --res-frame 60
    /// --res-keringeto 0.01 --light-probe --jsonl _local/traces/light-research.jsonl`: per `fenyekszamolasa` call the
    /// node's rotation chain, its lights and each vertex's file normal and colour), recomputed in the object's space
    /// as the exe does it (`n' − Mᵀ·L`, the opacity scaling without property 8). The trace holds model data: it
    /// stays in `_local/traces` (`IG2_TRACE_DIR` elsewhere); the test skips without it.
    #[test]
    fn research_probe_matches_the_exe() {
        let (n, lit) = replay_probe("light-research.jsonl");
        assert!(n == 0 || (n > 6000 && lit > 1000), "{n} vertices, {lit} lit by the parallel lights");
    }

    /// The trade screen's objects (`run_screen.py --kep 8 --script scripts/trade-pods.json --light-probe`,
    /// 2026-10-24: the panels, the card and the first offer's hull turning in): every vertex colour.
    #[test]
    fn trade_probe_matches_the_exe() {
        let (n, lit) = replay_probe("light-trade.jsonl");
        assert!(n == 0 || (n > 800 && lit > 50), "{n} vertices, {lit} lit by the parallel lights");
    }

    /// The colony screen's panel pass (`run_screen.py --kep 3 --planet 42 --probe probes/colony_lights.js`, quick-1.sav,
    /// 2026-10-25): the panel building under `mainmeshid2` (ambient 0x404040, `light5x` 0xffffff), the info box's model
    /// under `mainmeshid5` (`light5` 0xffffff, ambient 0x4b4b4b) and one object of the main view.
    #[test]
    fn colony_panel_probe_matches_the_exe() {
        let (n, lit) = replay_probe("light-colony42.jsonl");
        assert!(n == 0 || (n > 300 && lit > 50), "{n} vertices, {lit} lit by the parallel lights");
    }

    /// The starmap's long info planet and its clouds (`run_screen.py --kep 2 --planet 71 --rop 0xf --light-probe`,
    /// quick-1.sav, 2026-10-25; re/screens/starmap.md §3.3): the root's ambient and parallel light.
    #[test]
    fn starmap_long_info_probe_matches_the_exe() {
        let (n, lit) = replay_probe("light-planetinfo.jsonl");
        assert!(n == 0 || (n > 500 && lit > 100), "{n} vertices, {lit} lit by the parallel lights");
    }

    /// Replays a `--light-probe` trace through [`Lights::accumulate`]: (vertices, those not at the ambient alone).
    fn replay_probe(name: &str) -> (usize, usize) {
        let dir = std::env::var_os("IG2_TRACE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
        let Ok(text) = std::fs::read_to_string(dir.join(name)) else {
            eprintln!("skipped: no {name}");
            return (0, 0);
        };
        let f = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let (mut n, mut lit) = (0, 0);
        for line in text.lines() {
            let r: serde_json::Value = serde_json::from_str(line).unwrap();
            let flags = r["flags"].as_u64().unwrap();
            if flags & 1 == 0 {
                continue;
            }
            let m: M3 = std::array::from_fn(|i| f(&r["m"][i]));
            let mut lights = Lights::default();
            for l in r["lights"].as_array().unwrap() {
                let c = l["color"].as_u64().unwrap() as u32;
                match l["type"].as_u64().unwrap() {
                    0 => lights = lights.ambient(c),
                    1 => lights.0.push(Light::Parallel(std::array::from_fn(|i| f(&l["vec"][i])), bytes(c))),
                    _ => {}
                }
            }
            let op = r["opacity"].as_u64().unwrap() as u32;
            for v in r["verts"].as_array().unwrap() {
                let nv: V3 = std::array::from_fn(|i| f(&v[i]));
                let mut c = lights.accumulate(|l| {
                    let lo: V3 = std::array::from_fn(|i| m[i] * l[0] + m[3 + i] * l[1] + m[6 + i] * l[2]);
                    [nv[0] - lo[0], nv[1] - lo[1], -nv[2] - lo[2]]
                });
                if flags & 8 == 0 {
                    c = c.map(|x| ((x as u32 * op) >> 8) as u8);
                }
                assert_eq!(c, bytes(v[3].as_u64().unwrap() as u32), "{line:.80}");
                n += 1;
                lit += (c != [0x20; 3] && c != [0x14; 3]) as usize;
            }
        }
        (n, lit)
    }
}
