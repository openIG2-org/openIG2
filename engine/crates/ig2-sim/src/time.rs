//! Game calendar and the fixed-step simulation clock. Spec: `re/rules/time-and-rng.md` §1–2.
//! Verified at runtime: 20.00 ticks/s, `Ido` steps 1/4/16, calendar exact (`tests/trace.rs`).

/// `Ido` units per day / month / year.
pub const PER_DAY: i32 = 50;
pub const PER_MONTH: i32 = 30 * PER_DAY;
pub const PER_YEAR: i32 = 12 * PER_MONTH;

/// Real milliseconds per simulation tick (`OldIdo += 50`).
pub const SIM_TICK_MS: u32 = 50;
/// Real milliseconds per input/UI tick (`OldIdo2 += 10`).
pub const UI_TICK_MS: u32 = 10;
/// Backlog clamp (`if Old < now - 500 { Old = now - 500 }`).
pub const MAX_BACKLOG_MS: u32 = 500;

/// Game time `Ido`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct GameTime(pub i32);

impl GameTime {
    /// `DatumToIdo(ev, honap, nap, x)`.
    pub fn from_date(year: i32, month: i32, day: i32, sub: i32) -> Self {
        Self(year * PER_YEAR + (month - 1) * PER_MONTH + (day - 1) * PER_DAY + sub)
    }
    /// `Ido2Ev`.
    pub fn year(self) -> i32 {
        self.0 / PER_YEAR
    }
    /// `Ido2Honap`: 1..=12.
    pub fn month(self) -> i32 {
        (self.0 / PER_MONTH) % 12 + 1
    }
    /// `Ido2Nap`: 1..=30.
    pub fn day(self) -> i32 {
        (self.0 / PER_DAY) % 30 + 1
    }
}

/// Game speed setting (`IdoSpeed` 1..=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Speed {
    Normal = 1,
    Fast = 2,
    Fastest = 3,
}

impl Speed {
    /// `IdoSpeedSzorzo`: `Ido` units added per sim tick (`SetIdoSpeed`).
    pub fn ido_per_tick(self, multiplayer: bool) -> i32 {
        match (self, multiplayer) {
            (Speed::Normal, false) => 1,
            (Speed::Fast, false) => 4,
            (Speed::Fastest, false) => 16,
            (Speed::Normal, true) => 4,
            (Speed::Fast, true) => 8,
            (Speed::Fastest, true) => 16,
        }
    }
    /// `IdoSpeedSzorzoBattle`.
    pub fn battle_multiplier(self, multiplayer: bool) -> i32 {
        match (self, multiplayer) {
            (Speed::Normal, _) => 1,
            (Speed::Fast, _) => 2,
            (Speed::Fastest, false) => 4,
            (Speed::Fastest, true) => 2,
        }
    }
}

/// What `SetIdoLelassit` (exe 0x541270, `SetIdoSpeed(1, false)` in the ELF) writes: `IdoSpeed` 0xBAE258 (the
/// speed button the top bar lights) and, for a level 1..=3, `IdoSpeedSzorzo` 0xBAE25C with the battle
/// multiplier 0xBAE260 (`mult`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlowDown {
    pub level: i32,
    pub mult: Option<(i32, i32)>,
}

/// `SetIdoLelassit` (exe 0x541270; its argument is ignored): SP drops to level 1 (×1, battles ×1) unless the
/// speed is locked (`nemlehetidotallitani`, exe 0xBAD0D4: nothing written); MP returns to the program speed
/// (`ProgramSebesseg` 0xBAE54C: 1/2/3 → ×4/×8/×16, battles 1/2/2; 4 = nothing written; any other value only
/// sets `IdoSpeed`). It writes the globals at once, so the rest of the running sim tick already sees the new
/// multiplier (research loop, starmap movement).
pub fn slow_down(multiplayer: bool, speed_locked: bool, program_speed: i32) -> Option<SlowDown> {
    if !multiplayer {
        return (!speed_locked).then_some(SlowDown { level: 1, mult: Some((1, 1)) });
    }
    let speed = match program_speed {
        4 => return None,
        1 => Some(Speed::Normal),
        2 => Some(Speed::Fast),
        3 => Some(Speed::Fastest),
        _ => None,
    };
    Some(SlowDown { level: program_speed, mult: speed.map(|s| (s.ido_per_tick(true), s.battle_multiplier(true))) })
}

/// The `IdoSpeedSzorzo` after [`slow_down`].
pub fn slowed_down(speed_mult: i32, multiplayer: bool, speed_locked: bool, program_speed: i32) -> i32 {
    slow_down(multiplayer, speed_locked, program_speed).and_then(|s| s.mult).map_or(speed_mult, |(m, _)| m)
}

/// Wall-clock → tick scheduler with the original's catch-up rules: a tick fires while
/// `now - last > period` (strictly greater), `last += period`, and the backlog is clamped to 500 ms.
#[derive(Debug, Clone, Copy)]
pub struct TickClock {
    period: u32,
    last: u32,
}

impl TickClock {
    pub fn new(period_ms: u32, now_ms: u32) -> Self {
        Self { period: period_ms, last: now_ms }
    }

    /// Number of ticks due at `now_ms` (and consume them).
    pub fn advance(&mut self, now_ms: u32) -> u32 {
        if self.last < now_ms.saturating_sub(MAX_BACKLOG_MS) {
            self.last = now_ms - MAX_BACKLOG_MS;
        }
        let mut n = 0;
        while now_ms.wrapping_sub(self.last) > self.period {
            self.last += self.period;
            n += 1;
        }
        n
    }

    /// How far `now_ms` is into the next period, 0..=1 (after [`TickClock::advance`]): the renderer's
    /// interpolation factor between the last two ticks (ADR 0006).
    pub fn phase(&self, now_ms: u32) -> f32 {
        (now_ms.wrapping_sub(self.last) as f32 / self.period as f32).clamp(0.0, 1.0)
    }
}

/// Calendar state advanced by sim ticks (`FRISSIT_Datum` + `IdoNapValtozott`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calendar {
    pub now: GameTime,
    /// `Ido2`: ticks of running strategic time.
    pub ticks: u32,
    old_day: i32,
}

/// What a strategic tick produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TickEvents {
    /// The day-of-month changed: run relations, AI, spies, AI diplomacy.
    pub new_day: bool,
    /// `Nap % 6 == 1` on a new day: planet economy update (`FRISSIT_Planet(true, 0)`).
    pub planet_update: bool,
    /// Day 1 of months 1 and 7 (SP autosave point).
    pub autosave_point: bool,
}

impl Calendar {
    pub fn new(now: GameTime) -> Self {
        Self { now, ticks: 0, old_day: now.day() }
    }

    /// One strategic sim tick: `Ido += ido_per_tick`, then the day-change check.
    pub fn tick(&mut self, ido_per_tick: i32) -> TickEvents {
        self.now.0 += ido_per_tick;
        self.ticks += 1;
        let day = self.now.day();
        if day == self.old_day {
            return TickEvents::default();
        }
        self.old_day = day;
        TickEvents {
            new_day: true,
            planet_update: day % 6 == 1,
            autosave_point: day == 1 && self.now.month() % 6 == 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_formulas() {
        let t = GameTime(41_890_761); // quick.scn-era value: year 2327, month 4, day 6
        assert_eq!((t.year(), t.month(), t.day()), (2327, 4, 6));
        assert_eq!(GameTime::from_date(2327, 4, 6, 11), t);
    }

    #[test]
    fn clock_catch_up_and_clamp() {
        let mut c = TickClock::new(SIM_TICK_MS, 1000);
        assert_eq!(c.advance(1050), 0, "needs strictly more than 50 ms");
        assert_eq!(c.advance(1051), 1);
        assert_eq!(c.advance(10_000), 9, "backlog clamped to 500 ms; strict > 50 → at most 9 ticks");
    }

    #[test]
    fn clock_phase_is_the_way_into_the_next_tick() {
        let mut c = TickClock::new(SIM_TICK_MS, 1000);
        assert_eq!(c.advance(1075), 1);
        assert_eq!(c.phase(1075), 0.5);
        assert_eq!(c.phase(1050), 0.0);
        assert_eq!(c.phase(1200), 1.0, "clamped while ticks are due");
    }

    #[test]
    fn slow_down_drops_sp_to_normal_and_mp_to_the_program_speed() {
        assert_eq!(slowed_down(16, false, false, 3), 1);
        assert_eq!(slowed_down(16, false, true, 3), 16, "speed locked");
        assert_eq!((slowed_down(16, true, false, 1), slowed_down(4, true, false, 2), slowed_down(4, true, false, 3)), (4, 8, 16));
        assert_eq!(slowed_down(16, true, false, 4), 16, "program speed 4: unchanged");
    }

    /// Row 307: the slow-down also sets `IdoSpeed`, the lit speed button (exe 0x541270).
    #[test]
    fn slow_down_sets_the_speed_level_and_battle_multiplier() {
        let s = |level, mult| Some(SlowDown { level, mult });
        assert_eq!(slow_down(false, false, 3), s(1, Some((1, 1))));
        assert_eq!(slow_down(false, true, 3), None, "speed locked");
        assert_eq!(
            (slow_down(true, false, 1), slow_down(true, false, 2), slow_down(true, false, 3)),
            (s(1, Some((4, 1))), s(2, Some((8, 2))), s(3, Some((16, 2))))
        );
        assert_eq!(slow_down(true, false, 4), None, "the players pick the speed");
        assert_eq!(slow_down(true, false, 0), s(0, None), "only IdoSpeed");
    }

    #[test]
    fn no_day_is_skipped_at_max_speed() {
        let mut cal = Calendar::new(GameTime::from_date(2300, 1, 1, 0));
        let mut days = 0;
        for _ in 0..(PER_YEAR / 16 + 1) {
            if cal.tick(Speed::Fastest.ido_per_tick(false)).new_day {
                days += 1;
            }
        }
        assert_eq!(days, 360);
    }
}
