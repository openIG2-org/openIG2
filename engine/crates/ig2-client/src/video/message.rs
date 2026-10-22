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
    pub playback: Option<Playback>,
}

impl HeadClip {
    /// `GetTxtFileCommands` on a `TE` part: the clips of `(msg, part)` become `AnimFile`.
    pub fn part_read(&mut self, lists: &[ClipGroup], msg: u32, part: u32) {
        (self.anim, self.wait) = clip_names(lists, msg, part);
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

    /// `VegeAzElsoAnimnak`: from the speaking clip to the wait clip; none opens → the end clip.
    pub fn wait(&mut self, install: &Install, table: &[(String, String)], k: i32, race: u8) {
        if self.akt == 0 {
            self.playback = None;
            self.akt = 1;
        }
        let name = if k > 0 { table.get(k as usize).map(|t| t.1.clone()).unwrap_or_default() } else { self.wait.clone() };
        if !self.open(install, &name, true) {
            self.end(install, race);
        }
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
