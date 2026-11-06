//! Ours (ADR 0062 onwards): when each modern page is up and what its acts of the last frame do.

use super::*;

impl App {
    /// No main menu, battle, spy screen, help or Options is up over the game's own screens (the modern pages of
    /// the starmap, colony and the overlay screens show only then).
    pub(super) fn nothing_over(&self) -> bool {
        self.front.is_none() && self.spacewar.is_none() && self.spy.is_none() && self.help.is_none() && self.options.is_none()
    }

    /// Ours (ADR 0065): the modern starmap HUD is up (its option on, the starmap shown with nothing over it).
    pub(super) fn modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.colony.is_none() && self.overlay.is_none() && self.view.is_some()
    }

    /// Ours (ADR 0070): the modern colony screen is up (its option on, the colony shown with nothing over it; the planet
    /// info mode's long info is its own panel, ADR 0074).
    pub(super) fn colony_modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.overlay.is_none() && self.world.colony_curr.is_some() && self.colony.is_some()
    }

    /// Ours (ADR 0078): the modern main menu's page is up (it sees the pointer; a press over its panel is its own).
    pub(super) fn main_menu_modern_up(&self) -> bool {
        self.options.is_none() && self.front.as_ref().is_some_and(|f| f.s.modern && f.mp.is_none() && f.mods.is_none() && f.s.panel().is_some())
    }

    /// Ours (ADR 0078): the help screen as the modern page (with the modern in-game menu).
    pub(super) fn help_modern_up(&self) -> bool {
        self.display.modern && self.help.is_some()
    }

    /// Ours (ADR 0078): the statistics screen as the modern page (with the modern main menu).
    pub(super) fn stats_modern_up(&self) -> bool {
        self.options.is_none() && self.front.as_ref().is_some_and(|f| f.s.modern && f.stats.is_some())
    }

    /// Ours (ADR 0078): the modern replays page is up.
    pub(super) fn replays_modern_up(&self) -> bool {
        self.options.is_none() && self.front.as_ref().is_some_and(|f| f.s.modern && f.replays.is_some())
    }

    /// Ours (ADR 0078): the modern new game page is up.
    pub(super) fn new_game_modern_up(&self) -> bool {
        self.options.is_none() && self.front.as_ref().is_some_and(|f| (f.s.new_game_up() || f.s.name_page_up()) && f.mp.is_none() && f.mods.is_none())
    }

    /// Ours (ADR 0072): the modern info screen is up (its option on, the info screen shown with nothing over it).
    pub(super) fn info_modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.overlay == Some(crate::ui::Screen::Info) && self.hud.is_some()
    }

    /// The modern info screen's acts of its last frame: the bar's as the starmap HUD's, the rest through the classic
    /// `Info_Main` code at the hot-spot the original's player would use (re/screens/info.md §5..§7).
    pub(super) fn info_modern_apply(&mut self) {
        use crate::info_modern::{Act, Press};
        use crate::info_screen::Buttons;
        if !self.info_modern_up() {
            self.info_acts.clear();
            return;
        }
        for a in std::mem::take(&mut self.info_acts) {
            if let Act::Bar(b) = a {
                self.bar_act(b);
                continue;
            }
            if let Act::Colony(p) = a {
                self.show_colony_screen(p);
                continue;
            }
            let Some(h) = &mut self.hud else { return };
            let (s, human) = (&self.world.ui, self.world.ui.human);
            let (d, ui) = (&s.info, &mut h.info);
            let drag = |p: Press| match p {
                Press::Left => Buttons { press: true, down: true, left: true, ..Default::default() },
                Press::Right => Buttons { press: true, down: true, right: true, ..Default::default() },
                Press::Drag => Buttons { down: true, left: true, ..Default::default() },
                Press::Release => Buttons { release: true, left: true, ..Default::default() },
            };
            let mut out = Vec::new();
            match a {
                Act::Empire(e) if ui.spot_on(e as u32, s) => ui.hover(Some(e as u32)),
                Act::Chart(c) if ui.spot_on(16 + c as u32, s) => ui.press(Some(16 + c as u32), d, human, 0.0),
                Act::StarLog(on) if ui.spot_on(if on { 15 } else { 14 }, s) => ui.press(Some(if on { 15 } else { 14 }), d, human, 0.0),
                // The entry's row among the four shown (`BolygoFlottaPointer` moved to it), pressed.
                Act::Open(j) if !ui.detail => {
                    ui.pointer = j.min(d.list(ui.selected, human).len().saturating_sub(4));
                    let id = 9 + (j - ui.pointer) as u32;
                    if ui.spot_on(id, s) {
                        ui.press(Some(id), d, human, 0.0);
                    }
                }
                Act::Back if ui.spot_on(37, s) => ui.press(Some(37), d, human, 0.0),
                // The menu's item under the pointer on the release (`infosmSelected`).
                Act::Setting(k, item) if ui.spot_on(k, s) => {
                    (ui.select, ui.select_item) = (Some(k), item);
                    out = ui.orders(Some(k), [0.0; 2], drag(Press::Release), d, human);
                }
                Act::Triangle(at, p) => out = ui.orders(Some(29), at, drag(p), d, human),
                Act::Limit(x, p) if ui.spot_on(32, s) => out = ui.orders(Some(32), [x, 210.0], drag(p), d, human),
                _ => {}
            }
            for c in out {
                self.world.send(c);
            }
        }
    }

    /// Ours (ADR 0076): the modern messages screen is up (its option on, the screen shown with nothing over it).
    pub(super) fn messages_modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.overlay == Some(crate::ui::Screen::Messages) && self.hud.is_some()
    }

    /// The modern messages screen's acts of its last frame: the bar's as the starmap HUD's, the rest as the original's
    /// presses at its hot-spots (re/screens/messages.md §3), an answer as its key.
    pub(super) fn messages_modern_apply(&mut self) {
        use crate::messages_modern::{self as mm, Act};
        if !self.messages_modern_up() {
            self.messages_acts.clear();
            return;
        }
        for a in std::mem::take(&mut self.messages_acts) {
            let at = match a {
                Act::Bar(b) => {
                    self.bar_act(b);
                    continue;
                }
                Act::Answer(k) => {
                    if let Some(h) = &mut self.hud {
                        h.messages.key(["a", "b", "c", "d"][k.min(3)]);
                    }
                    continue;
                }
                Act::Row(i) => {
                    let arrows = ig2_sim::game::inbox_arrows_allowed(&self.world.game.state);
                    let n = self.world.ui.inbox.len();
                    match self.hud.as_mut().and_then(|h| h.messages.reach(i, n, arrows)) {
                        Some(k) => mm::row_at(k),
                        None => continue,
                    }
                }
                Act::Hurry => mm::TEXT_AT,
                Act::Leave => mm::LEAVE_AT,
            };
            self.screen_press_at(crate::ui::Screen::Messages, at);
        }
    }

    /// Ours (ADR 0076): the modern diplomacy screen is up (its option on, the screen shown with nothing over it).
    pub(super) fn diplomacy_modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.overlay == Some(crate::ui::Screen::Diplomacy) && self.diplomacy.is_some() && self.hud.is_some()
    }

    /// Ours (ADR 0076): one of the modern diplomacy, spy or trade screens is up; their panels own the whole screen.
    pub(super) fn pages_modern_up(&self) -> bool {
        self.diplomacy_modern_up() || self.spy_modern_up() || self.trade_modern_up()
    }

    /// Ours (ADR 0076): the modern trade screen is up (its option on, the screen shown with nothing over it).
    pub(super) fn trade_modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.overlay == Some(crate::ui::Screen::Trade) && self.trade.is_some() && self.hud.is_some()
    }

    /// The modern trade screen's acts of its last frame: the bar's as the starmap HUD's, the rest through the
    /// original's hot-spots' code (`Trade::modern_*`); OK's order leaves with the screen's next step.
    pub(super) fn trade_modern_apply(&mut self) {
        use crate::trade_modern::Act;
        // Ours (ADR 0084): nothing more while the screen closes.
        if !self.trade_modern_up() || self.trade.as_ref().is_some_and(|t| t.modern.is_some_and(|m| m.closing())) {
            self.trade_acts.clear();
            return;
        }
        for a in std::mem::take(&mut self.trade_acts) {
            let state = &self.world.game.state;
            match (a, &mut self.trade) {
                (Act::Bar(b), _) => self.bar_act(b),
                (Act::Row(k, row), Some(tr)) => tr.modern_pick(state, k, row),
                (Act::Qty(more), Some(tr)) => tr.modern_qty(state, more),
                (Act::Dest(p), Some(tr)) => tr.modern_destination(state, p),
                (Act::Buy, Some(tr)) => tr.modern_buy(state),
                (Act::Click, Some(tr)) => tr.sounds.push(crate::audio::CLICK),
                _ => {}
            }
        }
    }

    /// Ours (ADR 0076): the modern spy screen is up (its option on, the screen shown with nothing over it).
    pub(super) fn spy_modern_up(&self) -> bool {
        self.display.modern && self.spy.is_some() && self.front.is_none() && self.spacewar.is_none() && self.help.is_none() && self.options.is_none() && self.hud.is_some()
    }

    /// The modern spy screen's acts of its last frame: the bar's as the starmap HUD's, the arrows as the keys, the
    /// rest as the original's presses (`SpyView::modern_press`, `modern_select`).
    pub(super) fn spy_modern_apply(&mut self) {
        use crate::spy_modern::Act;
        if !self.spy_modern_up() || self.spy.as_ref().is_some_and(|s| s.screen.modern.is_some_and(|m| m.closing())) {
            self.spy_acts.clear();
            return;
        }
        for a in std::mem::take(&mut self.spy_acts) {
            if let Act::Bar(b) = a {
                self.bar_act(b);
                continue;
            }
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
            let Some(spy) = &mut self.spy else { return };
            let orders = match a {
                Act::Key(left) => {
                    spy.key(&d, left);
                    Vec::new()
                }
                Act::Pedestal(i) => spy.modern_select(&d, i),
                Act::Click => {
                    spy.click();
                    Vec::new()
                }
                Act::Press(id) => spy.modern_press(&d, id, None),
                Act::Pick(id, v) => spy.modern_press(&d, id, Some(v)),
                Act::Bar(_) => Vec::new(),
            };
            drop(d);
            let errors = spy.take_errors();
            for c in orders {
                self.world.send(c);
            }
            if let Some(e) = errors.into_iter().last() {
                self.scroll_error(e);
            }
        }
    }

    /// The modern diplomacy screen's acts of its last frame: the bar's as the starmap HUD's, a contact and a menu
    /// path as the original's presses on them (`diplomacy::Open::select`, `pick`).
    pub(super) fn diplomacy_modern_apply(&mut self) {
        use crate::diplomacy_modern::Act;
        if !self.diplomacy_modern_up() || self.diplomacy.as_ref().is_some_and(|d| d.screen.modern.is_some_and(|m| m.closing())) {
            self.diplomacy_acts.clear();
            return;
        }
        for a in std::mem::take(&mut self.diplomacy_acts) {
            let cmds = match (a, &mut self.diplomacy, &self.hud) {
                (Act::Bar(b), ..) => {
                    self.bar_act(b);
                    continue;
                }
                (Act::Select(k), Some(d), Some(h)) => d.select(&self.world.game.state, h, self.tables, k),
                (Act::Pick(path), Some(d), Some(h)) => d.pick(&self.world.game.state, h, self.tables, &path),
                (Act::Click, Some(d), _) => {
                    d.screen.sounds.push(crate::audio::CLICK);
                    continue;
                }
                _ => continue,
            };
            for c in cmds {
                self.world.send(c);
            }
        }
    }

    /// Ours (ADR 0071): the modern research screen is up (its option on, the screen shown with nothing over it).
    pub(super) fn research_modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.overlay == Some(crate::ui::Screen::Research) && self.research.is_some()
    }

    /// Ours (ADR 0075): the modern design screen is up (its option on, the screen shown with nothing over it).
    pub(super) fn design_modern_up(&self) -> bool {
        self.display.modern && self.nothing_over() && self.overlay == Some(crate::ui::Screen::Design) && self.design.is_some()
    }

    /// Ours (ADR 0071): the modern production view is up (under the modern starmap HUD).
    pub(super) fn production_modern_up(&self) -> bool {
        self.modern_up() && self.panels.production_open()
    }

    /// The pointer is over one of the modern HUD's panels (its last frame's).
    pub(super) fn over_gui(&self) -> bool {
        (self.modern_up() || self.colony_modern_up() || self.info_modern_up() || self.messages_modern_up() || self.pages_modern_up() || self.research_modern_up() || self.design_modern_up()) && self.gpu.as_ref().is_some_and(|g| g.gui.ctx.is_pointer_over_area())
    }

    /// The pointer is over the modern colony's card (its last frame's).
    pub(super) fn over_colony_card(&self) -> bool {
        self.gpu.as_ref().is_some_and(|g| {
            let at = g.gui.ctx.input(|i| i.pointer.latest_pos());
            let card = g.gui.ctx.memory(|m| m.area_rect(egui::Id::new("cmh-card")));
            at.zip(card).is_some_and(|(p, r)| r.contains(p))
        })
    }

    /// The modern colony screen's acts of its last frame: the bar's as the starmap HUD's, the rest through the classic
    /// hot-spots' code ([`crate::colony_ui::ColonyUi::modern_act`]).
    pub(super) fn colony_modern_apply(&mut self) {
        let up = self.colony_modern_up();
        if let Some(c) = &mut self.colony {
            (c.ui.modern, c.ui.war.modern) = (up, up);
        }
        if !up {
            self.colony_acts.clear();
            // A radar zoom key held as the panels went: its camera flag lets go.
            if self.colony_modern.zoom.take().is_some()
                && let Some(c) = &mut self.colony
            {
                (c.ui.cam_keys.zoom_in, c.ui.cam_keys.zoom_out) = (false, false);
            }
            return;
        }
        for a in std::mem::take(&mut self.colony_acts) {
            match a {
                crate::colony_modern::Act::Bar(b) => self.bar_act(b),
                // The both-buttons rename on the card: its releases do nothing (`Colony_Main` exe 0x4667fb).
                crate::colony_modern::Act::Card(_) if self.menus.rename.is_some() => {}
                a => self.colony_input(|ui, c, _, now| ui.modern_act(c, a, now)),
            }
        }
    }

    /// The modern production view's acts of its last frame through the original's panel code: the spots by id, a held
    /// count button as the original's held hot-spot at the half it lowers or raises with, a design as the list's
    /// release, the planet list opened on the press for the release to pick.
    pub(super) fn production_modern_apply(&mut self) {
        use crate::production_modern::Act;
        let up = self.production_modern_up();
        self.panels.modern = self.modern_up();
        let sel = self.selection();
        if !up || self.starmap_locked() {
            self.production_acts.clear();
            if self.production_hold.take().is_some() {
                let acts = self.panels.release(&self.world.game.state, &sel);
                self.apply(acts);
            }
            return;
        }
        for a in std::mem::take(&mut self.production_acts) {
            let state = &self.world.game.state;
            match a {
                Act::Spot(0x32) => {
                    let at = self.virtual_cursor();
                    let dos = self.menus.open_menu(state, MenuKind::Planets, at);
                    self.apply_do(dos);
                    // The release is the menu's (`starmap_release`), not the panel's.
                    (self.gui_held, self.menus.held) = (0, true);
                }
                Act::Spot(id) => {
                    if let Some(acts) = self.panels.press_id(state, &sel, id, [0.0; 2], true) {
                        self.apply(acts);
                    }
                }
                Act::Pick(l, k, d) => {
                    let acts = self.panels.modern_pick(state, &sel, l, k, d);
                    self.apply(acts);
                }
                Act::Hold(h) if h != self.production_hold => {
                    if self.production_hold.take().is_some() {
                        let acts = self.panels.release(&self.world.game.state, &sel);
                        self.apply(acts);
                    }
                    if let Some((l, k, plus)) = h {
                        let (id, at) = crate::production_modern::count_spot(l, k, plus);
                        if let Some(acts) = self.panels.press_id(&self.world.game.state, &sel, id, at, true) {
                            self.apply(acts);
                        }
                    }
                    self.production_hold = h;
                }
                Act::Hold(_) => {}
            }
        }
    }

    /// The modern research screen's acts of its last frame: the bar's as the starmap HUD's, the rest through the
    /// screen's own code ([`crate::research::Research::modern_act`]).
    pub(super) fn research_modern_apply(&mut self) {
        if !self.research_modern_up() {
            self.research_acts.clear();
            return;
        }
        for a in std::mem::take(&mut self.research_acts) {
            match a {
                crate::research_modern::Act::Bar(b) => self.bar_act(b),
                a => {
                    let Some(r) = &mut self.research else { continue };
                    for o in r.modern_act(&self.world.game.state, a) {
                        let crate::research::Act::Send(c) = o;
                        self.world.send(c);
                    }
                }
            }
        }
    }

    /// The modern design screen's acts of its last frame: the bar's as the starmap HUD's, the rest through the
    /// screen's own hot-spot code ([`crate::design::Design::modern_act`]).
    pub(super) fn design_modern_apply(&mut self) {
        if !self.design_modern_up() {
            self.design_acts.clear();
            return;
        }
        for a in std::mem::take(&mut self.design_acts) {
            match a {
                crate::design_modern::Act::Bar(b) => self.bar_act(b),
                a => _ = self.design_do(|d, c, _| d.modern_act(c, a)),
            }
        }
    }

    /// A modern top bar's act (the starmap HUD's and the colony's): the classic bar's code.
    pub(super) fn bar_act(&mut self, a: crate::starmap_modern::Act) {
        use crate::starmap_modern::Act;
        let time = self.time_keys_allowed() || self.multiplayer();
        match a {
            Act::Pause if time => self.toggle_pause(),
            Act::Speed(n) if time => self.set_speed(n),
            Act::Messages => self.messages_from_bar(),
            Act::History(on) => {
                if let Some(h) = &mut self.hud {
                    h.history_open = on;
                }
            }
            Act::Screen(n) => self.top_menu_screen(n),
            Act::Start(t) => self.quick_research(t),
            Act::Stop(t) => self.world.send(Command::Research { order: ig2_net::ResearchCmd::Stop { tech: t } }),
            _ => {}
        }
    }

    /// The pointer is over the modern HUD's info card.
    pub(super) fn over_info_card(&self) -> bool {
        self.gpu.as_ref().is_some_and(|g| {
            let at = g.gui.ctx.input(|i| i.pointer.latest_pos());
            let card = g.gui.ctx.memory(|m| m.area_rect(egui::Id::new("smh-info")));
            at.zip(card).is_some_and(|(p, r)| r.contains(p))
        })
    }

    /// The modern HUD's acts of its last frame, through the original's hot-spots' code.
    pub(super) fn modern_apply(&mut self) {
        use crate::starmap_modern::Act;
        if !self.modern_up() {
            self.modern_acts.clear();
            if let Some((s, _)) = self.modern_hold.take() {
                self.console_let_go(s);
            }
            return;
        }
        for a in std::mem::take(&mut self.modern_acts) {
            let locked = self.starmap_locked();
            match a {
                Act::Pause | Act::Speed(_) | Act::Messages | Act::History(_) | Act::Screen(_) | Act::Start(_) | Act::Stop(_) => self.bar_act(a),
                Act::Hold(h) if h != self.modern_hold => {
                    if let Some((s, _)) = self.modern_hold.take() {
                        self.console_let_go(s);
                    }
                    if let Some((s, left)) = h {
                        self.console_act(s, left, [0.0; 2]);
                    }
                    self.modern_hold = h;
                }
                Act::View(s) => {
                    self.star_buttons.press(s);
                    self.console_view(s);
                }
                Act::Minimap(at, first) => {
                    if first {
                        self.console_act(7, true, at);
                        self.star_buttons.release();
                    } else {
                        let t = orders::minimap_target(at);
                        let zoom = self.camera3()[2];
                        self.tween_to([t[0], t[1], zoom], 4);
                    }
                }
                // Under the long info the card's left button and the production button leave it (hot-spots 8, 11).
                Act::Info(false) if self.menus.info_on() && !locked => self.leave_long_info(false),
                Act::Panel(11) if self.menus.info_on() && !locked => self.leave_long_info(true),
                Act::Panel(_) if self.menus.info_on() => {}
                Act::Info(right) if !locked && !self.menus.info_on() && self.menus.rename.is_none() && !self.wait_release => {
                    if right {
                        self.info_box_click();
                    } else {
                        self.info_box_left();
                    }
                }
                Act::Panel(id) if !locked => {
                    let sel = self.selection();
                    if let Some(acts) = self.panels.press_id(&self.world.game.state, &sel, id, [0.0; 2], true) {
                        self.apply(acts);
                        let acts = self.panels.release(&self.world.game.state, &sel);
                        self.apply(acts);
                    }
                }
                _ => {}
            }
        }
    }

    /// Ours (ADR 0077): the modern battle HUD is up (the battle shown with nothing over it, the modern HUD's option).
    pub(super) fn battle_modern_up(&self) -> bool {
        self.display.modern && self.spacewar.is_some() && self.front.is_none() && self.options.is_none() && self.help.is_none()
    }

    /// Ours (ADR 0077, 0074): the modern fleet info, vehicles' or transfer panel is up (under the modern starmap HUD).
    pub(super) fn fleet_modern_up(&self) -> bool {
        self.modern_up() && (self.panels.transfer.is_some() || self.panels.info.is_some())
    }
}
