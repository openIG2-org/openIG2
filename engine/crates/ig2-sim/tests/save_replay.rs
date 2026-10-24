//! Original saves as sim state (`ig2_sim::save`, spec `re/formats/save.md`), and a replay from a save against the
//! exe: tools/trace/run_replay.py loads `_local/saves/quick-1.sav` in the GOG `ig2.exe` with its own `LoadSave`
//! and writes a save at every sim tick boundary (`_local/traces/replay-*/<k>.sav` + `replay.jsonl`); the port
//! loads the same save, runs the same ticks and must produce the same saves.
//!
//! Saves and traces hold game state, so they stay in `_local/` (never committed; `IG2_SAVE_DIR` / `IG2_TRACE_DIR`
//! point elsewhere). The rule tables come from the user's exe. The tests skip without them.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::path::PathBuf;

use ig2_formats::save::SaveGame;
use ig2_formats::text::{self, Lang};
use ig2_formats::{colony_map, misc_text, Install};
use ig2_sim::ai::{AiEvent, AiRules, PodRule};
use ig2_sim::colony::{BuildingKind, ColonyRules, Curve};
use ig2_sim::colony_side::{tech_strength, AutobuildMode, AutobuildRules, SideRules};
use ig2_sim::diplomacy::DiplomacyRules;
use ig2_sim::fleet::StarmapEvent;
use ig2_sim::game::{BattleEvent, ColonyOrder, DesignOrder, Fresh, Game, GameEvent, GameRules, InfoOrder, Libc, MessageOrder, ResearchOrder, StarmapOrder};
use ig2_sim::messages::{cstr, Texts, ENTRY};
use ig2_sim::placement::{PlacementRules, Terrain};
use ig2_sim::radar::RadarRules;
use ig2_sim::research::ResearchRules;
use ig2_sim::save::{group_chunks, modelled, va, SaveState};
use ig2_sim::ground::{GroundMap, GroundRules};
use ig2_sim::spacewar::{DesignRules, GunRules, Hull};
use ig2_sim::spy::{MissionRule, Spy, SpyRules};
use ig2_sim::story::{MessageText, StoryEvent};
use ig2_sim::trade::{Delivery, DELIVERY};
use serde_json::Value;

fn local(var: &str, sub: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local").join(sub))
}

fn install() -> Option<Install> {
    Install::from_env(Lang::En).map(|i| i.expect("valid install"))
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn load(p: &std::path::Path) -> SaveGame {
    SaveGame::parse(&std::fs::read(p).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The differing chunk groups of two grouped saves (first differing byte of each).
fn diff(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut out = Vec::new();
    for n in a.keys().chain(b.keys().filter(|k| !a.contains_key(*k))) {
        let (x, y) = (a.get(n).map_or(&[][..], |v| &v[..]), b.get(n).map_or(&[][..], |v| &v[..]));
        if x != y {
            let at: Vec<usize> = (0..x.len().max(y.len())).filter(|&i| x.get(i) != y.get(i)).collect();
            let i = at[0];
            let more: Vec<String> = at.iter().skip(1).take(7).map(|&j| format!("{j:#x}: {:?}/{:?}", x.get(j), y.get(j))).collect();
            let word = |v: &[u8]| v.get(i & !3..(i & !3) + 4).map(|w| i32::from_le_bytes(w.try_into().unwrap()));
            out.push(format!("{n}: len {}/{} first diff at {i:#x}: {:?} / {:?} (word {:?} / {:?}; {} bytes; {})", x.len(), y.len(), x.get(i), y.get(i),
                word(x), word(y), at.len(), more.join(", ")));
        }
    }
    out
}

/// Every system's view decoded from the state and stored back changes nothing.
fn store_everything(s: &mut SaveState) {
    let v = s.colony();
    s.store_colony(&v);
    let v = s.starmap();
    s.store_starmap(&v);
    let v = s.combat();
    s.store_combat(&v);
    let v = s.spies();
    s.store_spies(&v);
    let v = s.diplomacy();
    s.store_diplomacy(&v);
    let v = s.research();
    s.store_research(&v);
    let v = s.ai();
    s.store_ai(&v);
    let v = s.story();
    s.store_story(&v);
    let v = s.rng();
    s.store_rng(&v);
    let v = s.kidnap();
    s.store_kidnap(&v);
    for p in 1..=200 {
        let f = s.take_ground_field(p);
        s.store_ground_field(p, f);
    }
}

#[test]
fn saves_round_trip_through_the_sim_state() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let rules = ig2_rules::Rules::from_install(&inst).expect("catalogued exe");
    let dir = local("IG2_SAVE_DIR", "saves");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    paths.retain(|p| p.extension().is_some_and(|x| x == "sav"));
    paths.sort();
    if paths.is_empty() {
        eprintln!("no saves in {}; skipping", dir.display());
        return;
    }
    let mut bad = Vec::new();
    for p in &paths {
        let save = load(p);
        let want = group_chunks(&save);
        let mut s = SaveState::from_save(&save, &rules.tech_defaults);
        for d in diff(&s.chunks(), &want) {
            bad.push(format!("{}: loaded: {d}", p.display()));
        }
        store_everything(&mut s);
        for d in diff(&s.chunks(), &want) {
            bad.push(format!("{}: decoded and stored: {d}", p.display()));
        }
    }
    eprintln!("{} saves", paths.len());
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad[..bad.len().min(30)].join("\n"));
}

// ------------------------------------------------------------------------------------------------ replay

struct Recording {
    dir: PathBuf,
    /// One per save `k` (`{"k": "save"}`).
    events: Vec<Value>,
    /// `--stage`: the purchases made between `pre.sav` and save 0 (`{"k": "buy", "e", "msg", "accepted"}`).
    buys: Vec<Value>,
    /// `--orders` / `--colony` / `--research` / `--info`: starmap / colony / research / info messages staged before
    /// save `before` (`{"k": "sorder" | "corder" | "rorder" | "iorder" | "dorder", "before", "type", "vals"}`); `--answers`:
    /// the inbox steps (`{"k": "morder", "before", "op", "vals"}`).
    orders: Vec<Value>,
}

fn recordings() -> Vec<Recording> {
    let dir = local("IG2_TRACE_DIR", "traces");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    // `IG2_REPLAY_SKIP`: recordings whose name contains it are left out (not ported yet).
    let skip = std::env::var("IG2_REPLAY_SKIP").ok().filter(|s| !s.is_empty());
    // `IG2_REPLAY_ONLY`: only the recordings whose name contains it.
    let only = std::env::var("IG2_REPLAY_ONLY").ok().filter(|s| !s.is_empty());
    v.retain(|p| {
        p.is_dir()
            && p.file_name().and_then(|f| f.to_str()).is_some_and(|f| {
                f.starts_with("replay-") && skip.as_ref().is_none_or(|s| !f.contains(s.as_str())) && only.as_ref().is_none_or(|s| f.contains(s.as_str()))
            })
    });
    v.sort();
    v.into_iter()
        .filter_map(|dir| {
            let text = std::fs::read_to_string(dir.join("replay.jsonl")).ok()?;
            let all: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
            let (events, rest): (Vec<Value>, Vec<Value>) = all.into_iter().partition(|e| e["k"] == "save");
            let buys = rest.iter().filter(|e| e["k"] == "buy").cloned().collect();
            let orders = rest.into_iter().filter(|e| ["sorder", "corder", "rorder", "iorder", "dorder", "morder"].iter().any(|k| e["k"] == *k)).collect();
            Some(Recording { dir, events, buys, orders })
        })
        .collect()
}

fn volatile(ev: &Value) -> Vec<(u32, Vec<u8>)> {
    ev["vol"]["mem"].as_array().unwrap().iter().map(|r| (r[1].as_u64().unwrap() as u32, unhex(r[2].as_str().unwrap()))).collect()
}

#[test]
fn exe_survey() {
    let Some(inst) = install() else { return };
    let rules = ig2_rules::Rules::from_install(&inst).expect("catalogued exe");
    for rec in recordings() {
        let first = load(&rec.dir.join("0.sav"));
        let s = SaveState::from_save(&first, &rules.tech_defaults);
        // The live tech table at load = defaults + the save's per-tech state, apart from the localised names.
        let live = volatile(&rec.events[0]).into_iter().find(|r| r.0 == va::TECHS).map(|r| r.1).unwrap();
        let ours = s.tech().bytes;
        let off: Vec<usize> = (0..live.len()).filter(|&i| live[i] != ours[i] && i % 0x114 >= 0x10).collect();
        eprintln!("tech table: {} bytes differ outside the names: {:?}", off.len(), off.iter().take(10).map(|i| (i / 0x114, i % 0x114)).collect::<Vec<_>>());
        let a = group_chunks(&first);
        let last = rec.events.len() - 1;
        let b = group_chunks(&load(&rec.dir.join(format!("{last}.sav"))));
        for d in diff(&a, &b) {
            eprintln!("0 -> {last}: {d}");
        }
    }
}

// ------------------------------------------------------------------------------------------------ rules

fn game_rules(inst: &Install) -> GameRules {
    let r = ig2_rules::Rules::from_install(inst).expect("catalogued exe");
    let colony = ColonyRules {
        buildings: r
            .buildings
            .iter()
            .map(|b| BuildingKind {
                tech: b.tech,
                priority: b.priority,
                build_work: b.build_work,
                max_hp: b.max_hp,
                energy: b.energy,
                workers: b.workers,
                output: b.output,
                scaled_effect: b.flags[0] & 1 != 0,
                passive_effect: b.flags[1] & 1 != 0,
            })
            .collect(),
        curves: r.problem_curves.iter().map(|c| Curve { a: c.a, b: c.b, c: c.c, d: c.d, rate: c.rate, weight: c.weight }).collect(),
        surface_growth: r.race_surface_affinity.iter().map(|v| v[..9].try_into().unwrap()).collect(),
        self_destruct_fear: r.race_fear_threshold.clone(),
        kidnap_morale: r.kidnap_morale.clone(),
        race_production: r.race_production_mult.clone(),
    };
    let mut modes = vec![AutobuildMode::default()];
    for m in 1..=10 {
        let raw = inst.read(&format!("dataraw/text/autobld{m}.txt")).expect("autobld file");
        let f = misc_text::parse_autobuild(&text::decode(&raw, inst.lang()));
        modes.push(AutobuildMode::from_entries(f.threshold, f.listed_priority, &f.entries, &r.autobuild_names));
    }
    let firepower: Vec<i32> = r.techs.iter().map(|t| t.firepower).collect();
    let tech_hp = tech_strength(&firepower, &r.techs.iter().map(|t| t.hp).collect::<Vec<_>>());
    let side = SideRules {
        colony,
        firepower,
        tech_hp,
        race_research_time: r.race_research_time_mult.clone(),
        difficulty_money: r.difficulty_money_mult.clone(),
        production_default: r.production_default.clone().try_into().unwrap(),
        autobuild: AutobuildRules { problem: r.autobuild_problem.clone(), min: r.autobuild_min.clone(), max: r.autobuild_max.clone(), modes },
    };
    let raw = inst.read("dataraw/colony/terepleiro.txt").expect("terepleiro.txt");
    let surfaces = colony_map::parse_surfaces(&text::decode(&raw, inst.lang())).unwrap();
    let backgrounds = colony_map::parse_backgrounds(&text::decode(&raw, inst.lang())).unwrap();
    let terrains = surfaces
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let m = colony_map::ColonyMap::parse(&inst.read(&format!("dataraw/colony/gen_maps/{n}.map")).unwrap()).unwrap();
            (Terrain { mask: m.mask_a, kind: s.kind as u8, start: s.ai_build_start }, m.heights.to_vec(), -(m.base - 200.0))
        })
        .collect::<Vec<_>>();
    let ground_maps = terrains
        .iter()
        .map(|(t, h, base)| GroundMap { terrain: t.clone(), heights: h.clone(), base: *base, footprints: r.building_footprints.clone() })
        .collect();
    let terrains = terrains.into_iter().map(|t| t.0).collect();
    let design = DesignRules {
        hulls: r.hulls.iter().map(|h| Hull { class: h.class, flags: h.flags, tech: h.tech }).collect(),
        tech_price: r.techs.iter().map(|t| t.price).collect(),
        tech_time: r.techs.iter().map(|t| t.build_time).collect(),
        tech_hp: r.techs.iter().map(|t| t.hp).collect(),
        tech_capacity: r.techs.iter().map(|t| t.capacity).collect(),
        firepower: r.techs.iter().map(|t| t.firepower).collect(),
        race_pod_table: r.race_pod_table.clone(),
        pod_slot_types: r.pod_slots.iter().map(|t| t.iter().map(|h| h.iter().map(|s| s.pod_type).collect()).collect()).collect(),
        tank_speeds: r.tank_speeds.clone(),
    };
    GameRules {
        side,
        placement: PlacementRules { terrains, footprints: r.building_footprints.clone() },
        research: ResearchRules {
            race_time: r.race_research_time_mult.clone(),
            race_money: r.race_research_money_mult.clone(),
            difficulty_speed: r.difficulty_development_mult.clone(),
            difficulty_money: r.difficulty_money_mult.clone(),
        },
        diplomacy: DiplomacyRules {
            surface_affinity: r.race_surface_affinity.concat(),
            research_money: r.race_research_money_mult.clone(),
            research_time: r.race_research_time_mult.clone(),
            ..DiplomacyRules::from_table(&r.declare_war_unreliability)
        },
        spy: SpyRules {
            missions: r
                .spy_missions
                .iter()
                .map(|m| MissionRule {
                    kind: m.kind,
                    xp: m.xp,
                    skill_floor: m.skill_floor,
                    skill_full: m.skill_full,
                    chance: m.chance,
                    needs_target: m.needs_target != 0,
                    story: m.story != 0,
                    no_countdown: m.no_countdown != 0,
                    days: m.days,
                    cost: m.cost,
                    min_rank: m.min_rank,
                })
                .collect(),
            outcome_state: r.spy_outcome_state.clone(),
            rank_points: r.spy_rank_points.clone(),
            templates: r.spy_templates.iter().map(|t| Spy::from_bytes(t)).collect(),
            rank_xp: r.spy_rank_xp.clone(),
            training_xp: r.spy_training_xp.clone(),
            names: r.spy_names.clone(),
            building_tech: r.buildings.iter().map(|b| b.tech).collect(),
            hire_cost_mult: r.spy_hire_cost_mult.clone(),
        },
        ai: AiRules {
            build_mode: r.ai_build_mode.clone(),
            build_min: r.ai_build_min.clone(),
            build_max: r.ai_build_max.clone(),
            production_share: r.race_ai_production_share.clone(),
            military_mode: r.race_ai_military_autobuild.clone(),
            training_cost: r.spy_training_cost.clone(),
            mission_cost_mult: r.spy_race_cost_mult.clone(),
            design: design.clone(),
            pod_slot_max: r.pod_slots.iter().map(|t| t.iter().map(|h| h.iter().map(|s| s.max_count).collect()).collect()).collect(),
            pods: r.pod_types.iter().map(|p| PodRule { id: p.id, techs: std::array::from_fn(|i| p.techs.get(i).copied().unwrap_or(0)) }).collect(),
            ground_priority: r.ai_ground_priority.clone(),
        },
        radar: RadarRules { ranges: r.radar_ranges[..6].try_into().unwrap(), planet_ranges: r.planet_radar_ranges[..8].try_into().unwrap(), wormhole_colours: r.wormhole_colours },
        difficulty_production: r.difficulty_production_mult.clone(),
        difficulty_money: r.difficulty_money_mult.clone(),
        messages: Texts::default(),
        guns: GunRules {
            building_tech: r.buildings.iter().map(|b| b.tech).collect(),
            firepower: r.techs.iter().map(|t| t.firepower).collect(),
            tech_hp: r.techs.iter().map(|t| t.hp).collect(),
        },
        ground: GroundRules {
            weapons: r.ground_weapons.clone(),
            hull_hp: r.ground_hull_hp.clone(),
            building_hp: r.buildings.iter().map(|b| b.max_hp).collect(),
            map_size: r.ground_map_size,
            designs: design.clone(),
            rt_weapons: r.ground_rt_weapons.clone(),
            fortress: r.ground_fortress_weapons.clone(),
            turn_speeds: r.ground_turn_speeds.clone(),
            armour: r.ground_armour.clone(),
            kind0_width: r.building_footprint0.0,
        },
        surface_weathers: surfaces.iter().map(|s| s.weathers.clone()).collect(),
        backgrounds: backgrounds.into_iter().map(|b| b.weathers).collect(),
        ground_maps,
        space: Default::default(),
        satellite_prices: r.satellite_prices.clone(),
        inbox: ig2_sim::messages::InboxRules::from_install(inst),
        story: ig2_sim::game::StoryRules::new(r.campaign_words.clone(), r.story_recruit_names.clone(), &r.design_templates, r.production_template.clone(), inst),
    }
}

/// The texts (`uzenet.txt`, the story table) in the language the recordings' exe ran in: the one whose texts begin the
/// messages on the recordings' scrolling lines (the remaster's language setting when none shows one).
fn message_texts(recs: &[Recording]) -> (Texts, Lang) {
    let mut shown: Vec<(i32, Vec<u8>)> = Vec::new();
    for rec in recs {
        let last = load(&rec.dir.join(format!("{}.sav", rec.events.len() - 1)));
        let main = last.chunk("ScrollUzenetek").unwrap();
        for e in main.chunks(ENTRY).skip(1) {
            shown.push((i32::from_le_bytes(e[0x38..0x3c].try_into().unwrap()), cstr(e).to_vec()));
        }
    }
    let texts = |lang| {
        let inst = Install::from_env(lang).and_then(|i| i.ok())?;
        let raw = inst.read("dataraw/text/uzenet.txt").ok()?;
        let story = |p| inst.read(p).unwrap_or_default();
        Some(Texts::parse(&raw).with_story(&story("dataraw/text/s_story.txt"), &story("dataraw/text/txt02.txt"), lang))
    };
    for lang in Lang::ALL {
        let Some(t) = texts(lang) else { continue };
        let fits = |(id, text): &(i32, Vec<u8>)| {
            t.format(*id, &[]).is_some_and(|f| text.starts_with(&f[..f.windows(2).position(|w| w[0] == b'%').unwrap_or(f.len())]))
        };
        if !shown.is_empty() && shown.iter().all(fits) {
            return (t, lang);
        }
    }
    // No line to tell by: the language the remaster runs in (the tutorial's planet name, `tutorial.md` §4).
    let lang = std::env::var_os("IG2_INSTALL_DIR").map_or(Lang::En, |d| ig2_formats::locale::exe_language(std::path::Path::new(&d)).0);
    (texts(lang).unwrap_or_default(), lang)
}

/// `_s(0x3b, n)` (`s_design.txt`) in `lang`: the names `DesignPoenosanElnevez` gives.
fn design_texts(lang: Lang) -> BTreeMap<i32, Vec<u8>> {
    let Some(inst) = Install::from_env(lang).and_then(|i| i.ok()) else { return BTreeMap::new() };
    let mut keyed = ig2_formats::strings::KeyedStrings::default();
    if let Ok(b) = inst.read("dataraw/text/s_design.txt") {
        keyed.add_file(0x3b, &text::decode(&b, lang));
    }
    keyed.table(0x3b).map(|(n, s)| (n as i32, lang.encoding().encode(s).0.into_owned())).collect()
}

// ------------------------------------------------------------------------------------------------ the replay

#[test]
fn replay_matches_the_exe_tick_by_tick() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let recs = recordings();
    if recs.is_empty() {
        eprintln!("no replay recording in _local/traces; run tools/trace/run_replay.py");
        return;
    }
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let (messages, lang) = message_texts(&recs);
    let rules = GameRules { messages, ..game_rules(&inst) };
    let designs = design_texts(lang);
    let mut failed = Vec::new();
    for rec in recs {
        // A staged recording starts before the purchases (`pre.sav`); they are replayed and checked against save 0.
        let pre = rec.dir.join("pre.sav");
        let first = load(&if pre.exists() { pre } else { rec.dir.join("0.sav") });
        let mut state = SaveState::from_save(&first, &tech_defaults);
        state.set_volatile(&volatile(&rec.events[0]));
        // Appended records' uninitialised bytes: from the exe's save after the tick.
        let next: Rc<RefCell<Option<SaveGame>>> = Rc::default();
        let mut game = Game::new(state, &rules);
        game.fleet_name = Some(rec.events[0]["vol"]["alapnev"].as_str().unwrap().as_bytes().to_vec());
        game.design_texts = designs.clone();
        if let Some(names) = rec.events[0]["vol"]["empnames"].as_array() {
            game.empire_names = names.iter().map(|n| unhex(n.as_str().unwrap_or_default())).collect();
        }
        if let Some(names) = rec.events[0]["vol"]["famnames"].as_array() {
            game.family_names = names.iter().map(|n| unhex(n.as_str().unwrap_or_default())).collect();
        }
        let source = next.clone();
        game.uninit = Some(Box::new(move |what| source.borrow().as_ref().map_or_else(Vec::new, |s| fresh(s, what))));
        // Every `LoadSave` (the load and each of the tracer's saves) ends in `FlottakUjraSzamol`.
        game.saved();
        let mut bad = Vec::new();
        for b in &rec.buys {
            let msg = Delivery(unhex(b["msg"].as_str().unwrap())[..DELIVERY].try_into().unwrap());
            let ok = game.buy(b["e"].as_u64().unwrap() as usize, &msg);
            if ok != b["accepted"].as_bool().unwrap() {
                bad.push(format!("purchase {b}: accepted {ok}"));
            }
        }
        if !rec.buys.is_empty() {
            let d: Vec<String> = diff(&game.state.chunks(), &group_chunks(&load(&rec.dir.join("0.sav")))).into_iter().filter(|d| modelled(d.split(':').next().unwrap())).collect();
            if !d.is_empty() {
                bad.push(format!("after the purchases (save 0): {} chunk groups differ:
  {}", d.len(), d.join("
  ")));
            }
            game.saved();
        }
        let mut unmodelled: BTreeMap<String, usize> = BTreeMap::new();
        let mut battles = Vec::new();
        let (mut seen, mut sent) = (0, 0);
        let mut cursor_known = rec.events[0]["vol"]["mem"].as_array().unwrap().iter().any(|r| r[0] == "bgcursor");
        for (k, ev) in rec.events.iter().enumerate().skip(1) {
            // The lockstep messages the last tick sent reach their handlers in the next frame, before this tick.
            let emptied: Vec<usize> = game.events[sent..].iter().filter_map(GameEvent::emptied_fleet).collect();
            for f in emptied {
                game.fleet_emptied(f);
            }
            sent = game.events.len();
            game.advance_clock();
            // `--orders`/`--colony`: the tracer ran `MULTIGET_Starmap`/`MULTIGET_Colony` at `IdoNapValtozott`, after the clock step.
            for o in rec.orders.iter().filter(|o| o["before"].as_u64() == Some(k as u64)) {
                if let Some(seed) = o["libc"].as_u64().filter(|_| o == rec.orders.iter().find(|x| x["before"] == o["before"]).unwrap()) {
                    game.libc = Libc(seed as u32);
                }
                if o["k"] == "morder" {
                    inbox_step(&mut game, o);
                    continue;
                }
                let vals: Vec<i32> = o["vals"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
                let t = o["type"].as_i64().unwrap() as i32;
                if o["k"] == "dorder" {
                    game.design_order(vals[0] as u8, DesignOrder::from_message(t, &vals[1..]).expect("a design message"));
                } else if o["k"] == "iorder" {
                    game.info_order(vals[0] as u8, InfoOrder::from_message(t, &vals[1..]).expect("an info message"));
                } else if o["k"] == "rorder" {
                    game.research_order(vals[0] as u8, ResearchOrder::from_message(t, &vals[1..]).expect("a research message"));
                } else if o["k"] == "corder" {
                    game.colony_order(vals[0] as u8, ColonyOrder::from_message(t, &vals[1..]).expect("a colony message"));
                } else {
                    game.starmap_order(vals[0] as u8, StarmapOrder::from_message(t, &vals[1..]).expect("a starmap message"));
                }
            }
            let exe = load(&rec.dir.join(format!("{k}.sav")));
            ui_between_ticks(&mut game.state, &exe, ev);
            import_names(&mut game.state, &exe, &game.events[seen..]);
            seen = game.events.len();
            let want = group_chunks(&exe);
            let got = game.state.chunks();
            let (d, other): (Vec<String>, Vec<String>) = diff(&got, &want).into_iter().partition(|d| modelled(d.split(':').next().unwrap()));
            for o in other {
                *unmodelled.entry(o.split([':', '[']).next().unwrap().to_string()).or_default() += 1;
            }
            let mut d = d;
            // The unsaved globals the ported systems write, as the exe held them at the same moment.
            for (name, at, live) in ev["vol"]["mem"].as_array().unwrap().iter().map(|r| (r[0].as_str().unwrap(), r[1].as_u64().unwrap() as u32, unhex(r[2].as_str().unwrap()))) {
                if !COMPARED_VOLATILE.contains(&name) {
                    continue;
                }
                let ours = game.state.mem.read(at, live.len());
                if ours != live {
                    let i = (0..live.len()).find(|&i| ours[i] != live[i]).unwrap();
                    d.push(format!("{name} (unsaved, {at:#x}): first diff at {i:#x}: {} / {}", ours[i], live[i]));
                }
            }
            let st = game.state.story();
            for (name, q) in [("vegzes", &st.finished), ("talal", &st.specials)] {
                let live: Vec<(i32, i32)> = ev["vol"]["queues"][name].as_array().unwrap().iter().map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32)).collect();
                if q.entries != live {
                    d.push(format!("story queue {name} (unsaved): {:?} / {live:?}", q.entries));
                }
            }
            if !d.is_empty() || !bad.is_empty() {
                bad.push(format!("tick {k} (Ido2 {}): {} chunk groups differ:\n  {}", ev["ido2"], d.len(), d.join("\n  ")));
                break;
            }
            game.saved();
            *next.borrow_mut() = std::fs::read(rec.dir.join(format!("{}.sav", k + 1))).ok().and_then(|b| SaveGame::parse(&b).ok());
            game.libc = Libc(ev["seed"].as_u64().unwrap() as u32);
            // `melyikhatter`'s cursor (unsaved; dumped as `bgcursor` since the WF write-back): a recording without it
            // takes the one start that reproduces the exe's backgrounds at the first economy step.
            if !cursor_known && game.economy_due() {
                if let Some(exe) = next.borrow().as_ref() {
                    let bg = |p: usize| exe.chunk(&format!("WF[{}]+0x8d4", p - 1)).map(|b| i32::from_le_bytes(b[..4].try_into().unwrap()));
                    let fits = (0..rules.backgrounds.len() as i32).find(|&c| {
                        game.weather_draws(c, game.libc).0.iter().all(|&(p, _, b)| b.is_none() || b == bg(p))
                    });
                    if let Some(c) = fits {
                        game.state.mem.set_i32(va::BACKGROUND_CURSOR, c);
                    }
                    cursor_known = true;
                }
            }
            let before = game.events.len();
            game.run_tick();
            for e in game.events[before..].iter().filter(|e| matches!(e, GameEvent::Battle(BattleEvent::Space { .. } | BattleEvent::Ground { .. } | BattleEvent::GroundSide { .. }))) {
                battles.push(format!("tick {k}: {e:?}"));
            }
        }
        eprintln!("{}: {} ticks, events {:?}", rec.dir.display(), rec.events.len() - 1, event_counts(&game.events));
        // Trader income booked into money and `TradeIncomeThisYear` (fleet-movement.md §4.3; `--traders` stages it).
        let paid: Vec<(usize, usize, u8, i32)> = game.events.iter().filter_map(|e| match e {
            GameEvent::Starmap(StarmapEvent::TraderPaid { empire, amount, planet, fleet }) => Some((*fleet, *planet, *empire, *amount)),
            _ => None,
        }).collect();
        eprintln!("trader payments (empire, amount): {paid:?}");
        for b in &battles {
            eprintln!("battle {b}");
        }
        // The message ids the exe's scrolling line held (posted in the replayed ticks or before), and its spy reports.
        let mut ids = std::collections::BTreeSet::new();
        let (mut reports, mut kinds) = (0, std::collections::BTreeSet::new());
        for k in 0..rec.events.len() {
            let s = load(&rec.dir.join(format!("{k}.sav")));
            ids.extend(s.chunk("ScrollUzenetek").unwrap().chunks(ENTRY).skip(1).map(|e| i32::from_le_bytes(e[0x38..0x3c].try_into().unwrap())));
            reports = i32::from_le_bytes(s.chunk("KemHanyBeszolas").unwrap()[..4].try_into().unwrap());
            // KemBeszolasok: entry n at n·0x30, +0 the kind (info-messages.md §3.3).
            kinds.extend(s.chunk("KemBeszolasok").unwrap().chunks(0x30).skip(1).take(reports.max(0) as usize).map(|e| e[0]));
        }
        eprintln!("scrolling-line ids seen: {ids:?}; spy reports queued at the end: {reports}, report kinds seen: {kinds:x?}");
        eprintln!("chunks only unported systems change (ticks differing from the loaded save): {unmodelled:?}");
        if !bad.is_empty() {
            failed.push(format!("{}: {}", rec.dir.display(), bad.join("\n")));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// An appended record's uninitialised bytes ([`Game::uninit`]) as the exe saved them in `s`.
fn fresh(s: &SaveGame, what: Fresh) -> Vec<u8> {
    let rec = match what {
        Fresh::Fleet(f) => s.fleets.get(f).map(|r| r.to_vec()),
        Fresh::Ship(i) => s.ships.get(i).map(|r| r.to_vec()),
        Fresh::Building { planet, slot } => s.buildings.get(planet).and_then(|b| b.get(slot)).map(|r| r.to_vec()),
        Fresh::Design { empire, design } => s.designs.get(empire).and_then(|d| d.get(design)).map(|r| r.to_vec()),
        Fresh::Waypoint(i) => s.waypoints.get(i).map(|r| r.to_vec()),
        // A new unit object's uninitialised fields, as the exe saved them (the first `+0x19c` is the pointer).
        Fresh::GroundUnit { planet, slot } => {
            let mut pairs = Vec::new();
            let u = format!("WF[{}].unit[{slot}]", planet - 1);
            for o in ig2_sim::ground::UNINIT {
                // `+0x270..+0x27c` travel inside the `+0x264` block.
                let b = if (0x264..0x27c).contains(&o) {
                    s.chunk(&format!("{u}+0x264")).map(|b| &b[o - 0x264..])
                } else {
                    s.chunk(&format!("{u}+{o:#x}"))
                };
                if let Some(b) = b {
                    pairs.extend((o as u32).to_le_bytes());
                    pairs.extend_from_slice(&b[..4]);
                }
            }
            Some(pairs)
        }
    };
    rec.unwrap_or_default()
}

/// `EpuleteketJoHelyreTesz` (exe 0x53cd00, `colony-placement.md` §6) against the exe: `run_save.py --refit` perturbs
/// the buildings and the ground units' cells, writes a save (`<tag>-<2k>.sav`), calls the exe's refit and writes
/// another (`<tag>-<2k+1>.sav`); the port loads the first, refits and must write the second.
#[test]
fn building_refit_matches_the_exe() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let dir = local("IG2_TRACE_DIR", "traces").join("refit");
    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for e in std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default() {
        let name = e.file_name().and_then(|f| f.to_str()).unwrap_or_default().to_string();
        let Some((tag, k)) = name.strip_suffix(".sav").and_then(|n| n.rsplit_once('-')) else { continue };
        let Ok(k) = k.parse::<usize>() else { continue };
        if k % 2 == 0 {
            pairs.push((e.clone(), dir.join(format!("{tag}-{}.sav", k + 1))));
        }
    }
    pairs.retain(|(_, b)| b.exists());
    pairs.sort();
    if pairs.is_empty() {
        eprintln!("no refit recording in {}; run tools/trace/run_save.py --refit", dir.display());
        return;
    }
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    let (mut bad, mut moved, mut buildings) = (Vec::new(), 0, 0);
    for (a, b) in &pairs {
        let (before, after) = (load(a), load(b));
        let mut game = Game::new(SaveState::from_save(&before, &tech_defaults), &rules);
        let exe = after.clone();
        game.uninit = Some(Box::new(move |what| fresh(&exe, what)));
        game.saved();
        game.refit_buildings();
        let d: Vec<String> = diff(&game.state.chunks(), &group_chunks(&after)).into_iter().filter(|d| modelled(d.split(':').next().unwrap())).collect();
        if !d.is_empty() {
            bad.push(format!("{}: {} chunk groups differ:
  {}", b.display(), d.len(), d.join("
  ")));
        }
        for (p, (x, y)) in before.buildings.iter().zip(&after.buildings).enumerate().skip(1) {
            let n = |s: &SaveGame| s.chunk("HanyEpuletLefoglalva").map_or(0, |c| i32::from_le_bytes(c[4 * p..4 * p + 4].try_into().unwrap()).max(0) as usize);
            let pos = |r: &[u8]| (r[0], r[0xc..0x14].to_vec());
            let old: Vec<_> = x.iter().skip(1).take(n(&before)).map(|r| pos(r)).filter(|r| r.0 != 0).collect();
            let new: Vec<_> = y.iter().skip(1).take(n(&after)).map(|r| pos(r)).filter(|r| r.0 != 0).collect();
            buildings += new.len();
            moved += new.iter().filter(|r| !old.contains(r)).count();
        }
    }
    eprintln!("{} refits, {buildings} buildings re-placed, {moved} of them moved", pairs.len());
    assert!(bad.is_empty(), "{}", bad.join("
"));
}

/// The build cursor's search `LocateActBuildingSuitableLocation` (exe 0x486460, `colony-placement.md` §8) against the
/// exe: `run_save.py --cursor` moves units near buildings, writes a save (`<tag>-<k>.sav`), then on every owned planet
/// builds `MapAlloc` with the exe's own `Decode2LargeMap` and `Allocate` (as `Init3dColony` does) and runs the exe's
/// search from fuzzed starts for fuzzed kinds; the JSON holds each grid and `[kind, x, y, ok, x', y']`. The port's
/// `PlacementRules::screen_grid` must equal the grid and `locate_cursor` (with `SaveState::vehicle_cells`) each result.
#[test]
fn build_cursor_search_matches_the_exe() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let dir = local("IG2_TRACE_DIR", "traces").join("cursor");
    let mut saves: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "sav")).collect())
        .unwrap_or_default();
    saves.retain(|p| p.with_extension("json").exists());
    saves.sort();
    if saves.is_empty() {
        eprintln!("no cursor recording in {}; run tools/trace/run_save.py --cursor", dir.display());
        return;
    }
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    let (mut bad, mut probes, mut moved, mut grids) = (Vec::new(), 0, 0, 0);
    for sav in &saves {
        let state = SaveState::from_save(&load(sav), &tech_defaults);
        let world = state.colony();
        let json: Value = serde_json::from_str(&std::fs::read_to_string(sav.with_extension("json")).unwrap()).unwrap();
        for c in json["cursor"].as_array().unwrap() {
            let p = c["p"].as_u64().unwrap() as usize;
            let planet = &world.planets[p];
            let grid = rules.placement.screen_grid(planet).expect("colony map");
            if grid.cells() != unhex(c["grid"].as_str().unwrap()).as_slice() {
                bad.push(format!("{} planet {p}: MapAlloc differs", sav.display()));
            }
            grids += 1;
            let units = state.vehicle_cells(p);
            for q in c["probes"].as_array().unwrap() {
                let v: Vec<i32> = q.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
                let exe = (v[3] != 0).then_some((v[4], v[5]));
                let port = rules.placement.locate_cursor(planet, v[0] as u8, &units, (v[1], v[2]));
                if port != exe {
                    bad.push(format!("{} planet {p} kind {} from ({}, {}): exe {exe:?}, port {port:?}", sav.display(), v[0], v[1], v[2]));
                }
                probes += 1;
                moved += usize::from(exe.is_some_and(|e| e != (v[1], v[2])));
            }
        }
    }
    eprintln!("{} saves, {grids} grids, {probes} cursor searches ({moved} moved off the start)", saves.len());
    assert!(bad.is_empty(), "{} mismatches:
{}", bad.len(), bad[..bad.len().min(20)].join("
"));
}

/// `AddWormhole` (exe 0x53aec0, `fleet-movement.md` "Opening a wormhole") against the exe: `run_save.py --wormholes`
/// sets `OsszHanyWormhole`, writes a save (`<tag>-<2k>.sav`), calls the exe's `AddWormhole` with fuzzed coordinates
/// until it returns 0 and writes another (`<tag>-<2k+1>.sav`, the calls and results in its JSON); the port loads the
/// first, makes the same calls (same results) and must write the second.
#[test]
fn add_wormhole_matches_the_exe() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let dir = local("IG2_TRACE_DIR", "traces").join("addwormhole");
    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for e in std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default() {
        let name = e.file_name().and_then(|f| f.to_str()).unwrap_or_default().to_string();
        let Some((tag, k)) = name.strip_suffix(".sav").and_then(|n| n.rsplit_once('-')) else { continue };
        let Ok(k) = k.parse::<usize>() else { continue };
        if k % 2 == 0 {
            pairs.push((e.clone(), dir.join(format!("{tag}-{}.sav", k + 1))));
        }
    }
    pairs.retain(|(_, b)| b.exists() && b.with_extension("json").exists());
    pairs.sort();
    if pairs.is_empty() {
        eprintln!("no AddWormhole recording in {}; run tools/trace/run_save.py --wormholes", dir.display());
        return;
    }
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    let (mut bad, mut made, mut refused) = (Vec::new(), 0, 0);
    for (a, b) in &pairs {
        let (before, after) = (load(a), load(b));
        let rec: Value = serde_json::from_str(&std::fs::read_to_string(b.with_extension("json")).unwrap()).unwrap();
        let mut game = Game::new(SaveState::from_save(&before, &tech_defaults), &rules);
        let exe = after.clone();
        game.uninit = Some(Box::new(move |what| fresh(&exe, what)));
        game.saved();
        for c in rec["calls"].as_array().unwrap() {
            let c: Vec<i32> = c.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
            let w = game.add_wormhole(c[0], c[1], c[2], c[3]);
            if w != c[4] {
                bad.push(format!("{}: AddWormhole{:?} returned {w}, the exe {}", b.display(), &c[..4], c[4]));
            }
            if c[4] == 0 { refused += 1 } else { made += 1 }
        }
        let d: Vec<String> = diff(&game.state.chunks(), &group_chunks(&after)).into_iter().filter(|d| modelled(d.split(':').next().unwrap())).collect();
        if !d.is_empty() {
            bad.push(format!("{}: {} chunk groups differ:
  {}", b.display(), d.len(), d.join("
  ")));
        }
        // Story 239's reveal (not traced; exe 0x5ceb86): both ends shown, both fleets always visible.
        let w = game.state.mem.i32(va::WORMHOLE_COUNT);
        game.reveal_wormhole(w);
        let r = game.state.mem.read(va::WORMHOLES + 0x1c * w as u32, 0x1c);
        let map = game.state.starmap();
        let fl = |o: usize| map.fleets[i16::from_le_bytes([r[o], r[o + 1]]) as usize].flags;
        assert!(r[8] == 1 && r[0x14] == 1 && fl(10) & 0x100 != 0 && fl(0x16) & 0x100 != 0, "wormhole {w} not revealed");
    }
    eprintln!("{} AddWormhole recordings: {made} wormholes made, {refused} calls refused", pairs.len());
    assert!(refused > 0, "no refused call recorded");
    assert!(bad.is_empty(), "{}", bad.join("
"));
}

/// `--answers`' step: a story's message call (through the stories' event path) or a messages screen order.
fn inbox_step(game: &mut Game, o: &Value) {
    let v = o["vals"].as_array().unwrap();
    let int = |i: usize| v[i].as_i64().unwrap() as i32;
    let en = game.state.mem.i32(va::EN) as u8;
    match o["op"].as_str().unwrap() {
        "order" => {
            let vals: Vec<i32> = (1..v.len()).map(int).collect();
            game.message_order(en, MessageOrder::from_message(int(0), &vals).expect("a messages screen order"));
        }
        "new" => game.inbox_call(StoryEvent::NewMessage { id: int(0), arg: int(1) }),
        "race" => game.inbox_call(StoryEvent::MessageRace(int(0))),
        "param" => {
            let text: &'static str = Box::leak(v[1].as_str().unwrap().to_owned().into_boxed_str());
            game.inbox_call(StoryEvent::MessageParam { slot: int(0), text: MessageText::Literal(text) });
        }
        "paramint" => game.inbox_call(StoryEvent::MessageParamInt { slot: int(0), value: int(1) }),
        "remove" => game.inbox_call(StoryEvent::RemoveMessage(int(0))),
        "withdraw" => game.inbox_call(StoryEvent::WithdrawMessage { id: int(0), arg: 0 }),
        "disable" => game.inbox_call(StoryEvent::DisablePart(int(0))),
        op => panic!("unknown inbox step {op}"),
    }
}

/// The unsaved globals of `replay.js`'s dump that ported systems write (the rest are inputs or UI state).
const COMPARED_VOLATILE: &[&str] = &[
    "st_msg", "st_noaddship", "st_lastfn", "st_lastfn_r", "st_line", "st_nearest", "st_best", "st_kemazon", "st_kemtulaj",
    "st_hajokigy", "kemesemeny", "ctf",
    "speed", "rnd", "rndossz", "radar2", "radar2sq", "radarsz", "radarszsq", "dipnap", "dipstart", "st_flags", "st_last",
    "st_oldido", "st_szvegzett", "st_delayed", "st_restart", "st_skip", "st_err", "st_signal", "st_addship", "st_special",
    "st_radar", "st_dipl", "st_diplvalasz", "st_arr", "st_gr", "st_youwas", "st_grwin", "st_kem", "abmitepit", "mapcursor",
    "tradeports", "etahonap", "etadays", "infomonth", "spw_fleets", "spw_planetflag", "gvalue", "bgcursor", "tut_active",
    "tut_paused", "tut_event", "msg_over", "msg_typed", "msg_rows", "msg_nchoices", "msg_unread", "msg_ch0", "msg_ch1",
    "msg_ch2", "msg_ch3",
];

/// The UI's part between two ticks (`info-messages.md` §2.3): `FRISSIT_Scroll2` takes texts off the scrolling
/// line at the display's pace, as many as the exe's save shows taken; the line's text and flag as dumped (when
/// the recording has them).
fn ui_between_ticks(s: &mut SaveState, exe: &SaveGame, ev: &Value) {
    let want = i32::from_le_bytes(exe.chunk("UzenetekDb").unwrap()[..4].try_into().unwrap());
    let mut t = s.ticker();
    while t.count > want {
        t.pop();
    }
    s.store_ticker(&t);
    for (name, at, live) in ev["vol"]["mem"].as_array().unwrap().iter().map(|r| (r[0].as_str().unwrap(), r[1].as_u64().unwrap() as u32, unhex(r[2].as_str().unwrap()))) {
        if matches!(name, "scrolltext" | "scrollimp") {
            s.mem.write(at, &live);
        }
    }
}

/// Localised names given in the last tick (`MiFoFlottaNev`: a text table the port doesn't render), taken from the
/// exe's save.
fn import_names(s: &mut SaveState, exe: &SaveGame, events: &[GameEvent]) {
    for e in events {
        if let GameEvent::Ai(AiEvent::MainFleetNamed { fleet, .. }) = e {
            if let (Some(a), Some(b)) = (s.fleets.get_mut(*fleet), exe.fleets.get(*fleet)) {
                a[..0x14].copy_from_slice(&b[..0x14]);
            }
        }
    }
}

fn event_counts(events: &[GameEvent]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for e in events {
        let s = format!("{e:?}");
        let key: String = s.split(['(', ' ', '{']).take(2).collect::<Vec<_>>().join(" ");
        *m.entry(key).or_default() += 1;
    }
    m
}

/// `BirodalomMeghal` and `MegnyeresEllenoriz` on `quick-1.sav` (only AI 3 and the human 6 alive): AI 3's death
/// deletes its fleet and wins the game for the human, unless an empire dying later the same pass is still alive;
/// the human's death (a story's `DIE()`) loses it.
#[test]
fn empire_death_deletes_fleets_and_checks_victory() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let p = local("IG2_SAVE_DIR", "saves").join("quick-1.sav");
    if !p.exists() {
        eprintln!("no {}; skipping", p.display());
        return;
    }
    use ig2_sim::game::GameOver;
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    let save = load(&p);
    let owned = |g: &Game, e: u8| g.state.fleets.iter().skip(1).filter(|f| f[0x15] == e).count();
    let over = |g: &Game| g.events.iter().filter_map(|e| if let GameEvent::GameOver(o) = e { Some(*o) } else { None }).collect::<Vec<_>>();
    let alive = |g: &Game, e: u32| g.state.mem.u8(va::EMPIRE_ALIVE + e);

    // quick-1's only fleets are the traders' (empire 9): AI 3 gets the first.
    let mut g = Game::new(SaveState::from_save(&save, &tech_defaults), &rules);
    let f: usize = g.state.fleets.iter().skip(1).position(|f| f[0x15] == 9).expect("a trader's fleet") + 1;
    g.state.fleets[f][0x15] = 3;
    assert_eq!(owned(&g, 3), 1);
    g.empire_dies(3, &[]);
    assert_eq!((owned(&g, 3), alive(&g, 3)), (0, 0));
    assert_eq!(over(&g), [GameOver::Won]);

    let mut g = Game::new(SaveState::from_save(&save, &tech_defaults), &rules);
    g.empire_dies(3, &[1]);
    assert!(over(&g).is_empty(), "empire 1 dies later in the same pass: still an enemy here");

    let mut g = Game::new(SaveState::from_save(&save, &tech_defaults), &rules);
    g.state.fleets[f][0x15] = 6;
    g.empire_dies(6, &[]);
    assert_eq!((owned(&g, 6), alive(&g, 6)), (0, 0));
    assert_eq!(over(&g), [GameOver::Lost]);
    // `ExitToMainScreen` cleared `GameStarted`: the last AI's death no longer wins.
    g.empire_dies(3, &[]);
    assert_eq!((over(&g), g.over), (vec![GameOver::Lost], Some(GameOver::Lost)));

    // A story's `MEGNYERES(1)` then `MEGNYERES(0)`: the first ends the game.
    let mut g = Game::new(SaveState::from_save(&save, &tech_defaults), &rules);
    g.story_ends(&[ig2_sim::story::StoryEvent::Won, ig2_sim::story::StoryEvent::CampaignOver]);
    assert_eq!(over(&g), [GameOver::Won]);
}

/// The tutorial's in-place writes (`re/rules/tutorial.md` §3; `Game::tutorial_effect`) on an original save.
#[test]
fn tutorial_effects_write_the_game() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let p = local("IG2_SAVE_DIR", "saves").join("quick-1.sav");
    if !p.exists() {
        eprintln!("no {}; skipping", p.display());
        return;
    }
    use ig2_sim::tutorial::Effect;
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    let mut g = Game::new(SaveState::from_save(&load(&p), &tech_defaults), &rules);
    let en = g.state.mem.i32(va::EN) as usize;
    let money = |g: &Game| g.state.treasuries()[en].total;
    let m0 = money(&g);
    assert!(g.tutorial_effect(&Effect::GiveMoney(300000)));
    assert_eq!(money(&g), m0 + 300000.0);
    assert!(g.tutorial_effect(&Effect::Invention { tech: 0x12, state: 3 }));
    assert_eq!(g.state.mem.u8(va::TECHS + 0x12 * 0x114 + 0x45 + en as u32), 3);
    assert!(g.tutorial_effect(&Effect::DisableAutobuild));
    let n = g.state.planet_count();
    assert!((1..=n).all(|q| g.state.mem.u8(va::PLANETS + (q * 0x188) as u32 + 0x6c) == 0));
    assert!(g.tutorial_effect(&Effect::Traders { enabled: false }));
    assert!(g.state.fleets.iter().skip(1).filter(|f| f[0x15] == 9).all(|f| f[0x18] & 1 == 1 && f[0x16] == 0x9a && f[0x17] == 2));
    assert!(g.tutorial_effect(&Effect::Traders { enabled: true }));
    assert!(g.state.fleets.iter().skip(1).filter(|f| f[0x15] == 9).all(|f| f[0x18] & 1 == 0));
    assert!(g.tutorial_effect(&Effect::ExploreEmpire { empire: 6, diplomacy: false }));
    assert_eq!(g.state.mem.u8(va::EMPIRE_KNOWS + 6 + 0x13 * en as u32), 1);
    assert!(!g.tutorial_effect(&Effect::CenterFleet));
}

/// `GetWarfareStatus` (ELF 0x4a4f10) on the quick saves (no ground war running when saved): no field is contested, so
/// `Options_Main`'s load stays on the starmap. (`WF::GetStatus` itself: the ground tests.)
#[test]
fn warfare_status_is_zero_without_a_running_war() {
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    for name in ["quick-0.sav", "quick-1.sav", "quick-2.sav"] {
        let p = local("IG2_SAVE_DIR", "saves").join(name);
        if !p.exists() {
            eprintln!("no {}; skipping", p.display());
            continue;
        }
        let mut g = Game::new(SaveState::from_save(&load(&p), &tech_defaults), &rules);
        let n = g.state.planet_count();
        assert!((0..=n + 1).all(|q| g.warfare_status(q) == 0), "{name}");
    }
}

/// The tutorial's sim-side events (`re/rules/tutorial.md` §4): `EpuletNew`'s small house on an `EN` planet (0x13),
/// and `MULTIGET_Starmap`'s 0x15 for an own fleet sent past the radar's reach from the last own planet (not for a
/// near move).
#[test]
fn tutorial_events_from_buildings_and_moves() {
    let Some(inst) = install() else { return };
    let p = local("IG2_SAVE_DIR", "saves").join("quick-1.sav");
    if !p.exists() {
        return;
    }
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    let mut g = Game::new(SaveState::from_save(&load(&p), &tech_defaults), &rules);
    let en = g.state.mem.i32(va::EN) as u8;
    let c = g.state.colony();
    let n = g.state.planet_count();
    let own = (1..=n).find(|&q| c.planets[q].owner == en).expect("an own planet");
    let last = (1..=n).rev().find(|&q| c.planets[q].owner == en).unwrap();
    let (lx, ly) = (c.planets[last].x, c.planets[last].y);
    let waiting = |g: &mut Game, id: i32| {
        g.state.mem.set_u8(va::TUTORIAL_PAUSED, 1);
        g.state.mem.set_u8(va::TUTORIAL_ACTIVE, 0);
        g.state.mem.set_i32(va::TUTORIAL_EVENT, id);
    };
    let event = |g: &Game| (g.state.mem.i32(va::TUTORIAL_EVENT), g.state.mem.u8(va::TUTORIAL_PAUSED), g.state.mem.u8(va::TUTORIAL_ACTIVE));

    // A small house (kind 2) on an own planet.
    assert!(g.tutorial_effect(&ig2_sim::tutorial::Effect::GiveMoney(1_000_000)));
    waiting(&mut g, 0x13);
    g.colony_order(en, ColonyOrder::Build { planet: own as i32, kind: 2, x: 20, y: 20 });
    assert_eq!(event(&g), (0, 0, 1), "TutorialCheckEvent(0x13) unpaused the script");
    waiting(&mut g, 0x14);
    g.colony_order(en, ColonyOrder::Build { planet: own as i32, kind: 2, x: 20, y: 20 });
    assert_eq!(event(&g).0, 0x14, "another kind's event stays awaited");

    // A trader made ours: a near move keeps 0x15 awaited, one past the radar's reach raises it.
    let f = g.state.fleets.iter().skip(1).position(|f| f[0x15] == 9).expect("a trader's fleet") + 1;
    g.state.fleets[f][0x15] = en;
    waiting(&mut g, 0x15);
    g.starmap_order(en, StarmapOrder::Move { fleet: f as i32, order: 1, kind: 0, x: lx + 1.0, y: ly });
    assert_eq!(event(&g).0, 0x15);
    g.starmap_order(en, StarmapOrder::Move { fleet: f as i32, order: 0, kind: 0, x: lx + 5000.0, y: ly });
    assert_eq!(event(&g).0, 0x15, "order 0 doesn't count");
    g.starmap_order(en, StarmapOrder::Move { fleet: f as i32, order: 1, kind: 0, x: lx + 5000.0, y: ly });
    assert_eq!(event(&g), (0, 0, 1));
}

/// `GIVE KILLER FLEET` / `GIVE KOLONIZALO FLEET` (`re/rules/tutorial.md` §3 61, 62): EN's "Terminator" beside its
/// last planet with four ships and empire 6's planets explored; another empire's "Unbeatable" at ((6 − e)·20, 300);
/// "Colonization" with one ship.
#[test]
fn tutorial_fleets() {
    use ig2_sim::tutorial::Effect;
    let Some(inst) = install() else { return };
    let p = local("IG2_SAVE_DIR", "saves").join("quick-1.sav");
    if !p.exists() {
        return;
    }
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    let rules = game_rules(&inst);
    let mut g = Game::new(SaveState::from_save(&load(&p), &tech_defaults), &rules);
    let en = g.state.mem.i32(va::EN) as u8;
    let named = |g: &Game, n: &[u8]| g.state.fleets.iter().position(|f| f[..n.len()] == *n && f[n.len()] == 0);
    let f32_at = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    assert!(g.tutorial_effect(&Effect::KillerFleet(en as i32)));
    let t = named(&g, b"Terminator").expect("the Terminator");
    assert_eq!(g.state.fleets[t][0x15], en);
    assert!(i16::from_le_bytes([g.state.fleets[t][0x54], g.state.fleets[t][0x55]]) >= 4, "four ships");
    let other = if en == 3 { 4 } else { 3 };
    assert!(g.tutorial_effect(&Effect::KillerFleet(other as i32)));
    let u = named(&g, b"Unbeatable").expect("the Unbeatable");
    assert_eq!((g.state.fleets[u][0x15], f32_at(&g.state.fleets[u], 0x30), f32_at(&g.state.fleets[u], 0x34)), (other, (6 - other as i32) as f32 * 20.0, 300.0));
    assert!(g.tutorial_effect(&Effect::ColonizationFleet));
    assert!(named(&g, b"Colonization").is_some());
}
