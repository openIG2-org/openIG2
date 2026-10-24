//! The trade market: the per-tick `FRISSIT_Trade` (exe 0x605ed0: delivery countdowns and arrivals, refunds, the
//! random and the story merchant, the expiry of old offers) and a purchase (`MULTIGET_Trade`, exe 0x6070b0).
//! Spec: `re/rules/trade.md`.
//!
//! Offers (`TradeTervek`) and deliveries (`TradeSzallitas`) are kept as the original's raw records, so a save
//! round-trips. Everything outside the market (designs, planets and their trade ports, money, the story designs,
//! the units a delivery builds) is reached through [`Market`]. All draws are stream 11, apart from one libc
//! `rand()` per new month that only sets the trade screen's default delivery time ([`Trade::eta_days`]).

use crate::colony_side::EMPIRES;
use crate::rng::stream;

/// The trade stream (`rndlog(11, …)`).
pub const STREAM: usize = stream::TRADE;
/// Offers on the market at most (`TradeTervek` has room for 41).
pub const OFFERS: usize = 40;
/// Deliveries per empire at most.
pub const DELIVERIES: usize = 50;
/// A design record (an offer is one).
pub const RECORD: usize = 0x84;
/// A delivery record.
pub const DELIVERY: usize = 0x98;
/// A `StoryDesignData` record (`designs.txt`, `story-data.md`).
pub const STORY_DESIGN: usize = 0x94;
/// The story merchant's empire.
pub const STORY_EMPIRE: usize = 9;
/// Building kind of a trade port.
pub const TRADE_PORT: u8 = 50;
/// An offer may expire once it is older than this (`DatumToIdo(0, 7, 1, 0)`, 180 days).
const EXPIRY_AGE: i32 = 9000;
/// The random merchant picks designs from here on (the scenario's own come first).
const FIRST_MERCHANT_DESIGN: i32 = 0x29;
/// `st_GetTerv` looks for an existing story design from here on.
const FIRST_STORY_DESIGN: usize = 0x28;

pub type Record = [u8; RECORD];

fn rd_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn wr_i32(b: &mut [u8], o: usize, v: i32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// The design class `+0x16` (1 ship, 2 vehicle; an offer slot with class < 1 is free).
pub fn class(r: &[u8]) -> i16 {
    rd_i16(r, 0x16)
}

/// `TradeAr(i, n)`: the price of `n` units of an offer (a fighter unit is a fifth of the listed price per fighter).
pub fn price(offer: &Record, n: i32) -> i32 {
    let total = rd_i32(offer, 0x24).wrapping_mul(n);
    if offer[0x36] == 0 { total } else { total / 5 }
}

/// `DatumToIdo(ev, honap, nap, 0)` in the original's i32 arithmetic.
fn date_to_ido(year: i32, month: i32, day: i32) -> i32 {
    day.wrapping_add(month.wrapping_add(year.wrapping_mul(12)).wrapping_mul(30)).wrapping_sub(31).wrapping_mul(50)
}

/// `strcpy` into a record's name field (bounded by the record).
fn strcpy(r: &mut Record, src: &[u8]) {
    let n = src.iter().position(|&c| c == 0).unwrap_or(src.len());
    let n = n.min(RECORD - 1);
    r[..n].copy_from_slice(&src[..n]);
    r[n] = 0;
}

/// `strcat` onto a record's name (bounded by the record).
fn strcat(r: &mut Record, src: &[u8]) {
    let at = r.iter().position(|&c| c == 0).unwrap_or(RECORD - 1);
    let n = src.iter().position(|&c| c == 0).unwrap_or(src.len()).min(RECORD - 1 - at);
    r[at..at + n].copy_from_slice(&src[..n]);
    r[at + n] = 0;
}

/// `TervEqual2(a, b)`: same class, original owner, hull, hull tech, slot count and slot contents.
pub fn same_design(a: &Record, b: &Record) -> bool {
    if class(a) != class(b) || rd_i16(a, 0x40) != rd_i16(b, 0x40) {
        return false;
    }
    if (0x44..0x50).step_by(4).any(|o| rd_i32(a, o) != rd_i32(b, o)) {
        return false;
    }
    let slots = rd_i32(a, 0x4c);
    (1..=slots.max(0) as usize).all(|k| a.get(0x50 + k) == b.get(0x50 + k) && a.get(0x5d + k) == b.get(0x5d + k))
}

/// A delivery (`TradeSzallitas[e][i]`, also the purchase message): `+0` planet, `+4` days left, `+8` offer index,
/// `+0xc` units, `+0x10` price paid, `+0x14` the design as offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delivery(pub [u8; DELIVERY]);

impl Default for Delivery {
    fn default() -> Self {
        Self([0; DELIVERY])
    }
}

impl Delivery {
    pub fn planet(&self) -> i32 {
        rd_i32(&self.0, 0)
    }
    pub fn set_planet(&mut self, p: i32) {
        wr_i32(&mut self.0, 0, p);
    }
    pub fn days(&self) -> i32 {
        rd_i32(&self.0, 4)
    }
    pub fn set_days(&mut self, d: i32) {
        wr_i32(&mut self.0, 4, d);
    }
    pub fn offer(&self) -> i32 {
        rd_i32(&self.0, 8)
    }
    pub fn units(&self) -> i32 {
        rd_i32(&self.0, 0xc)
    }
    pub fn price(&self) -> i32 {
        rd_i32(&self.0, 0x10)
    }
    pub fn design(&self) -> Record {
        self.0[0x14..].try_into().unwrap()
    }
}

/// The market's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    /// `TradeTervek[0..TradeHanyTradeCuccVan]`.
    pub offers: Vec<Record>,
    /// `TradeSzallitas[e][0..TradeHanySzallitas[e]]` (index 0 unused).
    pub deliveries: Vec<Vec<Delivery>>,
    /// `TradeOldNap`: the day of the last daily pass (new game: −1).
    pub old_day: i32,
    /// `OldEtaHonap`: the month of the last delivery-time draw (new game: −1; not saved).
    pub eta_month: i32,
    /// `AktEtaDays`: the trade screen's default delivery time, 20..=39 days (not saved).
    pub eta_days: i32,
}

impl Default for Trade {
    fn default() -> Self {
        Self { offers: Vec::new(), deliveries: vec![Vec::new(); EMPIRES], old_day: -1, eta_month: -1, eta_days: 0 }
    }
}

/// The tick's inputs (and the story merchant's countdown, which it updates).
#[derive(Debug, Clone, Default)]
pub struct MarketContext {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    /// `IdoSpeedSzorzo`.
    pub speed_mult: i32,
    /// `ProgramPlayers`.
    pub program_players: usize,
    /// `PlayerKi[e] == 1`.
    pub human_controlled: [bool; EMPIRES],
    /// `DEBUGX`: every own planet has a trade port.
    pub debug: bool,
    /// `TradeCheat`: every own planet is a re-targeting destination.
    pub cheat: bool,
    /// `StoryAdat+0xc4`: the story merchant comes when it drops below 0.
    pub story_countdown: i32,
    /// `StoryAdat+0x60`: new offers are 20% cheaper (`st_CheckTradeTervAr`).
    pub story_discount: bool,
    /// `StoryDesignData[0..StoryDesignDataMut]`.
    pub story_designs: Vec<[u8; STORY_DESIGN]>,
    /// `StoryDesignElsoTerv`: the next free story design id (they start at 10000).
    pub story_next_id: i32,
}

/// What a tick did, for the UI and the other systems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeEvent {
    /// A delivery arrived: `design` is the buyer's design index the units were built from (`Legyart` and
    /// `Story_Special(0x13, …)` ran through [`Market::deliver`]).
    Arrived { empire: u8, planet: i32, design: usize, units: i32 },
    /// A delivery whose destination was lost and that had no trade port to go to: its price was refunded.
    Refunded { empire: u8, price: i32 },
    /// A delivery's destination was lost: it goes to the empire's last trade-port planet.
    Retargeted { empire: u8, planet: i32 },
    /// A merchant put design `design` of `empire` on the market in slot `slot`.
    Offered { slot: usize, empire: u8, design: usize, units: i32 },
    /// The offer in `slot` is named `DesignPoenosanElnevez(owner, hull, special)` (localised text from string
    /// table 0x3b; [`Market::design_name`] gave the port's text).
    OfferNamed { slot: usize, owner: i16, hull: i32, special: i32 },
    /// The offer in `slot` expired (the trade screen drops it if selected).
    OfferWithdrawn { slot: usize },
    /// A purchase was accepted (`e == EN`: the trade screen updates its selection).
    Bought { empire: u8, offer: i32, units: i32 },
}

/// The systems the market reaches.
pub trait Market {
    /// `rndlog(11, lo, hi)`.
    fn rnd(&mut self, lo: i32, hi: i32) -> i32;
    /// libc `rand()`.
    fn libc_rand(&mut self) -> i32;
    /// `HanyBolygo`.
    fn planet_count(&self) -> usize;
    /// `Bolygok[p].+0x20`.
    fn planet_owner(&self, p: i32) -> u8;
    /// `SearchBuilding(p, kind, true)`: finished, working buildings of a kind.
    fn working_buildings(&self, p: usize, kind: u8) -> i32;
    /// `Talalmanyok[t].status[e]`.
    fn tech_status(&self, t: i32, e: usize) -> u8;
    /// `Talalmanyok[t]` name (the live, localised table).
    fn tech_name(&self, t: i32) -> Vec<u8>;
    /// `Alapok[hull].+0xc`: the hull's tech.
    fn hull_tech(&self, hull: i32) -> i32;
    /// `OsszHanyTerv[e]`.
    fn design_count(&self, e: usize) -> usize;
    fn design(&self, e: usize, d: usize) -> Option<Record>;
    fn set_design(&mut self, e: usize, d: usize, r: &Record);
    /// `TervNew(e, id)` (`ai-empire.md` §10.2).
    fn new_design(&mut self, e: usize, id: i32) -> usize;
    /// `TervUjraSzamol(e, d, false, price)` (`space-combat.md` §2.4).
    fn recompute_design(&mut self, e: usize, d: usize, price: bool);
    /// `MoneyMennyi(e, 0)`.
    fn money(&self, e: usize) -> f64;
    /// `MoneyHozzaadLog(e, 0, x)` (an AI's income is scaled).
    fn add_money(&mut self, e: usize, x: f64);
    /// `SearchBolygohozKozeliFlotta(planet, 6, own, idle, false, false, 0)`, `Legyart(e, planet, f, d, units, 0,
    /// class == 1, …, single)` and `Story_Special(0x13, e, d, units, planet, 0, 0, 0)`.
    fn deliver(&mut self, e: usize, planet: i32, d: usize, units: i32);
    /// `DesignPoenosanElnevez(owner, hull, special)`; without the string table: the hull tech's name (its
    /// last fallback).
    fn design_name(&mut self, _owner: i16, hull: i32, _special: i32) -> Vec<u8> {
        let t = self.hull_tech(hull);
        self.tech_name(t)
    }
}

/// How a new offer is named.
enum Naming {
    /// After its hull tech (`Talalmanyok[+0x48]`).
    HullTech,
    /// `DesignPoenosanElnevez(…) + " " + the design's name`.
    Generated,
    /// The design's own name.
    Keep,
}

/// The tech a human must have for the story merchant to offer a hull (the `switch` at exe 0x606486).
fn hull_prerequisite(hull: i32) -> Option<i32> {
    Some(match hull {
        4 => 2,
        6 => 5,
        7 => 6,
        8 => 7,
        9 => 8,
        10 => 9,
        11 => 10,
        20 => 0x91,
        21 | 23 => 0x92,
        22 => 0x93,
        _ => return None,
    })
}

impl Trade {
    /// `FRISSIT_Trade` (spec §3).
    pub fn tick(&mut self, ctx: &mut MarketContext, m: &mut dyn Market) -> Vec<TradeEvent> {
        let mut ev = Vec::new();
        if self.eta_month != ctx.month {
            self.eta_days = m.libc_rand() % 20 + 20;
            self.eta_month = ctx.month;
        }
        let new_day = self.old_day != ctx.day;
        if new_day {
            self.old_day = ctx.day;
            self.countdowns(m, &mut ev);
        }
        for d in self.deliveries.iter_mut().skip(1) {
            d.retain(|x| x.days() != 0);
        }
        self.destinations(ctx, m, &mut ev);
        self.merchants(ctx, m, &mut ev);
        if new_day {
            self.expiry(ctx, m, &mut ev);
        }
        ev
    }

    /// The daily countdowns; a delivery reaching 0 arrives (spec §3.2).
    fn countdowns(&mut self, m: &mut dyn Market, ev: &mut Vec<TradeEvent>) {
        for e in 1..self.deliveries.len().min(EMPIRES) {
            for i in 0..self.deliveries[e].len() {
                let d = &mut self.deliveries[e][i];
                if d.days() > 0 {
                    d.set_days(d.days() - 1);
                }
                if d.days() == 0 {
                    let d = *d;
                    arrive(e, &d, m, ev);
                }
            }
        }
    }

    /// Deliveries whose destination isn't the buyer's any more (spec §3.3).
    fn destinations(&mut self, ctx: &MarketContext, m: &mut dyn Market, ev: &mut Vec<TradeEvent>) {
        for e in 1..self.deliveries.len().min(EMPIRES) {
            // The original repeats until a pass changes nothing; a re-targeted delivery is settled, so two passes
            // (the second for the deliveries skipped after a removal) and a bound are enough.
            for _ in 0..=DELIVERIES {
                let mut changed = false;
                let mut i = 0;
                while i < self.deliveries[e].len() {
                    if m.planet_owner(self.deliveries[e][i].planet()) as usize != e {
                        changed = true;
                        if has_port(e, ctx, m) {
                            let p = port_planet(e, ctx, m);
                            self.deliveries[e][i].set_planet(p);
                            ev.push(TradeEvent::Retargeted { empire: e as u8, planet: p });
                        } else {
                            let price = self.deliveries[e][i].price();
                            m.add_money(e, price as f64);
                            self.deliveries[e].remove(i);
                            ev.push(TradeEvent::Refunded { empire: e as u8, price });
                        }
                    }
                    // After a removal the delivery moved into slot i waits for the next pass, as in the original.
                    i += 1;
                }
                if !changed {
                    break;
                }
            }
        }
    }

    /// The random merchant, else the story merchant (spec §3.4, §3.5).
    fn merchants(&mut self, ctx: &mut MarketContext, m: &mut dyn Market, ev: &mut Vec<TradeEvent>) {
        let free = OFFERS as i32 - self.offers.len() as i32 + self.offers.iter().filter(|o| class(&o[..]) < 1).count() as i32;
        ctx.story_countdown = ctx.story_countdown.wrapping_sub(ctx.speed_mult);
        if free < 1 {
            return;
        }
        if m.rnd(0, 2000) < ctx.speed_mult {
            self.random_merchant(ctx, m, ev);
        } else if ctx.story_countdown < 0 {
            self.story_merchant(ctx, m, ev);
        }
    }

    fn random_merchant(&mut self, ctx: &MarketContext, m: &mut dyn Market, ev: &mut Vec<TradeEvent>) {
        let e = m.rnd(1, ctx.program_players as i32);
        let kind = m.rnd(1, 2);
        let mut n = m.rnd(1, 3);
        let mut fighters = m.rnd(1, 3) == 1;
        let mut colony = m.rnd(1, 10) == 1;
        let mut terraform = m.rnd(1, 20) == 1;
        let mut mobile_base = m.rnd(1, 30) == 1;
        if kind == 2 {
            (fighters, colony, terraform, mobile_base) = (false, false, false, false);
        } else if mobile_base {
            n = 1;
            (fighters, colony, terraform) = (false, false, false);
        } else if terraform {
            (fighters, colony) = (false, false);
        } else if colony {
            fighters = false;
        }
        let dear = m.rnd(1, 3) != 1 || colony || terraform;
        let r = if dear { m.rnd(110, 140) } else { m.rnd(70, 120) };
        let factor = (r as f64 / 100.0) as f32;
        if fighters {
            n = n.wrapping_mul(5);
        }
        let wish = fighters || colony || terraform || mobile_base;
        let e = e.max(0) as usize;
        let mut tries = 0;
        let d = loop {
            let d = m.rnd(FIRST_MERCHANT_DESIGN, m.design_count(e) as i32);
            tries += 1;
            if tries >= 500 {
                return;
            }
            // Past the table the original reads the heap; nothing there is a live design of the wanted class.
            let r = usize::try_from(d).ok().and_then(|d| m.design(e, d)).unwrap_or([0; RECORD]);
            if class(&r) as i32 != kind || rd_i32(&r, 0x80) != 0 {
                continue;
            }
            let plain = r[0x34] == 0 && r[0x35] == 0 && r[0x38] == 0;
            if (r[0x36] == 0 && plain && !wish)
                || (r[0x36] != 0 && fighters)
                || (r[0x34] != 0 && colony)
                || (r[0x35] != 0 && terraform)
                || (r[0x38] != 0 && mobile_base)
            {
                break (d as usize, r);
            }
        };
        if d.1[0x37] != 0 {
            return; // a space station is never offered
        }
        self.offer(e, d.0, n, factor, Naming::HullTech, ctx, m, ev);
    }

    fn story_merchant(&mut self, ctx: &mut MarketContext, m: &mut dyn Market, ev: &mut Vec<TradeEvent>) {
        ctx.story_countdown = m.rnd(120, 200).wrapping_mul(50);
        let mut tries = 0;
        let (id, owned) = loop {
            let Some((id, hull)) = random_story_design(ctx, m) else { return };
            let mut ok = true;
            let mut owned = false;
            let ht = m.hull_tech(hull);
            for p in 1..=ctx.program_players.min(EMPIRES - 1) {
                if !ctx.human_controlled[p] {
                    continue;
                }
                if let Some(t) = hull_prerequisite(hull) {
                    if m.tech_status(t, p) != 5 {
                        ok = false;
                    }
                }
                if m.tech_status(ht, p) == 5 {
                    owned = true;
                }
            }
            if tries < 5 && owned {
                ok = false;
            }
            tries += 1;
            if tries >= 500 {
                return;
            }
            if ok {
                break (id, owned);
            }
        };
        let d = story_design(STORY_EMPIRE, id, 3, &ctx.story_designs, m);
        let hull_tech = m.design(STORY_EMPIRE, d).map_or(0, |r| rd_i32(&r, 0x48));
        let n = if hull_tech < 5 { 5 } else { 1 };
        let r = if owned { m.rnd(80, 120) } else { m.rnd(110, 140) };
        let factor = (r as f64 / 100.0) as f32;
        let naming = if owned {
            Naming::HullTech
        } else if hull_tech > 4 {
            Naming::Generated
        } else {
            Naming::Keep
        };
        self.offer(STORY_EMPIRE, d, n, factor, naming, ctx, m, ev);
    }

    /// Design `d` of `e` goes on the market (spec §3.6).
    #[allow(clippy::too_many_arguments)]
    fn offer(&mut self, e: usize, d: usize, n: i32, factor: f32, naming: Naming, ctx: &MarketContext, m: &mut dyn Market,
        ev: &mut Vec<TradeEvent>) {
        let mut r = m.design(e, d).unwrap_or([0; RECORD]);
        wr_i32(&mut r, 0x6c, n);
        r[0x70] = 0;
        wr_i32(&mut r, 0x74, ctx.year);
        wr_i32(&mut r, 0x78, ctx.month);
        wr_i32(&mut r, 0x7c, ctx.day);
        m.set_design(e, d, &r);
        let slot = match self.offers.iter().position(|o| class(&o[..]) < 1) {
            Some(i) => i,
            None if self.offers.len() < OFFERS => {
                self.offers.push([0; RECORD]);
                self.offers.len() - 1
            }
            None => return,
        };
        let mut o = r;
        match naming {
            Naming::HullTech => {
                let name = m.tech_name(rd_i32(&o, 0x48));
                strcpy(&mut o, &name);
            }
            Naming::Generated => {
                let special = m.rnd(0, 1);
                let (owner, hull) = (rd_i16(&o, 0x40), rd_i32(&o, 0x44));
                strcpy(&mut o, &m.design_name(owner, hull, special));
                strcat(&mut o, b" ");
                strcat(&mut o, &r);
                ev.push(TradeEvent::OfferNamed { slot, owner, hull, special });
            }
            Naming::Keep => {}
        }
        let mut p = (rd_i32(&o, 0x24) as f32 * factor) as i32;
        if ctx.story_discount {
            p = (p as f64 * 0.8) as i32;
        }
        wr_i32(&mut o, 0x24, p);
        self.offers[slot] = o;
        ev.push(TradeEvent::Offered { slot, empire: e as u8, design: d, units: n });
    }

    /// Offers older than 180 days may expire, one draw per offer and day (spec §3.7).
    fn expiry(&mut self, ctx: &MarketContext, m: &mut dyn Market, ev: &mut Vec<TradeEvent>) {
        let now = date_to_ido(ctx.year, ctx.month, ctx.day);
        for (slot, o) in self.offers.iter_mut().enumerate() {
            let then = date_to_ido(rd_i32(o, 0x74), rd_i32(o, 0x78), rd_i32(o, 0x7c));
            if EXPIRY_AGE < now.wrapping_sub(then) && m.rnd(0, 500) < ctx.speed_mult {
                o[0x16..0x18].copy_from_slice(&(-1i16).to_le_bytes());
                ev.push(TradeEvent::OfferWithdrawn { slot });
            }
        }
    }

    /// `MULTIGET_Trade` for one message from empire `e` (spec §4): returns whether the purchase was accepted.
    pub fn buy(&mut self, e: usize, msg: &Delivery, m: &mut dyn Market) -> Option<TradeEvent> {
        if e >= self.deliveries.len() || self.deliveries[e].len() >= DELIVERIES {
            return None;
        }
        let cost = msg.price().wrapping_neg();
        // The offer index isn't checked in the original; past the market it reads stale records.
        let o = usize::try_from(msg.offer()).ok().filter(|&o| o < self.offers.len())?;
        let offer = &self.offers[o];
        let ok = cost as f64 <= m.money(e)
            && class(offer) >= 0
            && msg.units() <= rd_i32(offer, 0x6c)
            && m.planet_owner(msg.planet()) as usize == e;
        if !ok {
            return None;
        }
        m.add_money(e, cost as f64);
        self.deliveries[e].push(*msg);
        let offer = &mut self.offers[o];
        let left = rd_i32(offer, 0x6c).wrapping_sub(msg.units());
        wr_i32(offer, 0x6c, left);
        if left < 1 {
            offer[0x16..0x18].copy_from_slice(&(-1i16).to_le_bytes());
        }
        Some(TradeEvent::Bought { empire: e as u8, offer: msg.offer(), units: msg.units() })
    }
}

/// `MegerkezikSzallitmany(e, D)` (spec §3.2).
fn arrive(e: usize, d: &Delivery, m: &mut dyn Market, ev: &mut Vec<TradeEvent>) {
    let mut t = d.design();
    if class(&t) == 2 {
        t[0x40..0x42].copy_from_slice(&(e as i16).to_le_bytes());
    }
    let mut found = 0;
    for k in FIRST_MERCHANT_DESIGN as usize..=m.design_count(e) {
        if let Some(r) = m.design(e, k) {
            if class(&r) >= 0 && same_design(&r, &t) {
                found = k;
            }
        }
    }
    if found == 0 {
        found = m.new_design(e, 0);
        t[0x18] = 0;
        t[0x1a] = 1;
        m.set_design(e, found, &t);
    }
    m.recompute_design(e, found, false);
    m.deliver(e, d.planet(), found, d.units());
    ev.push(TradeEvent::Arrived { empire: e as u8, planet: d.planet(), design: found, units: d.units() });
}

/// `VanBolygoja(e)`: `e` owns a planet with a working trade port.
fn has_port(e: usize, ctx: &MarketContext, m: &dyn Market) -> bool {
    (1..=m.planet_count()).any(|p| m.planet_owner(p as i32) as usize == e && (m.working_buildings(p, TRADE_PORT) > 0 || ctx.debug))
}

/// `MelyikBolygojanVanMegKereskedelmiUrkikoto(e)`: the last such planet (`TradeCheat` counts too), else 0.
fn port_planet(e: usize, ctx: &MarketContext, m: &dyn Market) -> i32 {
    (1..=m.planet_count())
        .rev()
        .find(|&p| m.planet_owner(p as i32) as usize == e && (m.working_buildings(p, TRADE_PORT) > 0 || ctx.debug || ctx.cheat))
        .map_or(0, |p| p as i32)
}

/// `st_GetRandomTradeTerv(&hull)`: a random story design id and its hull, `None` for an id no record has.
fn random_story_design(ctx: &MarketContext, m: &mut dyn Market) -> Option<(i32, i32)> {
    let id = m.rnd(10_000, ctx.story_next_id.wrapping_sub(1));
    let rec = ctx.story_designs.iter().find(|s| rd_i32(&s[..], 0) == id)?;
    Some((id, rd_i32(rec, 0x24)))
}

/// `st_GetTerv(e, id, def)`: the design of `e` made from story design `id`, made on first use (spec §3.5).
pub(crate) fn story_design(e: usize, id: i32, def: usize, story_designs: &[[u8; STORY_DESIGN]], m: &mut dyn Market) -> usize {
    let n = m.design_count(e);
    if n >= FIRST_STORY_DESIGN {
        if let Some(k) = (FIRST_STORY_DESIGN..=n).find(|&k| m.design(e, k).is_some_and(|r| rd_i32(&r, 0x80) == id)) {
            return k;
        }
    }
    let Some(s) = story_designs.iter().find(|s| rd_i32(&s[..], 0) == id) else { return def };
    let k = m.new_design(e, rd_i32(s, 0x20));
    let mut r = m.design(e, k).unwrap_or([0; RECORD]);
    strcpy(&mut r, &s[4..]);
    wr_i32(&mut r, 0x80, id);
    r[0x18] = (rd_i32(s, 0x1c) != 0) as u8;
    r[0x19] = 1;
    wr_i32(&mut r, 0x44, rd_i32(s, 0x24));
    for j in 1..=rd_i32(s, 0x28).max(0) as usize {
        if 0x2c + 4 * j + 4 > STORY_DESIGN || 0x60 + 4 * j + 4 > STORY_DESIGN || 0x5d + j >= RECORD {
            break;
        }
        r[0x50 + j] = s[0x60 + 4 * j];
        r[0x5d + j] = s[0x2c + 4 * j];
    }
    m.set_design(e, k, &r);
    m.recompute_design(e, k, true);
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_prices() {
        assert_eq!(date_to_ido(0, 7, 1), 9000);
        assert_eq!(date_to_ido(2327, 10, 26), crate::time::GameTime::from_date(2327, 10, 26, 0).0);
        let mut o = [0u8; RECORD];
        wr_i32(&mut o, 0x24, 4704);
        assert_eq!(price(&o, 3), 14112);
        o[0x36] = 1;
        assert_eq!(price(&o, 3), 2822);
    }

    #[test]
    fn names_are_c_strings() {
        let mut r = [0x55u8; RECORD];
        strcpy(&mut r, b"Aquamarine\0junk");
        strcat(&mut r, b" ");
        strcat(&mut r, b"r284\0");
        assert_eq!(&r[..16], b"Aquamarine r284\0");
        assert_eq!(r[16], 0x55);
    }
}
