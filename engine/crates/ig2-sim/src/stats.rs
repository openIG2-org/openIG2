//! Ours (ADR 0034): the game's statistics for the end screen and the match history. Spec:
//! `re/rules/game-end.md` §11.
//!
//! [`Stats`] takes one day at each day change of the tick and a sample of every empire each [`Stats::every`]-th
//! day, from the synced state only: it writes nothing the original has (classic replays stay byte-exact) and every
//! multiplayer peer holds the same numbers.

use crate::info::{InfoWorld, ROWS};
use crate::save::SaveState;

/// The samples kept; past it every other one is dropped and the step doubles.
pub const MAX_SAMPLES: usize = 240;

/// One empire's numbers on a day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Row {
    pub planets: i32,
    pub population: i32,
    /// The planets' last tax (`InfoWorld::values` chart 0).
    pub income: i32,
    /// `Penz[e]`, truncated.
    pub credits: i64,
    /// The ships of the empire's fleets.
    pub ships: i32,
    /// Techs researched (status 5).
    pub techs: i32,
    /// `BattlesWon[e]`.
    pub battles: i32,
    /// The info screen's score (chart 9).
    pub score: i32,
}

/// The empires 1..=8 on one day (row `e − 1`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sample {
    /// `Ido` (the game clock) of the day.
    pub ido: i32,
    pub rows: [Row; ROWS],
}

/// The statistics of a running game. Synced state (in the snapshot).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stats {
    /// Game days counted.
    pub days: u32,
    /// Days between samples (1, 2, 4, …; 0 before the first day).
    pub every: u32,
    pub samples: Vec<Sample>,
    /// Ships built and lost, per empire (row `e − 1`), running totals.
    pub built: [i32; ROWS],
    pub lost: [i32; ROWS],
    /// The ships each empire had at the last day, and those built since.
    ships: [i32; ROWS],
    built_today: [i32; ROWS],
}

impl Stats {
    /// A ship left a shipyard of empire `e`.
    pub fn ship_built(&mut self, e: usize) {
        if let Some(i) = e.checked_sub(1).filter(|&i| i < ROWS) {
            self.built[i] += 1;
            self.built_today[i] += 1;
        }
    }

    /// A day change: the ships lost since the last day, and a sample on every [`Stats::every`]-th day.
    pub fn day(&mut self, now: Sample) {
        if self.every == 0 {
            self.every = 1;
        } else {
            for i in 0..ROWS {
                self.lost[i] += (self.ships[i] + self.built_today[i] - now.rows[i].ships).max(0);
            }
        }
        self.ships = now.rows.map(|r| r.ships);
        self.built_today = [0; ROWS];
        if self.days % self.every == 0 {
            self.samples.push(now);
            if self.samples.len() > MAX_SAMPLES {
                let mut k = 0;
                self.samples.retain(|_| {
                    k += 1;
                    k % 2 == 1
                });
                self.every *= 2;
            }
        }
        self.days += 1;
    }
}

/// Today's numbers of every empire (`re/rules/game-end.md` §11): a dead empire's row is zeros but its battles.
pub fn sample(s: &SaveState) -> Sample {
    let w: InfoWorld = s.info_world();
    let money = s.treasuries();
    let mut ships = [0i32; ROWS];
    for sh in w.ships.iter().skip(1) {
        let owner = usize::try_from(sh.fleet).ok().and_then(|f| w.fleet_owners.get(f)).copied().unwrap_or(0) as usize;
        if (1..=ROWS).contains(&owner) && sh.fleet > 0 {
            ships[owner - 1] += 1;
        }
    }
    let rows = std::array::from_fn(|i| {
        let e = i + 1;
        if !w.alive[e] {
            return Row { battles: w.battles_won[e], ..Row::default() };
        }
        let v = w.values(e);
        let credits = money[e].total;
        Row {
            planets: v[2],
            population: v[1],
            income: v[0],
            credits: if credits.is_finite() { credits as i64 } else { 0 },
            ships: ships[i],
            techs: v[5],
            battles: v[7],
            score: v[9],
        }
    });
    Sample { ido: s.mem.i32(crate::save::va::IDO), rows }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(ido: i32, ships: [i32; ROWS]) -> Sample {
        Sample { ido, rows: ships.map(|n| Row { ships: n, ..Row::default() }) }
    }

    /// Losses are yesterday's ships plus today's built minus today's; the first day only sets the count.
    #[test]
    fn ships_built_and_lost() {
        let mut s = Stats::default();
        s.day(day(0, [5, 3, 0, 0, 0, 0, 0, 0]));
        assert_eq!(s.lost, [0; ROWS]);
        s.ship_built(1);
        s.ship_built(1);
        s.ship_built(0);
        s.ship_built(9);
        s.day(day(1, [4, 3, 0, 0, 0, 0, 0, 0]));
        assert_eq!((s.built[0], s.lost[0]), (2, 3), "5 + 2 built − 4 left");
        // A captured fleet's ships raise the count without a build: no negative loss.
        s.day(day(2, [4, 6, 0, 0, 0, 0, 0, 0]));
        assert_eq!(s.lost[1], 0);
        assert_eq!(s.samples.len(), 3);
    }

    /// Past [`MAX_SAMPLES`] every other sample goes and the step doubles; the samples stay in order and evenly spaced.
    #[test]
    fn samples_thin_out() {
        let mut s = Stats::default();
        for d in 0..1000 {
            s.day(day(d, [0; ROWS]));
        }
        assert!(s.samples.len() <= MAX_SAMPLES);
        assert_eq!(s.every, 8);
        assert_eq!(s.days, 1000);
        let step = s.samples[1].ido - s.samples[0].ido;
        assert_eq!(step, 8);
        assert!(s.samples.windows(2).all(|w| w[1].ido - w[0].ido == step));
        assert_eq!(s.samples[0].ido, 0);
    }
}
