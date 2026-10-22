//! The sound tables (`<Screen>_Hangok`, re/screens/sound.md §1): `Hang` records of 0x11c bytes in the exe's .data,
//! file name at +0 (relative to `sounds/`), loop +0x104, volume in per cent +0x110, "load at init" +0x114
//! [`InitSounds` exe 0x55c790, `PlaySound` exe 0x55cd20]. A sound id is a table and an index. Also the music names
//! `ZeneNevek` (re/screens/audio.md §4).

use serde::Serialize;

use crate::{Error, PeImage, Result};

const RECORD: usize = 0x11c;

/// The tables of the catalogued build, in the order of [`Table`].
const TABLES: [(u32, usize); 11] = [
    (0x919a60, 11), // Global_Hangok (PlaySoundGlobal)
    (0x8ed140, 58), // Colony_Hangok
    (0x8f51f0, 44), // Gr_Hangok
    (0x902a68, 4),  // Des_Hangok
    (0x903008, 3),  // Dip_Hangok
    (0x9140e8, 6),  // Mainscr_Hangok
    (0x914950, 19), // Mes_Hangok
    (0x918c88, 4),  // Res_Hangok
    (0x91a838, 42), // SpWar_Hangok
    (0x926b90, 3),  // Spy_Hangok
    (0x928af0, 2),  // Trade_Hangok
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Table {
    Global,
    Colony,
    Gr,
    Des,
    Dip,
    Mainscr,
    Mes,
    Res,
    SpWar,
    Spy,
    Trade,
}

impl Table {
    pub const ALL: [Table; 11] = [
        Table::Global, Table::Colony, Table::Gr, Table::Des, Table::Dip, Table::Mainscr, Table::Mes, Table::Res,
        Table::SpWar, Table::Spy, Table::Trade,
    ];

    /// The tracer's name for it (`sound.js`: `Global_Hangok` → `Global`).
    pub fn name(self) -> &'static str {
        ["Global", "Colony", "Gr", "Des", "Dip", "Mainscr", "Mes", "Res", "SpWar", "Spy", "Trade"][self as usize]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hang {
    /// Relative to `sounds/`, as the 1999 build names it (`colony/Click.wav`).
    pub name: String,
    /// The loop field as stored (0, 4 or 6 in the GOG build; re/screens/audio.md §2).
    pub looping: i32,
    /// Per cent of the global sound volume.
    pub volume: i32,
    /// Loaded by `InitSounds` rather than when first played (re/screens/audio.md §2).
    pub preload: bool,
}

/// `ZeneNevek` exe 0x91a698: 19 pointers to the music files (`mfx/menu.ogg`).
const MUSIC_NAMES: (u32, usize) = (0x91a698, 19);

#[derive(Debug, Clone, Serialize)]
pub struct SoundTables {
    pub tables: Vec<Vec<Hang>>,
    /// The music tracks by id (`PlayMusicGlobal`'s index).
    pub music: Vec<String>,
}

impl SoundTables {
    pub fn from_exe(exe: &[u8]) -> Result<Self> {
        crate::check_build(exe)?;
        let img = PeImage::parse(exe)?;
        let i32_at = |b: &[u8], o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let mut tables = Vec::new();
        for (va, n) in TABLES {
            let b = img.read(va, RECORD * n)?;
            let mut t = Vec::with_capacity(n);
            for r in b.chunks_exact(RECORD) {
                let end = r[..0x100].iter().position(|&c| c == 0).ok_or(Error::Pe("unterminated Hang name"))?;
                t.push(Hang {
                    name: r[..end].iter().map(|&c| c as char).collect(),
                    looping: i32_at(r, 0x104),
                    volume: i32_at(r, 0x110),
                    preload: i32_at(r, 0x114) != 0,
                });
            }
            tables.push(t);
        }
        let ptrs = img.read(MUSIC_NAMES.0, 4 * MUSIC_NAMES.1)?;
        let mut music = Vec::new();
        for p in ptrs.chunks_exact(4) {
            let va = u32::from_le_bytes(p.try_into().unwrap());
            let b = img.read(va, 64)?;
            let end = b.iter().position(|&c| c == 0).ok_or(Error::Pe("unterminated music name"))?;
            music.push(b[..end].iter().map(|&c| c as char).collect());
        }
        Ok(Self { tables, music })
    }

    pub fn get(&self, table: Table, idx: usize) -> Option<&Hang> {
        self.tables[table as usize].get(idx)
    }
}

/// The GOG install's files for a table name, in lookup order: `dir/Name.wav` → `dataraw/sounds/dir/name.ogg`, else
/// `sounds/dir/name.ogg` (the `colony/st/` names live only there; re/screens/audio.md §1).
pub fn install_paths(name: &str) -> [String; 2] {
    let lower = name.replace('\\', "/").to_ascii_lowercase();
    let stem = lower.strip_suffix(".wav").unwrap_or(&lower);
    [format!("dataraw/sounds/{stem}.ogg"), format!("sounds/{stem}.ogg")]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_names_map_to_oggs() {
        assert_eq!(install_paths("colony/Click.wav")[0], "dataraw/sounds/colony/click.ogg");
    }
}
