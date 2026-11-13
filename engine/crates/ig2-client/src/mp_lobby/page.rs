//! Ours (ADR 0062, 0064, the boards' layout ADR 0078): the multiplayer screens as designed egui pages over the main
//! menu's scene. The connection page: the game lists (LAN, internet, saved games) as tabs over a table of games with
//! status tags and empty states, a panel to join by address or join code with the password, the relay server's panel,
//! Back and Host. The room: the eight seats (colour, race head, race, holder, tags), the galaxy's settings with
//! steppers, the host's connection panel (join code, port mapping, private, public list), the chat, Leave and Ready /
//! Start. A frame of the page returns [`Act`]s that [`MpScreen::apply`] carries out with the same calls the old hot
//! spots made; the text fields stay ours (their keys come through [`MpScreen::key`], ADR 0062 §4).

use egui::{Color32, Pos2, Rect, Vec2};
use ig2_net::galaxy::{GalaxyOptions, RoomCustom, Seat, CUSTOM_PLANETS, MAX_AIS, MONEY_MAX, PLANETS, START_PLANETS};
use ig2_net::lobby::SlotKind;
use ig2_net::rendezvous::Rendezvous;

use super::{room_host, room_state, spot, words, Field, Focus, MpScreen, Out, Plate, PortMap, Role};
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
    /// The host's new galaxy options (the custom game's values, ADR 0085 §4).
    Options(ig2_net::galaxy::GalaxyOptions),
    /// Esc: leave the room, then the screens.
    Back,
    /// The room-mods dialog's action (`true`) or Cancel (ADR 0086 §3).
    RoomMods(bool),
    /// The pointer on a text field: the connection page's (`Some`) or the room's chat line.
    Text(Option<Focus>, crate::text_edit::Mouse),
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

    /// `v` points down from the top on the 1080-point board, closer on a shorter page (a larger UI scale).
    fn y(&self, v: f32) -> f32 {
        self.r.top() + v * (self.r.height() / crate::gui::POINTS_HIGH).clamp(0.6, 1.0)
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

/// One of our text fields (its keys are ours): the text (dots for a password), the caret and the selection while
/// `focused`; a press puts the caret, a drag selects, a double-click picks a word (`crate::text_edit`), as an
/// [`Act::Text`] for `which`.
#[allow(clippy::too_many_arguments)]
fn field(ui: &mut egui::Ui, f: &Field, which: Option<Focus>, lang: ig2_formats::text::Lang, focused: bool, masked: bool, hint: &str, width: f32, height: f32, acts: &mut Vec<Act>) -> egui::Response {
    use crate::text_edit::Mouse;
    let text = if masked { "•".repeat(f.text.len()) } else { ig2_formats::text::decode(&f.text, lang) };
    let sel = f.sel.anchor.map(|a| a.min(f.text.len())).filter(|&a| focused && a != f.cursor).map(|a| a.min(f.cursor)..a.max(f.cursor));
    let (resp, hit) = ds::text_field_sel(ui, &text, focused.then_some(f.cursor), sel, hint, Vec2::new(width, height));
    let resp = resp.on_hover_cursor(egui::CursorIcon::Text);
    if let Some(at) = hit {
        let (pressed, shift) = ui.input(|i| (i.pointer.primary_pressed(), i.modifiers.shift));
        let m = if resp.double_clicked() {
            Some(Mouse::Word(at))
        } else if pressed && resp.is_pointer_button_down_on() {
            Some(Mouse::Press(at, shift))
        } else {
            resp.dragged().then_some(Mouse::Drag(at))
        };
        acts.extend(m.map(|m| Act::Text(which, m)));
    }
    resp
}

/// A panel at `rect` (an area of its own): its background behind `add`, `margin` inside.
#[allow(clippy::too_many_arguments)]
fn place<R>(ctx: &egui::Context, id: &str, rect: Rect, kind: ds::PanelKind, alpha: f32, margin: [f32; 2], opacity: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Area::new(egui::Id::new(id)).fixed_pos(rect.min).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        panel(ui, rect.size(), true, kind, alpha, margin, add)
    })
    .inner
}

/// A panel in `ui`: `size` wide, at least `size.y` high (exactly with `fixed`; 0: as its content), `margin` inside.
fn panel<R>(ui: &mut egui::Ui, size: Vec2, fixed: bool, kind: ds::PanelKind, alpha: f32, margin: [f32; 2], add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let at = ui.painter().add(egui::Shape::Noop);
    let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(margin[0] as i8, margin[1] as i8)).show(ui, |ui| {
        let inner = size - Vec2::new(2.0 * margin[0], 2.0 * margin[1]);
        ui.set_width(inner.x);
        if size.y > 0.0 {
            ui.set_min_height(inner.y);
            if fixed {
                ui.set_max_height(inner.y);
            }
        }
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
        add(ui)
    });
    ds::behind(ui, at, |p| ds::paint_panel(p, r.response.rect, kind, alpha));
    r.inner
}

/// A column of panels at `rect`, scrolling when they are taller than it (a short page: a larger UI scale). `add`
/// gets the column's height left under what it has added so far through its second argument.
fn column<R>(ctx: &egui::Context, id: &str, rect: Rect, opacity: f32, add: impl FnOnce(&mut egui::Ui, &dyn Fn(&egui::Ui) -> f32) -> R) -> R {
    egui::Area::new(egui::Id::new(id)).fixed_pos(rect.min).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        // The area's own room is smaller than the column: the column's rect, given.
        ui.allocate_ui_with_layout(rect.size(), egui::Layout::top_down(egui::Align::Min), |ui| {
            let scroll = egui::ScrollArea::vertical().id_salt(id).max_height(rect.height()).auto_shrink([false, true]);
            scroll.scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                let top = ui.cursor().top();
                add(ui, &|ui: &egui::Ui| rect.height() - (ui.cursor().top() - top))
            })
            .inner
        })
        .inner
    })
    .inner
}

/// The gap between a column's panels.
const PANEL_GAP: f32 = 24.0;

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
    let rect = Rect::from_min_size(Pos2::new(st.r.left() + st.m + 16.0, st.y(109.0)), Vec2::new(st.r.width() - 2.0 * st.m - 32.0, 68.0));
    bare(ctx, "mp-title", rect, opacity, egui::Layout::top_down(egui::Align::Min), |ui| {
        let f = Font { size: 34.0, line: 40.0, ..Font::PAGE_TITLE };
        Text::new(text, f).color(k.ink_silver).show(ui, Vec2::new(rect.width(), 40.0));
        ui.add_space(3.0);
        let sf = Font { size: 15.0, line: 20.0, weight: Weight::Medium, tracking: 0.1, upper: false, lcd: false };
        Text::new(sub, sf).color(k.info).show(ui, Vec2::new(rect.width(), 20.0));
    });
}

/// The tabs' labels (`.tabs button`: 500 17 / 0.12 em).
const TAB_FONT: Font = Font { size: 17.0, line: 20.0, weight: Weight::Medium, tracking: 0.12, upper: false, lcd: false };

/// The rows' text (500 17 / 0.06 em) and the drop-downs' (`.pick`).
const BODY: Font = Font { size: 17.0, line: 20.0, weight: Weight::Regular, tracking: 0.06, upper: false, lcd: false };
const PICK_FONT: Font = Font { size: 17.0, line: 20.0, weight: Weight::Medium, tracking: 0.06, upper: false, lcd: false };

/// The game table's cells (`.row`: 1fr 200 140 260, 16 apart, 16 in).
fn cells(r: Rect) -> Vec<Rect> {
    let r = r.shrink2(Vec2::new(16.0, 0.0));
    let k = ((r.width() - 48.0) * 0.6 / 600.0).clamp(0.0, 1.0);
    ds::columns(r, &[0.0, 200.0 * k, 140.0 * k, 260.0 * k], 16.0)
}

/// The footer's plates at the bottom right (`bottom` points up from the screen's bottom), right to left; the status
/// line at their left (`busy`: a spinner before it). Gives the clicked plate's index.
#[allow(clippy::too_many_arguments)]
fn footer(ctx: &egui::Context, st: Stage, bottom: f32, opacity: f32, plates: &[(String, f32, bool, bool)], status: &str, status_c: Color32, busy: bool) -> Option<usize> {
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
        if busy {
            ui.spinner();
        }
        // Left of the plates (not `min_rect`: `bare` makes it the whole footer).
        let end = ui.cursor().max.x - 8.0;
        let f = Font { size: 15.0, line: 22.0, weight: Weight::Regular, tracking: 0.05, upper: false, lcd: false };
        Text::new(status, f).color(status_c).lines(2).paint(ui, rect.with_max_x(end).with_min_y(rect.center().y - 11.0));
        out
    })
}

impl MpScreen {
    /// The race heads for the player rows (ADR 0041), read once from the install.
    pub fn load_heads(&mut self, ctx: &egui::Context, install: &ig2_formats::Install) {
        if self.custom_seat.is_none() {
            if let Some(t) = install.read_raw("ig2.exe").ok().and_then(|b| ig2_rules::PeImage::parse(&b).ok().map(|pe| crate::mainscr_scenario::Tables::read(&pe))) {
                self.custom_seat = Some(Seat { money: t.money[2].clamp(0, MONEY_MAX), tech: 2, planets: 2, aggression: 1, relation: 1 });
            }
        }
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
            self.room_mods_dialog(ctx, texts.lang, &mut acts);
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
                Act::Options(o) => self.set_options(o),
                Act::Back => self.key(None, true, net),
                Act::RoomMods(yes) => self.room_mods_answer(yes),
                Act::Text(which, m) => {
                    let f = match which {
                        Some(Focus::Address) => &mut self.address,
                        Some(Focus::Password) => &mut self.password,
                        Some(Focus::Relay) => &mut self.relay,
                        None => &mut self.entry,
                    };
                    crate::text_edit::mouse(f, m);
                    None
                }
            };
            outs.extend(o);
        }
        outs
    }

    /// Ours (ADR 0086 §3): a joiner refused for other mods sees which: the room's mods it lacks or has in another
    /// version, those the index doesn't have, its own extra ones; "Download and join" (or Restart and join when only
    /// the order differs) when every one can be fetched, else Open Mods. A modal over the connection page.
    fn room_mods_dialog(&self, ctx: &egui::Context, lang: ig2_formats::text::Lang, acts: &mut Vec<Act>) {
        use crate::room_mods::Offer;
        let Some(r) = &self.room_mods else { return };
        let w = |b: &[u8]| words::text(lang, b);
        let name = |m: &ig2_net::lobby::RoomMod| if m.version.is_empty() { m.id.clone() } else { format!("{} {}", m.id, m.version) };
        let unavailable = match &r.index {
            Some(Ok(i)) => r.diff.unavailable(i),
            _ => Vec::new(),
        };
        let mut lines = Vec::new();
        let mut group = |label: &[u8], items: Vec<String>| {
            if !items.is_empty() {
                lines.push(format!("{} {}", w(label), items.join(", ")));
            }
        };
        group(words::MODS_MISSING, r.diff.missing.iter().filter(|m| !unavailable.contains(m)).map(name).collect());
        group(words::MODS_OTHER, r.diff.other.iter().filter(|(m, _)| !unavailable.contains(&m)).map(|(m, mine)| format!("{} {mine} / {}", m.id, m.version)).collect());
        group(words::MODS_UNAVAILABLE, unavailable.iter().map(|m| name(m)).collect());
        group(words::MODS_EXTRA, r.diff.extra.iter().map(name).collect());
        if r.diff.order {
            lines.push(w(words::MODS_ORDER));
        } else if r.diff == crate::room_mods::Diff::default() {
            lines.push(w(words::MODS_FILES));
        }
        let offer = r.offer();
        match (&r.index, &r.error) {
            _ if r.installing() => lines.push(w(words::MODS_DOWNLOADING)),
            (None, _) => lines.push(w(words::MODS_CHECKING)),
            (_, Some(e)) => lines.push(e.clone()),
            (Some(Err(e)), _) if !e.is_empty() && r.diff.wanted().next().is_some() => lines.push(format!("{} {e}", w(words::MODS_NO_INDEX))),
            _ if matches!(offer, Offer::Join(_)) => lines.push(w(words::MODS_RESTARTS)),
            _ => {}
        }
        let yes = w(match offer {
            Offer::Join(false) => words::MODS_RESTART,
            Offer::Mods => words::MODS_OPEN,
            _ => words::MODS_JOIN,
        });
        let screen = ctx.screen_rect();
        let width = 640.0f32.min(screen.width() - 32.0);
        let area = egui::Modal::default_area(egui::Id::new("mp-room-mods")).fixed_pos(Pos2::new(screen.center().x - width / 2.0, screen.center().y - 160.0));
        egui::Modal::new(egui::Id::new("mp-room-mods")).area(area).frame(egui::Frame::NONE).backdrop_color(Color32::from_black_alpha(128)).show(ctx, |ui| {
            ui.set_width(width);
            ds::panel(ui, ds::PanelKind::Deep, 0.96, 28.0, |ui| {
                ui.set_width(width - 56.0);
                Text::new(&w(words::MODS_TITLE), Font::HEADING).show(ui, Vec2::new(ui.available_width(), Font::HEADING.line));
                ui.add_space(ds::SPACE[2]);
                let fit = Text::new(&lines.join("\n"), Font::BODY.sized(16.0)).color(ds::tokens().ink_silver).lines(14).layout(ui.ctx(), ui.available_width());
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), fit.galley.size().y), egui::Sense::hover());
                ui.painter().galley(rect.min, fit.galley, Color32::WHITE);
                ui.add_space(ds::SPACE[3]);
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 44.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(offer != Offer::Wait, ds::Plate::new(&yes).primary(true).width(240.0)).clicked() {
                        acts.push(Act::RoomMods(true));
                    }
                    if r.busy() {
                        ui.spinner();
                    }
                    if ui.add_enabled(!r.installing(), ds::Plate::new(&w(words::CANCEL_WORD)).width(160.0)).clicked() {
                        acts.push(Act::RoomMods(false));
                    }
                });
            });
        });
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
        place(ctx, "mp-list", st.left(st.y(216.0)), ds::PanelKind::Deep, 0.86, [24.0, 24.0], op, |ui| {
            // The list the page was opened for (Local network or Internet) and the saved games.
            let tab = self.saves.is_some() as usize;
            ui.horizontal(|ui| {
                let labels = [w(if self.inet { words::INTERNET } else { words::LAN }), w(words::SAVED_GAMES)];
                let mut n = tab;
                if ds::segmented_plates(ui, &mut n, &labels.each_ref().map(String::as_str), None, 44.0, TAB_FONT).changed() {
                    acts.push(Act::Spot(spot::SAVED, false));
                }
                if tab == 0 && self.internet.is_some() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.browsing.is_some() {
                            ui.spinner();
                        } else if ui.add(ds::Plate::new(&w(words::REFRESH)).width(180.0).font(ds::Plate::FONT.sized(17.0).tracking(0.12))).clicked() {
                            acts.push(Act::Spot(spot::REFRESH, false));
                        }
                    });
                }
            });
            ui.add_space(14.0);
            self.rows_list(ui, texts, acts);
        });
        // Join directly, the relay server.
        let right = st.right_col(st.y(216.0));
        column(ctx, "mp-right", right, op, |ui, _| {
            let wide = Vec2::new(ui.available_width(), 0.0);
            panel(ui, wide, false, ds::PanelKind::Glass, 0.88, [24.0, 22.0], |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let width = ui.available_width();
                title(ui, &w(words::JOIN_DIRECTLY));
                ui.add_space(8.0);
                label(ui, w(words::ADDRESS).trim_end_matches(':'));
                if field(ui, &self.address, Some(Focus::Address), lang, self.focus == Focus::Address, false, "192.168.0.10  /  IG2-…", width, 44.0, acts).clicked() {
                    acts.push(Act::Spot(spot::ADDRESS, false));
                }
                ui.add_space(8.0);
                label(ui, &w(words::PASSWORD));
                if field(ui, &self.password, Some(Focus::Password), lang, self.focus == Focus::Password, true, "", width, 44.0, acts).clicked() {
                    acts.push(Act::Spot(spot::PASSWORD, false));
                }
                ui.add_space(8.0);
                let saves = self.saves.is_some();
                let join_on = !saves && !self.busy() && (self.selected.is_some() || !self.address.text.is_empty());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.add_enabled(join_on, ds::Plate::new(&s(texts.get(16, 3, 3))).width(220.0)).clicked() {
                        acts.push(Act::Spot(spot::JOIN, false));
                    }
                });
            });
            // The browser build's relay server is fixed (`DEFAULT_RELAY`).
            if cfg!(target_arch = "wasm32") {
                return;
            }
            ui.add_space(PANEL_GAP);
            panel(ui, wide, false, ds::PanelKind::Glass, 0.88, [24.0, 22.0], |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let width = ui.available_width();
                title(ui, &w(words::RELAY_TITLE));
                ui.add_space(8.0);
                label(ui, &w(words::RELAY_ADDRESS));
                if field(ui, &self.relay, Some(Focus::Relay), lang, self.focus == Focus::Relay, false, "relay.example.org:port", width, 44.0, acts).clicked() {
                    acts.push(Act::Spot(spot::RELAY, false));
                }
                ui.add_space(10.0);
                hint(ui, &w(words::RELAY_HINT), k.ink_muted, 4);
            });
        });
        // Back, Host; the status: a refusal, a connect error, the host lost.
        // A join under way shows its stage and seconds (with a spinner) instead.
        let progress = self.progress(lang);
        let line = match &progress {
            Some(p) => s(p.clone()),
            None if self.lost && self.line.is_empty() => s(texts.get(14, 5, 3)),
            None => s(crate::tr::bytes(lang, &self.line)),
        };
        let plate = self.plate();
        let (main, main_spot) = match plate {
            Plate::Cancel => (w(words::STOP_JOIN), spot::CANCEL_JOIN),
            Plate::HostSaved => (w(words::HOST_SAVED), spot::HOST),
            Plate::Join => (s(texts.get(16, 3, 3)), spot::JOIN),
            Plate::Host => (w(words::HOST), spot::HOST),
        };
        let plates = [(s(texts.get(15, 3, 7)), 200.0, false, true), (main, 300.0, plate != Plate::Cancel, plate != Plate::HostSaved || self.selected.is_some())];
        match footer(ctx, st, 72.0, op, &plates, &line, hot, progress.is_some()) {
            Some(0) => acts.push(Act::Spot(spot::CANCEL, false)),
            Some(_) => acts.push(Act::Spot(main_spot, false)),
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
        let (head, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), egui::Sense::hover());
        let heads = [crate::tr::text(lang, "Game").to_owned(), w(words::HOST_MARK), w(words::PLAYERS), crate::tr::text(lang, "Status").to_owned()];
        ds::table_head(ui, &cells(head), &heads.each_ref().map(String::as_str), &[]);
        ui.add_space(18.0);
        // A room's Status cell: its state, and Password on a locked one.
        let status = |a: &ig2_net::discovery::Announcement, other: bool| {
            let state = room_state(a, other);
            let c = match state {
                words::INCOMPATIBLE => k.enemy,
                words::ROOM_OPEN => k.info,
                _ => k.ink_silver,
            };
            let mut t: Vec<(&[u8], Color32)> = vec![(state, c)];
            if a.password {
                t.push((words::LOCKED, k.ink_silver));
            }
            t
        };
        // One row: the name, the host (the save's players), the players, the tags; a click selects, a second click
        // (or a double click) joins.
        let mut row = |ui: &mut egui::Ui, n: usize, name: String, host: String, players: String, tags: &[(&[u8], Color32)], off: bool| {
            let on = self.selected == Some(n);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), egui::Sense::click());
            ds::row_back(ui, rect, ds::hover_t(ui.ctx(), resp.id, resp.hovered()), on);
            if on && self.reveal.take() {
                // Up and Down keep the selection in view.
                ui.scroll_to_rect(rect, None);
            }
            let c = cells(rect);
            let base = ds::baseline(rect, BODY);
            Text::new(&name, BODY).color(if off { k.ink_muted } else { k.ink }).baseline(base).paint(ui, c[0]);
            Text::new(&host, BODY).color(k.info).baseline(base).paint(ui, c[1]);
            Text::new(&players, Font { size: 19.0, line: 22.0, ..Font::READOUT }).upright().baseline(base).color(if off { k.ink_muted } else { k.ink }).paint(ui, c[2]);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(c[3]).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.spacing_mut().item_spacing.x = 8.0;
            for (t, col) in tags {
                ds::tag(&mut child, &w(t), *col);
            }
            if resp.clicked() {
                acts.push(Act::Spot(spot::LIST + n as u32, false));
            }
            if resp.double_clicked() && !on {
                acts.push(Act::Spot(spot::LIST + n as u32, false));
            }
        };
        let under = egui::ScrollArea::vertical().id_salt("mp-list").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).auto_shrink([false, false]).show(ui, |ui| {
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
                    let host: String = room_host(a).unwrap_or(&a.scenario).chars().take(24).collect();
                    row(ui, n, a.name.clone(), host, format!("{}/{}", a.humans, a.slots), &status(a, other), other || a.started);
                }
            } else {
                for (n, f) in self.list.iter().take(super::LIST_ROWS).enumerate() {
                    let a = &f.announcement;
                    let other = a.rules != self.rules;
                    let host = room_host(a).map_or_else(|| f.addr.to_string(), str::to_owned);
                    row(ui, n, a.name.clone(), host, format!("{}/{}", a.humans, a.slots), &status(a, other), other || a.started);
                }
            }
            // A click under the rows drops the selection.
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click()).1.clicked()
        });
        if under.inner {
            acts.push(Act::Spot(spot::DESELECT, false));
        }
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
        let top = Rect::from_min_size(Pos2::new(stage.r.left() + stage.m + 16.0, stage.y(109.0)), Vec2::new(stage.r.width() - 2.0 * stage.m - 32.0, 44.0));
        bare(ctx, "mp-title", top, op, egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 20.0;
            let humans = st.slots.iter().filter(|s| matches!(s.kind, SlotKind::Human(_))).count();
            let ready = st.members.iter().filter(|m| m.ready).count();
            let f = Font { size: 34.0, line: 40.0, ..Font::PAGE_TITLE };
            let room_w = (top.width() - 520.0).max(200.0);
            let fit = Text::new(&st.name, f).layout(ui.ctx(), room_w);
            Text::new(&st.name, f).color(k.ink_silver).show(ui, Vec2::new(fit.width.min(room_w) + 4.0, 44.0));
            ui.spacing_mut().item_spacing.x = 8.0;
            ds::tag(ui, &w(if host { words::HOST_MARK } else { words::JOINED }), k.ink_silver);
            if st.private {
                ds::tag(ui, &w(words::PRIVATE_BADGE), k.ink_silver);
            }
            if st.password {
                ds::tag(ui, &w(words::LOCKED), k.ink_silver);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let lf = Font { size: 24.0, line: 28.0, ..Font::READOUT };
                Text::new(&format!("{ready}/{humans}"), lf).color(k.lcd_ink).show(ui, Vec2::new(64.0, 28.0));
                Text::new(&w(words::READY_COUNT), Font::LABEL).color(k.ink_muted).right().show(ui, Vec2::new(120.0, 18.0));
            });
        });
        // The seats.
        let left = stage.left(stage.y(196.0));
        column(ctx, "mp-left", left, op, |ui, rest| {
            let wide = ui.available_width();
            panel(ui, Vec2::new(wide, 0.0), false, ds::PanelKind::Deep, 0.86, [24.0, 18.0], |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                // Ours (ADR 0085 §4): the custom game's empires in the seats' place while they are open.
                let empires = st.galaxy.filter(|o| o.custom.is_some() && ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new(EMPIRES))).unwrap_or(false));
                if let Some(o) = empires {
                    title(ui, crate::tr::text(lang, "Empires"));
                    ui.add_space(6.0);
                    self.custom_table(ui, texts, palette, o, st, host, acts);
                } else {
                    title(ui, &w(words::PLAYERS));
                    ui.add_space(6.0);
                    for (i, sl) in st.slots.iter().enumerate().take(8) {
                        self.slot_row(ui, texts, palette, i, sl, st, me, acts);
                    }
                }
            });
            // The galaxy's settings: four a row, the setting's name over its stepper (the host's).
            // Down to the column's bottom; a taller seats' panel (the empires' table) leaves it short: the column scrolls then.
            ui.add_space(PANEL_GAP);
            let h = rest(ui);
            panel(ui, Vec2::new(wide, h.max(160.0)), false, ds::PanelKind::Glass, 0.88, [24.0, 20.0], |ui| {
                {
                    match st.galaxy {
                        Some(o) => {
                            let planets = s(texts.get(15, 2, 8));
                            let at = PLANETS.iter().position(|&p| p == o.planets).unwrap_or(1);
                            // A custom game's planet count (ADR 0085 §4) in place of the room's steps.
                            let planets_v = match o.custom {
                                Some(c) => (format!("{}", c.planets), c.planets > CUSTOM_PLANETS.0, c.planets < CUSTOM_PLANETS.1),
                                None => (s(texts.get(14, 2, at as u16 + 1)), at > 0, at < 3),
                            };
                            let rows: [(String, String, bool, bool); super::OPTIONS] = [
                                (planets, planets_v.0, planets_v.1, planets_v.2),
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
                                    ui.add_space(SETTING_ROWS);
                                }
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 24.0;
                                    for (c, (name, value, down, up)) in four.iter().enumerate() {
                                        ui.allocate_ui_with_layout(Vec2::new(cell, SETTING_H), egui::Layout::top_down(egui::Align::Min), |ui| {
                                            ui.set_width(cell);
                                            ui.spacing_mut().item_spacing.y = SETTING_GAP;
                                            label(ui, name.trim_end_matches(':'));
                                            let step = ui.add_enabled_ui(host, |ui| ds::stepper(ui, value, cell - 64.0).1).inner;
                                            let moved = (step < 0 && *down) || (step > 0 && *up);
                                            if let (Some(cg), 0, 0, true) = (o.custom, r, c, moved) {
                                                let planets = cg.planets.saturating_add_signed(10 * step.signum() as i8);
                                                acts.push(Act::Options(GalaxyOptions { custom: Some(RoomCustom { planets, ..cg }.clamped()), ..o }));
                                            } else if moved {
                                                acts.push(Act::Spot(spot::OPTION + 2 * (4 * r + c) as u32 + (step > 0) as u32, false));
                                            }
                                        });
                                    }
                                });
                            }
                            self.custom_rows(ui, texts, o, host, acts);
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
                }
            });
        });
        // The host's connection, the chat.
        let right = stage.right_col(stage.y(196.0));
        column(ctx, "mp-right", right, op, |ui, rest| {
            let wide = ui.available_width();
            if let Some(Role::Host { listener, .. }) = &self.role {
                panel(ui, Vec2::new(wide, 0.0), false, ds::PanelKind::Glass, 0.88, [24.0, 18.0], |ui| {
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
                        let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), egui::Sense::hover());
                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
                        let changed = child.add_enabled_ui(live, |ui| ds::toggle(ui, &mut v, &off_l, &on_l).changed()).inner;
                        let f = Font { size: 17.0, line: 20.0, weight: Weight::Regular, tracking: 0.05, upper: false, lcd: false };
                        Text::new(name, f).color(if live { k.ink } else { k.ink_muted }).paint(ui, row.with_max_x(row.right() - child.min_rect().width() - 16.0));
                        if let Some((t, c)) = sub {
                            ui.add_space(-8.0);
                            hint(ui, &t, c, 2);
                        }
                        changed
                    };
                    let (map, map_on) = match listener.port_map() {
                        PortMap::Off => (None, false),
                        PortMap::Asking => (Some((w(words::MAP_ASKING), k.ink_muted)), true),
                        PortMap::Mapped(a) => (Some((crate::tr::text(lang, "Port mapping: on, reachable at {}").replace("{}", &a.to_string()), k.own)), true),
                        PortMap::Failed(e) => (Some((crate::tr::text(lang, "Port mapping failed: {}").replace("{}", &e), k.enemy)), false),
                    };
                    // A page can't open a router port (UPnP, NAT-PMP): no switch in the browser build.
                    if !cfg!(target_arch = "wasm32") && opt(ui, &w(words::MAP_LABEL), map, map_on, true) {
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
                            // A browser's room: no port reaches it.
                            Rendezvous::Registered(a) if a.port() == 0 => a.ip().to_string(),
                            Rendezvous::Registered(a) => a.to_string(),
                            _ => String::new(),
                        };
                        // A private room is never listed: its switch is off.
                        if opt(ui, &w(words::PUBLIC_LABEL), Some((crate::tr::text(lang, r).replace("{}", &seen), k.ink_muted)), self.public && !st.private, !st.private) {
                            acts.push(Act::Spot(spot::PUBLIC, false));
                        }
                    }
                });
                ui.add_space(PANEL_GAP);
            }
            let h = rest(ui).max(180.0);
            self.chat_panel(ui, Vec2::new(wide, h), lang, acts);
        });
        // Leave, Ready or Start; the status: who isn't ready, two empires needed, a start error.
        let label = if host {
            if self.force { w(words::START_ANYWAY) } else { s(texts.get(14, 1, 12)) }
        } else {
            s(texts.get(14, 5, 6))
        };
        let ready = st.members.iter().find(|m| Some(m.id) == me).is_some_and(|m| m.ready);
        let label = if !host && ready { format!("✔ {label}") } else { label };
        let plates = [(s(texts.get(14, 1, 11)), 200.0, false, true), (label, 260.0, host || ready, true)];
        match footer(ctx, stage, 56.0, op, &plates, &s(crate::tr::bytes(lang, &self.line)), hot, false) {
            Some(0) => acts.push(Act::Spot(spot::LEAVE, false)),
            Some(_) => acts.push(Act::Spot(spot::READY, false)),
            None => {}
        }
    }

    /// Ours (ADR 0085 §4): the custom game's switch and seed (the host's), and Empires: the seats' start values in the
    /// seats' panel (`custom_table`).
    fn custom_rows(&self, ui: &mut egui::Ui, texts: &UiTexts, o: GalaxyOptions, host: bool, acts: &mut Vec<Act>) {
        let tr = |t: &str| crate::tr::text(texts.lang, t).to_string();
        // Without the install's start tables there is nothing to start a custom game from.
        let Some(seat) = self.custom_seat.or(o.custom.map(|c| c.seats[0])) else { return };
        ui.add_space(SETTING_ROWS);
        let cell = (ui.available_width() - 3.0 * 24.0) / 4.0;
        let new_seed = || crate::new_game::wall_ticks() % 0x7fff;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 24.0;
            ui.allocate_ui_with_layout(Vec2::new(cell, SETTING_H), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(cell);
                ui.spacing_mut().item_spacing.y = SETTING_GAP;
                label(ui, &tr("Custom game"));
                let on = o.custom.is_some();
                let v = if on { tr("on") } else { tr("off (classic)") };
                let step = ui.add_enabled_ui(host, |ui| ds::stepper(ui, &v, cell - 64.0).1).inner;
                if step != 0 && (step > 0) != on {
                    let custom = (step > 0).then(|| {
                        let planets = o.planets.clamp(CUSTOM_PLANETS.0, CUSTOM_PLANETS.1);
                        let start = ig2_sim::game::new_game::skirmish_start_planets(2, planets as i32, o.ais as i32);
                        RoomCustom::new(planets, new_seed(), Seat { planets: start.clamp(1, 8) as u8, ..seat })
                    });
                    acts.push(Act::Options(GalaxyOptions { custom, ..o }));
                }
            });
            let Some(c) = o.custom else {
                ui.ctx().data_mut(|d| d.remove::<bool>(egui::Id::new(EMPIRES)));
                return;
            };
            ui.allocate_ui_with_layout(Vec2::new(cell, SETTING_H), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(cell);
                ui.spacing_mut().item_spacing.y = SETTING_GAP;
                label(ui, &tr("Seed"));
                let step = ui.add_enabled_ui(host, |ui| ds::stepper(ui, &c.seed.to_string(), cell - 64.0).1).inner;
                if step != 0 {
                    acts.push(Act::Options(GalaxyOptions { custom: Some(RoomCustom { seed: c.seed.wrapping_add_signed(step.signum()), ..c }), ..o }));
                }
            });
            ui.allocate_ui_with_layout(Vec2::new(cell, SETTING_H), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.add_space(SETTING_H - 36.0);
                if ui.add_enabled(host, ds::Plate::new(&tr("New seed")).width(cell).height(36.0)).clicked() {
                    acts.push(Act::Options(GalaxyOptions { custom: Some(RoomCustom { seed: new_seed(), ..c }), ..o }));
                }
            });
            // The empires' start values open in the seats' panel (and back).
            ui.allocate_ui_with_layout(Vec2::new(cell, SETTING_H), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.add_space(SETTING_H - 36.0);
                let id = egui::Id::new(EMPIRES);
                let open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
                if ui.add(ds::Plate::new(&tr(if open { "Players" } else { "Empires" })).width(cell).height(36.0)).clicked() {
                    ui.ctx().data_mut(|d| d.insert_temp(id, !open));
                }
            });
        });
    }

    /// Ours (ADR 0085 §4): the custom game's seats in play, a row each (the host's to edit, every seat sees them).
    #[allow(clippy::too_many_arguments)]
    fn custom_table(&self, ui: &mut egui::Ui, texts: &UiTexts, palette: &[[f32; 3]], o: GalaxyOptions, st: &ig2_net::lobby::LobbyState, host: bool, acts: &mut Vec<Act>) {
        let lang = texts.lang;
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, lang);
        let tr = |t: &str| crate::tr::text(lang, t).to_string();
        let k = ds::tokens();
        // A joiner's copy comes from the host: kept in range here too.
        let Some(c) = o.custom.map(RoomCustom::clamped) else { return };
        // The seats in play, a row each: the empire's colour and race; money, technology, start planets, a computer's
        // aggressiveness, the start relation.
        let widths = [100.0, 160.0, 0.0, 96.0, 0.0, 0.0];
        let heads_l = [tr("Race"), s(texts.get(15, 2, 0xb)), s(texts.get(15, 2, 0xa)), tr("Planets"), tr("Aggressiveness"), tr("Relations")];
        let tw = ui.available_width();
        let (head, _) = ui.allocate_exact_size(Vec2::new(tw, 22.0), egui::Sense::hover());
        let cols = ds::columns(head, &widths, 10.0);
        let refs: Vec<&str> = heads_l.iter().map(String::as_str).collect();
        ds::table_head(ui, &cols, &refs, &[]);
        let pick = Font { size: 15.0, line: 18.0, weight: Weight::Medium, tracking: 0.04, upper: false, lcd: false };
        let aggr = ["Low", "Normal", "High"];
        let rel = ["Hostile", "Neutral", "Friendly"];
        for sl in st.slots.iter().take(8).filter(|sl| matches!(sl.kind, SlotKind::Human(_) | SlotKind::Ai)) {
            let i = (sl.empire as usize).clamp(1, 8) - 1;
            let e = c.seats[i];
            let (row, _) = ui.allocate_exact_size(Vec2::new(tw, 40.0), egui::Sense::hover());
            let cc = ds::columns(row, &widths, 10.0);
            let col = palette.get(0xf + sl.empire as usize).copied().unwrap_or([1.0; 3]);
            ui.painter().rect_filled(Rect::from_min_size(row.min, Vec2::new(4.0, row.height())), 2.0, crate::gui::ink(col));
            Text::new(&s(texts.race(sl.race)), BODY).color(k.ink_silver).paint(ui, cc[0].shrink2(Vec2::new(12.0, 0.0)));
            let mut u: Vec<egui::Ui> = cc.iter().map(|&r| ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)))).collect();
            let money = u[1].add_enabled_ui(host, |ui| ds::stepper(ui, &e.money.to_string(), cc[1].width() - 64.0).1).inner;
            let planets = u[3].add_enabled_ui(host, |ui| ds::stepper(ui, &e.planets.to_string(), cc[3].width() - 64.0).1).inner;
            let mut select = |j: usize, id: &str, value: String, rows: Vec<(i32, String, bool)>| -> Option<i32> {
                if !host {
                    Text::new(&value, BODY).color(k.ink).paint(&u[j], cc[j].shrink2(Vec2::new(8.0, 0.0)));
                    return None;
                }
                ds::select(&mut u[j], (id, i), &value, &rows, Vec2::new(cc[j].width(), 34.0), pick, k.ink).1.flatten()
            };
            let tech_rows = (0..=3).map(|t| (t, s(texts.get(15, 5, t as u16 + 1)), false)).collect();
            let tech = select(2, "mp-cg-tech", s(texts.get(15, 5, e.tech as u16 + 1)), tech_rows);
            let aggression = if sl.kind == SlotKind::Ai { select(4, "mp-cg-aggr", tr(aggr[e.aggression as usize]), (0..3).map(|a| (a, tr(aggr[a as usize]), false)).collect()) } else { None };
            let relation = select(5, "mp-cg-rel", tr(rel[e.relation as usize]), (0..3).map(|a| (a, tr(rel[a as usize]), false)).collect());
            let n = Seat {
                money: if money != 0 { money_step(e.money, money > 0) } else { e.money },
                tech: tech.map_or(e.tech, |t| t as u8),
                planets: e.planets.saturating_add_signed(planets.signum() as i8).clamp(START_PLANETS.0, START_PLANETS.1),
                aggression: aggression.map_or(e.aggression, |a| a as u8),
                relation: relation.map_or(e.relation, |a| a as u8),
            };
            if n != e {
                let mut c = c;
                c.seats[i] = n;
                acts.push(Act::Options(GalaxyOptions { custom: Some(c.clamped()), ..o }));
            }
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
        ds::row_back(ui, rect, ds::hover_t(ui.ctx(), resp.id, hovered) * 0.6, own);
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
        let body = BODY;
        // The race: a drop-down (`.pick`, 190 × 38) for the host and the holder.
        let pick = Rect::from_min_size(Pos2::new(head.right() + 18.0, rect.center().y - 19.0), Vec2::new(190.0, 38.0));
        let race_name = |r: u8| s(texts.race(r));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pick).layout(egui::Layout::left_to_right(egui::Align::Center)));
        if st.galaxy.is_some() {
            if (host || own) && sl.kind != SlotKind::Closed {
                let races: Vec<(i32, String, bool)> = texts.playable_races().into_iter().map(|r| (r as i32, race_name(r), false)).collect();
                if let (_, Some(Some(r))) = ds::select(&mut child, ("mp-race", i), &race_name(sl.race), &races, pick.size(), PICK_FONT, k.ink) {
                    if r as u8 != sl.race {
                        acts.push(Act::Race(i, r as u8));
                    }
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
        let tags_w = (rect.width() * 0.32).min(300.0);
        let who = Rect::from_min_max(Pos2::new(pick.right() + 14.0, rect.top()), Pos2::new(rect.right() - 12.0 - tags_w, rect.bottom()));
        match sl.kind {
            SlotKind::Human(m) => {
                let name = st.members.iter().find(|x| x.id == m).map_or_else(String::new, |x| x.name.clone());
                Text::new(&name, body).color(k.ink).paint(ui, who);
            }
            kind if host => {
                // The rows' values: 0 open, 10 + level the computer, 20 closed.
                let levels: &[u8] = if st.galaxy.is_some() { &[1, 2, 3] } else { &[0] };
                let mut rows = vec![(0, holder(SlotKind::Open, 0), false)];
                rows.extend(levels.iter().map(|&l| (10 + l as i32, holder(SlotKind::Ai, l), false)));
                rows.push((20, holder(SlotKind::Closed, 0), false));
                let size = Vec2::new(who.width().min(260.0), pick.height());
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::new(who.left(), pick.top()), size)).layout(egui::Layout::left_to_right(egui::Align::Center)));
                let ink = if kind == SlotKind::Ai { k.info } else { k.ink_muted };
                let picked = match ds::select(&mut child, ("mp-holder", i), &holder(kind, now), &rows, size, PICK_FONT, ink).1 {
                    Some(Some(0)) => Some((SlotKind::Open, 0)),
                    Some(Some(20)) => Some((SlotKind::Closed, 0)),
                    Some(Some(v)) => Some((SlotKind::Ai, (v - 10) as u8)),
                    _ => None,
                };
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
            (_, Some(true)) => _ = ds::tag(&mut child, &s(texts.get(14, 5, 6)), k.own),
            (SlotKind::Human(0), _) => {}
            (_, Some(false)) => _ = ds::tag(&mut child, &w(words::NOT_READY), k.ink_muted),
            _ => {}
        }
        if sl.kind == SlotKind::Human(0) {
            ds::tag(&mut child, &w(words::HOST_MARK), k.ink_silver);
        }
        if own {
            ds::tag(&mut child, &w(words::YOU), k.ink_silver);
        }
        // Sit here: an open or computer seat, offered under the pointer.
        let free = matches!(sl.kind, SlotKind::Open | SlotKind::Ai) && !own;
        if free && hovered && child.add(ds::Plate::new(&w(words::TAKE_SEAT)).width(150.0).height(32.0).font(ds::Plate::FONT.sized(15.0).tracking(0.1))).clicked() {
            acts.push(Act::Spot(spot::HOLDER + i as u32, false));
        }
    }

    /// The chat panel: the lines (the newest at the bottom) and the line being typed.
    fn chat_panel(&self, ui: &mut egui::Ui, size: Vec2, lang: ig2_formats::text::Lang, acts: &mut Vec<Act>) {
        let w = |b: &[u8]| words::text(lang, b);
        let k = ds::tokens();
        panel(ui, size, true, ds::PanelKind::Deep, 0.86, [24.0, 20.0], |ui| {
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
            field(ui, &self.entry, None, lang, true, false, &w(words::CHAT_HINT), width, 42.0, acts);
        });
    }
}

/// `ig2-client --snapshot OUT --screen lobby|lobby-lan|lobby-room [--size WxH] [--mouse X,Y]`: the connection page with
/// sample internet rooms (`lobby-lan`: opened from Local network, no LAN game found), or a hosted room with a computer seat and chat lines (offline), over black.
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    use ig2_net::discovery::Announcement;
    let l = crate::load(o)?;
    let mut r = crate::render::Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let hud = crate::ui::Hud::load(&mut r, &l.install, &l.tables);
    let mut gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb).install_fonts(&l.install);
    let lan = o.screen.as_deref() == Some("lobby-lan");
    let mut s = MpScreen::open(!lan, "Dawee", [0; 32]);
    s.open = 100.0;
    s.load_heads(&gui.ctx, &l.install);
    let mut net = super::OfflineNet;
    if o.screen.as_deref() == Some("lobby-room") {
        s.host(&mut net);
        s.set_slot(3, SlotKind::Ai);
        s.set_slot(7, SlotKind::Closed);
        s.chat = vec![("Alice".into(), "hi all, starting in 5 minutes".into()), ("Dawee".into(), "one moment, picking a race".into())];
    } else if !lan {
        let room = |name: &str, scenario: &str, humans: u8, slots: u8, password: bool, started: bool, rules: u8| ig2_net::rendezvous::Listing {
            announcement: Announcement { session: 1, name: name.into(), port: 1, fingerprint: [0; 32], password, version: ig2_net::lobby::LOBBY_VERSION, rules: [rules; 32], scenario: scenario.into(), humans, slots, started },
            mods: [0; 32],
        };
        let at = std::net::SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 1);
        s.internet = Some(vec![
            (at, room("Alice's galaxy", "galaxy", 3, 8, false, false, 0)),
            (at, room("Bob's galaxy", "galaxy", 2, 4, true, false, 0)),
            (at, room("Zoltan's galaxy", "galaxy", 4, 4, false, false, 0)),
            (at, room("Kata's galaxy", "galaxy", 4, 4, false, true, 0)),
            (at, room("Peti's galaxy", "galaxy", 2, 8, false, false, 1)),
        ]);
        s.selected = Some(0);
        s.address.text = b"K7QF-29MX-TA4P".to_vec();
        // `lobby-joining`: a join to the first room under way for 7 s.
        if o.screen.as_deref() == Some("lobby-joining") {
            let (_tx, rx) = std::sync::mpsc::channel();
            s.connecting = Some((rx, None, None));
            s.joining = "Alice's galaxy".into();
            s.waited = 730;
        }
    }
    let theme = crate::theme::Theme::modern();
    let style = crate::gui::style(&theme);
    let k = crate::gui::Gui::scale(o.size.1);
    let v = crate::font::Virtual::new([o.size.0 as f32, o.size.1 as f32]);
    gui.pointer(o.mouse.map(|m| v.px(m[0], m[1])), k);
    for _ in 0..3 {
        gui.run(o.size, &style, |ctx| {
            s.page(ctx, &hud.texts, &hud.palette, &theme, false);
        });
    }
    let rgba = r.draw_to_image_over(&crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]), o.size, 0.0, |d, q, t| gui.paint(d, q, t));
    crate::assets::write_snapshot(out, o, &rgba)?;
    println!("{}", out.display());
    Ok(())
}

/// A galaxy setting's cell (its name over its stepper), the gap between the two and between the cells' rows: tight
/// enough that the custom game's third row fits under the eight seats at 1080 points.
const SETTING_H: f32 = 62.0;
const SETTING_GAP: f32 = 8.0;
const SETTING_ROWS: f32 = 12.0;

/// The egui memory key of the open custom game empires' table (ADR 0085 §4).
const EMPIRES: &str = "mp-cg-empires";

/// The money stepper's steps (the Custom game page's): 1000 to 10 000, 5000 to 100 000, 25 000 above.
fn money_step(v: i32, up: bool) -> i32 {
    let at = if up { v } else { v - 1 };
    let step = if at < 10_000 { 1000 } else if at < 100_000 { 5000 } else { 25_000 };
    let n = if up { (v / step + 1) * step } else { (v - 1).div_euclid(step) * step };
    n.clamp(0, MONEY_MAX)
}
