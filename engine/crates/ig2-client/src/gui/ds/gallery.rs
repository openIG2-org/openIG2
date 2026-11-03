//! The kit's gallery (ADR 0073 §7): every widget in every state on one 1920 × 1080 board at 100 % UI scale (a design
//! pixel a point), to set beside the design boards. `ig2-client --ui-gallery <out.png> [--size WxH]` draws it; the
//! hover and pressed states are forced ([`super::forced`]), the pointers are the modern set's frames.

use super::cursor::{Cursor, Set};
use super::*;
use egui::{Align, Color32, Pos2, Rect, Ui, Vec2};

const LONG_HU: &str = "Pixelenkénti megvilágítás és lágy árnyékok minden képernyőn";

/// A placed column of the board.
fn at(ctx: &egui::Context, id: &str, rect: Rect, add: impl FnOnce(&mut Ui)) {
    egui::Area::new(egui::Id::new(id)).fixed_pos(rect.min).fade_in(false).constrain(false).show(ctx, |ui| {
        ui.set_width(rect.width());
        ui.set_max_width(rect.width());
        add(ui);
    });
}

fn caption(ui: &mut Ui, s: &str) {
    ui.add_space(SPACE[1]);
    heading_label(ui, s);
}

/// The board.
pub fn page(ctx: &egui::Context, cursors: &[(Cursor, Vec<egui::TextureHandle>)]) {
    let k = tokens();
    let screen = ctx.screen_rect();
    let bg = ctx.layer_painter(egui::LayerId::background());
    vgrad(&bg, screen, 0, &[(0.0, hex(0x02060c)), (1.0, k.void)]);
    for i in 0..140u32 {
        let h = i.wrapping_mul(2654435761);
        let p = Pos2::new((h % 1920) as f32, ((h >> 11) % 1080) as f32);
        bg.circle_filled(p, 0.6 + (h >> 24) as f32 / 255.0, Color32::from_white_alpha(40 + (h >> 20 & 0x3f) as u8));
    }
    bg.text(Pos2::new(40.0, 28.0), egui::Align2::LEFT_TOP, "OPENIG2 DESIGN KIT", Font::PAGE_TITLE.id(), k.ink_silver);

    // Column 1: the main menu's panel and list rows.
    at(ctx, "g-menu", Rect::from_min_size(Pos2::new(40.0, 84.0), Vec2::new(440.0, 960.0)), |ui| {
        panel(ui, PanelKind::Deep, 0.8, 16.0, |ui| {
            ui.spacing_mut().item_spacing.y = SPACE[0];
            caption(ui, "Játék · menu");
            let play = [MenuEntry { text: "Tovább", sub: Some("Sokáig elment · 2357.03.11"), off: false }, MenuEntry::new("Új játék"), MenuEntry { text: "Betöltés", sub: None, off: true }, MenuEntry::new("Többjátékos és nagyon hosszú menüpont")];
            menu(ui, "g-menu-a", MenuStyle::MENU, &play, Some(0), Some(1));
            forced(true, false, || menu(ui, "g-menu-b", MenuStyle::MENU, &[MenuEntry::new("Beállítások (hover)")], None, None));
            forced(true, true, || menu(ui, "g-menu-c", MenuStyle::MENU, &[MenuEntry::new("Kilépés (pressed)")], None, None));
            separator(ui, 10.0);
            caption(ui, "Beállítások · rail");
            menu(ui, "g-rail", MenuStyle::RAIL, &[MenuEntry::new("Megjelenítés"), MenuEntry::new("Akadálymentesség")], Some(0), None);
            separator(ui, 10.0);
            caption(ui, "List rows");
            ui.add(ListRow::new("Standard romboló").value("2"));
            ui.add(ListRow::new("Kolonizáló").value("1").selected(true));
            forced(true, false, || ui.add(ListRow::new("Standard Tank (hover)").value("2")));
            ui.add(ListRow::new("Nem elérhető hajó").off(true));
            ui.add(ListRow::new("Fókuszált sor").focused(true));
            ui.add(ListRow::new(LONG_HU).value("12 345"));
            ui.add(ListRow::new("Magasabb sor, adatbetű").height(34.0).font(Font::DATA));
            separator(ui, 10.0);
            caption(ui, "Data table (raised panel, header strip)");
            let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), egui::Sense::hover());
            paint_panel(ui.painter(), strip, PanelKind::Raised, 1.0);
            header_strip(ui.painter(), strip);
            text(ui, strip.shrink2(Vec2::new(10.0, 0.0)), "Fleet 1", Font::HEADING.sized(16.0), tokens().ink);
            kv(ui, "Típus", "Romboló", Tone::Info, 130.0);
            kv(ui, "Erkölcs", "Lázadó", Tone::Enemy, 130.0);
            kv(ui, "Népesség nagyon hosszú címke", "Növekvő", Tone::Own, 130.0);
        });
    });

    // Column 2: a settings card and the controls in every state.
    at(ctx, "g-options", Rect::from_min_size(Pos2::new(520.0, 84.0), Vec2::new(560.0, 960.0)), |ui| {
        card(ui, Some("Képernyő"), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let mut seg = 1;
            let (_, _) = option_row(ui, "Megjelenítés", true, true, |ui| segmented(ui, &mut seg, &["Ablak", "Keret nélküli", "Teljes"], None));
            let mut on = true;
            option_row(ui, "Függőleges szinkron", false, true, |ui| toggle(ui, &mut on, "Ki", "Be"));
            let mut off = false;
            forced(true, false, || option_row(ui, "4:3 sávokkal (hover)", false, true, |ui| toggle(ui, &mut off, "Ki", "Be")));
            option_row(ui, "Felbontás (unavailable)", false, false, |ui| stepper(ui, "1920 × 1080", 128.0));
            let mut g = 0.5;
            option_row(ui, "Gamma", false, true, |ui| slider(ui, &mut g, "50%"));
            let mut sw = true;
            option_row(ui, "Modern kolónia (switch)", false, true, |ui| crate::gui::switch(ui, &mut sw));
            let mut sw2 = false;
            option_row(ui, LONG_HU, false, true, |ui| crate::gui::switch(ui, &mut sw2));
            option_row(ui, "Gyorsbetöltés (listening)", false, true, |ui| key_cap(ui, "Nyomj egy gombot", true));
            ui.add_space(SPACE[1]);
            let mut wide = 2;
            segmented(ui, &mut wide, &["Klasszikus", "Bővített", "Nagyon hosszú választás"], Some(ui.available_width()));
            ui.add_space(SPACE[1]);
            ui.horizontal(|ui| {
                ui.label("Drop-down");
                dropdown(ui, "g-dd", "Modern", 180.0, |ui| ui.label("Klasszikus"));
            });
        });
        ui.add_space(SPACE[2]);
        card(ui, Some("Text that fits"), |ui| {
            let k = tokens();
            let row = |ui: &mut Ui, label: &str, t: Text| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), egui::Sense::hover());
                let (a, b) = rect.split_left_right_at_x(rect.left() + 120.0);
                Text::new(label, Font::LABEL).color(k.ink_muted).paint(ui, a);
                ui.painter().rect_stroke(b, 2, egui::Stroke::new(1.0f32, k.rim), egui::StrokeKind::Inside);
                t.paint(ui, b.shrink2(Vec2::new(6.0, 0.0)));
            };
            row(ui, "Fits", Text::new("Gamma", Font::BODY));
            row(ui, "Elided", Text::new(LONG_HU, Font::BODY));
            row(ui, "Shrunk", Text::new("Textúra-felskálázás négyszeres", Font::BODY).shrink(12.0));
            row(ui, "Right", Text::new("12 345 $", Font::DATA).right().color(k.info));
            row(ui, "Readout", Text::new("2327 : 10 : 23", Font::READOUT.sized(20.0)).color(k.lcd_ink));
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), egui::Sense::hover());
            ui.painter().rect_stroke(rect, 2, egui::Stroke::new(1.0f32, k.rim), egui::StrokeKind::Inside);
            Text::new("Keret nélküli ablak az asztal felbontásán: gyors váltás más programokra, a játék mégis kitölti a képernyőt, és a kép éles marad.", Font::BODY.sized(15.0)).color(k.ink_silver).lines(2).paint(ui, rect.shrink(4.0));
        });
    });

    let frame = Rect::from_min_size(Pos2::new(1100.0, 72.0), Vec2::new(800.0, 984.0));
    paint_panel(&ctx.layer_painter(egui::LayerId::background()), frame, PanelKind::Frame, 0.6);

    // Column 3: plates, tabs, keys, readouts, labels, pointers.
    at(ctx, "g-parts", Rect::from_min_size(Pos2::new(1120.0, 84.0), Vec2::new(760.0, 960.0)), |ui| {
        panel(ui, PanelKind::Glass, 1.0, 16.0, |ui| {
            caption(ui, "Plates");
            ui.horizontal(|ui| {
                ui.add(Plate::new("Vissza").width(150.0));
                forced(true, false, || ui.add(Plate::new("Hover").width(150.0)));
                forced(true, true, || ui.add(Plate::new("Pressed").width(150.0)));
                ui.add(Plate::new("Alkalmaz").primary(true).width(150.0));
            });
            ui.add_space(SPACE[1]);
            ui.horizontal(|ui| {
                ui.add_enabled(false, Plate::new("Disabled").width(150.0));
                ui.add(Plate::new("Focused").width(150.0).focused(true));
                ui.add(Plate::new("Mentés").sub("2357.03.11").width(150.0));
                ui.add(Plate::new("Nagyon hosszú gombfelirat").width(150.0).height(44.0));
            });
            caption(ui, "Tabs");
            tabs(ui, "g-tabs", &["Birodalmak", "Bolygók", "Flották", "Kémek", "Egy nagyon hosszú fül"], 1);
            caption(ui, "Keys, round buttons");
            ui.horizontal(|ui| {
                key_group(ui, |ui| {
                    ui.add(IconKey::icon(Icon::Pause).on(true));
                    ui.add(IconKey::icon(Icon::Play));
                    forced(true, false, || ui.add(IconKey::icon(Icon::Fast)));
                    ui.add(IconKey::icon(Icon::Fastest));
                });
                key_group(ui, |ui| {
                    for i in [Icon::Menu, Icon::Plus, Icon::Minus, Icon::Check, Icon::Close] {
                        ui.add(IconKey::icon(i).tip("Tip"));
                    }
                    ui.add_enabled(false, IconKey::icon(Icon::ChevronDown));
                    ui.add(IconKey::icon(Icon::ChevronUp).size(Vec2::new(28.0, 28.0)));
                    let ring = |p: &egui::Painter, r: Rect, c: Color32| _ = p.circle_stroke(r.center(), r.width() / 2.5, egui::Stroke::new(2.0f32, c));
                    ui.add(IconKey::new(Glyph::Custom(&ring)));
                });
                round_button(ui, Glyph::Text("ÉP"), false, false);
                round_button(ui, Glyph::Text("ÉP"), false, true);
                round_button(ui, Glyph::Icon(Icon::Close), true, false);
            });
            caption(ui, "Readouts, bars");
            ui.horizontal(|ui| {
                lcd(ui, "$109090", 22.0, 7);
                lcd(ui, "2327 : 10 : 23", 22.0, 7);
                let r = lcd(ui, "64%", 18.0, 3);
                tip(r, "A rich tooltip wraps at 320 points and stops after eight lines.");
            });
            ui.horizontal(|ui| {
                bar(ui, 180.0, 6.0, 0.64, tokens().lcd_ink);
                bar(ui, 180.0, 10.0, 0.4, tokens().own);
                bar(ui, 180.0, 10.0, 0.85, tokens().enemy);
                bar(ui, 120.0, 8.0, 0.25, tokens().plate_high);
            });
            caption(ui, "Badges, title plate");
            ui.horizontal(|ui| {
                badge(ui, "Godan Királyság", Tone::Own, false, 200.0);
                badge(ui, "Kra'hen", Tone::Enemy, false, 200.0);
                badge(ui, "Semleges", Tone::Neutral, false, 200.0);
                badge(ui, "Szövetséges", Tone::Own, true, 200.0);
                badge(ui, "Háború", Tone::Enemy, true, 200.0);
                badge(ui, "Figyelem", Tone::Warn, true, 120.0);
            });
            ui.add_space(SPACE[1]);
            title_plate(ui, "Kolónia: Magellán", 420.0);
            caption(ui, "Confirm (frame panel behind)");
            confirm(ui.ctx(), "g-confirm", false, "", "", "", "");
            confirm_box(ui, 520.0, "Kilépsz a játékból?", "A nem mentett haladás elvész. Mentsd el a játékot a kilépés előtt, ha később folytatni szeretnéd.", "Kilépés", "Mégse");
            caption(ui, "Pointers");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for (c, frames) in cursors {
                    let (rect, r) = ui.allocate_exact_size(Vec2::new(44.0, 44.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4, tokens().glass_deep);
                    let tex = &frames[frames.len() / 3];
                    ui.painter().image(tex.id(), Rect::from_center_size(rect.center(), Vec2::splat(cursor::BOX)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    r.on_hover_text(format!("{c:?}"));
                }
            });
        });
    });
    let _ = Align::Center;
}

/// The pointers' frames as egui textures.
fn cursor_textures(ctx: &egui::Context) -> Vec<(Cursor, Vec<egui::TextureHandle>)> {
    let n = Set::PIXELS;
    Cursor::ALL
        .iter()
        .map(|&c| (c, (0..c.frames()).map(|i| ctx.load_texture(format!("cursor-{c:?}-{i}"), egui::ColorImage::from_rgba_unmultiplied([n, n], &c.rgba(i, n, &Tokens::DEEP_SPACE)), egui::TextureOptions::LINEAR)).collect()))
        .collect()
}

/// `--ui-gallery <out.png> [--size WxH]`: the board at `size` pixels (1920 × 1080 by default).
pub fn snapshot(args: &[String]) -> Result<(), String> {
    let out = args.first().ok_or("usage: --ui-gallery <out.png> [--size WxH]")?;
    let size = match args.iter().position(|a| a == "--size").and_then(|i| args.get(i + 1)) {
        Some(s) => s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))).ok_or("--size WxH")?,
        None => (1920, 1080),
    };
    set_motion(false);
    let mut r = crate::render::Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb);
    let style = crate::gui::style(&Theme::modern());
    let cursors = cursor_textures(&gui.ctx);
    for _ in 0..3 {
        gui.run(size, &style, |ctx| page(ctx, &cursors));
    }
    let rgba = r.draw_to_image_over(&crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]), size, 0.0, |d, q, t| gui.paint(d, q, t));
    crate::assets::write_png(std::path::Path::new(out), size.0, size.1, &rgba)?;
    println!("{out}: {}x{}", size.0, size.1);
    Ok(())
}
