//! Campaign scripting: the story engine (`Story_Check`, exe 0x5c5610; `FRISSIT_Story`, 0x5c1ff0; the story
//! queues, variables, logic flags, counters, planet ids, event flags and the undo list) with its hooks
//! (`Story_Special`, `Story_FleetArrivedSave`, …) and the scenario stories 360..375 (`st_Scen_include`,
//! 0x601580). Spec: `re/rules/campaign.md`.
//!
//! The original stories are C code; here each script is a [`Script`] run by [`Story::check`] in the exe's
//! order, followed by the stories `Story_Check` holds inline ([`crate::campaign`]). Of the scripts
//! [`ScenarioStories`], the random stories 220..239 ([`crate::brnd`], which runs first) and the multiplayer modes
//! ([`crate::multi`], which runs last) are ported; the campaign's
//! other scripts (§8) are not yet. The world the scripts read
//! comes through [`StoryWorld`], and what they do to it (the game won, an empire dies, info lines, undo
//! actions that reach other systems, fleet orders) goes out as [`StoryEvent`]s or [`StoryWorld`] calls.
//! The story side of the fleet helpers (`st_fNewEx`, `st_fAddShip`, `AutoAddship`) lives here. Randomness:
//! stream 5 (`st_bSearch`, `st_fSearch`, `st_fNewEx`, the campaign's choices).

use crate::campaign::{self, CampaignTables};
use crate::rng::{stream, Streams};

/// `rndlog(5, …)`.
pub const STREAM: usize = stream::STORY;
/// Story buckets (`StoryVodor[256]`, by `id & 0xff`).
pub const BUCKETS: usize = 256;
/// Event flags (`StoryEsemeny[17]`).
pub const FLAGS: usize = 17;
/// `StoryAdat` size.
pub const ADAT: usize = 0x128;
/// Table capacities (`st_*SearchCreate`).
pub const MAX_COUNTERS: usize = 300;
pub const MAX_LOGIC: usize = 150;
pub const MAX_VARS: usize = 500;
/// `st_Ozonviz` adds entries while the list is shorter than this.
pub const MAX_UNDO: usize = 299;
/// Ticks per day (`Ido` units): counters run in days × 50.
pub const DAY: i32 = 50;

/// `StoryEsemeny` indices.
pub mod flag {
    pub const ARRIVED: usize = 0;
    pub const SPACE_START: usize = 1;
    pub const GROUND_START: usize = 2;
    pub const SPACE_END: usize = 3;
    pub const GROUND_END: usize = 4;
    pub const COUNTER: usize = 5;
    pub const MESSAGE: usize = 6;
    /// `Story_MessageElevules` (`st_Elevul`): an inbox message lapsed (`info-messages.md` §4).
    pub const PROMOTED: usize = 7;
    pub const DISABLED: usize = 8;
    pub const READ: usize = 9;
    pub const RADAR_FLEET: usize = 10;
    pub const RADAR_PLANET: usize = 11;
    pub const SIGNAL: usize = 12;
    pub const DIPLOMACY: usize = 13;
    pub const SPECIAL: usize = 14;
    pub const SPY: usize = 15;
    pub const DAILY: usize = 16;
}

/// `StoryAdat` offsets the engine itself touches.
pub mod adat {
    /// Set by the epidemic stories; cleared when it is over (spec §4 step 3).
    pub const EPIDEMIC: usize = 0x30;
    /// Cleared by every battle hook.
    pub const BATTLE_88: usize = 0x88;
    /// A space battle's no-retreat switches (story 372 sets them).
    pub const NO_RETREAT_94: usize = 0x94;
    pub const NO_RETREAT_98: usize = 0x98;
}

/// `StoryVodorClass`: parallel `(id, value)` arrays (spec §2.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Queue {
    pub entries: Vec<(i32, i32)>,
}

impl Queue {
    pub fn add(&mut self, id: i32, value: i32) {
        self.entries.push((id, value));
    }

    /// `Remove(x)`: the first entry whose id is `x` or `−x`.
    pub fn remove(&mut self, id: i32) {
        if let Some(k) = self.entries.iter().position(|&(e, _)| e == id || e == id.wrapping_neg()) {
            self.entries.remove(k);
        }
    }

    pub fn remove_first(&mut self) {
        if !self.entries.is_empty() {
            self.entries.remove(0);
        }
    }

    pub fn contains(&self, id: i32) -> bool {
        self.entries.iter().any(|&(e, _)| e == id)
    }

    /// `GetDual(x)`: the value of the first entry with id `x`, else 0.
    pub fn value(&self, id: i32) -> i32 {
        self.entries.iter().find(|&&(e, _)| e == id).map_or(0, |&(_, v)| v)
    }

    pub fn first(&self) -> Option<(i32, i32)> {
        self.entries.first().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A 1-based `{id, value}` table with a count (`sSzamlalok`, `sLogikaiak`, `sValtozok`). Slots past the count
/// keep whatever they held: a slot `st_szStop` creates keeps a stale id (spec §2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Slots 0..=capacity; slot 0 is unused.
    pub slots: Vec<(i32, i32)>,
    pub count: usize,
}

impl Table {
    pub fn new(capacity: usize) -> Self {
        Self { slots: vec![(0, 0); capacity + 1], count: 0 }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len() - 1
    }

    /// The live entries 1..=count.
    pub fn live(&self) -> &[(i32, i32)] {
        &self.slots[1..=self.count]
    }

    /// Replace the live entries (slots past them are left as they are).
    pub fn set_live(&mut self, entries: &[(i32, i32)]) {
        self.slots[1..=entries.len()].copy_from_slice(entries);
        self.count = entries.len();
    }

    /// `st_*Search`: the first slot 1..=count with `id`.
    pub fn find(&self, id: i32) -> Option<usize> {
        (1..=self.count).find(|&i| self.slots[i].0 == id)
    }

    /// `st_*SearchCreate`: an existing slot, else a new one while there is room (the id isn't written).
    fn find_or_create(&mut self, id: i32) -> Option<usize> {
        self.find(id).or_else(|| {
            (self.count < self.capacity()).then(|| {
                self.count += 1;
                self.count
            })
        })
    }

    pub fn get(&self, id: i32) -> Option<i32> {
        self.find(id).map(|i| self.slots[i].1)
    }
}

/// A space-battle participant as `Story_SpaceBattleFinished` gets it (`spwsflotta`, 0x30 bytes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Participant {
    /// `+0x18`.
    pub owner: i32,
    /// `+0x20`.
    pub slot_id: i32,
    /// `+0x24`: the fleet's story id.
    pub fleet_id: i32,
}

/// The arriving fleet (`Story_FleetArrivedSave`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Arrival {
    /// `ErkezettFlotta`: the fleet's story id.
    pub fleet_id: i32,
    /// `ErkezettFlottaNumber`: its slot.
    pub fleet: i32,
    /// `FleetArrivedCel`: the target (planet or fleet slot).
    pub target: i32,
    /// `FleetArrivedCelTipus`: 0 a point, 1 a planet, 2 a fleet.
    pub target_type: i32,
    /// `FleetArrivedCelFlottaAzonosito`.
    pub target_fleet_id: i32,
}

/// The ground battle (`Story_GroundBattle*`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GroundBattle {
    pub fleet: i32,
    pub fleet_id: i32,
    pub fleet_owner: i32,
    pub planet: i32,
    pub old_owner: i32,
    pub attacker_won: bool,
    pub defender_won: bool,
}

/// What the scripts do to the rest of the game.
#[derive(Debug, Clone, PartialEq)]
pub enum StoryEvent {
    /// `MEGNYERES(1)`: the human wins.
    Won,
    /// `MEGHALAS(e)` (`DIE()` passes `EN`).
    EmpireDies(i32),
    /// `AddGameInfo(id, text)`: a starmap info line (the scenario countdown passes a number).
    GameInfo { id: i32, value: i32 },
    /// `RemoveGameInfo(id)`.
    RemoveGameInfo(i32),
    /// An undo action of a finished story that reaches another system (types 1–12, spec §2.3).
    Undo { kind: i32, value: i32 },
    /// `StoryAdat+0x30` cleared: remove info 0x65 from every planet that has it.
    EpidemicOver,
    /// `FlottaMozgat(f, 0, 0, 0, 0)`: the fleet stops.
    StopFleet(i32),
    /// `st_AddFleetToSpWar(f)`: the fleet joins the starting battle.
    JoinBattle(i32),
    /// `FlottaMozgat(f, kind, mode, x, y)` (exe 0x53d270): a fleet order (`fleet-movement.md` §3; kind 0
    /// stops, 1 goes to a point or a planet, 2 follows a fleet slot; the slot or planet travels as `x`).
    MoveFleet { fleet: i32, kind: i32, mode: i32, x: f32, y: f32 },
    /// `FlottaDelete(f)` (0x53d040).
    DeleteFleet(i32),
    /// `HajoNew(f, design)` (0x53e800): one ship of the fleet owner's design.
    NewShip { fleet: i32, design: i32 },
    /// `TankNew(f, design, count)` (0x541750).
    NewTanks { fleet: i32, design: i32, count: i32 },
    /// Fleet `+0x3c` (its speed) written directly.
    SetFleetSpeed { fleet: i32, speed: f32 },
    /// Ship `k`'s design (`Hajok[k] + 2`) written directly (story 404's refit).
    UpgradeShip { ship: i32, design: i32 },
    /// `FlottakUjraSzamol()` (0x53dd90): recompute the fleets' totals.
    RecalcFleets,
    /// `AddWormhole(x1, y1, x2, y2)` (0x53aec0) and `Wormhole3DSettings()`.
    Wormhole { x1: i32, y1: i32, x2: i32, y2: i32 },
    /// `st_BolygoDestroy(p)` (0x5d47b0): the planet is wiped out.
    DestroyPlanet(i32),
    /// `st_ShowAnim(_s(id))`: a video (UI).
    Anim(i32),
    /// `st_SetBolygoTulaj(p, owner, -1)`: colonised for `owner` when free (`FUN_0059af50(p, owner, race,
    /// 1, 0, 0, 0)`), else conquered (`BolygoElfoglal(p, owner)`, then planet `+0x21 := race` when non-zero).
    SetPlanetOwner { planet: i32, owner: i32, race: i32 },
    /// `st_AddMoney` → `FUN_0053fb10(e, -1, amount)`.
    AddMoney { empire: i32, amount: f64 },
    /// Story 908: every empire's designs 0..=40 reset to `Tervek0`, auto-designed and recomputed.
    ResetDesigns,
    /// `Story_VulkanKitores(p)` (0x5c95b0).
    Volcano(i32),
    /// `Diplomacy_AddSpecial(id, e)` (0x4bf4b0).
    DiplomacySpecial { id: i32, empire: i32 },
    /// `DestoryerATradereTutorialhoz()` (0x6057f0).
    DestroyerTutorial,
    /// `NewMessage(id, 0)` with empire `e`'s name as parameter 1 and its race's portrait.
    Message { id: i32, empire: i32 },
    /// `SaveRegistrySetting(StoryRegString(campaign), value)`: the storyline the next campaign avoids.
    SaveStoryline { campaign: i32, value: i32 },
    /// `InitStory`'s mirror flags: every planet's x (`+0x18`) / y (`+0x1c`) sign flipped (bit 31).
    Mirror { x: bool, y: bool },
    /// `EpuletNewSearchLocation(p, kind, true, 0, -1, -1)` (0x53cca0): a building placed for free.
    PlaceBuilding { planet: i32, kind: i32 },
    /// `NewMessage(id, arg)` (exe 0x508fe0): a message to the human. It slows the game at once
    /// (`SetIdoLelassit(2)`, [`crate::time::slowed_down`]): later counters of the same tick run at that speed.
    NewMessage { id: i32, arg: i32 },
    /// `SetMessageFajOverride(e)` (0x5094c0): the last message shows empire `e`'s race.
    MessageRace(i32),
    /// `SetParamStr(slot, text)` (0x509570): a parameter of the last message.
    MessageParam { slot: i32, text: MessageText },
    /// `KiszedUzenet(id)` (0x506200): the message is taken back.
    RemoveMessage(i32),
    /// `LetiltReszlet(part)` (0x5063d0): an answer of the message being shown is disabled.
    DisablePart(i32),
    /// `BolygoInfoRemove(p, info, false)` (0x53c0b0): a planet info line removed.
    RemovePlanetInfo { planet: i32, info: i32 },
    /// `st_FelfedezBirodalom(e)` (0x5ea380): the human meets empire `e` (`BirodalomIsmeri[EN][e] := 1`,
    /// `FelfedezBirodalom(e, true)` 0x4c1d60).
    DiscoverEmpire(i32),
    /// `st_bUnhide(−p)` (0x604040): every empire's view of the planet (`+0xda..`, i16 × 18) that is
    /// negative (unknown) becomes 0.
    UnhidePlanet(i32),
    /// Planet `+0x28` written directly.
    SetPlanetValue { planet: i32, value: i32 },
    /// `st_bSetBarat`: planet `+0xbc` written directly.
    SetPlanetFriendly { planet: i32, friendly: bool },
    // ---- st_Multi_include (crate::multi, spec §8.10)
    /// The human's view of the planet (`+0xd8[EN]`) below 1 becomes 1 (`st_bShow`).
    ShowPlanet(i32),
    /// `BolygoInfoAdd(p, info, true)` (0x53beb0): a planet info line.
    AddPlanetInfo { planet: i32, info: i32 },
    /// `st_AddGameInfo(id, …)` with the multiplayer modes' lines (UI; the undo bookkeeping is the story's).
    InfoLine { id: i32, line: InfoLine },
    /// `AddSzoveg(text)` (0x591360): a starmap text for the human (UI).
    Notice(Notice),
    /// `Multi_Send(0x66, EN, 4, &id)` (0x513840): the lockstep message `MULTIGET_Story` turns into
    /// `Story_Special(15, sender, id)`.
    SendStorySpecial(i32),
    /// Fleet `+0x52` bit 8 (always visible) set or cleared.
    SetFleetVisible { fleet: i32, on: bool },
    /// The planet's name (`+0x00`) written.
    RenamePlanet { planet: i32, name: &'static str },
    /// `HajoDelete(k)` (0x53e5d0).
    DeleteShip(i32),
    // ---- st_BRnd_include (crate::brnd, spec §8.11)
    /// `TervDelete(e, d, fleet, mark)` (exe 0x541d50): design `d`'s ships (of `fleet` only when non-zero),
    /// tanks and queue entries go, emptied fleets are deleted; `mark` frees the slot (`+0x16 := −1`).
    DeleteDesign { empire: i32, design: i32, fleet: i32, mark: bool },
    /// `TervUjraSzamol(e, d, 0, 1)` (exe 0x5423b0): the design's stats recomputed.
    RecalcDesign { empire: i32, design: i32 },
    /// The design's `+0x20` (its cost) times 5.
    DesignCostTimes5 { empire: i32, design: i32 },
    /// `st_BolygoMoralValt(p, x)`: planet `+0x174 += delta` (`−10·x`), `+0x178 := 6`.
    PlanetMorale { planet: i32, delta: i32 },
    /// `st_GlobalMoralValt(x)`: `GlobalisMoralCSokkentoBarkacs` (exe 0xBAEAC4) `+= delta` (`−10·x`), its speed
    /// (0xBAEB14) := 6.
    GlobalMorale(i32),
    /// `SetParamInt(slot, value)` (exe 0x509500): a number parameter of the last message.
    MessageParamInt { slot: i32, value: i32 },
    /// `st_SetTalalmanyAllapot(t, 5, −1)` (exe 0x602920): the human is given tech `t` (`TalalmanyKifejlesztve`,
    /// its research time, the recompute; the story queue entry is the story's).
    GiveTech(i32),
    /// `ScrollUzenet(_s(5, id), 1, true, false)` (exe 0x556b50): a line on the scrolling ticker.
    ScrollText(i32),
    /// Fleet `+0x16` (its story id) written (`st_RemoveFlottaAzonosito`: 0).
    SetFleetId { fleet: i32, id: i32 },
    /// `st_fEliminate`: fleet `+0x18` bit 1 set (it is sent away; the order follows as [`StoryEvent::MoveFleet`]).
    SendAway(i32),
    /// The wormhole `AddWormhole` just made (when it made one): both ends shown (`+8`, `+0x14` := 1) and their
    /// fleets always visible (`+0x52` bit 8).
    RevealWormhole,
    /// `EpuletNewSearchLocation(p, kind, true, level, −1, −1)` (`AutoAddBuilding`).
    PlaceStoryBuilding { planet: i32, kind: i32, level: i32 },
}

/// A message parameter's text (`SetParamStr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageText {
    /// `Birodalom_Nev[e]`: the empire's name.
    Empire(i32),
    /// The planet's name (`Bolygok[p]`, its first field).
    Planet(i32),
    /// `PlayerNev[e]` (exe 0xBAE588 + 11·e): the player's name.
    Player(i32),
    /// A tech's name (`Talalmanyok[t]`, its first field).
    Tech(i32),
    /// `_s(5, id)`: a story text.
    Text(i32),
}

/// The lines the multiplayer modes put on the starmap (`st_AddGameInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoLine {
    /// No text (the line's slot only).
    Blank,
    /// The planet's name and a count of days when there is one.
    Planet { planet: i32, count: Option<i32> },
    /// `PlayerNev[empire]`, "GREEN" / "YELLOW" / "RED" (`level` 1..=3), the planet.
    Threat { empire: i32, level: i32, planet: i32 },
    /// Capture the flag: per empire 1 and 2 the score, the touchdowns and whether it holds the other's flag.
    Score { score: [i32; 2], touchdowns: [i32; 2], flag: [bool; 2] },
    /// An empire's name (`Birodalom_Nev[e]`).
    Empire(i32),
}

/// The multiplayer modes' texts for the human (`AddSzoveg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// "AMAZING! YOU ARE THE WINNER!" (1001/1002) or "YOU ARE THE WINNER!" (1003) for the human, else
    /// "`PlayerNev[e]` is the winner!".
    Winner { empire: i32, amazing: bool },
    /// "You cannot colonize `<planet>`!"
    CannotColonise(i32),
    /// "You have the flag of `PlayerNev[from]`!" (the human took it) or "`PlayerNev[by]` has your flag!".
    FlagTaken { by: i32, from: i32 },
    /// "TOUCHDOWN!!!" (the human scored) or "`PlayerNev[by]` has captured your flag!".
    Touchdown { by: i32 },
    /// "You have lost the flag!" (the human's carrier) or "Your flag has been returned!" (empire `by`'s).
    FlagLost { by: i32 },
}

/// What a `.scn` file gives the story engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioStory {
    /// `vSet id value` → `st_vSet`.
    Var(i32, i32),
    /// A planet's `pAzon id` → `st_bAddNewAzonosito(planet, id)`.
    Planet(i32, i32),
}

/// A story fleet record (`StoryFlottaData`, 0x54 bytes, read from `flotta.txt`; `ig2_formats::story_data`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetData {
    /// `+0x00`: the story it belongs to (0 = any).
    pub story: i32,
    /// `+0x04`: the fleet's story id.
    pub fleet: i32,
    /// `+0x08`: `> 0` a story design id (`st_GetTerv`), else the negated design index.
    pub design: i32,
    /// `+0x0c`: ship counts `[difficulty − 1][level − 1]`.
    pub counts: [[i16; 9]; 4],
}

/// A script stopped by `CStoryWinException` (`st_bSearch(0, …)` found nothing, or a script threw it):
/// `Story_Check` catches it, `WIN(0, 1)`s the story and goes on with the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abort;

/// The world as the story engine and the scenario stories read it. Indices are the original's: planets and
/// fleets 1-based with slot 0 readable (a failed search returns 0 and the scripts read slot 0).
pub trait StoryWorld {
    fn ido(&self) -> i32;
    /// `IdoSpeedSzorzo`.
    fn speed_mult(&self) -> i32;
    /// `EN`.
    fn human(&self) -> i32;
    /// `Nehezseg`.
    fn difficulty(&self) -> i32;
    fn multiplayer(&self) -> bool;
    fn rng(&mut self) -> &mut Streams;
    /// `HanyBolygo`.
    fn planet_count(&self) -> i32;
    /// `+0x20`.
    fn planet_owner(&self, p: i32) -> i32;
    /// `+0x28` (story 369's threshold).
    fn planet_value(&self, p: i32) -> i32;
    /// `+0xd8[EN]`: the human's view of the planet (−1 unknown).
    fn planet_visibility(&self, p: i32) -> i32;
    /// `OsszHanyFlotta`.
    fn fleet_count(&self) -> i32;
    /// `+0x15` (0xff = a free slot).
    fn fleet_owner(&self, f: i32) -> i32;
    /// `+0x16`: the story id.
    fn fleet_id(&self, f: i32) -> i32;
    /// `BirodalomEl[e]`.
    fn empire_alive(&self, e: i32) -> bool;
    /// `BirodalomHanyBolygo[e]`.
    fn planets_of(&self, e: i32) -> i32;
    /// `MoneyMennyi(e, 0)`.
    fn money(&self, e: i32) -> f64;
    /// `Talalmany[tech].+0x45 + e`.
    fn tech_status(&self, tech: i32, e: i32) -> i32;
    /// `Viszony[a][b].+0x25`.
    fn relation_level(&self, a: i32, b: i32) -> i32;
    /// Planet `+0xb8 > 0` (an epidemic is running there).
    fn planet_infected(&self, _p: i32) -> bool {
        false
    }
    /// A live fleet (owner ≠ 0xff) with `+0x52` bit 2 (it carries the epidemic).
    fn fleet_infected(&self, _f: i32) -> bool {
        false
    }
    fn event(&mut self, ev: StoryEvent);

    // The campaign (spec §8). The defaults describe a world without a campaign.
    /// `CampaignType` (exe 0x928278): 1..=3 in a campaign, 0 otherwise.
    fn campaign_type(&self) -> i32 {
        0
    }
    /// The campaign tables read from the user's exe (`ig2_rules::Rules::campaign_words`).
    fn campaign_tables(&self) -> &CampaignTables {
        &campaign::NO_TABLES
    }
    /// `PlayerFaj[e]`.
    fn race(&self, _e: i32) -> i32 {
        0
    }
    /// `BirodalomIsmeri[a][b]`.
    fn knows(&self, _a: i32, _b: i32) -> bool {
        false
    }
    /// `AktPlanet` (exe 0xB9933C), `AktFleet` (0xB99664): the selected planet and fleet.
    fn akt_planet(&self) -> i32 {
        0
    }
    fn akt_fleet(&self) -> i32 {
        0
    }
    /// Planet `+0x18` / `+0x1c`.
    fn planet_xy(&self, _p: i32) -> (f32, f32) {
        (0.0, 0.0)
    }
    /// Planet `+0x6e`: its radar level.
    fn planet_radar(&self, _p: i32) -> i32 {
        0
    }
    /// `RadarHatotav2[e][level]` (exe 0xBB61A8, f32 `[19][8]`).
    fn radar_range(&self, _e: i32, _level: i32) -> f32 {
        0.0
    }
    /// Fleet `+0x30` / `+0x34`.
    fn fleet_xy(&self, _f: i32) -> (f32, f32) {
        (0.0, 0.0)
    }
    /// Fleet `+0x42` (i16): its order kind (0 none, 1 point/planet, 2 fleet).
    fn fleet_order(&self, _f: i32) -> i32 {
        0
    }
    /// Fleet `+0x5c` (story 403: non-zero while the escort is out).
    fn fleet_5c(&self, _f: i32) -> i32 {
        0
    }
    /// `OsszHanyHajo`.
    fn ship_count(&self) -> i32 {
        0
    }
    /// Ship `k` (`Hajok`, 6 bytes): `(+0 fleet, +2 design)`; a free slot has design 0x7fff.
    fn ship(&self, _k: i32) -> (i32, i32) {
        (0, 0x7fff)
    }
    /// `Tervek[e][d]` `+0x1c` (the design it refits to, `tUpgradeMire`) and `+0x24` (its price).
    fn design_upgrade(&self, _e: i32, _d: i32) -> i32 {
        0
    }
    fn design_price(&self, _e: i32, _d: i32) -> i32 {
        0
    }
    /// `Tervek[e][d]`: `+0x16 == 2 || +0x36 != 0` (a ground unit: `st_fAddShip` makes tanks).
    fn design_is_ground(&self, _e: i32, _d: i32) -> bool {
        false
    }
    /// `BolygoInfoIs(p, info)` (exe 0x53bfe0): one of the planet's info slots 1..=5 (`+0x16a`) holds `info`.
    fn planet_has_info(&self, _p: i32, _info: i32) -> bool {
        false
    }
    /// `BirodalomEro[e]` (exe 0xBAEA78, f32): the empire's strength.
    fn empire_strength(&self, _e: i32) -> f32 {
        0.0
    }
    /// `MiTervMutato[e][slot]` (exe 0xDAF668, i16 `[19][25]`): the AI's design per role.
    fn ai_design(&self, _e: i32, _slot: i32) -> i32 {
        0
    }
    /// `st_IsInRR(x, y, k)` (exe 0x5eb300): the point is inside some radar range.
    fn in_radar_range(&mut self, _x: i32, _y: i32, _k: i32) -> bool {
        false
    }
    /// `st_GetTerv(e, key, default)` (exe 0x5eacf0): empire `e`'s design for story design `key`, created
    /// from the story's design data the first time; `default` when the key is unknown.
    fn story_design(&mut self, _e: i32, _key: i32, default: i32) -> i32 {
        default
    }
    /// `FlottaNew(name, owner, id, x, y, speed)` (exe 0x53d400): the new fleet's slot.
    fn new_fleet(&mut self, _name: &str, _owner: i32, _id: i32, _x: f32, _y: f32, _speed: f32) -> i32 {
        0
    }
    /// `ConfigFile.GetParam(key, "0")` → `atoi` (`storyline`, `storymap`).
    fn config_int(&self, _key: &str) -> i32 {
        0
    }
    /// `ReadRegistrySetting(StoryRegString(campaign))`: `None` when the read fails.
    fn registry_storyline(&self, _campaign: i32) -> Option<i32> {
        None
    }
    /// `LoadScenario("scenario/<n>.scn", 1)`: loads the campaign map (and reseeds the RNG streams from its
    /// `RandSeed`); returns what the scenario gives the story engine, in file order (`vSet` lines, the
    /// planets' `pAzon`).
    fn load_scenario(&mut self, _n: i32) -> Vec<ScenarioStory> {
        Vec::new()
    }
    /// `Generate_StarNames()` (exe 0x59b860, stream 1): names the star systems; returns the planets it
    /// gives the story ids 500.. (`st_bAddNewAzonosito(planet, id)`, in call order) and the number of
    /// systems (variable 500).
    fn generate_star_names(&mut self) -> (Vec<(i32, i32)>, i32) {
        (Vec::new(), 0)
    }
    /// `InfoString_GetIndex(info) != 0` (exe 0x53ec40): the info string table (0xBAA1F0) has `info` past
    /// its first record.
    fn info_string(&self, _info: i32) -> bool {
        false
    }
    /// `CTF_value[side]` (exe 0xF261EC, i32 × 3): the attack each side lost in the last auto-resolved battle.
    fn ctf_value(&self, _side: i32) -> i32 {
        0
    }
    /// `TervNew(e, 0)` (exe 0x5422f0), the record copied over the new design, then `TervUjraSzamol(e, d, 0,
    /// 1)`: the design's slot.
    fn new_design(&mut self, _e: i32, _record: &[u8; DESIGN]) -> i32 {
        0
    }

    // st_BRnd_include (crate::brnd, spec §8.11).
    /// `TervNew(e, 0)` with the record copied over the new design (no recompute): the design's slot.
    fn new_design_raw(&mut self, _e: i32, _record: &[u8; DESIGN]) -> i32 {
        0
    }
    /// `OsszHanyTerv[e]` (exe 0xB9C028): the empire's last design slot.
    fn design_count(&self, _e: i32) -> i32 {
        0
    }
    /// `Tervek[e][d]` `+0x80`: the story design key `st_GetTerv` gave it (0 for the others).
    fn design_key(&self, _e: i32, _d: i32) -> i32 {
        0
    }
    /// `Tervek[e][d]` `+0x2c`: its attack (`st_GetOsszFlottaEro`).
    fn design_attack(&self, _e: i32, _d: i32) -> i32 {
        0
    }
    /// `Talalmany[t]` `+0x2d..=+0x2f`: the tech's prerequisites (0 = none).
    fn tech_prereqs(&self, _t: i32) -> [i32; 3] {
        [0; 3]
    }
    /// `spFlottak` (exe 0xBAA1C6 / 0xBAA1CC, i16): the fleet slots of the space battle starting.
    fn battle_fleets(&self) -> (i32, i32) {
        (0, 0)
    }
    /// Fleet `+0x40` (i16: non-zero while it has ships) and `+0x44` (f32, the order's target: a planet or fleet
    /// slot).
    fn fleet_command(&self, _f: i32) -> i32 {
        0
    }
    fn fleet_target(&self, _f: i32) -> f32 {
        0.0
    }
    /// Fleet `+0x18` (bit 1: `st_fEliminate` sent it away).
    fn fleet_state(&self, _f: i32) -> i32 {
        0
    }
    /// Planet `+0x70` and `+0x74` (`st_bGet3BestProd`'s production).
    fn planet_production(&self, _p: i32) -> (i32, i32) {
        (0, 0)
    }
    /// `EpuletekLeiro[kind]` `+0x10` (`ig2_rules::Building::workers`).
    fn building_workers(&self, _kind: i32) -> i32 {
        0
    }
    /// Some wormhole 1..=`OsszHanyWormhole` has both ends shown (`Wormhole[k]` `+8` and `+0x14`).
    fn wormhole_shown(&self) -> bool {
        false
    }
}

/// A design record's size (`Tervek[e][d]`, the `.scn` `TERV` layout).
pub const DESIGN: usize = 0x84;

/// One `st_*_include` script.
pub trait Script {
    fn run(&self, story: &mut Story, world: &mut dyn StoryWorld);
}

/// Two scripts in order (`Story_Check` runs `Scen`, then `Line`, …).
impl<A: Script, B: Script> Script for (A, B) {
    fn run(&self, story: &mut Story, world: &mut dyn StoryWorld) {
        self.0.run(story, world);
        self.1.run(story, world);
    }
}

/// The story engine's state (spec §2).
#[derive(Debug, Clone, PartialEq)]
pub struct Story {
    /// `StoryVodor`: the running stories with their start flags.
    pub buckets: Vec<Queue>,
    /// `HistoryVodor`: `(id, Ido)` of the finished stories.
    pub history: Queue,
    /// `StartVodor`: stories waiting for the next top-level check.
    pub starting: Queue,
    /// `VegzesVodor`: "finished" signals waiting.
    pub finished: Queue,
    /// `TalalmanyVodor`: `(type, data1)` specials waiting.
    pub specials: Queue,
    /// `OlvasasVodor`: message blocks read.
    pub read: Queue,
    /// `sBolygok`: `(id, planet)` (1-based in the original; no capacity check).
    pub planets: Vec<(i32, u8)>,
    pub counters: Table,
    pub logic: Table,
    pub vars: Table,
    /// `StoryError`.
    pub error: i32,
    /// `StoryEsemeny`.
    pub flags: [bool; FLAGS],
    /// `StoryLastChecked`.
    pub last_checked: i32,
    /// `SzamlaloMelyikVegzett`.
    pub counter_done: i32,
    /// `(StorySignalMelyik, StorySignalTipus)`.
    pub signal: (i32, i32),
    /// `FRISSIT_Story::OldIdo`.
    pub old_ido: i32,
    pub delayed: bool,
    /// `Story_SkipFirst`.
    pub skip_first: i32,
    /// `StoryAktKuldetesNowReStarting`.
    pub restarting: i32,
    /// `Story_check_locked` (never set by the exe).
    pub locked: bool,
    /// `Story_Check::ent_count`.
    pub depth: i32,
    /// `last_addship`.
    pub last_addship: i32,
    pub adat: [u8; ADAT],
    /// `st_vOzonviz`: `(type, a, owner)`.
    pub undo: Vec<(i32, i32, i32)>,
    /// `SpecialType`, `SpecialData1..7`.
    pub special: [i32; 8],
    /// `ArrivedToRadarBolygo`, `ArrivedToRadarFleet`.
    pub radar: (i32, i32),
    /// `StoryDiplomaciaMelyik`, `…Birodalom`, `…Valasz`.
    pub diplomacy: (i32, i32, i32),
    pub arrival: Arrival,
    pub ground: GroundBattle,
    /// `SpWarYouWasInBattle`.
    pub you_were_in_battle: bool,
    /// `SpwFlottak[..SpwFlottaNum]`.
    pub battle: Vec<Participant>,
    /// `SpWarKiNyert`: the winning owner.
    pub battle_winner: i32,
    /// `KemkedesTipus`.
    pub spy_kind: i32,
    /// `Story_SpWarTiltas`, `Story_SpWarSimulateTiltas`, `Story_GrWarTiltas`, `Story_GrWarSimulateTiltas`.
    pub space_forbidden: bool,
    pub space_simulate_forbidden: bool,
    pub ground_forbidden: bool,
    pub ground_simulate_forbidden: bool,
    /// `StoryUrharcViszony[19]`, `StoryUrharcVisszavonulhate[19]`, `StoryUrharcZachkinBarkacs`.
    pub battle_pact: [i32; 19],
    pub battle_retreat: [i32; 19],
    pub zachkin: i32,
    /// `StoryMessageBlokk`, `StoryMessageResz` (exe 0xF26574 / 0xF26578): the message `st_Message` tests.
    pub message: (i32, i32),
    /// `values.txt` (`st_GetValue`; exe 0xF26204 count, pairs at 0xF26208).
    pub values: Vec<(i32, i32)>,
    /// `StoryFlottaData` (exe 0xF27530, count 0xF2F870).
    pub fleet_data: Vec<FleetData>,
    /// `DisableAddShip` (exe 0xF273F0): the fleet id `AutoAddship` last filled (`st_fAddShip` skips it).
    pub disable_add_ship: i32,
    /// `st_fNewEx`'s last placement: `last_fn_b` (planet or fleet), `_r` (radius), `_q` (angle), `_x`, `_y`
    /// (exe 0xF27524, 0xF273F4, 0xF27520, 0xF27528, 0xF2752C).
    pub last_fn: [i32; 5],
    /// `InitStory`'s storyline story (exe 0xB77288) and campaign map (0xB7728C).
    pub line_story: i32,
    pub scenario: i32,
    /// `nearest_bir_planet` (exe 0xF2652C): the planet `st_GetBirodalom808hoz` last chose.
    pub nearest_planet: i32,
    /// `st_bGet3BestProd::best` (exe 0xF432B8): the three planets it last chose (a static).
    pub best_prod: [i32; 3],
    /// `Story_Minden_Flotta_Latszik` (exe 0xF265D4): story 237 shows every fleet.
    pub all_fleets_visible: u8,
    /// `StoryBuildingData` (exe 0xF22740, count 0xF23A00): the story buildings (`AutoAddBuilding`).
    pub building_data: Vec<BuildingData>,
}

/// A story building record (`StoryBuildingData`, 16 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingData {
    /// The story it belongs to (0 = any).
    pub story: i32,
    /// The planet's story id.
    pub planet: i32,
    pub kind: i32,
    /// `EpuletNewSearchLocation`'s fourth argument.
    pub level: i32,
}

/// `last_fn` indices.
pub mod last_fn {
    pub const B: usize = 0;
    pub const R: usize = 1;
    pub const Q: usize = 2;
    pub const X: usize = 3;
    pub const Y: usize = 4;
}

impl Default for Story {
    fn default() -> Self {
        Self {
            buckets: vec![Queue::default(); BUCKETS],
            history: Queue::default(),
            starting: Queue::default(),
            finished: Queue::default(),
            specials: Queue::default(),
            read: Queue::default(),
            planets: Vec::new(),
            counters: Table::new(MAX_COUNTERS),
            logic: Table::new(MAX_LOGIC),
            vars: Table::new(MAX_VARS),
            error: 0,
            flags: [false; FLAGS],
            last_checked: 0,
            counter_done: 0,
            signal: (0, 0),
            old_ido: 0,
            delayed: false,
            skip_first: 0,
            restarting: 0,
            locked: false,
            depth: 0,
            last_addship: 0,
            adat: [0; ADAT],
            undo: Vec::new(),
            special: [0; 8],
            radar: (0, 0),
            diplomacy: (0, 0, 0),
            arrival: Arrival::default(),
            ground: GroundBattle::default(),
            you_were_in_battle: false,
            battle: Vec::new(),
            battle_winner: 0,
            spy_kind: 0,
            space_forbidden: false,
            space_simulate_forbidden: false,
            ground_forbidden: false,
            ground_simulate_forbidden: false,
            battle_pact: [0; 19],
            battle_retreat: [0; 19],
            zachkin: 0,
            message: (0, 0),
            values: Vec::new(),
            fleet_data: Vec::new(),
            disable_add_ship: 0,
            last_fn: [0; 5],
            line_story: 0,
            scenario: 0,
            nearest_planet: 0,
            best_prod: [0; 3],
            all_fleets_visible: 0,
            building_data: Vec::new(),
        }
    }
}

const fn bucket(id: i32) -> usize {
    (id as u32 & 0xff) as usize
}

impl Story {
    // ---------------------------------------------------------------- StoryAdat
    pub fn adat_i32(&self, off: usize) -> i32 {
        i32::from_le_bytes(self.adat[off..off + 4].try_into().unwrap())
    }

    pub fn set_adat_i32(&mut self, off: usize, v: i32) {
        self.adat[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }

    // ---------------------------------------------------------------- stories (spec §2.1–§2.3)
    /// `st_Story(id, 1)`: is `id` running; when it is, `StoryLastChecked := id`.
    pub fn running(&mut self, id: i32) -> bool {
        let found = self.buckets[bucket(id)].contains(id);
        if found {
            self.last_checked = id;
        }
        found
    }

    /// `st_Story(id, 0)`.
    pub fn is_running(&self, id: i32) -> bool {
        self.buckets[bucket(id)].contains(id)
    }

    /// `st_StoryStart(0)`: the story being checked sees its first check.
    pub fn starting_now(&self) -> bool {
        let id = self.last_checked;
        self.buckets[bucket(id)].value(id) != 0
    }

    /// `StartStory(id, now)`.
    pub fn start_story(&mut self, id: i32, now: bool) -> bool {
        if id == 0 || self.running(id) {
            return false;
        }
        if now {
            self.buckets[bucket(id)].add(id, 1);
        } else {
            self.starting.add(id, 0);
        }
        true
    }

    /// `WIN(id, history)`: the story ends (spec §2.3). `id == 0` means the story being checked.
    pub fn win(&mut self, id: i32, history: bool, world: &mut dyn StoryWorld) {
        let id = if id == 0 { self.last_checked } else { id };
        self.running(id);
        for k in 0..self.undo.len() {
            let (kind, a, owner) = self.undo[k];
            if owner != id {
                continue;
            }
            match kind {
                0 | 12 => {}
                13 => {
                    let off = 0xd0usize.wrapping_add((a as usize).wrapping_mul(4));
                    if off + 4 <= ADAT && self.adat_i32(off) > 0 {
                        let v = self.adat_i32(off) - 1;
                        self.set_adat_i32(off, v);
                    }
                }
                14 => self.set_adat_i32(0x28, 0),
                15 => self.set_adat_i32(0x24, 0),
                k if (1..=11).contains(&k) => world.event(StoryEvent::Undo { kind, value: a }),
                _ => {}
            }
            self.undo[k] = (0, 0, 0);
        }
        while self.undo.last().is_some_and(|e| e.0 == 0) {
            self.undo.pop();
        }
        self.buckets[bucket(id)].remove(id);
        self.finished.add(id, 1);
        if history && !self.history.contains(id) {
            self.history.add(id, world.ido());
        }
    }

    /// `st_Ozonviz(type, a, owner)`: an undo action for `WIN(owner)`.
    pub fn add_undo(&mut self, kind: i32, a: i32, owner: i32) {
        if self.undo.len() >= MAX_UNDO {
            return;
        }
        match self.undo.iter().position(|e| e.0 == 0) {
            Some(k) => self.undo[k] = (kind, a, owner),
            None => self.undo.push((kind, a, owner)),
        }
    }

    /// `st_AddGameInfo(id, text, …, owner)` with the scenario countdown's number as the text.
    pub fn add_game_info(&mut self, id: i32, value: i32, owner: i32, world: &mut dyn StoryWorld) {
        self.add_info(id, StoryEvent::GameInfo { id, value }, owner, world);
    }

    /// `st_AddGameInfo(id, …, owner)` with one of the multiplayer modes' lines.
    pub fn add_info_line(&mut self, id: i32, line: InfoLine, owner: i32, world: &mut dyn StoryWorld) {
        self.add_info(id, StoryEvent::InfoLine { id, line }, owner, world);
    }

    fn add_info(&mut self, id: i32, line: StoryEvent, owner: i32, world: &mut dyn StoryWorld) {
        let owner = if owner == -1 { id / 10 } else { owner };
        world.event(line);
        let known = self.undo.contains(&(4, id, owner));
        if owner != 0 && !known {
            self.add_undo(4, id, owner);
        }
    }

    /// `st_RemoveGameInfo(id)`.
    pub fn remove_game_info(&mut self, id: i32, world: &mut dyn StoryWorld) {
        for e in self.undo.iter_mut() {
            if e.0 == 4 && e.1 == id {
                *e = (0, 0, 0);
            }
        }
        world.event(StoryEvent::RemoveGameInfo(id));
    }

    // ---------------------------------------------------------------- variables, flags, counters (§2.4, §2.5)
    pub fn v_set(&mut self, id: i32, v: i32) {
        match self.vars.find_or_create(id) {
            Some(i) => {
                self.vars.slots[i] = (id, v);
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    pub fn v_get(&mut self, id: i32) -> i32 {
        let v = self.vars.get(id);
        self.error = v.is_none() as i32;
        v.unwrap_or(0)
    }

    fn v_update(&mut self, id: i32, f: impl FnOnce(i32) -> i32) {
        let i = self.vars.find(id);
        if let Some(i) = i {
            self.vars.slots[i].1 = f(self.vars.slots[i].1);
        }
        self.error = i.is_none() as i32;
    }

    pub fn v_add(&mut self, id: i32, x: i32) {
        self.v_update(id, |v| v.wrapping_add(x));
    }

    pub fn v_sub(&mut self, id: i32, x: i32) {
        self.v_update(id, |v| v.wrapping_sub(x));
    }

    pub fn v_dec(&mut self, id: i32) {
        self.v_update(id, |v| v.wrapping_sub(1));
    }

    pub fn v_inc(&mut self, id: i32) {
        let v = self.v_get(id);
        self.v_set(id, v.wrapping_add(1));
    }

    fn v_test(&mut self, id: i32, f: impl FnOnce(i32) -> bool) -> bool {
        let v = self.vars.get(id);
        self.error = v.is_none() as i32;
        v.is_some_and(f)
    }

    pub fn v_is_equal(&mut self, id: i32, x: i32) -> bool {
        self.v_test(id, |v| v == x)
    }

    /// `st_vGreater`: `v ≥ x`.
    pub fn v_greater(&mut self, id: i32, x: i32) -> bool {
        self.v_test(id, |v| v >= x)
    }

    /// `st_vSmaller`: `v ≤ x`.
    pub fn v_smaller(&mut self, id: i32, x: i32) -> bool {
        self.v_test(id, |v| v <= x)
    }

    /// `st_lSetToTrue` / `st_lSetToFalse` (`st_lSearchCreate` writes the id itself).
    pub fn l_set(&mut self, id: i32, on: bool) {
        match self.logic.find_or_create(id) {
            Some(i) => {
                self.logic.slots[i] = (id, on as i32);
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    /// `st_lIsTrue` (= `st_lGet`).
    pub fn l_is_true(&mut self, id: i32) -> bool {
        let v = self.logic.get(id);
        self.error = v.is_none() as i32;
        v.is_some_and(|b| b & 1 != 0)
    }

    /// `st_lIsFalse`: also true when the flag doesn't exist.
    pub fn l_is_false(&mut self, id: i32) -> bool {
        let v = self.logic.get(id);
        self.error = v.is_none() as i32;
        v.is_none_or(|b| b & 1 == 0)
    }

    /// `st_szStart(id, days)`.
    pub fn sz_start(&mut self, id: i32, days: i32) {
        match self.counters.find_or_create(id) {
            Some(i) => {
                self.counters.slots[i] = (id, days.wrapping_mul(DAY));
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    /// `st_szStop(id)`: a new slot keeps its stale id.
    pub fn sz_stop(&mut self, id: i32) {
        match self.counters.find_or_create(id) {
            Some(i) => {
                self.counters.slots[i].1 = -1;
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    /// `st_szGet(id)`: days left, rounded up.
    pub fn sz_get(&mut self, id: i32) -> i32 {
        let v = self.counters.get(id);
        self.error = v.is_none() as i32;
        v.map_or(0, |t| t.wrapping_add(DAY - 1) / DAY)
    }

    pub fn sz_is_now_going(&mut self, id: i32) -> bool {
        let v = self.counters.get(id);
        self.error = v.is_none() as i32;
        v.is_some_and(|t| t > 0)
    }

    pub fn sz_is_ever_started(&mut self, id: i32) -> bool {
        let v = self.counters.get(id);
        self.error = v.is_none() as i32;
        v.is_some()
    }

    // ---------------------------------------------------------------- planet and fleet ids (§2.7)
    /// `st_bAddNewAzonosito(p, id)`.
    pub fn b_add_id(&mut self, p: i32, id: i32) {
        if p == 0 {
            return;
        }
        match self.planets.iter().position(|e| e.0 == id) {
            Some(k) => self.planets[k] = (id, p as u8),
            None => self.planets.push((id, p as u8)),
        }
    }

    /// `st_bRemoveAzonosito(id)`: the entry is zeroed, the count stays.
    pub fn b_remove_id(&mut self, id: i32) {
        if id < 0 {
            return;
        }
        if let Some(k) = self.planets.iter().position(|e| e.0 == id) {
            self.planets[k] = (0, 0);
        }
    }

    /// `st_bSearch(id, mode)` where an abort can't happen (`id != 0` or `mode == 1`); an abort reads as 0.
    pub fn b_search(&mut self, id: i32, mode: i32, world: &mut dyn StoryWorld) -> i32 {
        self.b_search_try(id, mode, world).unwrap_or(0)
    }

    /// `st_bSearch(id, mode)` (exe 0x603d30). `id ≤ 0`: a random planet of empire `−id` the human can see
    /// (stream 5, or 2 when `mode == 2`). With none, `id == 0` and `mode != 1` it throws (no draw):
    /// [`Abort`].
    pub fn b_search_try(&mut self, id: i32, mode: i32, world: &mut dyn StoryWorld) -> Result<i32, Abort> {
        if id > 0 {
            return Ok(self.planets.iter().find(|e| e.0 == id).map_or(0, |e| e.1 as i32));
        }
        let n = world.planet_count();
        let fits = |w: &dyn StoryWorld, q: i32| w.planet_owner(q) == id.wrapping_neg() && w.planet_visibility(q) >= 0;
        let count = (1..=n).filter(|&q| fits(&*world, q)).count() as i32;
        if count == 0 && id == 0 && mode != 1 {
            return Err(Abort);
        }
        let s = if mode == 2 { stream::GROUND_COMBAT } else { STREAM };
        let r = world.rng().range(s, 1, count);
        let mut k = 0;
        let mut found = 0;
        for q in 1..=n {
            if fits(&*world, q) {
                k += 1;
                if k == r {
                    found = q;
                }
            }
        }
        Ok(found)
    }

    /// `st_fSearch(id, _)`. `id == 0` means `−EN`; `id < 0`: a random fleet of empire `−id` (stream 5).
    pub fn f_search(&mut self, id: i32, world: &mut dyn StoryWorld) -> i32 {
        let id = if id == 0 { -world.human() } else { id };
        let n = world.fleet_count();
        if id > 0 {
            return (1..=n).find(|&f| world.fleet_id(f) == id && world.fleet_owner(f) != 0xff).unwrap_or(0);
        }
        let count = (1..=n).filter(|&f| world.fleet_owner(f) == -id).count() as i32;
        let r = world.rng().range(STREAM, 1, count);
        let mut k = 0;
        let mut found = 0;
        for f in 1..=n {
            if world.fleet_owner(f) == -id {
                k += 1;
                if k == r {
                    found = f;
                }
            }
        }
        found
    }

    // ---------------------------------------------------------------- event predicates (§5)
    pub fn daily(&self) -> bool {
        self.flags[flag::DAILY]
    }

    /// `st_Counter(id)`.
    pub fn counter(&self, id: i32) -> bool {
        self.flags[flag::COUNTER] && (id == 0 || self.counter_done == id)
    }

    /// `st_Signal(id, t)`.
    pub fn signal(&self, id: i32, t: i32) -> bool {
        self.flags[flag::SIGNAL] && (id == 0 || self.signal.0 == id) && (t == 0 || self.signal.1 == t)
    }

    /// `st_StoryFinish(id)`.
    pub fn story_finished(&self, id: i32) -> bool {
        self.flags[flag::SIGNAL] && (id == 0 || self.signal.0 == id) && self.signal.1 == 1
    }

    /// `st_Special(type, d1, d2, d3)`.
    pub fn special_is(&self, t: i32, d1: i32, d2: i32, d3: i32) -> bool {
        let s = &self.special;
        self.flags[flag::SPECIAL] && s[0] == t && (d1 == 0 || s[1] == d1) && (d2 == 0 || s[2] == d2) && (d3 == 0 || s[3] == d3)
    }

    /// `st_ArrivedS(fid)`: a fleet reached a point.
    pub fn arrived_point(&self, fid: i32) -> bool {
        self.flags[flag::ARRIVED] && self.arrival.target_type == 0 && (fid == 0 || self.arrival.fleet_id == fid)
    }

    /// `st_ArrivedB(fid, bid)`: a fleet reached a planet.
    pub fn arrived_planet(&mut self, fid: i32, bid: i32, world: &mut dyn StoryWorld) -> bool {
        if !(self.flags[flag::ARRIVED] && self.arrival.target_type == 1 && (fid == 0 || self.arrival.fleet_id == fid)) {
            return false;
        }
        bid == 0 || self.b_search(bid, 0, world) == self.arrival.target
    }

    /// `st_ArrivedF(fid, fid2)`: a fleet reached a fleet.
    pub fn arrived_fleet(&self, fid: i32, fid2: i32) -> bool {
        self.flags[flag::ARRIVED]
            && self.arrival.target_type == 2
            && (fid == 0 || self.arrival.fleet_id == fid)
            && (fid2 == 0 || self.arrival.target_fleet_id == fid2)
    }

    /// `st_SpWarPart(fid)`: the 1-based participant with story id `fid` in the finished battle (`fid == 0`:
    /// 1 if a battle finished).
    pub fn battle_part(&self, fid: i32) -> i32 {
        if !self.flags[flag::SPACE_END] {
            return 0;
        }
        if fid == 0 {
            return 1;
        }
        if self.battle.len() < 3 {
            // The fast path looks at the first two entries only.
            return (0..2).find(|&i| self.battle.get(i).is_some_and(|p| p.fleet_id == fid)).map_or(0, |i| i as i32 + 1);
        }
        self.battle.iter().position(|p| p.fleet_id == fid).map_or(0, |i| i as i32 + 1)
    }

    /// `st_SpWarWin(fid)`.
    pub fn battle_won(&self, fid: i32) -> bool {
        let i = self.battle_part(fid);
        i != 0 && self.battle.get(i as usize - 1).is_some_and(|p| p.owner == self.battle_winner)
    }

    /// `st_Diff(a, b, c, d)` by difficulty.
    pub fn diff(difficulty: i32, a: i32, b: i32, c: i32, d: i32) -> i32 {
        match difficulty {
            1 => a,
            2 => b,
            3 => c,
            _ => d,
        }
    }

    /// `st_GetValue(key)`: a `values.txt` value scaled by difficulty (f32, truncated); 10 when missing.
    pub fn get_value(values: &[(i32, i32)], key: i32, difficulty: i32) -> i32 {
        const SCALE: [f32; 5] = [0.0, 2.0, 1.3, 1.3, 1.0];
        match values.iter().find(|e| e.0 == key) {
            Some(&(_, v)) => (v as f32 * SCALE.get(difficulty as usize).copied().unwrap_or(0.0)) as i32,
            None => 10,
        }
    }

    // ---------------------------------------------------------------- hooks (§1)
    /// `NEWGAME_Story` (the story data files are read separately: `ig2_formats::story_data`).
    pub fn new_game(&mut self) {
        self.undo.clear();
        self.planets.clear();
        self.counters.count = 0;
        self.logic.count = 0;
        self.vars.count = 0;
        self.adat = [0; ADAT];
    }

    /// `Story_Generate`.
    pub fn generate(&mut self) {
        self.flags = [false; FLAGS];
    }

    /// `FRISSIT_Story` (spec §3): every tick.
    pub fn tick(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        let mut i = 1;
        while i <= self.counters.count {
            let t = self.counters.slots[i].1;
            if t >= 0 {
                let t = t.wrapping_sub(world.speed_mult());
                self.counters.slots[i].1 = t;
                if t < 1 {
                    self.flags[flag::COUNTER] = true;
                    self.counter_done = self.counters.slots[i].0;
                    self.counters.slots[i].1 = -1;
                    self.check(true, scripts, world);
                }
            }
            i += 1;
        }
        let ido = world.ido();
        if ido < self.old_ido || self.old_ido.wrapping_add(DAY) < ido {
            self.old_ido = ido;
            self.flags[flag::DAILY] = true;
            self.check(true, scripts, world);
        }
    }

    /// `Story_Check(top)` (spec §4).
    pub fn check(&mut self, top: bool, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.locked {
            return;
        }
        self.depth += 1;
        self.delayed = false;
        self.check_add_ship(world);
        self.restarting = 0;
        scripts.run(self, world);
        campaign::inline_stories(self, world);
        if self.adat_i32(adat::EPIDEMIC) != 0 && self.daily() {
            let planets = world.planet_count();
            let fleets = world.fleet_count();
            if !(1..=planets).any(|p| world.planet_infected(p)) && !(1..=fleets).any(|f| world.fleet_infected(f)) {
                self.set_adat_i32(adat::EPIDEMIC, 0);
                world.event(StoryEvent::EpidemicOver);
            }
        }
        self.check_add_ship(world);
        self.flags = [false; FLAGS];
        let keep = self.restarting;
        for b in self.buckets.iter_mut() {
            for e in b.entries.iter_mut() {
                if e.1 != 0 && e.0 != keep {
                    e.1 = 0;
                }
            }
        }
        if top {
            while let Some((id, kind)) = self.finished.first() {
                self.signal = (id, kind);
                self.flags[flag::SIGNAL] = true;
                self.finished.remove(id);
                self.check(false, scripts, world);
            }
        }
        if self.delayed {
            self.check(true, scripts, world);
        }
        if top {
            while let Some((id, _)) = self.starting.first() {
                self.buckets[bucket(id)].add(id, 1);
                self.starting.remove_first();
                self.check(false, scripts, world);
            }
            while let Some((t, d1)) = self.specials.first() {
                self.specials.remove_first();
                self.flags[flag::SPECIAL] = true;
                self.special[0] = t;
                self.special[1] = d1;
                self.check(false, scripts, world);
            }
        }
        self.skip_first = 0;
        self.depth -= 1;
        self.locked = false;
    }

    /// `st_CheckAddShip()`: a fleet `st_fNewEx` made and no script filled gets one ship of design 3.
    fn check_add_ship(&mut self, world: &mut dyn StoryWorld) {
        if self.last_addship != 0 {
            self.f_add_ship(self.last_addship, 3, 1, world);
            self.last_addship = 0;
        }
    }

    // ---------------------------------------------------------------- story fleets (spec §8.6)
    /// `st_fNewEx(name, owner, id, speed, mode, param)` (exe 0x604bc0): a new story fleet. Where it appears:
    /// `mode < 0` around planet `−mode` (radius `param·0.7 ..= param·1.5`, default 20); 1 around a planet
    /// (`param >> 16`, else a random planet of the human's, re-drawn while the point is inside a radar range,
    /// at most 1000 tries, the given planet for the first 99) at its radar range + `(param & 0xffff)·0.7 ..
    /// ·1.5`; 2 on planet `param` (else a random planet of the owner's); 4 on fleet `param`; 3 on a circle of
    /// 350 round the centre; anything else at (0, 0). The fleet gets `AutoAddship`'s ships; `last_addship`
    /// remembers it (one ship of design 3 at the end of the check unless the script adds ships).
    #[allow(clippy::too_many_arguments)]
    pub fn f_new_ex(&mut self, name: &str, owner: i32, id: i32, speed: f32, mode: i32, param: i32, w: &mut dyn StoryWorld) -> i32 {
        if self.last_addship != 0 {
            self.f_add_ship(self.last_addship, 3, 1, w);
        }
        self.last_addship = id;
        // `(q · 3.141592) / 180.0` in f64, as the exe computes it (its constant, not π).
        #[allow(clippy::approx_constant)]
        let rad = |q: i32| q as f64 * 3.141592 / 180.0;
        let (mut x, mut y) = (0, 0);
        if mode < 0 {
            let reach = if param == 0 { 20 } else { param } as f64;
            self.last_fn[last_fn::B] = mode.wrapping_neg();
            let r = w.rng().range(STREAM, (reach * 0.7) as i32, (reach * 1.5) as i32);
            self.last_fn[last_fn::R] = r;
            let q = w.rng().range(STREAM, 1, 360);
            self.last_fn[last_fn::Q] = q;
            let a = rad(q);
            let (px, py) = w.planet_xy(mode.wrapping_neg());
            x = (a.cos() * r as f64 + px as f64) as i32;
            self.last_fn[last_fn::X] = x;
            y = (a.sin() * r as f64 + py as f64) as i32;
            self.last_fn[last_fn::Y] = y;
        } else if mode == 1 {
            let reach = match param & 0xffff {
                0 => 20,
                r => r,
            } as f64;
            let (near, far) = (reach * 0.7, reach * 1.5);
            let mut given = (param as u32 >> 16) as i32;
            let mut tries = 1000;
            let mut fixed = 100;
            loop {
                tries -= 1;
                fixed -= 1;
                if fixed == 0 {
                    given = 0;
                }
                let p = if given != 0 { given } else { self.b_search(w.human().wrapping_neg(), 0, w) };
                self.last_fn[last_fn::B] = p;
                let range = w.radar_range(w.planet_owner(p), w.planet_radar(p)) as f64;
                let r = w.rng().range(STREAM, (range + near) as i32, (range + far) as i32);
                self.last_fn[last_fn::R] = r;
                let q = w.rng().range(STREAM, 1, 360);
                self.last_fn[last_fn::Q] = q;
                let a = rad(q);
                let (px, py) = w.planet_xy(p);
                x = (a.cos() * r as f64 + px as f64) as i32;
                self.last_fn[last_fn::X] = x;
                y = (a.sin() * r as f64 + py as f64) as i32;
                self.last_fn[last_fn::Y] = y;
                if tries == 0 || !w.in_radar_range(x, y, 3) {
                    break;
                }
            }
        } else if mode == 2 {
            let p = if param == 0 { self.b_search(owner.wrapping_neg(), 0, w) } else { param };
            self.last_fn[last_fn::B] = p;
            self.last_fn[last_fn::Q] = 0;
            self.last_fn[last_fn::R] = 0;
            let (px, py) = w.planet_xy(p);
            (x, y) = (px as i32, py as i32);
            self.last_fn[last_fn::X] = x;
            self.last_fn[last_fn::Y] = y;
        } else if mode == 4 {
            self.last_fn[last_fn::B] = param;
            self.last_fn[last_fn::Q] = 0;
            self.last_fn[last_fn::R] = 0;
            let (fx, fy) = w.fleet_xy(param);
            (x, y) = (fx as i32, fy as i32);
            self.last_fn[last_fn::X] = x;
            self.last_fn[last_fn::Y] = y;
        } else if mode == 3 {
            self.last_fn[last_fn::R] = 350;
            let q = w.rng().range(STREAM, 0, 359);
            self.last_fn[last_fn::Q] = q;
            let a = rad(q);
            x = (a.cos() * 350.0) as i32;
            self.last_fn[last_fn::X] = x;
            y = (a.sin() * 350.0) as i32;
            self.last_fn[last_fn::Y] = y;
        }
        let speed = if speed > 1.0 { speed } else { speed * 100.0 };
        let f = w.new_fleet(name, owner, id, x as f32, y as f32, speed);
        self.auto_add_ship(f, id, w);
        f
    }

    /// `st_fAddShip(id, design, count)` (exe 0x604410): `count` ships (or one `TankNew` of `count` for a
    /// ground design) of the owner's `design` for fleet `id` (`< 1`: the slot `−id`). Skipped for the
    /// fleet `AutoAddship` filled; clears `last_addship`.
    pub fn f_add_ship(&mut self, id: i32, design: i32, count: i32, w: &mut dyn StoryWorld) {
        if self.disable_add_ship == id {
            return;
        }
        self.last_addship = 0;
        let f = if id < 1 { id.wrapping_neg() } else { self.f_search(id, w) };
        if f < 1 {
            self.error = 1;
            return;
        }
        if w.design_is_ground(w.fleet_owner(f), design) {
            w.event(StoryEvent::NewTanks { fleet: f, design, count });
        } else {
            for _ in 0..count {
                w.event(StoryEvent::NewShip { fleet: f, design });
            }
        }
        self.error = 0;
    }

    /// `st_fAddTank(id, design, count)`: `TankNew` for fleet `id`; `last_addship` stays.
    pub fn f_add_tank(&mut self, id: i32, design: i32, count: i32, w: &mut dyn StoryWorld) {
        if self.disable_add_ship == id {
            return;
        }
        let f = if id < 1 { id.wrapping_neg() } else { self.f_search(id, w) };
        if f < 1 {
            self.error = 1;
            return;
        }
        w.event(StoryEvent::NewTanks { fleet: f, design, count });
        self.error = 0;
    }

    /// `GetRndStoryLevel()`: 1..=9 by the human's planet count (thresholds 2, 4, 6, 9, 12, 15, 19, 24).
    pub fn story_level(world: &dyn StoryWorld) -> usize {
        const STEPS: [i32; 8] = [2, 4, 6, 9, 12, 15, 19, 24];
        let n = world.planets_of(world.human());
        1 + STEPS.iter().take_while(|&&t| n >= t).count()
    }

    /// `AutoAddship(f, id)` (exe 0x5c1d80): the ships `flotta.txt` lists for fleet `id` (records of any
    /// story or of a running one), counts by difficulty and the human's size; marks `DisableAddShip`.
    pub fn auto_add_ship(&mut self, f: i32, id: i32, w: &mut dyn StoryWorld) {
        self.disable_add_ship = 0;
        let level = Self::story_level(&*w);
        let difficulty = w.difficulty();
        let mut any = false;
        for k in 0..self.fleet_data.len() {
            let rec = self.fleet_data[k].clone();
            if rec.fleet != id || (rec.story != 0 && !self.is_running(rec.story)) {
                continue;
            }
            let design = if rec.design < 1 { rec.design.wrapping_neg() } else { w.story_design(w.fleet_owner(f), rec.design, 3) };
            let count = rec.counts.get((difficulty - 1) as usize).map_or(0, |c| c[level - 1]);
            self.f_add_ship(f.wrapping_neg(), design, count as i32, w);
            any = true;
        }
        if any {
            self.disable_add_ship = id;
        }
    }

    /// `Story_Special(type, d1..d7)`.
    pub fn story_special(&mut self, data: [i32; 8], scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.special = data;
        self.flags[flag::SPECIAL] = true;
        self.check(true, scripts, world);
    }

    /// `Story_FleetArrivedSave(f)`: `order` is the fleet's target type `+0x42`, `target` its `(int) +0x44`.
    pub fn fleet_arrived_save(&mut self, f: i32, order: i32, target: i32, world: &dyn StoryWorld) {
        if !(0..=2).contains(&order) {
            return;
        }
        self.arrival = Arrival {
            fleet_id: world.fleet_id(f),
            fleet: f,
            target,
            target_type: order,
            target_fleet_id: if order == 2 { world.fleet_id(target) } else { 0 },
        };
        self.flags[flag::ARRIVED] = true;
    }

    /// `Story_FleetArrivedCheck()`.
    pub fn fleet_arrived_check(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.flags[flag::ARRIVED] {
            self.check(true, scripts, world);
        }
    }

    /// `Story_SpaceBattleFinished(list, n, check)`.
    pub fn space_battle_finished(&mut self, list: &[Participant], check: bool, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        let en = world.human();
        self.you_were_in_battle = list.iter().any(|p| p.owner == en);
        self.battle = list.to_vec();
        self.flags[flag::SPACE_END] = true;
        self.set_adat_i32(adat::BATTLE_88, 0);
        if check {
            self.check(true, scripts, world);
        }
    }

    /// `Story_SpaceBattleFinishedCheck()`.
    pub fn space_battle_finished_check(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.flags[flag::SPACE_END] {
            self.set_adat_i32(adat::BATTLE_88, 0);
            self.check(true, scripts, world);
        }
    }

    /// `Story_SpaceBattleStarted()` with the two fleets of `spFlottak` (exe 0xBAA1C6 / 0xBAA1CC).
    pub fn space_battle_started(&mut self, a: i32, b: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.space_forbidden = false;
        self.space_simulate_forbidden = false;
        for off in [0x88, adat::NO_RETREAT_94, adat::NO_RETREAT_98, 0x7c, 0x84] {
            self.set_adat_i32(off, 0);
        }
        for e in 1..19 {
            self.battle_pact[e] = 0;
            self.battle_retreat[e] = 0;
        }
        self.zachkin = 0;
        // Story 108: the scripted chase of fleet 0x43a by 0x439.
        if self.l_is_true(0x439) && world.fleet_id(a) == 0x43a && world.fleet_id(b) == 0x439 {
            let f = self.f_search(0x439, world);
            world.event(StoryEvent::StopFleet(f));
            let g = self.f_search(0x43a, world);
            world.event(StoryEvent::StopFleet(g));
            self.arrival.fleet_id = 0x43a;
            self.arrival.fleet = self.f_search(0x43a, world);
            self.arrival.target = self.f_search(0x439, world);
            self.arrival.target_type = 2;
            self.flags[flag::ARRIVED] = true;
            self.space_forbidden = true;
            return;
        }
        if self.running(0x6c) && self.l_is_false(0x439) && world.fleet_owner(a) == world.human() && world.fleet_id(b) == 0x43a {
            let f = self.f_search(0x439, world);
            if f != 0 {
                world.event(StoryEvent::JoinBattle(f));
                self.battle_pact[9] = 1;
                self.set_adat_i32(adat::NO_RETREAT_94, 1);
                self.set_adat_i32(adat::NO_RETREAT_98, 1);
            }
            return;
        }
        self.flags[flag::SPACE_START] = true;
        self.check(true, scripts, world);
    }

    /// `Story_GroundBattleSimulateCheck(f, p)` (exe 0x5c8e70): `Story_GrWarSimulateTiltas` is cleared, then set
    /// when a running story keeps planet `p` for the real-time war: 0x328 (planet 0x1F90), 0x32E (0x1FCC), 0x338
    /// (0x2031, while variable 0x2034 is 0). Returns it.
    pub fn ground_battle_simulate_check(&mut self, p: i32, world: &mut dyn StoryWorld) -> bool {
        self.ground_simulate_forbidden = false;
        for (story, planet) in [(0x328, 0x1f90), (0x32e, 0x1fcc), (0x338, 0x2031)] {
            if self.running(story) && self.b_search(planet, 0, world) == p && (story != 0x338 || self.v_get(0x2034) == 0) {
                self.ground_simulate_forbidden = true;
            }
        }
        self.ground_simulate_forbidden
    }

    /// `Story_GroundBattleStarted(f, p)`; `owner` = the planet's owner.
    pub fn ground_battle_started(&mut self, f: i32, p: i32, owner: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.adat_i32(0x54) != 0 {
            return;
        }
        if f == 0 || world.fleet_owner(f) == 0xff {
            self.ground.fleet = 0;
            self.ground.fleet_id = 0;
            self.ground.fleet_owner = 0;
        } else {
            self.ground.fleet = f;
            self.ground.fleet_id = world.fleet_id(f);
            self.ground.fleet_owner = world.fleet_owner(f);
        }
        self.set_adat_i32(0xf4, owner);
        self.set_adat_i32(0x40, 0);
        self.ground_forbidden = false;
        self.ground_simulate_forbidden = false;
        self.flags[flag::GROUND_START] = true;
        self.ground.planet = p;
        self.check(true, scripts, world);
    }

    /// `Story_GroundBattleFinishedSave(f, p, won)`; `owner` = the planet's owner now.
    pub fn ground_battle_finished_save(&mut self, f: i32, p: i32, owner: i32, attacker_won: bool, world: &dyn StoryWorld) {
        self.ground.fleet = f;
        if f == 0 || world.fleet_owner(f) == 0xff {
            self.ground.fleet_id = 0;
            self.ground.fleet_owner = 0;
        } else {
            self.ground.fleet_id = world.fleet_id(f);
            self.ground.fleet_owner = world.fleet_owner(f);
        }
        self.ground.planet = p;
        self.ground.old_owner = owner;
        self.flags[flag::GROUND_END] = true;
        self.ground.attacker_won = attacker_won;
        self.ground.defender_won = !attacker_won;
        self.set_adat_i32(adat::BATTLE_88, 0);
    }

    /// `Story_GroundBattleFinishedCheck()`: `buildings` = the planet's building count (`HanyEpulet`),
    /// `owner` its owner now.
    pub fn ground_battle_finished_check(&mut self, buildings: i32, owner: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if !self.flags[flag::GROUND_END] {
            return;
        }
        if buildings == 0 {
            self.ground.attacker_won = true;
            self.ground.old_owner = self.adat_i32(0xf4);
        } else if self.ground.attacker_won && owner == self.ground.old_owner {
            self.flags[flag::GROUND_END] = false;
            return;
        }
        self.check(true, scripts, world);
    }

    /// `Story_ArrivedToRadarRangeFleet(f)`.
    pub fn radar_fleet(&mut self, f: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.flags[flag::RADAR_FLEET] = true;
        self.radar.1 = f;
        self.check(true, scripts, world);
    }

    /// `Story_ArrivedToRadarRangeBolygo(p)`.
    pub fn radar_planet(&mut self, p: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.flags[flag::RADAR_PLANET] = true;
        self.radar.0 = p;
        self.check(true, scripts, world);
    }

    /// `Story_Diplomacy_Special(which, e)`.
    pub fn diplomacy_special(&mut self, which: i32, e: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.diplomacy = (which, e, 0);
        self.flags[flag::DIPLOMACY] = true;
        self.check(true, scripts, world);
    }

    /// `Story_KemEsemeny()` (SP only).
    pub fn spy_event(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if world.multiplayer() {
            return;
        }
        self.flags[flag::SPY] = true;
        self.spy_kind = 3;
        self.check(true, scripts, world);
    }
}

// ---------------------------------------------------------------- the scenario stories (§6)

/// `st_Scen_include` (exe 0x601580): stories 360..375.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScenarioStories;

/// The human wins: `WIN(0, 1)` + `MEGNYERES(1)`.
fn victory(s: &mut Story, w: &mut dyn StoryWorld) {
    s.win(0, true, w);
    w.event(StoryEvent::Won);
}

/// `DIE()`: `MEGHALAS(EN)`.
fn defeat(w: &mut dyn StoryWorld) {
    let en = w.human();
    w.event(StoryEvent::EmpireDies(en));
}

impl Script for ScenarioStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        let en = w.human();
        // 360: the fleet with id 3601 (or slot 0) is owned by anyone but empire 0.
        if s.running(360) && s.daily() {
            let f = s.f_search(3601, w);
            if w.fleet_owner(f) != 0 {
                victory(s, w);
            }
        }
        // 361: every listed empire is dead.
        if s.running(361) && s.daily() {
            let mut all_dead = true;
            for k in 1..8 {
                let e = s.v_get(3610 + k);
                if e != 0 && w.empire_alive(e) {
                    all_dead = false;
                }
            }
            if all_dead {
                victory(s, w);
            }
        }
        // 362: capture planet 3621.
        if s.running(362) && s.daily() {
            let p = s.b_search(3621, 0, w);
            if w.planet_owner(p) == en {
                victory(s, w);
            }
        }
        // 363: destroy fleet 3631.
        if s.running(363) && s.daily() && s.f_search(3631, w) == 0 {
            victory(s, w);
        }
        // 364: reach planet 3641 with one of the human's fleets, or own it.
        if s.running(364) {
            if s.flags[flag::ARRIVED] && s.arrival.target_type == 1 {
                let p = s.b_search(3641, 0, w);
                if p == s.arrival.target && w.fleet_owner(s.arrival.fleet) == en {
                    victory(s, w);
                }
            }
            if s.daily() {
                let p = s.b_search(3641, 0, w);
                if w.planet_owner(p) == en {
                    victory(s, w);
                }
            }
        }
        // 365: find planet 3651.
        if s.running(365) && s.daily() {
            let p = s.b_search(3651, 0, w);
            if w.planet_visibility(p) > 0 {
                victory(s, w);
            }
        }
        // 366: survive v(3661) days (no MEGNYERES).
        if s.running(366) {
            if s.starting_now() {
                let d = s.v_get(3661);
                s.sz_start(3661, d);
            }
            if s.counter(3661) {
                s.win(0, true, w);
            }
            if s.daily() {
                s.sz_get(3661);
            }
        }
        // 367: more money than v(3671).
        if s.running(367) && s.daily() {
            let money = w.money(en);
            if (s.v_get(3671) as f64) < money {
                victory(s, w);
            }
        }
        // 368: more planets than v(3681).
        if s.running(368) && s.daily() && s.v_get(3681) < w.planets_of(en) {
            victory(s, w);
        }
        // 369: own planet 3691 with more than v(3691) (+0x28).
        if s.running(369) && s.daily() {
            let p = s.b_search(3691, 0, w);
            if w.planet_owner(p) == en {
                let p = s.b_search(3691, 0, w);
                if s.v_get(3691) < w.planet_value(p) {
                    victory(s, w);
                }
            }
        }
        // 370: protect empire v(3701).
        if s.running(370) && s.daily() {
            let e = s.v_get(3701);
            if !w.empire_alive(e) {
                defeat(w);
            }
        }
        // 371: a deadline of v(3711) days, shown as an info line.
        if s.running(371) {
            if s.starting_now() {
                let d = s.v_get(3711);
                s.sz_start(3711, d);
                s.v_set(3712, 0);
            }
            if s.counter(3711) {
                defeat(w);
            }
            if s.daily() {
                let shown = s.v_get(3712);
                if shown != 0 {
                    s.remove_game_info(shown + 3710, w);
                }
                let d = s.sz_get(3711);
                let (id, value, mark) = if d >= 361 {
                    (3711, (d + 359) / 360, 1)
                } else if d >= 31 {
                    (3712, (d + 29) / 30, 2)
                } else {
                    (3713, d, 3)
                };
                if d >= 2 {
                    s.add_game_info(id, value, -1, w);
                    s.v_set(3712, mark);
                }
            }
        }
        // 372: fleet 3721 must fall in a space battle; a battle it survives kills the human.
        if s.running(372) {
            if s.flags[flag::SPACE_START] {
                s.set_adat_i32(adat::NO_RETREAT_94, 1);
                s.set_adat_i32(adat::NO_RETREAT_98, 1);
            }
            if s.battle_part(0) != 0 {
                if s.f_search(3721, w) == 0 {
                    victory(s, w);
                } else {
                    defeat(w);
                }
            }
        }
        // 373: empire v(3731) must not develop tech v(3732).
        if s.running(373) && s.daily() {
            let tech = s.v_get(3732);
            let e = s.v_get(3731);
            if w.tech_status(tech, e) == 5 {
                defeat(w);
            }
        }
        // 374: ally with one of the listed empires.
        if s.running(374) && s.daily() {
            let mut allied = false;
            for k in 1..8 {
                let e = s.v_get(3740 + k);
                if e != 0 {
                    let e = s.v_get(3740 + k);
                    if w.relation_level(en, e) == 5 {
                        allied = true;
                    }
                }
            }
            if allied {
                victory(s, w);
            }
        }
        // 375: wipe out empire v(3751) (no planet the human can see) with more planets than v(3751).
        if s.running(375) && s.daily() {
            let e = s.v_get(3751);
            if s.b_search(e.wrapping_neg(), 1, w) == 0 {
                if s.v_get(3751) < w.planets_of(en) {
                    victory(s, w);
                } else {
                    defeat(w);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct W {
        ido: i32,
        speed: i32,
        rng: Streams,
        owners: Vec<i32>,
        vis: Vec<i32>,
        fleets: Vec<(i32, i32)>,
        alive: [bool; 19],
        events: Vec<StoryEvent>,
    }

    impl StoryWorld for W {
        fn ido(&self) -> i32 {
            self.ido
        }
        fn speed_mult(&self) -> i32 {
            self.speed
        }
        fn human(&self) -> i32 {
            1
        }
        fn difficulty(&self) -> i32 {
            1
        }
        fn multiplayer(&self) -> bool {
            false
        }
        fn rng(&mut self) -> &mut Streams {
            &mut self.rng
        }
        fn planet_count(&self) -> i32 {
            self.owners.len() as i32 - 1
        }
        fn planet_owner(&self, p: i32) -> i32 {
            self.owners[p as usize]
        }
        fn planet_value(&self, _p: i32) -> i32 {
            0
        }
        fn planet_visibility(&self, p: i32) -> i32 {
            self.vis[p as usize]
        }
        fn fleet_count(&self) -> i32 {
            self.fleets.len() as i32 - 1
        }
        fn fleet_owner(&self, f: i32) -> i32 {
            self.fleets[f as usize].0
        }
        fn fleet_id(&self, f: i32) -> i32 {
            self.fleets[f as usize].1
        }
        fn empire_alive(&self, e: i32) -> bool {
            self.alive.get(e as usize).copied().unwrap_or(false)
        }
        fn planets_of(&self, _e: i32) -> i32 {
            3
        }
        fn money(&self, _e: i32) -> f64 {
            1000.0
        }
        fn tech_status(&self, _t: i32, _e: i32) -> i32 {
            0
        }
        fn relation_level(&self, _a: i32, _b: i32) -> i32 {
            3
        }
        fn event(&mut self, ev: StoryEvent) {
            self.events.push(ev);
        }
    }

    fn world() -> W {
        W { speed: 1, owners: vec![0, 1, 2, 2, 0], vis: vec![0, 1000, 1, -1, 1], fleets: vec![(0, 0), (1, 7), (2, 9), (0xff, 9)], ..W::default() }
    }

    #[test]
    fn queue_remove_matches_negated_ids() {
        let mut q = Queue::default();
        q.add(5, 1);
        q.add(-7, 2);
        q.add(7, 3);
        q.remove(7);
        assert_eq!(q.entries, vec![(5, 1), (7, 3)]);
        assert_eq!(q.value(7), 3);
        assert_eq!(q.value(8), 0);
    }

    #[test]
    fn variables_set_error_and_cap() {
        let mut s = Story::default();
        assert_eq!(s.v_get(3), 0);
        assert_eq!(s.error, 1);
        s.v_set(3, 9);
        s.v_add(3, 2);
        assert_eq!(s.v_get(3), 11);
        assert_eq!(s.error, 0);
        s.v_add(4, 1); // missing: no new entry
        assert_eq!((s.vars.count, s.error), (1, 1));
        for i in 0..600 {
            s.v_set(100 + i, i);
        }
        assert_eq!(s.vars.count, MAX_VARS);
        assert_eq!(s.error, 1);
        assert!(s.l_is_false(1));
        s.l_set(1, true);
        assert!(s.l_is_true(1));
    }

    #[test]
    fn counter_stop_keeps_a_stale_id() {
        let mut s = Story::default();
        s.sz_start(10, 2);
        s.sz_start(11, 1);
        s.counters.count = 1; // as after a new game: slot 2 still holds (11, 50)
        s.sz_stop(99);
        assert_eq!(s.counters.slots[2], (11, -1));
        assert_eq!(s.sz_get(10), 2);
        assert!(!s.sz_is_ever_started(99));
    }

    #[test]
    fn counters_fire_and_daily_needs_more_than_fifty() {
        let mut s = Story::default();
        let mut w = world();
        w.speed = 16;
        s.sz_start(3661, 1);
        s.old_ido = 0;
        w.ido = 50;
        s.tick(&ScenarioStories, &mut w);
        assert_eq!(s.counters.live(), &[(3661, 34)]);
        assert_eq!(s.old_ido, 0, "Ido 50 is not more than OldIdo + 50");
        w.ido = 51;
        for _ in 0..3 {
            s.tick(&ScenarioStories, &mut w);
        }
        assert_eq!(s.counters.live(), &[(3661, -1)]);
        assert_eq!(s.counter_done, 3661);
        assert_eq!(s.old_ido, 51);
        assert!(s.flags.iter().all(|&f| !f));
    }

    #[test]
    fn started_story_runs_its_start_once_and_wins() {
        let mut s = Story::default();
        let mut w = world();
        s.v_set(3661, 1);
        s.start_story(366, false);
        s.check(true, &ScenarioStories, &mut w);
        assert!(s.is_running(366));
        assert_eq!(s.buckets[366 & 0xff].entries, vec![(366, 0)]);
        assert_eq!(s.counters.live(), &[(3661, 50)]);
        w.ido = 120;
        for _ in 0..50 {
            s.tick(&ScenarioStories, &mut w);
        }
        assert!(!s.is_running(366));
        assert_eq!(s.history.entries, vec![(366, 120)]);
        assert!(s.finished.is_empty(), "the signal is consumed by the nested check");
        assert_eq!(s.signal, (366, 1));
        assert!(w.events.is_empty(), "366 only finishes");
    }

    #[test]
    fn win_applies_story_undo_entries() {
        let mut s = Story::default();
        let mut w = world();
        s.set_adat_i32(0x24, 7);
        s.set_adat_i32(0xd0 + 8, 2);
        s.add_undo(15, 0, 370);
        s.add_undo(13, 2, 370);
        s.add_undo(4, 3701, 371);
        s.add_undo(2, 0x30001, 370);
        s.win(370, true, &mut w);
        assert_eq!(s.adat_i32(0x24), 0);
        assert_eq!(s.adat_i32(0xd8), 1);
        assert_eq!(s.undo, vec![(0, 0, 0), (0, 0, 0), (4, 3701, 371)]);
        assert_eq!(w.events, vec![StoryEvent::Undo { kind: 2, value: 0x30001 }]);
        assert_eq!(s.finished.entries, vec![(370, 1)]);
        s.win(370, true, &mut w);
        assert_eq!(s.history.entries.len(), 1);
        assert_eq!(s.finished.entries.len(), 2);
    }

    #[test]
    fn planet_and_fleet_searches() {
        let mut s = Story::default();
        let mut w = world();
        s.b_add_id(3, 3621);
        s.b_add_id(0, 3622);
        assert_eq!(s.b_search(3621, 0, &mut w), 3);
        assert_eq!(s.b_search(3622, 0, &mut w), 0);
        // Empire 2 owns planets 2 and 3; the human sees only 2.
        let before = w.rng.state(STREAM);
        assert_eq!(s.b_search(-2, 1, &mut w), 2);
        assert_ne!(w.rng.state(STREAM), before);
        // No planet: still one draw.
        let before = w.rng.state(STREAM);
        assert_eq!(s.b_search(-5, 1, &mut w), 0);
        assert_eq!(w.rng.state(STREAM), crate::rng::step(before));
        assert_eq!(s.f_search(9, &mut w), 2);
        assert_eq!(s.f_search(0, &mut w), 1);
    }

    #[test]
    fn scenario_361_needs_every_listed_empire_dead() {
        let mut s = Story::default();
        let mut w = world();
        w.alive[3] = true;
        s.v_set(3611, 3);
        s.v_set(3612, 0);
        s.start_story(361, true);
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert!(w.events.is_empty());
        w.alive[3] = false;
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(w.events, vec![StoryEvent::Won]);
        assert!(!s.is_running(361));
    }

    #[test]
    fn scenario_371_countdown_info_lines() {
        let mut s = Story::default();
        let mut w = world();
        s.v_set(3711, 400);
        s.start_story(371, true);
        s.check(true, &ScenarioStories, &mut w);
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(w.events, vec![StoryEvent::GameInfo { id: 3711, value: 2 }]);
        assert_eq!(s.undo, vec![(4, 3711, 371)]);
        assert_eq!(s.vars.get(3712), Some(1));
        s.counters.slots[1].1 = 40 * DAY;
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(&w.events[1..], &[StoryEvent::RemoveGameInfo(3711), StoryEvent::GameInfo { id: 3712, value: 2 }]);
        assert_eq!(s.undo, vec![(4, 3712, 371)]);
    }

    #[test]
    fn epidemic_switch_clears_when_nothing_is_infected() {
        let mut s = Story::default();
        let mut w = world();
        s.set_adat_i32(adat::EPIDEMIC, 1);
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(s.adat_i32(adat::EPIDEMIC), 1, "only on the daily check");
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(s.adat_i32(adat::EPIDEMIC), 0);
        assert_eq!(w.events, vec![StoryEvent::EpidemicOver]);
    }

    #[test]
    fn get_value_scales_by_difficulty() {
        let values = [(5301, 1600)];
        assert_eq!(Story::get_value(&values, 5301, 1), 3200);
        assert_eq!(Story::get_value(&values, 5301, 2), 2080);
        assert_eq!(Story::get_value(&values, 5301, 4), 1600);
        assert_eq!(Story::get_value(&values, 1, 4), 10);
        assert_eq!(Story::diff(3, 1, 2, 3, 4), 3);
    }
}
