//! Ours (ADR 0078): the main menu board: the grouped menu (ADR 0059's panel) as a designed egui page over the main
//! menu's scene: the game's title, a deep panel of entries in groups (Play, Extras, then Settings and Quit), Continue
//! with the newest save under it, a sub-menu's entries with Back, the version at the bottom right. The entries act as
//! the panel's ([`crate::mainscr::MainScr::act_item`]); the rings stay hidden behind it as with the panel.

use egui::{Pos2, Rect, Vec2};

use crate::gui::ds::{self, Font, Text, Weight};
use crate::menu_flow::{Item, Panel};
use crate::ui::UiTexts;

/// Continue's line: the newest save's name (the font's bytes) and game date.
pub fn continue_line() -> Option<(Vec<u8>, [i32; 3])> {
    let dir = crate::options::slot_dir()?;
    let newest = crate::menu_flow::saves_newest_first(&dir).into_iter().next()?;
    let slot = crate::options::read_slots(Some(&dir)).into_iter().flatten().find(|sl| sl.file == newest)?;
    Some((slot.name, slot.date))
}

/// What a frame of the page picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Item(Item),
    /// A sub-menu's Back (Esc).
    Back,
}

/// The board's places (points at 1920 × 1080): the title, the panel, its width.
const TITLE: [f32; 2] = [112.0, 132.0];
const PANEL: [f32; 3] = [96.0, 252.0, 440.0];

/// The panel's groups: a heading (none for the last group) and its entries.
fn groups(p: Panel, history: bool) -> Vec<(Option<&'static str>, Vec<Item>)> {
    match p {
        Panel::Main => vec![
            (Some("Play"), vec![Item::Continue, Item::QuickGame, Item::NewGame, Item::Load, Item::Multiplayer, Item::Tutorial]),
            (Some("Extras"), [Item::History, Item::Replays, Item::Mods, Item::Credits].into_iter().filter(|&i| history || i != Item::History).collect()),
            (None, vec![Item::Settings, Item::Quit]),
        ],
        Panel::Single => vec![(None, vec![Item::Campaign, Item::Scenarios])],
        Panel::Multi => vec![(None, vec![Item::Local, Item::Internet])],
        Panel::Tutorial => vec![(None, (1..=5).map(Item::Part).collect())],
        Panel::Level => vec![(None, vec![Item::Level(0), Item::Level(2), Item::Level(1)])],
    }
}

/// The page title of a sub-menu (the remake text table's key); the main menu shows the game's name.
fn heading(p: Panel) -> Option<&'static str> {
    match p {
        Panel::Main => None,
        Panel::Single => Some("New game"),
        Panel::Multi => Some("Multiplayer"),
        Panel::Tutorial => Some("Tutorial"),
        Panel::Level => Some("Difficulty"),
    }
}

/// One frame of the page for the panel `p`: `continue_line` under Continue (the newest save's name and date), Continue
/// muted without a save. Gives the entry clicked.
pub fn page(ctx: &egui::Context, texts: &UiTexts, p: Panel, history: bool, can_continue: bool, continue_line: Option<&(Vec<u8>, [i32; 3])>, opacity: f32) -> Option<Pick> {
    let continue_line = continue_line.map(|(n, [y, m, d])| format!("{}  ·  {y}.{m:02}.{d:02}", ig2_formats::text::decode(n, texts.lang)));
    let k = ds::tokens();
    let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
    let screen = ctx.screen_rect();
    let at = |x: f32, y: f32| Pos2::new(screen.left() + x.min(screen.width() * 0.05).max(16.0) + (x - PANEL[0]).max(0.0), screen.top() + y);
    let mut pick = None;
    let all = groups(p, history);
    let n = all.iter().map(|g| g.1.len()).sum::<usize>() as f32 + if p == Panel::Main { 0.0 } else { 1.0 };
    // The board's column is `gap: 4px` between all its children: a heading 24 + 4, a rim line 4 + 14 + 1 + 14 + 4.
    let fixed = 48.0 + all.iter().filter(|g| g.0.is_some()).count() as f32 * 28.0 + (all.len() as f32 - 1.0 + (p != Panel::Main) as u8 as f32) * 37.0;
    // The entries 4 apart inside each group (Back is a group of its own); Continue's two lines take 49.
    let groups = all.len() as f32 + if p == Panel::Main { 0.0 } else { 1.0 };
    let cont = can_continue && p == Panel::Main;
    let line = |h: f32| fixed + n * h + (n - groups) * 4.0 + if cont { (49.0 - h).max(0.0) } else { 0.0 };
    // At a large UI scale the entries get lower (to 36 points), then the panel rises over the title's place.
    let room = screen.height() - PANEL[1] - 36.0 - fixed - (n - groups) * 4.0;
    let item_h = if cont { (room - 49.0) / (n - 1.0) } else { room / n }.clamp(36.0, 48.0);
    let top = PANEL[1].min(screen.height() - line(item_h) - 24.0).max(16.0);
    let show_title = top >= PANEL[1] - 20.0 || heading(p).is_none() && top >= 200.0;
    // The title: the game's name and ours, or the sub-menu's.
    egui::Area::new(egui::Id::new("mm-title")).fixed_pos(at(TITLE[0], TITLE[1])).fade_in(false).interactable(false).show(ctx, |ui| {
        ui.multiply_opacity(if show_title { opacity } else { 0.0 });
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let w = (screen.width() - 2.0 * TITLE[0]).max(200.0);
        let f = Font { size: 34.0, line: 40.0, ..Font::PAGE_TITLE };
        match heading(p) {
            None => {
                Text::new("IMPERIUM GALACTICA II", f).color(k.ink_silver).show(ui, Vec2::new(w, 40.0));
                ui.add_space(6.0);
                Text::new("OPENIG2", Font::LABEL.tracking(0.3)).color(k.ink_muted).show(ui, Vec2::new(w, 18.0));
            }
            Some(h) => {
                Text::new(&tr(h), f).color(k.ink_silver).show(ui, Vec2::new(w, 40.0));
            }
        }
    });
    // The panel.
    let width = PANEL[2].min(screen.width() - 32.0);
    egui::Area::new(egui::Id::new("mm-panel")).fixed_pos(at(PANEL[0], top)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        let place = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 24)).show(ui, |ui| {
            ui.set_width(width - 32.0);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let style = ds::MenuStyle { height: item_h, ..ds::MenuStyle::MENU };
            for (g, (head, items)) in all.iter().enumerate() {
                if g > 0 {
                    ds::menu_rule(ui);
                }
                if let Some(h) = head {
                    ds::menu_heading(ui, &tr(h));
                }
                let labels: Vec<String> = items.iter().map(|&i| tr(i.label())).collect();
                let entries: Vec<ds::MenuEntry> = items.iter().zip(&labels).map(|(&i, l)| {
                    let mut e = ds::MenuEntry::new(l);
                    if i == Item::Continue {
                        e.off = !can_continue;
                        e.sub = continue_line.as_deref().filter(|_| can_continue);
                    }
                    e
                }).collect();
                // The panel's default entry on the lit plate: Continue when there is a save.
                let sel = (g == 0 && p == Panel::Main && can_continue).then_some(0);
                if let Some(n) = ds::menu(ui, ("mm", g), style, &entries, sel, None) {
                    if !entries[n].off {
                        pick = Some(Pick::Item(items[n]));
                    }
                }
            }
            if p != Panel::Main {
                ds::menu_rule(ui);
                let back = tr("Back");
                if ds::menu(ui, "mm-back", style, &[ds::MenuEntry { hint: Some("ESC"), ..ds::MenuEntry::new(&back) }], None, None).is_some() {
                    pick = Some(Pick::Back);
                }
            }
        });
        ds::behind(ui, place, |pt| ds::paint_panel(pt, r.response.rect, ds::PanelKind::Deep, 0.80));
    });
    // The version at the bottom right.
    let f = Font { size: 13.0, line: 18.0, weight: Weight::Medium, tracking: 0.14, upper: false, lcd: false };
    let v = format!("openIG2 {}", crate::version());
    let rect = Rect::from_min_max(Pos2::new(screen.right() - 340.0, screen.bottom() - 46.0), Pos2::new(screen.right() - 40.0, screen.bottom() - 28.0));
    egui::Area::new(egui::Id::new("mm-version")).fixed_pos(rect.min).fade_in(false).interactable(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        Text::new(&v, f).color(k.ink_muted).right().show(ui, rect.size());
    });
    pick
}

/// What the name prompt asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamePick {
    /// OK: the entry's Enter.
    Accept,
    /// Back: the entry's Esc.
    Back,
}

/// Ours (ADR 0078): the multiplayer name entry (`EnterNameMode`) in the boards' look: a deep panel in the screen's
/// middle with the game's question (`GetString(15, 3, 2)`), the name in a recessed field with its caret, Back and
/// OK; the keys stay the entry's.
pub fn name_page(ctx: &egui::Context, texts: &UiTexts, n: &crate::mainscr::NameEntry, opacity: f32) -> Option<NamePick> {
    let k = ds::tokens();
    let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
    let screen = ctx.screen_rect();
    let w = 520.0f32.min(screen.width() - 32.0);
    let mut pick = None;
    ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, egui::Color32::from_black_alpha((90.0 * opacity) as u8));
    egui::Area::new(egui::Id::new("mm-name")).fixed_pos(Pos2::new(screen.center().x - w / 2.0, screen.top() + screen.height() * 0.32)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        let place = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new().inner_margin(egui::Margin { left: 28, right: 28, top: 28, bottom: 24 }).show(ui, |ui| {
            let iw = w - 56.0;
            ui.set_width(iw);
            ui.spacing_mut().item_spacing = Vec2::new(16.0, 16.0);
            let q = ig2_formats::text::decode(&texts.get(15, 3, 2), texts.lang);
            Text::new(q.trim(), Font::PAGE_TITLE.sized(24.0)).color(k.ink_silver).shrink(16.0).show(ui, Vec2::new(iw, 30.0));
            let (field, _) = ui.allocate_exact_size(Vec2::new(iw, 44.0), egui::Sense::hover());
            ds::field_back(ui.painter(), field, true);
            let caret = (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0;
            let name = ig2_formats::text::decode(&n.text, texts.lang);
            let (before, after) = name.split_at(name.char_indices().nth(n.cursor).map_or(name.len(), |c| c.0));
            let f = Font { size: 20.0, line: 24.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
            Text::new(&format!("{before}{}{after}", if caret { "|" } else { " " }), f).color(k.ink).paint(ui, field.shrink2(Vec2::new(14.0, 0.0)));
            ui.ctx().request_repaint();
            let hint = Font { size: 14.0, line: 20.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
            Text::new(&tr("Enter accepts, Esc goes back."), hint).color(k.ink_muted).show(ui, Vec2::new(iw, 20.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(ds::Plate::new(&tr("OK")).primary(true).width(160.0)).clicked() {
                    pick = Some(NamePick::Accept);
                }
                if ui.add(ds::Plate::new(&tr("Back")).width(160.0)).clicked() {
                    pick = Some(NamePick::Back);
                }
            });
        });
        ds::behind(ui, place, |pt| ds::paint_panel(pt, r.response.rect, ds::PanelKind::Deep, 0.90));
    });
    pick
}
