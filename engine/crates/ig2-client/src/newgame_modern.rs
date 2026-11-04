//! Ours (ADR 0078): the new game board: the campaign's race choice (`Mode_Fajvalasztas`, re/screens/mainscr.md §9.2)
//! as an egui page over the main menu's scene, in place of the three turning heads and the difficulty rings: the game
//! modes at the left (Campaign, Scenarios), the three races as round portraits, the picked race's panel with its
//! blurb (`fajinfo.txt`) and the difficulty, Back and Start. Start is `CampaignGameStart` for the race and level
//! ([`crate::mainscr::MainScr::start_campaign`]); Back and Esc close the choice as the original's Esc does.

use egui::{Color32, Pos2, Rect, Sense, Vec2};

use crate::gui::ds::{self, Font, Text, Weight};
use crate::menu_flow::Item;
use crate::ui::UiTexts;

/// The page's own state: the race (0..=2) and the level (the exe's: 0 easy, 1 hard, 2 normal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub race: usize,
    pub level: u8,
}

impl Default for State {
    fn default() -> Self {
        Self { race: 0, level: 2 }
    }
}

/// What a frame of the page asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Scenarios,
    Back,
    Start { race: u8, level: u8 },
    /// A race or a difficulty picked (the click's sound).
    Picked,
}

/// The campaign's races 0..=2 as race numbers (Solarian, Shinari, Kra'hen: `txt17` 1/6, 1/8, 1/10).
pub const RACES: [u8; 3] = [1, 3, 5];

/// The races' portraits: the diplomacy screen's 128 × 128 heads (`textures/diplomac/<race>a.bmp`).
pub fn load_heads(ctx: &egui::Context, install: &ig2_formats::Install) -> [Option<egui::TextureHandle>; 3] {
    RACES.map(|r| {
        let p = install.resolve_texture("textures/diplomac/", &format!("{r}a.bmp"))?;
        let img = crate::assets::Image::load(install, &p).ok()?;
        let pixels = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
        Some(ctx.load_texture(format!("ng-race-{r}"), pixels, egui::TextureOptions::LINEAR))
    })
}

/// The difficulty segments in their order on screen, as the exe's levels.
const LEVELS: [u8; 3] = [0, 2, 1];

/// The board's places (points at 1920 × 1080): the title, the modes' panel, the race row, the race panel.
const TITLE: [f32; 2] = [112.0, 132.0];
const NAV: [f32; 3] = [96.0, 252.0, 440.0];
const RACES_TOP: f32 = 340.0;
const RING: f32 = 228.0;
const RING_GAP: f32 = 44.0;
const SIDE: [f32; 2] = [1384.0, 440.0];

/// One frame of the page; `blurbs` = `fajinfo.txt`'s three in the font's bytes, `heads` the races' portraits.
pub fn page(ctx: &egui::Context, texts: &UiTexts, st: &mut State, blurbs: &[Vec<u8>; 3], heads: &[Option<egui::TextureHandle>; 3], opacity: f32) -> Option<Act> {
    let k = ds::tokens();
    let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
    let game = |b: &[u8]| ig2_formats::text::decode(b, texts.lang).trim().trim_matches('"').trim().to_string();
    let screen = ctx.screen_rect();
    let mut act = None;
    ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha((255.0 * 0.35 * opacity) as u8));
    // A narrow window keeps the board's margins in proportion; the middle row centres between the side panels.
    let margin = NAV[0].min(screen.width() * 0.05).max(16.0);
    let side_w = SIDE[1].min((screen.width() - 2.0 * margin) / 3.6);
    let at = |x: f32, y: f32| Pos2::new(screen.left() + x, screen.top() + y);
    let race_name = |r: usize| game(&texts.race(RACES[r]));
    // A short window (a large UI scale) moves the rows up with it.
    let ys = ds::squeeze(screen.height());
    let (title_y, nav_y, races_y) = (TITLE[1] * ys, NAV[1] * ys, RACES_TOP * ys);
    egui::Area::new(egui::Id::new("ng-title")).fixed_pos(at(margin + TITLE[0] - NAV[0], title_y)).fade_in(false).interactable(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        Text::new(&tr("New game"), Font { size: 34.0, line: 40.0, ..Font::PAGE_TITLE }).color(k.ink_silver).show(ui, Vec2::new(side_w * 2.0, 40.0));
    });
    // The modes: Campaign (here) and Scenarios, the hint under a rim line.
    egui::Area::new(egui::Id::new("ng-nav")).fixed_pos(at(margin, nav_y)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        let place = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 24)).show(ui, |ui| {
            ui.set_width(side_w - 32.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
            let labels = [tr(Item::Campaign.label()), tr(Item::Scenarios.label())];
            let entries: Vec<ds::MenuEntry> = labels.iter().map(|l| ds::MenuEntry::new(l)).collect();
            if ds::menu(ui, "ng-modes", ds::MenuStyle::MENU, &entries, Some(0), None) == Some(1) {
                act = Some(Act::Scenarios);
            }
            ui.add_space(14.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().hline((r.left() + 20.0)..=(r.right() - 20.0), r.center().y, egui::Stroke::new(1.0f32, k.rim));
            ui.add_space(14.0);
            let f = Font { size: 15.0, line: 24.0, weight: Weight::Regular, tracking: 0.06, upper: false, lcd: false };
            let w = ui.available_width() - 40.0;
            let hint = tr("Pick an empire, then the difficulty. The campaign plays the chosen race's story.");
            let h = Text::new(&hint, f).lines(4).layout(ui.ctx(), w).galley.rect.height().max(24.0);
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                Text::new(&hint, f).color(k.ink_muted).lines(4).show(ui, Vec2::new(w, h));
            });
        });
        ds::behind(ui, place, |p| ds::paint_panel(p, r.response.rect, ds::PanelKind::Deep, 0.80));
    });
    // The races: round portraits on a metal ring, the name under each.
    let mid_l = margin + side_w;
    let mid_r = screen.width() - margin - side_w;
    let ring = RING.min((mid_r - mid_l - 4.0 * RING_GAP) / 3.0).max(96.0);
    let row_w = 3.0 * ring + 2.0 * RING_GAP;
    let x0 = (mid_l + mid_r - row_w) / 2.0;
    egui::Area::new(egui::Id::new("ng-races")).fixed_pos(at(x0, races_y)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        ui.spacing_mut().item_spacing = Vec2::new(RING_GAP, 0.0);
        ui.horizontal(|ui| {
            for (r, head) in heads.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ring, ring + 18.0 + 22.0), Sense::click());
                let sel = st.race == r;
                let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
                let disc = Rect::from_min_size(rect.min, Vec2::splat(ring));
                let p = ui.painter();
                p.circle_filled(disc.center() + Vec2::new(0.0, 6.0), ring / 2.0 + 4.0, Color32::from_black_alpha(110));
                p.circle_filled(disc.center(), ring / 2.0, k.metal_shadow);
                let rim = if sel { k.metal_shine } else { k.metal };
                p.circle_stroke(disc.center(), ring / 2.0 - 1.0, egui::Stroke::new(2.0f32, rim));
                if sel || hover > 0.0 {
                    p.circle_stroke(disc.center(), ring / 2.0 + 1.0, egui::Stroke::new(2.0f32, k.rim_bright.gamma_multiply(if sel { 1.0 } else { hover })));
                }
                if let Some(t) = head {
                    let img = disc.shrink(6.0);
                    p.add(round_image(t.id(), img, Color32::WHITE));
                }
                let f = Font { size: 17.0, line: 22.0, weight: Weight::SemiBold, tracking: 0.16, upper: true, lcd: false };
                let name = Rect::from_min_size(Pos2::new(rect.left() - RING_GAP / 2.0, disc.bottom() + 18.0), Vec2::new(ring + RING_GAP, 22.0));
                Text::new(&race_name(r), f).center().color(if sel || hover > 0.5 { k.ink } else { k.ink_muted }).paint(ui, name);
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    st.race = r;
                    act = Some(Act::Picked);
                }
            }
        });
    });
    // The race's panel: its empire, the blurb, the difficulty.
    let side = egui::Area::new(egui::Id::new("ng-side")).fixed_pos(at(screen.width() - margin - side_w, nav_y)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        let place = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new().inner_margin(24).show(ui, |ui| {
            let w = side_w - 48.0;
            ui.set_width(w);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 16.0);
            let empire = texts.get(0x11, 1, RACES[st.race] as u16 + 33);
            Text::new(&game(&empire), Font::HEADING.upper(true)).color(k.ink).shrink(15.0).show(ui, Vec2::new(w, 26.0));
            let blurb = game(&blurbs[st.race]);
            let body = Font { size: 17.0, line: 26.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
            let h = Text::new(&blurb, body).lines(12).layout(ui.ctx(), w).galley.rect.height().max(26.0);
            Text::new(&blurb, body).color(k.ink).lines(12).show(ui, Vec2::new(w, h));
            let (sep, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
            ui.painter().hline(sep.x_range(), sep.center().y, egui::Stroke::new(1.0f32, k.rim));
            Text::new(&tr("Difficulty"), Font::LABEL).color(k.ink_muted).show(ui, Vec2::new(w, 18.0));
            let labels: Vec<String> = LEVELS.iter().map(|&l| tr(Item::Level(l).label())).collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let mut i = LEVELS.iter().position(|&l| l == st.level).unwrap_or(1);
            if ds::segmented_plates(ui, &mut i, &refs, Some(w), 44.0, Font { size: 17.0, line: 20.0, weight: Weight::Medium, tracking: 0.12, upper: false, lcd: false }).changed() {
                st.level = LEVELS[i];
                act = Some(Act::Picked);
            }
        });
        ds::behind(ui, place, |p| ds::paint_panel(p, r.response.rect, ds::PanelKind::Glass, 0.86));
    });
    // Back and Start at the bottom right.
    let (back, start) = (tr("Back"), tr("Start"));
    // The plates under the race's panel when the window is too short for their place.
    let foot_y = (screen.height() - 72.0 - 44.0).max(side.response.rect.bottom() - screen.top() + 16.0);
    let foot = Rect::from_min_max(at(screen.width() - margin - (side_w + 16.0).min(456.0), foot_y), at(screen.width() - margin, foot_y + 44.0));
    egui::Area::new(egui::Id::new("ng-foot")).fixed_pos(foot.min).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(opacity);
        ui.set_width(foot.width());
        ui.spacing_mut().item_spacing = Vec2::new(16.0, 0.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(ds::Plate::new(&start).primary(true).width(240.0f32.min((foot.width() - 16.0) * 0.545))).clicked() {
                act = Some(Act::Start { race: st.race as u8, level: st.level });
            }
            if ui.add(ds::Plate::new(&back).width(200.0f32.min((foot.width() - 16.0) * 0.455))).clicked() {
                act = Some(Act::Back);
            }
        });
    });
    // The right button anywhere off the page's controls is Back (ADR 0059), as the original's Esc.
    if act.is_none() && ctx.input(|i| i.pointer.secondary_clicked()) && !ctx.is_pointer_over_area() {
        act = Some(Act::Back);
    }
    act
}

/// A texture in a circle: a fan of triangles with the picture's coordinates.
fn round_image(tex: egui::TextureId, rect: Rect, tint: Color32) -> egui::Shape {
    let mut m = egui::Mesh::with_texture(tex);
    let (c, r) = (rect.center(), rect.width() / 2.0);
    m.colored_vertex(c, tint);
    m.vertices[0].uv = egui::pos2(0.5, 0.5);
    const N: u32 = 64;
    for i in 0..=N {
        let a = i as f32 / N as f32 * std::f32::consts::TAU;
        let (s, co) = a.sin_cos();
        // The heads fill about the middle 70 % of their pictures.
        m.vertices.push(egui::epaint::Vertex { pos: c + Vec2::new(co, s) * r, uv: egui::pos2(0.5 + co * 0.36, 0.52 + s * 0.36), color: tint });
        if i > 0 {
            m.add_triangle(0, i, i + 1);
        }
    }
    egui::Shape::mesh(m)
}
