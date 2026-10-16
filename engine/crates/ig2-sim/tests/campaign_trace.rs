//! Replays the campaign recorded from the running GOG `ig2.exe` (tools/trace/run_story.py `--campaign`,
//! `re/rules/campaign.md` §8): every record of `campaign-*.jsonl`.
//! - `init`: `InitStory` (exe 0x5c2940) inside the campaign start, with a snapshot after `LoadScenario`;
//! - `tick`: one `FRISSIT_Story` (0x5c1ff0) the game ran (counters, the daily check, the inline stories);
//! - `check`: one outermost `Story_Check` (0x5c5610) a game hook ran;
//! - `staged`: a call the tracer made through its trampoline: `StartStory` + `Story_Check(1)` (the inline
//!   stories 402..918), `Story_Special` (0x11, 0x12), a fleet arrival, event flags + `Story_Check(1)`, `WIN`.
//!
//! - `--keep multi`: the multiplayer modes 1001..1006 (`st_Multi_include`, §8.10) staged the same way, with
//!   counters, ground battles, specials 10/15, the radar offer's answers, base arrivals and space battles.
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

use ig2_sim::brnd::BRndStories;
use ig2_sim::campaign::CampaignTables;
use ig2_sim::multi::{MultiStories, FLAG_CARRIER_UNSET};
use ig2_sim::rng::{stream, Streams};
use ig2_sim::story::{
    self, BuildingData, FleetData, MessageText, Notice, Queue, ScenarioStories, ScenarioStory, Script, Story, StoryEvent, StoryWorld,
    DESIGN, FLAGS,
};
use ig2_sim::storyline::LineStories;
use serde_json::Value;

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

/// The campaign tables and the buildings' workers (`EpuletekLeiro` `+0x10`), from the user's exe.
fn tables() -> Option<(CampaignTables, Vec<i32>)> {
    let dir = std::env::var_os("IG2_INSTALL_DIR")?;
    let exe = std::fs::read(PathBuf::from(dir).join("ig2.exe")).ok()?;
    let rules = ig2_rules::Rules::from_exe(&exe).ok()?;
    let workers = rules.buildings.iter().map(|b| b.workers).collect();
    Some((CampaignTables::new(rules.campaign_words), workers))
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
    workers: &'a [i32],
    /// The designs `new_design_raw` made, checked at their `TervUjraSzamol`.
    made: HashMap<(i32, i32), [u8; DESIGN]>,
    /// The last value the port wrote to a planet's `+0x28`, a ship's design, a planet's morale (checked against
    /// `post` at the end).
    values: HashMap<i32, i32>,
    ship_designs: HashMap<i32, i32>,
    morale: HashMap<i32, i32>,
    gmoral: Option<i32>,
    /// Ships whose design the port changed (`UpgradeShip`), seen by later reads in the record.
    ships: Option<Vec<(i32, i32)>>,
}

fn planets_of(w: &Value) -> Vec<Planet> {
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
        })
        .collect()
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
        })
        .collect()
}

impl<'a> W<'a> {
    fn new(r: &Value, tables: &'a CampaignTables, infos: &'a [i32], workers: &'a [i32]) -> Self {
        let w = r["pre"]["w"].clone();
        let mut rng = Streams::default();
        rng.set_state(story::STREAM, w["seed5"].as_u64().unwrap() as u32);
        if let Some(s2) = w["seed2"].as_u64() {
            rng.set_state(stream::GROUND_COMBAT, s2 as u32);
        }
        let mut designs = HashMap::new();
        let acts: Vec<Value> = r["acts"].as_array().unwrap().iter().filter(|a| int(&a[2]) == 0).cloned().collect();
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
            events: Vec::new(),
            bad: Vec::new(),
            unchecked: Vec::new(),
            met: Vec::new(),
            infos,
            designs,
            f52: w_f52(&r["pre"]["w"]),
            workers,
            made: HashMap::new(),
            values: HashMap::new(),
            ship_designs: HashMap::new(),
            morale: HashMap::new(),
            gmoral: None,
            ships: None,
        }
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

    fn expect(&mut self, name: &str, args: &[Value], what: &str) {
        if let Some(a) = self.take(name, what) {
            let got: Vec<Value> = a[1].as_array().unwrap().clone();
            if got.len() < args.len() || got[..args.len()] != *args {
                self.bad.push(format!("{what}: exe args {got:?}"));
            }
        }
    }

    /// `NewMessage`: may slow the game down (`SetIdoLelassit`), which the rest of the tick sees.
    fn message(&mut self, id: i32, arg: i32, what: &str) {
        self.expect("message", &[v(id), v(arg)], what);
        self.w["speed"] = self.post_w["speed"].clone();
    }

    fn byte(&self, key: &str, i: i32) -> u8 {
        self.w[key].as_str().map(unhex).and_then(|b| b.get(i as usize).copied()).unwrap_or(0)
    }
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
        self.fleets.len() as i32 - 1
    }
    fn fleet_owner(&self, f: i32) -> i32 {
        self.fleets.get(f as usize).map_or(0, |f| f.owner)
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
    fn money(&self, _e: i32) -> f64 {
        self.w["money"].as_f64().unwrap()
    }
    fn tech_status(&self, tech: i32, e: i32) -> i32 {
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
        let p = &self.planets[p as usize];
        (f32::from_bits(p.x), f32::from_bits(p.y))
    }
    fn planet_radar(&self, p: i32) -> i32 {
        self.planets[p as usize].radar
    }
    fn radar_range(&self, e: i32, level: i32) -> f32 {
        // The trace holds the human's radar row and (newer traces) owner 0's, which st_fNewEx reads when the
        // human has no planet left (slot 0); the rows are alike in the older traces.
        match e {
            0 if self.w.get("rr0").is_some() => bits(&self.w["rr0"][level as usize]),
            e if e == self.en || e == 0 => bits(&self.w["rr2"][level as usize]),
            _ => panic!("radar row of empire {e} read"),
        }
    }
    fn fleet_xy(&self, f: i32) -> (f32, f32) {
        let f = &self.fleets[f as usize];
        (f.x, f.y)
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
        if let Some(s) = &self.ships {
            return s[k as usize];
        }
        let s = &self.w["ships"][k as usize];
        (int(&s[0]), int(&s[1]))
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
                int(&a[3])
            }
            None => default,
        }
    }
    fn new_fleet(&mut self, _name: &str, owner: i32, id: i32, x: f32, y: f32, speed: f32) -> i32 {
        let what = format!("FlottaNew({owner}, {id}, {x}, {y}, {speed})");
        let Some(a) = self.take("fleetnew", &what) else { return 0 };
        let want = [v(owner), v(id), v(x.to_bits()), v(y.to_bits()), v(speed.to_bits())];
        if a[1].as_array().unwrap()[..] != want {
            self.bad.push(format!("{what}: exe {}", a[1]));
        }
        let f = int(&a[3]);
        if f as usize >= self.fleets.len() {
            self.fleets.resize(f as usize + 1, Fleet { owner: 0xff, id: 0, x: 0.0, y: 0.0, order: 0, w5c: 0, speed: 0, command: 0, target: 0.0, state: 0 });
        }
        self.fleets[f as usize] = Fleet { owner, id, x, y, order: 0, w5c: 0, speed: speed.to_bits(), command: 0, target: 0.0, state: 0 };
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
        assert_eq!(e, self.en);
        self.w["designs"].get(d as usize).map_or(0, |x| int(&x[4]))
    }
    fn design_attack(&self, e: i32, d: i32) -> i32 {
        assert_eq!(e, self.en);
        self.w["designs"].get(d as usize).map_or(0, |x| int(&x[3]))
    }
    fn tech_prereqs(&self, t: i32) -> [i32; 3] {
        let b = unhex(self.w["techs"].as_str().unwrap());
        let k = 4 * t as usize;
        [b[k + 1] as i32, b[k + 2] as i32, b[k + 3] as i32]
    }
    fn battle_fleets(&self) -> (i32, i32) {
        (int(&self.w["spf"][0]), int(&self.w["spf"][1]))
    }
    fn fleet_command(&self, f: i32) -> i32 {
        self.fleets[f as usize].command
    }
    fn fleet_target(&self, f: i32) -> f32 {
        self.fleets[f as usize].target
    }
    fn fleet_state(&self, f: i32) -> i32 {
        self.fleets[f as usize].state
    }
    fn planet_production(&self, p: i32) -> (i32, i32) {
        self.planets[p as usize].prod
    }
    fn building_workers(&self, kind: i32) -> i32 {
        self.workers[kind as usize]
    }
    fn wormhole_shown(&self) -> bool {
        self.w["wh"].as_array().unwrap().iter().any(|k| int(&k[0]) != 0 && int(&k[1]) != 0)
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
                self.expect("move", &[v(fleet), v(kind), v(mode), v(x.to_bits()), v(y.to_bits())], &what)
            }
            StoryEvent::StopFleet(f) => self.expect("move", &[v(f), v(0), v(0), v(0u32), v(0u32)], &what),
            StoryEvent::DeleteFleet(f) => self.expect("delete", &[v(f)], &what),
            StoryEvent::NewShip { fleet, design } => self.expect("ship", &[v(fleet), v(design)], &what),
            StoryEvent::NewTanks { fleet, design, count } => self.expect("tank", &[v(fleet), v(design), v(count)], &what),
            StoryEvent::Wormhole { x1, y1, x2, y2 } => self.expect("wormhole", &[v(x1), v(y1), v(x2), v(y2)], &what),
            StoryEvent::DestroyPlanet(p) => self.expect("destroy", &[v(p)], &what),
            StoryEvent::Anim(_) => self.expect("anim", &[], &what),
            StoryEvent::SetPlanetOwner { planet, owner, race } => {
                if self.planet_owner(planet) == 0 {
                    self.expect("colonise", &[v(planet), v(owner), v(race), v(1), v(0), v(0), v(0)], &what)
                } else {
                    self.expect("conquer", &[v(planet), v(owner)], &what)
                }
                if let Some(p) = self.planets.get_mut(planet as usize) {
                    p.owner = owner;
                }
            }
            StoryEvent::AddMoney { empire, amount } => {
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
                    // A text of the string table: any string.
                    MessageText::Text(_) => {
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
                self.expect("unhide", &[v(-p)], &what);
                let post = planets_of(&self.post_world());
                if post.get(p as usize).is_some_and(|q| q.views.len() == 19 && q.views[1..].iter().any(|&x| x < 0)) {
                    self.bad.push(format!("{what}: exe views {:?}", post[p as usize].views));
                }
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
            StoryEvent::Undo { kind: 2 | 6, .. } | StoryEvent::EpidemicOver => {
                // BolygoInfoRemove calls of the undo list / the epidemic's end (not checked).
                while self.acts.get(self.next).is_some_and(|a| a[0] == "inforemove") {
                    self.next += 1;
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
            StoryEvent::ScrollText(_) => {
                self.take("scroll", &what);
            }
            StoryEvent::SetFleetId { fleet, id } => {
                let post = fleets_of(&self.post_world());
                if post.get(fleet as usize).map(|f| f.id) != Some(id) {
                    self.bad.push(format!("{what}: exe {:?}", post.get(fleet as usize).map(|f| f.id)));
                }
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
            StoryEvent::AddPlanetInfo { planet, info } => self.expect("infoadd", &[v(planet), v(info), v(1)], &what),
            StoryEvent::SendStorySpecial(id) => self.expect("multisend", &[v(0x66), v(self.en), v(4), v(id)], &what),
            StoryEvent::DeleteShip(k) => self.expect("shipdelete", &[v(k)], &what),
            StoryEvent::SetFleetVisible { fleet, on } => {
                if let Some(f) = self.f52.as_mut().and_then(|f| f.get_mut(fleet as usize)) {
                    *f = if on { *f | 0x100 } else { *f & !0x100 };
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
                    Notice::Winner { empire, amazing } if empire == en => {
                        Box::new(move |t| t == if amazing { "AMAZING! YOU ARE THE WINNER!" } else { "YOU ARE THE WINNER!" })
                    }
                    Notice::Winner { .. } => Box::new(|t| t.ends_with(" is the winner!")),
                    Notice::CannotColonise(_) => Box::new(|t| t.starts_with("You cannot colonize ")),
                    Notice::FlagTaken { by, .. } if by == en => Box::new(|t| t.starts_with("You have the flag of ")),
                    Notice::FlagTaken { .. } => Box::new(|t| t.ends_with(" has your flag!")),
                    Notice::Touchdown { by } if by == en => Box::new(|t| t == "TOUCHDOWN!!!"),
                    Notice::Touchdown { .. } => Box::new(|t| t.ends_with(" has captured your flag!")),
                    Notice::FlagLost { by } if by == en => Box::new(|t| t == "You have lost the flag!"),
                    Notice::FlagLost { .. } => Box::new(|t| t == "Your flag has been returned!"),
                };
                if let Some(a) = self.take("text", &what) {
                    if !a[1][0].as_str().is_some_and(&*ok) {
                        self.bad.push(format!("{what}: exe {}", a[1]));
                    }
                }
            }
            StoryEvent::Won
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
        if let Some(g) = self.gmoral {
            if int(&post["gmoral"][0]) != g || int(&post["gmoral"][1]) != 6 {
                self.bad.push(format!("global morale: port {g} exe {}", post["gmoral"]));
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
}

#[test]
fn campaign_matches_exe() {
    let files = traces("campaign-");
    let Some((tables, workers)) = tables() else {
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
        // `st_BRnd_include` runs first; only where the tracer kept it (its special-0x19 check runs in every check).
        let sc: Box<dyn Script> = match (keep.contains(&"brnd"), keep.contains(&"line")) {
            (true, true) => Box::new(((BRndStories, (ScenarioStories, LineStories)), MultiStories)),
            (true, false) => Box::new(((BRndStories, ScenarioStories), MultiStories)),
            (false, true) => Box::new(((ScenarioStories, LineStories), MultiStories)),
            (false, false) => Box::new((ScenarioStories, MultiStories)),
        };
        let sc = &*sc;
        let infos: Vec<i32> = setup["infos"].as_array().map_or(Vec::new(), |a| a.iter().map(int).collect());
        for r in &recs {
            let kind = r["k"].as_str().unwrap();
            if !matches!(kind, "init" | "tick" | "check" | "staged") {
                continue;
            }
            let (pre, post) = (&r["pre"], &r["post"]);
            let mut st = load(&pre["s"]);
            st.values.clone_from(&values);
            st.fleet_data.clone_from(&fleet_data);
            st.building_data.clone_from(&building_data);
            let mut w = W::new(r, &tables, &infos, &workers);
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
                        // The flag and its payload are in `pre` (written right before the call).
                        "flag" => st.check(true, sc, &mut w),
                        "win" => st.win(int(&a[0]), int(&a[1]) != 0, &mut w),
                        "spwfinished" => {
                            let list: Vec<story::Participant> = a
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|p| story::Participant { owner: int(&p[0]), slot_id: int(&p[1]), fleet_id: int(&p[1]) })
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
            let won = w.events.iter().filter(|e| **e == StoryEvent::Won).count() as i32;
            let died: Vec<i32> = w.events.iter().filter_map(|e| if let StoryEvent::EmpireDies(x) = e { Some(*x) } else { None }).collect();
            stats.won += won as usize;
            stats.died += died.len();
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
            bad.append(&mut w.bad);
            if w.next < w.acts.len() {
                bad.push(format!("exe calls the port didn't make: {:?}", &w.acts[w.next..]));
            }
            if !bad.is_empty() {
                failures.push(format!("{} seq {} {kind} {}: {}", path.display(), r["seq"], r["call"], bad.join("; ")));
            }
        }
    }
    eprintln!(
        "campaign replay: {} records {:?}; stories started {:?}, ended {:?}; exe calls {:?}; {} counters fired, \
         {} stream-5 draws, {} MEGNYERES, {} MEGHALAS, events not checked {:?}",
        stats.records, stats.kinds, stats.started, stats.ended, stats.acts, stats.counters_fired, stats.draws, stats.won,
        stats.died, stats.unchecked
    );
    assert!(failures.is_empty(), "{} mismatches, first: {}", failures.len(), failures[..failures.len().min(8)].join("\n"));
}
