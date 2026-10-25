//! Video playback (re/screens/video.md, ADR 0013): the 1999 code's `.bik` names played from the remaster's
//! `anim/mpeg4/*.mp4` in the user's install. A [`Player`] decodes on a thread of its own, a few frames ahead, and hands
//! out each frame when the wall clock reaches it, as the remaster's `videoentity` does; its sound goes through the
//! audio module's mixer (`Audio::play_video`) and does not time the picture.

pub mod credits;
pub mod decode;
pub mod message;
pub mod playback;

use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;

use crate::assets::Image;
use decode::{Clip, Frames};

/// `OpenBink`'s name → the remaster's file: `anim/mpeg4/` + the name with `\` → `/`, lower case, `.bik` → `.mp4`.
pub fn mp4_path(bik: &str) -> String {
    let n = bik.replace('\\', "/").to_ascii_lowercase();
    let n = n.trim_start_matches("anim/").trim_start_matches("mpeg4/");
    let stem = n.strip_suffix(".bik").or_else(|| n.strip_suffix(".mp4")).unwrap_or(n);
    format!("anim/mpeg4/{stem}.mp4")
}

/// A video the sim asks the front end for (re/screens/video.md §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// `st_ShowAnim(_s(6, id))`: a name of `s_anims.txt`.
    Anim(i32),
    /// `st_ShowAnim(file)`.
    File(String),
    /// `MEGHALAS`: the human's empire died.
    Lost,
    /// `MEGNYERES(true)`: no enemy left, or a story's win.
    Won,
    /// `MEGNYERES(false)`: story 863's end (it played its own videos): none.
    Ended,
}

impl Request {
    pub fn of(e: &ig2_sim::game::GameEvent) -> Option<Self> {
        use ig2_sim::game::{GameEvent, GameOver};
        use ig2_sim::story::StoryEvent;
        match e {
            GameEvent::Story(StoryEvent::Anim(id)) => Some(Self::Anim(*id)),
            GameEvent::Story(StoryEvent::AnimFile(f)) => Some(Self::File(f.to_string())),
            GameEvent::GameOver(GameOver::Lost) => Some(Self::Lost),
            GameEvent::GameOver(GameOver::Won) => Some(Self::Won),
            GameEvent::GameOver(GameOver::Ended) => Some(Self::Ended),
            _ => None,
        }
    }

    /// The `.bik` names `st_ShowAnim` plays for it, in order, by the human's race (`PlayerFaj[EN]`; the endings
    /// play only for a race < 9, re/rules/game-end.md §2): `MEGHALAS` 0x53b630 (a campaign: the race's loss clip,
    /// else `misc/altalanos_elvesztes.bik`; then `misc/end_gameover.bik`), `MEGNYERES(true)` 0x53f6b0 (a
    /// campaign: the race's win clip and `misc/end_campaign_win.bik`; else `misc/altalanos_megnyeres.bik` and
    /// `misc/end_victory.bik`), `MEGNYERES(false)`: none.
    pub fn names(&self, anims: &ig2_formats::strings::KeyedStrings, videos: &ig2_rules::Videos, race: usize, campaign: bool) -> Vec<String> {
        let pick = |t: &[String]| t.get(race).filter(|n| !n.is_empty()).cloned();
        let two = |a: Option<String>, b: &str| a.into_iter().chain([b.to_string()]).collect();
        match self {
            Self::Anim(id) => anims.get(6, *id as u32).map(str::to_string).into_iter().collect(),
            Self::File(f) => vec![f.clone()],
            Self::Lost | Self::Won | Self::Ended if race >= 9 => Vec::new(),
            Self::Lost if campaign => two(pick(&videos.loss), "misc/end_gameover.bik"),
            Self::Lost => two(Some("misc/altalanos_elvesztes.bik".into()), "misc/end_gameover.bik"),
            Self::Won if campaign => two(pick(&videos.win), "misc/end_campaign_win.bik"),
            Self::Won => two(Some("misc/altalanos_megnyeres.bik".into()), "misc/end_victory.bik"),
            Self::Ended => Vec::new(),
        }
    }

    /// `MEGHALAS` / `MEGNYERES`: the game ends (`ExitToMainScreen`, `AktKepKell = 0xF`) once its videos end.
    pub fn ends_game(&self) -> bool {
        matches!(self, Self::Lost | Self::Won | Self::Ended)
    }
}

/// `CampaignGameStart` exe 0x4f5e90's intro by the human's empire `EN` 1/3/5 (0x4f6008/1f/36), played through
/// `st_ShowAnim` (push it as [`Request::File`] onto `World::videos`).
pub fn intro_name(en: i32) -> Option<&'static str> {
    match en {
        1 => Some("intro_human.bik"),
        3 => Some("intro_politikus.bik"),
        5 => Some("intro_brutal.bik"),
        _ => None,
    }
}

/// `s_anims.txt` (`LoadStrTable(6, …)`): the story videos' names.
pub fn anim_names(install: &ig2_formats::Install) -> ig2_formats::strings::KeyedStrings {
    let mut k = ig2_formats::strings::KeyedStrings::default();
    if let Ok(b) = install.read("dataraw/text/s_anims.txt") {
        k.add_file(6, &String::from_utf8_lossy(&b));
    }
    k
}

/// Frames decoded ahead of the clock.
const AHEAD: usize = 4;

pub struct Player {
    clip: Arc<Clip>,
    /// The stream's frame rate (`ctx+0x38`: the track's timescale over its first sample's duration, 25).
    pub fps: f32,
    rx: Receiver<Image>,
    /// The next frame to hand out (display order, counted over the loops).
    next: usize,
    /// The frame received but not yet due.
    waiting: Option<Image>,
    /// The clock's origin (the caller's milliseconds at the first update).
    start_ms: Option<u64>,
    ended: bool,
}

impl Player {
    /// Opens `path` (an install path, see [`mp4_path`]) and starts decoding.
    pub fn open(install: &ig2_formats::Install, path: &str, looping: bool) -> Result<Self, String> {
        let data = install.read(path).map_err(|e| format!("{path}: {e}"))?;
        let clip = Arc::new(Clip::new(data).map_err(|e| format!("{path}: {e}"))?);
        let fps = clip.fps();
        let (tx, rx) = sync_channel(AHEAD);
        let c = clip.clone();
        std::thread::Builder::new()
            .name("video".into())
            .spawn(move || {
                loop {
                    let Ok(mut f) = Frames::new(c.clone()) else { return };
                    let mut any = false;
                    while let Some(img) = f.next_frame() {
                        any = true;
                        if tx.send(img).is_err() {
                            return;
                        }
                    }
                    if !looping || !any {
                        return;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { clip, fps, rx, next: 0, waiting: None, start_ms: None, ended: false })
    }

    /// The frame size.
    pub fn size(&self) -> (u32, u32) {
        self.clip.size()
    }

    /// The sound as a mixer stream at `rate` (`None` for the silent clips).
    pub fn sound(&self, rate: u32) -> Option<decode::Sound> {
        decode::Sound::new(self.clip.clone(), rate)
    }

    /// `videoentity::render` exe 0x70f9e0: the frame due at `now_ms`, `(int)((float)elapsed / (1000.0f / fps))`
    /// on the wall clock from the first call (the elapsed time as the exe's unsigned 32 bits through f64 to f32).
    fn target(&mut self, now_ms: u64) -> i64 {
        let start = *self.start_ms.get_or_insert(now_ms);
        let elapsed = (now_ms - start) as u32;
        if self.fps <= 0.0 {
            return -1;
        }
        ((elapsed as f64) as f32 / (1000.0f32 / self.fps)) as i64
    }

    /// Advances to `now_ms` (one drawn frame): takes the decoded frames while `target ≥ shown`, at most 10, and
    /// returns the last (the late ones are dropped); `None` when the shown one stays.
    pub fn update(&mut self, now_ms: u64) -> Option<Image> {
        if self.ended {
            return None;
        }
        let t = self.target(now_ms);
        let mut out = None;
        for _ in 0..10 {
            let img = match self.waiting.take() {
                Some(i) => i,
                None => match self.rx.try_recv() {
                    Ok(i) => i,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.ended = true;
                        break;
                    }
                },
            };
            if (self.next as i64) > t {
                self.waiting = Some(img);
                break;
            }
            self.next += 1;
            out = Some(img);
        }
        out
    }

    /// [`Self::update`] waiting for the decoder (captures on a script clock): every frame due by `now_ms`, the last
    /// returned.
    pub fn update_blocking(&mut self, now_ms: u64) -> Option<Image> {
        let t = self.target(now_ms);
        let mut out = None;
        while !self.ended && (self.next as i64) <= t {
            match self.waiting.take().or_else(|| self.rx.recv().ok()) {
                Some(img) => {
                    self.next += 1;
                    out = Some(img);
                }
                None => self.ended = true,
            }
        }
        out
    }

    /// The next frame in display order, waiting for the decoder (`None` at the end).
    pub fn next_blocking(&mut self) -> Option<Image> {
        let img = self.waiting.take().or_else(|| self.rx.recv().ok())?;
        self.next += 1;
        Some(img)
    }

    /// Every frame shown (or dropped) and the clip not looping.
    pub fn done(&self) -> bool {
        self.ended && self.waiting.is_none()
    }

    /// Frames handed out so far.
    #[cfg(test)]
    pub fn frame_index(&self) -> usize {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sim's requests resolve to files of the install: every story video of `s_anims.txt`, the endings.
    #[test]
    fn requests_resolve() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules = ig2_rules::Rules::from_install(&install).unwrap();
        let anims = anim_names(&install);
        let mut n = 0;
        for id in 0..10_000 {
            for name in Request::Anim(id).names(&anims, &rules.videos, 1, true) {
                assert!(install.resolve(&mp4_path(&name)).is_some(), "{name}");
                n += 1;
            }
        }
        assert_eq!(n, 57);
        for race in [1, 3, 5] {
            for r in [Request::Lost, Request::Won] {
                for campaign in [false, true] {
                    let names = r.names(&anims, &rules.videos, race, campaign);
                    assert_eq!(names.len(), 2);
                    assert!(names.iter().all(|n| install.resolve(&mp4_path(n)).is_some()), "{names:?}");
                }
            }
        }
        // Other races in a scenario: the generic pair; race 9+ and `MEGNYERES(false)`: nothing.
        assert_eq!(Request::Lost.names(&anims, &rules.videos, 2, false), ["misc/altalanos_elvesztes.bik", "misc/end_gameover.bik"]);
        assert!(Request::Won.names(&anims, &rules.videos, 9, false).is_empty());
        assert!(Request::Ended.names(&anims, &rules.videos, 1, true).is_empty());
    }

    #[test]
    fn bik_names_map_to_the_remaster_files() {
        assert_eq!(mp4_path("title.bik"), "anim/mpeg4/title.mp4");
        assert_eq!(mp4_path("misc\\race_01_loop.bik"), "anim/mpeg4/misc/race_01_loop.mp4");
        assert_eq!(mp4_path("anim\\Intro_Human.bik"), "anim/mpeg4/intro_human.mp4");
    }

    /// Without sound the frames come at their presentation times on the caller's clock; late ones are dropped.
    #[test]
    fn frames_follow_the_clock() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let mut p = Player::open(&install, "anim/mpeg4/misc/end_x.mp4", false).unwrap();
        let n = p.clip.video().samples.len();
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(p.update(1000).is_some()); // frame 0 at t 0
        assert!(p.update(1039).is_none());
        assert!(p.update(1040).is_some());
        assert_eq!(p.frame_index(), 2);
        let mut t = 1040;
        while !p.done() {
            t += 400;
            p.update(t);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(p.frame_index(), n);
    }
}
