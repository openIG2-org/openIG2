//! Story mods (ADR 0061): a mod's `stories.toml`, stories as triggers over the story engine's reads and actions as
//! its events, and the interpreter [`ModScripts`] that `Story_Check` runs after the built-in scripts.
//!
//! A story fires once, in the first check where all its triggers hold, or again and again (`repeat`). The state (fired
//! stories, the mods' flags, counters and timers) is ours, saved in our snapshot ([`crate::save::SaveState::mod_stories`]); without stories nothing runs.

use std::collections::{BTreeMap, BTreeSet};

use toml_edit::{DocumentMut, Item, TableLike, Value};

use crate::colony_side::EMPIRES;
use crate::story::{flag, Script, Story, StoryEvent, StoryWorld, DAY};

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
    All(Vec<Trigger>),
    Any(Vec<Trigger>),
    Not(Box<Trigger>),
    /// `ground`: `None` either kind.
    Battle { ground: Option<bool>, won: bool, empire: Who, planet: Option<i32> },
    Tech { tech: i32, empire: Who },
    Money { empire: Who, at_least: Option<i32>, at_most: Option<i32> },
    Relation { a: Who, b: Who, at_least: Option<i32>, at_most: Option<i32> },
    SpyReport { mission: Option<i32>, target: Option<Who>, success: Option<bool>, died: Option<bool> },
    Building { kind: i32, planet: Option<i32>, empire: Who },
    Timer(String),
}

/// A diplomatic state a story sets (`st_Set…Viszony`, story 9's trade agreement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diplomacy {
    War,
    Neutral,
    NonAggression,
    Alliance,
    Trade,
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
    GiveTech { tech: i32, empire: Who },
    RemoveTech { tech: i32, empire: Who },
    ChangeRelation { a: Who, b: Who, by: i32 },
    Diplomacy { a: Who, b: Who, state: Diplomacy },
    AddBuilding { planet: i32, kind: i32, level: i32 },
    /// Into planet `planet`'s garrison (`place` = −planet) or the mod fleet with story id `place`.
    GroundUnits { place: i32, design: i32, count: i32 },
    StartTimer { name: String, days: i32 },
    StopTimer(String),
    /// Another story of the same file, by key.
    Chain(String),
}

/// A repeating story's limits (ADR 0061 §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    /// The least gap between two firings, in days.
    pub every: i32,
    pub times: Option<i32>,
}

/// One `[[story]]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModStory {
    /// `<mod>/<name>`: the fired mark's key.
    pub key: String,
    pub when: Vec<Trigger>,
    pub actions: Vec<Action>,
    /// `None`: it fires once.
    pub repeat: Option<Repeat>,
}

/// The interpreter's state: every enabled mod's, in one place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModStoryState {
    /// The once-stories that fired.
    pub fired: BTreeSet<String>,
    pub flags: BTreeSet<String>,
    pub counters: BTreeMap<String, i32>,
    /// The repeating stories that fired: `(Ido of the last firing, times fired)` (since save format 15).
    pub repeats: BTreeMap<String, (i32, i32)>,
    /// The running timers: the `Ido` each runs out at (since save format 15).
    pub timers: BTreeMap<String, i32>,
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

    /// An empire, `"human"` when not given.
    fn who_or_human(&mut self, k: &'static str) -> Result<Who, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).map_or(Ok(Who::Human), |v| who(v, &at))
    }

    fn opt_who(&mut self, k: &'static str) -> Result<Option<Who>, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).map(|v| who(v, &at)).transpose()
    }

    fn opt_bool(&mut self, k: &'static str) -> Result<Option<bool>, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).map(|v| v.as_bool().ok_or_else(|| format!("{at}: expected true or false"))).transpose()
    }

    /// One of `words` (its index), `None` when not given.
    fn opt_word(&mut self, k: &'static str, words: &[&str]) -> Result<Option<usize>, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).map(|v| word(v, &at, words)).transpose()
    }

    fn word(&mut self, k: &'static str, words: &[&str]) -> Result<usize, String> {
        let at = format!("{}.{k}", self.at);
        self.opt_word(k, words)?.ok_or_else(|| format!("{at}: missing"))
    }

    fn name(&mut self, k: &'static str) -> Result<String, String> {
        let at = format!("{}.{k}", self.at);
        self.get(k).ok_or_else(|| format!("{at}: missing")).and_then(|v| name(v, &at))
    }

    /// `at_least` and `at_most`, at least one of them.
    fn bounds(&mut self, range: std::ops::RangeInclusive<i64>) -> Result<(Option<i32>, Option<i32>), String> {
        let b = (self.opt_int("at_least", range.clone())?, self.opt_int("at_most", range)?);
        if b == (None, None) {
            return Err(format!("{}: give at_least, at_most or both", self.at));
        }
        Ok(b)
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

fn word(v: &Value, at: &str, words: &[&str]) -> Result<usize, String> {
    v.as_str().and_then(|s| words.iter().position(|w| *w == s)).ok_or_else(|| format!("{at}: expected one of \"{}\"", words.join("\", \"")))
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
const TECH: std::ops::RangeInclusive<i64> = 0..=crate::save::TECHS as i64 - 1;
/// A building kind (`Epuletek` `+0`, a byte; 0 is an empty slot).
const BUILDING: std::ops::RangeInclusive<i64> = 1..=255;
const FLEET: std::ops::RangeInclusive<i64> = *FLEET_IDS.start() as i64..=*FLEET_IDS.end() as i64;

const TRIGGERS: &str =
    "day, planet_owner, empire_dead, flag, not_flag, counter, fleet_at, answered, all, any, not, battle, tech, money, relation, spy_report, building, timer";
const ACTIONS: &str = "message, ticker, video, give_planet, spawn_fleet, money, set_flag, clear_flag, set_counter, add_counter, win, lose, \
    give_tech, remove_tech, change_relation, diplomacy, add_building, ground_units, start_timer, stop_timer, chain";

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
            let mut f = Fields::new(v, at)?;
            let name = f.name("name")?;
            let (at_least, at_most) = f.bounds(ANY)?;
            f.done()?;
            Trigger::Counter { name, at_least, at_most }
        }
        "all" | "any" => {
            let list = v.as_array().ok_or_else(|| format!("{at}: expected an array [ … ] of triggers"))?;
            let ts = list.iter().enumerate().map(|(j, v)| trigger(v, &format!("{at} {}:", j + 1))).collect::<Result<_, _>>()?;
            if k == "all" { Trigger::All(ts) } else { Trigger::Any(ts) }
        }
        "not" => Trigger::Not(Box::new(trigger(v, &format!("{at}:"))?)),
        "battle" => {
            let mut f = Fields::new(v, at)?;
            let ground = f.opt_word("kind", &["space", "ground"])?.map(|k| k == 1);
            let won = f.word("result", &["won", "lost"])? == 0;
            let empire = f.who_or_human("empire")?;
            let planet = f.opt_int("planet", PLANET)?;
            f.done()?;
            Trigger::Battle { ground, won, empire, planet }
        }
        "tech" => {
            let mut f = Fields::new(v, at)?;
            let t = Trigger::Tech { tech: f.int("tech", TECH)?, empire: f.who_or_human("empire")? };
            f.done()?;
            t
        }
        "money" => {
            let mut f = Fields::new(v, at)?;
            let empire = f.who_or_human("empire")?;
            let (at_least, at_most) = f.bounds(ANY)?;
            f.done()?;
            Trigger::Money { empire, at_least, at_most }
        }
        "relation" => {
            let mut f = Fields::new(v, at)?;
            let (a, b) = (f.who("a")?, f.who("b")?);
            let (at_least, at_most) = f.bounds(0..=9)?;
            f.done()?;
            Trigger::Relation { a, b, at_least, at_most }
        }
        "spy_report" => {
            let mut f = Fields::new(v, at)?;
            let t = Trigger::SpyReport { mission: f.opt_int("mission", 0..=255)?, target: f.opt_who("target")?, success: f.opt_bool("success")?, died: f.opt_bool("died")? };
            f.done()?;
            t
        }
        "building" => {
            let mut f = Fields::new(v, at)?;
            let t = Trigger::Building { kind: f.int("kind", BUILDING)?, planet: f.opt_int("planet", PLANET)?, empire: f.who_or_human("empire")? };
            f.done()?;
            t
        }
        "timer" => Trigger::Timer(name(v, &at)?),
        "fleet_at" => {
            let mut f = Fields::new(v, at.clone())?;
            let fleet = f.int("fleet", FLEET)?;
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
                id: f.int("id", FLEET)?,
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
        "give_tech" | "remove_tech" => {
            let mut f = Fields::new(v, at)?;
            let (tech, empire) = (f.int("tech", TECH)?, f.who_or_human("empire")?);
            f.done()?;
            if k == "give_tech" { Action::GiveTech { tech, empire } } else { Action::RemoveTech { tech, empire } }
        }
        "change_relation" => {
            let mut f = Fields::new(v, at)?;
            let a = Action::ChangeRelation { a: f.who("a")?, b: f.who("b")?, by: f.int("by", -10000..=10000)? };
            f.done()?;
            a
        }
        "diplomacy" => {
            let mut f = Fields::new(v, at)?;
            let (a, b) = (f.who("a")?, f.who("b")?);
            let state = [Diplomacy::War, Diplomacy::Neutral, Diplomacy::NonAggression, Diplomacy::Alliance, Diplomacy::Trade]
                [f.word("state", &["war", "neutral", "non_aggression", "alliance", "trade"])?];
            f.done()?;
            Action::Diplomacy { a, b, state }
        }
        "add_building" => {
            let mut f = Fields::new(v, at)?;
            let a = Action::AddBuilding { planet: f.int("planet", PLANET)?, kind: f.int("kind", BUILDING)?, level: f.opt_int("level", 0..=255)?.unwrap_or(0) };
            f.done()?;
            a
        }
        "ground_units" => {
            let mut f = Fields::new(v, at.clone())?;
            let (planet, fleet) = (f.opt_int("planet", PLANET)?, f.opt_int("fleet", FLEET)?);
            let (design, count) = (f.int("design", 0..=crate::ai::DESIGN_SLOTS as i64 - 1)?, f.opt_int("count", 1..=1000)?.unwrap_or(1));
            f.done()?;
            let place = match (planet, fleet) {
                (Some(p), None) => -p,
                (None, Some(id)) => id,
                _ => return Err(format!("{at}: give planet or fleet (one of them)")),
            };
            Action::GroundUnits { place, design, count }
        }
        "start_timer" => {
            let mut f = Fields::new(v, at)?;
            let a = Action::StartTimer { name: f.name("name")?, days: f.int("days", 0..=(i32::MAX / DAY) as i64)? };
            f.done()?;
            a
        }
        "stop_timer" => Action::StopTimer(name(v, &at)?),
        "chain" => Action::Chain(name(v, &at)?),
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
        if let Some((k, _)) = t.iter().find(|(k, _)| !["name", "when", "do", "repeat", "every", "times"].contains(k)) {
            return Err(format!("{at}: unknown field `{k}` (known: name, when, do, repeat, every, times)"));
        }
        let field = |k: &str| t.get(k).and_then(Item::as_value);
        let repeat = match field("repeat").map(|v| v.as_bool().ok_or_else(|| format!("{at}: repeat: expected true or false"))).transpose()? {
            Some(true) => Some(Repeat {
                every: field("every").map(|v| int(v, &format!("{at}: every"), 1..=(i32::MAX / DAY) as i64)).transpose()?.unwrap_or(1),
                times: field("times").map(|v| int(v, &format!("{at}: times"), 1..=i32::MAX as i64)).transpose()?,
            }),
            _ if field("every").is_some() || field("times").is_some() => return Err(format!("{at}: every and times need repeat = true")),
            _ => None,
        };
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
        out.push(ModStory { key, when, actions, repeat });
    }
    // Chains name a story of the same file; resolved to its key, and no chain leads back to where it started.
    let keys: Vec<String> = out.iter().map(|s| s.key.clone()).collect();
    for s in out.iter_mut() {
        for a in s.actions.iter_mut() {
            if let Action::Chain(n) = a {
                let key = format!("{mod_name}/{n}");
                if !keys.contains(&key) {
                    return Err(format!("story {}: chain: no story \"{n}\" in this file", &s.key[mod_name.len() + 1..]));
                }
                *n = key;
            }
        }
    }
    for s in &out {
        let (mut todo, mut seen) = (vec![s.key.as_str()], BTreeSet::new());
        while let Some(k) = todo.pop() {
            for a in &out.iter().find(|s| s.key == k).expect("resolved above").actions {
                if let Action::Chain(next) = a {
                    if *next == s.key {
                        return Err(format!("story {}: its chains lead back to it", &s.key[mod_name.len() + 1..]));
                    }
                    if seen.insert(next.as_str()) {
                        todo.push(next);
                    }
                }
            }
        }
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
        Trigger::Answered { message, answer } => s.flags[flag::MESSAGE] && s.message == (*message, *answer),
        Trigger::All(ts) => ts.iter().all(|t| holds(t, s, w)),
        Trigger::Any(ts) => ts.iter().any(|t| holds(t, s, w)),
        Trigger::Not(t) => !holds(t, s, w),
        Trigger::Battle { ground, won, empire, planet: on } => {
            let e = empire.of(w);
            // A space battle: `Story_SpaceBattleFinished`'s participants and `SpWarKiNyert`.
            let space = *ground != Some(true)
                && on.is_none()
                && s.flags[flag::SPACE_END]
                && s.battle.iter().any(|p| p.owner == e)
                && (s.battle_winner == e) == *won;
            // A ground battle: the landing fleet's owner against the planet's owner when it started (`StoryAdat+0xf4`).
            let g = &s.ground;
            let (attacker, defender) = (g.fleet_owner, s.adat_i32(0xf4));
            let side = if attacker == e && defender != e {
                Some(g.attacker_won)
            } else if defender == e && attacker != e {
                Some(g.defender_won)
            } else {
                None
            };
            let ground_battle = *ground != Some(false) && s.flags[flag::GROUND_END] && on.is_none_or(|p| p == g.planet) && side == Some(*won);
            space || ground_battle
        }
        Trigger::Tech { tech, empire } => w.tech_status(*tech, empire.of(w)) == 5,
        Trigger::Money { empire, at_least, at_most } => {
            let m = w.money(empire.of(w));
            at_least.is_none_or(|b| m >= b as f64) && at_most.is_none_or(|b| m <= b as f64)
        }
        Trigger::Relation { a, b, at_least, at_most } => {
            let l = w.relation_level(a.of(w), b.of(w));
            at_least.is_none_or(|m| l >= m) && at_most.is_none_or(|m| l <= m)
        }
        Trigger::SpyReport { mission, target, success, died } => {
            let k = &s.kem;
            s.flags[flag::SPY]
                && s.spy_kind == 3
                && mission.is_none_or(|m| k.mission == m)
                && target.is_none_or(|t| k.target == t.of(w))
                && success.is_none_or(|x| k.success == x)
                && died.is_none_or(|d| (k.died == 1) == d)
        }
        Trigger::Building { kind, planet: on, empire } => {
            let e = empire.of(w);
            (1..=w.planet_count()).filter(|&p| on.is_none_or(|q| q == p) && w.planet_owner(p) == e).any(|p| {
                (1..=w.building_count(p)).any(|k| {
                    let (b, _, unfinished) = w.building(p, k);
                    b == *kind && unfinished == 0
                })
            })
        }
        Trigger::Timer(n) => st.timers.get(n).is_some_and(|&at| w.ido() >= at),
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
        Action::GiveTech { tech, empire } => {
            let e = empire.of(w);
            s.set_tech_state(*tech, e, w);
        }
        Action::RemoveTech { tech, empire } => w.event(StoryEvent::SetTechState { tech: *tech, state: 3, empire: empire.of(w) }),
        Action::ChangeRelation { a, b, by } => w.event(StoryEvent::RelationChange { a: a.of(w), b: b.of(w), delta: *by }),
        Action::Diplomacy { a, b, state } => {
            let (a, b) = (a.of(w), b.of(w));
            w.event(match state {
                Diplomacy::War => StoryEvent::DeclareWar { a, b },
                Diplomacy::Neutral => StoryEvent::MakeNeutral { a, b },
                Diplomacy::NonAggression => StoryEvent::MakeNonAggression { a, b },
                Diplomacy::Alliance => StoryEvent::MakeAllies { a, b },
                Diplomacy::Trade => StoryEvent::TradeAgreement { a, b },
            })
        }
        Action::AddBuilding { planet: p, kind, level } => {
            if planet(*p, w) {
                w.event(StoryEvent::PlaceStoryBuilding { planet: *p, kind: *kind, level: *level });
            }
        }
        Action::GroundUnits { place, design, count } => {
            let holder = if *place < 0 {
                planet(-place, w).then_some(*place)
            } else {
                (1..=w.fleet_count()).find(|&f| w.fleet_owner(f) != 0xff && w.fleet_id(f) == *place)
            };
            if let Some(fleet) = holder {
                w.event(StoryEvent::NewTanks { fleet, design: *design, count: *count });
            }
        }
        Action::StartTimer { name, days } => {
            let at = w.ido().saturating_add(days * DAY);
            s.mod_state.timers.insert(name.clone(), at);
        }
        Action::StopTimer(name) => {
            s.mod_state.timers.remove(name);
        }
        Action::Chain(_) => unreachable!("the interpreter runs chains"),
    }
}

/// May the story fire now? A once-story until it fired; a repeating one until `times`, `every` days apart (`gap`
/// false: a chain, which ignores `every`).
fn ready(story: &ModStory, s: &Story, ido: i32, gap: bool) -> bool {
    match story.repeat {
        None => !s.mod_state.fired.contains(&story.key),
        Some(r) => s.mod_state.repeats.get(&story.key).is_none_or(|&(last, n)| {
            r.times.is_none_or(|t| n < t) && (!gap || ido >= last.saturating_add(r.every.saturating_mul(DAY)))
        }),
    }
}

/// Its mark first (so a nested check its actions cause can't fire it again), then its actions in order; a chain fires
/// the named story at once when it may fire (the parser rules out loops).
fn fire(story: &ModStory, defs: &[ModStory], s: &mut Story, w: &mut dyn StoryWorld) {
    if story.repeat.is_some() {
        let r = s.mod_state.repeats.entry(story.key.clone()).or_insert((0, 0));
        *r = (w.ido(), r.1.saturating_add(1));
    } else {
        s.mod_state.fired.insert(story.key.clone());
    }
    for a in &story.actions {
        match a {
            Action::Chain(key) => {
                if let Some(next) = defs.iter().find(|d| d.key == *key)
                    && ready(next, s, w.ido(), false)
                {
                    fire(next, defs, s, w);
                }
            }
            _ => act(a, s, w),
        }
    }
}

/// The interpreter: every story that may fire and whose triggers all hold fires.
pub struct ModScripts;

impl Script for ModScripts {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        let defs = s.mod_defs.clone();
        for story in defs.iter() {
            if ready(story, s, w.ido(), true) && story.when.iter().all(|t| holds(t, s, w)) {
                fire(story, &defs, s, w);
            }
        }
    }
}

#[cfg(test)]
mod tests;
