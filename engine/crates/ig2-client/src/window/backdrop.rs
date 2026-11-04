//! The game behind the overlay screens (crate::backdrop, re/screens/overlay-background.md).

use super::*;

impl App {
    /// `AktKep` of what the window shows, Options and Help over the rest.
    pub(super) fn shown_kep(&self) -> i32 {
        if self.options.is_some() {
            ig2_sim::tutorial::SCREEN_OPTIONS
        } else if self.help.is_some() {
            1
        } else if self.spy.is_some() && self.spacewar.is_none() {
            7
        } else {
            akt_kep(self.overlay, self.colony.is_some(), self.spacewar.is_some())
        }
    }

    /// The starmap's or the colony's scene as `GFX_Starmap` / `GFX_Colony` draw it with `KilepesElott` set (no top
    /// bar, no panels); none during a battle.
    pub(super) fn game_scene(&mut self) -> Option<crate::render::Frame> {
        if self.spacewar.is_some() {
            return None;
        }
        let now = self.now_ms();
        let alpha = self.world.draw_phase(self.clock.phase(now));
        let t = now as f32 / 1000.0;
        let g = self.gpu.as_mut()?;
        match (&mut self.colony, &self.world.colony_prev, &self.world.colony_curr) {
            (Some(c), Some(prev), Some(curr)) => Some(c.view.build(&mut g.renderer, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, alpha, t)),
            _ => self.view.as_mut().map(|v| v.build(&g.assets, &self.world.prev, &self.world.curr, alpha, t)),
        }
    }

    /// The screen shown changed: the pictures the screen left takes (`Starmap_Kilep` exe 0x5b2200, `Colony_Kilep`
    /// 0x466390, the overlays' `_Kilep`, `SpaceWar_Kilep` 0x562b50, `Messages_Belep` 0x507f90).
    pub(super) fn backdrop_switch(&mut self) {
        use crate::backdrop::{Pic, Source};
        let kep = self.shown_kep();
        let old = std::mem::replace(&mut self.backdrop.kep, kep);
        if old == kep || old < 0 {
            return;
        }
        let Some(size) = self.gpu.as_ref().map(|g| g.band()) else { return };
        let t = self.now_ms() as f32 / 1000.0;
        // Messages, help and options draw the screen before (`SaveScreen(2)`), the others the starmap (`SaveScreen(1)`).
        let before = matches!(kep, 1 | 10 | 0xd);
        match old {
            2 if kep != 11 => {
                if before {
                    let g = self.gpu.as_mut().unwrap();
                    self.backdrop.save_last(&mut g.renderer, Pic::Before, Source::Game, size);
                }
                if let Some(f) = self.game_scene() {
                    let g = self.gpu.as_mut().unwrap();
                    self.backdrop.save(&mut g.renderer, Pic::Starmap, Source::Game, &f, size, t);
                }
            }
            3 if before => {
                if let Some(f) = self.game_scene() {
                    let g = self.gpu.as_mut().unwrap();
                    self.backdrop.save(&mut g.renderer, Pic::Before, Source::Game, &f, size, t);
                }
            }
            4..=9 if before => {
                let g = self.gpu.as_mut().unwrap();
                self.backdrop.save_last(&mut g.renderer, Pic::Before, Source::Screen, size);
            }
            11 if kep == 0xd => {
                let g = self.gpu.as_mut().unwrap();
                self.backdrop.save_last(&mut g.renderer, Pic::Before, Source::Screen, size);
            }
            _ => {}
        }
    }

    /// `Copy3hatterkep`'s grey on the screen `kep` (crate::backdrop).
    pub(super) fn backdrop_grey(&self, kep: i32) -> f32 {
        use crate::backdrop as b;
        let h = self.hud.as_ref();
        match kep {
            1 => self.help.as_ref().map_or(1.0, |h| b::help(h.s.frame)),
            4 => self.research.as_ref().map_or(1.0, |r| b::research(r.frame)),
            5 => self.design.as_ref().map_or(1.0, |d| b::design(d.frame)),
            6 => self.diplomacy.as_ref().map_or(1.0, |d| b::diplomacy(d.screen.dipframe)),
            7 => self.spy.as_ref().map_or(1.0, |s| b::spy(s.screen.frame)),
            8 => self.trade.as_ref().map_or(1.0, |t| b::trade(t.frame)),
            9 => h.map_or(1.0, |h| b::info(h.info.frame)),
            10 => h.map_or(1.0, |h| b::messages(h.messages.frame)),
            0xd => self.options.as_ref().map_or(1.0, |o| b::options(o.s.fade_level())),
            _ => 1.0,
        }
    }

    /// What goes under the screen shown (`Copy3hatterkep`): its picture, or in live mode (ADR 0015) the starmap or
    /// colony drawn anew, unless the picture is of another overlay screen.
    pub(super) fn backdrop(&mut self) -> Option<(crate::render::TexId, f32)> {
        let kep = self.shown_kep();
        let pic = crate::backdrop::pic_of(kep)?;
        let grey = self.backdrop_grey(kep);
        let taken = self.backdrop.pic(pic);
        if self.backdrop.live && taken.is_none_or(|(_, s)| s == crate::backdrop::Source::Game) {
            if let Some(f) = self.game_scene() {
                let t = self.now_ms() as f32 / 1000.0;
                let g = self.gpu.as_mut()?;
                let size = g.band();
                return Some((self.backdrop.draw_live(&mut g.renderer, &f, size, t), grey));
            }
        }
        taken.map(|(tex, _)| (tex, grey))
    }
}
