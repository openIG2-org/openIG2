//! The colony screen's voice lines (`ColonySpeaker`, a `SpeakingDirector` at exe 0xA3EA3C; ELF
//! `speech.cpp`, spec §11.12). Purely cosmetic, but a line that plays picks its sample with a libc `rand()`,
//! and that draw shifts the debris and so the order of list B: the replay has to count it.

/// One kind of line of one speaker (`Speeches`: exe `+0` samples, `+8` cooldown, `+0xc` kind).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceLine {
    pub speaker: i32,
    pub kind: i32,
    pub samples: i32,
    /// Frames until the line can play again (set to 10 by a play, counted down by [`Voices::refresh`]).
    pub cooldown: i32,
}

/// The speakers of the colony screen as `InitColonySpeech` (ELF 0x4d2250) loads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Voices {
    /// `Lang == 3` (exe 0x912724): no voices at all.
    pub silent: bool,
    /// The global `0x9B2EBC`: `RearrangeTankGroups` sets it while it merges groups (no death voice) and
    /// leaves it set when it returns early [original quirk].
    pub muted: bool,
    /// In the director's list order (the first match wins).
    pub lines: Vec<VoiceLine>,
}

impl Voices {
    /// `SpeakingDirector::Play(line, kind)` (exe 0x563cd0) for the human's race `en`: the speaker is
    /// `line` + 3 (races 4, 5) or + 6 (races 3, 7, 8). Returns whether a sample is picked, i.e. one libc
    /// draw (`Speeches::Play` 0x563d50: only off cooldown, and `rand() % samples` only with samples).
    pub fn play(&mut self, line: i32, kind: i32, en: i32) -> bool {
        if line == 0 || self.silent {
            return false;
        }
        let speaker = line
            + match en {
                4 | 5 => 3,
                3 | 7 | 8 => 6,
                _ => 0,
            };
        let Some(first) = self.lines.iter().position(|l| l.speaker == speaker) else { return false };
        let Some(l) = self.lines[first..].iter_mut().take_while(|l| l.speaker == speaker).find(|l| l.kind == kind) else {
            return false;
        };
        if l.cooldown != 0 {
            return false;
        }
        l.cooldown = 10;
        l.samples >= 1
    }

    /// `SpeakingDirector::Refresh` (ELF 0x4f2510, once per colony-screen frame from `Colony_Main`).
    pub fn refresh(&mut self) {
        for l in &mut self.lines {
            l.cooldown = (l.cooldown - 1).max(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voices() -> Voices {
        let l = |speaker, kind, samples| VoiceLine { speaker, kind, samples, cooldown: 0 };
        Voices { silent: false, muted: false, lines: vec![l(1, 5, 3), l(1, 7, 0), l(4, 5, 2), l(7, 9, 1)] }
    }

    #[test]
    fn a_line_draws_once_per_cooldown() {
        let mut v = voices();
        assert!(v.play(1, 5, 6));
        assert!(!v.play(1, 5, 6));
        for _ in 0..9 {
            v.refresh();
        }
        assert!(!v.play(1, 5, 6));
        v.refresh();
        assert!(v.play(1, 5, 6));
    }

    #[test]
    fn race_offsets_and_empty_lines() {
        let mut v = voices();
        assert!(v.play(1, 5, 5)); // speaker 4
        assert!(v.play(1, 9, 8)); // speaker 7
        assert!(!v.play(1, 7, 6)); // no samples: no draw, but the cooldown starts
        assert_eq!(v.lines[1].cooldown, 10);
        assert!(!v.play(2, 5, 6)); // no such speaker
        assert!(!v.play(0, 5, 6));
        v.silent = true;
        assert!(!v.play(1, 5, 6));
    }
}
