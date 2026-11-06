//! Ours (ADR 0078): the end-of-game board: the statistics screen as an egui page over the main menu's scene: the
//! outcome in large letters, the mode, days and end date under it; the empires as a table (the empire's colour
//! stripe, its race's head, the values in the LCD face, the score in the LCD's yellow, this machine's row lit, a
//! defeated empire muted); the graph with its value tabs, grid, day marks and the lines' names at their ends; Back or
//! Continue at the bottom right. Opened from the menu, the history's list in the same table look.

use egui::{Color32, Pos2, Rect, Sense, Vec2};

use super::{day_of, mode_name, Match, Outcome, StatsScreen, METRICS};
use crate::gui::ds::{self, Font, Text, Weight};
use crate::ui::UiTexts;

/// The races' heads (`textures/main/fejek3/<race>a.bmp`, as the multiplayer room's).
pub fn load_heads(ctx: &egui::Context, install: &ig2_formats::Install) -> Vec<Option<egui::TextureHandle>> {
    (1..=8)
        .map(|k| {
            let p = install.resolve_texture("textures/main/fejek3/", &format!("{k}a.bmp"))?;
            let img = crate::assets::Image::load(install, &p).ok()?;
            let pixels = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
            Some(ctx.load_texture(format!("stats-race-{k}"), pixels, egui::TextureOptions::LINEAR))
        })
        .collect()
}

/// The board's places (points at 1920 × 1080): the margin, the title's top, the table's top, the graph's height.
const MARGIN: f32 = 140.0;
const TITLE_Y: f32 = 64.0;
const TABLE_Y: f32 = 196.0;
const GRAPH_H: f32 = 372.0;
/// The table's value columns: 96 wide, 12 apart.
const COL_W: f32 = 96.0;
const COL_GAP: f32 = 12.0;

/// 412345 → "412k", 18400000 → "18.4M".
fn short(v: i64) -> String {
    match v.abs() {
        a if a >= 10_000_000 => format!("{:.1}M", v as f64 / 1e6),
        a if a >= 10_000 => format!("{}k", v / 1000),
        _ => v.to_string(),
    }
}

impl StatsScreen {
    /// One frame of the page; `palette` the game's colours (an empire's is `palette[15 + e]`), `heads` by race − 1.
    /// `true` when the screen closes.
    pub fn modern(&mut self, ctx: &egui::Context, texts: &UiTexts, palette: &[[f32; 3]], heads: &[Option<egui::TextureHandle>], opacity: f32) -> bool {
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let game = |b: &[u8]| ig2_formats::text::decode(b, texts.lang);
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha((173.0 * opacity) as u8));
        let margin = MARGIN.min(screen.width() * 0.073).max(16.0);
        // A short window (a large UI scale) moves the title and table up and lowers the rows.
        let ys = ds::squeeze(screen.height());
        let (title_y, table_y) = (TITLE_Y * ys, TABLE_Y * ys);
        let row_h = (52.0 * ys).max(40.0);
        let w = screen.width() - 2.0 * margin;
        let mut close = false;
        let shown = self.shown;
        // The title: the outcome (or the history's) and the line under it.
        let (title, sub, colour) = match self.shown() {
            Some(m) => {
                let [y, mo, d] = m.date;
                let fill = |pat: &str, v: String| game(&crate::tr::fill(texts.lang, pat, &[v.as_bytes()]));
                let sub = format!("{}  ·  {}  ·  {}", tr(mode_name(m.mode)), fill("{} game days", m.days.to_string()), fill("ended {}", format!("{y}.{mo:02}.{d:02}")));
                let c = match m.outcome {
                    Outcome::Won => k.own,
                    Outcome::Lost => k.enemy,
                    Outcome::Ended => k.ink_silver,
                };
                (tr(m.title()), sub, c)
            }
            None => (tr("Match history"), self.error.clone().unwrap_or_else(|| if self.list.is_empty() { tr("No finished games yet.") } else { String::new() }), k.ink_silver),
        };
        egui::Area::new(egui::Id::new("st-title")).fixed_pos(Pos2::new(screen.left() + margin, screen.top() + title_y)).fade_in(false).interactable(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 10.0);
            Text::new(&title, Font { size: 48.0, line: 56.0, weight: Weight::SemiBold, tracking: 0.3, upper: true, lcd: false }).color(colour).shrink(28.0).show(ui, Vec2::new(w, 56.0));
            Text::new(&sub, Font { size: 17.0, line: 24.0, weight: Weight::Medium, tracking: 0.1, upper: false, lcd: false }).color(k.ink_silver).show(ui, Vec2::new(w, 24.0));
        });
        let foot_y = screen.bottom() - 40.0 - 44.0;
        match self.shown() {
            Some(m) => {
                let m = m.clone();
                let table_h = self.modern_table(ctx, texts, palette, heads, &m, Rect::from_min_size(Pos2::new(screen.left() + margin, screen.top() + table_y), Vec2::new(w, row_h)), opacity);
                let top = screen.top() + table_y + table_h + 12.0;
                let gh = GRAPH_H.min(foot_y - 16.0 - top).max(160.0);
                self.modern_graph(ctx, texts, palette, &m, Rect::from_min_size(Pos2::new(screen.left() + margin, top), Vec2::new(w, gh)), opacity);
            }
            None => {
                let r = Rect::from_min_max(Pos2::new(screen.left() + margin, screen.top() + table_y), Pos2::new(screen.right() - margin, foot_y - 24.0));
                self.list_table(ctx, texts, r, opacity);
            }
        }
        // The command plate at the bottom right.
        let label = if shown.is_some() && self.from_list { "Back" } else if self.from_list { "Main menu" } else { "Continue" };
        let label = tr(label);
        egui::Area::new(egui::Id::new("st-foot")).fixed_pos(Pos2::new(screen.right() - margin - 220.0, foot_y)).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            if ui.add(ds::Plate::new(&label).width(220.0)).clicked() {
                close = self.back().is_some();
            }
        });
        // The right button off the page's controls is Back (ADR 0059).
        if ctx.input(|i| i.pointer.secondary_clicked()) && !ctx.is_pointer_over_area() {
            close = self.back().is_some();
        }
        close
    }

    /// The empires' table in `at` (its top and width, the rows' pitch its height); gives its height.
    #[allow(clippy::too_many_arguments)]
    fn modern_table(&self, ctx: &egui::Context, texts: &UiTexts, palette: &[[f32; 3]], heads: &[Option<egui::TextureHandle>], m: &Match, at: Rect, opacity: f32) -> f32 {
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let Some(last) = m.samples.last() else { return 0.0 };
        let cols = ["Planets", "Pop.", "Credits", "Ships", "Built", "Lost", "Techs", "Wins", "Score"];
        let pitch = at.height();
        let h = 16.0 + 32.0 + m.empires.len() as f32 * pitch + 20.0;
        let panel = Rect::from_min_size(at.min, Vec2::new(at.width(), h));
        let heads_h = (pitch - 16.0).min(36.0);
        egui::Area::new(egui::Id::new("st-table")).fixed_pos(panel.min).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            ds::paint_panel(ui.painter(), panel, ds::PanelKind::Deep, 0.90);
            let inner = Rect::from_min_max(panel.min + Vec2::new(24.0, 16.0), panel.max - Vec2::new(24.0, 20.0));
            // The value columns from the right; a narrow window narrows them.
            let cw = COL_W.min((inner.width() - 300.0) / 9.0 - COL_GAP).max(40.0);
            let col_x = |c: usize| inner.right() - 12.0 - (9 - c) as f32 * (cw + COL_GAP) + COL_GAP;
            let name_x = inner.left() + 6.0 + 12.0 + 40.0 + 12.0;
            let label = Font::LABEL.sized(15.0).weight(Weight::Medium);
            let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 32.0));
            Text::new(&tr("Empire"), label).color(k.ink_muted).paint(ui, Rect::from_min_max(Pos2::new(name_x, head.top()), Pos2::new(col_x(0) - COL_GAP, head.bottom())));
            for (c, name) in cols.iter().enumerate() {
                Text::new(&tr(name), label).color(k.ink_muted).right().shrink(11.0).paint(ui, Rect::from_min_size(Pos2::new(col_x(c), head.top()), Vec2::new(cw, 32.0)));
            }
            let num = Font { size: 18.0, line: 20.0, weight: Weight::Regular, tracking: 0.06, upper: false, lcd: true };
            let pts = Font { size: 20.0, line: 22.0, weight: Weight::Regular, tracking: 0.08, upper: false, lcd: true };
            for (n, e) in m.empires.iter().enumerate() {
                let i = e.empire as usize - 1;
                let r = &last.rows[i];
                let row = Rect::from_min_size(Pos2::new(inner.left(), head.bottom() + 4.0 + n as f32 * pitch), Vec2::new(inner.width(), pitch - 4.0));
                let me = e.empire == m.empire;
                let out = r.planets == 0 && r.ships == 0;
                let p = ui.painter();
                if me {
                    ds::hgrad(p, row, ds::RADIUS_PLATE, &[(0.0, Color32::from_white_alpha(36)), (1.0, Color32::from_white_alpha(5))]);
                    ds::rim(p, row, ds::RADIUS_PLATE, k.ink_silver);
                }
                let c = palette.get(15 + e.empire as usize).map_or(k.ink, |c| crate::gui::ink(*c));
                p.rect_filled(Rect::from_min_size(row.min, Vec2::new(6.0, row.height())), egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }, c);
                if let Some(Some(t)) = heads.get(e.race.wrapping_sub(1) as usize) {
                    let img = Rect::from_center_size(Pos2::new(row.left() + 6.0 + 12.0 + 20.0, row.center().y), Vec2::splat(heads_h));
                    p.image(t.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                let mut name = game_name(texts, &e.name);
                if name.is_empty() {
                    // A record without the empire's name: its race's empire (`txt17` 1/33 + race).
                    name = game_name(texts, &texts.get(0x11, 1, e.race as u16 + 33));
                }
                if me {
                    name.push_str(&tr(" (you)"));
                }
                let ink = if out { k.ink_muted } else { k.ink };
                Text::new(&name, Font::BODY.weight(Weight::Medium)).color(ink).paint(ui, Rect::from_min_max(Pos2::new(name_x, row.top()), Pos2::new(col_x(0) - COL_GAP, row.bottom())));
                let vals = [r.planets as i64, r.population as i64, r.credits, r.ships as i64, e.built as i64, e.lost as i64, r.techs as i64, r.battles as i64];
                for (c, v) in vals.iter().enumerate() {
                    let s = if c == 2 { format!("${}", short(*v)) } else { short(*v) };
                    mono(ui, Rect::from_min_size(Pos2::new(col_x(c), row.top()), Vec2::new(cw, row.height())), &s, num, if out { k.ink_muted } else { k.info });
                }
                Text::new(&short(r.score as i64), pts).color(k.lcd_ink).right().shrink(13.0).paint(ui, Rect::from_min_size(Pos2::new(col_x(8), row.top()), Vec2::new(cw, row.height())));
            }
        });
        h
    }

    /// The graph's panel in `r`: the value tabs, the grid and marks, a line per empire.
    fn modern_graph(&mut self, ctx: &egui::Context, texts: &UiTexts, palette: &[[f32; 3]], m: &Match, r: Rect, opacity: f32) {
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let tabs = ["Planets", "Population", "Credits", "Ships", "Techs", "Score"];
        // The tabs in the board's order, as the classic screen's metrics.
        let metric_of = |t: usize| METRICS.iter().position(|(n, _)| *n == tabs[t]).unwrap_or(0);
        let tab = tabs.iter().position(|&n| METRICS[self.metric].0 == n).unwrap_or(0);
        egui::Area::new(egui::Id::new("st-graph")).fixed_pos(r.min).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            ds::paint_panel(ui.painter(), r, ds::PanelKind::Glass, 0.90);
            let inner = r.shrink2(Vec2::new(24.0, 20.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            let labels: Vec<String> = tabs.iter().map(|t| tr(t)).collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let mut t = tab;
            let f = Font { size: 16.0, line: 20.0, weight: Weight::Medium, tracking: 0.1, upper: false, lcd: false };
            child.horizontal(|ui| {
                if ds::segmented_plates(ui, &mut t, &refs, None, 40.0, f).changed() {
                    self.metric = metric_of(t);
                }
            });
            let plot_top = child.min_rect().bottom() + 12.0;
            let value = METRICS[self.metric].1;
            let max = m.samples.iter().flat_map(|s| m.empires.iter().map(move |e| value(&s.rows[e.empire as usize - 1]))).fold(0.0f64, f64::max);
            // A round top: 1, 2 or 5 times a power of ten, in three steps.
            let step = nice(max / 3.0);
            let top = (step * 3.0).max(1.0);
            let plot = Rect::from_min_max(Pos2::new(inner.left() + 60.0, plot_top + 8.0), Pos2::new(inner.right() - 116.0, inner.bottom() - 26.0));
            let p = ui.painter();
            let lcd14 = Font { size: 14.0, line: 16.0, weight: Weight::Regular, tracking: 0.0, upper: false, lcd: true };
            for g in 0..=3 {
                let y = plot.bottom() - plot.height() * g as f32 / 3.0;
                p.hline(plot.x_range(), y, egui::Stroke::new(1.0f32, k.rim.gamma_multiply(0.6)));
                mono(ui, Rect::from_min_max(Pos2::new(inner.left(), y - 10.0), Pos2::new(plot.left() - 10.0, y + 10.0)), &format!("{:.0}", step * g as f64), lcd14, k.ink_muted);
            }
            let day = |d: u32| game_text(texts, &crate::tr::fill(texts.lang, "day {}", &[d.to_string().as_bytes()]));
            let marks = Font { size: 13.0, line: 18.0, weight: Weight::Regular, tracking: 0.08, upper: false, lcd: false };
            let my = Rect::from_min_max(Pos2::new(plot.left(), plot.bottom() + 6.0), Pos2::new(plot.right(), plot.bottom() + 24.0));
            Text::new(&day(0), marks).color(k.ink_muted).paint(ui, my);
            Text::new(&day(m.days / 2), marks).color(k.ink_muted).center().paint(ui, my);
            Text::new(&day(m.days), marks).color(k.ink_muted).right().paint(ui, my);
            let n = m.samples.len();
            let first = m.samples.first().map_or(0, |s| s.ido);
            let span = m.samples.last().map_or(0, |s| s.ido) - first;
            let at = |s: usize, v: f64| {
                let t = if span > 0 { (m.samples[s].ido - first) as f32 / span as f32 } else if n > 1 { s as f32 / (n - 1) as f32 } else { 1.0 };
                Pos2::new(plot.left() + t * plot.width(), plot.bottom() - (v / top) as f32 * plot.height())
            };
            // The others first, this machine's on top, thicker; the name at the end of its line and of the line that
            // rose highest of the others.
            let colour = |e: u8| palette.get(15 + e as usize).map_or(k.ink, |c| crate::gui::ink(*c));
            let peak = m.empires.iter().filter(|e| e.empire != m.empire).max_by(|a, b| {
                let pk = |e: &super::Empire| m.samples.iter().map(|s| value(&s.rows[e.empire as usize - 1])).fold(0.0, f64::max);
                pk(a).total_cmp(&pk(b))
            });
            let mut order: Vec<&super::Empire> = m.empires.iter().filter(|e| e.empire != m.empire).collect();
            order.extend(m.empires.iter().filter(|e| e.empire == m.empire));
            for e in order {
                let i = e.empire as usize - 1;
                let me = e.empire == m.empire;
                let pts: Vec<Pos2> = (0..n).map(|s| at(s, value(&m.samples[s].rows[i]))).collect();
                let c = colour(e.empire);
                p.add(egui::Shape::line(pts.clone(), egui::Stroke::new(if me { 4.0f32 } else { 2.0 }, c)));
                if let Some(end) = pts.last().filter(|_| me || peak.is_some_and(|pk| pk.empire == e.empire)) {
                    let mut name = race_short(texts, e);
                    if me {
                        name.push_str(&tr(" (you)"));
                    }
                    Text::new(&name, Font { size: 14.0, line: 18.0, weight: Weight::Regular, tracking: 0.07, upper: false, lcd: false }).color(if me { k.ink } else { c }).paint(ui, Rect::from_min_size(Pos2::new(end.x + 12.0, end.y - 9.0), Vec2::new(110.0, 18.0)));
                }
            }
        });
    }

    /// The history's list as the table's rows: date, mode, result, days, empires; a click opens the match.
    fn list_table(&mut self, ctx: &egui::Context, texts: &UiTexts, r: Rect, opacity: f32) {
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let mut open = None;
        egui::Area::new(egui::Id::new("st-list")).fixed_pos(r.min).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            ds::paint_panel(ui.painter(), r, ds::PanelKind::Deep, 0.90);
            let inner = Rect::from_min_max(r.min + Vec2::new(24.0, 16.0), r.max - Vec2::new(24.0, 20.0));
            let xs = [0.0, 0.16, 0.32, 0.46, 0.56];
            let heads = ["Date", "Mode", "Result", "Days", "Empires"];
            let label = Font::LABEL.sized(15.0).weight(Weight::Medium);
            let col = |c: usize, top: f32, h: f32| Rect::from_min_max(Pos2::new(inner.left() + 18.0 + xs[c] * inner.width(), top), Pos2::new(if c + 1 < xs.len() { inner.left() + 6.0 + xs[c + 1] * inner.width() } else { inner.right() - 12.0 }, top + h));
            for (c, h) in heads.iter().enumerate() {
                Text::new(&tr(h), label).color(k.ink_muted).paint(ui, col(c, inner.top(), 32.0));
            }
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(inner.left(), inner.top() + 36.0), inner.max)));
            egui::ScrollArea::vertical().id_salt("st-list-scroll").auto_shrink([false; 2]).show(&mut child, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                for (n, m) in self.list.iter().enumerate() {
                    let (row, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::click());
                    let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
                    ds::row_back(ui, row, hover, false);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let cell = |c: usize| Rect::from_min_max(Pos2::new(row.left() + 18.0 + xs[c] * row.width(), row.top()), Pos2::new(if c + 1 < xs.len() { row.left() + 6.0 + xs[c + 1] * row.width() } else { row.right() - 12.0 }, row.bottom()));
                    let result = match m.outcome {
                        Outcome::Won => k.own,
                        Outcome::Lost => k.enemy,
                        Outcome::Ended => k.ink_silver,
                    };
                    let num = Font { size: 18.0, line: 20.0, weight: Weight::Regular, tracking: 0.06, upper: false, lcd: true };
                    Text::new(&day_of(m.when), num).color(k.info).paint(ui, cell(0));
                    Text::new(&tr(mode_name(m.mode)), Font::BODY).color(k.ink).paint(ui, cell(1));
                    Text::new(&tr(m.title()), Font::BODY.weight(Weight::SemiBold)).color(result).paint(ui, cell(2));
                    Text::new(&m.days.to_string(), num).color(k.info).paint(ui, cell(3));
                    let names: Vec<String> = m.empires.iter().map(|e| game_name(texts, &e.name)).collect();
                    Text::new(&names.join(", "), Font::BODY).color(k.ink_silver).paint(ui, cell(4));
                    if resp.clicked() {
                        open = Some(n);
                    }
                }
            });
        });
        if let Some(n) = open {
            self.shown = Some(n);
            self.metric = 0;
        }
    }
}

/// `ig2-client --snapshot OUT --screen stats --save S [--script S]`: the save's game as won by its human (the
/// modern page over black), driven by the script's pointer; `--history` the list of that one match.
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    use crate::script::Input;
    let l = crate::load(o)?;
    let mut world = crate::start_world(o, &l.install, l.save.as_ref(), &l.rules, &l.tech_defaults, &l.volatile)?;
    // `--ticks N`: the game played on first (for the graph's samples).
    world.set_speed(o.speed);
    for _ in 0..o.ticks {
        world.tick();
    }
    let mut r = crate::render::Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let hud = crate::ui::Hud::load(&mut r, &l.install, &l.tables);
    let empire = world.game.state.human_controlled().iter().position(|&h| h).unwrap_or(1) as u8;
    let m = Match::record(&world.game, ig2_sim::game::GameOver::Won, empire, false, [2331, 1, 2], 1_785_000_000);
    let mut st = StatsScreen::end(m);
    let mut gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb).install_fonts(&l.install);
    let heads = load_heads(&gui.ctx, &l.install);
    let k = crate::gui::Gui::scale(o.size.1);
    let v = crate::font::Virtual::new([o.size.0 as f32, o.size.1 as f32]);
    let script = o.script.as_deref().map(crate::script::Script::load).transpose()?;
    let (end, grabs) = script.as_ref().map_or((0, vec![0]), |sc| (sc.end(), sc.grabs.clone()));
    let mut mouse = o.mouse.unwrap_or([320.0, 240.0]);
    let theme = crate::theme::Theme::modern();
    let style = crate::gui::style(&theme);
    for t in 0..=end {
        let mut buttons = Vec::new();
        for e in script.iter().flat_map(|sc| sc.at(t)) {
            match &e.input {
                Input::Move(at) => mouse = *at,
                Input::Button { button: 1, down, at } => {
                    mouse = *at;
                    buttons.push(*down);
                }
                _ => {}
            }
        }
        gui.pointer(Some(v.px(mouse[0], mouse[1])), k);
        let mut run = |g: &mut crate::gui::Gui| g.run(o.size, &style, |ctx| if st.modern(ctx, &hud.texts, &hud.palette, &heads, 1.0) { println!("t {t}: close") });
        // egui sizes its areas on their first frame and shows them on the next.
        if buttons.is_empty() || t == 0 {
            run(&mut gui);
        }
        for down in buttons {
            gui.button(0, down);
            run(&mut gui);
        }
        if grabs.contains(&t) {
            let frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
            let rgba = r.draw_to_image_over(&frame, o.size, 0.0, |d, q, tex| gui.paint(d, q, tex));
            let path = if script.is_some() { crate::script::grab_path(out, t) } else { out.to_path_buf() };
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {t} | metric {} samples {}", path.display(), st.metric, st.shown().map_or(0, |m| m.samples.len()));
        }
    }
    Ok(())
}

/// `s` upright in the LCD face, right-aligned and centred in `r` (the board's `.num`; the kit's LCD text slants).
fn mono(ui: &egui::Ui, r: Rect, s: &str, f: Font, c: Color32) {
    let g = ui.fonts(|x| x.layout_job(f.job(s, c)));
    let at = Pos2::new(r.right() - g.size().x, r.center().y - g.size().y / 2.0);
    ui.painter().with_clip_rect(r).galley(at, g, c);
}

/// An empire's name in the game's encoding.
fn game_name(texts: &UiTexts, b: &[u8]) -> String {
    ig2_formats::text::decode(b, texts.lang).trim().to_string()
}

fn game_text(texts: &UiTexts, b: &[u8]) -> String {
    ig2_formats::text::decode(b, texts.lang)
}

/// A graph line's name: the race's.
fn race_short(texts: &UiTexts, e: &super::Empire) -> String {
    let r = game_name(texts, &texts.race(e.race));
    if r.is_empty() { game_name(texts, &e.name) } else { r }
}

/// The smallest 1, 2 or 5 times a power of ten at or above `v`.
fn nice(v: f64) -> f64 {
    if v <= 0.0 {
        return 1.0;
    }
    let p = 10f64.powf(v.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|f| f * p).find(|&s| s >= v).unwrap_or(10.0 * p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!((short(412_345), short(950), short(18_400_000), short(-12_000)), ("412k".into(), "950".into(), "18.4M".into(), "-12k".into()));
        assert_eq!((nice(1973.0), nice(0.0), nice(3.0), nice(101.0)), (2000.0, 1.0, 5.0, 200.0));
    }
}
