//! The client's side of the simulation (ADR 0006): an original save loaded into `ig2-sim`, ticked at the original's
//! fixed 20 Hz, and the starmap read back out as plain snapshots the renderer interpolates between.
//!
//! The client never writes sim state except through the sim's own entry points (`Game::advance_clock`,
//! `Game::run_tick`, `Game::fleet_emptied`) and the player's commands (`ig2_net::Command`, ADR 0007: the
//! original's lockstep messages, which it applies in the next frame's message pass even in single player); the
//! renderer only sees [`Snapshot`]s.

use ig2_formats::save::SaveGame;
use ig2_net::Command;
use ig2_sim::game::{BattleEvent, ColonyOrderEvent, Game, GameEvent, GameRules, OrderEvent};
use ig2_sim::ground::{GroundEvent, Voice};
use ig2_sim::save::{va, SaveState};
use ig2_sim::time::{GameTime, Speed};

/// Empire slots (`EMPIRES` in the sim).
pub const EMPIRES: usize = 19;

/// A planet as the starmap shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanetView {
    /// `Bolygok[index]` (1-based).
    pub index: usize,
    /// Galaxy coordinates (`+0x18`/`+0x1c`).
    pub pos: [f32; 2],
    pub owner: u8,
    /// `+0x22`: surface type.
    pub surface: u8,
    /// `+0x00`: the name (the game's code page, NUL-terminated).
    pub name: String,
    /// Seen by the human: `+0xd8 + 2·EN` (i16) > 0, or `DEBUGX` (`rotate_stars`, `Show_Stars2d3d`; re/screens/starmap.md
    /// §3.4). −1 (never seen; a campaign starts with dozens) and 0 are hidden, the human's own planets too.
    pub known: bool,
    /// `+0xba`: the name blinks (`Show_Stars2d3d`, re/screens/hud.md §4.2).
    pub blink: bool,
    /// `+0xbc` bit 0: never a target (`SelectNearestUnit`'s enemies-only pick skips it).
    pub friendly: bool,
    /// `+0xac`: the rally planet of its new ship fleets (0 = none; re/screens/starmap.md §4.4).
    pub rally: usize,
    /// How the starmap draws it (`+0x17c..+0x185`, re/screens/starmap.md §3).
    pub look: PlanetLook,
}

/// A planet's starmap look: the `.scn` `PLANET` keys `szin bolygomeret csillagmeret SpinSpeed Yaw Dist Speed`
/// (`Init3dStarmap` ELF 0x6db470; re/screens/starmap.md §3).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlanetLook {
    /// `+0x17c` `szin`: the system star's colour, `0xTTBBGGRR` (TT = transparency).
    pub color: u32,
    /// `+0x180` `bolygomeret`: planet size.
    pub size: u8,
    /// `+0x181` `csillagmeret`: star size.
    pub star_size: u8,
    /// `+0x182` `SpinSpeed`.
    pub spin: u8,
    /// `+0x184` `Dist`: orbit radius code.
    pub dist: u8,
    /// `+0x185` `Speed`: orbit period code.
    pub speed: u8,
}

impl PlanetLook {
    fn read(r: &[u8]) -> Self {
        Self { color: u32::from_le_bytes(r[0x17c..0x180].try_into().unwrap()), size: r[0x180], star_size: r[0x181], spin: r[0x182], dist: r[0x184], speed: r[0x185] }
    }
}

/// The starmap camera as the save holds it (chunks `StarmapXStart/YStart/Rotate/Pitch/Zoom/PlanetKering`, exe
/// 0xB9A968..; re/screens/starmap.md §2).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SavedView {
    pub x: f32,
    pub y: f32,
    pub rotate: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub orbit: f32,
}

impl SavedView {
    /// From the save's chunks; `LoadSave` resets NaN or |x|, |y| > 100000 to 0 and clamps the zoom to 580.
    pub fn read(save: &SaveGame) -> Self {
        Self::from_chunks(|n| save.chunk(n).map(<[u8]>::to_vec))
    }

    /// From a chunk lookup (an original save's, or a state's own `SaveState::chunks`).
    pub fn from_chunks(chunk: impl Fn(&str) -> Option<Vec<u8>>) -> Self {
        let f = |n: &str| chunk(n).and_then(|b| b.get(..4).map(|b| f32::from_le_bytes(b.try_into().unwrap()))).unwrap_or(0.0);
        let sane = |v: f32| if v.is_nan() || v.abs() > 100000.0 { 0.0 } else { v };
        Self {
            x: sane(f("StarmapXStart")),
            y: sane(f("StarmapYStart")),
            rotate: sane(f("StarmapRotate")),
            pitch: sane(f("StarmapPitch")),
            zoom: sane(f("StarmapZoom")).min(580.0),
            orbit: sane(f("StarmapPlanetKering")),
        }
    }
}

/// Where a scenario parks the planets it doesn't use (quick-1.sav: 6 of its 95 at (−300, −300), off the map; the
/// traders fly there to leave). Not drawn and not part of the map's extent.
pub const OFF_MAP: [f32; 2] = [-300.0, -300.0];

impl PlanetView {
    pub fn on_map(&self) -> bool {
        self.pos != OFF_MAP
    }
}

/// A fleet as the starmap shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct FleetView {
    /// `Flottak[slot]`: the identity interpolation pairs by.
    pub slot: usize,
    pub pos: [f32; 2],
    pub owner: u8,
    /// Seen by the human (`+0x19 + EN`), or the human's own.
    pub visible: bool,
    /// Where an order sends it (`+0x44`/`+0x48` for a point order), when it moves.
    pub target: Option<[f32; 2]>,
    /// `+0x40`: the order (2 = attack: a red route).
    pub order: i16,
    /// `+0x52`: bit 5 = small sprite and no route, bit 11 = the name is shown (re/screens/hud.md §4.3), bit 14 =
    /// the special sprite column (re/screens/starmap.md §5).
    pub flags: u16,
    /// `+0`: the name (the game's code page).
    pub name: Vec<u8>,
    /// `+0x54`: ships aboard; `+0x16`: the story id.
    pub ships: i32,
    pub story_id: i16,
    /// `+0x42`: the order's target kind (0 point, 1 planet, 2 fleet) and `(int)+0x44`.
    pub kind: i16,
    pub target_index: usize,
}

/// The starmap at one sim tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// `Ido2` of the state.
    pub tick: i32,
    pub time: i32,
    pub human: u8,
    pub planets: Vec<PlanetView>,
    pub fleets: Vec<FleetView>,
    /// `StarmapPlanetKering`: the planets' orbit clock (advanced by the client, the sim doesn't port the view).
    pub orbit: f32,
    /// The radar ranges the starmap shows (re/screens/starmap.md §6.3).
    pub radar: crate::starmap_radar::Rings,
    /// The waypoint chains (`+0x50`, `Waypointek` +0/+4, next +8) of the fleets whose routes are drawn (the human's
    /// and story fleet 0x1f88), by slot: `Show_Flottak3d` draws the selected one's (re/screens/starmap.md §4.3).
    pub waypoints: Vec<(usize, Vec<[f32; 2]>)>,
    /// `MultiPlayerMode` (exe 0xBAE2C0) without `DEBUGX`: `Show_Flottak2d` draws only the human's own fleets' names.
    pub own_names_only: bool,
}

impl Snapshot {
    /// `Show_Atiranyitas3d` (ELF 0x6e34a0, exe 0x5a4fb0): from each of the human's planets with a rally planet to
    /// that planet, selected or not, seen or not (re/screens/starmap.md §4.4).
    pub fn rally_arrows(&self) -> impl Iterator<Item = ([f32; 2], [f32; 2])> + '_ {
        let at = |i: usize| self.planets.iter().find(|p| p.index == i).map(|p| p.pos);
        self.planets.iter().filter(|p| p.owner == self.human && p.rally != 0).filter_map(move |p| Some((p.pos, at(p.rally)?)))
    }

    pub fn read(game: &Game) -> Self {
        let state = &game.state;
        let m = &state.mem;
        let en = m.i32(va::EN).clamp(0, EMPIRES as i32 - 1) as usize;
        let map = state.starmap();
        let n = state.planet_count();
        let debug = m.u8(va::DEBUGX) != 0;
        let planets = (1..=n)
            .map(|p| {
                let r = m.read(va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, ig2_sim::save::PLANET_SIZE);
                let name: Vec<u8> = r[..0x18].iter().copied().take_while(|&b| b != 0).collect();
                let sp = &map.planets[p];
                PlanetView {
                    index: p,
                    pos: [sp.x, sp.y],
                    owner: sp.owner,
                    surface: r[0x22],
                    name: name.iter().map(|&b| b as char).collect(),
                    known: sp.visible[en] > 0 || debug,
                    blink: r[0xba] != 0,
                    friendly: r[0xbc] & 1 != 0,
                    rally: r[0xac] as usize,
                    look: PlanetLook::read(&r),
                }
            })
            .collect();
        let fleets = map
            .fleets
            .iter()
            .enumerate()
            .skip(1)
            // Bit 10 of `+0x52` keeps a fleet off the map (`Show_Flottak2d` ELF 0x6e3d30).
            .filter(|(_, f)| f.alive() && f.flags & 0x400 == 0)
            .map(|(slot, f)| FleetView {
                slot,
                pos: [f.x, f.y],
                owner: f.owner,
                visible: f.owner as usize == en || (f.visible[en] != 0 && f.state & 1 == 0),
                target: target(&map, f),
                order: f.order,
                flags: f.flags,
                name: f.name.iter().copied().take_while(|&b| b != 0).collect(),
                ships: f.ship_count,
                story_id: f.story_id,
                kind: f.kind,
                target_index: (f.tx as i32).max(0) as usize,
            })
            .collect();
        let (circles, planet_circles) = crate::starmap_radar::circles(state, &map, &game.radar_tables(), en);
        let radar = crate::starmap_radar::rings(&circles, planet_circles);
        let waypoints = map
            .fleets
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, f)| f.alive() && f.first_waypoint > 0 && (f.owner as usize == en || f.story_id == 0x1f88))
            .map(|(slot, f)| {
                let mut chain = Vec::new();
                let mut w = f.first_waypoint;
                // A free node's `next` is −1; the length cap guards a corrupt chain.
                while w > 0 && (w as usize) < map.waypoints.len() && chain.len() < map.waypoints.len() {
                    let n = map.waypoints[w as usize];
                    chain.push([n.x, n.y]);
                    w = n.next;
                }
                (slot, chain)
            })
            .collect();
        let own_names_only = m.u8(va::MULTIPLAYER) != 0 && !debug;
        Self { tick: m.i32(va::IDO2), time: m.i32(va::IDO), human: en as u8, planets, fleets, orbit: 0.0, radar, waypoints, own_names_only }
    }

    /// The fleets between `prev` (alpha 0) and `self` (alpha 1). A fleet pairs with the same slot in `prev`; one
    /// that is new, or jumped further than a tick's travel (a wormhole, a reused slot), is drawn where it is now.
    pub fn fleets_at(&self, prev: &Snapshot, alpha: f32) -> Vec<FleetView> {
        let a = alpha.clamp(0.0, 1.0);
        self.fleets
            .iter()
            .map(|f| {
                let before = prev.fleets.binary_search_by_key(&f.slot, |p| p.slot).ok().map(|i| &prev.fleets[i]);
                let mut out = f.clone();
                if let Some(b) = before.filter(|b| b.owner == f.owner) {
                    let (dx, dy) = (f.pos[0] - b.pos[0], f.pos[1] - b.pos[1]);
                    if dx * dx + dy * dy <= MAX_STEP * MAX_STEP {
                        out.pos = [b.pos[0] + dx * a, b.pos[1] + dy * a];
                    }
                }
                out
            })
            .collect()
    }

    pub fn date(&self) -> (i32, i32, i32) {
        let t = GameTime(self.time);
        (t.year(), t.month(), t.day())
    }
}

/// Where a fleet's order sends it: a point, a planet or another fleet (`+0x40` order, `+0x42` kind, `+0x44` target).
fn target(map: &ig2_sim::fleet::Starmap, f: &ig2_sim::fleet::Fleet) -> Option<[f32; 2]> {
    use ig2_sim::fleet::order;
    if f.order == order::NONE {
        return None;
    }
    let i = (f.tx as i32).max(0) as usize;
    match f.kind {
        order::POINT => Some([f.tx, f.ty]),
        order::PLANET => map.planets.get(i).filter(|_| i > 0).map(|p| [p.x, p.y]),
        order::FLEET => map.fleets.get(i).filter(|t| i > 0 && t.alive()).map(|t| [t.x, t.y]),
        _ => None,
    }
}

/// A move longer than this in one tick isn't drawn as a glide (galaxy units; the fastest fleet at ×16 moves far
/// less).
const MAX_STEP: f32 = 64.0;

/// `HangLejatszasKell`: the start voice a new campaign's first starmap entry plays.
const HANG_LEJATSZAS_KELL: u32 = 0xBAE26C;

/// The running game: the sim plus the last two snapshots.
pub struct World<'r> {
    pub game: Game<'r>,
    pub prev: Snapshot,
    pub curr: Snapshot,
    /// The camera the save was left with.
    pub view: SavedView,
    /// The colony screen's planet, if it is open, and its last two snapshots.
    pub colony: Option<usize>,
    pub colony_prev: Option<crate::colony::ColonySnap>,
    pub colony_curr: Option<crate::colony::ColonySnap>,
    /// What the 2D screens show (charts, inbox, the message line's queue).
    pub ui: crate::ui::UiSnap,
    /// The player's commands for the next message pass ([`World::pump`]).
    pub outbox: Vec<Command>,
    /// What the last pass's orders did for this player's screen (a split's new fleet).
    pub order_events: Vec<OrderEvent>,
    /// What the last pass's colony orders did (`MULTIGET_Colony`).
    pub colony_events: Vec<ColonyOrderEvent>,
    /// `ScrollHibaUzenet`: the screens' own error lines for the message ticker, shown before the sim's.
    pub local_lines: Vec<Vec<u8>>,
    /// The ids of the sim's messages the line took (`ScrollUzenetek+0x38`), for their voices (`FRISSIT_Scroll2`).
    pub popped: Vec<i32>,
    /// What the real-time battles did since the screen last looked (start, end).
    pub realtime: Vec<ig2_sim::game::RealTimeEvent>,
    /// A ground war went to the real-time screen since the window last looked (`GroundWarIndit`'s `StartGrWar`
    /// branch, single player's included): it ends the tutorial's `WAITFORBATTLE`.
    pub ground_war_routed: bool,
    /// The story's UI effects since the window last took them (single player).
    pub story_ui: Vec<StoryUi>,
    /// The videos the sim asked for since the window last took them (`st_ShowAnim`, the game's end).
    pub videos: Vec<crate::video::Request>,
    /// The ground wars' voice lines since the window last took them (planet, `PlaySpeechGrWar` site).
    pub ground_voices: Vec<(usize, Voice)>,
    /// The ground wars' sounds and flashes since the window last took them (planet, [`GroundEvent::Sound`] or
    /// [`GroundEvent::Flash`]).
    pub ground_fx: Vec<(usize, GroundEvent)>,
    /// `HangLejatszasKell` (exe 0xbae26c, `InitNewGame` step 6: the campaign type) as the first `Starmap_Belep`
    /// takes it for the start voice ([`crate::audio::Audio::play_start_voice`]).
    pub start_voice: Option<i32>,
    /// A multiplayer game (ADR 0014 §5): the session whose peer runs the sim, and the session's clock. The world
    /// holds the game between pumps (the peer holds a stand-in), so the screens read it as in single player.
    pub net: Option<(crate::mp_lobby::Session<'r>, std::time::Instant)>,
    /// The session's hold status the message line last told (ours: who paused, dropped, left or came back).
    pub net_seen: ig2_net::session::HoldStatus,
    /// The chat lines received (`name: text`) for the ring (`MULTIGET_Starmap`'s 0x3fc: `AddSzoveg(.., 3)`).
    pub chat_lines: Vec<(Vec<u8>, usize)>,
    /// The session ended under this machine (the host is gone, or the link failed): the window goes to the main menu.
    pub net_over: bool,
    /// The slot of the session's host at the last pump (a host migration changes it, rules/multiplayer.md §7.3).
    pub net_host: Option<u8>,
    /// `MainScenarioDescriptor` (exe 0xbbd140) is set: the game was started as a scenario from the main menu
    /// (`StoreScenarioDescription` 0x504120); `InitNewGame` (every other start and every `LoadSave`) clears it.
    pub scenario: bool,
}

/// The session as the wait overlay sees it (`crate::mp_wait`, [`World::wait_view`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitView {
    /// The local peer's turn.
    pub turn: u64,
    /// A player's pause holds the session.
    pub paused: bool,
    /// The session awaits a dropped or migrating player.
    pub reconnecting: bool,
    pub host: bool,
    /// The late players `(empire, name)`.
    pub late: Vec<(u8, String)>,
}

/// A story effect the window carries out (`rules/campaign.md`): the screens, the pause and the
/// registry are the UI's. Single player only: in canonical MP the story is the canonical `EN`'s, and the original's MP
/// ignores `SetIdoPause(1, 0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryUi {
    /// Story 400 at counter 4002: `SaveRegistrySetting(StoryRegString(campaign), value)` (exe 0x51b860, 0x5c5540):
    /// `Fable_<campaign>` in the options registry, the storyline the next campaign avoids (`InitStory`).
    Storyline { campaign: i32, value: i32 },
    /// `st_Foldrenges(p)`: `AktKepKell := 3`, `AktPlanetKell := p`, `Colony_foldrenges := p` (the camera shakes).
    Earthquake(usize),
    /// Story 854's riot: `AktKepKell := 3`, `AktPlanetKell := p`, `SetIdoPause(1, 0)` (exe 0x541300).
    Hold(usize),
    /// `NewMessage` of an automatic kind (exe 0x508fe0): `AktKepKell := 10`, the messages screen plays it.
    Messages,
}

impl StoryUi {
    fn of(e: &GameEvent) -> Option<Self> {
        use ig2_sim::story::StoryEvent;
        match e {
            GameEvent::Story(StoryEvent::SaveStoryline { campaign, value }) => Some(Self::Storyline { campaign: *campaign, value: *value }),
            GameEvent::Story(StoryEvent::Earthquake(p)) if *p > 0 => Some(Self::Earthquake(*p as usize)),
            GameEvent::Story(StoryEvent::HoldGroundWar { planet }) if *planet > 0 => Some(Self::Hold(*planet as usize)),
            GameEvent::MessagesScreen => Some(Self::Messages),
            _ => None,
        }
    }
}

/// The message ring's line (`AddSzoveg(text, colour)`, exe 0x591360) for a story's starmap text (`StoryEvent::Notice`)
/// on the machine of empire `en` (each machine of the original tests its own `EN`; `st_Multi_include` exe 0x5ee4d0,
/// stories 834/856 exe 0x5de61e / 0x5e31a2), `None` when it isn't this machine's.
fn notice_line(n: &ig2_sim::story::Notice, en: i32, game: &Game) -> Option<(Vec<u8>, usize)> {
    use ig2_sim::story::Notice;
    let m = &game.state.mem;
    let player = |e: i32| ig2_sim::messages::cstr(&m.read(0xBAE588 + 11 * e.clamp(0, 18) as u32, 11)).to_vec();
    let cat = |parts: &[&[u8]]| parts.concat();
    Some(match *n {
        Notice::Winner { empire, amazing, colour } => {
            let t = match (empire == en, amazing) {
                (true, true) => b"AMAZING! YOU ARE THE WINNER!".to_vec(),
                (true, false) => b"YOU ARE THE WINNER!".to_vec(),
                (false, _) => cat(&[&player(empire), b" is the winner!"]),
            };
            (t, colour as usize)
        }
        Notice::CannotColonise { planet, by } if by == en => {
            let name = ig2_sim::messages::cstr(&m.read(va::PLANETS + (planet.max(0) as usize * ig2_sim::save::PLANET_SIZE) as u32, 0x20)).to_vec();
            (cat(&[b"You cannot colonize ", &name, b"!"]), 2)
        }
        Notice::FlagTaken { by, from } if by == en => (cat(&[b"You have the flag of ", &player(from), b"!"]), 2),
        Notice::FlagTaken { by, from } if from == en => (cat(&[&player(by), b" has your flag!"]), 2),
        Notice::Touchdown { by } if by == en => (b"TOUCHDOWN!!!".to_vec(), 2),
        Notice::Touchdown { by } if 3 - by == en => (cat(&[&player(by), b" has captured your flag!"]), 2),
        Notice::FlagLost { by } if by == en => (b"You have lost the flag!".to_vec(), 2),
        Notice::FlagLost { by } if 3 - by == en => (b"Your flag has been returned!".to_vec(), 2),
        Notice::StoryText(id) => (game.rules.messages.story_text(id).to_vec(), 1),
        _ => return None,
    })
}

/// Canonical multiplayer: the screens read the local player's `EN` from the state, as every machine of the original
/// does (`re/rules/multiplayer.md` §5); the shared state holds the host's. The world writes the local one (`local`)
/// while it holds the game and puts the shared one back before the game goes into the peer: the sim reads
/// `ViewMode::Canonical`, the hashes and snapshots see the shared state. Classic games are left alone.
fn local_en(game: &mut Game, local: bool) {
    let ig2_sim::game::ViewMode::Canonical { en } = game.view else { return };
    let e = if local { game.local.empire } else { en };
    game.state.mem.set_i32(va::EN, e as i32);
    // The local human's own inbox too (`Game::show_inbox`, re/rules/multiplayer.md §5.3): its messages screen.
    game.show_inbox((local && e != en).then_some(e));
    // And the names it renamed that the lockstep hasn't carried yet (deviation 395).
    game.show_local_names(local);
    // And its own message line (each peer posts its own empire's lines, re/rules/multiplayer.md §2).
    game.show_local_ticker(local);
}

/// The message line's words for a change of the session's hold status (ours, in English: the original's MP had no
/// such lines). `name` = the player of an empire.
fn status_lines(was: &ig2_net::session::HoldStatus, now: &ig2_net::session::HoldStatus, name: impl Fn(u8) -> String) -> Vec<Vec<u8>> {
    let waiting = |s: &ig2_net::session::HoldStatus, e: u8| s.waiting.iter().any(|w| w.0 == e);
    let gone = |s: &ig2_net::session::HoldStatus, e: u8| waiting(s, e) || s.away.contains(&e);
    let mut lines = Vec::new();
    for &e in &now.paused {
        if !was.paused.contains(&e) {
            lines.push(format!("{} paused the game.", name(e)));
        }
    }
    for &e in &was.paused {
        if !now.paused.contains(&e) && !gone(now, e) {
            lines.push(format!("{} resumed the game.", name(e)));
        }
    }
    for &(e, ms) in &now.waiting {
        if !waiting(was, e) {
            lines.push(format!("{} lost the connection: waiting {} s for a rejoin.", name(e), ms.div_ceil(1000)));
        }
    }
    for &e in &now.away {
        if !was.away.contains(&e) {
            lines.push(format!("{} has left the game.", name(e)));
        }
    }
    for e in was.away.iter().copied().chain(was.waiting.iter().map(|w| w.0)) {
        if !gone(now, e) && !now.migrating.contains(&e) {
            lines.push(format!("{} is back.", name(e)));
        }
    }
    lines.into_iter().map(String::into_bytes).collect()
}

/// The ground wars' voice sites among `events` (`GroundSide`, ground-combat.md §11.12).
fn ground_voices(events: &[GameEvent]) -> impl Iterator<Item = (usize, Voice)> + '_ {
    events.iter().filter_map(|e| match *e {
        GameEvent::Battle(BattleEvent::GroundSide { planet, event: GroundEvent::Voice(v) }) => Some((planet, v)),
        _ => None,
    })
}

/// The ground wars' sounds and flashes among `events`.
fn ground_fx(events: &[GameEvent]) -> impl Iterator<Item = (usize, GroundEvent)> + '_ {
    events.iter().filter_map(|e| match *e {
        GameEvent::Battle(BattleEvent::GroundSide { planet, event: event @ (GroundEvent::Sound(_) | GroundEvent::Flash { .. }) }) => Some((planet, event)),
        _ => None,
    })
}

impl<'r> World<'r> {
    /// Load `save` into the sim. `exe_volatile` = globals the save doesn't carry, from the user's exe image.
    pub fn load(save: &SaveGame, rules: &'r GameRules, tech_defaults: &[u8], volatile: &[(u32, Vec<u8>)]) -> Self {
        let mut state = SaveState::from_save(save, tech_defaults);
        state.set_volatile(volatile);
        let mut game = Game::new(state, rules);
        // The client shows single player's real-time battles (re/screens/spacewar.md).
        game.sp_battles.live = true;
        // `LoadSave` ends with `FlottakUjraSzamol` (fleet-movement.md §4.3).
        game.saved();
        Self::from_game(game, SavedView::read(save))
    }

    /// A slot of the Options screen (re/screens/options.md §5): an original `save<n>.sav` as [`World::load`] loads
    /// it, or the remake's own `save<n>.ig2s` (ADR 0007 §4: the snapshot restored as it was saved).
    pub fn load_slot(path: &std::path::Path, rules: &'r GameRules, tech_defaults: &[u8], volatile: &[(u32, Vec<u8>)]) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::load_bytes(&bytes, rules, tech_defaults, volatile).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// [`World::load_slot`] of a slot's bytes.
    pub fn load_bytes(bytes: &[u8], rules: &'r GameRules, tech_defaults: &[u8], volatile: &[(u32, Vec<u8>)]) -> Result<Self, String> {
        if bytes.starts_with(ig2_net::snapshot::MAGIC) {
            let (snap, header) = ig2_net::snapshot::Snapshot::from_file(bytes).map_err(|e| e.to_string())?;
            if header.flags & 1 != 0 {
                return Err("a multiplayer save (resume it from a multiplayer room)".into());
            }
            let (mut game, _) = snap.restore(rules).map_err(|e| e.to_string())?;
            // As `World::load`: the client shows single player's real-time battles.
            game.sp_battles.live = true;
            let chunks = game.state.chunks();
            return Ok(Self::from_game(game, SavedView::from_chunks(|n| chunks.get(n).cloned())));
        }
        let save = SaveGame::parse(bytes).map_err(|e| e.to_string())?;
        Ok(Self::load(&save, rules, tech_defaults, volatile))
    }

    /// A game a start constructor just made (`InitNewGame` and its caller's tail, any mode; re/rules/new-game.md
    /// §1, §6): tick 0, before the first frame. The camera is the one step 52 (`StarmapSetSideView` + the copy to
    /// `StarmapXStart..`) left in the state, the orbit clock 0 (step 31), the selection `StarmapMode` 1 on
    /// `AktPlanet` (the last own planet) as the state holds it. Unlike [`World::load`] no `FlottakUjraSzamol`.
    pub fn new_game(mut game: Game<'r>) -> Self {
        game.sp_battles.live = true;
        // `Starmap_Belep` clears `HangLejatszasKell` before the first tick: the world takes it now.
        let voice = game.state.mem.i32(HANG_LEJATSZAS_KELL);
        game.state.mem.set_i32(HANG_LEJATSZAS_KELL, 0);
        let chunks = game.state.chunks();
        Self { start_voice: (voice > 0).then_some(voice), ..Self::from_game(game, SavedView::from_chunks(|n| chunks.get(n).cloned())) }
    }

    /// `CampaignGameStart`'s tail (exe 0x4f5e90, 0x4f6008/1f/36): the race's intro, `st_ShowAnim` by `EN`, before
    /// the first starmap frame.
    pub fn campaign_intro(&mut self) {
        if let Some(name) = crate::video::intro_name(self.curr.human as i32) {
            self.videos.push(crate::video::Request::File(name.to_string()));
        }
    }

    /// Multiplayer and this machine hosts the session (only the host saves it, [`Self::mp_save`]).
    pub fn mp_host(&self) -> bool {
        matches!(self.net, Some((crate::mp_lobby::Session::Host { .. }, _)))
    }

    /// Multiplayer: the host's MP save (`HostSession::save`: the shared state at the next turn and the session's humans,
    /// `rules/multiplayer.md` §7.2), resumed from a room; `None` in single player. A joiner has none (ours: only the
    /// host saves, so one file holds the session).
    pub fn mp_save(&mut self) -> Option<Result<Vec<u8>, String>> {
        let (net, _) = self.net.as_mut()?;
        let crate::mp_lobby::Session::Host { session, .. } = net else { return Some(Err("Only the host can save a multiplayer game.".into())) };
        local_en(&mut self.game, false);
        std::mem::swap(&mut self.game, &mut session.own.game);
        // The turns it catches up on leave their events in the peer for the next `mp_tick`.
        let r = session.save().map_err(|e| e.to_string());
        std::mem::swap(&mut self.game, &mut session.own.game);
        local_en(&mut self.game, true);
        Some(r)
    }

    /// Multiplayer: this machine's snapshot of the shared state, as its peer hashes it each turn (the play-through's
    /// desync check); `None` in single player.
    pub fn mp_snapshot(&mut self) -> Option<ig2_net::Snapshot> {
        let (net, _) = self.net.as_mut()?;
        local_en(&mut self.game, false);
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        let s = net.peer().snapshot();
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        local_en(&mut self.game, true);
        Some(s)
    }

    /// The state as the remake's save file (`.ig2s`) holds it.
    /// The live starmap camera into the state's `StarmapXStart` … `StarmapPlanetKering` (exe 0xB9A968..), as
    /// `LoadSave(…, 2)` saves them; the view a load reads back.
    pub fn store_view(&mut self, c: &crate::starmap::Camera) {
        self.view = SavedView { x: c.x, y: c.y, rotate: c.rotate, pitch: c.pitch, zoom: c.zoom, orbit: self.curr.orbit };
        let v = self.view;
        let names = ["StarmapXStart", "StarmapYStart", "StarmapRotate", "StarmapPitch", "StarmapZoom", "StarmapPlanetKering"];
        for (n, f) in names.iter().zip([v.x, v.y, v.rotate, v.pitch, v.zoom, v.orbit]) {
            self.game.state.set_ui_chunk(n, &f.to_le_bytes());
        }
    }

    pub fn save_file(&self) -> Vec<u8> {
        ig2_net::snapshot::Snapshot::capture(&self.game, &[], 0, 0).to_file(0, false)
    }

    fn from_game(game: Game<'r>, view: SavedView) -> Self {
        let curr = Snapshot { orbit: view.orbit, ..Snapshot::read(&game) };
        let ui = crate::ui::UiSnap::read(&game.state);
        Self { game, prev: curr.clone(), curr, view, colony: None, colony_prev: None, colony_curr: None, ui, outbox: Vec::new(), order_events: Vec::new(), colony_events: Vec::new(), local_lines: Vec::new(), popped: Vec::new(), realtime: Vec::new(), ground_war_routed: false, story_ui: Vec::new(), videos: Vec::new(), ground_voices: Vec::new(), ground_fx: Vec::new(), start_voice: None, net: None, net_seen: Default::default(), chat_lines: Vec::new(), net_over: false, net_host: None, scenario: false }
    }

    /// A multiplayer game from our room's start (`mp_lobby::launch`): the peer's start state, untouched (every machine
    /// starts from the same one; nothing here may write it outside the lockstep).
    pub fn new_mp(mut net: crate::mp_lobby::Session<'r>, rules: &'r GameRules) -> Self {
        let stand_in = Game::new(SaveState::from_parts(Default::default(), Vec::new()), rules);
        let mut game = std::mem::replace(&mut net.peer_mut().game, stand_in);
        local_en(&mut game, true);
        let chunks = game.state.chunks();
        let mut w = Self::from_game(game, SavedView::from_chunks(|n| chunks.get(n).cloned()));
        w.net = Some((net, std::time::Instant::now()));
        w
    }

    /// A multiplayer world's turn step, in place of [`World::tick`]: the game goes back into the peer, the session
    /// pumps (one turn at most: two ticks), the game comes out again; the turn's events feed the screens.
    fn mp_tick(&mut self) {
        let Some((net, epoch)) = &mut self.net else { return };
        local_en(&mut self.game, false);
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        let r = net.pump(epoch.elapsed(), 1);
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        local_en(&mut self.game, true);
        let events = std::mem::take(&mut net.peer_mut().events);
        if let Ok((_, said)) = &r {
            for e in said {
                if let ig2_net::session::SessionEvent::Said { empire, text } = e {
                    self.chat_lines.push((format!("{}: {text}", net.name_of(*empire)).into_bytes(), crate::chat::CHAT_COLOUR));
                }
            }
        }
        let status = net.status(epoch.elapsed());
        self.local_lines.extend(status_lines(&self.net_seen, &status, |e| net.name_of(e)));
        self.net_seen = status;
        let host = net.host_slot();
        if self.net_host.is_some_and(|h| h != host) {
            // Ours, in English: a survivor took the session over (ADR 0012).
            self.local_lines.push(format!("The host has left the game. {} hosts it now.", net.name_in(host)).into_bytes());
        }
        self.net_host = Some(host);
        if net.host_gone() && !self.net_over {
            // Ours, in English (the original's kicked machine only ends its game: `ReceiveData`'s packet 99).
            let line: &[u8] = if net.kicked() { b"The host has kicked you from the game." } else { b"The host has left the game." };
            self.local_lines.push(line.to_vec());
            self.net_over = true;
        }
        // The events of turns run outside this pump (a drop, a kick, a save catch the peer up) count too.
        self.game.post_local_messages(&events);
        self.videos.extend(events.iter().filter_map(crate::video::Request::of));
        self.take_notices(&events);
        self.realtime.extend(events.iter().filter_map(|e| if let GameEvent::RealTime(r) = e { Some(*r) } else { None }));
        self.ground_war_routed |= events.iter().any(|e| matches!(e, GameEvent::Battle(BattleEvent::GroundRealTime { .. })));
        self.ground_voices.extend(ground_voices(&events));
        self.ground_fx.extend(ground_fx(&events));
        // The orders' outcomes; the screens keep the local empire's.
        self.order_events.extend(events.iter().filter_map(|e| if let GameEvent::Order(o) = e { Some(*o) } else { None }));
        self.colony_events.extend(events.iter().filter_map(|e| if let GameEvent::ColonyOrder(o) = e { Some(*o) } else { None }));
        match r {
            Ok((0, _)) => return,
            Ok(_) => {}
            Err(e) => {
                eprintln!("ig2-client: multiplayer: {e}");
                self.net_over = true;
                return;
            }
        }
        let mut next = Snapshot::read(&self.game);
        let speed = self.game.state.mem.i32(va::IDO_SPEED);
        next.orbit = (self.curr.orbit as f64 + 2.0 * speed as f64 / 1500.0) as f32;
        self.prev = std::mem::replace(&mut self.curr, next);
        self.ui = crate::ui::UiSnap { scenario: self.scenario, ..crate::ui::UiSnap::read(&self.game.state) };
        self.colony_prev = self.colony_curr.take();
        if let Some(p) = self.colony {
            self.colony_curr = crate::colony::ColonySnap::read(&self.game.state, p);
        }
    }

    /// The story's starmap texts for this machine's empire (`StoryEvent::Notice`, [`notice_line`]) into the message ring.
    fn take_notices(&mut self, events: &[GameEvent]) {
        let en = self.game.local.empire as i32;
        let en = if self.net.is_some() { en } else { self.game.state.mem.i32(va::EN) };
        for e in events {
            if let GameEvent::Story(ig2_sim::story::StoryEvent::Notice(n)) = e {
                if let Some(line) = notice_line(n, en, &self.game) {
                    self.chat_lines.push(line);
                }
            }
        }
    }

    /// Multiplayer under a full-screen video, Options or Help: the session runs on, as the original's time does in MP
    /// (`SetIdoPause` 0x541300 has no effect there): the message pass and the ticks due. Their events wait in the
    /// queues for the game screens' next frame.
    pub fn run_on(&mut self, clock: &mut ig2_sim::time::TickClock, now_ms: u32) {
        if self.net.is_none() {
            return;
        }
        self.pump();
        for _ in 0..clock.advance(now_ms) {
            self.tick();
        }
    }

    /// The host's "Close connection" in the wait overlay (`crate::mp_wait`): the drop decides on the state at the next
    /// turn (it catches the peer up), so the game goes in for it.
    pub fn drop_player(&mut self, empire: u8) {
        let Some((net, epoch)) = &mut self.net else { return };
        local_en(&mut self.game, false);
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        let r = net.drop_player(empire, epoch.elapsed());
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        local_en(&mut self.game, true);
        if let Err(e) = r {
            eprintln!("ig2-client: close connection: {e}");
        }
    }

    /// The wait overlay's view of the session now.
    pub fn wait_view(&self) -> Option<WaitView> {
        let (net, epoch) = self.net.as_ref()?;
        let now = epoch.elapsed();
        let s = net.status(now);
        let late = net.late(now).into_iter().map(|e| (e, net.name_of(e))).collect();
        Some(WaitView { turn: net.peer().turn, paused: !s.paused.is_empty(), reconnecting: !s.waiting.is_empty() || !s.migrating.is_empty(), host: net.is_host(), late })
    }

    /// The chat entry's line (multiplayer only; `ExecuteString`, input.md §5.1): the host's `/kick` / `/kill`, else the
    /// chat line. `races` = `Faj_Nev` (the kick's race names); a failed kick is a local red line.
    pub fn say(&mut self, text: &[u8], races: &[Vec<u8>]) {
        let Some((net, epoch)) = &mut self.net else { return };
        if let crate::console::Line::Kick(arg) = crate::console::line(text, net.is_host()) {
            match crate::console::kick_target(arg, &net.players(races), net.peer().empire) {
                Ok(e) => {
                    // The kick decides on the state at the next turn (it catches the peer up): the game goes in.
                    local_en(&mut self.game, false);
                    std::mem::swap(&mut self.game, &mut net.peer_mut().game);
                    let r = net.kick(e, epoch.elapsed());
                    std::mem::swap(&mut self.game, &mut net.peer_mut().game);
                    local_en(&mut self.game, true);
                    if let Err(e) = r {
                        eprintln!("ig2-client: kick: {e}");
                    }
                }
                Err(line) => self.chat_lines.push((line.to_vec(), crate::console::RED)),
            }
            return;
        }
        net.say(&String::from_utf8_lossy(text));
    }

    /// Queue a command of the local player (`Multi_Send(.., EN, ..)`).
    pub fn send(&mut self, c: Command) {
        if self.net.is_none() {
            self.tutorial_sites(&c);
        }
        self.outbox.push(c);
    }

    /// The screens' `TutorialCheckEvent` sites that sit right at a message they send (`re/rules/tutorial.md` §4; a
    /// no-op unless the script awaits that id): `Colony_Main`'s tax buttons and menu (0x24) and auto-build menu
    /// (0x25), `Starmap_Main`'s destroyer in a production slot (6, design kind `+0x48` = 5) and satellites (0x2b),
    /// `Spy_Main`'s new mission (0xf: kind 2 on empire 7; 0x10: kind 0xd).
    fn tutorial_sites(&mut self, c: &Command) {
        use ig2_net::{SpyOrder, StarmapCmd};
        let en = self.game.state.mem.i32(va::EN).clamp(0, EMPIRES as i32 - 1) as usize;
        let id = match c {
            Command::Starmap { order: StarmapCmd::Setting { what: 1 | 2 | 8, .. } } if self.colony.is_some() => 0x24,
            Command::Starmap { order: StarmapCmd::Setting { what: 9, .. } } if self.colony.is_some() => 0x25,
            Command::Starmap { order: StarmapCmd::Production { op: 4, value, .. } } => {
                let kind = self.game.state.designs.get(en).and_then(|d| d.get((*value).max(0) as usize)).map_or(0, |r| r[0x48]);
                if kind == 5 { 6 } else { return }
            }
            Command::Starmap { order: StarmapCmd::Satellite { .. } } => 0x2b,
            Command::Spy { order: SpyOrder::Send { mission: 2, target: 7, .. } } => 0xf,
            Command::Spy { order: SpyOrder::Send { mission: 0xd, .. } } => 0x10,
            _ => return,
        };
        self.game.tutorial_check_event(id);
    }

    /// The frame's message pass (`MULTIGET_*`): the queued commands are applied for the human, in order, also
    /// while the clock is paused; the screens see the result at once.
    pub fn pump(&mut self) {
        if self.outbox.is_empty() {
            return;
        }
        // Multiplayer: the commands go out with the next input and run on every peer `delay` turns later.
        if let Some((net, _)) = &mut self.net {
            for c in std::mem::take(&mut self.outbox) {
                net.peer_mut().issue(c);
            }
            return;
        }
        let human = self.curr.human;
        let from = self.game.events.len();
        for c in std::mem::take(&mut self.outbox) {
            c.apply(&mut self.game, human);
        }
        self.realtime.extend(self.game.events[from..].iter().filter_map(|e| if let GameEvent::RealTime(r) = e { Some(*r) } else { None }));
        self.ground_war_routed |= self.game.events[from..].iter().any(|e| matches!(e, GameEvent::Battle(BattleEvent::GroundRealTime { .. })));
        self.order_events.extend(self.game.events[from..].iter().filter_map(|e| match e {
            GameEvent::Order(o) => Some(*o),
            _ => None,
        }));
        self.colony_events.extend(self.game.events[from..].iter().filter_map(|e| match e {
            GameEvent::ColonyOrder(o) => Some(*o),
            _ => None,
        }));
        self.ground_voices.extend(ground_voices(&self.game.events[from..]));
        self.ground_fx.extend(ground_fx(&self.game.events[from..]));
        if let Some(p) = self.colony {
            self.colony_curr = crate::colony::ColonySnap::read(&self.game.state, p);
        }
        // Same tick: the fleets keep gliding from `prev`.
        self.curr = Snapshot { orbit: self.curr.orbit, ..Snapshot::read(&self.game) };
        self.ui = crate::ui::UiSnap { scenario: self.scenario, ..crate::ui::UiSnap::read(&self.game.state) };
    }

    /// `IdoPause`.
    pub fn paused(&self) -> bool {
        self.game.paused()
    }

    /// The tail of `MULTIGET_Starmap` (ELF 0x719680), every frame: a selected fleet (`StarmapMode` 2) that is gone or
    /// no longer seen (and not a wormhole's) gives way to the first own fleet that isn't hidden, else the first own
    /// one, else the first seen one; with none the planet is selected again (`StarmapMode` 1).
    pub fn fix_selection(&self) -> Option<crate::orders::Selection> {
        use ig2_sim::fleet::{flag, DELETED};
        let v = self.game.local_view();
        if v.starmap_mode != 2 {
            return None;
        }
        let map = self.game.state.starmap();
        let en = v.empire as usize;
        let debug = self.game.state.mem.u8(va::DEBUGX) != 0;
        let seen = |f: &ig2_sim::fleet::Fleet| f.visible.get(en).is_some_and(|&b| b & 1 != 0) || f.flags & flag::ALWAYS_VISIBLE != 0;
        let gone = map.fleets.get(v.fleet.max(0) as usize).is_none_or(|f| {
            f.flags & flag::WORMHOLE == 0 && (f.owner == DELETED || ((!f.visible.get(en).is_some_and(|&b| b & 1 != 0) || f.state & 1 != 0) && !debug && f.flags & flag::ALWAYS_VISIBLE == 0))
        });
        if !gone {
            return None;
        }
        let fleets = || map.fleets.iter().enumerate().skip(1);
        let pick = fleets()
            .find(|(_, f)| f.owner as usize == en && f.state & 1 == 0)
            .or_else(|| fleets().find(|(_, f)| f.owner as usize == en))
            .or_else(|| fleets().find(|(_, f)| f.owner != DELETED && seen(f)))
            .map(|(i, _)| i);
        let sel = crate::orders::Selection { mode: 2, planet: v.planet.max(0) as usize, fleet: v.fleet.max(0) as usize };
        Some(match pick {
            Some(f) => crate::orders::Selection { fleet: f, ..sel },
            None => crate::orders::Selection { mode: 1, ..sel },
        })
    }

    /// The speed the session starts with (`SetIdoSpeed(level, false)`: 1/2/3 = ×1/×4/×16 in single player).
    pub fn set_speed(&mut self, speed: Speed) {
        // Multiplayer: the speed is the session's (a local write would desync).
        if self.net.is_some() {
            return;
        }
        let level = match speed {
            Speed::Normal => 1,
            Speed::Fast => 2,
            Speed::Fastest => 3,
        };
        self.game.set_speed(level, false);
    }

    /// One 50 ms sim tick, as the original's frame runs it: the lockstep messages the last tick sent first
    /// (`Command::from_event`: 0x1b, the story's 0x49 and 0x66, for `Game::sender`), then `FRISSIT_Datum` and the
    /// rest of the tick.
    pub fn tick(&mut self) {
        if self.net.is_some() {
            self.mp_tick();
            return;
        }
        // The client keeps no event history yet (the UI that shows them comes later).
        let events = self.game.take_events();
        self.videos.extend(events.iter().filter_map(crate::video::Request::of));
        self.story_ui.extend(events.iter().filter_map(StoryUi::of));
        self.take_notices(&events);
        for c in events.iter().filter_map(Command::from_event) {
            let sender = self.game.sender();
            c.apply(&mut self.game, sender);
        }
        crate::prof::count("ticks", 1);
        let run = crate::prof::scope("t_run");
        // `Fake_FRISSITES`' frozen SP branch (a battle on screen) runs no `FRISSIT_Starmap`: the orbits stand.
        let frozen = self.game.sp_screen_frozen();
        self.game.advance_clock();
        self.game.run_tick();
        drop(run);
        // At once: a battle starts paused, so no next tick would hand them over.
        self.realtime.extend(self.game.events.iter().filter_map(|e| if let GameEvent::RealTime(r) = e { Some(*r) } else { None }));
        self.ground_war_routed |= self.game.events.iter().any(|e| matches!(e, GameEvent::Battle(BattleEvent::GroundRealTime { .. })));
        self.ground_voices.extend(ground_voices(&self.game.events));
        self.ground_fx.extend(ground_fx(&self.game.events));
        let snap = crate::prof::scope("t_snap");
        let mut next = Snapshot::read(&self.game);
        drop(snap);
        // `FRISSIT_Starmap` (ELF 0x7132b0) ends with `StarmapPlanetKering += IdoSpeedSzorzo / 1500`.
        let speed = self.game.state.mem.i32(va::IDO_SPEED);
        next.orbit = if frozen { self.curr.orbit } else { (self.curr.orbit as f64 + speed as f64 / 1500.0) as f32 };
        self.prev = std::mem::replace(&mut self.curr, next);
        let ui = crate::prof::scope("t_ui");
        self.ui = crate::ui::UiSnap { scenario: self.scenario, ..crate::ui::UiSnap::read(&self.game.state) };
        drop(ui);
        let _colony = crate::prof::scope("t_colony");
        self.colony_prev = self.colony_curr.take();
        if let Some(p) = self.colony {
            self.colony_curr = crate::colony::ColonySnap::read(&self.game.state, p);
        }
    }

    /// The UI takes the message line's next text (`FRISSIT_Scroll2` pops `ScrollUzenetek[1]`; `Ticker::pop`).
    pub fn pop_message(&mut self) -> Option<(Vec<u8>, bool)> {
        if !self.local_lines.is_empty() {
            return Some((self.local_lines.remove(0), true));
        }
        let mut t = self.game.state.ticker();
        let id = t.pop()?;
        self.popped.push(id);
        let text: Vec<u8> = t.on_screen.iter().copied().take_while(|&b| b != 0).collect();
        let important = t.on_screen_important != 0;
        self.game.state.store_ticker(&t);
        self.ui = crate::ui::UiSnap { scenario: self.scenario, ..crate::ui::UiSnap::read(&self.game.state) };
        Some((text, important))
    }

    /// Open (or close) the colony screen of planet `p`.
    pub fn show_colony(&mut self, p: Option<usize>) {
        self.colony = p;
        self.colony_curr = p.and_then(|p| crate::colony::ColonySnap::read(&self.game.state, p));
        self.colony_prev = self.colony_curr.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fleet(slot: usize, x: f32, y: f32) -> FleetView {
        FleetView { slot, pos: [x, y], owner: 3, visible: true, target: None, order: 0, flags: 0, name: Vec::new(), ships: 1, story_id: 0, kind: 0, target_index: 0 }
    }

    #[test]
    fn fleets_glide_between_ticks_and_snap_on_jumps() {
        let prev = Snapshot { fleets: vec![fleet(1, 0.0, 0.0), fleet(2, 10.0, 10.0)], ..Default::default() };
        let curr = Snapshot { fleets: vec![fleet(1, 2.0, -4.0), fleet(2, 200.0, 10.0), fleet(5, 7.0, 7.0)], ..Default::default() };
        let mid = curr.fleets_at(&prev, 0.25);
        assert_eq!(mid[0].pos, [0.5, -1.0]);
        // A wormhole-sized jump and a new fleet are drawn where they are now.
        assert_eq!(mid[1].pos, [200.0, 10.0]);
        assert_eq!(mid[2].pos, [7.0, 7.0]);
        assert_eq!(curr.fleets_at(&prev, 1.0)[0].pos, [2.0, -4.0]);
        assert_eq!(curr.fleets_at(&prev, 0.0)[0].pos, [0.0, 0.0]);
    }

    #[test]
    fn a_reused_slot_does_not_glide() {
        let prev = Snapshot { fleets: vec![fleet(1, 0.0, 0.0)], ..Default::default() };
        let mut moved = fleet(1, 4.0, 0.0);
        moved.owner = 6;
        let curr = Snapshot { fleets: vec![moved], ..Default::default() };
        assert_eq!(curr.fleets_at(&prev, 0.5)[0].pos, [4.0, 0.0]);
    }

    type Loaded = (SaveGame, GameRules, Vec<u8>, Vec<(u32, Vec<u8>)>);

    fn quick1() -> Option<Loaded> {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return None;
        };
        let dir = std::env::var_os("IG2_SAVE_DIR")?;
        let bytes = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")).ok()?;
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        Some((SaveGame::parse(&bytes).unwrap(), rules, defaults, volatile))
    }

    /// The main menu's other starts (re/screens/mainscr.md §2.2): every scenario of the list, the five tutorial
    /// chapters and a skirmish with the menu's defaults build a running world whose human (empire 1 in the
    /// tutorials) has planets of its own. `Game::start_tutorial` is verified against ig2.exe in ig2-sim's
    /// tests/new_game.rs (re/rules/new-game.md §16).
    #[test]
    fn scenario_and_tutorial_starts_build_worlds() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let (rules, _) = crate::rules::load(&install).unwrap();
        let texts = crate::ui::UiTexts::load(&install);
        let entries = crate::mainscr_scenario::Assets::load(&install, &texts).entries;
        assert_eq!(entries.len(), 15, "the skirmish entry and 14 scenarios");
        let mut games = Vec::new();
        for e in entries.iter().filter(|e| !e.skirmish()) {
            games.push((e.registry.clone(), crate::new_game::scenario(&install, &e.file, 4242, &rules).unwrap()));
        }
        for n in 1..=5 {
            games.push((format!("tutorial{n}"), crate::new_game::tutorial(&install, n, 4242, &rules).unwrap()));
        }
        // The skirmish entry's defaults (`InitScirmishValtozok`): race 1, 3 computers, the rest at 2.
        let exe = ig2_rules::PeImage::parse(&install.read_raw("ig2.exe").unwrap()).map(|pe| crate::mainscr_scenario::Tables::read(&pe)).unwrap();
        let k = crate::mainscr_scenario::Skirmish::new(1);
        let cfg = ig2_sim::game::new_game::NewSkirmish { en: k.race, ais: k.computers, random_id: 1, tech: k.technology, planets: exe.planets[k.planets as usize], money: exe.money[k.money as usize], difficulty: k.difficulty, population: k.population, seed: 4242, ticks: 4242 };
        games.push(("skirmish".into(), crate::new_game::skirmish(&install, &cfg, &rules).unwrap()));
        for (name, g) in games {
            let w = World::new_game(g);
            // A scenario file may set its own `EN` (espion plays empire 3); the tutorials are empire 1's.
            if name.starts_with("tutorial") {
                assert_eq!(w.curr.human, 1, "{name}: EN");
            }
            let en = w.curr.human;
            assert!(w.curr.planets.iter().any(|p| p.owner == en), "{name}: the human's planets");
            assert!(!w.paused(), "{name}: running");
        }
    }

    /// A message of an automatic kind asks for the messages screen (`NewMessage` exe 0x508fe0: `AktKepKell := 10`,
    /// re/rules/info-messages.md §6); a plain one only waits in the inbox.
    #[test]
    fn an_automatic_message_asks_for_the_messages_screen() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let (rules, _) = crate::rules::load(&install).unwrap();
        let kinds = &rules.inbox.kinds;
        let auto = kinds.iter().find(|k| k.2).expect("an automatic kind").0;
        let plain = kinds.iter().find(|k| !k.2 && kinds.iter().all(|j| j.0 != k.0 || !j.2)).expect("a plain kind").0;
        let mut w = World::new_game(crate::new_game::tutorial(&install, 1, 4242, &rules).unwrap());
        w.game.take_events();
        w.game.inbox_call(ig2_sim::story::StoryEvent::NewMessage { id: plain, arg: 0 });
        w.tick();
        assert!(!w.story_ui.contains(&StoryUi::Messages));
        w.game.inbox_call(ig2_sim::story::StoryEvent::NewMessage { id: auto, arg: 0 });
        w.tick();
        assert!(w.story_ui.contains(&StoryUi::Messages));
    }

    /// The screen's list takes the sim's place (`AktUzenetPointer`, here moved by the screen before), so the row
    /// clicked is the entry the sim opens (`KiertekelUjuzenet`: pointer + row; re/rules/info-messages.md §7.3).
    #[test]
    fn a_clicked_row_opens_the_entry_the_sim_opens() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let (rules, _) = crate::rules::load(&install).unwrap();
        let kinds = &rules.inbox.kinds;
        let plain = kinds.iter().find(|k| !k.1 && !k.2 && kinds.iter().all(|j| j.0 != k.0 || (!j.1 && !j.2))).expect("a plain kind").0;
        let mut w = World::new_game(crate::new_game::tutorial(&install, 1, 4242, &rules).unwrap());
        for _ in 0..8 {
            w.game.inbox_call(ig2_sim::story::StoryEvent::NewMessage { id: plain, arg: 0 });
        }
        let n = w.game.state.inbox().count as usize;
        w.send(ig2_sim::game::MessageOrder::Scroll { pointer: 4, row: -1 }.into());
        w.pump();
        let mut s = crate::messages_screen::MessagesScreen::default();
        (s.debounce, s.opened) = (5, true);
        s.sync(ig2_sim::game::inbox_place(&w.game.state));
        assert_eq!(s.first, 4);
        assert_eq!(s.press([50.0, 230.0], n), crate::messages_screen::Press::Open(7));
        assert!(ig2_sim::game::inbox_open_allowed(&w.game.state));
        for o in std::mem::take(&mut s.orders) {
            w.send(o.into());
        }
        w.pump();
        assert_eq!(ig2_sim::game::dialogue_in_progress(&w.game.state), Some(7));
    }

    /// Canonical multiplayer: each machine posts its own empire's lines (`messages_for(events, own empire)`,
    /// re/rules/multiplayer.md §2) into a line of its own; the shared state's line is back whenever the game goes
    /// into the lockstep, so the peers' states stay alike.
    #[test]
    fn each_peer_posts_its_own_message_line() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let (rules, _) = crate::rules::load(&install).unwrap();
        let mut g = crate::new_game::tutorial(&install, 1, 4242, &rules).unwrap();
        g.canonical_view(1);
        g.local.empire = 2;
        let shared = g.state.ticker();
        local_en(&mut g, true);
        let done = |empire| GameEvent::Research(ig2_sim::research::ResearchEvent::Completed { empire, tech: 1 });
        g.post_local_messages(&[done(2), done(1)]);
        assert_eq!(g.state.ticker().count, shared.count + 1, "the joiner's research only");
        local_en(&mut g, false);
        assert_eq!(g.state.ticker(), shared, "the shared line goes into the lockstep");
        local_en(&mut g, true);
        local_en(&mut g, true);
        assert_eq!(g.state.ticker().count, shared.count + 1, "the local line comes back once");
    }

    /// The main menu without a save stands on campaign 1's start, but as no started campaign: no start voice and no
    /// clips queued, so the title is followed by the menu's music, not the human's start voice.
    #[test]
    fn menu_stand_in_is_not_a_started_campaign() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let mut o = crate::parse(&[]).unwrap();
        o.save = std::path::PathBuf::from("no-such-save.sav");
        assert!(o.main_menu);
        let w = crate::start_world(&o, &install, None, &rules, &defaults, &volatile).unwrap();
        assert_eq!(w.start_voice, None);
        assert!(w.videos.is_empty());
    }

    /// A new campaign's world against the exe's: `tools/trace/run_newgame.py` starts campaigns in ig2.exe as the race
    /// menu does and saves at tick 0 (`IG2_TRACE_DIR/newgame/`); the world built from the port's start shows what the
    /// world loaded from the exe's save shows: the camera, the selection, the planets, the fleets, the date.
    #[test]
    fn new_campaign_world_matches_the_exes_start() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let dir = std::env::var_os("IG2_TRACE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces")).join("newgame");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            eprintln!("{} missing; skipping", dir.display());
            return;
        };
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        // The campaign starts only (the folder also holds the other modes' runs).
        let mut runs: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("newgame-"))).collect();
        runs.sort();
        let mut checked = 0;
        for json in &runs {
            let Ok(bytes) = std::fs::read(json.with_extension("sav")) else { continue };
            let j: serde_json::Value = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
            let int = |v: &serde_json::Value| v.as_i64().unwrap_or(0) as i32;
            let cfg = ig2_sim::game::new_game::NewCampaign {
                campaign: int(&j["campaign"]),
                difficulty: int(&j["log"]["init"][3]),
                seed: j["log"]["srand"][0].as_u64().unwrap() as u32,
                ticks: j["log"]["srand"][1].as_u64().unwrap() as u32,
                storyline: int(&j["storyline"]),
                storymap: int(&j["storymap"]),
                last_storyline: j["registry"].as_i64().or_else(|| j["registry"].as_str().and_then(|s| s.parse().ok())).map(|v| v as i32),
            };
            let mut w = World::new_game(crate::new_game::campaign(&install, &cfg, &rules).unwrap());
            w.campaign_intro();
            let exe = World::load(&SaveGame::parse(&bytes).unwrap(), &rules, &defaults, &volatile);
            let name = json.file_stem().unwrap().to_string_lossy().to_string();
            assert_eq!(w.view, exe.view, "{name}: camera");
            assert!(w.view.zoom > 0.0 && w.view.pitch != 0.0, "{name}: the side view's camera");
            assert_eq!(w.view.orbit, 0.0, "{name}: StarmapPlanetKering");
            let sel = |w: &World| {
                let v = w.game.local_view();
                (v.empire, v.starmap_mode, v.planet, v.fleet)
            };
            assert_eq!(sel(&w), sel(&exe), "{name}: selection");
            assert_eq!(sel(&w).1, 1, "{name}: StarmapMode");
            assert_eq!((w.curr.tick, w.curr.time, w.curr.human), (exe.curr.tick, exe.curr.time, exe.curr.human), "{name}: clock");
            assert_eq!(w.curr.planets, exe.curr.planets, "{name}: planets");
            // Bit 15 of `+0x52` is `FlottakUjraSzamol`'s recount, which only `LoadSave` runs (`World::load`'s `saved`).
            let fleets = |w: &World| w.curr.fleets.iter().map(|f| (f.slot, f.pos, f.owner, f.visible, f.order, f.flags & !0x8000, f.ships)).collect::<Vec<_>>();
            assert_eq!(fleets(&w), fleets(&exe), "{name}: fleets");
            assert_eq!(w.curr.radar, exe.curr.radar, "{name}: radar");
            assert!(w.game.sp_battles.live && !w.paused(), "{name}: running, live battles");
            let intro = crate::video::intro_name(w.curr.human as i32).unwrap();
            assert_eq!(w.videos, [crate::video::Request::File(intro.into())], "{name}: the race's intro");
            assert_eq!(w.start_voice, Some(cfg.campaign), "{name}: the start voice");
            checked += 1;
        }
        eprintln!("{checked} new campaigns checked");
    }

    /// The Options screen's save and load: a game a few ticks on is saved to a slot (`save<n>.ig2s`), the slot list
    /// shows its title, loading it gives the same state, the same camera and the same next ticks.
    #[test]
    fn slot_save_and_load_round_trip() {
        let Some((save, rules, defaults, volatile)) = quick1() else { return };
        let mut w = World::load(&save, &rules, &defaults, &volatile);
        for _ in 0..30 {
            w.tick();
        }
        // The camera as moved since the load goes into the save.
        let cam = crate::starmap::Camera { x: 12.5, y: -40.0, rotate: 1.25, pitch: -0.7, zoom: 333.0, size: [640.0, 480.0], look: None };
        w.store_view(&cam);
        let dir = std::env::temp_dir().join(format!("ig2-slots-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (y, m, d) = w.curr.date();
        std::fs::write(dir.join("save3.ig2s"), crate::options::slot_file(w.save_file(), b"Round trip", [y, m, d])).unwrap();
        let slots = crate::options::read_slots(Some(&dir));
        let slot = slots[3].clone().expect("slot 3 listed");
        assert_eq!((slot.name.as_slice(), slot.date, slot.ours), (&b"Round trip"[..], [y, m, d], true));
        assert!(slots.iter().enumerate().all(|(i, s)| i == 3 || s.is_none()));
        let mut back = World::load_slot(&slot.file, &rules, &defaults, &volatile).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(ig2_net::snapshot::state_hash(&back.game, &[]), ig2_net::snapshot::state_hash(&w.game, &[]));
        assert_eq!(back.view, w.view);
        assert_eq!((back.view.x, back.view.zoom, back.view.rotate), (12.5, 333.0, 1.25));
        for _ in 0..40 {
            w.tick();
            back.tick();
        }
        assert_eq!(ig2_net::snapshot::state_hash(&back.game, &[]), ig2_net::snapshot::state_hash(&w.game, &[]), "the next ticks stay identical");
    }

    /// Orders go through `ig2_net::Command` and change the sim in the next pass, paused or not.
    #[test]
    fn commands_reach_the_sim() {
        use ig2_net::StarmapCmd;
        let Some((save, rules, defaults, volatile)) = quick1() else { return };
        let mut w = World::load(&save, &rules, &defaults, &volatile);
        w.set_speed(Speed::Fastest);
        w.send(Command::Starmap { order: StarmapCmd::Pause { paused: true } });
        w.pump();
        assert!(w.paused());
        w.send(Command::Starmap { order: StarmapCmd::Speed { level: 1 } });
        w.send(Command::Starmap { order: StarmapCmd::Pause { paused: false } });
        w.pump();
        assert!(!w.paused());
        assert_eq!(w.game.speed_level(), 1);
        let f = w.curr.fleets[0].slot;
        let order = ig2_sim::game::StarmapOrder::Move { fleet: f as i32, order: 1, kind: 0, x: 10.0, y: -20.0 };
        w.send(Command::Starmap { order: StarmapCmd::from(order) });
        w.pump();
        let g = w.curr.fleets.iter().find(|g| g.slot == f).unwrap();
        assert_eq!((g.order, g.target), (1, Some([10.0, -20.0])));
    }

    /// Shift + right click (re/screens/starmap.md §10.3): a moving fleet's mode-13 orders append
    /// waypoints, and the snapshot carries the chain `Show_Flottak3d` draws for the selected fleet (`ships-0.sav`:
    /// the human has fleets).
    #[test]
    fn shift_orders_chain_waypoints() {
        use ig2_net::StarmapCmd;
        let Some((_, rules, defaults, volatile)) = quick1() else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("ships-0.sav")) else { return };
        let mut w = World::load(&SaveGame::parse(&bytes).unwrap(), &rules, &defaults, &volatile);
        let human = w.curr.human;
        let f = w.curr.fleets.iter().find(|g| g.owner == human && g.flags & 1 == 0).unwrap().slot as i32;
        for (order, x, y) in [(1, 10.0, -20.0), (13, 30.0, -20.0), (13, 30.0, 40.0)] {
            let o = ig2_sim::game::StarmapOrder::Move { fleet: f, order, kind: 0, x, y };
            w.send(Command::Starmap { order: StarmapCmd::from(o) });
        }
        w.pump();
        let g = w.curr.fleets.iter().find(|g| g.slot == f as usize).unwrap();
        assert_eq!((g.order, g.target), (1, Some([10.0, -20.0])));
        let chain = w.curr.waypoints.iter().find(|(s, _)| *s == f as usize).map(|(_, c)| c.clone());
        assert_eq!(chain, Some(vec![[30.0, -20.0], [30.0, 40.0]]));
    }

    /// A campaign starts with dozens of planets at visibility −1 and 0; the starmap shows only those
    /// at `+0xd8 + 2·EN` > 0 (`rotate_stars`, `Show_Stars2d3d`, `GFX_StarmapRadar`, `SelectNearestUnit`), the
    /// exe's campaign-start saves (`IG2_TRACE_DIR/newgame/`) as loaded.
    #[test]
    fn a_campaign_start_shows_only_the_seen_planets() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let dir = std::env::var_os("IG2_TRACE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces")).join("newgame");
        let Ok(bytes) = std::fs::read(dir.join("newgame-c1-d1-s5150.sav")) else {
            eprintln!("{} missing; skipping", dir.display());
            return;
        };
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let w = World::load(&SaveGame::parse(&bytes).unwrap(), &rules, &defaults, &volatile);
        let map = w.game.state.starmap();
        let en = w.curr.human as usize;
        let hidden = w.curr.planets.iter().filter(|p| map.planets[p.index].visible[en] < 0).count();
        assert!(hidden > 20, "{hidden} planets never seen");
        let known: Vec<usize> = w.curr.planets.iter().filter(|p| p.known).map(|p| p.index).collect();
        let seen: Vec<usize> = (1..map.planets.len()).filter(|&p| map.planets[p].visible[en] > 0).collect();
        assert_eq!(known, seen);
        assert_eq!(known.len(), 3);
    }

    /// The real thing: `quick-1.sav` in the sim (skips without the install and the save).
    #[test]
    fn quick1_fleets_move_in_the_sim() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else {
            eprintln!("IG2_SAVE_DIR not set; skipping");
            return;
        };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else {
            eprintln!("no quick-1.sav; skipping");
            return;
        };
        let save = SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let mut w = World::load(&save, &rules, &defaults, &volatile);
        w.set_speed(Speed::Fastest);
        let start = w.curr.clone();
        assert!(start.planets.iter().filter(|p| p.on_map()).count() > 50);
        for _ in 0..20 {
            w.tick();
        }
        assert_eq!(w.curr.tick, start.tick + 20);
        assert_eq!(w.prev.tick, start.tick + 19);
        let moving: Vec<_> = w.curr.fleets.iter().filter(|f| w.prev.fleets.iter().any(|p| p.slot == f.slot && p.pos != f.pos)).collect();
        assert!(!moving.is_empty(), "no fleet moved");
        let f = moving[0];
        let p = w.prev.fleets.iter().find(|p| p.slot == f.slot).unwrap();
        let mid = w.curr.fleets_at(&w.prev, 0.5).into_iter().find(|m| m.slot == f.slot).unwrap();
        assert!((mid.pos[0] - (p.pos[0] + f.pos[0]) / 2.0).abs() < 1e-4);
    }

    /// The MP modes' starmap texts on the machine of empire 1 (`st_Multi_include`'s `AddSzoveg` calls, exe 0x5ee4d0:
    /// each machine tests its own `EN`; the colours pushed at 0x5ee7db, 0x5eeca1, 0x5ef5ed, 0x5efcc1, …).
    #[test]
    fn story_notices_are_this_machines_lines() {
        use ig2_sim::story::Notice;
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let rules: &'static GameRules = Box::leak(Box::new(rules));
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let mut w = World::load(&SaveGame::parse(&bytes).unwrap(), rules, &defaults, &volatile);
        w.game.state.mem.write(0xBAE588 + 22, b"Bob\0");
        let planet = ig2_sim::messages::cstr(&w.game.state.mem.read(va::PLANETS + ig2_sim::save::PLANET_SIZE as u32, 0x20)).to_vec();
        let line = |n: Notice| notice_line(&n, 1, &w.game);
        assert_eq!(line(Notice::Winner { empire: 1, amazing: true, colour: 7 }), Some((b"AMAZING! YOU ARE THE WINNER!".to_vec(), 7)));
        assert_eq!(line(Notice::Winner { empire: 2, amazing: false, colour: 2 }), Some((b"Bob is the winner!".to_vec(), 2)));
        assert_eq!(line(Notice::CannotColonise { planet: 1, by: 1 }), Some(([b"You cannot colonize ".as_slice(), &planet, b"!"].concat(), 2)));
        assert_eq!(line(Notice::CannotColonise { planet: 1, by: 2 }), None);
        assert_eq!(line(Notice::FlagTaken { by: 1, from: 2 }), Some((b"You have the flag of Bob!".to_vec(), 2)));
        assert_eq!(line(Notice::FlagTaken { by: 2, from: 1 }), Some((b"Bob has your flag!".to_vec(), 2)));
        assert_eq!(line(Notice::Touchdown { by: 2 }), Some((b"Bob has captured your flag!".to_vec(), 2)));
        assert_eq!(line(Notice::FlagLost { by: 2 }), Some((b"Your flag has been returned!".to_vec(), 2)));
        assert_eq!(line(Notice::FlagLost { by: 1 }), Some((b"You have lost the flag!".to_vec(), 2)));
    }
}

