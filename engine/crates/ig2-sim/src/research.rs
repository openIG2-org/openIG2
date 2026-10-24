//! Research: progress every tick (`FRISSIT_Research`, exe 0x51ddd0), a developed tech
//! (`TalalmanyKifejlesztve`, 0x524e50) and the AI's next pick (`FRISSIT_MI_FEJLESZTES`, 0x50b0b0).
//! Spec: `re/rules/production-research.md` §2.
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
    /// `Nehezseg`.
    pub difficulty: usize,
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
    /// The AI started a new project (status 4).
    Started { empire: u8, tech: usize },
}

fn labs_ok(techs: &TechTable, t: usize, labs: &[i32; 4]) -> bool {
    let need = techs.labs_needed(t);
    (0..3).all(|i| need[i] as i32 <= labs[i + 1])
}

/// Research of all empires for one tick, `labs` = `LaborSzint`. A human completion slows the game
/// down ([`ResearchContext::slow_down`]): `ctx.speed_mult` is the clock's value afterwards.
pub fn tick(techs: &mut TechTable, research: &mut Research, labs: &[[i32; 4]; EMPIRES], money: &mut [Treasury; EMPIRES],
    rules: &ResearchRules, ctx: &mut ResearchContext) -> Vec<ResearchEvent> {
    let mut events = Vec::new();
    let dev = rules.difficulty_speed.get(ctx.difficulty).copied().unwrap_or(1.0);
    for e in 1..EMPIRES {
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
    let m = rules.difficulty_money.get(ctx.difficulty).copied().unwrap_or(1.0) as f64;
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
