//! A real-time battle slot read back from the exe's memory (`SpwC`, exe 0x2D9C bytes per slot at 0xE417C0, spec
//! `re/rules/space-combat.md` §7): the regions `tools/trace/spwc.js` dumps (the slot `obj`, its units, fighters,
//! guns, the projectile pools, the AI squads, the strategic records, the globals `g<addr>`) decoded into a
//! [`Battle`]. The trace replay (`tests/spacebattle_trace.rs`) and the client's battle screen
//! (`ig2-client::spacewar_screen`, from a capture's dump) share it.

use std::collections::{BTreeMap, HashMap};

use crate::spacebattle::*;
use crate::spacewar::Design;


pub const UNIT: usize = 600;
pub const FIGHTER: usize = 0xa8;
pub const GUN: usize = 0xa8;
pub const SHOT: usize = 0xa8;
pub const SPRITE: usize = 0x4c;
pub const MOUNT: usize = 0x70;

pub fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
pub fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
pub fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub fn v4_at(b: &[u8], o: usize) -> V4 {
    V4 { x: f32_at(b, o), y: f32_at(b, o + 4), z: f32_at(b, o + 8), w: 0.0 }
}
pub fn v3_at(b: &[u8], o: usize) -> V4 {
    V4 { x: f32_at(b, o), y: f32_at(b, o + 4), z: f32_at(b, o + 8), w: 0.0 }
}
pub fn ptr(s: &str) -> u64 {
    u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}

/// Memory regions by name: (address, bytes).
/// Memory regions by name: (address, bytes).
#[derive(Clone, Default)]
pub struct Mem {
    pub r: BTreeMap<String, (u64, Vec<u8>)>,
}

impl Mem {
    pub fn get(&self, k: &str) -> &[u8] {
        &self.r[k].1
    }
    pub fn find(&self, prefix: &str) -> Option<(u64, &[u8])> {
        self.r.iter().find(|(k, _)| k.starts_with(prefix)).map(|(_, v)| (v.0, &v.1[..]))
    }
    pub fn global(&self, addr: u32) -> &[u8] {
        self.get(&format!("g{addr:x}"))
    }
}

/// The trace's static data.
/// The trace's static data.
pub struct Statics {
    pub weapons: Vec<u8>,
    /// Building types' hit points (exe 0x8EBA40, 0x24 bytes each).
    pub building_hp: Vec<i32>,
    pub types: HashMap<u64, ShipType>,
    pub designs: Vec<Vec<Design>>,
}

/// The trace's live weapon table (`fegyverek` as `InitSpaceWar` left it) and each record's 46 words.
pub fn weapons(st: &Statics) -> (Vec<WeaponRules>, Vec<Vec<i32>>) {
    let mut weapons = Vec::new();
    let mut words = Vec::new();
    for w in 0..40 {
        let r = &st.weapons[w * 0x5c..(w + 1) * 0x5c];
        let mut damage = [0; 12];
        for (k, d) in damage.iter_mut().enumerate() {
            *d = i32_at(r, 8 + 4 * k);
        }
        weapons.push(WeaponRules {
            kind: i32_at(r, 0),
            reload: i32_at(r, 4),
            damage,
            hit: i32_at(r, 0x38),
            spread: f32_at(r, 0x3c),
            turn: f32_at(r, 0x40),
            speed: f32_at(r, 0x44),
            accel: f32_at(r, 0x48),
            duration: i32_at(r, 0x4c),
            range: i32_at(r, 0x50),
            form: i32_at(r, 0x54),
            aimed: r[0x58] != 0,
            beam: r[0x5a] != 0,
        });
        // The words after a record are the next record's (an out-of-table damage read).
        words.push((0..46).map(|k| {
            let o = w * 0x5c + 4 * k;
            if o + 4 <= st.weapons.len() { i32_at(&st.weapons, o) } else { 0 }
        }).collect());
    }
    (weapons, words)
}

pub fn rules(mem: &Mem, st: &Statics) -> (BattleRules, Vec<Vec<i32>>) {
    let (weapons, words) = weapons(st);
    let forms = (0..40)
        .map(|f| {
            let b = mem.get(&format!("form{f}"));
            FormRules { loaded: b[0] != 0, slots: i32_at(b, 0x14).max(0) as usize, size: f32_at(b, 0x10) }
        })
        .collect();
    let bonus = mem.global(0xf27470);
    let story = mem.global(0xf2748c);
    let r = BattleRules {
        weapons,
        forms,
        multiplayer: mem.global(0xbae2c0)[0] != 0,
        story_hit_bonus: i32_at(bonus, 0).wrapping_add(i32_at(bonus, 4)),
        story_damage_bonus: i32_at(bonus, 8).wrapping_add(i32_at(bonus, 12)),
        paused: mem.global(0xbae264)[0] != 0,
        no_ai: i32_at(mem.global(0xb77244), 0) != 0,
        story_no_retreat_check: i32_at(story, 0) != 0,
        story_no_retreat: i32_at(story, 4) != 0,
        story_retreat_lock_a: mem.global(0xf6915d)[0] != 0,
        story_retreat_lock_b: mem.global(0xf66f2a)[0] != 0,
        fade: f32_at(mem.global(0xe72e14), 0),
    };
    (r, words)
}

/// Where the pools live, to turn projectile pointers into (form, slot).
pub fn pool_bases(mem: &Mem) -> Vec<(usize, u64, usize)> {
    let mut v = Vec::new();
    for (k, (p, b)) in &mem.r {
        if let Some(rest) = k.strip_prefix("pool") {
            let f: usize = rest.split('@').next().unwrap().parse().unwrap();
            v.push((f, *p, b.len() / SHOT));
        }
    }
    v
}

pub fn shot_ref(bases: &[(usize, u64, usize)], p: u64) -> Option<ShotRef> {
    if p == 0 {
        return None;
    }
    bases.iter().find(|&&(_, b, n)| b <= p && p < b + (n * SHOT) as u64).map(|&(f, b, _)| ShotRef { form: f, slot: ((p - b) / SHOT as u64) as usize })
}

pub fn index(base: u64, size: usize, p: u64) -> Option<usize> {
    (p != 0 && p >= base).then(|| ((p - base) / size as u64) as usize)
}

pub fn decode(mem: &Mem, st: &Statics) -> Battle {
    let o = mem.get("obj");
    let (ubase, units) = (mem.r["units"].0, mem.get("units"));
    let nu = i32_at(o, 0xc28).max(0) as usize;
    let fbase = mem.r.get("fighters").map_or(0, |r| r.0);
    let gbase = mem.r.get("guns").map_or(0, |r| r.0);
    let bases = pool_bases(mem);
    let mut b = Battle::default();
    b.tick = i32_at(o, 0xc4c);
    for i in 0..nu {
        let r = &units[i * UNIT..(i + 1) * UNIT];
        let kind = i32_at(r, 0x158);
        let target = u32_at(r, 0x154) as u64;
        let ships = mem.find(&format!("s{i}@")).map_or(Vec::new(), |(_, s)| s.chunks(4).map(|c| i32_at(c, 0)).collect());
        let mounts = mem.find(&format!("w{i}@")).map_or(Vec::new(), |(_, w)| {
            w.chunks(MOUNT)
                .map(|m| Mount {
                    weapon: i32_at(m, 0),
                    kind: i32_at(m, 4),
                    count: i32_at(m, 8),
                    timer: i32_at(m, 0xc),
                    timer_set: i32_at(m, 0x10),
                    reload: i32_at(m, 0x14),
                    damage4: i32_at(m, 0x28),
                    range: i32_at(m, 0x60),
                })
                .collect()
        });
        let mut waypoints = [[0; 4]; 4];
        for (k, w) in waypoints.iter_mut().enumerate() {
            for (j, x) in w.iter_mut().enumerate() {
                *x = i32_at(r, 0x74 + 16 * k + 4 * j);
            }
        }
        let cell4 = |a: usize| [i32_at(r, a), i32_at(r, a + 4), i32_at(r, a + 8), i32_at(r, a + 12)];
        let lightning = if u32_at(r, 0x244) != 0 { shot_ref(&bases, u32_at(r, 0x248) as u64) } else { None };
        b.units.push(Unit {
            alive: r[0] != 0,
            race: i32_at(r, 4),
            kind: i32_at(r, 8),
            design: i32_at(r, 0xc),
            model: i32_at(r, 0x10),
            owner: i32_at(r, 0x14),
            damage: i32_at(r, 0x18),
            hp: i32_at(r, 0x1c),
            shield_used: i32_at(r, 0x20),
            shield: i32_at(r, 0x24),
            ships,
            ship_count: i32_at(r, 0x1b0),
            hp_share: i32_at(r, 0x1b4),
            waypoint_count: i32_at(r, 0x2c),
            waypoints,
            pos: v4_at(r, 0x44),
            cell: cell4(0x54),
            dest: v4_at(r, 0x64),
            step: v4_at(r, 0xb4),
            step_cell: cell4(0xc4),
            vel: v3_at(r, 0xd4),
            speed: f32_at(r, 0x108),
            heading: v4_at(r, 0x110),
            mid_heading: v4_at(r, 0x120),
            plan_pos: v4_at(r, 0x130),
            plan_tick: i32_at(r, 0x140),
            f144: i32_at(r, 0x144),
            spin: [f32_at(r, 0x148), f32_at(r, 0x14c), f32_at(r, 0x150)],
            target: match kind {
                target_kind::GUN => index(gbase, GUN, target),
                _ => index(ubase, UNIT, target),
            },
            target_kind: kind,
            order: i32_at(r, 0x15c),
            turn: f32_at(r, 0x160),
            range_min: f32_at(r, 0x168),
            range_max: f32_at(r, 0x16c),
            radius: f32_at(r, 0x170),
            f174: f32_at(r, 0x174),
            exploding: i32_at(r, 0x17c),
            path_short: r[0x180] != 0,
            attacker: index(ubase, UNIT, u32_at(r, 0x190) as u64),
            attacker_dist2: f32_at(r, 0x194),
            picked: i32_at(r, 0x198),
            id: i32_at(r, 0x19c),
            experience: i32_at(r, 0x1a4),
            pods: u32_at(r, 0x1a8),
            look: i32_at(r, 0x1ac),
            weapon_mask: u32_at(r, 0x1bc),
            disabled: i32_at(r, 0x1c0),
            taken: i32_at(r, 0x1c4),
            original_owner: i32_at(r, 0x1c8),
            retreat_ticks: i32_at(r, 0x1cc),
            retreat_explodes: i32_at(r, 0x1d0) != 0,
            retreating: r[0x1d4] != 0,
            jammed: r[0x1d5] != 0,
            scanned: r[0x1d6] != 0,
            cloaked: r[0x1d7] != 0,
            cloak_cooldown: i32_at(r, 0x1d8),
            ship_type: st.types.get(&(u32_at(r, 0x214) as u64)).copied().unwrap_or_default(),
            mounts,
            idle: i32_at(r, 0x220),
            lightning,
            path_fails: i32_at(r, 0x254),
        });
    }
    if let Some((_, fb)) = mem.r.get("fighters") {
        for r in fb.chunks(FIGHTER).take(i32_at(o, 0xc2c).max(0) as usize) {
            let kind = i32_at(r, 0x38);
            let t = u32_at(r, 0x34) as u64;
            b.fighters.push(Fighter {
                alive: r[0] != 0,
                pos: v4_at(r, 4),
                dest: v4_at(r, 0x14),
                vel: v3_at(r, 0x24),
                dir: v3_at(r, 0x40),
                target: match kind {
                    1 => index(ubase, UNIT, t),
                    2 => index(fbase, FIGHTER, t),
                    _ => None,
                },
                target_kind: kind,
                turn: f32_at(r, 0x50),
                speed: f32_at(r, 0x54),
                goal_dist: f32_at(r, 0x58),
                goal_dist_prev: f32_at(r, 0x5c),
                state: i32_at(r, 0x60),
                jitter: i32_at(r, 0x64),
                timer: i32_at(r, 0x68),
                design: i32_at(r, 0x6c),
                owner: i32_at(r, 0x70),
                race: i32_at(r, 0x74),
                weapon: i32_at(r, 0x78),
                fleet: i32_at(r, 0x7c),
                count: i32_at(r, 0x80),
                hp: i32_at(r, 0x84),
                retreating: r[0x88] != 0,
                doomed: r[0x89] != 0,
                retreat_ticks: i32_at(r, 0x8c),
                lightning: if u32_at(r, 0x90) != 0 { shot_ref(&bases, u32_at(r, 0x94) as u64) } else { None },
            });
        }
    }
    if let Some((_, gb)) = mem.r.get("guns") {
        for r in gb.chunks(GUN).take(i32_at(o, 0xc90).max(0) as usize) {
            b.guns.push(Gun {
                pos: v4_at(r, 0),
                hits: i32_at(r, 0x10),
                weapon: i32_at(r, 0x14),
                timer: i32_at(r, 0x20),
                timer_set: i32_at(r, 0x24),
                reload: i32_at(r, 0x28),
                building: i32_at(r, 0x84),
                active: r[0x88] != 0,
                target: i32_at(r, 0x98),
                attacker: index(ubase, UNIT, u32_at(r, 0x9c) as u64),
                armour: f32_at(r, 0xa0),
            });
        }
    }
    b.pools = vec![Pool::default(); FORMS];
    for &(f, _, n) in &bases {
        let pool = mem.find(&format!("pool{f}@")).unwrap().1;
        let spr = mem.find(&format!("spr{f}@")).map(|x| x.1);
        for s in 0..n {
            let r = &pool[s * SHOT..(s + 1) * SHOT];
            let kind = i32_at(r, 0x1c);
            let (sh, tg) = (u32_at(r, 0x14) as u64, u32_at(r, 0x18) as u64);
            let shooter = match kind {
                shot_kind::FIGHTER_SHIP | shot_kind::FIGHTER_FIGHTER => index(fbase, FIGHTER, sh),
                shot_kind::GUN => index(gbase, GUN, sh),
                _ => index(ubase, UNIT, sh),
            };
            let target = match kind {
                shot_kind::AT_GUN => index(gbase, GUN, tg),
                shot_kind::AT_FIGHTER | shot_kind::FIGHTER_FIGHTER => index(fbase, FIGHTER, tg),
                _ => index(ubase, UNIT, tg),
            };
            let (sprite_pos, sprite_size) = spr.map_or((V4::default(), 0.0), |sp| (v3_at(sp, s * SPRITE), f32_at(sp, s * SPRITE + 0x14)));
            b.pools[f].shots.push(Shot {
                weapon: i32_at(r, 0),
                owner: i32_at(r, 4),
                count: i32_at(r, 8),
                mult: f32_at(r, 0xc),
                age: i32_at(r, 0x10),
                shooter,
                target: target.unwrap_or(usize::MAX),
                kind,
                pos: v4_at(r, 0x2c),
                vel: v3_at(r, 0x3c),
                dir: v3_at(r, 0x4c),
                turn: f32_at(r, 0x5c),
                speed: f32_at(r, 0x60),
                hits: r[0x64] != 0,
                attached: r[0x65] != 0,
                astray: r[0x66] != 0,
                attached_tick: i32_at(r, 0x68),
                straight: r[0x70] != 0,
                sprite_pos,
                sprite_size,
            });
        }
    }
    for e in 0..19 {
        let s = &o[e * 0x6c..(e + 1) * 0x6c];
        let n = i32_at(s, 4).max(0) as usize;
        let mut squads = Vec::new();
        if let Some((_, q)) = mem.find(&format!("sq{e}@")) {
            for g in 0..n {
                let r = &q[g * 0x1c..(g + 1) * 0x1c];
                let units = mem.find(&format!("sm{e}_{g}@")).map_or(Vec::new(), |(_, m)| {
                    m.chunks(4).take(i32_at(r, 0).max(0) as usize).map(|c| i32_at(c, 0) as usize).collect()
                });
                squads.push(Squad { state: i32_at(r, 4), centre: v3_at(r, 8), units });
            }
        }
        b.sides.push(Side {
            outgunned: s[0] != 0,
            centre: v3_at(s, 0x1c),
            rotation: [f32_at(s, 0x2c), f32_at(s, 0x30), f32_at(s, 0x34)],
            fighter_count: i32_at(s, 0x64),
            squads,
            fighter_aggression: i32_at(s, 0x40),
            ai_turns: i32_at(s, 0x4c),
            ai: s[0x50] != 0,
            strength: i32_at(s, 0x54),
            lost: i32_at(s, 0x58),
            may_retreat: s[0x68] != 0,
        });
    }
    for a in 0..19 {
        for c in 0..19 {
            b.hostile[a][c] = o[0xca0 + 19 * a + c];
        }
    }
    b.human = i32_at(o, 0xc3c);
    b.planet_owner = i32_at(o, 0xc8c);
    b.planet = i32_at(o, 0x1f28);
    b.cell = [f32_at(o, 0x804), f32_at(o, 0x808), f32_at(o, 0x80c)];
    b.grid = [i32_at(o, 0x810), i32_at(o, 0x814), i32_at(o, 0x818)];
    for k in 0..6 {
        b.bounds[k] = f32_at(o, 0x81c + 4 * k);
    }
    b.width = f32_at(o, 0x834);
    b.retreat_point = v4_at(o, 0x1f64);
    b.scale = f32_at(o, 0x1f88);
    b.shown = o[0x1f2c] != 0;
    b.ai_timer = f32_at(o, 0xc40);
    b.aim_timer = i32_at(o, 0xc44);
    b.winner = i32_at(o, 0xc48);
    b.end_timer = i32_at(o, 0xc50);
    b.invulnerable = i32_at(o, 0xc54) != 0;
    b.end_period = i32_at(o, 0xc58);
    b.pod_timer = i32_at(o, 0x1f84);
    b.victory = i32_at(o, 0x1f30) != 0;
    b.ended = o[0x1fb8] != 0;
    b.finished = o[0x1fba] != 0;
    b.ground_war = o[0x2004] != 0;
    b.planet_pos = [i32_at(o, 0x84c), i32_at(o, 0x850), i32_at(o, 0x854)];
    let np = i32_at(o, 0xe30).max(0) as usize;
    b.present.empires = (0..np.min(19)).map(|k| (o[0xe09 + k], o[0xe1c + k])).collect();
    let nf = i32_at(o, 0x1420).max(0) as usize;
    b.participants = (0..nf)
        .map(|k| Participant {
            owner: i32_at(o, 0xe98 + 0x30 * k),
            status: i32_at(o, 0xe9c + 0x30 * k),
            fleet: i32_at(o, 0xea0 + 0x30 * k),
            story_id: i32_at(o, 0xea4 + 0x30 * k),
            hp: [i32_at(o, 0xea8 + 0x30 * k), i32_at(o, 0xeac + 0x30 * k)],
        })
        .collect();
    b.active = o[0x1f2e] != 0;
    b.planet_size = i32_at(o, 0x858);
    b.map_pos = [f32_at(o, 0x1f78), f32_at(o, 0x1f7c)];
    for k in 0..19 {
        b.story_retreat[k] = i32_at(o, 0xe34 + 4 * k);
    }
    b.asteroids = (o[0x2d70] != 0).then(|| [i32_at(o, 0x2d74), i32_at(o, 0x2d78), i32_at(o, 0x2d7c)]);
    b.formation_choice = o[0x2d85] != 0;
    // The human's ship list (side `+0x5c` / `+0x60`), when the trace holds it (`--formation`'s snapshots).
    if let Some((_, l)) = mem.find("hlist@") {
        b.human_list = l
            .chunks_exact(0x1c)
            .map(|c| Entry { race: i32_at(c, 0), design: i32_at(c, 4), owner: i32_at(c, 8), ship: i32_at(c, 0xc), unit: i32_at(c, 0x14), experience: i32_at(c, 0x18) })
            .collect();
    }
    b.ground_war_started = o[0x2005] != 0;
    b.f2d84 = o[0x2d84] != 0;
    b.f2d86 = o[0x2d86] != 0;
    b.f2d98 = o[0x2d98] != 0;
    b.human_retreated = o[0x2d90] != 0;
    b.ai_retreated = o[0x2d91] != 0;
    for k in 0..19 {
        b.counts[k] = i32_at(o, 0x2b40 + 4 * k);
    }
    b
}
