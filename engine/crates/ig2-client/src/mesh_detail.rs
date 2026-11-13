//! The models' detail a mod switches on (ADR 0089 §6): Loop subdivision of a part's triangles with its sharp edges
//! kept hard, and the tangents a pack's normal map is read with. Without a mod's `[models]` nothing here runs on a
//! mesh (the tangents are carried, the shader reads them only beside a normal map).

use std::collections::HashMap;

use crate::scene::{cross, inverse, mul, normalise, Vert, M3, V3};

/// Faces meeting at more than this many degrees between their normals keep their edge hard (outlines, panel lines).
const CREASE_DEG: f32 = 40.0;

/// The file normal's z negated (`scene::Vert::lnormal`).
const FLIP_Z: M3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scl(a: V3, s: f32) -> V3 {
    a.map(|x| x * s)
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// `v` without its part along the unit `n`, normalised; zero when nothing is left.
fn across(v: V3, n: V3) -> V3 {
    let t = sub(v, scl(n, dot(n, v)));
    let l = len(t);
    if l > 1e-12 { scl(t, 1.0 / l) } else { [0.0; 3] }
}

/// A position's welding key: equal points one vertex (−0 is 0).
fn key(p: V3) -> [u32; 3] {
    p.map(|x| (x + 0.0).to_bits())
}

/// Two tangents' mean direction, the first's side.
fn half(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5, a[3]]
}

#[cfg(test)]
fn face_normal(p: [V3; 3]) -> Option<V3> {
    let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
    let l = len(n);
    (l > 1e-12).then(|| scl(n, 1.0 / l))
}

/// The subdivision levels a mod's `[models] smooth` gives the model file `path` (`dataraw/vector/…/<name>.xv`): the
/// ships (the battle's and the designer's) and the colony's buildings and vehicles; the screens' frames, buttons and
/// wire globes and the colony's ground keep their shape.
pub fn levels(path: &str) -> u8 {
    if detailed(path) { crate::visuals::mods().smooth.unwrap_or(0) } else { 0 }
}

fn detailed(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    ["dataraw/vector/spacewar/", "dataraw/vector/design/", "dataraw/vector/colony/"].iter().any(|d| p.starts_with(d)) && !p.contains("/terrainobj/")
}

/// `levels` rounds of Loop subdivision on a part's triangle list (three vertices a face) and its faces' wire-edge
/// flags (bit 0 the edge 2–0, bit 1 0–1, bit 2 1–2): every face becomes four. Points are welded by position; an edge
/// shared by two faces whose normals (turned the way their corners' normals face) differ by at most [`CREASE_DEG`]
/// is smoothed, any other (a sharp edge, a
/// border, three faces or more) is a crease: its points stay on it. A vertex on two creases that run on in a line
/// slides along them; one where they bend by more than [`CREASE_DEG`], or on three or more, stays put. Normals, UVs
/// and the other per-vertex data of the new points are their edge's two corners' means.
pub fn subdivide(verts: &[Vert], edges: &[u8], levels: u8) -> (Vec<Vert>, Vec<u8>) {
    let (mut v, mut e) = (verts.to_vec(), edges.to_vec());
    for _ in 0..levels {
        (v, e) = once(&v, &e);
    }
    (v, e)
}

fn once(verts: &[Vert], edges: &[u8]) -> (Vec<Vert>, Vec<u8>) {
    let tris = verts.len() / 3;
    let mut ids: HashMap<[u32; 3], usize> = HashMap::new();
    let mut pos: Vec<V3> = Vec::new();
    let id: Vec<usize> = verts
        .iter()
        .map(|v| {
            *ids.entry(key(v.pos)).or_insert_with(|| {
                pos.push(v.pos);
                pos.len() - 1
            })
        })
        .collect();
    let normals: Vec<Option<V3>> = verts.chunks_exact(3).map(|t| Some(oriented(t)).filter(|n| len(*n) > 1e-12).map(normalise)).collect();
    let pair = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
    // Each edge's faces and their third points.
    let mut adj: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for f in (0..tris).filter(|&f| normals[f].is_some()) {
        for c in 0..3 {
            let (a, b, o) = (id[3 * f + c], id[3 * f + (c + 1) % 3], id[3 * f + (c + 2) % 3]);
            adj.entry(pair(a, b)).or_default().push((f, o));
        }
    }
    let cos = CREASE_DEG.to_radians().cos();
    let crease = |fs: &[(usize, usize)]| match fs {
        [(f, _), (g, _)] => dot(normals[*f].unwrap_or_default(), normals[*g].unwrap_or_default()) < cos,
        _ => true,
    };
    let mut ring: Vec<Vec<usize>> = vec![Vec::new(); pos.len()];
    let mut creases: Vec<Vec<usize>> = vec![Vec::new(); pos.len()];
    for (&(a, b), fs) in &adj {
        ring[a].push(b);
        ring[b].push(a);
        if crease(fs) {
            creases[a].push(b);
            creases[b].push(a);
        }
    }
    // Sorted, so the sums don't depend on the map's order.
    ring.iter_mut().chain(creases.iter_mut()).for_each(|r| r.sort_unstable());
    let moved: Vec<V3> = (0..pos.len())
        .map(|i| {
            let p = pos[i];
            match creases[i].as_slice() {
                [a, b] => {
                    let (a, b) = (pos[*a], pos[*b]);
                    let straight = dot(normalise(sub(a, p)), normalise(sub(b, p))) < -cos;
                    if straight { add(scl(p, 0.75), scl(add(a, b), 0.125)) } else { p }
                }
                [] | [_] if !ring[i].is_empty() => {
                    let k = ring[i].len() as f32;
                    let beta = if ring[i].len() == 3 { 3.0 / 16.0 } else { 3.0 / (8.0 * k) };
                    let sum = ring[i].iter().fold([0.0; 3], |s, &j| add(s, pos[j]));
                    add(scl(p, 1.0 - k * beta), scl(sum, beta))
                }
                _ => p,
            }
        })
        .collect();
    let edge_point = |a: usize, b: usize| match adj.get(&pair(a, b)).map(Vec::as_slice) {
        Some(fs @ [(_, c), (_, d)]) if !crease(fs) => add(scl(add(pos[a], pos[b]), 0.375), scl(add(pos[*c], pos[*d]), 0.125)),
        _ => scl(add(pos[a], pos[b]), 0.5),
    };
    let mid = |x: &Vert, y: &Vert, p: V3| {
        let ll = (len(x.lnormal) + len(y.lnormal)) * 0.5;
        Vert {
            pos: p,
            normal: normalise(add(x.normal, y.normal)),
            uv: [(x.uv[0] + y.uv[0]) * 0.5, (x.uv[1] + y.uv[1]) * 0.5],
            lnormal: scl(normalise(add(x.lnormal, y.lnormal)), ll),
            fnormal: normalise(add(x.fnormal, y.fnormal)),
            tangent: half(x.tangent, y.tangent),
            ltangent: half(x.ltangent, y.ltangent),
        }
    };
    let mut out = Vec::with_capacity(verts.len() * 4);
    let mut flags = Vec::with_capacity(tris * 4);
    for f in 0..tris {
        let c: [Vert; 3] = std::array::from_fn(|k| Vert { pos: moved[id[3 * f + k]], ..verts[3 * f + k] });
        let m = |k: usize| {
            let (a, b) = (k, (k + 1) % 3);
            mid(&verts[3 * f + a], &verts[3 * f + b], edge_point(id[3 * f + a], id[3 * f + b]))
        };
        let (m01, m12, m20) = (m(0), m(1), m(2));
        out.extend([c[0], m01, m20, m01, c[1], m12, m20, m12, c[2], m01, m12, m20]);
        // A child keeps its parent's flag on the parent edge it lies along.
        let pf = edges.get(f).copied().unwrap_or(7);
        flags.extend([pf & 0b011, pf & 0b110, pf & 0b101, 0]);
    }
    (out, flags)
}

/// The normals of a subdivided part (ADR 0089 §6). The models' file normals are per corner, in the engine's own
/// convention (neither the faces' geometric normals nor shared between them), so each corner takes the area-weighted
/// mean of the file normals at its point over the faces within [`CREASE_DEG`] of its own: smooth across the rounded
/// surfaces, sharp along the creases. `normal`, `fnormal` and `lnormal` each in their own space, `lnormal` keeping its
/// length.
pub fn smooth_normals(verts: &mut [Vert]) {
    let faces: Vec<V3> = verts.chunks_exact(3).map(oriented).collect();
    let mut at: HashMap<[u32; 3], Vec<usize>> = HashMap::new();
    for (i, v) in verts.iter().enumerate() {
        at.entry(key(v.pos)).or_default().push(i);
    }
    let cos = CREASE_DEG.to_radians().cos();
    let unit = |v: V3| if len(v) > 1e-20 { scl(v, 1.0 / len(v)) } else { [0.0; 3] };
    let old = verts.to_vec();
    for (i, v) in verts.iter_mut().enumerate() {
        let own = unit(faces[i / 3]);
        if own == [0.0; 3] {
            continue;
        }
        let group: Vec<usize> = at[&key(v.pos)].iter().copied().filter(|&j| dot(unit(faces[j / 3]), own) >= cos).collect();
        let mean = |f: &dyn Fn(&Vert) -> V3| normalise(group.iter().fold([0.0; 3], |s, &j| add(s, scl(f(&old[j]), len(faces[j / 3])))));
        v.normal = mean(&|x| x.normal);
        v.fnormal = mean(&|x| x.fnormal);
        v.lnormal = scl(mean(&|x| x.lnormal), len(old[i].lnormal));
    }
}

/// A face's normal turned the way its corners' normals face (the files wind faces either way), twice its area long.
fn oriented(t: &[Vert]) -> V3 {
    let n = cross(sub(t[1].pos, t[0].pos), sub(t[2].pos, t[0].pos));
    if dot(n, add(add(t[0].normal, t[1].normal), t[2].normal)) < 0.0 { scl(n, -1.0) } else { n }
}

/// The tangents a normal map is read with (ADR 0089 §6), from the triangles' UVs: per vertex the direction of
/// growing u along its surface, `w` the side (±1) of growing v, as `cross(n, t)·w`. `tangent` in the part's root
/// space beside `normal`; `ltangent` in the lighting space beside `lnormal`, through `m` (the part's matrix) back to
/// the mesh's own space and the lighting chain `ml` with z negated. Corners sharing a point, UV and normal share one.
/// A face without UV area adds nothing; a vertex left without a direction gets none (zero: lit as before).
pub fn tangents(verts: &mut [Vert], m: &M3, ml: &M3) {
    let corner = |v: &Vert| (key(v.pos), v.uv.map(f32::to_bits), key(v.normal));
    let mut acc: HashMap<_, (V3, V3)> = HashMap::new();
    for t in verts.chunks_exact(3) {
        let (e1, e2) = (sub(t[1].pos, t[0].pos), sub(t[2].pos, t[0].pos));
        let (du1, dv1, du2, dv2) = (t[1].uv[0] - t[0].uv[0], t[1].uv[1] - t[0].uv[1], t[2].uv[0] - t[0].uv[0], t[2].uv[1] - t[0].uv[1]);
        let r = du1 * dv2 - du2 * dv1;
        if r.abs() < 1e-12 {
            continue;
        }
        let tu = scl(sub(scl(e1, dv2), scl(e2, dv1)), 1.0 / r);
        let tv = scl(sub(scl(e2, du1), scl(e1, du2)), 1.0 / r);
        for v in t {
            let e = acc.entry(corner(v)).or_insert(([0.0; 3], [0.0; 3]));
            *e = (add(e.0, tu), add(e.1, tv));
        }
    }
    let to_light = inverse(m).map(|mi| mul(ml, &mul(&FLIP_Z, &mi)));
    let frame = |n: V3, t: V3, b: V3| {
        let n = normalise(n);
        let t = across(t, n);
        let w = if dot(cross(n, t), b) < 0.0 { -1.0 } else { 1.0 };
        if t == [0.0; 3] { [0.0; 4] } else { [t[0], t[1], t[2], w] }
    };
    for v in verts.iter_mut() {
        let Some(&(t, b)) = acc.get(&corner(v)) else { continue };
        v.tangent = frame(v.normal, t, b);
        v.ltangent = to_light.map_or([0.0; 4], |l| frame(v.lnormal, crate::scene::apply(&l, t), crate::scene::apply(&l, b)));
    }
}

/// A tangent turned with its normal by `m` (any scale, maybe a mirror): the direction through `m`, the side flipped
/// where `m` mirrors.
pub fn turn(m: &M3, t: [f32; 4]) -> [f32; 4] {
    let d = normalise(crate::scene::apply(m, [t[0], t[1], t[2]]));
    if t[3] == 0.0 {
        return [0.0; 4];
    }
    [d[0], d[1], d[2], t[3] * crate::scene::det(m).signum()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vert(p: V3, uv: [f32; 2], n: V3) -> Vert {
        Vert { pos: p, normal: n, uv, lnormal: [n[0], n[1], -n[2]], fnormal: n, tangent: [0.0; 4], ltangent: [0.0; 4] }
    }

    /// The unit square in the z = 0 plane as two triangles, UVs its xy.
    fn quad() -> Vec<Vert> {
        let z = [0.0, 0.0, 1.0];
        let c = |x: f32, y: f32| vert([x, y, 0.0], [x, y], z);
        vec![c(0.0, 0.0), c(1.0, 0.0), c(1.0, 1.0), c(0.0, 0.0), c(1.0, 1.0), c(0.0, 1.0)]
    }

    #[test]
    fn a_flat_quad_stays_flat_and_square() {
        for levels in 1..=2u8 {
            let (v, e) = subdivide(&quad(), &[7, 7], levels);
            let faces = 2 * 4usize.pow(levels as u32);
            assert_eq!((v.len(), e.len()), (faces * 3, faces));
            for x in &v {
                assert_eq!(x.pos[2], 0.0);
                assert!((0.0..=1.0).contains(&x.pos[0]) && (0.0..=1.0).contains(&x.pos[1]));
                // One round splits it evenly: the UVs ride with the points.
                assert!(levels > 1 || (x.uv[0] - x.pos[0]).abs() < 1e-6 && (x.uv[1] - x.pos[1]).abs() < 1e-6, "{x:?}");
            }
            // The corners stay (two creases bending 90°).
            for corner in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]] {
                assert!(v.iter().any(|x| x.pos == corner));
            }
        }
    }

    fn cube() -> Vec<Vert> {
        let mut v = Vec::new();
        for axis in 0..3 {
            for s in [-1.0f32, 1.0] {
                let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                let p = |x: f32, y: f32| {
                    let mut q = [0.0; 3];
                    (q[axis], q[a], q[b]) = (s, x, y);
                    let mut n = [0.0; 3];
                    n[axis] = s;
                    vert(q, [x, y], n)
                };
                let q = [p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0)];
                let (i, j) = if s > 0.0 { (1, 3) } else { (3, 1) };
                v.extend([q[0], q[i], q[2], q[0], q[2], q[j]]);
            }
        }
        v
    }

    #[test]
    fn a_cube_keeps_its_edges() {
        // Every edge of a cube bends 90°: all creases, every corner on three; nothing moves off the faces.
        let v = cube();
        let (out, _) = subdivide(&v, &[], 2);
        assert_eq!(out.len(), v.len() * 16);
        for x in &out {
            let m = x.pos.iter().fold(0.0f32, |m, c| m.max(c.abs()));
            assert!((m - 1.0).abs() < 1e-6, "{:?} left the cube", x.pos);
        }
        for c in [[1.0, 1.0, 1.0], [-1.0, -1.0, -1.0], [1.0, -1.0, 1.0]] {
            assert!(out.iter().any(|x| x.pos == c), "corner {c:?} moved");
        }
    }

    #[test]
    fn a_shallow_dome_is_smoothed_inside_its_rim() {
        // A hexagon fan with its centre raised a little: the rim is a border (kept in its plane), the inner edges
        // are smooth, so the peak comes down.
        let up = [0.0, 0.0, 1.0];
        let rim: Vec<V3> = (0..6).map(|k| (k as f32 * std::f32::consts::FRAC_PI_3).sin_cos()).map(|(s, c)| [c, s, 0.0]).collect();
        let top = [0.0, 0.0, 0.2];
        let v: Vec<Vert> = (0..6).flat_map(|k| [vert(top, [0.5, 0.5], up), vert(rim[k], [0.0; 2], up), vert(rim[(k + 1) % 6], [0.0; 2], up)]).collect();
        let (out, flags) = subdivide(&v, &[7; 6], 1);
        let peak = out.iter().map(|x| x.pos[2]).fold(0.0f32, f32::max);
        assert!(peak > 0.05 && peak < 0.2, "{peak}");
        assert!(out.iter().filter(|x| x.pos[0].hypot(x.pos[1]) > 0.8).all(|x| x.pos[2] == 0.0));
        // Each face's four children: the middle one draws no wire edge, the corners keep their parent's.
        assert_eq!(&flags[..4], &[0b011, 0b110, 0b101, 0]);
        // Shaded flat, then smoothed: the peak's corners share one normal, straight up; a rim corner leans out.
        let mut faceted: Vec<Vert> = v.chunks_exact(3).flat_map(|t| {
            let n = face_normal([t[0].pos, t[1].pos, t[2].pos]).unwrap();
            t.iter().map(move |x| Vert { normal: n, ..*x }).collect::<Vec<_>>()
        }).collect();
        smooth_normals(&mut faceted);
        for x in faceted.iter().filter(|x| x.pos == top) {
            assert!((x.normal[2] - 1.0).abs() < 1e-6 && (x.lnormal[2] + 1.0).abs() < 1e-6, "{:?}", x.normal);
        }
        assert!(faceted.iter().filter(|x| x.pos != top).all(|x| x.normal[2] < 1.0 && dot(x.normal, [x.pos[0], x.pos[1], 0.0]) > 0.0));
        // A cube's corners keep their own faces' normals (every edge bends 90°).
        let mut box_ = cube();
        smooth_normals(&mut box_);
        assert!(box_.iter().zip(cube()).all(|(a, b)| a.normal == b.normal));
    }

    #[test]
    fn ships_and_buildings_only() {
        assert!(detailed("dataraw/vector/spacewar/hajo1.xv") && detailed("dataraw/vector/design/alapok/a.xv") && detailed("dataraw/vector/colony/kkpv2.xv"));
        assert!(!detailed("dataraw/vector/colony/terrainobj/t.xv") && !detailed("dataraw/vector/talalmanyok/globe.xv") && !detailed("dataraw/vector/screen/gomb.xv"));
        // Without a mod nothing is smoothed.
        assert_eq!(levels("dataraw/vector/spacewar/hajo1.xv"), 0);
    }

    #[test]
    fn tangents_follow_the_uvs() {
        let mut v = quad();
        tangents(&mut v, &crate::scene::IDENTITY, &crate::scene::IDENTITY);
        for x in &v {
            assert_eq!(x.tangent, [1.0, 0.0, 0.0, 1.0]);
            // The lighting space negates z: the frame mirrors, so v's side flips.
            assert_eq!(x.ltangent, [1.0, 0.0, 0.0, -1.0]);
        }
        // UVs running the other way round: v grows down, the side is −1.
        let mut w = quad();
        w.iter_mut().for_each(|x| x.uv[1] = 1.0 - x.uv[1]);
        tangents(&mut w, &crate::scene::IDENTITY, &crate::scene::IDENTITY);
        assert!(w.iter().all(|x| x.tangent == [1.0, 0.0, 0.0, -1.0]));
        // A part scaled and turned: the tangent turns with it, a unit vector across the normal.
        let m = crate::scene::mul(&crate::scene::create_matrix(0.3, 0.2, 0.1), &crate::scene::scale([2.0, 3.0, 1.0]));
        let mut s = quad();
        s.iter_mut().for_each(|x| {
            x.pos = crate::scene::apply(&m, x.pos);
            x.normal = normalise(crate::scene::apply(&inverse(&m).map(|i| transpose(&i)).unwrap(), x.normal));
        });
        tangents(&mut s, &m, &crate::scene::IDENTITY);
        for x in &s {
            let t = [x.tangent[0], x.tangent[1], x.tangent[2]];
            assert!((len(t) - 1.0).abs() < 1e-5 && dot(t, x.normal).abs() < 1e-5);
            assert!((dot(t, normalise(crate::scene::apply(&m, [1.0, 0.0, 0.0]))) - 1.0).abs() < 1e-5);
            assert_eq!(x.tangent[3], 1.0);
            assert!((x.ltangent[0] - 1.0).abs() < 1e-5, "{:?}", x.ltangent);
        }
        // A degenerate UV face adds none.
        let mut d = quad();
        d.iter_mut().for_each(|x| x.uv = [0.5, 0.5]);
        tangents(&mut d, &crate::scene::IDENTITY, &crate::scene::IDENTITY);
        assert!(d.iter().all(|x| x.tangent == [0.0; 4]));
        // A mirror flips the side.
        assert_eq!(turn(&crate::scene::scale([-2.0, 2.0, 2.0]), [1.0, 0.0, 0.0, 1.0]), [-1.0, 0.0, 0.0, -1.0]);
    }

    fn transpose(m: &M3) -> M3 {
        std::array::from_fn(|k| m[(k % 3) * 3 + k / 3])
    }
}



