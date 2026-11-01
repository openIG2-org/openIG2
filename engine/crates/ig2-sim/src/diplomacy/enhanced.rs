//! Ours (ADR 0045 part 3): the enhanced ruleset's diplomacy for computer-played empires (`re/rules/ai-empire.md`
//! §15.6). It picks and judges the original's messages by interest (relative power, shared enemies, borders, a
//! runaway leader, spies and broken treaties), reads synced state only and draws no random numbers.

use super::{cat, field, level, negotiate, talk, Diplomacy, Message, World, EMPIRES};

/// Days since the last message to an empire before the next one (at war / otherwise).
pub const WAR_INTERVAL: i32 = 30;
pub const INTERVAL: i32 = 90;
/// The strongest empire is a runaway leader at 1.5 × the second's power (with three or more alive).
const RUNAWAY: f32 = 1.5;
/// Spy grievance or unreliability above this: no new treaty; above [`BREAK_SPIES`] / [`BREAK_UNRELIABLE`]: a pact
/// is broken.
const GRUDGE: i16 = 1000;
const BREAK_SPIES: i16 = 2500;
const BREAK_UNRELIABLE: i16 = 3000;

impl Diplomacy {
    /// The strongest living empire (first strict maximum of `power`) when at least three live and it holds
    /// [`RUNAWAY`] × the second's power; 0 = none.
    pub(super) fn runaway_leader(&self, w: &World) -> usize {
        let (mut first, mut second, mut n) = (0, 0, 0);
        for c in (1..=w.ctx.program_players.min(EMPIRES - 1)).filter(|&c| w.ctx.alive[c]) {
            n += 1;
            if first == 0 || self.power[c] > self.power[first] {
                second = first;
                first = c;
            } else if second == 0 || self.power[c] > self.power[second] {
                second = c;
            }
        }
        if n >= 3 && self.power[first] >= self.power[second] * RUNAWAY { first } else { 0 }
    }

    /// `x` loses against `y`: out-gunned in space (danger ≥ 3) or `y` holds 1.25 × its power.
    fn weak(&self, x: usize, y: usize) -> bool {
        self.danger[x] >= 3 || self.power[y] > self.power[x] * 1.25
    }

    /// `x` holds its own against `y`: not out-gunned in space and at least as strong.
    fn winning(&self, x: usize, y: usize) -> bool {
        self.danger[x] < 3 && self.power[x] >= self.power[y]
    }

    fn grudge(&self, x: usize, y: usize) -> bool {
        let r = self.rel(x, y);
        r.v[field::SPY_GRIEVANCE] > GRUDGE || r.v[field::UNRELIABLE] > GRUDGE
    }

    /// Both `x` and `y` fight the same third empire, or a runaway leader other than them is loose.
    fn shared_enemy(&self, w: &World, x: usize, y: usize, leader: usize) -> bool {
        (leader != 0 && leader != x && leader != y)
            || (1..=w.ctx.program_players2.min(EMPIRES - 1))
                .any(|c| c != x && c != y && w.ctx.alive[c] && self.level(x, c) == level::WAR && self.level(y, c) == level::WAR)
    }

    /// `x` wants the war or cease-fire with `y` to end.
    fn peace_wanted(&self, x: usize, y: usize, leader: usize) -> bool {
        match self.level(x, y) {
            level::WAR => self.weak(x, y) || (leader != 0 && leader != x && leader != y && !self.winning(x, y)),
            level::CEASEFIRE => true,
            _ => false,
        }
    }

    fn nap_wanted(&self, w: &World, x: usize, y: usize, leader: usize, target: usize) -> bool {
        let r = self.rel(x, y);
        let near = r.v[field::DISTANCE] > 0 && r.v[field::DISTANCE] < 5000;
        r.level == level::NEUTRAL
            && !self.grudge(x, y)
            && y != target
            && y != leader
            && ((leader != 0 && leader != x)
                || (near && self.power[y] >= self.power[x] * 0.8)
                || r.v[field::OPINION] > 6000
                || self.shared_enemy(w, x, y, leader))
    }

    fn alliance_wanted(&self, w: &World, x: usize, y: usize, leader: usize, target: usize) -> bool {
        let r = self.rel(x, y);
        r.level == level::NAP
            && !self.grudge(x, y)
            && r.v[field::TRUST] > 5500
            && y != target
            && y != leader
            && self.shared_enemy(w, x, y, leader)
    }

    fn trade_wanted(&self, x: usize, y: usize, leader: usize, target: usize) -> bool {
        let r = self.rel(x, y);
        r.level >= level::NEUTRAL && r.trade == 0 && !self.grudge(x, y) && y != target && (y != leader || x == leader)
    }

    /// `x` and `y` together are a match for the runaway leader, `y` is neutral with it and `x` neutral or at war.
    fn war_together_wanted(&self, w: &World, x: usize, y: usize, leader: usize) -> bool {
        leader != 0
            && leader != x
            && leader != y
            && self.level(x, y) >= level::NEUTRAL
            && self.level(y, leader) == level::NEUTRAL
            && matches!(self.level(x, leader), level::WAR | level::NEUTRAL)
            && self.wars(w.ctx, x) - (self.level(x, leader) == level::WAR) as i32 == 0
            && self.power[x] + self.power[y] >= self.power[leader] * 0.75
    }

    /// The message an enhanced empire `a` sends to `b` (`None` = nothing to say), first match wins (§15.6).
    pub(super) fn enhanced_pick(&self, w: &World, a: usize, b: usize, leader: usize, target: usize) -> Option<Message> {
        let r = self.rel(a, b);
        let msg = |c, s, t| Some(Message::ai(c, s, t));
        if !w.ctx.story_quiet {
            if r.level >= level::NAP && (r.v[field::SPY_GRIEVANCE] > BREAK_SPIES || r.v[field::UNRELIABLE] > BREAK_UNRELIABLE) {
                let sub = if r.level == level::ALLIANCE { negotiate::BREAK_ALLIANCE } else { negotiate::BREAK_NAP };
                return msg(cat::NEGOTIATE, sub, 0);
            }
            if self.peace_wanted(a, b, leader) {
                return msg(cat::NEGOTIATE, if r.level == level::WAR { negotiate::CEASEFIRE } else { negotiate::PEACE }, 0);
            }
            if self.war_together_wanted(w, a, b, leader) {
                return msg(cat::NEGOTIATE, negotiate::WAR_TOGETHER, leader as i32);
            }
            // The empire `a` wants to attack (it is at war with nobody then), if `b` is neutral with it.
            if target != 0 && target != b && r.level >= level::NEUTRAL && self.level(b, target) == level::NEUTRAL {
                return msg(cat::NEGOTIATE, negotiate::WAR_TOGETHER, target as i32);
            }
            if self.alliance_wanted(w, a, b, leader, target) {
                return msg(cat::NEGOTIATE, negotiate::ALLIANCE, 0);
            }
            if self.nap_wanted(w, a, b, leader, target) {
                return msg(cat::NEGOTIATE, negotiate::NAP, 0);
            }
            if self.trade_wanted(a, b, leader, target) {
                return msg(cat::NEGOTIATE, negotiate::TRADE, 0);
            }
        }
        if r.v[field::SPY_GRIEVANCE] > GRUDGE && r.level > level::WAR {
            return msg(cat::TALK, talk::SPIES, 0);
        }
        None
    }

    /// Whether enhanced empire `b` accepts `a`'s negotiation `msg` (cease-fire, peace, pacts, war together, trade).
    pub(super) fn enhanced_accepts(&self, w: &World, a: usize, b: usize, msg: Message) -> bool {
        let leader = self.runaway_leader(w);
        match msg.sub {
            negotiate::CEASEFIRE | negotiate::PEACE => {
                self.level(b, a) <= level::CEASEFIRE && (self.peace_wanted(b, a, leader) || !self.winning(b, a))
            }
            negotiate::NAP => self.nap_wanted(w, b, a, leader, self.target(w, b)),
            negotiate::ALLIANCE => self.alliance_wanted(w, b, a, leader, self.target(w, b)),
            negotiate::TRADE => self.trade_wanted(b, a, leader, self.target(w, b)),
            negotiate::WAR_TOGETHER => {
                let t = msg.value.max(0) as usize;
                (t == leader && t != 0 && self.level(b, t) == level::NEUTRAL && self.wars(w.ctx, b) == 0 && self.power[a] + self.power[b] >= self.power[t] * 0.75)
                    || (t != 0 && t != a && self.level(b, t) == level::NEUTRAL && self.wars(w.ctx, b) == 0 && self.power[a] + self.power[b] >= self.power[t] * 2.0)
                    || (self.wars(w.ctx, b) == 0 && self.target(w, b) == t)
            }
            _ => false,
        }
    }
}

impl Diplomacy {
    /// `MI_DIPLOMACY`'s pair step for an enhanced empire `a`: the original's checks, then a message every
    /// [`INTERVAL`] days at most ([`WAR_INTERVAL`] at war) when [`Diplomacy::enhanced_pick`] has one; no draw.
    pub(super) fn consider_enhanced(&mut self, w: &mut World, a: usize, b: usize, leader: usize, target: usize) {
        let ctx = &*w.ctx;
        let cell = a * 9 + b;
        if a == b || !ctx.alive[a] || !ctx.alive[b] || a == 5 || b == 5 || w.starmap.met[a][b] == 0 {
            return;
        }
        if self.last_contact[cell] <= if self.level(a, b) == level::WAR { WAR_INTERVAL } else { INTERVAL } {
            return;
        }
        let waiting = !ctx.multiplayer && b == ctx.human_empire && self.slot(a).waiting & 1 != 0;
        if !(w.ctx.multiplayer || (!waiting && w.ctx.enabled)) {
            return;
        }
        if let Some(msg) = self.enhanced_pick(w, a, b, leader, target) {
            self.last_contact[cell] = 0;
            self.send(w, a, b, false, msg);
        }
    }
}
