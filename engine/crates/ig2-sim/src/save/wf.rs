//! The ground battlefields in a save (`WF::LoadSave` exe 0x485420 and `WF_Tank::LoadSave` 0x4859a0, both
//! directions of one function, `re/formats/save.md` §4.13): battlefield `i` = planet `i + 1`'s
//! [`Field`], decoded from the chunks `WF[i]…` and written back in the exe's order. The exe writes version 0x6a.

use crate::ground::{Bolt, Field, Fighter, Group, Missile, Shell};

/// A record without most of its per-unit state (`WF_Tank +0x188`).
const SHORT: u32 = 0x400_0000;
/// The ammunition words in save order: weapon bit `b` (`+0x1d0`) travels as weapon index `AMMO[b]`'s counter
/// (`+0xa8 + 4·k`) and reload time (`+0x1f0 + 4·k`).
const AMMO: [usize; 14] = [0, 8, 3, 12, 2, 13, 10, 11, 4, 5, 6, 7, 9, 1];

fn i32_of(b: &[u8]) -> i32 {
    b.get(..4).map_or(0, |x| i32::from_le_bytes(x.try_into().unwrap()))
}
fn f32_of(b: &[u8]) -> f32 {
    f32::from_bits(i32_of(b) as u32)
}
fn ints(b: &[u8]) -> Vec<i32> {
    b.chunks_exact(4).map(i32_of).collect()
}
fn f3(b: &[u8], o: usize) -> [f32; 3] {
    std::array::from_fn(|k| f32_of(&b[o + 4 * k..]))
}

/// Battlefield `i`'s chunks `(name, bytes)` (as `ig2_formats::save` names them) → planet `i + 1`'s field. Fields the
/// save doesn't hold keep the constructors' values; as the exe's load does, every unit's body turning speed `+0xe0`
/// becomes 0.65.
pub(super) fn decode<'a>(chunks: impl Iterator<Item = (&'a str, &'a [u8])>, tag: &str) -> Field {
    let mut f = Field::default();
    let (mut shells, mut bolts, mut missiles) = (Vec::new(), Vec::new(), Vec::new());
    let mut present: [Vec<bool>; 3] = Default::default();
    let mut unit: Option<(usize, Group)> = None;
    let (mut ammo, mut ammo2) = (0usize, 0usize);
    let mut origin_seen = false;
    let bits_of = |g: &Group| (0..14).filter(|b| g.weapons & (1 << b) != 0).map(|b| AMMO[b]).collect::<Vec<_>>();
    for (name, b) in chunks {
        let rest = &name[tag.len()..];
        let i = || i32_of(b);
        let u = || i32_of(b) as u32;
        let fl = || f32_of(b);
        if let Some(r) = rest.strip_prefix(".unit[") {
            let (k, field) = r.split_once(']').unwrap();
            let k: usize = k.parse().unwrap();
            if unit.as_ref().is_none_or(|(j, _)| *j != k) {
                if let Some((j, g)) = unit.take() {
                    f.units[j] = g;
                }
                unit = Some((k, Group::default()));
                (ammo, ammo2, origin_seen) = (0, 0, false);
            }
            let g = &mut unit.as_mut().unwrap().1;
            if field.starts_with(".ammo2[") {
                if let Some(&k) = bits_of(g).get(ammo2) {
                    g.rt.reload[k] = fl();
                }
                ammo2 += 1;
                continue;
            }
            if field.starts_with(".ammo[") {
                if let Some(&k) = bits_of(g).get(ammo) {
                    g.rt.reload_left[k] = fl();
                }
                ammo += 1;
                continue;
            }
            let rt = &mut g.rt;
            match field {
                "+0x188" => g.flags = u(),
                "+0x10c" => rt.velocity = fl(),
                "+0x104" => rt.blocked = b[0],
                "+0x1b0" => g.dest.0 = i(),
                "+0x1b4" => g.dest.1 = i(),
                "+0x23c" => g.target = i(),
                "+0x14" => g.tail.f14 = u(),
                "+0xe8" => g.tail.fe8 = u(),
                "+0x158" => g.max_hp = i(),
                "+0x15c" => g.design = i(),
                "+0x194" => g.owner = i(),
                "+0x198" => g.side = i(),
                // Saved twice: a full record's pointer value first, then the members' origins.
                "+0x19c" if !origin_seen && g.flags & SHORT == 0 => {
                    g.tail.origin_ptr = u();
                    origin_seen = true;
                }
                "+0x19c" => g.origin = ints(b),
                "+0x108" => rt.max_speed = fl(),
                "+0x110" => rt.accel = fl(),
                "+0x114" => rt.decel = fl(),
                "+0x118" => rt.path_timer = i(),
                "+0x1a0" => g.cell.0 = i(),
                "+0x1a4" => g.cell.1 = i(),
                "+0x1a8" => g.last_cell.0 = i(),
                "+0x1ac" => g.last_cell.1 = i(),
                "+0x1b8" => g.move_state.0 = i(),
                "+0x1cc" => g.regen = i(),
                "+0x1d0" => g.weapons = u(),
                "+0x1e0" => g.carrier = i(),
                "+0x1e4" => g.partner = i(),
                "+0x1e8" => g.paralysed = i(),
                "+0x1ec" => g.seized = i(),
                "+0xc" => rt.step.0 = fl(),
                "+0x10" => rt.step.1 = fl(),
                "+0x12c" => rt.want_angle = fl(),
                "+0x18" => rt.aim = fl(),
                "+0x150" => rt.speed = fl(),
                "+0x98" => rt.dir.0 = fl(),
                "+0x9c" => rt.dir.1 = fl(),
                "+0x148" => rt.range = fl(),
                "+0x11c" => g.pods = if (0..=2).contains(&i()) { i() } else { 1 },
                "+0x288" => g.air = i(),
                "+0x120" => g.ammo = i(),
                "+0xe0" => {}
                "+0xe4" => rt.turn.1 = fl(),
                "+0x1c0" => rt.angle = fl(),
                "+0x1c4" => rt.turret_angle = fl(),
                "+0x228" => rt.f228 = u(),
                "+0x1c8" => rt.elevation = fl(),
                "+0x1d4" => g.pos.0 = fl(),
                "+0x1d8" => g.pos.1 = fl(),
                "+0x1dc" => rt.z = fl(),
                "+0x1c" => rt.basis = std::array::from_fn(|k| f32_of(&b[4 * k..])),
                "+0x244" => g.index = i(),
                "+0x248" => rt.ai_group = i(),
                "+0x154" => g.hp = ints(b),
                "+0x18c" => g.flags2 = u(),
                "+0x138" => g.prev_size = i(),
                "+0x146" => g.tail.f146 = b[0],
                "+0x13c" => g.tail.f13c.copy_from_slice(&b[..10]),
                "+0x240" => g.tail.f240 = u(),
                "+0x24c" => rt.ai_home = i(),
                "+0x250" => rt.laying = b[0],
                "+0x254" => rt.mine_cell.0 = i(),
                "+0x258" => rt.mine_cell.1 = i(),
                "+0x25c" => g.last_hit = Some((i(), g.last_hit.map_or(0, |h| h.1))),
                "+0x260" => g.last_hit = Some((g.last_hit.map_or(0, |h| h.0), i())),
                "+0x238" => g.building = i(),
                "+0x264" => {
                    rt.drop = f3(b, 0);
                    g.tail.f270 = i32_of(&b[0xc..]) as u32;
                    g.tail.f274 = i32_of(&b[0x10..]) as u32;
                    g.transport = i32_of(&b[0x14..]) as u32;
                }
                "+0x130" => g.has_gun = b[0] != 0,
                "+0x134" => g.hull = i(),
                _ => {}
            }
            continue;
        }
        let flags = |b: &[u8]| ints(b).into_iter().map(|x| x != 0).collect::<Vec<_>>();
        match rest {
            "+0x8c8" => f.last_status = i(),
            "+0x8cc" => f.status_valid = i() != 0,
            "+0x18" => f.tail.f18 = u(),
            ".countdown" => f.countdown = i(),
            "+0x190" => f.finished_flag = b[0] != 0,
            "+0x1ec" => {
                f.tail.strike = b.to_vec();
                for (k, x) in f.lists.fighters.iter_mut().enumerate() {
                    let o = 8 + 0x40 * k;
                    *x = Fighter {
                        pos: f3(b, o),
                        vel: f3(b, o + 0xc),
                        step: f3(b, o + 0x18),
                        phase: i32_of(&b[o + 0x24..]),
                        ground: f32_of(&b[o + 0x28..]),
                        cooldown: i32_of(&b[o + 0x34..]),
                    };
                }
            }
            "+0x32c" => f.lists.fighters_on = b[0] != 0,
            "+0x184" => f.tail.f184 = b[0],
            "+0x18c" => f.tail.planet = i(),
            "+0x1a8" | "+0x1ac" | "+0x1b0" | "+0x1b4" | "+0x1b8" => {
                let k = (usize::from_str_radix(&rest[3..], 16).unwrap() - 0x1a8) / 4;
                f.tail.f1a8[k] = u();
            }
            "+0x334" => f.self_destruct = i(),
            "+0x338" => f.attack_fleet = i(),
            "+0x33c" => {
                f.tail.ai0 = b[..0x40].to_vec();
                for (e, a) in f.ai.iter_mut().enumerate().skip(1) {
                    let w = &b[0x40 * e..];
                    a.strategy = i32_of(w);
                    a.mode = i32_of(&w[4..]);
                    a.phase = i32_of(&w[8..]);
                    a.a = (f32_of(&w[0xc..]), f32_of(&w[0x10..]));
                    a.b = (f32_of(&w[0x14..]), f32_of(&w[0x18..]));
                    a.start = (f32_of(&w[0x1c..]), f32_of(&w[0x20..]));
                    a.timer = i32_of(&w[0x24..]);
                    a.counter = i32_of(&w[0x28..]);
                    a.targets = std::array::from_fn(|g| i32_of(&w[0x2c + 4 * g..]));
                }
            }
            "+0x7fc" => {
                for (e, t) in f.timers.iter_mut().enumerate() {
                    *t = (i32_of(&b[8 * e..]), i32_of(&b[8 * e + 4..]));
                }
            }
            "+0x88c" => f.simulating = b[0] != 0,
            "+0x88d" => f.not_watching.copy_from_slice(&b[..18]),
            "+0x89f" => f.retreat.copy_from_slice(&b[..18]),
            ".rocket_present" => present[0] = flags(b),
            ".paralyser_present" => present[1] = flags(b),
            ".laser_present" => present[2] = flags(b),
            r if r.starts_with(".rocket+") => {
                let o = usize::from_str_radix(&r[10..], 16).unwrap();
                if o == 8 {
                    shells.push(Shell { vel: [0.0; 3], pos: [0.0; 3], life: 0, target: 0, pull: 0.0, dmg: 0, by: 0, jitter: 1 });
                }
                let s = shells.last_mut().unwrap();
                match o {
                    8..=0x10 => s.vel[(o - 8) / 4] = fl(),
                    0x14..=0x1c => s.pos[(o - 0x14) / 4] = fl(),
                    0x20 => s.life = i(),
                    0x24 => s.target = i(),
                    0x28 => s.pull = fl(),
                    0x2c => s.dmg = i(),
                    0x30 => s.by = i(),
                    _ => s.jitter = b[0],
                }
            }
            ".paralyser" => {
                f.tail.bolt_heads.push(b[..8].try_into().unwrap());
                bolts.push(Bolt { vel: f3(b, 8), pos: f3(b, 0x14), dest: f3(b, 0x20), life: i32_of(&b[0x2c..]), target: i32_of(&b[0x30..]), power: i32_of(&b[0x34..]) });
            }
            ".laser" => {
                f.tail.missile_heads.push(b[..8].try_into().unwrap());
                missiles.push(Missile {
                    vel: f3(b, 8),
                    pos: f3(b, 0x14),
                    dest: f3(b, 0x20),
                    life: i32_of(&b[0x2c..]),
                    dmg: i32_of(&b[0x30..]),
                    target: i32_of(&b[0x34..]),
                    kind: i32_of(&b[0x38..]),
                    fade: f32_of(&b[0x3c..]),
                    by: i32_of(&b[0x40..]),
                });
            }
            ".alive" => {
                f.alive = b.iter().map(|&x| x != 0).collect();
                f.units = vec![Group::default(); f.alive.len()];
            }
            ".unit_present" => f.tail.absent = flags(b).into_iter().map(|x| !x).collect(),
            "+0x8d0" => f.weather = i(),
            "+0x8d4" => f.background = i(),
            _ => {}
        }
    }
    if let Some((j, g)) = unit.take() {
        f.units[j] = g;
    }
    for (k, g) in f.units.iter_mut().enumerate() {
        if f.alive[k] {
            g.rt.turn.0 = 0.65;
            g.rt.planet = f.tail.planet;
        }
    }
    // The present objects of each list take their slots in order; absent slots are empty.
    fn place<T>(present: &[bool], objs: Vec<T>) -> Vec<Option<T>> {
        let mut it = objs.into_iter();
        present.iter().map(|&p| if p { it.next() } else { None }).collect()
    }
    f.lists.shells = place(&present[0], shells);
    f.lists.bolts = place(&present[1], bolts);
    f.lists.missiles = place(&present[2], missiles);
    f
}

/// Planet `i + 1`'s field as `WF::LoadSave(save)` writes battlefield `i`.
pub(super) fn encode(f: &Field, tag: &str) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut put = |n: String, b: Vec<u8>| out.push((n, b));
    let w = |x: i32| x.to_le_bytes().to_vec();
    let wu = |x: u32| x.to_le_bytes().to_vec();
    let wf = |x: f32| x.to_bits().to_le_bytes().to_vec();
    let p = tag;
    put(format!("{p}+0x8c8"), w(f.last_status));
    // `WF::LoadSave` clears the status cache before the variable in either direction: a save writes 0
    // ([`super::SaveState::battlefields_saved`] clears it in the state).
    put(format!("{p}+0x8cc"), w(0));
    put(format!("{p}+0x18"), wu(f.tail.f18));
    put(format!("{p}.countdown"), w(f.countdown));
    let l = &f.lists;
    put(format!("{p}.unit_slots"), w(f.units.len() as i32));
    put(format!("{p}.rockets"), w(l.shells.len() as i32));
    put(format!("{p}.paralysers"), w(l.bolts.len() as i32));
    put(format!("{p}.lasers"), w(l.missiles.len() as i32));
    put(format!("{p}+0x190"), vec![f.finished_flag as u8]);
    let mut strike = f.tail.strike.clone();
    strike.resize(0x140, 0);
    for (k, x) in l.fighters.iter().enumerate() {
        let o = 8 + 0x40 * k;
        let mut at = |off: usize, v: [u8; 4]| strike[o + off..o + off + 4].copy_from_slice(&v);
        for j in 0..3 {
            at(4 * j, x.pos[j].to_bits().to_le_bytes());
            at(0xc + 4 * j, x.vel[j].to_bits().to_le_bytes());
            at(0x18 + 4 * j, x.step[j].to_bits().to_le_bytes());
        }
        at(0x24, x.phase.to_le_bytes());
        at(0x28, x.ground.to_bits().to_le_bytes());
        at(0x34, x.cooldown.to_le_bytes());
    }
    put(format!("{p}+0x1ec"), strike);
    put(format!("{p}+0x32c"), vec![l.fighters_on as u8]);
    put(format!("{p}+0x184"), vec![f.tail.f184]);
    put(format!("{p}+0x18c"), w(f.tail.planet));
    for (k, v) in f.tail.f1a8.iter().enumerate() {
        put(format!("{p}+{:#x}", 0x1a8 + 4 * k), wu(*v));
    }
    put(format!("{p}+0x334"), w(f.self_destruct));
    put(format!("{p}+0x338"), w(f.attack_fleet));
    let mut ai = f.tail.ai0.clone();
    ai.resize(0x40, 0);
    for a in f.ai.iter().skip(1) {
        for v in [a.strategy, a.mode, a.phase] {
            ai.extend(w(v));
        }
        for v in [a.a.0, a.a.1, a.b.0, a.b.1, a.start.0, a.start.1] {
            ai.extend(wf(v));
        }
        ai.extend(w(a.timer));
        ai.extend(w(a.counter));
        for v in a.targets {
            ai.extend(w(v));
        }
    }
    put(format!("{p}+0x33c"), ai);
    put(format!("{p}+0x7fc"), f.timers.iter().flat_map(|t| [t.0.to_le_bytes(), t.1.to_le_bytes()]).flatten().collect());
    put(format!("{p}+0x88c"), vec![f.simulating as u8]);
    put(format!("{p}+0x88d"), f.not_watching.to_vec());
    put(format!("{p}+0x89f"), f.retreat.to_vec());
    let flags = |v: &mut dyn Iterator<Item = bool>| v.flat_map(|x| (x as u32).to_le_bytes()).collect::<Vec<u8>>();
    put(format!("{p}.rocket_present"), flags(&mut l.shells.iter().map(Option::is_some)));
    for s in l.shells.iter().flatten() {
        for (k, v) in s.vel.iter().chain(&s.pos).enumerate() {
            put(format!("{p}.rocket+{:#x}", 8 + 4 * k), wf(*v));
        }
        put(format!("{p}.rocket+0x20"), w(s.life));
        put(format!("{p}.rocket+0x24"), w(s.target));
        put(format!("{p}.rocket+0x28"), wf(s.pull));
        put(format!("{p}.rocket+0x2c"), w(s.dmg));
        put(format!("{p}.rocket+0x30"), w(s.by));
        put(format!("{p}.rocket+0x34"), vec![s.jitter]);
    }
    put(format!("{p}.paralyser_present"), flags(&mut l.bolts.iter().map(Option::is_some)));
    for (k, b) in l.bolts.iter().flatten().enumerate() {
        let mut r = f.tail.bolt_heads.get(k).map_or(vec![0; 8], |h| h.to_vec());
        for v in b.vel.iter().chain(&b.pos).chain(&b.dest) {
            r.extend(wf(*v));
        }
        for v in [b.life, b.target, b.power] {
            r.extend(w(v));
        }
        put(format!("{p}.paralyser"), r);
    }
    put(format!("{p}.laser_present"), flags(&mut l.missiles.iter().map(Option::is_some)));
    for (k, m) in l.missiles.iter().flatten().enumerate() {
        let mut r = f.tail.missile_heads.get(k).map_or(vec![0; 8], |h| h.to_vec());
        for v in m.vel.iter().chain(&m.pos).chain(&m.dest) {
            r.extend(wf(*v));
        }
        for v in [m.life, m.dmg, m.target, m.kind] {
            r.extend(w(v));
        }
        r.extend(wf(m.fade));
        r.extend(w(m.by));
        put(format!("{p}.laser"), r);
    }
    put(format!("{p}.alive"), f.alive.iter().map(|&a| a as u8).collect());
    let present = (0..f.units.len()).map(|k| f.alive[k] || !f.tail.absent.get(k).copied().unwrap_or(false));
    put(format!("{p}.unit_present"), flags(&mut present.into_iter()));
    for (k, g) in f.units.iter().enumerate().filter(|(k, _)| f.alive[*k]) {
        unit(g, &format!("{p}.unit[{k}]"), &mut put);
    }
    put(format!("{p}+0x8d0"), w(f.weather));
    put(format!("{p}+0x8d4"), w(f.background));
    out
}

/// `WF_Tank::LoadSave(save, 0x6a, planet)` for one alive unit.
fn unit(g: &Group, u: &str, put: &mut impl FnMut(String, Vec<u8>)) {
    let full = g.flags & SHORT == 0;
    let rt = &g.rt;
    let mut i = |o: u32, v: i32| put(format!("{u}+{o:#x}"), v.to_le_bytes().to_vec());
    let n = g.hp.len();
    let bits: Vec<usize> = (0..14).filter(|b| g.weapons & (1 << b) != 0).map(|b| AMMO[b]).collect();
    let fb = |x: f32| x.to_bits() as i32;
    i(0x188, g.flags as i32);
    i(0x10c, fb(rt.velocity));
    let mut items: Vec<(String, Vec<u8>)> = Vec::new();
    let mut add = |o: u32, v: Vec<u8>| items.push((format!("{u}+{o:#x}"), v));
    let le = |x: i32| x.to_le_bytes().to_vec();
    if full {
        add(0x104, vec![rt.blocked]);
    }
    for (o, v) in [(0x1b0, g.dest.0), (0x1b4, g.dest.1), (0x23c, g.target), (0x14, g.tail.f14 as i32), (0xe8, g.tail.fe8 as i32),
        (0x158, g.max_hp), (0x15c, g.design), (0x190, n as i32), (0x194, g.owner), (0x198, g.side)] {
        add(o, le(v));
    }
    if full {
        add(0x19c, le(g.tail.origin_ptr as i32));
        for (o, v) in [(0x108, fb(rt.max_speed)), (0x110, fb(rt.accel)), (0x114, fb(rt.decel)), (0x118, rt.path_timer)] {
            add(o, le(v));
        }
    }
    add(0x1a0, le(g.cell.0));
    add(0x1a4, le(g.cell.1));
    if full {
        for (o, v) in [(0x1a8, g.last_cell.0), (0x1ac, g.last_cell.1), (0x1b8, g.move_state.0), (0x1cc, g.regen)] {
            add(o, le(v));
        }
    }
    add(0x1d0, le(g.weapons as i32));
    if full {
        for (o, v) in [(0x1e0, g.carrier), (0x1e4, g.partner), (0x1e8, g.paralysed), (0x1ec, g.seized)] {
            add(o, le(v));
        }
    }
    add(0xc, le(fb(rt.step.0)));
    add(0x10, le(fb(rt.step.1)));
    if full {
        add(0x12c, le(fb(rt.want_angle)));
    }
    add(0x18, le(fb(rt.aim)));
    if full {
        add(0x150, le(fb(rt.speed)));
        add(0x98, le(fb(rt.dir.0)));
        add(0x9c, le(fb(rt.dir.1)));
    }
    add(0x148, le(fb(rt.range)));
    add(0x11c, le(g.pods));
    if full {
        add(0x288, le(g.air));
        add(0x120, le(g.ammo));
    }
    for (j, &k) in bits.iter().enumerate() {
        items.push((format!("{u}.ammo[{j}]"), le(fb(rt.reload_left[k]))));
    }
    let mut add = |o: u32, v: Vec<u8>| items.push((format!("{u}+{o:#x}"), v));
    if full {
        add(0xe0, le(fb(rt.turn.0)));
    }
    add(0xe4, le(fb(rt.turn.1)));
    if full {
        add(0x1c0, le(fb(rt.angle)));
    }
    for (o, v) in [(0x1c4, fb(rt.turret_angle)), (0x228, rt.f228 as i32), (0x1c8, fb(rt.elevation)), (0x1d4, fb(g.pos.0)),
        (0x1d8, fb(g.pos.1)), (0x1dc, fb(rt.z))] {
        add(o, le(v));
    }
    for (j, &k) in bits.iter().enumerate() {
        items.push((format!("{u}.ammo2[{j}]"), le(fb(rt.reload[k]))));
    }
    let mut add = |o: u32, v: Vec<u8>| items.push((format!("{u}+{o:#x}"), v));
    add(0x1c, rt.basis.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect());
    add(0x244, le(g.index));
    add(0x248, le(rt.ai_group));
    let arr = |v: &[i32]| (0..n).flat_map(|k| v.get(k).copied().unwrap_or(0).to_le_bytes()).collect::<Vec<u8>>();
    add(0x154, arr(&g.hp));
    add(0x19c, arr(&g.origin));
    add(0x18c, le(g.flags2 as i32));
    if full {
        add(0x138, le(g.prev_size));
    }
    add(0x146, vec![g.tail.f146]);
    add(0x13c, g.tail.f13c.to_vec());
    if full {
        add(0x240, le(g.tail.f240 as i32));
    }
    add(0x24c, le(rt.ai_home));
    if full {
        add(0x250, vec![rt.laying]);
        add(0x254, le(rt.mine_cell.0));
        add(0x258, le(rt.mine_cell.1));
    }
    let (h0, h1) = g.last_hit.unwrap_or((0, 0));
    add(0x25c, le(h0));
    add(0x260, le(h1));
    add(0x238, le(g.building));
    if full {
        let mut b: Vec<u8> = rt.drop.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect();
        for v in [g.tail.f270, g.tail.f274, g.transport] {
            b.extend(v.to_le_bytes());
        }
        add(0x264, b);
    }
    add(0x130, vec![g.has_gun as u8]);
    add(0x134, le(g.hull));
    for (n, b) in items {
        put(n, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_round_trips() {
        let mut f = Field { alive: vec![false, true], units: vec![Group::default(), Group::default()], ..Default::default() };
        f.tail.absent = vec![false; 2];
        let g = &mut f.units[1];
        g.weapons = 0b10_0000_0000_0110;
        g.rt.reload_left[8] = 3.0;
        g.rt.reload[3] = 7.5;
        g.rt.reload[1] = 1.25;
        g.hp = vec![10, 20];
        g.origin = vec![-4, 5];
        g.rt.turn.0 = 0.65;
        g.tail.origin_ptr = 0xdead;
        g.last_hit = Some((10000, 3));
        f.lists.shells = vec![None, Some(Shell { vel: [1.0, 2.0, 3.0], pos: [4.0; 3], life: 5, target: 6, pull: 0.5, dmg: 7, by: 8, jitter: 1 })];
        f.weather = 2;
        f.background = 4;
        let c = encode(&f, "WF[3]");
        let back = decode(c.iter().map(|(n, b)| (n.as_str(), &b[..])), "WF[3]");
        assert_eq!(back, f);
        assert_eq!(encode(&back, "WF[3]"), c);
    }
}
