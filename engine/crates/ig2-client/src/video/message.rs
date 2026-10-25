//! The messages screen's talking-head clip (re/screens/video.md §5.3): `StartAnim` exe 0x5095b0,
//! `VegeAzElsoAnimnak` 0x509e40, `StartAnim_EndOfMessage` 0x5096a0, played by `MessagesAnimFrissit` 0x506f60 in the
//! screen at (201, 204), silent, never skipped. `AnimAkt` (0xd909bc): 0 the speaking clip (looped), 1 the wait clip
//! (looped), 2 the end clip (once).

use ig2_formats::dialogue::ClipGroup;
use ig2_formats::Install;

use super::playback::{Place, Playback};

/// `OpenBink(name, 0, 0xc9, 0xcc, 1)`.
pub const AT: [f32; 2] = [201.0, 204.0];

/// `GetUzenetAnimLista` (ELF 0x5a4a80): the speaking and wait clips of message `msg`'s part `part`, as `.bik` names
/// (`messages/` + the clip unless it holds `misc/`); `"messages/"` (no file) where the list has none.
pub fn clip_names(lists: &[ClipGroup], msg: u32, part: u32) -> (String, String) {
    let name = |c: &str| if c.contains("misc/") { format!("{c}.bik") } else { format!("messages/{c}.bik") };
    let found = lists.iter().find(|g| g.message_ids.contains(&msg)).and_then(|g| g.parts.iter().find(|p| p.part == part));
    match found {
        Some(p) => (name(&p.clip), p.loop_clip.as_deref().map_or_else(|| "messages/".to_string(), name)),
        None => ("messages/".to_string(), "messages/".to_string()),
    }
}

/// `ReadUzenetAnimLista`: `text/uzenetanim_<EN>.txt` for the human's empire 1, 3, 5, else 1.
pub fn load_lists(install: &Install, en: i32) -> Vec<ClipGroup> {
    let n = if matches!(en, 1 | 3 | 5) { en } else { 1 };
    install
        .read(&format!("dataraw/text/uzenetanim_{n}.txt"))
        .map(|b| ig2_formats::dialogue::parse_clip_lists(&String::from_utf8_lossy(&b)))
        .unwrap_or_default()
}

#[derive(Default)]
pub struct HeadClip {
    /// `AnimAkt`.
    pub akt: u8,
    /// `AnimFile` and the wait clip (`AnimFile + 0x100`), from the last `TE` part read.
    pub anim: String,
    pub wait: String,
    /// `SoundFile` (exe 0xd90bc0): `%04d_%02d.ogg` of the last `TE` part read, the speech `StartAnim` plays.
    pub sound: String,
    /// `UzenetFaj` (exe 0xd90ddc): the part's `#n` (1..=19, else 1), 0 without one.
    pub face: i32,
    pub playback: Option<Playback>,
    /// A clip opened by the typing pass (`MessagesGfx2D_2`'s end of text, after `MessagesAnimFrissit` drew): it
    /// plays from the next drawn frame.
    pub fresh: bool,
}

/// `PlayHalkAlienHang` exe 0x509120 (ELF 0x5a1470): the alien voice's index for the speaking clip `anim`
/// (`AnimFile`), the human's empire `en`, the message's `UzenetekRandomKitol` `k` and `UzenetFaj` `face`; −1 none.
pub fn alien_index(anim: &str, en: i32, k: i32, face: i32) -> i32 {
    let mut idx = -1;
    if matches!(en, 1 | 3 | 5) {
        if anim.contains("pirate") {
            idx = 10;
        }
        if anim.contains("trader") {
            idx = 9;
        }
    }
    if anim.contains("misc") && en != k - 1 {
        idx = k - 1;
    }
    if face != 0 {
        idx = face;
    }
    idx
}

/// The record of `Mes_Hangok[3..]` (`alienhangok/Alien-*.wav`) for an index, from the exe's jump table at 0x509228.
pub fn alien_record(idx: i32) -> Option<u16> {
    const REC: [u16; 19] = [11, 9, 3, 3, 1, 1, 2, 8, 7, 4, 15, 4, 0, 0, 4, 4, 12, 4, 10];
    (idx != -1).then(|| if (1..=19).contains(&idx) { REC[idx as usize - 1] } else { 4 })
}

impl HeadClip {
    /// `GetTxtFileCommands` on a `TE` part: the clips of `(msg, part)` become `AnimFile`.
    pub fn part_read(&mut self, lists: &[ClipGroup], msg: u32, part: u32) {
        (self.anim, self.wait) = clip_names(lists, msg, part);
        self.sound = format!("{msg:04}_{part:02}.ogg");
        let face = lists.iter().find(|g| g.message_ids.contains(&msg)).and_then(|g| g.parts.iter().find(|p| p.part == part)).and_then(|p| p.face);
        self.face = face.map_or(0, |f| if (1..=19).contains(&f) { f } else { 1 });
    }

    fn open(&mut self, install: &Install, name: &str, looping: bool) -> bool {
        self.playback = Playback::open(install, name, Place::At(AT), looping, true);
        self.playback.is_some()
    }

    /// `StartAnim`: `AnimAkt = 0`; the story's race override `k` (`UzenetekRandomKitol`, 1..=8, 13, 14) picks the
    /// table's loop clip, else `AnimFile`.
    pub fn start(&mut self, install: &Install, table: &[(String, String)], k: i32) {
        self.playback = None;
        self.akt = 0;
        let name = if k > 0 { table.get(k as usize).map(|t| t.0.clone()).unwrap_or_default() } else { self.anim.clone() };
        self.open(install, &name, true);
    }

    /// `VegeAzElsoAnimnak`: from the speaking clip to the wait clip; none opens → the end clip. Returns whether the
    /// end clip started (its sounds are the caller's).
    pub fn wait(&mut self, install: &Install, table: &[(String, String)], k: i32, race: u8) -> bool {
        if self.akt == 0 {
            self.playback = None;
            self.akt = 1;
        }
        let name = if k > 0 { table.get(k as usize).map(|t| t.1.clone()).unwrap_or_default() } else { self.wait.clone() };
        !self.open(install, &name, true) && self.end(install, race)
    }

    /// `StartAnim_EndOfMessage`: once, `misc/end_<race>.bik` for races 1, 3, 5, else `misc/end_x.bik`. Returns
    /// whether it started (the caller plays `messages_end.wav` and ends the music's ducking).
    pub fn end(&mut self, install: &Install, race: u8) -> bool {
        if self.akt == 2 {
            return false;
        }
        self.playback = None;
        let name = if matches!(race, 1 | 3 | 5) { format!("misc/end_{race}.bik") } else { "misc/end_x.bik".to_string() };
        self.akt = 2;
        self.open(install, &name, false);
        true
    }

    /// `Messages_Kilep`'s `CloseBink`.
    pub fn close(&mut self) {
        self.playback = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `PlayHalkAlienHang`'s picks: the clip's name, the story's race override, `#n`. Observed (speech probe,
    /// 2026-10-23, `EN` 1): a pirate clip played `Mes_Hangok[7]`, `UzenetFaj` 17 `[15]`, a misc clip with race 5 `[6]`.
    #[test]
    fn alien_voices() {
        assert_eq!(alien_index("messages/pirate/p01.bik", 1, 0, 0), 10);
        assert_eq!(alien_index("messages/pirate/p01.bik", 2, 0, 0), -1);
        assert_eq!(alien_index("messages/trader/k01.bik", 5, 0, 0), 9);
        assert_eq!(alien_index("misc/race_04_loop.bik", 1, 5, 0), 4);
        assert_eq!(alien_index("misc/race_04_loop.bik", 4, 5, 0), -1);
        assert_eq!(alien_index("misc/race_04_loop.bik", 1, 0, 0), -1);
        assert_eq!(alien_index("messages/trader/k01.bik", 1, 0, 17), 17);
        let mes = |anim, k, face| alien_record(alien_index(anim, 1, k, face)).map(|r| 3 + r);
        assert_eq!([mes("messages/pirate/probe", 0, 0), mes("messages/trader/probe", 0, 17), mes("misc/probe", 5, 0)], [Some(7), Some(15), Some(6)]);
        assert_eq!([10, 9, 4, 17, 13, 20, 0].map(alien_record), [Some(4), Some(7), Some(3), Some(12), Some(0), Some(4), Some(4)]);
        assert_eq!(alien_record(-1), None);
    }

    #[test]
    fn names_from_the_list() {
        let lists = ig2_formats::dialogue::parse_clip_lists("5540, 5541\r\n01 !trader/k01_0-700 !trader/k01_700-1000 #9\r\n02 !misc/race_01_loop\r\n");
        assert_eq!(clip_names(&lists, 5541, 1), ("messages/trader/k01_0-700.bik".into(), "messages/trader/k01_700-1000.bik".into()));
        assert_eq!(clip_names(&lists, 5540, 2), ("misc/race_01_loop.bik".into(), "messages/".into()));
        assert_eq!(clip_names(&lists, 5540, 3), ("messages/".into(), "messages/".into()));
        assert_eq!(clip_names(&lists, 1, 1), ("messages/".into(), "messages/".into()));
    }

    /// The clip states: a missing wait clip falls through to the end clip, which plays once.
    #[test]
    fn states() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let lists = load_lists(&install, 1);
        let mut h = HeadClip::default();
        let g = lists.iter().find(|g| g.parts.iter().any(|p| p.loop_clip.is_none())).unwrap();
        let p = g.parts.iter().find(|p| p.loop_clip.is_none()).unwrap();
        h.part_read(&lists, g.message_ids[0], p.part);
        h.start(&install, &[], 0);
        assert_eq!(h.akt, 0);
        assert!(h.playback.is_some(), "{}", h.anim);
        h.wait(&install, &[], 0, 1);
        assert_eq!(h.akt, 2);
        assert!(h.playback.is_some());
        assert!(!h.end(&install, 1));
    }
}
