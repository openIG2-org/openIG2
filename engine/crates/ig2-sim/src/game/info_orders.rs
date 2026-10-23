//! The info screen's lockstep messages (`MULTIGET_Info` exe 0x4ece60, ELF 0x569360; re/screens/info.md §7): the
//! money triangle's shares (0x5b) and the spending limit (0x5c) that `Info_Main`'s hot-spots 29 and 32 send with
//! `Multi_Send(type, EN, …)`. The planet settings selectors (26..=28) send the starmap's 0x19 instead.

use super::*;
use crate::ai::AiState;
use crate::save::va;

/// One info message's payload (the handler's `Multi_Get` values after the issuer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InfoOrder {
    /// 0x5b: `MoneyTeruletCsuszka[e][1..=3]` = `sliders` (production, buildings, research), `[4]` and `[5]` = 0,
    /// then `MoneyInit(false, e)` (exe 0x540060).
    Shares { sliders: [f64; 3] },
    /// 0x5c: `MoneyHatar[e][0]` (the overall reserve, the screen's spending limit) = `limit`.
    Limit { limit: f64 },
}

impl InfoOrder {
    /// The order a `MULTIGET_Info` message carries: its type and the values after the issuer as dwords, each f64 as
    /// its low then high word (`Multi_Get` order).
    pub fn from_message(kind: i32, v: &[i32]) -> Option<Self> {
        let f = |i: usize| -> Option<f64> {
            let (lo, hi) = (*v.get(2 * i)? as u32 as u64, *v.get(2 * i + 1)? as u32 as u64);
            Some(f64::from_bits(hi << 32 | lo))
        };
        Some(match kind {
            0x5b => Self::Shares { sliders: [f(0)?, f(1)?, f(2)?] },
            0x5c => Self::Limit { limit: f(0)? },
            _ => return None,
        })
    }

    /// The message's type and dwords (the inverse of [`InfoOrder::from_message`]).
    pub fn to_message(self) -> (i32, Vec<i32>) {
        let words = |x: &[f64]| x.iter().flat_map(|x| [x.to_bits() as u32 as i32, (x.to_bits() >> 32) as u32 as i32]).collect();
        match self {
            Self::Shares { sliders } => (0x5b, words(&sliders)),
            Self::Limit { limit } => (0x5c, words(&[limit])),
        }
    }
}

impl Game<'_> {
    /// `MULTIGET_Info`'s handling of one message from empire `issuer` (no checks in the exe: the issuer is a slot).
    pub fn info_order(&mut self, issuer: u8, o: InfoOrder) {
        let e = issuer as usize;
        if e >= EMPIRES {
            return;
        }
        let at = |k: usize| va::SLIDERS + (e * 48 + 8 * k) as u32;
        match o {
            InfoOrder::Shares { sliders } => {
                for (k, s) in sliders.into_iter().enumerate() {
                    self.state.mem.set_f64(at(k + 1), s);
                }
                self.state.mem.set_f64(at(4), 0.0);
                self.state.mem.set_f64(at(5), 0.0);
                self.money_init(e);
            }
            InfoOrder::Limit { limit } => {
                let mut money = self.state.treasuries();
                money[e].reserve[0] = limit;
                self.state.store_treasuries(&money);
            }
        }
    }

    /// `MoneyInit(false, e)` (exe 0x540060): every empire's shares from its sliders (`MoneyTeruletSzorzoCalculate`),
    /// then for `e` (every empire for 0) the pools 1..=5 re-dealt from the total by the new shares and their
    /// reserves cleared.
    pub(super) fn money_init(&mut self, e: usize) {
        let mut ai = AiState::default();
        for (x, s) in ai.sliders.iter_mut().enumerate() {
            *s = std::array::from_fn(|k| self.state.mem.f64(va::SLIDERS + (x * 48 + 8 * k) as u32));
        }
        let mut money = self.state.treasuries();
        ai.income_shares(&mut money);
        for (x, t) in money.iter_mut().enumerate().skip(1) {
            if e == x || e == 0 {
                for k in 1..6 {
                    t.pools[k] = t.share[k] * t.total;
                    t.reserve[k] = 0.0;
                }
            }
        }
        self.state.store_treasuries(&money);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_decode_in_multi_get_order() {
        let o = InfoOrder::Shares { sliders: [0.5, 0.25, -0.0] };
        let (k, v) = o.to_message();
        assert_eq!((k, v.len()), (0x5b, 6));
        assert_eq!(InfoOrder::from_message(k, &v), Some(o));
        // 1500.0 = 0x4097700000000000: low word first.
        assert_eq!(InfoOrder::from_message(0x5c, &[0, 0x40977000]), Some(InfoOrder::Limit { limit: 1500.0 }));
        assert_eq!(InfoOrder::from_message(0x5c, &[0]), None);
        assert_eq!(InfoOrder::from_message(0x5d, &[0, 0]), None);
    }
}
