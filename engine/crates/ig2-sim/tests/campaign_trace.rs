//! Replays the campaign recorded from the running GOG `ig2.exe` (tools/trace/run_story.py `--campaign`,
//! `re/rules/campaign.md` §8): every record of `campaign-*.jsonl`.
//! - `init`: `InitStory` (exe 0x5c2940) inside the campaign start, with a snapshot after `LoadScenario`;
//! - `tick`: one `FRISSIT_Story` (0x5c1ff0) the game ran (counters, the daily check, the inline stories);
//! - `check`: one outermost `Story_Check` (0x5c5610) a game hook ran;
//! - `staged`: a call the tracer made through its trampoline: `StartStory` + `Story_Check(1)` (the inline
//!   stories 402..918), `Story_Special` (0x11, 0x12), a fleet arrival, event flags + `Story_Check(1)`, `WIN`.
//!
//! - `--keep begin`: the opening episodes 1..24 (`st_Begin_include`, §8.12) staged with the world they test.
//! - `--keep multi`: the multiplayer modes 1001..1006 (`st_Multi_include`, §8.10) staged the same way, with
//!   counters, ground battles, specials 10/15, the radar offer's answers, base arrivals and space battles.
//! - `--keep brnd`: the random stories 220..239 (`st_BRnd_include`, §8.11); `--keep elem`: the storyline
//!   episodes of `st_Elem_include` part 1 (§8.12; the script's other blocks patched out).
//! - `--keep fable`: the random fables 301..316 (`st_Fable_include`, §8.15) with their timers, answers, space
//!   battles of the merchants, the escort's arrival and the agent's death.
//!
//! The tracer patches the scripts the port doesn't have out of the exe, so both sides run the same stories.
//! The port starts from the record's `pre` story state and world; afterwards the whole story state, stream 5,
//! `MEGNYERES`/`MEGHALAS` and every call the exe's stories made to the rest of the game (`acts`: new fleets,
//! fleet orders, ships, wormholes, …, in order, with their arguments) must match; the world's answers to the
//! calls that return something (`FlottaNew`'s slot, `st_IsInRR`, `st_GetTerv`) come from the record. Needs the
//! traces (`_local/traces` or `IG2_TRACE_DIR`) and the install (`IG2_INSTALL_DIR`, for the campaign tables);
//! skips without them.

use std::collections::HashMap;
use std::path::PathBuf;

use ig2_sim::begin::BeginStories;
use ig2_sim::brnd::BRndStories;
use ig2_sim::rnd::RndStories;
use ig2_sim::campaign::CampaignTables;
use ig2_sim::elem::ElemStories;
use ig2_sim::fable::FableStories;
use ig2_sim::multi::{MultiStories, FLAG_CARRIER_UNSET};
use ig2_sim::rng::{stream, Streams};
use ig2_sim::story::{
    self, BuildingData, FieldUnit, FleetData, KemNews, MessageText, Notice, Queue, ScenarioStories, ScenarioStory, Script, SpyRecord,
    Story, StoryEvent, StoryWorld, DESIGN, FLAGS,
};
use ig2_sim::storyline::LineStories;
use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::messages::{story_line, LineNames, Texts};

use serde_json::Value;

/// The scripts' ticker lines checked against the exe's.
static SCROLL_LINES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with(prefix) && f.ends_with(".jsonl")));
    v.sort();
    v
}

/// The buildings' workers, hit points and techs (`EpuletekLeiro` `+0x10`, `+8`, `+0`), from the user's exe.
struct Buildings {
    workers: Vec<i32>,
    hp: Vec<i32>,
    tech: Vec<i32>,
}

/// The campaign tables and the buildings, from the user's exe.
fn tables() -> Option<(CampaignTables, Buildings)> {
    let dir = std::env::var_os("IG2_INSTALL_DIR")?;
    let exe = std::fs::read(PathBuf::from(dir).join("ig2.exe")).ok()?;
    let rules = ig2_rules::Rules::from_exe(&exe).ok()?;
    let b = &rules.buildings;
    let buildings = Buildings {
        workers: b.iter().map(|b| b.workers).collect(),
        hp: b.iter().map(|b| b.max_hp).collect(),
        tech: b.iter().map(|b| b.tech as i32).collect(),
    };
    Some((CampaignTables::new(rules.campaign_words), buildings))
}

/// The story texts (`s_story.txt`, `txt02.txt`) of every language the install has, for the scripts' ticker lines.
fn story_texts() -> &'static [(Lang, Texts)] {
    static TEXTS: std::sync::OnceLock<Vec<(Lang, Texts)>> = std::sync::OnceLock::new();
    TEXTS.get_or_init(|| {
        Lang::ALL
            .into_iter()
            .filter_map(|lang| {
                let inst = Install::from_env(lang)?.ok()?;
                let read = |p| inst.read(p).unwrap_or_default();
                Some((lang, Texts::default().with_story(&read("dataraw/text/s_story.txt"), &read("dataraw/text/txt02.txt"), lang)))
            })
            .collect()
    })
}

/// A name the traces don't dump: matches any text (`\u{1}` in the rendered line).
struct AnyName;

impl LineNames for AnyName {
    fn tech(&self, _: i32) -> Vec<u8> {
        vec![1]
    }
    fn planet(&self, _: i32) -> Vec<u8> {
        vec![1]
    }
    fn empire(&self, _: i32) -> Vec<u8> {
        vec![1]
    }
}

/// The exe's ticker text (read as the system's ANSI code page, or as UTF-8 by the fable tracer, whose undecodable
/// bytes turn into U+FFFD) against the port's line decoded in its language, `\u{1}` matching any name.
fn same_line(exe: &str, port: &str) -> bool {
    // Non-ASCII runs compare as one placeholder when the exe's text lost them.
    let norm = |s: &str| -> String {
        if !exe.contains('\u{fffd}') {
            return s.to_string();
        }
        let mut out = String::new();
        for c in s.chars() {
            let c = if c.is_ascii() { c } else { '?' };
            if !(c == '?' && out.ends_with('?')) {
                out.push(c);
            }
        }
        out
    };
    let (exe, port) = (norm(exe), norm(port));
    let parts: Vec<&str> = port.split('\u{1}').collect();
    if parts.len() == 1 {
        return exe == port;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !exe.starts_with(first) || !exe[first.len()..].ends_with(last) || exe.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &exe[first.len()..exe.len() - last.len()];
    for p in &parts[1..parts.len() - 1] {
        match rest.find(p) {
            Some(i) => rest = &rest[i + p.len()..],
            None => return false,
        }
    }
    true
}

/// The scripts in `Story_Check`'s order.
struct Chain(Vec<Box<dyn Script>>);

impl Script for Chain {
    fn run(&self, story: &mut Story, world: &mut dyn StoryWorld) {
        self.run_in(story, world, self);
    }

    fn run_in(&self, story: &mut Story, world: &mut dyn StoryWorld, all: &dyn Script) {
        for s in &self.0 {
            s.run_in(story, world, all);
        }
    }
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn ints(v: &Value) -> Vec<i32> {
    v.as_array().unwrap().iter().map(int).collect()
}
fn pairs(v: &Value) -> Vec<(i32, i32)> {
    v.as_array().unwrap().iter().map(|p| (int(&p[0]), int(&p[1]))).collect()
}
fn queue(v: &Value) -> Queue {
    Queue { entries: pairs(v) }
}
fn bits(v: &Value) -> f32 {
    f32::from_bits(v.as_u64().unwrap() as u32)
}

#[derive(Clone)]
struct Planet {
    owner: i32,
    value: i32,
    vis: i32,
    x: u32,
    y: u32,
    radar: i32,
    /// `+0xbc`, the info slots 1..=5 (`+0x16a`), the views `+0xd8` (st_Line traces only).
    friendly: Option<u8>,
    infos: Vec<i32>,
    views: Vec<i16>,
    /// `+0xb8` (the epidemic), `+0x70`/`+0x74` (production), `+0x174` (morale; st_BRnd traces).
    b8: i32,
    prod: (i32, i32),
    morale: Option<i32>,
    /// `HanyEpulet[p]`, `+0xbb` (st_Begin traces).
    buildings: i32,
    notice: Option<i32>,
    /// `+0x30` (morale), `+0x22` (surface), `+0x24` (richness; st_Rnd traces: after st_BRnd's three, or after
    /// st_Begin's two when both were kept).
    morale30: i32,
    surface: i32,
    richness: i32,
    /// `+0xbb`, `+0x186`, `+0x187` (st_Elem traces).
    bb: Option<u8>,
    extra: Option<(u8, u8)>,
}

#[derive(Clone)]
struct Fleet {
    owner: i32,
    id: i32,
    x: f32,
    y: f32,
    order: i32,
    w5c: i32,
    speed: u32,
    /// `+0x40`, `+0x44`, `+0x18` (st_BRnd traces).
    command: i32,
    target: f32,
    state: i32,
    /// `+0x52`, `+0x60` (st_Begin traces).
    flags: i32,
    power: i32,
    /// `+0x54`: the ship count (st_Rnd part 2b traces).
    ships: i32,
}

/// The world of one record, with the exe's calls as the oracle.
struct W<'a> {
    w: Value,
    tables: &'a CampaignTables,
    rng: Streams,
    planets: Vec<Planet>,
    fleets: Vec<Fleet>,
    en: i32,
    /// The record's outermost calls, in order, and the next one to match.
    acts: Vec<Value>,
    next: usize,
    ground: HashMap<(i32, i32), bool>,
    config: HashMap<String, i32>,
    registry: Option<Option<i32>>,
    /// `init`: the world after `LoadScenario` and the story state after `InitStory`.
    mid: Option<Value>,
    pre_story: Value,
    post_story: Option<Value>,
    post_w: Value,
    pre_w: Value,
    events: Vec<StoryEvent>,
    bad: Vec<String>,
    unchecked: Vec<String>,
    /// Empires the stories made the human meet (`st_FelfedezBirodalom`).
    met: Vec<i32>,
    /// The info string ids (`InfoString_GetIndex`'s table) and the design records seen at `TervUjraSzamol`.
    infos: &'a [i32],
    designs: HashMap<(i32, i32), Vec<u8>>,
    /// Fleets' `+0x52` with the port's `SetFleetVisible` applied (checked against `post`).
    f52: Option<Vec<i32>>,
    buildings: &'a Buildings,
    /// `Tankok` with the port's `TankDelete`s applied (st_Begin traces).
    tanks: Vec<(i32, i32, i32)>,
    /// A building the trace didn't dump was read: the record can't be checked.
    missing: std::cell::Cell<bool>,
    /// st_Begin's direct writes, checked against `post` at the end: fleets' `+0x52` (set, cleared), planets'
    /// epidemic, the relations' value 2, the trade offers added, designs' price.
    fleet_flags: HashMap<i32, (i32, i32)>,
    epidemic: HashMap<i32, i32>,
    relations: HashMap<i32, i32>,
    offers_added: i32,
    prices: HashMap<i32, i32>,
    /// The designs `new_design_raw` made, checked at their `TervUjraSzamol`.
    made: HashMap<(i32, i32), [u8; DESIGN]>,
    /// The last value the port wrote to a planet's `+0x28`, a ship's design, a planet's morale (checked against
    /// `post` at the end).
    values: HashMap<i32, i32>,
    ship_designs: HashMap<i32, i32>,
    morale: HashMap<i32, i32>,
    gmoral: Option<i32>,
    /// The last value the port wrote to a planet's `+0x24` (st_Rnd).
    richness: HashMap<i32, i32>,
    /// Ships whose design the port changed (`UpgradeShip`), seen by later reads in the record.
    ships: Option<Vec<(i32, i32)>>,
    /// st_Elem: the planets' `+0x186/+0x187` the port wrote, the techs' `[t, +0x34, +0x38, left]` with the port's
    /// writes (checked against `post` at the end).
    extras: HashMap<i32, bool>,
    tt: Option<Vec<[i32; 4]>>,
    /// st_Rnd part 2's direct writes, checked against `post` at the end: the human's spies' state and experience
    /// added, the techs seen, the buildings' bytes.
    spy_states: HashMap<i32, i32>,
    spy_xp: HashMap<i32, i32>,
    techs_seen: Vec<(i32, i32)>,
    building_bytes: HashMap<(i32, i32, i32), i32>,
    /// st_Fable: the story agent's name, the human's spies `(+0x10, +0x16, name)`, `spFlottak` `[count, (slot,
    /// flag) × 4]`, the fleets the record made, the spies the port made or killed.
    agent: Option<String>,
    spies: Vec<(i32, i32, String)>,
    spf: Option<(i32, Vec<(i32, i32)>)>,
    new_fleets: Vec<i32>,
    story_spy: Option<(i32, i32)>,
    /// `OsszHanyFlotta`: st_Fable traces also hold the stale records past it (story 311 reads them).
    nfl: i32,
    /// st_Rnd part 2b: fleets joining the starting battle, fleets' `+0x18` bits, techs' prerequisites written.
    joined: Vec<i32>,
    fleet_state: HashMap<i32, (i32, i32)>,
    prereqs: HashMap<(i32, i32), i32>,
    joined_n: Option<i32>,
    /// The `TankDelete`s the game made inside the stories' calls, with the index of the outermost call they ran in
    /// (`HajoDelete` drops the units its fleet can no longer carry); applied as that call is matched.
    nested_tank_deletes: Vec<(usize, Value)>,
    /// The `FlottaMozgat`s nested in an outermost call (`FlottaDelete` stops the fleets following the deleted one).
    nested_moves: Vec<(usize, Value)>,
    /// st_Rnd part 2c: `_s(5, id)`, 138's recruit names, `AlapFlottaNev` (from the setup record); the port's writes,
    /// checked against `post` at the end: fleet names, positions and owners, units' designs, design keys, flags and
    /// names, other empires' views of planets, planet names, the recruits.
    stext: HashMap<i32, String>,
    recruits: Vec<String>,
    alapnev: String,
    fleet_names: HashMap<i32, Option<i32>>,
    fleet_pos: HashMap<i32, (f32, f32)>,
    fleet_owners: HashMap<i32, i32>,
    tank_designs: HashMap<i32, i32>,
    design_keys: HashMap<(i32, i32), i32>,
    design_flags: HashMap<i32, (i32, i32)>,
    design_names: HashMap<i32, Option<i32>>,
    views_of: HashMap<(i32, i32), i32>,
    planet_names: HashMap<i32, i32>,
    recruited: Vec<(i32, i32, i32)>,
    stray: Option<(i32, i32)>,
    unhidden: Vec<i32>,
    fleet_ids: HashMap<i32, i32>,
    made_keys: HashMap<(i32, i32), i32>,
    /// st_Elem part 2a: the battlefields' self-destruct timers the port wrote, the levels both ways of the wars
    /// declared (checked against `post` at the end).
    self_destruct: HashMap<i32, i32>,
    wars: Vec<i32>,
    /// st_Elem part 2b: the battlefields' units (`wfus` rows) as the record changes them, the units the port made
    /// riot `(planet, unit, fleet, design)`, the planet whose battlefield it held, the building slots it deleted
    /// (checked against `post` at the end).
    wf_units: HashMap<i32, Vec<Value>>,
    rioted: Vec<(i32, i32, i32, i32)>,
    held: Option<i32>,
    deleted: Vec<(i32, i32)>,
    /// st_Elem part 2d: designs' bytes written (`(design, offset)`), planets' `+0x187` alone.
    design_bytes: HashMap<(i32, i32), i32>,
    extra187: HashMap<i32, bool>,
    /// st_Elem part 2f: the alliances made (836's, 864's; checked against `post`'s `vlev` with the wars), 840's colony
    /// discount applied to `pre`'s `cdis` (designs `[d, +0x34, +0x20, +0x24]`, planets `[p, +0x78, its +0x34, +0xa0]`),
    /// whether the record is a tick (the planets' `+0xa0` move with production then).
    allies: Vec<(i32, i32)>,
    cdis: Option<Value>,
    tick: bool,
    /// st_Elem part 2g: the wars between two others declared (862's 1/5; checked against `post`'s `vlev`).
    pair_wars: Vec<(i32, i32)>,
    /// The fighters (designs with `+0x16 ≠ 2`) the record's TankDeletes took from each fleet (`+0x58`).
    fighters_lost: HashMap<i32, i32>,
}

fn planets_of(w: &Value) -> Vec<Planet> {
    let elem = w.get("vrel").is_some();
    w["planets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Planet {
            owner: int(&p[0]),
            value: int(&p[1]),
            vis: int(&p[2]),
            x: p[3].as_u64().unwrap() as u32,
            y: p[4].as_u64().unwrap() as u32,
            radar: int(&p[5]),
            friendly: p.get(8).map(|x| int(x) as u8),
            infos: p.get(9).map(ints).unwrap_or_default(),
            views: p.get(10).and_then(|x| x.as_str()).map(|h| unhex(h).chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()).unwrap_or_default(),
            b8: p.get(6).map_or(0, int),
            prod: p.get(12).map_or((0, 0), |b| (int(&p[11]), int(b))),
            morale: p.get(13).map(int),
            // st_Elem's rows (17 fields, like st_Rnd's) hold +0xbb, +0x186, +0x187 at 14..=16.
            buildings: if elem { 0 } else { p.get(14).map_or(0, int) },
            notice: if elem { None } else { p.get(15).map(int) },
            morale30: if elem { 0 } else { rnd_field(p, 0) },
            surface: if elem { 0 } else { rnd_field(p, 1) },
            richness: if elem { 0 } else { rnd_field(p, 2) },
            bb: if elem { p.get(14).map(|x| int(x) as u8) } else { None },
            extra: if elem { p.get(16).map(|x| (int(&p[15]) as u8, int(x) as u8)) } else { None },
        })
        .collect()
}

/// A planet row's st_Rnd field `i`: rows of 17 (st_BRnd's + st_Rnd's) or 19 (with st_Begin's two before them).
fn rnd_field(p: &Value, i: usize) -> i32 {
    let row = p.as_array().unwrap();
    match row.len() {
        17 => int(&row[14 + i]),
        19 => int(&row[16 + i]),
        _ => 0,
    }
}

fn fleets_of(w: &Value) -> Vec<Fleet> {
    w["fleets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| Fleet {
            owner: int(&f[0]),
            id: int(&f[1]),
            x: bits(&f[2]),
            y: bits(&f[3]),
            order: int(&f[4]),
            w5c: int(&f[5]),
            speed: f[6].as_u64().unwrap() as u32,
            command: f.get(7).map_or(0, int),
            target: f.get(8).map_or(0.0, bits),
            state: f.get(9).map_or(0, int),
            flags: f.get(10).map_or(0, int),
            power: f.get(11).map_or(0, int),
            ships: f.get(12).map_or(0, int),
        })
        .collect()
}

impl<'a> W<'a> {
    fn new(r: &Value, tables: &'a CampaignTables, infos: &'a [i32], buildings: &'a Buildings) -> Self {
        let w = r["pre"]["w"].clone();
        let mut rng = Streams::default();
        rng.set_state(story::STREAM, w["seed5"].as_u64().unwrap() as u32);
        if let Some(s2) = w["seed2"].as_u64() {
            rng.set_state(stream::GROUND_COMBAT, s2 as u32);
        }
        if let Some(s4) = w["seed4"].as_u64() {
            rng.set_state(stream::SPY, s4 as u32);
        }
        // st_Elem part 2d: stream 0 (806's first pick).
        if let Some(s0) = w["seed0"].as_u64() {
            rng.set_state(0, s0 as u32);
        }
        let mut designs = HashMap::new();
        // `EpuletDelete` is logged for story 817; `InitStory`'s own (the campaign map's set-up) aren't the stories'.
        let init = r["k"] == "init";
        let acts: Vec<Value> =
            r["acts"].as_array().unwrap().iter().filter(|a| int(&a[2]) == 0 && !(init && a[0] == "bdelete")).cloned().collect();
        let mut ground = HashMap::new();
        let mut config = HashMap::new();
        let mut registry = None;
        for p in r["probes"].as_array().unwrap() {
            match p[0].as_str().unwrap() {
                "addship" if int(&p[5]) >= 0 => {
                    ground.insert((int(&p[5]), int(&p[2])), int(&p[6]) != 0);
                }
                "config" => {
                    config.insert(p[1].as_str().unwrap().to_string(), p[2].as_str().unwrap().trim().parse::<i32>().unwrap_or(0));
                }
                "registry" => registry = Some((int(&p[2]) != 0).then(|| int(&p[3]))),
                "design" => {
                    designs.insert((int(&p[1]), int(&p[2])), unhex(p[3].as_str().unwrap()));
                }
                _ => {}
            }
        }
        let spies = w["kem"].as_array().map_or(Vec::new(), |k| {
            // The name up to its NUL (the first traces decoded the bytes after it as U+FFFD).
            k.iter().map(|x| (int(&x[0]), int(&x[1]), x[2].as_str().unwrap_or("").split(['\0', '\u{fffd}']).next().unwrap().to_string())).collect()
        });
        let nfl = fleet_total(&w);
        let spf = w["spfl"].as_array().map(|a| (int(&a[0]), a[1..].iter().map(|e| (int(&e[0]), int(&e[1]))).collect()));
        Self {
            planets: planets_of(&w),
            fleets: fleets_of(&w),
            en: int(&w["en"]),
            w,
            tables,
            rng,
            acts,
            next: 0,
            ground,
            config,
            registry,
            mid: r.get("mid").cloned(),
            pre_story: r["pre"]["s"].clone(),
            post_story: Some(r["post"]["s"].clone()),
            post_w: r["post"]["w"].clone(),
            pre_w: r["pre"]["w"].clone(),
            events: Vec::new(),
            bad: Vec::new(),
            unchecked: Vec::new(),
            met: Vec::new(),
            infos,
            designs,
            f52: w_f52(&r["pre"]["w"]),
            buildings,
            tanks: r["pre"]["w"]["tanks"].as_array().map_or(Vec::new(), |t| t.iter().map(|x| (int(&x[0]), int(&x[1]), int(&x[2]))).collect()),
            missing: std::cell::Cell::new(false),
            fleet_flags: HashMap::new(),
            epidemic: HashMap::new(),
            relations: HashMap::new(),
            offers_added: 0,
            prices: HashMap::new(),
            made: HashMap::new(),
            values: HashMap::new(),
            ship_designs: HashMap::new(),
            morale: HashMap::new(),
            gmoral: None,
            richness: HashMap::new(),
            ships: None,
            extras: HashMap::new(),
            tt: None,
            spy_states: HashMap::new(),
            spy_xp: HashMap::new(),
            techs_seen: Vec::new(),
            building_bytes: HashMap::new(),
            agent: None,
            spies,
            spf,
            new_fleets: Vec::new(),
            story_spy: None,
            nfl,
            joined: Vec::new(),
            fleet_state: HashMap::new(),
            prereqs: HashMap::new(),
            joined_n: None,
            nested_tank_deletes: nested_tank_deletes(r, init),
            nested_moves: nested_acts(r, init, "move"),
            stext: HashMap::new(),
            recruits: Vec::new(),
            alapnev: String::new(),
            fleet_names: HashMap::new(),
            fleet_pos: HashMap::new(),
            fleet_owners: HashMap::new(),
            tank_designs: HashMap::new(),
            design_keys: HashMap::new(),
            design_flags: HashMap::new(),
            design_names: HashMap::new(),
            views_of: HashMap::new(),
            planet_names: HashMap::new(),
            recruited: Vec::new(),
            stray: None,
            unhidden: Vec::new(),
            fleet_ids: HashMap::new(),
            made_keys: HashMap::new(),
            self_destruct: HashMap::new(),
            wars: Vec::new(),
            wf_units: r["pre"]["w"]["wfus"].as_array().map_or_else(HashMap::new, |rows| rows.iter().map(|r| (int(&r[0]), r[1].as_array().unwrap().clone())).collect()),
            rioted: Vec::new(),
            held: None,
            deleted: Vec::new(),
            design_bytes: HashMap::new(),
            extra187: HashMap::new(),
            allies: Vec::new(),
            pair_wars: Vec::new(),
            fighters_lost: HashMap::new(),
            cdis: None,
            tick: r["k"].as_str() == Some("tick"),
        }
    }

    /// st_Elem part 2f: 840's colony discount (spec §8.14 part 2f) on the `cdis` model.
    fn colony_discount(&mut self, halve: bool, what: &str) {
        let Some(mut m) = self.cdis.take().or_else(|| self.w.get("cdis").cloned()) else {
            self.bad.push(format!("{what}: cdis not dumped"));
            return;
        };
        let f = |x: i32| if halve { x / 2 } else { x.wrapping_add(x) };
        for d in m[0].as_array_mut().unwrap() {
            if int(&d[1]) != 0 {
                d[2] = v(f(int(&d[2])));
                d[3] = v(f(int(&d[3])));
            }
        }
        for q in m[1].as_array_mut().unwrap() {
            if int(&q[1]) > 0 && int(&q[2]) != 0 {
                q[3] = v(f(int(&q[3])));
            }
        }
        self.cdis = Some(m);
    }

    /// st_Elem part 2b: the `wfus` row of planet `p` in `post`.
    fn post_units(&self, p: i32) -> Option<(Vec<Value>, i32)> {
        let post = self.post_world();
        let row = post["wfus"].as_array()?.iter().find(|r| int(&r[0]) == p)?.clone();
        Some((row[1].as_array().unwrap().clone(), int(&row[2])))
    }

    /// The next outermost call of the exe: must be `name`; returns it.
    fn take(&mut self, name: &str, what: &str) -> Option<Value> {
        match self.acts.get(self.next) {
            Some(a) if a[0] == name => {
                self.next += 1;
                Some(a.clone())
            }
            other => {
                self.bad.push(format!("port {what}, exe {}", other.map_or("nothing".into(), |a| a.to_string())));
                // Resynchronise on the next call of that name, so one difference reports once.
                if let Some(k) = self.acts[self.next..].iter().position(|a| a[0] == name) {
                    self.next += k + 1;
                    return Some(self.acts[self.next - 1].clone());
                }
                None
            }
        }
    }

    /// A script's ticker line (`ScrollUzenet(text, 1, imp, 0)`, `info-messages.md` §3.4): the exe's text in one of
    /// the install's languages, id 1, and the flags where the tracer logged them.
    fn scroll(&mut self, ev: &StoryEvent, what: &str) {
        let Some(x) = self.take("scroll", what) else { return };
        let a = x[1].as_array().unwrap();
        let exe = a[0].as_str().unwrap_or("");
        let mut important = None;
        let fits = story_texts().iter().any(|(lang, texts)| {
            let Some((line, imp)) = story_line(ev, texts, &AnyName) else { return false };
            important = Some(imp);
            same_line(exe, &ig2_formats::text::decode(&line, *lang))
        });
        if !story_texts().is_empty() && !fits {
            let port = story_texts().iter().find(|(l, _)| *l == Lang::Hu).or(story_texts().first()).and_then(|(l, t)| {
                story_line(ev, t, &AnyName).map(|(line, _)| ig2_formats::text::decode(&line, *l))
            });
            self.bad.push(format!("{what}: ticker text: port {port:?} exe {exe:?}"));
        }
        SCROLL_LINES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let flags = a.get(2).map(|_| (int(&a[2]), int(&a[3])));
        if int(&a[1]) != 1 || flags.is_some_and(|f| Some(f) != important.map(|i| (i as i32, 0))) {
            self.bad.push(format!("{what}: ticker id/flags: port (1, {important:?}, 0) exe {}", x[1]));
        }
    }

    fn expect(&mut self, name: &str, args: &[Value], what: &str) {
        if let Some(a) = self.take(name, what) {
            let got: Vec<Value> = a[1].as_array().unwrap().clone();
            if got.len() < args.len() || got[..args.len()] != *args {
                self.bad.push(format!("{what}: exe args {got:?}"));
            }
        }
    }

    /// A call on a battlefield unit: `[p, k, extra…, [[p, k]…]]` (part 2a's traces: no list). A stale battlefield may
    /// hold the unit's pointer too: any planet listed matches.
    fn expect_unit(&mut self, name: &str, planet: i32, unit: i32, extra: &[Value], what: &str) {
        if let Some(a) = self.take(name, what) {
            let got = a[1].as_array().unwrap();
            let n = 2 + extra.len();
            let here = |x: &Value| int(&x[0]) == planet && int(&x[1]) == unit;
            let listed = got.get(n).and_then(|l| l.as_array()).is_some_and(|l| l.iter().any(here));
            let first = got.len() >= 2 && int(&got[0]) == planet && int(&got[1]) == unit;
            if got.len() < n || !(first || listed) || got[2..n] != *extra {
                self.bad.push(format!("{what}: exe args {got:?}"));
            }
        }
    }

    /// `NewMessage`: may slow the game down (`SetIdoLelassit`), which the rest of the tick sees.
    fn message(&mut self, id: i32, arg: i32, what: &str) {
        self.expect("message", &[v(id), v(arg)], what);
        self.w["speed"] = self.post_w["speed"].clone();
    }

    /// A design field of the human's (`designs` column `col`) or of story 7's planet's owner (`odes` column
    /// `other`); another empire's design can't be checked.
    fn design_field(&self, e: i32, d: i32, col: usize, other: usize) -> i32 {
        if e == self.en {
            return self.w["designs"].get(d as usize).map_or(0, |x| int(&x[col]));
        }
        match self.w.get("odes") {
            Some(o) if int(&o[0]) == e => o[1].get(d as usize).map_or(0, |x| int(&x[other])),
            _ => {
                self.missing.set(true);
                0
            }
        }
    }

    /// st_Elem: the building slots of planet `p` (an array `[p, [[kind, +8]…]]` per planet; st_Begin's `bld` is an
    /// object).
    fn elem_buildings(&self, p: i32) -> Option<Vec<Value>> {
        let rows = self.w["bld"].as_array()?;
        rows.iter().find(|r| int(&r[0]) == p).map(|r| r[1].as_array().unwrap().clone())
    }

    /// st_Elem: the techs 6 and 161..=171 as the record holds them, with the port's writes.
    fn techs(&self) -> Vec<[i32; 4]> {
        self.tt.clone().unwrap_or_else(|| tech_rows(&self.w))
    }

    fn byte(&self, key: &str, i: i32) -> u8 {
        self.w[key].as_str().map(unhex).and_then(|b| b.get(i as usize).copied()).unwrap_or(0)
    }
}

fn tech_rows(w: &Value) -> Vec<[i32; 4]> {
    w["tt"].as_array().expect("techs not recorded").iter().map(|r| [int(&r[0]), int(&r[1]), int(&r[2]), int(&r[3])]).collect()
}

/// The nested `TankDelete`s of a record with the index (among the outermost calls the replay matches) of the call
/// they ran in.
fn nested_tank_deletes(r: &Value, init: bool) -> Vec<(usize, Value)> {
    nested_acts(r, init, "tankdelete")
}

/// The nested calls `name` with the index of the outermost call they run in.
fn nested_acts(r: &Value, init: bool, name: &str) -> Vec<(usize, Value)> {
    let mut out = Vec::new();
    let mut outer = 0usize;
    for a in r["acts"].as_array().unwrap() {
        if int(&a[2]) == 0 {
            if !(init && a[0] == "bdelete") {
                outer += 1;
            }
        } else if a[0] == name {
            out.push((outer.wrapping_sub(1), a[1].clone()));
        }
    }
    out
}

impl W<'_> {
    /// Applies the nested `TankDelete`s of the outermost call `k` (`usize::MAX`: every one left).
    fn apply_nested_tank_deletes(&mut self, k: usize) {
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.nested_tank_deletes).into_iter().partition(|(i, _)| k == usize::MAX || *i == k);
        self.nested_tank_deletes = later;
        for (_, a) in now {
            let (place, design, count) = (int(&a[0]), int(&a[1]), int(&a[2]));
            if let Some(k) = (1..self.tanks.len()).find(|&k| self.tanks[k].0 == place && self.tanks[k].1 == design) {
                let t = &mut self.tanks[k];
                t.2 -= count.min(t.2);
                if t.2 < 1 {
                    t.1 = 0x7fff;
                }
            }
        }
    }

    /// A design's row in `des2` (every empire's designs while a hero runs: `[+0x2c, +0x16, +0x80]`).
    fn des2(&self, e: i32, d: i32) -> Option<&Value> {
        self.w["des2"].get(e as usize).and_then(|x| x.get(d as usize))
    }
}

/// `OsszHanyFlotta` (st_Fable traces record it: their fleet lists go on to the ship count).
fn fleet_total(w: &Value) -> i32 {
    w.get("nfl").map_or(w["fleets"].as_array().unwrap().len() as i32 - 1, int)
}

fn w_f52(w: &Value) -> Option<Vec<i32>> {
    w["f52"].as_array().map(|a| a.iter().map(int).collect())
}

fn v<T: Into<Value>>(x: T) -> Value {
    x.into()
}

impl StoryWorld for W<'_> {
    fn ido(&self) -> i32 {
        int(&self.w["ido"])
    }
    fn speed_mult(&self) -> i32 {
        int(&self.w["speed"])
    }
    fn human(&self) -> i32 {
        self.en
    }
    fn difficulty(&self) -> i32 {
        int(&self.w["neh"])
    }
    fn multiplayer(&self) -> bool {
        int(&self.w["mp"]) != 0
    }
    fn rng(&mut self) -> &mut Streams {
        &mut self.rng
    }
    fn planet_count(&self) -> i32 {
        self.planets.len() as i32 - 1
    }
    fn planet_owner(&self, p: i32) -> i32 {
        self.planets.get(p as usize).map_or(0, |p| p.owner)
    }
    fn planet_value(&self, p: i32) -> i32 {
        self.planets[p as usize].value
    }
    fn planet_infected(&self, p: i32) -> bool {
        self.planets[p as usize].b8 > 0
    }
    fn planet_visibility(&self, p: i32) -> i32 {
        self.planets[p as usize].vis
    }
    fn fleet_count(&self) -> i32 {
        self.nfl
    }
    fn fleet_owner(&self, f: i32) -> i32 {
        match self.fleets.get(f as usize) {
            Some(q) => q.owner,
            None => self.w["fext"].get(f as usize - self.fleets.len()).map_or(0, |e| int(&e[0])),
        }
    }
    fn fleet_id(&self, f: i32) -> i32 {
        self.fleets[f as usize].id
    }
    fn empire_alive(&self, e: i32) -> bool {
        self.byte("el", e) != 0
    }
    fn planets_of(&self, e: i32) -> i32 {
        int(&self.w["hanyb"][e as usize])
    }
    fn fleet_infected(&self, f: i32) -> bool {
        let f = &self.fleets[f as usize];
        f.owner != 0xff && f.flags & 4 != 0
    }
    fn money(&self, _e: i32) -> f64 {
        self.w["money"].as_f64().unwrap()
    }
    fn tech_status(&self, tech: i32, e: i32) -> i32 {
        // st_Rnd 127's records hold every empire's statuses (`tstat`, 19 bytes a tech).
        if let Some(h) = self.w["tstat"].as_str() {
            return unhex(h)[19 * tech as usize + e as usize] as i32;
        }
        match self.w["techs"].as_str() {
            Some(h) => {
                assert_eq!(e, self.en);
                unhex(h)[4 * tech as usize] as i32
            }
            None => 0,
        }
    }
    fn relation_level(&self, _a: i32, b: i32) -> i32 {
        int(&self.w["visz"][b as usize])
    }
    fn campaign_type(&self) -> i32 {
        int(&self.w["camp"])
    }
    fn campaign_tables(&self) -> &CampaignTables {
        self.tables
    }
    fn race(&self, e: i32) -> i32 {
        self.byte("faj", e) as i32
    }
    fn knows(&self, a: i32, b: i32) -> bool {
        assert_eq!(a, self.en);
        self.byte("ismeri", b) != 0 || self.met.contains(&b)
    }
    fn planet_has_info(&self, p: i32, info: i32) -> bool {
        self.planets[p as usize].infos.contains(&info)
    }
    fn empire_strength(&self, e: i32) -> f32 {
        bits(&self.w["ero"][e as usize])
    }
    fn akt_planet(&self) -> i32 {
        int(&self.w["akt"][0])
    }
    fn akt_fleet(&self) -> i32 {
        int(&self.w["akt"][1])
    }
    fn planet_xy(&self, p: i32) -> (f32, f32) {
        // st_Elem 834 places its fleets on planet −1: the record before the planets (`bm1`).
        if p < 0 {
            let e = &self.w["bm1"];
            return (bits(&e[0]), bits(&e[1]));
        }
        let p = &self.planets[p as usize];
        (f32::from_bits(p.x), f32::from_bits(p.y))
    }
    fn planet_radar(&self, p: i32) -> i32 {
        self.planets[p as usize].radar
    }
    fn radar_range(&self, e: i32, level: i32) -> f32 {
        // The trace holds the human's radar row and (newer traces) owner 0's, which st_fNewEx reads when the
        // human has no planet left (slot 0); the rows are alike in the older traces.
        if let Some(r) = self.w.get("rrall") {
            return bits(&r[e as usize][level as usize]);
        }
        match e {
            0 if self.w.get("rr0").is_some() => bits(&self.w["rr0"][level as usize]),
            e if e == self.en || e == 0 => bits(&self.w["rr2"][level as usize]),
            _ => panic!("radar row of empire {e} read"),
        }
    }
    fn fleet_xy(&self, f: i32) -> (f32, f32) {
        match self.fleets.get(f as usize) {
            Some(q) => (q.x, q.y),
            None => {
                let e = &self.w["fext"][f as usize - self.fleets.len()];
                (bits(&e[1]), bits(&e[2]))
            }
        }
    }
    fn conquest_ends_epidemic(&mut self, p: i32) -> bool {
        // Traces without the fleets' `+0x52` (st_Line's, st_Elem's) can't tell: the epidemic goes on.
        if self.w["fleets"].as_array().is_none_or(|a| a.iter().all(|f| f.get(10).is_none())) {
            return false;
        }
        let owner = self.planet_owner(p);
        let (px, py) = self.planet_xy(p);
        let planets = (1..self.planets.len()).any(|q| q as i32 != p && self.planets[q].b8 > 0);
        let fleets = self.fleets.iter().enumerate().skip(1).any(|(_, f)| {
            let (dx, dy) = (f.x - px, f.y - py);
            f.flags & 4 != 0 && !(f.owner == owner && dy * dy + dx * dx < 100.0)
        });
        !planets && !fleets
    }
    fn fleet_ship_count(&self, f: i32) -> i32 {
        self.fleets[f as usize].ships
    }
    fn near_fleet(&mut self, p: i32, radius: f32) -> i32 {
        let what = format!("SearchBolygohozKozeliFlotta({p}, {radius})");
        match self.take("nearfleet", &what) {
            Some(a) => {
                if int(&a[1][0]) != p || a[1][1].as_u64() != Some(radius.to_bits() as u64) {
                    self.bad.push(format!("{what}: exe {}", a[1]));
                }
                int(&a[3])
            }
            None => 0,
        }
    }
    fn near_fleet_ex(&mut self, p: i32, radius: f32, own: bool, idle: bool, room: bool, calm: bool, last: i32) -> i32 {
        let what = format!("SearchBolygohozKozeliFlotta({p}, {radius}, {own}, {idle}, {room}, {calm}, {last})");
        match self.take("nearfleet", &what) {
            Some(a) => {
                let flags = [own, idle, room, calm].map(|b| b as i64);
                let got: Vec<i64> = (2..6).map(|i| a[1].get(i).and_then(|x| x.as_i64()).unwrap_or(-1) & 0xff).collect();
                if int(&a[1][0]) != p || a[1][1].as_u64() != Some(radius.to_bits() as u64) || got != flags || a[1].get(6).map(int) != Some(last) {
                    self.bad.push(format!("{what}: exe {}", a[1]));
                }
                int(&a[3])
            }
            None => 0,
        }
    }
    fn design_kind(&self, e: i32, d: i32) -> i32 {
        match self.w["des2"].get(e as usize).and_then(|x| x.get(d as usize)) {
            Some(r) => int(&r[1]),
            None => {
                self.missing.set(true);
                0
            }
        }
    }
    fn fleet_order(&self, f: i32) -> i32 {
        self.fleets[f as usize].order
    }
    fn fleet_5c(&self, f: i32) -> i32 {
        self.fleets[f as usize].w5c
    }
    fn ship_count(&self) -> i32 {
        self.w["ships"].as_array().map_or(0, |s| s.len() as i32 - 1)
    }
    fn ship(&self, k: i32) -> (i32, i32) {
        // A slot past the list (801's stale ship variables) reads free.
        if let Some(s) = &self.ships {
            return s.get(k.max(0) as usize).copied().unwrap_or((0, 0x7fff));
        }
        match self.w["ships"].get(k.max(0) as usize) {
            Some(s) => (int(&s[0]), int(&s[1])),
            None => (0, 0x7fff),
        }
    }
    fn design_upgrade(&self, e: i32, d: i32) -> i32 {
        assert_eq!(e, self.en);
        self.w["designs"].get(d as usize).map_or(0, |x| int(&x[0]))
    }
    fn design_price(&self, e: i32, d: i32) -> i32 {
        assert_eq!(e, self.en);
        self.w["designs"].get(d as usize).map_or(0, |x| int(&x[1]))
    }
    fn design_is_ground(&self, e: i32, d: i32) -> bool {
        *self.ground.get(&(e, d)).unwrap_or(&false)
    }
    fn ai_design(&self, e: i32, _slot: i32) -> i32 {
        int(&self.w["mi13"][e as usize])
    }
    fn in_radar_range(&mut self, x: i32, y: i32, k: i32) -> bool {
        let what = format!("st_IsInRR({x}, {y}, {k})");
        match self.take("inrr", &what) {
            Some(a) => {
                if ints(&a[1]) != [x, y, k] {
                    self.bad.push(format!("{what}: exe {}", a[1]));
                }
                int(&a[3]) != 0
            }
            None => false,
        }
    }
    fn story_design(&mut self, e: i32, key: i32, default: i32) -> i32 {
        let what = format!("st_GetTerv({e}, {key}, {default})");
        match self.take("getterv", &what) {
            Some(a) => {
                if ints(&a[1]) != [e, key, default] {
                    self.bad.push(format!("{what}: exe {}", a[1]));
                }
                // A design `st_GetTerv` made in the record carries the key (st_Rnd 2c's searches read it).
                let d = int(&a[3]);
                if self.des2(e, d).is_none() && d > 0 {
                    self.made_keys.insert((e, d), key);
                }
                d
            }
            None => default,
        }
    }
    fn new_fleet(&mut self, _name: &ig2_sim::story::FleetName, owner: i32, id: i32, x: f32, y: f32, speed: f32) -> i32 {
        let what = format!("FlottaNew({owner}, {id}, {x}, {y}, {speed})");
        let Some(a) = self.take("fleetnew", &what) else { return 0 };
        let want = [v(owner), v(id), v(x.to_bits()), v(y.to_bits()), v(speed.to_bits())];
        if a[1].as_array().unwrap()[..] != want {
            self.bad.push(format!("{what}: exe {}", a[1]));
        }
        let f = int(&a[3]);
        if f as usize >= self.fleets.len() {
            self.fleets.resize(f as usize + 1, Fleet { owner: 0xff, id: 0, x: 0.0, y: 0.0, order: 0, w5c: 0, speed: 0, command: 0, target: 0.0, state: 0, flags: 0, power: 0, ships: 0 });
        }
        // +0x5c and +0x60 come from FlottaNew/HajoNew (not the stories): the exe's values after the record.
        let post = fleets_of(&self.post_world()).get(f as usize).map_or((0, 0), |q| (q.w5c, q.power));
        self.fleets[f as usize] =
            Fleet { owner, id, x, y, order: 0, w5c: post.0, speed: speed.to_bits(), command: 0, target: 0.0, state: 0, flags: 0, power: post.1, ships: 0 };
        self.new_fleets.push(f);
        self.nfl = self.nfl.max(f);
        f
    }
    fn config_int(&self, key: &str) -> i32 {
        *self.config.get(key).unwrap_or(&0)
    }
    fn registry_storyline(&self, _campaign: i32) -> Option<i32> {
        self.registry.flatten()
    }
    fn load_scenario(&mut self, n: i32) -> Vec<ScenarioStory> {
        self.expect("loadscenario", &[v(format!("scenario/{n}.scn")), v(1)], &format!("LoadScenario({n})"));
        let Some(mid) = self.mid.clone() else {
            self.bad.push("no snapshot after LoadScenario".into());
            return Vec::new();
        };
        // What LoadScenario gave the story engine: the variables and planet ids it added.
        let pre = load(&self.w_story_pre());
        let after = load(&mid["s"]);
        let mut out: Vec<ScenarioStory> = Vec::new();
        for &(k, val) in after.vars.live() {
            if pre.vars.get(k) != Some(val) {
                out.push(ScenarioStory::Var(k, val));
            }
        }
        for &(k, p) in &after.planets {
            if !pre.planets.contains(&(k, p)) {
                out.push(ScenarioStory::Planet(p as i32, k));
            }
        }
        self.w = mid["w"].clone();
        self.planets = planets_of(&self.w);
        self.fleets = fleets_of(&self.w);
        self.nfl = fleet_total(&self.w);
        self.en = int(&self.w["en"]);
        self.rng.set_state(story::STREAM, self.w["seed5"].as_u64().unwrap() as u32);
        out
    }
    fn info_string(&self, info: i32) -> bool {
        self.infos.iter().position(|&x| x == info).is_some_and(|k| k > 0)
    }
    fn ctf_value(&self, side: i32) -> i32 {
        int(&self.w["ctf"][side as usize])
    }
    fn new_design_raw(&mut self, e: i32, record: &[u8; DESIGN]) -> i32 {
        let what = format!("TervNew({e}, 0)");
        let Some(a) = self.take("tervnew", &what) else { return 0 };
        if ints(&a[1]) != [e, 0] {
            self.bad.push(format!("{what}: exe {}", a[1]));
        }
        let d = int(&a[3]);
        self.made.insert((e, d), *record);
        d
    }
    fn design_count(&self, e: i32) -> i32 {
        assert_eq!(e, self.en);
        self.w["designs"].as_array().map_or(0, |d| d.len() as i32 - 1)
    }
    fn design_key(&self, e: i32, d: i32) -> i32 {
        if let Some(&k) = self.design_keys.get(&(e, d)).or(self.made_keys.get(&(e, d))) {
            return k;
        }
        // st_Rnd part 2c traces: every empire's keys (`des2`).
        if let Some(r) = self.des2(e, d).filter(|r| r.get(2).is_some()) {
            return int(&r[2]);
        }
        if e != self.en {
            self.missing.set(true);
            return 0;
        }
        self.w["designs"].get(d as usize).map_or(0, |x| int(&x[4]))
    }
    fn design_attack(&self, e: i32, d: i32) -> i32 {
        if let Some(r) = self.w["des2"].get(e as usize).and_then(|x| x.get(d as usize)) {
            return int(&r[0]);
        }
        assert_eq!(e, self.en);
        self.w["designs"].get(d as usize).map_or(0, |x| int(&x[3]))
    }
    fn tech_prereqs(&self, t: i32) -> [i32; 3] {
        let b = unhex(self.w["techs"].as_str().unwrap());
        let k = 4 * t as usize;
        [b[k + 1] as i32, b[k + 2] as i32, b[k + 3] as i32]
    }
    fn battle_fleets(&self) -> (i32, i32) {
        // st_Fable traces: `spFlottak` with its count.
        if let Some(spf) = &self.spf {
            return (spf.1[0].0, spf.1[1].0);
        }
        (int(&self.w["spf"][0]), int(&self.w["spf"][1]))
    }
    fn fleet_command(&self, f: i32) -> i32 {
        self.fleets.get(f as usize).map_or(0, |f| f.command)
    }
    fn fleet_target(&self, f: i32) -> f32 {
        self.fleets.get(f as usize).map_or(0.0, |f| f.target)
    }
    fn fleet_state(&self, f: i32) -> i32 {
        self.fleets.get(f as usize).map_or(0, |f| f.state)
    }
    fn planet_production(&self, p: i32) -> (i32, i32) {
        self.planets[p as usize].prod
    }
    fn building_workers(&self, kind: i32) -> i32 {
        self.buildings.workers[kind as usize]
    }
    fn fleet_power(&self, f: i32) -> i32 {
        self.fleets[f as usize].power
    }
    fn building_count(&self, p: i32) -> i32 {
        match self.elem_buildings(p) {
            Some(b) => b.len() as i32,
            None => {
                // st_Elem traces dump only some planets' slots.
                if self.w["bld"].is_array() {
                    self.missing.set(true);
                }
                self.planets[p as usize].buildings
            }
        }
    }
    fn building(&self, p: i32, k: i32) -> (i32, i32, i32) {
        // st_Elem traces: `[kind, +8]` per slot of the human's and empire 11's planets.
        if let Some(b) = self.elem_buildings(p) {
            // A slot the port deleted (`EpuletDelete`: kind 0) reads empty.
            let kind = |s: &Value| if self.deleted.contains(&(p, k)) { 0 } else { int(&s[0]) };
            return b.get(k as usize - 1).map_or((0, 0, 0), |s| (kind(s), s.get(2).map_or(0, int), int(&s[1])));
        }
        match self.w["bld"].get(p.to_string()).and_then(|b| b.get(k as usize)) {
            Some(b) => (int(&b[0]), int(&b[1]), int(&b[2])),
            None => {
                self.missing.set(true);
                (0, 0, 0)
            }
        }
    }
    fn building_hp(&self, kind: i32) -> i32 {
        self.buildings.hp[kind as usize]
    }
    fn building_tech(&self, kind: i32) -> i32 {
        self.buildings.tech[kind as usize]
    }
    fn tank_count(&self) -> i32 {
        self.tanks.len() as i32 - 1
    }
    fn tank(&self, k: i32) -> (i32, i32, i32) {
        self.tanks[k as usize]
    }
    fn design_class(&self, e: i32, d: i32) -> i32 {
        self.design_field(e, d, 5, 0)
    }
    fn design_size(&self, e: i32, d: i32) -> i32 {
        self.design_field(e, d, 6, 1)
    }
    fn design_colony(&self, e: i32, d: i32) -> bool {
        assert_eq!(e, self.en);
        // st_Elem traces have the byte after st_BRnd's five columns, st_Begin's after its own.
        let col = if self.w.get("vrel").is_some() { 5 } else { 7 };
        self.w["designs"].get(d as usize).is_some_and(|x| int(&x[col]) != 0)
    }
    fn trade_offer_count(&self) -> i32 {
        self.w["trade"].as_array().map_or(0, |t| t.len() as i32)
    }
    fn trade_offer(&self, k: i32) -> (i32, i32) {
        let t = &self.w["trade"][k as usize];
        (int(&t[0]), int(&t[1]))
    }
    fn trade_agreement(&self, a: i32, b: i32) -> bool {
        assert_eq!(a, self.en);
        int(&self.w["tra"][b as usize]) != 0
    }
    fn planet_epidemic(&self, p: i32) -> i32 {
        self.planets[p as usize].b8
    }
    fn tech_priority(&self, t: i32) -> i32 {
        int(&self.w["tres"][t as usize][0])
    }
    fn tech_research(&self, t: i32) -> (i32, i32) {
        // st_Rnd traces hold the research costs only (`tcost`).
        if let Some(c) = self.w.get("tcost") {
            return (int(&c[t as usize]), 0);
        }
        let r = &self.w["tres"][t as usize];
        (int(&r[1]), int(&r[2]))
    }
    fn researching(&self, e: i32) -> i32 {
        assert_eq!(e, self.en);
        // st_Fable traces: `research`.
        int(self.w.get("research").unwrap_or(&self.w["res"]))
    }
    fn spy_count(&self, e: i32) -> i32 {
        // st_Rnd 138: the recruits made earlier in the record count.
        let made = self.recruited.iter().map(|r| r.0).max().unwrap_or(0);
        if made > 0 && e == self.en {
            return int(&self.w["spies"][e as usize][0]).max(made);
        }
        // st_Fable traces: the human's spies only (`kemn`, `kem`).
        if let Some(n) = self.w.get("kemn") {
            assert_eq!(e, self.en);
            return int(n);
        }
        int(&self.w["spies"][e as usize][0])
    }
    fn spy_record(&self, e: i32, k: i32) -> SpyRecord {
        if self.w.get("kemn").is_some() {
            assert_eq!(e, self.en);
            let agent = self.agent.as_deref();
            return self.spies.get(k as usize).map_or(SpyRecord::default(), |(state, active, name)| SpyRecord {
                state: *state,
                active: *active,
                agent: agent == Some(name.as_str()),
                ..SpyRecord::default()
            });
        }
        // st_Rnd 138: a recruit made earlier in the record.
        if let Some(&(_, n, _)) = self.recruited.iter().find(|r| e == self.en && r.0 == k) {
            let mut name = [0u8; 16];
            for (i, c) in self.recruits.get(n as usize).map_or("", |x| x.as_str()).chars().take(15).enumerate() {
                name[i] = c as u32 as u8;
            }
            return SpyRecord { state: 1, race: 14, active: 1, name, ..SpyRecord::default() };
        }
        let Some(r) = self.w["spies"][e as usize][1].get(k as usize) else { return SpyRecord::default() };
        let mut name = [0u8; 16];
        // st_Rnd part 2c traces: also `+0x31`, `+0x16` and the name.
        if let Some(n) = r.get(10).and_then(|n| n.as_str()) {
            for (i, c) in n.chars().take(15).enumerate() {
                name[i] = c as u32 as u8;
            }
        }
        SpyRecord {
            state: int(&r[0]),
            race: int(&r[1]),
            target: int(&r[2]),
            mission: int(&r[3]),
            skill: (int(&r[4]), int(&r[5])),
            skill_31: r.get(8).map_or(0, int),
            active: r.get(9).map_or(0, int),
            name,
            ..SpyRecord::default()
        }
    }
    fn planet_morale(&self, p: i32) -> i32 {
        self.planets[p as usize].morale30
    }
    fn spy_rank(&self, e: i32, k: i32) -> i32 {
        int(&self.w["spies"][e as usize][1][k as usize][7])
    }
    fn building_bytes(&self, p: i32, k: i32) -> (i32, i32) {
        // st_Elem part 2b traces: `+0x19` after the damage (no `+0x18`); part 2d: the port's writes in the record.
        if let Some(b) = self.elem_buildings(p) {
            if let Some(&x) = self.building_bytes.get(&(p, k, 0x19)) {
                return (0, x);
            }
            return match b.get(k as usize - 1).and_then(|s| s.get(3)) {
                Some(x) => (0, int(x)),
                None => {
                    self.missing.set(true);
                    (0, 0)
                }
            };
        }
        match self.w["bld"].get(p.to_string()).and_then(|b| b.get(k as usize)) {
            Some(b) => (int(&b[3]), int(&b[4])),
            None => {
                self.missing.set(true);
                (0, 0)
            }
        }
    }
    fn planet_surface(&self, p: i32) -> i32 {
        self.planets[p as usize].surface
    }
    fn planet_richness(&self, p: i32) -> i32 {
        self.planets[p as usize].richness
    }
    fn wormhole_count(&self) -> i32 {
        // st_Fable traces: `wormn`, `worm` (from slot 0: `(A known, A's fleet, B known, B's fleet)`).
        if let Some(n) = self.w.get("wormn") {
            return int(n);
        }
        self.w["whn"].as_i64().map_or(self.w["wh"].as_array().map_or(0, |a| a.len() as i32), |n| n as i32)
    }
    fn wormhole(&self, k: i32) -> [(bool, i32); 2] {
        if let Some(r) = self.w.get("worm") {
            let r = &r[k as usize];
            return [(int(&r[0]) != 0, int(&r[1])), (int(&r[2]) != 0, int(&r[3]))];
        }
        let e = &self.w["wh"][k as usize - 1];
        [(int(&e[0]) & 1 != 0, e.get(2).map_or(0, int)), (int(&e[1]) & 1 != 0, e.get(3).map_or(0, int))]
    }
    fn wormhole_shown(&self) -> bool {
        self.w["wh"].as_array().unwrap().iter().any(|k| int(&k[0]) != 0 && int(&k[1]) != 0)
    }
    fn planet_extra(&self, p: i32) -> bool {
        if let Some(&on) = self.extra187.get(&p) {
            return on;
        }
        match self.extras.get(&p) {
            Some(&on) => on,
            None => self.planets[p as usize].extra.expect("+0x187 not recorded").1 != 0,
        }
    }
    fn tech_time_left(&self, t: i32, e: i32) -> i32 {
        assert_eq!(e, self.en);
        self.techs().iter().find(|r| r[0] == t).map_or_else(|| panic!("tech {t} not recorded"), |r| r[3])
    }
    fn fleet_flags(&self, f: i32) -> i32 {
        self.fleets.get(f as usize).map_or(0, |f| f.flags)
    }
    fn spy_rank_xp(&self, rank: i32) -> i32 {
        // st_Rnd traces: `KemRangXPHatar[0..6]`; st_Fable's: ranks 1 and 2.
        if let Some(x) = self.w.get("rankxp2") {
            return int(&x[rank as usize]);
        }
        int(&self.w["rankxp"][rank as usize - 1])
    }
    fn spy_mission_on(&self, k: i32) -> bool {
        int(&self.w["kmon"][(k - 14) as usize]) != 0
    }
    fn recruit_name(&self, k: i32) -> Vec<u8> {
        self.recruits.get(k as usize).map_or(Vec::new(), |n| n.chars().map(|c| c as u32 as u8).collect())
    }
    fn fleet_named(&self, f: i32, text: i32) -> bool {
        let want = self.stext.get(&text).cloned().unwrap_or_default();
        match self.fleet_names.get(&f) {
            Some(Some(t)) => *t == text,
            Some(None) => self.alapnev == want,
            None => self.w["fnames"].get(f as usize).and_then(|n| n.as_str()) == Some(want.as_str()),
        }
    }
    fn fleet_units(&self, f: i32) -> (i32, i32) {
        // A fleet the record made: as many of design 0x6f6 as the exe made in a row from here (178).
        if self.new_fleets.contains(&f) {
            let mut n = 0;
            let mut k = self.next;
            while let (Some(a), Some(b)) = (self.acts.get(k), self.acts.get(k + 1)) {
                if a[0] == "getterv" && ints(&a[1]) == [10, 0x6f6, 3] && b[0] == "ship" && int(&b[1][0]) == f {
                    n += 1;
                    k += 2;
                } else {
                    break;
                }
            }
            return (n, 0);
        }
        let row = &self.w["fleets"][f as usize];
        (int(&row[5]), row.get(13).map_or(0, int))
    }
    fn buildings_occupied(&self, p: i32) -> i32 {
        // st_Elem part 2a traces: every planet's `HanyEpulet` (exe 0xBA9518).
        match self.w["hanyo"].get(p as usize) {
            Some(n) => int(n),
            None => {
                self.missing.set(true);
                0
            }
        }
    }
    fn battlefield_units(&self, p: i32) -> Vec<(bool, i32)> {
        // st_Elem part 2b traces: the planets' rows with the record's changes.
        if self.w.get("wfus").is_some() {
            return self.battlefield_records(p).iter().map(|u| (u.alive, u.owner)).collect();
        }
        match self.w.get("wfu") {
            // A `null` entry: the tracer couldn't read the rest of the list.
            Some(u) if int(&u[0]) == p && u[1].as_array().unwrap().iter().all(|x| !x.is_null()) => {
                u[1].as_array().unwrap().iter().map(|x| (int(&x[0]) != 0, int(&x[1]))).collect()
            }
            _ => {
                self.missing.set(true);
                Vec::new()
            }
        }
    }
    fn battlefield_records(&self, p: i32) -> Vec<FieldUnit> {
        match self.wf_units.get(&p) {
            Some(u) if u.iter().all(|x| !x.is_null()) => u
                .iter()
                .map(|x| FieldUnit {
                    alive: int(&x[0]) != 0,
                    owner: int(&x[1]),
                    flags: x[2].as_u64().unwrap() as u32,
                    flags2: int(&x[3]) as u8,
                    count: int(&x[4]),
                    design: int(&x[5]),
                })
                .collect(),
            _ => {
                self.missing.set(true);
                Vec::new()
            }
        }
    }
    fn planet_garrison(&self, p: i32) -> i32 {
        match self.w["gar"].get(p as usize) {
            Some(n) => int(n),
            None => {
                self.missing.set(true);
                0
            }
        }
    }
    fn copy_design_new(&mut self, e: i32, from: i32, d: i32) -> i32 {
        let what = format!("TervNew({e}, 0) + TervCopy2({e}, {from}:{d})");
        let Some(a) = self.take("tervnew", &what) else { return 0 };
        if ints(&a[1]) != [e, 0] {
            self.bad.push(format!("{what}: exe {}", a[1]));
        }
        let new = int(&a[3]);
        self.expect("tervcopy2", &[v(e), v(from), v(d), v(new)], &what);
        new
    }
    fn story_text(&self, id: i32) -> Vec<u8> {
        self.stext.get(&id).map_or(Vec::new(), |t| t.chars().map(|c| c as u32 as u8).collect())
    }
    fn design_hull(&self, e: i32, d: i32) -> i32 {
        assert_eq!(e, self.en);
        // st_Elem part 2d rows: +0x48 at 6, +0x36 at 7.
        match self.w["designs"].get(d as usize).and_then(|x| x.get(6)) {
            Some(x) => int(x),
            None => {
                self.missing.set(true);
                0
            }
        }
    }
    fn fleet_fighters(&self, f: i32) -> i32 {
        // `pre`'s `+0x58` less the fighters the record's TankDeletes took (`TankFlottahozHozzaszamol`).
        match self.w["fveh"].get(f as usize) {
            Some(n) => int(n) - self.fighters_lost.get(&f).copied().unwrap_or(0),
            None => {
                self.missing.set(true);
                0
            }
        }
    }
    fn design_44(&self, e: i32, d: i32) -> i32 {
        assert_eq!(e, self.en);
        // st_Elem part 2g rows: +0x44 at 9.
        match self.w["designs"].get(d as usize).and_then(|x| x.get(9)) {
            Some(x) => int(x),
            None => {
                self.missing.set(true);
                0
            }
        }
    }
    fn design_36(&self, e: i32, d: i32) -> i32 {
        assert_eq!(e, self.en);
        match self.w["designs"].get(d as usize).and_then(|x| x.get(7)) {
            Some(x) => int(x),
            None => {
                self.missing.set(true);
                0
            }
        }
    }
    fn date(&self) -> [i32; 3] {
        let d = &self.w["date"];
        [int(&d[0]), int(&d[1]), int(&d[2])]
    }
    fn new_spy(&mut self, e: i32) -> i32 {
        let what = format!("Spy_New({e})");
        let Some(a) = self.take("spynew", &what) else { return 0 };
        if ints(&a[1]) != [e] {
            self.bad.push(format!("{what}: exe {}", a[1]));
        }
        // st_Rnd part 2c traces: `[slot, stream 4 after]` (Spy_New draws its spy on stream 4).
        if let Some(r) = a[3].as_array() {
            self.rng.set_state(stream::SPY, r[1].as_u64().unwrap() as u32);
            return int(&r[0]);
        }
        int(&a[3])
    }
    fn lab_level(&self, e: i32, k: i32) -> i32 {
        assert_eq!(e, self.en);
        int(&self.w["labs"][k as usize])
    }
    fn new_design(&mut self, e: i32, record: &[u8; DESIGN]) -> i32 {
        let what = format!("TervNew({e}, 0)");
        let Some(a) = self.take("tervnew", &what) else { return 0 };
        if ints(&a[1]) != [e, 0] {
            self.bad.push(format!("{what}: exe {}", a[1]));
        }
        let d = int(&a[3]);
        self.expect("tervszamol", &[v(e), v(d), v(0), v(1)], &format!("TervUjraSzamol({e}, {d}, 0, 1)"));
        match self.designs.get(&(e, d)) {
            Some(r) if (0..DESIGN).all(|i| FLAG_CARRIER_UNSET.contains(&i) || r[i] == record[i]) => {}
            other => self.bad.push(format!("{what}: design record {:x?}, exe {other:x?}", &record[..])),
        }
        d
    }
    fn generate_star_names(&mut self) -> (Vec<(i32, i32)>, i32) {
        self.expect("starnames", &[], "Generate_StarNames");
        let post = load(self.post_story.as_ref().unwrap());
        let n = post.vars.get(500).unwrap_or(0);
        let mut names: Vec<(i32, i32)> = post.planets.iter().filter(|e| (500..500 + n).contains(&e.0)).map(|e| (e.1 as i32, e.0)).collect();
        names.sort_by_key(|e| e.1);
        (names, n)
    }
    fn event(&mut self, ev: StoryEvent) {
        let what = format!("{ev:?}");
        match ev {
            StoryEvent::MoveFleet { fleet, kind, mode, x, y } => {
                self.expect("move", &[v(fleet), v(kind), v(mode), v(x.to_bits()), v(y.to_bits())], &what);
                // FlottaMozgat sets the order (+0x40) later reads in the record see.
                if let Some(f) = self.fleets.get_mut(fleet as usize) {
                    f.command = kind;
                }
            }
            StoryEvent::StopFleet(f) => {
                self.expect("move", &[v(f), v(0), v(0), v(0u32), v(0u32)], &what);
                if let Some(q) = self.fleets.get_mut(f as usize) {
                    q.command = 0;
                }
            }
            StoryEvent::DeleteFleet(f) => {
                self.expect("delete", &[v(f)], &what);
                // `FlottaDelete` stops the fleets that followed it (nested `FlottaMozgat(g, 0, 0, 0, 0)`): later reads
                // of their order (`st_FlottaAll`, story 834) see it.
                let k = self.next.wrapping_sub(1);
                for (_, a) in self.nested_moves.iter().filter(|(i, _)| *i == k) {
                    if let Some(q) = self.fleets.get_mut(int(&a[0]) as usize) {
                        q.command = int(&a[1]);
                    }
                }
                // A deleted fleet is free (owner 0xff): later searches in the record skip it.
                if let Some(q) = self.fleets.get_mut(f as usize) {
                    q.owner = 0xff;
                }
                // Its units go with it (`FlottaDelete`'s `TankDelete`s).
                for t in self.tanks.iter_mut().skip(1).filter(|t| t.0 == f) {
                    *t = (t.0, 0x7fff, 0);
                }
            }
            StoryEvent::NewShip { fleet, design } => self.expect("ship", &[v(fleet), v(design)], &what),
            StoryEvent::NewTanks { fleet, design, count } => {
                self.expect("tank", &[v(fleet), v(design), v(count)], &what);
                // st_Elem part 2b: the garrison's new units join the planet's battlefield (808): later reads see
                // `post`'s list.
                if self.wf_units.contains_key(&fleet.wrapping_neg()) {
                    if let Some((u, _)) = self.post_units(fleet.wrapping_neg()) {
                        self.wf_units.insert(fleet.wrapping_neg(), u);
                    }
                }
                // `TankNew` (exe 0x541750): the units join their group, else take the last free record (st_Rnd 2c
                // traces compare `Tankok` after the record).
                if let Some(t) = self.tanks.iter_mut().skip(1).find(|t| t.0 == fleet && t.1 == design) {
                    t.2 += count;
                } else if let Some(k) = (1..self.tanks.len()).rev().find(|&k| self.tanks[k].1 == 0x7fff) {
                    self.tanks[k] = (fleet, design, count);
                } else {
                    self.tanks.push((fleet, design, count));
                }
            }
            StoryEvent::Wormhole { x1, y1, x2, y2 } => self.expect("wormhole", &[v(x1), v(y1), v(x2), v(y2)], &what),
            StoryEvent::DestroyPlanet(p) => {
                self.expect("destroy", &[v(p)], &what);
                // `st_BolygoDestroy` (exe 0x5d47b0): `BolygoElfoglal(p, 0)`, then every empire's view (+0xda..) := -1;
                // later reads in the record (801's landing test) see it.
                if let Some(q) = self.planets.get_mut(p as usize).filter(|_| p > 0) {
                    q.owner = 0;
                    q.vis = -1;
                }
            }
            StoryEvent::Anim(_) | StoryEvent::AnimFile(_) => self.expect("anim", &[], &what),
            StoryEvent::SetPlanetOwner { planet, owner, race } => {
                if self.planet_owner(planet) == 0 {
                    self.expect("colonise", &[v(planet), v(owner), v(race), v(1), v(0), v(0), v(0)], &what)
                } else {
                    self.expect("conquer", &[v(planet), v(owner)], &what);
                    // `BolygoElfoglal` clears the planet's epidemic (`+0xb8`): the story engine's epidemic test
                    // later in the record sees it.
                    if let Some(p) = self.planets.get_mut(planet as usize) {
                        p.b8 = 0;
                    }
                }
                if let Some(p) = self.planets.get_mut(planet as usize) {
                    p.owner = owner;
                }
            }
            StoryEvent::AddMoney { empire, amount } => {
                // The money the scripts read later in the record (`MoneyMennyi`) includes this.
                if empire == self.en {
                    let m = self.w["money"].as_f64().unwrap() + amount;
                    self.w["money"] = m.into();
                }
                if let Some(a) = self.take("money", &what) {
                    if ints(&a[1].as_array().unwrap()[..2].to_vec().into()) != [empire, -1] || a[1][2].as_f64() != Some(amount) {
                        self.bad.push(format!("{what}: exe args {}", a[1]));
                    }
                }
            }
            StoryEvent::ResetDesigns => {
                let mut n = [0, 0];
                while let Some(a) = self.acts.get(self.next) {
                    match a[0].as_str() {
                        Some("autodesign") => n[0] += 1,
                        Some("tervszamol") => n[1] += 1,
                        _ => break,
                    }
                    self.next += 1;
                }
                if n != [18 * 40, 18 * 40] {
                    self.bad.push(format!("{what}: exe {n:?} TervAutoDesign/TervUjraSzamol calls"));
                }
            }
            StoryEvent::Volcano(p) => self.expect("volcano", &[v(p)], &what),
            StoryEvent::DiplomacySpecial { id, empire } => self.expect("diplspecial", &[v(id), v(empire)], &what),
            StoryEvent::DestroyerTutorial => self.expect("tutorial", &[], &what),
            StoryEvent::RecalcFleets => self.expect("recalc", &[], &what),
            StoryEvent::Message { id, empire } => {
                self.message(id, 0, &what);
                // Traces that log `SetParamStr` show the sender's name between the two.
                if self.acts.get(self.next).is_some_and(|a| a[0] == "param") {
                    self.expect("param", &[v(1), v("empire"), v(empire)], &what);
                }
                self.expect("msgrace", &[v(empire)], &what);
            }
            StoryEvent::SaveStoryline { value, .. } => {
                if let Some(a) = self.take("saveregistry", &what) {
                    if int(&a[1][1]) != value {
                        self.bad.push(format!("{what}: exe {}", a[1]));
                    }
                }
            }
            StoryEvent::PlaceBuilding { planet, kind } => {
                self.expect("building", &[v(planet), v(kind), v(1), v(0), v(-1), v(-1)], &what)
            }
            StoryEvent::Mirror { x, y } => {
                let post = planets_of(&self.post_world());
                for (p, (a, b)) in self.planets.iter().zip(&post).enumerate().skip(1) {
                    if b.x != a.x ^ ((x as u32) << 31) || b.y != a.y ^ ((y as u32) << 31) {
                        self.bad.push(format!("{what}: planet {p} {:#x},{:#x} → exe {:#x},{:#x}", a.x, a.y, b.x, b.y));
                        break;
                    }
                }
            }
            StoryEvent::UpgradeShip { ship, design } => {
                // Checked against `post` at the end (a later write wins).
                self.ship_designs.insert(ship, design);
                let n = self.ship_count();
                let mut ships: Vec<(i32, i32)> = (0..=n).map(|k| self.ship(k)).collect();
                ships[ship as usize].1 = design;
                self.ships = Some(ships);
            }
            StoryEvent::SetFleetSpeed { fleet, speed } => {
                let post = fleets_of(&self.post_world());
                if post.get(fleet as usize).map(|f| f.speed) != Some(speed.to_bits()) {
                    self.bad.push(format!("{what}: exe {:?}", post.get(fleet as usize).map(|f| f32::from_bits(f.speed))));
                }
            }
            StoryEvent::NewMessage { id, arg } => self.message(id, arg, &what),
            StoryEvent::MessageRace(e) => self.expect("msgrace", &[v(e)], &what),
            StoryEvent::MessageParam { slot, text } => {
                let (kind, i) = match text {
                    MessageText::Empire(e) => ("empire", e),
                    MessageText::Planet(p) => ("planet", p),
                    MessageText::Player(e) => ("player", e),
                    MessageText::Tech(t) => ("tech", t),
                    MessageText::Fleet(f) => ("fleet", f),
                    MessageText::Participant(k) => ("participant", k),
                    MessageText::Spy { empire, slot } => ("spy", empire * 13 + slot),
                    MessageText::Literal(t) => {
                        if let Some(a) = self.take("param", &what) {
                            if int(&a[1][0]) != slot || a[1][1] != "other" || a[1][2] != t {
                                self.bad.push(format!("{what}: exe {}", a[1]));
                            }
                        }
                        self.events.push(ev);
                        return;
                    }
                    // A text of the string table: any string.
                    // A list of planets, a design's name, a hero's texts (st_Rnd): any string too.
                    // st_Rnd's numbers: the exe's `%.2f` text.
                    MessageText::Number(_) | MessageText::Position(_) => {
                        let want = match text {
                            MessageText::Number(x) => format!("{x:.2}"),
                            MessageText::Position([x, y, z]) => format!("{x:.2}, {y:.2}, {z:.2}"),
                            _ => unreachable!(),
                        };
                        if let Some(a) = self.take("param", &what) {
                            if int(&a[1][0]) != slot || a[1][1] != "other" || a[1][2] != want.as_str() {
                                self.bad.push(format!("{what}: port {want:?}, exe {}", a[1]));
                            }
                        }
                        self.events.push(ev);
                        return;
                    }
                    MessageText::Text(_)
                    | MessageText::Star(_)
                    | MessageText::Planets(_)
                    | MessageText::Design { .. }
                    | MessageText::Hero { .. } => {
                        if let Some(a) = self.take("param", &what) {
                            if int(&a[1][0]) != slot || a[1][1] != "other" {
                                self.bad.push(format!("{what}: exe {}", a[1]));
                            }
                        }
                        self.events.push(ev);
                        return;
                    }
                };
                self.expect("param", &[v(slot), v(kind), v(i)], &what)
            }
            StoryEvent::RemoveMessage(id) => self.expect("rmmessage", &[v(id)], &what),
            StoryEvent::DisablePart(n) => self.expect("disablepart", &[v(n)], &what),
            StoryEvent::RemovePlanetInfo { planet, info } => {
                self.expect("inforemove", &[v(planet), v(info), v(0)], &what);
                if let Some(p) = self.planets.get_mut(planet as usize) {
                    if let Some(k) = p.infos.iter().position(|&i| i == info) {
                        p.infos[k] = 0;
                    }
                }
            }
            StoryEvent::DiscoverEmpire(e) => {
                self.expect("discover", &[v(e), v(1)], &what);
                self.met.push(e);
            }
            StoryEvent::UnhidePlanet(p) => {
                // st_bUnhide(x): the planet itself (`x = −p`) or its story id (st_Begin's base, 11).
                if let Some(a) = self.take("unhide", &what) {
                    if !(int(&a[1][0]) == -p || int(&a[1][0]) > 0) {
                        self.bad.push(format!("{what}: exe args {}", a[1]));
                    }
                }
                // Checked at the end: a later write (st_Rnd 138 hides it from the others) wins.
                self.unhidden.push(p);
                let pl = &mut self.planets[p as usize];
                pl.vis = pl.vis.max(0);
            }
            StoryEvent::SetPlanetValue { planet, value } => {
                self.values.insert(planet, value);
                self.planets[planet as usize].value = value;
            }
            StoryEvent::SetPlanetFriendly { planet, friendly } => {
                let post = planets_of(&self.post_world());
                if post.get(planet as usize).and_then(|q| q.friendly) != Some(friendly as u8) {
                    self.bad.push(format!("{what}: exe {:?}", post.get(planet as usize).and_then(|q| q.friendly)));
                }
            }
            // Undo type 3: `Diplomacy_RemoveSpecial(a & 0xffff, a >> 16)` (logged since st_Elem part 2c).
            StoryEvent::Undo { kind: 3, value } => {
                if self.acts.get(self.next).is_some_and(|x| x[0] == "diplremove") {
                    self.expect("diplremove", &[v(value & 0xffff), v(value >> 16)], &what);
                }
            }
            // Undo type 5: `st_DisableKemKuldetes(a)` (st_Rnd 127's mission).
            StoryEvent::Undo { kind: 5, value } => self.expect("kemdisable", &[v(value)], &what),
            StoryEvent::Undo { kind: 2 | 6, .. } | StoryEvent::EpidemicOver => {
                // BolygoInfoRemove calls of the undo list (one each) / the epidemic's end (not checked).
                // The epidemic's end removes info 0x65 only (a later BolygoInfoRemove of the script's isn't its).
                let all = ev == StoryEvent::EpidemicOver;
                while self.acts.get(self.next).is_some_and(|a| a[0] == "inforemove" && (!all || int(&a[1][1]) == 0x65)) {
                    self.next += 1;
                    if !all {
                        break;
                    }
                }
                self.unchecked.push(what.split([' ', '(']).next().unwrap_or("").to_string());
            }
            StoryEvent::ShowPlanet(p) => {
                if planets_of(&self.post_world()).get(p as usize).is_none_or(|q| q.vis < 1) {
                    self.bad.push(format!("{what}: the exe's view of it is still below 1"));
                }
                let pl = &mut self.planets[p as usize];
                pl.vis = pl.vis.max(1);
            }
            StoryEvent::DeleteDesign { empire, design, fleet, mark } => {
                self.expect("tervdelete", &[v(empire), v(design), v(fleet), v(mark as i32)], &what)
            }
            StoryEvent::RecalcDesign { empire, design } => {
                self.expect("tervszamol", &[v(empire), v(design), v(0), v(1)], &what);
                let made = self.made.get(&(empire, design)).copied();
                match (made, self.designs.get(&(empire, design))) {
                    // The name (+0x00..+0x15) is the exe's text; the unset bytes are its stack's.
                    (Some(r), Some(x)) if (0x16..DESIGN).all(|i| FLAG_CARRIER_UNSET.contains(&i) || x[i] == r[i]) => {}
                    (r, x) => self.bad.push(format!("{what}: design record {r:x?}, exe {x:x?}")),
                }
            }
            StoryEvent::PlanetMorale { planet, delta } if planet as usize >= self.planets.len() => {
                // st_Rnd 174's slip: a story id as the planet: the write lands past the planets (exe 0xC2CA04/08).
                // The story's start record holds no `pre` value (the dump begins once 174 runs).
                match self.stray.map(|s| s.0).or_else(|| self.w["stray"].get(0).map(int)) {
                    Some(was) => self.stray = Some((was.wrapping_add(delta), 6)),
                    None => self.unchecked.push("PlanetMorale past the planets".into()),
                }
            }
            StoryEvent::PlanetMorale { planet, delta } => {
                let cur = match self.morale.get(&planet) {
                    Some(&m) => m,
                    None => self.planets[planet as usize].morale.unwrap_or(0),
                };
                self.morale.insert(planet, cur.wrapping_add(delta));
            }
            StoryEvent::GlobalMorale(delta) => {
                let cur = self.gmoral.unwrap_or_else(|| int(&self.w["gmoral"][0]));
                self.gmoral = Some(cur.wrapping_add(delta));
            }
            StoryEvent::MessageParamInt { slot, value } => self.expect("paramint", &[v(slot), v(value)], &what),
            StoryEvent::GiveTech(t) => self.expect("techstate", &[v(t), v(5), v(-1)], &what),
            StoryEvent::ScrollText { .. }
            | StoryEvent::ScrollSabotage { .. }
            | StoryEvent::ScrollNumbers { .. }
            | StoryEvent::ScrollNumber { .. }
            | StoryEvent::ScrollEmpirePlanet { .. } => self.scroll(&ev, &what),
            StoryEvent::SetFleetId { fleet, id } => {
                // Checked at the end: a later write wins (st_Rnd 184 names its fleet, then clears the id).
                self.fleet_ids.insert(fleet, id);
                self.fleets[fleet as usize].id = id;
            }
            StoryEvent::SendAway(f) => {
                if fleets_of(&self.post_world()).get(f as usize).is_none_or(|q| q.state & 2 == 0) {
                    self.bad.push(format!("{what}: the exe's fleet isn't marked"));
                }
                self.fleets[f as usize].state |= 2;
            }
            StoryEvent::PlaceStoryBuilding { planet, kind, level } => {
                self.expect("building", &[v(planet), v(kind), v(1), v(level), v(-1), v(-1)], &what)
            }
            StoryEvent::AddPlanetInfo { planet, info } => {
                self.expect("infoadd", &[v(planet), v(info), v(1)], &what);
                self.add_info(planet, info);
            }
            StoryEvent::AddPlanetInfoHidden { planet, info } => {
                self.expect("infoadd", &[v(planet), v(info), v(0)], &what);
                self.add_info(planet, info);
            }
            StoryEvent::SetPlanetRichness { planet, value } => {
                self.richness.insert(planet, value);
                self.planets[planet as usize].richness = value;
            }
            StoryEvent::SetTechState { tech, state, empire } => self.expect("techstate", &[v(tech), v(state), v(empire)], &what),
            StoryEvent::SendStorySpecial { id, .. } => self.expect("multisend", &[v(0x66), v(self.en), v(4), v(id)], &what),
            StoryEvent::DeleteShip(k) => {
                self.expect("shipdelete", &[v(k)], &what);
                // `HajoDelete` (exe 0x53e5d0) marks the slot free and drops the units its fleet can't carry.
                let n = self.ship_count();
                if (k as usize) <= n as usize {
                    let mut ships: Vec<(i32, i32)> = (0..=n).map(|j| self.ship(j)).collect();
                    ships[k as usize].1 = 0x7fff;
                    self.ships = Some(ships);
                }
                // A ship the record refitted and then deleted (801 loads and lands it in one check) ends free.
                if let Some(d) = self.ship_designs.get_mut(&k) {
                    *d = 0x7fff;
                }
                self.apply_nested_tank_deletes(self.next.wrapping_sub(1));
            }
            StoryEvent::SetFleetVisible { fleet, on } => {
                if let Some(f) = self.f52.as_mut().and_then(|f| f.get_mut(fleet as usize)) {
                    *f = if on { *f | 0x100 } else { *f & !0x100 };
                }
                // st_Rnd part 2b: checked with the other `+0x52` bits at the end.
                if self.w.get("fext").is_some() {
                    let e = self.fleet_flags.entry(fleet).or_default();
                    let (set, clear) = if on { (0x100, 0) } else { (0, 0x100) };
                    e.0 = (e.0 & !clear) | set;
                    e.1 = (e.1 & !set) | clear;
                }
            }
            StoryEvent::RenamePlanet { planet, name } => {
                let post = self.post_world();
                if post["names"].get(planet as usize).and_then(|n| n.as_str()) != Some(name) {
                    self.bad.push(format!("{what}: exe {:?}", post["names"].get(planet as usize)));
                }
            }
            StoryEvent::Notice(n) => {
                let en = self.en;
                let ok: Box<dyn Fn(&str) -> bool> = match n {
                    Notice::Winner { empire, amazing, .. } if empire == en => {
                        Box::new(move |t| t == if amazing { "AMAZING! YOU ARE THE WINNER!" } else { "YOU ARE THE WINNER!" })
                    }
                    Notice::Winner { .. } => Box::new(|t| t.ends_with(" is the winner!")),
                    Notice::CannotColonise { .. } => Box::new(|t| t.starts_with("You cannot colonize ")),
                    Notice::FlagTaken { by, .. } if by == en => Box::new(|t| t.starts_with("You have the flag of ")),
                    Notice::FlagTaken { .. } => Box::new(|t| t.ends_with(" has your flag!")),
                    Notice::Touchdown { by } if by == en => Box::new(|t| t == "TOUCHDOWN!!!"),
                    Notice::Touchdown { .. } => Box::new(|t| t.ends_with(" has captured your flag!")),
                    Notice::FlagLost { by } if by == en => Box::new(|t| t == "You have lost the flag!"),
                    Notice::FlagLost { .. } => Box::new(|t| t == "Your flag has been returned!"),
                    Notice::StoryText(id) => {
                        // The act's text and the setup's decode the accented letters differently: ASCII compared.
                        let ascii = |t: &str| t.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect::<String>();
                        let want = ascii(&self.stext.get(&id).cloned().unwrap_or_default());
                        Box::new(move |t| ascii(t) == want)
                    }
                };
                if let Some(a) = self.take("text", &what) {
                    if !a[1][0].as_str().is_some_and(&*ok) {
                        self.bad.push(format!("{what}: exe {}", a[1]));
                    }
                }
            }
            StoryEvent::FleetFlags { fleet, set, clear } => {
                if let Some(f) = self.fleets.get_mut(fleet as usize) {
                    f.flags = (f.flags | set as i32) & !(clear as i32);
                }
                let e = self.fleet_flags.entry(fleet).or_default();
                e.0 = (e.0 & !(clear as i32)) | set as i32;
                e.1 = (e.1 & !(set as i32)) | clear as i32;
            }
            StoryEvent::RadarNotice(p) => {
                if planets_of(&self.post_world()).get(p as usize).and_then(|q| q.notice) != Some(1) {
                    self.bad.push(format!("{what}: the exe's +0xbb isn't 1"));
                }
            }
            StoryEvent::SetPlanetView { planet, view } => {
                let post = planets_of(&self.post_world());
                let got = post.get(planet as usize).map(|q| q.views.get(self.en as usize).copied());
                if got != Some(Some(view as i16)) {
                    self.bad.push(format!("{what}: exe {got:?}"));
                }
            }
            StoryEvent::SetEpidemic { planet, value } => {
                self.epidemic.insert(planet, value);
                self.planets[planet as usize].b8 = value;
            }
            StoryEvent::VirusCheck(p) => {
                self.expect("virus", &[v(p)], &what);
                // `Virus_CheckAdd` adds info 100 (seen by later `BolygoInfoIs` in the record).
                self.add_info(p, 100);
            }
            StoryEvent::RelationChange { b, delta, .. } if (0..19).contains(&b) => {
                *self.relations.entry(b).or_default() += delta;
            }
            StoryEvent::TradeAgreement { b, .. } if (0..19).contains(&b) => {
                if int(&self.post_world()["tra"][b as usize]) == 0 {
                    self.bad.push(format!("{what}: no trade agreement in the exe"));
                }
            }
            StoryEvent::DeleteTanks { place, design, count } => {
                self.expect("tankdelete", &[v(place), v(design), v(count)], &what);
                // TankDeleteGrWar (exe 0x541510): the first record of the place and design.
                if let Some(k) = (1..self.tanks.len()).find(|&k| self.tanks[k].0 == place && self.tanks[k].1 == design) {
                    // st_Elem part 2g rows: `+0x16` at 10 (a fleet's fighters drop with a non-ground design's).
                    let kind = self.w["designs"].get(design as usize).and_then(|x| x.get(10)).map(int);
                    if place > 0 && kind.is_some_and(|k| k != 2) {
                        *self.fighters_lost.entry(place).or_default() += count.min(self.tanks[k].2);
                    }
                    let t = &mut self.tanks[k];
                    t.2 -= count.min(t.2);
                    if t.2 < 1 {
                        t.1 = 0x7fff;
                    }
                }
            }
            StoryEvent::DesignPriceDiv { design, div, .. } => {
                let cur = *self.prices.entry(design).or_insert_with(|| self.w["designs"].get(design as usize).map_or(0, |x| int(&x[1])));
                self.prices.insert(design, cur / div);
            }
            StoryEvent::DesignForSale { design, .. } => {
                let cur = *self.prices.entry(design).or_insert_with(|| self.w["designs"].get(design as usize).map_or(0, |x| int(&x[1])));
                self.prices.insert(design, cur / 3);
                self.offers_added += 1;
                let post = self.post_world();
                if post["designs"].get(design as usize).map(|x| int(&x[8])) != Some(3) {
                    self.bad.push(format!("{what}: the exe's design +0x6c isn't 3"));
                }
            }
            StoryEvent::TechCheaper(t) => {
                let (pre, post) = (self.w["tres"][t as usize].clone(), self.post_world()["tres"][t as usize].clone());
                let want: Vec<i32> = (0..4).map(|i| if i == 0 { int(&pre[0]) } else { int(&pre[i]) / 3 }).collect();
                if ints(&post) != want {
                    self.bad.push(format!("{what}: port {want:?} exe {post}"));
                }
            }
            StoryEvent::DamageBuilding { planet, slot, damage } => {
                let post = self.post_world();
                // st_Elem traces: `[p, [[kind, +8, +4]…]]` from slot 1.
                let got = match post["bld"].as_array() {
                    Some(rows) => rows.iter().find(|r| int(&r[0]) == planet).and_then(|r| r[1].get(slot as usize - 1)).and_then(|b| b.get(2)).map(int),
                    None => post["bld"].get(planet.to_string()).and_then(|b| b.get(slot as usize)).map(|b| int(&b[1])),
                };
                if got != Some(damage) {
                    self.bad.push(format!("{what}: exe {got:?}"));
                }
            }
            StoryEvent::SpyTarget { slot, empire } => {
                let post = self.post_world();
                let got = int(&post["spies"][self.en as usize][1][slot as usize][2]);
                if got != empire {
                    self.bad.push(format!("{what}: exe {got}"));
                }
            }
            StoryEvent::MakeNeutral { a, b } => {
                // st_Elem 824 inlines it: `BirodalomBeketKot` and the levels.
                let name = if self.acts.get(self.next).is_some_and(|x| x[0] == "peace") { "peace" } else { "neutral" };
                self.expect(name, &[v(a), v(b)], &what);
                // Both levels 3: the later reads in the record see it.
                if (0..19).contains(&b) {
                    self.w["visz"][b as usize] = 3.into();
                    let post = self.post_world();
                    if int(&post["visz"][b as usize]) != 3 || int(&post["visz2"][b as usize]) != 3 {
                        self.bad.push(format!("{what}: exe levels {} / {}", post["visz"][b as usize], post["visz2"][b as usize]));
                    }
                }
            }
            StoryEvent::MakeNonAggression { a, b } => {
                // st_Elem 858: `st_SetNonagressionViszony` (its `BirodalomBeketKot` inside), both levels 4.
                self.expect("nonaggr", &[v(a), v(b)], &what);
                if (0..19).contains(&b) {
                    self.w["visz"][b as usize] = 4.into();
                    let post = self.post_world();
                    if int(&post["visz"][b as usize]) != 4 || int(&post["visz2"][b as usize]) != 4 {
                        self.bad.push(format!("{what}: exe levels {} / {}", post["visz"][b as usize], post["visz2"][b as usize]));
                    }
                }
            }
            StoryEvent::RemoveDiplomacySpecial { id, empire } => self.expect("diplremove", &[v(id), v(empire)], &what),
            StoryEvent::SpyMission { id, on } => self.expect(if on { "kemenable" } else { "kemdisable" }, &[v(id)], &what),
            StoryEvent::SpyState { slot, state } => {
                self.spy_states.insert(slot, state);
            }
            StoryEvent::SpyExperience { slot, add } => {
                *self.spy_xp.entry(slot).or_default() += add;
            }
            StoryEvent::TechSeen { tech, owner } => self.techs_seen.push((tech, owner)),
            StoryEvent::Colonise { planet, owner } => {
                self.expect("colonise", &[v(planet), v(owner), v(owner), v(0), v(0), v(0), v(0)], &what);
                if let Some(p) = self.planets.get_mut(planet as usize) {
                    p.owner = owner;
                }
            }
            StoryEvent::BuildingByte { planet, slot, offset, value } => {
                self.building_bytes.insert((planet, slot, offset), value);
            }
            StoryEvent::JoinBattle(f) => {
                // `st_AddFleetToSpWar`: entry `n + 1` (n < 4); entries 1 and 2 are the battle's two fleets, which later
                // stories of the check read. st_Fable's traces dump the whole list (`spfl`).
                if let Some((n, e)) = self.spf.as_mut() {
                    if *n < 4 {
                        *n += 1;
                        e[*n as usize - 1] = (f, 1);
                    }
                } else {
                    let n = self.joined_n.unwrap_or_else(|| self.w.get("spw").map_or(0, |x| int(&x[0])));
                    if n < 4 {
                        self.joined_n = Some(n + 1);
                        if n < 2 {
                            self.w["spf"][n as usize] = f.into();
                        }
                    }
                }
                self.joined.push(f);
            }
            StoryEvent::FleetState { fleet, set, clear } => {
                let (set, clear) = (set as i32, clear as i32);
                let e = self.fleet_state.entry(fleet).or_default();
                e.0 = (e.0 & !clear) | set;
                e.1 = (e.1 & !set) | clear;
                if let Some(f) = self.fleets.get_mut(fleet as usize) {
                    f.state = (f.state | set) & !clear;
                }
            }
            StoryEvent::DeleteBuilding { planet, slot } => {
                self.expect("bdelete", &[v(planet), v(slot)], &what);
                self.deleted.push((planet, slot));
                // st_Elem part 2b: `EpuletDelete` takes a defence building's turrets off the battlefield (808's start):
                // later reads see `post`'s list.
                if self.wf_units.contains_key(&planet) {
                    if let Some((u, _)) = self.post_units(planet) {
                        self.wf_units.insert(planet, u);
                    }
                }
            }
            StoryEvent::TechPrereq { tech, slot, value } => {
                self.prereqs.insert((tech, slot), value);
            }
            StoryEvent::TechResearchTime { .. } => {}
            StoryEvent::Earthquake(p) => {
                if int(&self.post_world()["quake"]) != p {
                    self.bad.push(format!("{what}: exe Colony_foldrenges {}", self.post_world()["quake"]));
                }
            }
            StoryEvent::SpyCaught { spy, empire, enemy } => {
                self.expect("spyreport", &[v(3), v(spy), v(0), v(0), v(0), v(0), v(empire), v(enemy), v(0)], &what)
            }
            StoryEvent::TradePrices { .. } if self.w["trade"].is_null() => self.unchecked.push("TradePrices".into()),
            StoryEvent::TradePrices { cheap } => {
                let f = if cheap { 0.8 } else { 1.25 };
                let (pre, post) = (ints(&self.w["trade"]), ints(&self.post_world()["trade"]));
                let want: Vec<i32> = pre.iter().map(|&x| (x as f64 * f) as i32).collect();
                if want != post {
                    self.bad.push(format!("{what}: port {want:?} exe {post:?}"));
                }
            }
            StoryEvent::RazePlanet(p) => {
                for k in 1..=self.building_count(p) {
                    if self.building(p, k).0 != 0 {
                        self.expect("bdelete", &[v(p), v(k)], &what);
                    }
                }
                self.expect("conquer", &[v(p), v(0)], &what);
                self.planets[p as usize].owner = 0;
            }
            StoryEvent::SetPlanetXY { planet, x, y } => {
                let post = planets_of(&self.post_world());
                if post.get(planet as usize).map(|q| (q.x, q.y)) != Some((x.to_bits(), y.to_bits())) {
                    self.bad.push(format!("{what}: exe {:?}", post.get(planet as usize).map(|q| (f32::from_bits(q.x), f32::from_bits(q.y)))));
                }
                let pl = &mut self.planets[planet as usize];
                (pl.x, pl.y) = (x.to_bits(), y.to_bits());
            }
            StoryEvent::PlanetRadarMsg { planet, on } => {
                let post = planets_of(&self.post_world());
                if post.get(planet as usize).and_then(|q| q.bb) != Some(on as u8) {
                    self.bad.push(format!("{what}: exe {:?}", post.get(planet as usize).and_then(|q| q.bb)));
                }
            }
            StoryEvent::PlanetExtra { planet, on } => {
                self.extras.insert(planet, on);
            }
            StoryEvent::TechTimes { tech, empire, halve, price } => {
                assert_eq!(empire, self.en);
                let mut tt = self.techs();
                if let Some(r) = tt.iter_mut().find(|r| r[0] == tech) {
                    let f = |x: i32| if halve { x / 2 } else { x.wrapping_shl(1) };
                    if price {
                        r[1] = f(r[1]);
                    }
                    r[2] = f(r[2]);
                    r[3] = f(r[3]);
                } else {
                    self.bad.push(format!("{what}: tech not recorded"));
                }
                self.tt = Some(tt);
            }
            StoryEvent::MakeAllies { a, b } if self.w.get("vlev").is_some() => {
                // st_Elem part 2f traces: every pair's levels dumped, checked at the end (836's wars follow).
                self.expect("peace", &[v(a), v(b)], &what);
                self.allies.push((a, b));
                if a == self.en {
                    self.w["visz"][b as usize] = v(5);
                }
            }
            StoryEvent::MakeAllies { a, b } => {
                self.expect("peace", &[v(a), v(b)], &what);
                let post = ints(&self.post_world()["vrel"]);
                if post[..2] != [5, 5] {
                    self.bad.push(format!("{what}: exe levels {post:?}"));
                }
                self.w["visz"][b as usize] = v(5);
            }
            StoryEvent::DeclareWar { a, b } if a != self.en && self.w.get("vlev").is_some() => {
                // st_Elem part 2g traces: 862's war of empire 1 on 5, the levels checked at the end.
                self.pair_wars.push((a, b));
            }
            StoryEvent::DeclareWar { a, b } if self.w.get("visz2").is_some() && self.w.get("rel").is_some() => {
                // st_Elem part 2a traces: any empire; `Viszony[b][a].+4 −= 10000` with the levels 1 both ways, checked
                // at the end.
                assert_eq!(a, self.en);
                if (0..19).contains(&b) {
                    *self.relations.entry(b).or_default() -= 10000;
                    self.wars.push(b);
                    self.w["visz"][b as usize] = v(1);
                }
            }
            StoryEvent::DeclareWar { a, b } => {
                assert_eq!((a, b), (self.en, 5));
                let (pre, post) = (ints(&self.w["vrel"]), ints(&self.post_world()["vrel"]));
                let want = [1, 1, (pre[2] as i16).wrapping_sub(10000) as i32];
                if post != want {
                    self.bad.push(format!("{what}: port {want:?} exe {post:?}"));
                }
                self.w["visz"][b as usize] = v(1);
            }
            StoryEvent::WithdrawMessage { id, arg } => self.expect("withdraw", &[v(id), v(arg)], &what),
            StoryEvent::SetPlanetRadar { planet, level } => {
                if let Some(p) = self.planets.get_mut(planet as usize) {
                    p.radar = level;
                }
                // st_Elem part 2e traces (`wf334` dumped): 834's `+0x6e := 5` (nothing else in the record writes it).
                if self.w.get("wf334").is_some() {
                    let got = planets_of(&self.post_world()).get(planet as usize).map(|q| q.radar);
                    if got != Some(level) {
                        self.bad.push(format!("{what}: exe {got:?}"));
                    }
                }
            }
            StoryEvent::ShowWormhole(k) => {
                let post = self.post_world();
                if post["worm"].get(k as usize).is_none_or(|r| int(&r[0]) == 0 || int(&r[2]) == 0) {
                    self.bad.push(format!("{what}: exe {:?}", post["worm"].get(k as usize)));
                }
            }
            StoryEvent::SetPlanetViewOf { planet, empire, view } => {
                self.views_of.insert((planet, empire), view);
            }
            StoryEvent::NamePlanet { planet, text } => {
                self.planet_names.insert(planet, text);
            }
            StoryEvent::StoryRecruit { slot, name, xp } => {
                self.recruited.push((slot, name, xp));
            }
            StoryEvent::NameFleet { fleet, text } => {
                self.fleet_names.insert(fleet, text);
            }
            StoryEvent::SetFleetPos { fleet, x, y } => {
                self.fleet_pos.insert(fleet, (x, y));
                if let Some(f) = self.fleets.get_mut(fleet as usize) {
                    (f.x, f.y) = (x, y);
                }
            }
            StoryEvent::SetFleetOwner { fleet, owner } => {
                self.fleet_owners.insert(fleet, owner);
                if let Some(f) = self.fleets.get_mut(fleet as usize) {
                    f.owner = owner;
                }
            }
            StoryEvent::SetTankDesign { tank, design } => {
                self.tank_designs.insert(tank, design);
                if let Some(t) = self.tanks.get_mut(tank as usize) {
                    t.1 = design;
                }
            }
            StoryEvent::CopyDesign { empire, from, to } => self.expect("tervcopy", &[v(empire), v(from), v(to)], &what),
            StoryEvent::SetDesignKey { empire, design, key } => {
                self.design_keys.insert((empire, design), key);
            }
            StoryEvent::DesignFlags { design, b18, b19, .. } => {
                self.design_flags.insert(design, (b18, b19));
            }
            StoryEvent::NameDesign { design, text, .. } => {
                self.design_names.insert(design, text);
            }
            StoryEvent::StorySpy { slot, xp } => {
                self.story_spy = Some((slot, xp));
                let name = self.agent.clone().unwrap_or_default();
                if self.spies.len() <= slot as usize {
                    self.spies.resize(slot as usize + 1, (0, 0, String::new()));
                }
                self.spies[slot as usize] = (1, 1, name);
            }
            StoryEvent::SpyReport { kind, spy } => {
                let en = self.en;
                self.expect("kemjelentes", &[v(kind), v(spy), v(0), v(0), v(0), v(0), v(en), v(0)], &what)
            }
            StoryEvent::KillSpy { empire, slot } => {
                assert_eq!(empire, self.en);
                if let Some(k) = self.spies.get_mut(slot as usize) {
                    k.0 = 0;
                }
            }
            StoryEvent::SelfDestruct { planet, value } => {
                self.self_destruct.insert(planet, value);
            }
            StoryEvent::MadUnit { planet, unit } => self.expect_unit("madness", planet, unit, &[v(1)], &what),
            StoryEvent::CalmUnit { planet, unit } => {
                self.expect_unit("madness", planet, unit, &[v(0)], &what);
                self.expect_unit("notarget", planet, unit, &[], &what);
            }
            StoryEvent::RiotUnit { planet, unit, fleet } => {
                let row = self.wf_units.get(&planet).and_then(|u| u.get(unit as usize)).cloned().unwrap_or(Value::Null);
                let (count, design) = (row.get(4).map_or(0, int), row.get(5).map_or(0, int));
                self.expect("tankdel0", &[v(planet.wrapping_neg()), v(design), v(count)], &what);
                self.expect_unit("setside", planet, unit, &[v(10)], &what);
                // The core `TankDelete`: the first garrison record of the design.
                if let Some(k) = (1..self.tanks.len()).find(|&k| self.tanks[k].0 == planet.wrapping_neg() && self.tanks[k].1 == design) {
                    let t = &mut self.tanks[k];
                    t.2 -= count.min(t.2);
                    if t.2 < 1 {
                        t.1 = 0x7fff;
                    }
                }
                if let Some(x) = self.wf_units.get_mut(&planet).and_then(|u| u.get_mut(unit as usize)) {
                    x[1] = v(10);
                }
                self.rioted.push((planet, unit, fleet, design));
            }
            StoryEvent::UnitDesign { planet, unit, design } => {
                if let Some(x) = self.wf_units.get_mut(&planet).and_then(|u| u.get_mut(unit as usize)) {
                    x[5] = v(design);
                }
                if let Some(r) = self.rioted.iter_mut().rev().find(|r| (r.0, r.1) == (planet, unit)) {
                    r.3 = design;
                }
            }
            StoryEvent::DesignByte { design, offset, value, .. } => {
                self.design_bytes.insert((design, offset), value);
            }
            StoryEvent::ColoniseAs { planet, owner, race } => {
                self.expect("colonise", &[v(planet), v(owner), v(race), v(0), v(0), v(0), v(0)], &what);
                if let Some(p) = self.planets.get_mut(planet as usize) {
                    p.owner = owner;
                }
            }
            StoryEvent::PlaceBuildingUnbuilt { planet, kind } => {
                self.expect("building", &[v(planet), v(kind), v(0), v(0), v(-1), v(-1)], &what)
            }
            StoryEvent::MergeFleets { into, from } => {
                self.expect("osszevon", &[v(into), v(from)], &what);
                // `FlottaOsszevon` (fleet-movement.md §4.6): `from` is freed, `into` takes its story id when it has none
                // and its vehicles (`+0x5c`); its ships go over.
                let src = self.fleets.get(from as usize).cloned();
                if let Some(src) = src {
                    if let Some(g) = self.fleets.get_mut(into as usize) {
                        if g.id == 0 {
                            g.id = src.id;
                        }
                        g.w5c = g.w5c.wrapping_add(src.w5c);
                    }
                    self.fleets[from as usize].owner = 0xff;
                    let n = self.ship_count();
                    let mut ships: Vec<(i32, i32)> = (0..=n).map(|k| self.ship(k)).collect();
                    for s in ships.iter_mut().skip(1) {
                        if s.0 == from {
                            s.0 = into;
                        }
                    }
                    self.ships = Some(ships);
                }
            }
            StoryEvent::PlanetExtraFlag { planet, on } => {
                self.extra187.insert(planet, on);
            }
            StoryEvent::HoldGroundWar { planet } => {
                self.expect("pause", &[v(1), v(0)], &what);
                self.held = Some(planet);
            }
            StoryEvent::SpyOrder { spy, mission } => {
                let en = self.en;
                self.expect("multisend", &[v(0x49), v(en), v(4), v(spy), v(mission)], &what)
            }
            StoryEvent::ColonyDiscount { halve } => self.colony_discount(halve, &what),
            // Undo type 11 (840's discount ends with the story): the prices doubled back.
            StoryEvent::Undo { kind: 11, .. } => self.colony_discount(false, &what),
            StoryEvent::SelfDestructTimer { planet, value } => {
                // st_Elem part 2e: the dump has planet 8140's `WF+0x334` (the record's last write wins).
                let post = self.post_world();
                if post["wf334"].get(0).map(int) == Some(planet) && post["wf334"].get(1).map(int) != Some(value) {
                    self.bad.push(format!("{what}: exe {}", post["wf334"]));
                }
            }
            StoryEvent::Won
            | StoryEvent::CampaignOver
            | StoryEvent::EmpireDies(_)
            | StoryEvent::GameInfo { .. }
            | StoryEvent::InfoLine { .. }
            | StoryEvent::RemoveGameInfo(_) => {}
            _ => self.unchecked.push(what.split([' ', '(']).next().unwrap_or("").to_string()),
        }
        self.events.push(ev);
    }
}

impl W<'_> {
    /// The writes checked against `post` once the record ran: planets' `+0x28` and morale, ships' designs,
    /// the global morale.
    fn finish(&mut self) {
        let post = self.post_world();
        let planets = planets_of(&post);
        for (&p, &val) in &self.values {
            if planets.get(p as usize).map(|q| q.value) != Some(val) {
                self.bad.push(format!("planet {p} +0x28: port {val} exe {:?}", planets.get(p as usize).map(|q| q.value)));
            }
        }
        for (&p, &m) in &self.morale {
            if planets.get(p as usize).and_then(|q| q.morale) != Some(m) {
                self.bad.push(format!("planet {p} morale: port {m} exe {:?}", planets.get(p as usize).and_then(|q| q.morale)));
            }
        }
        for (&k, &d) in &self.ship_designs {
            match post["ships"].get(k as usize) {
                Some(s) if int(&s[1]) == d => {}
                other => self.bad.push(format!("ship {k} design: port {d} exe {other:?}")),
            }
        }
        let pre_fleets = fleets_of(&self.w);
        let post_fleets = fleets_of(&post);
        // The bits the stories touch (0x40 radar contact reported, 0x100 visible, 0x200 friendly, 0x800 named), from
        // `pre` with the port's writes; a fleet the record made: the bits the port wrote.
        for (&f, &(set, clear)) in &self.fleet_flags {
            // A fleet the record made starts from `FlottaNew`'s bits (0x800 for a named one), not `pre`'s: only the
            // bits the port wrote count.
            let mask = if (f as usize) < pre_fleets.len() && !self.new_fleets.contains(&f) { 0x4b40 } else { set | clear };
            let was = pre_fleets.get(f as usize).map_or(0, |q| q.flags);
            let want = ((was & !clear) | set) & mask;
            let got = post_fleets.get(f as usize).map_or(0, |q| q.flags) & mask;
            if got != want {
                self.bad.push(format!("fleet {f} +0x52: port {want:#x} exe {got:#x}"));
            }
        }
        for (&p, &val) in &self.richness {
            if planets.get(p as usize).map(|q| q.richness) != Some(val) {
                self.bad.push(format!("planet {p} +0x24: port {val} exe {:?}", planets.get(p as usize).map(|q| q.richness)));
            }
        }
        for (&p, &b8) in &self.epidemic {
            if planets.get(p as usize).map(|q| q.b8) != Some(b8) {
                self.bad.push(format!("planet {p} +0xb8: port {b8} exe {:?}", planets.get(p as usize).map(|q| q.b8)));
            }
        }
        for (&e, &d) in &self.relations {
            let (was, got) = (int(&self.w["rel"][e as usize]), int(&post["rel"][e as usize]));
            if (was + d) as i16 as i32 != got {
                self.bad.push(format!("relation of {e}: port {was} + {d}, exe {got}"));
            }
        }
        if self.offers_added != 0 {
            let n = |w: &Value| w["trade"].as_array().map_or(0, |t| t.len() as i32);
            if n(&post) != n(&self.w) + self.offers_added {
                self.bad.push(format!("trade offers: port +{} exe {} → {}", self.offers_added, n(&self.w), n(&post)));
            }
        }
        for (&d, &price) in &self.prices {
            // A design st_GetTerv made in this record: its price came from the story design data.
            if self.w["designs"].as_array().is_some_and(|x| d as usize >= x.len()) {
                continue;
            }
            if post["designs"].get(d as usize).map(|x| int(&x[1])) != Some(price) {
                self.bad.push(format!("design {d} price: port {price} exe {:?}", post["designs"].get(d as usize).map(|x| int(&x[1]))));
            }
        }
        // `TankDelete`s the game made inside the stories' calls (`HajoDelete` drops the units its fleet can no longer
        // carry): applied as the exe's nested calls say.
        self.apply_nested_tank_deletes(usize::MAX);
        if let Some(t) = post["tanks"].as_array() {
            let exe: Vec<(i32, i32, i32)> = t.iter().map(|x| (int(&x[0]), int(&x[1]), int(&x[2]))).collect();
            if exe != self.tanks && self.events.iter().any(|e| matches!(e, StoryEvent::DeleteTanks { .. })) {
                self.bad.push(format!("Tankok: port {:?} exe {exe:?}", self.tanks));
            }
        }
        for (&p, &on) in &self.extras {
            // Traces before st_Elem don't record the bytes; part 2d's 806 clears `+0x187` alone afterwards.
            let b = self.extra187.get(&p).copied().unwrap_or(on);
            if planets.get(p as usize).is_some_and(|q| q.extra.is_some()) && planets[p as usize].extra != Some((on as u8, b as u8)) {
                self.bad.push(format!("planet {p} +0x186/+0x187: port {on}/{b} exe {:?}", planets.get(p as usize).and_then(|q| q.extra)));
            }
        }
        for (&p, &on) in &self.extra187 {
            if self.extras.contains_key(&p) {
                continue;
            }
            let got = planets.get(p as usize).and_then(|q| q.extra).map(|x| x.1);
            if got != Some(on as u8) {
                self.bad.push(format!("planet {p} +0x187: port {on} exe {got:?}"));
            }
        }
        if let Some(tt) = &self.tt {
            let exe = tech_rows(&post);
            if *tt != exe {
                self.bad.push(format!("techs: port {tt:?} exe {exe:?}"));
            }
        }
        let en = self.en as usize;
        for (&k, &state) in &self.spy_states {
            let got = int(&post["spies"][en][1][k as usize][0]);
            if got != state {
                self.bad.push(format!("spy {k} state: port {state} exe {got}"));
            }
        }
        for (&k, &add) in &self.spy_xp {
            let (was, got) = (int(&self.w["spies"][en][1][k as usize][6]), int(&post["spies"][en][1][k as usize][6]));
            if was.wrapping_add(add) != got {
                self.bad.push(format!("spy {k} experience: port {was} + {add}, exe {got}"));
            }
        }
        if !self.techs_seen.is_empty() {
            let seen = unhex(post["tseen"].as_str().unwrap_or(""));
            for &(t, e) in &self.techs_seen {
                if seen.get(9 * t as usize + e as usize) != Some(&1) {
                    self.bad.push(format!("tech {t} seen of {e}: exe {:?}", seen.get(9 * t as usize + e as usize)));
                }
            }
        }
        for (&(p, k, off), &val) in &self.building_bytes {
            let i = if off == 0x18 { 3 } else { 4 };
            // st_Elem traces: `[p, [[kind, +8, +4, +0x19]…]]` from slot 1.
            let got = match post["bld"].as_array() {
                Some(rows) if off == 0x19 => rows.iter().find(|r| int(&r[0]) == p).and_then(|r| r[1].get(k as usize - 1)).and_then(|b| b.get(3)).map(int),
                _ => post["bld"].get(p.to_string()).and_then(|b| b.get(k as usize)).map(|b| int(&b[i])),
            };
            if got != Some(val) {
                self.bad.push(format!("building {p}/{k} +{off:#x}: port {val} exe {got:?}"));
            }
        }
        if !self.joined.is_empty() {
            // st_AddFleetToSpWar: `spFlottak[1..=n]` (n < 5) gets the fleets in order after the two of the battle.
            let (n0, n1) = (int(&self.w["spw"][0]), int(&post["spw"][0]));
            let want: Vec<i32> = self.joined.iter().copied().take((4 - n0).max(0) as usize).collect();
            let got: Vec<i32> = (n0 + 1..=n1).map(|k| int(&post["spw"][1][k as usize][0])).collect();
            if got != want {
                self.bad.push(format!("spFlottak joined: port {want:?} exe {got:?} (count {n0} → {n1})"));
            }
        }
        for (&f, &(set, clear)) in &self.fleet_state {
            let was = pre_fleets.get(f as usize).map_or(0, |q| q.state);
            let want = ((was & !clear) | set) & (set | clear);
            let got = post_fleets.get(f as usize).map_or(0, |q| q.state) & (set | clear);
            if got != want {
                self.bad.push(format!("fleet {f} +0x18: port {want:#x} exe {got:#x}"));
            }
        }
        if !self.prereqs.is_empty() {
            let techs = unhex(post["techs"].as_str().unwrap());
            for (&(t, k), &val) in &self.prereqs {
                let got = techs[4 * t as usize + k as usize] as i32;
                if got != val {
                    self.bad.push(format!("tech {t} prerequisite {k}: port {val:#x} exe {got:#x}"));
                }
            }
        }
        if let Some(g) = self.gmoral {
            if int(&post["gmoral"][0]) != g || int(&post["gmoral"][1]) != 6 {
                self.bad.push(format!("global morale: port {g} exe {}", post["gmoral"]));
            }
        }
        for (&p, &val) in &self.self_destruct {
            let got = post["wfsd"].get(p as usize).map(int);
            if got != Some(val) {
                self.bad.push(format!("planet {p} WF+0x334: port {val} exe {got:?}"));
            }
        }
        for &b in &self.wars {
            let (l1, l2) = (int(&post["visz"][b as usize]), int(&post["visz2"][b as usize]));
            if (l1, l2) != (1, 1) {
                self.bad.push(format!("war with {b}: exe levels {l1} / {l2}"));
            }
        }
        if let Some(lev) = post["vlev"].as_str().map(unhex) {
            let en = self.en;
            for &(a, b) in &self.pair_wars {
                // 862's alliance with the human follows the war: with the human one of the two it wins.
                if self.allies.iter().any(|&p| p == (a, b) || p == (b, a)) {
                    continue;
                }
                for (x, y) in [(a, b), (b, a)] {
                    let got = lev.get((x * 9 + y) as usize).copied();
                    if got != Some(1) {
                        self.bad.push(format!("war {a}/{b}: level [{x}][{y}] port 1 exe {got:?}"));
                    }
                }
            }
            for &(a, b) in &self.allies {
                // A war the record declared afterwards (836) wins over the alliance.
                let war = (a == en && self.wars.contains(&b)) || (b == en && self.wars.contains(&a));
                let want: u8 = if war { 1 } else { 5 };
                for (x, y) in [(a, b), (b, a)] {
                    let got = lev.get((x * 9 + y) as usize).copied();
                    if got != Some(want) {
                        self.bad.push(format!("alliance {a}/{b}: level [{x}][{y}] port {want} exe {got:?}"));
                    }
                }
            }
        }
        if let Some(mut model) = self.cdis.take() {
            let got = &post["cdis"];
            if got[0] != model[0] {
                self.bad.push(format!("colony discount designs: port {} exe {}", model[0], got[0]));
            }
            // The dump lists the human's planets: one the record handed over (840's takeover) drops out.
            let en = self.en;
            model[1].as_array_mut().unwrap().retain(|q| planets.get(int(&q[0]) as usize).is_some_and(|x| x.owner == en));
            if !self.tick && got[1] != model[1] {
                self.bad.push(format!("colony discount planets: port {} exe {}", model[1], got[1]));
            }
        }
        self.finish_2c(&post);
        self.finish_2b(&post);
    }

    /// st_Rnd part 2c's direct writes against `post`.
    fn finish_2c(&mut self, post: &Value) {
        let planets = planets_of(post);
        let fleets = fleets_of(post);
        for (&f, &id) in &self.fleet_ids {
            if fleets.get(f as usize).map(|q| q.id) != Some(id) {
                self.bad.push(format!("SetFleetId {{ fleet: {f}, id: {id} }}: exe {:?}", fleets.get(f as usize).map(|q| q.id)));
            }
        }
        for &p in &self.unhidden {
            if let Some(q) = planets.get(p as usize).filter(|q| q.views.len() == 19) {
                if (1..19).any(|e| q.views[e] < 0 && !self.views_of.contains_key(&(p, e as i32))) {
                    self.bad.push(format!("UnhidePlanet({p}): exe views {:?}", q.views));
                }
            }
        }
        for (&(p, e), &view) in &self.views_of {
            let got = planets.get(p as usize).and_then(|q| q.views.get(e as usize).copied());
            if got != Some(view as i16) {
                self.bad.push(format!("planet {p} view of {e}: port {view} exe {got:?}"));
            }
        }
        for (&p, &text) in &self.planet_names {
            let want = self.stext.get(&text).cloned().unwrap_or_default();
            if post["pname"].get(0).map(int) != Some(p) || post["pname"][1].as_str() != Some(want.as_str()) {
                self.bad.push(format!("planet {p} name: port {want:?} exe {}", post["pname"]));
            }
        }
        for &(slot, name, xp) in &self.recruited {
            let en = self.en as usize;
            let r = &post["spies"][en][1][slot as usize];
            let want = self.recruits.get(name as usize).cloned().unwrap_or_default();
            // 138's record: alive (1), race 14, +0x16 = 1, rank 2, the experience drawn, the name.
            if r.is_null() || int(&r[0]) != 1 || int(&r[1]) != 14 || int(&r[6]) != xp || int(&r[7]) != 2 || int(&r[9]) != 1 || r[10].as_str() != Some(want.as_str()) {
                self.bad.push(format!("recruit {slot}: port ({want:?}, {xp}) exe {r}"));
            }
        }
        for (&f, text) in &self.fleet_names {
            let want = match text {
                Some(t) => self.stext.get(t).cloned().unwrap_or_default(),
                None => self.alapnev.clone(),
            };
            if post["fnames"].get(f as usize).and_then(|n| n.as_str()) != Some(want.as_str()) {
                self.bad.push(format!("fleet {f} name: port {want:?} exe {:?}", post["fnames"].get(f as usize)));
            }
        }
        for (&f, &(x, y)) in &self.fleet_pos {
            // A later order may have moved it: the record's own moves come after (checked as calls).
            if fleets.get(f as usize).is_none_or(|q| q.x != x || q.y != y) && !self.events.iter().any(|e| matches!(e, StoryEvent::MoveFleet { fleet, .. } if *fleet == f)) {
                self.bad.push(format!("fleet {f} position: port ({x}, {y}) exe {:?}", fleets.get(f as usize).map(|q| (q.x, q.y))));
            }
        }
        // st_Rnd 2c traces: a fleet of the record's `pre` that became the human's took a story's write (178).
        if post.get("kmon").is_some() {
            for (f, (a, b)) in fleets_of(&self.w).iter().zip(&fleets).enumerate() {
                if a.owner != 0xff && b.owner == self.en && a.owner != self.en && !self.fleet_owners.contains_key(&(f as i32)) {
                    self.bad.push(format!("fleet {f} became the human's: the port didn't make it so"));
                }
            }
        }
        for (&f, &o) in &self.fleet_owners {
            if fleets.get(f as usize).map(|q| q.owner) != Some(o) {
                self.bad.push(format!("fleet {f} owner: port {o} exe {:?}", fleets.get(f as usize).map(|q| q.owner)));
            }
        }
        for (&k, &d) in &self.tank_designs {
            let got = post["tanks"].get(k as usize).map(|t| int(&t[1]));
            if got != Some(d) {
                self.bad.push(format!("units {k} design: port {d} exe {got:?}"));
            }
        }
        let en = self.en;
        for (&(e, d), &key) in self.design_keys.iter().filter(|_| post.get("des2").is_some()) {
            let got = post["des2"].get(e as usize).and_then(|x| x.get(d as usize)).map(|r| int(&r[2]));
            if got != Some(key) {
                self.bad.push(format!("design {e}/{d} key: port {key} exe {got:?}"));
            }
        }
        for (&d, &(b18, b19)) in &self.design_flags {
            let got = post["dnames"].get(d as usize).map(|r| (int(&r[0]), int(&r[1])));
            if got != Some((b18, b19)) {
                self.bad.push(format!("design {en}/{d} +0x18/+0x19: port ({b18}, {b19}) exe {got:?}"));
            }
        }
        for (&d, text) in &self.design_names {
            let r = &post["dnames"][d as usize];
            let want = match text {
                Some(t) => self.stext.get(t).cloned().unwrap_or_default(),
                None => r[3].as_str().unwrap_or("").to_string(),
            };
            if r[2].as_str() != Some(want.as_str()) {
                self.bad.push(format!("design {en}/{d} name: port {want:?} exe {}", r[2]));
            }
        }
        if let Some((m, b)) = self.stray {
            if int(&post["stray"][0]) != m || int(&post["stray"][1]) != b {
                self.bad.push(format!("174's stray morale write: port ({m}, {b}) exe {}", post["stray"]));
            }
        }
        if let Some(s4) = post["seed4"].as_u64() {
            if self.rng.state(stream::SPY) != s4 as u32 {
                self.bad.push(format!("stream 4: port {:#x} exe {s4:#x}", self.rng.state(stream::SPY)));
            }
        }
        if let Some(s0) = post["seed0"].as_u64() {
            if self.rng.state(0) != s0 as u32 {
                self.bad.push(format!("stream 0: port {:#x} exe {s0:#x}", self.rng.state(0)));
            }
        }
        // st_Elem part 2d: designs' bytes (st_Elem rows: +0x34 at 5, +0x28 at 8) and the keys the stories wrote (+0x80
        // at 4).
        for (&(d, off), &val) in &self.design_bytes {
            let col = match off {
                0x34 => 5,
                0x28 => 8,
                _ => continue,
            };
            let got = post["designs"].get(d as usize).and_then(|x| x.get(col)).map(int);
            if got != Some(val) {
                self.bad.push(format!("design {d} +{off:#x}: port {val} exe {got:?}"));
            }
        }
        if post.get("seed0").is_some() {
            for (&(e, d), &key) in &self.design_keys {
                let got = post["designs"].get(d as usize).and_then(|x| x.get(4)).map(int);
                if e == self.en && got != Some(key) {
                    self.bad.push(format!("design {d} key: port {key} exe {got:?}"));
                }
            }
        }
    }

    /// `BolygoInfoAdd`: the info takes a free slot of the five (seen by later `BolygoInfoIs` in the record); with
    /// none free nothing is added (st_Rnd 149 then picks the same planet again).
    fn add_info(&mut self, planet: i32, info: i32) {
        if let Some(p) = self.planets.get_mut(planet as usize) {
            if !p.infos.contains(&info) {
                match p.infos.iter().position(|&i| i == 0) {
                    Some(k) => p.infos[k] = info,
                    None if p.infos.len() < 5 => p.infos.push(info),
                    None => {}
                }
            }
        }
    }

    fn w_story_pre(&self) -> Value {
        self.pre_story.clone()
    }
    fn post_world(&self) -> Value {
        self.post_w.clone()
    }
}

impl W<'_> {
    /// st_Elem part 2b's writes: the rioted units (owner and `+0x198` 10, `+0x285` 1, the origin fleets, the design),
    /// their battlefields' status caches 0, the held screen.
    fn finish_2b(&mut self, post: &Value) {
        for &(p, k, fleet, design) in &self.rioted.clone() {
            let Some((units, cache)) = self.post_units(p) else {
                self.bad.push(format!("riot on {p}: no units in post"));
                continue;
            };
            let u = &units[k as usize];
            let n = int(&u[4]);
            let origins = ints(&u[8]);
            if (int(&u[1]), int(&u[6]), int(&u[7]), int(&u[5])) != (10, 10, 1, design) || origins.len() != n.clamp(0, 16) as usize || origins.iter().any(|&o| o != fleet) {
                self.bad.push(format!("riot unit {p}/{k}: port fleet {fleet} design {design}, exe {u}"));
            }
            if cache != 0 {
                self.bad.push(format!("riot on {p}: exe WF+0x8cc {cache}"));
            }
        }
        if let Some(p) = self.held {
            let got = ints(&post["kep"]);
            if got != [3, p, p, 1] {
                self.bad.push(format!("held planet {p}: exe AktKepKell, AktPlanetKell, 0xF261F8, IdoPause {got:?}"));
            }
        }
    }
}

fn load(s: &Value) -> Story {
    let mut st = Story::default();
    for (k, q) in s["buckets"].as_object().unwrap() {
        st.buckets[k.parse::<usize>().unwrap()] = queue(q);
    }
    st.history = queue(&s["hist"]);
    st.starting = queue(&s["start"]);
    st.finished = queue(&s["vegzes"]);
    st.specials = queue(&s["talal"]);
    st.read = queue(&s["olvas"]);
    st.planets = pairs(&s["b"]).into_iter().map(|(id, p)| (id, p as u8)).collect();
    st.counters.set_live(&pairs(&s["sz"]));
    st.logic.set_live(&pairs(&s["l"]));
    st.vars.set_live(&pairs(&s["v"]));
    st.error = int(&s["err"]);
    let flags = unhex(s["flags"].as_str().unwrap());
    for (f, &b) in st.flags.iter_mut().zip(&flags[..FLAGS]) {
        *f = b != 0;
    }
    st.last_checked = int(&s["last"]);
    st.counter_done = int(&s["szv"]);
    st.old_ido = int(&s["oldido"]);
    st.delayed = int(&s["delayed"]) != 0;
    st.skip_first = int(&s["skip"]);
    st.restarting = int(&s["restart"]);
    st.adat.copy_from_slice(&unhex(s["adat"].as_str().unwrap()));
    st.undo = s["ozon"].as_array().unwrap().iter().map(|e| (int(&e[0]), int(&e[1]), int(&e[2]))).collect();
    let sig = ints(&s["signal"]);
    st.signal = (sig[0], sig[1]);
    st.last_addship = int(&s["addship"]);
    st.special.copy_from_slice(&ints(&s["special"]));
    let r = ints(&s["radar"]);
    st.radar = (r[0], r[1]);
    let d = ints(&s["dipl"]);
    st.diplomacy = (d[0], d[1], d[2]);
    let a = ints(&s["arr"]);
    st.arrival = story::Arrival { fleet_id: a[0], fleet: a[1], target: a[2], target_type: a[3], target_fleet_id: a[4] };
    let g = ints(&s["gr"]);
    st.ground = story::GroundBattle {
        fleet: g[0],
        fleet_id: g[1],
        fleet_owner: g[2],
        planet: g[3],
        old_owner: g[4],
        attacker_won: g[5] != 0,
        defender_won: g[6] != 0,
    };
    st.you_were_in_battle = int(&s["youwas"]) != 0;
    st.spy_kind = int(&s["kem"]);
    let m = ints(&s["msg"]);
    st.message = (m[0], m[1]);
    st.disable_add_ship = int(&s["disable"]);
    st.last_fn.copy_from_slice(&ints(&s["fn"]));
    st.line_story = int(&s["line"]);
    st.scenario = int(&s["scen"]);
    st.nearest_planet = s.get("near").map_or(0, int);
    if let Some(b) = s.get("best3") {
        st.best_prod.copy_from_slice(&ints(b));
    }
    st.all_fleets_visible = s.get("allvis").map_or(0, |x| int(x) as u8);
    if let Some(k) = s.get("kn") {
        let k = ints(k);
        st.kem = KemNews {
            news: k[0],
            spy: k[1],
            died: k[2],
            success: k[3] != 0,
            mission: k[4],
            target: k[5],
            tech: k[6],
            techs: k[7..12].try_into().unwrap(),
        };
    }
    st.counter_spy = s.get("ci").map_or(0, int);
    st.battle_winner = s.get("spwin").or(s.get("kinyert")).map_or(0, int);
    st.spy_story = s.get("kaz").map_or(0, int);
    st.spy_answer = s.get("kval").map_or(0, int);
    st.spy_owner = s.get("ktul").map_or(0, int);
    st.spy_slot = s.get("kakt").map_or(0, int);
    st.last_collect_fleet = s.get("lcol").map_or(0, int);
    if let Some(n) = s.get("n3") {
        st.nearest3.copy_from_slice(&ints(n));
    }
    if let Some(n) = s.get("n4") {
        st.brutal4.copy_from_slice(&ints(n));
    }
    // st_Rnd traces: the finished space battle's participants.
    if let Some(b) = s.get("battle").and_then(|b| b.as_array()) {
        st.battle = b.iter().map(|p| story::Participant { owner: int(&p[0]), slot_id: int(&p[1]), fleet_id: int(&p[2]) }).collect();
    }
    if let Some(k) = s.get("kemev") {
        let k = ints(k);
        st.kem = KemNews { news: k[0], spy: k[1], died: k[2], ..KemNews::default() };
    }
    if let Some(p) = s.get("pact") {
        st.battle_pact.copy_from_slice(&ints(p));
    }
    if let Some(l) = s.get("spwl") {
        // The participants the snapshot could read (at least three: the scripts read past a shorter list), cut to
        // SpwFlottaNum.
        let n = int(&s["spw"][1]).max(0) as usize;
        st.battle = l.as_array().unwrap().iter().take(n).map(|p| story::Participant { owner: int(&p[0]), slot_id: int(&p[1]), fleet_id: int(&p[2]) }).collect();
    }
    st
}

/// The port's state against a recorded snapshot: the differing fields.
fn compare(st: &Story, s: &Value) -> Vec<String> {
    let want = load(s);
    let mut bad = Vec::new();
    macro_rules! field {
        ($($f:ident).+) => {
            if st.$($f).+ != want.$($f).+ {
                bad.push(format!("{}: port {:?} exe {:?}", stringify!($($f).+), st.$($f).+, want.$($f).+));
            }
        };
    }
    for k in 0..story::BUCKETS {
        if st.buckets[k] != want.buckets[k] {
            bad.push(format!("bucket {k}: port {:?} exe {:?}", st.buckets[k].entries, want.buckets[k].entries));
        }
    }
    field!(history);
    field!(starting);
    field!(finished);
    field!(specials);
    field!(read);
    field!(planets);
    if st.counters.live() != want.counters.live() {
        bad.push(format!("counters: port {:?} exe {:?}", st.counters.live(), want.counters.live()));
    }
    if st.logic.live() != want.logic.live() {
        bad.push(format!("logic: port {:?} exe {:?}", st.logic.live(), want.logic.live()));
    }
    if st.vars.live() != want.vars.live() {
        bad.push(format!("vars: port {:?} exe {:?}", st.vars.live(), want.vars.live()));
    }
    field!(error);
    field!(flags);
    field!(last_checked);
    field!(counter_done);
    field!(old_ido);
    field!(delayed);
    field!(skip_first);
    field!(restarting);
    if st.adat != want.adat {
        let diff: Vec<usize> = (0..story::ADAT).filter(|&i| st.adat[i] != want.adat[i]).collect();
        bad.push(format!("adat differs at {diff:x?}"));
    }
    field!(undo);
    field!(signal);
    field!(last_addship);
    field!(special);
    field!(radar);
    field!(diplomacy);
    field!(arrival);
    field!(ground);
    field!(you_were_in_battle);
    field!(spy_kind);
    field!(message);
    field!(disable_add_ship);
    field!(last_fn);
    field!(line_story);
    field!(scenario);
    field!(nearest_planet);
    field!(best_prod);
    field!(all_fleets_visible);
    field!(counter_spy);
    field!(spy_story);
    field!(spy_answer);
    field!(spy_owner);
    field!(spy_slot);
    field!(last_collect_fleet);
    if s.get("n3").is_some() {
        field!(nearest3);
    }
    if s.get("n4").is_some() {
        field!(brutal4);
    }
    if s.get("kemev").is_some() {
        field!(kem);
    }
    if s.get("pact").is_some() {
        field!(battle_pact);
    }
    bad
}

fn story_data(setup: &Value) -> (Vec<(i32, i32)>, Vec<FleetData>, Vec<BuildingData>) {
    let d = &setup["data"];
    let values = pairs(&d["values"]);
    let fleets = d["fleets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let c = ints(&f[3]);
            let mut counts = [[0i16; 9]; 4];
            for (i, x) in c.iter().enumerate() {
                counts[i / 9][i % 9] = *x as i16;
            }
            FleetData { story: int(&f[0]), fleet: int(&f[1]), design: int(&f[2]), counts }
        })
        .collect();
    let buildings = d["buildings"].as_array().map_or(Vec::new(), |b| {
        b.iter().map(|r| BuildingData { story: int(&r[0]), planet: int(&r[1]), kind: int(&r[2]), level: int(&r[3]) }).collect()
    });
    (values, fleets, buildings)
}

/// st_Fable's direct writes against the exe's world after the record: fleet ids and the story's `+0x18`/`+0x52`
/// bits, planet radars, the human's spies alive, `spFlottak`, the story spy's record.
fn fable_post(w: &W, post: &Value) -> Vec<String> {
    let mut bad = Vec::new();
    let exe = fleets_of(post);
    let pre = fleets_of(&w.pre_w);
    // The stale records past the fleet count aren't compared.
    let n = fleet_total(&w.pre_w).min(fleet_total(post)) as usize;
    for (f, (a, b)) in w.fleets.iter().zip(&exe).enumerate().take(n + 1).skip(1) {
        // Slots the record's game code filled anew (the fleets made, wormholes) aren't the story's writes.
        let old = pre.get(f);
        if w.new_fleets.contains(&(f as i32)) || old.is_none_or(|o| o.owner == 0xff || o.owner != b.owner || (o.flags ^ b.flags) & 0x400 != 0) {
            continue;
        }
        if a.id != b.id || (a.state ^ b.state) & 3 != 0 || (a.flags ^ b.flags) & 0xb80 != 0 {
            bad.push(format!("fleet {f}: port id {} +0x18 {:#x} +0x52 {:#x}, exe {} {:#x} {:#x}", a.id, a.state, a.flags, b.id, b.state, b.flags));
        }
    }
    let planets = planets_of(post);
    for (p, (a, b)) in w.planets.iter().zip(&planets).enumerate() {
        if a.radar != b.radar {
            bad.push(format!("planet {p} radar: port {} exe {}", a.radar, b.radar));
        }
    }
    if let Some(k) = post["kem"].as_array() {
        for (i, x) in k.iter().enumerate() {
            let alive = |v: i32| v > 0;
            if w.spies.get(i).is_some_and(|s| alive(s.0) != alive(int(&x[0]))) {
                bad.push(format!("spy {i}: port alive {:?}, exe {}", w.spies[i].0, x[0]));
            }
        }
    }
    if let (Some((n, e)), Some(a)) = (&w.spf, post["spf"].as_array()) {
        let exe: Vec<(i32, i32)> = a[1..].iter().map(|x| (int(&x[0]), int(&x[1]))).collect();
        if *n != int(&a[0]) || e[..*n as usize] != exe[..*n as usize] {
            bad.push(format!("spFlottak: port {n} {e:?}, exe {a:?}"));
        }
    }
    if let Some((k, xp)) = w.story_spy {
        let row = unhex(post["kemrow"].as_str().unwrap_or(""));
        let r = row.get(0x4c * k as usize..0x4c * (k as usize + 1));
        let want = fable_spy_record(xp);
        // The name (to +0x0e) and the bytes the exe leaves as its stack held them are not compared.
        let skip = |i: usize| i < 0x10 || [0x13, 0x19, 0x1d, 0x22, 0x23].contains(&i);
        if r.is_none_or(|r| (0..0x4c).any(|i| !skip(i) && r[i] != want[i])) {
            bad.push(format!("story spy {k}: exe record {r:x?}, port {want:x?}"));
        }
        if r.is_some_and(|r| !r.starts_with(w.agent.as_deref().unwrap_or("?").as_bytes())) {
            bad.push(format!("story spy {k}: not the agent's name"));
        }
    }
    bad
}

/// The story spy's record (`campaign.md` §8.11) with its drawn XP.
fn fable_spy_record(xp: i32) -> [u8; 0x4c] {
    let mut r = [0u8; 0x4c];
    let mut put = |o: usize, b: &[u8]| r[o..o + b.len()].copy_from_slice(b);
    put(0x10, &1i16.to_le_bytes());
    put(0x12, &[1]);
    put(0x14, &4i16.to_le_bytes());
    put(0x16, &1i16.to_le_bytes());
    put(0x1a, &4i16.to_le_bytes());
    put(0x1c, &[3]);
    put(0x24, &1000i32.to_le_bytes());
    put(0x30, &0x100i32.to_le_bytes());
    put(0x3c, &xp.to_le_bytes());
    put(0x40, &1i32.to_le_bytes());
    r
}

#[derive(Default)]
struct Stats {
    records: usize,
    kinds: std::collections::BTreeMap<String, usize>,
    started: std::collections::BTreeMap<i32, usize>,
    ended: std::collections::BTreeMap<i32, usize>,
    acts: std::collections::BTreeMap<String, usize>,
    counters_fired: usize,
    draws: usize,
    won: usize,
    died: usize,
    unchecked: std::collections::BTreeMap<String, usize>,
    /// The port's events by kind (coverage).
    events: std::collections::BTreeMap<String, usize>,
}

#[test]
fn campaign_matches_exe() {
    let files = traces("campaign-");
    let Some((tables, buildings)) = tables() else {
        eprintln!("skipped: IG2_INSTALL_DIR not set (the campaign tables come from the exe)");
        return;
    };
    if files.is_empty() {
        eprintln!("skipped: no campaign-*.jsonl traces");
        return;
    }
    let mut stats = Stats::default();
    let mut failures = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let recs: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let Some(setup) = recs.iter().find(|r| r["k"] == "setup") else { continue };
        let (values, fleet_data, building_data) = story_data(setup);
        // The scripts the tracer kept in the exe (the others are patched out on both sides); `st_Multi_include`
        // runs last (it does nothing in the traces that patched it out: no mode runs there).
        let keep: Vec<&str> = setup["keep"].as_array().map(|k| k.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
        // `Story_Check`'s order: `Begin`, `Rnd`, `BRnd` (only where the tracer kept them: BRnd's special-0x19 check
        // runs in every check), `Scen`, `Line`, `Elem`, `Multi`.
        let mut chain: Vec<Box<dyn Script>> = Vec::new();
        if keep.contains(&"begin") {
            chain.push(Box::new(BeginStories));
        }
        if keep.contains(&"rnd") {
            // Traces recorded before part 2c (no `_s` texts in the setup) had its stories patched out.
            let rnd = if setup.get("stext5").is_some_and(|x| !x.is_null()) { RndStories::default() } else { RndStories::without(&ig2_sim::rnd::PART_2C) };
            chain.push(Box::new(rnd));
        }
        if keep.contains(&"brnd") {
            chain.push(Box::new(BRndStories));
        }
        if keep.contains(&"fable") {
            chain.push(Box::new(FableStories));
        }
        chain.push(Box::new(ScenarioStories));
        if keep.contains(&"line") {
            chain.push(Box::new(LineStories));
        }
        if keep.contains(&"elem") {
            // Traces recorded before part 2a (no `elem2a` in the setup) had its stories patched out.
            // ... and those before part 2b (no `elem2b`) 808 and 854, before part 2c (no `elem2c`) 810, 857, 858, 860.
            let (a, b) = (setup["elem2a"].as_bool() == Some(true), setup["elem2b"].as_bool() == Some(true));
            let mut skip = Vec::new();
            if setup["elem2c"].as_bool() != Some(true) {
                skip.extend(ig2_sim::elem::PART_2C);
            }
            // ... and before part 2d (no `elem2d`) 801, 804, 806, 807, 809.
            if setup["elem2d"].as_bool() != Some(true) {
                skip.extend(ig2_sim::elem::PART_2D);
            }
            // ... and before part 2e (no `elem2e`) 814, 815, 821, 832, 834.
            if setup["elem2e"].as_bool() != Some(true) {
                skip.extend(ig2_sim::elem::PART_2E);
            }
            // ... and before part 2f (no `elem2f`) 836, 839, 840, 843.
            if setup["elem2f"].as_bool() != Some(true) {
                skip.extend(ig2_sim::elem::PART_2F);
            }
            // ... and before part 2g (no `elem2g`) 844, 856, 862, 863.
            if setup["elem2g"].as_bool() != Some(true) {
                skip.extend(ig2_sim::elem::PART_2G);
            }
            if !a {
                skip.extend(ig2_sim::elem::PART_2A);
            }
            if !b {
                skip.extend(ig2_sim::elem::PART_2B);
            }
            let elem = ElemStories::without(&skip);
            chain.push(Box::new(elem));
        }
        chain.push(Box::new(MultiStories));
        let sc = &Chain(chain);
        let agent = setup["agent"].as_str().map(str::to_string);
        // st_Rnd part 2c: `_s(5, id)` of the stories' fleet, design and planet names, 138's recruits,
        // `AlapFlottaNev`.
        let stext: HashMap<i32, String> =
            setup["stext5"].as_array().map_or(HashMap::new(), |a| a.iter().map(|x| (int(&x[0]), x[1].as_str().unwrap_or("").to_string())).collect());
        let recruits: Vec<String> = setup["recruits"].as_array().map_or(Vec::new(), |a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect());
        let alapnev = setup["alapnev"].as_str().unwrap_or("").to_string();
        let infos: Vec<i32> = setup["infos"].as_array().map_or(Vec::new(), |a| a.iter().map(int).collect());
        for r in &recs {
            let kind = r["k"].as_str().unwrap();
            if !matches!(kind, "init" | "tick" | "check" | "staged") {
                continue;
            }
            let (pre, post) = (&r["pre"], &r["post"]);
            // A snapshot the tracer failed to take (a script error on the game thread): nothing to compare.
            if pre["s"].get("buckets").is_none() || post["s"].get("buckets").is_none() {
                *stats.unchecked.entry("record: no snapshot".into()).or_default() += 1;
                continue;
            }
            let mut st = load(&pre["s"]);
            st.values.clone_from(&values);
            st.fleet_data.clone_from(&fleet_data);
            st.building_data.clone_from(&building_data);
            let mut w = W::new(r, &tables, &infos, &buildings);
            w.agent.clone_from(&agent);
            w.stext.clone_from(&stext);
            w.recruits.clone_from(&recruits);
            w.alapnev.clone_from(&alapnev);
            let seed0 = w.rng.state(story::STREAM);
            stats.records += 1;
            let label = if kind == "staged" { format!("staged {}", r["call"].as_str().unwrap()) } else { kind.to_string() };
            *stats.kinds.entry(label).or_default() += 1;
            match kind {
                "init" => st.init_story(int(&r["arg"]), &mut w),
                "tick" => st.tick(sc, &mut w),
                "check" => st.check(int(&r["arg"]) != 0, sc, &mut w),
                _ => {
                    let a = &r["args"];
                    match r["call"].as_str().unwrap() {
                        "start" => {
                            st.start_story(int(&a[0]), false);
                            st.check(true, sc, &mut w);
                        }
                        "special" => st.story_special(ints(a).try_into().unwrap(), sc, &mut w),
                        "arrive" => {
                            let (f, order, target) = (int(&a[0]), int(&a[1]), int(&a[2]));
                            st.fleet_arrived_save(f, order, target, &w);
                            st.fleet_arrived_check(sc, &mut w);
                        }
                        // A story mission's question staged as `StoryEredmenyKem*` stages it (flag 15, kind 1 or 2,
                        // `KemkedesValasz` −1): asked through `Story::spy_question`, its answer against the exe's.
                        "flag" if int(&a[0]) == 15 && matches!(st.spy_kind, 1 | 2) && st.spy_answer == -1 => {
                            let k = (0..12).find(|&k| tables.story_mission(k) == st.spy_story).expect("a story mission");
                            let got = st.spy_question(st.spy_kind == 1, st.spy_owner, st.spy_slot, k + 14, sc, &mut w);
                            let kval = int(&post["s"]["kval"]);
                            assert_eq!(got, kval > 0, "{} seq {}: story mission {} question", path.display(), r["seq"], st.spy_story);
                            // The tracer's `post` is from before the exe's clamp of a negative answer.
                            st.spy_answer = kval;
                            *stats.kinds.entry("spy question".into()).or_default() += 1;
                        }
                        // The flag and its payload are in `pre` (written right before the call).
                        "flag" => st.check(true, sc, &mut w),
                        "win" => st.win(int(&a[0]), int(&a[1]) != 0, &mut w),
                        "spwfinished" => {
                            let list: Vec<story::Participant> = a
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|p| story::Participant {
                                    owner: int(&p[0]),
                                    slot_id: int(&p[1]),
                                    fleet_id: int(p.get(2).unwrap_or(&p[1])),
                                })
                                .collect();
                            st.space_battle_finished(&list, true, sc, &mut w);
                        }
                        other => panic!("unknown staged call {other}"),
                    }
                }
            }
            // Coverage.
            let pre_s = load(&pre["s"]);
            for k in 0..story::BUCKETS {
                for &(id, _) in &st.buckets[k].entries {
                    if !pre_s.buckets[k].contains(id) {
                        *stats.started.entry(id).or_default() += 1;
                    }
                }
                for &(id, _) in &pre_s.buckets[k].entries {
                    if !st.buckets[k].contains(id) {
                        *stats.ended.entry(id).or_default() += 1;
                    }
                }
            }
            stats.counters_fired += st.counters.live().iter().zip(pre_s.counters.live()).filter(|(a, b)| a.1 == -1 && b.1 >= 0).count();
            for a in &w.acts {
                *stats.acts.entry(a[0].as_str().unwrap().to_string()).or_default() += 1;
            }
            let mut s5 = seed0;
            for _ in 0..100_000 {
                if s5 == w.rng.state(story::STREAM) {
                    break;
                }
                s5 = ig2_sim::rng::step(s5);
                stats.draws += 1;
            }
            let mut bad = compare(&st, &post["s"]);
            if w.rng.state(story::STREAM) != post["w"]["seed5"].as_u64().unwrap() as u32 {
                bad.push(format!("stream 5: port {:#x} exe {:#x}", w.rng.state(story::STREAM), post["w"]["seed5"].as_u64().unwrap()));
            }
            if let Some(s2) = post["w"]["seed2"].as_u64() {
                if w.rng.state(stream::GROUND_COMBAT) != s2 as u32 {
                    bad.push(format!("stream 2: port {:#x} exe {s2:#x}", w.rng.state(stream::GROUND_COMBAT)));
                }
            }
            // Fleets' always-visible bit (1003's radar); slots the record appended aren't compared.
            if let (Some(port), Some(exe)) = (&w.f52, w_f52(&post["w"])) {
                let diff: Vec<usize> = (0..port.len().min(exe.len())).filter(|&f| (port[f] ^ exe[f]) & 0x100 != 0).collect();
                if !diff.is_empty() {
                    bad.push(format!("fleet +0x52 bit 8 differs for fleets {diff:?}"));
                }
            }
            let (e0, e1) = (ints(&pre["ends"]), ints(&post["ends"]));
            let won = w.events.iter().filter(|e| matches!(e, StoryEvent::Won | StoryEvent::CampaignOver)).count() as i32;
            let died: Vec<i32> = w.events.iter().filter_map(|e| if let StoryEvent::EmpireDies(x) = e { Some(*x) } else { None }).collect();
            stats.won += won as usize;
            stats.died += died.len();
            for e in &w.events {
                *stats.events.entry(format!("{e:?}").split([' ', '(']).next().unwrap_or("").to_string()).or_default() += 1;
            }
            for u in &w.unchecked {
                *stats.unchecked.entry(u.clone()).or_default() += 1;
            }
            if won != e1[0] - e0[0] {
                bad.push(format!("MEGNYERES calls: port {won} exe {}", e1[0] - e0[0]));
            }
            if died.len() as i32 != e1[2] - e0[2] || died.last().is_some_and(|&x| x != e1[3]) {
                bad.push(format!("MEGHALAS calls: port {died:?} exe {} (last {})", e1[2] - e0[2], e1[3]));
            }
            w.finish();
            if keep.contains(&"fable") && kind != "init" {
                bad.extend(fable_post(&w, &post["w"]));
            }
            bad.append(&mut w.bad);
            if w.next < w.acts.len() {
                bad.push(format!("exe calls the port didn't make: {:?}", &w.acts[w.next..]));
            }
            if w.missing.get() {
                if std::env::var_os("IG2_MISSING").is_some() {
                    eprintln!("not dumped: {} seq {}", path.display(), r["seq"]);
                }
                *stats.unchecked.entry("record: buildings not dumped".into()).or_default() += 1;
                continue;
            }
            if !bad.is_empty() {
                failures.push(format!("{} seq {} {kind} {}: {}", path.display(), r["seq"], r["call"], bad.join("; ")));
            }
        }
    }
    eprintln!(
        "campaign replay: {} records {:?}; stories started {:?}, ended {:?}; exe calls {:?}; {} counters fired, \
         {} stream-5 draws, {} MEGNYERES, {} MEGHALAS, events not checked {:?}; port events {:?}",
        stats.records, stats.kinds, stats.started, stats.ended, stats.acts, stats.counters_fired, stats.draws, stats.won,
        stats.died, stats.unchecked, stats.events
    );
    eprintln!(
        "ticker lines checked: {} (texts in {} languages)",
        SCROLL_LINES.load(std::sync::atomic::Ordering::Relaxed),
        story_texts().len()
    );
    // `IG2_FAILURES=<file>`: every mismatch, for triage.
    if let Some(f) = std::env::var_os("IG2_FAILURES") {
        std::fs::write(f, failures.join("\n")).unwrap();
    }
    assert!(failures.is_empty(), "{} mismatches, first: {}", failures.len(), failures[..failures.len().min(8)].join("\n"));
}
