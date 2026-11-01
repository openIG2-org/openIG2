//! Ours (ADR 0062, 0064): the multiplayer screens as designed egui pages. The connection page: the game lists (LAN,
//! internet, saved games) as tabs with rows, badges and empty states, a card to join by address or join code with
//! the password, the relay server's card, Host and Join. The room: the eight slots as player rows (colour, race head,
//! race, holder, badges), the galaxy's settings with steppers, the host's connection card (port mapping, private,
//! public list, join code), the chat, Leave and Ready / Start. A frame of the page returns [`Act`]s that
//! [`MpScreen::apply`] carries out with the same calls the old hot spots made; the text fields stay ours (their keys
//! come through [`MpScreen::key`], ADR 0062 §4).

use egui::{Color32, CornerRadius, Margin, RichText, Stroke, Vec2};
use ig2_net::galaxy::{MAX_AIS, PLANETS};
use ig2_net::lobby::SlotKind;
use ig2_net::rendezvous::Rendezvous;

use super::{spot, words, Field, Focus, MpScreen, Out, PortMap, Role};
use crate::ui::UiTexts;

/// The page's size in points (the screen is 540 high).
const PAGE: [f32; 2] = [760.0, 526.0];

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

/// A small rounded label in `c`.
fn badge(ui: &mut egui::Ui, text: &str, c: Color32) {
    egui::Frame::new()
        .fill(c.gamma_multiply(0.18))
        .stroke(Stroke::new(1.0f32, c.gamma_multiply(0.7)))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(7, 1))
        .show(ui, |ui| ui.label(RichText::new(text).small().strong().color(c)));
}

/// One of our text fields (its keys are ours): the text (stars for a password), the caret while `focused`, scrolled
/// to keep the caret in view. A click focuses it.
fn field(ui: &mut egui::Ui, f: &Field, lang: ig2_formats::text::Lang, focused: bool, masked: bool, hint: &str, width: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click());
    let v = ui.visuals();
    let w = ui.style().interact(&resp);
    let edge = if focused { Stroke::new(1.5f32, v.hyperlink_color) } else { w.bg_stroke };
    ui.painter().rect(rect, 8.0, v.extreme_bg_color, edge, egui::StrokeKind::Inside);
    let inner = rect.shrink2(egui::vec2(10.0, 0.0));
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text = if masked { "•".repeat(f.text.len()) } else { ig2_formats::text::decode(&f.text, lang) };
    let painter = ui.painter_at(inner);
    if text.is_empty() {
        painter.text(inner.left_center(), egui::Align2::LEFT_CENTER, hint, font, v.weak_text_color());
    } else {
        let galley = ui.painter().layout_no_wrap(text, font, v.text_color());
        let caret = galley.pos_from_cursor(egui::text::CCursor::new(f.cursor.min(f.text.len()))).min.x;
        let shift = (caret - inner.width() + 4.0).max(0.0);
        let at = egui::pos2(inner.left() - shift, inner.center().y - galley.size().y / 2.0);
        if focused && (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0 {
            let x = at.x + caret;
            painter.line_segment([egui::pos2(x, rect.top() + 7.0), egui::pos2(x, rect.bottom() - 7.0)], Stroke::new(1.5f32, v.text_color()));
        }
        painter.galley(at, galley, v.text_color());
    }
    if focused {
        ui.ctx().request_repaint();
    }
    resp
}

/// A stepper filling the width: ‹ value ›, the arrows off at the ends (none when not `live`); `Some(up)` when one
/// was clicked.
fn stepper(ui: &mut egui::Ui, value: &str, down: bool, up: bool, live: bool) -> Option<bool> {
    let mut out = None;
    ui.horizontal(|ui| {
        (ui.spacing_mut().item_spacing.x, ui.spacing_mut().button_padding.x) = (2.0, 2.0);
        let arrow = |ui: &mut egui::Ui, on: bool, s: &str| ui.add_enabled(on, egui::Button::new(RichText::new(s).strong()).min_size(egui::vec2(20.0, 20.0)));
        let side = if live { 24.0 } else { 0.0 };
        if live && arrow(ui, down, "‹").clicked() {
            out = Some(false);
        }
        let width = ui.available_width() - side;
        fixed(ui, width, egui::Label::new(RichText::new(value).strong().size(13.5)).truncate(), egui::Align::Center);
        if live && arrow(ui, up, "›").clicked() {
            out = Some(true);
        }
    });
    out
}

/// `widget` in a box `width` wide (a row's height), aligned `align` in it.
fn fixed(ui: &mut egui::Ui, width: f32, widget: impl egui::Widget, align: egui::Align) -> egui::Response {
    let layout = match align {
        egui::Align::Center => egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        _ => egui::Layout::left_to_right(egui::Align::Center),
    };
    ui.allocate_ui_with_layout(egui::vec2(width, ui.spacing().interact_size.y), layout, |ui| {
        ui.set_width(width);
        ui.add(widget)
    })
    .inner
}

/// A heading line: the title and what it shows.
fn heading(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.heading(RichText::new(title).strong());
    if !sub.is_empty() {
        ui.label(RichText::new(sub).weak());
    }
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
    }

    /// One frame of the page; `flow`: a right click is Back (ADR 0059).
    pub fn page(&mut self, ctx: &egui::Context, texts: &UiTexts, palette: &[[f32; 3]], theme: &crate::theme::Theme, flow: bool) -> Vec<Act> {
        let mut acts = Vec::new();
        let hot = crate::gui::ink(theme.hot);
        crate::gui::page(ctx, "multiplayer", Vec2::from(PAGE), |ui| {
            ui.set_opacity(self.open / 100.0);
            if self.state().is_some() {
                self.room(ui, texts, palette, hot, &mut acts);
            } else {
                self.connection(ui, texts, hot, &mut acts);
            }
        });
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

    fn connection(&self, ui: &mut egui::Ui, texts: &UiTexts, hot: Color32, acts: &mut Vec<Act>) {
        let lang = texts.lang;
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, lang);
        let w = |b: &[u8]| words::text(lang, b);
        let title = s(texts.get(15, 1, if self.inet { 30 } else { 29 }));
        heading(ui, &title, &format!("{} {}", w(words::PLAYING_AS), self.name));
        ui.add_space(8.0);
        let body = ui.available_height() - 48.0;
        ui.horizontal_top(|ui| {
            // The game lists.
            ui.vertical(|ui| {
                ui.set_width(430.0);
                ui.set_height(body);
                crate::gui::card(ui, None, |ui| {
                    ui.set_min_height(body - 22.0);
                    ui.horizontal(|ui| {
                        let tab = match (&self.saves, &self.internet) {
                            (Some(_), _) => 2,
                            (None, Some(_)) => 1,
                            _ => 0,
                        };
                        for (k, label) in [w(words::LAN), w(words::INTERNET), w(words::SAVED_GAMES)].into_iter().enumerate() {
                            if ui.add(egui::Button::selectable(tab == k, RichText::new(label).strong())).clicked() && tab != k {
                                match (tab, k) {
                                    (2, 1) => acts.extend([Act::Spot(spot::SAVED, false), Act::Spot(spot::BROWSE, false)]),
                                    (2, _) => acts.push(Act::Spot(spot::SAVED, false)),
                                    (_, 2) => acts.push(Act::Spot(spot::SAVED, false)),
                                    _ => acts.push(Act::Spot(spot::BROWSE, false)),
                                }
                            }
                        }
                        if tab == 1 {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.browsing.is_some() {
                                    ui.spinner();
                                } else if ui.button(w(words::REFRESH)).clicked() {
                                    acts.push(Act::Spot(spot::REFRESH, false));
                                }
                            });
                        }
                    });
                    ui.separator();
                    self.rows_list(ui, texts, acts);
                });
            });
            ui.add_space(4.0);
            // Join directly, the relay server.
            ui.vertical(|ui| {
                crate::gui::card(ui, Some(&w(words::JOIN_DIRECTLY)), |ui| {
                    let width = ui.available_width();
                    ui.label(RichText::new(w(words::ADDRESS).trim_end_matches(':')).small().weak());
                    if field(ui, &self.address, lang, self.focus == Focus::Address, false, "192.168.0.10  /  IG2-…", width).clicked() {
                        acts.push(Act::Spot(spot::ADDRESS, false));
                    }
                    ui.add_space(2.0);
                    ui.label(RichText::new(w(words::PASSWORD)).small().weak());
                    if field(ui, &self.password, lang, self.focus == Focus::Password, true, "", width).clicked() {
                        acts.push(Act::Spot(spot::PASSWORD, false));
                    }
                });
                ui.add_space(8.0);
                crate::gui::card(ui, Some(&w(words::RELAY_TITLE)), |ui| {
                    let width = ui.available_width();
                    if field(ui, &self.relay, lang, self.focus == Focus::Relay, false, "relay.example.org", width).clicked() {
                        acts.push(Act::Spot(spot::RELAY, false));
                    }
                    ui.add(egui::Label::new(RichText::new(w(words::RELAY_HINT)).small().weak()).wrap());
                });
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if crate::gui::secondary(ui, &s(texts.get(15, 3, 7))).clicked() {
                acts.push(Act::Spot(spot::CANCEL, false));
            }
            // The status: a refusal, a connect error, the host lost.
            let line = if self.lost && self.line.is_empty() { texts.get(14, 5, 3) } else { crate::tr::bytes(lang, &self.line) };
            if self.connecting.is_some() {
                ui.spinner();
            }
            ui.add(egui::Label::new(RichText::new(s(line)).color(hot)).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let saves = self.saves.is_some();
                let join_on = !saves && self.connecting.is_none() && (self.selected.is_some() || !self.address.text.is_empty());
                if !saves && crate::gui::primary(ui, join_on, &s(texts.get(16, 3, 3))).clicked() {
                    acts.push(Act::Spot(spot::JOIN, false));
                }
                let host = if saves { w(words::HOST_SAVED) } else { w(words::HOST) };
                if crate::gui::primary(ui, !saves || self.selected.is_some(), &host).clicked() {
                    acts.push(Act::Spot(spot::HOST, false));
                }
            });
        });
    }

    /// The connection page's list: its rows, or what is missing.
    fn rows_list(&self, ui: &mut egui::Ui, texts: &UiTexts, acts: &mut Vec<Act>) {
        let lang = texts.lang;
        let w = |b: &[u8]| words::text(lang, b);
        let (dim, accent) = (ui.visuals().weak_text_color(), ui.visuals().hyperlink_color);
        let warn = Color32::from_rgb(226, 160, 84);
        if self.rows() == 0 {
            let (what, hint) = match (&self.saves, &self.internet) {
                (Some(_), _) => (words::NO_SAVES, words::NO_SAVES_HINT),
                (None, Some(_)) if self.browsing.is_some() => (words::ASKING, &b""[..]),
                (None, Some(_)) if self.relay.text.is_empty() => (words::NEED_RELAY, words::RELAY_HINT),
                (None, Some(_)) => (words::NO_PUBLIC, words::NO_PUBLIC_HINT),
                (None, None) if self.inet => (words::NONE_FOUND, words::INET),
                (None, None) => (words::NONE_FOUND, words::NONE_FOUND_HINT),
            };
            ui.add_space(70.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(w(what)).size(17.0));
                ui.add(egui::Label::new(RichText::new(w(hint)).small().color(dim)).wrap());
            });
            return;
        }
        // One row: the name, its badges, the players; a click selects, a second click (or a double click) joins.
        let mut row = |ui: &mut egui::Ui, k: usize, name: String, sub: String, players: String, badges: &[(&[u8], Color32)], off: bool| {
            let on = self.selected == Some(k);
            let r = egui::Frame::new().corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new(name).strong().color(if off { dim } else { ui.visuals().text_color() }));
                        if !sub.is_empty() {
                            ui.label(RichText::new(sub).small().color(dim));
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(players).strong());
                        for (b, c) in badges {
                            badge(ui, &w(b), *c);
                        }
                    });
                });
            });
            let resp = ui.interact(r.response.rect, egui::Id::new(("mp-row", k)), egui::Sense::click());
            let fill = if on { Some(accent.gamma_multiply(0.22)) } else if resp.hovered() { Some(accent.gamma_multiply(0.10)) } else { None };
            if let Some(fill) = fill {
                ui.painter().rect_filled(r.response.rect, 8.0, fill);
            }
            if on {
                ui.painter().rect_stroke(r.response.rect, 8.0, Stroke::new(1.0f32, accent), egui::StrokeKind::Inside);
                // Up and Down keep the selection in view.
                if self.reveal.take() {
                    ui.scroll_to_rect(r.response.rect, None);
                }
            }
            if resp.clicked() {
                acts.push(Act::Spot(spot::LIST + k as u32, false));
            }
            if resp.double_clicked() && !on {
                acts.push(Act::Spot(spot::LIST + k as u32, false));
            }
        };
        egui::ScrollArea::vertical().id_salt("mp-list").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if let Some(saves) = &self.saves {
                for (k, sv) in saves.iter().take(super::LIST_ROWS).enumerate() {
                    let names: Vec<&str> = sv.save.session.humans.iter().map(|h| h.name.as_str()).collect();
                    row(ui, k, ig2_formats::text::decode(&sv.title, lang), names.join(", "), format!("{}", names.len()), &[], false);
                }
            } else if let Some(rooms) = &self.internet {
                for (k, (_, l)) in rooms.iter().take(super::LIST_ROWS).enumerate() {
                    let a = &l.announcement;
                    let other = a.version != ig2_net::lobby::LOBBY_VERSION || a.rules != self.rules || l.mods != self.mods;
                    let mut b: Vec<(&[u8], Color32)> = Vec::new();
                    if a.started {
                        b.push((words::STARTED, dim));
                    }
                    if other {
                        b.push((words::INCOMPATIBLE, warn));
                    }
                    if a.password {
                        b.push((words::LOCKED, accent));
                    }
                    let sub: String = a.scenario.chars().take(24).collect();
                    row(ui, k, a.name.clone(), sub, format!("{}/{}", a.humans, a.slots), &b, other || a.started);
                }
            } else {
                for (k, f) in self.list.iter().take(super::LIST_ROWS).enumerate() {
                    let a = &f.announcement;
                    let other = a.rules != self.rules;
                    let mut b: Vec<(&[u8], Color32)> = Vec::new();
                    if a.started {
                        b.push((words::STARTED, dim));
                    }
                    if other {
                        b.push((words::INCOMPATIBLE, warn));
                    }
                    if a.password {
                        b.push((words::LOCKED, accent));
                    }
                    row(ui, k, a.name.clone(), f.addr.to_string(), format!("{}/{}", a.humans, a.slots), &b, other || a.started);
                }
            }
        });
    }

    // -------------------------------------------------------------------------------------------- the room

    fn room(&self, ui: &mut egui::Ui, texts: &UiTexts, palette: &[[f32; 3]], hot: Color32, acts: &mut Vec<Act>) {
        let Some(st) = self.state() else { return };
        let lang = texts.lang;
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, lang);
        let w = |b: &[u8]| words::text(lang, b);
        let host = self.is_host();
        let me = self.member();
        let accent = ui.visuals().hyperlink_color;
        let dim = ui.visuals().weak_text_color();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&st.name).size(22.0).strong());
            ui.add_space(4.0);
            badge(ui, &w(if host { words::HOST_MARK } else { words::JOINED }), accent);
            if st.private {
                badge(ui, &w(words::PRIVATE_BADGE), dim);
            }
            if st.password {
                badge(ui, &w(words::LOCKED), dim);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let humans = st.slots.iter().filter(|s| matches!(s.kind, SlotKind::Human(_))).count();
                let ready = st.members.iter().filter(|m| m.ready).count();
                ui.label(RichText::new(format!("{} {ready}/{humans}", w(words::READY_COUNT))).weak());
            });
        });
        ui.add_space(4.0);
        let body = ui.available_height() - 46.0;
        let left = (ui.available_width() * 0.64).min(440.0);
        ui.horizontal_top(|ui| {
            // The players and the galaxy.
            ui.vertical(|ui| {
                ui.set_width(left);
                ui.set_height(body);
                ui.spacing_mut().item_spacing.y = 6.0;
                crate::gui::card(ui, Some(&w(words::PLAYERS)), |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, sl) in st.slots.iter().enumerate().take(8) {
                        self.slot_row(ui, texts, palette, i, sl, st, me, acts);
                    }
                });
                if let Some(o) = st.galaxy {
                    crate::gui::card(ui, None, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
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
                        // Three a row: the setting's name over its value (the host's stepper).
                        let cell = (ui.available_width() - 2.0 * 12.0) / 3.0;
                        for (r, three) in rows.chunks(3).enumerate() {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 12.0;
                                for (c, (label, value, down, up)) in three.iter().enumerate() {
                                    ui.allocate_ui_with_layout(egui::vec2(cell, 34.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                        ui.set_width(cell);
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        ui.add(egui::Label::new(RichText::new(label.trim_end_matches(':')).small().weak()).truncate());
                                        if let Some(up) = stepper(ui, value, host && *down, host && *up, host) {
                                            acts.push(Act::Spot(spot::OPTION + 2 * (3 * r + c) as u32 + up as u32, false));
                                        }
                                    });
                                }
                            });
                        }
                    });
                } else {
                    crate::gui::card(ui, Some(&w(words::SAVED)), |ui| {
                        ui.add(egui::Label::new(RichText::new(w(words::NEED_SAVED)).small().weak()).wrap());
                    });
                }
            });
            // The host's connection, the chat.
            ui.vertical(|ui| {
                ui.set_height(body);
                ui.spacing_mut().item_spacing.y = 6.0;
                if let Some(Role::Host { listener, .. }) = &self.role {
                    crate::gui::card(ui, Some(&w(words::CONNECTION)), |ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        let toggle = |ui: &mut egui::Ui, label: &str, sub: Option<String>, on: bool, live: bool| -> bool {
                            let mut v = on;
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(ui.available_width() - 44.0);
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.add_enabled(live, egui::Label::new(RichText::new(label).size(13.5)).wrap());
                                    if let Some(sub) = sub {
                                        ui.add(egui::Label::new(RichText::new(sub).small().weak()).wrap());
                                    }
                                });
                                ui.add_enabled_ui(live, |ui| crate::gui::switch(ui, &mut v));
                            });
                            v != on
                        };
                        let (map, map_on) = match listener.port_map() {
                            PortMap::Off => (None, false),
                            PortMap::Asking => (Some(w(words::MAP_ASKING)), true),
                            PortMap::Mapped(a) => (Some(crate::tr::text(lang, "Port mapping: on, reachable at {}").replace("{}", &a.to_string())), true),
                            PortMap::Failed(e) => (Some(crate::tr::text(lang, "Port mapping failed: {}").replace("{}", &e)), false),
                        };
                        if toggle(ui, &w(words::MAP_LABEL), map, map_on, true) {
                            acts.push(Act::Spot(spot::PORT_MAP, false));
                        }
                        if toggle(ui, &w(words::PRIVATE_LABEL), Some(w(words::PRIVATE_HINT)), st.private, true) {
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
                            if toggle(ui, &w(words::PUBLIC_LABEL), Some(crate::tr::text(lang, r).replace("{}", &seen)), self.public && !st.private, !st.private) {
                                acts.push(Act::Spot(spot::PUBLIC, false));
                            }
                        }
                        match self.join_code() {
                            Some(c) => {
                                let r = ui.add(egui::Button::new(w(words::COPY)).min_size(egui::vec2(ui.available_width(), 26.0)));
                                if r.on_hover_text(format!("{}\n{}", c.encode(), w(words::COPY_HINT))).clicked() {
                                    acts.push(Act::Spot(spot::CODE, false));
                                }
                            }
                            None => {
                                ui.add(egui::Label::new(RichText::new(w(words::NO_CODE)).small().weak()).wrap());
                            }
                        }
                    });
                }
                self.chat_card(ui, lang, ui.available_height());
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if crate::gui::secondary(ui, &s(texts.get(14, 1, 11))).clicked() {
                acts.push(Act::Spot(spot::LEAVE, false));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if host {
                    if self.force { w(words::START_ANYWAY) } else { s(texts.get(14, 1, 12)) }
                } else {
                    s(texts.get(14, 5, 6))
                };
                let ready = st.members.iter().find(|m| Some(m.id) == me).is_some_and(|m| m.ready);
                let r = if !host && ready {
                    ui.add(egui::Button::new(RichText::new(format!("✔ {label}")).strong()).fill(ui.visuals().selection.bg_fill).min_size(egui::vec2(120.0, 32.0)))
                } else {
                    crate::gui::primary(ui, true, &label)
                };
                if r.clicked() {
                    acts.push(Act::Spot(spot::READY, false));
                }
                // The status: who isn't ready, two empires needed, a start error.
                ui.add(egui::Label::new(RichText::new(s(crate::tr::bytes(lang, &self.line))).color(hot)).truncate());
            });
        });
    }

    /// A slot's row: the empire's colour, the race's head and name (a drop-down for the host and the holder), the
    /// holder (the host's drop-down: open, the computer at a level, closed), badges; Take seat while the pointer is on
    /// a free seat.
    #[allow(clippy::too_many_arguments)]
    fn slot_row(&self, ui: &mut egui::Ui, texts: &UiTexts, palette: &[[f32; 3]], i: usize, sl: &ig2_net::lobby::Slot, st: &ig2_net::lobby::LobbyState, me: Option<u8>, acts: &mut Vec<Act>) {
        let lang = texts.lang;
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, lang);
        let w = |b: &[u8]| words::text(lang, b);
        let host = self.is_host();
        let own = me.is_some_and(|m| sl.kind == SlotKind::Human(m));
        let (accent, dim) = (ui.visuals().hyperlink_color, ui.visuals().weak_text_color());
        let bg = ui.painter().add(egui::Shape::Noop);
        let row = egui::Frame::new().corner_radius(CornerRadius::same(7)).inner_margin(Margin::symmetric(4, 0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let sp = ui.spacing_mut();
            (sp.item_spacing.x, sp.interact_size.y, sp.button_padding.y) = (6.0, 21.0, 0.0);
            let hovered = ui.rect_contains_pointer(egui::Rect::from_min_size(ui.max_rect().min, egui::vec2(ui.max_rect().width(), 23.0)));
            ui.horizontal(|ui| {
                // ADR 0041: a stripe in the empire's colour (`ColonyKisInfo`'s owner colour 0xf + empire), the race's head.
                let (stripe, _) = ui.allocate_exact_size(egui::vec2(4.0, 22.0), egui::Sense::hover());
                if sl.kind != SlotKind::Closed {
                    let c = palette.get(0xf + sl.empire as usize).copied().unwrap_or([1.0; 3]);
                    ui.painter().rect_filled(stripe, 2.0, crate::gui::ink(c));
                }
                let (head, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
                match self.heads.get((sl.race as usize).wrapping_sub(1)) {
                    Some(Some(t)) if sl.kind != SlotKind::Closed => {
                        egui::Image::new(egui::load::SizedTexture::new(t.id(), head.size())).corner_radius(4.0).paint_at(ui, head);
                    }
                    _ => {
                        ui.painter().rect_filled(head, 4.0, ui.visuals().extreme_bg_color);
                    }
                }
                // The race.
                let race_name = |r: u8| s(texts.get(0x11, 1, (r.clamp(0, 19) + 5) as u16));
                let race_w = 96.0;
                if st.galaxy.is_some() {
                    if (host || own) && sl.kind != SlotKind::Closed {
                        let mut pick = None;
                        egui::ComboBox::from_id_salt(("mp-race", i)).width(race_w).truncate().selected_text(race_name(sl.race)).show_ui(ui, |ui| {
                            for r in 1..=8u8 {
                                if ui.add(egui::Button::selectable(r == sl.race, race_name(r))).clicked() {
                                    pick = Some(r);
                                }
                            }
                        });
                        if let Some(r) = pick.filter(|&r| r != sl.race) {
                            acts.push(Act::Race(i, r));
                        }
                    } else {
                        fixed(ui, race_w + 18.0, egui::Label::new(race_name(sl.race)).truncate(), egui::Align::Min);
                    }
                } else if let Some(h) = self.resume.as_ref().and_then(|(sv, _)| sv.session.humans.iter().find(|h| h.empire == sl.empire)) {
                    // A resumed save: whose empire it was.
                    fixed(ui, race_w + 18.0, egui::Label::new(RichText::new(&h.name).color(dim)).truncate(), egui::Align::Min);
                }
                // The holder; ADR 0033: the computer's level (the skirmish menu's strings 15/4/1..3).
                let level = |l: u8| s(texts.get(15, 4, l as u16));
                let now = st.galaxy.map_or(2, |o| o.difficulty[(sl.empire as usize).clamp(1, 8) - 1].clamp(1, 3));
                let holder = |k: SlotKind, l: u8| match k {
                    SlotKind::Ai if l != 0 => format!("{} · {}", w(words::COMPUTER), level(l)),
                    SlotKind::Ai => w(words::COMPUTER),
                    SlotKind::Closed => w(words::CLOSED),
                    _ => w(words::OPEN_SEAT),
                };
                let now = if st.galaxy.is_some() { now } else { 0 };
                let holder_w = 150.0;
                match sl.kind {
                    SlotKind::Human(m) => {
                        let name = st.members.iter().find(|x| x.id == m).map_or_else(String::new, |x| x.name.clone());
                        fixed(ui, holder_w + 18.0, egui::Label::new(RichText::new(name).strong()).truncate(), egui::Align::Min);
                    }
                    k if host => {
                        let mut pick = None;
                        let levels: &[u8] = if st.galaxy.is_some() { &[1, 2, 3] } else { &[0] };
                        egui::ComboBox::from_id_salt(("mp-holder", i)).width(holder_w).truncate().selected_text(holder(k, now)).show_ui(ui, |ui| {
                            if ui.add(egui::Button::selectable(k == SlotKind::Open, holder(SlotKind::Open, 0))).clicked() {
                                pick = Some((SlotKind::Open, 0));
                            }
                            for &l in levels {
                                if ui.add(egui::Button::selectable(k == SlotKind::Ai && l == now, holder(SlotKind::Ai, l))).clicked() {
                                    pick = Some((SlotKind::Ai, l));
                                }
                            }
                            if ui.add(egui::Button::selectable(k == SlotKind::Closed, holder(SlotKind::Closed, 0))).clicked() {
                                pick = Some((SlotKind::Closed, 0));
                            }
                        });
                        if let Some((n, l)) = pick {
                            if n != k {
                                acts.push(Act::Slot(i, n));
                            }
                            if l != 0 && l != now {
                                acts.push(Act::Level(i, l));
                            }
                        }
                    }
                    k => {
                        fixed(ui, holder_w + 18.0, egui::Label::new(RichText::new(holder(k, now)).color(dim)).truncate(), egui::Align::Min);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let ready = match sl.kind {
                        SlotKind::Human(m) => st.members.iter().find(|x| x.id == m).map(|x| x.ready),
                        _ => None,
                    };
                    match (sl.kind, ready) {
                        (SlotKind::Human(0), _) => badge(ui, &w(words::HOST_MARK), accent),
                        (_, Some(true)) => badge(ui, &s(texts.get(14, 5, 6)), Color32::from_rgb(120, 210, 140)),
                        (_, Some(false)) => badge(ui, &w(words::NOT_READY), dim),
                        _ => {}
                    }
                    if own {
                        badge(ui, &w(words::YOU), accent);
                    }
                    // Sit here: an open or computer seat, offered under the pointer.
                    let free = matches!(sl.kind, SlotKind::Open | SlotKind::Ai) && !own;
                    if free && hovered && ui.small_button(w(words::TAKE_SEAT)).clicked() {
                        acts.push(Act::Spot(spot::HOLDER + i as u32, false));
                    }
                });
            });
        });
        if own || ui.rect_contains_pointer(row.response.rect) {
            ui.painter().set(bg, egui::Shape::rect_filled(row.response.rect, 7.0, accent.gamma_multiply(if own { 0.16 } else { 0.07 })));
        }
    }

    /// The chat: the lines (the newest at the bottom) and the line being typed.
    fn chat_card(&self, ui: &mut egui::Ui, lang: ig2_formats::text::Lang, height: f32) {
        let w = |b: &[u8]| words::text(lang, b);
        crate::gui::card(ui, Some(&w(words::CHAT)), |ui| {
            let lines = (height - 80.0).max(28.0);
            egui::ScrollArea::vertical().id_salt("mp-chat").max_height(lines).min_scrolled_height(lines).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for (who, line) in &self.chat {
                    // A line of this machine's own (no speaker) shows bare.
                    if who.is_empty() {
                        ui.label(RichText::new(line).small().italics().weak());
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{who}:")).strong().color(ui.visuals().hyperlink_color));
                            ui.label(line);
                        });
                    }
                }
            });
            let width = ui.available_width();
            field(ui, &self.entry, lang, true, false, &w(words::CHAT_HINT), width);
        });
    }
}
