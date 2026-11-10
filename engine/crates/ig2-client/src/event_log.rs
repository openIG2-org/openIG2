//! Ours (ADR 0040, re/screens/messages.md §9): the event log. Every scrolling line the sim gives the local empire
//! (`Game::messages_for`, the line's own texts) is kept with its date, a category and the planet or fleet it is
//! about; the messages screen shows them in a panel with a filter row and a search field, and a line leads to its
//! planet or fleet on the starmap. Client-side only: nothing goes into the sim or the lockstep.

use ig2_formats::text::Lang;
use ig2_sim::colony::ColonyEvent;
use ig2_sim::colony_side::SideEvent;
use ig2_sim::diplomacy::DiploEvent;
use ig2_sim::fleet::StarmapEvent;
use ig2_sim::game::{BattleEvent, Game, GameEvent};
use ig2_sim::ground::GroundEvent;
use ig2_sim::spy::{self, SpyEvent};
use ig2_sim::time::GameTime;

use crate::chat::EntryKey;
use crate::font::{Align, Font};
use crate::kit::{inside_f, Virtual};
use crate::render::{Frame, TexId};

/// The newest lines kept.
pub const KEEP: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Battle,
    Research,
    Diplomacy,
    Colony,
    Story,
    /// Only under "All" (a lost contact).
    Other,
}

/// The filter row: its label and the category it shows (`None` = all).
pub const FILTERS: [(&str, Option<Category>); 6] = [
    ("All", None),
    ("Battles", Some(Category::Battle)),
    ("Research", Some(Category::Research)),
    ("Diplomacy", Some(Category::Diplomacy)),
    ("Colonies", Some(Category::Colony)),
    ("Story", Some(Category::Story)),
];

/// What a line is about: where a click on it leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Planet(usize),
    Fleet(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// `Ido` when it was posted.
    pub ido: i32,
    pub text: Vec<u8>,
    pub category: Category,
    pub target: Option<Target>,
}

/// One game's lines, oldest first.
#[derive(Debug, Clone, Default)]
pub struct Log {
    pub entries: Vec<Entry>,
}

impl Log {
    /// The lines `events` post for empire `en` (each event's own, so it keeps its category and target).
    pub fn record(&mut self, game: &Game, events: &[GameEvent], en: usize, ido: i32) {
        for ev in events {
            let posts = game.messages_for(std::slice::from_ref(ev), en);
            if posts.is_empty() {
                continue;
            }
            let (category, target) = classify(ev);
            self.entries.extend(posts.into_iter().map(|(_, text, _, _)| Entry { ido, text, category, target }));
        }
        let over = self.entries.len().saturating_sub(KEEP);
        self.entries.drain(..over);
    }
}

/// A planet index as a target (planets are 1-based; 0 is none).
fn planet(p: usize) -> Option<Target> {
    (p > 0).then_some(Target::Planet(p))
}

/// The category of the lines `ev` posts, and the planet or fleet they are about.
pub fn classify(ev: &GameEvent) -> (Category, Option<Target>) {
    use Category::*;
    let side = |s: &SideEvent| match *s {
        SideEvent::TechCaptured { .. } => (Research, None),
        SideEvent::Abandoned { planet: p, .. } | SideEvent::Revolt { planet: p, .. } | SideEvent::Rebelled { planet: p, .. } | SideEvent::Colony { planet: p, .. } => {
            (Colony, planet(p))
        }
        _ => (Colony, None),
    };
    match ev {
        GameEvent::Battle(b) => match *b {
            BattleEvent::Space { planet: p, attacker, .. } => (Battle, planet(p).or(Some(Target::Fleet(attacker)))),
            BattleEvent::Ground { planet: p, .. } | BattleEvent::GroundSide { planet: p, event: GroundEvent::Taken { .. } } => (Battle, planet(p)),
            _ => (Battle, None),
        },
        GameEvent::Research(_) => (Research, None),
        GameEvent::Side(s) => side(s),
        GameEvent::Spy(SpyEvent::Colony(s)) => match s {
            SideEvent::Colony { planet: p, event: ColonyEvent::SpyExpelled { .. } } => (Colony, planet(*p)),
            s => side(s),
        },
        GameEvent::Spy(SpyEvent::Struck { mission, a, .. }) => {
            let at = matches!(*mission as usize, spy::mission::SABOTAGE | spy::mission::REBELLION).then(|| planet((*a).max(0) as usize)).flatten();
            (Diplomacy, at)
        }
        GameEvent::Spy(_) | GameEvent::WarDeclared { .. } => (Diplomacy, None),
        GameEvent::Starmap(s) | GameEvent::Diplomacy(DiploEvent::Starmap(s)) => match *s {
            StarmapEvent::EnemyApproaching { fleet, .. } => (Battle, Some(Target::Fleet(fleet))),
            _ => (Other, None),
        },
        GameEvent::Diplomacy(_) => (Diplomacy, None),
        GameEvent::Story(ig2_sim::story::StoryEvent::EmpireDies(_)) => (Diplomacy, None),
        GameEvent::Story(_) => (Story, None),
        _ => (Other, None),
    }
}

/// `e` under the filter and the search text (ASCII case ignored; an empty search matches all).
pub fn matches(e: &Entry, filter: Option<Category>, search: &[u8]) -> bool {
    filter.is_none_or(|c| c == e.category) && (search.is_empty() || e.text.windows(search.len()).any(|w| w.eq_ignore_ascii_case(search)))
}

/// The "EVENT LOG" label on the messages screen (virtual pixels: x, y, w, h).
pub const LABEL: [f32; 4] = [6.0, 196.0, 54.0, 8.0];
const LABEL_TEXT: &str = "EVENT LOG";
/// The panel `[x, y, w, h]`, the filter row's and the search's y, the first line's y, the line step and count.
const PANEL: [f32; 4] = [200.0, 20.0, 436.0, 452.0];
const TABS_Y: f32 = 26.0;
const SEARCH_Y: f32 = 40.0;
const LINES_Y: f32 = 56.0;
const LINE: f32 = 11.0;
const ROWS: usize = 37;
/// Character cells of size 2.
const CELL: f32 = 6.0;

/// The panel's state (the messages screen's).
#[derive(Debug, Clone, Default)]
pub struct Panel {
    pub open: bool,
    /// An index into [`FILTERS`].
    pub filter: usize,
    pub search: crate::chat::Entry,
    /// Lines scrolled past (from the newest).
    pub scroll: usize,
    /// The shown line under the pointer.
    pub hover: Option<usize>,
    /// The label under the pointer.
    pub label_hover: bool,
}

/// The filter labels in `lang` (ADR 0042) and their x.
fn tabs(lang: Lang) -> [(Vec<u8>, f32); 6] {
    let mut x = PANEL[0] + 8.0;
    FILTERS.map(|(l, _)| {
        let l = crate::tr::get(lang, l);
        let at = x;
        x += (l.len() as f32 + 2.0) * CELL;
        (l, at)
    })
}

impl Panel {
    /// The lines shown under the filter and the search, newest first.
    pub fn shown<'a>(&self, log: &'a Log) -> Vec<&'a Entry> {
        let filter = FILTERS[self.filter].1;
        log.entries.iter().rev().filter(|e| matches(e, filter, &self.search.text)).collect()
    }

    pub fn over_label(at: [f32; 2]) -> bool {
        inside_f(LABEL, at)
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.hover = None;
    }

    /// The pointer moved.
    pub fn hover_at(&mut self, at: [f32; 2], log: &Log) {
        self.label_hover = Self::over_label(at);
        self.hover = None;
        if !self.open || !inside_f([PANEL[0], LINES_Y, PANEL[2], ROWS as f32 * LINE], at) {
            return;
        }
        let k = ((at[1] - LINES_Y) / LINE) as usize + self.scroll;
        self.hover = (k < self.shown(log).len()).then_some(k);
    }

    /// Scroll by `lines` (positive = older).
    pub fn scroll_by(&mut self, lines: i32, log: &Log) {
        let max = self.shown(log).len().saturating_sub(ROWS);
        self.scroll = (self.scroll as i32 + lines).clamp(0, max as i32) as usize;
    }

    /// A left press while the panel is up: a filter, or a line's target.
    pub fn press(&mut self, at: [f32; 2], log: &Log, lang: Lang) -> Option<Target> {
        for (i, (l, x)) in tabs(lang).into_iter().enumerate() {
            if inside_f([x, TABS_Y - 2.0, l.len() as f32 * CELL, 11.0], at) {
                (self.filter, self.scroll) = (i, 0);
                return None;
            }
        }
        self.hover_at(at, log);
        self.hover.and_then(|k| self.shown(log).get(k).and_then(|e| e.target))
    }

    /// A key while the panel is up: the search's (Esc closes the panel, Enter does nothing).
    pub fn key(&mut self, k: EntryKey) {
        match k {
            EntryKey::Escape => self.toggle(),
            EntryKey::Enter => {}
            k => {
                self.search.key(k);
                self.scroll = 0;
            }
        }
    }
}

/// The label, and the panel when open (`hu`: Hungarian articles as the line's history shows them).
#[allow(clippy::too_many_arguments)]
pub fn draw(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], p: &Panel, log: &Log, hu: bool, time: f32) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    let lit = |on: bool| col(if on { 6 } else { 2 });
    let lang = if hu { Lang::Hu } else { Lang::En };
    font.text(f, v, &crate::tr::get(lang, LABEL_TEXT), LABEL[0], LABEL[1], 0, 2, Align::Left, lit(p.label_hover || p.open), None);
    if !p.open {
        return;
    }
    crate::kit::fader::fader(f, white, v, PANEL, [0.0, 0.0, 0.0], 20.0, [0.0, 60.0, 0.0], 30.0);
    for (i, (l, x)) in tabs(lang).into_iter().enumerate() {
        font.text(f, v, &l, x, TABS_Y, 0, 2, Align::Left, lit(i == p.filter), None);
    }
    let x0 = PANEL[0] + 8.0;
    let search = crate::tr::get(lang, "Search:");
    font.text(f, v, &search, x0, SEARCH_Y, 0, 2, Align::Left, col(2), None);
    let sx = x0 + (search.len() + 1) as f32 * CELL;
    font.text(f, v, &p.search.text, sx, SEARCH_Y, 0, 2, Align::Left, col(crate::chat::CHAT_COLOUR), None);
    if (time * 1000.0 / 200.0) as i64 % 2 == 0 {
        let c = col(1);
        let at = v.px(sx + (p.search.cursor as f32 + 0.5) * CELL, SEARCH_Y + font.height(2) + 1.0);
        f.push(white, crate::render::Blend::Alpha, crate::render::Instance::new(crate::render::kind::SPRITE, at, [CELL * v.k / 2.0, v.k], [c[0], c[1], c[2], 1.0]));
    }
    let shown = p.shown(log);
    if shown.is_empty() {
        font.text(f, v, &crate::tr::get(lang, "No messages."), x0, LINES_Y, 0, 2, Align::Left, col(2), None);
    }
    for (row, (k, e)) in shown.iter().enumerate().skip(p.scroll).take(ROWS).enumerate() {
        let y = LINES_Y + row as f32 * LINE;
        let t = GameTime(e.ido);
        let date = format!("{:04}.{:02}.{:02}", t.year(), t.month(), t.day());
        font.text(f, v, date.as_bytes(), x0, y, 0, 2, Align::Left, col(2), None);
        let c = if p.hover == Some(k) && e.target.is_some() { 6 } else { 0x21 };
        font.text(f, v, &crate::diplomacy::articles(&e.text, hu), x0 + 11.0 * CELL, y, 0x36, 2, Align::Left, col(c), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ig2_sim::research::ResearchEvent;
    use ig2_sim::story::StoryEvent;

    #[test]
    fn categories_and_targets() {
        let space = GameEvent::Battle(BattleEvent::Space { attacker: 4, defender: 7, planet: 12, winner: 1, lopsided_sp: false, human: true });
        assert_eq!(classify(&space), (Category::Battle, Some(Target::Planet(12))));
        let deep = GameEvent::Battle(BattleEvent::Space { attacker: 4, defender: 7, planet: 0, winner: 1, lopsided_sp: false, human: true });
        assert_eq!(classify(&deep), (Category::Battle, Some(Target::Fleet(4))));
        let ground = GameEvent::Battle(BattleEvent::Ground { fleet: 3, planet: 9, attacker: 1, owner: 2, lopsided: false, won: true, story_planet: -1 });
        assert_eq!(classify(&ground), (Category::Battle, Some(Target::Planet(9))));
        let near = GameEvent::Starmap(StarmapEvent::EnemyApproaching { fleet: 5, to: 1 });
        assert_eq!(classify(&near), (Category::Battle, Some(Target::Fleet(5))));
        assert_eq!(classify(&GameEvent::Research(ResearchEvent::Completed { empire: 1, tech: 3 })), (Category::Research, None));
        assert_eq!(classify(&GameEvent::Side(SideEvent::TechCaptured { empire: 1, tech: 3 })), (Category::Research, None));
        assert_eq!(classify(&GameEvent::Diplomacy(DiploEvent::War { a: 1, b: 2 })), (Category::Diplomacy, None));
        assert_eq!(classify(&GameEvent::Diplomacy(DiploEvent::EmpireDied { empire: 3 })), (Category::Diplomacy, None));
        assert_eq!(classify(&GameEvent::Story(StoryEvent::EmpireDies(3))), (Category::Diplomacy, None));
        assert_eq!(classify(&GameEvent::Side(SideEvent::Revolt { planet: 8, owner: 1 })), (Category::Colony, Some(Target::Planet(8))));
        assert_eq!(classify(&GameEvent::Side(SideEvent::Colony { planet: 8, event: ColonyEvent::UnrestWarning })), (Category::Colony, Some(Target::Planet(8))));
        assert_eq!(classify(&GameEvent::Side(SideEvent::YearClosed)), (Category::Colony, None));
        let expelled = GameEvent::Spy(SpyEvent::Colony(SideEvent::Colony { planet: 6, event: ColonyEvent::SpyExpelled { empire: 1 } }));
        assert_eq!(classify(&expelled), (Category::Colony, Some(Target::Planet(6))));
        let sabotage = GameEvent::Spy(SpyEvent::Struck { mission: spy::mission::SABOTAGE as u16, a: 11, b: 2 });
        assert_eq!(classify(&sabotage), (Category::Diplomacy, Some(Target::Planet(11))));
        let money = GameEvent::Spy(SpyEvent::Struck { mission: spy::mission::STEAL_MONEY as u16, a: 11, b: 2 });
        assert_eq!(classify(&money), (Category::Diplomacy, None));
        assert_eq!(classify(&GameEvent::Starmap(StarmapEvent::ContactLost { fleet: 2 })), (Category::Other, None));
    }

    /// The lines are `messages_for`'s own, per event, for the given empire only.
    #[test]
    fn records_the_sims_lines() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let (rules, _) = crate::rules::load(&install).unwrap();
        let g = crate::new_game::tutorial(&install, 1, 4242, &rules).unwrap();
        let done = |empire| GameEvent::Research(ResearchEvent::Completed { empire, tech: 1 });
        let revolt = || GameEvent::Side(SideEvent::Revolt { planet: 3, owner: 1 });
        let mut log = Log::default();
        log.record(&g, &[done(1), done(2), revolt()], 1, 18000);
        let want: Vec<Vec<u8>> = g.messages_for(&[done(1), revolt()], 1).into_iter().map(|p| p.1).collect();
        assert_eq!(log.entries.iter().map(|e| e.text.clone()).collect::<Vec<_>>(), want);
        assert_eq!(log.entries.iter().map(|e| (e.category, e.target, e.ido)).collect::<Vec<_>>(), [(Category::Research, None, 18000), (Category::Colony, Some(Target::Planet(3)), 18000)]);
    }

    #[test]
    fn filter_and_search() {
        let e = |text: &[u8], category| Entry { ido: 0, text: text.to_vec(), category, target: None };
        let log = Log { entries: vec![e(b"Laser researched", Category::Research), e(b"Battle at Earth", Category::Battle), e(b"War: Garthog", Category::Diplomacy)] };
        let mut p = Panel::default();
        assert_eq!(p.shown(&log).len(), 3);
        assert_eq!(p.shown(&log)[0].text, b"War: Garthog", "newest first");
        p.filter = 2;
        assert_eq!(p.shown(&log).iter().map(|e| e.category).collect::<Vec<_>>(), [Category::Research]);
        p.filter = 0;
        for c in b"EARTH" {
            p.key(EntryKey::Char(*c));
        }
        assert_eq!(p.shown(&log).iter().map(|e| &e.text[..]).collect::<Vec<_>>(), [&b"Battle at Earth"[..]]);
        assert!(!matches(&log.entries[1], Some(Category::Story), b""));
        p.open = true;
        p.key(EntryKey::Escape);
        assert!(!p.open, "Esc closes the panel");
    }
}
