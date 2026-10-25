//! The battle screen's effects (re/screens/spacewar.md §6.3, §6.4, §6.7, §6.8): the five effect-particle sets
//! (explosions, hit flashes, engine trails, missile puffs), the projectile forms' look, the lightning, the shield rings,
//! the debris and the battle's sounds. Cosmetic: the exe
//! draws libc `rand()` for all of it; the client keeps its own stream. The tables (`maxhanyefektparticle`,
//! `fegyveralakok`, `hajtomuvek`) come from the user's `ig2.exe` (ADR 0004).

use std::collections::HashMap;

use ig2_rules::sounds::Table;
use ig2_sim::spacebattle::{shot_kind, Battle, BattleEvent, Shot, ShotRef, WeaponRules};

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

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}

/// `randomvektor(v, k)`: three `rand(100) − 50`, normalised, × k.
fn random_vector(r: &mut MsvcRand, k: f32) -> [f32; 3] {
    let v: [f32; 3] = std::array::from_fn(|_| r.random(100) as f32 - 50.0);
    normalise(v).map(|x| x * k)
}

/// `randommerolegesvektor(out, d, k)` (ELF 0x7c5a90): a random vector across `d`, of length k.
fn random_across(r: &mut MsvcRand, d: [f32; 3], k: f32) -> [f32; 3] {
    let v = random_vector(r, 1.0);
    normalise(cross(v, d)).map(|x| x * k)
}

/// A lightning bolt (`villam`, spec §6.8.4): the base points on the line A–B, the current points jittering about
/// them with their words, and the ribbon's look.
#[derive(Debug, Clone)]
pub struct Bolt {
    /// +0x1c width, +0x18 brightness, +0x3c amplitude (`J·W`).
    pub width: f32,
    bright: f32,
    amp: f32,
    /// +0x20 Hermite samples a span, +0x10 a flare at the end, +0x24 the texture's repeat, +0x28 its scroll.
    pub seg: usize,
    pub flare: bool,
    pub texscale: f32,
    texspeed: f32,
    /// +0x2c: the `rand(100)` at or above which the bolt is lit.
    frag: f32,
    base: Vec<[f32; 3]>,
    pub cur: Vec<([f32; 3], u32)>,
    vel: Vec<[f32; 3]>,
    dir: [f32; 3],
    /// +0x40 the texture offset, +0x30 the flare's size.
    pub tex: f32,
    pub flare_size: f32,
}

impl Bolt {
    /// `villam::create(n, W, I, J, seg, flare, 3, −0.05, 15, −1, 1, A, B)` (ELF 0x7c5c20).
    #[allow(clippy::too_many_arguments)]
    fn new(r: &mut MsvcRand, n: usize, w: f32, i: f32, j: f32, seg: usize, flare: bool, a: [f32; 3], b: [f32; 3]) -> Self {
        let amp = j * w;
        let dir = std::array::from_fn(|k| b[k] - a[k]);
        let mut base = vec![a; n];
        let mut cur = vec![(a, 0x80ff_ffff); n];
        let mut vel = vec![[0.0; 3]; n];
        base[n - 1] = b;
        cur[n - 1] = (b, 0x80ff_ffff);
        for p in 1..n - 1 {
            let off = random_across(r, dir, 1.0);
            let t = p as f32 / n as f32;
            base[p] = std::array::from_fn(|k| dir[k] * t + a[k]);
            cur[p] = (std::array::from_fn(|k| dir[k] * t + a[k] + off[k] * amp), 0x80ff_ffff);
            vel[p] = random_across(r, dir, amp / 6.0);
        }
        Self { width: w, bright: i, amp, seg, flare, texscale: 3.0, texspeed: -0.05, frag: 15.0, base, cur, vel, dir, tex: 0.0, flare_size: 0.0 }
    }

    /// `setvillampontok(A, B)` (ELF 0x7c6f20): new ends and base points; the jitter stays.
    fn set(&mut self, a: [f32; 3], b: [f32; 3]) {
        let n = self.base.len();
        self.dir = std::array::from_fn(|k| b[k] - a[k]);
        for p in 1..n - 1 {
            let t = p as f32 / n as f32;
            self.base[p] = std::array::from_fn(|k| self.dir[k] * t + a[k]);
        }
        self.cur[0] = (a, 0x80ff_ffff);
        self.cur[n - 1] = (b, 0x80ff_ffff);
        (self.base[0], self.base[n - 1]) = (a, b);
    }

    /// `villam::Refresh` (ELF 0x7c60c0), once a battle tick.
    fn refresh(&mut self, r: &mut MsvcRand) {
        let n = self.base.len();
        let lit = self.frag <= r.random(100) as f32;
        let word = |r: &mut MsvcRand, bright: f32| {
            let c = if lit { r.random(0x80) + 0x80 } else { 0 };
            let c = (c as f32 * bright) as u32;
            0x8000_0000 | c << 16 | c << 8 | c
        };
        for p in 1..n - 1 {
            for k in 0..3 {
                self.cur[p].0[k] += self.vel[p][k];
            }
            self.cur[p].1 = word(r, self.bright);
            if (self.amp * 2.0 * self.amp * 2.0) < dist2(self.cur[p].0, self.base[p]) {
                self.vel[p] = random_across(r, self.dir, self.amp / 6.0);
                let (c, b) = (self.cur[p].0, self.base[p]);
                self.cur[p].0 = std::array::from_fn(|k| (b[k] - c[k]) * 0.4 + c[k]);
            }
        }
        let w = word(r, self.bright);
        self.cur[0].1 = w;
        self.cur[n - 1].1 = w;
        self.tex += self.texspeed;
        while self.tex > 1.0 {
            self.tex -= 1.0;
        }
        while self.tex < -1.0 {
            self.tex += 1.0;
        }
        self.flare_size = self.width * 4.0 + r.random(100) as f32 / 100.0 * self.width * 2.0;
    }

    /// `villam::Update`'s curve (ELF 0x7c6590): `seg` cubic Hermite samples a span (Catmull-Rom tangents inside,
    /// `1.5·(p1 − p0) − 0.5·t1` at the ends), each with its span's first word, then the last point.
    pub fn samples(&self) -> Vec<([f32; 3], u32)> {
        let p = &self.cur;
        let n = p.len();
        if self.seg < 2 || n < 3 {
            return p.clone();
        }
        let sub = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| a[k] - b[k]) };
        let mut t = vec![[0.0f32; 3]; n];
        for i in 1..n - 1 {
            let (d0, d1) = (sub(p[i].0, p[i - 1].0), sub(p[i + 1].0, p[i].0));
            t[i] = std::array::from_fn(|k| d0[k] + (d1[k] - d0[k]) * 0.5);
        }
        let (e0, e1) = (sub(p[1].0, p[0].0), sub(p[n - 1].0, p[n - 2].0));
        t[0] = std::array::from_fn(|k| e0[k] * 1.5 - t[1][k] * 0.5);
        t[n - 1] = std::array::from_fn(|k| e1[k] * 1.5 - t[n - 2][k] * 0.5);
        let mut out = Vec::with_capacity((n - 1) * self.seg + 1);
        for i in 0..n - 1 {
            for s in 0..self.seg {
                let u = s as f32 / self.seg as f32;
                let (u2, u3) = (u * u, u * u * u);
                let (h0, h1, h2, h3) = ((u3 * 2.0 - u2 * 3.0) + 1.0, (u3 - u2 * 2.0) + u, u3 * -2.0 + u2 * 3.0, u3 - u2);
                out.push((std::array::from_fn(|k| p[i].0[k] * h0 + t[i][k] * h1 + p[i + 1].0[k] * h2 + t[i + 1][k] * h3), p[i].1));
            }
        }
        out.push(p[n - 1]);
        out
    }
}

/// A shield ring (`karikakirak`, spec §6.8.5): the tick it started (0 = free), the hit point, its facing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ring {
    pub start: i32,
    pub at: [f32; 3],
    pub dir: [f32; 3],
}

impl Ring {
    /// `karikafrissit`'s frame (all f32): `((40 − 800/((tick − start) + 20))/28)·35`.
    pub fn frame(&self, tick: i32) -> f32 {
        ((40.0f32 - 800.0 / ((tick - self.start) + 20) as f32) / 28.0) * 35.0
    }
}

/// A piece of debris (`robbanasdarabok`, spec §6.8.6): its model (`darab1..3`), size, place, spin, turn and counter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Debris {
    pub model: usize,
    pub size: f32,
    pub pos: [f32; 3],
    spin: [f32; 3],
    pub turn: crate::scene::M3,
    pub counter: i32,
}

impl Debris {
    /// `SetObjTransparency` while shown: 0 to counter 30, then `(c − 30)·0.02` (f64).
    pub fn transparency(&self) -> f32 {
        if self.counter <= 30 {
            0.0
        } else {
            ((self.counter - 30) as f64 * 0.02) as f32
        }
    }
}

/// What the loops of weapons 9..12 do (`lovedeksound`, spec §6.8.3): `key` names the shot's handle (`(form, slot,
/// handle)`), the volume is `ConvertVolume`'s argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loop {
    Start { key: (usize, usize, u8), id: SoundId },
    Volume { key: (usize, usize, u8), volume: i32 },
    End { key: (usize, usize, u8) },
}

/// A shot's loop state: its weapon (a reused slot ends the old loops) and the volumes `+0x90`, `+0x94`.
#[derive(Debug, Clone, Copy, Default)]
struct Looping {
    weapon: i32,
    handles: u8,
    v90: i32,
    v94: i32,
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
    /// Weapon 12's seven bolts per shot, the bolts it jumped to units and fighter groups with.
    pub shot_bolts: HashMap<(usize, usize), Vec<Bolt>>,
    pub unit_bolts: HashMap<usize, Bolt>,
    pub fighter_bolts: HashMap<usize, Bolt>,
    pub rings: [Ring; 20],
    pub debris: Vec<Debris>,
    loops: HashMap<(usize, usize), Looping>,
    /// The loop commands due since the caller last took them.
    pub loop_out: Vec<Loop>,
}

impl Fx {
    pub fn new(tables: Tables) -> Self {
        let sets = std::array::from_fn(|i| vec![Particle::default(); tables.pools[i]]);
        let mut rand = MsvcRand(1);
        // `initgfx`: each of the 40 nodes takes `darab{random(3) + 1}.xvb`.
        let one: crate::scene::M3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let debris = (0..40).map(|_| Debris { model: rand.random(3) as usize, size: 0.0, pos: [0.0; 3], spin: [0.0; 3], turn: one, counter: 0 }).collect();
        Self {
            tables,
            sets,
            cursors: [0; 5],
            rand,
            parttick: 0,
            sounds: Vec::new(),
            shot_bolts: HashMap::new(),
            unit_bolts: HashMap::new(),
            fighter_bolts: HashMap::new(),
            rings: [Ring::default(); 20],
            debris,
            loops: HashMap::new(),
            loop_out: Vec::new(),
        }
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
        let s = (1.0 + (self.random(20) as f32 + 80.0) / 2.0) * size;
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
                // `particlerobbanas5(1, 0.3)` (ELF 0x688270): a green glow left in place, 4..7 ticks (the 0.2 is f64).
                let s = 5.0 * 0.3;
                let dec = (0.2 - ((self.random(100) - 50) as f32 / 1000.0) as f64) as f32;
                self.push(3, Particle { pos: p, color: 0x005c_ff5c, base: s, size: s, life: 1.0, damp: 0.0, mult: 1.0, dec, ..Default::default() });
            }
            // 6, 7 are `glow` (they need the shot); 8 is the lightning.
            _ => {}
        }
    }

    /// `particlerobbanas6(1, size, p, c)` (ELF 0x688450): a faint red glow (`c` 255: (31, 15, 15)) fading over ~228 ticks.
    fn glow(&mut self, size: f32, p: [f32; 3], c: u32) {
        let s = 5.0 * size;
        let (g, r) = ((c << 4) >> 8, (c << 5) >> 8);
        let color = 0xff00_0000 | g << 16 | g << 8 | r;
        self.push(3, Particle { pos: p, color, base: s, size: s, life: 1.0, damp: 0.0, mult: 0.98, ..Default::default() });
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
                let p = [s.pos.x, s.pos.y, s.pos.z];
                match kind {
                    6 => {
                        if s.attached {
                            continue;
                        }
                        // Bigger as it nears the target: `0.5·d1/(d2 + d1/5)` (0.5 f64).
                        let Some((a, t)) = ends(b, s) else { continue };
                        let (d1, d2) = (dist2(a, t).sqrt(), dist2(p, t).sqrt());
                        self.glow((0.5 * (d1 / (d2 + d1 / 5.0)) as f64) as f32, p, 0xff);
                    }
                    7 => {
                        // Swelling and fading with the shot's age.
                        let Some(w) = weapons.get(s.weapon.max(0) as usize) else { continue };
                        let life = if s.kind == shot_kind::AT_GUN { w.duration * 10 } else { w.duration }.max(1);
                        let t = s.age as f32 * 255.0 / life as f32;
                        let a0 = self.tables.forms.get(w.form.max(0) as usize).map_or(0, |f| f.alpha);
                        let a = (a0 as f32 + (255 - a0) as f32 * t / 255.0) as i32;
                        let size = self.tables.forms.get(fi).map_or(0.0, |f| f.size) * (t / 36.0);
                        self.glow(size, p, (255 - a).clamp(0, 255) as u32);
                    }
                    _ => self.shot_puffs(kind, p),
                }
            }
        }
        self.bolts(b);
        for e in events {
            match *e {
                BattleEvent::Fired { shot, .. } => {
                    let Some(s) = b.pools.get(shot.form).and_then(|p| p.shots.get(shot.slot)) else { continue };
                    if let Some(snd) = fire_sound(s.weapon) {
                        self.sounds.push(snd);
                    }
                    let w = s.weapon;
                    if let Some(old) = self.loops.remove(&(shot.form, shot.slot)) {
                        for h in 0..old.handles {
                            self.loop_out.push(Loop::End { key: (shot.form, shot.slot, h) });
                        }
                    }
                    self.start_loops(shot, w);
                }
                BattleEvent::ShieldHit { at, .. } => {
                    // `karikakirak`: the first free ring at the hit point, facing the planet's centre.
                    let at = [at.x, at.y, at.z];
                    let c = b.planet_pos.map(|x| x as f32);
                    if let Some(r) = self.rings.iter_mut().find(|r| r.start == 0) {
                        *r = Ring { start: b.tick, at, dir: normalise(std::array::from_fn(|k| c[k] - at[k])) };
                    }
                }
                BattleEvent::FightersLost { fighter, .. } => {
                    // `VadaszRobban` (ELF 0x69c1a0): one puff whatever the count, no sound.
                    if let Some(g) = b.fighters.get(fighter) {
                        self.explosion(0, 0.07, [g.pos.x, g.pos.y, g.pos.z]);
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
        self.debris_tick();
        for r in &mut self.rings {
            if r.start != 0 && r.frame(b.tick) > 30.0 {
                r.start = 0;
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
                    // `robbanasdarabok` twice.
                    for _ in 0..2 {
                        let r = fx.random(100) as f32 / 100.0 * 5.0 + 1.0;
                        if div == 100.0 {
                            fx.piece(at, sc * r / 25.0, 1.5 / r);
                        } else {
                            fx.piece(at, sc * r / 18.0, 3.0 / r);
                        }
                    }
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

    /// `robbanasdarabok(p, size, speed)` (ELF 0x686830, exe 0x57f170): the first free piece, spinning.
    fn piece(&mut self, p: [f32; 3], size: f32, speed: f32) {
        let size = if size as f64 > 0.002 { 0.002 } else { size };
        let Some(i) = self.debris.iter().position(|d| d.counter == 0) else { return };
        let mut spin = [0.0f32; 3];
        for s in &mut spin {
            *s = (self.random(200) as f32 + 50.0) / 400.0 * speed;
        }
        for s in &mut spin {
            if self.random(2) != 0 {
                *s = -*s;
            }
        }
        // `ScaleObja` keeps the record's rotation `+0x5c`: a reused piece starts at the turn its last life left
        // (exe 0x57f170 has no `RotateObj`) [obs fx8 t2100: the exe's fresh pieces show every face, not one pose].
        let d = &mut self.debris[i];
        (d.size, d.pos, d.spin, d.counter) = (size, p, spin, 1);
    }

    /// `Darabok` (ELF 0x69a7d0): the shown pieces move by their spin and turn by it, until counter 80.
    fn debris_tick(&mut self) {
        for d in self.debris.iter_mut().filter(|d| d.counter != 0) {
            for k in 0..3 {
                d.pos[k] += d.spin[k];
            }
            d.turn = crate::scene::mul(&crate::scene::create_matrix(d.spin[0], d.spin[1], d.spin[2]), &d.turn);
            d.counter += 1;
            if d.counter > 80 {
                d.counter = 0;
            }
        }
    }

    /// The lightning (`SpaceWarLovedekMozgat`, `Mozgatas`, `SpaceVadaszMozgat`; spec §6.8.4), once a battle tick: weapon
    /// 12's shots keep seven bolts from the shooter to the target, the units and groups it jumped to (the sim's
    /// `lightning`) one from the target's bolt to them.
    fn bolts(&mut self, b: &Battle) {
        let live = |r: &ShotRef| b.pools.get(r.form).and_then(|p| p.shots.get(r.slot)).filter(|s| s.shooter.is_some() && s.weapon == 12 && s.kind == shot_kind::SHIP);
        self.shot_bolts.retain(|k, _| live(&ShotRef { form: k.0, slot: k.1 }).is_some());
        for (fi, pool) in b.pools.iter().enumerate() {
            for (si, s) in pool.shots.iter().enumerate() {
                if live(&ShotRef { form: fi, slot: si }).is_none() {
                    continue;
                }
                let Some((a, t)) = ends(b, s) else { continue };
                if !self.shot_bolts.contains_key(&(fi, si)) {
                    let r = &mut self.rand;
                    let mut v = vec![Bolt::new(r, 12, 1.0, 1.0, 1.666_666_6, 4, true, a, t)];
                    for k in 1..7 {
                        v.push(Bolt::new(r, 12, 1.0, if k < 4 { 0.2 } else { 0.1 }, 1.666_666_6, 3, false, a, t));
                    }
                    self.shot_bolts.insert((fi, si), v);
                }
                let v = self.shot_bolts.get_mut(&(fi, si)).unwrap();
                for bolt in v.iter_mut() {
                    bolt.set(a, t);
                    bolt.refresh(&mut self.rand);
                }
            }
        }
        let from = |fx: &Self, r: Option<ShotRef>| -> Option<([f32; 3], [f32; 3])> {
            let r = r?;
            let s = live(&r)?;
            let t = b.units.get(s.target)?;
            Some(([t.pos.x, t.pos.y, t.pos.z], fx.shot_bolts.get(&(r.form, r.slot))?[0].cur.get(9)?.0))
        };
        for (i, u) in b.units.iter().enumerate() {
            match from(self, u.lightning).filter(|_| u.alive) {
                Some((t, p9)) => {
                    let me = [u.pos.x, u.pos.y, u.pos.z];
                    if !self.unit_bolts.contains_key(&i) {
                        let bolt = Bolt::new(&mut self.rand, 10, 0.5, 0.5, 3.333_333_3, 4, true, t, me);
                        self.unit_bolts.insert(i, bolt);
                    }
                    let bolt = self.unit_bolts.get_mut(&i).unwrap();
                    bolt.set(p9, me);
                    bolt.refresh(&mut self.rand);
                }
                None => {
                    self.unit_bolts.remove(&i);
                }
            }
        }
        for (i, g) in b.fighters.iter().enumerate() {
            match from(self, g.lightning).filter(|_| g.alive) {
                Some((t, p9)) => {
                    let me = [g.pos.x, g.pos.y, g.pos.z];
                    if !self.fighter_bolts.contains_key(&i) {
                        let bolt = Bolt::new(&mut self.rand, 10, 0.2, 0.5, 10.0, 4, true, t, me);
                        self.fighter_bolts.insert(i, bolt);
                    }
                    let bolt = self.fighter_bolts.get_mut(&i).unwrap();
                    bolt.set(p9, me);
                    bolt.refresh(&mut self.rand);
                }
                None => {
                    self.fighter_bolts.remove(&i);
                }
            }
        }
    }

    /// `lovedeksound`'s loops (ELF 0x69c890, exe 0x57adb0; spec §6.8.3), once a rendered frame: weapons 9 and 10
    /// crossfade two loops with the attachment, 11 swells with the distance or the attachment's age, 12 hums; a shot
    /// that is gone ends its loops (`lovedeksoundend`).
    pub fn frame_loops(&mut self, b: &Battle, weapons: &[WeaponRules]) {
        let out = &mut self.loop_out;
        self.loops.retain(|&(f, s), l| {
            let keep = b.pools.get(f).and_then(|p| p.shots.get(s)).is_some_and(|x| x.shooter.is_some() && x.weapon == l.weapon);
            if !keep {
                for h in 0..l.handles {
                    out.push(Loop::End { key: (f, s, h) });
                }
            }
            keep
        });
        for (&(f, si), l) in self.loops.iter_mut() {
            let Some(s) = b.pools.get(f).and_then(|p| p.shots.get(si)) else { continue };
            let life = weapons.get(s.weapon.max(0) as usize).map_or(1, |w| w.duration);
            match s.weapon {
                9 | 10 => {
                    if !s.attached {
                        l.v94 = (l.v94 - 4).max(0);
                        l.v90 = (l.v90 + 4).min(70);
                    } else {
                        l.v90 = (l.v90 - 4).max(0);
                        l.v94 = (l.v94 + 4).min(70);
                    }
                    if life != 0 && life * 9 / 10 < s.age {
                        let cap = 70 - ((s.age - life * 9 / 10) * 700) / life;
                        l.v90 = l.v90.min(cap);
                        l.v94 = l.v94.min(cap);
                    }
                    out.push(Loop::Volume { key: (f, si, 0), volume: l.v90 });
                    out.push(Loop::Volume { key: (f, si, 1), volume: l.v94 });
                }
                11 => {
                    if !s.attached {
                        let Some((a, t)) = ends(b, s) else { continue };
                        let p = [s.pos.x, s.pos.y, s.pos.z];
                        let (d1, d2) = (dist2(a, t).sqrt(), dist2(p, t).sqrt());
                        l.v90 = ((d1 * 20.0 / (d2 + d1 / 5.0)) as i32).max(0);
                    } else {
                        let a = s.age - (b.tick - s.attached_tick);
                        let half = (life - a) / 2;
                        // The exe divides by zero here when `life − a < 2`.
                        l.v90 = if half == 0 || s.age - a < half { 100 } else { ((life - s.age) * 100) / half };
                    }
                    out.push(Loop::Volume { key: (f, si, 0), volume: l.v90 });
                }
                _ => out.push(Loop::Volume { key: (f, si, 0), volume: 80 }),
            }
        }
    }

    /// A new shot of weapon 9..12 starts its loops at volume 0 (`Duplicate`, `Play(h, 4)`).
    fn start_loops(&mut self, r: ShotRef, weapon: i32) {
        let recs: &[u16] = match weapon {
            9 | 10 => &[37, 38],
            11 => &[22],
            12 => &[14],
            _ => return,
        };
        for (h, &rec) in recs.iter().enumerate() {
            self.loop_out.push(Loop::Start { key: (r.form, r.slot, h as u8), id: SoundId::new(Table::SpWar, rec) });
            self.loop_out.push(Loop::Volume { key: (r.form, r.slot, h as u8), volume: 0 });
        }
        self.loops.insert((r.form, r.slot), Looping { weapon, handles: recs.len() as u8, v90: 0, v94: 0 });
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
/// volume written into its record (the loops of weapons 9..12: [`Fx::frame_loops`]).
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

/// A shot's shooter and target positions by its kind (`shot_kind`).
fn ends(b: &Battle, s: &Shot) -> Option<([f32; 3], [f32; 3])> {
    let unit = |i: usize| b.units.get(i).map(|u| [u.pos.x, u.pos.y, u.pos.z]);
    let fighter = |i: usize| b.fighters.get(i).map(|g| [g.pos.x, g.pos.y, g.pos.z]);
    let gun = |i: usize| b.guns.get(i).map(|g| [g.pos.x, g.pos.y, g.pos.z]);
    let sh = s.shooter?;
    match s.kind {
        shot_kind::SHIP => Some((unit(sh)?, unit(s.target)?)),
        shot_kind::FIGHTER_SHIP => Some((fighter(sh)?, unit(s.target)?)),
        shot_kind::GUN => Some((gun(sh)?, unit(s.target)?)),
        shot_kind::AT_GUN => Some((unit(sh)?, gun(s.target)?)),
        shot_kind::AT_FIGHTER => Some((unit(sh)?, fighter(s.target)?)),
        _ => Some((fighter(sh)?, fighter(s.target)?)),
    }
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
    fn a_shield_ring_lives_31_ticks() {
        // `karikafrissit`: the frame passes 30 between 30 and 31 ticks after the hit (spec §6.8.5).
        let r = Ring { start: 100, ..Default::default() };
        assert!(r.frame(130) <= 30.0);
        assert!(r.frame(131) > 30.0);
        assert_eq!(r.frame(100), 0.0);
    }

    #[test]
    fn the_glow_of_kind_6_is_a_faint_red() {
        let mut fx = Fx::new(Tables { pools: [4, 4, 4, 4, 4], ..Default::default() });
        fx.glow(1.0, [0.0; 3], 0xff);
        // `(c << 5) >> 8` red, `(c << 4) >> 8` green and blue (ELF 0x688450).
        assert_eq!(fx.sets[3][0].color & 0x00ff_ffff, 0x000f_0f1f);
    }

    #[test]
    fn a_bolt_keeps_its_ends_and_samples_its_spans() {
        let mut r = MsvcRand(1);
        let mut b = Bolt::new(&mut r, 12, 1.0, 1.0, 1.666_666_6, 4, true, [0.0; 3], [0.0, 120.0, 0.0]);
        b.refresh(&mut r);
        let s = b.samples();
        assert_eq!(s.len(), 11 * 4 + 1);
        assert_eq!(s[0].0, [0.0; 3]);
        assert_eq!(s.last().unwrap().0, [0.0, 120.0, 0.0]);
        // Every point within twice the amplitude of the line, after a refresh (`Refresh` pulls it back).
        for p in &b.cur {
            assert!(p.0[0].hypot(p.0[2]) <= 2.0 * 1.666_666_6 + 1e-3);
        }
    }

    #[test]
    fn debris_fades_out_after_80_ticks() {
        let mut fx = Fx::new(Tables { pools: [4, 4, 4, 4, 4], ..Default::default() });
        fx.piece([0.0; 3], 0.01, 1.0);
        assert_eq!(fx.debris.iter().filter(|d| d.counter != 0).count(), 1);
        let d = fx.debris.iter().position(|d| d.counter != 0).unwrap();
        assert_eq!(fx.debris[d].size, 0.002);
        let mut ticks = 0;
        while fx.debris[d].counter != 0 {
            fx.debris_tick();
            ticks += 1;
        }
        assert_eq!(ticks, 80);
    }

    /// `robbanasdarabok` (exe 0x57f170) sets scale, place, spin and counter but no rotation: a reused piece keeps the
    /// turn of its last life [obs fx8 dumps: the pieces' records at counter 2..7 are turned, not the identity].
    #[test]
    fn a_reused_piece_keeps_its_turn() {
        let mut fx = Fx::new(Tables { pools: [4, 4, 4, 4, 4], ..Default::default() });
        fx.piece([0.0; 3], 0.001, 1.0);
        let d = fx.debris.iter().position(|d| d.counter != 0).unwrap();
        while fx.debris[d].counter != 0 {
            fx.debris_tick();
        }
        let turn = fx.debris[d].turn;
        assert_ne!(turn, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        fx.piece([0.0; 3], 0.001, 1.0);
        assert_eq!((fx.debris[d].counter, fx.debris[d].turn), (1, turn));
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
