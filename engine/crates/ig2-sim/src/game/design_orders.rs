//! The design orders (`MULTIGET_Design` exe 0x4b4d60, ELF 0x50c430; re/rules/design-orders.md): a new or edited
//! design (0x33, the Design screen), the sale of a design's units and the design's retirement (0x34, the Design
//! screen and the starmap's fleet and vehicle panels) and the refit of a design's units to its upgrade target (0x35,
//! the panels). The handler's UI follow-ups (`Design_UjraRendez`, `AktTerv`, `StarmapAktFlottaValtozott`, the
//! fleet view's `AktFleet`) are the client's, through [`OrderEvent`]s.

use super::*;
use crate::ai::{sale_divisor, sale_price};

/// A design record (`Tervek[e][d]`, 0x84 bytes).
pub type DesignRecord = [u8; 0x84];

/// One design message's payload (the handler's `Multi_Get` values after the issuer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignOrder {
    /// 0x33: `mode` (`DesignTervezesMod`; 3 = edit `design`, else a new design), the record, `copy` (edit into a
    /// new design and make it the old one's upgrade target).
    Store { mode: i32, design: i32, record: DesignRecord, copy: bool },
    /// 0x34: sell (`mode` 2) `holder`'s units of `design` (0 = all of them), then delete them (`holder` 0: the
    /// design too).
    Sell { design: i32, mode: i32, holder: i32 },
    /// 0x35: `holder`'s units of `design` refitted to its upgrade target.
    Refit { design: i32, holder: i32 },
}

impl DesignOrder {
    /// The order a `MULTIGET_Design` message carries: its type and the values after the issuer as dwords (0x33's
    /// record as its 33 dwords, the flag byte as one).
    pub fn from_message(kind: i32, v: &[i32]) -> Option<Self> {
        let at = |i: usize| v.get(i).copied();
        Some(match kind {
            0x33 => {
                let words = v.get(2..35)?;
                let mut record = [0; 0x84];
                for (k, w) in words.iter().enumerate() {
                    record[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
                }
                Self::Store { mode: at(0)?, design: at(1)?, record, copy: at(35)? & 1 != 0 }
            }
            0x34 => Self::Sell { design: at(0)?, mode: at(1)?, holder: at(2)? },
            0x35 => Self::Refit { design: at(0)?, holder: at(1)? },
            _ => return None,
        })
    }

    /// The message's type and dwords (the inverse of [`DesignOrder::from_message`]).
    pub fn to_message(self) -> (i32, Vec<i32>) {
        match self {
            Self::Store { mode, design, record, copy } => {
                let mut v = vec![mode, design];
                v.extend(record.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())));
                v.push(copy as i32);
                (0x33, v)
            }
            Self::Sell { design, mode, holder } => (0x34, vec![design, mode, holder]),
            Self::Refit { design, holder } => (0x35, vec![design, holder]),
        }
    }
}

fn rd_i32(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}

impl Game<'_> {
    /// `MULTIGET_Design`'s handling of one message from empire `issuer`. A design index outside the issuer's table
    /// (the original would write out of its bounds) is refused.
    pub fn design_order(&mut self, issuer: u8, o: DesignOrder) {
        let e = issuer as usize;
        if !(1..EMPIRES).contains(&e) {
            return;
        }
        let count = self.state.combat().designs.get(e).map_or(0, |t| t.len().saturating_sub(1)) as i32;
        let known = |d: i32| (1..=count).contains(&d);
        match o {
            DesignOrder::Store { mode, design: d, record, copy } => {
                if mode != 3 {
                    let mut n = 0;
                    self.with_ai(|_, w, hooks| {
                        n = w.new_design(e, 0, hooks);
                        w.copy_design(e, n, &record);
                    });
                    self.events.push(GameEvent::Order(OrderEvent::DesignStored { empire: issuer, design: n }));
                    return;
                }
                if !known(d) {
                    return;
                }
                let old = rd_i32(&self.state.combat().designs[e][d as usize].raw, 0x24) as f32;
                let new = rd_i32(&record, 0x24) as f32;
                let mut bc = 0;
                self.with_ai(|_, w, hooks| {
                    if copy {
                        let n = w.new_design(e, 0, hooks);
                        bc = n as i32;
                        w.copy_design(e, n, &record);
                        let mut r = w.des(e, d).unwrap().raw;
                        (r[0x18], r[0x19], r[0x1a]) = (0, 1, 1);
                        r[0x1c..0x20].copy_from_slice(&bc.to_le_bytes());
                        w.copy_design(e, d as usize, &r);
                        for k in 1..=w.design_count(e) {
                            let r = w.des(e, k as i32).unwrap().raw;
                            if i16::from_le_bytes([r[0x16], r[0x17]]) >= 0 && rd_i32(&r, 0x1c) == d {
                                let mut r = r;
                                r[0x1c..0x20].copy_from_slice(&bc.to_le_bytes());
                                w.copy_design(e, k, &r);
                            }
                        }
                    } else {
                        let (n, sum) = w.design_units(e, 0, d);
                        let cost = n.wrapping_mul(rd_i32(&record, 0x24)).wrapping_sub(sum).wrapping_mul(2).max(0);
                        if cost as f64 <= w.spy.colony.money[e].available(0) || cost < 1 {
                            let sw = &mut w.spy;
                            sw.colony.add_money(e, 0, -cost as f64, sw.side_rules, sw.econ);
                            w.copy_design(e, d as usize, &record);
                            w.upgrade_units_to(e, 0, d);
                            bc = d;
                        }
                    }
                });
                if !copy && bc > 0 {
                    self.change_field_designs(e, d, d, None);
                }
                self.with_ai(|_, w, _| w.rescale_queues(e, d, bc, old, new, copy));
                if bc > 0 {
                    self.events.push(GameEvent::Order(OrderEvent::DesignStored { empire: issuer, design: bc as usize }));
                }
            }
            DesignOrder::Sell { design: d, mode, holder: h } => {
                if !known(d) {
                    return;
                }
                let mut sold = None;
                if mode == 2 {
                    let g = self.globals();
                    let human = g.en;
                    self.with_ai(|_, w, _| {
                        let (n, sum) = w.design_units(e, h, d);
                        let r = w.des(e, d).unwrap().raw;
                        let price = sale_price(sum, sale_divisor(&r, w.ctx.story_resale, human));
                        if n < 1 {
                            return;
                        }
                        let sw = &mut w.spy;
                        sw.colony.add_money(e, 0, price as f64, sw.side_rules, sw.econ);
                        let hero = rd_i32(&r, 0x44) == 5 && w.ctx.story_resale;
                        if !hero {
                            let mut r = r;
                            r[0x6c..0x70].copy_from_slice(&n.to_le_bytes());
                            r[0x70] = 0;
                            for (o, x) in [(0x74, g.year), (0x78, g.month), (0x7c, g.day)] {
                                r[o..o + 4].copy_from_slice(&x.to_le_bytes());
                            }
                            r[0x1a] = 1;
                            w.copy_design(e, d as usize, &r);
                            sold = Some((n, r));
                        }
                    });
                }
                if let Some((n, r)) = sold {
                    self.offer_retired(r);
                    self.story_special([0x17, e as i32, d, n, 0, 0, 0, 0]);
                }
                self.with_ai(|_, w, _| w.delete_design(e, d, h, true));
                self.events.push(GameEvent::Order(OrderEvent::DesignSold { empire: issuer, design: d as usize, holder: h }));
            }
            DesignOrder::Refit { design: d, holder: h } => {
                if !known(d) {
                    return;
                }
                let mut field = None;
                let mut done = false;
                self.with_ai(|_, w, _| {
                    let (n, sum) = w.design_units(e, h, d);
                    let up = w.des_i32(e, w.des_i32(e, d, 0x1c), 0x24);
                    let cost = n.wrapping_mul(up).wrapping_sub(sum).wrapping_mul(2).max(0);
                    if cost as f64 <= w.spy.colony.money[e].available(0) || cost < 1 {
                        let sw = &mut w.spy;
                        sw.colony.add_money(e, 0, -cost as f64, sw.side_rules, sw.econ);
                        field = w.refit_units(e, h, d);
                        done = true;
                    }
                });
                if let Some(to) = field {
                    self.change_field_designs(e, d, to, Some(-h as usize));
                }
                if done {
                    self.events.push(GameEvent::Order(OrderEvent::Refitted { empire: issuer, design: d as usize, holder: h }));
                }
            }
        }
        // `TankAtpakol`-like changes put garrisons on (or take them off) the battlefield at once.
        self.battlefield_hand_offs();
    }

    /// The battlefields' half of a design change ([`ground::War::change_design`]): every planet's (or only
    /// `planet`'s) units of `e` with design `from` take `to`. Battlefields without such units are left alone.
    fn change_field_designs(&mut self, e: usize, from: i32, to: i32, planet: Option<usize>) {
        let all: Vec<usize> = match planet {
            Some(p) => vec![p],
            None => (1..=self.state.planet_count()).collect(),
        };
        for p in all {
            let Some(f) = self.state.ground_field(p) else { continue };
            if (0..f.units.len()).any(|i| f.redesigned(i, e as i32, from)) {
                self.with_war(p, |w| w.change_design(e as i32, from, to));
            }
        }
    }

    /// 0x34's market offer (re/rules/design-orders.md §3): the first free slot of `TradeTervek` (class < 1), else a
    /// new one while there are fewer than 40, gets the retired design at `rndlog(11, 110, 130)`% of its price
    /// (`st_CheckTradeTervAr`: 80% of that with the story's discount).
    fn offer_retired(&mut self, r: DesignRecord) {
        let mut t = self.state.trade();
        let slot = match t.offers.iter().position(|o| trade::class(o) < 1) {
            Some(k) => k,
            None if t.offers.len() < 40 => {
                t.offers.push([0; 0x84]);
                t.offers.len() - 1
            }
            None => return self.state.store_trade(&t),
        };
        let mut rng = self.state.rng();
        let pct = rng.range(trade::STREAM, 110, 130);
        self.state.store_rng(&rng);
        let mut o = r;
        let mut p = rd_i32(&o, 0x24).wrapping_mul(pct) / 100;
        if adat_i32(&self.adat(), 0x60) != 0 {
            p = (p as f64 * 0.8) as i32;
        }
        o[0x24..0x28].copy_from_slice(&p.to_le_bytes());
        t.offers[slot] = o;
        self.state.store_trade(&t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_decode_in_multi_get_order() {
        let mut record = [0u8; 0x84];
        record[0x24] = 0x34;
        record[0x83] = 0x80;
        let o = DesignOrder::Store { mode: 3, design: 5, record, copy: true };
        let (k, v) = o.to_message();
        assert_eq!((k, v.len()), (0x33, 36));
        assert_eq!(DesignOrder::from_message(k, &v), Some(o));
        assert_eq!(DesignOrder::from_message(0x34, &[7, 2, -12]), Some(DesignOrder::Sell { design: 7, mode: 2, holder: -12 }));
        assert_eq!(DesignOrder::from_message(0x35, &[7, 3]), Some(DesignOrder::Refit { design: 7, holder: 3 }));
        assert_eq!(DesignOrder::from_message(0x35, &[7]), None);
        assert_eq!(DesignOrder::from_message(0x36, &[7, 3]), None);
    }
}
