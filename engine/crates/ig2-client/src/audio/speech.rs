//! The speakers (`SpeakingDirector`, re/screens/audio.md §5): the starmap's fleets and the colony's units speak a
//! line of their kind, at most once a cooldown. The lines come from the user's exe (`ig2_rules::speech`).

use ig2_rules::speech::Speaker;

use super::Rand;

/// Which director (`StarmapSpeaker` exe 0xf17d88, `ColonySpeaker` 0xa3ea3c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    Starmap,
    Colony,
    /// `SpaceWarSpeaker` (exe 0xe72e84).
    SpaceWar,
}

/// The starmap's kinds (`StarmapInitSpeech`).
pub mod starmap {
    pub const MOVE: i32 = 0;
    pub const SELECT: i32 = 1;
    pub const ATTACK: i32 = 2;
}

/// The colony's kinds the screen speaks (`InitColonySpeech`; the war's own: ground-combat.md §11.12).
pub mod colony {
    pub const MOVE: i32 = 0;
    pub const SELECTION: i32 = 1;
    pub const ATTACK: i32 = 2;
}

/// A line to speak: `SpeakingDirector::Play(&who, line, kind)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Speak {
    pub who: Who,
    pub line: i32,
    pub kind: i32,
}

#[derive(Debug, Clone)]
struct Set {
    speaker: i32,
    kind: i32,
    files: Vec<String>,
    /// `Speeches+0x10`.
    cooldown: i32,
}

/// One `SpeakingDirector`.
#[derive(Debug, Clone, Default)]
pub struct Director {
    sets: Vec<Set>,
}

impl Director {
    pub fn new(t: &Speaker) -> Self {
        Self { sets: t.sets.iter().map(|s| Set { speaker: s.speaker, kind: s.kind, files: s.files.clone(), cooldown: 0 }).collect() }
    }

    /// `SpeakingDirector::Play(line, kind)` exe 0x563cd0 for the human's empire `en`: the file to play, if any.
    /// `Speeches::Play` (ELF 0x680200) only off cooldown, which it sets to 10, and `random(count)` picks the n-th of
    /// its list, which `Add` built by prepending: the n-th from the last added. `silent` = `Lang == 3`. The exe
    /// asserts on a missing speaker or kind; here it is silent.
    pub fn play(&mut self, line: i32, kind: i32, en: i32, silent: bool, rand: &mut Rand) -> Option<String> {
        if line == 0 || silent {
            return None;
        }
        let speaker = line
            + match en {
                4 | 5 => 3,
                3 | 7 | 8 => 6,
                _ => 0,
            };
        let s = self.sets.iter_mut().find(|s| s.speaker == speaker && s.kind == kind)?;
        if s.cooldown != 0 {
            return None;
        }
        s.cooldown = 10;
        let n = rand.below(s.files.len() as i32) as usize;
        s.files.iter().rev().nth(n).cloned()
    }

    /// `SpeakingDirector::Refresh` (ELF 0x4f2510): every set's cooldown − 1, floored at 0.
    pub fn refresh(&mut self) {
        for s in &mut self.sets {
            s.cooldown = (s.cooldown - 1).max(0);
        }
    }
}

/// Both directors and their refresh counters.
#[derive(Debug, Clone, Default)]
pub struct Speakers {
    pub starmap: Director,
    pub colony: Director,
    pub spacewar: Director,
    /// `spwcounter` (exe 0xe60430): +1 a battle-screen UI tick, the refresh past 10 (every 11th).
    spacewar_counter: i64,
    /// `StarmapCounter`: +1 a UI tick on the starmap (`FRISSIT_Starmap2`).
    starmap_counter: i64,
    /// `ColonyCounter`'s count for the refresh: +1 a UI tick on every screen (`Frissit2_Colony`).
    colony_counter: i64,
}

impl Speakers {
    pub fn new(t: &ig2_rules::speech::Speech) -> Self {
        Self { starmap: Director::new(&t.starmap), colony: Director::new(&t.colony), spacewar: Director::new(&t.spacewar), ..Default::default() }
    }

    /// One 100 Hz UI tick: each director refreshes on every 10th count of its counter.
    pub fn tick(&mut self, on_starmap: bool) {
        if on_starmap {
            self.starmap_counter += 1;
            if self.starmap_counter % 10 == 0 {
                self.starmap.refresh();
            }
        }
        self.colony_counter += 1;
        if self.colony_counter % 10 == 0 {
            self.colony.refresh();
        }
    }

    /// One UI tick of the battle screen (`SpaceWarFrissit2`): `spwcounter` past 10 → 0 and a refresh.
    pub fn tick_spacewar(&mut self) {
        self.spacewar_counter += 1;
        if self.spacewar_counter > 10 {
            self.spacewar_counter = 0;
            self.spacewar.refresh();
        }
    }

    pub fn director(&mut self, who: Who) -> &mut Director {
        match who {
            Who::Starmap => &mut self.starmap,
            Who::Colony => &mut self.colony,
            Who::SpaceWar => &mut self.spacewar,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ig2_rules::speech::LineSet;

    fn director() -> Director {
        let set = |speaker, kind, n: usize| LineSet { speaker, kind, files: (0..n).map(|i| format!("f{speaker}_{kind}_{i}")).collect() };
        Director::new(&Speaker { sets: vec![set(1, 1, 3), set(4, 1, 2), set(7, 0, 1)] })
    }

    /// A line speaks once a cooldown of 10 refreshes, picks from the end of the list, and the race moves the speaker.
    #[test]
    fn cooldown_pick_and_race() {
        let mut d = director();
        let mut r = Rand(0);
        // MSVC rand() from seed 0: 38 → 38 % 3 = 2: the third from the last added, the first file.
        assert_eq!(d.play(1, 1, 6, false, &mut r).as_deref(), Some("f1_1_0"));
        assert_eq!(d.play(1, 1, 6, false, &mut r), None);
        for _ in 0..9 {
            d.refresh();
        }
        assert_eq!(d.play(1, 1, 6, false, &mut r), None);
        d.refresh();
        assert!(d.play(1, 1, 6, false, &mut r).is_some());
        assert!(d.play(1, 1, 5, false, &mut r).unwrap().starts_with("f4_1"));
        assert_eq!(d.play(1, 0, 8, false, &mut r).as_deref(), Some("f7_0_0"));
        assert_eq!(d.play(2, 1, 1, false, &mut r), None);
        assert_eq!(d.play(0, 1, 1, false, &mut r), None);
        let mut d = director();
        assert_eq!(d.play(1, 1, 1, true, &mut r), None);
    }

    /// The starmap's refresh runs on its own screen only; the colony's everywhere. Observed (speech probe, 2026-10-23):
    /// `StarmapSpeaker`'s (1, 1) at 10 after a play, unchanged by a second, then −1 every 10 UI ticks (903, 913, …).
    #[test]
    fn refresh_counters() {
        let mut s = Speakers { starmap: director(), colony: director(), ..Default::default() };
        let mut r = Rand(0);
        assert!(s.starmap.play(1, 1, 1, false, &mut r).is_some());
        assert!(s.colony.play(1, 1, 1, false, &mut r).is_some());
        for _ in 0..100 {
            s.tick(false);
        }
        assert!(s.starmap.play(1, 1, 1, false, &mut r).is_none());
        assert!(s.colony.play(1, 1, 1, false, &mut r).is_some());
        for _ in 0..100 {
            s.tick(true);
        }
        assert!(s.starmap.play(1, 1, 1, false, &mut r).is_some());
    }
}
