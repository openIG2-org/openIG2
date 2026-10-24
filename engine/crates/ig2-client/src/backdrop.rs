//! The game behind the overlay screens (re/screens/overlay-background.md, ADR 0015).
//!
//! ig2.exe draws no starmap or colony under research, design, diplomacy, spy, trade, info, messages, options and
//! help: each screen's 3D update first lays a screenshot over the viewport (`Copy3hatterkep` exe 0x4cb690), dimmed
//! to about half as it opens. The screenshots are `SaveScreen(n)` (exe 0x524700, picture `0x33 + n`) taken by the
//! screen left: `Starmap_Kilep` takes picture 0x34 of the starmap without its HUD on every exit, picture 0x35 of the
//! screen with its HUD when it goes to messages, help or options; the colony and the other screens take 0x35 of
//! themselves on the way to those three. Classic mode keeps those frozen pictures; live mode (the default, ADR 0015)
//! draws the starmap or colony underneath anew every frame instead.

use crate::render::{Renderer, Target};

/// The two `SaveScreen` pictures the overlays draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pic {
    /// Picture 0x34 (`SaveScreen(1)`): research, design, diplomacy, spy, trade, info.
    Starmap,
    /// Picture 0x35 (`SaveScreen(2)`): messages, help, options.
    Before,
}

/// Where a picture came from: the starmap or colony (which live mode draws anew) or another overlay (kept as taken).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Game,
    Screen,
}

/// The pictures, the frame last put on the window, and the mode.
pub struct Backdrop {
    /// The "live background" option: the starmap or colony goes on running behind the screens.
    pub live: bool,
    pics: [Option<(Target, Source)>; 2],
    /// Live mode's target, drawn every frame an overlay is up.
    live_target: Option<Target>,
    /// The last frame drawn on the window, its time and its `AktKep`.
    pub last: Option<(crate::render::Frame, f32)>,
    pub kep: i32,
}

impl Backdrop {
    pub fn new(live: bool) -> Self {
        Self { live, pics: [None, None], live_target: None, last: None, kep: -1 }
    }

    /// `SaveScreen`: `frame` drawn into the picture (its target kept and resized to `size`).
    pub fn save(&mut self, r: &mut Renderer, pic: Pic, source: Source, frame: &crate::render::Frame, size: (u32, u32), t: f32) {
        let slot = &mut self.pics[pic as usize];
        let mut tg = match slot.take() {
            Some((tg, _)) => tg,
            None => r.target(size),
        };
        r.draw_into(frame, &mut tg, size, t);
        *slot = Some((tg, source));
    }

    /// `SaveScreen` of the last frame drawn on the window (the back buffer as the screen left it).
    pub fn save_last(&mut self, r: &mut Renderer, pic: Pic, source: Source, size: (u32, u32)) {
        if let Some((f, t)) = self.last.take() {
            self.save(r, pic, source, &f, size, t);
            self.last = Some((f, t));
        }
    }

    /// The picture a screen draws and where it came from: `None` before any was taken (`Copy3hatterkep` draws
    /// nothing: the loaded bit is clear).
    pub fn pic(&self, pic: Pic) -> Option<(crate::render::TexId, Source)> {
        self.pics[pic as usize].as_ref().map(|(tg, s)| (tg.tex, *s))
    }

    /// Live mode: the game's frame drawn into the live target, its texture.
    pub fn draw_live(&mut self, r: &mut Renderer, frame: &crate::render::Frame, size: (u32, u32), t: f32) -> crate::render::TexId {
        let tg = match &mut self.live_target {
            Some(tg) => tg,
            None => self.live_target.insert(r.target(size)),
        };
        r.draw_into(frame, tg, size, t);
        tg.tex
    }
}

/// `--snapshot --backdrop`: which picture `--screen` draws and its grey at the held frame (open, or `--anim-frame`,
/// `--info-star`).
pub fn snapshot_pic(o: &crate::Options) -> Option<(Pic, f32)> {
    let held = o.spy_frame;
    Some(match o.screen.as_deref()? {
        "research" => (Pic::Starmap, research(held.unwrap_or(60.0))),
        "design" => (Pic::Starmap, design(60.0)),
        "diplomacy" => (Pic::Starmap, diplomacy(held.unwrap_or(80.0))),
        "spy" => (Pic::Starmap, spy(held.unwrap_or(50.0))),
        "trade" => (Pic::Starmap, trade(50.0)),
        "info" => (Pic::Starmap, info(o.info_star.unwrap_or(45.0))),
        "messages" => (Pic::Before, messages(15.0)),
        _ => return None,
    })
}

/// The picture a screen (`AktKep`) draws under itself, if any.
pub fn pic_of(kep: i32) -> Option<Pic> {
    match kep {
        4..=9 => Some(Pic::Starmap),
        1 | 10 | 0xd => Some(Pic::Before),
        _ => None,
    }
}

/// `Copy3hatterkep`'s grey `0xFFgggggg` as `g / 255`, from `255 − c`.
fn grey(c: i32) -> f32 {
    (255 - c) as f32 / 255.0
}

/// Research (`Research3DUpdate` ELF 0x5e4e10): `c = (int)(resframe/60·128)`.
pub fn research(resframe: f32) -> f32 {
    grey((resframe / 60.0 * 128.0) as i32)
}

/// Design (`Design3DUpdate` exe 0x4ad8e0): `c = (int)(desframe/60·128)`.
pub fn design(desframe: f32) -> f32 {
    grey((desframe / 60.0 * 128.0) as i32)
}

/// Diplomacy (exe 0x4be2d0): `c = (int)(dipframe/80·128)`.
pub fn diplomacy(dipframe: f32) -> f32 {
    grey((dipframe / 80.0 * 128.0) as i32)
}

/// Spy (`Spying3DUpdate` exe 0x590410): `c = min(128, (int)(spyframe/50·128))`.
pub fn spy(spyframe: f32) -> f32 {
    grey(((spyframe / 50.0 * 128.0) as i32).min(128))
}

/// Trade (`Trade3DUpdate` exe 0x608340): `c = min(128, (int)(traframe/50·128))`.
pub fn trade(traframe: f32) -> f32 {
    grey(((traframe / 50.0 * 128.0) as i32).min(128))
}

/// Info (exe 0x4eb020): `c = (int)(infoframe/45·128)` (the star log's frames past 45 dim it further).
pub fn info(infoframe: f32) -> f32 {
    grey((infoframe / 45.0 * 128.0) as i32)
}

/// Messages (`Messages3DUpdate` ELF 0x5a3ad0): `c = (int)(195·min(mesframe, 15)/15)`.
pub fn messages(mesframe: f32) -> f32 {
    grey((195.0 * mesframe.min(15.0) / 15.0) as i32)
}

/// Options (`GFX_Options` exe 0x516f30): `c = min(128, (int)(F/100·128))`.
pub fn options(f: f32) -> f32 {
    grey(((f / 100.0 * 128.0) as i32).min(128))
}

/// Help (`GFX_Help` ELF 0x54f160): `c = min(128, (int)(helpframe/100·128))`.
pub fn help(helpframe: f32) -> f32 {
    grey(((helpframe / 100.0 * 128.0) as i32).min(128))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screens_dim_the_picture_to_about_half() {
        let g = |v: f32| (v * 255.0).round() as i32;
        assert_eq!((g(research(0.0)), g(research(60.0))), (255, 127));
        assert_eq!((g(design(30.0)), g(design(60.0))), (191, 127));
        assert_eq!((g(diplomacy(40.0)), g(diplomacy(80.0))), (191, 127));
        assert_eq!((g(spy(50.0)), g(spy(60.0))), (127, 127));
        assert_eq!((g(trade(25.0)), g(trade(50.0))), (191, 127));
        assert_eq!((g(info(45.0)), g(info(75.0))), (127, 42));
        assert_eq!((g(messages(7.5)), g(messages(30.0))), (158, 60));
        assert_eq!((g(options(50.0)), g(options(100.0))), (191, 127));
        assert_eq!((g(help(0.0)), g(help(100.0))), (255, 127));
    }

    #[test]
    fn each_screen_draws_its_picture() {
        assert_eq!((4..=9).map(pic_of).collect::<Vec<_>>(), vec![Some(Pic::Starmap); 6]);
        assert_eq!([1, 10, 0xd].map(pic_of), [Some(Pic::Before); 3]);
        assert_eq!([0, 2, 3, 11].map(pic_of), [None; 4]);
    }
}
