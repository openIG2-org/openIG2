//! Seeded random streams, bit-exact with the original `rndold` (ELF 0x622130; inlined into
//! `rndlog` @0x542da0 in `ig2.exe`). Spec: `re/rules/time-and-rng.md` §3.
//! Verified against 1,114 calls traced from the running game (`tests/trace.rs`).

/// Number of original streams (`RandSeed[0..21]`).
pub const CLASSIC_STREAMS: usize = 21;

/// Classic stream allocation (constant first argument at the original call sites).
pub mod stream {
    pub const GENERATION: usize = 1;
    pub const GROUND_COMBAT: usize = 2;
    pub const PLANET: usize = 3;
    pub const SPY: usize = 4;
    pub const STORY: usize = 5;
    pub const SPACE_COMBAT: usize = 6;
    pub const AUTO_BATTLE: usize = 7;
    pub const REBELLION_VIRUS: usize = 8;
    pub const AI_DIPLOMACY: usize = 9;
    pub const AI: usize = 10;
    pub const TRADE: usize = 11;
    pub const WATER_FX: usize = 12;
    pub const GROUND_AI: usize = 13;
    pub const STARMAP: usize = 14;
    /// First of the streams added by ADR 0003 for call sites that used unseeded libc `rand()`.
    pub const FIRST_EXTRA: usize = 21;
}

/// One `rndold` state update: MSVC LCG step, then rotate left by 21.
#[inline]
pub fn step(state: u32) -> u32 {
    state.wrapping_mul(214_013).wrapping_add(2_531_011).rotate_left(21)
}

/// The value `rndold` returns for a freshly stepped state.
#[inline]
pub fn value(state: u32, lo: i32, hi: i32) -> i32 {
    if lo > hi {
        return lo;
    }
    let span = (hi as i64 - lo as i64 + 1) as u64;
    (((state >> 16) & 0x7FFF) as u64 % span) as i32 + lo
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Streams {
    state: Vec<u32>,
    /// `RndValasztas` running total (`RndOsszEsely`), shared like the original's global.
    choice_total: i32,
    /// `RndHanyadik`: candidates offered since the last `RndInitValasztas`.
    choice_count: i32,
}

impl Default for Streams {
    fn default() -> Self {
        Self::new(CLASSIC_STREAMS)
    }
}

impl Streams {
    /// All streams at state 0 (what the ELF build's `RandomInit` produces).
    pub fn new(count: usize) -> Self {
        Self { state: vec![0; count.max(CLASSIC_STREAMS)], choice_total: 0, choice_count: 0 }
    }

    /// Load a scenario/save `RandSeed` line: 20 values into streams 1..=20 (`LoadScenario`).
    pub fn load_scenario_seeds(&mut self, seeds: &[i64]) {
        for (i, &v) in seeds.iter().take(20).enumerate() {
            self.state[i + 1] = v as u32;
        }
    }

    /// The values to write back as a `RandSeed` line (streams 1..=20).
    pub fn scenario_seeds(&self) -> Vec<i64> {
        self.state[1..=20].iter().map(|&s| s as i32 as i64).collect()
    }

    pub fn state(&self, stream: usize) -> u32 {
        self.state[stream]
    }

    pub fn set_state(&mut self, stream: usize, s: u32) {
        self.state[stream] = s;
    }

    /// `rndold(stream, lo, hi)`: inclusive range; returns `lo` when `lo > hi` (the state still advances).
    pub fn range(&mut self, stream: usize, lo: i32, hi: i32) -> i32 {
        let s = step(self.state[stream]);
        self.state[stream] = s;
        value(s, lo, hi)
    }

    /// `RndInitValasztas`: start a weighted choice.
    pub fn choice_begin(&mut self) {
        self.choice_total = 0;
        self.choice_count = 0;
    }

    /// `(RndHanyadik, RndOsszEsely)`.
    pub fn choice_state(&self) -> (i32, i32) {
        (self.choice_count, self.choice_total)
    }

    pub fn set_choice_state(&mut self, count: i32, total: i32) {
        self.choice_count = count;
        self.choice_total = total;
    }

    /// `RndValasztas(stream, weight)`: called once per candidate in order; returns true if this
    /// candidate replaces the current pick (reservoir-style: P = weight / running total).
    pub fn choice_offer(&mut self, stream: usize, weight: i32) -> bool {
        self.choice_count = self.choice_count.wrapping_add(1);
        self.choice_total = self.choice_total.wrapping_add(weight);
        self.range(stream, 1, self.choice_total) <= weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sequence_from_zero() {
        // Hand-checked against the formula: state 0 → 2531011 → rotl 21.
        let s1 = step(0);
        assert_eq!(s1, 2_531_011u32.rotate_left(21));
        let mut r = Streams::default();
        let v = r.range(3, 1, 100);
        assert_eq!(v, ((s1 >> 16) & 0x7FFF) as i32 % 100 + 1);
        assert_eq!(r.state(3), s1);
    }

    #[test]
    fn empty_range_returns_lo_but_advances() {
        let mut r = Streams::default();
        assert_eq!(r.range(1, 5, 4), 5);
        assert_ne!(r.state(1), 0);
    }

    #[test]
    fn seeds_roundtrip() {
        let seeds: Vec<i64> = (1..=20).map(|i| i * -12345).collect();
        let mut r = Streams::default();
        r.load_scenario_seeds(&seeds);
        assert_eq!(r.scenario_seeds(), seeds);
        assert_eq!(r.state(0), 0);
    }
}
