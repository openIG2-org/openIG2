//! One sim tick of the whole strategic game, the systems in the exe's order (`re/rules/time-and-rng.md` §1), on a
//! [`SaveState`]:
//!
//! ```text
//! FRISSIT_Datum                       Game::advance_clock
//! if IdoNapValtozott:                 Game::run_tick from here
//!     Nap % 6 == 1: FRISSIT_Planet    colony_side::ColonyWorld::economy_step
//!     FRISSIT_Viszony                 diplomacy::Diplomacy::daily, its deaths Game::empire_dies (game::death)
//!     FRISSIT_MI                      ai::AiState::daily
//!     FRISSIT_Spy                     spy::Spies::daily
//!     MI_DIPLOMACY                    diplomacy::Diplomacy::ai_messages
//! FRISSIT_Trade                       trade::Trade::tick (deliveries, merchants, expiry)
//! FRISSIT_Info                        info::Charts::sample (on a new month)
//! FRISSIT_Research                    research::tick
//! FRISSIT_Story                       story::Story::tick (the scenario stories)
//! FRISSIT_Messages                    messages::Inbox::expire
//! FRISSIT_Diplomacy                   diplomacy::Diplomacy::tick
//! FRISSIT_Epuletek                    Game::buildings_tick (repairs, refresh of planets at war)
//! FRISSIT_Starmap                     fleet::Starmap::tick (colonisation arrivals), then production::Shipyard::tick
//! MegnyeresEllenoriz                  Game::victory_check (game::death)
//! IdoMultiPlayerBattle += 1
//! ```
//!
//! A purchase from the trade screen (`MULTIGET_Trade`, through the lockstep queue) is [`Game::buy`].
//!
//! The messages the systems post to the human's scrolling line (`ScrollUzenetId`) are posted from the tick's
//! events at its end, in event order ([`Game::post_messages`], `re/rules/info-messages.md` §3).
//!
//! Not ported, so skipped: `FRISSIT_Design` (empty in the exe),
//! the ground and space battles' real-time ticks. Their effects on the other systems (a fleet
//! stopped by a story, a war the AI declares) are reported as [`GameEvent`]s, and the few the replay needs are
//! carried out here. Each system decodes its state from the [`SaveState`] and stores it back, so systems see each
//! other's writes as in the exe.

mod battles;
mod colony_orders;
mod death;
mod design_orders;
mod info_orders;
mod message_orders;
mod orders;
mod realtime;
mod research_orders;
mod tutorial;
pub mod new_game;

pub use colony_orders::{repair_price, ColonyOrder, ColonyOrderEvent};
pub use design_orders::{DesignOrder, DesignRecord};
pub use info_orders::InfoOrder;
pub use message_orders::{choice_disabled, dialogue_in_progress, MessageOrder};
pub use orders::{OrderEvent, StarmapOrder};
pub use realtime::{RealTime, RealTimeEvent, SpBattles};
pub use research_orders::ResearchOrder;

pub use battles::BattleEvent;
pub use death::{GameOver, LAST_ENEMY_STORY};

use crate::ai::{self, AiContext, AiHooks, AiRules};
use crate::colony::{ColonyEvent, EconomyContext, Planet, StoryHooks};
use crate::colony_side::{ColonyArrivals, SideEvent, SideRules, EMPIRES};
use crate::ground;
use crate::diplomacy::{self, DiploEvent, DiploPlanet, Diplomacy, DiplomacyContext, DiplomacyRules};
use crate::fleet::{flag, order, Arrivals, Wormhole, StarPlanet, Starmap, StarmapContext, StarmapEvent, DELETED};
use crate::messages::{self, cstr, Texts};
use crate::placement::PlacementRules;
use crate::production::{ProductionContext, ProductionEvent, Queue, Shipyard, YardFleet, YardPlanet};
use crate::radar::RadarRules;
use crate::research::{self, ResearchContext, ResearchEvent, ResearchRules};
use crate::rng::Streams;
use crate::save::{fleet_totals_put, put_fleet, put_wormhole, va, HumanSide, SaveState, PLANET_SIZE};
use crate::spacewar::{CombatFleet, CombatWorld, Ship, Vehicle, NO_DESIGN};
use crate::spy::{self, SpyContext, SpyEvent, SpyRules, StoryCheck};
use crate::story::{ScenarioStories, StoryEvent, StoryWorld};
use crate::time::GameTime;
use crate::trade::{self, Delivery, Market, MarketContext, TradeEvent};

/// The rule tables every system reads (from the user's exe and install, ADR 0004).
#[derive(Debug, Clone, Default)]
pub struct GameRules {
    pub side: SideRules,
    pub placement: PlacementRules,
    pub research: ResearchRules,
    pub diplomacy: DiplomacyRules,
    pub spy: SpyRules,
    pub ai: AiRules,
    pub radar: RadarRules,
    /// `NehezsegGyartasSzorzo` / `NehezsegPenzSzorzo` by difficulty.
    pub difficulty_production: Vec<f32>,
    pub difficulty_money: Vec<f32>,
    /// `uzenet.txt` in the game's language (the scrolling messages' texts).
    pub messages: Texts,
    /// The auto-resolved space battle's planet guns and the ground war (`SimulateSpBattle`, `WF::Simulate`).
    pub guns: crate::spacewar::GunRules,
    pub ground: crate::ground::GroundRules,
    /// `terepleiro.txt`: each `[felszin]`'s weathers (by colony map) and each `[hatter]`'s (the battlefields'
    /// backgrounds, `ground::pick_background`).
    pub surface_weathers: Vec<Vec<i32>>,
    pub backgrounds: Vec<Vec<i32>>,
    /// The colony maps (by `Bolygok +0x23`) for placing vehicles on a battlefield outside the real-time battle
    /// (`LocateTankSuitablePosition`; heights for the units' `+0x1dc`).
    pub ground_maps: Vec<ground::GroundMap>,
    /// The real-time space battle (weapons, projectile forms, formations, ship types; multiplayer only so far).
    pub space: crate::spacebattle::SpaceRules,
    /// `MuholdAr` (exe 0x9138D0): satellite prices by kind (`MULTIGET_Starmap` 0x18).
    pub satellite_prices: Vec<i32>,
    /// `uzenetek.txt`'s kinds and the conversations' `msg%04d.txt` (the messages screen's orders).
    pub inbox: messages::InboxRules,
}

/// The MSVC CRT `rand()` (`holdrand · 214013 + 2531011`, bits 16..30): the unseeded libc draws of the original.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Libc(pub u32);

impl Libc {
    pub fn rand(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
}

/// Where the sim's per-peer view state comes from (ADR 0007 §5, `re/rules/multiplayer.md`): `EN` (the local
/// human), the shown screen and planet (`StarmapMode`/`AktPlanet`) and the selected fleet (`AktFleet`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// The original: the sim reads (and the UI writes) those globals in the state. Single player and every
    /// byte-exact replay.
    #[default]
    Classic,
    /// Lockstep multiplayer: the sim reads canonical values, the same on every peer: `EN` = `en` (the host's
    /// empire), nothing on screen, no selection, and the original's `Multi_Send(.., EN, ..)` goes out for every
    /// human. Each peer's own view is [`Game::local`], which the sim never reads; draws that only pick what the
    /// local player sees come from [`Game::cosmetic`]. Synced state (in the snapshot).
    Canonical { en: u8 },
}

/// One player's view: whose empire they play and what they look at (the original's `EN`, `StarmapMode`,
/// `AktPlanet`, `AktFleet`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LocalView {
    pub empire: u8,
    /// `StarmapMode` (exe 0xB9998C): 1 = a planet's screen is up.
    pub starmap_mode: i32,
    pub planet: i32,
    pub fleet: i32,
}

/// What a tick asked of systems that aren't ported, or reported to the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    Side(SideEvent),
    Diplomacy(DiploEvent),
    Ai(ai::AiEvent),
    Spy(SpyEvent),
    Research(ResearchEvent),
    Story(StoryEvent),
    Starmap(StarmapEvent),
    Production(ProductionEvent),
    Trade(TradeEvent),
    /// The AI declared war (`ElkuldHaduzenet`); carried out by diplomacy.
    WarDeclared { a: u8, b: u8 },
    /// `HajoDelete` emptied the human's own fleet: `Multi_Send(0x1b, EN, f)`. The lockstep queue hands it to
    /// `MULTIGET_Starmap` in a later frame: the driver calls [`Game::fleet_emptied`] before the next tick.
    FleetEmptied { fleet: usize },
    /// A battle the starmap started (`SpaceWarIndit`, `GroundWarIndit`).
    Battle(BattleEvent),
    /// A real-time battle of canonical multiplayer (`game::realtime`).
    RealTime(RealTimeEvent),
    /// The game ends for the human (`MEGHALAS`, `MEGNYERES`; `game::death`).
    GameOver(GameOver),
    /// A starmap order's outcome for the issuer's screen (`game::orders`).
    Order(OrderEvent),
    /// A colony order's outcome for the issuer's screen (`game::colony_orders`).
    ColonyOrder(ColonyOrderEvent),
}

impl GameEvent {
    /// The fleet of a 0x1b message (`GameEvent::FleetEmptied`, or the AI pass's `HumanFleetEmptied`).
    pub fn emptied_fleet(&self) -> Option<usize> {
        match self {
            GameEvent::FleetEmptied { fleet }
            | GameEvent::Ai(ai::AiEvent::HumanFleetEmptied { fleet })
            | GameEvent::Starmap(StarmapEvent::FleetEmptied { fleet }) => Some(*fleet),
            _ => None,
        }
    }
}

/// The game: its state and the rules.
pub struct Game<'r> {
    pub state: SaveState,
    pub rules: &'r GameRules,
    pub libc: Libc,
    /// `AlapFlottaNev` (the localised default fleet name; `None` = the English "Fleet").
    pub fleet_name: Option<Vec<u8>>,
    /// The bytes of records appended to grown arrays before the code writes them (uninitialised heap in the
    /// original): `None` = zero.
    pub uninit: Option<Box<dyn FnMut(Fresh) -> Vec<u8> + 'r>>,
    pub events: Vec<GameEvent>,
    /// `BirodalomNev[e]` (exe 0xBB6730, unsaved, localised): the empire names messages quote.
    pub empire_names: Vec<Vec<u8>>,
    /// `KemCsaladtag_Nev[x]` (exe 0xBB6968, unsaved, localised): the family members the kidnap message names.
    pub family_names: Vec<Vec<u8>>,
    /// Classic (the view in the state) or canonical (multiplayer). Synced.
    pub view: ViewMode,
    /// This peer's view in [`ViewMode::Canonical`] (unused in classic mode, where the state holds it). Per peer:
    /// not in the snapshot or the hash.
    pub local: LocalView,
    /// The per-peer libc stream of [`ViewMode::Canonical`] for draws that only pick what the local player sees or
    /// hears (classic mode draws them from [`Game::libc`], like the exe). Not in the snapshot or the hash.
    pub cosmetic: Libc,
    /// The events up to here had their battlefield side carried out ([`Game::battlefield_hand_offs`]).
    hand_offs: usize,
    /// The real-time battles of canonical multiplayer (`game::realtime`). Synced.
    pub realtime: RealTime,
    /// The single-player battle screen (ours, classic view; not in the state).
    pub sp_battles: SpBattles,
    /// How the game ended (`GameStarted` 0xBAE297 cleared by `ExitToMainScreen`): `MEGHALAS` and `MEGNYERES` end
    /// it only while it is `None` (`game::death`). Not in the state: a loaded game runs again.
    pub over: Option<GameOver>,
}

/// One planet's draws in the economy prologue: `(planet, weather, background)` ([`Game::weather_draws`]).
pub type WeatherDraw = (usize, i32, Option<i32>);

/// An appended record whose untouched bytes [`Game::uninit`] supplies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fresh {
    Fleet(usize),
    /// A ship record (`Hajok[i]`, 6 bytes; `+5` is never written).
    Ship(usize),
    /// A building record appended to planet `planet`'s block (`EpuletNew` reallocates by 20).
    Building { planet: usize, slot: usize },
    Design { empire: usize, design: usize },
    /// A unit object new on planet `planet`'s battlefield at `slot`: [`ground::UNINIT`] fields as `(offset, value)`
    /// pairs ([`ground::Group::fill_uninit`]).
    GroundUnit { planet: usize, slot: usize },
    /// A waypoint record (`Waypointek[i]`, 0xc bytes) the array grew by: record 0 is never written, the others'
    /// `+0xa..+0xc`.
    Waypoint(usize),
}

/// The tick's global inputs read from the state.
struct Globals {
    year: i32,
    month: i32,
    day: i32,
    tick: i32,
    ido2: i32,
    speed: i32,
    multiplayer: bool,
    en: usize,
    /// `ViewMode::Canonical`.
    canonical: bool,
    /// The empires whose machine sends the original's `Multi_Send(.., EN, ..)` ([`ViewMode`]).
    sends: [bool; EMPIRES],
    /// `UAktUzenetFajta` (exe 0xDABDBC, the inbox message on screen); 0 in canonical mode.
    message_shown: i32,
    human: [bool; EMPIRES],
    race: [u8; EMPIRES],
    alive: [bool; EMPIRES],
    difficulty: i32,
    pp: usize,
    pp2: usize,
    all_techs: bool,
    speed_locked: bool,
    program_speed: i32,
    tutorial: bool,
}

fn adat_i32(adat: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(adat[off..off + 4].try_into().unwrap())
}

impl<'r> Game<'r> {
    pub fn new(state: SaveState, rules: &'r GameRules) -> Self {
        Self {
            state,
            rules,
            libc: Libc::default(),
            fleet_name: None,
            uninit: None,
            events: Vec::new(),
            empire_names: Vec::new(),
            family_names: Vec::new(),
            hand_offs: 0,
            view: ViewMode::Classic,
            local: LocalView::default(),
            cosmetic: Libc::default(),
            realtime: RealTime::default(),
            sp_battles: SpBattles::default(),
            over: None,
        }
    }

    /// Switch to the canonical view of a lockstep session whose canonical `EN` is `en` (the host's empire). It's
    /// synced state: every peer sets the same `en` before the first tick (the host sends it with the start
    /// message), and the snapshot carries it. The local view starts as the state's classic one.
    pub fn canonical_view(&mut self, en: u8) {
        if self.view == ViewMode::Classic {
            self.local = self.local_view();
        }
        self.view = ViewMode::Canonical { en };
        self.per_human_copies();
        // `IdoSpeedSzorzoBattle` isn't saved: a loaded game gets it as MP `SetIdoSpeed` (ELF 0x6233d0) sets it with the
        // game speed (×4 → 1, ×8 and ×16 → 2), so every peer's battles step alike (`multiplayer.md` §7).
        let m = &mut self.state.mem;
        if m.i32(va::BATTLE_SPEED) <= 0 {
            m.set_i32(va::BATTLE_SPEED, if m.i32(va::IDO_SPEED) <= 4 { 1 } else { 2 });
        }
    }

    /// Canonical multiplayer: every human but `EN` gets its own contact list and spy report queue
    /// (`re/rules/multiplayer.md` §5): `FelfedezBirodalom(e)` for each empire 1..=8 it has met. Kept if it has one.
    fn per_human_copies(&mut self) {
        let ViewMode::Canonical { en } = self.view else { return };
        let humans = self.state.human_controlled();
        for h in (1..EMPIRES).filter(|&h| humans[h] && h != en as usize) {
            if self.state.humans.contains_key(&(h as u8)) {
                continue;
            }
            self.state.humans.insert(h as u8, HumanSide::default());
            let knows: Vec<usize> = (1..9).filter(|&e| self.state.mem.u8(va::EMPIRE_KNOWS + (h * EMPIRES + e) as u32) != 0).collect();
            self.with_diplomacy(|d, w| {
                for &e in &knows {
                    d.discover_by(w, h, e);
                }
            });
        }
    }

    /// The local player's view: the state's globals (classic), else [`Game::local`].
    /// `GetENRadarKifejlesztve` on the current state, without keeping the result (the starmap's radar ranges call
    /// it first: re/screens/starmap.md §6.3). Rows a loaded save left empty start from `InitNewGame`'s
    /// `RadarHatotav2Init` (the exe's live tables hold them after a load).
    pub fn radar_tables(&self) -> crate::radar::RadarTables {
        let g = self.globals();
        let ctx = self.starmap_context(&g);
        let mut t = self.state.starmap().radar;
        let init = crate::radar::RadarTables::new_game(&self.rules.radar);
        for e in 1..EMPIRES {
            if t.range[e].iter().all(|&r| r == 0.0) {
                (t.range[e], t.range_sq[e]) = (init.range[e], init.range_sq[e]);
            }
        }
        t.refresh(&self.rules.radar, &ctx);
        t
    }

    pub fn local_view(&self) -> LocalView {
        match self.view {
            ViewMode::Classic => {
                let m = &self.state.mem;
                LocalView {
                    empire: m.i32(va::EN).clamp(0, EMPIRES as i32 - 1) as u8,
                    starmap_mode: m.i32(va::STARMAP_MODE),
                    planet: m.i32(va::AKT_PLANET),
                    fleet: m.i32(va::AKT_FLEET),
                }
            }
            ViewMode::Canonical { .. } => self.local,
        }
    }

    /// The UI changes the local view: classic mode writes the original's globals (game state there: the sim reads
    /// them), canonical mode keeps it beside the state.
    pub fn set_view(&mut self, v: LocalView) {
        match self.view {
            ViewMode::Classic => {
                let m = &mut self.state.mem;
                m.set_i32(va::EN, v.empire as i32);
                m.set_i32(va::STARMAP_MODE, v.starmap_mode);
                m.set_i32(va::AKT_PLANET, v.planet);
                m.set_i32(va::AKT_FLEET, v.fleet);
            }
            ViewMode::Canonical { .. } => self.local = v,
        }
    }

    /// The starmap's transfer panel opens between `fleets` (`StarmapTransferMode` exe 0xB9A304,
    /// `StarmapTransferFlotta1/2` 0xB9A62C/0xB9A954; a planet is −p) or closes (`None`): `FRISSIT_Starmap` doesn't
    /// send 0x1b for the panel's emptied fleets while it is up (re/screens/starmap.md §10.2, §13). Classic mode
    /// only: the canonical view keeps no panel.
    pub fn set_transfer(&mut self, fleets: Option<(i32, i32)>) {
        if let ViewMode::Classic = self.view {
            let m = &mut self.state.mem;
            m.set_u8(va::STARMAP_TRANSFER_MODE, fleets.is_some() as u8);
            if let Some((a, b)) = fleets {
                m.set_i32(va::STARMAP_TRANSFER_FLEETS.0, a);
                m.set_i32(va::STARMAP_TRANSFER_FLEETS.1, b);
            }
        }
    }

    /// The planet on screen as the sim sees it (`StarmapMode == 1` → `AktPlanet`); none in canonical mode.
    fn shown_planet(&self) -> Option<i32> {
        let m = &self.state.mem;
        match self.view {
            ViewMode::Classic => (m.i32(va::STARMAP_MODE) == 1).then(|| m.i32(va::AKT_PLANET)),
            ViewMode::Canonical { .. } => None,
        }
    }

    fn globals(&self) -> Globals {
        let m = &self.state.mem;
        let en = match self.view {
            ViewMode::Classic => m.i32(va::EN).clamp(0, EMPIRES as i32 - 1) as usize,
            ViewMode::Canonical { en } => (en as usize).min(EMPIRES - 1),
        };
        let human = self.state.human_controlled();
        let classic = |at: u32| if self.view == ViewMode::Classic { m.i32(at) } else { 0 };
        Globals {
            year: m.i32(va::EV),
            month: m.i32(va::HONAP),
            day: m.i32(va::NAP),
            tick: m.i32(va::IDO),
            ido2: m.i32(va::IDO2),
            speed: m.i32(va::IDO_SPEED),
            multiplayer: m.u8(va::MULTIPLAYER) != 0,
            en,
            canonical: self.view != ViewMode::Classic,
            sends: match self.view {
                ViewMode::Classic => std::array::from_fn(|e| e == en),
                ViewMode::Canonical { .. } => human,
            },
            message_shown: classic(va::INBOX_SHOWING),
            human,
            race: self.state.races(),
            alive: self.state.alive(),
            difficulty: m.i32(va::NEHEZSEG),
            pp: m.i32(va::PROGRAM_PLAYERS).max(0) as usize,
            pp2: m.i32(va::PROGRAM_PLAYERS2).max(0) as usize,
            all_techs: m.i32(va::ALL_TECHS) != 0,
            speed_locked: m.i32(va::SPEED_LOCKED) != 0,
            program_speed: m.i32(va::PROGRAM_SEBESSEG),
            tutorial: m.u8(va::TUTORIAL_ACTIVE) != 0 || m.u8(va::TUTORIAL_PAUSED) != 0,
        }
    }

    fn adat(&self) -> Vec<u8> {
        self.state.mem.read(va::STORY_ADAT, crate::story::ADAT)
    }

    /// `LoadSave`'s epilogue (exe 0x543c80, ELF `save.cpp`), after writing a save as after loading one:
    /// `KemKuldetesek[0x160] = 0`, the battlefields' status caches cleared (`WF::LoadSave`) and `FlottakUjraSzamol()` (every fleet's totals, colony ships, terraformers and
    /// pools, every garrison recounted; `ai-empire.md` §12.9). It changes the state: a colonising fleet's colony ship
    /// has left the pool, and the recount puts it back (`fleet-movement.md` §4.3). `FRISSITES_IGAZIT` is the UI's.
    pub fn saved(&mut self) {
        self.state.mem.set_i32(va::MISSIONS + 0x160, 0);
        self.state.battlefields_saved();
        self.with_ai(|_, w, _| w.recount());
    }

    /// `EpuleteketJoHelyreTesz` (exe 0x53cd00, the last call of `LoadScenario`; `colony-placement.md` §6): on every
    /// owned planet each building in slot order is `EpuletDelete`d and `EpuletNew`ed finished and undamaged at the
    /// first free spot of `LocateSuitableLocation` searched from its old cell (the old cell when there is none). The
    /// new record takes the planet's first empty slot: a building behind a hole moves into it and isn't visited again.
    pub fn refit_buildings(&mut self) {
        let g = self.globals();
        let econ = self.economy_context(&g);
        for p in 1..=self.state.planet_count() {
            let owner = self.state.mem.u8(va::PLANETS + (p * PLANET_SIZE) as u32 + 0x20);
            if owner == 0 {
                continue;
            }
            let mut j = 1;
            while j as i32 <= self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32) {
                let rec = self.state.buildings.get(p).and_then(|b| b.get(j)).map_or([0; 0x20], |r| r[..0x20].try_into().unwrap());
                let kind = rec[0];
                if kind != 0 {
                    let f = |o: usize| f32::from_le_bytes(rec[o..o + 4].try_into().unwrap()) as i32;
                    let (x, y) = (f(0xc), f(0x10));
                    self.with_war(p, |w| w.delete_building(j as i32));
                    let mut world = self.state.colony();
                    let cells = self.state.vehicle_cells(p);
                    let (x, y) = self.rules.placement.locate(&world.planets[p], kind, &cells, Some((x, y))).unwrap_or((x, y));
                    let slot = world.new_building(p, kind, true, (x as f32, y as f32), 0, &self.rules.side, &econ);
                    self.state.store_colony(&world);
                    self.with_war(p, |w| {
                        w.add_building_turrets(kind, owner as i32, slot as i32);
                        w.building_built(slot as i32);
                    });
                }
                j += 1;
            }
        }
    }

    /// `AddWormhole(x1, y1, x2, y2)` (exe 0x53aec0, `fleet-movement.md` "Opening a wormhole"): with more than four
    /// wormholes it returns 0; else wormhole `w = ++OsszHanyWormhole` gets end A at `((f32)x1, (f32)y1)` and end B
    /// at `((f32)x2, (f32)y2)`, both unrevealed, each with its fleet `FlottaNew("- Wormhole -", 0, 0, x, y, 0)`
    /// flagged `+0x53 |= 6` (wormhole, bit 9), and the tint `+0x18 + i = 255 − (255 − c[w][i]) / 2`. Returns `w`.
    /// The four `WormHole` scene objects it also makes are render-only.
    pub fn add_wormhole(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        let n = self.state.mem.i32(va::WORMHOLE_COUNT);
        if n > 4 {
            return 0;
        }
        let w = n + 1;
        self.state.mem.set_i32(va::WORMHOLE_COUNT, w);
        let mut ends = [0i16; 2];
        self.with_ai(|_, world, hooks| {
            for (k, (x, y)) in [(x1, y1), (x2, y2)].into_iter().enumerate() {
                let f = world.new_fleet(b"- Wormhole -", 0, 0, x as f32, y as f32, 0.0, hooks);
                world.starmap.fleets[f].flags |= flag::WORMHOLE | 1 << 9;
                world.mirror(f);
                ends[k] = f as i16;
            }
        });
        let at = va::WORMHOLES + 0x1c * w as u32;
        let mut r = self.state.mem.read(at, 0x1c);
        let c = self.rules.radar.wormhole_colours[w as usize];
        let hole = Wormhole {
            ax: x1 as f32,
            ay: y1 as f32,
            revealed_a: false,
            fleet_a: ends[0],
            bx: x2 as f32,
            by: y2 as f32,
            revealed_b: false,
            fleet_b: ends[1],
            tint: std::array::from_fn(|i| (255 - (255 - c[i] as u8 as i32) / 2) as u8),
        };
        put_wormhole(&mut r, &hole);
        self.state.mem.write(at, &r);
        w
    }

    /// Story 239's follow-up to a wormhole `AddWormhole` made (exe 0x5ceb86..0x5cebc8, `StoryEvent::RevealWormhole`):
    /// both ends revealed (`+8`, `+0x14` := 1) and both fleets always visible (`+0x53 |= 1`).
    pub fn reveal_wormhole(&mut self, w: i32) {
        let at = va::WORMHOLES + 0x1c * w as u32;
        let mut r = self.state.mem.read(at, 0x1c);
        let mut hole = crate::save::wormhole_from(&r);
        (hole.revealed_a, hole.revealed_b) = (true, true);
        put_wormhole(&mut r, &hole);
        self.state.mem.write(at, &r);
        let mut map = self.state.starmap();
        for f in [hole.fleet_a, hole.fleet_b] {
            if let Some(x) = map.fleets.get_mut(f as usize) {
                x.flags |= flag::ALWAYS_VISIBLE;
            }
        }
        self.state.store_starmap(&map);
    }

    /// `FRISSIT_Datum`: `Ido += IdoSpeedSzorzo`, `Ido2 += 1`, the calendar.
    pub fn advance_clock(&mut self) {
        self.space_battles_settle();
        if self.mp_battle_running() {
            return;
        }
        let m = &mut self.state.mem;
        let ido = m.i32(va::IDO).wrapping_add(m.i32(va::IDO_SPEED));
        let t = GameTime(ido);
        m.set_i32(va::IDO, ido);
        m.set_i32(va::IDO2, m.i32(va::IDO2).wrapping_add(1));
        m.set_i32(va::EV, t.year());
        m.set_i32(va::HONAP, t.month());
        m.set_i32(va::NAP, t.day());
    }

    /// The rest of the tick after the clock step (see the module docs).
    pub fn run_tick(&mut self) {
        if self.mp_battle_running() {
            self.battle_tick();
            return;
        }
        let m = &mut self.state.mem;
        let (nap, old) = (m.i32(va::NAP), m.i32(va::OLD_NAP));
        let first_event = self.events.len();
        if nap != old {
            m.set_i32(va::OLD_NAP, nap);
            self.daily();
        }
        self.trade();
        self.battlefield_hand_offs();
        self.info();
        self.research();
        self.story();
        self.inbox();
        self.diplomacy_tick();
        self.buildings_tick();
        self.starmap();
        self.victory_check(&[]);
        let m = &mut self.state.mem;
        m.set_i32(0xBAE054, m.i32(0xBAE054).wrapping_add(1));
        self.battlefield_hand_offs();
        self.grwar();
        self.space_tick();
        if self.view != ViewMode::Classic {
            self.dropships_mp();
        }
        self.fill_fresh_buildings();
        self.post_messages(first_event);
    }

    /// Building records appended this tick hold uninitialised bytes in the original: the records past the used
    /// slots all of them, a used slot the bytes no ported code writes ([`crate::save::BUILDING_UNWRITTEN`]).
    pub(super) fn fill_fresh_buildings(&mut self) {
        for (p, j) in std::mem::take(&mut self.state.fresh_buildings) {
            let u = self.fresh::<{ crate::save::BUILDING_SIZE }>(Fresh::Building { planet: p, slot: j });
            let used = self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
            if let Some(r) = self.state.buildings.get_mut(p).and_then(|b| b.get_mut(j)) {
                if j > used {
                    *r = u;
                } else {
                    for o in crate::save::BUILDING_UNWRITTEN {
                        r[o] = u[o];
                    }
                    r[0x19] = (r[0x19] & 0x7f) | (u[0x19] & 0x80);
                }
            }
        }
    }

    /// `FRISSIT_Trade` (`re/rules/trade.md` §3).
    fn trade(&mut self) {
        let mut ctx = self.market_context();
        let mut t = self.state.trade();
        let events = t.tick(&mut ctx, &mut TradeView { game: self });
        self.state.store_trade(&t);
        self.state.mem.set_i32(va::STORY_ADAT + 0xc4, ctx.story_countdown);
        self.events.extend(events.into_iter().map(GameEvent::Trade));
    }

    /// A purchase message from empire `e` (`MULTIGET_Trade`, `re/rules/trade.md` §4); whether it was accepted.
    /// The sender's `Trade_Main` raises the tutorial's event 0x27 when the offer's hull tech (`+0x14 + 0x48`) is 5
    /// (re/screens/trade.md §6.2 step 4), here with the human's message.
    pub fn buy(&mut self, e: usize, msg: &Delivery) -> bool {
        if e as i32 == self.state.mem.i32(va::EN) && i32::from_le_bytes(msg.0[0x5c..0x60].try_into().unwrap()) == 5 {
            self.tutorial_check_event(0x27);
        }
        let mut t = self.state.trade();
        let ev = t.buy(e, msg, &mut TradeView { game: self });
        self.state.store_trade(&t);
        let ok = ev.is_some();
        self.events.extend(ev.map(GameEvent::Trade));
        ok
    }

    fn market_context(&self) -> MarketContext {
        let g = self.globals();
        let m = &self.state.mem;
        let a = self.adat();
        MarketContext {
            year: g.year,
            month: g.month,
            day: g.day,
            speed_mult: g.speed,
            program_players: g.pp,
            human_controlled: g.human,
            debug: m.u8(va::DEBUGX) != 0,
            cheat: m.u8(va::TRADE_CHEAT) != 0,
            story_countdown: adat_i32(&a, 0xc4),
            story_discount: adat_i32(&a, 0x60) != 0,
            story_designs: self.state.story_designs(),
            story_next_id: m.i32(va::STORY_DESIGN_NEXT),
        }
    }

    /// `MoneyHozzaad`'s income multiplier for a non-human empire (`None` for a human).
    fn ai_income(&self, e: usize) -> Option<f64> {
        let g = self.globals();
        if g.human.get(e).copied().unwrap_or(false) {
            return None;
        }
        let m = self.rules.difficulty_money.get(g.difficulty.max(0) as usize).copied().unwrap_or(1.0) as f64;
        Some(if g.multiplayer { m * 0.5 } else { m })
    }

    fn daily(&mut self) {
        if self.state.mem.i32(va::NAP) % 6 == 1 {
            self.economy();
            self.battlefield_hand_offs();
        }
        let from = self.events.len();
        self.diplomacy_daily(false);
        self.empire_deaths(from);
        if self.state.mem.i32(va::AI_OFF) == 0 {
            self.ai();
        }
        self.spies();
        if self.state.mem.i32(va::AI_OFF) == 0 {
            self.diplomacy_daily(true);
        }
        self.battlefield_hand_offs();
    }

    // -------------------------------------------------------------------------------------------- contexts

    fn economy_context(&self, g: &Globals) -> EconomyContext {
        let a = self.adat();
        let k = self.state.kidnap();
        EconomyContext {
            year: g.year,
            month: g.month,
            day: g.day,
            multiplayer: g.multiplayer,
            human_empire: g.en as u8,
            human_controlled: g.human,
            empire_race: g.race,
            players: g.pp2 as i32,
            kidnap_state: std::array::from_fn(|e| k[e]),
            global_morale: self.state.mem.i32(va::GLOBAL_MORALE),
            global_morale_speed: self.state.mem.i32(va::GLOBAL_MORALE_SPEED),
            story: StoryHooks {
                food_halved_planet: adat_i32(&a, 0x20),
                lab_bonus_empire: adat_i32(&a, 0x24),
                food_lost: adat_i32(&a, 0x2c),
                hospital_watch: adat_i32(&a, 0x30),
                lab_start: std::array::from_fn(|i| adat_i32(&a, 0x9c + 4 * i)),
            },
            story_values: [0x5fb, 0x60f].iter().map(|&c| (c, self.story_var(c) as u8)).collect(),
        }
    }

    /// `st_vGet(id)` without the side effects (unset = 0).
    fn story_var(&self, id: i32) -> i32 {
        self.state.story().vars.get(id).unwrap_or(0)
    }

    fn store_economy_context(&mut self, ctx: &EconomyContext) {
        let m = &mut self.state.mem;
        m.set_i32(va::GLOBAL_MORALE, ctx.global_morale);
        m.set_i32(va::GLOBAL_MORALE_SPEED, ctx.global_morale_speed);
        m.set_i32(va::STORY_ADAT + 0x2c, ctx.story.food_lost);
    }

    fn research_context(&self, g: &Globals) -> ResearchContext {
        ResearchContext {
            speed_mult: g.speed,
            day: g.day,
            difficulty: g.difficulty.max(0) as usize,
            multiplayer: g.multiplayer,
            human_empire: g.en as u8,
            human_controlled: g.human,
            race: g.race,
            story_bonus: adat_i32(&self.adat(), 0x6c),
            cheat: self.state.mem.i32(va::RESEARCH_CHEAT) != 0,
            all_techs: g.all_techs,
            players: g.pp2 as i32,
            speed_locked: g.speed_locked,
            program_speed: g.program_speed,
        }
    }

    fn starmap_context(&self, g: &Globals) -> StarmapContext {
        let m = &self.state.mem;
        let a = self.adat();
        let rel = m.read(va::RELATIONS, EMPIRES * EMPIRES * 0x26);
        let tech = self.state.tech();
        StarmapContext {
            tick: g.ido2,
            speed_mult: g.speed,
            multiplayer: g.multiplayer,
            speed_locked: g.speed_locked,
            program_speed: g.program_speed,
            human_empire: g.en as u8,
            human_controlled: g.human,
            alive: g.alive,
            empire_race: g.race,
            debug: m.u8(va::DEBUGX) != 0,
            all_fleets_visible: m.i32(va::STORY_ALL_FLEETS_VISIBLE) != 0,
            debug_speed: m.i32(va::DEBUG_SPEED) != 0,
            relations: (0..EMPIRES).map(|x| std::array::from_fn(|y| (rel[(x * EMPIRES + y) * 0x26 + 0x24], rel[(x * EMPIRES + y) * 0x26 + 0x25]))).collect(),
            default_fleet_name: self.fleet_name(),
            program_players: g.pp,
            program_players2: g.pp2,
            radar_tech: std::array::from_fn(|e| (1..=4u8).rev().find(|&c| tech.status(0x20 + c as usize, e) == 5).unwrap_or(0)),
            story_tracked_fleet: adat_i32(&a, 0x28),
            story_radar_bonus: adat_i32(&a, 0x38) != 0,
            story_no_supply: adat_i32(&a, 0x48) != 0,
            story_supply_planet: adat_i32(&a, 0x50),
            tutorial_sighting: g.tutorial && m.i32(va::TUTORIAL_EVENT) == 1,
            radar_rules: self.rules.radar.clone(),
            in_battle: Vec::new(),
            sends: g.sends,
            viewers: g.sends,
            transfer: (m.i32(va::AKT_KEP) == 2 && m.u8(va::STARMAP_TRANSFER_MODE) != 0)
                .then(|| (m.i32(va::STARMAP_TRANSFER_FLEETS.0), m.i32(va::STARMAP_TRANSFER_FLEETS.1))),
        }
    }

    /// Waypoint records appended since there were `from` (`FlottaAddWaypoint`'s pool grows): the bytes the code
    /// never writes come from [`Game::uninit`] (all of record 0, `+0xa..+0xc` of the others).
    fn fresh_waypoints(&mut self, from: usize) {
        let n = self.state.waypoints.len();
        let Some(f) = self.uninit.as_mut() else { return };
        for i in from..n {
            let raw = f(Fresh::Waypoint(i));
            if raw.len() < 0xc {
                continue;
            }
            let r = &mut self.state.waypoints[i];
            if i == 0 {
                r.copy_from_slice(&raw[..0xc]);
            } else {
                r[0xa..0xc].copy_from_slice(&raw[0xa..0xc]);
            }
        }
    }

    /// `AlapFlottaNev` (localised; "Fleet" in English).
    pub fn fleet_name(&self) -> Vec<u8> {
        self.fleet_name.clone().unwrap_or_else(|| b"Fleet".to_vec())
    }

    fn diplomacy_context(&self, g: &Globals) -> DiplomacyContext {
        let m = &self.state.mem;
        let a = self.adat();
        DiplomacyContext {
            player_ki: m.read(va::PLAYER_KI, EMPIRES).try_into().unwrap(),
            program_players: g.pp,
            program_players2: g.pp2,
            alive: g.alive,
            ai: m.flags(va::AI_EMPIRE),
            may_attack: m.flags(va::MAY_ATTACK),
            human: g.human,
            race: g.race,
            human_empire: g.en,
            multiplayer: g.multiplayer,
            debug: m.u8(va::DEBUGX) != 0,
            enabled: m.u8(va::DIPLOMACY_ENABLED) != 0,
            ai_off: m.i32(va::AI_OFF) != 0,
            story_quiet: adat_i32(&a, 0xb0) != 0,
            spare_human: std::array::from_fn(|e| adat_i32(&a, 0xd0 + 4 * e) != 0),
            difficulty: g.difficulty,
            tutorial: g.tutorial,
            forced_answer: m.i32(va::FORCED_ANSWER),
            main_fleet: m.i32s(va::AI_MAIN_FLEET),
            month: g.month,
            day: g.day,
            kidnap: self.state.kidnap(),
            all_techs: g.all_techs,
        }
    }

    fn diplo_planets(&self) -> Vec<DiploPlanet> {
        let m = &self.state.mem;
        (0..=self.state.planet_count())
            .map(|p| {
                let r = m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE);
                let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
                DiploPlanet {
                    owner: r[0x20],
                    x: f32::from_bits(i32_at(0x18) as u32),
                    y: f32::from_bits(i32_at(0x1c) as u32),
                    population: i32_at(0x28),
                    defence: i32_at(0x5c),
                    ship_capacity: i32_at(0x70),
                    vehicle_capacity: i32_at(0x74),
                    fortification: m.i32(va::FORTRESS + 4 * p as u32),
                    visible: std::array::from_fn(|e| i16::from_le_bytes([r[0xd8 + 2 * e], r[0xd9 + 2 * e]])),
                }
            })
            .collect()
    }

    fn store_diplo_planets(&mut self, pl: &[DiploPlanet]) {
        for (p, d) in pl.iter().enumerate() {
            let at = va::PLANETS + (p * PLANET_SIZE) as u32;
            self.state.mem.set_u8(at + 0x20, d.owner);
            for e in 0..EMPIRES {
                self.state.mem.write(at + 0xd8 + 2 * e as u32, &d.visible[e].to_le_bytes());
            }
        }
    }

    fn spy_context(&self, g: &Globals) -> SpyContext {
        let m = &self.state.mem;
        SpyContext {
            alive: g.alive,
            human: g.human,
            race: g.race,
            human_empire: g.en,
            program_players: g.pp as i32,
            multiplayer: g.multiplayer,
            difficulty: g.difficulty,
            debug: m.u8(va::DEBUGX) != 0,
            tutorial: g.tutorial,
            no_recruits: m.u8(va::TUTORIAL_NO_RECRUITS) != 0,
            forced: m.i32(va::SPY_FORCED),
            day: g.day,
            senders: (self.view != ViewMode::Classic).then_some(g.sends),
            listed: std::array::from_fn(|e| e != g.en && self.state.humans.contains_key(&(e as u8))),
        }
    }

    // -------------------------------------------------------------------------------------------- systems

    fn economy(&mut self) {
        self.battlefield_weather();
        let g = self.globals();
        let mut ctx = self.economy_context(&g);
        let mut w = self.state.colony();
        let mut rng = self.state.rng();
        let (placement, state) = (&self.rules.placement, &self.state);
        let mut place = |pl: &Planet, p: usize, k: u8| placement.place(pl, k, &state.vehicle_cells(p));
        let (_, events) = w.economy_step(&self.rules.side, &mut ctx, &mut rng, &mut place);
        self.state.store_colony(&w);
        self.state.store_rng(&rng);
        self.store_economy_context(&ctx);
        for e in &events {
            match *e {
                SideEvent::RecallAttackers { planet } => self.recall_attackers(planet),
                // A rebellion's outcome (`colony-side.md` §3, exe 0x47565e..0x475683): `Story_Special(0x15, p)` when
                // the planet went independent, `0x16` for result 3. Inside the planet loop in the exe; nothing later
                // in the step reads the stories.
                SideEvent::Rebelled { planet, result: r @ (2 | 3), .. } => {
                    self.story_special([if r == 2 { 0x15 } else { 0x16 }, planet as i32, 0, 0, 0, 0, 0, 0]);
                }
                _ => {}
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Side));
    }

    /// The prologue of `FRISSIT_Planet(true, 0)` (exe 0x4749b9..0x474ab0, `colony-economy.md` §3): each planet's
    /// ground battlefield gets its weather (`WF[p]+0x8d0`, [`ground::weather`]) and, when its colony map is below
    /// `hanyterep`, its background (`WF[p]+0x8d4`, exe 0x47a380, [`ground::pick_background`]), skipping the planet
    /// on screen (none in canonical mode).
    fn battlefield_weather(&mut self) {
        let cursor = self.state.mem.i32(va::BACKGROUND_CURSOR);
        let (draws, cursor, libc) = self.weather_draws(cursor, self.libc);
        for (p, weather, background) in draws {
            let mut field = self.state.take_ground_field(p);
            field.weather = weather;
            if let Some(b) = background {
                field.background = b;
            }
            self.state.store_ground_field(p, field);
        }
        self.state.mem.set_i32(va::BACKGROUND_CURSOR, cursor);
        self.libc = libc;
    }

    /// The prologue's draws from background cursor `cursor` and libc `libc`: `(planet, weather, background)` per
    /// planet drawn for, the cursor and libc after. Changes nothing (`melyikhatter`'s cursor 0x996488 isn't saved:
    /// a replay without its value can try each).
    pub fn weather_draws(&self, mut cursor: i32, mut libc: Libc) -> (Vec<WeatherDraw>, i32, Libc) {
        let m = &self.state.mem;
        let shown = self.shown_planet();
        let terrains = self.rules.placement.terrains.len();
        let mut out = Vec::new();
        for p in 1..=self.state.planet_count() {
            if shown == Some(p as i32) {
                continue;
            }
            let r = m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE);
            let (surface, map) = (r[0x22], r[0x23] as usize);
            let info = (0..5).any(|k| i16::from_le_bytes([r[0x16a + 2 * k], r[0x16b + 2 * k]]) == 0x776);
            let weather = ground::weather(surface, info, &mut || libc.rand());
            let background = (map < terrains).then(|| {
                let weathers = self.rules.surface_weathers.get(map).map_or(&[][..], |w| &w[..]);
                ground::pick_background(&mut cursor, weathers, &self.rules.backgrounds, &mut || libc.rand())
            });
            out.push((p, weather, background));
        }
        (out, cursor, libc)
    }

    /// Whether this tick's daily block runs the economy step (`FRISSIT_Planet(true, 0)` when the day changed and
    /// `Nap % 6 == 1`).
    pub fn economy_due(&self) -> bool {
        let m = &self.state.mem;
        m.i32(va::NAP) != m.i32(va::OLD_NAP) && m.i32(va::NAP) % 6 == 1
    }

    /// `FRISSIT_Grwar` (exe 0x4747c0, ground-combat.md §11.7) after the tick (`Fake_FRISSITES`, unless an MP space
    /// battle runs (0xBAD114) or the SP battle screen is up): `gvalue` (0x9B2900) += 1, then every planet's
    /// self-destruct countdown per battle step and, every 300th call, `WF::Collapse`
    /// ([`ground::War::grwar_planet`]). The colony screen's `WF::MI` / `WF::Refresh` for the shown planet are the
    /// real-time battle's (not run here). `FRISSIT_Colony` (0x474680), which follows, changes nothing a save holds:
    /// the passability maps' 600-tick expiry, smoke emitters (0xA3E9C8, with libc draws), debris and the UI effect list.
    fn grwar(&mut self) {
        let m = &self.state.mem;
        if m.i32(va::MP_SPACE_BATTLE) != 0 || (m.i32(va::AKT_KEP) == 0xb && !self.globals().multiplayer) {
            return;
        }
        let gvalue = m.i32(va::GVALUE).wrapping_add(1);
        self.state.mem.set_i32(va::GVALUE, gvalue);
        let clock = ground::Clock { speed: self.state.mem.i32(va::BATTLE_SPEED), paused: false };
        for p in 1..=self.state.mem.i32(va::PLANET_COUNT2).max(0) as usize {
            let due = gvalue % 300 == 0 || self.state.ground_field(p).is_some_and(|f| f.self_destruct > -1);
            if due {
                self.with_war(p, |w| w.grwar_planet(clock, gvalue));
            }
        }
        if self.view != ViewMode::Classic {
            self.grwar_mp();
        }
    }

    /// `FlottaMozgat(f, 0, 0, 0, 0)` for every fleet attacking planet `p` (`order 2, kind 1`).
    fn recall_attackers(&mut self, p: usize) {
        let g = self.globals();
        let ctx = self.starmap_context(&g);
        let mut map = self.state.starmap();
        let mut ev = Vec::new();
        for f in 1..map.fleets.len() {
            let fl = &map.fleets[f];
            if fl.alive() && fl.order == order::ATTACK && fl.kind == order::PLANET && fl.tx == p as f32 {
                map.order(f, 0, 0, 0.0, 0.0, &ctx, &mut ev);
            }
        }
        self.state.store_starmap(&map);
        self.events.extend(ev.into_iter().map(GameEvent::Starmap));
    }

    /// `FRISSIT_Viszony` (`messages == false`) or `MI_DIPLOMACY`.
    fn diplomacy_daily(&mut self, messages: bool) {
        self.with_diplomacy(|d, w| if messages { d.ai_messages(w) } else { d.daily(w) });
    }

    /// Run `f` on diplomacy's state and world (decoded from the save state, stored back afterwards).
    fn with_diplomacy(&mut self, f: impl FnOnce(&mut Diplomacy, &mut diplomacy::World)) {
        let g = self.globals();
        let mut ctx = self.diplomacy_context(&g);
        let star = self.starmap_context(&g);
        let mut d = self.state.diplomacy();
        let mut pl = self.diplo_planets();
        let mut map = self.state.starmap();
        let mut tech = self.state.tech();
        let mut rng = self.state.rng();
        let mut libc = self.libc;
        let mut draw = || libc.rand();
        let mut cosmetic = self.cosmetic;
        let mut draw_view = || cosmetic.rand();
        let canonical = self.view != ViewMode::Classic;
        let events = {
            let mut w = diplomacy::World {
                ctx: &mut ctx,
                planets: &mut pl,
                starmap: &mut map,
                star_ctx: &star,
                tech: &mut tech,
                rng: &mut rng,
                rules: &self.rules.diplomacy,
                libc: &mut draw,
                view_libc: if canonical { Some(&mut draw_view) } else { None },
                events: Vec::new(),
            };
            f(&mut d, &mut w);
            w.events
        };
        self.libc = libc;
        self.cosmetic = cosmetic;
        self.store_diplomacy_world(&d, &ctx, &pl, &map, &tech, &rng);
        self.events.extend(events.into_iter().map(GameEvent::Diplomacy));
    }

    fn store_diplomacy_world(&mut self, d: &Diplomacy, ctx: &DiplomacyContext, pl: &[DiploPlanet], map: &crate::fleet::Starmap,
        tech: &crate::colony_side::TechTable, rng: &Streams) {
        self.state.store_diplomacy(d);
        self.store_diplo_planets(pl);
        self.state.store_starmap(map);
        self.state.store_tech(tech);
        self.state.store_rng(rng);
        self.state.store_kidnap(&ctx.kidnap);
        for (e, &a) in ctx.alive.iter().enumerate() {
            if !a {
                self.state.mem.set_u8(va::EMPIRE_ALIVE + e as u32, 0);
            }
        }
    }

    fn diplomacy_tick(&mut self) {
        self.with_diplomacy(|d, w| d.tick(w));
    }

    /// A human's message out of the lockstep queue (`MULTIGET_Diplomacy` exe 0x4c7880, packet 0x3e →
    /// `FeldolgozAjanlat`, `re/rules/diplomacy.md` §10.2); its deals reach money, planets and research.
    pub fn diplomacy_message(&mut self, letter: diplomacy::Letter) {
        self.with_deals(|d, w, deals| d.receive_message(w, deals, letter));
    }

    /// Human `issuer`'s diplomacy screen writes to its contact-list slot of `empire` (`re/screens/diplomacy.md` §8).
    pub fn diplomacy_screen(&mut self, issuer: usize, empire: usize, op: diplomacy::ScreenOp) {
        let (en, ido) = (self.state.mem.i32(va::EN).clamp(0, 18) as usize, self.state.mem.i32(va::IDO));
        let mut d = self.state.diplomacy();
        d.screen(en, issuer, empire, op, ido);
        self.state.store_diplomacy(&d);
    }

    /// A human's answer (packet 0x3f → `FeldolgozValasz`, §10.2).
    pub fn diplomacy_answer(&mut self, answer: diplomacy::Answer) {
        self.with_deals(|d, w, deals| d.receive_answer(w, deals, answer));
    }

    /// [`Game::with_diplomacy`] with the colony world as the deals' side (`diplomacy::ColonyDeals`).
    fn with_deals(&mut self, f: impl FnOnce(&mut Diplomacy, &mut diplomacy::World, &mut dyn diplomacy::Deals)) {
        let g = self.globals();
        let rules = self.rules;
        let econ = self.economy_context(&g);
        let rctx = self.research_context(&g);
        let mut colony = self.state.colony();
        let mut research = self.state.research();
        let mut deals = diplomacy::ColonyDeals {
            world: &mut colony,
            rules: &rules.side,
            ctx: &econ,
            research: &mut research,
            research_rules: &rules.research,
            research_ctx: &rctx,
            events: Vec::new(),
            research_events: Vec::new(),
        };
        self.with_diplomacy(|d, w| f(d, w, &mut deals));
        let (side, done) = (std::mem::take(&mut deals.events), std::mem::take(&mut deals.research_events));
        // Diplomacy's tech table is authoritative (stored above), and so are the deaths it stored.
        colony.tech = self.state.tech();
        let alive = self.state.alive();
        for (a, &b) in colony.alive.iter_mut().zip(alive.iter()) {
            *a &= b;
        }
        self.state.store_colony(&colony);
        self.state.store_research(&research);
        self.events.extend(side.into_iter().map(GameEvent::Side));
        self.events.extend(done.into_iter().map(GameEvent::Research));
    }

    fn ai(&mut self) {
        self.with_ai(|st, w, hooks| st.daily(w, hooks));
    }

    /// Run `f` on the AI's world (decoded from the state, stored back afterwards).
    fn with_ai(&mut self, f: impl FnOnce(&mut ai::AiState, &mut ai::World, &mut dyn AiHooks)) {
        let g = self.globals();
        let m = &self.state.mem;
        let a = self.adat();
        let ctx = AiContext {
            ai: m.flags(va::AI_EMPIRE),
            program_players2: g.pp2,
            spies_enabled: m.u8(va::SPY_ENABLED) != 0,
            story_resale: adat_i32(&a, 0xac) != 0,
            may_attack: m.flags(va::MAY_ATTACK),
            story_no_war: adat_i32(&a, 0xb0) != 0,
            story_guard: [adat_i32(&a, 0xb8) != 0, adat_i32(&a, 0xbc) != 0],
            spare_human: std::array::from_fn(|e| adat_i32(&a, 0xd0 + 4 * e) != 0),
            year: g.year,
            month: g.month,
            tick: g.tick,
            ai_off: m.i32(va::AI_OFF) != 0,
            design_debug: m.i32(va::DESIGN_DEBUG) != 0,
        };
        let sctx = self.spy_context(&g);
        let econ = self.economy_context(&g);
        let rctx = self.research_context(&g);
        let star = self.starmap_context(&g);
        let mut st = self.state.ai();
        let mut colony = self.state.colony();
        let mut combat = self.state.combat();
        let mut research = self.state.research();
        let mut relations = self.state.relations();
        let mut kidnap = self.state.kidnap();
        let mut rng = self.state.rng();
        let mut spies = self.state.spies();
        let mut map = self.state.starmap();
        let mut fleet_numbers = m.i32s(va::FLEET_NUMBERS);
        let danger = m.i32s(va::AI_DANGER);
        let power = m.f32s(va::EMPIRE_POWER);
        let planet_count = m.i32s(va::EMPIRE_PLANETS);
        let mut known = self.state.known_since();
        let mut story = |_: StoryCheck| false;
        let mut hooks = WarHooks { wars: Vec::new(), uninit: self.uninit.as_mut() };
        let waypoints = self.state.waypoints.len();
        let (events, spy_events) = {
            let mut w = ai::World {
                ctx: &ctx,
                rules: &self.rules.ai,
                spy: spy::World {
                    ctx: &sctx,
                    colony: &mut colony,
                    side_rules: &self.rules.side,
                    econ: &econ,
                    combat: &mut combat,
                    research: &mut research,
                    research_rules: &self.rules.research,
                    research_ctx: &rctx,
                    relations: &mut relations,
                    kidnap: &mut kidnap,
                    rng: &mut rng,
                    rules: &self.rules.spy,
                    story: &mut story,
                    events: Vec::new(),
                },
                spies: &mut spies,
                starmap: &mut map,
                star_ctx: &star,
                fleet_numbers: &mut fleet_numbers,
                danger: &danger,
                power: &power,
                planet_count: &planet_count,
                known_since: &mut known,
                events: Vec::new(),
            };
            f(&mut st, &mut w, &mut hooks);
            (w.events, w.spy.events)
        };
        self.state.store_ai(&st);
        self.state.store_colony(&colony);
        self.state.store_combat(&combat);
        // The AI keeps the pools in the combat view (`ai::World::starmap`); the starmap's copy is stored last.
        map.colony_ship_pool = combat.colony_ship_pool;
        map.terraformer_pool = combat.terraformer_pool;
        self.state.store_starmap(&map);
        self.state.store_research(&research);
        self.state.store_relations(&relations);
        self.state.store_kidnap(&kidnap);
        self.state.store_rng(&rng);
        self.state.store_spies(&spies);
        self.state.mem.set_i32s(va::FLEET_NUMBERS, &fleet_numbers);
        self.state.store_known_since(&known);
        self.events.extend(hooks.wars.into_iter().map(|(a, b)| GameEvent::WarDeclared { a: a as u8, b: b as u8 }));
        self.fresh_waypoints(waypoints);
        self.events.extend(events.into_iter().map(GameEvent::Ai));
        self.spy_hooks(spy_events);
    }

    /// The spy pass's calls into other systems, in event order (inside the pass in the exe; nothing later in it
    /// reads what they change): `Story_Special(0xb, EN, e)` + `FelfedezBirodalom(e)` for a caught spy of an unknown
    /// empire, and each report's queue entry and `Story_KemEsemeny` (`info-messages.md` §3.3).
    fn spy_hooks(&mut self, events: Vec<SpyEvent>) {
        let en = self.globals().en as i32;
        for ev in events {
            let hook = ev.clone();
            self.events.push(GameEvent::Spy(ev));
            match hook {
                SpyEvent::Discovered { empire, by } if by as i32 == en => {
                    self.story_special([0x0b, en, empire as i32, 0, 0, 0, 0, 0]);
                    self.with_diplomacy(|d, w| d.discover(w, empire as usize));
                }
                // Another human's (canonical multiplayer, `re/rules/multiplayer.md` §5): its own copies only.
                SpyEvent::Discovered { empire, by } => self.with_diplomacy(|d, w| d.discover_by(w, by as usize, empire as usize)),
                SpyEvent::Report(r) => {
                    self.state.push_spy_report(&r);
                    self.with_story(|st, w| st.spy_event(&ScenarioStories, w));
                }
                SpyEvent::ReportTo { to, report } => self.state.push_spy_report_to(to, &report),
                SpyEvent::Combat(crate::spacewar::CombatEvent::HumanFleetEmptied { fleet }) => {
                    self.events.push(GameEvent::FleetEmptied { fleet });
                }
                _ => {}
            }
        }
    }

    fn spies(&mut self) {
        self.with_spies(|spies, w| spies.daily(w));
    }

    /// A human's spy-screen order out of the lockstep queue (`MULTIGET_Spy` exe 0x5883c0, `espionage.md` §8).
    pub fn spy_order(&mut self, issuer: usize, order: spy::Order) {
        let (training, mult) = (self.rules.ai.training_cost.clone(), self.rules.ai.mission_cost_mult.clone());
        // The spy screen's own steps (re/screens/spy.md §7): the issuer's report queue and its stream-0 rolls.
        let en = self.globals().en;
        match order {
            spy::Order::Acknowledge => return self.state.pop_spy_report(issuer, en),
            spy::Order::Estimate { spy: k, mission: m, target: t } => {
                let ctx = self.spy_context(&self.globals());
                let (spies, mut rng) = (self.state.spies(), self.state.rng());
                spies.escape_chance(&ctx, &self.rules.spy, &mut rng, issuer, k as usize, m as usize, t as usize, spy::ESTIMATE_ROUNDS, spy::UI_STREAM);
                return self.state.store_rng(&rng);
            }
            spy::Order::Judge { choice, owner, spy: k } => {
                let (spies, mut rng) = (self.state.spies(), self.state.rng());
                let (fate, kind) = spies.judge(&mut rng, owner as usize, k as usize, choice);
                self.state.store_rng(&rng);
                self.state.set_spy_report_kind(issuer, en, kind);
                let verdict = spy::Order::Verdict { fate, owner, spy: k };
                return self.with_spies(|spies, w| spies.order(w, issuer, verdict, &training, &mult));
            }
            _ => {}
        }
        self.with_spies(|spies, w| spies.order(w, issuer, order, &training, &mult));
    }

    /// Run `f` on espionage's state and world (decoded from the save state, stored back afterwards), then carry out
    /// its events ([`Game::spy_hooks`]).
    fn with_spies(&mut self, f: impl FnOnce(&mut spy::Spies, &mut spy::World)) {
        let g = self.globals();
        let sctx = self.spy_context(&g);
        let econ = self.economy_context(&g);
        let rctx = self.research_context(&g);
        let mut colony = self.state.colony();
        let mut combat = self.state.combat();
        let mut research = self.state.research();
        let mut relations = self.state.relations();
        let mut kidnap = self.state.kidnap();
        let mut rng = self.state.rng();
        let mut spies = self.state.spies();
        let mut story = |_: StoryCheck| false;
        let events = {
            let mut w = spy::World {
                ctx: &sctx,
                colony: &mut colony,
                side_rules: &self.rules.side,
                econ: &econ,
                combat: &mut combat,
                research: &mut research,
                research_rules: &self.rules.research,
                research_ctx: &rctx,
                relations: &mut relations,
                kidnap: &mut kidnap,
                rng: &mut rng,
                rules: &self.rules.spy,
                story: &mut story,
                events: Vec::new(),
            };
            f(&mut spies, &mut w);
            w.events
        };
        self.state.store_colony(&colony);
        self.state.store_combat(&combat);
        self.state.store_research(&research);
        self.state.store_relations(&relations);
        self.state.store_kidnap(&kidnap);
        self.state.store_rng(&rng);
        self.state.store_spies(&spies);
        self.spy_hooks(events);
    }

    /// `MULTIGET_Starmap`'s message 0x1b (ELF 0x719680): fleet `f` is deleted (`FlottaDelete`) if it still holds
    /// no ships (`+0x54 < 1`).
    pub fn fleet_emptied(&mut self, f: usize) {
        if self.state.combat().fleets.get(f).is_some_and(|x| x.totals.ships < 1) {
            self.with_ai(|_, w, _| w.delete_fleet(f));
        }
    }

    fn research(&mut self) {
        let g = self.globals();
        let mut ctx = self.research_context(&g);
        let mut techs = self.state.tech();
        let mut res = self.state.research();
        let labs: [[i32; 4]; EMPIRES] = std::array::from_fn(|e| self.state.mem.i32s(va::LABS + 16 * e as u32));
        let mut money = self.state.treasuries();
        let events = research::tick(&mut techs, &mut res, &labs, &mut money, &self.rules.research, &mut ctx);
        self.state.store_tech(&techs);
        self.state.store_research(&res);
        self.state.store_treasuries(&money);
        self.state.mem.set_i32(va::IDO_SPEED, ctx.speed_mult);
        // A finished project: story special 0x0D(e, t), then the AI's designs `FRISSIT_MI_DESIGN(e, false)`
        // (production-research.md §2; inside the empire loop in the exe, which nothing later in it reads).
        // A human's hull tech: `TervezAutomatikus` inside `TalalmanyKifejlesztve`.
        for e in &events {
            match *e {
                ResearchEvent::AutoDesign { empire, tech } => self.automatic_design(empire as usize, tech),
                ResearchEvent::Completed { empire, tech } => {
                    self.story_special([0x0d, empire as i32, tech as i32, 0, 0, 0, 0, 0]);
                    self.with_ai(|st, w, hooks| st.design(w, empire as usize, false, hooks));
                }
                _ => {}
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Research));
    }

    /// `TervezAutomatikus(e, t)` (exe 0x4b72b0): the automatic design a human's developed hull tech brings.
    pub(crate) fn automatic_design(&mut self, e: usize, t: usize) {
        self.with_ai(|_, w, hooks| w.automatic_design(e, t, hooks));
    }

    /// `Story_Special(data)`: the story engine checks the scripts against it.
    pub fn story_special(&mut self, data: [i32; 8]) {
        self.with_story(|st, w| st.story_special(data, &ScenarioStories, w));
    }

    /// `MULTIGET_Story`'s message 0x65 (ELF 0x740670): `StartStory(id, false)` (queued for the next check).
    pub fn story_start(&mut self, id: i32) {
        self.with_story(|st, _| {
            st.start_story(id, false);
        });
    }

    fn story(&mut self) {
        self.with_story(|st, w| st.tick(&ScenarioStories, w));
    }

    /// Run `f` on the story engine (its state decoded from the save state, stored back afterwards); the
    /// scripts' requests to other systems that the replay needs are carried out.
    fn with_story(&mut self, f: impl FnOnce(&mut crate::story::Story, &mut StoryView)) {
        let g = self.globals();
        let mut st = self.state.story();
        let mut rng = self.state.rng();
        let events = {
            let mut w = StoryView { state: &self.state, g: &g, rng: &mut rng, events: Vec::new() };
            f(&mut st, &mut w);
            w.events
        };
        self.state.store_story(&st);
        self.state.store_rng(&rng);
        let mut made = 0;
        for e in &events {
            match *e {
                StoryEvent::StopFleet(f) => {
                    let ctx = self.starmap_context(&g);
                    let mut map = self.state.starmap();
                    let mut ev = Vec::new();
                    map.order(f.max(0) as usize, 0, 0, 0.0, 0.0, &ctx, &mut ev);
                    self.state.store_starmap(&map);
                }
                StoryEvent::Wormhole { x1, y1, x2, y2 } => made = self.add_wormhole(x1, y1, x2, y2),
                StoryEvent::RevealWormhole if made != 0 => self.reveal_wormhole(made),
                _ => {}
            }
        }
        self.inbox_events(&events);
        self.story_ends(&events);
        self.events.extend(events.into_iter().map(GameEvent::Story));
    }

    fn starmap(&mut self) {
        let marked = self.starmap_ground_marks();
        let g = self.globals();
        let mut ctx = self.starmap_context(&g);
        ctx.in_battle.extend(marked);
        let ships = self.state.ship_designs();
        let (events, paused) = self.starmap_pass(&mut ctx, |map, ctx, rng, hooks| map.tick_until_battle(&ships, ctx, rng, hooks));
        self.state.mem.set_i32(va::IDO_SPEED, ctx.speed_mult);
        // The human met an empire: story special 0x0B(EN, o) and `FelfedezBirodalom(o)` (fleet-movement.md §6.3;
        // inside the radar pass in the exe, which nothing after it in the tick reads).
        for e in &events {
            if let StarmapEvent::EmpireMet { by, empire } = *e {
                if by as usize == g.en {
                    self.story_special([0x0b, by as i32, empire as i32, 0, 0, 0, 0, 0]);
                    self.with_diplomacy(|d, w| d.discover(w, empire as usize));
                } else if self.state.humans.contains_key(&by) {
                    // Another human's radar (canonical multiplayer, `re/rules/multiplayer.md` §5.1): its list only.
                    self.with_diplomacy(|d, w| d.discover_by(w, by as usize, empire as usize));
                }
            }
        }
        self.story_arrivals(&events);
        let battle = events.last().copied().filter(|_| paused.is_some());
        self.events.extend(events.into_iter().map(GameEvent::Starmap));
        // An arrival that attacks: `SpaceWarIndit` / `GroundWarIndit` inside the step (`game::battles`), then the
        // step ends and the arrival's story checks run; no later fleet moves, and production doesn't run.
        if let Some(p) = paused {
            match battle {
                Some(StarmapEvent::SpaceBattle { fleet, target }) => self.space_war(fleet, target, 0),
                Some(StarmapEvent::PlanetAttack { fleet, planet }) => self.planet_attack(fleet, planet),
                _ => {}
            }
            let g = self.globals();
            let mut ctx = self.starmap_context(&g);
            let (events, ()) = self.starmap_pass(&mut ctx, |map, ctx, rng, hooks| (map.resume(p, ctx, rng, hooks), ()));
            self.story_arrivals(&events);
            self.arrival_checks();
            self.events.extend(events.into_iter().map(GameEvent::Starmap));
            return;
        }
        // The production block ends `FRISSIT_Starmap` (production-research.md §3).
        self.production(&g, ctx.speed_mult);
    }

    /// Run `f` on the starmap with the arrival hooks (colony world, combat view), then store everything back; the
    /// colony side's events are added to the game's. Returns `f`'s events and its other result.
    fn starmap_pass<R>(&mut self, ctx: &mut StarmapContext,
        f: impl FnOnce(&mut Starmap, &mut StarmapContext, &mut Streams, &mut StarmapHooks) -> (Vec<StarmapEvent>, R))
        -> (Vec<StarmapEvent>, R) {
        let g = self.globals();
        let mut map = self.state.starmap();
        let mut rng = self.state.rng();
        let mut colony = self.state.colony();
        let mut combat = self.state.combat();
        let mut fleet_numbers = self.state.mem.i32s(va::FLEET_NUMBERS);
        let econ = self.economy_context(&g);
        let state = &self.state;
        let vehicles = |p: usize| state.vehicle_cells(p);
        let (events, extra, side, overflow, merges) = {
            let mut hooks = StarmapHooks {
                colony: ColonyArrivals {
                    world: &mut colony, rules: &self.rules.side, ctx: &econ, placement: &self.rules.placement, vehicles: &vehicles,
                    events: Vec::new(),
                },
                combat: &mut combat,
                fleet_numbers: &mut fleet_numbers,
                overflow: 0,
                merges: Vec::new(),
            };
            let (ev, extra) = f(&mut map, ctx, &mut rng, &mut hooks);
            (ev, extra, hooks.colony.events, hooks.overflow, hooks.merges)
        };
        // `FlottaMozgat` gave colony ships and terraformers of re-ordered colonising fleets back to the pools.
        for e in &events {
            match *e {
                StarmapEvent::ColonyShipFreed { empire } => map.colony_ship_pool[empire as usize % EMPIRES] += 1,
                StarmapEvent::TerraformerFreed { empire } => map.terraformer_pool[empire as usize % EMPIRES] += 1,
                _ => {}
            }
        }
        self.state.store_colony(&colony);
        // The combat view holds the totals, ships and vehicles; the starmap's records the rest (stored last).
        self.state.store_combat(&combat);
        self.state.store_starmap(&map);
        self.state.mem.set_i32s(va::FLEET_NUMBERS, &fleet_numbers);
        if overflow != 0 {
            let at = va::FLEET_NUMBERS + 4 * DELETED as u32;
            self.state.mem.set_i32(at, self.state.mem.i32(at) - overflow);
        }
        // `FlottaOsszevon` moves the selection (`AktFleet`) to the merged fleet (in canonical mode a peer's UI
        // follows `StarmapEvent::Merge`).
        for (into, from) in merges {
            if self.view == ViewMode::Classic && self.state.mem.i32(va::AKT_FLEET) == from as i32 {
                self.state.mem.set_i32(va::AKT_FLEET, into as i32);
            }
        }
        self.state.store_rng(&rng);
        // Tutorial event 1: the pass's first sighting renames its planet (the exe re-reads the flags each time).
        if let Some(p) = events.iter().find_map(|e| match *e {
            StarmapEvent::TutorialPlanetSighted { planet } => Some(planet),
            _ => None,
        }) {
            self.tutorial_sighting(p);
        }
        self.events.extend(side.into_iter().map(GameEvent::Side));
        (events, extra)
    }

    /// The story's view of the starmap's arrivals (fleet-movement.md §4.3, `campaign.md` §2): every
    /// `Story_FleetArrivedSave(f)` stores the arrival, and `Story_FleetArrivedCheck()` runs at the end of that fleet's
    /// step, unless its arrival started a battle (the loop stops; the flag waits for the next check). The arrivals'
    /// `Story_Special` calls (colonisation, terraforming, scripted fleets, wormholes, protected targets) run in between.
    fn story_arrivals(&mut self, events: &[StarmapEvent]) {
        let mut pending = false;
        for (i, e) in events.iter().enumerate() {
            // An arrival's `Story_Special` runs at its place, before the fleet's arrival check.
            if let StarmapEvent::StorySpecial(data) = *e {
                self.story_special(data);
                continue;
            }
            let (StarmapEvent::Arrived { fleet, kind, target, fleet_id, target_fleet_id }
            | StarmapEvent::ArrivalResaved { fleet, kind, target, fleet_id, target_fleet_id }) = *e
            else {
                continue;
            };
            if pending && matches!(e, StarmapEvent::Arrived { .. }) {
                self.with_story(|st, w| st.fleet_arrived_check(&ScenarioStories, w));
            }
            if !(0..=2).contains(&kind) {
                pending = false;
                continue;
            }
            let arrival = crate::story::Arrival {
                fleet_id: fleet_id as i32,
                fleet: fleet as i32,
                target,
                target_type: kind as i32,
                target_fleet_id: target_fleet_id as i32,
            };
            self.with_story(|st, _| {
                st.arrival = arrival;
                st.flags[crate::story::flag::ARRIVED] = true;
            });
            let battle = events[i + 1..].iter().take_while(|x| !matches!(x, StarmapEvent::Arrived { .. })).any(|x| {
                matches!(*x, StarmapEvent::SpaceBattle { fleet: g, .. } | StarmapEvent::PlanetAttack { fleet: g, .. } if g == fleet)
            });
            pending = !battle;
        }
        if pending {
            self.with_story(|st, w| st.fleet_arrived_check(&ScenarioStories, w));
        }
    }

    fn production(&mut self, g: &Globals, speed: i32) {
        let ctx = self.production_context(g, speed);
        let mut yard = self.shipyard();
        let mut money = self.state.treasuries();
        let events = yard.tick(&mut money, &ctx);
        self.state.store_treasuries(&money);
        self.store_shipyard(&yard, &events);
        self.events.extend(events.into_iter().map(GameEvent::Production));
    }

    fn production_context(&self, g: &Globals, speed: i32) -> ProductionContext {
        let d = g.difficulty.max(0) as usize;
        let name = self.fleet_name();
        ProductionContext {
            speed_mult: speed,
            day: g.day,
            multiplayer: g.multiplayer,
            human_controlled: g.human,
            difficulty_production: self.rules.difficulty_production.get(d).copied().unwrap_or(1.0),
            difficulty_money: self.rules.difficulty_money.get(d).copied().unwrap_or(1.0),
            default_fleet_name: name.clone(),
            numbered_fleet_name: name,
        }
    }

    /// Production's view: planets, fleets, ships, vehicles, designs and the per-planet/per-empire tables.
    fn shipyard(&self) -> Shipyard {
        let s = &self.state;
        let m = &s.mem;
        let combat = s.combat();
        Shipyard {
            planets: (0..=s.planet_count()).map(|p| yard_planet(&m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE))).collect(),
            fleets: s.fleets.iter().map(|r| YardFleet { fleet: crate::fleet::Fleet::from_record(r), totals: crate::spacewar::FleetTotals::from_record(r) }).collect(),
            ships: combat.ships.clone(),
            vehicles: combat.vehicles.clone(),
            designs: combat.designs.clone(),
            last_fleet: (0..crate::colony_side::PLANET_SLOTS).map(|p| m.i32(va::LAST_NEAR_FLEET + 4 * p as u32)).collect(),
            fleet_numbers: m.i32s(va::FLEET_NUMBERS),
            colony_ship_pool: m.i32s(va::COLONY_SHIP_POOL),
            terraformer_pool: m.i32s(va::TERRAFORMER_POOL),
        }
    }

    /// The bytes of an appended record before the code writes it ([`Game::uninit`]; zero without it).
    fn fresh<const N: usize>(&mut self, what: Fresh) -> [u8; N] {
        let mut out = [0; N];
        if let Some(f) = self.uninit.as_mut() {
            let b = f(what);
            let n = b.len().min(N);
            out[..n].copy_from_slice(&b[..n]);
        }
        out
    }

    /// Production's view back into the state. `events` tell which fleet and ship slots `FlottaNew`/`HajoNew` (re)used:
    /// they clear bytes the views don't hold (fleet `+0x2c/+0x2d`, ship `+4`); appended records start from
    /// [`Game::uninit`].
    fn store_shipyard(&mut self, yard: &Shipyard, events: &[ProductionEvent]) {
        for i in self.state.fleets.len()..yard.fleets.len() {
            let r = self.fresh::<0x80>(Fresh::Fleet(i));
            self.state.fleets.push(r);
        }
        for i in self.state.ships.len()..yard.ships.len() {
            let r = self.fresh::<6>(Fresh::Ship(i));
            self.state.ships.push(r);
        }
        for e in events {
            match *e {
                ProductionEvent::FleetCreated { fleet } => self.state.fleets[fleet][0x2c..0x2e].fill(0),
                ProductionEvent::ShipCreated { ship } => self.state.ships[ship][4] = 0,
                _ => {}
            }
        }
        let st = &mut self.state;
        for (p, y) in yard.planets.iter().enumerate() {
            let at = va::PLANETS + (p * PLANET_SIZE) as u32;
            let mut r = st.mem.read(at, PLANET_SIZE);
            put_yard_planet(&mut r, y);
            st.mem.write(at, &r);
        }
        st.fleets.resize(yard.fleets.len(), [0; 0x80]);
        st.mem.set_i32(va::FLEET_COUNT, yard.fleets.len() as i32 - 1);
        for (r, y) in st.fleets.iter_mut().zip(&yard.fleets) {
            put_fleet(r, &y.fleet);
            fleet_totals_put(r, &y.totals);
        }
        let mut cw = st.combat();
        cw.ships = yard.ships.iter().map(|s| Ship { fleet: s.fleet, design: s.design }).collect();
        cw.vehicles = yard.vehicles.iter().map(|v| Vehicle { fleet: v.fleet, design: v.design, count: v.count }).collect();
        cw.colony_ship_pool = yard.colony_ship_pool;
        cw.terraformer_pool = yard.terraformer_pool;
        // The fleet records were written above; keep their totals.
        cw.fleets = st.fleets.iter().map(|r| crate::save::combat_fleet(r)).collect();
        st.store_combat(&cw);
        st.mem.set_i32s(va::LAST_NEAR_FLEET, &yard.last_fleet);
        st.mem.set_i32s(va::FLEET_NUMBERS, &yard.fleet_numbers);
    }
}

/// The trade market's view of the game (`re/rules/trade.md`): everything lives in the save state, so a delivery's
/// units and its story check see what the market wrote before them.
struct TradeView<'a, 'r> {
    game: &'a mut Game<'r>,
}

impl Market for TradeView<'_, '_> {
    fn rnd(&mut self, lo: i32, hi: i32) -> i32 {
        let mut r = self.game.state.rng();
        let v = r.range(trade::STREAM, lo, hi);
        self.game.state.store_rng(&r);
        v
    }
    fn libc_rand(&mut self) -> i32 {
        self.game.libc.rand()
    }
    fn planet_count(&self) -> usize {
        self.game.state.planet_count()
    }
    fn planet_owner(&self, p: i32) -> u8 {
        self.game.state.mem.u8(va::PLANETS.wrapping_add((p as u32).wrapping_mul(PLANET_SIZE as u32)).wrapping_add(0x20))
    }
    fn working_buildings(&self, p: usize, kind: u8) -> i32 {
        self.game.state.working_buildings(p, kind)
    }
    fn tech_status(&self, t: i32, e: usize) -> u8 {
        if !(0..crate::save::TECHS as i32).contains(&t) {
            return 0;
        }
        self.game.state.mem.u8(va::TECHS + (t as usize * crate::save::TECH_SIZE) as u32 + 0x45 + e as u32)
    }
    fn tech_name(&self, t: i32) -> Vec<u8> {
        if !(0..crate::save::TECHS as i32).contains(&t) {
            return vec![0];
        }
        let mut n = self.game.state.mem.read(va::TECHS + (t as usize * crate::save::TECH_SIZE) as u32, 0x40);
        n.truncate(n.iter().position(|&c| c == 0).unwrap_or(n.len() - 1) + 1);
        n
    }
    fn hull_tech(&self, hull: i32) -> i32 {
        usize::try_from(hull).ok().and_then(|h| self.game.rules.ai.design.hulls.get(h)).map_or(0, |h| h.tech)
    }
    fn design_count(&self, e: usize) -> usize {
        self.game.state.designs.get(e).map_or(0, |t| t.len().saturating_sub(1))
    }
    fn design(&self, e: usize, d: usize) -> Option<trade::Record> {
        self.game.state.designs.get(e)?.get(d).copied()
    }
    fn set_design(&mut self, e: usize, d: usize, r: &trade::Record) {
        if let Some(x) = self.game.state.designs.get_mut(e).and_then(|t| t.get_mut(d)) {
            *x = *r;
        }
    }
    fn new_design(&mut self, e: usize, id: i32) -> usize {
        let race = self.game.state.races()[e.min(EMPIRES - 1)];
        let g = &mut *self.game;
        if g.state.designs.len() <= e {
            g.state.designs.resize(e + 1, Vec::new());
        }
        let uninit = &mut g.uninit;
        let d = crate::spacewar::new_design_record(&mut g.state.designs[e], race, id, |n| {
            let mut out = [0; 0x84];
            if let Some(f) = uninit.as_mut() {
                let b = f(Fresh::Design { empire: e, design: n });
                let k = b.len().min(0x84);
                out[..k].copy_from_slice(&b[..k]);
            }
            out
        });
        let n = g.state.designs[e].len() as i32 - 1;
        g.state.mem.set_i32(va::DESIGN_COUNT + 4 * e as u32, n);
        d
    }
    fn recompute_design(&mut self, e: usize, d: usize, price: bool) {
        let race = self.game.state.races()[e.min(EMPIRES - 1)];
        let rules = &self.game.rules.ai.design;
        if let Some(x) = self.game.state.designs.get_mut(e).and_then(|t| t.get_mut(d)) {
            let mut des = crate::spacewar::Design::from_bytes(x);
            des.recompute(rules, race, price);
            *x = des.to_bytes();
        }
    }
    fn money(&self, e: usize) -> f64 {
        self.game.state.treasuries()[e.min(EMPIRES - 1)].available(0)
    }
    fn add_money(&mut self, e: usize, x: f64) {
        let income = self.game.ai_income(e);
        let mut t = self.game.state.treasuries();
        t[e.min(EMPIRES - 1)].add(0, x, income);
        self.game.state.store_treasuries(&t);
    }
    fn deliver(&mut self, e: usize, planet: i32, d: usize, units: i32) {
        let g = self.game.globals();
        let ctx = self.game.production_context(&g, g.speed);
        let mut yard = self.game.shipyard();
        let p = planet.max(0) as usize;
        if p < yard.planets.len() {
            if let Some(des) = yard.designs.get(e).and_then(|t| t.get(d)).cloned() {
                let f = yard.delivery_fleet(p, 6.0, true, true, false, false, 0);
                let mut ev = Vec::new();
                yard.build(p, f, d as i16, units, 0, des.class == 1, &des, true, &ctx, &mut ev);
                self.game.store_shipyard(&yard, &ev);
                self.game.events.extend(ev.into_iter().map(GameEvent::Production));
            }
        }
        self.game.story_special([0x13, e as i32, d as i32, units, planet, 0, 0, 0]);
    }
}

fn yard_planet(r: &[u8]) -> YardPlanet {
    let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    YardPlanet {
        owner: r[0x20],
        x: f32::from_bits(i32_at(0x18) as u32),
        y: f32::from_bits(i32_at(0x1c) as u32),
        halted: r[0x64],
        capacity: [i32_at(0x70), i32_at(0x74)],
        queue: Queue::from_bytes(&r[0x78..0xb8]),
        garrison: i32_at(0x58),
        garrison_strength: i32_at(0x5c),
        stations: std::array::from_fn(|k| i16::from_le_bytes([r[0x100 + 2 * k], r[0x101 + 2 * k]])),
        station_count: i32_at(0x114),
    }
}

fn put_yard_planet(r: &mut [u8], p: &YardPlanet) {
    r[0x78..0xb8].copy_from_slice(&p.queue.to_bytes());
    r[0x58..0x5c].copy_from_slice(&p.garrison.to_le_bytes());
    r[0x5c..0x60].copy_from_slice(&p.garrison_strength.to_le_bytes());
    for (k, s) in p.stations.iter().enumerate() {
        r[0x100 + 2 * k..0x102 + 2 * k].copy_from_slice(&s.to_le_bytes());
    }
    r[0x114..0x118].copy_from_slice(&p.station_count.to_le_bytes());
}

/// The starmap's arrival hooks (`fleet-movement.md` §4.3): colonisation, terraforming and traders through the colony
/// world; the units of merges and deletions (`HajoAtpakol`, `TankAtpakol`, `HajoDelete`) through the combat world,
/// whose view of the touched fleets is refreshed from the records first and copied back after.
struct StarmapHooks<'a> {
    colony: ColonyArrivals<'a>,
    combat: &'a mut CombatWorld,
    /// `OsszHanySajatFlotta`.
    fleet_numbers: &'a mut [i32; EMPIRES],
    /// Decrements of `OsszHanySajatFlotta[0xFF]` (exe 0xBB44AC = `Tankok[96].count`) while `Tankok` is shorter.
    overflow: i32,
    /// `(into, from)` per merge, for `AktFleet`.
    merges: Vec<(usize, usize)>,
}

impl StarmapHooks<'_> {
    fn sync_in(&mut self, map: &Starmap, fleets: &[usize]) {
        let cw = &mut *self.combat;
        if cw.fleets.len() < map.fleets.len() {
            cw.fleets.resize(map.fleets.len(), CombatFleet { owner: DELETED, ..Default::default() });
        }
        for &f in fleets {
            let (r, c) = (&map.fleets[f], &mut cw.fleets[f]);
            (c.owner, c.flags, c.x, c.y) = (r.owner, r.flags, r.x, r.y);
            (c.colony_ships, c.terraformers) = (r.colony_ships, r.terraformers);
        }
        cw.colony_ship_pool = map.colony_ship_pool;
        cw.terraformer_pool = map.terraformer_pool;
    }

    fn sync_out(&self, map: &mut Starmap, fleets: &[usize]) {
        let cw = &*self.combat;
        for &f in fleets {
            let (r, c) = (&mut map.fleets[f], &cw.fleets[f]);
            r.flags = c.flags;
            (r.colony_ships, r.terraformers) = (c.colony_ships, c.terraformers);
            (r.ship_count, r.strength, r.ground_strength) = (c.totals.ships, c.totals.attack, c.totals.ground_strength);
        }
        map.colony_ship_pool = cw.colony_ship_pool;
        map.terraformer_pool = cw.terraformer_pool;
    }

    /// `OsszHanySajatFlotta[owner] −= 1`; owner 0xFF reads `Tankok[96].count`.
    fn count_down(&mut self, owner: u8) {
        match self.fleet_numbers.get_mut(owner as usize) {
            Some(c) => *c -= 1,
            None => match self.combat.vehicles.get_mut(COUNTER_OVERFLOW_VEHICLE) {
                Some(v) => v.count -= 1,
                None => self.overflow += 1,
            },
        }
    }
}

/// `OsszHanySajatFlotta[0xFF]` is `Tankok[96].count` (`ai::units`).
const COUNTER_OVERFLOW_VEHICLE: usize = 96;

impl Arrivals for StarmapHooks<'_> {
    fn colonise(&mut self, p: usize, owner: u8, star: &mut StarPlanet, rng: &mut Streams) {
        self.colony.colonise(p, owner, star, rng);
    }
    fn terraform(&mut self, p: usize, owner: u8) -> u8 {
        self.colony.terraform(p, owner)
    }
    fn trader_paid(&mut self, empire: u8, amount: i32) {
        self.colony.trader_paid(empire, amount);
    }
    fn merge_units(&mut self, into: usize, from: usize, map: &mut Starmap) {
        self.sync_in(map, &[into, from]);
        self.combat.merge_units(into, from);
        self.sync_out(map, &[into, from]);
        self.count_down(map.fleets[from].owner);
        self.merges.push((into, from));
    }
    /// The ships go in ship order; the last `HajoDelete` of an AI's fleet re-enters `FlottaDelete`, which counts the
    /// owner down and frees the slot, so the outer call then counts down `[0xFF]` [original bug].
    fn delete_units(&mut self, f: usize, map: &mut Starmap) {
        self.sync_in(map, &[f]);
        let owner = map.fleets[f].owner;
        let cw = &mut *self.combat;
        for i in 1..cw.ships.len() {
            let s = cw.ships[i];
            if s.fleet >= 0 && s.fleet as usize == f && s.design != NO_DESIGN {
                cw.delete_ship(i);
            }
        }
        for k in 1..cw.vehicles.len() {
            let v = cw.vehicles[k];
            if v.fleet >= 0 && v.fleet as usize == f && v.design != NO_DESIGN {
                cw.delete_vehicles(v.fleet, v.design, v.count);
            }
        }
        let reentered = !cw.fleets[f].alive();
        self.sync_out(map, &[f]);
        self.count_down(owner);
        if reentered {
            self.count_down(DELETED);
        }
    }
}

/// The AI's hooks: war declarations are recorded (diplomacy carries them out when the replay meets one); appended
/// records come from [`Game::uninit`].
struct WarHooks<'a, 'r> {
    wars: Vec<(usize, usize)>,
    uninit: Option<&'a mut Box<dyn FnMut(Fresh) -> Vec<u8> + 'r>>,
}

impl WarHooks<'_, '_> {
    fn fresh<const N: usize>(&mut self, what: Fresh) -> [u8; N] {
        let mut out = [0; N];
        if let Some(f) = self.uninit.as_mut() {
            let b = f(what);
            let n = b.len().min(N);
            out[..n].copy_from_slice(&b[..n]);
        }
        out
    }
}

impl AiHooks for WarHooks<'_, '_> {
    fn declare_war(&mut self, _w: &mut ai::World<'_>, a: usize, b: usize) {
        self.wars.push((a, b));
    }

    fn fresh_fleet(&mut self, f: usize) -> [u8; 0x80] {
        self.fresh(Fresh::Fleet(f))
    }

    fn fresh_design(&mut self, e: usize, d: usize) -> [u8; 0x84] {
        self.fresh(Fresh::Design { empire: e, design: d })
    }
}

/// The story engine's view of the state.
struct StoryView<'a> {
    state: &'a SaveState,
    g: &'a Globals,
    rng: &'a mut Streams,
    events: Vec<StoryEvent>,
}

impl StoryView<'_> {
    fn planet(&self, p: i32) -> Vec<u8> {
        self.state.mem.read(va::PLANETS + (p.max(0) as usize * PLANET_SIZE) as u32, PLANET_SIZE)
    }
    fn fleet(&self, f: i32) -> Option<&[u8; 0x80]> {
        self.state.fleets.get(f.max(0) as usize)
    }
}

impl StoryWorld for StoryView<'_> {
    fn ido(&self) -> i32 {
        self.g.tick
    }
    fn canonical(&self) -> bool {
        self.g.canonical
    }
    fn speed_mult(&self) -> i32 {
        self.g.speed
    }
    fn human(&self) -> i32 {
        self.g.en as i32
    }
    fn difficulty(&self) -> i32 {
        self.g.difficulty
    }
    fn multiplayer(&self) -> bool {
        self.g.multiplayer
    }
    fn rng(&mut self) -> &mut Streams {
        self.rng
    }
    fn planet_count(&self) -> i32 {
        self.state.planet_count() as i32
    }
    fn planet_owner(&self, p: i32) -> i32 {
        self.planet(p)[0x20] as i32
    }
    fn planet_value(&self, p: i32) -> i32 {
        i32::from_le_bytes(self.planet(p)[0x28..0x2c].try_into().unwrap())
    }
    fn planet_visibility(&self, p: i32) -> i32 {
        let e = self.g.en;
        let r = self.planet(p);
        i16::from_le_bytes([r[0xd8 + 2 * e], r[0xd9 + 2 * e]]) as i32
    }
    fn fleet_count(&self) -> i32 {
        self.state.fleets.len() as i32 - 1
    }
    fn fleet_owner(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0xff, |r| r[0x15] as i32)
    }
    fn fleet_id(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0, |r| i16::from_le_bytes([r[0x16], r[0x17]]) as i32)
    }
    fn empire_alive(&self, e: i32) -> bool {
        self.g.alive.get(e.max(0) as usize).copied().unwrap_or(false)
    }
    fn planets_of(&self, e: i32) -> i32 {
        self.state.mem.i32(va::EMPIRE_PLANETS + 4 * e.max(0) as u32)
    }
    fn money(&self, e: i32) -> f64 {
        self.state.treasuries()[e.clamp(0, EMPIRES as i32 - 1) as usize].available(0)
    }
    fn tech_status(&self, tech: i32, e: i32) -> i32 {
        self.state.mem.u8(va::TECHS + (tech.max(0) as usize * crate::save::TECH_SIZE) as u32 + 0x45 + e.max(0) as u32) as i32
    }
    fn relation_level(&self, a: i32, b: i32) -> i32 {
        self.state.mem.u8(va::RELATIONS + ((a.max(0) as usize * EMPIRES + b.max(0) as usize) * 0x26) as u32 + 0x25) as i32
    }
    fn planet_infected(&self, p: i32) -> bool {
        let r = self.planet(p);
        i16::from_le_bytes([r[0xb8], r[0xb9]]) > 0
    }
    fn fleet_infected(&self, f: i32) -> bool {
        self.fleet(f).is_some_and(|r| r[0x15] != 0xff && r[0x52] & 4 != 0)
    }
    fn event(&mut self, ev: StoryEvent) {
        self.events.push(ev);
    }
}

// ------------------------------------------------------------------------------------------------ info and messages

impl Game<'_> {
    /// `FRISSIT_Info` (`info-messages.md` §1): the charts sample once a month.
    fn info(&mut self) {
        let month = self.state.mem.i32(va::HONAP);
        if self.state.mem.i32(va::INFO_MONTH) == month {
            return;
        }
        let w = self.state.info_world();
        let mut c = self.state.info_charts();
        c.sample(&w);
        self.state.store_info_charts(&c);
        self.state.mem.set_i32(va::INFO_MONTH, month);
    }

    /// `FRISSIT_Messages` (`info-messages.md` §4): a message past its expiry leaves the inbox and tells the
    /// stories (`Story_MessageElevules`).
    fn inbox(&mut self) {
        let mut b = self.state.inbox();
        let Some(block) = b.expire(self.state.mem.i32(va::IDO), self.globals().message_shown) else { return };
        self.state.store_inbox(&b);
        self.with_story(|st, w| {
            st.message.0 = block;
            st.flags[crate::story::flag::PROMOTED] = true;
            st.check(true, &ScenarioStories, w);
        });
    }

    fn planet_name(&self, p: usize) -> Vec<u8> {
        cstr(&self.state.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, 0x20)).to_vec()
    }

    fn tech_name(&self, t: usize) -> Vec<u8> {
        cstr(&self.state.mem.read(va::TECHS + (t * crate::save::TECH_SIZE) as u32, 0x20)).to_vec()
    }

    fn empire_name(&self, e: u8) -> Vec<u8> {
        self.empire_names.get(e as usize).cloned().unwrap_or_default()
    }

    /// A story script's own line on the scrolling ticker (`info-messages.md` §3.4): its text and important flag.
    pub fn story_line(&self, ev: &StoryEvent) -> Option<(Vec<u8>, bool)> {
        messages::story_line(ev, &self.rules.messages, self)
    }
}

impl messages::LineNames for Game<'_> {
    fn tech(&self, t: i32) -> Vec<u8> {
        if (0..crate::save::TECHS as i32).contains(&t) { self.tech_name(t as usize) } else { Vec::new() }
    }
    fn planet(&self, p: i32) -> Vec<u8> {
        if (0..=self.state.planet_count() as i32).contains(&p) { self.planet_name(p as usize) } else { Vec::new() }
    }
    fn empire(&self, e: i32) -> Vec<u8> {
        self.empire_name(e.clamp(0, 255) as u8)
    }
}

impl Game<'_> {
    /// The messages the tick's systems posted to the human (`ScrollUzenetId` at the sites of
    /// `info-messages.md` §3.2), from the events since `from`, in order. Classic mode only: in canonical mode the
    /// scrolling line is each peer's UI state ([`Game::messages_for`] with its own empire).
    pub fn post_messages(&mut self, from: usize) {
        if self.view != ViewMode::Classic {
            return;
        }
        let posts = self.messages_for(&self.events[from..], self.globals().en);
        if posts.is_empty() {
            return;
        }
        let mut t = self.state.ticker();
        for (id, text, important, interrupt) in posts {
            t.post(&text, id, important, interrupt);
        }
        self.state.store_ticker(&t);
    }

    /// The scrolling messages `events` post for empire `en`: `(id, text, important, interrupt)`. Events the
    /// systems only report for `EN` (unrest warnings, spy reports, contact lost, an enemy approaching) are the
    /// canonical `EN`'s in canonical mode (`re/rules/multiplayer.md` §2).
    pub fn messages_for(&self, events: &[GameEvent], en: usize) -> Vec<(i32, Vec<u8>, bool, bool)> {
        let en = en.min(EMPIRES - 1);
        let m = &self.state.mem;
        let knows = |a: u8| m.u8(va::EMPIRE_KNOWS + (en * EMPIRES) as u32 + a as u32) != 0;
        let debug = m.u8(va::DEBUGX) != 0;
        let is_en = |e: u8| e as usize == en;
        let canonical_en = self.globals().en;
        // A lost contact goes to the fleet's owner (decoded only when there is one).
        let lost = |e: &GameEvent| matches!(e, GameEvent::Starmap(StarmapEvent::ContactLostNetwork { .. }));
        let fleets = if events.iter().any(lost) { self.state.starmap().fleets } else { Vec::new() };
        let fleet_owner = |f: usize| fleets.get(f).map(|x| x.owner as usize);
        // A story line's place in `posts`: its one parameter is the finished text, posted with id 1.
        const STORY_LINE: i32 = i32::MIN;
        // (id, parameters, important, interrupt)
        let mut posts: Vec<(i32, Vec<Vec<u8>>, bool, bool)> = Vec::new();
        for ev in events {
            match ev {
                GameEvent::Research(ResearchEvent::Completed { empire, tech }) if is_en(*empire) => {
                    posts.push((2, vec![self.tech_name(*tech)], false, false))
                }
                GameEvent::Diplomacy(DiploEvent::EmpireDied { empire: e }) if *e < 9 && !is_en(*e) => {
                    posts.push((3, vec![self.empire_name(*e)], false, false))
                }
                GameEvent::Story(StoryEvent::EmpireDies(e)) if (0..9).contains(e) && *e as usize != en => {
                    posts.push((3, vec![self.empire_name(*e as u8)], false, false))
                }
                // The scripts post their own text (`ScrollUzenet(text, 1, imp, 0)`) for the canonical `EN`.
                GameEvent::Story(s) if is_en(canonical_en as u8) => {
                    if let Some((text, important)) = self.story_line(s) {
                        posts.push((STORY_LINE, vec![text], important, false));
                    }
                }
                GameEvent::Diplomacy(DiploEvent::War { a, b }) if (knows(*a) || knows(*b) || debug) && *a < 9 && *b < 9 => {
                    posts.push((6, vec![self.empire_name(*a), self.empire_name(*b)], true, false))
                }
                GameEvent::Diplomacy(DiploEvent::Message { from, to, .. }) if is_en(*to) => {
                    posts.push((10, vec![self.empire_name(*from)], true, false))
                }
                GameEvent::Diplomacy(DiploEvent::Discovered { empire, by }) if is_en(*by) => posts.push((26, vec![self.empire_name(*empire)], true, false)),
                GameEvent::Side(s) | GameEvent::Spy(SpyEvent::Colony(s)) => match *s {
                    SideEvent::Abandoned { planet, owner } if is_en(owner) => posts.push((5, vec![self.planet_name(planet)], true, false)),
                    SideEvent::TechCaptured { empire, tech } if is_en(empire) => posts.push((7, vec![self.tech_name(tech)], false, false)),
                    SideEvent::Revolt { planet, owner } if is_en(owner) => posts.push((8, vec![self.planet_name(planet)], true, true)),
                    SideEvent::Colony { planet, event: ColonyEvent::UnrestWarning } => posts.push((9, vec![self.planet_name(planet)], true, false)),
                    SideEvent::Colony { planet, event: ColonyEvent::SpyExpelled { empire } } if is_en(empire) => {
                        posts.push((13, vec![self.planet_name(planet)], false, false))
                    }
                    SideEvent::Rebelled { planet, new_owner, .. } if is_en(new_owner) => posts.push((19, vec![self.planet_name(planet)], true, false)),
                    SideEvent::YearClosed => {
                        for (id, at) in [(20, va::TAX_LAST), (21, va::TRADE_LAST)] {
                            let v = m.i32(at + 4 * en as u32);
                            if v > 0 {
                                posts.push((id, vec![v.to_string().into_bytes()], false, false));
                            }
                        }
                    }
                    _ => {}
                },
                GameEvent::Spy(SpyEvent::Report(r)) if is_en(canonical_en as u8) => posts.extend(r.messages().into_iter().map(|id| (id, Vec::new(), false, false))),
                GameEvent::Spy(SpyEvent::ReportTo { to, report }) if is_en(*to) => {
                    posts.extend(report.messages().into_iter().map(|id| (id, Vec::new(), false, false)))
                }
                GameEvent::Spy(SpyEvent::Returned { .. }) => posts.push((0x82, Vec::new(), false, false)),
                GameEvent::Spy(SpyEvent::Struck { mission, a, b }) => {
                    let (a, b) = (*a, *b);
                    let building_tech = |kind: i32| self.rules.side.colony.buildings.get(kind as usize).map_or(0, |k| k.tech as usize);
                    let post = match *mission as usize {
                        spy::mission::STEAL_MONEY => Some((0x87, vec![Vec::new()])),
                        spy::mission::SABOTAGE => Some((0x88, vec![self.tech_name(building_tech(b)), self.planet_name(a as usize)])),
                        spy::mission::DESTROY_SHIP => Some((0x89, vec![self.tech_name(a.max(0) as usize)])),
                        spy::mission::REBELLION => Some((0x8b, vec![self.planet_name(a as usize)])),
                        spy::mission::ASSASSINATE => Some((0x8c, vec![Vec::new()])),
                        spy::mission::KIDNAP => Some((0x8d, vec![self.family_names.get(a as usize).cloned().unwrap_or_default()])),
                        spy::mission::MASS_UNREST => Some((0x8e, vec![Vec::new()])),
                        _ => None,
                    };
                    if let Some((id, params)) = post {
                        posts.push((id, params, true, false));
                    }
                }
                GameEvent::Battle(b) => {
                    for (id, planet, important) in b.messages(en as u8) {
                        posts.push((id, vec![planet.map(|p| self.planet_name(p)).unwrap_or_default()], important, false));
                    }
                }
                GameEvent::Starmap(s) | GameEvent::Diplomacy(DiploEvent::Starmap(s)) => match s {

                    StarmapEvent::ContactLost { .. } => posts.push((4, Vec::new(), false, false)),
                    StarmapEvent::ContactLostNetwork { fleet } if fleet_owner(*fleet) == Some(en) => posts.push((4, Vec::new(), false, false)),
                    StarmapEvent::EnemyApproaching { to, .. } if is_en(*to) => posts.push((14, Vec::new(), false, false)),
                    _ => {}
                },
                _ => {}
            }
        }
        posts
            .into_iter()
            .filter_map(|(id, mut params, important, interrupt)| {
                if id == STORY_LINE {
                    return Some((1, params.swap_remove(0), important, interrupt));
                }
                let params: Vec<&[u8]> = params.iter().map(|p| &p[..]).collect();
                self.rules.messages.format(id, &params).map(|text| (id, text, important, interrupt))
            })
            .collect()
    }
}
