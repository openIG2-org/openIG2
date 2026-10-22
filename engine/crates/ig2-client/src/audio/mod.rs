//! The game's audio (re/screens/audio.md): the sound tables' effects with `PlaySound`'s three voices per record, the
//! music selection and crossfade of `PlayMusicMain`/`Menu`/`GrWar`/`SpWar` and `FRISSIT2_SoundSystem`, the volumes of
//! the options file. Everything is read from the user's install and exe at run time.
//!
//! Without a sound card (or in `--snapshot`) the state runs the same and only the log of played ids is kept, so a
//! script's (input → sound) table can be compared with the exe's (`sound.js`).

pub mod decode;
pub mod mixer;
pub mod output;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ig2_formats::Install;
use ig2_rules::sounds::{install_paths, SoundTables, Table};

use mixer::{Chunk, Mixer};

/// A sound id: a table and an index (`PlaySoundGlobal(i)` = `Global`, i).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId {
    pub table: Table,
    pub idx: u16,
}

impl SoundId {
    pub const fn new(table: Table, idx: u16) -> Self {
        Self { table, idx }
    }

    /// `PlaySoundGlobal(i)`.
    pub const fn global(idx: u16) -> Self {
        Self { table: Table::Global, idx }
    }
}

/// The standard button click, `PlaySoundGlobal(0)` (most presses of every screen).
pub const CLICK: SoundId = SoundId::global(0);

/// A played sound, as the tracer logs it (`sound.js`: table, index, the UI tick).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Played {
    pub id: SoundId,
    pub ui: u64,
}

/// The music selectors (re/screens/audio.md §4). The menu, space war and ground war callers are screens still to come.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Music {
    Main,
    Menu,
    GrWar,
    SpWar,
}

/// `ConvertVolume` (ELF 0x670660): per cent → SDL_mixer's 0..=128, in f64, truncated.
pub fn convert_volume(v: f64) -> i32 {
    (v * mixer::MAX_VOLUME as f64 / 100.0) as i32
}

/// `SE_GetRealName` (ELF 0x6725a0) for a voice's path (`sounds/…`, `dataraw/messagesounds/…`): lower case, `.wav`
/// and `.mp2` become `.ogg`, the remaster's `dataraw/` copy first.
pub fn real_names(path: &str) -> [String; 2] {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let stem = lower.strip_suffix(".wav").or_else(|| lower.strip_suffix(".mp2")).or_else(|| lower.strip_suffix(".ogg")).unwrap_or(&lower);
    let stem = stem.strip_prefix("dataraw/").unwrap_or(stem);
    [format!("dataraw/{stem}.ogg"), format!("{stem}.ogg")]
}

/// MSVC's `rand()` (the exe's libc), for the music picks: cosmetic, a stream of the client's own.
fn libc_rand(seed: &mut u32) -> i32 {
    *seed = seed.wrapping_mul(214013).wrapping_add(2531011);
    ((*seed >> 16) & 0x7fff) as i32
}

/// The libc `rand()` of the screens' cosmetic sound picks (the colony's ambients, build and demolish sounds): the exe
/// draws them from the one libc stream the sim also uses; the client keeps them on a stream of its own so the UI
/// never moves a lockstep draw (re/deviations.md).
#[derive(Debug, Clone)]
pub struct Rand(pub u32);

impl Rand {
    /// Seeded from the clock (cosmetic).
    pub fn seeded() -> Self {
        Self(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos()))
    }

    /// `rand()`, 0..=0x7fff.
    pub fn rand(&mut self) -> i32 {
        libc_rand(&mut self.0)
    }

    /// `random(n)` (ELF 0x4f0200) = the exe's inlined `rand() % n`.
    pub fn below(&mut self, n: i32) -> i32 {
        if n < 1 { 0 } else { self.rand() % n }
    }
}

/// What reaches SDL_mixer for an engine volume `v` (0..=128): `SoundEngine::Play` exe 0x55e2a0 hands sgsnd
/// `(v·100) >> 7` per cent, which sgsnd scales back to 0..=128.
pub fn mixer_volume(v: i32) -> i32 {
    ((v * 100) >> 7) * mixer::MAX_VOLUME / 100
}

/// `PlaySound`'s volume for a record of `vol` per cent at the global sound volume `g` (exe 0x55cd20, f64).
pub fn effect_volume(g: i32, vol: i32) -> i32 {
    ((g * vol) as f64 / 100.0 * mixer::MAX_VOLUME as f64 / 100.0) as i32
}

/// `SetGlobalMusicVolume` (ELF 0x670940, exe 0x55d0f0): the music's engine volume at fade level `hangero`, the
/// global music volume `m` and the duck level `duck`.
pub fn music_volume(hangero: f32, m: i32, duck: f32) -> i32 {
    let x = ((hangero as f64 / 100.0) * (m as f32) as f64 - duck as f64) as i32;
    convert_volume(x.max(0) as f64 * 0.9)
}

/// The music globals of the exe (re/screens/audio.md §4; exe addresses in the field docs).
#[derive(Debug, Clone, PartialEq)]
pub struct MusicState {
    /// The track loaded (`Zene`, handle exe 0x919a38), by `ZeneNevek` id.
    pub track: Option<usize>,
    /// `ZeneEppenJatszott` exe 0xe3e3ac.
    pub eppen: usize,
    /// `ZeneRegiMainZene` exe 0xe3e3b0.
    pub regi: usize,
    /// `ZeneHangero` exe 0xe4167c and its target `ZeneHangeroCel` 0x91a694.
    pub hangero: f32,
    pub cel: f32,
    /// `KovetkezoZene` exe 0xe41680: the track waiting for the fade-out to end.
    pub next: Option<usize>,
    /// `GlobalMusicVolumeCsokkento` exe 0xe4166c and `…Real` 0xe41678 (message speech ducking).
    pub duck: i32,
    pub duck_real: f32,
}

impl Default for MusicState {
    fn default() -> Self {
        // `ZeneHangero` starts at 0 and rises toward `Cel` 100 before any track (observed, music1 trace).
        Self { track: None, eppen: 0, regi: 0, hangero: 0.0, cel: 100.0, next: None, duck: 0, duck_real: 0.0 }
    }
}

impl MusicState {
    /// `PlayMusicMain` exe 0x5407c0: the next main track, or `None` while one plays. `race` = `PlayerFaj[EN]`,
    /// `rand` = libc `rand()` (drawn only for the first pick).
    pub fn main_pick(&self, race: u8, self_playing: bool, rand: impl FnOnce() -> i32) -> Option<usize> {
        if (3..12).contains(&self.eppen) {
            return None;
        }
        let mut x = match self.regi {
            3 | 4 => self.regi + 1,
            5 => 3,
            6 | 7 => self.regi + 1,
            8 => 6,
            9 | 10 => self.regi + 1,
            11 => 9,
            0 => match race {
                1 => 6 + (rand() % 3) as usize,
                3 => 9 + (rand() % 3) as usize,
                5 => 3 + (rand() % 3) as usize,
                _ => 9,
            },
            r => r,
        };
        if self_playing {
            x = 1;
        }
        Some(x)
    }

    /// `PlayMusicGlobal` (ELF 0x671b00): start at once (returns the track to start) or fade out toward it.
    fn play(&mut self, id: usize, immediate: bool) -> Option<usize> {
        self.eppen = id;
        if self.track.is_none() || immediate {
            self.hangero = 100.0;
            self.cel = 100.0;
            self.track = Some(id);
            Some(id)
        } else {
            self.cel = 0.0;
            self.next = Some(id);
            None
        }
    }

    /// `FRISSIT2_SoundSystem` exe 0x55c4f0, one 100 Hz UI tick: the duck level ±0.5, the fade ±1.0. Returns the
    /// track to start when a fade-out ends.
    pub fn tick(&mut self) -> Option<usize> {
        let target = self.duck as f32;
        if target > self.duck_real {
            self.duck_real = ((self.duck_real as f64 + 0.5) as f32).min(target);
        } else if target < self.duck_real {
            self.duck_real = ((self.duck_real as f64 - 0.5) as f32).max(target);
        }
        if self.hangero == self.cel {
            return None;
        }
        if self.cel == 0.0 {
            self.hangero -= 1.0;
            if self.hangero <= self.cel {
                self.hangero = 0.0;
                self.cel = 100.0;
                self.track = self.next.take();
                return self.track;
            }
        } else if self.cel == 100.0 {
            self.hangero = (self.hangero + 1.0).min(self.cel);
        }
        None
    }
}

struct Voices {
    chunk: Option<Chunk>,
    channels: [Option<usize>; 3],
}

pub struct Audio {
    install: Install,
    tables: SoundTables,
    output: Option<output::Output>,
    mixer: Option<Arc<Mutex<Mixer>>>,
    records: HashMap<SoundId, Voices>,
    /// `SOUND` / `MUSIC` (exe 0x919930/0x919931) and the options' volumes (`GlobalSoundVolume` 0x91992c,
    /// `GlobalMusicVolume` 0x919928).
    pub sound: bool,
    pub music_on: bool,
    pub sound_volume: i32,
    pub music_volume: i32,
    pub music: MusicState,
    /// The 100 Hz UI ticks run so far (the log's clock).
    pub ui: u64,
    pub log: Vec<Played>,
    /// The voices started (`PlayWav`, `Speech::Play`, `PlayMessagesHang`): the VFS path and the UI tick.
    pub voice_log: Vec<(String, u64)>,
    /// The client's stand-in for libc `rand()` in the music picks (cosmetic; the sim's libc draws stay its own).
    seed: u32,
}

impl Audio {
    /// The sound card when `device`, else silent (the state and log still run).
    pub fn new(install: Install, tables: SoundTables, device: bool) -> Self {
        let (output, mixer) = match device.then(|| output::open(0)).flatten() {
            Some((o, m)) => (Some(o), Some(m)),
            None => (None, None),
        };
        let opts = ig2_formats::options::Options::load();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos());
        Self {
            install,
            tables,
            output,
            mixer,
            records: HashMap::new(),
            sound: true,
            music_on: true,
            sound_volume: opts.int("Sound Volume").unwrap_or(100).clamp(0, 100),
            music_volume: opts.int("Music Volume").unwrap_or(100).clamp(0, 100),
            music: MusicState::default(),
            ui: 0,
            log: Vec::new(),
            voice_log: Vec::new(),
            seed,
        }
    }

    pub fn has_device(&self) -> bool {
        self.output.is_some()
    }

    fn load(&self, id: SoundId) -> Option<Chunk> {
        let m = self.mixer.as_ref()?;
        let rate = m.lock().ok()?.rate;
        let h = self.tables.get(id.table, id.idx as usize)?;
        let path = install_paths(&h.name).into_iter().find(|p| self.install.resolve(p).is_some())?;
        decode::chunk(self.install.read(&path).ok()?, rate).ok()
    }

    /// `PlaySound(&<table>[idx], 0)` exe 0x55cd20: the first stopped of the record's three voices, else the first
    /// restarted.
    pub fn play(&mut self, id: SoundId) {
        if !self.sound {
            return;
        }
        let Some(h) = self.tables.get(id.table, id.idx as usize) else { return };
        let volume = mixer_volume(effect_volume(self.sound_volume, h.volume));
        self.log.push(Played { id, ui: self.ui });
        if !self.records.contains_key(&id) {
            let chunk = self.load(id);
            self.records.insert(id, Voices { chunk, channels: [None; 3] });
        }
        let Some(mixer) = self.mixer.clone() else { return };
        let Ok(mut m) = mixer.lock() else { return };
        let v = self.records.get_mut(&id).unwrap();
        let Some(chunk) = v.chunk.clone() else { return };
        let mut slot = None;
        for i in 0..3 {
            let ch = *v.channels[i].get_or_insert_with(|| m.alloc());
            if !m.playing(ch) {
                slot = Some(ch);
                break;
            }
        }
        let ch = slot.unwrap_or_else(|| {
            let ch = v.channels[0].unwrap();
            m.halt(ch);
            ch
        });
        m.set_volume(ch, volume);
        m.play(ch, &chunk, 0);
    }

    /// The record's first voice (`Hang+0x100`), once it was loaded.
    fn first_voice(&self, id: SoundId) -> Option<usize> {
        self.records.get(&id)?.channels[0]
    }

    /// `StopSoundGlobal(id)` (exe 0x55d250).
    pub fn stop_sound(&mut self, id: SoundId) {
        if let Some(ch) = self.first_voice(id) {
            self.stop(ch);
        }
    }

    /// `PlayOrContinueSoundGlobal` (ELF 0x671500): unloaded → `PlaySoundGlobal`; stopped → played again on its
    /// first voice; playing → nothing.
    pub fn play_or_continue(&mut self, id: SoundId) {
        if !self.sound {
            return;
        }
        let Some(ch) = self.first_voice(id) else { return self.play(id) };
        if self.playing(ch) {
            return;
        }
        let (Some(m), Some(Some(chunk))) = (self.mixer.clone(), self.records.get(&id).map(|v| v.chunk.clone())) else { return };
        if let Ok(mut m) = m.lock() {
            m.play(ch, &chunk, 0);
        };
    }

    /// `MuteSoundGlobal` / `UnMuteSoundGlobal`: the first voice at 0, or back at the record's volume.
    pub fn mute(&mut self, id: SoundId, mute: bool) {
        let Some(ch) = self.first_voice(id) else { return };
        let v = if mute { 0 } else { self.tables.get(id.table, id.idx as usize).map_or(0, |h| mixer_volume(effect_volume(self.sound_volume, h.volume))) };
        if let Some(m) = &self.mixer
            && let Ok(mut m) = m.lock()
        {
            m.set_volume(ch, v);
        }
    }

    /// The messages screen's typer (`Global_Hangok[7]`, re/screens/audio.md §6.6).
    pub fn typing(&mut self, t: crate::messages_screen::Typing) {
        use crate::messages_screen::Typing;
        const TYPER: SoundId = SoundId::global(7);
        match t {
            Typing::Start => {
                self.stop_sound(TYPER);
                self.play(TYPER);
                self.mute(TYPER, true);
            }
            Typing::Char { space } => {
                self.play_or_continue(TYPER);
                self.mute(TYPER, true);
                if !space {
                    self.mute(TYPER, false);
                }
            }
            Typing::Stop => self.stop_sound(TYPER),
        }
    }

    /// A voice file (`PlayWav(name, 0)` exe 0x563e60, `Speech::Play` 0x563d50): a wave of its own at
    /// `ConvertVolume(G)`, nothing played without `SOUND`. The VFS takes the localised copy first. Returns the channel
    /// (`None` silent, or no such file).
    pub fn play_wav(&mut self, path: &str) -> Option<usize> {
        if !self.sound {
            return None;
        }
        self.voice_log.push((path.to_string(), self.ui));
        let mixer = self.mixer.clone()?;
        let rate = mixer.lock().ok()?.rate;
        let real = real_names(path).into_iter().find(|p| self.install.resolve(p).is_some())?;
        let chunk = decode::chunk(self.install.read(&real).ok()?, rate).ok()?;
        let mut m = mixer.lock().ok()?;
        let ch = m.alloc_free();
        m.set_volume(ch, mixer_volume(convert_volume(self.sound_volume as f64)));
        m.play(ch, &chunk, 0);
        Some(ch)
    }

    /// `SoundEngine::GetStatus(h) == 2`.
    pub fn playing(&self, channel: usize) -> bool {
        self.mixer.as_ref().is_some_and(|m| m.lock().is_ok_and(|m| m.playing(channel)))
    }

    /// `SoundEngine::Stop`.
    pub fn stop(&mut self, channel: usize) {
        if let Some(m) = &self.mixer
            && let Ok(mut m) = m.lock()
        {
            m.halt(channel);
        }
    }

    /// `PlayMusicMain`/`Menu`/`GrWar`/`SpWar(a, immediate)` → `PlayMusicExistCheck` → `PlayMusicGlobal`.
    pub fn play_music(&mut self, which: Music, immediate: bool, race: u8) {
        if !self.music_on {
            return;
        }
        let id = match which {
            Music::Main => {
                let mut seed = self.seed;
                let pick = self.music.main_pick(race, false, || libc_rand(&mut seed));
                self.seed = seed;
                let Some(id) = pick else { return };
                self.music.regi = id;
                id
            }
            Music::Menu => {
                self.music.regi = 0;
                12
            }
            Music::GrWar => 13 + (libc_rand(&mut self.seed) % 3) as usize,
            // `SDL_GetTicks()·7 % 3`: the client's milliseconds are its UI ticks × 10.
            Music::SpWar => 16 + (self.ui as u32).wrapping_mul(10).wrapping_mul(7) as usize % 3,
        };
        if let Some(t) = self.music.play(id, immediate) {
            self.start_track(t);
        }
    }

    fn start_track(&mut self, id: usize) {
        let Some(mixer) = self.mixer.clone() else { return };
        let Some(name) = self.tables.music.get(id) else { return };
        let Ok(rate) = mixer.lock().map(|m| m.rate) else { return };
        let Ok(r) = self.install.open_stream(name) else { return };
        let Ok(stream) = decode::OggStream::new(r, rate) else { return };
        if let Ok(mut m) = mixer.lock() {
            m.set_music_volume(mixer_volume(music_volume(self.music.hangero, self.music_volume, self.music.duck_real)));
            m.play_music(Box::new(stream));
        }
    }

    /// One 100 Hz UI tick (`Fake_FRISSITES` → `FRISSIT2_SoundSystem`).
    pub fn tick(&mut self) {
        self.ui += 1;
        if !self.music_on {
            return;
        }
        if let Some(t) = self.music.tick() {
            self.start_track(t);
        }
        if let Some(m) = &self.mixer
            && let Ok(mut m) = m.lock()
        {
            m.set_music_volume(mixer_volume(music_volume(self.music.hangero, self.music_volume, self.music.duck_real)));
        }
    }

    /// Once a frame (`UpdateFrame` exe 0x4e3e90): a loaded track that has ended → the next main track at once.
    pub fn frame(&mut self, race: u8) {
        let ended = match &self.mixer {
            Some(m) => m.lock().is_ok_and(|m| !m.music_playing()),
            None => false,
        };
        if self.music.track.is_some() && ended {
            self.music.eppen = 0;
            self.play_music(Music::Main, true, race);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volumes_follow_the_exe() {
        assert_eq!(convert_volume(100.0), 128);
        assert_eq!(convert_volume(90.0), 115);
        assert_eq!(effect_volume(100, 90), 115);
        assert_eq!(mixer_volume(115), 113);
        assert_eq!(music_volume(100.0, 100, 0.0), 115);
        assert_eq!(music_volume(50.0, 100, 0.0), 57);
        assert_eq!(music_volume(100.0, 100, 33.0), 77);
    }

    #[test]
    fn main_music_rotates_within_the_race() {
        let mut m = MusicState::default();
        // Observed (music2 trace, 2026-10-22): quick-1's human is race 6 → `PlayMusicMain` picks 9 (mfx/pol1.ogg).
        assert_eq!(m.main_pick(6, false, || unreachable!()), Some(9));
        assert_eq!(m.main_pick(1, false, || 4), Some(7));
        assert_eq!(m.main_pick(4, false, || 0), Some(9));
        m.regi = 5;
        assert_eq!(m.main_pick(5, false, || unreachable!()), Some(3));
        m.regi = 11;
        assert_eq!(m.main_pick(3, true, || 0), Some(1));
        m.eppen = 7;
        assert_eq!(m.main_pick(1, false, || 0), None);
    }

    /// Observed (music2 trace): menu at full, `PlayMusicMain(0, 0)` at UI tick 825; `ZeneHangero` 99 at 826 … 1 at
    /// 924, pol1 starts at 925 with 0, then 1 at 926 … 100 at 1025.
    #[test]
    fn a_crossfade_takes_100_ticks_each_way() {
        let mut m = MusicState::default();
        assert_eq!(m.play(12, true), Some(12));
        assert_eq!(m.play(6, false), None);
        let mut started = None;
        let mut n = 0;
        while started.is_none() {
            started = m.tick();
            n += 1;
        }
        assert_eq!((started, n, m.hangero), (Some(6), 100, 0.0));
        m.tick();
        assert_eq!(m.hangero, 1.0);
        for _ in 1..100 {
            m.tick();
        }
        assert_eq!(m.hangero, 100.0);
        m.duck = 33;
        for _ in 0..66 {
            m.tick();
        }
        assert_eq!(m.duck_real, 33.0);
    }

    /// Every table sound and music track of the user's install decodes (IG2_INSTALL_DIR; skipped without it).
    #[test]
    fn the_installs_sounds_decode() {
        let Some(inst) = Install::from_env(ig2_formats::text::Lang::En) else { return };
        let inst = inst.expect("valid install");
        let t = SoundTables::from_exe(&inst.read_raw("ig2.exe").unwrap()).unwrap();
        let mut n = 0;
        for table in Table::ALL {
            for h in &t.tables[table as usize] {
                let Some(p) = install_paths(&h.name).into_iter().find(|p| inst.resolve(p).is_some()) else { continue };
                let c = decode::chunk(inst.read(&p).unwrap(), 48000).unwrap_or_else(|e| panic!("{p}: {e}"));
                assert!(c.len() > 100, "{p}");
                n += 1;
            }
        }
        assert_eq!(n, 196 - 10);
        for m in &t.music {
            let mut s = decode::OggStream::new(inst.open_stream(m).unwrap(), 48000).unwrap();
            let mut buf = [[0.0f32; 2]; 4800];
            assert_eq!(mixer::Stream::read(&mut s, &mut buf), 4800, "{m}");
        }
    }
}
