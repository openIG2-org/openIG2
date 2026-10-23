//! The voice lines of the starmap's and the colony's speakers (re/screens/audio.md §5): `StarmapInitSpeech` exe
//! 0x5ad820 and `InitColonySpeech` exe 0x47f400 are flat runs of `SpeakingDirector::Add(file, speaker, kind)` calls
//! (exe 0x563b00) with literal names, read here from the user's exe.

use serde::Serialize;

use crate::{Error, PeImage, Result};

/// `SpeakingDirector::Add` (ELF 0x680020).
const ADD: u32 = 0x563b00;
/// (function, the director it fills: `StarmapSpeaker` / `ColonySpeaker`).
const STARMAP: (u32, u32) = (0x5ad820, 0xf17d88);
const COLONY: (u32, u32) = (0x47f400, 0xa3ea3c);
/// `InitSpaceWarSpeech` (exe 0x55e8b0) into `SpaceWarSpeaker` (exe 0xe72e84): the colony's soldiers' files.
const SPACEWAR: (u32, u32) = (0x55e8b0, 0xe72e84);

/// One speaker's lines of one kind (`Speeches`), in the order the exe adds them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineSet {
    pub speaker: i32,
    pub kind: i32,
    /// `Sounds/Starmap/Speeches/Move/H_soldier1_02.wav`, …
    pub files: Vec<String>,
}

/// A `SpeakingDirector` as its init function builds it: one set per (speaker, kind), in first-added order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Speaker {
    pub sets: Vec<LineSet>,
}

impl Speaker {
    pub fn set(&self, speaker: i32, kind: i32) -> Option<&LineSet> {
        self.sets.iter().find(|s| s.speaker == speaker && s.kind == kind)
    }

    pub fn len(&self) -> usize {
        self.sets.iter().map(|s| s.files.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Speech {
    pub starmap: Speaker,
    pub colony: Speaker,
    pub spacewar: Speaker,
}

impl Speech {
    pub fn from_exe(exe: &[u8]) -> Result<Self> {
        crate::check_build(exe)?;
        let img = PeImage::parse(exe)?;
        Ok(Self { starmap: read(&img, STARMAP)?, colony: read(&img, COLONY)?, spacewar: read(&img, SPACEWAR)? })
    }
}

/// Decodes the init function up to its `ret`. Only the instructions these two functions use are known: pushes of
/// immediates, `mov ecx, imm32` (the director), `mov al, [m]` / `mov [m], al` (`KELLBESZED = SOUND`) and the call.
fn read(img: &PeImage, (func, director): (u32, u32)) -> Result<Speaker> {
    let mut out = Speaker::default();
    let mut stack: Vec<u32> = Vec::new();
    let mut ecx = 0;
    let mut pc = func;
    let u32_at = |va: u32| img.read(va, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    loop {
        let op = img.read(pc, 1)?[0];
        match op {
            0x6a => {
                stack.push(img.read(pc + 1, 1)?[0] as i8 as i32 as u32);
                pc += 2;
            }
            0x68 => {
                stack.push(u32_at(pc + 1)?);
                pc += 5;
            }
            0xb9 => {
                ecx = u32_at(pc + 1)?;
                pc += 5;
            }
            0xa0 | 0xa2 => pc += 5,
            0xe8 => {
                let target = (pc + 5).wrapping_add(u32_at(pc + 1)?);
                if target != ADD || ecx != director || stack.len() < 3 {
                    return Err(Error::Pe("unexpected call in a speech init function"));
                }
                let (name, speaker, kind) = (stack.pop().unwrap(), stack.pop().unwrap() as i32, stack.pop().unwrap() as i32);
                let b = img.read(name, 128)?;
                let end = b.iter().position(|&c| c == 0).ok_or(Error::Pe("unterminated speech name"))?;
                let file = b[..end].iter().map(|&c| c as char).collect();
                match out.sets.iter_mut().find(|s| s.speaker == speaker && s.kind == kind) {
                    Some(s) => s.files.push(file),
                    None => out.sets.push(LineSet { speaker, kind, files: vec![file] }),
                }
                pc += 5;
            }
            0xc3 if stack.is_empty() => return Ok(out),
            _ => return Err(Error::Pe("unknown instruction in a speech init function")),
        }
    }
}
