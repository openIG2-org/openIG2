//! What `UpdateFrame` (exe 0x4e3e90) draws over a finished frame, last (re/screens/input.md §8): the hot-spot tooltip
//! (here the top bar's; the screens with their own keep theirs), then the game pointer, `MOUSE.BMP`'s 20 × 20 cell of
//! column `Mouse_Tipus` and row `(MouseFazis / 5) % 40`. Nothing while a screen changes (`Belep`/`Atlepes`).

use super::App;
use crate::render::Frame;

/// The colony screen's part of the overlay: its pointer (virtual pixels), tooltip (hot-spot id and text), pointer type
/// and phase.
struct ColonyOver {
    at: [f32; 2],
    tip: Option<(u32, Vec<u8>)>,
    kind: i32,
    phase: u64,
}

/// The tooltip and pointer of one frame, picked before the frame borrows the window ([`App::frame_overlay`]).
#[derive(Default)]
pub(super) struct Overlay {
    tip: Option<(crate::kit::Tooltip, Vec<u8>)>,
    /// The pointer's place (virtual pixels), type and phase (`MouseFazis`).
    pointer: Option<([i32; 2], i32, u64)>,
}

impl Overlay {
    /// The tooltip, then the pointer, over everything.
    pub fn draw(&self, f: &mut Frame, h: &crate::ui::Hud, size: [f32; 2]) {
        if let (Some((t, text)), Some(font)) = (&self.tip, &h.font) {
            t.draw(f, font, h.white, &crate::font::Virtual::new(size), &h.palette, text);
        }
        if let Some((at, kind, phase)) = self.pointer {
            h.draw_pointer_phase(f, size, at, kind, phase);
        }
    }
}

impl App {
    /// The starmap's tooltip at `at` (`crate::starmap_tip`).
    fn starmap_tip(&mut self, at: [f32; 2]) -> Option<crate::starmap_tip::StarmapTip> {
        let console = self.console_spot(at);
        let c = crate::starmap_tip::Ctx {
            texts: &self.hud.as_ref()?.texts,
            state: &self.world.game.state,
            sel: self.selection(),
            menus: &self.menus,
            panels: &self.panels,
            console,
            locked: self.tutorial.as_ref().is_some_and(|t| t.t.gates.starmap),
            human: self.world.curr.human,
        };
        crate::starmap_tip::tip(&c, at)
    }

    /// The starmap view's pointer type at `at` (`crate::orders::view_pointer`) when hot-spot 0x35 is the one under it:
    /// on without the long info, a panel or the tutorial's starmap lock, no lower spot under the pointer.
    fn starmap_view_pointer(&self, at: [f32; 2], tip: Option<u32>) -> Option<i32> {
        let locked = self.tutorial.as_ref().is_some_and(|t| t.t.gates.starmap);
        if locked || self.menus.info_on() || self.panels.info.is_some() || self.panels.transfer.is_some() || tip.is_some_and(|id| id != 0x35) {
            return None;
        }
        let v = self.view.as_ref()?;
        let sel = self.selection();
        let fleet_5c = self.world.game.state.fleets.get(sel.fleet).map_or(0, |f| i32::from_le_bytes([f[0x5c], f[0x5d], f[0x5e], f[0x5f]]));
        let c = crate::orders::ViewCtx { sel, production: self.panels.production_open(), redirect: self.menus.redirect, alt: self.modifiers.alt_key(), fleet_5c };
        crate::orders::view_pointer(&self.world.curr, &v.camera, at, &c)
    }

    /// The research screen's hand (`Research::hand`).
    fn research_hand(&self, at: [f32; 2]) -> bool {
        match (&self.research, &self.research_models) {
            (Some(r), Some(m)) => r.hand(&self.world.game.state, at, &m.centres),
            _ => false,
        }
    }

    /// The colony screen up (not under another screen).
    fn colony_shown(&self) -> bool {
        self.colony.is_some() && self.overlay.is_none() && self.spy.is_none() && self.help.is_none() && self.options.is_none()
    }

    /// The colony screen's tooltip, pointer type and phase at its pointer (`ColonyUi::tip`, `cursor`, `mouse_phase`).
    fn colony_overlay(&self) -> Option<ColonyOver> {
        let (Some(c), Some(h), Some(snap)) = (&self.colony, &self.hud, &self.world.colony_curr) else { return None };
        let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        let ctx = crate::colony_ui::Ctx { state: &self.world.game.state, snap, rules: self.tables, sim: self.world.game.rules, texts: &h.texts, tech_name: &|t| h.tech_name(t) };
        Some(ColonyOver { at, tip: c.ui.tip(&ctx, at), kind: c.ui.cursor(&ctx, at), phase: c.ui.mouse_phase().max(0) as u64 })
    }

    /// The pointer on the 640 × 480 virtual screen of the screen up.
    fn overlay_at(&self) -> [f32; 2] {
        match () {
            _ if self.front.is_some() => self.front.as_ref().map_or([0.0; 2], |f| f.at),
            _ if self.options.is_some() => self.options.as_ref().map_or([0.0; 2], |o| o.s.at),
            _ if self.help.is_some() => self.help.as_ref().map_or([0.0; 2], |h| h.s.at),
            _ => self.virtual_cursor(),
        }
    }

    /// Once a frame: the top bar's tooltip (`Mouse_Init`'s texts `GetString(18, 1, n)` on every screen below 0xe,
    /// `UpdateFrame`'s rest timer) and the pointer `UpdateFrame` draws: none while a screen changes, outside the window,
    /// or where the screen draws its own (the colony, the space battle, the tutorial's recording); the hand (1) over the
    /// bar's spots, the main menu's rings and boxes, research's rows and models and design's pods and models, the
    /// starmap view's types 2/5/6/7, else the arrow (0).
    pub(super) fn frame_overlay(&mut self) -> Overlay {
        // The game draws its own pointer on every screen: the window's is hidden.
        if !self.cursor_hidden
            && let Some(g) = &self.gpu
        {
            g.window.set_cursor_visible(false);
            self.cursor_hidden = true;
        }
        let now = self.start.elapsed().as_secs_f64() * 1000.0;
        let at = self.overlay_at();
        let game = self.front.is_none();
        let colony = self.colony_shown().then(|| self.colony_overlay()).flatten();
        let starmap_shown = game && self.colony.is_none() && self.overlay.is_none() && self.spy.is_none() && self.help.is_none() && self.options.is_none() && self.spacewar.is_none();
        let starmap = if starmap_shown && crate::hud::bar_tip_spot(at).is_none() { self.starmap_tip(at) } else { None };
        // The info screen's hot-spots (`Info_MouseInit`'s texts); a held one keeps its own (`Mouse_Megfogva`).
        let info = match (&self.hud, self.overlay) {
            (Some(h), Some(crate::ui::Screen::Info)) if game => {
                let id = h.info.held.or_else(|| h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui));
                id.map(|id| (id, h.info.tip_text(id, &h.texts, &h.empire_names)))
            }
            _ => None,
        };
        // The screen's own pointer type where no bar spot is under the pointer (`Mouse_Tipus`, −1: the spot's, 0).
        // Every screen's spots pass 1 (the hand), the starmap's 0x33, 0x34, 0x35, 0x58, 0x59, 100 and 0x6f 0
        // (`Starmap_MouseInit` exe 0x5b99d0); the screens' own writers come on top.
        let spot = |t: Option<&crate::kit::Tooltip>| t.is_some_and(|t| t.under != 0);
        let screen_kind = match self.overlay {
            _ if !game || crate::hud::bar_tip_spot(at).is_some() => 0,
            _ if self.options.is_some() => spot(self.options.as_ref().map(|o| &o.s.tip)) as i32,
            _ if self.help.is_some() => spot(self.help.as_ref().map(|h| &h.s.tip)) as i32,
            _ if self.spy.is_some() => spot(self.spy.as_ref().map(|s| &s.screen.tip)) as i32,
            Some(crate::ui::Screen::Research) => (spot(self.research.as_ref().map(|r| &r.tip)) || self.research_hand(at)) as i32,
            Some(crate::ui::Screen::Design) => self.design.as_ref().is_some_and(|d| d.tip.under != 0 || d.pods.hand) as i32,
            Some(crate::ui::Screen::Trade) => spot(self.trade.as_ref().map(|t| &t.tip)) as i32,
            Some(crate::ui::Screen::Info) => info.is_some() as i32,
            None if starmap_shown => match starmap.as_ref().map(|t| t.id) {
                Some(id) if ![0x33, 0x34, 0x35, 0x58, 0x59, 100, 0x6f].contains(&id) => 1,
                id => self.starmap_view_pointer(at, id).unwrap_or(0),
            },
            _ => 0,
        };
        let mut out = Overlay::default();
        if game && self.akt_kep_now() < 0xe {
            // `AktIkonKiir`: the bar's slots (2..=8) win over the screen's (id + 0x1e).
            let bar = crate::hud::bar_tip_spot(at);
            let (slot, text) = match (bar, &self.hud, &colony, &starmap) {
                (Some(n), Some(h), ..) => (n, h.texts.get(18, 1, n as u16 - 1)),
                (None, _, Some(ColonyOver { tip: Some((id, t)), .. }), _) => (id + 0x1e, t.clone()),
                (None, _, _, Some(t)) => (t.id + 0x1e, t.text.clone()),
                (None, ..) if info.is_some() => info.clone().map_or((0, Vec::new()), |(id, t)| (id + 0x1e, t)),
                _ => (0, Vec::new()),
            };
            self.bar_tip.moved(colony.as_ref().map_or(at, |c| c.at));
            let len = text.split(|&b| b == 0).next().map_or(0, <[u8]>::len);
            if starmap.as_ref().is_some_and(|t| t.now) {
                self.bar_tip.update_now(now, slot, len);
            } else {
                self.bar_tip.update(now, slot, len, self.mouse_held != 0);
            }
            if self.bar_tip.shown != 0 {
                out.tip = Some((self.bar_tip, text));
            }
        }
        let own = self.spacewar.is_some() || self.tutorial_owns_input();
        if self.pointer_in && !own && !self.screen_changing() {
            out.pointer = Some(match &colony {
                // The colony's own type (spec §11.10), but the hand over the bar.
                Some(c) => {
                    let kind = if crate::hud::bar_tip_spot(c.at).is_some() { 1 } else { c.kind };
                    ([c.at[0] as i32, c.at[1] as i32], kind, c.phase)
                }
                None => {
                    let kind = if !game {
                        self.front.as_ref().is_some_and(|f| f.s.hand) as i32
                    } else if crate::hud::bar_tip_spot(at).is_some() {
                        1
                    } else {
                        screen_kind
                    };
                    ([at[0] as i32, at[1] as i32], kind, now as u64 / 10)
                }
            });
        }
        out
    }
}
