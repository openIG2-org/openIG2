//! Story mods (ADR 0061): a mod's `stories.toml`, stories as triggers over the story engine's reads and actions as
//! its events, and the interpreter [`ModScripts`] that `Story_Check` runs after the built-in scripts.
//!
//! A story fires once, in the first check where all its triggers hold. The state (fired stories, the mods' flags and
//! counters) is ours, saved in our snapshot ([`crate::save::SaveState::mod_stories`]); without stories nothing runs.

use std::collections::{BTreeMap, BTreeSet};

use toml_edit::{DocumentMut, Item, TableLike, Value};

use crate::colony_side::EMPIRES;
use crate::story::{Script, Story, StoryEvent, StoryWorld, DAY};

/// Fleet story ids a mod may use: above the campaign's (1..=868, 2301, 2311) and within the fleet's i16 field.
pub const FLEET_IDS: std::ops::RangeInclusive<i32> = 10000..=32767;

/// An empire as a story names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// `EN`.
    Human,
    Empire(i32),
}

impl Who {
    fn of(self, w: &dyn StoryWorld) -> i32 {
        match self {
            Who::Human => w.human(),
            Who::Empire(e) => e,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Trigger {
    Day(i32),
    PlanetOwner { planet: i32, empire: Who },
    EmpireDead(Who),
    Flag(String),
    NotFlag(String),
    Counter { name: String, at_least: Option<i32>, at_most: Option<i32> },
    FleetAt { fleet: i32, planet: i32, within: f32 },
    Answered { message: i32, answer: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Message(i32),
    Ticker(i32),
    Video(i32),
    GivePlanet { planet: i32, empire: Who },
    SpawnFleet { id: i32, owner: Who, planet: i32, design: i32, ships: i32, name: Option<i32> },
    Money { empire: Who, amount: f64 },
    SetFlag(String),
    ClearFlag(String),
    SetCounter { name: String, value: i32 },
    AddCounter { name: String, value: i32 },
    Win,
    Lose,
}

/// One `[[story]]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModStory {
    /// `<mod>/<name>`: the fired mark's key.
    pub key: String,
    pub when: Vec<Trigger>,
    pub actions: Vec<Action>,
}

/// The interpreter's state: every enabled mod's, in one place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModStoryState {
    pub fired: BTreeSet<String>,
    pub flags: BTreeSet<String>,
    pub counters: BTreeMap<String, i32>,
}

// ------------------------------------------------------------------------------------------------ parsing

fn name_ok(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A table's fields, every one of them used: unknown keys and missing ones are errors.
struct Fields<'a> {
    at: String,
    t: &'a dyn TableLike,
    used: Vec<&'static str>,
}

impl<'a> Fields<'a> {
    fn new(v: &'a Value, at: String) -> Result<Self, String> {
        let t = v.as_inline_table().map(|t| t as &dyn TableLike).ok_or_else(|| format!("{at}: expected a table {{ … }}"))?;
        Ok(Self { at, t, used: Vec::new() })
    }

    fn get(&mut self, k: &'static str) -> Option<&'a Value> {
        self.used.push(k);
        self.t.get(k).and_then(Item::as_value)
    }

    fn int(&mut self, k: &'static str, range: std::ops::RangeInclusive<i64>) -> Result<i32, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).ok_or_else(|| format!("{at}: missing")).and_then(|v| int(v, &at, range))
    }

    fn opt_int(&mut self, k: &'static str, range: std::ops::RangeInclusive<i64>) -> Result<Option<i32>, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).map(|v| int(v, &at, range)).transpose()
    }

    fn who(&mut self, k: &'static str) -> Result<Who, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).ok_or_else(|| format!("{at}: missing")).and_then(|v| who(v, &at))
    }

    fn name(&mut self, k: &'static str) -> Result<String, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).ok_or_else(|| format!("{at}: missing")).and_then(|v| name(v, &at))
    }

    /// Fails on a key no getter asked for.
    fn done(self) -> Result<(), String> {
        match self.t.iter().find(|(k, _)| !self.used.contains(k)) {
            Some((k, _)) => Err(format!("{}: unknown field `{k}` (known: {})", self.at, self.used.join(", "))),
            None => Ok(()),
        }
    }
}

fn int(v: &Value, at: &str, range: std::ops::RangeInclusive<i64>) -> Result<i32, String> {
    let i = v.as_integer().ok_or_else(|| format!("{at}: expected an integer"))?;
    if !range.contains(&i) {
        return Err(format!("{at}: {i} is out of range {}..={}", range.start(), range.end()));
    }
    Ok(i as i32)
}

fn who(v: &Value, at: &str) -> Result<Who, String> {
    match v.as_str() {
        Some("human") => Ok(Who::Human),
        Some(s) => Err(format!("{at}: \"{s}\" is not an empire (0..={} or \"human\")", EMPIRES - 1)),
        None => int(v, at, 0..=EMPIRES as i64 - 1).map(Who::Empire),
    }
}

fn name(v: &Value, at: &str) -> Result<String, String> {
    match v.as_str() {
        Some(s) if name_ok(s) => Ok(s.to_owned()),
        Some(s) => Err(format!("{at}: \"{s}\" is not a name (letters, digits, '_' and '-')")),
        None => Err(format!("{at}: expected a name")),
    }
}

fn yes(v: &Value, at: &str) -> Result<(), String> {
    match v.as_bool() {
        Some(true) => Ok(()),
        _ => Err(format!("{at}: expected `true`")),
    }
}

const PLANET: std::ops::RangeInclusive<i64> = 1..=0xffff;
const KEY: std::ops::RangeInclusive<i64> = 1..=0xffff;
const ANY: std::ops::RangeInclusive<i64> = i32::MIN as i64..=i32::MAX as i64;

const TRIGGERS: &str = "day, planet_owner, empire_dead, flag, not_flag, counter, fleet_at, answered";
const ACTIONS: &str = "message, ticker, video, give_planet, spawn_fleet, money, set_flag, clear_flag, set_counter, add_counter, win, lose";

/// The one key of a `when` or `do` entry.
fn entry<'a>(v: &'a Value, at: &str) -> Result<(&'a str, &'a Value), String> {
    let t = v.as_inline_table().ok_or_else(|| format!("{at}: expected a table with one key, like {{ day = 30 }}"))?;
    let mut it = t.iter();
    match (it.next(), it.next()) {
        (Some((k, v)), None) => Ok((k, v)),
        _ => Err(format!("{at}: expected exactly one key")),
    }
}

fn trigger(v: &Value, at: &str) -> Result<Trigger, String> {
    let (k, v) = entry(v, at)?;
    let at = format!("{at} {k}");
    let t = match k {
        "day" => Trigger::Day(int(v, &at, 0..=(i32::MAX / DAY) as i64)?),
        "planet_owner" => {
            let mut f = Fields::new(v, at)?;
            let t = Trigger::PlanetOwner { planet: f.int("planet", PLANET)?, empire: f.who("empire")? };
            f.done()?;
            t
        }
        "empire_dead" => Trigger::EmpireDead(who(v, &at)?),
        "flag" => Trigger::Flag(name(v, &at)?),
        "not_flag" => Trigger::NotFlag(name(v, &at)?),
        "counter" => {
            let mut f = Fields::new(v, at.clone())?;
            let t = Trigger::Counter { name: f.name("name")?, at_least: f.opt_int("at_least", ANY)?, at_most: f.opt_int("at_most", ANY)? };
            f.done()?;
            if matches!(t, Trigger::Counter { at_least: None, at_most: None, .. }) {
                return Err(format!("{at}: give at_least, at_most or both"));
            }
            t
        }
        "fleet_at" => {
            let mut f = Fields::new(v, at.clone())?;
            let fleet = f.int("fleet", *FLEET_IDS.start() as i64..=*FLEET_IDS.end() as i64)?;
            let planet = f.int("planet", PLANET)?;
            let within = f.opt_int("within", 1..=10000)?.unwrap_or(20) as f32;
            f.done()?;
            Trigger::FleetAt { fleet, planet, within }
        }
        "answered" => {
            let mut f = Fields::new(v, at)?;
            let t = Trigger::Answered { message: f.int("message", 1..=9999)?, answer: f.int("answer", 1..=99)? };
            f.done()?;
            t
        }
        _ => return Err(format!("{at}: unknown trigger (known: {TRIGGERS})")),
    };
    Ok(t)
}

fn action(v: &Value, at: &str) -> Result<Action, String> {
    let (k, v) = entry(v, at)?;
    let at = format!("{at} {k}");
    let counter = |v: &Value, at: String| -> Result<(String, i32), String> {
        let mut f = Fields::new(v, at)?;
        let r = (f.name("name")?, f.int("value", ANY)?);
        f.done()?;
        Ok(r)
    };
    let a = match k {
        "message" => Action::Message(int(v, &at, 1..=9999)?),
        "ticker" => Action::Ticker(int(v, &at, KEY)?),
        "video" => Action::Video(int(v, &at, KEY)?),
        "give_planet" => {
            let mut f = Fields::new(v, at)?;
            let a = Action::GivePlanet { planet: f.int("planet", PLANET)?, empire: f.who("empire")? };
            f.done()?;
            a
        }
        "spawn_fleet" => {
            let mut f = Fields::new(v, at)?;
            let a = Action::SpawnFleet {
                id: f.int("id", *FLEET_IDS.start() as i64..=*FLEET_IDS.end() as i64)?,
                owner: f.who("owner")?,
                planet: f.int("planet", PLANET)?,
                design: f.int("design", 0..=crate::ai::DESIGN_SLOTS as i64 - 1)?,
                ships: f.opt_int("ships", 1..=100)?.unwrap_or(1),
                name: f.opt_int("name", KEY)?,
            };
            f.done()?;
            a
        }
        "money" => {
            let mut f = Fields::new(v, at)?;
            let a = Action::Money { empire: f.who("empire")?, amount: f.int("amount", ANY)? as f64 };
            f.done()?;
            a
        }
        "set_flag" => Action::SetFlag(name(v, &at)?),
        "clear_flag" => Action::ClearFlag(name(v, &at)?),
        "set_counter" => counter(v, at).map(|(name, value)| Action::SetCounter { name, value })?,
        "add_counter" => counter(v, at).map(|(name, value)| Action::AddCounter { name, value })?,
        "win" => yes(v, &at).map(|()| Action::Win)?,
        "lose" => yes(v, &at).map(|()| Action::Lose)?,
        _ => return Err(format!("{at}: unknown action (known: {ACTIONS})")),
    };
    Ok(a)
}

/// A mod's `stories.toml` (ADR 0061 §1, §7); errors name the story and the entry.
pub fn parse(text: &str, mod_name: &str) -> Result<Vec<ModStory>, String> {
    let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string().trim_end().to_owned())?;
    if let Some((k, _)) = doc.iter().find(|(k, _)| *k != "story") {
        return Err(format!("unknown table `{k}` (a file holds [[story]] tables)"));
    }
    let Some(item) = doc.get("story") else { return Ok(Vec::new()) };
    let tables = item.as_array_of_tables().ok_or("`story` must be [[story]] tables")?;
    let mut out: Vec<ModStory> = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        let name = match t.get("name").and_then(Item::as_value) {
            Some(v) => name(v, &format!("story {}", i + 1))?,
            None => return Err(format!("story {}: missing name", i + 1)),
        };
        let at = format!("story {name}");
        let key = format!("{mod_name}/{name}");
        if out.iter().any(|s| s.key == key) {
            return Err(format!("{at}: a second story of that name"));
        }
        if let Some((k, _)) = t.iter().find(|(k, _)| !["name", "when", "do"].contains(k)) {
            return Err(format!("{at}: unknown field `{k}` (known: name, when, do)"));
        }
        let list = |k: &str| -> Result<Vec<&Value>, String> {
            match t.get(k) {
                None => Ok(Vec::new()),
                Some(item) => item.as_array().map(|a| a.iter().collect()).ok_or_else(|| format!("{at}: `{k}` must be an array [ … ]")),
            }
        };
        let when = list("when")?.into_iter().enumerate().map(|(j, v)| trigger(v, &format!("{at}: when {}:", j + 1))).collect::<Result<_, _>>()?;
        let actions: Vec<Action> = list("do")?.into_iter().enumerate().map(|(j, v)| action(v, &format!("{at}: do {}:", j + 1))).collect::<Result<_, _>>()?;
        if actions.is_empty() {
            return Err(format!("{at}: no `do` actions"));
        }
        out.push(ModStory { key, when, actions });
    }
    Ok(out)
}

/// Every enabled mod's stories in the order they run (lowest priority first, ADR 0061 §5).
pub fn load(install: &ig2_formats::Install) -> Result<Vec<ModStory>, String> {
    let mut out = Vec::new();
    for (m, bytes) in install.story_files().map_err(|e| e.to_string())? {
        let from = format!("mod {m}: {}", ig2_formats::vfs::STORY_FILE);
        let text = String::from_utf8(bytes).map_err(|_| format!("{from}: not UTF-8 text"))?;
        out.extend(parse(&text, m).map_err(|e| format!("{from}: {e}"))?);
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------------ the interpreter

fn holds(t: &Trigger, s: &Story, w: &dyn StoryWorld) -> bool {
    let st = &s.mod_state;
    let planet = |p: i32| (1..=w.planet_count()).contains(&p);
    match t {
        Trigger::Day(d) => w.ido() >= d * DAY,
        Trigger::PlanetOwner { planet: p, empire } => planet(*p) && w.planet_owner(*p) == empire.of(w),
        Trigger::EmpireDead(e) => !w.empire_alive(e.of(w)),
        Trigger::Flag(f) => st.flags.contains(f),
        Trigger::NotFlag(f) => !st.flags.contains(f),
        Trigger::Counter { name, at_least, at_most } => {
            let v = st.counters.get(name).copied().unwrap_or(0);
            at_least.is_none_or(|m| v >= m) && at_most.is_none_or(|m| v <= m)
        }
        Trigger::FleetAt { fleet, planet: p, within } => {
            planet(*p) && {
                let (px, py) = w.planet_xy(*p);
                (1..=w.fleet_count()).any(|f| {
                    let (fx, fy) = w.fleet_xy(f);
                    w.fleet_owner(f) != 0xff && w.fleet_id(f) == *fleet && (fx - px) * (fx - px) + (fy - py) * (fy - py) <= within * within
                })
            }
        }
        Trigger::Answered { message, answer } => s.flags[crate::story::flag::MESSAGE] && s.message == (*message, *answer),
    }
}

fn act(a: &Action, s: &mut Story, w: &mut dyn StoryWorld) {
    let planet = |p: i32, w: &dyn StoryWorld| (1..=w.planet_count()).contains(&p);
    match a {
        Action::Message(id) => {
            w.event(StoryEvent::NewMessage { id: *id, arg: 0 });
            for h in w.other_humans() {
                w.event(StoryEvent::HumanMessage { empire: h, id: *id, arg: 0 });
            }
        }
        Action::Ticker(text) => w.event(StoryEvent::ScrollText { text: *text, important: false }),
        Action::Video(id) => w.event(StoryEvent::Anim(*id)),
        Action::GivePlanet { planet: p, empire } => {
            if planet(*p, w) {
                let owner = empire.of(w);
                w.event(StoryEvent::SetPlanetOwner { planet: *p, owner, race: w.race(owner) });
            }
        }
        Action::SpawnFleet { id, owner, planet: p, design, ships, name } => {
            if planet(*p, w) {
                let label = name.map_or(crate::story::FleetName::Bytes(Vec::new()), crate::story::FleetName::Text);
                let owner = owner.of(w);
                s.f_new_ex(label, owner, *id, 0.0, 2, *p, w);
                s.f_add_ship(*id, *design, *ships, w);
            }
        }
        Action::Money { empire, amount } => w.event(StoryEvent::AddMoney { empire: empire.of(w), amount: *amount }),
        Action::SetFlag(f) => {
            s.mod_state.flags.insert(f.clone());
        }
        Action::ClearFlag(f) => {
            s.mod_state.flags.remove(f);
        }
        Action::SetCounter { name, value } => {
            s.mod_state.counters.insert(name.clone(), *value);
        }
        Action::AddCounter { name, value } => {
            let c = s.mod_state.counters.entry(name.clone()).or_insert(0);
            *c = c.wrapping_add(*value);
        }
        Action::Win => w.event(StoryEvent::Won),
        Action::Lose => w.event(StoryEvent::EmpireDies(w.human())),
    }
}

/// The interpreter: every unfired story whose triggers all hold fires (its mark first, then its actions in order).
pub struct ModScripts;

impl Script for ModScripts {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        let defs = s.mod_defs.clone();
        for story in defs.iter() {
            if !s.mod_state.fired.contains(&story.key) && story.when.iter().all(|t| holds(t, s, w)) {
                s.mod_state.fired.insert(story.key.clone());
                for a in &story.actions {
                    act(a, s, w);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
