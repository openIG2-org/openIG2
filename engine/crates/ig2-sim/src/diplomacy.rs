//! Diplomacy: the daily relation update (`FRISSIT_Viszony`, exe 0x50df60), the AI's daily messages
//! (`MI_DIPLOMACY`, exe 0x4c7060) with the answers and their effects, the human's messages, deals and
//! answers (`FeldolgozAjanlat` 0x4c1240, `FeldolgozValasz` 0x4c1480) and the per-tick bookkeeping of the
//! human's contact list (`FRISSIT_Diplomacy`, 0x4c0f90). Spec: `re/rules/diplomacy.md`.
//!
//! Every AI decision draws on seeded stream 9. The original's libc `rand()` picks dialogue texts and flips
//! the coin of the AI's answer to a counter-proposal: all of it comes from [`World::libc`] (spec §9; ADR
//! 0003: a seeded stream in the engine). Deals reach the colony, money and research systems through the
//! [`Deals`] hook ([`ColonyDeals`] is the real one). Texts, story hooks and an empire's death are events.

use crate::colony::{self, EconomyContext};
use crate::colony_side::{status, ColonyWorld, SideEvent, SideRules, TechTable, EMPIRES};
use crate::fleet::{order, Starmap, StarmapContext, StarmapEvent, DELETED};
use crate::research::{self, Research, ResearchContext, ResearchEvent, ResearchRules};
use crate::rng::Streams;

/// The diplomacy stream (`rndlog(9, …)`).
pub const STREAM: usize = 9;
/// Values per relation record.
pub const FIELDS: usize = 18;
/// `DiplomaciaMikorHivtaUtoljara` is declared `[9][9]` but indexed `a·9 + b` with a, b ≤ 18 (spec §4).
pub const LAST_CONTACT_CELLS: usize = 9 * 18 + 18 + 1;
/// Techs whose status the alliance pass looks at (1..=192).
const LAST_TECH: usize = 192;

/// `Viszony[a][b].+0x25`.
pub mod level {
    pub const WAR: u8 = 1;
    pub const CEASEFIRE: u8 = 2;
    pub const NEUTRAL: u8 = 3;
    pub const NAP: u8 = 4;
    pub const ALLIANCE: u8 = 5;
}

/// Indices into [`Relation::v`] (spec §2).
pub mod field {
    pub const SPY_GRIEVANCE: usize = 0;
    pub const TRUST: usize = 1;
    pub const STORY: usize = 2;
    pub const ATTITUDE: usize = 3;
    pub const MESSAGES: usize = 4;
    pub const UNRELIABLE: usize = 5;
    pub const FRIENDLY: usize = 6;
    pub const RESPECT: usize = 7;
    pub const FAVOUR: usize = 8;
    pub const THREATS: usize = 9;
    pub const INSULTS: usize = 10;
    pub const GREETINGS: usize = 12;
    pub const SCHEMING: usize = 13;
    pub const POWER: usize = 14;
    pub const FRIENDS: usize = 15;
    pub const DISTANCE: usize = 16;
    pub const OPINION: usize = 17;
}

/// Message categories (`ajanlat.eng` codes, spec §5).
pub mod cat {
    pub const NEGOTIATE: i32 = 0;
    /// The sender gives `sub/value` for the receiver's `sub2/value2`.
    pub const OFFER_FOR: i32 = 1;
    /// The sender demands `sub/value`.
    pub const DEMAND: i32 = 2;
    /// The sender gives `sub/value`.
    pub const OFFER: i32 = 3;
    /// A story special (`sub` indexes `SpecialLeiro`).
    pub const SPECIAL: i32 = 4;
    pub const TALK: i32 = 6;
}

/// Deal items (`sub`/`sub2` of cat 1–3; `value` says which), spec §6.3.
pub mod item {
    pub const MONEY: i32 = 0;
    /// The giver's radar map of empire `value`'s planets.
    pub const RADAR: i32 = 1;
    pub const PLANET: i32 = 2;
    pub const TECH: i32 = 3;
    /// Release a kidnapped family member (`value` = special 1..=5).
    pub const HOSTAGE: i32 = 4;
    /// Declare war on `value`.
    pub const WAR: i32 = 5;
    /// Make peace with `value`.
    pub const PEACE: i32 = 6;
}

/// `cat::NEGOTIATE` subjects.
pub mod negotiate {
    pub const CEASEFIRE: i32 = 0;
    pub const PEACE: i32 = 1;
    pub const DECLARE_WAR: i32 = 2;
    pub const NAP: i32 = 3;
    pub const BREAK_NAP: i32 = 4;
    pub const ALLIANCE: i32 = 5;
    pub const BREAK_ALLIANCE: i32 = 6;
    pub const WAR_TOGETHER: i32 = 7;
    pub const TRADE: i32 = 8;
}

/// `cat::TALK` subjects.
pub mod talk {
    pub const URGE_PEACE: i32 = 0;
    pub const THREAT: i32 = 1;
    pub const IMPROVE: i32 = 2;
    pub const SPIES: i32 = 3;
    pub const INSULT: i32 = 4;
    pub const SCHEMING: i32 = 6;
    pub const GREETING: i32 = 8;
    pub const URGE_ALLIANCE: i32 = 9;
}

/// One `Viszony[a][b]` record (0x26 bytes): a's view of b.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Relation {
    /// `+2i`, see [`field`].
    pub v: [i16; FIELDS],
    /// `+0x24`: trade agreement.
    pub trade: u8,
    /// `+0x25`: see [`level`].
    pub level: u8,
}

impl Relation {
    pub const SIZE: usize = 0x26;

    pub fn from_bytes(b: &[u8]) -> Self {
        Self { v: std::array::from_fn(|i| i16::from_le_bytes([b[2 * i], b[2 * i + 1]])), trade: b[0x24], level: b[0x25] }
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        for (i, x) in self.v.iter().enumerate() {
            out[2 * i..2 * i + 2].copy_from_slice(&x.to_le_bytes());
        }
        out[0x24] = self.trade;
        out[0x25] = self.level;
        out
    }

    fn add(&mut self, f: usize, d: i32) {
        self.v[f] = self.v[f].wrapping_add(d as i16);
    }
}

/// The tables diplomacy reads from the user's exe (ADR 0004).
#[derive(Debug, Clone, Default)]
pub struct DiplomacyRules {
    /// `Megbizhatatlansag_DeclareWar` (exe 0x902FC0): `[undeclared][row][0 victim, 1 others]` unreliability
    /// added by a war (spec §8.1).
    pub declare_war: [[[i32; 2]; 4]; 2],
    /// `FajBolygoViszony` (exe 0x90B9D8), flat `race·9 + surface`: planet values (spec §6.3).
    pub surface_affinity: Vec<i32>,
    /// `FajSpeckoKutatasMoneySzorzo` / `FajSpeckoKutatasIdoSzorzo` per race: tech values.
    pub research_money: Vec<f32>,
    pub research_time: Vec<f32>,
}

impl DiplomacyRules {
    /// From the 16 i32 of the exe table in memory order.
    pub fn from_table(v: &[i32]) -> Self {
        let mut declare_war = [[[0; 2]; 4]; 2];
        for (i, x) in v.iter().take(16).enumerate() {
            declare_war[i / 8][i / 2 % 4][i % 2] = *x;
        }
        Self { declare_war, ..Default::default() }
    }
}

/// A message `(category, subject, value, subject 2, value 2)` (spec §10.1). For negotiate and talk
/// messages `value` is the third empire; the AI's own messages have `sub2 = value2 = −1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Message {
    pub cat: i32,
    pub sub: i32,
    pub value: i32,
    pub sub2: i32,
    pub value2: i32,
}

impl Message {
    /// A message as the AI sends it (`ElkuldUzenetMI(…, −1, −1, −1, …)`).
    pub const fn ai(cat: i32, sub: i32, value: i32) -> Self {
        Self { cat, sub, value, sub2: -1, value2: -1 }
    }
}

/// A message as the human's contact list keeps it (`+0x30`): sender, receiver, declaration flag.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Letter {
    pub from: i32,
    pub to: i32,
    pub declaration: bool,
    pub msg: Message,
}

/// An answer (`DiplomaciaiValaszTipus`, spec §10.1) from `from` to the proposer `to`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Answer {
    pub from: i32,
    pub to: i32,
    pub msg: Message,
    /// `+0x10`: a counter-proposal (only the human's screen sends one).
    pub counter: bool,
    /// `+0x11`: the AI's reply to a counter-proposal.
    pub reply: bool,
    /// `+0x12`.
    pub yes: bool,
}

/// One entry of the human's contact list (`AktPlayerek`, 0xc4 bytes), spec §10.1: what the simulation
/// reads and writes (the rest is dialogue state).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerSlot {
    /// `+0x00`: the empire.
    pub empire: i32,
    /// `+0x04`: `PlayerKi[empire]` when it was met (`FelfedezBirodalom`).
    pub controller: i32,
    /// `+0x0c`: screen state, cleared by answers and auto-cancels.
    pub screen: u8,
    /// `+0x10`: the `Ido` of the human's last message to this empire (the screen waits 250 before the next).
    pub sent_at: i32,
    /// `+0x14`: set by `InitBirodalom`; messages of this empire reach the human.
    pub open: u8,
    /// `+0x16`: the human's message to this empire awaits its answer (set by the screen).
    pub asked: u8,
    /// `+0x17`: a message of this empire waits for the human's answer.
    pub waiting: u8,
    /// `+0x18` / `+0x2c`: an answer / a counter-answer arrived.
    pub answered: u8,
    pub countered: u8,
    /// `+0x30`: the waiting message; `+0x58`: auto-cancelled.
    pub letter: Letter,
    pub cancelled: u8,
    /// `+0x64` / `+0x94`: the last answer / counter-answer.
    pub answer: Answer,
    pub counter_answer: Answer,
}

/// What the human's diplomacy screen writes to its own contact-list slot of an empire (`DiplomGfx2D_1` ELF
/// 0x511190, `re/screens/diplomacy.md` §7, §9), as a lockstep command applied before the message or answer it
/// belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenOp {
    /// "Send it now!": `+0x0c = 1`, `+0x16 = 1`, `+0x10 = Ido`.
    Sent,
    /// The waiting message was answered: `+0x17 = 0`.
    Answered,
    /// The arrived answer was acknowledged: `+0x18 = 0`.
    AnswerSeen,
    /// The counter-answer was answered: `+0x2c = 0`.
    CounterSeen,
    /// `CancelLastMessage` (multiplayer): `+0x16 = 0`, `+0x0c = 0`; packet 0x40 clears the receiving human's
    /// waiting message (`+0x17 = 0`, `MULTIGET_Diplomacy` ELF 0x521118).
    Cancelled,
    /// The birthday greeting was sent: `VoltmarUdvozloUzenetEbbenAzEvben[empire] = 1`.
    Greeted,
}

/// Another human's contact list in canonical multiplayer (`re/rules/multiplayer.md` §5.1): what
/// [`Diplomacy::players`], [`Diplomacy::ghost`] and [`Diplomacy::shown_players`] are for the canonical `EN`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContactList {
    pub empire: u8,
    pub players: Vec<PlayerSlot>,
    pub ghost: PlayerSlot,
    pub shown: i32,
}

/// What `DiplomaciaErtek` reads of a planet (spec §6.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanetTerms {
    /// `+0x44`: last month's tax.
    pub tax: i32,
    /// `+0x20` / `+0x21` / `+0x22`.
    pub owner: u8,
    pub race: u8,
    pub surface: u8,
    /// `GetBolygoGazdagsag`.
    pub richness: i32,
    /// `getMiAltalElfoglalhato`.
    pub capturable: bool,
    /// `BolygoNeededMoral[p]`.
    pub needed_morale: i32,
    /// `SearchBuilding(p, k, false)` for k = 0..=5: finished buildings of kind k.
    pub buildings: [i32; 6],
}

/// What deals (spec §6.3, §7) touch outside diplomacy. `()` touches nothing (for callers without deals).
pub trait Deals {
    /// `Money[e]`.
    fn money(&self, _e: usize) -> f64 {
        0.0
    }
    /// `MoneyHozzaadLog(e, 0, x)`.
    fn add_money(&mut self, _e: usize, _x: f64) {}
    /// Planet `p` as `DiplomaciaErtek` sees it.
    fn planet_terms(&self, _p: usize) -> PlanetTerms {
        PlanetTerms::default()
    }
    /// `BolygoElfoglal(p, owner)`, keeping `w.planets`, `w.tech` and `w.starmap` in step. Returns the
    /// `(human, empire)` meetings it caused (`FelfedezBirodalom`, in order).
    fn transfer_planet(&mut self, _w: &mut World<'_>, _p: usize, _owner: u8) -> Vec<(u8, u8)> {
        Vec::new()
    }
    /// `TalalmanyKifejlesztve(e, t)`.
    fn develop(&mut self, _tech: &mut TechTable, _e: usize, _t: usize) {}
    /// A radar-map deal made planet `p` visible to `e` (`+0xd8 + 2e = 1`; `w.planets` is already updated).
    fn reveal(&mut self, _p: usize, _e: usize) {}
}

impl Deals for () {}

/// The real [`Deals`]: the colony world (`colony_side`), money and research. Diplomacy's own
/// [`World::tech`] and `starmap.met` are authoritative; a planet transfer swaps them in and out.
pub struct ColonyDeals<'a> {
    pub world: &'a mut ColonyWorld,
    pub rules: &'a SideRules,
    pub ctx: &'a EconomyContext,
    pub research: &'a mut Research,
    pub research_rules: &'a ResearchRules,
    pub research_ctx: &'a ResearchContext,
    pub events: Vec<SideEvent>,
    pub research_events: Vec<ResearchEvent>,
}

impl Deals for ColonyDeals<'_> {
    fn money(&self, e: usize) -> f64 {
        self.world.money[e].total
    }

    fn add_money(&mut self, e: usize, x: f64) {
        self.world.add_money(e, 0, x, self.rules, self.ctx);
    }

    fn planet_terms(&self, p: usize) -> PlanetTerms {
        let Some(pl) = self.world.planets.get(p) else { return PlanetTerms::default() };
        let mut buildings = [0; 6];
        for b in &pl.buildings {
            if (b.kind as usize) < 6 && b.work_left == 0 {
                buildings[b.kind as usize] += 1;
            }
        }
        PlanetTerms {
            tax: pl.tax_last,
            owner: pl.owner,
            race: pl.race,
            surface: pl.surface,
            richness: colony::production_base(pl, &self.rules.colony, self.ctx),
            capturable: pl.owner < 9 && !pl.ai_proof,
            needed_morale: self.world.needed_morale.get(p).copied().unwrap_or(0),
            buildings,
        }
    }

    fn transfer_planet(&mut self, w: &mut World<'_>, p: usize, owner: u8) -> Vec<(u8, u8)> {
        if p >= self.world.planets.len() {
            return Vec::new();
        }
        std::mem::swap(&mut self.world.tech, w.tech);
        self.world.knows = w.starmap.met;
        self.world.alive = w.ctx.alive;
        let mut events = Vec::new();
        self.world.transfer(p, owner, self.rules, self.ctx, &mut events);
        std::mem::swap(&mut self.world.tech, w.tech);
        w.starmap.met = self.world.knows;
        let pl = &self.world.planets[p];
        if let Some(d) = w.planets.get_mut(p) {
            d.owner = pl.owner;
            d.visible = pl.visibility;
        }
        let mut met = Vec::new();
        for e in &events {
            match *e {
                // `BolygoElfoglal` stops the fleets attacking the planet (`order 2, kind 1, target p`).
                SideEvent::RecallAttackers { planet } => {
                    for f in 1..w.starmap.fleets.len() {
                        let fl = &w.starmap.fleets[f];
                        if fl.owner != DELETED && fl.order == order::ATTACK && fl.kind == order::PLANET && fl.tx == planet as f32 {
                            stop_fleet(w, f);
                        }
                    }
                }
                SideEvent::Met { who, whom } => met.push((who, whom)),
                _ => {}
            }
        }
        self.events.extend(events);
        met
    }

    fn develop(&mut self, tech: &mut TechTable, e: usize, t: usize) {
        research::developed(tech, self.research, e, t, self.research_rules, self.research_ctx, &mut self.research_events);
    }

    fn reveal(&mut self, p: usize, e: usize) {
        if let Some(v) = self.world.planets.get_mut(p).and_then(|pl| pl.visibility.get_mut(e)) {
            *v = 1;
        }
    }
}

/// `FlottaMozgat(f, 0, 0, 0, 0)`: the fleet stops (a freed colony ship or terraformer returns to its pool).
fn stop_fleet(w: &mut World, f: usize) {
    let mut ev = Vec::new();
    w.starmap.order(f, order::NONE, order::POINT, 0.0, 0.0, w.star_ctx, &mut ev);
    for e in ev {
        match e {
            StarmapEvent::ColonyShipFreed { empire } => w.starmap.colony_ship_pool[empire as usize] += 1,
            StarmapEvent::TerraformerFreed { empire } => w.starmap.terraformer_pool[empire as usize] += 1,
            _ => {}
        }
        w.events.push(DiploEvent::Starmap(e));
    }
}

/// State owned by diplomacy.
#[derive(Debug, Clone, PartialEq)]
pub struct Diplomacy {
    /// `Viszony[a][b]`.
    pub relations: Vec<[Relation; EMPIRES]>,
    /// `BirodalomEro`, `BirodalomUrEro`, `BirodalomHanyBolygo`, `BirodalomVisszaSzamlalas`.
    pub power: [f32; EMPIRES],
    pub space_power: [f32; EMPIRES],
    pub planets: [i32; EMPIRES],
    pub countdown: [i32; EMPIRES],
    /// `MiVeszelyAllapot` (1..=5).
    pub danger: [i32; EMPIRES],
    /// `DiplomaciaMikorHivtaUtoljara`, flat `a·9 + b` ([`LAST_CONTACT_CELLS`]).
    pub last_contact: Vec<i32>,
    /// `BirodalomMiotaIsmeri[a][b]`.
    pub known_since: [[i32; EMPIRES]; EMPIRES],
    /// `AktPlayerek[0..MaxPlayerek]`.
    pub players: Vec<PlayerSlot>,
    /// What `AktPlayerek[-1]` reads and writes when an empire has no slot (empires ≥ 9: the bytes before the
    /// array, kept as their own slot).
    pub ghost: PlayerSlot,
    /// `HanyevesAzUralkodo[e]` / `VoltmarUdvozloUzenetEbbenAzEvben[e]`: ruler age, greeting sent this year
    /// (they only feed texts).
    pub ruler_age: [i32; EMPIRES],
    pub greeted: [u8; EMPIRES],
    /// `DipOldNap`: the day [`Diplomacy::tick`] last saw.
    pub last_day: i32,
    /// `DiplomacyStarted` (set by `NEWGAME_Diplomacy`).
    pub started: bool,
    /// `MostHanyPlayerVan`: the contact count the screen shows, `MaxPlayerek` as of the last tick.
    pub shown_players: i32,
    /// Canonical multiplayer: every other human's contact list, by empire (empty in single player).
    pub others: Vec<ContactList>,
}

impl Default for Diplomacy {
    fn default() -> Self {
        Self {
            relations: vec![[Relation::default(); EMPIRES]; EMPIRES],
            power: [0.0; EMPIRES],
            space_power: [0.0; EMPIRES],
            planets: [0; EMPIRES],
            countdown: [0; EMPIRES],
            danger: [0; EMPIRES],
            last_contact: vec![0; LAST_CONTACT_CELLS],
            known_since: [[0; EMPIRES]; EMPIRES],
            players: Vec::new(),
            ghost: PlayerSlot::default(),
            ruler_age: [0; EMPIRES],
            greeted: [0; EMPIRES],
            last_day: 0,
            started: true,
            shown_players: 0,
            others: Vec::new(),
        }
    }
}

/// Global inputs.
#[derive(Debug, Clone, Default)]
pub struct DiplomacyContext {
    /// `ProgramPlayers` / `ProgramPlayers2`.
    pub program_players: usize,
    pub program_players2: usize,
    /// `BirodalomEl` (cleared here when an empire dies), `BirodalomMIzik`, `BirodalomTamadhat`, `PlayerKi == 1`.
    pub alive: [bool; EMPIRES],
    pub ai: [bool; EMPIRES],
    pub may_attack: [bool; EMPIRES],
    pub human: [bool; EMPIRES],
    /// `PlayerKi`: who controls each empire (1 = a human), copied into a new contact slot.
    pub player_ki: [u8; EMPIRES],
    /// `PlayerFaj`.
    pub race: [u8; EMPIRES],
    /// `EN`.
    pub human_empire: usize,
    pub multiplayer: bool,
    /// `DEBUGX`.
    pub debug: bool,
    /// `DiplomaciaEngedelyezve`.
    pub enabled: bool,
    /// exe 0xB77244: no `FRISSIT_MI`, no `MI_DIPLOMACY`.
    pub ai_off: bool,
    /// `StoryAdat+0xb0 != 0`: the story holds back AI proposals.
    pub story_quiet: bool,
    /// `StoryAdat+0xd0 + 4e != 0`: `e` must not pick the human as a target.
    pub spare_human: [bool; EMPIRES],
    /// `Nehezseg`.
    pub difficulty: i32,
    /// Tutorial active or paused.
    pub tutorial: bool,
    /// exe 0xB7724C: 0 = the AI decides, 1 = always no, other = always yes.
    pub forced_answer: i32,
    /// `MiFoFlotta`: the AI's main fleet (a fleet slot, 0 reads slot 0).
    pub main_fleet: [i32; EMPIRES],
    /// `Honap`, `Nap` (for [`Diplomacy::tick`]).
    pub month: i32,
    pub day: i32,
    /// `CsaladtagElrabolvaKi[0..9]` then `CsaladtagElrabolvaKialtal[0..9]` (exe 0xBB405C, contiguous): the
    /// spy system's kidnapped family members and by whom; deals release them.
    pub kidnap: [i32; 18],
    /// The "all techs" switch (exe 0xB77254), for the tech-status recompute after a tech deal.
    pub all_techs: bool,
}

impl DiplomacyContext {
    /// `CsaladtagElrabolvaKi[e]` (indices past 8 read the next array, as in the original).
    pub fn kidnapped(&self, e: usize) -> i32 {
        self.kidnap.get(e).copied().unwrap_or(0)
    }

    /// A deal releases `e`'s family member: `Ki[e] = Kialtal[e] = 0`.
    fn release(&mut self, e: usize) {
        for i in [e, 9 + e] {
            if let Some(x) = self.kidnap.get_mut(i) {
                *x = 0;
            }
        }
    }
}

/// What diplomacy reads of a planet.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DiploPlanet {
    pub owner: u8,
    pub x: f32,
    pub y: f32,
    /// `+0x28`, `+0x5c`, `+0x70`, `+0x74`.
    pub population: i32,
    pub defence: i32,
    pub ship_capacity: i32,
    pub vehicle_capacity: i32,
    /// `BolygoMennyiErodEro[p]`.
    pub fortification: i32,
    /// `+0xd8 + 2e`.
    pub visible: [i16; EMPIRES],
}

/// Effects other systems or the UI carry out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiploEvent {
    /// `TutorialCheckEvent(id)` from `FeldolgozValasz` (exe 0x4c16ff, `re/rules/tutorial.md` §4).
    Tutorial(i32),
    /// `BirodalomMeghal(e)` (exe 0x53b630): `alive[e]` is cleared here, the rest is the empire system's.
    EmpireDied { empire: u8 },
    /// A message and, for an AI receiver, its answer (`None` = it waits for the human).
    Message { from: u8, to: u8, msg: Message, accepted: Option<bool> },
    /// An answer was processed (`FeldolgozValasz`; a counter-proposal's own answer is the AI's reply).
    Answered(Answer),
    /// `FelfedezBirodalom`: human `by` learns of an empire (contact list, scroll message).
    Discovered { empire: u8, by: u8 },
    /// A war started (`DiplomaciaMegtamadas`; the scroll message when the human knows either side).
    War { a: u8, b: u8 },
    /// libc `rand()` drawn for a dialogue text, with its value (spec §9).
    RandDraw { value: i32 },
    /// `Story_Special(0xc, cat, proposer, answerer, sub, …)` after an accepted answer.
    StorySpecial { proposer: u8, answerer: u8, msg: Message },
    /// `Story_Diplomacy_Special(SpecialLeiro[special], to)`: a special (cat 4) reached the AI.
    DiplomacySpecial { special: i32, to: u8 },
    /// [`Diplomacy::tick`] withdrew a message of `from` to human `to` that could no longer be carried out.
    AutoCancelled { from: u8, to: u8 },
    /// A fleet whose attack order was cancelled by a peace (`BirodalomBeketKot`) or a planet deal.
    Starmap(StarmapEvent),
}

/// Mutable world the passes work on.
pub struct World<'a> {
    pub ctx: &'a mut DiplomacyContext,
    /// 1-based (index 0 unused), `HanyBolygo` = len − 1.
    pub planets: &'a mut [DiploPlanet],
    /// Fleets (owner, attack, ground strength, orders), `met` (`BirodalomIsmeri`), colony ship pools.
    pub starmap: &'a mut Starmap,
    pub star_ctx: &'a StarmapContext,
    pub tech: &'a mut TechTable,
    pub rng: &'a mut Streams,
    pub rules: &'a DiplomacyRules,
    /// libc `rand()` (0..=32767): text variants and the counter-proposal coin (spec §9).
    pub libc: &'a mut dyn FnMut() -> i32,
    /// The draws that only pick the local human's text (`EN` receives a message): `None` = from `libc`, like the
    /// exe; the per-peer cosmetic stream in canonical multiplayer (`game::ViewMode`, ADR 0007 §5).
    pub view_libc: Option<&'a mut dyn FnMut() -> i32>,
    pub events: Vec<DiploEvent>,
}

fn sign(x: i32) -> i32 {
    x.signum()
}

impl Diplomacy {
    fn rel(&self, a: usize, b: usize) -> &Relation {
        &self.relations[a][b]
    }
    fn rel_mut(&mut self, a: usize, b: usize) -> &mut Relation {
        &mut self.relations[a][b]
    }
    fn level(&self, a: usize, b: usize) -> u8 {
        self.relations.get(a).and_then(|r| r.get(b)).map_or(0, |r| r.level)
    }
    fn set_level(&mut self, a: usize, b: usize, l: u8) {
        self.relations[a][b].level = l;
        self.relations[b][a].level = l;
    }

    // ------------------------------------------------------------------ FRISSIT_Viszony (spec §3)

    /// `FRISSIT_Viszony`: the daily relation update.
    pub fn daily(&mut self, w: &mut World) {
        let p2 = w.ctx.program_players2.min(EMPIRES - 1);
        for e in 1..=p2 {
            self.power[e] = 1.0;
            self.planets[e] = 0;
            self.space_power[e] = 0.0;
        }
        for pl in w.planets.iter().skip(1) {
            let o = pl.owner as usize;
            if o == 0 || o >= EMPIRES {
                continue;
            }
            let mut e = self.power[o] + (pl.population / 2000) as f32;
            e += (pl.defence + pl.fortification) as f32;
            e += ((pl.ship_capacity + pl.vehicle_capacity) / 200) as f32;
            self.power[o] = e;
            self.planets[o] += 1;
            self.countdown[o] = 100;
        }
        for fl in w.starmap.fleets.iter().skip(1) {
            let o = fl.owner as usize;
            if fl.owner == DELETED || o >= EMPIRES {
                continue;
            }
            self.power[o] += fl.strength as f32;
            self.power[o] += fl.ground_strength as f32;
            self.space_power[o] += fl.strength as f32;
        }
        for e in 1..=p2 {
            if w.ctx.alive[e] && self.planets[e] == 0 && w.ctx.ai[e] {
                self.countdown[e] -= 1;
                if self.countdown[e] < 1 || (self.power[e] == 1.0 && self.countdown[e] < 96) {
                    w.ctx.alive[e] = false;
                    w.events.push(DiploEvent::EmpireDied { empire: e as u8 });
                }
            }
        }
        let pairs: Vec<(usize, usize)> = (1..=p2)
            .flat_map(|a| (1..=p2).map(move |b| (a, b)))
            .filter(|&(a, b)| a != b)
            .collect();
        for &(a, b) in &pairs {
            if w.ctx.alive[a] && w.ctx.alive[b] {
                self.drift(w, a, b, p2);
            }
        }
        for &(a, b) in &pairs {
            if w.ctx.alive[a] && w.ctx.alive[b] {
                self.opinion(w, a, b);
            }
        }
        for e in 1..=p2 {
            if !w.ctx.alive[e] {
                continue;
            }
            self.danger[e] = 1;
            let mut enemies = 0.0f32;
            for c in 1..=p2 {
                if w.ctx.alive[c] && c != e && self.level(e, c) == level::WAR {
                    enemies += self.space_power[c];
                    self.danger[e] = 2;
                }
            }
            if self.danger[e] == 2 && self.space_power[e] < enemies {
                self.danger[e] = 3;
            }
            if self.danger[e] == 3 && self.planets[e] < 3 {
                self.danger[e] = 4;
            }
            if self.planets[e] == 0 {
                self.danger[e] = 5;
            }
        }
    }

    /// Pair pass 1 (spec §3.3).
    fn drift(&mut self, w: &World, a: usize, b: usize, p2: usize) {
        use field::*;
        let ratio = self.power[a] / self.power[b];
        let ln = ((ratio as f64).ln() as f32) as f64;
        let k = 5000 - ((-5000.0f64 / 5.0f64.ln()) * ln) as i32;
        let power = (10000 - k.clamp(0, 10000)) as i16;
        let r = &mut self.relations[a][b];
        r.v[POWER] = power;
        r.v[UNRELIABLE] = r.v[UNRELIABLE].wrapping_sub(5).max(0);
        r.v[SPY_GRIEVANCE] = r.v[SPY_GRIEVANCE].wrapping_sub(5).max(0);
        r.v[MESSAGES] = 0;
        match r.level {
            level::WAR => r.add(TRUST, -8),
            level::NAP => r.add(TRUST, 6),
            level::ALLIANCE => r.add(TRUST, 8),
            _ => {}
        }
        if r.level < 2 && r.v[TRUST] > 5000 {
            r.v[TRUST] = 5000;
        }
        if r.level > 4 && r.v[TRUST] < 5000 {
            r.v[TRUST] = 5000;
        }
        r.add(TRUST, 4 * sign(5000 - r.v[TRUST] as i32));
        r.v[TRUST] = r.v[TRUST].clamp(0, 10000);
        r.add(STORY, sign(5000 - r.v[STORY] as i32));
        r.add(FRIENDLY, 3 * sign(5000 - r.v[FRIENDLY] as i32));
        r.add(RESPECT, 4 * sign(power as i32 - r.v[RESPECT] as i32));
        r.add(FAVOUR, sign(5000 - r.v[FAVOUR] as i32));
        for f in THREATS..=SCHEMING {
            r.v[f] = r.v[f].wrapping_sub(5).max(0);
        }
        let f = ratio.clamp(0.5, 1.5);
        let friendly = r.v[FRIENDLY];
        r.v[ATTITUDE] = if friendly < 5000 { ((friendly as i32 - 5000) as f32 * f + 5000.0) as i32 as i16 } else { friendly };
        if r.v[FAVOUR] > 5000 {
            r.v[ATTITUDE] = ((r.v[FAVOUR] as i32 - 5000) as f64 * 0.3 + r.v[ATTITUDE] as f64) as i32 as i16;
        }

        let (mut s, mut n) = (0.0f32, 0.0f32);
        for pi in w.planets.iter().skip(1).filter(|p| p.owner as usize == a) {
            for pj in w.planets.iter().skip(1).filter(|p| p.owner as usize == b) {
                n += 1.0;
                // `(|Δy| + |Δx|) + s` in the exe; f32 addition commutes, so `+=` is the same.
                s += (pi.y - pj.y).abs() + (pi.x - pj.x).abs();
            }
        }
        let distance = if n == 0.0 { 0 } else { (((s * 10000.0) / n) / 600.0) as i32 as i16 };

        let mut acc = 5000i32;
        for c in 1..=p2 {
            if w.ctx.alive[c] && c != a && c != b {
                let (ra, rb) = (self.rel(a, c), self.rel(b, c));
                acc += (rb.level as i32 - 3) * (ra.level as i32 - 3) * 250
                    + ((rb.v[OPINION] as i32 - 5000) * (ra.v[OPINION] as i32 - 5000)) / 7000;
            }
        }
        let r = &mut self.relations[a][b];
        r.v[DISTANCE] = distance;
        r.v[FRIENDS] = acc.clamp(0, 10000) as i16;
    }

    /// Pair pass 2 (spec §3.4).
    fn opinion(&mut self, w: &mut World, a: usize, b: usize) {
        use field::*;
        const WAR: [i32; 6] = [0, -2000, -1000, 0, 1000, 2000];
        let r = &mut self.relations[a][b];
        let d = |f: usize| r.v[f] as f64;
        let near = (r.v[DISTANCE] as i32 - 5000).min(0);
        let war = WAR.get(r.level as usize).copied().unwrap_or(0);
        let x = 5000.0 - d(UNRELIABLE) * 0.5 - d(SPY_GRIEVANCE)
            + (d(TRUST) - 5000.0) * 0.3
            + (d(STORY) - 5000.0)
            + (d(ATTITUDE) - 5000.0) * 0.5
            + (d(POWER) - 5000.0) * 0.4
            + (d(FRIENDS) - 5000.0) * 0.2
            + near as f64 * 0.5
            + war as f64;
        let mut o = x as i32 as i16;
        match r.level {
            level::WAR => o = o.min(3000),
            level::CEASEFIRE => o = o.min(5000),
            level::NAP => o = o.max(4000),
            level::ALLIANCE => o = o.max(7000),
            _ => {}
        }
        r.v[OPINION] = o;
        let lvl = r.level;
        for (i, v) in r.v.iter_mut().enumerate() {
            if i != FAVOUR {
                *v = (*v).clamp(0, 10000);
            }
        }
        if lvl > 3 {
            for t in 1..=LAST_TECH {
                if w.tech.status(t, b) == crate::colony_side::status::OWNED {
                    w.tech.mark_seen(t, a, b);
                }
            }
        }
    }

    // ------------------------------------------------------------------ AI helpers (spec §6)

    /// `MI_HannyalHarcol(e)`.
    pub fn wars(&self, ctx: &DiplomacyContext, e: usize) -> i32 {
        (1..=ctx.program_players2.min(EMPIRES - 1)).filter(|&c| c != e && self.level(e, c) == level::WAR).count() as i32
    }

    fn bonus(ctx: &DiplomacyContext, m: i32) -> i32 {
        m + if ctx.multiplayer { 1000 } else { 0 }
    }

    /// The `(15 − w)·500` / `(13 − w)·500` sum of the peace and cease-fire rules.
    fn peace_sum(r: &Relation, m: i32) -> i32 {
        use field::*;
        let half = ((r.v[FRIENDS] as i32 - 5000) as f64 * 0.2 - (r.v[UNRELIABLE] as f64 * 0.5 + r.v[SPY_GRIEVANCE] as f64 * 0.5)) * 0.5;
        r.v[POWER] as i32 + (r.v[FRIENDLY] as i32 - 5000) / 2 + half as i32 + m
    }

    /// `SzeretneBeketKotni(x, y, m)`.
    pub fn wants_peace(&self, ctx: &DiplomacyContext, x: usize, y: usize, m: i32) -> bool {
        use field::*;
        let r = self.rel(x, y);
        if r.level > 2 {
            return false;
        }
        let w = self.wars(ctx, x);
        r.v[FRIENDLY] >= 4900
            && (r.v[FRIENDLY] >= 5500 || r.v[POWER] >= 6000 || w > 1)
            && (15 - w) * 500 < Self::peace_sum(r, Self::bonus(ctx, m))
    }

    /// `SzeretneTuzSzunetet(x, y, m)`.
    pub fn wants_ceasefire(&self, ctx: &DiplomacyContext, x: usize, y: usize, m: i32) -> bool {
        use field::*;
        let r = self.rel(x, y);
        if r.level != level::WAR {
            return false;
        }
        let w = self.wars(ctx, x);
        r.v[FRIENDLY] >= 4900
            && (r.v[FRIENDLY] >= 5500 || r.v[POWER] >= 5500 || w > 1)
            && (13 - w) * 500 < Self::peace_sum(r, Self::bonus(ctx, m))
    }

    /// `SzeretneNonAgressionPackot(x, y, m)`.
    pub fn wants_nap(&self, w: &World, x: usize, y: usize, m: i32) -> bool {
        use field::*;
        let r = self.rel(x, y);
        r.level == level::NEUTRAL
            && r.v[FRIENDLY] >= 4900
            && r.v[SPY_GRIEVANCE] < 1000
            && r.v[OPINION] as i32 + Self::bonus(w.ctx, m) > 6000
            && self.target(w, x) != y
    }

    /// `SzeretneSzovetseget(x, y, m)`.
    pub fn wants_alliance(&self, w: &World, x: usize, y: usize, m: i32) -> bool {
        use field::*;
        let ctx = &*w.ctx;
        let pp = ctx.program_players.min(EMPIRES - 1);
        let strongest = |skip: usize| {
            let mut best = 0;
            for c in 1..=pp {
                if ctx.alive[c] && c != y && c != skip && (best == 0 || self.power[best] < self.power[c]) {
                    best = c;
                }
            }
            best
        };
        let s1 = strongest(usize::MAX);
        let s2 = strongest(s1);
        let r = self.rel(x, y);
        let allied = (self.level(y, s1) == level::ALLIANCE) as u8 + (self.level(y, s2) == level::ALLIANCE) as u8 + (x == s1 || x == s2) as u8;
        r.level == level::NAP
            && allied < 3
            && r.v[FRIENDLY] >= 4900
            && r.v[SPY_GRIEVANCE] < 1000
            && r.v[TRUST] > 5720
            && r.v[OPINION] as i32 + Self::bonus(ctx, m) > 7000
            && self.target(w, x) != y
    }

    /// `SzeretneTradeAggreementet(x, y, m)`.
    pub fn wants_trade(&self, w: &World, x: usize, y: usize, m: i32) -> bool {
        use field::*;
        let r = self.rel(x, y);
        r.level > 3
            && r.trade == 0
            && r.v[FRIENDLY] >= 4900
            && r.v[SPY_GRIEVANCE] < 1000
            && (r.v[TRUST] > 5360 || r.v[FAVOUR] > 5100)
            && r.v[OPINION] as i32 + (r.v[FAVOUR] as i32 - 5000) / 2 + Self::bonus(w.ctx, m) > 7000
            && self.target(w, x) != y
    }

    /// `NemAkarBeszolni(x, y)`.
    pub fn too_scared(&self, ctx: &DiplomacyContext, x: usize, y: usize) -> bool {
        let r = self.rel(x, y);
        r.level < 2 && (r.v[field::POWER] >= 7000 || (r.v[field::POWER] >= 6000 && self.wars(ctx, x) > 1))
    }

    /// `MI_KitAkarTamadni(e)`: whom `e` wants to attack (0 = nobody), spec §6.2.
    pub fn target(&self, w: &World, e: usize) -> usize {
        let ctx = &*w.ctx;
        if !ctx.may_attack[e] || self.wars(ctx, e) != 0 {
            return 0;
        }
        let g = if ctx.difficulty > 2 { 1.2f32 } else { 1.0 };
        let fleet = |slot: i32| w.starmap.fleets.get(slot.max(0) as usize).map_or((0, 0), |f| (f.strength, f.ground_strength));
        let mine = fleet(ctx.main_fleet[e]);
        let (mut best, mut pick) = (1e8f32, 0);
        for c in 1..=ctx.program_players.min(EMPIRES - 1) {
            let human = ctx.human[c];
            if c == e
                || !ctx.alive[c]
                || !ctx.may_attack[c]
                || w.starmap.met[e][c] == 0
                || !(ctx.multiplayer || c != ctx.human_empire || !ctx.spare_human[e])
                || !(ctx.difficulty > 1 || !human || self.wars(ctx, c) == 0)
            {
                continue;
            }
            let r = self.rel(e, c);
            let mut score = r.v[field::OPINION] as i32 - r.v[field::UNRELIABLE] as i32 / 3;
            score += match r.level {
                level::CEASEFIRE => -500,
                level::NAP => 3000,
                level::ALLIANCE => 6000,
                _ => 0,
            };
            if human && self.wars(ctx, c) > 0 {
                score += (3 - ctx.difficulty) * 500;
            }
            let reach = w.planets.iter().skip(1).any(|p| {
                p.owner as usize == c
                    && p.visible[e] > 0
                    && (((p.fortification + p.defence) as f32 * g) as i32 <= mine.1 || self.planets[e] == 0)
            });
            if (score as f32) < best && reach && fleet(ctx.main_fleet[c]).0 / 4 < mine.0 {
                best = score as f32;
                pick = c;
            }
        }
        pick
    }

    // ------------------------------------------------------------------ MI_DIPLOMACY (spec §4)

    fn slot(&mut self, e: usize) -> &mut PlayerSlot {
        match self.players.iter().rposition(|s| s.empire == e as i32) {
            Some(i) => &mut self.players[i],
            None => &mut self.ghost,
        }
    }

    /// Human `x` keeps a contact list: the canonical `en`, or another human in canonical multiplayer.
    pub fn keeps(&self, en: usize, x: usize) -> bool {
        x == en || self.others.iter().any(|c| c.empire as usize == x)
    }

    /// Run `f` with `owner`'s contact list as [`Diplomacy::players`] (the original's code for `EN` on `owner`'s
    /// machine; `en` = the canonical `EN`, whose list is `players` itself). `None` if `owner` keeps none.
    pub fn with_list<R>(&mut self, en: usize, owner: usize, f: impl FnOnce(&mut Self) -> R) -> Option<R> {
        if owner == en {
            return Some(f(self));
        }
        let i = self.others.iter().position(|c| c.empire as usize == owner)?;
        let mut c = std::mem::take(&mut self.others[i]);
        self.swap_list(&mut c);
        let r = f(self);
        self.swap_list(&mut c);
        self.others[i] = c;
        Some(r)
    }

    fn swap_list(&mut self, c: &mut ContactList) {
        std::mem::swap(&mut c.players, &mut self.players);
        std::mem::swap(&mut c.ghost, &mut self.ghost);
        std::mem::swap(&mut c.shown, &mut self.shown_players);
    }

    /// Whether `owner`'s list has a slot for `e`.
    fn lists(&mut self, en: usize, owner: usize, e: usize) -> bool {
        self.with_list(en, owner, |d| d.players.iter().any(|s| s.empire == e as i32)).unwrap_or(false)
    }

    /// `FelfedezBirodalom(e)`: the human meets `e` (a contact slot for empires 1..=8; message 0x1A).
    pub fn discover(&mut self, w: &mut World, e: usize) {
        self.discover_as(w, w.ctx.human_empire, e);
    }

    /// `FelfedezBirodalom(e)` on `owner`'s machine: `owner`'s list (canonical multiplayer, `multiplayer.md` §5.1).
    pub fn discover_by(&mut self, w: &mut World, owner: usize, e: usize) {
        self.with_list(w.ctx.human_empire, owner, |d| d.discover_as(w, owner, e));
    }

    fn discover_as(&mut self, w: &mut World, owner: usize, e: usize) {
        if e == 0 || w.ctx.race[e] == 5 || self.players.iter().any(|s| s.empire == e as i32) {
            return;
        }
        if e != owner && e < 9 {
            self.players.push(PlayerSlot { empire: e as i32, controller: w.ctx.player_ki[e] as i32, open: 1, ..Default::default() });
            w.events.push(DiploEvent::Discovered { empire: e as u8, by: owner as u8 });
        }
    }

    /// `MI_DIPLOMACY`: every AI empire may send one message to each empire it knows.
    pub fn ai_messages(&mut self, w: &mut World) {
        if w.ctx.ai_off || !(w.ctx.enabled || w.ctx.multiplayer) {
            return;
        }
        let pp = w.ctx.program_players.min(EMPIRES - 1);
        for a in 1..=pp {
            for b in 1..=pp {
                self.last_contact[a * 9 + b] += 1;
            }
        }
        for a in 1..=pp {
            if w.ctx.human[a] {
                continue;
            }
            for b in 1..=pp {
                self.consider(w, a, b);
            }
        }
    }

    fn rnd(w: &mut World, hi: i32) -> i32 {
        w.rng.range(STREAM, 1, hi)
    }

    fn consider(&mut self, w: &mut World, a: usize, b: usize) {
        use field::*;
        let ctx = &*w.ctx;
        let sp_debug = ctx.debug && !ctx.multiplayer;
        let cell = a * 9 + b;
        if a == b || !ctx.alive[a] || !ctx.alive[b] || a == 5 || b == 5 || w.starmap.met[a][b] == 0 {
            return;
        }
        if !(self.last_contact[cell] > 300 || (self.last_contact[cell] > 30 && sp_debug)) {
            return;
        }
        if !(Self::rnd(w, 100) == 1 || (sp_debug && Self::rnd(w, 10) == 1)) {
            return;
        }
        let ctx = &*w.ctx;
        let waiting = !ctx.multiplayer && b == ctx.human_empire && self.slot(a).waiting & 1 != 0;
        if !(w.ctx.multiplayer || (!waiting && w.ctx.enabled)) {
            return;
        }
        let story = |w: &World| !w.ctx.story_quiet;
        let lvl = |s: &Self| s.level(a, b);
        let mut pick: Option<(i32, i32)> = None;
        let mut third = 0;
        if story(w) {
            if lvl(self) < 3 && self.wants_peace(w.ctx, a, b, -1000) && Self::rnd(w, 100) < 51 && Self::rnd(w, 2) == 1 {
                pick = Some((cat::NEGOTIATE, negotiate::PEACE));
            }
            if lvl(self) == level::WAR
                && self.wants_ceasefire(w.ctx, a, b, -1000)
                && Self::rnd(w, 100) < 34
                && Self::rnd(w, 2) == 1
            {
                pick = Some((cat::NEGOTIATE, negotiate::CEASEFIRE));
            }
            if lvl(self) == level::NEUTRAL && self.wants_nap(w, a, b, -1000) && Self::rnd(w, 100) < 34 {
                pick = Some(if Self::rnd(w, 2) == 1 { (cat::NEGOTIATE, negotiate::NAP) } else { (cat::TALK, talk::IMPROVE) });
            }
            if lvl(self) == level::NAP && self.wants_alliance(w, a, b, -1000) && Self::rnd(w, 100) < 34 {
                if Self::rnd(w, 3) == 1 {
                    pick = Some((cat::NEGOTIATE, negotiate::ALLIANCE));
                } else if Self::rnd(w, 2) == 1 {
                    pick = Some((cat::TALK, talk::IMPROVE));
                }
            }
            if lvl(self) > 3
                && self.rel(a, b).trade == 0
                && self.wants_trade(w, a, b, -600)
                && Self::rnd(w, 100) < 34
            {
                pick = Some((cat::NEGOTIATE, negotiate::TRADE));
            }
        }
        if lvl(self) == level::ALLIANCE && Self::rnd(w, 100) < 6 {
            pick = Some((cat::TALK, talk::GREETING));
        }
        let mut bad = false;
        if Self::rnd(w, 100) < 51
            && (self.target(w, a) == b
                || (Self::rnd(w, 100) < 51 && lvl(self) == level::WAR && !self.wants_ceasefire(w.ctx, a, b, -1000)))
            && self.known_since[a][b] < 500
            && !self.too_scared(w.ctx, a, b)
        {
            if Self::rnd(w, 100) < 31 && lvl(self) < 4 {
                pick = Some((cat::TALK, if Self::rnd(w, 2) == 1 { talk::THREAT } else { talk::INSULT }));
            }
            bad = Self::rnd(w, 100) < 51;
        }
        if Self::rnd(w, 100) < 51 && self.rel(a, b).v[SPY_GRIEVANCE] > 1000 && lvl(self) > 1 {
            pick = Some((cat::TALK, talk::SPIES));
            if Self::rnd(w, 100) < 51 && lvl(self) > 2 {
                bad = true;
            }
        }
        let mut skip_rest = !story(w);
        if story(w) && bad && lvl(self) > 2 {
            match lvl(self) {
                level::ALLIANCE => pick = Some((cat::NEGOTIATE, negotiate::BREAK_ALLIANCE)),
                level::NAP => pick = Some((cat::NEGOTIATE, negotiate::BREAK_NAP)),
                _ => {}
            }
            if Self::rnd(w, 100) < 51
                && (lvl(self) == level::NEUTRAL || w.ctx.difficulty > 2)
                && self.target(w, a) == b
                && self.known_since[a][b] < 0
            {
                self.declare_war(w, a, b);
                pick = None;
            }
            skip_rest = !story(w);
        }
        if !skip_rest && Self::rnd(w, 100) < 26 && lvl(self) > 2 {
            let t = self.target(w, a);
            if t > 0 && t != b && self.rel(b, t).v[OPINION] < 4500 && self.level(b, t) == level::NEUTRAL {
                pick = Some(if Self::rnd(w, 2) == 1 { (cat::NEGOTIATE, negotiate::WAR_TOGETHER) } else { (cat::TALK, talk::SCHEMING) });
                third = t as i32;
            }
        }
        Self::rnd(w, 100);
        if let Some((c, s)) = pick {
            self.last_contact[cell] = 0;
            self.send(w, a, b, false, Message::ai(c, s, third));
        }
    }

    /// `ElkuldHaduzenet(a, b)`.
    fn declare_war(&mut self, w: &mut World, a: usize, b: usize) {
        if w.ctx.race[a] == 5 || w.ctx.race[b] == 5 || (!w.ctx.enabled && !w.ctx.multiplayer) {
            self.war(w, a, b, true, 0);
        } else {
            self.send(w, a, b, true, Message::ai(cat::NEGOTIATE, negotiate::DECLARE_WAR, -1));
        }
    }

    // ------------------------------------------------------------------ messages (spec §5–7)

    /// `ElkuldUzenetMI`: the AI sends (race 5 on either side: dropped).
    fn send(&mut self, w: &mut World, a: usize, b: usize, declaration: bool, msg: Message) {
        if w.ctx.race[a] == 5 || w.ctx.race[b] == 5 {
            return;
        }
        self.receive_message(w, &mut (), Letter { from: a as i32, to: b as i32, declaration, msg });
    }

    /// `DiplomaciaTalkAbout(a, b, sub, third)` (exe 0x4bdac0): `a` tells `b` a talk line as a declaration
    /// (`ElkuldUzenetMI(a, b, 1, TALK, sub, third, …)`; the `MiertValaszoltAztAmitValaszolt` reason only picks a text).
    pub fn talk_about(&mut self, w: &mut World, a: usize, b: usize, sub: i32, third: i32) {
        self.send(w, a, b, true, Message::ai(cat::TALK, sub, third));
    }

    fn draw_text(w: &mut World) {
        let value = match w.view_libc.as_mut() {
            Some(draw) => draw(),
            None => (w.libc)(),
        };
        w.events.push(DiploEvent::RandDraw { value });
    }

    /// `FeldolgozAjanlat` (spec §5): a message arrives, from the AI or out of the human's queue
    /// (`MULTIGET_Diplomacy`, spec §10.2). An AI receiver answers at once.
    pub fn receive_message(&mut self, w: &mut World, deals: &mut dyn Deals, letter: Letter) {
        let (a, b, msg) = (letter.from.max(0) as usize, letter.to.max(0) as usize, letter.msg);
        let en = w.ctx.human_empire;
        if self.keeps(en, b) && !self.lists(en, b, a) {
            self.discover_by(w, b, a);
        }
        w.starmap.met[a][b] = 1;
        w.starmap.met[b][a] = 1;
        self.rel_mut(b, a).add(field::MESSAGES, 1000);
        if !w.ctx.tutorial && msg.cat == cat::NEGOTIATE {
            self.immediate(w, a, b, msg.sub);
        }
        self.react(w, a, b, msg);
        let mut accepted = None;
        if a != b && self.keeps(en, b) {
            self.with_list(en, b, |d| {
                if d.slot(a).open != 0 || letter.declaration {
                    Self::draw_text(w);
                    let s = d.slot(a);
                    s.waiting = 1;
                    s.letter = letter;
                    s.cancelled = 0;
                }
            });
        }
        if !w.ctx.human[b] {
            let yes = match w.ctx.forced_answer {
                0 => self.decide(w, deals, a, b, msg),
                f => f != 1,
            };
            accepted = Some(yes);
            w.events.push(DiploEvent::Message { from: a as u8, to: b as u8, msg, accepted });
            let answer = Answer { from: b as i32, to: a as i32, msg, counter: false, reply: false, yes };
            self.receive_answer(w, deals, answer);
        } else {
            w.events.push(DiploEvent::Message { from: a as u8, to: b as u8, msg, accepted });
        }
    }

    /// `VegrehajtAzonnalVegrehajtodoDolgokat`, negotiate messages.
    fn immediate(&mut self, w: &mut World, a: usize, b: usize, sub: i32) {
        match sub {
            negotiate::DECLARE_WAR => self.war(w, a, b, true, 0),
            negotiate::BREAK_NAP => {
                self.rel_mut(b, a).add(field::UNRELIABLE, 1000);
                self.make_peace(w, b, a);
                self.set_level(a, b, level::NEUTRAL);
                self.rel_mut(b, a).trade = 0;
                self.rel_mut(a, b).trade = 0;
            }
            negotiate::BREAK_ALLIANCE => {
                self.make_peace(w, b, a);
                self.rel_mut(a, b).add(field::UNRELIABLE, 2000);
                self.set_level(a, b, level::NEUTRAL);
            }
            _ => {}
        }
    }

    /// `AjanlatHatastLekezel`: the receiver's reaction (stream 9 for talk messages).
    fn react(&mut self, w: &mut World, a: usize, b: usize, msg: Message) {
        use field::*;
        if msg.cat == cat::NEGOTIATE {
            let q = self.rel_mut(b, a);
            match msg.sub {
                negotiate::DECLARE_WAR => {
                    if q.v[FAVOUR] < 5000 {
                        q.add(UNRELIABLE, 5000 - q.v[FAVOUR] as i32);
                    }
                    if q.v[FRIENDLY] > 5000 {
                        q.add(UNRELIABLE, 1000);
                    }
                    q.add(UNRELIABLE, 2000);
                    q.add(FRIENDLY, -3000);
                }
                negotiate::BREAK_NAP | negotiate::BREAK_ALLIANCE => {
                    q.add(FRIENDLY, -3000);
                    q.add(UNRELIABLE, 3000);
                }
                _ => {}
            }
            return;
        }
        if msg.cat != cat::TALK {
            return;
        }
        let weak = (self.power[b] as f64) < self.power[a] as f64 * 0.7;
        let stronger = (self.power[b] as f64) <= self.power[a] as f64 * 1.4;
        let chance = |x: f64| (x as i32).clamp(20, 80);
        match msg.sub {
            talk::URGE_PEACE | talk::IMPROVE | talk::URGE_ALLIANCE => {
                let q = *self.rel(b, a);
                let p = chance((((q.v[THREATS] as f64 - (q.v[POWER] as i32 - 5000) as f64 / 2.0) - 1000.0) * 60.0) / 2000.0 + 20.0);
                let x = Self::rnd(w, 100);
                let q = self.rel_mut(b, a);
                if p < x {
                    q.add(THREATS, 1000);
                    if q.v[THREATS] < 2001 {
                        q.add(FRIENDLY, 1000);
                    }
                    q.add(RESPECT, -1000);
                } else {
                    q.add(FRIENDLY, -(q.v[THREATS] as i32 / 2));
                    q.add(THREATS, -500);
                }
                if q.v[INSULTS] > 0 {
                    q.add(UNRELIABLE, 500);
                }
            }
            talk::THREAT | talk::SPIES | talk::INSULT => {
                let q = self.rel_mut(b, a);
                q.add(INSULTS, 1000);
                if q.v[RESPECT] < 5000 {
                    q.v[RESPECT] = 5000;
                }
                let mut case = stronger as i32;
                if weak {
                    case = 2;
                }
                if msg.sub == talk::SPIES {
                    let s = self.rel(a, b).v[SPY_GRIEVANCE];
                    if s < 100 {
                        case -= 1;
                    }
                    if s > 1500 {
                        case += 1;
                    }
                    case = case.clamp(0, 2);
                    self.rel_mut(a, b).v[SPY_GRIEVANCE] = s.wrapping_sub(500).max(0);
                }
                let m = if msg.sub == talk::INSULT { 1.5f32 } else { 1.0 };
                if case == 1 || case == 2 {
                    let q = *self.rel(b, a);
                    let p = chance((((q.v[INSULTS] as f64 - (q.v[POWER] as i32 - 5000) as f64 / 1.5) - 1000.0) * 60.0) / 2000.0 + 20.0);
                    if Self::rnd(w, 100) <= p {
                        let q = self.rel_mut(b, a);
                        q.add(INSULTS, -1000);
                        q.v[RESPECT] = 5000;
                        case = 3;
                    }
                }
                let q = self.rel_mut(b, a);
                match case {
                    0 => q.v[FRIENDLY] = (q.v[FRIENDLY] as f32 - m * 1500.0) as i32 as i16,
                    1 => {
                        q.add(FRIENDLY, -500);
                        q.add(RESPECT, 300);
                    }
                    2 => q.v[RESPECT] = (m * 1000.0 + q.v[RESPECT] as f32) as i32 as i16,
                    _ => q.add(FRIENDLY, -1000),
                }
                if q.v[FAVOUR] < 4900 {
                    q.add(UNRELIABLE, 2000);
                }
                if q.v[THREATS] > 0 {
                    q.add(UNRELIABLE, 500);
                }
                if (msg.sub == talk::THREAT || msg.sub == talk::INSULT) && q.v[FAVOUR] > 5000 {
                    q.v[FAVOUR] = 5000;
                }
            }
            talk::SCHEMING => {
                let t = msg.value.max(0) as usize;
                let q = *self.rel(b, a);
                let d = ((self.rel(b, t).v[OPINION] as i32 - q.v[OPINION] as i32) / 3).clamp(-2000, 2000);
                let p = chance(((q.v[SCHEMING] as i32 + d) as f64 * 40.0) / 2000.0 + 50.0);
                if p < Self::rnd(w, 100) {
                    self.rel_mut(b, a).add(SCHEMING, 1000);
                    self.rel_mut(b, t).add(FRIENDLY, -2000);
                } else {
                    let s = q.v[SCHEMING] as i32;
                    self.rel_mut(b, a).add(FRIENDLY, -2000 - s / 2);
                    self.rel_mut(b, t).add(FRIENDLY, s + 500);
                    self.rel_mut(t, b).add(FRIENDLY, 500);
                }
            }
            talk::GREETING => {
                let q = self.rel_mut(b, a);
                q.add(GREETINGS, 1000);
                if q.v[GREETINGS] < 500 {
                    q.add(FRIENDLY, 500);
                }
                q.add(RESPECT, -500);
            }
            _ => {}
        }
    }

    /// `EldontIgenNemUzenet` (spec §6, §6.4): does the AI `b` accept `a`'s message? Deals that pass
    /// change the favour balance here already (cat 2 and 3).
    fn decide(&mut self, w: &mut World, deals: &dyn Deals, a: usize, b: usize, msg: Message) -> bool {
        use field::*;
        if w.ctx.tutorial {
            return true;
        }
        match msg.cat {
            cat::NEGOTIATE if matches!(msg.sub, negotiate::DECLARE_WAR | negotiate::BREAK_NAP | negotiate::BREAK_ALLIANCE) => return true,
            cat::TALK => return matches!(msg.sub, 0 | 1 | 2 | 3 | 4 | 6 | 8 | 9),
            cat::SPECIAL => {
                w.events.push(DiploEvent::DiplomacySpecial { special: msg.sub, to: b as u8 });
                return false;
            }
            cat::DEMAND if msg.sub == item::TECH && w.tech.status(msg.value.max(0) as usize, b) != status::OWNED => return false,
            cat::OFFER_FOR if msg.sub2 == item::TECH && w.tech.status(msg.value2.max(0) as usize, b) != status::OWNED => return false,
            _ => {}
        }
        let bonus = if w.ctx.race[a] == 7 { 1000 } else { 0 };
        let lvl = self.level(b, a);
        let (mut give, mut want) = (0, 0);
        if matches!(msg.cat, cat::OFFER_FOR | cat::DEMAND | cat::OFFER) {
            give = self.deal_value(w, deals, a, msg.sub, msg.value, msg.cat == cat::DEMAND, msg.cat == cat::OFFER_FOR, lvl);
            if msg.cat == cat::OFFER_FOR {
                want = self.deal_value(w, deals, b, msg.sub2, msg.value2, true, true, lvl);
                if msg.sub == item::RADAR && msg.sub2 != item::RADAR {
                    return false;
                }
            }
        }
        let wars = self.wars(w.ctx, b);
        let level_of = |s: &Self, x: i32, y: i32| s.level(x.max(0) as usize, y.max(0) as usize);
        match msg.cat {
            cat::NEGOTIATE => match msg.sub {
                negotiate::CEASEFIRE => self.wants_ceasefire(w.ctx, b, a, bonus),
                negotiate::PEACE => self.wants_peace(w.ctx, b, a, bonus),
                // Alliance offers are judged by the NAP rule (spec §6).
                negotiate::NAP | negotiate::ALLIANCE => self.wants_nap(w, b, a, bonus),
                negotiate::WAR_TOGETHER => wars == 0 && self.target(w, b) as i32 == msg.value,
                negotiate::TRADE => self.wants_trade(w, b, a, bonus),
                _ => false,
            },
            cat::OFFER => {
                if (msg.sub == item::PLANET && !deals.planet_terms(msg.value.max(0) as usize).capturable)
                    // sic: both binaries index the tech table swapped here (tech `b`, empire `value`).
                    || (msg.sub == item::TECH && w.tech.status(b, msg.value.max(0) as usize) == status::OWNED)
                {
                    return false;
                }
                self.rel_mut(b, a).add(FAVOUR, give / 10);
                self.rel_mut(a, b).add(FAVOUR, -(give / 10));
                true
            }
            cat::DEMAND => {
                let q = *self.rel(b, a);
                let mut t = (q.v[FAVOUR] as i32 - 5000) * 5;
                if q.v[FRIENDLY] > 5000 {
                    t += 5000;
                }
                if q.level > 3 {
                    t += 5000;
                }
                if q.level > 4 {
                    t += if msg.sub == item::TECH { 30000 } else { 10000 };
                }
                if self.planets[b] < 6 && msg.sub == item::MONEY {
                    t /= 2;
                }
                t = t.wrapping_add((bonus - 5000 + (q.v[RESPECT] as i32).max(5000) - q.v[UNRELIABLE] as i32).wrapping_mul(10));
                if q.level < 3
                    || q.v[FAVOUR] < 4500
                    || (q.v[FAVOUR] < 5501 && q.level < 4 && q.v[FRIENDLY] < 5001 && q.v[RESPECT] < 6000)
                {
                    t = 0;
                }
                if w.ctx.difficulty == 4 || q.v[FAVOUR] < 4500 {
                    return false;
                }
                match msg.sub {
                    item::MONEY | item::TECH | item::HOSTAGE => {
                        if give > t {
                            return false;
                        }
                        let tenth = (give / 10) as i16;
                        let r = self.rel_mut(b, a);
                        r.v[FAVOUR] = r.v[FAVOUR].wrapping_sub(tenth).wrapping_sub(500);
                        r.add(RESPECT, -2000);
                        self.rel_mut(a, b).add(FAVOUR, tenth as i32 + 500);
                        true
                    }
                    item::RADAR => msg.value != b as i32 && (q.level > 3 || level_of(self, b as i32, msg.value) <= 2),
                    _ => false,
                }
            }
            cat::OFFER_FOR => {
                let q = *self.rel(b, a);
                let mut s: i32 = if q.v[FRIENDLY] > 5500 { 10000 } else { 5000 };
                if q.v[FRIENDLY] < 4500 {
                    s -= 10000;
                }
                s += match q.level {
                    1 => -30000,
                    2 => -20000,
                    4 => 10000,
                    5 if msg.sub == item::TECH => 40000,
                    5 => 20000,
                    _ => 0,
                };
                if q.v[FAVOUR] < 4500 {
                    s -= 10000;
                }
                if msg.sub == item::MONEY && msg.sub2 == item::MONEY {
                    if want < give {
                        return true;
                    }
                    s -= 1_000_000;
                }
                let planets_a = self.planets[a];
                match msg.sub {
                    item::RADAR if b as i32 == msg.value => return false,
                    item::WAR => {
                        let r = self.relations.get(b).and_then(|row| row.get(msg.value.max(0) as usize)).copied().unwrap_or_default();
                        if level_of(self, a as i32, msg.value) == level::WAR || q.v[UNRELIABLE] > 1000 || r.v[OPINION] != 1 {
                            return false;
                        }
                        give = (r.v[POWER] as i32 - 4000).wrapping_mul(10000);
                        if wars > 1 {
                            give = give.wrapping_add((wars - 1) * 20000);
                        }
                        if planets_a > 10 {
                            give = give.wrapping_mul(2);
                        }
                    }
                    item::PEACE => {
                        if !self.wants_peace(w.ctx, a, b, 0) || q.v[POWER] < 6000 || q.level != level::WAR || q.v[UNRELIABLE] > 1000 {
                            return false;
                        }
                        give = (q.v[POWER] as i32 - 5000).wrapping_mul(10000);
                        if wars > 1 {
                            give = give.wrapping_add((wars - 1) * 20000);
                        }
                        if planets_a > 10 {
                            want = want.wrapping_mul(2); // sic: the original doubles the other side
                        }
                    }
                    _ => {}
                }
                match msg.sub2 {
                    item::RADAR if b as i32 == msg.value2 => return false,
                    item::WAR => {
                        self.target(w, b); // evaluated, result unused
                        let r = self.relations.get(b).and_then(|row| row.get(msg.value2.max(0) as usize)).copied().unwrap_or_default();
                        if q.v[UNRELIABLE] > 1000 || wars > 0 || r.v[POWER] > 5500 || r.level > 3 || r.level == level::WAR {
                            return false;
                        }
                        want = (r.v[POWER] as i32 + 2).wrapping_mul(if planets_a > 5 { 20000 } else { 10000 });
                    }
                    item::PEACE => {
                        self.wants_peace(w.ctx, a, b, 0); // evaluated, result unused
                        if q.level != level::WAR || q.v[UNRELIABLE] > 1000 {
                            return false;
                        }
                        want = (7000 - q.v[POWER] as i32).wrapping_mul(if planets_a > 5 { 40000 } else { 20000 });
                        if msg.sub == item::PLANET {
                            want = want.wrapping_mul(planets_a);
                        }
                    }
                    _ => {}
                }
                want < s.wrapping_add(bonus).wrapping_add(give)
            }
            _ => false,
        }
    }

    /// `DiplomaciaErtek(e, kind, value, demand, offer_for, level)` (exe 0x4bd3a0), spec §6.3: what a deal
    /// item is worth.
    #[allow(clippy::too_many_arguments)]
    pub fn deal_value(&self, w: &World, deals: &dyn Deals, e: usize, kind: i32, value: i32, demand: bool, offer_for: bool, lvl: u8) -> i32 {
        match kind {
            item::MONEY => {
                if deals.money(e) < value as f64 && offer_for && !demand {
                    0
                } else {
                    value
                }
            }
            item::RADAR => {
                if e as i32 == value {
                    0
                } else {
                    8000
                }
            }
            item::PLANET => {
                let t = deals.planet_terms(value.max(0) as usize);
                let n = self.planets[e];
                let mut x = t.tax.wrapping_mul(24).min(100_000);
                if demand {
                    if n < 4 {
                        x = 10_000_000;
                    }
                    if n < 5 {
                        x = x.wrapping_mul(2);
                    }
                    if n < 6 {
                        x = x.wrapping_mul(2);
                    }
                }
                let x = (x as f64 * if demand { 1.2 } else { 0.6 }) as i32;
                let race = if t.owner != 0 { t.race as usize } else { e };
                let aff = w.rules.surface_affinity.get(race * 9 + t.surface as usize).copied().unwrap_or(0);
                let s = ((t.richness - 100) as f64 / 100.0) as f32 as f64 + (aff - 100) as f64 / 100.0;
                let x = (((s as f32) / 3.0 + 1.0) * x as f32) as i32;
                if !t.capturable {
                    return if demand { 10_000_000 } else { 0 };
                }
                if demand {
                    return x;
                }
                let b = &t.buildings;
                if t.needed_morale < 300 || b[1] == 0 || b[2] + b[3] == 0 || b[4] + b[5] == 0 || (offer_for && lvl == level::WAR) {
                    0
                } else {
                    x
                }
            }
            item::TECH => {
                let t = value.max(0) as usize;
                let race = w.ctx.race[e] as usize;
                let money = w.rules.research_money.get(race).copied().unwrap_or(0.0);
                let time = w.rules.research_time.get(race).copied().unwrap_or(0.0);
                let x = ((w.tech.research_cost(t) as f32 * money * w.tech.research_time(t, e) as f32)
                    / (w.tech.base_research_time(t) as f32 * time + 1.0)) as i32;
                match w.tech.status(t, e) {
                    status::UNKNOWN => (x as f64 * 1.3) as i32,
                    status::OWNED => 0,
                    _ => x,
                }
            }
            item::HOSTAGE => match value {
                1 => 50000,
                2 => 25000,
                3 => 0,
                4 | 5 => 1000,
                _ => kind,
            },
            item::WAR => match self.level(e, value.max(0) as usize) {
                level::WAR | level::CEASEFIRE => 30000,
                level::NEUTRAL => 10000,
                level::NAP | level::ALLIANCE => 0,
                _ => kind,
            },
            _ => kind,
        }
    }

    /// `FeldolgozValasz` (spec §7): an answer reaches the proposer `answer.to`, from the AI or out of the
    /// human's queue. An accepted deal is carried out.
    pub fn receive_answer(&mut self, w: &mut World, deals: &mut dyn Deals, answer: Answer) {
        let text = (w.libc)();
        w.events.push(DiploEvent::RandDraw { value: text });
        let (b, a) = (answer.from.max(0) as usize, answer.to.max(0) as usize);
        let en = w.ctx.human_empire;
        if answer.counter {
            if w.ctx.human[a] {
                self.with_list(en, a, |d| {
                    let s = d.slot(b);
                    s.countered = 1;
                    s.screen = 0;
                    s.counter_answer = answer;
                });
                return;
            }
            // `EldontIgenNemValasz`: a coin flip (libc `rand() % 2`).
            let mut yes = match w.ctx.forced_answer {
                0 => (w.libc)() % 2 != 0,
                f => f != 1,
            };
            if answer.msg.cat != cat::OFFER_FOR {
                return;
            }
            if answer.msg.sub == item::TECH && w.tech.status(answer.msg.value.max(0) as usize, a) != status::OWNED {
                yes = false;
            }
            let reply = Answer { from: a as i32, to: b as i32, msg: answer.msg, counter: false, reply: true, yes };
            return self.receive_answer(w, deals, reply);
        }
        self.with_list(en, a, |d| {
            let s = d.slot(b);
            s.answered = 1;
            s.asked = 0;
            s.screen = 0;
            s.answer = answer;
        });
        // The tutorial's two answers from empire 6 (exe 0x4c16ff; `yes` not tested): 0x2a for (1, 3, 0x12, 3, 0xf),
        // 0xd for (6, 2, ..).
        let m = answer.msg;
        if answer.from == 6 {
            if (m.cat, m.sub, m.value, m.sub2, m.value2) == (1, 3, 0x12, 3, 0xf) {
                w.events.push(DiploEvent::Tutorial(0x2a));
            } else if (m.cat, m.sub) == (6, 2) {
                w.events.push(DiploEvent::Tutorial(0xd));
            }
        }
        w.events.push(DiploEvent::Answered(answer));
        if w.ctx.tutorial || !answer.yes {
            return;
        }
        let msg = answer.msg;
        let third = msg.value.max(0) as usize;
        match msg.cat {
            cat::NEGOTIATE => match msg.sub {
                negotiate::CEASEFIRE => {
                    self.make_peace(w, a, b);
                    self.set_level(a, b, level::CEASEFIRE);
                }
                negotiate::WAR_TOGETHER => {
                    self.war(w, a, third, true, b);
                    self.war(w, b, third, true, a);
                }
                negotiate::PEACE => self.make_peace(w, a, b),
                negotiate::NAP => {
                    self.make_peace(w, a, b);
                    self.set_level(a, b, level::NAP);
                }
                negotiate::ALLIANCE => {
                    self.make_peace(w, a, b);
                    self.set_level(a, b, level::ALLIANCE);
                }
                negotiate::TRADE => {
                    self.rel_mut(a, b).trade = 1;
                    self.rel_mut(b, a).trade = 1;
                }
                _ => {}
            },
            cat::OFFER_FOR => {
                self.hand_over(w, deals, a, b, b, msg.sub, msg.value, false);
                self.hand_over(w, deals, b, a, b, msg.sub2, msg.value2, true);
            }
            cat::DEMAND => self.hand_over(w, deals, b, a, b, msg.sub, msg.value, true),
            cat::OFFER => self.hand_over(w, deals, a, b, b, msg.sub, msg.value, false),
            _ => {}
        }
        w.events.push(DiploEvent::StorySpecial { proposer: a as u8, answerer: b as u8, msg });
    }

    /// `x` gives a deal item to `y` (spec §7); `answerer` makes peace for a peace item, `payer_first` orders
    /// the two money bookings.
    #[allow(clippy::too_many_arguments)]
    fn hand_over(&mut self, w: &mut World, deals: &mut dyn Deals, x: usize, y: usize, answerer: usize, kind: i32, value: i32, payer_first: bool) {
        let v = value.max(0) as usize;
        match kind {
            item::MONEY => {
                if payer_first {
                    deals.add_money(x, -(value as f64));
                    deals.add_money(y, value as f64);
                } else {
                    deals.add_money(y, value as f64);
                    deals.add_money(x, -(value as f64));
                }
            }
            item::PLANET => {
                for (who, e) in deals.transfer_planet(w, v, y as u8) {
                    if self.keeps(w.ctx.human_empire, who as usize) {
                        self.discover_by(w, who as usize, e as usize);
                    }
                }
            }
            item::TECH => {
                deals.develop(w.tech, y, v);
                w.tech.mark_seen(v, x, y);
                w.tech.recompute(w.ctx.program_players2 as i32, w.ctx.all_techs);
            }
            item::PEACE => self.make_peace(w, v, answerer),
            item::WAR => self.war(w, x, v, true, 0),
            item::HOSTAGE => w.ctx.release(y),
            item::RADAR => {
                // Planets 1..HanyBolygo − 1: the original's loop skips the last planet.
                let last = w.planets.len().saturating_sub(1);
                for p in 1..last {
                    let pl = &mut w.planets[p];
                    if pl.owner as i32 == value && pl.visible.get(x).is_some_and(|&s| s > 0) && pl.visible.get(y) == Some(&0) {
                        pl.visible[y] = 1;
                        deals.reveal(p, y);
                    }
                }
            }
            _ => {}
        }
    }

    /// `FRISSIT_Diplomacy` (exe 0x4c0f90), every sim tick (spec §10.3): rulers' birthdays and the
    /// auto-cancel of messages to the human that can no longer be carried out.
    pub fn tick(&mut self, w: &mut World) {
        if self.last_day != w.ctx.day {
            self.last_day = w.ctx.day;
            if w.ctx.month == 1 && w.ctx.day == 1 {
                for e in 1..EMPIRES {
                    self.ruler_age[e] += 1;
                    self.greeted[e] = 0;
                }
            }
        }
        if !self.started {
            return;
        }
        let en = w.ctx.human_empire;
        self.auto_cancel(w, en);
        for owner in self.others.iter().map(|c| c.empire as usize).collect::<Vec<_>>() {
            self.with_list(en, owner, |d| d.auto_cancel(w, owner));
        }
    }

    /// The diplomacy screen of human `owner` writes `op` to its slot of `empire` at time `ido` (`en` = the canonical
    /// `EN`; an empire without a slot writes the spare slot −1, like the original).
    pub fn screen(&mut self, en: usize, owner: usize, empire: usize, op: ScreenOp, ido: i32) {
        if op == ScreenOp::Greeted {
            if let Some(g) = self.greeted.get_mut(empire) {
                *g = 1;
            }
            return;
        }
        self.with_list(en, owner, |d| {
            let s = d.slot(empire);
            match op {
                ScreenOp::Sent => {
                    s.screen = 1;
                    s.asked = 1;
                    s.sent_at = ido;
                }
                ScreenOp::Answered => s.waiting = 0,
                ScreenOp::AnswerSeen => s.answered = 0,
                ScreenOp::CounterSeen => s.countered = 0,
                ScreenOp::Cancelled => {
                    s.asked = 0;
                    s.screen = 0;
                }
                ScreenOp::Greeted => {}
            }
        });
        if op == ScreenOp::Cancelled && empire != owner {
            self.with_list(en, empire, |d| d.slot(owner).waiting = 0);
        }
    }

    /// The auto-cancel of `list_owner`'s list (`players`; spec §10.3).
    fn auto_cancel(&mut self, w: &mut World, list_owner: usize) {
        self.shown_players = self.players.len() as i32;
        for i in 0..self.players.len() {
            let s = self.players[i];
            if s.cancelled != 0 || s.waiting == 0 {
                continue;
            }
            let (f, t, m) = (s.letter.from, s.letter.to, s.letter.msg);
            let lvl = |x: i32, y: i32| self.level(x.max(0) as usize, y.max(0) as usize);
            let owner = |p: i32| w.planets.get(p.max(0) as usize).map_or(0, |pl| pl.owner as i32);
            let freed = |e: i32| w.ctx.kidnapped(e.max(0) as usize) == 0;
            // Can `giver` still hand the item (kind, value) to `taker`?
            let gone = |kind: i32, value: i32, giver: i32, taker: i32| {
                (kind == item::HOSTAGE && freed(taker))
                    || (kind == item::PLANET && owner(value) != giver)
                    || (kind == item::WAR && lvl(value, giver) == level::WAR)
            };
            let stale = match m.cat {
                cat::NEGOTIATE => m.sub == negotiate::ALLIANCE && lvl(f, t) == level::ALLIANCE,
                cat::OFFER_FOR => gone(m.sub, m.value, f, t) || gone(m.sub2, m.value2, t, f),
                cat::DEMAND => gone(m.sub, m.value, t, f),
                cat::OFFER => gone(m.sub, m.value, f, t),
                _ => false,
            };
            if stale {
                let s = &mut self.players[i];
                s.cancelled = 1;
                s.waiting = 0;
                w.events.push(DiploEvent::AutoCancelled { from: f.max(0) as u8, to: list_owner as u8 });
            }
        }
    }

    // ------------------------------------------------------------------ shared effects (spec §8)

    /// `DiplomaciaMegtamadas(a, b, declared, partner)`.
    pub fn war(&mut self, w: &mut World, a: usize, b: usize, declared: bool, partner: usize) {
        use field::*;
        if a == 9 || a == 10 || b == 9 || b == 10 || a == b || self.level(a, b) == level::WAR {
            return;
        }
        w.starmap.met[a][b] = 1;
        w.starmap.met[b][a] = 1;
        let row = match self.level(a, b) {
            level::ALLIANCE => 0,
            level::NAP => 1,
            level::NEUTRAL => 2,
            _ => 3,
        };
        let table = w.rules.declare_war[!declared as usize][row];
        let q = self.rel_mut(b, a);
        if q.v[FRIENDLY] > 1999 {
            q.v[FRIENDLY] = 2000;
        }
        for k in 1..EMPIRES {
            let lk = self.level(b, k);
            if lk == level::WAR || lk == level::CEASEFIRE || w.starmap.met[k][a] == 0 || k == a || k == partner {
                continue;
            }
            let r = self.rel_mut(k, a);
            r.add(UNRELIABLE, table[if k == b { 0 } else { 1 }]);
            for (l, d) in [(level::ALLIANCE, -5000), (level::NAP, -2000)] {
                if lk == l {
                    r.add(FRIENDLY, d);
                    if r.v[ATTITUDE] < 0 {
                        r.v[ATTITUDE] = 0;
                    }
                }
            }
        }
        self.set_level(a, b, level::WAR);
        self.rel_mut(a, b).trade = 0;
        self.rel_mut(b, a).trade = 0;
        w.events.push(DiploEvent::War { a: a as u8, b: b as u8 });
        let en = w.ctx.human_empire;
        for (x, y) in [(a, b), (b, a)] {
            if self.keeps(en, x) && !self.lists(en, x, y) {
                self.discover_by(w, x, y);
            }
        }
    }

    /// `BirodalomBeketKot(a, b)`: neutral, attack orders between them stop.
    pub fn make_peace(&mut self, w: &mut World, a: usize, b: usize) {
        self.set_level(a, b, level::NEUTRAL);
        // The four tests of the original, in order, on every fleet slot: a's fleets attacking b's planets,
        // b's attacking a's, then the same against fleets.
        let tests = [(a, b, order::PLANET), (b, a, order::PLANET), (a, b, order::FLEET), (b, a, order::FLEET)];
        let mut f = 1;
        while f < w.starmap.fleets.len() {
            for (own, other, kind) in tests {
                let fl = &w.starmap.fleets[f];
                if fl.owner as usize != own || fl.order != order::ATTACK || fl.kind != kind {
                    continue;
                }
                let target = (fl.tx as i32).max(0) as usize;
                let owner = if kind == order::PLANET {
                    w.planets.get(target).map(|p| p.owner)
                } else {
                    w.starmap.fleets.get(target).map(|g| g.owner)
                };
                if owner.is_some_and(|o| o as usize == other) {
                    let mut ev = Vec::new();
                    w.starmap.order(f, order::NONE, order::POINT, 0.0, 0.0, w.star_ctx, &mut ev);
                    for e in ev {
                        match e {
                            StarmapEvent::ColonyShipFreed { empire } => w.starmap.colony_ship_pool[empire as usize] += 1,
                            StarmapEvent::TerraformerFreed { empire } => w.starmap.terraformer_pool[empire as usize] += 1,
                            _ => {}
                        }
                        w.events.push(DiploEvent::Starmap(e));
                    }
                }
            }
            f += 1;
        }
        for e in [a, b] {
            for k in 1..EMPIRES {
                self.known_since[e][k] = self.known_since[e][k].max(600);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::Fleet;

    struct Fixture {
        ctx: DiplomacyContext,
        planets: Vec<DiploPlanet>,
        map: Starmap,
        star: StarmapContext,
        tech: TechTable,
        rng: Streams,
        rules: DiplomacyRules,
        libc: Box<dyn FnMut() -> i32>,
    }

    impl Fixture {
        fn new() -> Self {
            let mut ctx = DiplomacyContext { program_players: 4, program_players2: 4, enabled: true, human_empire: 1, ..Default::default() };
            for e in 1..=4 {
                ctx.alive[e] = true;
                ctx.ai[e] = e != 1;
                ctx.may_attack[e] = true;
            }
            ctx.human[1] = true;
            let planet = |owner, x| DiploPlanet { owner, x, population: 40_000, defence: 10, ..Default::default() };
            let mut map = Starmap { fleets: vec![Fleet { owner: DELETED, ..Default::default() }], ..Default::default() };
            map.planets = vec![Default::default(); 5];
            Self {
                ctx,
                planets: vec![DiploPlanet::default(), planet(1, 0.0), planet(2, 60.0), planet(3, 120.0), planet(4, 180.0)],
                map,
                star: StarmapContext::default(),
                tech: TechTable::default(),
                rng: Streams::new(21),
                rules: DiplomacyRules::from_table(&[100, 10, 200, 20, 300, 30, 400, 40, 1, 2, 3, 4, 5, 6, 7, 8]),
                libc: Box::new(|| 0),
            }
        }
        fn world(&mut self) -> World<'_> {
            World {
                ctx: &mut self.ctx,
                planets: &mut self.planets,
                starmap: &mut self.map,
                star_ctx: &self.star,
                tech: &mut self.tech,
                rng: &mut self.rng,
                rules: &self.rules,
                libc: &mut *self.libc,
                view_libc: None,
                events: Vec::new(),
            }
        }
    }

    fn neutral() -> Diplomacy {
        let mut d = Diplomacy::default();
        for row in d.relations.iter_mut() {
            for r in row.iter_mut() {
                *r = Relation { v: [5000; FIELDS], trade: 0, level: level::NEUTRAL };
            }
        }
        d
    }

    #[test]
    fn record_round_trip() {
        let mut b = [0u8; 0x26];
        for (i, x) in b.iter_mut().enumerate() {
            *x = i as u8 * 7;
        }
        assert_eq!(Relation::from_bytes(&b).to_bytes(), b);
    }

    #[test]
    fn rules_table_order() {
        let r = DiplomacyRules::from_table(&(0..16).collect::<Vec<_>>());
        assert_eq!(r.declare_war[0][0], [0, 1]);
        assert_eq!(r.declare_war[0][3], [6, 7]);
        assert_eq!(r.declare_war[1][2], [12, 13]);
    }

    #[test]
    fn power_and_drift() {
        let mut fx = Fixture::new();
        fx.planets[2].population = 400_000; // empire 2: 1 + 200 + 10 = 211; the others 31
        let mut d = neutral();
        d.set_level(2, 3, level::WAR);
        d.relations[2][3].v[field::TRUST] = 6000;
        d.relations[2][3].v[field::MESSAGES] = 3000;
        let mut w = fx.world();
        d.daily(&mut w);
        assert_eq!(d.power[2], 211.0);
        assert_eq!(d.power[3], 31.0);
        let r = d.relations[2][3];
        // ln(211/31)/ln 5 ≈ 1.192 → k = 5000 + 5960 → clamped 10000 → v14 = 0.
        assert_eq!(r.v[field::POWER], 0);
        assert_eq!(d.relations[3][2].v[field::POWER], 10000);
        // War: trust −8, capped at 5000, then 4 toward 5000.
        assert_eq!(r.v[field::TRUST], 5000);
        assert_eq!(r.v[field::MESSAGES], 0);
        // 60 units apart, one planet pair: 60·10000/600.
        assert_eq!(r.v[field::DISTANCE], 1000);
        // Opinion capped at 3000 during a war.
        assert!(r.v[field::OPINION] <= 3000);
        assert_eq!(d.danger[3], 2);
    }

    #[test]
    fn countdown_kills_planetless_ai() {
        let mut fx = Fixture::new();
        fx.planets[4].owner = 0;
        let mut d = neutral();
        d.countdown[4] = 96;
        let mut w = fx.world();
        d.daily(&mut w);
        // Power 1.0 and countdown 95 < 96.
        assert!(!w.ctx.alive[4]);
        assert!(w.events.contains(&DiploEvent::EmpireDied { empire: 4 }));
    }

    #[test]
    fn war_spreads_unreliability() {
        let mut fx = Fixture::new();
        let mut d = neutral();
        for a in 1..=4 {
            for b in 1..=4 {
                fx.map.met[a][b] = 1;
            }
        }
        d.relations[3][4].level = level::ALLIANCE;
        d.relations[4][3].level = level::ALLIANCE;
        let mut w = fx.world();
        d.war(&mut w, 2, 3, true, 0);
        assert_eq!(d.relations[2][3].level, level::WAR);
        assert_eq!(d.relations[3][2].level, level::WAR);
        // Row 2 (neutral, declared): victim +300, others +30.
        assert_eq!(d.relations[3][2].v[field::UNRELIABLE], 5300);
        assert_eq!(d.relations[4][2].v[field::UNRELIABLE], 5030);
        // 4 is 3's ally.
        assert_eq!(d.relations[4][2].v[field::FRIENDLY], 0);
        assert_eq!(d.relations[3][2].v[field::FRIENDLY], 2000);
    }

    #[test]
    fn peace_stops_attack_orders() {
        let mut fx = Fixture::new();
        fx.map.fleets.push(Fleet { owner: 2, order: order::ATTACK, kind: order::PLANET, tx: 3.0, ..Default::default() });
        fx.map.fleets.push(Fleet { owner: 2, order: order::ATTACK, kind: order::PLANET, tx: 4.0, ..Default::default() });
        let mut d = neutral();
        let mut w = fx.world();
        d.make_peace(&mut w, 2, 3);
        assert_eq!(w.starmap.fleets[1].order, order::NONE);
        assert_eq!(w.starmap.fleets[2].order, order::ATTACK);
        assert_eq!(d.known_since[3][1], 600);
    }

    #[test]
    fn ai_alliance_offers_use_the_nap_rule() {
        let fx = &mut Fixture::new();
        let mut d = neutral();
        d.relations[3][2].level = level::NAP;
        d.relations[3][2].v[field::OPINION] = 9000;
        d.relations[3][2].v[field::SPY_GRIEVANCE] = 0;
        let mut w = fx.world();
        let msg = Message::ai(cat::NEGOTIATE, negotiate::ALLIANCE, 0);
        // The NAP rule needs level 3: an alliance offer between NAP partners is refused.
        assert!(!d.decide(&mut w, &(), 2, 3, msg));
        d.relations[3][2].level = level::NEUTRAL;
        assert!(d.decide(&mut w, &(), 2, 3, msg));
    }

    #[test]
    fn last_contact_cells_overlap() {
        let mut fx = Fixture::new();
        fx.ctx.program_players = 18;
        let mut d = neutral();
        let mut w = fx.world();
        d.ai_messages(&mut w);
        // Cell 19 is both (1, 10) and (2, 1).
        assert_eq!(d.last_contact[19], 2);
        assert_eq!(d.last_contact[10], 1);
    }

    /// A [`Deals`] that books money and records what it was asked to do.
    #[derive(Default)]
    struct Ledger {
        money: [f64; EMPIRES],
        terms: PlanetTerms,
        transfers: Vec<(usize, u8)>,
        developed: Vec<(usize, usize)>,
    }

    impl Deals for Ledger {
        fn money(&self, e: usize) -> f64 {
            self.money[e]
        }
        fn add_money(&mut self, e: usize, x: f64) {
            self.money[e] += x;
        }
        fn planet_terms(&self, _p: usize) -> PlanetTerms {
            self.terms
        }
        fn transfer_planet(&mut self, w: &mut World<'_>, p: usize, owner: u8) -> Vec<(u8, u8)> {
            self.transfers.push((p, owner));
            w.planets[p].owner = owner;
            Vec::new()
        }
        fn develop(&mut self, tech: &mut TechTable, e: usize, t: usize) {
            self.developed.push((e, t));
            tech.set_status(t, e, status::OWNED);
        }
    }

    fn deal(cat: i32, sub: i32, value: i32, sub2: i32, value2: i32) -> Message {
        Message { cat, sub, value, sub2, value2 }
    }

    #[test]
    fn item_values() {
        let mut fx = Fixture::new();
        let d = neutral();
        let w = fx.world();
        let mut ledger = Ledger::default();
        ledger.money[1] = 500.0;
        // Money the giver doesn't have is worth nothing in an offer-for, but counts in a demand.
        assert_eq!(d.deal_value(&w, &ledger, 1, item::MONEY, 800, false, true, 3), 0);
        assert_eq!(d.deal_value(&w, &ledger, 1, item::MONEY, 800, true, true, 3), 800);
        assert_eq!(d.deal_value(&w, &ledger, 1, item::RADAR, 1, false, false, 3), 0);
        assert_eq!(d.deal_value(&w, &ledger, 1, item::RADAR, 2, false, false, 3), 8000);
        assert_eq!(d.deal_value(&w, &ledger, 1, item::HOSTAGE, 2, false, false, 3), 25000);
        // Unknown specials and peace return the item kind (the ELF leaves the result uninitialised).
        assert_eq!(d.deal_value(&w, &ledger, 1, item::HOSTAGE, 9, false, false, 3), item::HOSTAGE);
        assert_eq!(d.deal_value(&w, &ledger, 1, item::PEACE, 2, false, false, 3), item::PEACE);
        // War on a neutral empire.
        assert_eq!(d.deal_value(&w, &ledger, 1, item::WAR, 2, false, false, 3), 10000);
    }

    #[test]
    fn planet_value() {
        let mut fx = Fixture::new();
        fx.rules.surface_affinity = vec![100; 15 * 9];
        fx.rules.surface_affinity[3 * 9 + 2] = 160;
        let mut d = neutral();
        d.planets[1] = 7;
        let w = fx.world();
        let terms = PlanetTerms {
            tax: 1000, owner: 2, race: 3, surface: 2, richness: 130, capturable: true, needed_morale: 500,
            buildings: [0, 1, 1, 0, 0, 1],
        };
        let mut ledger = Ledger { terms, ..Default::default() };
        // 24000 × 0.6 = 14400; (0.3 + 0.6) / 3 + 1 = 1.3 → 18720.
        assert_eq!(d.deal_value(&w, &ledger, 1, item::PLANET, 2, false, false, 3), 18720);
        // Demand: × 1.2 (seven planets) → 28800 × 1.3 = 37440.
        assert_eq!(d.deal_value(&w, &ledger, 1, item::PLANET, 2, true, false, 3), 37440);
        // Offered at war, or without a building of kinds 2/3: worthless.
        assert_eq!(d.deal_value(&w, &ledger, 1, item::PLANET, 2, false, true, level::WAR), 0);
        ledger.terms.buildings[2] = 0;
        assert_eq!(d.deal_value(&w, &ledger, 1, item::PLANET, 2, false, false, 3), 0);
        // Not capturable by the AI: a demand is priced out.
        ledger.terms.capturable = false;
        assert_eq!(d.deal_value(&w, &ledger, 1, item::PLANET, 2, true, false, 3), 10_000_000);
    }

    #[test]
    fn demand_paid_when_favour_allows() {
        let mut fx = Fixture::new();
        let mut d = neutral();
        d.planets[2] = 8;
        d.set_level(1, 2, level::ALLIANCE);
        d.relations[2][1].v[field::FAVOUR] = 6000;
        d.relations[2][1].v[field::UNRELIABLE] = 0;
        let mut ledger = Ledger::default();
        let mut w = fx.world();
        // Threshold: (6000 − 5000)·5 + 5000 (level > 3) + 10000 (alliance) + (−5000 + 5000 − 0)·10 = 20000.
        let msg = deal(cat::DEMAND, item::MONEY, 3000, -1, -1);
        d.receive_message(&mut w, &mut ledger, Letter { from: 1, to: 2, declaration: false, msg });
        assert!(w.events.contains(&DiploEvent::Message { from: 1, to: 2, msg, accepted: Some(true) }));
        assert_eq!(ledger.money[2], -3000.0);
        assert_eq!(ledger.money[1], 3000.0);
        assert_eq!(d.relations[2][1].v[field::FAVOUR], 6000 - 300 - 500);
        assert_eq!(d.relations[1][2].v[field::FAVOUR], 5000 + 300 + 500);
        assert_eq!(d.relations[2][1].v[field::RESPECT], 3000);
        // On the hardest difficulty the AI never pays.
        w.ctx.difficulty = 4;
        w.events.clear();
        d.receive_message(&mut w, &mut ledger, Letter { from: 1, to: 2, declaration: false, msg });
        assert!(w.events.contains(&DiploEvent::Message { from: 1, to: 2, msg, accepted: Some(false) }));
    }

    #[test]
    fn offer_for_trades_planet_and_tech() {
        let mut fx = Fixture::new();
        fx.tech.set_status(7, 2, status::OWNED);
        fx.rules.surface_affinity = vec![100; 135];
        let mut d = neutral();
        d.planets[1] = 3;
        d.relations[2][1].v[field::FRIENDLY] = 6000;
        let mut ledger = Ledger::default();
        ledger.money[1] = 1e9;
        let mut w = fx.world();
        // Money for money: the AI takes it when it gets more than it gives.
        let msg = deal(cat::OFFER_FOR, item::MONEY, 900, item::MONEY, 800);
        d.receive_message(&mut w, &mut ledger, Letter { from: 1, to: 2, declaration: false, msg });
        assert_eq!((ledger.money[1], ledger.money[2]), (1e9 - 900.0 + 800.0, 100.0));
        // A planet of the human's for empire 2's tech 7 (which the tech table prices at 0).
        let msg = deal(cat::OFFER_FOR, item::PLANET, 1, item::TECH, 7);
        ledger.terms = PlanetTerms { tax: 100, owner: 1, race: 1, richness: 100, capturable: true, needed_morale: 500, buildings: [0, 1, 1, 0, 1, 0], ..Default::default() };

        d.receive_message(&mut w, &mut ledger, Letter { from: 1, to: 2, declaration: false, msg });
        assert_eq!(ledger.transfers, vec![(1, 2)]);
        assert_eq!(ledger.developed, vec![(1, 7)]);
        assert_eq!(w.tech.bytes[7 * 0x114 + 0xc0 + 9 * 2 + 1], 1);
    }

    #[test]
    fn counter_proposal_coin() {
        let mut fx = Fixture::new();
        let draws = std::rc::Rc::new(std::cell::RefCell::new(vec![77, 3, 12]));
        let feed = draws.clone();
        fx.libc = Box::new(move || feed.borrow_mut().remove(0));
        let mut d = neutral();
        d.players.push(PlayerSlot { empire: 2, open: 1, asked: 1, ..Default::default() });
        let mut ledger = Ledger::default();
        let mut w = fx.world();
        // The human (1) counters AI 2's message: 1 gives 400 money for a radar map of 3.
        let msg = deal(cat::OFFER_FOR, item::MONEY, 400, item::RADAR, 3);
        d.receive_answer(&mut w, &mut ledger, Answer { from: 1, to: 2, msg, counter: true, reply: false, yes: true });
        // Draws: the text (77), the coin (3: odd = yes), the reply's text (12).
        assert!(draws.borrow().is_empty());
        let reply = Answer { from: 2, to: 1, msg, counter: false, reply: true, yes: true };
        assert!(w.events.contains(&DiploEvent::Answered(reply)));
        assert_eq!((ledger.money[1], ledger.money[2]), (-400.0, 400.0));
        let s = d.players[0];
        assert_eq!((s.answered, s.asked, s.answer), (1, 0, reply));
    }

    #[test]
    fn radar_map_skips_the_last_planet() {
        let mut fx = Fixture::new();
        for p in 1..=4 {
            fx.planets[p].owner = 3;
            fx.planets[p].visible[2] = 1;
        }
        let mut d = neutral();
        let mut w = fx.world();
        d.hand_over(&mut w, &mut (), 2, 4, 4, item::RADAR, 3, false);
        assert_eq!(w.planets[1..].iter().map(|p| p.visible[4]).collect::<Vec<_>>(), vec![1, 1, 1, 0]);
    }

    #[test]
    fn stale_messages_are_cancelled() {
        let mut fx = Fixture::new();
        fx.ctx.day = 5;
        let mut d = neutral();
        let letter = |msg| Letter { from: 2, to: 1, declaration: false, msg };
        d.players = vec![
            // 2 offers planet 2 (its own): still valid.
            PlayerSlot { empire: 2, waiting: 1, letter: letter(deal(cat::OFFER, item::PLANET, 2, -1, -1)), ..Default::default() },
            // 3 offers planet 2 (not its own any more).
            PlayerSlot { empire: 3, waiting: 1, letter: Letter { from: 3, ..letter(deal(cat::OFFER, item::PLANET, 2, -1, -1)) }, ..Default::default() },
            // 4 demands that the human release a hostage nobody holds.
            PlayerSlot { empire: 4, waiting: 1, letter: Letter { from: 4, ..letter(deal(cat::DEMAND, item::HOSTAGE, 1, -1, -1)) }, ..Default::default() },
        ];
        let mut w = fx.world();
        d.tick(&mut w);
        assert_eq!(d.players.iter().map(|s| (s.waiting, s.cancelled)).collect::<Vec<_>>(), vec![(1, 0), (0, 1), (0, 1)]);
        assert_eq!(d.last_day, 5);
    }

    #[test]
    fn cancel_clears_the_other_humans_waiting_message() {
        let mut d = neutral();
        d.players.push(PlayerSlot { empire: 2, asked: 1, screen: 1, ..Default::default() });
        d.others.push(ContactList { empire: 2, players: vec![PlayerSlot { empire: 1, waiting: 1, ..Default::default() }], ..Default::default() });
        d.screen(1, 1, 2, ScreenOp::Cancelled, 0);
        assert_eq!((d.players[0].asked, d.players[0].screen, d.others[0].players[0].waiting), (0, 0, 0));
    }

    #[test]
    fn message_to_the_human_waits() {
        let mut fx = Fixture::new();
        let mut d = neutral();
        d.players.push(PlayerSlot { empire: 3, open: 1, ..Default::default() });
        let mut w = fx.world();
        let msg = Message::ai(cat::NEGOTIATE, negotiate::NAP, 0);
        d.receive_message(&mut w, &mut (), Letter { from: 3, to: 1, declaration: false, msg });
        let s = d.players[0];
        assert_eq!((s.waiting, s.letter.msg, s.letter.from), (1, msg, 3));
        assert_eq!(w.events.iter().filter(|e| matches!(e, DiploEvent::RandDraw { .. })).count(), 1);
    }
}
