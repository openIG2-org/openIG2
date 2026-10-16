//! The client's side of the simulation (ADR 0006): an original save loaded into `ig2-sim`, ticked at the original's
//! fixed 20 Hz, and the starmap read back out as plain snapshots the renderer interpolates between.
//!
//! The client never writes sim state except through the sim's own entry points (`Game::advance_clock`,
//! `Game::run_tick`, `Game::fleet_emptied`); the renderer only sees [`Snapshot`]s.

use ig2_formats::save::SaveGame;
use ig2_sim::game::{Game, GameEvent, GameRules};
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
        let f = |n: &str| save.chunk(n).and_then(|b| b.get(..4)).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()));
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
}

impl Snapshot {
    pub fn read(state: &SaveState) -> Self {
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
            })
            .collect();
        Self { tick: m.i32(va::IDO2), time: m.i32(va::IDO), human: en as u8, planets, fleets, orbit: 0.0 }
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
}

impl<'r> World<'r> {
    /// Load `save` into the sim. `exe_volatile` = globals the save doesn't carry, from the user's exe image.
    pub fn load(save: &SaveGame, rules: &'r GameRules, tech_defaults: &[u8], volatile: &[(u32, Vec<u8>)]) -> Self {
        let mut state = SaveState::from_save(save, tech_defaults);
        state.set_volatile(volatile);
        let mut game = Game::new(state, rules);
        // `LoadSave` ends with `FlottakUjraSzamol` (fleet-movement.md §4.3).
        game.saved();
        let view = SavedView::read(save);
        let curr = Snapshot { orbit: view.orbit, ..Snapshot::read(&game.state) };
        let ui = crate::ui::UiSnap::read(&game.state);
        Self { game, prev: curr.clone(), curr, view, colony: None, colony_prev: None, colony_curr: None, ui }
    }

    /// `IdoSpeedSzorzo` for a single-player speed setting.
    pub fn set_speed(&mut self, speed: Speed) {
        let mp = self.game.state.mem.u8(va::MULTIPLAYER) != 0;
        self.game.state.mem.set_i32(va::IDO_SPEED, speed.ido_per_tick(mp));
    }

    /// One 50 ms sim tick, as the original's frame runs it: the lockstep messages the last tick sent first
    /// (`Game::fleet_emptied`), then `FRISSIT_Datum` and the rest of the tick.
    pub fn tick(&mut self) {
        let emptied: Vec<usize> = self.game.events.iter().filter_map(GameEvent::emptied_fleet).collect();
        for f in emptied {
            self.game.fleet_emptied(f);
        }
        // The client keeps no event history yet (the UI that shows them comes later).
        self.game.events.clear();
        self.game.advance_clock();
        self.game.run_tick();
        let mut next = Snapshot::read(&self.game.state);
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
        let mut t = self.game.state.ticker();
        t.pop()?;
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
        FleetView { slot, pos: [x, y], owner: 3, visible: true, target: None, order: 0, flags: 0, name: Vec::new() }
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
