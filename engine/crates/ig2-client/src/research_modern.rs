//! Ours (ADR 0071): the research screen (`AktKep` 4, re/screens/research.md) as modern panels on the egui
//! layer, the "Modern research screen" option, drawn with the design kit (ADR 0073, `gui::ds`) after the research
//! board. The same functions as the original's screen: the twelve categories (the wheels' buttons) as tiles in three
//! groups at the bottom, the category's known techs in their status with the progress or the cost (the lists), the
//! shown tech's info lines, prerequisites, labs needed against the player's, days and cost (the info box and the
//! panels' texts), the funding (the urgent button, hot-spot 4: double money, double speed), start or stop (hot-spot
//! 1), the running project with the jump to it (hot-spot 0x19) and the auto-research switches (§5.8); the exit is the
//! top bar's starmap pick. The original's wire sphere, ball and tech models stay in the middle (the console is not
//! drawn: `crate::research::ResearchModels::modern`). The window fills a [`View`] from what `ResearchGfx2D_2` reads,
//! runs [`show`] once a frame and hands the [`Act`]s to [`crate::research::Research::modern_act`], which runs the
//! original's code; the keys stay the screen's.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

use crate::gui::ds::{self, Font, PanelKind, Plate, Text, Weight};
use crate::starmap_modern::{self as sm};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// The top bar's (the starmap HUD's acts).
    Bar(sm::Act),
    /// A category chosen (`TT_*`).
    Area(usize),
    /// A field of the shown category chosen (`TalalmanyKivalasztva`).
    Field(i32),
    /// Start or stop (`ResearchStartStop`).
    StartStop,
    /// The urgent button (`ResearchDoubleGomb`).
    Urgent,
    /// The running project shown (hot-spot 0x19).
    Current,
    /// An auto-research switch flipped (0 ships, 1 buildings, 2 tanks).
    Auto(usize),
}

/// A category.
#[derive(Debug, Clone, Default)]
pub struct Area {
    pub n: usize,
    pub name: String,
    /// Known techs (status ≥ 2) and the owned ones.
    pub known: usize,
    pub owned: usize,
    /// The running project is in it.
    pub running: bool,
}

/// One row of the category's list.
#[derive(Debug, Clone, Default)]
pub struct Tech {
    pub field: i32,
    pub name: String,
    /// 2 seen, 3 researchable, 4 researching, 5 owned.
    pub status: u8,
    /// The progress per cent (0 or 100 shows none).
    pub pct: u32,
    /// What it costs to research.
    pub cost: String,
}

/// The shown tech.
#[derive(Debug, Clone, Default)]
pub struct Detail {
    pub name: String,
    pub info: Vec<String>,
    /// The prerequisites' label and each one with whether the player owns it (none: "nothing").
    pub need: (String, Vec<(String, bool)>),
    /// Per lab (ships, tanks, buildings): needed, the player's, the tooltips (required, operational).
    pub labs: Vec<(i32, i32, String, String)>,
    /// The days' label and value, unless owned.
    pub days: Option<(String, String)>,
    /// The cost at normal and at double money, unless owned.
    pub cost: Option<(String, String)>,
}

/// The running project.
#[derive(Debug, Clone, Default)]
pub struct Current {
    pub name: String,
    pub money: String,
    pub days: String,
    pub part: f32,
}

/// The labels the panels need: start, stop, the running project's jump, no research, done, locked, funding, normal,
/// double money, labs, the auto switches' heading, on, off, exit, the three groups.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub start: String,
    pub stop: String,
    pub jump: String,
    pub none: String,
    pub done: String,
    pub locked: String,
    pub funding: String,
    pub normal: String,
    pub double: String,
    pub labs: String,
    pub auto: String,
    pub on: String,
    pub off: String,
    pub exit: String,
    pub groups: [String; 3],
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    pub areas: Vec<Area>,
    pub area: usize,
    pub techs: Vec<Tech>,
    pub field: i32,
    pub detail: Option<Detail>,
    pub current: Option<Current>,
    /// Start (`false`) or stop (`true`) and whether it acts now.
    pub stop: bool,
    pub start_on: bool,
    pub urgent: bool,
    pub auto: [(String, bool); 3],
    pub labels: Labels,
    /// Tooltips: start/stop, the jump, urgent, the auto switches.
    pub tips: [String; 6],
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub bar: sm::State,
}

/// The list's and the detail's widths, the category tiles' height.
const LIST_W: f32 = 460.0;
const INFO_W: f32 = 470.0;
const TILE_H: f32 = 44.0;
/// The auto switches' strip: its pads, the heading, the gap and the plates.
const AUTO_H: f32 = 32.0 + 18.0 + 8.0 + 32.0;
/// A status word (500 14 / 0.1 em), a tile's name (15 / 0.06 em).
const STATUS: Font = Font::DATA.sized(14.0).tracking(0.10).upper(true);
const TILE: Font = Font::DATA.sized(15.0).tracking(0.06);

/// The categories' groups (ships and weapons with the special one, buildings, vehicles), in the board's order.
const GROUPS: [&[usize]; 3] = [&[1, 2, 3, 4, 5, 12], &[6, 7, 8, 9], &[10, 11]];

/// One frame of the screen; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = sm::board_bar(ctx, &v.bar, &mut st.bar, Act::Bar);
    let top = sm::bar_bottom(ctx) + 20.0;
    let screen = ctx.screen_rect();
    // The bottom row first: the panels above stop over it.
    let nav = egui::Area::new(egui::Id::new("rmh-areas")).anchor(Align2::LEFT_BOTTOM, vec2(ds::EDGE, -ds::EDGE)).fade_in(false).show(ctx, |ui| {
        ds::entrance(ui);
        categories(ui, v, screen.width() - 2.0 * ds::EDGE, &mut acts)
    });
    let room = nav.response.rect.top() - 16.0 - top;
    // The list gives way when it and the info would meet (a large UI scale).
    let lw = LIST_W.min(screen.width() - 2.0 * ds::EDGE - INFO_W - 16.0);
    // The auto switches as one strip right over the tiles; the list takes all the height above it.
    egui::Area::new(egui::Id::new("rmh-auto")).anchor(Align2::LEFT_BOTTOM, vec2(ds::EDGE, nav.response.rect.top() - 16.0 - screen.bottom())).fade_in(false).show(ctx, |ui| {
        ds::entrance(ui);
        ds::panel(ui, PanelKind::Glass, 0.92, 16.0, |ui| auto(ui, v, lw, &mut acts));
    });
    egui::Area::new(egui::Id::new("rmh-list")).fixed_pos(pos2(ds::EDGE, top)).fade_in(false).show(ctx, |ui| {
        ds::entrance(ui);
        // The panel's pads, the title row and its gap, the strip and the gap over it.
        ds::panel(ui, PanelKind::Deep, 0.88, 16.0, |ui| list(ui, v, lw, (room - 32.0 - 34.0 - AUTO_H - 16.0).max(26.0 * 4.0), &mut acts));
    });
    egui::Area::new(egui::Id::new("rmh-info")).anchor(Align2::RIGHT_TOP, vec2(-ds::EDGE, top)).fade_in(false).show(ctx, |ui| {
        ds::entrance(ui);
        ds::panel(ui, PanelKind::Glass, 0.92, 16.0, |ui| {
            // Taller than its room on the last frame (a large UI scale): it scrolls in all of the room. Only then: a
            // scroll area keeps the height of its first frame, when the room is still a guess.
            let tall_id = egui::Id::new("rmh-info-tall");
            let tall = ui.ctx().data(|d| d.get_temp::<bool>(tall_id)).unwrap_or(false);
            let h = room - 32.0;
            let content = if tall {
                egui::ScrollArea::vertical().id_salt("rmh-info-scroll").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(h).min_scrolled_height(h).show(ui, |ui| info(ui, v, h, &mut acts)).content_size.y
            } else {
                ui.scope(|ui| info(ui, v, h, &mut acts)).response.rect.height()
            };
            ui.ctx().data_mut(|d| d.insert_temp(tall_id, content > h + 0.5));
        });
    });
    acts
}

/// The category's title with its owned / known count, its known techs with their status at the right.
fn list(ui: &mut Ui, v: &View, lw: f32, height: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let w = lw - 32.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing.y = 8.0;
    let area = v.areas.iter().find(|a| a.n == v.area);
    let (r, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    let base = ds::fit::baseline(r, Font::PANEL_TITLE);
    let count = area.map_or(String::new(), |a| format!("{} / {} {}", a.owned, a.known, v.labels.done));
    let cw = Text::new(&count, Font::LABEL).layout(ui.ctx(), w * 0.45).width;
    Text::new(&count, Font::LABEL).right().color(k.ink_muted).baseline(base).paint(ui, r.with_min_x(r.right() - cw));
    Text::new(area.map_or("", |a| a.name.as_str()), Font::PANEL_TITLE).baseline(base).paint(ui, r.with_max_x(r.right() - cw - 12.0));
    // At least `height` once it scrolls: the area keeps its last size, so the first frame's guess would stick.
    egui::ScrollArea::vertical().max_height(height).min_scrolled_height(height).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for t in &v.techs {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
            let on = t.field == v.field;
            ds::row_back(ui, r, hot, on);
            let inner = r.shrink2(vec2(8.0, 0.0));
            let base = ds::fit::baseline(r, Font::BODY);
            // The status at the right: done, the bar and per cent, the cost, locked.
            let (right_w, col) = match t.status {
                5 => (Text::new(&v.labels.done, STATUS).right().color(k.ink_muted).baseline(base).paint(ui, inner.with_min_x(inner.right() - 120.0)).width, k.ink),
                4 => {
                    let pct = format!("{}%", t.pct);
                    let pr = Rect::from_min_max(pos2(inner.right() - 44.0, r.top()), inner.max);
                    Text::new(&pct, Font::READOUT.sized(15.0).tracking(0.04)).right().color(k.lcd_ink).baseline(base).paint(ui, pr);
                    let b = Rect::from_center_size(pos2(pr.left() - 8.0 - 60.0, r.center().y), vec2(120.0, 6.0));
                    let p = ui.painter();
                    p.rect_filled(b, 3, k.glass);
                    p.rect_filled(b.with_max_x(b.left() + (b.width() * t.pct as f32 / 100.0).max(6.0)), 3, k.lcd_ink);
                    ds::rim(p, b, 3, k.rim);
                    (44.0 + 8.0 + 120.0, k.ink)
                }
                3 => (Text::new(&t.cost, STATUS).right().color(k.info).baseline(base).paint(ui, inner.with_min_x(inner.right() - 140.0)).width, k.ink),
                _ => (Text::new(&v.labels.locked, STATUS).right().color(k.ink_muted).baseline(base).paint(ui, inner.with_min_x(inner.right() - 120.0)).width, k.ink_muted),
            };
            let name_box = inner.with_max_x(inner.right() - right_w - 12.0);
            Text::new(&t.name, Font::BODY).color(col.lerp_to_gamma(Color32::WHITE, hot * 0.5)).baseline(base).paint(ui, name_box);
            if resp.clicked() {
                acts.push(Act::Field(t.field));
            }
        }
    });
}

/// The auto-research switches: their heading over one row of three plates, lit while on.
fn auto(ui: &mut Ui, v: &View, lw: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let w = lw - 32.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
    Text::new(&v.labels.auto, Font::LABEL).color(k.ink_muted).show(ui, vec2(w, 18.0));
    ui.horizontal(|ui| {
        let pw = (w - 2.0 * 8.0) / 3.0;
        for (i, (name, on)) in v.auto.iter().enumerate() {
            let state = if *on { &v.labels.on } else { &v.labels.off };
            let label = format!("{name} \u{b7} {state}");
            let b = ui.add_sized(vec2(pw, 32.0), Plate::new(&label).width(pw).pad(8.0).height(32.0).primary(*on).font(Plate::FONT.sized(13.0).tracking(0.08)));
            if ds::tip(b, &v.tips[3 + i]).clicked() {
                acts.push(Act::Auto(i));
            }
        }
    });
}

/// The shown tech: its name, its lines, the table (prerequisites, labs, days), the funding, start or stop, the
/// running project.
fn info(ui: &mut Ui, v: &View, room: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let w = INFO_W - 32.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing.y = 14.0;
    if let Some(d) = &v.detail {
        Text::new(&d.name, Font::HEADING.sized(24.0)).show(ui, vec2(w, 30.0));
        let (body, stats) = stat_lines(&d.info);
        if !body.is_empty() {
            let lines = ((room - 420.0) / 26.0).clamp(2.0, 8.0) as usize;
            let fit = Text::new(&body, Font::BODY).lines(lines).layout(ui.ctx(), w);
            let (r, resp) = ui.allocate_exact_size(vec2(w, fit.galley.size().y), Sense::hover());
            ui.painter().galley(r.min, fit.galley.clone(), Color32::WHITE);
            if fit.cut {
                ds::tip(resp, &body);
            }
        }
        // The table (`.ig-table`, 120 for the labels and 16 before the values, 6 between rows).
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let row = |ui: &mut Ui, label: &str, h: f32| {
                let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                let first = Rect::from_min_size(r.min, vec2(w, 20.0));
                Text::new(label, Font::LABEL).shrink(9.0).color(k.ink_muted).baseline(ds::fit::baseline(first, Font::DATA)).paint(ui, first.with_max_x(r.left() + 120.0));
                r.with_min_x(r.left() + 136.0)
            };
            for (label, value) in &stats {
                let r = row(ui, label, 22.0);
                Text::new(value, Font::DATA.sized(18.0)).baseline(ds::fit::baseline(r, Font::DATA)).paint(ui, r);
            }
            let need = &d.need.1;
            let r = row(ui, &d.need.0, 22.0 * need.len().max(1) as f32 - 2.0);
            for (i, (name, owned)) in need.iter().enumerate() {
                let line = Rect::from_min_size(pos2(r.left(), r.top() + 22.0 * i as f32), vec2(r.width(), 20.0));
                Text::new(name, Font::DATA).color(if *owned { k.info } else { k.ink_muted }).baseline(ds::fit::baseline(line, Font::DATA)).paint(ui, line);
            }
            if !d.labs.is_empty() {
                let r = row(ui, &v.labels.labs, 20.0);
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)));
                c.spacing_mut().item_spacing.x = 16.0;
                for (need, have, tip_need, tip_have) in &d.labs {
                    let s = format!("{have} / {need}");
                    let resp = Text::new(&s, Font::DATA).color(if have < need { k.enemy } else { k.info }).show(&mut c, vec2(64.0, 20.0));
                    ds::tip(resp, &format!("{tip_need}: {need}\n{tip_have}: {have}"));
                }
            }
            if let Some((label, days)) = &d.days {
                let r = row(ui, label, 20.0);
                Text::new(days, Font::DATA).color(k.info).baseline(ds::fit::baseline(r, Font::DATA)).paint(ui, r);
            }
        });
        if let Some((normal, double)) = &d.cost {
            ds::separator(ui, 0.0);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                Text::new(&v.labels.funding, Font::LABEL).color(k.ink_muted).show(ui, vec2(w, 18.0));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let half = (w - 8.0) / 2.0;
                    for (urgent, label, cost) in [(false, &v.labels.normal, normal), (true, &v.labels.double, double)] {
                        let on = v.urgent == urgent;
                        let b = ui.add(Plate::new(label).sub(cost).width(half).height(64.0).primary(on).font(Plate::FONT.sized(16.0).upper(true)));
                        // The urgent button flips the funding: a press on the other plate.
                        if ds::pressed(&ds::tip(b, &v.tips[2])) && !on {
                            acts.push(Act::Urgent);
                        }
                    }
                });
            });
        }
    }
    let label = if v.stop { &v.labels.stop } else { &v.labels.start };
    let b = ui.add_enabled(v.start_on, Plate::new(label).width(w).height(52.0).primary(v.stop));
    if ds::pressed(&ds::tip(b, &v.tips[0])) {
        acts.push(Act::StartStop);
    }
    // The running project: its name (a click shows it), its bar, its money and days.
    match &v.current {
        Some(c) => {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
            let p = ui.painter();
            p.rect_filled(r, ds::RADIUS_PLATE, k.glass_raised);
            ds::rim(p, r, ds::RADIUS_PLATE, k.rim.lerp_to_gamma(k.rim_bright, hot));
            let inner = r.shrink2(vec2(10.0, 6.0));
            let top = Rect::from_min_size(inner.min, vec2(inner.width(), 20.0));
            let jw = Text::new(&v.labels.jump, Font::LABEL.sized(11.0)).layout(ui.ctx(), inner.width() * 0.4).width;
            Text::new(&v.labels.jump, Font::LABEL.sized(11.0)).right().color(k.ink_muted).baseline(ds::fit::baseline(top, Font::DATA)).paint(ui, top.with_min_x(top.right() - jw));
            Text::new(&c.name, Font::DATA).color(k.ink.lerp_to_gamma(Color32::WHITE, hot)).baseline(ds::fit::baseline(top, Font::DATA)).paint(ui, top.with_max_x(top.right() - jw - 8.0));
            let b = Rect::from_min_size(pos2(inner.left(), top.bottom() + 4.0), vec2(inner.width(), 6.0));
            ui.painter().rect_filled(b, 3, k.glass_deep);
            ui.painter().rect_filled(b.with_max_x(b.left() + (b.width() * c.part.clamp(0.0, 1.0)).max(6.0)), 3, k.lcd_ink);
            ds::rim(ui.painter(), b, 3, k.rim);
            let low = Rect::from_min_max(pos2(inner.left(), b.bottom() + 2.0), inner.max);
            let base = ds::fit::baseline(low, Font::LABEL.sized(12.0));
            Text::new(&c.money, Font::LABEL.sized(12.0).upper(false)).color(k.ink_silver).baseline(base).paint(ui, low.with_max_x(low.center().x));
            Text::new(&c.days, Font::LABEL.sized(12.0).upper(false)).right().color(k.ink_silver).baseline(base).paint(ui, low.with_min_x(low.center().x));
            if ds::tip(resp, &v.tips[1]).clicked() {
                acts.push(Act::Current);
            }
        }
        None => _ = Text::new(&v.labels.none, Font::LABEL).color(k.ink_muted).show(ui, vec2(w, 18.0)),
    }
}

/// The info lines as running text and the stat pairs some end with ("…  Firepower: 25"): a short label and a value
/// with a digit after the last colon become table rows, the rest stays text.
fn stat_lines(lines: &[String]) -> (String, Vec<(String, String)>) {
    let mut body = Vec::new();
    let mut stats = Vec::new();
    for l in lines {
        let pair = l.rsplit_once(':').map(|(head, v)| (head.trim_end(), v.trim())).filter(|(_, v)| !v.is_empty() && v.chars().count() <= 12 && v.chars().any(|c| c.is_ascii_digit()));
        // The label: after the last wide gap (tab or 2+ spaces), else the whole head if it is short.
        let split = pair.and_then(|(head, v)| {
            let cut = head.rfind('\t').or_else(|| head.rfind("  ").map(|i| i + 1));
            let (text, label) = match cut {
                Some(i) => (head[..i].trim(), head[i + 1..].trim()),
                None => ("", head.trim()),
            };
            (!label.is_empty() && label.chars().count() <= 24).then(|| (text.to_string(), label.to_string(), v.to_string()))
        });
        match split {
            Some((text, label, v)) => {
                if !text.is_empty() {
                    body.push(text);
                }
                stats.push((label, v));
            }
            None => body.push(l.trim().to_string()),
        }
    }
    (body.join(" "), stats)
}

/// The categories as tiles in their three groups (`.grp`, flex 3 : 2 : 1, two rows), the exit at the right. Short of
/// room (a large UI scale) the groups shrink to 80 % of their content and then scroll; the exit stays on screen.
fn categories(ui: &mut Ui, v: &View, width: f32, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let room = width - ds::EXIT_W - 3.0 * 12.0;
        let h = 8.0 + 16.0 + 6.0 + 2.0 * TILE_H + 6.0 + 10.0;
        let flex: Vec<f32> = GROUPS.iter().map(|ns| ns.len().div_ceil(2) as f32).collect();
        let mut least: Vec<f32> = GROUPS.iter().map(|ns| {
            let need = needs(ui.ctx(), v, ns);
            need.iter().sum::<f32>() + 6.0 * (need.len() - 1) as f32 + 20.0
        }).collect();
        let scale = (room / least.iter().sum::<f32>()).clamp(0.8, 1.0);
        least.iter_mut().for_each(|l| *l *= scale);
        let widths = flex_widths(&flex, &least, room);
        let wide = widths.iter().sum::<f32>() + 2.0 * 12.0 > width - ds::EXIT_W - 12.0 + 0.5;
        let mut groups = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (g, ns) in GROUPS.iter().enumerate() {
                    group(ui, v, g, ns, vec2(widths[g], h), acts);
                }
            });
        };
        if wide {
            // No drag-to-scroll: a press on a tile would start the area's drag. The wheel and the bar scroll it.
            ui.allocate_ui_with_layout(vec2(width - ds::EXIT_W - 12.0, h + 12.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                // The plain wheel over the strip scrolls it sideways.
                if ui.rect_contains_pointer(ui.max_rect()) {
                    ui.ctx().input_mut(|i| (i.smooth_scroll_delta.x, i.smooth_scroll_delta.y) = (i.smooth_scroll_delta.x + i.smooth_scroll_delta.y, 0.0));
                }
                egui::ScrollArea::horizontal().id_salt("rmh-cats").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).min_scrolled_height(h).auto_shrink([false, true]).show(ui, groups);
            });
        } else {
            groups(ui);
        }
        // The exit: the top bar's starmap pick (`AktKepKell` 2).
        ui.allocate_ui_with_layout(vec2(ds::EXIT_W, h), egui::Layout::bottom_up(egui::Align::Min), |ui| {
            if ui.add(Plate::new(&v.labels.exit).width(ds::EXIT_W).height(56.0)).clicked() {
                acts.push(Act::Bar(sm::Act::Screen(2)));
            }
        });
    });
}

/// One group's frame, heading and tiles.
fn group(ui: &mut Ui, v: &View, g: usize, ns: &[usize], size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let gw = size.x;
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, ds::RADIUS_PANEL, k.glass_frame);
    ds::rim(p, r, ds::RADIUS_PANEL, k.metal_dark);
    ds::edge_line(p, r, ds::RADIUS_PANEL as f32, true, k.metal_light);
    let head = Rect::from_min_size(r.min + vec2(10.0, 8.0), vec2(gw - 20.0, 16.0));
    Text::new(&v.labels.groups[g], Font::LABEL.sized(12.0).tracking(0.16)).color(k.ink_silver).paint(ui, head);
    let widths = columns(ui.ctx(), v, ns, gw - 20.0);
    for (i, n) in ns.iter().enumerate() {
        let Some(a) = v.areas.iter().find(|a| a.n == *n) else { continue };
        let x: f32 = widths[..i / 2].iter().map(|w| w + 6.0).sum();
        let at = pos2(head.left() + x, head.bottom() + 6.0 + (i % 2) as f32 * (TILE_H + 6.0));
        let tw = widths[i / 2];
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(at, vec2(tw, TILE_H))));
        // Every category's button stays on, the chosen one too (`Forgotabla` clicks and turns it again).
        let resp = tile(&mut c, a, a.n == v.area, vec2(tw, TILE_H));
        if ds::pressed(&resp) {
            acts.push(Act::Area(a.n));
        }
    }
}

/// A group's column widths (`.cats`: auto columns, two rows): each its widest tile's content, the room left shared
/// out evenly; short of room, shrunk in proportion (the names then fit by shrinking).
fn columns(ctx: &egui::Context, v: &View, ns: &[usize], width: f32) -> Vec<f32> {
    let need = needs(ctx, v, ns);
    let cols = need.len();
    share(&need, width - 6.0 * (cols - 1) as f32)
}

/// A group's columns' natural widths: each its widest tile's padding, name, gap and count (and the running dot).
fn needs(ctx: &egui::Context, v: &View, ns: &[usize]) -> Vec<f32> {
    let cols = ns.len().div_ceil(2);
    (0..cols)
        .map(|c| {
            ns[2 * c..(2 * c + 2).min(ns.len())]
                .iter()
                .filter_map(|n| v.areas.iter().find(|a| a.n == *n))
                .map(|a| {
                    let name = Text::new(&a.name, TILE.weight(Weight::Medium)).layout(ctx, 1000.0).width;
                    let count = Text::new(&format!("{}/{}", a.owned, a.known), Font::READOUT.sized(15.0).tracking(0.08)).layout(ctx, 1000.0).width;
                    24.0 + name + 10.0 + count + if a.running { 12.0 } else { 0.0 }
                })
                .fold(0.0, f32::max)
        })
        .collect()
}

/// `room` shared by `flex` (CSS `flex: n`, basis 0), none under its `least`: the floored ones keep it, the rest share
/// what is left by their flex.
fn flex_widths(flex: &[f32], least: &[f32], room: f32) -> Vec<f32> {
    let mut fixed = vec![false; flex.len()];
    loop {
        let left = room - least.iter().zip(&fixed).filter(|(_, f)| **f).map(|(l, _)| l).sum::<f32>();
        let total: f32 = flex.iter().zip(&fixed).filter(|(_, f)| !**f).map(|(x, _)| x).sum();
        let w: Vec<f32> = (0..flex.len()).map(|i| if fixed[i] { least[i] } else { left * flex[i] / total.max(1e-3) }).collect();
        match (0..flex.len()).find(|&i| !fixed[i] && w[i] < least[i]) {
            Some(i) => fixed[i] = true,
            None => return w,
        }
    }
}

/// Widths `need` in `room`: the room left shared out evenly; short of room, the narrow ones keep their width and the
/// widest share the rest.
fn share(need: &[f32], room: f32) -> Vec<f32> {
    let cols = need.len();
    let sum: f32 = need.iter().sum();
    if sum <= room {
        return need.iter().map(|w| w + (room - sum) / cols as f32).collect();
    }
    // Short of room: the narrow columns keep their width, the widest ones share the rest (their names shrink).
    let mut sorted = need.to_vec();
    sorted.sort_by(f32::total_cmp);
    let (mut left, mut cap) = (room, room / cols as f32);
    for (i, w) in sorted.iter().enumerate() {
        cap = left / (cols - i) as f32;
        if *w > cap {
            break;
        }
        left -= w;
    }
    need.iter().map(|w| w.min(cap)).collect()
}

/// A category tile (`.cat`): the name, owned / known on LCD digits, the owned share as a line along the bottom; the
/// chosen one pressed with a white rim, its line in LCD ink; the running project's category marked with a dot.
fn tile(ui: &mut Ui, a: &Area, on: bool, size: egui::Vec2) -> egui::Response {
    let k = ds::tokens();
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered() && ui.is_enabled());
    let sel = ui.ctx().animate_bool_with_time(resp.id.with("sel"), on, ds::anim(ds::GLIDE_TIME));
    let p = ui.painter();
    ds::plate_fill(p, r, ds::RADIUS_PLATE, on || resp.is_pointer_button_down_on());
    if sel > 0.0 {
        ds::rim(p, r, ds::RADIUS_PLATE, k.ink.gamma_multiply(sel));
    } else {
        ds::hover_rim(p, r, ds::RADIUS_PLATE, hot);
    }
    let enabled = a.known > 0;
    let count = format!("{}/{}", a.owned, a.known);
    let cf = Font::READOUT.sized(15.0).tracking(0.08);
    let inner = r.shrink2(vec2(12.0, 0.0));
    let base = ds::fit::baseline(r, TILE);
    let cw = Text::new(&count, cf).right().color(if !enabled { k.ink_muted } else if on { k.ink } else { k.ink_silver }).baseline(base).paint(ui, inner.with_min_x(inner.right() - 60.0)).width;
    let mut name = inner.with_max_x(inner.right() - cw - 10.0);
    if a.running {
        // The running project's category: a lit dot before the name.
        let c = pos2(name.left() + 3.0, r.center().y);
        ui.painter().circle_filled(c, 3.0, k.lcd_ink);
        name = name.with_min_x(name.left() + 12.0);
    }
    Text::new(&a.name, TILE.weight(Weight::Medium)).shrink(12.0).color(if enabled { k.ink } else { k.ink_muted }).baseline(base).paint(ui, name);
    if enabled && a.known > 0 {
        let part = a.owned as f32 / a.known as f32;
        let line = Rect::from_min_size(pos2(r.left(), r.bottom() - 3.0), vec2(r.width() * part, 3.0));
        ui.painter().rect_filled(line, egui::CornerRadius { sw: ds::RADIUS_PLATE, ..Default::default() }, k.ink_muted.lerp_to_gamma(k.lcd_ink, sel));
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(v: &View, st: &mut State, events: Vec<egui::Event>, width: f32) -> (egui::Context, Vec<Act>) {
        run_at(v, st, events, vec2(width, 1080.0))
    }

    fn run_at(v: &View, st: &mut State, events: Vec<egui::Event>, size: egui::Vec2) -> (egui::Context, Vec<Act>) {
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        ds::set_motion(false);
        let mut acts = Vec::new();
        for i in 0..4 {
            let ev = if i == 3 { events.clone() } else { Vec::new() };
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)), events: ev, ..Default::default() };
            let _ = ctx.run(input, |ctx| acts = show(ctx, v, st));
        }
        (ctx, acts)
    }

    fn view(name: &str) -> View {
        View {
            areas: (1..=12).map(|n| Area { n, name: format!("{name} {n}"), known: 3, owned: 1, running: n == 2 }).collect(),
            area: 2,
            techs: vec![Tech { field: 1, name: "Laser".into(), status: 5, pct: 100, ..Default::default() }, Tech { field: 2, name: name.into(), status: 4, pct: 40, ..Default::default() }],
            field: 2,
            detail: Some(Detail { name: name.into(), info: vec!["A gun".into()], need: ("Required".into(), vec![("Laser".into(), true)]), labs: vec![(1, 2, "need".into(), "have".into())], days: Some(("Days".into(), "4".into())), cost: Some(("$9".into(), "$18".into())) }),
            stop: true,
            start_on: true,
            ..Default::default()
        }
    }

    /// Short of room the narrow columns keep their content's width and only the widest are cut.
    #[test]
    fn narrow_columns_keep_their_width() {
        assert_eq!(share(&[100.0, 200.0, 300.0], 660.0), vec![120.0, 220.0, 320.0]);
        // 1616 by 3 : 2 : 1 with the third group floored at 277: 803.4 and 535.6 for the others.
        let w = flex_widths(&[3.0, 2.0, 1.0], &[600.0, 400.0, 277.0], 1616.0);
        assert!((w[2] - 277.0).abs() < 0.01 && (w[0] - 803.4).abs() < 0.01 && (w[1] - 535.6).abs() < 0.01, "{w:?}");
        assert_eq!(share(&[100.0, 200.0, 300.0], 450.0), vec![100.0, 175.0, 175.0]);
        assert_eq!(share(&[100.0, 100.0], 100.0), vec![50.0, 50.0]);
    }

    #[test]
    fn stat_pairs_leave_the_text() {
        let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (body, stats) = stat_lines(&lines(&["A gun that fires.", "Very well.       Firepower: 25"]));
        assert_eq!((body.as_str(), stats), ("A gun that fires. Very well.", vec![("Firepower".to_string(), "25".to_string())]));
        let (body, stats) = stat_lines(&lines(&["Range: 3", "Ratio: a long sentence, not a stat value at all"]));
        assert_eq!((body.as_str(), stats), ("Ratio: a long sentence, not a stat value at all", vec![("Range".to_string(), "3".to_string())]));
    }

    #[test]
    fn the_panels_keep_the_boards_places() {
        for w in [1440.0, 1920.0, 2520.0] {
            let (ctx, acts) = run(&view("Ion"), &mut State::default(), Vec::new(), w);
            assert!(acts.is_empty());
            let r = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap();
            let (l, a, i, n) = (r("rmh-list"), r("rmh-auto"), r("rmh-info"), r("rmh-areas"));
            assert!((l.width() - LIST_W).abs() < 1.0 && (i.width() - INFO_W).abs() < 1.0, "{w}: {l:?} {i:?}");
            assert!(l.bottom() < a.top() && a.bottom() < n.top() && i.bottom() < n.top(), "{w}");
            assert!(i.right() <= w - ds::EDGE + 0.5 && n.right() <= w - ds::EDGE + 0.5, "{w}: {n:?}");
        }
    }

    /// A long category fills the height down to the auto strip, which sits right over the tiles (#104).
    #[test]
    fn the_list_takes_the_height_over_the_auto_strip() {
        let mut v = view("Ion");
        v.techs = (1..=12).map(|f| Tech { field: f, name: format!("Tech {f}"), status: 5, pct: 100, ..Default::default() }).collect();
        for (h, rows) in [(600.0, 6.0), (720.0, 10.0), (1080.0, 12.0)] {
            let (ctx, _) = run_at(&v, &mut State::default(), Vec::new(), vec2(1920.0, h));
            let r = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap();
            let (l, a, n) = (r("rmh-list"), r("rmh-auto"), r("rmh-areas"));
            eprintln!("{h}: list {l:?} auto {a:?} areas {n:?}");
            assert!(l.bottom() < a.top() && a.bottom() < n.top(), "{h}");
            assert!((n.top() - a.bottom() - 16.0).abs() < 1.0, "{h}: the strip sits over the tiles");
            assert!(l.height() - 32.0 - 34.0 >= rows * 26.0 - 1.0, "{h}: {} rows", (l.height() - 66.0) / 26.0);
        }
    }

    /// A large UI scale (1920x1080 at 150 % and 200 %): the categories scroll, the exit and every panel stay on screen,
    /// the list and the info don't meet.
    #[test]
    fn a_large_ui_scale_keeps_the_exit_on_screen() {
        for size in [vec2(1280.0, 720.0), vec2(960.0, 540.0)] {
            let (ctx, _) = run_at(&view("Long category name"), &mut State::default(), Vec::new(), size);
            let r = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap();
            let (l, a, i, n) = (r("rmh-list"), r("rmh-auto"), r("rmh-info"), r("rmh-areas"));
            assert!(n.right() <= size.x - ds::EDGE + 0.5 && n.bottom() <= size.y - ds::EDGE + 0.5, "{size:?}: {n:?}");
            assert!(l.right() < i.left() && a.bottom() < n.top() && i.bottom() < n.top(), "{size:?}: {l:?} {i:?} {n:?}");
            // The first tile (category 1) and the exit take a press where they stand.
            let click = |p: egui::Pos2| vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }, egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }];
            let (_, acts) = run_at(&view("Long category name"), &mut State::default(), click(pos2(n.left() + 30.0, n.top() + 8.0 + 16.0 + 6.0 + 20.0)), size);
            assert_eq!(acts, vec![Act::Area(1)], "{size:?}");
            let (_, acts) = run_at(&view("Long category name"), &mut State::default(), click(pos2(n.right() - 20.0, n.bottom() - 20.0)), size);
            assert_eq!(acts, vec![Act::Bar(sm::Act::Screen(2))], "{size:?}");
        }
    }

    /// The wheel over the scrolled categories brings the last group into view: its last tile takes a press.
    #[test]
    fn the_wheel_reaches_the_last_category() {
        let size = vec2(1280.0, 720.0);
        let v = view("Long category name");
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        ds::set_motion(false);
        let mut st = State::default();
        let mut acts = Vec::new();
        let mut frame = |events: Vec<egui::Event>| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)), events, ..Default::default() };
            let _ = ctx.run(input, |ctx| acts = show(ctx, &v, &mut st));
            ctx.memory(|m| m.area_rect(egui::Id::new("rmh-areas"))).unwrap()
        };
        for _ in 0..3 {
            frame(Vec::new());
        }
        let n = frame(Vec::new());
        let over = pos2(n.left() + 100.0, n.center().y);
        frame(vec![egui::Event::PointerMoved(over), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -2000.0), modifiers: Default::default() }]);
        // egui eases the wheel's scroll over a few frames.
        for _ in 0..60 {
            frame(Vec::new());
        }
        // Category 11: the last group's second row, at the right end of the scrolled strip.
        let at = pos2(n.right() - ds::EXIT_W - 12.0 - 40.0, n.top() + 8.0 + 16.0 + 6.0 + TILE_H + 6.0 + 20.0);
        frame(vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }, egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(acts, vec![Act::Area(11)]);
    }

    /// Long names are cut inside their boxes: the panels keep their widths.
    #[test]
    fn long_names_keep_the_panels() {
        let (ctx, _) = run(&view(&"Very long translated research name ".repeat(5)), &mut State::default(), Vec::new(), 1920.0);
        let r = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap();
        assert!((r("rmh-list").width() - LIST_W).abs() < 1.0 && (r("rmh-info").width() - INFO_W).abs() < 1.0);
    }

    #[test]
    fn a_click_on_a_category_and_a_row_asks_for_them() {
        let v = view("Ion");
        let mut st = State::default();
        let (ctx, _) = run(&v, &mut st, Vec::new(), 1920.0);
        let n = ctx.memory(|m| m.area_rect(egui::Id::new("rmh-areas"))).unwrap();
        let l = ctx.memory(|m| m.area_rect(egui::Id::new("rmh-list"))).unwrap();
        let click = |p: egui::Pos2| vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }, egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }];
        // The first group's third tile (category 3): column 1, row 0.
        let room = 1920.0 - 2.0 * ds::EDGE - ds::EXIT_W - 36.0;
        let widths = columns(&ctx, &v, GROUPS[0], room * 3.0 / 6.0 - 20.0);
        let at = pos2(n.left() + 10.0 + widths[0] + 6.0 + 20.0, n.top() + 8.0 + 16.0 + 6.0 + 20.0);
        let (_, acts) = run(&v, &mut st, click(at), 1920.0);
        assert_eq!(acts, vec![Act::Area(3)]);
        // The first row of the list: field 1.
        let at = pos2(l.left() + 40.0, l.top() + 16.0 + 26.0 + 8.0 + 13.0);
        let (_, acts) = run(&v, &mut st, click(at), 1920.0);
        assert_eq!(acts, vec![Act::Field(1)]);
    }

    /// A list longer than its panel still takes a click on a row (the scroll area doesn't scroll by dragging).
    #[test]
    fn a_click_on_a_row_of_a_long_list_picks_it() {
        let mut v = view("Ion");
        v.techs.extend((0..60).map(|i| Tech { field: 10 + i, name: format!("Tech {i}"), status: 3, ..Default::default() }));
        let mut st = State::default();
        let (ctx, _) = run(&v, &mut st, Vec::new(), 1920.0);
        let l = ctx.memory(|m| m.area_rect(egui::Id::new("rmh-list"))).unwrap();
        let at = pos2(l.left() + 40.0, l.top() + 16.0 + 26.0 + 8.0 + 26.0 + 13.0);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let (mut acts, mut t) = (Vec::new(), 1.0);
        for e in [vec![egui::Event::PointerMoved(at)], vec![], vec![button(true)], vec![button(false)]] {
            t += 1.0 / 60.0;
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0))), time: Some(t), events: e, ..Default::default() };
            let _ = ctx.run(input, |ctx| acts.extend(show(ctx, &v, &mut st)));
        }
        assert!(acts.contains(&Act::Field(2)), "{acts:?}");
    }
}
