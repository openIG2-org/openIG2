//! Research: progress every tick (`FRISSIT_Research`, exe 0x51ddd0), a developed tech
//! (`TalalmanyKifejlesztve`, 0x524e50) and the AI's next pick (`FRISSIT_MI_FEJLESZTES`, 0x50b0b0).
//! Spec: `re/rules/production-research.md` §2.
//!
//! Ours (ADR 0037): auto research for human empires, [`auto_pick`].
//!
//! Not ported: the AI's new designs (`FRISSIT_MI_DESIGN`), automatic designs for the human
//! (`TervezAutomatikus`), messages, tutorial and story hooks; all reported as [`ResearchEvent`]s.

use crate::colony_side::{status, TechTable, Treasury, EMPIRES, TECHS};
use crate::time::slowed_down;

/// Tech 0x95 also develops tech 0xAA.
const LINKED: (usize, usize) = (0x95, 0xaa);

/// Rule tables research reads (from the user's exe, ADR 0004).
#[derive(Debug, Clone, Default)]
pub struct ResearchRules {
    /// `FajSpeckoKutatasIdoSzorzo[race]` / `FajSpeckoKutatasMoneySzorzo[race]`.
    pub race_time: Vec<f32>,
    pub race_money: Vec<f32>,
    /// `FejlesztesSebessegSzorzo[difficulty]`: AI research speed.
    pub difficulty_speed: Vec<f32>,
    /// `NehezsegPenzSzorzo[difficulty]`: AI income (`MoneyHozzaad`).
    pub difficulty_money: Vec<f32>,
}

/// Per-empire research state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Research {
    /// `ResearchAktFejlesztett[e]` (exe 0xB9A9B8): the tech being researched, 0 = none.
    pub current: [i32; EMPIRES],
    /// `KutatasSurgos[e]` (exe 0xB9C014): urgent (double speed, double cost).
    pub urgent: [u8; EMPIRES],
}

impl Default for Research {
    fn default() -> Self {
        Self { current: [0; EMPIRES], urgent: [0; EMPIRES] }
    }
}

/// Global inputs of a research tick.
#[derive(Debug, Clone, Default)]
pub struct ResearchContext {
    /// `IdoSpeedSzorzo`.
    pub speed_mult: i32,
    /// `Nap` (1..=30).
    pub day: i32,
    /// `Nehezseg` by empire (ADR 0033: an MP computer's own level).
    pub difficulty: [usize; EMPIRES],
    pub multiplayer: bool,
    /// `EN`.
    pub human_empire: u8,
    /// `PlayerKi[e] == 1`.
    pub human_controlled: [bool; EMPIRES],
    /// `PlayerFaj[e]`.
    pub race: [u8; EMPIRES],
    /// `StoryAdat+0x6c`: the human's research bonus in percent (SP).
    pub story_bonus: i32,
    /// The research cheat (exe 0xB7727C): the human ignores labs; every running project completes.
    pub cheat: bool,
    /// The "all techs" switch (exe 0xB77254), for the status recompute.
    pub all_techs: bool,
    /// `ProgramPlayers2`, for the status recompute.
    pub players: i32,
    /// `nemlehetidotallitani` (exe 0xBAD0D4): scripted, the speed can't change (SP).
    pub speed_locked: bool,
    /// `ProgramSebesseg` (exe 0xBAE54C): the MP game speed 1..=4.
    pub program_speed: i32,
    /// Ours (ADR 0037): each empire's auto-research switches ([`auto`] bits; `Game::auto_research`).
    pub auto: [u8; EMPIRES],
    /// Out: `SetIdoLelassit` ran (the caller writes `IdoSpeed` and the battle multiplier, [`crate::time::slow_down`]).
    pub slowed: bool,
}

impl ResearchContext {
    /// `SetIdoLelassit` ([`crate::time::slowed_down`]). It takes effect at once: empires later in the
    /// same research loop, and the rest of the tick, run at the new `IdoSpeedSzorzo`.
    pub fn slow_down(&mut self) {
        self.speed_mult = slowed_down(self.speed_mult, self.multiplayer, self.speed_locked, self.program_speed);
        self.slowed = true;
    }
}

/// What other systems or the UI do after a research tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchEvent {
    /// A project finished (story special 0x0D; the AI then designs ships, `FRISSIT_MI_DESIGN`; the
    /// human gets a message, a slow-down and tutorial checks 9/10 for techs 0x12/6).
    Completed { empire: u8, tech: usize },
    /// `TervezAutomatikus(e, t)`: a human-controlled empire gets an automatic design.
    AutoDesign { empire: u8, tech: usize },
    /// The AI started a new project (status 4), or auto research did (ours, [`auto_pick`]).
    Started { empire: u8, tech: usize },
}

pub(crate) fn labs_ok(techs: &TechTable, t: usize, labs: &[i32; 4]) -> bool {
    let need = techs.labs_needed(t);
    (0..3).all(|i| need[i] as i32 <= labs[i + 1])
}

/// Research of all empires for one tick, `labs` = `LaborSzint`. A human completion slows the game
/// down ([`ResearchContext::slow_down`]): `ctx.speed_mult` is the clock's value afterwards.
pub fn tick(techs: &mut TechTable, research: &mut Research, labs: &[[i32; 4]; EMPIRES], money: &mut [Treasury; EMPIRES],
    rules: &ResearchRules, ctx: &mut ResearchContext) -> Vec<ResearchEvent> {
    let mut events = Vec::new();
    for e in 1..EMPIRES {
        let dev = rules.difficulty_speed.get(ctx.difficulty[e]).copied().unwrap_or(1.0);
        if research.current[e] < 1 {
            auto_pick(techs, research, labs, e, ctx, &mut events);
        }
        let t = research.current[e];
        if t < 1 {
            money[e].reserve[3] = 0.0;
            continue;
        }
        let t = t as usize;
        if t >= TECHS {
            continue; // outside the table: undefined in the original
        }
        let ai = !ctx.human_controlled[e];
        let urgent = research.urgent[e] != 0;
        let race = ctx.race[e] as usize;
        let need = techs.labs_needed(t);
        let extra = |i: usize| if need[i] != 0 { (labs[e][i + 1] - need[i] as i32).max(0) } else { 0 };
        let mut v = (((extra(0) as f64 + 5.0 + extra(1) as f64 + extra(2) as f64) / 5.0) * ctx.speed_mult as f64) as f32;
        if urgent {
            v *= 2.0;
        }
        if ai {
            v *= dev;
        }
        if e == ctx.human_empire as usize && !ctx.multiplayer {
            v = (((ctx.story_bonus as f64 + 100.0) / 100.0) * v as f64) as f32;
        }
        let time_mult = rules.race_time.get(race).copied().unwrap_or(0.0);
        let limit = techs.base_research_time(t) as f32 * time_mult;
        let left = limit - techs.research_time(t, e) as f32;
        if left <= v {
            v = left;
        }
        if v < 1.0 {
            v = 1.0;
        }
        if (e == ctx.human_empire as usize && ctx.cheat) || labs_ok(techs, t, &labs[e]) {
            let base = techs.research_cost(t) as f32 * rules.race_money.get(race).copied().unwrap_or(0.0);
            let mut c = base * v / limit;
            if urgent {
                c *= 2.0;
            }
            if ai {
                c /= dev;
            }
            if money[e].available(3) >= c as f64 {
                let income = ai_income(rules, ctx, e);
                money[e].add(3, -c as f64, income);
                let p = (techs.research_time(t, e) as f32 + v) as i32;
                techs.set_research_time(t, e, p);
            }
            let month = (31 - ctx.day) as f64 * 50.0 * v as f64 / ctx.speed_mult as f64;
            let mut room = (limit - techs.research_time(t, e) as f32) as f64;
            if urgent {
                room *= 2.0;
            }
            money[e].reserve[3] = base as f64 * room.min(month) / limit as f64;
        } else {
            money[e].reserve[3] = 0.0;
        }
        if ctx.cheat {
            techs.set_research_time(t, e, (limit + 10.0) as i32);
        }
        if limit <= techs.research_time(t, e) as f32 || techs.status(t, e) == status::OWNED {
            if e == ctx.human_empire as usize {
                ctx.slow_down();
            }
            developed(techs, research, e, t, rules, ctx, &mut events);
            research.current[e] = 0;
            events.push(ResearchEvent::Completed { empire: e as u8, tech: t });
            ai_pick(techs, research, labs, e, ctx, &mut events);
        }
    }
    events
}

fn ai_income(rules: &ResearchRules, ctx: &ResearchContext, e: usize) -> Option<f64> {
    if ctx.human_controlled[e] {
        return None;
    }
    let m = rules.difficulty_money.get(ctx.difficulty[e]).copied().unwrap_or(1.0) as f64;
    Some(if ctx.multiplayer { m * 0.5 } else { m })
}

/// `TalalmanyKifejlesztve(e, t)` (spec §2.1).
pub fn developed(techs: &mut TechTable, research: &mut Research, e: usize, mut t: usize, rules: &ResearchRules,
    ctx: &ResearchContext, events: &mut Vec<ResearchEvent>) {
    loop {
        if techs.status(t, e) == status::OWNED {
            return;
        }
        let mult = rules.race_time.get(ctx.race[e] as usize).copied().unwrap_or(0.0);
        techs.set_research_time(t, e, (techs.base_research_time(t) as f32 * mult) as i32);
        techs.set_status(t, e, status::OWNED);
        if ctx.human_controlled[e] && techs.auto_design(t) {
            events.push(ResearchEvent::AutoDesign { empire: e as u8, tech: t });
        }
        techs.recompute(ctx.players, ctx.all_techs);
        if research.current[e] == t as i32 {
            research.current[e] = 0;
        }
        if t != LINKED.0 {
            return;
        }
        t = LINKED.1;
    }
}

/// `FRISSIT_MI_FEJLESZTES(e)` (spec §2.2): an idle AI starts the researchable tech with the lowest
/// non-zero priority for its race.
pub fn ai_pick(techs: &mut TechTable, research: &mut Research, labs: &[[i32; 4]; EMPIRES], e: usize,
    ctx: &ResearchContext, events: &mut Vec<ResearchEvent>) {
    if ctx.human_controlled[e] || research.current[e] != 0 {
        return;
    }
    let r = if ctx.race[e] < 9 { ctx.race[e] as usize } else { 1 };
    let cheat = e == ctx.human_empire as usize && ctx.cheat;
    let (mut pick, mut best) = (0, -1i32);
    for t in 1..TECHS {
        if techs.status(t, e) != status::RESEARCHABLE || !(cheat || labs_ok(techs, t, &labs[e])) {
            continue;
        }
        let b = techs.ai_priority(t, r) as i32;
        if (b != 0 && (b < best || best < 1)) || (b == 0 && best < 0) {
            pick = t;
            best = b;
        }
    }
    if pick > 0 {
        techs.set_status(pick, e, status::RESEARCHING);
        research.current[e] = pick as i32;
        events.push(ResearchEvent::Started { empire: e as u8, tech: pick });
    }
}

/// Ours (ADR 0037): the auto-research switches, bits of `Game::auto_research[e]`.
pub mod auto {
    pub const SHIPS: u8 = 1;
    pub const BUILDINGS: u8 = 2;
    pub const TANKS: u8 = 4;
    pub const ALL: u8 = SHIPS | BUILDINGS | TANKS;

    /// The switch that covers tech `t`, by its research-screen category `(t − 1) / 16 + 1`: ships, weapons, radars,
    /// shields, bombs → ships; civil, research, military buildings, power plants → buildings; tanks, turrets → tanks;
    /// special (12, the story's techs) → none.
    pub fn switch(t: usize) -> u8 {
        match t.wrapping_sub(1) / 16 + 1 {
            1..=5 => SHIPS,
            6..=9 => BUILDINGS,
            10 | 11 => TANKS,
            _ => 0,
        }
    }
}

/// Ours (ADR 0037, production-research.md §2.5): an idle human-controlled empire with an auto-research switch on
/// starts the researchable tech of a switched-on category that its labs allow, by the AI's priority for its race
/// (lowest non-zero first, priority 0 last), then the cheapest research cost, then the lowest number.
pub fn auto_pick(techs: &mut TechTable, research: &mut Research, labs: &[[i32; 4]; EMPIRES], e: usize,
    ctx: &ResearchContext, events: &mut Vec<ResearchEvent>) {
    let on = ctx.auto[e] & auto::ALL;
    if on == 0 || !ctx.human_controlled[e] || research.current[e] != 0 {
        return;
    }
    let r = if ctx.race[e] < 9 { ctx.race[e] as usize } else { 1 };
    let cheat = e == ctx.human_empire as usize && ctx.cheat;
    let pick = (1..TECHS)
        .filter(|&t| auto::switch(t) & on != 0 && techs.status(t, e) == status::RESEARCHABLE)
        .filter(|&t| cheat || labs_ok(techs, t, &labs[e]))
        .min_by_key(|&t| {
            let b = techs.ai_priority(t, r);
            (b == 0, b, techs.research_cost(t), t)
        });
    if let Some(t) = pick {
        techs.set_status(t, e, status::RESEARCHING);
        research.current[e] = t as i32;
        events.push(ResearchEvent::Started { empire: e as u8, tech: t });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const E: usize = 1;

    /// A table where techs `ts` are researchable for empire 1 with AI priority `prio` (race 1) and research cost
    /// `cost`, needing ship lab `lab`.
    fn table(ts: &[(usize, u8, i32, u8)]) -> TechTable {
        let mut t = TechTable::default();
        for &(n, prio, cost, lab) in ts {
            let o = n * crate::colony_side::TECH_STRIDE;
            t.bytes[o + 0xb7 + 1] = prio;
            t.bytes[o + 0x34..o + 0x38].copy_from_slice(&cost.to_le_bytes());
            t.bytes[o + 0x31] = lab;
            t.set_status(n, E, status::RESEARCHABLE);
        }
        t
    }

    fn ctx(auto: u8) -> ResearchContext {
        let mut c = ResearchContext { human_empire: E as u8, ..Default::default() };
        c.human_controlled[E] = true;
        c.race[E] = 1;
        c.auto[E] = auto;
        c
    }

    fn pick(t: &mut TechTable, c: &ResearchContext, ship_lab: i32) -> i32 {
        let mut r = Research::default();
        let mut labs = [[0; 4]; EMPIRES];
        labs[E][1] = ship_lab;
        let mut ev = Vec::new();
        auto_pick(t, &mut r, &labs, E, c, &mut ev);
        if r.current[E] != 0 {
            assert_eq!(t.status(r.current[E] as usize, E), status::RESEARCHING);
            assert_eq!(ev, [ResearchEvent::Started { empire: E as u8, tech: r.current[E] as usize }]);
        }
        r.current[E]
    }

    #[test]
    fn switches_follow_the_research_categories() {
        // Fighter, bombs' last field, civil buildings' first, power plants' last, wheeled tank, turrets' last, X-ray.
        let got: Vec<u8> = [1, 80, 81, 144, 145, 176, 177, 0].map(auto::switch).to_vec();
        use auto::*;
        assert_eq!(got, [SHIPS, SHIPS, BUILDINGS, BUILDINGS, TANKS, TANKS, 0, 0]);
    }

    #[test]
    fn pick_by_priority_then_cost_within_the_switched_categories() {
        // Ships 1 (prio 3), 5 (prio 2, cost 900), 9 (prio 2, cost 500), 10 (prio 0, cheapest); building 113 (prio 1);
        // tank 145 (prio 1); special 177 (prio 1, never picked).
        let ts = [(1, 3, 100, 0), (5, 2, 900, 0), (9, 2, 500, 0), (10, 0, 10, 0), (113, 1, 100, 0), (145, 1, 100, 0), (177, 1, 1, 0)];
        assert_eq!(pick(&mut table(&ts), &ctx(auto::SHIPS), 0), 9);
        assert_eq!(pick(&mut table(&ts), &ctx(auto::BUILDINGS), 0), 113);
        // Both of priority 1: the cost ties, so the lower number.
        assert_eq!(pick(&mut table(&ts), &ctx(auto::BUILDINGS | auto::TANKS), 0), 113);
        assert_eq!(pick(&mut table(&ts), &ctx(auto::TANKS), 0), 145);
        // Priority 0 only when nothing else is open.
        assert_eq!(pick(&mut table(&[(1, 0, 50, 0), (2, 0, 40, 0)]), &ctx(auto::SHIPS), 0), 2);
    }

    #[test]
    fn pick_needs_the_labs_an_idle_human_and_a_switch() {
        let ts = [(5, 1, 100, 4), (9, 2, 100, 1)];
        assert_eq!(pick(&mut table(&ts), &ctx(auto::SHIPS), 1), 9, "tech 5 needs ship lab 4");
        assert_eq!(pick(&mut table(&ts), &ctx(auto::SHIPS), 4), 5);
        let mut cheat = ctx(auto::SHIPS);
        cheat.cheat = true;
        assert_eq!(pick(&mut table(&ts), &cheat, 0), 5, "the research cheat ignores labs");
        assert_eq!(pick(&mut table(&ts), &ctx(0), 4), 0, "all switches off: the original");
        assert_eq!(pick(&mut table(&ts), &ctx(auto::TANKS), 4), 0, "no tank tech open");
        let mut ai = ctx(auto::ALL);
        ai.human_controlled[E] = false;
        assert_eq!(pick(&mut table(&ts), &ai, 4), 0, "the AI keeps its own pick");
        let (mut t, mut r) = (table(&ts), Research::default());
        r.current[E] = 9;
        auto_pick(&mut t, &mut r, &[[4; 4]; EMPIRES], E, &ctx(auto::ALL), &mut Vec::new());
        assert_eq!(r.current[E], 9, "a running project stays");
    }

    #[test]
    fn tick_starts_and_researches_on_the_same_tick() {
        let mut t = table(&[(9, 2, 1000, 0)]);
        t.bytes[9 * crate::colony_side::TECH_STRIDE + 0x38..][..4].copy_from_slice(&6000i32.to_le_bytes());
        let rules = ResearchRules { race_time: vec![1.0; 9], race_money: vec![1.0; 9], ..Default::default() };
        let mut money = [Treasury::default(); EMPIRES];
        money[E].pools[3] = 1e9;
        money[E].total = 1e9;
        money[E].share[3] = 1.0;
        let mut c = ResearchContext { speed_mult: 1, day: 1, ..ctx(0) };
        let mut r = Research::default();
        let labs = [[0; 4]; EMPIRES];
        assert!(tick(&mut t, &mut r, &labs, &mut money, &rules, &mut c).is_empty());
        assert_eq!((r.current[E], t.research_time(9, E)), (0, 0), "switches off: idle stays idle");
        c.auto[E] = auto::SHIPS;
        let ev = tick(&mut t, &mut r, &labs, &mut money, &rules, &mut c);
        assert_eq!(ev, [ResearchEvent::Started { empire: E as u8, tech: 9 }]);
        assert_eq!(r.current[E], 9);
        assert!(t.research_time(9, E) > 0);
    }
}
