//! The starmap's radar ranges (re/screens/starmap.md §6.3): `RadarRangeKi` (ELF 0x70e030) collects the circles of
//! the human's and its allies' radar planets and the human's fleets, `RadarRangeReszKi` (0x70dc80) puts dots on each
//! circle, `CopyRadarParticles` (0x70caa0) removes the dots inside other circles and the circles another swallows,
//! and turns what is left into the `radar.bmp` particles (`FStars` set 2) and the `rrange.xvb`/`rrang2.xvb` discs.

/// One radar circle (`RadarKorok`, 0x14 bytes): galaxy centre, radius and its source (> 0 planet, < 0 fleet).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub centre: [f32; 2],
    pub r: f32,
    pub id: i16,
}

/// A dot on a circle's visible arc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dot {
    pub pos: [f32; 2],
    /// A fleet's circle (blue `0xc04000`) rather than a planet's (red `0xff`).
    pub fleet: bool,
}

/// What the starmap draws: the dots and the discs of the circles nothing swallowed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rings {
    pub dots: Vec<Dot>,
    /// Centre, radius, fleet (`rrang2.xvb`) or planet (`rrange.xvb`).
    pub discs: Vec<([f32; 2], f32, bool)>,
}

/// `RadarRangeReszKi`'s caps: 400 circles, 4000 dots; `CopyRadarParticles` keeps 999 dots.
const MAX_CIRCLES: usize = 400;
const MAX_DOTS: usize = 4000;
const MAX_PARTICLES: usize = 999;
/// `0x461c3c00`: an arc already joined.
const USED: f32 = 9999.0;

struct Korok {
    c: Circle,
    dead: bool,
    first: usize,
    last: usize,
}

/// `RadarRangeReszKi` for every circle in order, then `CopyRadarParticles`. `planet_circles` = how many of the
/// first circles are planets' (`StarmapRadarFlottaHatar`).
pub fn rings(circles: &[Circle], planet_circles: usize) -> Rings {
    // 1-based like the original.
    let mut kor: Vec<Korok> = vec![Korok { c: Circle { centre: [0.0; 2], r: 0.0, id: 0 }, dead: true, first: 0, last: 0 }];
    // (x, y, circle; 0 = removed).
    let mut pont: Vec<([f32; 2], usize)> = vec![([0.0; 2], 0)];
    let mut fleet_dots = usize::MAX;
    for (i, c) in circles.iter().enumerate() {
        if i == planet_circles {
            fleet_dots = pont.len() - 1;
        }
        if kor.len() > MAX_CIRCLES {
            break;
        }
        let k = kor.len();
        let n = if c.r < 70.0 { if c.r < 35.0 { 2 } else { 3 } } else { 4 };
        let n_dots = pont.len() - 1;
        kor.push(Korok { c: *c, dead: false, first: n_dots + 1, last: (n_dots + n * 8).min(MAX_DOTS) });
        let step = 1.5707964f32 / n as f32 / 2.0;
        let mut a = step / 2.0;
        let [x, y] = c.centre;
        for _ in 0..n {
            let co = ((a as f64).cos() as f32) * c.r;
            let si = ((a as f64).sin() as f32) * c.r;
            for p in [[x + co, y + si], [x + co, y - si], [x - co, y + si], [x - co, y - si], [x + si, y + co], [x + si, y - co], [x - si, y + co], [x - si, y - co]] {
                if pont.len() <= MAX_DOTS {
                    pont.push((p, k));
                }
            }
            a += step;
        }
    }
    let planet_circles = planet_circles.min(kor.len() - 1);
    if fleet_dots == usize::MAX {
        fleet_dots = pont.len() - 1;
    }
    let n = kor.len() - 1;
    let sq = |v: f32| v * v;
    let (mut start, mut end) = (vec![0.0f32; n + 2], vec![0.0f32; n + 2]);
    for c in 1..=n {
        if kor[c].dead {
            continue;
        }
        let [cx, cy] = kor[c].c.centre;
        let rc = kor[c].c.r;
        let rc2 = sq(rc);
        let mut cnt = 0;
        for o in 1..=n {
            if kor[c].dead || kor[o].dead || o == c {
                continue;
            }
            let dx = kor[o].c.centre[0] - cx;
            let dy = kor[o].c.centre[1] - cy;
            let d2 = sq(dx) + sq(dy);
            let ro = kor[o].c.r;
            if d2 > sq(ro + rc) {
                continue;
            }
            if rc <= ro || sq(rc - ro) + 1.0 <= d2 {
                // The dots of `o` inside `c`.
                for j in kor[o].first..=kor[o].last.min(pont.len() - 1) {
                    if pont[j].1 != 0 && sq(pont[j].0[0] - cx) + sq(pont[j].0[1] - cy) <= rc2 - 1.0 {
                        pont[j].1 = 0;
                    }
                }
            } else {
                // `c` swallows `o`.
                for j in kor[o].first..=kor[o].last.min(pont.len() - 1) {
                    pont[j].1 = 0;
                }
                kor[o].dead = true;
            }
            // Two planets' equal circles each holding the other's centre: the arc of `c` inside `o`.
            if d2 <= rc2 && rc == ro && c <= planet_circles && o <= planet_circles && 0.01 < d2 {
                cnt += 1;
                let d = (d2 as f64).sqrt() as f32;
                let mut ang = (dx / d) as f64;
                ang = if dy < 0.0 { std::f64::consts::TAU - ang.acos() } else { ang.acos() };
                let mut ang = ang as f32;
                let half = ((d / 2.0 / rc) as f64).acos() as f32;
                if ang < half {
                    ang += 6.2831855;
                }
                start[cnt] = ang - half;
                end[cnt] = ang + half;
            }
        }
        // Covered all round by such arcs: the circle goes. (The original compares the last arc's start each time,
        // so the "first" arc is the last one or the one found right after a smaller start.)
        if cnt > 2 {
            let mut k = 1;
            for i in 2..=cnt {
                if start[cnt] < start[k] {
                    k = i;
                }
            }
            let stop = start[k] + 6.2831855;
            let mut akt = end[k] + 0.005;
            start[k] = USED;
            loop {
                let mut joined = false;
                for i in 1..=cnt {
                    if start[i] <= akt && akt <= end[i] {
                        akt = end[i] + 0.005;
                        start[i] = USED;
                        joined = true;
                    }
                }
                if !joined {
                    break;
                }
            }
            if stop <= akt {
                for j in kor[c].first..=kor[c].last.min(pont.len() - 1) {
                    pont[j].1 = 0;
                }
                kor[c].dead = true;
            }
        }
    }
    let dots = pont
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, p)| p.1 != 0)
        .take(MAX_PARTICLES)
        .map(|(i, p)| Dot { pos: p.0, fleet: i > fleet_dots })
        .collect();
    let discs = kor.iter().enumerate().skip(1).filter(|(_, k)| !k.dead).map(|(i, k)| (k.c.centre, k.c.r, i > planet_circles)).collect();
    Rings { dots, discs }
}

/// `RadarRangeKi`: the circles in the original's order and how many of them are planets'. For each empire allied
/// with the human (`Viszony` level 5, the human included, 1..=`ProgramPlayers2`), each planet with radar level
/// `+0x6e` 1 or ≥ 3 that this empire owns, or of level ≥ 3 whoever owns it (once per ally), radius
/// `RadarHatotav2[owner][level]`; then the human's fleets (`+0x52` bit 5 clear) and the story's tracked fleet
/// (`StoryAdat+0x28`, `+0x18` bit 0 clear) with a radar range `+0x7c` > 0.
pub fn circles(state: &ig2_sim::save::SaveState, map: &ig2_sim::fleet::Starmap, radar: &ig2_sim::radar::RadarTables, en: usize) -> (Vec<Circle>, usize) {
    use ig2_sim::save::va;
    let players = (state.mem.i32(va::PROGRAM_PLAYERS2).max(0) as usize).min(crate::world::EMPIRES - 1);
    let rel = state.relations();
    let allies = (1..=players).filter(|&l| l == en || rel.get(en).is_some_and(|r| r[l].level == 5));
    let mut out = Vec::new();
    for u in allies {
        for (p, sp) in map.planets.iter().enumerate().skip(1) {
            let lvl = sp.radar_level;
            if lvl != 0 && lvl != 2 && (sp.owner as usize == u || lvl > 1) {
                let r = radar.range.get(sp.owner as usize).and_then(|row| row.get(lvl as usize)).copied().unwrap_or(0.0);
                out.push(Circle { centre: [sp.x, sp.y], r, id: p as i16 });
            }
        }
    }
    let planets = out.len();
    let story = i32::from_le_bytes(state.mem.read(va::STORY_ADAT + 0x28, 4).try_into().unwrap());
    for (f, fl) in map.fleets.iter().enumerate().skip(1) {
        let own = fl.owner as usize == en && fl.flags & 0x20 == 0;
        if (own || (story == f as i32 && fl.state & 1 == 0)) && fl.radar > 0.0 {
            out.push(Circle { centre: [fl.x, fl.y], r: fl.radar, id: -(f as i16) });
        }
    }
    (out, planets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: f32, y: f32, r: f32, id: i16) -> Circle {
        Circle { centre: [x, y], r, id }
    }

    /// quick-1.sav's rings against the exe's live `radar.bmp` particles (`run_screen.py --particles`,
    /// `_local/traces/starmap-particles.json`; IG2_INSTALL_DIR and IG2_SAVE_DIR): every dot, in order, bit for bit.
    #[test]
    fn quick1_rings_match_the_exes_particles() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let traces = std::env::var("IG2_TRACE_DIR").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../_local/traces").into());
        let Ok(text) = std::fs::read_to_string(format!("{traces}/starmap-particles.json")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let w = crate::world::World::load(&save, &rules, &defaults, &volatile);
        let d: serde_json::Value = serde_json::from_str(&text).unwrap();
        let set = &d["fstars"][2];
        let recs = set["recs"].as_str().unwrap();
        let b: Vec<u8> = (0..recs.len() / 2).map(|i| u8::from_str_radix(&recs[2 * i..2 * i + 2], 16).unwrap()).collect();
        let f = |r: &[u8], o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let exe: Vec<([f32; 2], u32)> = b.chunks_exact(0x4c).map(|r| ([f(r, 4), f(r, 8)], u32::from_le_bytes(r[0xc..0x10].try_into().unwrap()))).collect();
        let port: Vec<([f32; 2], u32)> = w.curr.radar.dots.iter().map(|d| (d.pos, if d.fleet { 0xc04000 } else { 0xff })).collect();
        assert_eq!(port.len(), exe.len());
        for (i, (a, b)) in port.iter().zip(&exe).enumerate() {
            assert!(a.0.map(f32::to_bits) == b.0.map(f32::to_bits) && a.1 == b.1, "dot {i}: port {a:?}, exe {b:?}");
        }
    }

    #[test]
    fn a_lone_circle_has_eight_dots_per_step() {
        let r = rings(&[c(0.0, 0.0, 20.0, 1)], 1);
        assert_eq!(r.dots.len(), 16);
        assert!(r.dots.iter().all(|d| ((d.pos[0] * d.pos[0] + d.pos[1] * d.pos[1]).sqrt() - 20.0).abs() < 1e-3 && !d.fleet));
        assert_eq!(rings(&[c(0.0, 0.0, 50.0, 1)], 1).dots.len(), 24);
        assert_eq!(rings(&[c(0.0, 0.0, 80.0, 1)], 1).dots.len(), 32);
    }

    #[test]
    fn overlapping_circles_lose_the_dots_inside_each_other() {
        let r = rings(&[c(0.0, 0.0, 20.0, 1), c(30.0, 0.0, 20.0, 2)], 2);
        assert!(r.dots.len() < 32);
        for d in &r.dots {
            let a = d.pos[0] * d.pos[0] + d.pos[1] * d.pos[1];
            let b = (d.pos[0] - 30.0).powi(2) + d.pos[1] * d.pos[1];
            assert!(a > 399.0 && b > 399.0 || (a - 400.0).abs() < 1.0 || (b - 400.0).abs() < 1.0);
        }
        assert_eq!(r.discs.len(), 2);
    }

    #[test]
    fn a_swallowed_circle_goes_with_its_disc() {
        let r = rings(&[c(0.0, 0.0, 60.0, 1), c(5.0, 0.0, 20.0, -3)], 1);
        assert_eq!(r.discs, vec![([0.0, 0.0], 60.0, false)]);
        assert!(r.dots.iter().all(|d| !d.fleet));
    }
}
