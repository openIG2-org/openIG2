//! Ours (ADR 0076): the modern diplomacy screen on the egui layer (ADR 0062), the "Modern diplomacy screen" option,
//! drawn with the design kit (ADR 0073, `gui::ds`) after the diplomacy board. The 3D ring with the leader's video
//! stays (`DiplomacyScene::modern`: raised to the board's centre, no cages); the contacts' heads become a list of
//! cards, the info panel a card, the proposal menus (`Menuk`, re/screens/diplomacy.md §6) a cascade of panels and the
//! dialogue texts a panel with the one-level answer menus as buttons. A pick replays the original's press, drag and
//! release on the menu rows (`diplomacy::Open::pick`), so every proposal, answer and story hand-off stays the
//! original's.

use egui::{pos2, vec2, Color32, Rect, Sense, Ui};

use crate::diplomacy::menu;
use crate::gui::ds::{self, Font, PanelKind, Text};
use crate::starmap_modern::{self as sm};

/// What the panels ask for.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Bar(sm::Act),
    /// Contact `k` pressed (`CheckAndColorize`).
    Select(usize),
    /// A menu path `[(menu, row), …]`, root first, picked.
    Pick(Vec<(usize, usize)>),
    /// A row that opens a sub-menu (or a greyed one) pressed: the original's press clicks (`DiplomGfx2D_1`).
    Click,
}

/// A contact card: its slot, the empire's name, the relation's status, the race for the head picture, selected,
/// a message from it waiting (the green blink), alive.
#[derive(Debug, Clone, Default)]
pub struct Contact {
    pub k: usize,
    pub name: String,
    pub status: String,
    pub tone: Tone,
    pub race: usize,
    pub selected: bool,
    pub waiting: bool,
    pub alive: bool,
}

/// A relation's colour: war (and cease-fire) red, alliance green, else blue.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Tone {
    #[default]
    Neutral,
    Hostile,
    Friendly,
}

impl Tone {
    fn color(self) -> Color32 {
        let k = ds::tokens();
        match self {
            Tone::Neutral => k.info,
            Tone::Hostile => k.enemy,
            Tone::Friendly => k.own,
        }
    }
}

/// A menu row: its text, greyed (`off`, or an empty list behind it), the menu it opens.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub text: String,
    pub off: bool,
    pub sub: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub contacts: String,
    pub planets: String,
    pub treaty: String,
    pub status: String,
    pub exit: String,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    /// The screen takes input (opened, not closing): the panels show.
    pub live: bool,
    pub contacts: Vec<Contact>,
    /// The selected empire's card: its name, known planets, treaty, status.
    pub info: Option<(String, String, String, String, Tone)>,
    /// Every `Menuk` record's rows (by menu id) and the ones the original shows this frame, root first.
    pub menus: Vec<Vec<Row>>,
    pub shown: Vec<usize>,
    /// The dialogue: own lines, the other side's, and how far each has faded in (0..=1).
    pub own: Vec<String>,
    pub other: Vec<String>,
    pub own_alpha: f32,
    pub other_alpha: f32,
    pub other_tone: Tone,
    pub labels: Labels,
}

/// What stays between frames: the cascade opened by clicks (root first), reset when the root menu changes; the
/// path of the last pick for the dialogue's heading.
#[derive(Default)]
pub struct State {
    pub bar: sm::State,
    pub pics: Option<crate::info_modern::Pics>,
    path: Vec<(usize, usize)>,
    root: Option<usize>,
    heading: Vec<String>,
}

/// The board's measures (1920 × 1080).
const CARD_W: f32 = 360.0;
const CARD_H: f32 = 112.0;
const INFO_W: f32 = 440.0;
const MENUS_TOP: f32 = 620.0;
const ROOT_W: f32 = 330.0;
const SUB_W: f32 = 240.0;
const ROW_H: f32 = 38.0;
const TEXT: Rect = Rect { min: egui::Pos2::new(640.0, 790.0), max: egui::Pos2::new(1440.0, 1020.0) };

/// The menus whose rows open sub-menus (`run_cascade`); the others are one-level answers (`run_plain`).
const CASCADES: [usize; 3] = [menu::MAIN, menu::EXCHANGE, menu::BLOCKED];

pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = sm::board_bar(ctx, &v.bar, &mut st.bar, Act::Bar);
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let top = sm::bar_bottom(ctx);
    let g = ds::squeeze(h);
    if !v.live {
        st.path.clear();
        st.heading.clear();
        return acts;
    }
    // The cascade by clicks: open while its root menu shows.
    let root = v.shown.first().copied().filter(|r| CASCADES.contains(r));
    if root != st.root || st.path.first().is_some_and(|&(m, _)| Some(m) != root) {
        st.path.clear();
        if root.is_some() {
            st.heading.clear();
        }
        st.root = root;
    }
    let contacts = Rect::from_min_max(pos2(ds::BOARD_EDGE, top + 40.0), pos2(ds::BOARD_EDGE + CARD_W, MENUS_TOP * g - 16.0));
    ds::place(ctx, "dmh-contacts", contacts, |ui| contacts_panel(ui, v, st, contacts.size(), &mut acts));
    if let Some(info) = &v.info {
        let r = Rect::from_min_size(pos2(w - ds::BOARD_EDGE - INFO_W.min(w * 0.25), top + 28.0), vec2(INFO_W.min(w * 0.25), 156.0));
        ds::place(ctx, "dmh-info", r, |ui| info_panel(ui, v, info, r.size()));
    }
    // The cascade: the root, then each opened sub-menu beside the one before, its top level with the opening row.
    if let Some(root) = root {
        let mut x = ds::BOARD_EDGE;
        let mut y = MENUS_TOP * g;
        let mut id = root;
        for level in 0..3 {
            let rows = v.menus.get(id).map_or(&[][..], |m| &m[..]);
            if rows.is_empty() {
                break;
            }
            let width = if level == 0 { ROOT_W } else { SUB_W };
            let r = Rect::from_min_size(pos2(x, y), vec2(width, rows.len() as f32 * ROW_H + 20.0));
            let picked = st.path.get(level).map(|&(_, row)| row);
            if let Some((row, off)) = ds::place(ctx, format!("dmh-menu{level}"), r, |ui| menu_panel(ui, rows, picked, r.size())).inner {
                if off {
                    acts.push(Act::Click);
                    break;
                }
                st.path.truncate(level);
                st.path.push((id, row));
                st.heading = st.path.iter().filter_map(|&(m, i)| v.menus.get(m)?.get(i).map(|r| r.text.clone())).collect();
                if rows[row].sub.is_none_or(|s| v.menus.get(s).is_none_or(|m| m.is_empty())) {
                    acts.push(Act::Pick(std::mem::take(&mut st.path)));
                } else {
                    acts.push(Act::Click);
                }
            }
            let Some(&(_, row)) = st.path.get(level) else { break };
            let Some(sub) = rows.get(row).and_then(|r| r.sub) else { break };
            x = r.right() + 12.0;
            y = r.top() + row as f32 * ROW_H;
            id = sub;
        }
    }
    let text = Rect::from_min_max(pos2(TEXT.left().max(w / 2.0 - 320.0), TEXT.top() * g), pos2(TEXT.right().min(w / 2.0 + 480.0), h - 60.0 * g));
    let plain: Vec<usize> = v.shown.iter().copied().filter(|m| !CASCADES.contains(m)).collect();
    ds::place(ctx, "dmh-text", text, |ui| text_panel(ui, v, st, &plain, text.size(), &mut acts));
    let exit = Rect::from_min_size(pos2(w - ds::BOARD_EDGE - ds::EXIT_W, h - 40.0 * g - 44.0), vec2(ds::EXIT_W, 44.0));
    ds::place(ctx, "dmh-exit", exit, |ui| {
        if ui.add(ds::Plate::new(&v.labels.exit).width(ds::EXIT_W).height(44.0)).clicked() {
            acts.push(Act::Bar(sm::Act::Screen(2)));
        }
    });
    acts
}

/// The contacts: the heading, then a card per contact (the leader's picture, the empire, the relation), scrolling
/// when more than fit.
fn contacts_panel(ui: &mut Ui, v: &View, st: &mut State, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    Text::new(&v.labels.contacts, Font::LABEL).color(k.ink_muted).paint(ui, Rect::from_min_size(rect.min, vec2(size.x, 20.0)));
    let list = rect.with_min_y(rect.top() + 30.0);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("dmh-contacts").auto_shrink([false; 2]).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = 12.0;
        for c in &v.contacts {
            let (r, resp) = ui.allocate_exact_size(vec2(list.width(), CARD_H), Sense::click());
            let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered() && c.alive);
            let p = ui.painter();
            ds::hgrad(p, r, ds::RADIUS_PANEL, &[(0.0, Color32::from_white_alpha(36)), (1.0, Color32::from_white_alpha(5))]);
            let rim = if c.selected { k.ink_silver } else { k.rim.lerp_to_gamma(k.rim_bright, hot) };
            ds::rim(p, r, ds::RADIUS_PANEL, rim);
            let pic = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(114.0, 100.0));
            p.rect_filled(pic, ds::RADIUS_PLATE, Color32::BLACK);
            if let Some(t) = st.pics.as_ref().and_then(|x| x.head(c.race)) {
                let side = pic.height();
                let img = Rect::from_center_size(pic.center(), vec2(side, side));
                // The portrait's square has a wide margin: its middle fills the well.
                p.with_clip_rect(pic).image(t.id(), img.expand(side * 0.18), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), if c.alive { Color32::WHITE } else { Color32::from_gray(90) });
            }
            // A message from this empire waits: its mark blinks green (`JelolFej`'s 0x0E860E).
            if c.waiting && (ui.input(|i| i.time) * 2.5) as i64 % 2 == 0 {
                ds::rim(p, pic, ds::RADIUS_PLATE, k.own);
                ui.ctx().request_repaint();
            }
            let tx = pic.right() + 14.0;
            let name = Rect::from_min_max(pos2(tx, r.center().y - 24.0), pos2(r.right() - 12.0, r.center().y + 2.0));
            Text::new(&c.name, Font::HEADING.sized(18.0)).shrink(13.0).lines(1).color(if c.alive { k.ink } else { k.ink_muted }).paint(ui, name);
            let status = Rect::from_min_max(pos2(tx, r.center().y + 2.0), pos2(r.right() - 12.0, r.center().y + 24.0));
            Text::new(&c.status, Font::LABEL.sized(14.0)).shrink(10.0).color(c.tone.color()).paint(ui, status);
            let resp = resp.on_hover_cursor(if c.alive { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default });
            if resp.clicked() && c.alive && !c.selected {
                acts.push(Act::Select(c.k));
                st.heading.clear();
            }
        }
    });
}

/// The selected empire's card: its name, then known planets, treaty and status as table rows.
fn info_panel(ui: &mut Ui, v: &View, info: &(String, String, String, String, Tone), size: egui::Vec2) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Glass, 0.9);
    let inner = rect.shrink2(vec2(24.0, 20.0));
    Text::new(&info.0, Font::HEADING).shrink(15.0).color(k.ink).paint(ui, Rect::from_min_size(inner.min, vec2(inner.width(), 26.0)));
    let rows = [(&v.labels.planets, &info.1, k.info), (&v.labels.treaty, &info.2, k.info), (&v.labels.status, &info.3, info.4.color())];
    for (n, (l, val, c)) in rows.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(inner.left(), inner.top() + 38.0 + n as f32 * 28.0), vec2(inner.width(), 22.0));
        let base = ds::fit::baseline(r, Font::DATA);
        Text::new(l, Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, r.with_max_x(r.left() + 140.0));
        Text::new(val, Font::DATA).shrink(11.0).baseline(base).color(c).paint(ui, r.with_min_x(r.left() + 150.0));
    }
}

/// One menu of the cascade: its rows, the opened one framed with a chevron when it leads on; a greyed row only
/// clicks. Returns the row pressed and whether it is greyed.
fn menu_panel(ui: &mut Ui, rows: &[Row], picked: Option<usize>, size: egui::Vec2) -> Option<(usize, bool)> {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.9);
    let mut out = None;
    for (i, row) in rows.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + 10.0, rect.top() + 10.0 + i as f32 * ROW_H), vec2(rect.width() - 20.0, ROW_H));
        let resp = ui.interact(r, ui.id().with(("dmh-row", i)), Sense::click());
        let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered() && !row.off);
        ds::row_back(ui, r, hot, picked == Some(i));
        let room = if row.sub.is_some() { 28.0 } else { 8.0 };
        let c = pos2(r.right() - 16.0, r.center().y);
        Text::new(&row.text, Font::BODY.sized(19.0)).shrink(13.0).color(if row.off { k.ink_muted } else { k.ink }).paint(ui, Rect::from_min_max(r.min + vec2(8.0, 0.0), r.max - vec2(room, 0.0)));
        if row.sub.is_some() && !row.off && (picked == Some(i) || hot > 0.0) {
            let ink = k.ink_silver.gamma_multiply(if picked == Some(i) { 1.0 } else { hot });
            ui.painter().line(vec![c + vec2(-3.0, -5.0), c + vec2(2.0, 0.0), c + vec2(-3.0, 5.0)], egui::Stroke::new(1.5f32, ink));
        }
        let resp = resp.on_hover_cursor(if row.off { egui::CursorIcon::NotAllowed } else { egui::CursorIcon::PointingHand });
        if resp.clicked() {
            out = Some((i, row.off));
        }
    }
    out
}

/// The dialogue: the heading (the picked menu path), our lines in our colour, the other side's in the relation's,
/// wrapped and scrolling; the one-level answer menus as buttons along the foot.
fn text_panel(ui: &mut Ui, v: &View, st: &State, plain: &[usize], size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.9);
    let inner = rect.shrink2(vec2(24.0, 18.0));
    let heading = st.heading.join(" · ");
    Text::new(&heading, Font::LABEL).shrink(10.0).color(k.ink_muted).paint(ui, Rect::from_min_size(inner.min, vec2(inner.width(), 18.0)));
    let buttons: Vec<(usize, usize, &Row)> = plain.iter().flat_map(|&m| v.menus.get(m).into_iter().flatten().enumerate().map(move |(i, r)| (m, i, r))).collect();
    let foot = if buttons.is_empty() { 0.0 } else { 46.0 };
    let body = Rect::from_min_max(pos2(inner.left(), inner.top() + 28.0), pos2(inner.right(), inner.bottom() - foot));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("dmh-body").auto_shrink([false; 2]).stick_to_bottom(true).show(&mut child, |ui| {
        let f = Font::BODY.sized(18.0);
        let mut job = egui::text::LayoutJob { wrap: egui::text::TextWrapping { max_width: body.width() - 16.0, ..Default::default() }, ..Default::default() };
        let mut add = |lines: &[String], c: Color32| {
            if lines.is_empty() {
                return;
            }
            let sep = if job.text.is_empty() { "" } else { "\n" };
            job.append(&format!("{sep}{}", lines.join(" ")), 0.0, egui::TextFormat { font_id: f.id(), color: c, extra_letter_spacing: f.spacing(), line_height: Some(30.0), ..Default::default() });
        };
        add(&v.own, k.own.gamma_multiply(v.own_alpha));
        add(&v.other, v.other_tone.color().gamma_multiply(v.other_alpha));
        let galley = ui.fonts(|x| x.layout_job(job));
        let (r, _) = ui.allocate_exact_size(vec2(body.width() - 16.0, galley.size().y.max(30.0)), Sense::hover());
        ui.painter().galley(r.min, galley, k.ink);
    });
    let mut x = inner.left();
    for (m, i, row) in buttons {
        let wdt = (ui.fonts(|f| f.layout_no_wrap(row.text.clone(), Font::BODY.sized(15.0).id(), k.ink).size().x) + 40.0).clamp(96.0, (inner.right() - x).max(96.0));
        let r = Rect::from_min_size(pos2(x, inner.bottom() - 38.0), vec2(wdt, 38.0));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r));
        if c.add_enabled(!row.off, ds::Plate::new(&row.text).width(wdt).height(38.0).font(Font::BODY.sized(15.0))).clicked() {
            acts.push(Act::Pick(vec![(m, i)]));
        }
        x = r.right() + 8.0;
    }
}

/// What the modern diplomacy screen shows this frame, from the classic screen's state.
pub fn view(h: &crate::ui::Hud, state: &ig2_sim::save::SaveState, rules: &ig2_rules::Rules, s: &crate::ui::UiSnap, d: &crate::diplomacy::Open, gates: Option<&[bool; 20]>, time: f32) -> View {
    use ig2_sim::diplomacy::level;
    let lang = h.texts.lang;
    let hu = d.texts.hungarian;
    let dec = |b: &[u8]| {
        let b = crate::diplomacy::articles(b, hu);
        ig2_formats::text::decode(b.split(|&c| c == 0).next().unwrap_or(&[]), lang).trim().to_string()
    };
    let ours = |e: &str| dec(&h.texts.ours(e));
    let dv = crate::diplomacy::View::read(state);
    let scr = &d.screen;
    let tone = |lvl: u8| match lvl {
        level::WAR | level::CEASEFIRE => Tone::Hostile,
        level::ALLIANCE | level::NAP => Tone::Friendly,
        _ => Tone::Neutral,
    };
    let name = |e: usize| dec(h.empire_names.get(e).map_or(&[][..], |n| &n[..]));
    let contacts = (0..dv.slots.len().min(crate::diplomacy::HEADS))
        .map(|k| {
            let e = dv.empire(k).clamp(0, 18) as usize;
            let lvl = dv.rel(dv.en, e).level;
            let alive = dv.alive_slot(k);
            Contact {
                k,
                name: name(e),
                status: if alive { dec(d.texts.status.get(lvl as usize).map_or(&[][..], |x| &x[..])) } else { dec(d.texts.phrases.get(6).map_or(&[][..], |x| &x[..])) },
                tone: tone(lvl),
                race: s.roles.get(e).copied().unwrap_or(1).clamp(1, 8) as usize,
                selected: scr.selected == k as i32,
                waiting: dv.slots[k].waiting != 0,
                alive,
            }
        })
        .collect();
    let info = (scr.selected >= 0).then(|| {
        let e = dv.empire(scr.selected as usize).clamp(0, 18) as usize;
        let r = dv.rel(dv.en, e);
        let treaty = h.texts.get(6, 10, if r.trade != 0 { 11 } else { 12 });
        (name(e), crate::diplomacy::known_planets(state, dv.en, e).to_string(), dec(&treaty), dec(d.texts.status.get(r.level as usize).map_or(&[][..], |x| &x[..])), tone(r.level))
    });
    let lists = [menu::MONEY, menu::HOSTAGE, menu::PLANETS, menu::FOR_HOSTAGE, menu::FOR_PLANETS, menu::INFO, menu::FOR_INFO, menu::TALK_EMPIRES, menu::ALLIES];
    let menus = scr
        .menus
        .iter()
        .map(|m| {
            m.entries
                .iter()
                .map(|en| {
                    let sub = (en.sub >= 0).then_some(en.sub as usize);
                    let listy = en.sub >= 0 && (lists.contains(&(en.sub as usize)) || (menu::TECHS as i32..=0x10).contains(&en.sub) || (menu::FOR_TECHS as i32..=0x24).contains(&en.sub) || en.sub == 0x32);
                    let empty = sub.is_some_and(|s| scr.menus.get(s).is_none_or(|x| x.entries.is_empty()));
                    Row { text: dec(&en.text), off: en.off || (listy && empty), sub }
                })
                .collect()
        })
        .collect();
    let label = |i: u16| dec(&h.texts.get(6, 10, i)).trim_end_matches(':').to_string();
    let other_tone = info.as_ref().map_or(Tone::Neutral, |i| i.4);
    // Each side's lines under its empire's name, as the board's "Empire: text".
    let said = |who: String, lines: &[Vec<u8>]| -> Vec<String> {
        let mut out: Vec<String> = lines.iter().map(|l| dec(l)).filter(|l| !l.is_empty()).collect();
        if let Some(first) = out.first_mut() {
            *first = format!("{who}: {first}");
        }
        out
    };
    let own = said(name(dv.en.min(18)), &scr.own_text);
    let other = said(info.as_ref().map(|i| i.0.clone()).unwrap_or_default(), &scr.other_text);
    View {
        bar: h.bar_view(state, rules, s, s.human as u8, gates, time, 6),
        live: scr.live(),
        contacts,
        info,
        menus,
        shown: scr.shown.clone(),
        own,
        other,
        own_alpha: (scr.own_fade / 127.0).clamp(0.0, 1.0),
        other_alpha: (scr.other_fade / 127.0).clamp(0.0, 1.0),
        other_tone,
        labels: Labels { contacts: ours("Contacts"), planets: label(9), treaty: label(10), status: label(13), exit: ours("Exit") },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str) -> View {
        let row = |t: &str, sub: Option<usize>| Row { text: t.into(), off: false, sub };
        let mut menus = vec![Vec::new(); menu::COUNT];
        menus[menu::MAIN] = vec![row(name, Some(menu::NEGOTIATE)), row(name, None)];
        menus[menu::NEGOTIATE] = vec![row(name, None), row(name, None)];
        menus[menu::SEND] = vec![row(name, None), row(name, None)];
        let contacts = (0..3).map(|k| Contact { k, name: name.into(), status: name.into(), race: 1, selected: k == 0, alive: true, ..Default::default() }).collect();
        View {
            live: true,
            contacts,
            info: Some((name.into(), "1".into(), name.into(), name.into(), Tone::Hostile)),
            menus,
            shown: vec![menu::MAIN, menu::SEND],
            own: vec![name.repeat(20)],
            other: vec![name.repeat(20)],
            own_alpha: 1.0,
            other_alpha: 1.0,
            labels: Labels { exit: name.into(), contacts: name.into(), ..Default::default() },
            ..Default::default()
        }
    }

    fn run(v: &View, clicks: &[egui::Pos2]) -> (Vec<Rect>, Vec<Act>) {
        let gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb);
        let mut st = State::default();
        let (mut acts, mut rects) = (Vec::new(), Vec::new());
        for f in 0..(6 + 3 * clicks.len()) {
            let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1920.0, 1080.0))), ..Default::default() };
            let button = |p, pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            if f >= 2 && let Some(&p) = clicks.get((f - 2) / 3) {
                input.events = match (f - 2) % 3 {
                    0 => vec![egui::Event::PointerMoved(p)],
                    1 => vec![button(p, true)],
                    _ => vec![button(p, false)],
                };
            }
            let _ = gui.ctx.run(input, |ctx| acts.extend(show(ctx, v, &mut st)));
            rects = ["dmh-contacts", "dmh-info", "dmh-menu0", "dmh-text", "dmh-exit"].map(|id| gui.ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()).to_vec();
        }
        (rects, acts)
    }

    /// Long texts never move a panel; a click on a row with a sub-menu opens it, a leaf picks the path; an answer
    /// button picks its one-level menu row; a contact selects it.
    #[test]
    fn panels_and_picks() {
        let (short, _) = run(&view("Tra"), &[]);
        let (long, _) = run(&view(&"Kémelhárítási jelentés ".repeat(10)), &[]);
        assert_eq!(short, long);
        let m = short[2];
        let first = pos2(m.left() + 60.0, m.top() + 10.0 + ROW_H / 2.0);
        let (_, a) = run(&view("Tra"), &[first]);
        assert!(!a.iter().any(|a| matches!(a, Act::Pick(_))), "{a:?}");
        let sub_leaf = pos2(m.right() + 12.0 + 60.0, m.top() + 10.0 + ROW_H * 1.5);
        let (_, a) = run(&view("Tra"), &[first, sub_leaf]);
        assert!(a.contains(&Act::Pick(vec![(menu::MAIN, 0), (menu::NEGOTIATE, 1)])), "{a:?}");
        let (_, a) = run(&view("Tra"), &[pos2(m.left() + 60.0, m.top() + 10.0 + ROW_H * 1.5)]);
        assert!(a.contains(&Act::Pick(vec![(menu::MAIN, 1)])), "{a:?}");
        let t = short[3];
        let (_, a) = run(&view("Tra"), &[pos2(t.left() + 24.0 + 30.0, t.bottom() - 18.0 - 19.0)]);
        assert!(a.contains(&Act::Pick(vec![(menu::SEND, 0)])), "{a:?}");
        let c = short[0];
        let (_, a) = run(&view("Tra"), &[pos2(c.left() + 100.0, c.top() + 30.0 + CARD_H + 12.0 + 50.0)]);
        assert!(a.contains(&Act::Select(1)), "{a:?}");
        let (_, a) = run(&view("Tra"), &[short[4].center()]);
        assert!(a.contains(&Act::Bar(sm::Act::Screen(2))), "{a:?}");
    }
}
