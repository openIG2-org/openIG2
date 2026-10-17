//! `FRISSIT_MI_DESIGN(e, all)` (exe 0x50abf0): the AI's design per slot, with `TervNew`, `TervClear` and
//! `TervAutoDesign` (`re/rules/ai-empire.md` §10).

use super::{AiEvent, AiHooks, AiState, World, BEST, DESIGN_SLOTS, SPECIAL};
use crate::spacewar::Design;

/// Hull slots 1..=24 (`Alapok[1..25]`).
const HULL_SLOTS: usize = DESIGN_SLOTS - 1;
/// The colony-ship and terraformer slots, designed only while there are planets to take.
const COLONY_SLOTS: [usize; 2] = [13, 14];
/// Designs below this index are the scenario's (`Tervek0`); the AI's own start here.
const FIRST_OWN: usize = 0x29;
/// Ground hull techs.
const GROUND_HULLS: std::ops::RangeInclusive<i32> = 0x91..=0x95;
/// `TervAutoDesign`'s per-race allow lists (`9·variant + race`) for shields 0x31..=0x36, techs 0x41..=0x43 and
/// 0x44..=0x45.
const SHIELDS_OK: [u8; 18] = [0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0];
const T41_OK: [u8; 18] = [0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 1];
const T44_OK: [u8; 18] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0];

fn rd_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl World<'_> {
    /// `OsszHanyTerv[e]`: the highest design index of `e`.
    pub(crate) fn design_count(&self, e: usize) -> usize {
        self.spy.combat.designs.get(e).map_or(0, |t| t.len().saturating_sub(1))
    }

    /// Edit design `d` of `e` as raw bytes (fields re-read afterwards).
    fn edit_design(&mut self, e: usize, d: usize, f: impl FnOnce(&mut [u8; 0x84])) {
        if let Some(x) = self.spy.combat.designs.get_mut(e).and_then(|t| t.get_mut(d)) {
            let mut raw = x.to_bytes();
            f(&mut raw);
            *x = Design::from_bytes(&raw);
        }
    }

    /// `TervUjraSzamol(e, d, false, true)` (`space-combat.md` §2.4).
    fn recompute_design(&mut self, e: usize, d: usize) {
        let race = self.race(e);
        let rules = &self.rules.design;
        if let Some(x) = self.spy.combat.designs.get_mut(e).and_then(|t| t.get_mut(d)) {
            x.recompute(rules, race, true);
            x.raw = x.to_bytes();
        }
    }

    /// `TervClear(e, d)` (§10.2).
    fn clear_design(&mut self, e: usize, d: usize) {
        self.edit_design(e, d, crate::spacewar::clear_design_record);
    }

    /// `TervNew(e, id)` (§10.2): the lowest deleted design, else a new record at the end. Returns its index.
    fn new_design(&mut self, e: usize, id: i32, hooks: &mut dyn AiHooks) -> usize {
        let n = self.design_count(e);
        let free = (1..=n).find(|&k| self.des(e, k as i32).is_some_and(|x| rd_i16(&x.raw, 0x16) < 0));
        let d = match free {
            Some(k) => k,
            None => {
                let raw = hooks.fresh_design(e, n + 1);
                let t = &mut self.spy.combat.designs[e];
                if t.is_empty() {
                    t.push(Design::from_bytes(&[0; 0x84]));
                }
                t.push(Design::from_bytes(&raw));
                t.len() - 1
            }
        };
        self.edit_design(e, d, |r| r[0] = 0);
        self.clear_design(e, d);
        let owner = if id > 0 { id as u16 } else { self.race(e) as u16 };
        self.edit_design(e, d, |r| {
            r[0x18] = 1;
            r[0x40..0x42].copy_from_slice(&owner.to_le_bytes());
        });
        d
    }

    /// `TervezAutomatikus(e, t)` (exe 0x4b72b0, ELF 0x61bc80; `TalalmanyKifejlesztve` calls it for a human's tech with
    /// `+0x26`/`+0x28` set, re/screens/research.md §6): when `t` is the tech of a hull `Alapok[1..25]` (the first),
    /// a new design of that hull named after the tech (`strcpy` of the tech's name), recomputed, fitted with
    /// `TervAutoDesign(e, d, 0)` and recomputed again. The name is the exe's localised one: reported as
    /// [`AiEvent::DesignNamed`].
    pub fn automatic_design(&mut self, e: usize, t: usize, hooks: &mut dyn AiHooks) {
        let Some(h) = (1..=HULL_SLOTS).find(|&h| self.rules.design.hulls.get(h).is_some_and(|x| x.tech == t as i32)) else { return };
        let d = self.new_design(e, 0, hooks);
        let name = self.spy.colony.tech.name_bytes(t);
        self.edit_design(e, d, |r| {
            r[0x44..0x48].copy_from_slice(&(h as i32).to_le_bytes());
            let n = name.len().min(r.len());
            r[..n].copy_from_slice(&name[..n]);
        });
        self.recompute_design(e, d);
        self.auto_design(e, d, 0);
        self.recompute_design(e, d);
        self.events.push(AiEvent::DesignNamed { empire: e as u8, design: d, slot: h, special: false });
    }

    /// `TalalmanyRarakhato(h, t, k, min, e, pt)` (§10.3): tech `t` fits slot `k` of hull `h` for pod table `pt`.
    fn tech_fits(&self, h: usize, t: usize, k: usize, min: u8, e: usize, pt: i16) -> bool {
        if (0xa1..=0xa9).contains(&t) && self.ctx.design_debug {
            return true;
        }
        if self.spy.colony.tech.status(t, e) < min {
            return false;
        }
        let rules = &self.rules.design;
        let table = rules.race_pod_table.get(pt.max(0) as usize).copied().unwrap_or(0) - 1;
        let Some(&kind) = rules.pod_slot_types.get(table.max(0) as usize).and_then(|x| x.get(h)).and_then(|x| x.get(k.wrapping_sub(1)))
        else {
            return false;
        };
        self.rules.pods.iter().find(|p| p.id == kind).is_some_and(|p| p.techs.contains(&(t as i32)))
    }

    /// `GetMaxRarakhato(pt, h, k)`.
    fn slot_max(&self, pt: i16, h: usize, k: usize) -> i32 {
        let table = self.rules.design.race_pod_table.get(pt.max(0) as usize).copied().unwrap_or(0) - 1;
        self.rules.pod_slot_max.get(table.max(0) as usize).and_then(|x| x.get(h)).and_then(|x| x.get(k.wrapping_sub(1))).copied().unwrap_or(0)
    }

    /// `TervAutoDesign(e, d, variant)` (§10.3): the best tech the empire has for every slot, as many as fit.
    fn auto_design(&mut self, e: usize, d: usize, variant: usize) {
        let Some(x) = self.des(e, d as i32) else { return };
        let mut raw = x.to_bytes();
        let race = match self.race(e) {
            r @ 0..=8 => r as usize,
            _ => 1,
        };
        let hull = rd_i32(&raw, 0x44);
        let pt = rd_i16(&raw, 0x40);
        let hu = hull.max(0) as usize;
        let hull_tech = self.rules.design.hulls.get(hu).map_or(0, |h| h.tech);
        let tech = &self.spy.colony.tech;
        let allow = |table: &[u8; 18]| table[9 * variant + race] != 0;
        let mut k = 1;
        while k as i32 <= self.rules.design.slot_count(pt, hu) {
            raw[0x50 + k] = 0;
            let (mut best, mut pick) = (-1000, 0);
            for t in 1..=192 {
                if tech.status(t, e) != 5 {
                    continue;
                }
                if e == self.spy.ctx.human_empire && t == 0x12 && hull_tech == 5 {
                    self.events.push(AiEvent::Tutorial { id: 0xb });
                }
                let mut s = tech.byte(t, 0x3c + race) as i32 + 100;
                if variant != 0 && GROUND_HULLS.contains(&hull_tech) {
                    s = self.rules.ground_priority.get(t * 9 + race).copied().unwrap_or(0) as i32 + 100;
                }
                if variant == 0 && (0x1a..=0x1d).contains(&t) {
                    s -= 100;
                }
                if !allow(&SHIELDS_OK) && (0x31..=0x36).contains(&t) {
                    s -= 100;
                }
                if !allow(&T41_OK) && (0x41..=0x43).contains(&t) {
                    s -= 100;
                }
                if !allow(&T44_OK) && (0x44..=0x45).contains(&t) {
                    s -= 100;
                }
                if s > 0 && self.tech_fits(hu, t, k, 5, e, pt) && best <= s {
                    best = s;
                    pick = t;
                }
            }
            if pick > 0 {
                raw[0x50 + k] = pick as u8;
            }
            let have = tech.byte(raw[0x50 + k] as usize, 0x24);
            let max = self.slot_max(pt, hu, k);
            raw[0x5d + k] = if max < have as i32 { max as u8 } else { have };
            k += 1;
        }
        self.edit_design(e, d, |r| *r = raw);
    }

    /// Queue lines of `e`'s planets building design `d` first get its new build time (§10.1 `queue_retime`).
    fn retime_queues(&mut self, e: usize, d: usize, old: i32) {
        let new = self.des_i32(e, d as i32, 0x20);
        for p in self.spy.colony.planets.iter_mut().skip(1).filter(|p| p.owner as usize == e) {
            for l in 0..2 {
                let q = &mut p.production;
                if rd_i16(q, 8 * l) as i32 == d as i32 {
                    let o = 0x28 + 4 * l;
                    let left = rd_i32(q, o).wrapping_sub(old).wrapping_add(new).max(0);
                    q[o..o + 4].copy_from_slice(&left.to_le_bytes());
                }
            }
        }
    }
}

impl AiState {
    /// `FRISSIT_MI_DESIGN(e, all)` (§10): a design for every hull slot the empire can build (slots 13 and 14
    /// only while there are planets to take, or with `all`), redone while upgrades are due; then every fleet is
    /// recounted.
    pub fn design(&mut self, w: &mut World, e: usize, all: bool, hooks: &mut dyn AiHooks) {
        if w.star_ctx.human_controlled.get(e).copied().unwrap_or(false) || w.ctx.ai_off {
            return;
        }
        let mp = w.spy.ctx.multiplayer;
        let upgrade = self.no_upgrade[e] == 0;
        for s in 1..=HULL_SLOTS {
            for v in [BEST, SPECIAL] {
                if mp && v == SPECIAL {
                    continue;
                }
                if !(self.empty_planets[e] != 0 || !COLONY_SLOTS.contains(&s) || all) {
                    continue;
                }
                let hull = w.rules.design.hulls.get(s).copied().unwrap_or_default();
                let t = hull.tech.max(0) as usize;
                if v == SPECIAL && (w.spy.colony.tech.byte(t, 0x26) != 0 || hull.flags[..5].iter().any(|&f| f != 0)) {
                    self.designs[SPECIAL][e][s] = 0;
                    continue;
                }
                let n = w.design_count(e) as i32;
                for j in [BEST, SPECIAL] {
                    if self.designs[j][e][s] as i32 > n {
                        self.designs[j][e][s] = 0;
                    }
                }
                let mut d = self.designs[v][e][s] as i32;
                let mut redo = upgrade;
                if d == 0 {
                    if w.spy.colony.tech.status(t, e) != 5 {
                        continue;
                    }
                    for k in super::design::FIRST_OWN..=n.max(0) as usize {
                        let Some(x) = w.des(e, k as i32) else { continue };
                        let r = &x.raw;
                        if v == BEST && rd_i16(r, 0x16) >= 0 && r[0x18] != 0 && r[0x19] == 0 && rd_i32(r, 0x44) == s as i32 {
                            d = k as i32;
                        }
                    }
                    if d == 0 {
                        d = w.new_design(e, 0, hooks) as i32;
                        redo = true;
                    }
                    if d < 1 {
                        continue;
                    }
                    let du = d as usize;
                    if redo {
                        w.clear_design(e, du);
                    }
                    let name: Option<Vec<u8>> = mp.then(|| {
                        let b = &w.spy.colony.tech.bytes;
                        let rec = b.get(t * crate::colony_side::TECH_STRIDE..).unwrap_or(&[]);
                        rec[..rec.iter().position(|&c| c == 0).unwrap_or(rec.len())].to_vec()
                    });
                    w.edit_design(e, du, |r| {
                        r[0x44..0x48].copy_from_slice(&(s as i32).to_le_bytes());
                        r[..0x15].fill(0);
                        if let Some(name) = &name {
                            let len = name.len().min(0x14);
                            r[..len].copy_from_slice(&name[..len]);
                        }
                    });
                    if !mp {
                        w.events.push(AiEvent::DesignNamed { empire: e as u8, design: du, slot: s, special: v == SPECIAL });
                    }
                    self.designs[v][e][s] = d as i16;
                    w.edit_design(e, du, |r| r[0x20..0x24].fill(0));
                }
                if d > 0 {
                    let du = d as usize;
                    let old = w.des_i32(e, d, 0x20);
                    if redo {
                        w.auto_design(e, du, v);
                    }
                    w.recompute_design(e, du);
                    if old > 0 {
                        w.retime_queues(e, du, old);
                    }
                }
            }
        }
        if self.no_upgrade[e] == 0 {
            let mut k = FIRST_OWN;
            while k <= w.design_count(e) {
                let live = w.des(e, k as i32).is_some_and(|x| rd_i16(&x.raw, 0x16) >= 0 && x.raw[0x18] != 0);
                let used = (1..=HULL_SLOTS).any(|s| self.designs[BEST][e][s] as usize == k || self.designs[SPECIAL][e][s] as usize == k);
                if live && !used {
                    let old = w.des_i32(e, k as i32, 0x20);
                    w.auto_design(e, k, BEST);
                    w.recompute_design(e, k);
                    w.retime_queues(e, k, old);
                }
                k += 1;
            }
        }
        w.recount();
    }
}
