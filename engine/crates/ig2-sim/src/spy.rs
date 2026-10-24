//! Espionage: the daily spy pass (`FRISSIT_Spy`, exe 0x584b30): ranks, counter-intelligence, double agents,
//! the catch check halfway through a mission, mission results (reports, theft, sabotage, rebellions,
//! assassinations, kidnapping), the fate of a spy an AI catches (`DontottAKemSorsarol` 0x584350) and new
//! recruits (`Spy_New` 0x58f830, `Spy_Copy` 0x58d170). Spec: `re/rules/espionage.md`.
//!
//! Every draw is on seeded stream 4; the original makes no libc `rand()` call here. Missions reach the colony
//! world (`colony_side`: planets, buildings, money, techs, rebellions), the ships (`spacewar`) and research
//! through [`World`]. Messages, story and tutorial hooks and `FelfedezBirodalom` are [`SpyEvent`]s; story
//! missions ask [`World::story`].

use crate::colony::EconomyContext;
use crate::colony_side::{status, ColonyWorld, SideEvent, SideRules, EMPIRES};
use crate::diplomacy::{field, Relation};
use crate::research::{self, Research, ResearchContext, ResearchEvent, ResearchRules};
use crate::rng::{stream, Streams};
use crate::spacewar::{CombatEvent, CombatWorld, NO_DESIGN};

/// The spy stream (`rndlog(4, …)`).
pub const STREAM: usize = stream::SPY;
/// Empires with a roster (`Kemek[9]`).
pub const ROSTERS: usize = 9;
/// Slots per roster: 0 (a spare) and 1..=12.
pub const SLOTS: usize = 13;
/// `OsszHanyKem` never grows past this.
pub const MAX_SPIES: i32 = 12;
/// Spy record size.
pub const RECORD: usize = 0x4c;
/// Techs a mission looks at (1..=192).
const LAST_TECH: usize = 192;

/// `+0x10`.
pub mod state {
    pub const NONE: i16 = 0;
    pub const HOME: i16 = 1;
    pub const RETURNING: i16 = 2;
    pub const MISSION: i16 = 3;
    /// Caught by a human-controlled empire; waits for its decision (`MULTIGET_Spy`).
    pub const CAPTURED: i16 = 4;
}

/// Regular missions (`KemKuldetesek` 1..=13; 14..=26 are story missions).
pub mod mission {
    pub const TRAINING: usize = 1;
    pub const EMPIRE_REPORT: usize = 2;
    pub const PLANET_REPORT: usize = 3;
    pub const TECH_REPORT: usize = 4;
    pub const STEAL_MONEY: usize = 5;
    pub const SABOTAGE: usize = 6;
    pub const DESTROY_SHIP: usize = 7;
    pub const STEAL_TECH: usize = 8;
    pub const REBELLION: usize = 9;
    pub const ASSASSINATE: usize = 10;
    pub const KIDNAP: usize = 11;
    pub const MASS_UNREST: usize = 12;
    pub const COUNTER_INTELLIGENCE: usize = 13;
}

/// Catch-check outcomes (index into `KemKuldetesAtmenet`).
pub mod outcome {
    /// Identified: the spy is lost.
    pub const LOST: usize = 1;
    pub const CAUGHT: usize = 2;
    /// Spotted: sent home.
    pub const SPOTTED: usize = 3;
    /// Unnoticed (also every mission result).
    pub const DONE: usize = 4;
}

/// Failure codes of a mission result (report value, spec §5.1).
pub mod fail {
    pub const TARGET_DEAD: i32 = 997;
    pub const LOST_DUEL: i32 = 998;
    pub const NOTHING: i32 = 999;
}

/// Skill indices into [`Spy::skills`] (`+0x31..+0x34`).
pub mod skill {
    pub const INFILTRATION: usize = 0;
    pub const COUNTER: usize = 1;
    pub const COMBAT: usize = 2;
    pub const LOYALTY: usize = 3;
}

/// One spy record (`Kemek[e][k]`, 0x4c bytes); every byte round-trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spy {
    /// `+0x00`: NUL-terminated name.
    pub name: [u8; 16],
    /// `+0x10`, see [`state`].
    pub state: i16,
    /// `+0x12`: race (template index).
    pub race: u8,
    pub b13: u8,
    pub w14: i16,
    /// `+0x16`: offered to the human, not hired yet.
    pub applicant: i16,
    /// `+0x18`: double agent for this empire (0 = none).
    pub master: u8,
    pub b19: u8,
    /// `+0x1a`: mission, `+0x1c`: target empire, `+0x1e`/`+0x20`: parameters.
    pub mission: u16,
    pub target: u8,
    pub b1d: u8,
    pub param1: i16,
    pub param2: i16,
    pub w22: i16,
    /// `+0x24`: mission length, `+0x28`: time left, `+0x2c`: catch moment.
    pub length: i32,
    pub left: i32,
    pub catch_at: i32,
    pub b30: u8,
    /// `+0x31..+0x34`, see [`skill`].
    pub skills: [u8; 4],
    pub b35: [u8; 3],
    /// `+0x38`: skill points to distribute.
    pub points: i32,
    /// `+0x3c`: experience; `+0x40`: rank.
    pub xp: i32,
    pub rank: u8,
    /// `+0x41 + t`: known to empire `t` (index `t`, 1..=8 used).
    pub known_to: [u8; 11],
}

impl Default for Spy {
    fn default() -> Self {
        Self::from_bytes(&[0; RECORD])
    }
}

fn rd_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl Spy {
    pub fn from_bytes(b: &[u8]) -> Self {
        Self {
            name: b[..16].try_into().unwrap(),
            state: rd_i16(b, 0x10),
            race: b[0x12],
            b13: b[0x13],
            w14: rd_i16(b, 0x14),
            applicant: rd_i16(b, 0x16),
            master: b[0x18],
            b19: b[0x19],
            mission: rd_i16(b, 0x1a) as u16,
            target: b[0x1c],
            b1d: b[0x1d],
            param1: rd_i16(b, 0x1e),
            param2: rd_i16(b, 0x20),
            w22: rd_i16(b, 0x22),
            length: rd_i32(b, 0x24),
            left: rd_i32(b, 0x28),
            catch_at: rd_i32(b, 0x2c),
            b30: b[0x30],
            skills: b[0x31..0x35].try_into().unwrap(),
            b35: b[0x35..0x38].try_into().unwrap(),
            points: rd_i32(b, 0x38),
            xp: rd_i32(b, 0x3c),
            rank: b[0x40],
            known_to: b[0x41..0x4c].try_into().unwrap(),
        }
    }

    pub fn to_bytes(&self) -> [u8; RECORD] {
        let mut b = [0u8; RECORD];
        b[..16].copy_from_slice(&self.name);
        b[0x10..0x12].copy_from_slice(&self.state.to_le_bytes());
        b[0x12] = self.race;
        b[0x13] = self.b13;
        b[0x14..0x16].copy_from_slice(&self.w14.to_le_bytes());
        b[0x16..0x18].copy_from_slice(&self.applicant.to_le_bytes());
        b[0x18] = self.master;
        b[0x19] = self.b19;
        b[0x1a..0x1c].copy_from_slice(&self.mission.to_le_bytes());
        b[0x1c] = self.target;
        b[0x1d] = self.b1d;
        b[0x1e..0x20].copy_from_slice(&self.param1.to_le_bytes());
        b[0x20..0x22].copy_from_slice(&self.param2.to_le_bytes());
        b[0x22..0x24].copy_from_slice(&self.w22.to_le_bytes());
        b[0x24..0x28].copy_from_slice(&self.length.to_le_bytes());
        b[0x28..0x2c].copy_from_slice(&self.left.to_le_bytes());
        b[0x2c..0x30].copy_from_slice(&self.catch_at.to_le_bytes());
        b[0x30] = self.b30;
        b[0x31..0x35].copy_from_slice(&self.skills);
        b[0x35..0x38].copy_from_slice(&self.b35);
        b[0x38..0x3c].copy_from_slice(&self.points.to_le_bytes());
        b[0x3c..0x40].copy_from_slice(&self.xp.to_le_bytes());
        b[0x40] = self.rank;
        b[0x41..0x4c].copy_from_slice(&self.known_to);
        b
    }

    /// `strcmp(name, other) == 0` on the NUL-terminated name.
    fn named(&self, other: &[u8]) -> bool {
        let len = self.name.iter().position(|&c| c == 0).unwrap_or(16);
        &self.name[..len] == other
    }

    fn known_to(&self, t: usize) -> bool {
        self.known_to.get(t).is_some_and(|&b| b != 0)
    }
}

/// A mission's rules (`KemKuldetesek` record, spec §1.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MissionRule {
    /// 1: the parameter is a planet of the target.
    pub kind: i32,
    pub xp: i32,
    pub skill_floor: u8,
    pub skill_full: u8,
    pub chance: u8,
    pub needs_target: bool,
    pub story: bool,
    pub no_countdown: bool,
    /// `+0x10` / `+0x14` / `+0x18`: length in days, cost, the rank the AI needs (`re/rules/ai-empire.md` §6).
    pub days: i32,
    pub cost: i32,
    pub min_rank: u8,
}

/// Rule tables (from the user's exe, ADR 0004).
#[derive(Debug, Clone, Default)]
pub struct SpyRules {
    pub missions: Vec<MissionRule>,
    /// `KemKuldetesAtmenet[outcome]`: the spy's new state.
    pub outcome_state: Vec<i32>,
    /// `KemSzintLepesSzetoszthato[race]`.
    pub rank_points: Vec<i32>,
    /// `KemTartalekok[race]`.
    pub templates: Vec<Spy>,
    /// `KemRangXPHatar[0..=21]` (21: the i32 after the table).
    pub rank_xp: Vec<i32>,
    /// `TrainingKuldetesXP[level]`.
    pub training_xp: Vec<i32>,
    pub names: Vec<Vec<u8>>,
    /// `EpuletekLeiro[kind].+0`: the tech a building kind belongs to (sabotage's filter).
    pub building_tech: Vec<u8>,
    /// `KemFelvetelPenzSzorzoFajonkent[race]` (exe 0x9137FC): an applicant's price per rank, ÷ 3000.
    pub hire_cost_mult: Vec<f32>,
}

impl SpyRules {
    fn mission(&self, m: usize) -> MissionRule {
        self.missions.get(m).copied().unwrap_or_default()
    }
    fn state_after(&self, outcome: usize) -> i16 {
        self.outcome_state.get(outcome).copied().unwrap_or(0) as i16
    }
}

/// A spy-screen order (`MULTIGET_Spy`'s messages, spec §8); `spy` is a slot of the sender's roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// 0x47: dismiss.
    Dismiss { spy: u8 },
    /// 0x48: call a spy on a mission home.
    Recall { spy: u8 },
    /// 0x49: send a spy at home on mission `mission` against `target` for `days` days.
    Send { spy: u8, mission: u16, target: u8, param1: i16, param2: i16, days: i32 },
    /// 0x4a: the sender's verdict (`fate` 1..=6, §4.2) on `owner`'s caught spy.
    Verdict { fate: i32, owner: u8, spy: u8 },
    /// 0x4b: an applicant: 1 hire, 2 reject, 3 turn away (kept in the spare slot 0 nine times in ten).
    Applicant { choice: i32, spy: u8 },
    /// 0x4c: one skill point into byte `+0x30 + skill` (1..=4 = the skills).
    Skill { spy: u8, skill: u8 },
    /// Ours (re/screens/spy.md §7): the spy screen's verdict buttons (`Spy_Main` exe 0x58d410, `choice` 1..=4):
    /// the original rolls on stream 0 in the UI (spy.cpp 0x313/0x318) before it sends 0x4a, so the roll travels
    /// with the order. Applied by `Game::spy_order` (the roll, the report's follow-up kind, then [`Order::Verdict`]).
    Judge { choice: i32, owner: u8, spy: u8 },
    /// Ours: the spy screen closes the report on show (`KemHanyBeszolas −= 1`, the queue moves up); the queue is
    /// synced state (re/rules/multiplayer.md §5.2). Applied by `Game::spy_order`.
    Acknowledge,
    /// Ours: the mission planner's chance estimate `KemEselySzamolas1(EN, spy, mission, target, 1000, 0)` draws
    /// on stream 0 in the UI (exe 0x587b80); its draws travel as an order so every peer's stream 0 stays in step.
    Estimate { spy: u8, mission: u16, target: u8 },
}

/// `KemEselySzamolas1` ELF 0x6ca200's draw count per round: `rounds` × this (plus the counter-spy picks).
pub const ESTIMATE_ROUNDS: i32 = 1000;
/// The stream the spy screen's own rolls use (`rndlog(0, …)`).
pub const UI_STREAM: usize = 0;

/// State owned by espionage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spies {
    /// `Kemek[e][k]`.
    pub roster: [[Spy; SLOTS]; ROSTERS],
    /// `OsszHanyKem[e]`.
    pub count: [i32; ROSTERS],
    /// `HanyKemVan[e]`: working spies (state > 0, not applicants).
    pub working: [i32; ROSTERS],
    /// `KemKuldetesek[m].+0`: missions enabled (the story toggles them; the daily pass clears 11).
    pub enabled: Vec<u8>,
}

impl Default for Spies {
    fn default() -> Self {
        Self { roster: [[Spy::default(); SLOTS]; ROSTERS], count: [0; ROSTERS], working: [0; ROSTERS], enabled: Vec::new() }
    }
}

/// Global inputs.
#[derive(Debug, Clone, Default)]
pub struct SpyContext {
    /// `BirodalomEl`, `PlayerKi == 1`, `PlayerFaj`.
    pub alive: [bool; EMPIRES],
    pub human: [bool; EMPIRES],
    pub race: [u8; EMPIRES],
    /// `EN`.
    pub human_empire: usize,
    /// `ProgramPlayers`.
    pub program_players: i32,
    pub multiplayer: bool,
    /// `Nehezseg`.
    pub difficulty: i32,
    /// `DEBUGX`.
    pub debug: bool,
    /// Tutorial active or paused.
    pub tutorial: bool,
    /// `TutorialUjKemekLetiltva` (exe 0xF66F2B).
    pub no_recruits: bool,
    /// exe 0xB77250 (SP debug): 1 every mission succeeds, 2 every spy is caught, 3 a recruit every day.
    pub forced: i32,
    /// `Nap`.
    pub day: i32,
    /// The empires whose own machine sends the original's `Multi_Send(.., EN, ..)`: `EN` alone (classic, the
    /// default `None`), or every human (`game::ViewMode::Canonical`, ADR 0007 §5).
    pub senders: Option<[bool; EMPIRES]>,
    /// Canonical multiplayer: the other humans, each with its own report queue (`re/rules/multiplayer.md` §5.2).
    pub listed: [bool; EMPIRES],
}

impl SpyContext {
    fn forced(&self, v: i32) -> bool {
        !self.multiplayer && self.forced == v
    }
    fn alive(&self, e: usize) -> bool {
        self.alive.get(e).copied().unwrap_or(false)
    }
    fn human(&self, e: usize) -> bool {
        self.human.get(e).copied().unwrap_or(false)
    }
    /// Whether empire `e`'s machine sends a `Multi_Send(.., EN, ..)` ([`SpyContext::senders`]).
    pub fn sends(&self, e: usize) -> bool {
        match self.senders {
            Some(s) => s.get(e).copied().unwrap_or(false),
            None => e == self.human_empire,
        }
    }
    fn race(&self, e: usize) -> u8 {
        self.race.get(e).copied().unwrap_or(0)
    }
}

/// A story mission's question (`StoryEredmenyKemElkapasa` when `caught`, else `StoryEredmenyKemSikeres`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryCheck {
    pub caught: bool,
    pub empire: u8,
    pub spy: u8,
    pub mission: u16,
    pub target: u8,
    pub param1: i16,
    pub param2: i16,
}

/// `KemJelentesAdd(kind, a, b, c, d, x6..x14)` (exe 0x58d000, `info-messages.md` §3.3): a report to the human's
/// spy screen (its `KemBeszolasok` entry, `Story_KemEsemeny`, maybe a scrolling message).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Report {
    pub kind: u8,
    pub a: i32,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    /// `x6..x14`.
    pub x: [i32; 9],
}

/// The report queue's entry size.
pub const REPORT: usize = 0x30;
/// `KemBeszolasok` holds entries 1..=50.
pub const MAX_REPORTS: i32 = 50;

impl Report {
    fn new(kind: u8, a: i32, b: u8) -> Self {
        Self { kind, a, b, ..Default::default() }
    }
    /// The report of a mission's state change: `m, t, p1, p2` then the result's values.
    fn mission(kind: u8, a: i32, (b, c, d): (u8, u8, i16), s: &Spy, p1: i32, r: [i32; 5]) -> Self {
        let mut x = [0; 9];
        x[..4].copy_from_slice(&[s.mission as i32, s.target as i32, p1, s.param2 as i32]);
        x[4..].copy_from_slice(&r);
        Self { kind, a, b, c, d: d as u8, x }
    }
    /// Writes the entry's fields into `rec` (0x30 bytes; the padding keeps its bytes).
    pub fn write(&self, rec: &mut [u8]) {
        rec[0] = self.kind;
        rec[4..8].copy_from_slice(&self.a.to_le_bytes());
        rec[8..11].copy_from_slice(&[self.b, self.c, self.d]);
        for (i, v) in self.x.iter().enumerate() {
            rec[0xc + 4 * i..0x10 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
    }
    /// The scrolling message it posts (`ScrollUzenetId(id, "", 0, 0)`): its own, then the fate function's.
    pub fn messages(&self) -> Vec<i32> {
        let mut out = Vec::new();
        if self.kind <= 6 && self.b != 2 {
            out.push(if self.b == 4 && self.c == 0 { 100 } else { 100 + self.kind as i32 });
        }
        match self.kind {
            0xf | 0x12 => out.push(0x65),
            0x10 => out.push(0x71),
            0x11 => out.push(0x6a),
            _ => {}
        }
        out
    }
}

/// Effects other systems or the UI carry out.
#[derive(Debug, Clone, PartialEq)]
pub enum SpyEvent {
    /// A report to the human (`KemJelentesAdd`).
    Report(Report),
    /// A report to another human with its own queue (canonical multiplayer, `re/rules/multiplayer.md` §5.2).
    ReportTo { to: u8, report: Report },
    /// A mission hit the human (`target`; `info-messages.md` §3.3's victim messages): `a` = the planet (sabotage,
    /// rebellion), the hull tech (ship) or the family member (kidnap); `b` = the building type (sabotage).
    Struck { mission: u16, a: i32, b: i32 },
    RankUp { empire: u8, spy: u8, rank: u8 },
    /// A double agent was exposed by its own counter-intelligence and died.
    Exposed { empire: u8, spy: u8 },
    /// The catch check (spec §4): `outcome` 1..=4; `fate` = the AI captor's decision.
    Caught { empire: u8, spy: u8, target: u8, outcome: u8, fate: Option<u8> },
    /// A story mission's catch check said "caught".
    StoryCaught { empire: u8, spy: u8 },
    /// `Story_Special(0xb, t, e)` and `FelfedezBirodalom(e, true)`: human `by` caught a spy of an unknown empire.
    Discovered { empire: u8, by: u8 },
    /// A spy got home (message 0x82 to the human).
    Returned { empire: u8, spy: u8 },
    /// A mission ended; `report` = the result's values (`info-messages.md` §3.3's `r0..r4`; fail codes in `report[4]`).

    MissionDone { empire: u8, spy: u8, mission: u16, target: u8, ok: bool, report: [i32; 5] },
    /// `TutorialCheckEvent(n)`.
    Tutorial(u8),
    /// A building was sabotaged (`EpuletDelete`; ground-war units lose it as a target).
    BuildingDestroyed { planet: usize, slot: usize, kind: u8 },
    /// A ship was destroyed (`HajoDelete`).
    ShipDestroyed { ship: usize, hull_tech: i32 },
    /// A new spy (an applicant for a human-controlled empire).
    Recruited { empire: u8, spy: u8, applicant: bool },
    Colony(SideEvent),
    Research(ResearchEvent),
    Combat(CombatEvent),
}

/// The world a spy pass works on.
pub struct World<'a> {
    pub ctx: &'a SpyContext,
    /// Planets, buildings, money, techs and `BirodalomIsmeri` (`knows`).
    pub colony: &'a mut ColonyWorld,
    pub side_rules: &'a SideRules,
    pub econ: &'a EconomyContext,
    /// Ships, fleets and designs.
    pub combat: &'a mut CombatWorld,
    pub research: &'a mut Research,
    pub research_rules: &'a ResearchRules,
    pub research_ctx: &'a ResearchContext,
    /// `Viszony[a][b]` (the spy grievance is [`field::SPY_GRIEVANCE`]).
    pub relations: &'a mut [[Relation; EMPIRES]],
    /// `CsaladtagElrabolvaKi[0..9]` then `CsaladtagElrabolvaKialtal[0..9]`.
    pub kidnap: &'a mut [i32; 18],
    pub rng: &'a mut Streams,
    pub rules: &'a SpyRules,
    /// Story missions: the story script's verdict.
    pub story: &'a mut dyn FnMut(StoryCheck) -> bool,
    pub events: Vec<SpyEvent>,
}

impl World<'_> {
    fn r(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.range(STREAM, lo, hi)
    }
    fn offer(&mut self, weight: i32) -> bool {
        self.rng.choice_offer(STREAM, weight)
    }
    fn grievance(&mut self, t: usize, e: usize, x: i16) {
        if let Some(r) = self.relations.get_mut(t).and_then(|r| r.get_mut(e)) {
            r.v[field::SPY_GRIEVANCE] = r.v[field::SPY_GRIEVANCE].wrapping_add(x);
        }
    }
    /// `KemJelentesAdd`, when `to` is the human (or another human with its own queue).
    fn report(&mut self, to: usize, r: Report) {
        if to == self.ctx.human_empire {
            self.events.push(SpyEvent::Report(r));
        } else if self.ctx.listed.get(to).copied().unwrap_or(false) {
            self.events.push(SpyEvent::ReportTo { to: to as u8, report: r });
        }
    }
    fn struck(&mut self, t: usize, mission: usize, a: i32, b: i32) {
        if t == self.ctx.human_empire {
            self.events.push(SpyEvent::Struck { mission: mission as u16, a, b });
        }
    }
    fn planet_owner(&self, p: usize) -> u8 {
        self.colony.planets.get(p).map_or(0, |pl| pl.owner)
    }
    /// `getFellazadhat && getMiAltalElfoglalhato`, or MP.
    fn can_incite(&self, p: usize) -> bool {
        let pl = &self.colony.planets[p];
        (pl.owner < 9 && !pl.loyal && !pl.ai_proof) || self.ctx.multiplayer
    }
    fn rebel(&mut self, p: usize, to: usize) -> i32 {
        let mut ev = Vec::new();
        let r = self.colony.rebel(p, to as u8, true, self.side_rules, self.econ, self.rng, &mut ev);
        self.events.extend(ev.into_iter().map(SpyEvent::Colony));
        r
    }
    fn develop(&mut self, e: usize, t: usize) {
        let mut ev = Vec::new();
        research::developed(&mut self.colony.tech, self.research, e, t, self.research_rules, self.research_ctx, &mut ev);
        self.events.extend(ev.into_iter().map(SpyEvent::Research));
    }
    /// Ships 1..n of `t`'s fleets, alive, whose design `+0x80 == 0` (and hull tech `p1` unless 0).
    fn ship_target(&self, i: usize, t: usize, p1: i32) -> Option<i32> {
        let sh = self.combat.ships[i];
        if sh.design == NO_DESIGN || self.combat.fleets.get(sh.fleet.max(0) as usize).map_or(0xff, |f| f.owner) as usize != t {
            return None;
        }
        let d = self.combat.designs.get(t)?.get(sh.design.max(0) as usize)?;
        let hidden = rd_i32(&d.raw, 0x80) != 0;
        (!hidden && (p1 == 0 || p1 == d.hull_tech)).then_some(d.hull_tech)
    }
}

/// x87 `ftol` truncation (out of range → `0x80000000`).
fn ftol(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 { i32::MIN } else { x as i32 }
}

impl Spies {
    /// A fresh roster with the missions' enabled flags from the rules.
    pub fn new(rules: &SpyRules, enabled: &[u8]) -> Self {
        let mut s = Self { enabled: enabled.to_vec(), ..Default::default() };
        s.enabled.resize(rules.missions.len(), 0);
        s
    }

    fn spy(&self, e: usize, k: usize) -> &Spy {
        &self.roster[e][k]
    }
    fn spy_mut(&mut self, e: usize, k: usize) -> &mut Spy {
        &mut self.roster[e][k]
    }
    /// `OsszHanyKem[e]`, capped to the slots that exist.
    fn len(&self, e: usize) -> usize {
        self.count.get(e).copied().unwrap_or(0).clamp(0, SLOTS as i32 - 1) as usize
    }

    /// `FRISSIT_Spy`: the daily pass (spec §2).
    pub fn daily(&mut self, w: &mut World) {
        if let Some(x) = self.enabled.get_mut(mission::KIDNAP) {
            *x = 0;
        }
        // The pass's "slipped"/success local, reused by the story-caught report (uninitialised at first).
        let mut flag = 0u8;
        for e in 1..ROSTERS {
            if w.ctx.race(e) == 5 || !w.ctx.alive(e) {
                continue;
            }
            let mut k = 1;
            while k <= self.len(e) {
                if self.spy(e, k).state > 0 {
                    self.spy_day(w, e, k, &mut flag);
                }
                k += 1;
            }
        }
        self.recruit(w);
        self.recount(w);
    }

    /// `HanyKemVan[e]` (§7).
    pub(crate) fn recount(&mut self, w: &World) {
        for e in 1..ROSTERS {
            self.working[e] = 0;
            if w.ctx.race(e) != 5 && w.ctx.alive(e) {
                self.working[e] = (1..=self.len(e)).filter(|&k| self.spy(e, k).state > 0 && self.spy(e, k).applicant == 0).count() as i32;
            }
        }
    }

    /// `MULTIGET_Spy` (exe 0x5883c0, spec §8): empire `e`'s order from the spy screen, then the recount. `training` =
    /// `TrainingKuldetesAr`, `mult` = `KemKuldetesPenzSzorzoFajonkent`. An order naming a slot, empire, mission or
    /// table entry outside the exe's arrays is dropped (the exe would read past them).
    pub fn order(&mut self, w: &mut World, e: usize, o: Order, training: &[i32], mult: &[f32]) {
        if e >= ROSTERS {
            return;
        }
        let money = |w: &World, e: usize| w.colony.money.get(e).map_or(0.0, |m| m.available(0));
        let race = w.ctx.race(e) as usize;
        match o {
            Order::Dismiss { spy } if (spy as usize) < SLOTS => self.spy_mut(e, spy as usize).state = state::NONE,
            Order::Recall { spy } if (spy as usize) < SLOTS => {
                let s = self.spy_mut(e, spy as usize);
                if s.state == state::MISSION {
                    s.mission = 0;
                    s.state = state::HOME;
                }
            }
            Order::Send { spy, mission: m, target, param1, param2, days } if (spy as usize) < SLOTS && (m as usize) < w.rules.missions.len() => {
                let cost = if m as usize == mission::TRAINING {
                    match usize::try_from(param1).ok().and_then(|i| training.get(i)) {
                        Some(&c) => c,
                        None => return,
                    }
                } else {
                    w.rules.mission(m as usize).cost
                };
                let c = (cost as f32 * mult.get(race).copied().unwrap_or(0.0)) as i32;
                if c as f64 <= money(w, e) && self.spy(e, spy as usize).state == state::HOME {
                    w.colony.add_money(e, 0, -c as f64, w.side_rules, w.econ);
                    let len = days.wrapping_mul(50);
                    let catch_at = w.r(len / 3, len.wrapping_sub(30));
                    let s = self.spy_mut(e, spy as usize);
                    s.state = state::MISSION;
                    s.mission = m;
                    s.target = target;
                    s.param1 = param1;
                    s.param2 = param2;
                    s.length = len;
                    s.left = len;
                    s.catch_at = catch_at;
                }
            }
            Order::Verdict { fate, owner, spy } if (owner as usize) < ROSTERS && (spy as usize) < SLOTS => {
                self.fate(w, e, fate, owner as usize, spy as usize);
            }
            Order::Applicant { choice, spy } if (spy as usize) < SLOTS => {
                let k = spy as usize;
                if choice == 1 {
                    let s = *self.spy(e, k);
                    let c = if s.race == 14 { 0 } else { (w.rules.hire_cost_mult.get(race).copied().unwrap_or(0.0) * 3000.0 * s.rank as f32) as i32 };
                    if c as f64 <= money(w, e) || c == 0 {
                        w.colony.add_money(e, 0, -c as f64, w.side_rules, w.econ);
                        self.spy_mut(e, k).applicant = 0;
                    } else {
                        self.spy_mut(e, k).state = state::NONE;
                    }
                }
                if choice == 2 {
                    self.spy_mut(e, k).state = state::NONE;
                } else if choice == 3 {
                    if w.r(1, 10) > 1 {
                        self.roster[e][0] = *self.spy(e, k);
                    }
                    self.spy_mut(e, k).state = state::NONE;
                }
            }
            Order::Skill { spy, skill } if (spy as usize) < SLOTS && 0x30 + (skill as usize) < RECORD => {
                let s = self.spy_mut(e, spy as usize);
                if s.state > 0 && s.points > 0 {
                    let mut b = s.to_bytes();
                    b[0x30 + skill as usize] = b[0x30 + skill as usize].wrapping_add(1);
                    *s = Spy::from_bytes(&b);
                    s.points -= 1;
                }
            }
            _ => {}
        }
        self.recount(w);
    }

    fn spy_day(&mut self, w: &mut World, e: usize, k: usize, flag: &mut u8) {
        let en = w.ctx.human_empire;
        // §3.1 ranks.
        loop {
            let s = self.spy_mut(e, k);
            if s.rank >= 20 || w.rules.rank_xp.get(s.rank as usize + 1).copied().unwrap_or(i32::MAX) > s.xp {
                break;
            }
            s.rank += 1;
            s.points = s.points.wrapping_add(w.rules.rank_points.get(s.race as usize).copied().unwrap_or(0));
            if e == en {
                let rank = s.rank;
                w.events.push(SpyEvent::RankUp { empire: e as u8, spy: k as u8, rank });
                w.report(e, Report { x: [0, 0, rank as i32, 0, 0, 0, 0, 0, 0], ..Report::new(0xc, k as i32, 0) });
            }
        }
        // §3.2 counter-intelligence experience (f64), once a month.
        let s = *self.spy(e, k);
        if s.state == state::MISSION && s.mission as usize == mission::COUNTER_INTELLIGENCE && w.ctx.day == 1 {
            let g = s.xp as f64 * 0.03 + 1.0;
            let r = w.r(50, 100);
            let d = if r as f64 <= g { w.r(50, 100) as f64 } else { g };
            self.spy_mut(e, k).xp = ftol(s.xp as f64 + d);
        }
        // §3.3 double-agent exposure.
        let s = *self.spy(e, k);
        let counter = |x: &Spy| x.state == state::MISSION && x.mission as usize == mission::COUNTER_INTELLIGENCE;
        if (s.state == state::HOME || counter(&s)) && s.master != 0 {
            let mut j = 1;
            while j <= self.len(e) {
                let other = *self.spy(e, j);
                if j != k && counter(&other) {
                    let d = (s.skills[skill::INFILTRATION] as i32 + 10 - other.skills[skill::COUNTER] as i32).clamp(0, 20);
                    if w.r(0, d * 5000) < 50 {
                        self.spy_mut(e, k).state = state::NONE;
                        if e == en {
                            w.events.push(SpyEvent::Exposed { empire: e as u8, spy: k as u8 });
                            w.report(e, Report { x: [0, s.master as i32, 0, 0, 0, 0, 0, 0, 0], ..Report::new(0x13, k as i32, 1) });
                        }
                    }
                }
                j += 1;
            }
        }
        // §4, §3.4.
        let st = self.spy(e, k).state;
        if st != state::RETURNING && st != state::MISSION {
            return;
        }
        let s = *self.spy(e, k);
        if st == state::MISSION && s.left.wrapping_sub(50) < s.catch_at && s.catch_at < s.left {
            self.catch_check(w, e, k, flag);
        }
        let m = self.spy(e, k).mission as usize;
        if !w.rules.mission(m).no_countdown {
            let s = self.spy_mut(e, k);
            s.left = s.left.wrapping_sub(50);
        }
        if self.spy(e, k).left < 1 {
            if self.spy(e, k).state == state::RETURNING {
                self.spy_mut(e, k).state = state::HOME;
                if e == en {
                    w.events.push(SpyEvent::Returned { empire: e as u8, spy: k as u8 });
                }
            }
            if self.spy(e, k).state == state::MISSION {
                self.result(w, e, k, flag);
            }
        }
    }

    /// The catch check (spec §4).
    fn catch_check(&mut self, w: &mut World, e: usize, k: usize, flag: &mut u8) {
        let s = *self.spy(e, k);
        let (m, t) = (s.mission as usize, s.target as usize);
        let rule = w.rules.mission(m);
        if rule.story {
            let check = StoryCheck { caught: true, empire: e as u8, spy: k as u8, mission: s.mission, target: s.target, param1: s.param1, param2: s.param2 };
            if (w.story)(check) {
                let st = w.rules.state_after(outcome::LOST);
                self.spy_mut(e, k).state = st;
                w.events.push(SpyEvent::StoryCaught { empire: e as u8, spy: k as u8 });
                w.report(e, Report::mission(1, k as i32, (outcome::LOST as u8, *flag, st), &s, s.param1 as i32, [0; 5]));
            }
            return;
        }
        if !rule.needs_target {
            return;
        }
        let tr = if t < ROSTERS { t } else { 0 };
        let tn = if t < ROSTERS { self.len(t) } else { 0 };
        let mut guard = [false; SLOTS];
        let mut n = 0;
        for (j, g) in guard.iter_mut().enumerate().take(tn + 1).skip(1) {
            let x = self.roster[tr][j];
            if x.state == state::MISSION && x.mission as usize == mission::COUNTER_INTELLIGENCE {
                n += 1;
                *g = true;
            }
        }
        let known = (1..=8).filter(|&x| w.ctx.alive(x) && s.known_to(x)).count();
        let (mut defender, mut sum) = (0, 0);
        for _ in 0..n {
            w.rng.choice_begin();
            let mut pick = 0;
            for (j, &g) in guard.iter().enumerate().take(tn + 1).skip(1) {
                if g && w.offer(1) {
                    pick = j;
                }
            }
            if pick > 0 {
                guard[pick] = false;
                sum += self.spy(tr, pick).skills[skill::COUNTER] as i32 * 10;
                defender = pick;
            }
        }
        let mut att = w.r(0, s.skills[skill::INFILTRATION] as i32 * 140);
        let def = w.r(0, sum / (n + 1) * 10);
        if s.known_to(t) {
            att /= 3;
        }
        if w.ctx.race(e) == 3 {
            att = (att as f64 * 1.5) as i32;
        }
        if w.ctx.difficulty == 1 && !w.ctx.multiplayer && w.ctx.human(e) {
            att = (att as f64 * 1.5) as i32;
        }
        if def <= att || w.ctx.tutorial {
            defender = 0;
        }
        let master = s.master as usize;
        let slipped = defender == 0 && master != t && !w.ctx.forced(2);
        *flag = slipped as u8;
        if !slipped {
            let c0 = w.r(0, self.spy(tr, defender).skills[skill::COMBAT] as i32 * 10);
            let c4 = w.r(0, s.skills[skill::COMBAT] as i32 * 10);
            let c = c4 * 100 / (c0 + 1 + c4);
            let mut fate = None;
            let out = if master == t {
                outcome::DONE
            } else if c < 45 || w.ctx.forced(2) {
                if w.colony.knows[t][e] == 0 {
                    if t == w.ctx.human_empire || w.ctx.listed[t] {
                        w.events.push(SpyEvent::Discovered { empire: e as u8, by: t as u8 });
                    }
                    w.colony.knows[t][e] = 1;
                }
                if w.ctx.human(t) {
                    w.report(t, Report { x: [0, e as i32, k as i32, 0, 0, 0, 0, 0, 0], ..Report::new(3, defender as i32, 0) });
                } else {
                    let mut f = w.r(2, 4);
                    if f == 2 || f == 3 {
                        // The captor's own slot k (original).
                        let own = *self.spy(tr, k);
                        let r = w.r(1, own.skills[skill::LOYALTY] as i32 + 3);
                        if r > 3 || own.master != 0 {
                            f = if w.r(1, 2) == 1 { 5 } else { 6 };
                        }
                    }
                    fate = Some(f);
                }
                w.grievance(t, e, 2000);
                outcome::CAUGHT
            } else if c < 52 {
                if known < 3 {
                    let x = w.r(50, 100) * 50;
                    let sm = self.spy_mut(e, k);
                    sm.length = x;
                    sm.left = x;
                    w.grievance(t, e, 500);
                    outcome::SPOTTED
                } else {
                    w.report(t, Report { x: [0, e as i32, k as i32, 0, 0, 0, 0, 0, 0], ..Report::new(4, defender as i32, 0) });
                    w.grievance(t, e, 2000);
                    outcome::LOST
                }
            } else {
                w.grievance(t, e, 500);
                outcome::DONE
            };
            let new_state = w.rules.state_after(out);
            let sm = self.spy_mut(e, k);
            sm.state = new_state;
            if out == outcome::CAUGHT || out == outcome::SPOTTED {
                if let Some(b) = sm.known_to.get_mut(t) {
                    *b = 1;
                }
            }
            sm.mission = 0;
            w.events.push(SpyEvent::Caught { empire: e as u8, spy: k as u8, target: t as u8, outcome: out as u8, fate: fate.map(|f| f as u8) });
            w.report(e, Report::mission(1, k as i32, (out as u8, 0, new_state), &s, s.param1 as i32, [0; 5]));
            if master == t {
                w.report(t, Report::mission(7, 0, (out as u8, 0, new_state), &s, s.param1 as i32, [0; 5]));
            }
            if let Some(f) = fate {
                self.fate(w, t, f, e, k);
            }
        }
        self.spy_mut(e, k).catch_at = -1000;
    }

    /// `DontottAKemSorsarol(t, fate, e, k)` (spec §4.2) and its report to `e` (`info-messages.md` §3.3).
    fn fate(&mut self, w: &mut World, t: usize, f: i32, e: usize, k: usize) {
        let kind = match f {
            1 | 5 => {
                self.spy_mut(e, k).state = state::HOME;
                if f == 1 { 0xf } else { 0x12 }
            }
            2 => {
                if self.copy(t, e, k) < 0 {
                    self.spy_mut(e, k).state = state::NONE;
                    0x11
                } else {
                    0x10
                }
            }
            3 => {
                let s = self.spy_mut(e, k);
                s.state = state::HOME;
                s.master = t as u8;
                0x12
            }
            4 | 6 => {
                self.spy_mut(e, k).state = state::NONE;
                0x11
            }
            _ => return,
        };
        w.report(e, Report { x: [0, t as i32, 0, 0, 0, 0, 0, 0, 0], ..Report::new(kind, k as i32, 0) });
    }

    /// The mission result (spec §5).
    fn result(&mut self, w: &mut World, e: usize, k: usize, flag: &mut u8) {
        let s = *self.spy(e, k);
        let (m, t) = (s.mission as usize, s.target as usize);
        let (p1, p2) = (s.param1 as i32, s.param2 as i32);
        let rule = w.rules.mission(m);
        let mut report = [0i32; 5];
        // `local_1c`: the mission's first parameter as the effect left it.
        let mut p1_after = p1;
        let ok = if rule.story {
            let check = StoryCheck { caught: false, empire: e as u8, spy: k as u8, mission: s.mission, target: s.target, param1: s.param1, param2: s.param2 };
            (w.story)(check)
        } else {
            let mut ok = w.r(1, 100) <= chance(&rule, s.skills[skill::INFILTRATION], m, w.ctx, e);
            if !w.ctx.multiplayer && w.ctx.debug && e == w.ctx.human_empire {
                ok = true;
            }
            if w.ctx.tutorial {
                ok = true;
            }
            if rule.needs_target && !w.ctx.alive(t) {
                ok = false;
            }
            if rule.kind == 1 && p1 > 0 && w.planet_owner(p1 as usize) as usize != t {
                ok = false;
            }
            if w.ctx.forced(1) {
                ok = true;
            }
            if w.ctx.race(t) == 5 {
                ok = false;
            }
            if rule.needs_target && !w.ctx.alive(t) {
                report[4] = fail::TARGET_DEAD;
                false
            } else if ok {
                self.effect(w, e, k, m, t, (&mut p1_after, p2), &mut report)
            } else {
                false
            }
        };
        let mut x = rule.xp;
        if s.mission as usize == mission::TRAINING {
            x = w.rules.training_xp.get(p1.max(0) as usize).copied().unwrap_or(0);
        }
        let gain = if ok { w.r(x / 2, x) } else { w.r(200, 400) };
        let new_state = w.rules.state_after(outcome::DONE);
        let sm = self.spy_mut(e, k);
        sm.xp = sm.xp.wrapping_add(gain);
        sm.state = new_state;
        if !rule.story {
            sm.mission = 0;
        }
        w.grievance(t, e, 500);
        w.events.push(SpyEvent::MissionDone { empire: e as u8, spy: k as u8, mission: s.mission, target: s.target, ok, report });
        *flag = ok as u8;
        w.report(e, Report::mission(1, k as i32, (outcome::DONE as u8, ok as u8, new_state), &s, p1_after, report));
        if !rule.story {
            w.report(s.master as usize, Report::mission(7, 0, (outcome::DONE as u8, ok as u8, new_state), &s, p1_after, report));
        }
    }

    /// A mission's effect (spec §5.2–§5.12); false = it failed after all (code in `report[4]`).
    #[allow(clippy::too_many_arguments)]
    fn effect(&mut self, w: &mut World, e: usize, k: usize, m: usize, t: usize, (p1_after, p2): (&mut i32, i32), report: &mut [i32; 5]) -> bool {
        let p1 = *p1_after;
        let np = w.colony.planets.len();
        let nothing = |report: &mut [i32; 5]| {
            report[4] = fail::NOTHING;
            false
        };
        match m {
            mission::EMPIRE_REPORT => {
                for p in 1..np {
                    let pl = &w.colony.planets[p];
                    if pl.owner as usize == t {
                        report[0] += 1;
                        report[1] = report[1].wrapping_add(pl.population);
                    }
                }
                for i in 1..w.combat.ships.len() {
                    let sh = w.combat.ships[i];
                    if sh.design != NO_DESIGN && w.combat.fleets.get(sh.fleet.max(0) as usize).is_some_and(|f| f.owner as usize == t) {
                        report[2] += 1;
                    }
                }
                report[3] = (1..=LAST_TECH).filter(|&x| w.colony.tech.status(x, t) == status::OWNED).count() as i32;
                true
            }
            mission::PLANET_REPORT => {
                let mut found = p1.max(0) as usize;
                if p1 == 0 {
                    let (mut a, mut b, mut i) = (0, 0, 0);
                    for p in 1..np {
                        if w.colony.planets[p].owner as usize == t && w.colony.planets[p].visibility[e] == 0 {
                            if w.r(0, i) == 0 {
                                a = p;
                            }
                            if w.r(0, i) == 0 {
                                b = p;
                            }
                            i += 1;
                        }
                    }
                    if a == b || w.r(0, 2) != 0 {
                        b = 0;
                    }
                    for p in [a, b] {
                        if p > 0 {
                            w.colony.planets[p].visibility[e] = 1;
                        }
                    }
                    report[0] = a as i32;
                    report[1] = b as i32;
                    if a == 0 && b == 0 {
                        found = (1..np).rfind(|&p| w.colony.planets[p].owner as usize == t && w.colony.planets[p].visibility[e] > 0).unwrap_or(0);
                        if found == 0 {
                            return nothing(report);
                        }
                        *p1_after = found as i32;
                    }
                }
                if found != 0 {
                    report[0] = found as i32;
                }
                true
            }
            mission::TECH_REPORT => {
                let mut pick = [0usize; 3];
                let mut i = 0;
                for x in 1..=LAST_TECH {
                    if w.colony.tech.status(x, t) != status::OWNED {
                        continue;
                    }
                    let unseen = !w.colony.tech.seen(x, e, t);
                    for slot in &mut pick {
                        if (*slot == 0 || w.r(0, i) == 0) && unseen {
                            *slot = x;
                            break;
                        }
                    }
                    i += 1;
                }
                let [a, mut b, mut c] = pick;
                if b == c || a == c {
                    c = 0;
                }
                if a == b {
                    b = 0;
                }
                for x in [a, b, c] {
                    if x > 0 {
                        w.colony.tech.mark_seen(x, e, t);
                    }
                }
                report[..3].copy_from_slice(&[a as i32, b as i32, c as i32]);
                if a == 0 && b == 0 && c == 0 {
                    return nothing(report);
                }
                true
            }
            mission::STEAL_MONEY => {
                let r = w.r(0, ftol(w.colony.money[t].total));
                let x = r / 100 * 100;
                if x < 1 && w.ctx.human(t) {
                    report[0] = x;
                    return false;
                }
                w.colony.add_money(t, 0, (-x) as f64, w.side_rules, w.econ);
                let mut gain = x;
                if !w.ctx.human(t) {
                    gain = gain.wrapping_add(w.r(2000, 10000));
                }
                w.colony.add_money(e, 0, gain as f64, w.side_rules, w.econ);
                report[0] = gain;
                w.struck(t, m, 0, 0);
                true
            }
            mission::SABOTAGE => {
                let mut hit = (0, 0);
                for (p, tech) in [(p1, p2), (p1, 0), (0, p2), (0, 0)] {
                    hit = search_building(w, t, p, tech);
                    if hit.0 > 0 {
                        break;
                    }
                }
                let (p, slot) = hit;
                if p < 1 {
                    return nothing(report);
                }
                let kind = w.colony.planets[p].buildings[slot - 1].kind;
                w.colony.planets[p].buildings[slot - 1].kind = 0;
                if let Some(n) = w.colony.occupied.get_mut(p) {
                    *n -= 1;
                }
                report[..3].copy_from_slice(&[p as i32, slot as i32, kind as i32]);
                w.events.push(SpyEvent::BuildingDestroyed { planet: p, slot, kind });
                w.struck(t, m, p as i32, kind as i32);
                true
            }
            mission::DESTROY_SHIP => {
                w.rng.choice_begin();
                let (mut pick, mut hull) = (0, 0);
                // The second pass (no hull filter) runs only when the first found nothing; it reuses the
                // weighted-choice state (no candidates were offered, so it is empty).
                for pass in [p1, 0] {
                    for i in 1..w.combat.ships.len() {
                        if let Some(h) = w.ship_target(i, t, pass) {
                            if w.offer(1) {
                                pick = i;
                                hull = h;
                            }
                        }
                    }
                    if pick != 0 {
                        break;
                    }
                }
                if pick < 1 {
                    return nothing(report);
                }
                w.combat.delete_ship(pick);
                w.events.extend(w.combat.events.drain(..).map(SpyEvent::Combat));
                report[0] = pick as i32;
                report[1] = hull;
                w.events.push(SpyEvent::ShipDestroyed { ship: pick, hull_tech: hull });
                w.struck(t, m, hull, 0);
                true
            }
            mission::STEAL_TECH => {
                let mut x = p1;
                if x == 0 {
                    w.rng.choice_begin();
                    for y in 1..=LAST_TECH {
                        if w.colony.tech.status(y, e) < status::OWNED && w.colony.tech.status(y, t) == status::OWNED && w.offer(1) {
                            x = y as i32;
                        }
                    }
                }
                *p1_after = x;
                if x < 1 || x as usize > LAST_TECH || w.colony.tech.status(x as usize, t) != status::OWNED {
                    return nothing(report);
                }
                match x {
                    0x12 => w.events.push(SpyEvent::Tutorial(9)),
                    6 => w.events.push(SpyEvent::Tutorial(10)),
                    _ => {}
                }
                w.develop(e, x as usize);
                let master = self.spy(e, k).master as usize;
                if master != 0 {
                    w.develop(master, x as usize);
                }
                true
            }
            mission::REBELLION => {
                w.rng.choice_begin();
                let mut pick = 0;
                for p in 1..np {
                    if (p1 == 0 || p1 as usize == p) && w.planet_owner(p) as usize == t && w.can_incite(p) && w.offer(1) {
                        pick = p;
                    }
                }
                if pick < 1 {
                    return nothing(report);
                }
                report[0] = pick as i32;
                report[1] = w.rebel(pick, e);
                w.struck(t, m, pick as i32, 0);
                true
            }
            mission::ASSASSINATE => {
                w.rng.choice_begin();
                let mut victim = 0;
                let tr = if t < ROSTERS { t } else { 0 };
                let tn = if t < ROSTERS { self.len(t) } else { 0 };
                for j in 1..=tn {
                    let x = self.spy(tr, j);
                    if (x.state == state::HOME || x.state == state::RETURNING) && x.applicant == 0 && w.offer(1) {
                        victim = j;
                    }
                }
                if victim < 1 {
                    return nothing(report);
                }
                let a = w.r(0, self.spy(tr, victim).skills[skill::COMBAT] as i32 * 4);
                let b = w.r(0, self.spy(e, k).skills[skill::COMBAT] as i32 * 4);
                if a >= b {
                    report[4] = fail::LOST_DUEL;
                    return false;
                }
                self.spy_mut(tr, victim).state = state::NONE;
                report[0] = self.spy(tr, victim).rank as i32;
                w.report(t, Report { x: [0, e as i32, k as i32, 0, 0, 0, 0, 0, 0], ..Report::new(6, victim as i32, 0) });
                w.struck(t, m, 0, 0);
                true
            }
            mission::KIDNAP => {
                if t >= 9 || w.kidnap[t] != 0 || (!w.ctx.multiplayer && t == 5) {
                    return nothing(report);
                }
                let x = w.r(1, 5);
                w.kidnap[t] = x;
                w.kidnap[9 + t] = e as i32;
                report[0] = x;
                w.struck(t, m, x, 0);
                true
            }
            mission::MASS_UNREST => {
                let mut i = 1;
                while i <= w.r(2, 4) {
                    w.rng.choice_begin();
                    let mut pick = 0;
                    for p in 1..np {
                        if w.planet_owner(p) as usize == t && w.can_incite(p) {
                            let m = w.colony.planets[p].morale as i64 + 100;
                            let weight = if m == 0 { 0 } else { (100_000 / m) as i32 };
                            if w.offer(weight) {
                                pick = p;
                            }
                        }
                    }
                    if pick > 0 {
                        w.rebel(pick, e);
                    }
                    i += 1;
                }
                w.struck(t, m, 0, 0);
                true
            }
            // Training, counter-intelligence and unknown missions: no effect.
            _ => true,
        }
    }

    /// Recruitment (spec §6).
    fn recruit(&mut self, w: &mut World) {
        let pp = w.ctx.program_players.max(1);
        let r = w.r(0, 12500 / pp);
        if !((r < 50 || w.ctx.forced(3)) && !w.ctx.no_recruits) {
            return;
        }
        let e = w.r(1, pp) as usize;
        if e >= ROSTERS {
            return;
        }
        let applicants = (1..=self.len(e)).filter(|&k| self.spy(e, k).state > 0 && self.spy(e, k).applicant != 0).count();
        if !w.ctx.alive(e) || applicants != 0 || w.ctx.race(e) == 5 {
            return;
        }
        let k = if self.spy(e, 0).state == 0 { self.new_spy(w, e) } else { self.copy(e, e, 0) };
        if k > 0 {
            let applicant = w.ctx.human(e);
            if applicant {
                self.spy_mut(e, k as usize).applicant = 1;
                w.report(e, Report { x: [0, e as i32, 0, 0, 0, 0, 0, 0, 0], ..Report::new(2, k, 0) });
            }
            w.events.push(SpyEvent::Recruited { empire: e as u8, spy: k as u8, applicant });
        }
    }

    /// The slot a new record goes to: the first unused one, else a new one; −1 when the roster is full.
    fn free_slot(&mut self, e: usize) -> i32 {
        let n = self.count[e];
        let mut k = 1;
        while k <= n && (k as usize) < SLOTS && self.spy(e, k as usize).state > 0 {
            k += 1;
        }
        if k > n {
            if n > MAX_SPIES - 1 {
                return -1;
            }
            self.count[e] = n + 1;
            k = n + 1;
        }
        k
    }

    /// `Spy_Copy(e, from, k)` (spec §6.2): returns the new slot or −1.
    pub fn copy(&mut self, e: usize, from: usize, k: usize) -> i32 {
        let slot = self.free_slot(e);
        if slot < 0 {
            return -1;
        }
        let rec = *self.spy(from, k);
        self.roster[e][slot as usize] = rec;
        self.spy_mut(from, k).state = state::NONE;
        self.spy_mut(e, slot as usize).state = state::HOME;
        slot
    }

    /// `Spy_New(e)` (spec §6.1): returns the new slot or −1.
    pub fn new_spy(&mut self, w: &mut World, e: usize) -> i32 {
        let slot = self.free_slot(e);
        if slot < 0 {
            return -1;
        }
        let k = slot as usize;
        let mut race = w.r(1, 14);
        while matches!(race, 5 | 9 | 10 | 11 | 12) {
            race = w.r(1, 14);
        }
        self.roster[e][k] = w.rules.templates.get(race as usize).copied().unwrap_or_default();
        let last = w.rules.names.len() as i32 - 1;
        let name = loop {
            let i = w.r(0, last);
            let name = w.rules.names.get(i as usize).cloned().unwrap_or_default();
            if !(1..=self.len(e)).any(|j| self.spy(e, j).state > 0 && self.spy(e, j).named(&name)) {
                break name;
            }
        };
        let len = self.len(e);
        let hi = (1..=len)
            .filter(|&j| self.spy(e, j).state > 0 && self.spy(e, j).applicant == 0)
            .map(|j| self.spy(e, j).rank as i32)
            .max()
            .unwrap_or(0)
            .max(2);
        let s = self.spy_mut(e, k);
        s.name[..name.len()].copy_from_slice(&name);
        s.name[name.len()] = 0;
        s.state = state::HOME;
        s.race = race as u8;
        let mut rank = w.r(1, hi + 1);
        if w.r(1, 20) == 1 {
            rank += 2;
        }
        if self.count[e] < 2 && !w.ctx.multiplayer && w.ctx.difficulty == 1 && w.ctx.human(e) {
            rank += 2;
        }
        let rank = rank.min(20);
        let xp_at = |r: i32| w.rules.rank_xp.get(r as usize).copied().unwrap_or(0);
        let (lo, hi) = (xp_at(rank), xp_at(rank + 1));
        let xp = w.r(lo, hi);
        let own_race = w.ctx.race(e) as i32 == race;
        let steps = (rank - 1) * w.rules.rank_points.get(race as usize).copied().unwrap_or(0);
        let s = self.spy_mut(e, k);
        s.rank = rank as u8;
        s.xp = xp;
        if own_race {
            s.skills[skill::LOYALTY] = s.skills[skill::LOYALTY].wrapping_add(2);
        }
        for _ in 0..steps.max(0) {
            let i = w.r(1, 4) as usize;
            let s = self.spy_mut(e, k);
            s.skills[i - 1] = s.skills[i - 1].wrapping_add(1);
        }
        slot
    }
}

impl Spies {
    /// `KemEselySzamolas1(e, k, m, t, rounds, stream)` (ELF 0x6ca200, exe 0x587b80; re/screens/spy.md §6.2): how
    /// often, in percent, spy `k` of `e` gets past `t`'s counter-spies (state 3 on mission 13), playing the duel
    /// `rounds` times on `stream`. Missions without a target: 100, no draws.
    #[allow(clippy::too_many_arguments)]
    pub fn escape_chance(&self, ctx: &SpyContext, rules: &SpyRules, rng: &mut Streams, e: usize, k: usize, m: usize, t: usize, rounds: i32, stream: usize) -> i32 {
        let Some(rule) = rules.missions.get(m) else { return 0 };
        if !rule.needs_target || e >= ROSTERS || k >= SLOTS || t >= ROSTERS {
            return 100;
        }
        let spy = self.roster[e][k];
        let n = self.count[t].clamp(0, SLOTS as i32 - 1) as usize;
        let mut escaped = 0;
        let mut round = 1;
        while round <= rounds {
            let mut on_guard = [false; SLOTS];
            let mut left = 0;
            for (g, s) in on_guard.iter_mut().zip(&self.roster[t]).take(n + 1).skip(1) {
                if s.state == state::MISSION && s.mission as usize == mission::COUNTER_INTELLIGENCE {
                    left += 1;
                    *g = true;
                }
            }
            let (mut catcher, mut guard) = (0, 0);
            let mut r = 1;
            while r <= left {
                rng.choice_begin();
                let mut pick = 0;
                for (j, &g) in on_guard.iter().enumerate().take(n + 1).skip(1) {
                    if g && rng.choice_offer(stream, 1) {
                        pick = j;
                    }
                }
                if pick > 0 {
                    left -= 1;
                    catcher = pick;
                    on_guard[pick] = false;
                    guard += self.roster[t][pick].skills[skill::COUNTER] as i32 * 10 / r;
                }
                r += 1;
            }
            let mut a = rng.range(stream, 0, spy.skills[skill::INFILTRATION] as i32 * 140);
            let d = rng.range(stream, 0, guard * 10);
            if spy.known_to.get(t).copied().unwrap_or(0) != 0 {
                a /= 3;
            }
            if ctx.race(e) == 3 {
                a = (a as f64 * 1.5) as i32;
            }
            if ctx.difficulty == 1 && !ctx.multiplayer && ctx.human(e) {
                a = (a as f64 * 1.5) as i32;
            }
            if d <= a {
                catcher = 0;
            }
            if catcher == 0 {
                escaped += 1;
            }
            round += 1;
        }
        let mut c = if rounds > 0 { escaped * 100 / rounds } else { 0 };
        if m > 0 && rule.no_countdown {
            c = 0;
        }
        c
    }

    /// `KemEselySzamolas2(e, k, m, t)` (ELF 0x6ca620, exe 0x587de0; `re/rules/ai-empire.md` §6.5): the success
    /// chance in percent the planner shows and the AI weighs. `kidnapped` = `CsaladtagElrabolvaKi[t]`.
    #[allow(clippy::too_many_arguments)]
    pub fn success_chance(&self, ctx: &SpyContext, rules: &SpyRules, e: usize, k: usize, m: usize, t: usize, kidnapped: i32) -> i32 {
        let (Some(rule), Some(spy)) = (rules.missions.get(m), self.roster.get(e).and_then(|r| r.get(k))) else { return 0 };
        mission_chance(rule, spy.skills[skill::INFILTRATION], m, ctx, e, t, kidnapped)
    }

    /// The spy screen's verdict roll (`Spy_Main` exe 0x58d410, spy.cpp 0x313/0x318): `choice` 1 release, 2 turn,
    /// 3 make a double agent, 4 execute on `owner`'s caught spy `k`. Turning (2, 3) fails unless `rndlog(0, 1,
    /// loyalty + 3) < 6` and the spy is nobody's double agent; then a coin `rndlog(0, 1, 2)` makes it escape (fate
    /// 5) or die escaping (6). Returns the fate sent (0x4a) and the report's follow-up kind (8..=0xe).
    pub fn judge(&self, rng: &mut Streams, owner: usize, k: usize, choice: i32) -> (i32, u8) {
        let s = self.roster.get(owner).and_then(|r| r.get(k)).copied().unwrap_or_default();
        match choice {
            2 | 3 => {
                if rng.range(UI_STREAM, 1, s.skills[skill::LOYALTY] as i32 + 3) < 6 && s.master == 0 {
                    (choice, if choice == 2 { 8 } else { 9 })
                } else if rng.range(UI_STREAM, 1, 2) == 1 {
                    (5, 10)
                } else {
                    (6, 11)
                }
            }
            1 => (1, 0xd),
            4 => (4, 0xe),
            _ => (choice, 0),
        }
    }
}

/// `KemEselySzamolas2`'s body: [`chance`], 0 for missions without a countdown, a kidnap with no one to take
/// (`t == 0`, taken already, or `t`'s race 5 in SP), halved against race 5 in SP.
pub fn mission_chance(rule: &MissionRule, infiltration: u8, m: usize, ctx: &SpyContext, e: usize, t: usize, kidnapped: i32) -> i32 {
    let mut c = chance(rule, infiltration, m, ctx, e);
    if rule.no_countdown {
        c = 0;
    }
    let mp = ctx.multiplayer;
    if m == mission::KIDNAP && (t == 0 || kidnapped != 0 || (!mp && t == 5)) {
        c = 0;
    }
    if !mp && ctx.race(t) == 5 {
        c /= 2;
    }
    c
}

/// The success chance in percent (spec §5.1, f32).
pub fn chance(rule: &MissionRule, infiltration: u8, m: usize, ctx: &SpyContext, e: usize) -> i32 {
    let mut mult: f32 = if ctx.race(e) == 3 { 2.0 } else { 1.0 };
    if ctx.difficulty == 1 && !ctx.multiplayer && ctx.human(e) {
        mult *= 2.0;
    }
    let (floor, full, max) = (rule.skill_floor as i32, rule.skill_full as i32, rule.chance as i32);
    let s = infiltration as i32;
    let mut c = if s < floor {
        0
    } else {
        let f = if s < full {
            let mut f = (s - floor + 1) as f32 / (full - floor + 1) as f32;
            if f > 1.0 {
                f = 1.0;
            }
            max as f32 * f
        } else {
            max as f32
        };
        (f * mult) as i32
    };
    if c > max + 10 {
        c = max + 10;
    }
    if c > 90 && m > 1 {
        c = 90;
    } else if c > 100 {
        c = 100;
    }
    c
}

/// `st_eSearch2spy(4, owner, p, tech)` (spec §5.6): a random finished building, `(planet, slot)` (1-based).
pub(crate) fn search_building(w: &mut World, owner: usize, p: i32, tech: i32) -> (usize, usize) {
    let mut found = (0, 0);
    let mut i = 0;
    for q in 1..w.colony.planets.len() {
        if !((p == 0 || p as usize == q) && w.colony.planets[q].owner as usize == owner) {
            continue;
        }
        for slot in 1..=w.colony.planets[q].buildings.len() {
            let b = &w.colony.planets[q].buildings[slot - 1];
            let fits = tech == 0 || w.rules.building_tech.get(b.kind as usize).copied().unwrap_or(0) as i32 == tech;
            if b.kind != 0 && b.work_left == 0 && fits {
                if w.r(0, i) == 0 {
                    found = (q, slot);
                }
                i += 1;
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Planet;
    use crate::spacewar::{CombatFleet, Design, Ship};

    fn rules() -> SpyRules {
        let mut missions = vec![MissionRule::default(); 27];
        for (m, r) in missions.iter_mut().enumerate().skip(1).take(12) {
            *r = MissionRule { kind: (m == 6) as i32, xp: 1000, skill_floor: 1, skill_full: 5, chance: 100, needs_target: true, ..Default::default() };
        }
        missions[13] = MissionRule { xp: 500, skill_floor: 1, skill_full: 5, chance: 100, no_countdown: true, ..Default::default() };
        let mut template = Spy { state: 1, rank: 1, ..Default::default() };
        template.name[..4].copy_from_slice(b"Tmpl");
        SpyRules {
            missions,
            outcome_state: vec![0, state::NONE as i32, state::CAPTURED as i32, state::RETURNING as i32, state::HOME as i32],
            rank_points: vec![3; 15],
            templates: vec![template; 15],
            rank_xp: (0..22).map(|r| if r == 21 { 0 } else { r * 1000 }).collect(),
            training_xp: vec![0, 100, 200, 300, 400, 500],
            names: (0..37).map(|i| format!("Spy{i}").into_bytes()).collect(),
            building_tech: (0..60).collect(),
            hire_cost_mult: vec![1.0; 15],
        }
    }

    struct Fixture {
        ctx: SpyContext,
        colony: ColonyWorld,
        side: SideRules,
        econ: EconomyContext,
        combat: CombatWorld,
        research: Research,
        rr: ResearchRules,
        rc: ResearchContext,
        rel: Vec<[Relation; EMPIRES]>,
        kidnap: [i32; 18],
        rng: Streams,
        rules: SpyRules,
    }

    impl Fixture {
        fn new() -> Self {
            let mut ctx = SpyContext { program_players: 8, human_empire: 6, day: 5, ..Default::default() };
            for e in 1..=8 {
                ctx.alive[e] = true;
                ctx.race[e] = e as u8;
            }
            ctx.human[6] = true;
            let mut colony = ColonyWorld { occupied: vec![0; 201], ..Default::default() };
            colony.planets = vec![Planet::default(); 6];
            for p in 1..6 {
                colony.planets[p].owner = if p <= 3 { 3 } else { 4 };
                colony.planets[p].population = 1000 * p as i32;
            }
            let mut rng = Streams::default();
            rng.set_state(STREAM, 12345);
            Self {
                ctx,
                colony,
                side: SideRules::default(),
                econ: EconomyContext::default(),
                combat: CombatWorld::default(),
                research: Research::default(),
                rr: ResearchRules::default(),
                rc: ResearchContext { players: 8, ..Default::default() },
                rel: vec![[Relation::default(); EMPIRES]; EMPIRES],
                kidnap: [0; 18],
                rng,
                rules: rules(),
            }
        }

        fn run<R>(&mut self, f: impl FnOnce(&mut World) -> R) -> (R, Vec<SpyEvent>) {
            let mut story = |_: StoryCheck| false;
            let mut w = World {
                ctx: &self.ctx,
                colony: &mut self.colony,
                side_rules: &self.side,
                econ: &self.econ,
                combat: &mut self.combat,
                research: &mut self.research,
                research_rules: &self.rr,
                research_ctx: &self.rc,
                relations: &mut self.rel,
                kidnap: &mut self.kidnap,
                rng: &mut self.rng,
                rules: &self.rules,
                story: &mut story,
                events: Vec::new(),
            };
            let r = f(&mut w);
            (r, w.events)
        }
    }

    fn on_mission(m: usize, target: u8, skill: u8) -> Spy {
        Spy { state: state::MISSION, mission: m as u16, target, left: 50, catch_at: -1000, skills: [skill; 4], rank: 1, ..Default::default() }
    }

    #[test]
    fn reports_reach_each_human_with_a_queue() {
        let mut f = Fixture::new();
        let r = Report { kind: 2, a: 1, ..Default::default() };
        let (_, ev) = f.run(|w| {
            w.report(6, r);
            w.report(3, r);
        });
        // Classic: only `EN`'s.
        assert_eq!(ev, vec![SpyEvent::Report(r)]);
        f.ctx.listed[3] = true;
        let (_, ev) = f.run(|w| {
            w.report(6, r);
            w.report(3, r);
            w.report(4, r);
        });
        assert_eq!(ev, vec![SpyEvent::Report(r), SpyEvent::ReportTo { to: 3, report: r }]);
    }

    #[test]
    fn record_round_trips() {
        let b: Vec<u8> = (0..RECORD as u8).collect();
        assert_eq!(Spy::from_bytes(&b).to_bytes().to_vec(), b);
    }

    #[test]
    fn chance_scales_and_caps_in_f32() {
        let rule = MissionRule { skill_floor: 3, skill_full: 7, chance: 50, ..Default::default() };
        let ctx = SpyContext { race: [3; EMPIRES], ..Default::default() };
        assert_eq!(chance(&rule, 2, 4, &ctx, 1), 0);
        // (5 - 3 + 1) / (7 - 3 + 1) = 0.6 · 50 = 30 · 2 = 60, capped at max + 10.
        assert_eq!(chance(&rule, 5, 4, &ctx, 1), 60);
        let rule = MissionRule { skill_floor: 1, skill_full: 2, chance: 95, ..Default::default() };
        assert_eq!(chance(&rule, 9, 4, &ctx, 1), 90);
        assert_eq!(chance(&rule, 9, 1, &ctx, 1), 100);
    }

    #[test]
    fn countdown_brings_a_spy_home() {
        let mut fx = Fixture::new();
        let mut spies = Spies::new(&fx.rules, &[1; 27]);
        spies.count[3] = 1;
        spies.roster[3][1] = Spy { state: state::RETURNING, left: 40, rank: 1, ..Default::default() };
        fx.run(|w| spies.daily(w));
        assert_eq!(spies.roster[3][1].state, state::HOME);
        assert_eq!(spies.roster[3][1].left, -10);
        assert_eq!(spies.working[3], 1);
        assert_eq!(spies.enabled[mission::KIDNAP], 0);
    }

    #[test]
    fn ranks_follow_experience() {
        let mut fx = Fixture::new();
        let mut spies = Spies::new(&fx.rules, &[1; 27]);
        spies.count[2] = 1;
        spies.roster[2][1] = Spy { state: state::HOME, xp: 3500, rank: 1, race: 2, ..Default::default() };
        fx.run(|w| spies.daily(w));
        assert_eq!(spies.roster[2][1].rank, 3);
        assert_eq!(spies.roster[2][1].points, 6);
    }

    #[test]
    fn kidnapping_marks_the_family_member() {
        let mut fx = Fixture::new();
        let mut spies = Spies::new(&fx.rules, &[1; 27]);
        spies.count[3] = 1;
        spies.roster[3][1] = on_mission(mission::KIDNAP, 4, 200);
        let (_, ev) = fx.run(|w| spies.daily(w));
        assert!((1..=5).contains(&fx.kidnap[4]));
        assert_eq!(fx.kidnap[13], 3);
        assert!(ev.iter().any(|e| matches!(e, SpyEvent::MissionDone { ok: true, .. })));
        assert_eq!(fx.rel[4][3].v[field::SPY_GRIEVANCE], 500);
        assert_eq!(spies.roster[3][1].state, state::HOME);
        assert_eq!(spies.roster[3][1].mission, 0);
    }

    #[test]
    fn stealing_a_tech_develops_it() {
        let mut fx = Fixture::new();
        for t in 1..=LAST_TECH {
            fx.colony.tech.set_status(t, 3, status::UNKNOWN);
            fx.colony.tech.set_status(t, 4, if t == 17 { status::OWNED } else { status::UNKNOWN });
        }
        let mut spies = Spies::new(&fx.rules, &[1; 27]);
        spies.count[3] = 1;
        spies.roster[3][1] = on_mission(mission::STEAL_TECH, 4, 200);
        fx.run(|w| spies.daily(w));
        assert_eq!(fx.colony.tech.status(17, 3), status::OWNED);
    }

    #[test]
    fn sabotage_and_ship_strikes_remove_their_target() {
        let mut fx = Fixture::new();
        fx.colony.planets[4].buildings = vec![crate::colony::Building { kind: 12, ..Default::default() }];
        fx.colony.occupied[4] = 1;
        fx.combat.fleets = vec![CombatFleet { owner: 0xff, ..Default::default() }, CombatFleet { owner: 4, ..Default::default() }];
        fx.combat.designs = vec![Vec::new(); 19];
        fx.combat.designs[4] = vec![Design::from_bytes(&[0; 0x84]); 3];
        fx.combat.ships = vec![Ship { fleet: 0, design: NO_DESIGN }, Ship { fleet: 1, design: 2 }];
        let mut spies = Spies::new(&fx.rules, &[1; 27]);
        spies.count[3] = 2;
        spies.roster[3][1] = on_mission(mission::SABOTAGE, 4, 200);
        spies.roster[3][2] = on_mission(mission::DESTROY_SHIP, 4, 200);
        let (_, ev) = fx.run(|w| spies.daily(w));
        assert_eq!(fx.colony.planets[4].buildings[0].kind, 0);
        assert_eq!(fx.colony.occupied[4], 0);
        assert_eq!(fx.combat.ships[1].design, NO_DESIGN);
        assert!(ev.iter().any(|e| matches!(e, SpyEvent::ShipDestroyed { ship: 1, .. })));
    }

    #[test]
    fn a_caught_spy_angers_the_target_and_meets_it() {
        let mut fx = Fixture::new();
        fx.ctx.forced = 2;
        let mut spies = Spies::new(&fx.rules, &[1; 27]);
        spies.count[3] = 1;
        spies.roster[3][1] = Spy { left: 100, catch_at: 75, ..on_mission(mission::STEAL_MONEY, 4, 5) };
        let (_, ev) = fx.run(|w| spies.daily(w));
        assert_eq!(fx.colony.knows[4][3], 1);
        assert_eq!(fx.rel[4][3].v[field::SPY_GRIEVANCE], 2000);
        assert_eq!(spies.roster[3][1].known_to[4], 1);
        assert_eq!(spies.roster[3][1].catch_at, -1000);
        assert!(ev.iter().any(|e| matches!(e, SpyEvent::Caught { outcome: 2, fate: Some(_), .. })));
    }

    #[test]
    fn recruits_get_unique_names_and_a_rank() {
        let mut fx = Fixture::new();
        fx.ctx.forced = 3;
        let mut spies = Spies::new(&fx.rules, &[1; 27]);
        for _ in 0..40 {
            fx.run(|w| spies.daily(w));
        }
        for e in 1..ROSTERS {
            let names: Vec<_> = (1..=spies.len(e)).filter(|&k| spies.roster[e][k].state > 0).map(|k| spies.roster[e][k].name).collect();
            let mut dedup = names.clone();
            dedup.sort();
            dedup.dedup();
            assert_eq!(names.len(), dedup.len());
            for k in 1..=spies.len(e) {
                let s = &spies.roster[e][k];
                assert!(s.state == 0 || (1..=20).contains(&s.rank));
                assert!(e != 6 || s.state == 0 || s.applicant == 1);
            }
            assert!(spies.count[e] <= MAX_SPIES);
        }
        assert!(spies.count.iter().sum::<i32>() > 0);
    }

    #[test]
    fn reports_post_by_kind_and_outcome() {
        let r = |kind, b, c| Report { kind, b, c, ..Default::default() }.messages();
        assert_eq!(r(1, 4, 0), [100]);
        assert_eq!(r(1, 4, 1), [101]);
        assert_eq!(r(1, 2, 0), [0; 0]);
        assert_eq!(r(1, 1, 0), [101]);
        assert_eq!(r(2, 0, 0), [102]);
        assert_eq!(r(6, 0, 0), [106]);
        assert_eq!(r(7, 4, 0), [0; 0]);
        assert_eq!(r(0xc, 0, 0), [0; 0]);
        assert_eq!(r(0xf, 0, 0), [0x65]);
        assert_eq!(r(0x10, 0, 0), [0x71]);
        assert_eq!(r(0x11, 0, 0), [0x6a]);
        assert_eq!(r(0x12, 0, 0), [0x65]);
        let mut rec = [0xaau8; REPORT];
        Report { kind: 1, a: 3, b: 4, c: 1, d: 2, x: [5, 6, 7, 8, 9, 10, 11, 12, -1] }.write(&mut rec);
        assert_eq!(&rec[..0xc], &[1, 0xaa, 0xaa, 0xaa, 3, 0, 0, 0, 4, 1, 2, 0xaa]);
        assert_eq!(&rec[0x2c..], &[0xff; 4]);
    }
}
