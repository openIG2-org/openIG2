//! Ours (ADR 0038 §6, the boards' look ADR 0078): the replays list as a modern page over the main menu's scene, on the
//! save and load board's layout: the recorded games' table at the left (started, mode, size; newest first), the
//! selected replay's card with Watch at the right, the footer's hint and Back. A double click or Watch plays the
//! replay ([`crate::replay_viewer::ListAct::Open`]); Back, Esc and Enter close the list as the classic panel's.

use crate::gui::ds::{self, Font, Text};
use crate::replay_viewer::{ListAct, ReplayList};
use crate::ui::UiTexts;

/// The board's size (points at 1920 × 1080), as the save and load page's.
const PAGE: [f32; 2] = [1440.0, 880.0];
/// The right column's width.
const ASIDE: f32 = 440.0;

impl ReplayList {
    /// One frame of the page; gives what was asked for.
    pub fn modern(&mut self, ctx: &egui::Context, texts: &UiTexts) -> Option<ListAct> {
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let k = ds::tokens();
        let mut act = None;
        crate::gui::board_at(ctx, "replays", egui::Vec2::from(PAGE), 0.55, 12.0, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let w = ui.available_width();
            Text::new(&tr("Replays"), Font::PAGE_TITLE).color(k.ink_silver).show(ui, egui::vec2(w, 36.0));
            ui.add_space(20.0);
            let body = ui.available_height() - 20.0 - 64.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.vertical(|ui| {
                    ui.set_width(w - ASIDE - 32.0);
                    ui.set_height(body);
                    act = act.take().or(self.modern_table(ui, texts, body));
                });
                ui.add_space(32.0);
                ui.vertical(|ui| {
                    ui.set_width(ASIDE);
                    act = act.take().or(self.modern_aside(ui, texts));
                });
            });
            ui.add_space(20.0);
            // The footer: the folder's error or the hint, Back.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 64.0), egui::Sense::hover());
            ui.painter().hline(rect.x_range(), rect.top() + 0.5, egui::Stroke::new(1.0f32, k.rim));
            let row = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + 20.0), rect.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
            if child.add(ds::Plate::new(&tr("Back")).width(200.0)).clicked() {
                act = Some(ListAct::Close);
            }
            let (line, ink) = match &self.error {
                Some(e) => (e.clone(), k.enemy),
                None => (tr("A double click or Watch plays the replay; Esc goes back."), k.ink_silver),
            };
            let f = Font::BODY.sized(15.0).tracking(0.05);
            Text::new(&line, Font { line: 22.0, ..f }).color(ink).paint(ui, row.with_max_x(row.right() - 216.0));
        });
        act
    }

    /// The table: started (UTC), mode, size; a click selects, a double click plays.
    fn modern_table(&mut self, ui: &mut egui::Ui, texts: &UiTexts, body: f32) -> Option<ListAct> {
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let k = ds::tokens();
        let widths = [0.0, 220.0, 140.0];
        let (head, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::hover());
        let heads = [tr("Started (UTC)"), tr("Mode"), tr("Size")];
        ds::table_head(ui, &ds::table_cols(head, &widths), &heads.each_ref().map(String::as_str), &[false, false, true]);
        ui.add_space(4.0);
        if self.entries().is_empty() {
            ui.add_space(120.0);
            let w = ui.available_width();
            Text::new(&tr("No recorded games yet."), Font::HEADING).color(k.ink_silver).center().show(ui, egui::vec2(w, 30.0));
            return None;
        }
        let mut act = None;
        let lcd = Font { size: 16.0, line: 18.0, ..Font::READOUT }.tracking(0.08);
        let body_f = Font::BODY.sized(17.0);
        egui::ScrollArea::vertical().id_salt("replays").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(body - 32.0).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for i in 0..self.entries().len() {
                let e = &self.entries()[i];
                let mode = tr(if e.multiplayer { "Multiplayer" } else { "Single player" });
                let size = format!("{} KB", e.kb);
                let cells = [(e.when.as_str(), lcd, k.info), (mode.as_str(), body_f, k.ink), (size.as_str(), body_f, k.ink_muted)];
                let resp = ds::table_row(ui, 40.0, &widths, &cells, &[false, false, true], Some(self.selected == Some(i)));
                if resp.double_clicked() {
                    act = Some(ListAct::Open(e.path.clone()));
                } else if resp.clicked() {
                    self.selected = Some(i);
                }
            }
        });
        act
    }

    /// The right column: the selected replay's card and Watch.
    fn modern_aside(&mut self, ui: &mut egui::Ui, texts: &UiTexts) -> Option<ListAct> {
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let k = ds::tokens();
        let mut act = None;
        let sel = self.selected.and_then(|i| self.entries().get(i)).cloned();
        ds::panel(ui, ds::PanelKind::Glass, 0.88, 24.0, |ui| {
            ui.set_width(ASIDE - 48.0);
            ui.spacing_mut().item_spacing.y = 14.0;
            ds::heading_label(ui, &tr("Selected replay"));
            let w = ui.available_width();
            let title = Font::HEADING.sized(24.0).tracking(0.08);
            match &sel {
                Some(e) => {
                    Text::new(&e.when, Font { line: 30.0, ..title }).show(ui, egui::vec2(w, 30.0));
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ds::kv(ui, &tr("Mode"), &tr(if e.multiplayer { "Multiplayer" } else { "Single player" }), ds::Tone::Info, 158.0);
                    ds::kv(ui, &tr("Size"), &format!("{} KB", e.kb), ds::Tone::Info, 158.0);
                    ui.add_space(14.0);
                    if ui.add(ds::Plate::new(&tr("Watch")).primary(true).width(w)).clicked() {
                        act = Some(ListAct::Open(e.path.clone()));
                    }
                }
                None => {
                    Text::new(&tr("No replay selected"), Font { line: 30.0, ..title }).color(k.ink_muted).show(ui, egui::vec2(w, 30.0));
                    ui.add_enabled(false, ds::Plate::new(&tr("Watch")).width(w));
                }
            }
        });
        act
    }
}

/// `ig2-client --snapshot OUT --screen replays [--size WxH] [--mouse X,Y]`: the page over black on the replays folder
/// (`IG2_SLOT_DIR`'s `replays/`).
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    let l = crate::load(o)?;
    let mut r = crate::render::Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let hud = crate::ui::Hud::load(&mut r, &l.install, &l.tables);
    let mut gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb).install_fonts(&l.install);
    let mut s = ReplayList::open(None);
    let theme = crate::theme::Theme::modern();
    let style = crate::gui::style(&theme);
    let k = crate::gui::Gui::scale(o.size.1);
    let v = crate::font::Virtual::new([o.size.0 as f32, o.size.1 as f32]);
    gui.pointer(o.mouse.map(|m| v.px(m[0], m[1])), k);
    for _ in 0..40 {
        gui.run(o.size, &style, |ctx| {
            s.modern(ctx, &hud.texts);
        });
    }
    let rgba = r.draw_to_image_over(&crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]), o.size, 0.0, |d, q, t| gui.paint(d, q, t));
    crate::assets::write_snapshot(out, o, &rgba)?;
    println!("{}", out.display());
    Ok(())
}
