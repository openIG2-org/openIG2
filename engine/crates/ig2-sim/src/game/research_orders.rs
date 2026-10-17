//! The research screen's lockstep messages (`MULTIGET_Research` exe 0x51ee70, ELF 0x5ec6e0; re/screens/research.md
//! §6): the orders `Research_Main`'s buttons send with `Multi_Send(type, EN, …)` and every peer applies in the same
//! frame. The handler's only screen write (`ButtonPressed = 0` for the local player) is the client's.

use super::*;
use crate::colony_side::{status, TECHS};

/// One research message's payload (the handler's `Multi_Get` values after the issuer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchOrder {
    /// 0x29 (`ResearchStartStop`, the running project's button): `ResearchAktFejlesztett[e] = 0`, tech `tech` back to
    /// researchable (status 3).
    Stop { tech: i32 },
    /// 0x2a (the same button on a researchable tech): `ResearchAktFejlesztett[e] = tech`, status 4.
    Start { tech: i32 },
    /// 0x2b (`ResearchDoubleGomb`): toggle `KutatasSurgos[e]` (urgent: double speed, double cost).
    Urgent,
    /// 0x31 (a debug key, not on the screen): every tech 0xc0..=1 that some empire 1..=8 has any status for and the
    /// issuer doesn't own is developed (`TalalmanyKifejlesztve`, then `Story_Special(0x0d, e, 5)` in SP), then
    /// `Research_TalalmanyUjraSzamol`.
    DevelopAll,
}

impl ResearchOrder {
    /// The order a `MULTIGET_Research` message carries: its type and the values after the issuer, in `Multi_Get`
    /// order.
    pub fn from_message(kind: i32, v: &[i32]) -> Option<Self> {
        let at = |i: usize| v.get(i).copied();
        Some(match kind {
            0x29 => Self::Stop { tech: at(0)? },
            0x2a => Self::Start { tech: at(0)? },
            0x2b => Self::Urgent,
            0x31 => Self::DevelopAll,
            _ => return None,
        })
    }
}

impl Game<'_> {
    /// `MULTIGET_Research`'s handling of one message from empire `issuer`. A tech outside the table (the exe would
    /// write past it) is refused.
    pub fn research_order(&mut self, issuer: u8, o: ResearchOrder) {
        let e = issuer as usize;
        if e >= EMPIRES {
            return;
        }
        let tech = |t: i32| usize::try_from(t).ok().filter(|&t| t < TECHS);
        match o {
            ResearchOrder::Stop { tech: t } | ResearchOrder::Start { tech: t } => {
                let Some(t) = tech(t) else { return };
                let start = matches!(o, ResearchOrder::Start { .. });
                let mut res = self.state.research();
                let mut techs = self.state.tech();
                res.current[e] = if start { t as i32 } else { 0 };
                techs.set_status(t, e, if start { status::RESEARCHING } else { status::RESEARCHABLE });
                self.state.store_research(&res);
                self.state.store_tech(&techs);
            }
            ResearchOrder::Urgent => {
                let mut res = self.state.research();
                res.urgent[e] = (res.urgent[e] == 0) as u8;
                self.state.store_research(&res);
            }
            ResearchOrder::DevelopAll => self.develop_all(e),
        }
    }

    /// Message 0x31 (exe 0x51ef84..0x51f024): the techs from the last down, each developed and announced before the
    /// next is looked at (the story check may change the table).
    fn develop_all(&mut self, e: usize) {
        let g = self.globals();
        let ctx = self.research_context(&g);
        for t in (1..=0xc0).rev() {
            let mut techs = self.state.tech();
            if (1..=8).all(|x| techs.status(t, x) == 0) || techs.status(t, e) == status::OWNED {
                continue;
            }
            let mut res = self.state.research();
            let mut events = Vec::new();
            research::developed(&mut techs, &mut res, e, t, &self.rules.research, &ctx, &mut events);
            self.state.store_tech(&techs);
            self.state.store_research(&res);
            for ev in &events {
                if let ResearchEvent::AutoDesign { empire, tech } = *ev {
                    self.automatic_design(empire as usize, tech);
                }
            }
            self.events.extend(events.into_iter().map(GameEvent::Research));
            if !g.multiplayer {
                self.story_special([0x0d, e as i32, 5, 0, 0, 0, 0, 0]);
            }
        }
        let mut techs = self.state.tech();
        techs.recompute(ctx.players, ctx.all_techs);
        self.state.store_tech(&techs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_decode_in_multi_get_order() {
        assert_eq!(ResearchOrder::from_message(0x29, &[17]), Some(ResearchOrder::Stop { tech: 17 }));
        assert_eq!(ResearchOrder::from_message(0x2a, &[3]), Some(ResearchOrder::Start { tech: 3 }));
        assert_eq!(ResearchOrder::from_message(0x2b, &[]), Some(ResearchOrder::Urgent));
        assert_eq!(ResearchOrder::from_message(0x31, &[]), Some(ResearchOrder::DevelopAll));
        assert_eq!(ResearchOrder::from_message(0x2a, &[]), None);
        assert_eq!(ResearchOrder::from_message(0x2c, &[1]), None);
    }
}
