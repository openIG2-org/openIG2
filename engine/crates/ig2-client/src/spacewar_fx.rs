//! The battle screen's effects (re/screens/spacewar.md §6.3, §6.4, §6.7): the five effect-particle sets (explosions,
//! hit flashes, engine trails, missile puffs), the projectile forms' look and the battle's sounds. Cosmetic: the exe
//! draws libc `rand()` for all of it; the client keeps its own stream. The tables (`maxhanyefektparticle`,
//! `fegyveralakok`, `hajtomuvek`) come from the user's `ig2.exe` (ADR 0004).

use ig2_rules::sounds::Table;
use ig2_sim::spacebattle::{Battle, BattleEvent, WeaponRules};

use crate::audio::SoundId;
use crate::starmap::MsvcRand;

/// `maxhanyefektparticle` (exe 0x926B20, 5 × i32): the sets' ring sizes.
const POOLS: u32 = 0x926B20;
/// `fegyveralakok` (exe 0x9256F8, 40 × 0x78).
const FORMS: (u32, usize, usize) = (0x9256F8, 0x78, 40);
/// `hajtomuvek` (exe 0x922188, `race·12 + type`, 0x3c bytes: the trail kind, then 4 thrusters × 3 f32).
const TRAILS: (u32, usize, usize) = (0x922188, 0x3c, 240);

/// The effect sets (`SpwC +0x1528 + 0x48·set`): 0 `animated.bmp`, 1 `anim2.bmp`, 2 `smoke.bmp` (25-frame 5 × 5 sheets),
/// 3 `star.bmp`, 4 `texcsor.bmp`.
pub const SET_TEXTURES: [&str; 5] = ["animated", "anim2", "smoke", "star", "texcsor"];
pub const SET_FRAMES: [usize; 5] = [25, 25, 25, 0, 0];

/// A projectile form's drawing fields (`fegyveralakok`, spec §1.4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Form {
    /// +0: the model (`vector/spacewar/<name>.xvb`), empty for an unused form.
    pub name: String,
    /// +0x10 sprite size, +0x20 alpha (a transparency), +0x24 animation period, +0x28 step, +0x2c kind (the spawner).
    pub size: f32,
    pub alpha: i32,
    pub period: f32,
    pub step: f32,
    pub kind: i32,
}

/// A thruster record (`hajtomuvek`): the trail kind and four offsets.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Trail {
    pub kind: i32,
    pub at: [[f32; 3]; 4],
}

/// The exe's tables the effects read.
#[derive(Debug, Clone, Default)]
pub struct Tables {
    pub pools: [usize; 5],
    pub forms: Vec<Form>,
    pub trails: Vec<Trail>,
}

impl Tables {
    pub fn read(install: &ig2_formats::Install) -> Result<Self, String> {
        let exe = std::fs::read(install.root().join("ig2.exe")).map_err(|e| e.to_string())?;
        let img = ig2_rules::PeImage::parse(&exe).map_err(|e| e.to_string())?;
        let read = |(va, size, n): (u32, usize, usize)| img.read(va, size * n).map(|b| b.to_vec()).map_err(|e| e.to_string());
        let i32_at = |b: &[u8], o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let f32_at = |b: &[u8], o: usize| f32::from_bits(i32_at(b, o) as u32);
        let p = read((POOLS, 4, 5))?;
        let pools = std::array::from_fn(|i| i32_at(&p, 4 * i).clamp(0, 4096) as usize);
        let forms = read(FORMS)?
            .chunks_exact(FORMS.1)
            .map(|r| Form {
                name: String::from_utf8_lossy(&r[..r[..16].iter().position(|&c| c == 0).unwrap_or(16)]).into_owned(),
                size: f32_at(r, 0x10),
                alpha: i32_at(r, 0x20),
                period: f32_at(r, 0x24),
                step: f32_at(r, 0x28),
                kind: i32_at(r, 0x2c),
            })
            .collect();
        // The exe stores the offsets as f32 (the ELF as ints): `Hajtomu` exe 0x567140.
        let trails = read(TRAILS)?.chunks_exact(TRAILS.1).map(|r| Trail { kind: i32_at(r, 0), at: std::array::from_fn(|k| std::array::from_fn(|j| f32_at(r, 4 + 12 * k + 4 * j))) }).collect();
        Ok(Self { pools, forms, trails })
    }
}

/// An effect particle (0x4c bytes; spec §6.7.1).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Particle {
    /// +0 position (battle units).
    pub pos: [f32; 3],
    /// +0xc `0xTTBBGGRR`, T a transparency; 0 = dead.
    pub color: u32,
    /// +0x10 base size, +0x14 size.
    pub base: f32,
    pub size: f32,
    /// +0x18 velocity, +0x2c frame, +0x30 life, +0x34 velocity damping, +0x38 life factor, +0x3c life decrement,
    /// +0x40 size-pulse amplitude, +0x44 its frequency, +0x48 its random range.
    pub vel: [f32; 3],
    pub frame: f32,
    pub life: f32,
    pub damp: f32,
    pub mult: f32,
    pub dec: f32,
    pub amp: f32,
    pub freq: f32,
    pub range: i32,
}

/// A sound the battle plays (`PlaySound(&SpWar_Hangok[i])`) with the volume written into its record first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sound {
    pub id: SoundId,
    pub volume: i32,
}

fn spwar(i: u16, volume: i32) -> Sound {
    Sound { id: SoundId::new(Table::SpWar, i), volume }
}

fn normalise(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 {
        v.map(|x| x / l)
    } else {
        v
    }
}

/// The battle's effects state, per battle slot shown.
#[derive(Debug, Clone)]
pub struct Fx {
    pub tables: Tables,
    pub sets: [Vec<Particle>; 5],
    cursors: [usize; 5],
    rand: MsvcRand,
    /// `parttick` (exe 0xE3E390): +1 per set updated.
    parttick: i32,
    /// The sounds due since the caller last took them.
    pub sounds: Vec<Sound>,
}

impl Fx {
    pub fn new(tables: Tables) -> Self {
        let sets = std::array::from_fn(|i| vec![Particle::default(); tables.pools[i]]);
        Self { tables, sets, cursors: [0; 5], rand: MsvcRand(1), parttick: 0, sounds: Vec::new() }
    }

    /// `random(n)`: no draw below 1.
    fn random(&mut self, n: i32) -> i32 {
        if n < 1 {
            0
        } else {
            self.rand.rand() % n
        }
    }

    fn push(&mut self, set: usize, p: Particle) {
        let n = self.sets[set].len();
        if n == 0 {
            return;
        }
        let c = self.cursors[set] % n;
        self.sets[set][c] = p;
        self.cursors[set] = (c + 1) % n;
    }

    /// Three `rand(1000) − 500` draws, normalised (`randomvektor`).
    fn random_dir(&mut self) -> [f32; 3] {
        let v: [f32; 3] = std::array::from_fn(|_| (self.random(1000) - 500) as f32);
        normalise(v)
    }

    /// `ujparticlerobbanas(set, size, p)` (ELF 0x688a50, exe 0x5820c0): a white explosion sprite at `p`, still.
    pub fn explosion(&mut self, set: usize, size: f32, p: [f32; 3]) {
        let s = (1.3f64 * size as f64) as f32 * 11.0 * 4.0;
        let _ = self.random_dir();
        self.push(set, Particle { pos: p, color: 0x00ff_ffff, base: s, size: s, life: 1.0, damp: 0.88, mult: 0.98, dec: 0.01, ..Default::default() });
    }

    /// `particlerobbanas7(1, size, p, d)` (ELF 0x688630): an engine-trail puff in set 3 drifting along `d`.
    fn trail_puff(&mut self, size: f32, p: [f32; 3], d: [f32; 3]) {
        let pos = std::array::from_fn(|i| p[i] + d[i] * self.random(100) as f32 * size);
        let s = (1 + (self.random(20) + 80) / 2) as f32 * size;
        self.push(3, Particle { pos, color: 0x00ff_ffff, base: s, size: s, vel: d, life: 1.0, damp: 0.99, mult: 0.6, ..Default::default() });
    }

    /// The puffs a flying projectile of form kind `kind` leaves each tick (`SpaceWarLovedekKirajzol`'s jump table,
    /// spec §6.3.1 step 6).
    fn shot_puffs(&mut self, kind: i32, p: [f32; 3]) {
        match kind {
            1 => {
                // `particlerobbanas(5, 0.04, 0.3, 1.0)`: one orange puff, then four grey ones.
                let l = 0.04;
                let color = ((self.random(64) as u32) << 16) | 0x50c0;
                let base = (self.random(200) / 20 + 1) as f32 * l;
                let d = self.random_dir();
                let speed = (self.random(100) as f32 / 100.0 + 1.5) * l;
                let dec = 0.04 - (self.random(100) - 50) as f32 / 10000.0;
                let freq = self.random(100) as f32 / 200.0 * 3.0;
                self.push(3, Particle { pos: p, color, base, size: base, vel: d.map(|x| x * speed), life: 1.0, damp: 0.88, mult: 0.99, dec, amp: 0.2, freq, ..Default::default() });
                for _ in 0..4 {
                    let base = (self.random(100) / 20 + 1) as f32 * l;
                    let d = self.random_dir();
                    let speed = (self.random(100) as f32 / 100.0 + 0.1) * l;
                    let dec = 0.01 - (self.random(100) - 50) as f32 / 10000.0;
                    let freq = self.random(100) as f32 / 200.0 * 3.0;
                    self.push(3, Particle { pos: p, color: 0x0034_4040, base, size: base, vel: d.map(|x| x * speed), life: 1.0, damp: 0.94, mult: 0.98, dec, amp: 0.2, freq, ..Default::default() });
                }
            }
            2..=4 => {
                // `particlerobbanas2..4`: white sparks in set 4.
                let (n, l, damp, mult) = match kind {
                    2 => (3, 0.4, 0.94, 0.96),
                    3 => (5, 0.05, 0.99, 0.96),
                    _ => (5, 0.1, 0.99, 0.89),
                };
                for _ in 0..n {
                    let base = (1.0 + self.random(100) as f32 / 20.0) * l;
                    let d = self.random_dir();
                    let r = self.random(if kind == 2 { 100 } else { 1000 }) as f32;
                    let speed = match kind {
                        2 => (r / 100.0 + 0.1) * l,
                        3 => (r / 100.0 + 0.1) * l,
                        _ => (r / 600.0 + 0.1) * l,
                    };
                    let vel = d.map(|x| x * speed);
                    let pos = if kind == 4 { std::array::from_fn(|i| p[i] + vel[i]) } else { p };
                    let dec = 0.01 - (self.random(100) - 50) as f32 / 10000.0;
                    self.push(4, Particle { pos, color: 0x00ff_ffff, base, size: base, vel, life: 1.0, damp, mult, dec, ..Default::default() });
                }
            }
            5 => {
                // `particlerobbanas5`: a green glow left in place.
                let s = 5.0 * 0.3;
                let dec = 0.05 - (self.random(100) - 50) as f32 / 1000.0;
                self.push(3, Particle { pos: p, color: 0x005c_ff5c, base: s, size: s, life: 1.0, damp: 0.0, mult: 1.0, dec, ..Default::default() });
            }
            // 6, 7 (`particlerobbanas6`) not read; 8 is the lightning.
            _ => {}
        }
    }

    /// One battle tick (`SpaceWarFrissit`'s visible part): the projectiles' puffs, the hits (`Talalat`), the engine
    /// trails (`Hajtomu`), the dying ships (`urhajomeghalas`), then `ParticleFrissit` on the five sets. `matrices` are
    /// the units' node matrices, `toward_eye` the unit vector from the battle towards the camera.
    pub fn tick(&mut self, b: &Battle, weapons: &[WeaponRules], events: &[BattleEvent], matrices: &[crate::scene::M3], toward_eye: [f32; 3]) {
        // The projectiles in flight.
        for (fi, pool) in b.pools.iter().enumerate() {
            let kind = self.tables.forms.get(fi).map_or(0, |f| f.kind);
            if kind == 0 {
                continue;
            }
            for s in pool.shots.iter().filter(|s| s.shooter.is_some()) {
                if kind == 6 && s.attached {
                    continue;
                }
                self.shot_puffs(kind, [s.pos.x, s.pos.y, s.pos.z]);
            }
        }
        for e in events {
            match *e {
                BattleEvent::Fired { shot, .. } => {
                    let Some(s) = b.pools.get(shot.form).and_then(|p| p.shots.get(shot.slot)) else { continue };
                    if let Some(snd) = fire_sound(s.weapon) {
                        self.sounds.push(snd);
                    }
                }
                BattleEvent::Hit { unit, damage, beam, weapon } => {
                    let Some(u) = b.units.get(unit) else { continue };
                    if !beam {
                        // `Talalat` (ELF 0x69bae0): a flash near the target, `animated` or `anim2`.
                        let sc = u.ship_type.scale;
                        let p: [f32; 3] = [u.pos.x, u.pos.y, u.pos.z];
                        let p = p.map(|x| x + (self.random(100) - 50) as f32 * sc * 10.0);
                        let k = self.random(2) as usize;
                        let f = if k == 1 { 0.5 } else { 1.0 };
                        let size = (sc * 10.0).min(sc * 30.0 * damage as f32 / 30.0 * f);
                        self.explosion(k, size, p);
                        // `lovedeksoundtalalat` (ELF 0x69c0b0): Chainblast6 or Chainblast1 at 60.
                        let i = if self.random(2) == 0 { 13 } else { 8 };
                        self.sounds.push(spwar(i, 60));
                    } else if weapon == 11 {
                        self.sounds.push(spwar(23, 100));
                    }
                }
                _ => {}
            }
        }
        let _ = weapons;
        // `Hajtomu` (ELF 0x69a9c0, exe 0x567140).
        for (i, u) in b.units.iter().enumerate() {
            if !u.alive || u.cloaked {
                continue;
            }
            let Some(t) = self.tables.trails.get((u.race * 12 + u.kind).max(0) as usize).copied() else { continue };
            let m = matrices.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
            let sc = u.ship_type.scale;
            let p = [u.pos.x, u.pos.y, u.pos.z];
            let th = |k: usize, mirror: bool| {
                let a = t.at[k];
                let v = [a[1] * sc * 1000.0 * if mirror { -1.0 } else { 1.0 }, -a[2] * sc * 1000.0, a[0] * sc * 1000.0];
                let r = crate::scene::apply(&m, v);
                [p[0] + r[0], p[1] + r[1], p[2] + r[2]]
            };
            let d = crate::scene::apply(&m, [0.0, -0.1, 0.0]);
            let pts: Vec<[f32; 3]> = match t.kind {
                1 => vec![th(0, false), th(1, false)],
                2 => (0..4).map(|k| th(k, false)).collect(),
                3 => (0..4).map(|k| th(k, false)).chain((0..4).map(|k| th(k, true))).collect(),
                4 => (0..3).map(|k| th(k, false)).collect(),
                5 => vec![th(0, false)],
                _ => Vec::new(),
            };
            for q in pts {
                self.trail_puff(sc * 4.0, q, d);
            }
        }
        // `urhajomeghalas` (ELF 0x6a1690, exe 0x582550; spec §6.4).
        for u in b.units.iter().filter(|u| !u.alive && u.exploding > 0) {
            let c = u.exploding;
            let sc = u.ship_type.scale;
            let q = toward_eye.map(|x| x * sc * 400.0);
            let p = [u.pos.x, u.pos.y, u.pos.z];
            let blast = |fx: &mut Self| {
                let i = 8 + fx.random(6) as u16;
                fx.sounds.push(spwar(i, 0x37));
            };
            if !u.retreating {
                let near = |fx: &mut Self, div: f32, qk: f32, size: f32, set_pick: bool| {
                    let at: [f32; 3] = std::array::from_fn(|i| p[i] + (fx.random(100) - 50) as f32 / div + q[i] * qk);
                    let set = if set_pick {
                        let _ = fx.random(2);
                        fx.random(2) as usize
                    } else {
                        0
                    };
                    fx.explosion(set, size, at);
                    blast(fx);
                };
                if c < 10 {
                    if self.random(100) < 40 {
                        near(self, 100.0, 1.0, sc * 0.05 * 700.0, false);
                    }
                } else if (16..=17).contains(&c) {
                    near(self, 80.0, 1.0, sc * 0.1 * 700.0, false);
                } else if c == 20 {
                    let i = self.random(3) as u16;
                    self.sounds.push(spwar(i, 100));
                } else if (26..=45).contains(&c) && self.random(100) < 40 {
                    near(self, 50.0, 2.0, sc * 0.05 * 700.0, true);
                }
            } else if c < 2 {
                self.explosion(0, sc * 0.4 * 700.0, p);
                blast(self);
            } else if c < 20 {
                self.explosion(0, 1.0 / (c + 5) as f32 * sc * 700.0, p);
                blast(self);
            }
        }
        for set in 0..5 {
            self.update(set);
        }
    }

    /// `ParticleFrissit(set)` (ELF 0x5f43a0, exe 0x52c500; spec §6.7.1).
    fn update(&mut self, set: usize) {
        self.parttick = self.parttick.wrapping_add(1);
        let tick = self.parttick as f32;
        for i in 0..self.sets[set].len() {
            let mut p = self.sets[set][i];
            p.frame = (p.frame as f64 + 0.7) as f32;
            if p.color != 0 {
                for k in 0..3 {
                    p.pos[k] += p.vel[k];
                    p.vel[k] *= p.damp;
                }
                p.life = p.life * p.mult - p.dec;
                let t = ((1.0 - p.life) * 255.0) as i32 as u8 as u32;
                p.color = (p.color & 0x00ff_ffff) | t << 24;
                if (p.life as f64) < 0.01 {
                    p.life = 0.0;
                    p.color = 0;
                }
                let r = if p.range > 0 { self.rand.rand() % p.range } else { 0 };
                let arg = tick * p.freq + (r / 1000) as f32;
                p.size = ((arg as f64).sin() as f32 * p.base) * p.amp + p.base;
            }
            self.sets[set][i] = p;
        }
    }
}

/// `lovedeksound` on a new projectile (ELF 0x69c890, exe 0x57adb0; spec §6.7.3): the weapon's firing sound and the
/// volume written into its record. The looped hums of weapons 9..12 are not played yet.
pub fn fire_sound(weapon: i32) -> Option<Sound> {
    Some(match weapon {
        0..=4 => spwar(29 + weapon as u16, 70),
        5 | 6 => spwar(35, 70),
        7 => spwar(26, 70),
        8 => spwar(34, 70),
        9 => spwar(19, 80),
        10 => spwar(36, 80),
        11 => spwar(21, 80),
        13..=15 => spwar(39 + (weapon - 13) as u16, 80),
        16 | 18 => spwar(24, 80),
        17 | 19 => spwar(25, 80),
        30 => spwar(29, 60),
        _ => return None,
    })
}

/// A projectile's sprite word and size this tick (`SpaceWarLovedekKirajzol` steps 2–4, spec §6.3.1): `(word, size)`;
/// size 0 = not drawn (a free slot, weapon 12's arc).
pub fn shot_look(s: &ig2_sim::spacebattle::Shot, weapons: &[WeaponRules], forms: &[Form], form_size: f32, tick: i32) -> (u32, f32) {
    let Some(w) = weapons.get(s.weapon.max(0) as usize) else { return (0, 0.0) };
    let mut t = w.duration;
    if s.kind == ig2_sim::spacebattle::shot_kind::AT_GUN {
        t *= 10;
    }
    let t = t.max(1);
    let age = s.age;
    let a0 = forms.get(w.form.max(0) as usize).map_or(0, |f| f.alpha);
    if !w.beam {
        if s.weapon == 12 {
            return (0, 0.0);
        }
        let a = if age < t / 2 { a0 } else { a0 + (255 - a0) * ((age - t / 2) * 0x1ff / t) / 255 };
        return ((a.clamp(0, 255) as u32) << 24 | 0xff_ffff, form_size);
    }
    let a = if s.weapon == 8 || s.weapon == 11 || age < t * 9 / 10 { a0 } else { a0 + (255 - a0) * ((age - t * 9 / 10) * 0x9f6 / t) / 255 };
    let word = (a.clamp(0, 255) as u32) << 24 | 0xff_ffff;
    if !s.attached {
        return (word, form_size);
    }
    if s.weapon == 11 {
        let d = age - (tick - s.attached_tick);
        let x = if (age - d) < (t - d) / 2 { age - d } else { (t - d) - (age - d) };
        let x = x as f32 / 20.0;
        return (word, 80.0 - (80.0 - form_size) / (x + 1.0));
    }
    (word, form_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explosion_fades_out_in_54_ticks() {
        let mut fx = Fx::new(Tables { pools: [4, 4, 4, 4, 4], ..Default::default() });
        fx.explosion(0, 1.0, [0.0; 3]);
        let mut ticks = 0;
        while fx.sets[0][0].color != 0 {
            fx.update(0);
            ticks += 1;
        }
        // life = life·0.98 − 0.01 falls under 0.01 on the 54th update (spec §6.7.1).
        assert_eq!(ticks, 54);
    }

    #[test]
    fn the_laser_fires_laser01() {
        // [obs: s4 and s4fx, `SpWar_Hangok[29]` Laser01 when the human's lasers (weapon 0) fire at t1015]
        assert_eq!(fire_sound(0), Some(spwar(29, 70)));
        assert_eq!(fire_sound(12), None);
    }

    #[test]
    fn a_trail_puff_lives_ten_ticks() {
        let mut fx = Fx::new(Tables { pools: [4, 4, 4, 4, 4], ..Default::default() });
        fx.trail_puff(0.01, [0.0; 3], [0.0, -0.1, 0.0]);
        let mut ticks = 0;
        while fx.sets[3][0].color != 0 {
            fx.update(3);
            ticks += 1;
        }
        assert_eq!(ticks, 10);
    }
}
