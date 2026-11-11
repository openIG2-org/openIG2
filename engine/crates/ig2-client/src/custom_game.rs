//! Ours (ADR 0085 point 4, part 2): the modern Custom game page: the galaxy's settings (planet count, seed) at the
//! left, a table of empires (the player and 1..=7 computer empires: race, name, start money, technology, start planets,
//! computer level, aggressiveness, start relation), a preview of the galaxy those settings start under the table, where a
//! click on a star picks the player's start (part 3, `re/rules/new-game.md` §13.2), Back and Start. The settings are
//! remembered in our settings file as the quick game's are (`CustomGame`).

use egui::{Pos2, Rect, Vec2};

use crate::gui::ds::{self, Font, Text, Weight};
use crate::ui::UiTexts;

/// The settings file's key of the last custom game's settings ([`Settings::from_setting`]).
pub const KEY: &str = "CustomGame";
/// The galaxy's planet count (ADR 0085 §1: the exe hangs at 200, so 150 is the cap).
pub const PLANETS: (i32, i32) = (20, 150);
/// The computer empires.
pub const COMPUTERS: (usize, usize) = (1, 7);
/// An empire's start planets.
pub const START_PLANETS: (u8, u8) = (1, 8);
/// An empire's name: `PlayerNev`'s 11-byte buffer less its terminator (re/rules/new-game.md §6).
pub const NAME_LEN: usize = 10;
const MONEY_MAX: i32 = 1_000_000;

/// One empire of a custom game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Empire {
    /// The race number (1..=8 and the mods' races).
    pub race: u8,
    /// Empty: the race's name (`PlayerNev = Faj_Nev[race]`).
    pub name: String,
    pub money: i32,
    /// 0..=3 (`ScirmishTalalmanyFejlesztes`'s levels).
    pub tech: u8,
    /// 1..=8.
    pub planets: u8,
    /// A computer's level: 1 easy, 2 normal, 3 hard (ADR 0033; the skirmish menu's difficulty).
    pub level: u8,
    /// A computer's aggressiveness: 0 low, 1 normal (the original's), 2 high.
    pub aggression: u8,
    /// Its start relation towards the other empires: 0 hostile, 1 neutral (the original's), 2 friendly.
    pub relation: u8,
}

/// A custom game's settings: the galaxy and the empires, the player's first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub planets: i32,
    pub seed: u32,
    pub empires: Vec<Empire>,
    /// The player's start planet: `None` the original's draw.
    pub start: Option<usize>,
}

/// `InitScirmishBolygok`'s start planets of an empire (re/rules/new-game.md §3) for population `p`, `n` planets and
/// `ais` computer empires.
fn start_planets(p: i32, n: i32, ais: i32) -> i32 {
    match p {
        1 => 1 + (n > 30) as i32 + (n > 60) as i32,
        2 => n / 15,
        3 => n / (ais + 3),
        _ => 1,
    }
}

impl Settings {
    /// The page's first settings: `InitScirmishValtozok`'s defaults (3 computers, every table at its step 2) with the
    /// races 1, 2, 3, 4 and the start planets of population 2.
    pub fn new(tables: &crate::mainscr_scenario::Tables, seed: u32) -> Self {
        let planets = tables.planets[2].clamp(PLANETS.0, PLANETS.1);
        let start = start_planets(2, planets, 3).clamp(START_PLANETS.0 as i32, START_PLANETS.1 as i32) as u8;
        let empire = |race| Empire { race, name: String::new(), money: tables.money[2].clamp(0, MONEY_MAX), tech: 2, planets: start, level: 2, aggression: 1, relation: 1 };
        Self { planets, seed, empires: (1..=4).map(empire).collect(), start: None }
    }

    /// The settings file's value: `planets,seed,start` (0 = random) then per empire
    /// `|race,money,tech,planets,level,aggression,relation,name`.
    pub fn to_setting(&self) -> String {
        let mut s = format!("{},{},{}", self.planets, self.seed, self.start.unwrap_or(0));
        for e in &self.empires {
            s += &format!("|{},{},{},{},{},{},{},{}", e.race, e.money, e.tech, e.planets, e.level, e.aggression, e.relation, e.name.replace('|', ""));
        }
        s
    }

    /// [`Settings::to_setting`] read back; a missing or out-of-range value keeps [`Settings::new`]'s.
    pub fn from_setting(v: Option<&str>, tables: &crate::mainscr_scenario::Tables, seed: u32) -> Self {
        let mut s = Self::new(tables, seed);
        let Some(v) = v else { return s };
        let mut parts = v.split('|');
        let num = |t: Option<&str>| t.and_then(|t| t.trim().parse::<i64>().ok());
        let in_range = |n: Option<i64>, lo: i64, hi: i64| n.filter(|n| (lo..=hi).contains(n));
        let mut g = parts.next().unwrap_or("").split(',');
        if let Some(n) = in_range(num(g.next()), PLANETS.0 as i64, PLANETS.1 as i64) {
            s.planets = n as i32;
        }
        if let Some(n) = in_range(num(g.next()), 0, u32::MAX as i64) {
            s.seed = n as u32;
        }
        s.start = in_range(num(g.next()), 1, s.planets as i64).map(|p| p as usize);
        let base = s.empires[1].clone();
        let empires: Vec<Empire> = parts
            .take(1 + COMPUTERS.1)
            .map(|p| {
                let mut f = p.splitn(8, ',');
                let mut e = base.clone();
                let set = |dst: &mut i64, lo: i64, hi: i64, t: Option<&str>| {
                    if let Some(n) = in_range(num(t), lo, hi) {
                        *dst = n;
                    }
                };
                let mut v = [e.race as i64, e.money as i64, e.tech as i64, e.planets as i64, e.level as i64, e.aggression as i64, e.relation as i64];
                let ranges = [(1, 255), (0, MONEY_MAX as i64), (0, 3), (START_PLANETS.0 as i64, START_PLANETS.1 as i64), (1, 3), (0, 2), (0, 2)];
                for (dst, (lo, hi)) in v.iter_mut().zip(ranges) {
                    set(dst, lo, hi, f.next());
                }
                (e.race, e.money, e.tech, e.planets, e.level, e.aggression, e.relation) = (v[0] as u8, v[1] as i32, v[2] as u8, v[3] as u8, v[4] as u8, v[5] as u8, v[6] as u8);
                e.name = f.next().unwrap_or("").chars().take(NAME_LEN).collect();
                e
            })
            .collect();
        if empires.len() > COMPUTERS.0 {
            s.empires = empires;
        }
        s
    }

    /// The sim's custom game (ADR 0085 §1, `ig2_sim::game::new_game::CustomGame`): the player is the empire of its
    /// race's number (`EN`, as the original's skirmish), each computer the empire of its own race's number while that is
    /// free, else the first free one; the rest are out of the game. A computer's level 1/2/3 is `Nehezseg` 1/2/4; the
    /// game's difficulty is the computers' mean level, the population (the people on the start planets) the one whose
    /// original count comes nearest the player's start planets. `name` gives a typed name's bytes in the game's font.
    pub fn custom(&self, kind: i32, name: impl Fn(&str) -> Vec<u8>) -> ig2_sim::game::new_game::CustomGame {
        use ig2_sim::game::new_game::{Aggressiveness, CustomEmpire, CustomGame, Side, StartRelation};
        let computers = &self.empires[1..];
        let ais = computers.len() as i32;
        let level = computers.iter().map(|e| e.level as i32).sum::<i32>() as f32 / computers.len().max(1) as f32;
        let me = &self.empires[0];
        // Ties go to the original's default (2).
        let population = [2, 1, 3, 0].into_iter().min_by_key(|&p| (start_planets(p, self.planets, ais) - me.planets as i32).abs()).unwrap_or(2);
        // Each row's empire number 1..=8.
        let mut slot: Vec<usize> = Vec::new();
        for e in &self.empires {
            let own = e.race as usize;
            let n = if (1..=8).contains(&own) && !slot.contains(&own) { own } else { (1..=8).find(|n| !slot.contains(n)).unwrap_or(8) };
            slot.push(n);
        }
        let off = CustomEmpire { side: Side::Off, race: 0, name: None, money: 0, tech: 0, planets: 1, level: 0, start: None, aggressiveness: Aggressiveness::Normal, relations: [StartRelation::Neutral; 9] };
        let mut empires: [CustomEmpire; 8] = std::array::from_fn(|i| CustomEmpire { race: i as u8 + 1, ..off.clone() });
        for (row, (e, &n)) in self.empires.iter().zip(&slot).enumerate() {
            let relation = [StartRelation::Hostile, StartRelation::Neutral, StartRelation::Friendly][e.relation.min(2) as usize];
            let mut relations = [relation; 9];
            relations[n] = StartRelation::Neutral;
            empires[n - 1] = CustomEmpire {
                side: if row == 0 { Side::Human } else { Side::Computer },
                race: e.race,
                name: (!e.name.is_empty()).then(|| name(&e.name)),
                money: e.money,
                tech: e.tech as i32,
                planets: e.planets as i32,
                level: if row == 0 { 0 } else { [1, 2, 4][(e.level.clamp(1, 3) - 1) as usize] },
                start: if row == 0 { self.start } else { None },
                aggressiveness: [Aggressiveness::Low, Aggressiveness::Normal, Aggressiveness::High][e.aggression.min(2) as usize],
                relations,
            };
        }
        CustomGame {
            seed: self.seed,
            ticks: self.seed,
            random_id: kind,
            planets: self.planets,
            difficulty: (level.round() as i32).clamp(1, 3),
            population,
            en: slot[0] as i32,
            drawn_opponents: None,
            empires,
        }
    }

    /// A computer empire more, of the first race nobody has.
    fn add(&mut self, races: &[u8]) {
        if self.empires.len() > COMPUTERS.1 {
            return;
        }
        let mut e = self.empires.last().cloned().unwrap_or_else(|| self.empires[0].clone());
        e.race = races.iter().copied().find(|r| !self.empires.iter().any(|x| x.race == *r)).unwrap_or(e.race);
        e.name.clear();
        self.empires.push(e);
    }
}

/// The money stepper's steps: 1000 to 10 000, 5000 to 100 000, 25 000 above.
fn money_step(v: i32, up: bool) -> i32 {
    let at = if up { v } else { v - 1 };
    let step = if at < 10_000 { 1000 } else if at < 100_000 { 5000 } else { 25_000 };
    let n = if up { (v / step + 1) * step } else { (v - 1).div_euclid(step) * step };
    n.clamp(0, MONEY_MAX)
}

/// What the page's map shows of the galaxy a custom game starts (spec §13.2, part 3): the start itself, run on a
/// scratch `Game` of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// Planets 1..=n (index 0 = planet 1): the galaxy position (`+0x18`, `+0x1c`) and the owner (`+0x20`, 0 nobody).
    pub planets: Vec<([f32; 2], u8)>,
    /// Each living empire's start planet (`BirodalomBolygo` 0xF17394 + 4e, index 1..=8; 0 none).
    pub homes: [usize; 9],
}

impl Preview {
    pub fn of(state: &ig2_sim::save::SaveState) -> Self {
        use ig2_sim::save::{va, PLANET_SIZE};
        let planets = (1..=state.planet_count())
            .map(|p| {
                let at = va::PLANETS + (p * PLANET_SIZE) as u32;
                ([state.mem.f32(at + 0x18), state.mem.f32(at + 0x1c)], state.mem.u8(at + 0x20))
            })
            .collect();
        let alive = state.alive();
        let homes = std::array::from_fn(|e| if (1..9).contains(&e) && alive[e] { state.mem.i32(0xF17394 + 4 * e as u32).max(0) as usize } else { 0 });
        Self { planets, homes }
    }

    /// The galaxy a custom game starts: [`crate::new_game::custom`] on a `Game` of its own.
    pub fn generate(install: &ig2_formats::Install, cfg: &ig2_sim::game::new_game::CustomGame, rules: &ig2_sim::game::GameRules) -> Option<Self> {
        crate::new_game::custom(install, cfg, rules).map(|g| Self::of(&g.state)).map_err(|e| eprintln!("ig2-client: custom game preview: {e}")).ok()
    }

    fn same_stars(&self, o: &Self) -> bool {
        self.planets.len() == o.planets.len() && self.planets.iter().zip(&o.planets).all(|(a, b)| a.0 == b.0)
    }
}

/// The map's view of the planets in `r`: their bounds fitted, one scale for both axes, the galaxy's +x to the left and
/// +y up as on the starmap's minimap (`crate::orders::minimap_target`, exe 0x5b2ec9).
fn map_view(r: Rect, planets: &[([f32; 2], u8)]) -> impl Fn([f32; 2]) -> Pos2 {
    let (lo, hi) = planets.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), (p, _)| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let s = (r.width() / (hi[0] - lo[0]).max(1.0)).min(r.height() / (hi[1] - lo[1]).max(1.0));
    let o = r.center();
    move |p| Pos2::new(o.x + (c[0] - p[0]) * s, o.y + (c[1] - p[1]) * s)
}

/// The field the keys type into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Seed,
    Name(usize),
}

/// The galaxy column's width.
const SIDE: f32 = 340.0;
/// The empires table's narrowest: its fixed columns and gaps (432) and six drop-downs and fields 110 wide. A narrower
/// page puts the galaxy under the table.
const TABLE_MIN: f32 = 1092.0;
/// The map's height on a narrower page (its row beside the galaxy column).
const NARROW_MAP_H: f32 = 360.0;

/// What a frame of the page gathered: its act, a field under the pointer, a drop-down open as the frame began (this
/// frame's click only closes it).
#[derive(Default)]
struct Out {
    act: Option<Act>,
    on_field: bool,
    dropped: bool,
}

/// What the page asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// A setting changed (the click's sound).
    Click,
    Back,
    Start,
}

/// The page's state.
pub struct CustomPage {
    pub s: Settings,
    focus: Option<Focus>,
    /// The skirmish entry's `NewgameRandomID`.
    pub kind: i32,
    /// The map: the start it was made from (names left out) and what it shows (`None`: the start failed).
    preview: Option<(ig2_sim::game::new_game::CustomGame, Option<Preview>)>,
    /// Empires 0..=8's colours (`DDColor[e + 15]`, the starmap's).
    colours: [egui::Color32; 9],
}

impl CustomPage {
    /// The page with the last custom game's settings (our settings file), a new seed when there are none.
    pub fn open(tables: &crate::mainscr_scenario::Tables, kind: i32, seed: u32, install: &ig2_formats::Install) -> Self {
        let palette = crate::starmap::read_palette(install).unwrap_or_default();
        let colours = std::array::from_fn(|e| palette.get(e + 15).map_or(egui::Color32::GRAY, |c| egui::Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)));
        Self { s: Settings::from_setting(ig2_formats::options::Options::load().get(KEY), tables, seed), focus: None, kind, preview: None, colours }
    }

    /// The start the map shows: the page's custom game without the names (they reach no draw).
    fn galaxy_key(&self) -> ig2_sim::game::new_game::CustomGame {
        let mut c = self.s.custom(self.kind, |_| Vec::new());
        for e in &mut c.empires {
            e.name = None;
        }
        c
    }

    /// A new map when the start changed; a chosen star is dropped when the planets moved (another galaxy).
    fn refresh(&mut self, galaxy: impl FnOnce(&ig2_sim::game::new_game::CustomGame) -> Option<Preview>) {
        let key = self.galaxy_key();
        if self.preview.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let new = galaxy(&key);
        let old = self.preview.as_ref().and_then(|(_, p)| p.as_ref());
        if let (Some(old), Some(new)) = (old, &new) {
            if self.s.start.is_some() && !old.same_stars(new) {
                // The next frame makes the random start's map.
                self.s.start = None;
            }
        }
        self.preview = Some((key, new));
    }

    /// The map under the empires table: the stars, the start planets in their empire's colour, the player's ringed; a
    /// click on a star picks the player's start, the Random start plate sets it back.
    fn map(&mut self, ui: &mut egui::Ui, size: Vec2, tr: &dyn Fn(&str) -> String, dropped: bool) -> bool {
        let k = ds::tokens();
        let mut changed = false;
        let head = 40.0;
        let (row, _) = ui.allocate_exact_size(Vec2::new(size.x, head), egui::Sense::hover());
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        ds::heading_label(&mut c, &tr("Start position"));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if c.add_enabled(self.s.start.is_some(), ds::Plate::new(&tr("Random start")).width(220.0).height(36.0)).clicked() {
            self.s.start = None;
            changed = true;
        }
        let hint = Font { size: 14.0, line: 20.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
        c.add_space(16.0);
        let text = if self.s.start.is_some() { tr("You start on the star you picked.") } else { tr("Click a star to start there.") };
        // What the heading and the plate leave of the row (a narrower map's).
        let room = (c.available_width() - Text::new(&tr("Start position"), Font::LABEL).layout(c.ctx(), f32::INFINITY).width - 40.0).clamp(0.0, 360.0);
        Text::new(&text, hint).color(k.ink_muted).shrink(11.0).show(&mut c, Vec2::new(room, 20.0));
        ui.add_space(8.0);
        let h = (size.y - head - 8.0).max(60.0);
        let (frame, resp) = ui.allocate_exact_size(Vec2::new(size.x, h), egui::Sense::click());
        let p = ui.painter_at(frame);
        p.rect_filled(frame, ds::RADIUS_PLATE, k.glass_deep);
        ds::rim(&p, frame, ds::RADIUS_PLATE, k.rim);
        let Some(map) = self.preview.as_ref().and_then(|(_, m)| m.as_ref()) else { return changed };
        let at_map = map_view(frame.shrink(24.0), &map.planets);
        let en = self.preview.as_ref().map_or(0, |(c, _)| c.en as usize);
        // The planet under the pointer: the nearest within 12 px.
        let hover = resp.hover_pos().and_then(|at| {
            map.planets.iter().enumerate().map(|(i, s)| (i + 1, at_map(s.0).distance(at))).filter(|&(_, d)| d < 12.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(p, _)| p)
        });
        for (i, &(pos, owner)) in map.planets.iter().enumerate() {
            let at = at_map(pos);
            let home = owner != 0 && map.homes.get(owner as usize) == Some(&(i + 1));
            match (owner, home) {
                (0, _) => _ = p.circle_filled(at, 2.5, k.ink_muted),
                (_, false) => _ = p.circle_filled(at, 3.5, self.colours[owner as usize % 9]),
                (_, true) => {
                    p.circle_filled(at, 5.5, self.colours[owner as usize % 9]);
                    p.circle_stroke(at, 8.0, egui::Stroke::new(1.0f32, self.colours[owner as usize % 9]));
                }
            }
        }
        if let Some(&home) = map.homes.get(en).filter(|&&h| h > 0) {
            let at = at_map(map.planets[home - 1].0);
            p.circle_stroke(at, 12.0, egui::Stroke::new(2.0f32, k.focus));
            let f = Font { size: 13.0, line: 16.0, weight: Weight::Medium, tracking: 0.06, upper: true, lcd: false };
            let r = Rect::from_min_size(at + Vec2::new(16.0, -8.0), Vec2::new(160.0, 16.0));
            Text::new(&tr("Your start"), f).color(k.focus).paint(ui, r);
        }
        if let Some(h) = hover {
            p.circle_stroke(at_map(map.planets[h - 1].0), 10.0, egui::Stroke::new(1.5f32, k.ink));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if resp.clicked() && !dropped && self.s.start != Some(h) {
                self.s.start = Some(h);
                changed = true;
            }
        }
        changed
    }

    /// A field takes the keys.
    pub fn typing(&self) -> bool {
        self.focus.is_some()
    }

    /// A key: typing into the seed or a name while one takes the keys (Enter, Esc or Tab end it; `chord`, Ctrl or Alt
    /// held, types nothing); else Esc is Back, Enter Start, neither while a drop-down is `popup` open.
    pub fn key(&mut self, key: &winit::keyboard::Key, chord: bool, popup: bool) -> Option<Act> {
        use winit::keyboard::{Key, NamedKey};
        if popup {
            return None;
        }
        let Some(f) = self.focus else {
            return match key.as_ref() {
                Key::Named(NamedKey::Escape) => Some(Act::Back),
                Key::Named(NamedKey::Enter) => Some(Act::Start),
                _ => None,
            };
        };
        match key.as_ref() {
            Key::Named(NamedKey::Escape | NamedKey::Enter | NamedKey::Tab) => self.focus = None,
            Key::Named(NamedKey::Backspace) => match f {
                Focus::Seed => self.s.seed /= 10,
                Focus::Name(i) => _ = self.s.empires.get_mut(i).and_then(|e| e.name.pop()),
            },
            _ if chord => {}
            Key::Named(NamedKey::Space) => self.type_text(f, " "),
            Key::Character(c) => self.type_text(f, c),
            _ => {}
        }
        None
    }

    fn type_text(&mut self, f: Focus, text: &str) {
        match f {
            Focus::Seed => {
                for d in text.chars().filter_map(|c| c.to_digit(10)) {
                    if let Some(n) = self.s.seed.checked_mul(10).and_then(|n| n.checked_add(d)) {
                        self.s.seed = n;
                    }
                }
            }
            Focus::Name(i) => {
                if let Some(e) = self.s.empires.get_mut(i) {
                    for c in text.chars().filter(|c| !c.is_control() && *c != '|') {
                        if e.name.chars().count() < NAME_LEN {
                            e.name.push(c);
                        }
                    }
                }
            }
        }
    }

    /// One frame of the page; `heads` gives a race's portrait, `galaxy` the map of a start ([`Preview::generate`]).
    pub fn show(&mut self, ctx: &egui::Context, texts: &UiTexts, mut heads: impl FnMut(&egui::Context, u8) -> Option<egui::TextureHandle>, galaxy: impl FnOnce(&ig2_sim::game::new_game::CustomGame) -> Option<Preview>) -> Option<Act> {
        // Not while a field takes the keys: each typed seed digit would build a galaxy.
        if self.focus.is_none() {
            self.refresh(galaxy);
        }
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let game = |b: &[u8]| ig2_formats::text::decode(b, texts.lang).trim().trim_end_matches(':').trim().to_string();
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        let mut out = Out::default();
        crate::gui::board_at(ctx, "custom-game", Vec2::new(1760.0, 900.0), 0.55, 12.0, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let w = ui.available_width();
            Text::new(&tr("Custom game"), Font::PAGE_TITLE).color(k.ink_silver).show(ui, Vec2::new(w, 36.0));
            ui.add_space(20.0);
            let body = ui.available_height() - 20.0 - 64.0;
            if w >= SIDE + 32.0 + TABLE_MIN {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.vertical(|ui| {
                        ui.set_width(SIDE);
                        ui.set_height(body);
                        self.galaxy_column(ui, texts, &mut out);
                    });
                    ui.add_space(32.0);
                    ui.vertical(|ui| {
                        let tw = w - SIDE - 32.0;
                        ui.set_width(tw);
                        ui.set_height(body);
                        self.empire_table(ui, tw, texts, &mut heads, &mut out);
                        ui.add_space(24.0);
                        let rest = Vec2::new(tw, ui.available_height());
                        if self.map(ui, rest, &tr, out.dropped) {
                            out.act = Some(Act::Click);
                        }
                    });
                });
            } else {
                // A narrower page (a larger UI scale): the table over the galaxy and the map, all scrolling (sideways
                // too, where the table's narrowest is wider than the page).
                // Bars that show without the pointer over them: the page hints at what lies past its edge.
                ui.spacing_mut().scroll = egui::style::ScrollStyle { bar_width: 8.0, ..egui::style::ScrollStyle::solid() };
                let cw = (w - ui.spacing().scroll.allocated_width()).max(TABLE_MIN);
                egui::ScrollArea::both().id_salt("cg-body").max_height(body).auto_shrink([false; 2]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.vertical(|ui| {
                        ui.set_width(cw);
                        self.empire_table(ui, cw, texts, &mut heads, &mut out);
                        ui.add_space(24.0);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            ui.vertical(|ui| {
                                ui.set_width(SIDE);
                                self.galaxy_column(ui, texts, &mut out);
                            });
                            ui.add_space(32.0);
                            ui.vertical(|ui| {
                                if self.map(ui, Vec2::new(cw - SIDE - 32.0, NARROW_MAP_H), &tr, out.dropped) {
                                    out.act = Some(Act::Click);
                                }
                            });
                        });
                    });
                });
            }
            ui.add_space(20.0);
            // The footer: Start and Back.
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 64.0), egui::Sense::hover());
            ui.painter().hline(rect.x_range(), rect.top() + 0.5, egui::Stroke::new(1.0f32, k.rim));
            let row = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + 20.0), rect.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
            child.spacing_mut().item_spacing.x = 12.0;
            if child.add(ds::Plate::new(&game(&texts.get(15, 2, 2))).primary(true).width(200.0)).clicked() {
                out.act = Some(Act::Start);
            }
            if child.add(ds::Plate::new(&game(&texts.get(15, 2, 3))).width(200.0)).clicked() {
                out.act = Some(Act::Back);
            }
        });
        if pressed && !out.on_field {
            self.focus = None;
        }
        out.act
    }

    /// The galaxy's settings ([`SIDE`] wide): the planet count, the seed, New seed.
    fn galaxy_column(&mut self, ui: &mut egui::Ui, texts: &UiTexts, out: &mut Out) {
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let game = |b: &[u8]| ig2_formats::text::decode(b, texts.lang).trim().trim_end_matches(':').trim().to_string();
        ui.spacing_mut().item_spacing.y = 12.0;
        ds::heading_label(ui, &tr("Galaxy"));
        let label = |ui: &mut egui::Ui, s: &str| Text::new(s, Font::LABEL).color(k.ink_muted).show(ui, Vec2::new(SIDE, 18.0));
        label(ui, &game(&texts.get(15, 2, 8)));
        let (_, d) = ds::stepper(ui, &self.s.planets.to_string(), SIDE - 64.0);
        if d != 0 {
            let n = (self.s.planets + 5 * d).clamp(PLANETS.0, PLANETS.1);
            if n != self.s.planets {
                self.s.planets = n;
                out.act = Some(Act::Click);
            }
        }
        ui.add_space(8.0);
        label(ui, &tr("Seed"));
        let caret = (self.focus == Some(Focus::Seed)).then(|| self.s.seed.to_string().len());
        let resp = ds::text_field(ui, &self.s.seed.to_string(), caret, "", Vec2::new(SIDE, 40.0)).on_hover_cursor(egui::CursorIcon::Text);
        out.on_field |= resp.hovered();
        if resp.clicked() {
            self.focus = Some(Focus::Seed);
        }
        if ui.add(ds::Plate::new(&tr("New seed")).width(SIDE).height(40.0)).clicked() {
            self.s.seed = crate::new_game::wall_ticks().wrapping_mul(2_654_435_761);
            out.act = Some(Act::Click);
        }
        ui.add_space(8.0);
        let hint = Font { size: 14.0, line: 20.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
        let text = tr("The same seed and settings make the same galaxy.");
        Text::new(&text, hint).color(k.ink_muted).lines(3).show(ui, Vec2::new(SIDE, 60.0));
    }

    /// The empires' table `tw` wide and Add computer empire under it.
    fn empire_table(&mut self, ui: &mut egui::Ui, tw: f32, texts: &UiTexts, heads: &mut dyn FnMut(&egui::Context, u8) -> Option<egui::TextureHandle>, out: &mut Out) {
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let game = |b: &[u8]| ig2_formats::text::decode(b, texts.lang).trim().trim_end_matches(':').trim().to_string();
        let races = texts.playable_races();
        let race_name = |r: u8| game(&texts.race(r));
        ds::heading_label(ui, &tr("Empires"));
        ui.add_space(12.0);
        let widths = [48.0, 0.0, 0.0, 150.0, 0.0, 104.0, 0.0, 0.0, 0.0, 40.0];
        let (head, _) = ui.allocate_exact_size(Vec2::new(tw, 24.0), egui::Sense::hover());
        let cols = ds::columns(head, &widths, 10.0);
        let heads_l = ["".to_string(), tr("Race"), tr("Name"), game(&texts.get(15, 2, 0xb)), game(&texts.get(15, 2, 0xa)), tr("Planets"), tr("Computer"), tr("Aggressiveness"), tr("Relations"), String::new()];
        let refs: Vec<&str> = heads_l.iter().map(String::as_str).collect();
        ds::table_head(ui, &cols, &refs, &[]);
        ui.add_space(6.0);
        let mut remove = None;
        let n = self.s.empires.len();
        let pick = Font { size: 16.0, line: 20.0, weight: Weight::Medium, tracking: 0.04, upper: false, lcd: false };
        for i in 0..n {
            let (row, resp) = ui.allocate_exact_size(Vec2::new(tw, 52.0), egui::Sense::hover());
            ds::row_back(ui, row, ds::hover_t(ui.ctx(), resp.id, ui.rect_contains_pointer(row)) * 0.6, i == 0);
            let c = ds::columns(row, &widths, 10.0);
            let cell = |r: Rect, h: f32| Rect::from_center_size(r.center(), Vec2::new(r.width(), h));
            let mut u: Vec<egui::Ui> = c.iter().map(|&r| ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)))).collect();
            let e = &mut self.s.empires[i];
            // The race's head.
            let hr = Rect::from_center_size(c[0].center(), Vec2::splat(40.0));
            let u0 = &u[0];
            u0.painter().rect_filled(hr, ds::RADIUS_PLATE, egui::Color32::BLACK);
            if let Some(t) = heads(u0.ctx(), e.race) {
                egui::Image::new(egui::load::SizedTexture::new(t.id(), hr.size())).corner_radius(ds::RADIUS_PLATE).paint_at(u0, hr);
            }
            ds::rim(u0.painter(), hr, ds::RADIUS_PLATE, k.rim);
            let mut select = |j: usize, id: &str, value: String, rows: Vec<(i32, String, bool)>| -> Option<i32> {
                let sid = u[j].id().with((id, i));
                out.dropped |= u[j].memory(|m| m.data.get_temp::<bool>(sid).unwrap_or(false));
                ds::select(&mut u[j], (id, i), &value, &rows, cell(c[j], 38.0).size(), pick, k.ink).1.flatten()
            };
            let race_rows = races.iter().map(|&r| (r as i32, race_name(r), false)).collect();
            if let Some(r) = select(1, "cg-race", race_name(e.race), race_rows) {
                e.race = r as u8;
                out.act = Some(Act::Click);
            }
            let tech_rows = (0..=3).map(|t| (t, game(&texts.get(15, 5, t as u16 + 1)), false)).collect();
            if let Some(t) = select(4, "cg-tech", game(&texts.get(15, 5, e.tech as u16 + 1)), tech_rows) {
                e.tech = t as u8;
                out.act = Some(Act::Click);
            }
            if i > 0 {
                let level_rows = (1..=3).map(|l| (l, game(&texts.get(15, 4, l as u16)), false)).collect();
                if let Some(l) = select(6, "cg-level", game(&texts.get(15, 4, e.level as u16)), level_rows) {
                    e.level = l as u8;
                    out.act = Some(Act::Click);
                }
                let aggr = ["Low", "Normal", "High"];
                let aggr_rows = (0..3).map(|a| (a, tr(aggr[a as usize]), false)).collect();
                if let Some(a) = select(7, "cg-aggr", tr(aggr[e.aggression.min(2) as usize]), aggr_rows) {
                    e.aggression = a as u8;
                    out.act = Some(Act::Click);
                }
                let rel = ["Hostile", "Neutral", "Friendly"];
                let rel_rows = (0..3).map(|a| (a, tr(rel[a as usize]), false)).collect();
                if let Some(a) = select(8, "cg-rel", tr(rel[e.relation.min(2) as usize]), rel_rows) {
                    e.relation = a as u8;
                    out.act = Some(Act::Click);
                }
            } else {
                let f = Font { size: 16.0, line: 20.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
                Text::new(&tr("You"), f).color(k.info).paint(&u[6], c[6].shrink2(Vec2::new(14.0, 0.0)));
            }
            let e = &mut self.s.empires[i];
            // The name.
            let caret = (self.focus == Some(Focus::Name(i))).then(|| e.name.chars().count());
            let resp = ds::text_field(&mut u[2], &e.name, caret, &race_name(e.race), cell(c[2], 38.0).size()).on_hover_cursor(egui::CursorIcon::Text);
            out.on_field |= resp.hovered();
            if resp.clicked() {
                self.focus = Some(Focus::Name(i));
            }
            // The money and the start planets.
            let (_, d) = ds::stepper(&mut u[3], &format!("$ {}", e.money), c[3].width() - 64.0);
            if d != 0 {
                e.money = money_step(e.money, d > 0);
                out.act = Some(Act::Click);
            }
            let (_, d) = ds::stepper(&mut u[5], &e.planets.to_string(), c[5].width() - 64.0);
            let p = (e.planets as i32 + d).clamp(START_PLANETS.0 as i32, START_PLANETS.1 as i32) as u8;
            if p != e.planets {
                e.planets = p;
                out.act = Some(Act::Click);
            }
            if i > 0 && n - 1 > COMPUTERS.0 && ds::pressed(&u[9].add(ds::IconKey::icon(ds::Icon::Trash).size(Vec2::splat(36.0)).tip(&tr("Remove")))) {
                remove = Some(i);
            }
            ui.add_space(6.0);
        }
        if let Some(i) = remove {
            self.s.empires.remove(i);
            self.focus = None;
            out.act = Some(Act::Click);
        }
        ui.add_space(10.0);
        if ui.add_enabled(self.s.empires.len() <= COMPUTERS.1, ds::Plate::new(&tr("Add computer empire")).width(300.0).height(40.0)).clicked() {
            self.s.add(&races);
            out.act = Some(Act::Click);
        }
    }
}

/// `--snapshot OUT --screen custom [--mouse X,Y]`: the page with the remembered settings over black.
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    let l = crate::load(o)?;
    let mut r = crate::render::Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let hud = crate::ui::Hud::load(&mut r, &l.install, &l.tables);
    let mut gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb).install_fonts(&l.install);
    let a = crate::mainscr_scenario::Assets::load(&l.install, &hud.texts);
    let mut page = CustomPage::open(&a.tables, 1, 1234, &l.install);
    let mut pics = crate::scenarios_modern::Pics::default();
    let style = crate::gui::style(&crate::theme::Theme::modern());
    let k = crate::gui::Gui::scale(o.size.1);
    let v = crate::font::Virtual::new([o.size.0 as f32, o.size.1 as f32]);
    gui.pointer(o.mouse.map(|m| v.px(m[0], m[1])), k);
    for _ in 0..40 {
        gui.run(o.size, &style, |ctx| _ = page.show(ctx, &hud.texts, |ctx, race| pics.head(ctx, &l.install, race as i32), |c| Preview::generate(&l.install, c, &l.rules)));
    }
    let rgba = r.draw_to_image_over(&crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]), o.size, 0.0, |d, q, t| gui.paint(d, q, t));
    crate::assets::write_snapshot(out, o, &rgba)?;
    println!("{}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mainscr_scenario::Tables;

    const T: Tables = Tables { planets: [40, 60, 80, 100, 120], money: [5000, 10000, 20000, 40000, 80000, 160000] };

    /// The defaults: the original's step-2 values, four empires.
    #[test]
    fn defaults() {
        let s = Settings::new(&T, 7);
        assert_eq!((s.planets, s.seed, s.empires.len()), (80, 7, 4));
        assert_eq!(s.empires.iter().map(|e| e.race).collect::<Vec<_>>(), [1, 2, 3, 4]);
        assert!(s.empires.iter().all(|e| e.money == 20000 && e.tech == 2 && e.planets == 5 && e.level == 2 && e.aggression == 1 && e.relation == 1));
    }

    /// The settings file's value round-trips; bad values keep the defaults.
    #[test]
    fn setting() {
        let mut s = Settings::new(&T, 7);
        s.planets = 150;
        s.seed = 4_000_000_000;
        s.empires[0].name = "Kirk".into();
        s.empires[2] = Empire { race: 8, name: "A|b".into(), money: 0, tech: 3, planets: 8, level: 3, aggression: 2, relation: 0 };
        s.add(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let back = Settings::from_setting(Some(&s.to_setting()), &T, 1);
        s.empires[2].name = "Ab".into();
        assert_eq!(back, s);
        assert_eq!(back.empires[4].race, 3);
        let bad = Settings::from_setting(Some("999,x|9,-5,7,0,9,9,9,TooLongName12"), &T, 3);
        assert_eq!((bad.planets, bad.seed, bad.empires.len()), (80, 3, 4));
        let one = Settings::from_setting(Some("30,5|2,7000,1,3,1,0,2,Me|3,0,0,1,3,2,0,"), &T, 3);
        assert_eq!(one.planets, 30);
        assert_eq!(one.empires[0], Empire { race: 2, name: "Me".into(), money: 7000, tech: 1, planets: 3, level: 1, aggression: 0, relation: 2 });
    }

    /// The sim's custom game: the empire numbers by race, the levels as `Nehezseg`, the names, the rest out.
    #[test]
    fn custom() {
        use ig2_sim::game::new_game::{Aggressiveness, Side, StartRelation};
        let mut s = Settings::new(&T, 99);
        s.planets = 90;
        s.empires[0] = Empire { race: 3, name: "Kirk".into(), money: 33000, tech: 1, planets: 3, ..s.empires[0].clone() };
        // The computers' races: 2, 3 (the player's), 4.
        s.empires[2].level = 3;
        s.empires[2].aggression = 2;
        s.empires[2].relation = 2;
        let c = s.custom(1, |n| n.bytes().collect());
        assert_eq!((c.en, c.random_id, c.planets, c.seed, c.ticks, c.drawn_opponents), (3, 1, 90, 99, 99, None));
        assert_eq!(c.population, 1);
        let sides: Vec<Side> = c.empires.iter().map(|e| e.side).collect();
        assert_eq!(sides, [Side::Computer, Side::Computer, Side::Human, Side::Computer, Side::Off, Side::Off, Side::Off, Side::Off]);
        let me = &c.empires[2];
        assert_eq!((me.race, me.name.as_deref(), me.money, me.tech, me.planets, me.level), (3, Some(&b"Kirk"[..]), 33000, 1, 3, 0));
        // The second computer (race 3) took empire 1, the first free one.
        let e1 = &c.empires[0];
        assert_eq!((e1.race, e1.name.as_deref(), e1.level, e1.aggressiveness), (3, None, 4, Aggressiveness::High));
        assert_eq!((e1.relations[3], e1.relations[1]), (StartRelation::Friendly, StartRelation::Neutral));
        assert_eq!((c.empires[1].level, c.empires[3].race), (2, 4));
        assert_eq!(c.clamped(), c);
    }

    /// Typing: digits into the seed, a capped name, chords and an open drop-down ignored; Esc ends a field, then is Back.
    #[test]
    fn keys() {
        use winit::keyboard::{Key, NamedKey};
        let ch = |s: &str| Key::Character(s.into());
        let mut p = CustomPage { s: Settings::new(&T, 12), focus: Some(Focus::Seed), kind: 1, preview: None, colours: [egui::Color32::GRAY; 9] };
        p.key(&ch("3"), false, false);
        p.key(&ch("x"), false, false);
        p.key(&Key::Named(NamedKey::Backspace), false, false);
        p.key(&ch("45"), false, false);
        assert_eq!(p.s.seed, 1245);
        p.focus = Some(Focus::Name(1));
        p.key(&ch("Abcdefghijkl"), false, false);
        p.key(&ch("v"), true, false);
        assert_eq!(p.s.empires[1].name, "Abcdefghij");
        assert_eq!(p.key(&Key::Named(NamedKey::Escape), false, false), None);
        assert!(!p.typing());
        assert_eq!(p.key(&Key::Named(NamedKey::Escape), false, true), None);
        assert_eq!(p.key(&Key::Named(NamedKey::Escape), false, false), Some(Act::Back));
        assert_eq!(p.key(&Key::Named(NamedKey::Enter), false, false), Some(Act::Start));
    }

    /// Spec §13.2 part 3: a new map only when the start changed; a picked star stays while the planets stay, and is
    /// dropped when they moved (another seed).
    #[test]
    fn refresh() {
        let mut p = CustomPage { s: Settings::new(&T, 12), focus: None, kind: 1, preview: None, colours: [egui::Color32::GRAY; 9] };
        let map = |x: f32| Some(Preview { planets: vec![([x, 0.0], 0)], homes: [0; 9] });
        let mut calls = 0;
        p.refresh(|_| { calls += 1; map(1.0) });
        p.s.start = Some(1);
        p.refresh(|_| { calls += 1; map(1.0) });
        p.refresh(|_| { calls += 1; map(1.0) });
        assert_eq!((calls, p.s.start), (2, Some(1)));
        p.s.seed += 1;
        p.refresh(|_| map(2.0));
        assert_eq!(p.s.start, None);
    }

    /// Spec §13.2 part 3 (IG2_INSTALL_DIR; skipped without it): the map is the start's galaxy, a picked star becomes
    /// the player's start planet in the same galaxy, and the pick is kept with the settings.
    #[test]
    fn preview_is_the_start() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let (rules, _) = crate::rules::load(&install).unwrap();
        let mut s = Settings::new(&T, 4242);
        let cfg = |s: &Settings| s.custom(1, |n| n.bytes().collect());
        let t = std::time::Instant::now();
        let map = Preview::generate(&install, &cfg(&s), &rules).unwrap();
        eprintln!("preview in {:?}", t.elapsed());
        assert_eq!(map.planets.len(), 80);
        let real = crate::new_game::custom(&install, &cfg(&s), &rules).unwrap();
        assert_eq!(Preview::of(&real.state), map);
        let en = cfg(&s).en as usize;
        assert!(map.homes[en] > 0 && (1..=4).all(|e| map.planets[map.homes[e] - 1].1 == e as u8));
        let free = (1..=80).find(|&p| map.planets[p - 1].1 == 0).unwrap();
        s.start = Some(free);
        let picked = Preview::generate(&install, &cfg(&s), &rules).unwrap();
        assert!(picked.same_stars(&map));
        assert_eq!((picked.homes[en], picked.planets[free - 1].1), (free, en as u8));
        assert_eq!(Settings::from_setting(Some(&s.to_setting()), &T, 1).start, Some(free));
    }

    #[test]
    fn money_steps() {
        assert_eq!(money_step(0, true), 1000);
        assert_eq!(money_step(0, false), 0);
        assert_eq!(money_step(9000, true), 10000);
        assert_eq!(money_step(10000, false), 9000);
        assert_eq!(money_step(12345, true), 15000);
        assert_eq!(money_step(12345, false), 10000);
        assert_eq!(money_step(100000, true), 125000);
        assert_eq!(money_step(MONEY_MAX, true), MONEY_MAX);
    }
}
