//! The info screen's charts: `FRISSIT_Info` (exe 0x4e59f0) samples ten statistics of every empire once a month
//! into `InfoCharts` and keeps a smoothed copy (`InfoChartsSmoothed`). Spec: `re/rules/info-messages.md` §1.
//!
//! The charts are kept as the original's `f32` cells (`[10 charts][8 empires][55 months]`, the newest last), so a
//! save round-trips bit for bit. [`InfoWorld`] holds what the sampler reads from the other systems.

use crate::colony_side::{TechTable, EMPIRES};
use crate::spacewar::{Design, Ship, Vehicle};
use crate::spy::Spies;

/// Charts per empire.
pub const CHARTS: usize = 10;
/// Empires with charts (1..=8, row `e − 1`).
pub const ROWS: usize = 8;
/// Monthly cells per row.
pub const CELLS: usize = 55;
/// Bytes of `InfoCharts` (and of `InfoChartsSmoothed`).
pub const SIZE: usize = CHARTS * ROWS * CELLS * 4;
/// Planet slots the ground strength scans (`Bolygok[1..=199]`, whatever `HanyBolygo` is).
const PLANET_SLOTS: usize = 200;
/// The techs counted (1..=192).
const TECHS: usize = 192;
/// A ship or vehicle slot without a design.
const NO_DESIGN: i16 = 0x7fff;

/// What the sampler reads.
#[derive(Debug, Clone, Default)]
pub struct InfoWorld {
    /// `BirodalomEl[e]`.
    pub alive: [bool; EMPIRES],
    /// Planet slots 0..200: owner (`+0x20`), last month's tax (`+0x44`), population (`+0x28`).
    pub planets: Vec<(u8, i32, i32)>,
    /// `HanyBolygo`.
    pub planet_count: usize,
    /// Fleet owners (`Flottak[f].+0x15`, index 0 unused).
    pub fleet_owners: Vec<u8>,
    /// `Hajok`, `Tankok` (index 0 unused).
    pub ships: Vec<Ship>,
    pub vehicles: Vec<Vehicle>,
    /// `Tervek[e]`.
    pub designs: Vec<Vec<Design>>,
    pub tech: TechTable,
    pub spies: Spies,
    /// `BattlesWon[e]`.
    pub battles_won: [i32; EMPIRES],
    /// `Viszony[e][i].+0x22`.
    pub opinion: Vec<[i16; EMPIRES]>,
}

/// `cvttss2si` / `cvttsd2si`: out of range and NaN give `i32::MIN`.
fn trunc(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 { i32::MIN } else { x as i32 }
}

impl InfoWorld {
    /// `sqrt(hp · attack)` of empire `e`'s design `d` and its `+0x36` flag (a vehicle that fights in space).
    fn design(&self, e: usize, d: i16) -> Option<(f32, bool)> {
        if d == NO_DESIGN {
            return None;
        }
        let x = self.designs.get(e)?.get(usize::try_from(d).ok()?)?;
        Some(((x.hp.wrapping_mul(x.attack) as f64).sqrt() as f32, x.hull_flags[2] != 0))
    }

    /// `acc` plus the vehicles of fleet `f` (planet −f for a garrison) whose space flag is `space`.
    fn add_vehicles(&self, e: usize, f: i16, space: bool, mut acc: i32) -> i32 {
        for v in self.vehicles.iter().skip(1).filter(|v| v.fleet == f) {
            if let Some((s, _)) = self.design(e, v.design).filter(|x| x.1 == space) {
                acc = trunc((s * v.count as f32 + acc as f32) as f64);
            }
        }
        acc
    }

    /// `CalcSpacePower(f)` (exe 0x4e4d60): the fleet's ships, then its space-going vehicles.
    pub fn space_power(&self, f: usize) -> i32 {
        let e = self.fleet_owners.get(f).copied().unwrap_or(0) as usize;
        let mut acc = 0;
        for s in self.ships.iter().skip(1).filter(|s| s.fleet as i32 == f as i32) {
            if let Some((x, _)) = self.design(e, s.design) {
                acc = trunc((x + acc as f32) as f64);
            }
        }
        self.add_vehicles(e, f as i16, true, acc)
    }

    /// The ten chart values of a living empire (spec §1.2).
    pub fn values(&self, e: usize) -> [i32; CHARTS] {
        let owned = || self.planets.iter().enumerate().skip(1).take(self.planet_count).filter(|(_, p)| p.0 as usize == e);
        let tax = owned().fold(0i32, |a, (_, p)| a.wrapping_add(p.1));
        let pop = owned().fold(0i32, |a, (_, p)| a.wrapping_add(p.2));
        let planets = owned().count() as i32;
        let fleets: Vec<usize> = (1..self.fleet_owners.len()).filter(|&f| self.fleet_owners[f] as usize == e).collect();
        let space = fleets.iter().fold(0i32, |a, &f| a.wrapping_add(self.space_power(f)));
        let mut ground = 0;
        for &f in &fleets {
            ground = self.add_vehicles(e, f as i16, false, ground);
        }
        for p in 1..PLANET_SLOTS.min(self.planets.len()) {
            if self.planets[p].0 as usize == e {
                ground = self.add_vehicles(e, -(p as i16), false, ground);
            }
        }
        let techs = (1..=TECHS).filter(|&t| self.tech.status(t, e) == 5).count() as i32;
        let spies = (1..=self.spies.count[e].max(0) as usize)
            .filter_map(|k| self.spies.roster[e].get(k))
            .filter(|s| s.applicant == 0 && s.state > 0)
            .fold(0i32, |a, s| a + s.rank as i32);
        let battles = self.battles_won[e];
        let mut opinion = 0i32;
        for i in 1..EMPIRES {
            if self.alive[i] {
                opinion = (opinion + self.opinion[e][i] as i32).max(0);
            }
        }
        let score = (pop as f64 / 100.0 + tax as f64 / 10.0 + planets as f64 * 100.0 + space as f64 + ground as f64
            + techs as f64 * 10.0
            + spies as f64 * 10.0)
            / 10.0
            + battles.wrapping_mul(10) as f64;
        [tax, pop, planets, space, ground, techs, spies, battles, opinion, trunc(score)]
    }
}

/// `InfoCharts` and `InfoChartsSmoothed`.
#[derive(Debug, Clone, PartialEq)]
pub struct Charts {
    pub raw: Vec<f32>,
    pub smoothed: Vec<f32>,
}

impl Charts {
    pub fn from_bytes(raw: &[u8], smoothed: &[u8]) -> Self {
        let cells = |b: &[u8]| b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        Self { raw: cells(raw), smoothed: cells(smoothed) }
    }

    pub fn to_bytes(&self) -> (Vec<u8>, Vec<u8>) {
        let bytes = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect();
        (bytes(&self.raw), bytes(&self.smoothed))
    }

    fn row(c: usize, r: usize) -> usize {
        (r + c * ROWS) * CELLS
    }

    /// `InfoChartAdd(c, r, 0, v)` (exe 0x4eecb0): the newest cell, then the row's smoothing (spec §1.3).
    pub fn add(&mut self, c: usize, r: usize, v: i32) {
        let at = Self::row(c, r);
        self.raw[at + CELLS - 1] = v as f32;
        let x = &self.raw[at..at + CELLS];
        let pos = |v: f32| if 0.0 > v { 0.0 } else { v };
        for i in 0..CELLS - 8 {
            let s = pos(x[i + 1] * 2.0) + pos(x[i]) + pos(x[i + 2] * 4.0) + pos(x[i + 3] * 5.0) + pos(x[i + 4] * 5.0)
                + pos(x[i + 5] * 4.0)
                + pos(x[i + 6] * 2.0)
                + pos(x[i + 7]);
            self.smoothed[at + i + 4] = s / 24.0;
        }
    }

    /// `InfoChartsSample(0)` (exe 0x4eee00): shift every row by a month, then sample each empire (spec §1.2).
    pub fn sample(&mut self, w: &InfoWorld) {
        for e in 1..=ROWS {
            for c in 0..CHARTS {
                let at = Self::row(c, e - 1);
                self.raw.copy_within(at + 1..at + CELLS, at);
            }
            let values = if w.alive[e] { w.values(e) } else { [-1; CHARTS] };
            for (c, v) in values.into_iter().enumerate() {
                self.add(c, e - 1, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> Charts {
        Charts { raw: vec![-1.0; SIZE / 4], smoothed: vec![-1.0; SIZE / 4] }
    }

    #[test]
    fn a_sample_lands_in_the_last_cell_and_smooths_the_row() {
        let mut c = blank();
        c.add(3, 2, 48);
        let at = Charts::row(3, 2);
        assert_eq!(c.raw[at + 54], 48.0);
        // Cell 50 of the smoothed row covers raw 46..=53 (all −1, clamped to 0) and nothing else.
        assert_eq!(c.smoothed[at + 50], 0.0);
        c.raw[at + 53] = 48.0;
        c.add(3, 2, 48);
        assert_eq!(c.smoothed[at + 50], 2.0);
        assert_eq!(c.smoothed[at + 49], 0.0);
        assert_eq!(c.smoothed[at + 3], -1.0);
        assert_eq!(c.smoothed[at + 51], -1.0);
    }

    #[test]
    fn a_dead_empire_gets_minus_one_everywhere() {
        let mut c = blank();
        let w = InfoWorld { opinion: vec![[0; EMPIRES]; EMPIRES], ..Default::default() };
        c.raw[Charts::row(0, 0) + 54] = 7.0;
        c.sample(&w);
        assert_eq!(c.raw[Charts::row(0, 0) + 53], 7.0);
        assert_eq!(c.raw[Charts::row(0, 0) + 54], -1.0);
    }
}
