//! The starmap's lockstep messages (`MULTIGET_Starmap` exe 0x59d930, ELF 0x719680; re/screens/starmap.md §10): the
//! orders the starmap screen sends with `Multi_Send(type, EN, …)` and every peer applies in the same frame. The
//! screen's own bookkeeping in the handler (selection, transfer-screen flags, textures, the tutorial's event 0x15)
//! is the client's.
//!
//! Not here: 0x1b (fleet emptied: [`Game::fleet_emptied`]), the session messages (0x1/0x2 join, 0x3fc/0x3fd chat,
//! 0x401/0x402 desync checks, 0x404 a player dropped, 0x405 log files, 2000 a player left: the lobby's and
//! `ig2-net`'s business).

use super::*;

/// One starmap message's payload (the handler's `Multi_Get` values after the issuer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StarmapOrder {
    /// 0x14: split fleet `fleet`: a new fleet of the issuer at its place (`FlottaNew(AlapFlottaNev, e, id, x, y, 0)`),
    /// the old one stops; the screen then opens the transfer between the two.
    Split { fleet: i32 },
    /// 0x15: `FlottaMozgat(fleet, order, kind, x, y)` (any fleet: the screen also steers story fleet 0x1f88).
    Move { fleet: i32, order: i32, kind: i32, x: f32, y: f32 },
    /// 0x16: send the issuer's nearest colony ship (`kind` 1, `DeleteKolonizalo`) or terraformer (2,
    /// `DeleteTerraformalo`) to `planet`.
    Colonise { planet: i32, kind: i32 },
    /// 0x17: sell station `index` (1-based, `+0x100 + 2(index − 1)`) of `planet` for `CsokkentettTervAr`.
    SellStation { planet: i32, index: i32 },
    /// 0x18: buy satellite `kind` (`MuholdAr[kind]`) over `planet` (`+0xc4 + e`).
    Satellite { planet: i32, kind: i32 },
    /// 0x19: a planet setting: 1/2 tax −/+ (`+0x34`, 0..=7), 3/4 aggressiveness −/+ (`+0x6d`, 0..=3), 5/6/7 copy
    /// tax / autobuild (`+0x6c`) / aggressiveness to the issuer's other planets, 8/9/10 set them to `value`.
    Setting { planet: i32, what: i32, value: i32 },
    /// 0x1a: the whole production block `+0x78..+0xb8` of the issuer's `planet`.
    Queue { planet: i32, block: [u8; 64] },
    /// 0x1c: move up to `count` units of design `design` from holder `from` to `to` (fleet > 0, planet −p), ships
    /// first (only ship slot `ship` when it is non-zero), then vehicles, then the vehicles the ships no longer
    /// carry.
    Transfer { from: i32, to: i32, design: i32, ship: i32, count: i32 },
    /// 0x1d: a production queue edit on line `line` (0 ships, 1 vehicles) of the issuer's `planet`, on the slot
    /// whose UI byte (`+0x98 + 4L + k`) is `slot`: 1/2 count −/+ `value` times, 3 toggle repeat, 4 put design
    /// `value` there (0: remove the slot; slot 0 refunds the started unit), 5 toggle "into a fleet".
    Production { op: i32, planet: i32, line: i32, slot: i32, value: i32 },
    /// 0x400: `planet`'s rally planet (`+0xac`, 0 = none).
    Rally { planet: i32, target: i32 },
    /// 0x40f: the head units of `planet`'s queues are finished (`+0xa0 + 4L = −1`; a debug key).
    FinishHeads { planet: i32 },
    /// 0x403: `Multiplayer_speed` (exe 0xBAE0AC).
    MpSpeed { value: i32 },
    /// 0x406: `SetIdoSpeed(level, true)`.
    Speed { level: i32 },
    /// 0x407: `SetIdoPause(paused, true)`.
    Pause { paused: bool },
}

impl StarmapOrder {
    /// The order a `MULTIGET_Starmap` message carries: its type and the values after the issuer, in `Multi_Get`
    /// order (0x1d's unused fourth value included; 0x1a's block as 16 little-endian words).
    pub fn from_message(kind: i32, v: &[i32]) -> Option<Self> {
        let at = |i: usize| v.get(i).copied();
        Some(match kind {
            0x14 => Self::Split { fleet: at(0)? },
            0x15 => Self::Move { fleet: at(0)?, order: at(1)?, kind: at(2)?, x: f32::from_bits(at(3)? as u32), y: f32::from_bits(at(4)? as u32) },
            0x16 => Self::Colonise { planet: at(0)?, kind: at(1)? },
            0x17 => Self::SellStation { planet: at(0)?, index: at(1)? },
            0x18 => Self::Satellite { planet: at(0)?, kind: at(1)? },
            0x19 => Self::Setting { planet: at(0)?, what: at(1)?, value: at(2)? },
            0x1a => {
                let words = v.get(1..17)?;
                let mut block = [0u8; 64];
                for (i, w) in words.iter().enumerate() {
                    block[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
                }
                Self::Queue { planet: at(0)?, block }
            }
            0x1c => Self::Transfer { from: at(0)?, to: at(1)?, design: at(2)?, ship: at(3)?, count: at(4)? },
            0x1d => Self::Production { op: at(0)?, planet: at(1)?, line: at(2)?, slot: at(4)?, value: at(5)? },
            0x400 => Self::Rally { planet: at(0)?, target: at(1)? },
            0x403 => Self::MpSpeed { value: at(0)? },
            0x406 => Self::Speed { level: at(0)? },
            0x407 => Self::Pause { paused: at(0)? & 1 != 0 },
            0x40f => Self::FinishHeads { planet: at(0)? },
            _ => return None,
        })
    }
}

/// What an order did that the issuer's screen reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderEvent {
    /// 0x14: the new fleet `new` beside `fleet` (`AktFleet` and the transfer screen, for the issuer).
    Split { empire: u8, fleet: usize, new: usize },
    /// 0x18 refused: not enough money (`ScrollHibaUzenet`, for the issuer).
    SatelliteRefused { empire: u8, planet: usize },
    /// `MULTIGET_Design` 0x33 stored design `design` (the new one, or the edited one when affordable): the Design
    /// screen re-sorts its list and selects it (for the issuer).
    DesignStored { empire: u8, design: usize },
    /// 0x34: `holder`'s units of `design` were sold or deleted (`holder` 0: the design too).
    DesignSold { empire: u8, design: usize, holder: i32 },
    /// 0x35: `holder`'s units of `design` were refitted (`StarmapAktFlottaValtozott`).
    Refitted { empire: u8, design: usize, holder: i32 },
}

/// `MuholdAr` (exe 0x9138D0, `i32 × 3`): satellite prices by kind (rules, from the user's exe).
pub(super) fn satellite_price(rules: &GameRules, kind: i32) -> Option<i32> {
    rules.satellite_prices.get(usize::try_from(kind).ok()?).copied()
}

/// Exe globals the orders write.
mod at {
    /// `IdoSpeed`, `Multiplayer_speed`.
    pub const SPEED_LEVEL: u32 = 0xBAE258;
    pub const MP_SPEED: u32 = 0xBAE0AC;
}

impl Game<'_> {
    /// `MULTIGET_Starmap`'s handling of one message from empire `issuer`. Values the original would read out of its
    /// arrays' bounds (a fleet, planet or queue line that isn't there) are refused.
    pub fn starmap_order(&mut self, issuer: u8, o: StarmapOrder) {
        let e = issuer as usize % EMPIRES;
        let planets = self.state.planet_count();
        let planet = |p: i32| usize::try_from(p).ok().filter(|&p| (1..=planets).contains(&p));
        match o {
            StarmapOrder::Split { fleet } => self.split(issuer, fleet),
            StarmapOrder::Move { fleet, order, kind, x, y } => {
                let Some(f) = self.fleet_slot(fleet) else { return };
                self.with_ai(|_, w, _| w.order(f, order as i16, kind as i16, x, y));
            }
            StarmapOrder::Colonise { planet: p, kind } => {
                let Some(p) = planet(p) else { return };
                let (px, py) = { let c = self.state.colony(); (c.planets[p].x, c.planets[p].y) };
                self.with_ai(|_, w, hooks| match kind {
                    1 => {
                        let f = w.nearest_colony_fleet(px as i32, py as i32, e);
                        w.send_colony_ship(f, p, hooks);
                    }
                    2 => {
                        let f = w.nearest_terraformer_fleet(px as i32, py as i32, e);
                        w.send_terraformer(f, p, hooks);
                    }
                    _ => {}
                });
            }
            StarmapOrder::SellStation { planet: p, index } => {
                let Some(p) = planet(p) else { return };
                self.with_ai(|_, w, _| w.sell_station(e, p, index));
            }
            StarmapOrder::Satellite { planet: p, kind } => {
                let (Some(p), Some(price)) = (planet(p), satellite_price(self.rules, kind)) else { return };
                let mut c = self.state.colony();
                if c.money[e].available(0) < price as f64 {
                    self.events.push(GameEvent::Order(OrderEvent::SatelliteRefused { empire: issuer, planet: p }));
                    return;
                }
                let income = self.ai_income(e);
                c.money[e].add(0, -price as f64, income);
                c.planets[p].spies[e] = kind as u8;
                self.state.store_colony(&c);
            }
            StarmapOrder::Setting { planet: p, what, value } => {
                let Some(p) = planet(p) else { return };
                let mut c = self.state.colony();
                let n = planets.min(c.planets.len() - 1);
                setting(&mut c.planets[..=n], p, e, what, value as u8);
                self.state.store_colony(&c);
            }
            StarmapOrder::Queue { planet: p, block } => {
                let Some(p) = planet(p) else { return };
                let mut c = self.state.colony();
                if c.planets[p].owner as usize == e {
                    c.planets[p].production = block;
                    self.state.store_colony(&c);
                }
            }
            StarmapOrder::Transfer { from, to, design, ship, count } => self.transfer(e, from, to, design, ship, count),
            StarmapOrder::Production { op, planet: p, line, slot, value } => {
                let (Some(p), Ok(line @ 0..=1)) = (planet(p), usize::try_from(line)) else { return };
                let mut c = self.state.colony();
                if c.planets[p].owner as usize != e {
                    return;
                }
                let combat = self.state.combat();
                let des = |d: i16, o: usize| {
                    combat.designs.get(e).and_then(|t| t.get(usize::try_from(d).ok()?)).map_or(0, |x| i32::from_le_bytes(x.raw[o..o + 4].try_into().unwrap()))
                };
                let station = |d: i16| combat.designs.get(e).and_then(|t| t.get(usize::try_from(d).ok()?)).is_some_and(|x| x.raw[0x37] != 0);
                let refund = production(&mut c.planets[p].production, line, op, slot as u8, value, &des, &station);
                if let Some(x) = refund {
                    let income = self.ai_income(e);
                    c.money[e].add(1, x, income);
                }
                self.state.store_colony(&c);
            }
            StarmapOrder::Rally { planet: p, target } => {
                let Some(p) = planet(p) else { return };
                let mut c = self.state.colony();
                c.planets[p].production[RALLY] = target as u8;
                self.state.store_colony(&c);
            }
            StarmapOrder::FinishHeads { planet: p } => {
                let Some(p) = planet(p) else { return };
                let mut c = self.state.colony();
                let q = &mut c.planets[p].production;
                for line in 0..2 {
                    if i16::from_le_bytes([q[8 * line], q[8 * line + 1]]) != 0 {
                        q[LEFT + 4 * line..LEFT + 4 * line + 4].copy_from_slice(&(-1i32).to_le_bytes());
                    }
                }
                self.state.store_colony(&c);
            }
            StarmapOrder::MpSpeed { value } => self.state.mem.set_i32(at::MP_SPEED, value),
            StarmapOrder::Speed { level } => self.set_speed(level, true),
            StarmapOrder::Pause { paused } => self.set_pause(paused, true),
        }
        // `TankAtpakol` puts garrisons on (or takes them off) the battlefield at once.
        self.battlefield_hand_offs();
    }

    /// `SetIdoSpeed(level, net)` (exe 0x541330): SP 1/2/3 → `IdoSpeedSzorzo` 1/4/16 (battles 1/2/4) unless the
    /// speed is locked (`nemlehetidotallitani`); MP follows `ProgramSebesseg` (1/2/3 → 4/8/16, battles 1/2/2) and only
    /// a lockstep message (`net`) picks one when the host lets the players choose (4: 4/8/16, battles 1/2/4). Any
    /// other level only sets `IdoSpeed`.
    pub fn set_speed(&mut self, level: i32, net: bool) {
        let m = &mut self.state.mem;
        let (mp, program) = (m.u8(va::MULTIPLAYER) != 0, m.i32(va::PROGRAM_SEBESSEG));
        let pick = |level: i32, mp: bool| match level {
            1 => Some(if mp { (4, 1) } else { (1, 1) }),
            2 => Some(if mp { (8, 2) } else { (4, 2) }),
            3 => Some((16, 4)),
            _ => None,
        };
        let set = if !mp {
            if m.i32(va::SPEED_LOCKED) != 0 {
                return;
            }
            (level, pick(level, false))
        } else if program != 4 {
            (program, match program { 1 => Some((4, 1)), 2 => Some((8, 2)), 3 => Some((16, 2)), _ => None })
        } else if net {
            (level, pick(level, true))
        } else {
            return;
        };
        m.set_i32(at::SPEED_LEVEL, set.0);
        if let Some((ido, battle)) = set.1 {
            m.set_i32(va::IDO_SPEED, ido);
            m.set_i32(va::BATTLE_SPEED, battle);
        }
    }

    /// `SetIdoPause(paused, net)` (exe 0x541300): MP never pauses unless the host lets the players choose
    /// (`ProgramSebesseg` 4) and the call comes from a lockstep message.
    pub fn set_pause(&mut self, paused: bool, net: bool) {
        let m = &mut self.state.mem;
        let p = if m.u8(va::MULTIPLAYER) != 0 {
            if m.i32(va::PROGRAM_SEBESSEG) != 4 {
                false
            } else if net {
                paused
            } else {
                return;
            }
        } else {
            paused
        };
        m.set_u8(realtime::at::PAUSE, p as u8);
    }

    /// `IdoPause` (exe 0xBAE264): the strategic clock stands still (the driver doesn't tick).
    pub fn paused(&self) -> bool {
        self.state.mem.u8(realtime::at::PAUSE) != 0
    }

    /// `IdoSpeed` (exe 0xBAE258): the speed button shown as selected (1..=3).
    pub fn speed_level(&self) -> i32 {
        self.state.mem.i32(at::SPEED_LEVEL)
    }

    /// The tick's events, handed over (the battlefield hand-offs are done up to here).
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        self.battlefield_hand_offs();
        self.hand_offs = 0;
        std::mem::take(&mut self.events)
    }

    fn fleet_slot(&self, f: i32) -> Option<usize> {
        let n = self.state.starmap().fleets.len();
        usize::try_from(f).ok().filter(|&f| f >= 1 && f < n)
    }

    /// 0x14 (exe 0x59d9b9): only the issuer's own fleet.
    fn split(&mut self, issuer: u8, fleet: i32) {
        let Some(f) = self.fleet_slot(fleet) else { return };
        let name = self.fleet_name();
        let mut new = 0;
        self.with_ai(|_, w, hooks| {
            let r = &w.starmap.fleets[f];
            if r.owner != issuer {
                return;
            }
            let (id, x, y) = (r.story_id, r.x, r.y);
            new = w.new_fleet(&name, issuer, id, x, y, 0.0, hooks);
            w.order(f, order::NONE, order::POINT, 0.0, 0.0);
        });
        if new > 0 {
            // The local player's screen selects the new fleet (`AktFleet`, saved) and opens the transfer.
            if self.view == ViewMode::Classic && issuer as usize == self.globals().en {
                self.state.mem.set_i32(va::AKT_FLEET, new as i32);
            }
            self.events.push(GameEvent::Order(OrderEvent::Split { empire: issuer, fleet: f, new }));
        }
    }

    /// 0x1c (exe 0x59e2b4): both holders must be the issuer's.
    fn transfer(&mut self, e: usize, from: i32, to: i32, design: i32, ship: i32, count: i32) {
        let n = self.state.starmap().fleets.len() as i32;
        let planets = self.state.planet_count() as i32;
        let c = self.state.colony();
        let map = self.state.starmap();
        let owned = |h: i32| {
            (h > 0 && h < n && map.fleets[h as usize].owner as usize == e)
                || (h < 0 && -h <= planets && c.planets[(-h) as usize].owner as usize == e)
        };
        if !owned(from) || !owned(to) {
            return;
        }
        let Ok(d) = i16::try_from(design) else { return };
        self.with_ai(|_, w, _| w.transfer_units(e, from, to, d, ship, count));
    }
}

/// `+0x78 + …` offsets inside the production block (`production-research.md` §3).
const COUNT: usize = 0x88 - 0x78;
const REPEAT: usize = 0x90 - 0x78;
const UI: usize = 0x98 - 0x78;
const LEFT: usize = 0xa0 - 0x78;
const BUILT: usize = 0xa8 - 0x78;
const RALLY: usize = 0xac - 0x78;
const INTO: usize = 0xad - 0x78;

/// 0x19 (exe 0x59dd5a): settings on `+0x34` (tax), `+0x6c` (autobuild), `+0x6d` (aggressiveness).
fn setting(planets: &mut [Planet], p: usize, e: usize, what: i32, value: u8) {
    let pl = &mut planets[p];
    match what {
        1 if pl.tax_level != 0 => pl.tax_level -= 1,
        2 if pl.tax_level < 7 => pl.tax_level += 1,
        3 if pl.autobuild_aggr != 0 => pl.autobuild_aggr -= 1,
        4 if pl.autobuild_aggr < 3 => pl.autobuild_aggr += 1,
        5..=7 => {
            let (tax, mode, aggr) = (pl.tax_level, pl.autobuild_mode, pl.autobuild_aggr);
            for q in planets.iter_mut().skip(1) {
                if q.owner as usize == e && q.visibility[e] >= 0 {
                    match what {
                        5 => q.tax_level = tax,
                        6 => q.autobuild_mode = mode,
                        _ => q.autobuild_aggr = aggr,
                    }
                }
            }
        }
        8 => pl.tax_level = value,
        9 => pl.autobuild_mode = value,
        10 => pl.autobuild_aggr = value,
        _ => {}
    }
}

/// 0x1d (exe 0x59e7a2) on one planet's production block. `des(d, o)` reads an i32 of the issuer's design `d`,
/// `station(d)` its `+0x37`. Returns the refund for a removed started unit (`MoneyHozzaadLog(e, 1, x)`).
fn production(q: &mut [u8; 64], line: usize, op: i32, slot: u8, value: i32, des: &dyn Fn(i16, usize) -> i32, station: &dyn Fn(i16) -> bool) -> Option<f64> {
    let design = |q: &[u8; 64], k: usize| i16::from_le_bytes([q[8 * line + 2 * k], q[8 * line + 2 * k + 1]]);
    let set_design = |q: &mut [u8; 64], k: usize, d: i16| q[8 * line + 2 * k..8 * line + 2 * k + 2].copy_from_slice(&d.to_le_bytes());
    let at = |base: usize, k: usize| base + 4 * line + k;
    // The slot whose UI byte matches (the last one).
    let k = (0..4).rev().find(|&k| q[at(UI, k)] == slot)?;
    let mut refund = None;
    match op {
        1 => {
            let mut b = q[at(COUNT, k)];
            for _ in 0..value.max(0) {
                if b > 1 {
                    b -= 1;
                }
            }
            if value > 0 {
                q[at(COUNT, k)] = b;
            }
            if q[at(COUNT, 0)] <= q[BUILT + line] {
                q[BUILT + line] = q[at(COUNT, 0)].wrapping_sub(1);
            }
        }
        2 => {
            if value > 0 {
                let mut b = q[at(COUNT, k)];
                for _ in 0..value {
                    if b < 100 {
                        b += 1;
                    }
                }
                q[at(COUNT, k)] = b;
            }
        }
        3 => {
            if !station(design(q, k)) {
                q[at(REPEAT, k)] = (q[at(REPEAT, k)] == 0) as u8;
            }
        }
        4 => {
            let d = design(q, k);
            if k == 0 && d != 0 {
                // f32 in the exe: (1 − left / time) · price, then to f64.
                let left = i32::from_le_bytes(q[LEFT + 4 * line..LEFT + 4 * line + 4].try_into().unwrap());
                let time = des(design(q, 0), 0x20);
                refund = Some(((1.0 - left as f32 / time as f32) * des(d, 0x24) as f32) as f64);
            }
            let mut k = k;
            if d == 0 || value != 0 {
                while k > 0 && design(q, k - 1) == 0 {
                    k -= 1;
                }
                set_design(q, k, value as i16);
                q[at(COUNT, k)] = 1;
                q[at(REPEAT, k)] = 0;
                q[at(INTO, k)] = 0;
            } else {
                for j in k..3 {
                    let next = design(q, j + 1);
                    set_design(q, j, next);
                    q[at(COUNT, j)] = q[at(COUNT, j + 1)];
                    q[at(REPEAT, j)] = q[at(REPEAT, j + 1)];
                    q[at(INTO, j)] = q[at(INTO, j + 1)];
                    q.swap(at(UI, j), at(UI, j + 1));
                }
                set_design(q, 3, 0);
            }
            if k == 0 {
                let time = des(design(q, 0), 0x20);
                q[LEFT + 4 * line..LEFT + 4 * line + 4].copy_from_slice(&time.to_le_bytes());
                q[BUILT + line] = 0;
            }
        }
        5 => q[at(INTO, k)] = (q[at(INTO, k)] == 0) as u8,
        _ => {}
    }
    refund
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue(designs: [i16; 4], counts: [u8; 4]) -> [u8; 64] {
        let mut q = [0u8; 64];
        for k in 0..4 {
            q[2 * k..2 * k + 2].copy_from_slice(&designs[k].to_le_bytes());
            q[COUNT + k] = counts[k];
            q[UI + k] = k as u8;
        }
        q
    }

    #[test]
    fn removing_a_slot_shifts_the_queue_and_swaps_the_ui_bytes() {
        let mut q = queue([5, 6, 7, 0], [3, 4, 5, 0]);
        q[LEFT..LEFT + 4].copy_from_slice(&50i32.to_le_bytes());
        let des = |d: i16, o: usize| if o == 0x20 { 100 * d as i32 } else { 1000 };
        let r = production(&mut q, 0, 4, 0, 0, &des, &|_| false);
        // (1 − 50/500) · 1000 = 900 for the started unit.
        assert_eq!(r, Some(900.0));
        assert_eq!(&q[0..8], &[6, 0, 7, 0, 0, 0, 0, 0]);
        assert_eq!(&q[COUNT..COUNT + 4], &[4, 5, 0, 0]);
        assert_eq!(&q[UI..UI + 4], &[1, 2, 3, 0]);
        assert_eq!(i32::from_le_bytes(q[LEFT..LEFT + 4].try_into().unwrap()), 600);
    }

    #[test]
    fn a_new_design_drops_into_the_first_free_slot() {
        let mut q = queue([5, 0, 0, 0], [3, 0, 0, 0]);
        production(&mut q, 0, 4, 3, 9, &|_, _| 10, &|_| false);
        assert_eq!(&q[0..4], &[5, 0, 9, 0]);
        assert_eq!(q[COUNT + 1], 1);
        // Counts stay in 1..=100.
        production(&mut q, 0, 2, 1, 150, &|_, _| 10, &|_| false);
        assert_eq!(q[COUNT + 1], 100);
        production(&mut q, 0, 1, 1, 150, &|_, _| 10, &|_| false);
        assert_eq!(q[COUNT + 1], 1);
    }

    #[test]
    fn settings_copy_to_the_issuers_seen_planets() {
        let mut ps = vec![Planet::default(); 4];
        for p in &mut ps[1..] {
            p.owner = 6;
        }
        ps[2].visibility[6] = -1;
        ps[1].tax_level = 7;
        setting(&mut ps, 1, 6, 2, 0);
        assert_eq!(ps[1].tax_level, 7);
        setting(&mut ps, 1, 6, 5, 0);
        assert_eq!([ps[2].tax_level, ps[3].tax_level], [0, 7]);
    }
}
