//! Ours (ADR 0078): the help board: the Help screen (`AktKep` 1, re/screens/help.md) as an egui page under the top
//! bar: the topics of `helpmain.txt` at the left with a search field, the open topic's sub-topics under it and Back;
//! the topic's text at the right as a page: its title, paragraphs, the pictures (`help/<name>_bmp.pvr`) in frames,
//! the `<C5>term:<C1> text` lines as a definition list, a sub-topic's heading where the list jumps. The topic files'
//! own markup (`+`, `-`, `*` lines, `<Fn>`, `<Cn>`, `<P…>`, `<SUBTOPIC>`) is read here into blocks; the original
//! screen's 69-column wrap and line table stay with the classic look.

use std::collections::HashMap;

use egui::{Color32, Pos2, Rect, Sense, Vec2};

use crate::gui::ds::{self, Font, Text, Weight};
use crate::help::HelpScreen;
use crate::ui::UiTexts;

/// One piece of a topic's text.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// The `<F4>` line: the topic's title.
    Title(String),
    /// An `<F3>` line: a section's heading; `Some(k)` after the `k`-th `<SUBTOPIC>` mark.
    Heading(String, Option<usize>),
    /// A line of text: its runs and their colours (`<Cn>`).
    Para(Vec<(String, u8)>),
    /// A `<C5>term:<C1> text` line.
    Def(String, String),
    /// A `<P…>` picture by its name.
    Pic(String),
    /// Empty lines (one or more).
    Gap,
}

/// A topic file's lines as `wrap` reads them (help.rs: `+` starts a line, `-` goes on with a space, `*` is copied as
/// it is, any other line ends the parse), then into blocks.
pub fn parse(file: &str) -> Vec<Block> {
    let mut lines: Vec<String> = Vec::new();
    for raw in file.split('\n') {
        let raw = raw.trim_end_matches('\r');
        let (Some(c), body) = (raw.chars().next(), raw.get(2..).unwrap_or("")) else { break };
        match c {
            '+' | '*' => lines.push(body.to_string()),
            '-' => match lines.last_mut() {
                Some(l) => {
                    l.push(' ');
                    l.push_str(body.trim_start());
                }
                None => lines.push(body.to_string()),
            },
            _ => break,
        }
    }
    let (mut font, mut col) = (1u8, 1u8);
    let mut subs = 0usize;
    let mut pending_sub = None;
    let mut out: Vec<Block> = Vec::new();
    for l in lines {
        let mut runs: Vec<(String, u8)> = Vec::new();
        let mut pics = Vec::new();
        // The line's font: the one its first letters are in (a heading switches back to the body's at its end).
        let mut line_font = None;
        let mut rest = l.as_str();
        while !rest.is_empty() {
            if let Some(tag) = rest.strip_prefix('<').and_then(|r| r.find('>').map(|e| &r[..e])) {
                let d = |s: &str| s.parse::<u8>().ok();
                match tag.as_bytes().first() {
                    Some(b'F') if d(&tag[1..]).is_some() => font = d(&tag[1..]).unwrap_or(1),
                    Some(b'C') if d(&tag[1..]).is_some() => col = d(&tag[1..]).unwrap_or(1),
                    Some(b'P') => pics.push(tag[1..].to_string()),
                    _ if tag == "SUBTOPIC" => {
                        pending_sub = Some(subs);
                        subs += 1;
                    }
                    _ => runs.push((format!("<{tag}>"), col)),
                }
                rest = &rest[tag.len() + 2..];
            } else {
                let n = rest.char_indices().skip(1).find(|c| c.1 == '<').map_or(rest.len(), |c| c.0);
                if !rest[..n].trim().is_empty() {
                    line_font.get_or_insert(font);
                }
                match runs.last_mut() {
                    Some(r) if r.1 == col => r.0.push_str(&rest[..n]),
                    _ => runs.push((rest[..n].to_string(), col)),
                }
                rest = &rest[n..];
            }
        }
        let text: String = runs.iter().map(|r| r.0.as_str()).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        let block = if let Some(p) = pics.pop() {
            Block::Pic(p)
        } else if text.is_empty() {
            Block::Gap
        } else if line_font == Some(4) {
            Block::Title(text)
        } else if line_font == Some(3) {
            Block::Heading(text, pending_sub.take())
        } else if let [(term, 5), (_, c), ..] = runs.iter().filter(|r| !r.0.trim().is_empty()).collect::<Vec<_>>()[..] && *c != 5 {
            let def: String = runs.iter().skip_while(|r| r.1 == 5 || r.0.trim().is_empty()).map(|r| r.0.as_str()).collect();
            Block::Def(term.trim().trim_end_matches(':').to_string(), def.split_whitespace().collect::<Vec<_>>().join(" "))
        } else {
            let mut r: Vec<(String, u8)> = runs.into_iter().filter(|r| !r.0.is_empty()).collect();
            if let Some(f) = r.first_mut() {
                f.0 = f.0.trim_start().to_string();
            }
            Block::Para(r)
        };
        // Runs of empty lines are one gap (a picture's lines under it are its room on the original screen).
        if block == Block::Gap && matches!(out.last(), Some(Block::Gap | Block::Pic(_)) | None) {
            continue;
        }
        out.push(block);
    }
    while out.last() == Some(&Block::Gap) {
        out.pop();
    }
    out
}

/// What the page asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Back,
}

/// The page's own state: the search, the sub-topic picked (the list jumps to it once), the topics' blocks and the
/// pictures, read on demand.
#[derive(Default)]
pub struct Page {
    pub query: String,
    /// The search field takes the keys.
    pub typing: bool,
    sub: Option<usize>,
    jump: bool,
    docs: HashMap<usize, Vec<Block>>,
    pics: HashMap<String, Option<egui::TextureHandle>>,
}

/// The board's sizes (points at 1920 × 1080): the page's box, the topic column, its gap.
const BOX: [f32; 3] = [96.0, 1640.0, 920.0];
const NAV_W: f32 = 360.0;
const GAP: f32 = 24.0;

impl Page {
    /// A key while the search field has it: a character, Backspace; Enter or Esc end the typing. `true`: taken.
    pub fn key(&mut self, k: &winit::keyboard::Key<&str>) -> bool {
        use winit::keyboard::{Key, NamedKey};
        if !self.typing {
            return false;
        }
        match k {
            Key::Named(NamedKey::Enter | NamedKey::Escape) => self.typing = false,
            Key::Named(NamedKey::Backspace) => {
                self.query.pop();
            }
            Key::Named(NamedKey::Space) => self.query.push(' '),
            Key::Character(c) => self.query.push_str(c),
            // The F-keys still leave for their screens.
            Key::Named(NamedKey::F1 | NamedKey::F2 | NamedKey::F3 | NamedKey::F4 | NamedKey::F5 | NamedKey::F6 | NamedKey::F7 | NamedKey::F8 | NamedKey::F9 | NamedKey::F10) => return false,
            _ => {}
        }
        true
    }

    fn doc(&mut self, s: &HelpScreen, i: usize, texts: &UiTexts) -> &Vec<Block> {
        self.docs.entry(i).or_insert_with(|| parse(&ig2_formats::text::decode(s.file(i), texts.lang)))
    }

    fn picture(&mut self, ctx: &egui::Context, install: &ig2_formats::Install, name: &str) -> Option<egui::TextureHandle> {
        self.pics
            .entry(name.to_string())
            .or_insert_with(|| {
                let file = name.to_ascii_lowercase().replace('.', "_");
                let img = crate::assets::Image::load(install, &format!("dataraw/help/{file}.pvr")).ok()?;
                let pixels = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
                Some(ctx.load_texture(format!("help-{file}"), pixels, egui::TextureOptions::LINEAR))
            })
            .clone()
    }

    /// One frame of the page over the help screen `s` (its topic is `s.sel`), faded by `opacity`.
    pub fn show(&mut self, ctx: &egui::Context, texts: &UiTexts, s: &mut HelpScreen, install: &ig2_formats::Install, opacity: f32) -> Option<Act> {
        s.drawn_modern();
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let screen = ctx.screen_rect();
        let mut act = None;
        // The screen under the top bar dimmed (the bar stays the game's).
        let top = BOX[0].min(screen.height() * 0.09);
        ctx.layer_painter(egui::LayerId::background()).rect_filled(Rect::from_min_max(Pos2::new(screen.left(), screen.top() + top * 0.46), screen.max), 0.0, Color32::from_black_alpha((153.0 * opacity) as u8));
        let w = BOX[1].min(screen.width() - 32.0);
        let h = BOX[2].min(screen.height() - top - 40.0);
        let r = Rect::from_min_size(Pos2::new(screen.center().x - w / 2.0, screen.top() + top), Vec2::new(w, h));
        let nav_w = NAV_W.min(w * 0.3);
        let nav = Rect::from_min_size(r.min, Vec2::new(nav_w, h));
        let art = Rect::from_min_max(Pos2::new(nav.right() + GAP, r.top()), r.max);
        // The topics: names in the game's text, an all-capitals one in sentence case as the rest.
        let names: Vec<String> = s.topics.iter().map(|t| sentence(&ig2_formats::text::decode(&t.name, texts.lang))).collect();
        let q = self.query.to_lowercase();
        let shown: Vec<usize> = (0..s.topics.len())
            .filter(|&i| q.is_empty() || names[i].to_lowercase().contains(&q) || ig2_formats::text::decode(s.file(i), texts.lang).to_lowercase().contains(&q))
            .collect();
        egui::Area::new(egui::Id::new("help-nav")).fixed_pos(nav.min).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            ds::paint_panel(ui.painter(), nav, ds::PanelKind::Deep, 0.90);
            let inner = nav.shrink2(Vec2::new(12.0, 20.0));
            // The search field (a recessed well, the glass icon, the query or its placeholder).
            let field = Rect::from_min_size(inner.min + Vec2::new(4.0, 0.0), Vec2::new(inner.width() - 8.0, 44.0));
            let fr = ui.interact(field, ui.id().with("search"), Sense::click());
            if fr.clicked() {
                self.typing = true;
            } else if ui.input(|i| i.pointer.primary_clicked()) && !fr.hovered() {
                self.typing = false;
            }
            if fr.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
            }
            ds::field_back(ui.painter(), field, self.typing);
            let p = ui.painter();
            let glass = Rect::from_center_size(Pos2::new(field.left() + 21.0, field.center().y), Vec2::splat(18.0));
            p.circle_stroke(glass.center() - Vec2::splat(1.5), 6.0, egui::Stroke::new(2.0f32, k.ink_muted));
            p.line_segment([glass.center() + Vec2::splat(3.0), glass.center() + Vec2::splat(7.5)], egui::Stroke::new(2.0f32, k.ink_muted));
            let f = Font { size: 16.0, line: 20.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
            let tf = Rect::from_min_max(Pos2::new(field.left() + 42.0, field.top()), Pos2::new(field.right() - 12.0, field.bottom()));
            let caret = if self.typing && (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0 { "|" } else { "" };
            if self.query.is_empty() && !self.typing {
                Text::new(&tr("Search"), f).color(k.ink_muted).paint(ui, tf);
            } else {
                Text::new(&format!("{}{caret}", self.query), f).color(k.ink).paint(ui, tf);
            }
            if self.typing {
                ui.ctx().request_repaint();
            }
            // The topics and the open one's sub-topics.
            let back_h = 44.0;
            // The board's rows 2 lower than the field's 12, their text 2 above the row's middle (the sections' 1).
            let list = Rect::from_min_max(Pos2::new(inner.left(), field.bottom() + 14.0), Pos2::new(inner.right(), inner.bottom() - back_h - 12.0));
            // The list scrolls when the open topic's sections or a small window don't leave it room, with the page's
            // track at its right.
            let rows_h: f32 = shown.iter().map(|&i| 42.0 + if i == s.sel { 36.0 * s.topics[i].subs.len() as f32 } else { 0.0 }).sum();
            let over = rows_h > list.height() + 2.0;
            let rows = if over { list.with_max_x(list.right() - 16.0) } else { list };
            let mut rows_ui = ui.new_child(egui::UiBuilder::new().max_rect(rows).layout(egui::Layout::top_down(egui::Align::Min)));
            let out = egui::ScrollArea::vertical().id_salt("help-topics").auto_shrink([false; 2]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(&mut rows_ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                let row_font = Font { size: 17.0, line: 20.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
                let sub_font = Font { size: 15.0, line: 18.0, weight: Weight::Medium, tracking: 0.10, upper: true, lcd: false };
                for &i in &shown {
                    let open = i == s.sel;
                    let mut rows = vec![(None, 40.0)];
                    if open {
                        rows.extend((0..s.topics[i].subs.len()).map(|n| (Some(n), 34.0)));
                    }
                    for (sub, rh) in rows {
                        let (rr, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::click());
                        let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
                        let sel = open && sub == self.sub;
                        ds::row_back(ui, rr, hover, sel);
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let (label, font, x, c, up) = match sub {
                            None => (names[i].clone(), if open { row_font.weight(Weight::SemiBold) } else { row_font }, 16.0, k.ink, 2.0),
                            Some(n) => (ig2_formats::text::decode(&s.topics[i].subs[n], texts.lang), sub_font, 36.0, if sel { k.ink } else { k.ink_silver }, 1.0),
                        };
                        Text::new(&label, font).color(c).shrink(font.size - 3.0).paint(ui, Rect::from_min_max(Pos2::new(rr.left() + x, rr.top() - up), Pos2::new(rr.right() - 12.0, rr.bottom() - up)));
                        if resp.clicked() {
                            if i != s.sel {
                                s.sel = i;
                                self.sub = None;
                            } else {
                                self.sub = sub;
                            }
                            self.jump = true;
                        }
                    }
                }
            });
            if over {
                track(ui, &out, Rect::from_min_max(Pos2::new(list.right() - 8.0, list.top() + 4.0), Pos2::new(list.right(), list.bottom() - 4.0)), "help-topics-track");
            }
            if shown.is_empty() {
                Text::new(&tr("No topic found."), Font::BODY).color(k.ink_muted).paint(ui, Rect::from_min_size(Pos2::new(list.left() + 16.0, list.top()), Vec2::new(list.width() - 16.0, 26.0)));
            }
            let back = Rect::from_min_size(Pos2::new(inner.left() + 4.0, inner.bottom() - back_h), Vec2::new(inner.width() - 8.0, back_h));
            if ui.put(back, ds::Plate::new(&tr("Back")).width(back.width())).clicked() {
                act = Some(Act::Back);
            }
        });
        // The topic's page.
        let sel = s.sel.min(s.topics.len().saturating_sub(1));
        let blocks = if s.topics.is_empty() { Vec::new() } else { self.doc(s, sel, texts).clone() };
        let jump = std::mem::take(&mut self.jump).then_some(self.sub);
        egui::Area::new(egui::Id::new("help-art")).fixed_pos(art.min).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            ds::paint_panel(ui.painter(), art, ds::PanelKind::Glass, 0.90);
            let inner = Rect::from_min_max(art.min + Vec2::new(40.0, 31.0), art.max - Vec2::new(56.0, 32.0));
            ui.set_clip_rect(art);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            child.spacing_mut().item_spacing = Vec2::ZERO;
            let mut area = egui::ScrollArea::vertical().id_salt(("help-scroll", sel)).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
            if jump == Some(None) {
                area = area.vertical_scroll_offset(0.0);
            }
            let out = area.show(&mut child, |ui| {
                let w = ui.available_width().min(900.0);
                let body = Font { size: 18.0, line: 29.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
                let dl = Font { size: 17.0, line: 26.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
                let term_w = blocks.iter().filter_map(|b| if let Block::Def(t, _) = b { Some(Text::new(t, dl.weight(Weight::SemiBold)).layout(ui.ctx(), f32::INFINITY).width) } else { None }).fold(0.0f32, f32::max).min(w * 0.4);
                for (n, b) in blocks.iter().enumerate() {
                    let prev = n.checked_sub(1).map(|p| &blocks[p]);
                    let next = blocks.get(n + 1);
                    match b {
                        Block::Title(t) => {
                            let f = Font { size: 30.0, line: 36.0, weight: Weight::SemiBold, tracking: 0.2, upper: true, lcd: false };
                            Text::new(t, f).color(k.ink_silver).shrink(20.0).show(ui, Vec2::new(ui.available_width(), 36.0));
                            ui.add_space(25.0);
                        }
                        Block::Heading(t, sub) => {
                            let f = Font { size: 22.0, line: 30.0, weight: Weight::SemiBold, tracking: 0.08, upper: false, lcd: false };
                            ui.add_space(if prev.is_some_and(|p| !matches!(p, Block::Gap | Block::Title(_))) { 24.0 } else { 8.0 });
                            let resp = Text::new(t, f).color(k.info).lines(2).show(ui, Vec2::new(w, 30.0));
                            if sub.is_some() && jump == Some(*sub) {
                                ui.scroll_to_rect_animation(resp.rect.expand(8.0), Some(egui::Align::TOP), egui::style::ScrollAnimation::none());
                            }
                            ui.add_space(10.0);
                        }
                        Block::Para(runs) => {
                            let mut job = egui::text::LayoutJob::default();
                            for (t, c) in runs {
                                let ink = match c {
                                    5 => k.info,
                                    3 => k.ink_silver,
                                    _ => k.ink,
                                };
                                let fmt = egui::TextFormat { font_id: body.id(), extra_letter_spacing: body.spacing(), color: ink, line_height: Some(body.line), ..Default::default() };
                                job.append(t, 0.0, fmt);
                            }
                            job.wrap.max_width = w;
                            let g = ui.fonts(|f| f.layout_job(job));
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, g.rect.height()), Sense::hover());
                            ui.painter().galley(rect.min, g, k.ink);
                        }
                        Block::Def(t, d) => {
                            let x = term_w + 20.0;
                            let dh = Text::new(d, dl).lines(8).layout(ui.ctx(), w - x).galley.rect.height().max(26.0);
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, dh), Sense::hover());
                            Text::new(t, dl.weight(Weight::SemiBold)).color(k.info).lines(8).paint(ui, Rect::from_min_size(rect.min, Vec2::new(term_w, dh)));
                            Text::new(d, dl).color(k.ink).lines(8).paint(ui, Rect::from_min_max(Pos2::new(rect.left() + x, rect.top()), rect.max));
                        }
                        Block::Pic(name) => {
                            ui.add_space(12.0);
                            if let Some(tex) = self.picture(ui.ctx(), install, name) {
                                let [tw, th] = tex.size().map(|v| v as f32);
                                // The board's frame: the pictures at the 1080-line screen's scale (2.25: the design screen's 764 wide).
                                let pw = (tw * 2.25).min(766.0).min(w - 12.0);
                                let ph = pw * th / tw.max(1.0);
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(pw + 12.0, ph + 12.0), Sense::hover());
                                let p = ui.painter();
                                p.rect_filled(rect, ds::RADIUS_PANEL, Color32::BLACK);
                                ds::rim(p, rect, ds::RADIUS_PANEL, k.rim);
                                p.image(tex.id(), rect.shrink(6.0), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                            }
                            ui.add_space(28.0);
                        }
                        Block::Gap => {
                            let defs = matches!(prev, Some(Block::Def(..))) && matches!(next, Some(Block::Def(..)));
                            if !matches!(prev, Some(Block::Title(_) | Block::Heading(..)) | None) && !matches!(next, Some(Block::Heading(..) | Block::Pic(_))) {
                                ui.add_space(if defs { 10.0 } else { 16.0 });
                            }
                        }
                    }
                    if matches!(b, Block::Para(_) | Block::Def(..)) && matches!(next, Some(Block::Para(_) | Block::Def(..))) {
                        ui.add_space(4.0);
                    }
                }
                ui.add_space(24.0);
            });
            // The board's track: 8 wide, 18 from the right edge.
            track(ui, &out, Rect::from_min_max(Pos2::new(art.right() - 26.0, art.top() + 32.0), Pos2::new(art.right() - 18.0, art.bottom() - 32.0)), "help-track");
        });
        act
    }
}

/// A scroll area's track at `track` while its content is taller than it: the thumb a lit plate; dragged, it scrolls.
fn track<R>(ui: &egui::Ui, out: &egui::scroll_area::ScrollAreaOutput<R>, track: Rect, id: &str) {
    let k = ds::tokens();
    let (view, content) = (out.inner_rect.height(), out.content_size.y);
    if content <= view + 1.0 {
        return;
    }
    let p = ui.painter();
    p.rect_filled(track, 4, k.glass_deep);
    ds::rim(p, track, 4, k.rim);
    let th = (track.height() * view / content).max(24.0);
    let max = content - view;
    let mut state = out.state;
    let resp = ui.interact(track.expand2(Vec2::new(6.0, 0.0)), ui.id().with(id), Sense::click_and_drag());
    if let Some(at) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        state.offset.y = ((at.y - track.top() - th / 2.0) / (track.height() - th) * max).clamp(0.0, max);
        state.store(ui.ctx(), out.id);
    }
    let y = track.top() + (track.height() - th) * (state.offset.y / max).clamp(0.0, 1.0);
    p.rect_filled(Rect::from_min_size(Pos2::new(track.left(), y), Vec2::new(track.width(), th)), 4, k.plate_high);
}

/// An all-capitals name ("GYÁRTÁS") in sentence case ("Gyártás"); others as they are.
fn sentence(s: &str) -> String {
    let s = s.trim();
    if s.chars().any(char::is_lowercase) {
        return s.to_string();
    }
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks() {
        let f = "* <F4><C3>      DESIGN (F5)\r\n+ \r\n+ <F2><C1>\r\n+ The design screen\r\n- lets you equip.\r\n+ \r\n*    <Pdes_kis.bmp>\r\n+ \r\n+  \r\n+ After.\r\n+ \r\n+ <C5>Base:<C1> The base\r\n- vehicle.\r\n+\r\n+ <C5>Price:<C1> Cost.\r\n+ <SUBTOPIC>\r\n+ <F3><C5>New Design<F2><C1>\r\nstop\r\n+ never\r\n";
        assert_eq!(
            parse(f),
            vec![
                Block::Title("DESIGN (F5)".into()),
                Block::Gap,
                Block::Para(vec![("The design screen lets you equip.".into(), 1)]),
                Block::Gap,
                Block::Pic("des_kis.bmp".into()),
                Block::Para(vec![("After.".into(), 1)]),
                Block::Gap,
                Block::Def("Base".into(), "The base vehicle.".into()),
                Block::Gap,
                Block::Def("Price".into(), "Cost.".into()),
                Block::Gap,
                Block::Heading("New Design".into(), Some(0)),
            ]
        );
        assert_eq!(sentence("GYÁRTÁS"), "Gyártás");
        assert_eq!(sentence("Felső ikonsor"), "Felső ikonsor");
    }
}
