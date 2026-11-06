//! Ours (the boards' look, ADR 0078): the scenario / skirmish list (`Mode_Scenario_Valasztas`, re/screens/mainscr.md
//! §9.3) as a modern page over the main menu's scene, on the save and load board's layout: the entries at the left
//! (the skirmish entries in the info colour), at the right the shown entry's picture, its name, difficulty and race,
//! then its description or high scores (the switch of box 0x27), or for a skirmish entry its seven settings with
//! steppers; the footer's Back (box 0x16) and Start (box 0x15). The acts go through [`ScenarioList`]'s own boxes'
//! code ([`ScenarioList::pick`], [`ScenarioList::step`], [`ScenarioList::start`], [`ScenarioList::cancel`]).

use std::collections::HashMap;

use crate::gui::ds::{self, Font, Text};
use crate::mainscr_scenario::{fmt1, fmts, ScenarioList, Tables};
use crate::ui::UiTexts;

/// The board's size (points at 1920 × 1080), as the save and load page's.
const PAGE: [f32; 2] = [1440.0, 880.0];
/// The list's width.
const LIST: f32 = 440.0;

/// What a frame of the page asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// A row, a setting or the high-score switch (the click's sound).
    Click,
    /// Start: the caller closes the list ([`ScenarioList::start`]) and starts the chosen entry
    /// ([`crate::mainscr::MainScr::start_chosen`]).
    Start,
    /// Back: the caller closes the list ([`ScenarioList::cancel`]).
    Back,
}

/// The page's pictures, loaded once each: the scenarios' (by `pic`) and the races' heads (race 1..=8).
#[derive(Default)]
pub struct Pics {
    scenarios: HashMap<String, Option<egui::TextureHandle>>,
    heads: Option<Vec<Option<egui::TextureHandle>>>,
}

fn texture(ctx: &egui::Context, install: &ig2_formats::Install, dir: &str, name: &str, key: String) -> Option<egui::TextureHandle> {
    let p = install.resolve_texture(dir, name)?;
    let img = crate::assets::Image::load(install, &p).ok()?;
    let pixels = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
    Some(ctx.load_texture(key, pixels, egui::TextureOptions::LINEAR))
}

impl Pics {
    /// The scenario picture `pic` (`SetKepName(0x3e, entry + 8)`: `scenario/<dir>/<name>`).
    fn scenario(&mut self, ctx: &egui::Context, install: &ig2_formats::Install, pic: &str) -> Option<egui::TextureHandle> {
        self.scenarios
            .entry(pic.to_string())
            .or_insert_with(|| {
                let (dir, name) = pic.rsplit_once('\\').unwrap_or(("", pic));
                texture(ctx, install, &format!("scenario/{}/", dir.replace('\\', "/")), name, format!("sc-{pic}"))
            })
            .clone()
    }

    /// Race `race`'s head (`textures/main/fejek3/<race>a.bmp`).
    fn head(&mut self, ctx: &egui::Context, install: &ig2_formats::Install, race: i32) -> Option<egui::TextureHandle> {
        let heads = self.heads.get_or_insert_with(|| (1..=8).map(|k| texture(ctx, install, "textures/main/fejek3/", &format!("{k}a.bmp"), format!("sc-head-{k}"))).collect());
        heads.get((race - 1).max(0) as usize).cloned().flatten()
    }
}

/// One frame of the page; gives what was asked for.
pub fn page(ctx: &egui::Context, texts: &UiTexts, install: &ig2_formats::Install, sc: &mut ScenarioList, tables: &Tables, pics: &mut Pics) -> Option<Act> {
    let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
    let game = |b: &[u8]| ig2_formats::text::decode(b, texts.lang).trim().trim_end_matches(':').trim().to_string();
    let k = ds::tokens();
    let mut act = None;
    crate::gui::board_at(ctx, "scenarios", egui::Vec2::from(PAGE), 0.55, 12.0, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let w = ui.available_width();
        Text::new(&tr("Scenarios"), Font::PAGE_TITLE).color(k.ink_silver).show(ui, egui::vec2(w, 36.0));
        ui.add_space(20.0);
        let body = ui.available_height() - 20.0 - 64.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.vertical(|ui| {
                ui.set_width(LIST);
                ui.set_height(body);
                act = act.or(list(ui, texts, sc, body));
            });
            ui.add_space(32.0);
            ui.vertical(|ui| {
                ui.set_width(w - LIST - 32.0);
                ui.set_height(body);
                act = act.or(details(ui, texts, install, sc, tables, pics, body));
            });
        });
        ui.add_space(20.0);
        // The footer: Start and Back.
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 64.0), egui::Sense::hover());
        ui.painter().hline(rect.x_range(), rect.top() + 0.5, egui::Stroke::new(1.0f32, k.rim));
        let row = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + 20.0), rect.max);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = 12.0;
        if child.add_enabled(sc.chosen().is_some(), ds::Plate::new(&game(&texts.get(15, 2, 2))).primary(true).width(200.0)).clicked() {
            act = Some(Act::Start);
        }
        if child.add(ds::Plate::new(&game(&texts.get(15, 2, 3))).width(200.0)).clicked() {
            act = Some(Act::Back);
        }
    });
    act
}

/// Carry out a Start or Back of the page on the main menu: the list closes, Start starts the chosen entry.
pub fn finish(s: &mut crate::mainscr::MainScr, a: Act) {
    let Some(sc) = s.scenario.as_mut() else { return };
    match a {
        Act::Start if sc.start() => s.start_chosen(),
        Act::Back => sc.cancel(),
        _ => {}
    }
}

/// The entries; a click selects.
fn list(ui: &mut egui::Ui, texts: &UiTexts, sc: &mut ScenarioList, body: f32) -> Option<Act> {
    let k = ds::tokens();
    let mut act = None;
    let f = Font::BODY.sized(17.0);
    egui::ScrollArea::vertical().id_salt("scenario-list").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(body).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for i in 0..sc.entries.len() {
            let e = &sc.entries[i];
            let name = ig2_formats::text::decode(&e.name, texts.lang);
            let ink = if e.skirmish() { k.info } else { k.ink };
            if ds::table_row(ui, 36.0, &[0.0], &[(name.as_str(), f, ink)], &[], Some(sc.selected == i)).clicked() && sc.selected != i {
                sc.pick(i);
                // The details follow the selection at once (the classic panel swaps its picture after a fade).
                sc.shown = i;
                act = Some(Act::Click);
            }
        }
    });
    act
}

/// The shown entry: picture, name, difficulty, race; its description, high scores or skirmish settings.
#[allow(clippy::too_many_arguments)]
fn details(ui: &mut egui::Ui, texts: &UiTexts, install: &ig2_formats::Install, sc: &mut ScenarioList, tables: &Tables, pics: &mut Pics, body: f32) -> Option<Act> {
    let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
    let game = |b: &[u8]| ig2_formats::text::decode(b, texts.lang).trim().trim_end_matches(':').trim().to_string();
    let k = ds::tokens();
    let e = sc.entry().cloned()?;
    let mut act = None;
    let w = ui.available_width();
    // The picture (380 × 238 in the original) and beside it the name, difficulty and race.
    let pic_w = (w * 0.5).min(456.0);
    let pic_h = pic_w * 238.0 / 380.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(24.0, 0.0);
        let (r, _) = ui.allocate_exact_size(egui::vec2(pic_w, pic_h), egui::Sense::hover());
        ds::picture_well(ui.painter(), r);
        if let Some(t) = pics.scenario(ui.ctx(), install, &e.pic) {
            ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        ui.vertical(|ui| {
            let iw = ui.available_width();
            ui.spacing_mut().item_spacing.y = 10.0;
            let title = Font::HEADING.sized(26.0).tracking(0.08);
            Text::new(&ig2_formats::text::decode(&e.name, texts.lang), Font { line: 32.0, ..title }).lines(2).show(ui, egui::vec2(iw, 64.0));
            ui.spacing_mut().item_spacing.y = 2.0;
            if !e.skirmish() {
                ds::kv(ui, &game(&texts.get(15, 2, 6)), &game(&texts.get(15, 4, e.difficulty.max(0) as u16)), ds::Tone::Info, 158.0);
            }
            if !sc.high_score || e.skirmish() {
                let race = if e.skirmish() { sc.sk.race } else { e.race };
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let (r, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
                    if let Some(t) = pics.head(ui.ctx(), install, race) {
                        ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                    }
                    Text::new(&game(&texts.get(0x11, 1, (race.clamp(0, 19) + 5) as u16)), Font::BODY.sized(18.0)).color(k.ink).show(ui, egui::vec2(iw - 76.0, 64.0));
                });
            }
        });
    });
    ui.add_space(20.0);
    let rest = (body - pic_h - 20.0).max(80.0);
    if e.skirmish() {
        egui::ScrollArea::vertical().id_salt("scenario-settings").max_height(rest).auto_shrink([false, false]).show(ui, |ui| {
            for s in 0..7 {
                let label = match s {
                    0 => tr("Race"),
                    1 => tr("Players"),
                    _ => game(&texts.get(15, 2, [6, 8, 0xa, 0xb, 0xc][s - 2])),
                };
                let sk = sc.sk;
                let value = match s {
                    0 => game(&texts.get(0x11, 1, (sk.race.clamp(0, 19) + 5) as u16)),
                    1 => game(&fmt1(&texts.get(15, 2, 5), sk.computers + 1)),
                    2 => game(&texts.get(15, 4, sk.difficulty as u16)),
                    3 => game(&fmt1(&texts.get(15, 2, 7), tables.planets[sk.planets.clamp(0, 4) as usize])),
                    4 => game(&fmts(&texts.get(15, 2, 9), &texts.get(15, 5, sk.technology as u16 + 1))),
                    5 => format!("$ {}", tables.money[sk.money.clamp(0, 5) as usize]),
                    _ => game(&texts.get(15, 6, sk.population as u16 + 1)),
                };
                let (_, (_, d)) = ds::option_row(ui, &label, false, true, |ui| ds::stepper(ui, &value, 220.0));
                let (lo, hi) = ScenarioList::bounds(s);
                let v = sk.value(s);
                if (d > 0 && v < hi) || (d < 0 && v > lo) {
                    sc.step(s, d > 0);
                    act = Some(Act::Click);
                }
            }
        });
        return act;
    }
    // The description or the high scores, and the switch between them (box 0x27).
    let switch_h = 44.0;
    egui::ScrollArea::vertical().id_salt("scenario-text").max_height(rest - switch_h - 16.0).auto_shrink([false, false]).show(ui, |ui| {
        let f = Font { line: 24.0, ..Font::BODY.sized(16.0).tracking(0.04) };
        if sc.high_score {
            ds::heading_label(ui, &game(&fmts(&texts.get(15, 2, 0xf), &e.name)));
            ui.add_space(8.0);
            for (i, slot) in (0..ig2_formats::misc_text::HIGHSCORE_SLOTS).map(|i| (i, e.scores.get(i).filter(|(_, s)| *s >= 1))) {
                let (name, days, ink) = match slot {
                    Some((n, score)) => (ig2_formats::text::decode(n, texts.lang), format!("{} {}", score / 0x32, game(&texts.get(15, 2, 0x10))), k.ink),
                    None => (game(&texts.get(15, 2, 0x11)), String::new(), k.ink_muted),
                };
                let num = format!("{:02}.", i + 1);
                ds::table_row(ui, 30.0, &[48.0, 0.0, 160.0], &[(num.as_str(), f, k.ink_muted), (name.as_str(), f, ink), (days.as_str(), f, k.info)], &[false, false, true], None);
            }
        } else {
            let text: Vec<String> = e.lines.iter().filter(|l| !l.is_empty()).map(|l| ig2_formats::text::decode(l, texts.lang).trim_end().to_string()).collect();
            let iw = ui.available_width();
            for l in text {
                Text::new(&l, f).color(k.ink_silver).show(ui, egui::vec2(iw, 24.0));
            }
        }
    });
    ui.add_space(16.0);
    let label = game(&texts.get(15, 2, if sc.high_score { 0xe } else { 0xd }));
    if ui.add(ds::Plate::new(&label).width(260.0).height(switch_h)).clicked() {
        sc.high_score = !sc.high_score;
        act = Some(Act::Click);
    }
    act
}
