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
    /// Seen by the human (`+0xd8 + 2·EN`).
    pub known: bool,
    /// `+0xba`: the name blinks (`Show_Stars2d3d`, re/screens/hud.md §4.2).
    pub blink: bool,
    /// `+0xbc` bit 0: never a target (`SelectNearestUnit`'s enemies-only pick skips it).
    pub friendly: bool,
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
}

impl Snapshot {
    pub fn read(game: &Game) -> Self {
        let state = &game.state;
        let m = &state.mem;
        let en = m.i32(va::EN).clamp(0, EMPIRES as i32 - 1) as usize;
        let map = state.starmap();
        let n = state.planet_count();
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
                    known: sp.visible[en] != 0 || sp.owner as usize == en,
                    blink: r[0xba] != 0,
                    friendly: r[0xbc] & 1 != 0,
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
        Self { tick: m.i32(va::IDO2), time: m.i32(va::IDO), human: en as u8, planets, fleets, orbit: 0.0, radar }
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
    /// The videos the sim asked for since the window last took them (`st_ShowAnim`, the game's end).
    pub videos: Vec<crate::video::Request>,
    /// The ground wars' voice lines since the window last took them (planet, `PlaySpeechGrWar` site).
    pub ground_voices: Vec<(usize, Voice)>,
    /// `HangLejatszasKell` (exe 0xbae26c, `InitNewGame` step 6: the campaign type) as the first `Starmap_Belep`
    /// takes it for the start voice ([`crate::audio::Audio::play_start_voice`]).
    pub start_voice: Option<i32>,
    /// A multiplayer game (ADR 0014 §5): the session whose peer runs the sim, and the session's clock. The world
    /// holds the game between pumps (the peer holds a stand-in), so the screens read it as in single player.
    pub net: Option<(crate::mp_lobby::Session<'r>, std::time::Instant)>,
}

/// The ground wars' voice sites among `events` (`GroundSide`, ground-combat.md §11.12).
fn ground_voices(events: &[GameEvent]) -> impl Iterator<Item = (usize, Voice)> + '_ {
    events.iter().filter_map(|e| match *e {
        GameEvent::Battle(BattleEvent::GroundSide { planet, event: GroundEvent::Voice(v) }) => Some((planet, v)),
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
        if bytes.starts_with(ig2_net::snapshot::MAGIC) {
            let (snap, _) = ig2_net::snapshot::Snapshot::from_file(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            let (game, _) = snap.restore(rules).map_err(|e| format!("{}: {e}", path.display()))?;
            let chunks = game.state.chunks();
            return Ok(Self::from_game(game, SavedView::from_chunks(|n| chunks.get(n).cloned())));
        }
        let save = SaveGame::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
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

    /// The state as the remake's save file (`.ig2s`) holds it.
    pub fn save_file(&self) -> Vec<u8> {
        ig2_net::snapshot::Snapshot::capture(&self.game, &[], 0, 0).to_file(0, false)
    }

    fn from_game(game: Game<'r>, view: SavedView) -> Self {
        let curr = Snapshot { orbit: view.orbit, ..Snapshot::read(&game) };
        let ui = crate::ui::UiSnap::read(&game.state);
        Self { game, prev: curr.clone(), curr, view, colony: None, colony_prev: None, colony_curr: None, ui, outbox: Vec::new(), order_events: Vec::new(), colony_events: Vec::new(), local_lines: Vec::new(), popped: Vec::new(), realtime: Vec::new(), videos: Vec::new(), ground_voices: Vec::new(), start_voice: None, net: None }
    }

    /// A multiplayer game from our room's start (`mp_lobby::launch`): the peer's start state, untouched (every machine
    /// starts from the same one; nothing here may write it outside the lockstep).
    pub fn new_mp(mut net: crate::mp_lobby::Session<'r>, rules: &'r GameRules) -> Self {
        let stand_in = Game::new(SaveState::from_parts(Default::default(), Vec::new()), rules);
        let game = std::mem::replace(&mut net.peer_mut().game, stand_in);
        let chunks = game.state.chunks();
        let mut w = Self::from_game(game, SavedView::from_chunks(|n| chunks.get(n).cloned()));
        w.net = Some((net, std::time::Instant::now()));
        w
    }

    /// A multiplayer world's turn step, in place of [`World::tick`]: the game goes back into the peer, the session
    /// pumps (one turn at most: two ticks), the game comes out again; the turn's events feed the screens.
    fn mp_tick(&mut self) {
        let Some((net, epoch)) = &mut self.net else { return };
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        let r = net.pump(epoch.elapsed(), 1);
        std::mem::swap(&mut self.game, &mut net.peer_mut().game);
        let events = std::mem::take(&mut net.peer_mut().events);
        match r {
            Ok((0, _)) => return,
            Ok(_) => {}
            Err(e) => {
                eprintln!("ig2-client: multiplayer: {e}");
                return;
            }
        }
        self.videos.extend(events.iter().filter_map(crate::video::Request::of));
        self.realtime.extend(events.iter().filter_map(|e| if let GameEvent::RealTime(r) = e { Some(*r) } else { None }));
        self.ground_voices.extend(ground_voices(&events));
        let mut next = Snapshot::read(&self.game);
        let speed = self.game.state.mem.i32(va::IDO_SPEED);
        next.orbit = (self.curr.orbit as f64 + 2.0 * speed as f64 / 1500.0) as f32;
        self.prev = std::mem::replace(&mut self.curr, next);
        self.ui = crate::ui::UiSnap::read(&self.game.state);
        self.colony_prev = self.colony_curr.take();
        if let Some(p) = self.colony {
            self.colony_curr = crate::colony::ColonySnap::read(&self.game.state, p);
        }
    }

    /// Queue a command of the local player (`Multi_Send(.., EN, ..)`).
    pub fn send(&mut self, c: Command) {
        self.outbox.push(c);
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
        self.order_events.extend(self.game.events[from..].iter().filter_map(|e| match e {
            GameEvent::Order(o) => Some(*o),
            _ => None,
        }));
        self.colony_events.extend(self.game.events[from..].iter().filter_map(|e| match e {
            GameEvent::ColonyOrder(o) => Some(*o),
            _ => None,
        }));
        self.ground_voices.extend(ground_voices(&self.game.events[from..]));
        if let Some(p) = self.colony {
            self.colony_curr = crate::colony::ColonySnap::read(&self.game.state, p);
        }
        // Same tick: the fleets keep gliding from `prev`.
        self.curr = Snapshot { orbit: self.curr.orbit, ..Snapshot::read(&self.game) };
        self.ui = crate::ui::UiSnap::read(&self.game.state);
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
    /// (`Game::fleet_emptied`), then `FRISSIT_Datum` and the rest of the tick.
    pub fn tick(&mut self) {
        if self.net.is_some() {
            self.mp_tick();
            return;
        }
        // The client keeps no event history yet (the UI that shows them comes later).
        let events = self.game.take_events();
        self.videos.extend(events.iter().filter_map(crate::video::Request::of));
        for f in events.iter().filter_map(GameEvent::emptied_fleet) {
            self.game.fleet_emptied(f);
        }
        self.game.advance_clock();
        self.game.run_tick();
        // At once: a battle starts paused, so no next tick would hand them over.
        self.realtime.extend(self.game.events.iter().filter_map(|e| if let GameEvent::RealTime(r) = e { Some(*r) } else { None }));
        self.ground_voices.extend(ground_voices(&self.game.events));
        let mut next = Snapshot::read(&self.game);
        // `FRISSIT_Starmap` (ELF 0x7132b0) ends with `StarmapPlanetKering += IdoSpeedSzorzo / 1500`.
        let speed = self.game.state.mem.i32(va::IDO_SPEED);
        next.orbit = (self.curr.orbit as f64 + speed as f64 / 1500.0) as f32;
        self.prev = std::mem::replace(&mut self.curr, next);
        self.ui = crate::ui::UiSnap::read(&self.game.state);
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
        self.ui = crate::ui::UiSnap::read(&self.game.state);
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
    /// tutorials) has planets of its own. Untraced: `Game::start_tutorial` reuses the scenario start's verified parts
    /// (re/rules/new-game.md §13).
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
}
