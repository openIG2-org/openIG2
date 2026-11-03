//! Ours (ADR 0062, 0064, the boards' layout ADR 0078): the multiplayer screens as designed egui pages over the main
//! menu's scene. The connection page: the game lists (LAN, internet, saved games) as tabs over a table of games with
//! status tags and empty states, a panel to join by address or join code with the password, the relay server's panel,
//! Back and Host. The room: the eight seats (colour, race head, race, holder, tags), the galaxy's settings with
//! steppers, the host's connection panel (join code, port mapping, private, public list), the chat, Leave and Ready /
//! Start. A frame of the page returns [`Act`]s that [`MpScreen::apply`] carries out with the same calls the old hot
//! spots made; the text fields stay ours (their keys come through [`MpScreen::key`], ADR 0062 §4).

use egui::{Color32, Pos2, Rect, Vec2};
use ig2_net::galaxy::{MAX_AIS, PLANETS};
use ig2_net::lobby::SlotKind;
use ig2_net::rendezvous::Rendezvous;

use super::{spot, words, Field, Focus, MpScreen, Out, PortMap, Role};
use crate::gui::ds::{self, Font, Text, Weight};
use crate::ui::UiTexts;

/// What a frame of the page asks for ([`MpScreen::apply`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// The old hot spot's press (`right`: the right button's).
    Spot(u32, bool),
    /// The host sets slot `i` to open, computer or closed.
    Slot(usize, SlotKind),
    /// Slot `i`'s race.
    Race(usize, u8),
    /// The computer in slot `i` plays at this level (1..3).
    Level(usize, u8),
    /// Esc: leave the room, then the screens.
    Back,
}

/// The boards' frame on the screen (points): the side margin (96 at 1920), the right column's width (584).
#[derive(Clone, Copy)]
struct Stage {
    r: Rect,
    m: f32,
    right: f32,
}

impl Stage {
    fn new(ctx: &egui::Context) -> Self {
        let r = ctx.screen_rect();
        Self { r, m: (r.width() * 0.05).clamp(16.0, 96.0), right: 584f32.min(r.width() * 0.34) }
    }

    /// The left column (the list, the seats) from `top` down to 120 points above the bottom.
    fn left(&self, top: f32) -> Rect {
        Rect::from_min_max(Pos2::new(self.r.left() + self.m, top), Pos2::new(self.r.right() - self.m - 40.0 - self.right, self.r.bottom() - 120.0))
    }

    /// The right column from `top`.
    fn right_col(&self, top: f32) -> Rect {
        Rect::from_min_max(Pos2::new(self.r.right() - self.m - self.right, top), Pos2::new(self.r.right() - self.m, self.r.bottom() - 120.0))
    }
}

/// The boards' small square tag (`.badge`: raised glass, a rim, 600 12 / 0.12 em upper case) in `c`.
fn tag(ui: &mut egui::Ui, text: &str, c: Color32) {
    let k = ds::tokens();
    let f = Font { size: 12.0, line: 14.0, weight: Weight::SemiBold, tracking: 0.12, upper: true, lcd: false };
    let fit = Text::new(text, f).layout(ui.ctx(), 220.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(fit.width + 20.0, 26.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, ds::RADIUS_PLATE, k.glass_raised);
    ds::rim(ui.painter(), rect, ds::RADIUS_PLATE, k.rim);
    Text::new(text, f).color(c).center().paint(ui, rect.shrink2(Vec2::new(6.0, 0.0)));
}

/// A section label (`.lbl`).
fn label(ui: &mut egui::Ui, text: &str) {
    ds::heading_label(ui, text);
}

/// A panel's title (`.ig-panel-title`: 600 20 / 0.08 em, as written: the boards' "Közvetlen csatlakozás").
fn title(ui: &mut egui::Ui, text: &str) {
    let f = Font::HEADING;
    Text::new(text, f).show(ui, Vec2::new(ui.available_width(), f.line));
}

/// A hint line (`.hint`: 400 14 / 20, muted), wrapped to `lines`.
fn hint(ui: &mut egui::Ui, text: &str, c: Color32, lines: usize) {
    let f = Font { size: 14.0, line: 20.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
    let fit = Text::new(text, f).color(c).lines(lines).layout(ui.ctx(), ui.available_width());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), fit.galley.size().y), egui::Sense::hover());
    ui.painter().galley(rect.min, fit.galley, Color32::WHITE);
}

/// One of our text fields (`.field input`: 44 high, recessed; its keys are ours): the text (dots for a password),
/// the caret while `focused`, scrolled to keep the caret in view; the hint muted while empty. A click focuses it.
#[allow(clippy::too_many_arguments)]
fn field(ui: &mut egui::Ui, f: &Field, lang: ig2_formats::text::Lang, focused: bool, masked: bool, hint: &str, width: f32, height: f32) -> egui::Response {
    let k = ds::tokens();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
    let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
    let p = ui.painter();
    p.rect_filled(rect, ds::RADIUS_PLATE, k.glass_deep);
    ds::rim(p, rect, ds::RADIUS_PLATE, k.rim.lerp_to_gamma(k.rim_bright, hover));
    ds::edge_line(p, rect, ds::RADIUS_PLATE as f32, true, Color32::from_black_alpha(200));
    if focused {
        ds::focus_mark(p, rect, ds::RADIUS_PLATE as f32, 2.0);
    }
    let inner = rect.shrink2(Vec2::new(14.0, 0.0));
    let font = Font { size: 17.0, line: 20.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
    let text = if masked { "•".repeat(f.text.len()) } else { ig2_formats::text::decode(&f.text, lang) };
    if text.is_empty() {
        Text::new(hint, font).color(k.ink_muted).paint(ui, inner);
    } else {
        let galley = ui.painter().layout_job(font.job(&text, k.ink));
        let caret = galley.pos_from_cursor(egui::text::CCursor::new(f.cursor.min(text.chars().count()))).min.x;
        let shift = (caret - inner.width() + 4.0).max(0.0);
        let at = Pos2::new(inner.left() - shift, inner.center().y - galley.size().y / 2.0);
        let painter = ui.painter_at(inner.expand2(Vec2::new(2.0, 0.0)));
        if focused && (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0 {
            let x = at.x + caret;
            painter.line_segment([Pos2::new(x, rect.top() + 11.0), Pos2::new(x, rect.bottom() - 11.0)], egui::Stroke::new(1.5f32, k.ink));
        }
        painter.galley(at, galley, k.ink);
    }
    if focused {
        ui.ctx().request_repaint();
    }
    resp
}

/// A panel at `rect` (an area of its own): its background behind `add`, `margin` inside.
#[allow(clippy::too_many_arguments)]
fn place<R>(ctx: &egui::Context, id: &str, rect: Rect, kind: ds::PanelKind, alpha: f32, margin: [f32; 2], opacity: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Area::new(egui::Id::new(id)).fixed_pos(rect.min).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        let at = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(margin[0] as i8, margin[1] as i8)).show(ui, |ui| {
            let inner = rect.size() - Vec2::new(2.0 * margin[0], 2.0 * margin[1]);
            ui.set_width(inner.x);
            if rect.height() > 0.0 {
                ui.set_min_height(inner.y);
                ui.set_max_height(inner.y);
            }
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
            add(ui)
        });
        ds::behind(ui, at, |p| ds::paint_panel(p, r.response.rect, kind, alpha));
        r.inner
    })
    .inner
}

/// Free content at `rect` (no panel).
fn bare<R>(ctx: &egui::Context, id: &str, rect: Rect, opacity: f32, layout: egui::Layout, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Area::new(egui::Id::new(id)).fixed_pos(rect.min).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        ui.allocate_ui_with_layout(rect.size(), layout, |ui| {
            ui.set_min_size(rect.size());
            add(ui)
        })
        .inner
    })
    .inner
}

/// The page title (34 / 40, 600, 0.3 em, silver) and what it shows under it in the info colour.
fn heading(ctx: &egui::Context, st: Stage, opacity: f32, text: &str, sub: &str) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(Pos2::new(st.r.left() + st.m + 16.0, st.r.top() + 112.0), Vec2::new(st.r.width() - 2.0 * st.m - 32.0, 68.0));
    bare(ctx, "mp-title", rect, opacity, egui::Layout::top_down(egui::Align::Min), |ui| {
        let f = Font { size: 34.0, line: 40.0, ..Font::PAGE_TITLE };
        Text::new(text, f).color(k.ink_silver).show(ui, Vec2::new(rect.width(), 40.0));
        ui.add_space(8.0);
        let sf = Font { size: 15.0, line: 20.0, weight: Weight::Medium, tracking: 0.1, upper: false, lcd: false };
        Text::new(sub, sf).color(k.info).show(ui, Vec2::new(rect.width(), 20.0));
    });
}

/// The tab well (`.tabs`: a recessed strip, 44-high plates, 500 17 / 0.12 em). Gives the clicked tab.
fn tab_well(ui: &mut egui::Ui, labels: &[String], selected: usize) -> Option<usize> {
    let k = ds::tokens();
    let f = Font { size: 17.0, line: 20.0, weight: Weight::Medium, tracking: 0.12, upper: false, lcd: false };
    let widths: Vec<f32> = labels.iter().map(|s| Text::new(s, f).layout(ui.ctx(), f32::INFINITY).width + 44.0).collect();
    let (whole, _) = ui.allocate_exact_size(Vec2::new(widths.iter().sum::<f32>() + 4.0 * (labels.len() as f32 + 1.0), 52.0), egui::Sense::hover());
    ui.painter().rect_filled(whole, ds::RADIUS_PANEL, k.glass_deep);
    ds::rim(ui.painter(), whole, ds::RADIUS_PANEL, k.rim);
    let mut x = whole.left() + 4.0;
    let mut out = None;
    for (i, (s, w)) in labels.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, whole.top() + 4.0), Vec2::new(*w, 44.0));
        let resp = ui.interact(r, ui.id().with(("mp-tab", i)), egui::Sense::click());
        let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
        if i == selected {
            ds::plate_fill(ui.painter(), r, ds::RADIUS_PLATE, false);
        } else {
            ds::rim(ui.painter(), r, ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hover));
        }
        let c = if i == selected { k.ink } else { k.ink_muted.lerp_to_gamma(k.ink_silver, hover) };
        Text::new(s, f).color(c).center().paint(ui, r);
        if resp.clicked() && i != selected {
            out = Some(i);
        }
        x += w + 4.0;
    }
    out
}

/// A table's cells: the name's column takes what the fixed ones leave (`.row`: 1fr 200 140 260, 16 apart, 16 in).
fn cells(r: Rect, fixed: &[f32]) -> Vec<Rect> {
    let inner = r.shrink2(Vec2::new(16.0, 0.0));
    let rest = inner.width() - fixed.iter().sum::<f32>() - 16.0 * fixed.len() as f32;
    let mut x = inner.left();
    std::iter::once(rest.max(40.0)).chain(fixed.iter().copied()).map(|w| {
        let c = Rect::from_x_y_ranges(x..=x + w, r.y_range());
        x += w + 16.0;
        c
    }).collect()
}

/// A row's look: selected (`.sel`: a white wash fading right, the silver rim), else the bright rim under the pointer.
fn row_look(ui: &egui::Ui, rect: Rect, on: bool, hover: f32) {
    let k = ds::tokens();
    if on {
        ds::hgrad(ui.painter(), rect, ds::RADIUS_PLATE, &[(0.0, Color32::from_white_alpha(36)), (1.0, Color32::from_white_alpha(5))]);
        ds::rim(ui.painter(), rect, ds::RADIUS_PLATE, k.ink_silver);
    } else {
        ds::rim(ui.painter(), rect, ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hover));
    }
}

/// The footer's plates at the bottom right (`bottom` points up from the screen's bottom), right to left; the status
/// line at their left. Gives the clicked plate's index.
fn footer(ctx: &egui::Context, st: Stage, bottom: f32, opacity: f32, plates: &[(String, f32, bool, bool)], status: &str, status_c: Color32) -> Option<usize> {
    let rect = Rect::from_min_max(Pos2::new(st.r.left() + st.m, st.r.bottom() - bottom - 48.0), Pos2::new(st.r.right() - st.m, st.r.bottom() - bottom));
    bare(ctx, "mp-footer", rect, opacity, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        let mut out = None;
        // Right to left: the last plate first.
        for (i, (s, w, primary, live)) in plates.iter().enumerate().rev() {
            if ui.add_enabled(*live, ds::Plate::new(s).width(*w).primary(*primary && *live)).clicked() {
                out = Some(i);
            }
        }
        let used = ui.min_rect().width();
        let f = Font { size: 15.0, line: 22.0, weight: Weight::Regular, tracking: 0.05, upper: false, lcd: false };
        Text::new(status, f).color(status_c).lines(2).paint(ui, rect.with_max_x(rect.right() - used - 24.0));
        out
    })
}

impl MpScreen {
    /// The race heads for the player rows (ADR 0041), read once from the install.
    pub fn load_heads(&mut self, ctx: &egui::Context, install: &ig2_formats::Install) {
        if !self.heads.is_empty() {
            return;
        }
        self.heads = (1..=8)
            .map(|k| {
                let p = install.resolve_texture("textures/main/fejek3/", &format!("{k}a.bmp"))?;
                let img = crate::assets::Image::load(install, &p).ok()?;
                let pixels = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
                Some(ctx.load_texture(format!("mp-race-{k}"), pixels, egui::TextureOptions::LINEAR))
            })
            .collect();
        // A mod's race wears its original's head (ADR 0069 §13).
        for a in crate::rules::added_names(install).0 {
            let head = self.heads.get(a.like.wrapping_sub(1)).cloned().flatten();
            if self.heads.len() < a.race {
                self.heads.resize(a.race, None);
            }
            self.heads[a.race - 1] = head;
        }
    }

    /// One frame of the page; `flow`: a right click is Back (ADR 0059).
    pub fn page(&mut self, ctx: &egui::Context, texts: &UiTexts, palette: &[[f32; 3]], theme: &crate::theme::Theme, flow: bool) -> Vec<Act> {
        let mut acts = Vec::new();
        let hot = crate::gui::ink(theme.hot);
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha(115));
        if self.state().is_some() {
            self.room(ctx, texts, palette, hot, &mut acts);
        } else {
            self.connection(ctx, texts, hot, &mut acts);
        }
        // Not while a drop-down is open: the click closes it.
        if flow && ctx.input(|i| i.pointer.secondary_clicked()) && !egui::Popup::is_any_open(ctx) {
            acts.push(Act::Back);
        }
        // Not before the page is open (the old boxes switched on at the fade's end).
        if self.open < 100.0 {
            acts.retain(|a| *a == Act::Back);
        }
        acts
    }

    /// The page's acts, in order.
    pub fn apply(&mut self, acts: Vec<Act>, net: &mut dyn super::Net) -> Vec<Out> {
        let mut outs = Vec::new();
        for a in acts {
            let o = match a {
                Act::Spot(id, right) => self.act(id, right, net),
                Act::Slot(i, kind) => self.set_slot(i, kind),
                Act::Race(i, race) => self.set_race(i, race),
                Act::Level(i, level) => self.set_level(i, level),
                Act::Back => self.key(None, true, net),
            };
            outs.extend(o);
        }
        outs
    }

    // -------------------------------------------------------------------------------------------- connection

    /// The connection page (the browser board): the title, the lists' panel, the join and relay panels, Back and Host.
    fn connection(&self, ctx: &egui::Context, texts: &UiTexts, hot: Color32, acts: &mut Vec<Act>) {
        let lang = texts.lang;
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, lang);
        let w = |b: &[u8]| words::text(lang, b);
        let k = ds::tokens();
        let st = Stage::new(ctx);
        let op = self.open / 100.0;
        heading(ctx, st, op, &s(texts.get(15, 1, if self.inet { 30 } else { 29 })), &format!("{} {}", w(words::PLAYING_AS), self.name));
        // The game lists.
        place(ctx, "mp-list", st.left(st.r.top() + 216.0), ds::PanelKind::Deep, 0.86, [24.0, 24.0], op, |ui| {
            let tab = match (&self.saves, &self.internet) {
                (Some(_), _) => 2,
                (None, Some(_)) => 1,
                _ => 0,
            };
            ui.horizontal(|ui| {
                let labels = [w(words::LAN), w(words::INTERNET), w(words::SAVED_GAMES)];
                if let Some(n) = tab_well(ui, &labels, tab) {
                    match (tab, n) {
                        (2, 1) => acts.extend([Act::Spot(spot::SAVED, false), Act::Spot(spot::BROWSE, false)]),
                        (2, _) => acts.push(Act::Spot(spot::SAVED, false)),
                        (_, 2) => acts.push(Act::Spot(spot::SAVED, false)),
                        _ => acts.push(Act::Spot(spot::BROWSE, false)),
                    }
                }
                if tab == 1 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.browsing.is_some() {
                            ui.spinner();
                        } else if ui.add(ds::Plate::new(&w(words::REFRESH)).width(180.0).font(ds::Plate::FONT.sized(17.0).tracking(0.12))).clicked() {
                            acts.push(Act::Spot(spot::REFRESH, false));
                        }
                    });
                }
            });
            ui.add_space(16.0);
            self.rows_list(ui, texts, acts);
        });
        // Join directly, the relay server.
        let right = st.right_col(st.r.top() + 216.0);
        let join = Rect::from_min_size(right.min, Vec2::new(right.width(), 0.0));
        let h = place(ctx, "mp-join", join, ds::PanelKind::Glass, 0.88, [24.0, 24.0], op, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let width = ui.available_width();
            title(ui, &w(words::JOIN_DIRECTLY));
            ui.add_space(8.0);
            label(ui, w(words::ADDRESS).trim_end_matches(':'));
            if field(ui, &self.address, lang, self.focus == Focus::Address, false, "192.168.0.10  /  IG2-…", width, 44.0).clicked() {
                acts.push(Act::Spot(spot::ADDRESS, false));
            }
            ui.add_space(8.0);
            label(ui, &w(words::PASSWORD));
            if field(ui, &self.password, lang, self.focus == Focus::Password, true, "", width, 44.0).clicked() {
                acts.push(Act::Spot(spot::PASSWORD, false));
            }
            ui.add_space(8.0);
            let saves = self.saves.is_some();
            let join_on = !saves && self.connecting.is_none() && (self.selected.is_some() || !self.address.text.is_empty());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.add_enabled(join_on, ds::Plate::new(&s(texts.get(16, 3, 3))).width(220.0)).clicked() {
                    acts.push(Act::Spot(spot::JOIN, false));
                }
            });
            ui.min_rect().height()
        });
        let relay = Rect::from_min_size(Pos2::new(right.left(), right.top() + h + 48.0 + 24.0), Vec2::new(right.width(), 0.0));
        place(ctx, "mp-relay", relay, ds::PanelKind::Glass, 0.88, [24.0, 24.0], op, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let width = ui.available_width();
            title(ui, &w(words::RELAY_TITLE));
            ui.add_space(8.0);
            if field(ui, &self.relay, lang, self.focus == Focus::Relay, false, "relay.example.org:port", width, 44.0).clicked() {
                acts.push(Act::Spot(spot::RELAY, false));
            }
            hint(ui, &w(words::RELAY_HINT), k.ink_muted, 4);
        });
        // Back, Host; the status: a refusal, a connect error, the host lost.
        let line = if self.lost && self.line.is_empty() { texts.get(14, 5, 3) } else { crate::tr::bytes(lang, &self.line) };
        let line = if self.connecting.is_some() && line.is_empty() { w(words::CONNECTING) } else { s(line) };
        let saves = self.saves.is_some();
        let host = if saves { w(words::HOST_SAVED) } else { w(words::HOST) };
        let plates = [(s(texts.get(15, 3, 7)), 200.0, false, true), (host, 300.0, true, !saves || self.selected.is_some())];
        match footer(ctx, st, 72.0, op, &plates, &line, hot) {
            Some(0) => acts.push(Act::Spot(spot::CANCEL, false)),
            Some(_) => acts.push(Act::Spot(spot::HOST, false)),
            None => {}
        }
    }

    /// The connection page's table: its rows (game, host, players, status), or what is missing.
    fn rows_list(&self, ui: &mut egui::Ui, texts: &UiTexts, acts: &mut Vec<Act>) {
        let lang = texts.lang;
        let w = |b: &[u8]| words::text(lang, b);
        let k = ds::tokens();
        if self.rows() == 0 {
            let (what, sub) = match (&self.saves, &self.internet) {
                (Some(_), _) => (words::NO_SAVES, words::NO_SAVES_HINT),
                (None, Some(_)) if self.browsing.is_some() => (words::ASKING, &b""[..]),
                (None, Some(_)) if self.relay.text.is_empty() => (words::NEED_RELAY, words::RELAY_HINT),
                (None, Some(_)) => (words::NO_PUBLIC, words::NO_PUBLIC_HINT),
                (None, None) if self.inet => (words::NONE_FOUND, words::INET),
                (None, None) => (words::NONE_FOUND, words::NONE_FOUND_HINT),
            };
            ui.add_space(120.0);
            let wd = ui.available_width();
            Text::new(&w(what), Font::HEADING).color(k.ink_silver).center().show(ui, Vec2::new(wd, 30.0));
            ui.add_space(8.0);
            let f = Font { size: 15.0, line: 22.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
            Text::new(&w(sub), f).color(k.ink_muted).center().show(ui, Vec2::new(wd, 22.0));
            return;
        }
        const COLS: [f32; 3] = [200.0, 140.0, 260.0];
        let (head, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), egui::Sense::hover());
        let heads = [crate::tr::text(lang, "Game").to_owned(), w(words::HOST_MARK), w(words::PLAYERS), crate::tr::text(lang, "Status").to_owned()];
        for (c, t) in cells(head, &COLS).into_iter().zip(&heads) {
            Text::new(t, Font::LABEL).color(k.ink_muted).paint(ui, c);
        }
        ui.add_space(4.0);
        // One row: the name, the host (or the scenario, the save's players), the players, the tags; a click selects, a
        // second click (or a double click) joins.
        let mut row = |ui: &mut egui::Ui, n: usize, name: String, host: String, players: String, tags: &[(&[u8], Color32)], off: bool| {
            let on = self.selected == Some(n);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), egui::Sense::click());
            row_look(ui, rect, on, ds::hover_t(ui.ctx(), resp.id, resp.hovered()));
            if on && self.reveal.take() {
                // Up and Down keep the selection in view.
                ui.scroll_to_rect(rect, None);
            }
            let c = cells(rect, &COLS);
            let body = Font { size: 17.0, line: 20.0, weight: Weight::Regular, tracking: 0.06, upper: false, lcd: false };
            Text::new(&name, body).color(if off { k.ink_muted } else { k.ink }).paint(ui, c[0]);
            Text::new(&host, body).color(k.info).paint(ui, c[1]);
            Text::new(&players, Font { size: 19.0, line: 22.0, ..Font::READOUT }).color(if off { k.ink_muted } else { k.ink }).paint(ui, c[2]);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(c[3]).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.spacing_mut().item_spacing.x = 8.0;
            for (t, col) in tags {
                tag(&mut child, &w(t), *col);
            }
            if resp.clicked() {
                acts.push(Act::Spot(spot::LIST + n as u32, false));
            }
            if resp.double_clicked() && !on {
                acts.push(Act::Spot(spot::LIST + n as u32, false));
            }
        };
        egui::ScrollArea::vertical().id_salt("mp-list").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            if let Some(saves) = &self.saves {
                for (n, sv) in saves.iter().take(super::LIST_ROWS).enumerate() {
                    let names: Vec<&str> = sv.save.session.humans.iter().map(|h| h.name.as_str()).collect();
                    row(ui, n, ig2_formats::text::decode(&sv.title, lang), names.join(", "), format!("{}", names.len()), &[], false);
                }
            } else if let Some(rooms) = &self.internet {
                for (n, (_, l)) in rooms.iter().take(super::LIST_ROWS).enumerate() {
                    let a = &l.announcement;
                    let other = a.version != ig2_net::lobby::LOBBY_VERSION || a.rules != self.rules || l.mods != self.mods;
                    let mut t: Vec<(&[u8], Color32)> = Vec::new();
                    if a.started {
                        t.push((words::STARTED, k.ink_silver));
                    }
                    if other {
                        t.push((words::INCOMPATIBLE, k.enemy));
                    }
                    if a.password {
                        t.push((words::LOCKED, k.ink_silver));
                    }
                    let sub: String = a.scenario.chars().take(24).collect();
                    row(ui, n, a.name.clone(), sub, format!("{}/{}", a.humans, a.slots), &t, other || a.started);
                }
            } else {
                for (n, f) in self.list.iter().take(super::LIST_ROWS).enumerate() {
                    let a = &f.announcement;
                    let other = a.rules != self.rules;
                    let mut t: Vec<(&[u8], Color32)> = Vec::new();
                    if a.started {
                        t.push((words::STARTED, k.ink_silver));
                    }
                    if other {
                        t.push((words::INCOMPATIBLE, k.enemy));
                    }
                    if a.password {
                        t.push((words::LOCKED, k.ink_silver));
                    }
                    row(ui, n, a.name.clone(), f.addr.to_string(), format!("{}/{}", a.humans, a.slots), &t, other || a.started);
                }
            }
        });
    }

    // -------------------------------------------------------------------------------------------- the room

    /// The room (the lobby room board): the title with the host's tag and the ready count, the seats, the galaxy's
    /// settings, the host's connection panel, the chat, Leave and Ready or Start.
    fn room(&self, ctx: &egui::Context, texts: &UiTexts, palette: &[[f32; 3]], hot: Color32, acts: &mut Vec<Act>) {
        let Some(st) = self.state() else { return };
        let lang = texts.lang;
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, lang);
        let w = |b: &[u8]| words::text(lang, b);
        let k = ds::tokens();
        let host = self.is_host();
        let me = self.member();
        let stage = Stage::new(ctx);
        let op = self.open / 100.0;
        // The title row: the room's name, its tags; the ready count at the right.
        let top = Rect::from_min_size(Pos2::new(stage.r.left() + stage.m + 16.0, stage.r.top() + 112.0), Vec2::new(stage.r.width() - 2.0 * stage.m - 32.0, 44.0));
        bare(ctx, "mp-title", top, op, egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 20.0;
            let humans = st.slots.iter().filter(|s| matches!(s.kind, SlotKind::Human(_))).count();
            let ready = st.members.iter().filter(|m| m.ready).count();
            let f = Font { size: 34.0, line: 40.0, ..Font::PAGE_TITLE };
            let room_w = (top.width() - 520.0).max(200.0);
            let fit = Text::new(&st.name, f).layout(ui.ctx(), room_w);
            Text::new(&st.name, f).color(k.ink_silver).show(ui, Vec2::new(fit.width.min(room_w) + 4.0, 44.0));
            ui.spacing_mut().item_spacing.x = 8.0;
            tag(ui, &w(if host { words::HOST_MARK } else { words::JOINED }), k.ink_silver);
            if st.private {
                tag(ui, &w(words::PRIVATE_BADGE), k.ink_silver);
            }
            if st.password {
                tag(ui, &w(words::LOCKED), k.ink_silver);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let lf = Font { size: 24.0, line: 28.0, ..Font::READOUT };
                Text::new(&format!("{ready}/{humans}"), lf).color(k.lcd_ink).show(ui, Vec2::new(64.0, 28.0));
                Text::new(&w(words::READY_COUNT), Font::LABEL).color(k.ink_muted).right().show(ui, Vec2::new(120.0, 18.0));
            });
        });
        // The seats.
        let left = stage.left(stage.r.top() + 196.0);
        let seats = Rect::from_min_size(left.min, Vec2::new(left.width(), 0.0));
        let h = place(ctx, "mp-seats", seats, ds::PanelKind::Deep, 0.86, [24.0, 20.0], op, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            title(ui, &w(words::PLAYERS));
            for (i, sl) in st.slots.iter().enumerate().take(8) {
                self.slot_row(ui, texts, palette, i, sl, st, me, acts);
            }
            ui.min_rect().height()
        });
        // The galaxy's settings: four a row, the setting's name over its stepper (the host's).
        let galaxy = Rect::from_min_size(Pos2::new(left.left(), left.top() + h + 40.0 + 40.0), Vec2::new(left.width(), 0.0));
        place(ctx, "mp-galaxy", galaxy, ds::PanelKind::Glass, 0.88, [24.0, 20.0], op, |ui| {
            match st.galaxy {
                Some(o) => {
                    let planets = s(texts.get(15, 2, 8));
                    let at = PLANETS.iter().position(|&p| p == o.planets).unwrap_or(1);
                    let rows: [(String, String, bool, bool); super::OPTIONS] = [
                        (planets, s(texts.get(14, 2, at as u16 + 1)), at > 0, at < 3),
                        (s(texts.get(14, 1, 1)), format!("{}", o.ais), o.ais > 0, o.ais < MAX_AIS),
                        (w(words::SHAPE), format!("{}", o.shape), o.shape > 1, o.shape < 2),
                        (w(words::BATTLES), w(words::BATTLE[o.battle_status.min(3) as usize]), o.battle_status > 0, o.battle_status < 3),
                        (w(words::SPEED), w(words::SPEED_BY[o.host_time as usize]), o.host_time, !o.host_time),
                        (w(words::FORMATION), w(words::FORMATION_BY[o.formation as usize]), o.formation, !o.formation),
                        (w(words::SPECTATORS), w(words::SPECTATORS_BY[o.spectators as usize]), o.spectators, !o.spectators),
                        (w(words::AI), w(words::AI_BY[o.enhanced_ai as usize]), o.enhanced_ai, !o.enhanced_ai),
                    ];
                    let cell = (ui.available_width() - 3.0 * 24.0) / 4.0;
                    for (r, four) in rows.chunks(4).enumerate() {
                        if r > 0 {
                            ui.add_space(16.0);
                        }
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 24.0;
                            for (c, (name, value, down, up)) in four.iter().enumerate() {
                                ui.allocate_ui_with_layout(Vec2::new(cell, 66.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                    ui.set_width(cell);
                                    ui.spacing_mut().item_spacing.y = 8.0;
                                    label(ui, name.trim_end_matches(':'));
                                    let step = ui.add_enabled_ui(host, |ui| ds::stepper(ui, value, cell - 64.0).1).inner;
                                    if (step < 0 && *down) || (step > 0 && *up) {
                                        acts.push(Act::Spot(spot::OPTION + 2 * (4 * r + c) as u32 + (step > 0) as u32, false));
                                    }
                                });
                            }
                        });
                    }
                }
                None => {
                    title(ui, &w(words::SAVED));
                    ui.add_space(8.0);
                    hint(ui, &w(words::NEED_SAVED), k.ink_muted, 2);
                }
            }
            // Ours (ADR 0068 §6): everyone in the room has the host's mods (a mismatch is refused).
            let mods = crate::mods_page::loaded();
            if !mods.is_empty() {
                ui.add_space(16.0);
                hint(ui, &format!("{} {}", crate::tr::text(lang, "Mods in this game:"), mods.join(", ")), k.ink_muted, 2);
            }
        });
        // The host's connection, the chat.
        let right = stage.right_col(stage.r.top() + 196.0);
        let mut chat_top = right.top();
        if let Some(Role::Host { listener, .. }) = &self.role {
            let h = place(ctx, "mp-conn", Rect::from_min_size(right.min, Vec2::new(right.width(), 0.0)), ds::PanelKind::Glass, 0.88, [24.0, 20.0], op, |ui| {
                ui.spacing_mut().item_spacing.y = 14.0;
                title(ui, &w(words::CONNECTION));
                match self.join_code() {
                    Some(c) => {
                        label(ui, crate::tr::text(lang, "Join code"));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let copy_w = 200.0;
                            let lw = ui.available_width() - copy_w - 12.0;
                            let (r, _) = ui.allocate_exact_size(Vec2::new(lw, 40.0), egui::Sense::hover());
                            ds::lcd_window(ui.painter(), r);
                            Text::new(&c.encode(), Font { size: 28.0, line: 32.0, ..Font::READOUT }).color(k.lcd_ink).center().shrink(18.0).paint(ui, r.shrink2(Vec2::new(8.0, 0.0)));
                            if ui.add(ds::Plate::new(&w(words::COPY)).width(copy_w).font(ds::Plate::FONT.sized(16.0).tracking(0.1))).clicked() {
                                acts.push(Act::Spot(spot::CODE, false));
                            }
                        });
                        hint(ui, &w(words::COPY_HINT), k.ink_muted, 2);
                    }
                    None => hint(ui, &w(words::NO_CODE), k.ink_muted, 2),
                }
                ui.painter().hline(ui.available_rect_before_wrap().x_range(), ui.cursor().top(), egui::Stroke::new(1.0f32, k.rim));
                ui.add_space(1.0);
                // A switch's row (`.opt`): the label, the toggle, the line under it.
                let (off_l, on_l) = (s(texts.get(13, 3, 2)), s(texts.get(13, 3, 1)));
                let opt = |ui: &mut egui::Ui, name: &str, sub: Option<(String, Color32)>, on: bool, live: bool| -> bool {
                    let mut v = on;
                    let (_, changed) = ds::option_row_in(ui, 36.0, Font { size: 16.0, line: 20.0, weight: Weight::Regular, tracking: 0.05, upper: false, lcd: false }, name, false, live, |ui| ds::toggle(ui, &mut v, &off_l, &on_l).changed());
                    if let Some((t, c)) = sub {
                        ui.add_space(-10.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            hint(ui, &t, c, 2);
                        });
                    }
                    changed
                };
                let (map, map_on) = match listener.port_map() {
                    PortMap::Off => (None, false),
                    PortMap::Asking => (Some((w(words::MAP_ASKING), k.ink_muted)), true),
                    PortMap::Mapped(a) => (Some((crate::tr::text(lang, "Port mapping: on, reachable at {}").replace("{}", &a.to_string()), k.own)), true),
                    PortMap::Failed(e) => (Some((crate::tr::text(lang, "Port mapping failed: {}").replace("{}", &e), k.enemy)), false),
                };
                if opt(ui, &w(words::MAP_LABEL), map, map_on, true) {
                    acts.push(Act::Spot(spot::PORT_MAP, false));
                }
                if opt(ui, &w(words::PRIVATE_LABEL), Some((w(words::PRIVATE_HINT), k.ink_muted)), st.private, true) {
                    acts.push(Act::Spot(spot::PRIVATE, false));
                }
                let relay = match listener.rendezvous() {
                    Rendezvous::Off => None,
                    Rendezvous::Asking => Some("Relay server: opening the room..."),
                    Rendezvous::Registered(_) => Some("Relay server: room open, seen at {}"),
                    Rendezvous::Refused(ig2_net::rendezvous::Refusal::Full) => Some("Relay server: full, try later"),
                    Rendezvous::Refused(_) => Some("Relay server: refused the room"),
                    Rendezvous::NoAnswer => Some("Relay server: no answer"),
                };
                if let Some(r) = relay {
                    let seen = match listener.rendezvous() {
                        Rendezvous::Registered(a) => a.to_string(),
                        _ => String::new(),
                    };
                    // A private room is never listed: its switch is off.
                    if opt(ui, &w(words::PUBLIC_LABEL), Some((crate::tr::text(lang, r).replace("{}", &seen), k.ink_muted)), self.public && !st.private, !st.private) {
                        acts.push(Act::Spot(spot::PUBLIC, false));
                    }
                }
                ui.min_rect().height()
            });
            chat_top += h + 40.0 + 24.0;
        }
        let chat = Rect::from_min_max(Pos2::new(right.left(), chat_top), right.max);
        if chat.height() > 120.0 {
            self.chat_panel(ctx, chat, lang, op);
        }
        // Leave, Ready or Start; the status: who isn't ready, two empires needed, a start error.
        let label = if host {
            if self.force { w(words::START_ANYWAY) } else { s(texts.get(14, 1, 12)) }
        } else {
            s(texts.get(14, 5, 6))
        };
        let ready = st.members.iter().find(|m| Some(m.id) == me).is_some_and(|m| m.ready);
        let label = if !host && ready { format!("✔ {label}") } else { label };
        let plates = [(s(texts.get(14, 1, 11)), 200.0, false, true), (label, 260.0, host || ready, true)];
        match footer(ctx, stage, 56.0, op, &plates, &s(crate::tr::bytes(lang, &self.line)), hot) {
            Some(0) => acts.push(Act::Spot(spot::LEAVE, false)),
            Some(_) => acts.push(Act::Spot(spot::READY, false)),
            None => {}
        }
    }

    /// A seat's row (`.seat`: 46 high; the empire's colour, the race's head, the race (a drop-down for the host and the
    /// holder), the holder (the host's drop-down: open, the computer at a level, closed), tags; Take seat while the
    /// pointer is on a free seat; your own seat lit.
    #[allow(clippy::too_many_arguments)]
    fn slot_row(&self, ui: &mut egui::Ui, texts: &UiTexts, palette: &[[f32; 3]], i: usize, sl: &ig2_net::lobby::Slot, st: &ig2_net::lobby::LobbyState, me: Option<u8>, acts: &mut Vec<Act>) {
        let lang = texts.lang;
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, lang);
        let w = |b: &[u8]| words::text(lang, b);
        let k = ds::tokens();
        let host = self.is_host();
        let own = me.is_some_and(|m| sl.kind == SlotKind::Human(m));
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), egui::Sense::hover());
        let hovered = ui.rect_contains_pointer(rect);
        row_look(ui, rect, own, ds::hover_t(ui.ctx(), resp.id, hovered) * 0.6);
        // ADR 0041: a stripe in the empire's colour (`ColonyKisInfo`'s owner colour 0xf + empire), the race's head.
        let stripe = Rect::from_min_size(rect.min, Vec2::new(6.0, rect.height()));
        if sl.kind != SlotKind::Closed {
            let c = palette.get(0xf + sl.empire as usize).copied().unwrap_or([1.0; 3]);
            ui.painter().rect_filled(stripe, egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }, crate::gui::ink(c));
        }
        let head = Rect::from_min_size(Pos2::new(rect.left() + 20.0, rect.center().y - 20.0), Vec2::splat(40.0));
        ui.painter().rect_filled(head, ds::RADIUS_PLATE, Color32::BLACK);
        if let Some(Some(t)) = self.heads.get((sl.race as usize).wrapping_sub(1)).filter(|_| sl.kind != SlotKind::Closed) {
            egui::Image::new(egui::load::SizedTexture::new(t.id(), head.size())).corner_radius(ds::RADIUS_PLATE).paint_at(ui, head);
        }
        ds::rim(ui.painter(), head, ds::RADIUS_PLATE, k.rim);
        let body = Font { size: 17.0, line: 20.0, weight: Weight::Regular, tracking: 0.06, upper: false, lcd: false };
        // The race: a drop-down (`.pick`, 190 × 38) for the host and the holder.
        let pick = Rect::from_min_size(Pos2::new(head.right() + 14.0, rect.center().y - 19.0), Vec2::new(190.0, 38.0));
        let race_name = |r: u8| s(texts.race(r));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pick).layout(egui::Layout::left_to_right(egui::Align::Center)));
        child.spacing_mut().interact_size.y = 38.0;
        if st.galaxy.is_some() {
            if (host || own) && sl.kind != SlotKind::Closed {
                let mut picked = None;
                egui::ComboBox::from_id_salt(("mp-race", i)).width(pick.width() - 8.0).truncate().selected_text(race_name(sl.race)).show_ui(&mut child, |ui| {
                    for r in texts.playable_races() {
                        if ui.add(egui::Button::selectable(r == sl.race, race_name(r))).clicked() {
                            picked = Some(r);
                        }
                    }
                });
                if let Some(r) = picked.filter(|&r| r != sl.race) {
                    acts.push(Act::Race(i, r));
                }
            } else if sl.kind != SlotKind::Closed {
                Text::new(&race_name(sl.race), body).color(k.ink_silver).paint(ui, pick.shrink2(Vec2::new(14.0, 0.0)));
            }
        } else if let Some(h) = self.resume.as_ref().and_then(|(sv, _)| sv.session.humans.iter().find(|h| h.empire == sl.empire)) {
            // A resumed save: whose empire it was.
            Text::new(&h.name, body).color(k.ink_muted).paint(ui, pick.shrink2(Vec2::new(14.0, 0.0)));
        }
        // The holder; ADR 0033: the computer's level (the skirmish menu's strings 15/4/1..3).
        let level = |l: u8| s(texts.get(15, 4, l as u16));
        let now = st.galaxy.map_or(2, |o| o.difficulty[(sl.empire as usize).clamp(1, 8) - 1].clamp(1, 3));
        let holder = |kind: SlotKind, l: u8| match kind {
            SlotKind::Ai if l != 0 => format!("{} · {}", w(words::COMPUTER), level(l)),
            SlotKind::Ai => w(words::COMPUTER),
            SlotKind::Closed => w(words::CLOSED),
            _ => w(words::OPEN_SEAT),
        };
        let now = if st.galaxy.is_some() { now } else { 0 };
        let tags_w = 300.0;
        let who = Rect::from_min_max(Pos2::new(pick.right() + 14.0, rect.top()), Pos2::new(rect.right() - 12.0 - tags_w, rect.bottom()));
        match sl.kind {
            SlotKind::Human(m) => {
                let name = st.members.iter().find(|x| x.id == m).map_or_else(String::new, |x| x.name.clone());
                Text::new(&name, body).color(k.ink).paint(ui, who);
            }
            kind if host => {
                let mut picked = None;
                let levels: &[u8] = if st.galaxy.is_some() { &[1, 2, 3] } else { &[0] };
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(who.shrink2(Vec2::new(0.0, 4.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
                child.spacing_mut().interact_size.y = 38.0;
                egui::ComboBox::from_id_salt(("mp-holder", i)).width((who.width() - 8.0).min(260.0)).truncate().selected_text(holder(kind, now)).show_ui(&mut child, |ui| {
                    if ui.add(egui::Button::selectable(kind == SlotKind::Open, holder(SlotKind::Open, 0))).clicked() {
                        picked = Some((SlotKind::Open, 0));
                    }
                    for &l in levels {
                        if ui.add(egui::Button::selectable(kind == SlotKind::Ai && l == now, holder(SlotKind::Ai, l))).clicked() {
                            picked = Some((SlotKind::Ai, l));
                        }
                    }
                    if ui.add(egui::Button::selectable(kind == SlotKind::Closed, holder(SlotKind::Closed, 0))).clicked() {
                        picked = Some((SlotKind::Closed, 0));
                    }
                });
                if let Some((n, l)) = picked {
                    if n != kind {
                        acts.push(Act::Slot(i, n));
                    }
                    if l != 0 && l != now {
                        acts.push(Act::Level(i, l));
                    }
                }
            }
            kind => {
                Text::new(&holder(kind, now), body).color(if kind == SlotKind::Ai { k.info } else { k.ink_muted }).paint(ui, who);
            }
        }
        // The tags at the right; Take seat on a free one under the pointer.
        let tags = Rect::from_min_max(Pos2::new(rect.right() - 12.0 - tags_w, rect.top()), Pos2::new(rect.right() - 12.0, rect.bottom()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tags).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = 8.0;
        let ready = match sl.kind {
            SlotKind::Human(m) => st.members.iter().find(|x| x.id == m).map(|x| x.ready),
            _ => None,
        };
        match (sl.kind, ready) {
            (_, Some(true)) => tag(&mut child, &s(texts.get(14, 5, 6)), k.own),
            (SlotKind::Human(0), _) => {}
            (_, Some(false)) => tag(&mut child, &w(words::NOT_READY), k.ink_muted),
            _ => {}
        }
        if sl.kind == SlotKind::Human(0) {
            tag(&mut child, &w(words::HOST_MARK), k.ink_silver);
        }
        if own {
            tag(&mut child, &w(words::YOU), k.ink_silver);
        }
        // Sit here: an open or computer seat, offered under the pointer.
        let free = matches!(sl.kind, SlotKind::Open | SlotKind::Ai) && !own;
        if free && hovered && child.add(ds::Plate::new(&w(words::TAKE_SEAT)).width(150.0).height(32.0).font(ds::Plate::FONT.sized(15.0).tracking(0.1))).clicked() {
            acts.push(Act::Spot(spot::HOLDER + i as u32, false));
        }
    }

    /// The chat panel: the lines (the newest at the bottom) and the line being typed.
    fn chat_panel(&self, ctx: &egui::Context, rect: Rect, lang: ig2_formats::text::Lang, op: f32) {
        let w = |b: &[u8]| words::text(lang, b);
        let k = ds::tokens();
        place(ctx, "mp-chat", rect, ds::PanelKind::Deep, 0.86, [24.0, 20.0], op, |ui| {
            title(ui, &w(words::CHAT));
            ui.add_space(10.0);
            let lines = (ui.available_height() - 42.0 - 10.0).max(22.0);
            let f = Font { size: 16.0, line: 22.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
            egui::ScrollArea::vertical().id_salt("mp-chat").max_height(lines).min_scrolled_height(lines).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for (who, line) in &self.chat {
                    let mut job = egui::text::LayoutJob::default();
                    let fmt = |c: Color32| egui::TextFormat { font_id: f.id(), extra_letter_spacing: f.spacing(), color: c, line_height: Some(f.line), ..Default::default() };
                    // A line of this machine's own (no speaker) shows bare.
                    if who.is_empty() {
                        job.append(line, 0.0, fmt(k.ink_muted));
                    } else {
                        job.append(&format!("{who}: "), 0.0, fmt(k.info));
                        job.append(line, 0.0, fmt(k.ink));
                    }
                    job.wrap.max_width = ui.available_width();
                    let g = ui.painter().layout_job(job);
                    let (r, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
                    ui.painter().galley(r.min, g, k.ink);
                }
            });
            ui.add_space(10.0);
            let width = ui.available_width();
            field(ui, &self.entry, lang, true, false, &w(words::CHAT_HINT), width, 42.0);
        });
    }
}
