//! Replays diplomacy recorded from the running GOG `ig2.exe` (tools/trace/run_diplomacy.py,
//! `re/rules/diplomacy.md`): every `FRISSIT_Viszony` (exe 0x50df60) and `MI_DIPLOMACY` (0x4c7060) call of
//! `diplo-*.jsonl`, plain and fuzzed, with the libc `rand()` draws made inside each call; and the staged
//! human-side calls of `--mode human`: `FeldolgozAjanlat` (0x4c1240: the human's messages and deals, AI
//! messages), `FeldolgozValasz` (0x4c1480: the human's answers and counter-proposals) and
//! `FRISSIT_Diplomacy` (0x4c0f90: the auto-cancel), whose deals run through the colony-side port
//! ([`ColonyDeals`]: money, planet transfers, techs).
//!
//! A trace holds byte regions: a full dump, then per record the changes made before the call (other
//! systems, the fuzzer) and by the call. The replay rebuilds the state, runs the port, writes its result
//! back into the raw regions and compares them byte for byte with the recorded "after" state (the colony
//! world, money and the contact list are compared as parsed records). The port's libc `rand()` source
//! returns the recorded draws in order; their number must match.
//! Traces hold game state, so they stay in `_local/` (never committed). The rule tables come from the
//! user's exe. The test skips unless `IG2_INSTALL_DIR` and the traces exist.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::colony::{Building, BuildingKind, ColonyRules, EconomyContext, Planet};
use ig2_sim::colony_side::{ColonyWorld, SideRules, TechTable, Treasury, EMPIRES, PLANET_SLOTS};
use ig2_sim::diplomacy::{
    Answer, ColonyDeals, DiploEvent, DiploPlanet, Diplomacy, DiplomacyContext, DiplomacyRules, Letter, Message, PlayerSlot,
    Relation, World, LAST_CONTACT_CELLS,
};
use ig2_sim::fleet::{Fleet, Starmap, StarmapContext, Waypoint};
use ig2_sim::research::{Research, ResearchContext, ResearchRules};
use ig2_sim::rng::Streams;
use serde_json::Value;

fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with(prefix) && f.ends_with(".jsonl")));
    v.sort();
    v
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn f64_at(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn put(b: &mut [u8], o: usize, v: &[u8]) {
    b[o..o + v.len()].copy_from_slice(v);
}

type Regions = BTreeMap<String, Vec<u8>>;

fn apply(state: &mut Regions, diff: &Value) {
    for (name, d) in diff.as_object().unwrap() {
        let r = state.entry(name.clone()).or_default();
        r.resize(d["len"].as_u64().unwrap() as usize, 0);
        for run in d["runs"].as_array().unwrap() {
            let (o, bytes) = (run[0].as_u64().unwrap() as usize, unhex(run[1].as_str().unwrap()));
            put(r, o, &bytes);
        }
    }
}

fn install() -> Option<Install> {
    Install::from_env(Lang::En).map(|i| i.expect("valid install"))
}

const SLOT: usize = 0xc4;

fn f32s(b: &[u8]) -> [f32; EMPIRES] {
    std::array::from_fn(|e| f32_at(b, 4 * e))
}
fn i32s(b: &[u8]) -> [i32; EMPIRES] {
    std::array::from_fn(|e| i32_at(b, 4 * e))
}
fn flags(b: &[u8]) -> [bool; EMPIRES] {
    std::array::from_fn(|e| b[e] != 0)
}

fn message(b: &[u8], o: usize) -> Message {
    Message { cat: i32_at(b, o), sub: i32_at(b, o + 4), value: i32_at(b, o + 8), sub2: i32_at(b, o + 12), value2: i32_at(b, o + 16) }
}

/// An answer record (spec §10.1) at `o`.
fn answer(b: &[u8], o: usize) -> Answer {
    let msg = Message { cat: i32_at(b, o + 8), ..message(b, o + 0x10) };
    Answer {
        from: i32_at(b, o),
        to: i32_at(b, o + 4),
        msg: Message { sub: i32_at(b, o + 0x14), value: i32_at(b, o + 0x18), sub2: i32_at(b, o + 0x1c), value2: i32_at(b, o + 0x20), ..msg },
        counter: b[o + 0x10] != 0,
        reply: b[o + 0x11] != 0,
        yes: b[o + 0x12] != 0,
    }
}

fn slot(b: &[u8], i: usize) -> PlayerSlot {
    let o = i * SLOT;
    PlayerSlot {
        empire: i32_at(b, o),
        screen: b[o + 0x0c],
        open: b[o + 0x14],
        asked: b[o + 0x16],
        waiting: b[o + 0x17],
        answered: b[o + 0x18],
        countered: b[o + 0x2c],
        letter: Letter { from: i32_at(b, o + 0x38), to: i32_at(b, o + 0x3c), declaration: b[o + 0x30] != 0, msg: message(b, o + 0x40) },
        cancelled: b[o + 0x58],
        answer: answer(b, o + 0x64),
        counter_answer: answer(b, o + 0x94),
    }
}

/// Slot −1 (empires without a slot, exe 0xA41DD4) lies over `AktDipUzenet` (0xA41E14 = slot −1 + 0x40) and
/// `HanyevesAzUralkodo` (0xA41E48 = + 0x74): its message and answer copies alias them (not modelled, only
/// texts read them), so they're left out of the comparison.
fn unaliased(mut s: PlayerSlot) -> PlayerSlot {
    s.letter.msg = Message::default();
    s.cancelled = 0;
    s.answer = Answer::default();
    s.counter_answer = Answer::default();
    s
}

fn model(s: &Regions) -> Diplomacy {
    let v = &s["visz"];
    let players = &s["players"];
    let mut d = Diplomacy {
        relations: (0..EMPIRES)
            .map(|a| std::array::from_fn(|b| Relation::from_bytes(&v[a * 0x2d2 + b * Relation::SIZE..])))
            .collect(),
        power: f32s(&s["ero"]),
        space_power: f32s(&s["urero"]),
        planets: i32s(&s["hany"]),
        countdown: i32s(&s["vissza"]),
        danger: i32s(&s["veszely"]),
        last_contact: (0..LAST_CONTACT_CELLS).map(|i| i32_at(&s["mikor"], 4 * i)).collect(),
        known_since: std::array::from_fn(|a| std::array::from_fn(|b| i32_at(&s["miota"], 4 * (a * EMPIRES + b)))),
        players: (1..players.len() / SLOT).map(|i| slot(players, i)).collect(),
        ghost: slot(players, 0),
        ..Default::default()
    };
    if let (Some(kor), Some(udv), Some(nap), Some(st)) = (s.get("kor"), s.get("udv"), s.get("dipnap"), s.get("dipstart")) {
        d.ruler_age = i32s(kor);
        d.greeted = udv[..EMPIRES].try_into().unwrap();
        d.last_day = i32_at(nap, 0);
        d.started = st[0] != 0;
    }
    d
}

fn context(s: &Regions, g: &Value) -> DiplomacyContext {
    let gi = |k: &str| g[k].as_i64().unwrap_or(0);
    DiplomacyContext {
        program_players: gi("pp") as usize,
        program_players2: gi("pp2") as usize,
        alive: flags(&s["el"]),
        ai: flags(&s["mizik"]),
        may_attack: flags(&s["tamadhat"]),
        human: std::array::from_fn(|e| s["ki"][e] == 1),
        race: s["faj"][..EMPIRES].try_into().unwrap(),
        human_empire: gi("en") as usize,
        multiplayer: gi("mp") != 0,
        debug: gi("debug") != 0,
        enabled: gi("enabled") != 0,
        ai_off: gi("aioff") != 0,
        story_quiet: gi("quiet") != 0,
        spare_human: std::array::from_fn(|e| i32_at(&s["spare"], 4 * e) != 0),
        difficulty: gi("diff") as i32,
        tutorial: gi("tut") != 0,
        forced_answer: gi("forced") as i32,
        main_fleet: i32s(&s["mifo"]),
        month: gi("honap") as i32,
        day: gi("nap") as i32,
        kidnap: s.get("kidnap").map_or([0; 18], |k| std::array::from_fn(|i| i32_at(k, 4 * i))),
        all_techs: gi("alltechs") != 0,
    }
}

fn planets(s: &Regions) -> Vec<DiploPlanet> {
    let (b, erod) = (&s["planets"], &s["erod"]);
    (0..b.len() / 0x188)
        .map(|p| {
            let r = &b[p * 0x188..(p + 1) * 0x188];
            DiploPlanet {
                owner: r[0x20],
                x: f32_at(r, 0x18),
                y: f32_at(r, 0x1c),
                population: i32_at(r, 0x28),
                defence: i32_at(r, 0x5c),
                ship_capacity: i32_at(r, 0x70),
                vehicle_capacity: i32_at(r, 0x74),
                fortification: i32_at(erod, 4 * p),
                visible: std::array::from_fn(|e| i16_at(r, 0xd8 + 2 * e)),
            }
        })
        .collect()
}

fn starmap(s: &Regions) -> Starmap {
    let f = &s["fleets"];
    let mut m = Starmap {
        fleets: f.chunks(0x80).map(Fleet::from_record).collect(),
        waypoints: s.get("waypoints").map_or(&[][..], |w| &w[..]).chunks(12).map(|w| Waypoint { x: f32_at(w, 0), y: f32_at(w, 4), next: i16_at(w, 8) }).collect(),
        colony_ship_pool: i32s(&s["cpool"]),
        terraformer_pool: i32s(&s["tpool"]),
        ..Default::default()
    };
    for a in 0..EMPIRES {
        for b in 0..EMPIRES {
            m.met[a][b] = s["ismeri"][a * EMPIRES + b];
        }
    }
    m
}

// ---------------------------------------------------------------- the colony world of a human-side record

/// A planet record (0x188 bytes) with its building slots (`colony-economy.md` §2).
fn colony_planet(r: &[u8], bld: &[u8], unrest: i32) -> Planet {
    let mut p = Planet {
        x: f32_at(r, 0x18),
        y: f32_at(r, 0x1c),
        owner: r[0x20],
        race: r[0x21],
        surface: r[0x22],
        colony_map: r[0x23],
        richness: i32_at(r, 0x24),
        population: i32_at(r, 0x28),
        population_change: i32_at(r, 0x2c),
        morale: i32_at(r, 0x30),
        tax_level: r[0x34],
        gdp_acc: i32_at(r, 0x38),
        gdp_last: i32_at(r, 0x3c),
        tax_acc: i32_at(r, 0x40),
        tax_last: i32_at(r, 0x44),
        energy_produced: i32_at(r, 0x48),
        energy_demand: i32_at(r, 0x4c),
        workers_demand: i32_at(r, 0x50),
        field_54: u16::from_le_bytes([r[0x54], r[0x55]]),
        field_58: i32_at(r, 0x58),
        field_5c: i32_at(r, 0x5c),
        protected: r[0x64] != 0,
        autobuild_counter: i32_at(r, 0x68),
        autobuild_mode: r[0x6c],
        autobuild_aggr: r[0x6d],
        ship_capacity: i32_at(r, 0x70),
        vehicle_capacity: i32_at(r, 0x74),
        production: r[0x78..0xb8].try_into().unwrap(),
        epidemic: i16_at(r, 0xb8),
        field_114: i32_at(r, 0x114),
        morale_penalty: i32_at(r, 0x174),
        morale_penalty_rate: i32_at(r, 0x178),
        loyal: r[0x186] != 0,
        ai_proof: r[0x187] != 0,
        unrest,
        ..Default::default()
    };
    p.spies.copy_from_slice(&r[0xc4..0xc4 + 19]);
    for e in 0..19 {
        p.visibility[e] = i16_at(r, 0xd8 + 2 * e);
    }
    for k in 1..p.problems.len() {
        p.problems[k] = i32_at(r, 0x118 + 4 * k);
    }
    for k in 0..5 {
        p.events[k] = i16_at(r, 0x16a + 2 * k);
    }
    p.buildings = bld
        .chunks(32)
        .map(|s| Building {
            kind: s[0],
            owner: s[1],
            damage: i32_at(s, 4),
            work_left: u16::from_le_bytes([s[8], s[9]]),
            planet: s[0xa],
            x: f32_at(s, 0xc),
            y: f32_at(s, 0x10),
            yield_pct: s[0x18],
            flags: s[0x19],
            shown_energy: i16_at(s, 0x1a),
            shown_workers: i16_at(s, 0x1c),
            efficiency: i16_at(s, 0x1e),
        })
        .collect();
    p
}

fn colony_world(s: &Regions, g: &Value) -> ColonyWorld {
    let raw = &s["planets"];
    let n = raw.len() / 0x188;
    let per = |k: &str| -> Vec<i32> {
        let mut v = vec![0; PLANET_SLOTS.max(n)];
        for (i, x) in s[k].chunks(4).enumerate().take(v.len()) {
            v[i] = i32::from_le_bytes(x.try_into().unwrap());
        }
        v
    };
    let (nb, lazad) = (per("nb"), per("lazad"));
    let mut planets = vec![Planet::default(); n];
    let mut o = 0;
    for p in 1..n {
        let len = nb[p].max(0) as usize * 0x20;
        planets[p] = colony_planet(&raw[p * 0x188..(p + 1) * 0x188], &s["bld"][o..o + len], lazad[p]);
        o += len;
    }
    let mut w = ColonyWorld {
        planets,
        occupied: per("hanyep"),
        needed_morale: per("moral"),
        last_near_fleet: per("utolso"),
        all_techs: g["alltechs"].as_i64().unwrap_or(0) != 0,
        difficulty: g["diff"].as_i64().unwrap_or(0).max(0) as usize,
        ..Default::default()
    };
    for e in 0..EMPIRES {
        let six = |k: &str| -> [f64; 6] { std::array::from_fn(|i| f64_at(&s[k], e * 48 + 8 * i)) };
        w.money[e] = Treasury { total: f64_at(&s["money"], 8 * e), pools: six("pools"), reserve: six("hatar"), share: six("szorzo") };
    }
    w
}

struct HumanRules {
    side: SideRules,
    research: ResearchRules,
}

fn human_rules(r: &ig2_rules::Rules) -> HumanRules {
    let colony = ColonyRules {
        buildings: r
            .buildings
            .iter()
            .map(|b| BuildingKind { tech: b.tech, priority: b.priority, build_work: b.build_work, max_hp: b.max_hp, ..Default::default() })
            .collect(),
        race_production: r.race_production_mult.clone(),
        ..Default::default()
    };
    HumanRules {
        side: SideRules {
            colony,
            race_research_time: r.race_research_time_mult.clone(),
            difficulty_money: r.difficulty_money_mult.clone(),
            production_default: r.production_default.clone().try_into().unwrap(),
            ..Default::default()
        },
        research: ResearchRules {
            race_time: r.race_research_time_mult.clone(),
            race_money: r.race_research_money_mult.clone(),
            ..Default::default()
        },
    }
}

/// Write the port's state back into the raw regions.
fn write_back(s: &mut Regions, d: &Diplomacy, ctx: &DiplomacyContext, map: &Starmap, tech: &TechTable, rng: &Streams) {
    let v = s.get_mut("visz").unwrap();
    for a in 0..EMPIRES {
        for b in 0..EMPIRES {
            put(v, a * 0x2d2 + b * Relation::SIZE, &d.relations[a][b].to_bytes());
        }
    }
    for (name, xs) in [("ero", d.power), ("urero", d.space_power)] {
        let r = s.get_mut(name).unwrap();
        for (e, x) in xs.iter().enumerate() {
            put(r, 4 * e, &x.to_le_bytes());
        }
    }
    for (name, xs) in [("hany", d.planets), ("vissza", d.countdown), ("veszely", d.danger), ("cpool", map.colony_ship_pool), ("tpool", map.terraformer_pool)] {
        let r = s.get_mut(name).unwrap();
        for (e, x) in xs.iter().enumerate() {
            put(r, 4 * e, &x.to_le_bytes());
        }
    }
    let r = s.get_mut("mikor").unwrap();
    for (i, x) in d.last_contact.iter().enumerate() {
        put(r, 4 * i, &x.to_le_bytes());
    }
    let r = s.get_mut("miota").unwrap();
    for a in 0..EMPIRES {
        for b in 0..EMPIRES {
            put(r, 4 * (a * EMPIRES + b), &d.known_since[a][b].to_le_bytes());
        }
    }
    let r = s.get_mut("ismeri").unwrap();
    for a in 0..EMPIRES {
        for b in 0..EMPIRES {
            r[a * EMPIRES + b] = map.met[a][b];
        }
    }
    for (b, &alive) in s.get_mut("el").unwrap().iter_mut().zip(&ctx.alive) {
        if !alive {
            *b = 0;
        }
    }
    let r = s.get_mut("fleets").unwrap();
    for (f, fl) in map.fleets.iter().enumerate() {
        let o = f * 0x80;
        put(r, o + 0x40, &fl.order.to_le_bytes());
        put(r, o + 0x42, &fl.kind.to_le_bytes());
        put(r, o + 0x44, &fl.tx.to_le_bytes());
        put(r, o + 0x48, &fl.ty.to_le_bytes());
        put(r, o + 0x50, &fl.first_waypoint.to_le_bytes());
        put(r, o + 0x52, &fl.flags.to_le_bytes());
    }
    let r = s.entry("waypoints".into()).or_default();
    for (i, w) in map.waypoints.iter().enumerate() {
        put(r, 12 * i, &w.x.to_le_bytes());
        put(r, 12 * i + 4, &w.y.to_le_bytes());
        put(r, 12 * i + 8, &w.next.to_le_bytes());
    }
    s.insert("techs".into(), tech.bytes.clone());
    let r = s.get_mut("seeds").unwrap();
    for i in 0..21 {
        put(r, 4 * i, &rng.state(i).to_le_bytes());
    }
    if let Some(r) = s.get_mut("kidnap") {
        for (i, x) in ctx.kidnap.iter().enumerate() {
            put(r, 4 * i, &x.to_le_bytes());
        }
    }
}

const COMPARED: &[&str] = &[
    "visz", "ero", "urero", "hany", "vissza", "veszely", "mikor", "ismeri", "miota", "el", "fleets", "waypoints", "cpool",
    "tpool", "techs", "seeds", "kidnap",
];

#[derive(Default)]
struct Stats {
    calls: BTreeMap<String, usize>,
    messages: usize,
    answers: usize,
    wars: usize,
    deaths: usize,
    draws: usize,
    accepted_deals: BTreeMap<(i32, i32, i32), usize>,
    cancels: usize,
    transfers: usize,
    kinds: BTreeMap<(i32, i32, Option<bool>), usize>,
}

#[test]
fn diplomacy_matches_every_traced_call() {
    let paths = traces("diplo-");
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    if paths.is_empty() {
        eprintln!("no diplomacy trace in _local/traces; run tools/trace/run_diplomacy.py");
        return;
    }
    let r = ig2_rules::Rules::from_install(&inst).expect("catalogued exe");
    let rules = DiplomacyRules {
        surface_affinity: r.race_surface_affinity.concat(),
        research_money: r.race_research_money_mult.clone(),
        research_time: r.race_research_time_mult.clone(),
        ..DiplomacyRules::from_table(&r.declare_war_unreliability)
    };
    let hr = human_rules(&r);
    let mut st = Stats::default();
    let mut bad = Vec::new();
    for p in paths {
        let mut state = Regions::new();
        for line in std::fs::read_to_string(&p).unwrap().lines() {
            let ev: Value = serde_json::from_str(line).unwrap();
            let k = ev["k"].as_str().unwrap();
            if k == "init" {
                for (n, h) in ev["regions"].as_object().unwrap() {
                    state.insert(n.clone(), unhex(h.as_str().unwrap()));
                }
                continue;
            }
            if !matches!(k, "visz" | "mi" | "ajanlat" | "valasz" | "frissit") {
                continue;
            }
            apply(&mut state, &ev["pre"]);
            let before = state.clone();
            let mut after = state.clone();
            apply(&mut after, &ev["post"]);
            let what = format!("{} {k} seq {}", p.file_name().unwrap().to_string_lossy(), ev["seq"]);
            let Some(recorded) = ev["rand"].as_array() else {
                eprintln!("{what}: rand() draws lost, skipped");
                state = after;
                continue;
            };
            let recorded: Vec<i32> = recorded.iter().map(|x| x.as_i64().unwrap() as i32).collect();
            let human = matches!(k, "ajanlat" | "valasz" | "frissit");

            let mut d = model(&before);
            let mut ctx = context(&before, &ev["g"]);
            let mut pl = planets(&before);
            let mut map = starmap(&before);
            let star = StarmapContext { human_controlled: ctx.human, ..Default::default() };
            let mut tech = TechTable { bytes: before["techs"].clone() };
            let mut rng = Streams::new(21);
            for i in 0..21 {
                rng.set_state(i, i32_at(&before["seeds"], 4 * i) as u32);
            }
            let used = RefCell::new(0usize);
            let mut libc = || {
                let mut n = used.borrow_mut();
                *n += 1;
                recorded.get(*n - 1).copied().unwrap_or(0)
            };
            let mut world = human.then(|| colony_world(&before, &ev["g"]));
            let econ = EconomyContext {
                multiplayer: ctx.multiplayer,
                human_empire: ctx.human_empire as u8,
                human_controlled: ctx.human,
                empire_race: ctx.race,
                players: ctx.program_players2 as i32,
                ..Default::default()
            };
            let mut research = Research::default();
            if let Some(kutat) = before.get("kutat") {
                research.current = i32s(kutat);
            }
            let rctx = ResearchContext {
                human_empire: ctx.human_empire as u8,
                human_controlled: ctx.human,
                race: ctx.race,
                players: ctx.program_players2 as i32,
                all_techs: ctx.all_techs,
                ..Default::default()
            };
            let args: Vec<i32> = ev["args"].as_array().map_or(Vec::new(), |a| a.iter().map(|x| x.as_i64().unwrap() as i32).collect());
            let mut transfers = 0;
            let events = {
                let mut w = World {
                    ctx: &mut ctx,
                    planets: &mut pl,
                    starmap: &mut map,
                    star_ctx: &star,
                    tech: &mut tech,
                    rng: &mut rng,
                    rules: &rules,
                    libc: &mut libc,
                    events: Vec::new(),
                };
                match k {
                    "visz" => d.daily(&mut w),
                    "mi" => d.ai_messages(&mut w),
                    _ => {
                        let mut deals = ColonyDeals {
                            world: world.as_mut().unwrap(),
                            rules: &hr.side,
                            ctx: &econ,
                            research: &mut research,
                            research_rules: &hr.research,
                            research_ctx: &rctx,
                            events: Vec::new(),
                            research_events: Vec::new(),
                        };
                        let msg = |o: usize| Message { cat: args[o], sub: args[o + 1], value: args[o + 2], sub2: args[o + 3], value2: args[o + 4] };
                        match k {
                            "ajanlat" => {
                                let letter = Letter { from: args[3], to: args[4], declaration: args[1] & 0xff != 0, msg: msg(5) };
                                d.receive_message(&mut w, &mut deals, letter)
                            }
                            "valasz" => {
                                let flags = args[5];
                                let m = Message { cat: args[3], sub: args[6], value: args[7], sub2: args[8], value2: args[9] };
                                let a = Answer { from: args[1], to: args[2], msg: m, counter: flags & 0xff != 0, reply: flags >> 8 & 0xff != 0, yes: flags >> 16 & 0xff != 0 };
                                d.receive_answer(&mut w, &mut deals, a)
                            }
                            _ => d.tick(&mut w),
                        }
                        transfers = deals.events.iter().filter(|e| matches!(e, ig2_sim::colony_side::SideEvent::GarrisonLost { .. })).count();
                    }
                }
                std::mem::take(&mut w.events)
            };
            let draws = *used.borrow();
            *st.calls.entry(k.to_string()).or_default() += 1;
            st.draws += draws;
            st.transfers += transfers;
            for e in &events {
                match e {
                    DiploEvent::Message { accepted, msg, .. } => {
                        *st.kinds.entry((msg.cat, msg.sub, *accepted)).or_default() += 1;
                        st.messages += 1;
                        st.answers += accepted.is_some() as usize;
                    }
                    DiploEvent::Answered(a) if a.yes && (1..=3).contains(&a.msg.cat) => {
                        *st.accepted_deals.entry((a.msg.cat, a.msg.sub, a.msg.sub2)).or_default() += 1;
                    }
                    DiploEvent::War { .. } => st.wars += 1,
                    DiploEvent::EmpireDied { .. } => st.deaths += 1,
                    DiploEvent::AutoCancelled { .. } => st.cancels += 1,
                    _ => {}
                }
            }

            let mut ours = before.clone();
            write_back(&mut ours, &d, &ctx, &map, &tech, &rng);
            for &n in COMPARED {
                let (Some(a), Some(b)) = (ours.get(n), after.get(n)) else { continue };
                if a != b {
                    let i = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)).unwrap();
                    bad.push(format!("{what}: {n} len {}/{} differs at {i:#x}: ours {:?} exe {:?}", a.len(), b.len(), a.get(i), b.get(i)));
                }
            }
            let theirs = &after["players"];
            let exe_slots: Vec<PlayerSlot> = (1..theirs.len() / SLOT).map(|i| slot(theirs, i)).collect();
            if exe_slots != d.players {
                let i = (0..exe_slots.len().max(d.players.len())).find(|&i| exe_slots.get(i) != d.players.get(i)).unwrap();
                bad.push(format!("{what}: contact slot {i}: ours {:?} exe {:?}", d.players.get(i), exe_slots.get(i)));
            }
            if unaliased(slot(theirs, 0)) != unaliased(d.ghost) {
                bad.push(format!("{what}: slot -1: ours {:?} exe {:?}", d.ghost, slot(theirs, 0)));
            }
            if k == "frissit" {
                let ages = model(&after);
                if (ages.ruler_age, ages.greeted, ages.last_day) != (d.ruler_age, d.greeted, d.last_day) {
                    bad.push(format!("{what}: ruler ages / greetings / day differ"));
                }
            }
            if let Some(ours) = &world {
                let theirs = colony_world(&after, &ev["g"]);
                for p in 1..ours.planets.len() {
                    if ours.planets[p] != theirs.planets[p] {
                        bad.push(format!("{what}: planet {p}: ours {:?}\n exe {:?}", ours.planets[p], theirs.planets[p]));
                    }
                }
                if ours.money != theirs.money {
                    let e = (0..EMPIRES).find(|&e| ours.money[e] != theirs.money[e]).unwrap();
                    bad.push(format!("{what}: money of {e}: ours {:?} exe {:?}", ours.money[e], theirs.money[e]));
                }
                if ours.last_near_fleet != theirs.last_near_fleet {
                    bad.push(format!("{what}: BolygoUtolsoKozeliFlotta differs"));
                }
                if let Some(kutat) = after.get("kutat") {
                    if research.current != i32s(kutat) {
                        bad.push(format!("{what}: research ours {:?} exe {:?}", research.current, i32s(kutat)));
                    }
                }
            }
            if draws != recorded.len() {
                bad.push(format!("{what}: rand() draws ours {draws} exe {}", recorded.len()));
            }
            // Continue from the recorded state either way.
            state = after;
        }
    }
    eprintln!(
        "{:?} calls, {} messages ({} answered by the AI), {} wars, {} deaths, {} rand() draws, {} auto-cancels, {} planet transfers",
        st.calls, st.messages, st.answers, st.wars, st.deaths, st.draws, st.cancels, st.transfers
    );
    eprintln!("messages by (cat, sub, accepted): {:?}", st.kinds);
    eprintln!("accepted deals by (cat, sub, sub2): {:?}", st.accepted_deals);
    if !bad.is_empty() {
        for b in bad.iter().take(30) {
            eprintln!("{b}");
        }
        panic!("{} mismatches", bad.len());
    }
}
