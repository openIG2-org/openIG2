//! The generated MP galaxy's room (re/screens/start.md, ADR 0014): the original room's options (`Start_Init` exe
//! 0x5bfbf0, click handler 0x5c0120), its AI pick (exe 0x5bd6c0) and the start every machine builds itself
//! (`FRISSIT_Start` exe 0x5bc590 / `ReceiveStartData` 0x5bc9d0 → `InitNewGame(seed, 2, 0, 1)`).

use ig2_sim::game::new_game::{NewMpGalaxy, StartData};
use ig2_sim::game::{Game, GameRules, Libc};
use serde::{Deserialize, Serialize};

use crate::snapshot::state_hash;

/// The room's empire slots (`ProgramPlayers` 8).
pub const SLOTS: u8 = 8;
/// The planet counts the room offers (ids 0x22..0x25: (id − 0x21)·20).
pub const PLANETS: [u8; 4] = [20, 40, 60, 80];
/// The most AI players (ids 0x2A..0x30: 0..=6).
pub const MAX_AIS: u8 = 6;

/// The host's options (re/screens/start.md §1; defaults = `Start_Init`'s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalaxyOptions {
    /// `HanyBolygo`: one of [`PLANETS`].
    pub planets: u8,
    /// `HanyGepiJatekos` as the host set it (the AI slots placed may be fewer: at most the free slots).
    pub ais: u8,
    /// `ProgramGalaxyShape`: 1 or 2.
    pub shape: u8,
    /// `MultiplayerBattleStatusz`: 0 space and ground in real time, 1 space only, 2 ground only, 3 neither.
    pub battle_status: u8,
    /// `OnlyServerCanModifyTime`.
    pub host_time: bool,
}

impl Default for GalaxyOptions {
    fn default() -> Self {
        Self { planets: 40, ais: 2, shape: 1, battle_status: 0, host_time: false }
    }
}

impl GalaxyOptions {
    /// Clamped to the room's ranges.
    pub fn clamped(self) -> Self {
        Self {
            planets: if PLANETS.contains(&self.planets) { self.planets } else { 40 },
            ais: self.ais.min(MAX_AIS),
            shape: self.shape.clamp(1, 2),
            battle_status: self.battle_status.min(3),
            host_time: self.host_time,
        }
    }
}

/// The start of a generated galaxy as every member gets it (message 3's seed with message 2's room, ours with the
/// host's ticks and the start state's hash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalaxyStart {
    /// `RandomSeedNumber`: the host's `rand() % 0x7fff` after `srand(ticks)`.
    pub seed: u32,
    /// The host's `SDL_GetTicks()` for `NEWGAME_Diplomacy`'s reseed (shared: deviation 223).
    pub ticks: u32,
    pub options: GalaxyOptions,
    /// `PlayerFaj` of empires 1..=8 (index 0 = empire 1).
    pub races: [u8; 8],
    /// The AI empires (`BirodalomEl`).
    pub ais: Vec<u8>,
    /// [`state_hash`] of the built start state (the joiners check theirs against it).
    pub hash: u64,
}

/// `FRISSIT_Start`'s seed: `srand(ticks); rand() % 0x7fff`.
pub fn seed_from_ticks(ticks: u32) -> u32 {
    (Libc(ticks).rand() % 0x7fff) as u32
}

impl GalaxyStart {
    /// The host's start (the hash still 0: [`GalaxyStart::build`] fills it in).
    pub fn new(ticks: u32, options: GalaxyOptions, races: [u8; 8], ais: Vec<u8>) -> Self {
        Self { seed: seed_from_ticks(ticks), ticks, options, races, ais, hash: 0 }
    }

    /// The start state (spec §4): `Game::before_start`, `Game::start_mp_galaxy` with `host` (the host's empire) as
    /// `EN` and `humans`, `Game::start_multiplayer` (the canonical view, ADR 0007 §5) and `session` as libc, as
    /// `session::HostSession::start` does for a scenario.
    pub fn build<'r>(&self, data: &StartData, rules: &'r GameRules, host: u8, humans: &[u8], session: u64) -> Game<'r> {
        let mut races = vec![0u8; 19];
        for (i, r) in races.iter_mut().enumerate().skip(1) {
            *r = match i {
                1..=8 => self.races[i - 1],
                15 | 17 | 18 => 1,
                16 => 3,
                _ => i as u8,
            };
        }
        let cfg = NewMpGalaxy {
            seed: self.seed,
            ticks: self.ticks,
            planets: self.options.planets as i32,
            en: host as i32,
            humans: humans.iter().map(|&e| e as i32).collect(),
            alive: self.ais.iter().map(|&e| e as i32).collect(),
            speed: 4,
            shape: self.options.shape as i32,
            battle_status: self.options.battle_status as i32,
            server_time: self.options.host_time as i32,
            races,
        };
        let mut game = Game::before_start(data, rules);
        game.start_mp_galaxy(&cfg, data);
        game.start_multiplayer(host, humans);
        game.libc = Libc(session as u32);
        game
    }

    /// [`GalaxyStart::build`], the hash filled in (the host).
    pub fn build_hashed<'r>(&mut self, data: &StartData, rules: &'r GameRules, host: u8, humans: &[u8], session: u64) -> Game<'r> {
        let game = self.build(data, rules, host, humans, session);
        self.hash = state_hash(&game, &[]);
        game
    }
}

/// The AI pick (exe 0x5bd6c0): every slot in `free` (neither human nor closed) loses its AI, then `count` (at most
/// `free.len()`) of them are drawn at random, `draw(1..=n)` like `rndlog(1, 1, 8)` with redraws. Returns the AI slots.
pub fn pick_ais(free: &[u8], count: u8, mut draw: impl FnMut(u32) -> u32) -> Vec<u8> {
    let n = (count as usize).min(free.len());
    let mut ais = Vec::with_capacity(n);
    while ais.len() < n {
        let e = free[draw(free.len() as u32) as usize % free.len()];
        if !ais.contains(&e) {
            ais.push(e);
        }
    }
    ais.sort_unstable();
    ais
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_the_crt_rand_mod_0x7fff() {
        // MSVC `srand(1); rand()` = 41.
        assert_eq!(seed_from_ticks(1), 41);
        assert!(seed_from_ticks(0xdead_beef) < 0x7fff);
    }

    #[test]
    fn ai_pick_takes_at_most_the_free_slots() {
        let mut k = 0u32;
        let mut draw = |n: u32| {
            k = k.wrapping_mul(1103515245).wrapping_add(12345);
            (k >> 8) % n
        };
        assert_eq!(pick_ais(&[2, 3, 4], 6, &mut draw), vec![2, 3, 4]);
        let two = pick_ais(&[2, 3, 4, 5, 6, 7, 8], 2, &mut draw);
        assert_eq!(two.len(), 2);
        assert!(two[0] < two[1]);
        assert!(pick_ais(&[], 3, &mut draw).is_empty());
    }

    #[test]
    fn options_are_clamped() {
        let o = GalaxyOptions { planets: 33, ais: 9, shape: 0, battle_status: 7, host_time: true }.clamped();
        assert_eq!(o, GalaxyOptions { planets: 40, ais: 6, shape: 1, battle_status: 3, host_time: true });
    }
}
