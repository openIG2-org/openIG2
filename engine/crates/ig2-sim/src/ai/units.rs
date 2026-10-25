//! Unit bookkeeping the AI's part 2 shares (`re/rules/ai-empire.md` §12): fleet orders (`FlottaMozgat`), new and
//! deleted fleets, ship and vehicle transfers, merges, the recount (`FlottakUjraSzamol`) and design deletion
//! (`TervDelete`).
//!
//! A fleet slot has two views: the record (`w.starmap.fleets[f]`: name, owner, flags, position, orders) and the
//! totals (`w.spy.combat.fleets[f].totals`). The record is authoritative for its own fields, the combat view for
//! the totals; [`World::mirror`] copies each into the other after every change. Garrison sums live on the colony
//! world's planets (`+0x58` / `+0x5c`). The same operations exist for other contexts in `spacewar`
//! (`HajoDelete`, `FlottaDelete`), `production` (`FlottaNew`, `TankNew`) and `fleet` (`FlottaMozgat`); this module
//! keeps both fleet views and the garrisons in step, as the AI needs.

use super::{AiEvent, AiHooks, World, EMPIRES};
use crate::fleet::{flag, Fleet, StarmapEvent, DELETED};
use crate::spacewar::{CombatEvent, CombatFleet, Design, FleetTotals, Vehicle, FLAG_DESIGN28, NO_DESIGN};

/// `Tankok` grows up to this many entries (`production::MAX_VEHICLE_ENTRIES`).
const MAX_VEHICLES: usize = crate::production::MAX_VEHICLE_ENTRIES;
/// `OsszHanySajatFlotta[0xFF]` is `Tankok[96].count` (exe 0xBB40B0 + 0x3FC = 0xBB44AC).
const COUNTER_OVERFLOW_VEHICLE: usize = 96;

/// The name up to its terminating NUL (C `strcmp` semantics).
pub(crate) fn cstr(name: &[u8]) -> &[u8] {
    &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())]
}

fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl World<'_> {
    /// Copy fleet `f`'s record fields into its combat view and the totals into the record.
    pub(crate) fn mirror(&mut self, f: usize) {
        let cw = &mut self.spy.combat.fleets;
        if cw.len() < self.starmap.fleets.len() {
            cw.resize(self.starmap.fleets.len(), CombatFleet { owner: DELETED, ..Default::default() });
        }
        let (Some(r), Some(c)) = (self.starmap.fleets.get_mut(f), cw.get_mut(f)) else { return };
        c.owner = r.owner;
        c.flags = r.flags;
        (c.x, c.y) = (r.x, r.y);
        (c.colony_ships, c.terraformers) = (r.colony_ships, r.terraformers);
        r.ship_count = c.totals.ships;
        r.strength = c.totals.attack;
        r.ground_strength = c.totals.ground_strength;
    }

    /// Fleet slots `1..=OsszHanyFlotta`.
    pub(crate) fn fleet_count(&self) -> usize {
        self.starmap.fleets.len().saturating_sub(1)
    }

    /// Fleet `f`'s record (`f` ≥ 0; slot 0 included).
    pub(crate) fn rec(&self, f: i32) -> Option<&Fleet> {
        self.starmap.fleets.get(usize::try_from(f).ok()?)
    }

    pub(crate) fn totals(&self, f: i32) -> FleetTotals {
        usize::try_from(f).ok().and_then(|f| self.spy.combat.fleets.get(f)).map_or_else(FleetTotals::default, |c| c.totals)
    }

    /// Design `d` of empire `e` (`Tervek[e][d]`).
    pub(crate) fn des(&self, e: usize, d: i32) -> Option<&Design> {
        self.spy.combat.designs.get(e)?.get(usize::try_from(d).ok()?)
    }

    /// A raw i32 of design `d` of `e` (0 outside the table).
    pub(crate) fn des_i32(&self, e: usize, d: i32, o: usize) -> i32 {
        self.des(e, d).map_or(0, |x| rd_i32(&x.raw, o))
    }

    fn human_controlled(&self, e: u8) -> bool {
        self.star_ctx.human_controlled.get(e as usize).copied().unwrap_or(false)
    }

    /// The owner of a vehicle holder: fleet `h > 0`, planet `−h` otherwise.
    pub(crate) fn holder_owner(&self, h: i32) -> u8 {
        if h < 1 {
            self.spy.colony.planets.get(h.unsigned_abs() as usize).map_or(DELETED, |p| p.owner)
        } else {
            self.rec(h).map_or(DELETED, |r| r.owner)
        }
    }

    /// `FlottaMozgat(f, ord, kind, x, y)` (`fleet-movement.md` §2): colony ships and terraformers of a colonising
    /// fleet return to the pools.
    pub(crate) fn order(&mut self, f: usize, ord: i16, kind: i16, x: f32, y: f32) {
        if f >= self.starmap.fleets.len() {
            return;
        }
        let mut ev = Vec::new();
        self.starmap.order(f, ord, kind, x, y, self.star_ctx, &mut ev);
        for e in ev {
            let cw = &mut *self.spy.combat;
            match e {
                StarmapEvent::ColonyShipFreed { empire } => cw.colony_ship_pool[empire as usize % EMPIRES] += 1,
                StarmapEvent::TerraformerFreed { empire } => cw.terraformer_pool[empire as usize % EMPIRES] += 1,
                _ => {}
            }
        }
        self.mirror(f);
    }

    /// `FlottaMozgatHaNincsAzonosito`: [`World::order`] unless the fleet has a story id.
    pub(crate) fn order_unscripted(&mut self, f: usize, ord: i16, kind: i16, x: f32, y: f32) {
        if self.starmap.fleets.get(f).is_some_and(|r| r.story_id == 0) {
            self.order(f, ord, kind, x, y);
        }
    }

    /// Clear bit 5 when a fleet holds only colony ships and terraformers (the `FlottaMozgat` rule).
    pub(crate) fn settle_radar_flag(&mut self, f: usize) {
        if let Some(r) = self.starmap.fleets.get_mut(f) {
            if r.colony_ships as i32 + r.terraformers as i32 == r.ship_count {
                r.flags &= !flag::RADAR_IGNORED;
            }
        }
        self.mirror(f);
    }

    /// `HajoFlottahozHozzaszamol(f, d, n)` (`space-combat.md` §3).
    fn ship_sums(&mut self, f: i32, d: i16, n: i32) {
        let Ok(fu) = usize::try_from(f) else { return };
        let Some(owner) = self.starmap.fleets.get(fu).map(|r| r.owner).filter(|&o| o != DELETED) else { return };
        let Some(des) = self.des(owner as usize, d as i32).cloned() else { return };
        self.spy.combat.fleets[fu].totals.add_ship(&des, n);
        let o = owner as usize % EMPIRES;
        let r = &mut self.starmap.fleets[fu];
        if des.hull_flags[0] != 0 {
            r.colony_ships = r.colony_ships.wrapping_add(n as i16);
            self.spy.combat.colony_ship_pool[o] += n;
        }
        if des.hull_flags[1] != 0 {
            r.terraformers = r.terraformers.wrapping_add(n as i16);
            self.spy.combat.terraformer_pool[o] += n;
        }
        if d == 28 {
            if n > 0 {
                r.flags |= FLAG_DESIGN28;
            } else if n < 0 {
                r.flags &= !FLAG_DESIGN28;
            }
        }
        self.mirror(fu);
    }

    /// `TankFlottahozHozzaszamol(h, d, n)`: ground vehicles add to the holder's ground sums (a planet's garrison
    /// `+0x58` / `+0x5c`), anything else in a fleet counts as fighters.
    fn vehicle_sums(&mut self, h: i32, d: i16, n: i32) {
        let owner = self.holder_owner(h);
        if owner == DELETED {
            return;
        }
        let Some(des) = self.des(owner as usize, d as i32).cloned() else { return };
        if h > 0 {
            self.spy.combat.fleets[h as usize].totals.add_vehicle(&des, n);
            self.mirror(h as usize);
        } else if des.class == 2 {
            let pl = &mut self.spy.colony.planets[h.unsigned_abs() as usize];
            pl.field_58 = pl.field_58.wrapping_add(n);
            let s = (des.hp.wrapping_mul(des.attack) as f32 as f64).sqrt() as f32;
            pl.field_5c = (s * n as f32 + pl.field_5c as f32) as i32;
        }
    }

    /// `HajoAtpakol(i, g)`: ship `i` moves to fleet `g`.
    pub(crate) fn move_ship(&mut self, i: usize, g: usize) {
        let s = self.spy.combat.ships[i];
        if s.design == NO_DESIGN {
            return;
        }
        self.spy.combat.ships[i].fleet = g as i16;
        self.ship_sums(s.fleet as i32, s.design, -1);
        self.ship_sums(g as i32, s.design, 1);
    }

    /// `TankDelete(h, d, n)`: the lowest-index entry of holder `h` with design `d` loses up to `n` vehicles; a
    /// planet's ground-war map loses `n`.
    pub(crate) fn delete_vehicles(&mut self, h: i32, d: i16, n: i32) {
        let vs = &mut self.spy.combat.vehicles;
        if let Some(i) = (1..vs.len()).find(|&i| vs[i].design == d && vs[i].fleet as i32 == h) {
            let m = n.min(vs[i].count);
            vs[i].count -= m;
            if vs[i].count < 1 {
                vs[i].design = NO_DESIGN;
            }
            self.vehicle_sums(h, d, -m);
        }
        if h < 0 && n > 0 {
            self.events.push(AiEvent::GarrisonMap { planet: h.unsigned_abs() as usize, design: d, count: -n });
        }
    }

    /// `TankNew(h, d, n)` (`production-research.md` §5.1): ground vehicles anywhere, others only in fleets.
    pub(crate) fn new_vehicles(&mut self, h: i32, d: i16, n: i32) {
        let owner = self.holder_owner(h);
        let class = self.des(owner as usize, d as i32).map_or(0, |x| x.class);
        if !(class > 1 || h > 0) {
            return;
        }
        let holder = h as i16;
        let vs = &mut self.spy.combat.vehicles;
        let live = (1..vs.len()).rev().find(|&i| vs[i].design != NO_DESIGN && vs[i].design == d && vs[i].fleet == holder);
        let slot = match live.or_else(|| (1..vs.len()).rev().find(|&i| vs[i].design == NO_DESIGN)) {
            Some(i) => i,
            None => {
                if vs.len() > MAX_VEHICLES {
                    return;
                }
                if vs.is_empty() {
                    vs.push(Vehicle { fleet: 0, design: NO_DESIGN, count: 0 });
                }
                vs.push(Vehicle { fleet: 0, design: NO_DESIGN, count: 0 });
                vs.len() - 1
            }
        };
        let v = &mut vs[slot];
        if v.design == NO_DESIGN {
            *v = Vehicle { fleet: holder, design: d, count: n };
        } else {
            v.count = v.count.wrapping_add(n);
        }
        self.vehicle_sums(h, d, n);
        if h < 0 {
            self.events.push(AiEvent::GarrisonMap { planet: h.unsigned_abs() as usize, design: d, count: n });
        }
    }

    /// `TankAtpakol(from, to, d, n)` (§12.6).
    pub(crate) fn transfer_vehicles(&mut self, from: i32, to: i32, d: i16, n: i32) {
        self.delete_vehicles(from, d, n);
        if from < 0 {
            self.events.push(AiEvent::GarrisonRearranged { planet: from.unsigned_abs() as usize });
        }
        self.new_vehicles(to, d, n);
    }

    /// `HajoDelete(i)` (`space-combat.md` §5.3): ground vehicles beyond the capacity go (every non-fighter entry
    /// loses up to the excess, which is not reduced [original bug]); an emptied fleet of an AI is deleted.
    pub(crate) fn delete_ship(&mut self, i: usize) {
        let s = self.spy.combat.ships[i];
        if s.design == NO_DESIGN {
            return;
        }
        self.spy.combat.ships[i].design = NO_DESIGN;
        self.ship_sums(s.fleet as i32, s.design, -1);
        let f = s.fleet as i32;
        let t = self.totals(f);
        let excess = t.ground - t.capacity;
        let owner = self.rec(f).map_or(DELETED, |r| r.owner);
        let mut k = 1;
        while k < self.spy.combat.vehicles.len() && excess > 0 {
            let v = self.spy.combat.vehicles[k];
            if v.design != NO_DESIGN && v.fleet == s.fleet && self.des(owner as usize, v.design as i32).is_none_or(|x| x.raw[0x36] == 0) {
                self.delete_vehicles(f, v.design, excess.min(v.count));
            }
            k += 1;
        }
        let Some((flags, owner)) = self.rec(f).map(|r| (r.flags, r.owner)) else { return };
        let special = flags & (flag::WORMHOLE | flag::PROTECTED) != 0;
        if self.totals(f).ships < 1 && !special {
            if self.spy.ctx.sends(owner as usize) {
                self.events.push(AiEvent::HumanFleetEmptied { fleet: f as usize });
            }
            if !self.human_controlled(owner) {
                self.delete_fleet(f as usize);
            }
        }
    }

    /// `FlottaDelete(f)` (`space-combat.md` §5.3, wormholes excepted): followers stop, every ship and vehicle goes,
    /// `OsszHanySajatFlotta[owner] −= 1` with the owner read at the end — after a re-entry through the last
    /// `HajoDelete` that is 0xFF, whose counter is `Tankok[96].count` [original bug] — and the slot is freed.
    pub(crate) fn delete_fleet(&mut self, f: usize) {
        let Some(r) = self.starmap.fleets.get(f) else { return };
        if r.owner == DELETED || r.flags & flag::WORMHOLE != 0 {
            return;
        }
        for g in 1..self.starmap.fleets.len() {
            let x = &self.starmap.fleets[g];
            if x.owner != DELETED && x.order > 0 && x.kind == crate::fleet::order::FLEET && x.tx == f as f32 {
                self.order(g, 0, 0, 0.0, 0.0);
                self.events.push(AiEvent::Combat(CombatEvent::FollowerStopped { fleet: g }));
            }
        }
        for i in 1..self.spy.combat.ships.len() {
            let s = self.spy.combat.ships[i];
            if s.fleet as usize == f && s.fleet >= 0 && s.design != NO_DESIGN {
                self.delete_ship(i);
            }
        }
        for k in 1..self.spy.combat.vehicles.len() {
            let v = self.spy.combat.vehicles[k];
            if v.fleet >= 0 && v.fleet as usize == f && v.design != NO_DESIGN {
                self.delete_vehicles(f as i32, v.design, v.count);
            }
        }
        let owner = self.starmap.fleets[f].owner;
        match self.fleet_numbers.get_mut(owner as usize) {
            Some(c) => *c -= 1,
            None => {
                if let Some(v) = self.spy.combat.vehicles.get_mut(COUNTER_OVERFLOW_VEHICLE) {
                    v.count -= 1;
                }
            }
        }
        self.starmap.fleets[f].owner = DELETED;
        self.mirror(f);
        self.events.push(AiEvent::Combat(CombatEvent::FleetDeleted { fleet: f, owner }));
    }

    /// `FlottaNew(name, owner, id, x, y, speed)` (`production-research.md` §5.1): the last free slot, else a new
    /// one. Returns the slot.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_fleet(&mut self, name: &[u8], owner: u8, id: i16, x: f32, y: f32, speed: f32, hooks: &mut dyn AiHooks) -> usize {
        let o = owner as usize % EMPIRES;
        self.fleet_numbers[o] = self.fleet_numbers[o].wrapping_add(1);
        let numbered = cstr(name) == cstr(&self.star_ctx.default_fleet_name);
        let text = if numbered {
            let first = self.fleet_numbers[o];
            let label = |n: i32| [cstr(name), b" ", n.to_string().as_bytes()].concat();
            let probe = label(first);
            let fl = &self.starmap.fleets;
            let taken = (0..fl.len().saturating_sub(1)).filter(|&g| fl[g].owner == owner && cstr(&fl[g].name) == probe).count();
            label(first + taken as i32)
        } else {
            cstr(name).to_vec()
        };
        let slot = (1..self.starmap.fleets.len()).rev().find(|&g| self.starmap.fleets[g].owner == DELETED).unwrap_or_else(|| {
            if self.starmap.fleets.is_empty() {
                self.starmap.fleets.push(Fleet { owner: DELETED, ..Default::default() });
            }
            let raw = hooks.fresh_fleet(self.starmap.fleets.len());
            self.starmap.fleets.push(Fleet::from_record(&raw));
            self.starmap.fleets.len() - 1
        });
        let default_name = self.star_ctx.default_fleet_name.clone();
        let f = &mut self.starmap.fleets[slot];
        for (i, &c) in text.iter().chain(std::iter::once(&0)).enumerate().take(f.name.len()) {
            f.name[i] = c;
        }
        f.owner = owner;
        f.story_id = id;
        f.state &= !3;
        f.visible[1..].fill(0);
        f.field_2c = [0; 2];
        if let Some(v) = f.visible.get_mut(o) {
            *v = 1;
        }
        if speed <= 1.0 {
            f.speed = ((if speed == 0.0 { 6.0 } else { speed }) as f64 / 100.0) as f32;
            f.speed_override = 0.0;
        } else {
            f.speed = (speed as f64 / 100.0) as f32;
            f.speed_override = f.speed;
        }
        (f.x, f.y) = (x, y);
        (f.order, f.kind, f.tx, f.ty) = (0, 0, 0.0, 0.0);
        (f.colony_ships, f.terraformers, f.first_waypoint) = (0, 0, 0);
        let named = cstr(&f.name) != cstr(&default_name);
        f.flags = (f.flags & 0x8000) | if named { flag::NAMED } else { 0 };
        f.radar = 0.0;
        self.mirror(slot);
        self.spy.combat.fleets[slot].totals = FleetTotals::default();
        self.mirror(slot);
        slot
    }

    /// `FlottaOsszevon(into, from)` (§12.7).
    pub(crate) fn merge(&mut self, into: usize, from: usize) {
        let n = self.fleet_count();
        if into == 0 || from == 0 || into > n || from > n || into == from {
            return;
        }
        let default = self.star_ctx.default_fleet_name.clone();
        let src = self.starmap.fleets[from].clone();
        let r = &mut self.starmap.fleets[into];
        if r.radar < src.radar {
            r.radar = src.radar;
        }
        if cstr(&r.name) == cstr(&default) {
            let len = (cstr(&src.name).len() + 1).min(r.name.len());
            r.name[..len].copy_from_slice(&src.name[..len]);
        }
        if r.story_id == 0 {
            r.story_id = src.story_id;
        }
        let named = cstr(&r.name) != cstr(&default);
        r.flags = (r.flags & !flag::NAMED) | if named { flag::NAMED } else { 0 };
        self.mirror(into);
        for g in 1..=n {
            let x = &self.starmap.fleets[g];
            if x.owner != DELETED && g != into && g != from && x.order > 0 && x.kind == crate::fleet::order::FLEET && x.tx == from as f32 {
                let keep = x.flags & flag::RADAR_IGNORED;
                let ord = x.order;
                self.order(g, ord, crate::fleet::order::FLEET, into as f32, 0.0);
                let x = &mut self.starmap.fleets[g];
                x.flags = (x.flags & !flag::RADAR_IGNORED) | keep;
                self.mirror(g);
            }
        }
        for i in 1..self.spy.combat.ships.len() {
            let s = self.spy.combat.ships[i];
            if s.fleet as usize == from && s.fleet > 0 && s.design != NO_DESIGN {
                self.move_ship(i, into);
            }
        }
        for k in 1..self.spy.combat.vehicles.len() {
            let v = self.spy.combat.vehicles[k];
            if v.fleet as usize == from && v.fleet > 0 && v.design != NO_DESIGN {
                self.transfer_vehicles(from as i32, into as i32, v.design, v.count);
            }
        }
        let o = self.starmap.fleets[from].owner as usize;
        if let Some(c) = self.fleet_numbers.get_mut(o) {
            *c -= 1;
        }
        self.starmap.fleets[from].owner = DELETED;
        self.mirror(from);
        let r = &mut self.starmap.fleets[into];
        if self.star_ctx.human_controlled.get(r.owner as usize).copied().unwrap_or(false) {
            r.flags &= !flag::RADAR_IGNORED;
        }
        self.settle_radar_flag(into);
    }

    /// `FlottakUjraSzamol()` (§12.9): every fleet's totals and every garrison from the ships and vehicles.
    pub(crate) fn recount(&mut self) {
        let n = self.fleet_count();
        for f in 1..=n {
            if self.starmap.fleets[f].owner != DELETED {
                self.spy.combat.fleets[f].totals = FleetTotals::default();
                let r = &mut self.starmap.fleets[f];
                (r.colony_ships, r.terraformers) = (0, 0);
                self.mirror(f);
            }
        }
        for p in self.spy.colony.planets.iter_mut().skip(1) {
            (p.field_58, p.field_5c, p.field_60) = (0, 0, 0);
        }
        let cw = &mut *self.spy.combat;
        for e in 1..EMPIRES {
            cw.colony_ship_pool[e] = 0;
            cw.terraformer_pool[e] = 0;
        }
        for s in cw.ships.iter_mut().skip(1) {
            if s.design != NO_DESIGN && s.fleet as i32 > n as i32 {
                s.design = NO_DESIGN;
            }
        }
        for v in cw.vehicles.iter_mut().skip(1) {
            if v.design != NO_DESIGN && v.fleet as i32 > n as i32 {
                v.design = NO_DESIGN;
            }
        }
        for i in 1..self.spy.combat.ships.len() {
            let s = self.spy.combat.ships[i];
            if s.design != NO_DESIGN {
                self.ship_sums(s.fleet as i32, s.design, 1);
            }
        }
        for k in 1..self.spy.combat.vehicles.len() {
            let v = self.spy.combat.vehicles[k];
            if v.design != NO_DESIGN {
                self.vehicle_sums(v.fleet as i32, v.design, v.count);
            }
        }
    }

    /// `TervDelete(e, d, fleet, mark)` (§12.8): the units, stations and queue entries of design `d` (and of designs
    /// derived from it, `+0x1c == d`); `mark` also flags the design deleted.
    pub(crate) fn delete_design(&mut self, e: usize, d: i32, fleet: i32, mark: bool) {
        if d <= 0 {
            return;
        }
        let of = |w: &World, x: i16| x as i32 == d || w.des_i32(e, x as i32, 0x1c) == d;
        for i in 1..self.spy.combat.ships.len() {
            let s = self.spy.combat.ships[i];
            let owner = self.rec(s.fleet as i32).map_or(DELETED, |r| r.owner);
            if s.design != NO_DESIGN && of(self, s.design) && owner as usize == e && (fleet == 0 || s.fleet as i32 == fleet) {
                self.delete_ship(i);
            }
        }
        for k in 1..self.spy.combat.vehicles.len() {
            let v = self.spy.combat.vehicles[k];
            if v.design == NO_DESIGN || !of(self, v.design) {
                continue;
            }
            if self.holder_owner(v.fleet as i32) as usize == e && (fleet == 0 || v.fleet as i32 == fleet) {
                self.delete_vehicles(v.fleet as i32, v.design, v.count);
            }
        }
        if fleet == 0 {
            for p in 1..self.spy.colony.planets.len() {
                if self.spy.colony.planets[p].owner as usize != e {
                    continue;
                }
                let mut st = self.spy.colony.planets[p].stations;
                let mut count = self.spy.colony.planets[p].field_114;
                for k in 1..=count.clamp(0, st.len() as i32) as usize {
                    if of(self, st[k - 1]) {
                        st[k - 1] = 0;
                    }
                }
                let mut k = 1;
                while k <= count && (k as usize) <= st.len() {
                    if st[k as usize - 1] == 0 {
                        count -= 1;
                        for j in k..=count {
                            let j = j as usize;
                            if j < st.len() {
                                st[j - 1] = st[j];
                            }
                        }
                    } else {
                        k += 1;
                    }
                }
                let pl = &mut self.spy.colony.planets[p];
                (pl.stations, pl.field_114) = (st, count);
            }
        }
        for g in 1..=self.fleet_count() {
            let r = &self.starmap.fleets[g];
            let t = self.totals(g as i32);
            if r.owner != DELETED && t.ships + t.ground < 1 && r.flags & (flag::WORMHOLE | flag::PROTECTED) == 0 {
                self.delete_fleet(g);
            }
        }
        if fleet == 0 {
            for p in 1..self.spy.colony.planets.len() {
                if self.spy.colony.planets[p].owner as usize != e {
                    continue;
                }
                for line in 0..2 {
                    loop {
                        let q = &self.spy.colony.planets[p].production;
                        let word = |k: usize| i16::from_le_bytes([q[8 * line + 2 * k], q[8 * line + 2 * k + 1]]) as i32;
                        let Some(k) = (0..4).find(|&k| word(k) == d) else { break };
                        let q = &mut self.spy.colony.planets[p].production;
                        for j in k..3 {
                            let (a, b) = (8 * line + 2 * j, 8 * line + 2 * j + 2);
                            q.copy_within(b..b + 2, a);
                            for base in [0x10, 0x18, 0x35] {
                                q[base + 4 * line + j] = q[base + 4 * line + j + 1];
                            }
                        }
                        q[8 * line + 6..8 * line + 8].fill(0);
                        if k == 0 {
                            let head = i16::from_le_bytes([q[8 * line], q[8 * line + 1]]) as i32;
                            let time = self.des_i32(e, head, 0x20);
                            let q = &mut self.spy.colony.planets[p].production;
                            q[0x28 + 4 * line..0x2c + 4 * line].copy_from_slice(&time.to_le_bytes());
                            q[0x30 + line] = 0;
                        }
                    }
                }
            }
            if mark {
                if let Some(x) = self.spy.combat.designs.get_mut(e).and_then(|t| t.get_mut(d as usize)) {
                    x.raw[0x16..0x18].copy_from_slice(&(-1i16).to_le_bytes());
                    *x = Design::from_bytes(&x.raw);
                }
            }
        }
    }

    /// `MULTIGET_Starmap` 0x17 (exe 0x59dbe8): station `index` (1-based) of planet `p` is sold for
    /// `CsokkentettTervAr(e, d)` (`MoneyHozzaadLog(e, 0, x)`) and the list closes up.
    pub(crate) fn sell_station(&mut self, e: usize, p: usize, index: i32) {
        let n = self.spy.colony.planets[p].field_114;
        if index < 1 || index > n || n < 1 {
            return;
        }
        let st = self.spy.colony.planets[p].stations;
        let Some(&d) = st.get(index as usize - 1) else { return };
        let price = self.design(e, d as i32).map_or(0, |r| super::resale(r, self.ctx, self.spy.ctx.human_empire));
        let sw = &mut self.spy;
        sw.colony.add_money(e, 0, price as f64, sw.side_rules, sw.econ);
        let pl = &mut self.spy.colony.planets[p];
        pl.field_114 -= 1;
        for i in index..=pl.field_114 {
            let (a, b) = (i as usize - 1, i as usize);
            if b < pl.stations.len() {
                pl.stations[a] = pl.stations[b];
            }
        }
    }

    /// `MULTIGET_Starmap` 0x1c (exe 0x59e2b4): up to `n` units of design `d` go from holder `from` to `to` (a fleet,
    /// or planet p as −p; both the issuer's): ships first (`HajoAtpakol`, only ship `ship` when non-zero; never onto
    /// a planet, which the exe would write outside the fleet table), then the first vehicle entry (`TankAtpakol`,
    /// clamped to `to`'s free capacity for ground vehicles), then the ground vehicles `from` can no longer carry.
    pub(crate) fn transfer_units(&mut self, e: usize, from: i32, to: i32, d: i16, ship: i32, n: i32) {
        let mut n = n;
        let mut i = 1;
        while i < self.spy.combat.ships.len() && n > 0 {
            let s = self.spy.combat.ships[i];
            if s.fleet as i32 == from && s.design == d && (ship == 0 || i as i32 == ship) && to > 0 {
                self.move_ship(i, to as usize);
                n -= 1;
            }
            i += 1;
        }
        let ground = |w: &Self, d: i16| w.des(e, d as i32).is_some_and(|x| x.class == 2);
        if ground(self, d) && to > 0 {
            let t = self.totals(to);
            n = n.min(t.capacity - t.ground);
        }
        let mut i = 1;
        while i < self.spy.combat.vehicles.len() && n > 0 {
            let v = self.spy.combat.vehicles[i];
            if v.fleet as i32 == from && v.design == d {
                n = n.min(v.count);
                self.transfer_vehicles(from, to, d, n);
                break;
            }
            i += 1;
        }
        let mut excess = if from > 0 { let t = self.totals(from); (t.ground - t.capacity).max(0) } else { 0 };
        let mut i = 1;
        while i < self.spy.combat.vehicles.len() && excess > 0 {
            let v = self.spy.combat.vehicles[i];
            if v.design != NO_DESIGN && v.fleet as i32 == from && ground(self, v.design) {
                let mut m = excess.min(v.count);
                if to > 0 {
                    let t = self.totals(to);
                    m = m.min((t.capacity - t.ground).max(0));
                }
                self.transfer_vehicles(from, to, v.design, m);
                excess -= m;
            }
            i += 1;
        }
    }
}
