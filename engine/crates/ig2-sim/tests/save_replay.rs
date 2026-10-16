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
use ig2_sim::game::{Fresh, Game, GameEvent, GameRules, Libc};
use ig2_sim::placement::{PlacementRules, Terrain};
use ig2_sim::radar::RadarRules;
use ig2_sim::research::ResearchRules;
use ig2_sim::save::{group_chunks, modelled, va, SaveState};
use ig2_sim::spacewar::{DesignRules, Hull};
use ig2_sim::spy::{MissionRule, Spy, SpyRules};
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
            let i = (0..x.len().max(y.len())).find(|&i| x.get(i) != y.get(i)).unwrap();
            out.push(format!("{n}: len {}/{} first diff at {i:#x}: {:?} / {:?}", x.len(), y.len(), x.get(i), y.get(i)));
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
    events: Vec<Value>,
}

fn recordings() -> Vec<Recording> {
    let dir = local("IG2_TRACE_DIR", "traces");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    v.retain(|p| p.is_dir() && p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with("replay-")));
    v.sort();
    v.into_iter()
        .filter_map(|dir| {
            let text = std::fs::read_to_string(dir.join("replay.jsonl")).ok()?;
            let events = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
            Some(Recording { dir, events })
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
    let terrains = surfaces
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let m = colony_map::ColonyMap::parse(&inst.read(&format!("dataraw/colony/gen_maps/{n}.map")).unwrap()).unwrap();
            Terrain { mask: m.mask_a, kind: s.kind as u8, start: s.ai_build_start }
        })
        .collect();
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
        },
        ai: AiRules {
            build_mode: r.ai_build_mode.clone(),
            build_min: r.ai_build_min.clone(),
            build_max: r.ai_build_max.clone(),
            production_share: r.race_ai_production_share.clone(),
            military_mode: r.race_ai_military_autobuild.clone(),
            training_cost: r.spy_training_cost.clone(),
            mission_cost_mult: r.spy_race_cost_mult.clone(),
            design,
            pod_slot_max: r.pod_slots.iter().map(|t| t.iter().map(|h| h.iter().map(|s| s.max_count).collect()).collect()).collect(),
            pods: r.pod_types.iter().map(|p| PodRule { id: p.id, techs: std::array::from_fn(|i| p.techs.get(i).copied().unwrap_or(0)) }).collect(),
            ground_priority: r.ai_ground_priority.clone(),
        },
        radar: RadarRules { ranges: r.radar_ranges[..6].try_into().unwrap(), planet_ranges: r.planet_radar_ranges[..8].try_into().unwrap() },
        difficulty_production: r.difficulty_production_mult.clone(),
        difficulty_money: r.difficulty_money_mult.clone(),
    }
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
    let rules = game_rules(&inst);
    for rec in recs {
        let first = load(&rec.dir.join("0.sav"));
        let mut state = SaveState::from_save(&first, &tech_defaults);
        state.set_volatile(&volatile(&rec.events[0]));
        // Appended records' uninitialised bytes: from the exe's save after the tick.
        let next: Rc<RefCell<Option<SaveGame>>> = Rc::default();
        let mut game = Game::new(state, &rules);
        game.fleet_name = Some(rec.events[0]["vol"]["alapnev"].as_str().unwrap().as_bytes().to_vec());
        let source = next.clone();
        game.uninit = Some(Box::new(move |what| {
            let s = source.borrow();
            let Some(s) = s.as_ref() else { return Vec::new() };
            let rec = match what {
                Fresh::Fleet(f) => s.fleets.get(f).map(|r| r.to_vec()),
                Fresh::Design { empire, design } => s.designs.get(empire).and_then(|d| d.get(design)).map(|r| r.to_vec()),
            };
            rec.unwrap_or_default()
        }));
        let mut bad = Vec::new();
        let mut unmodelled: BTreeMap<String, usize> = BTreeMap::new();
        let mut seen = 0;
        for (k, ev) in rec.events.iter().enumerate().skip(1) {
            game.advance_clock();
            let exe = load(&rec.dir.join(format!("{k}.sav")));
            import_trade(&mut game.state, &exe);
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
            if !d.is_empty() {
                bad.push(format!("tick {k} (Ido2 {}): {} chunk groups differ:\n  {}", ev["ido2"], d.len(), d.join("\n  ")));
                break;
            }
            *next.borrow_mut() = std::fs::read(rec.dir.join(format!("{}.sav", k + 1))).ok().and_then(|b| SaveGame::parse(&b).ok());
            game.libc = Libc(ev["seed"].as_u64().unwrap() as u32);
            game.run_tick();
        }
        eprintln!("{}: {} ticks, events {:?}", rec.dir.display(), rec.events.len() - 1, event_counts(&game.events));
        eprintln!("chunks only unported systems change (ticks differing from the loaded save): {unmodelled:?}");
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}

/// The unsaved globals of `replay.js`'s dump that ported systems write (the rest are inputs or UI state).
const COMPARED_VOLATILE: &[&str] = &[
    "speed", "rnd", "rndossz", "radar2", "radar2sq", "radarsz", "radarszsq", "dipnap", "dipstart", "st_flags", "st_last",
    "st_oldido", "st_szvegzett", "st_delayed", "st_restart", "st_skip", "st_err", "st_signal", "st_addship", "st_special",
    "st_radar", "st_dipl", "st_diplvalasz", "st_arr", "st_gr", "st_youwas", "st_grwin", "st_kem", "abmitepit", "mapcursor",
    "tradeports",
];

/// What the unported `FRISSIT_Trade` (the merchants, ELF 0x7b0ad0) wrote into chunks the port models, taken from
/// the exe's save: its stream 11, the merchant countdown `StoryAdat+0xc4`, and the trade fields `+0x6c..+0x80`
/// (offer count, date) of the design it put on the market. Nothing ported reads them.
fn import_trade(s: &mut SaveState, exe: &SaveGame) {
    let seeds = exe.chunk("RandSeed").unwrap();
    s.mem.write(va::RAND_SEED + 4 * 11, &seeds[44..48]);
    let adat = exe.chunk("StoryAdat").unwrap();
    s.mem.write(va::STORY_ADAT + 0xc4, &adat[0xc4..0xc8]);
    for (ours, theirs) in s.designs.iter_mut().zip(&exe.designs) {
        for (a, b) in ours.iter_mut().zip(theirs) {
            if a[0x74..0x80] != b[0x74..0x80] && b[0x6c..0x80].iter().any(|&x| x != 0) {
                a[0x6c..0x80].copy_from_slice(&b[0x6c..0x80]);
            }
        }
    }
}

/// Localised names the AI gave in the last tick (`DesignPoenosanElnevez`, `MiFoFlottaNev`: text tables the port
/// doesn't render), taken from the exe's save.
fn import_names(s: &mut SaveState, exe: &SaveGame, events: &[GameEvent]) {
    for e in events {
        match e {
            GameEvent::Ai(AiEvent::DesignNamed { empire, design, .. }) => {
                let (e, d) = (*empire as usize, *design);
                if let (Some(a), Some(b)) = (s.designs.get_mut(e).and_then(|v| v.get_mut(d)), exe.designs.get(e).and_then(|v| v.get(d))) {
                    a[..0x15].copy_from_slice(&b[..0x15]);
                }
            }
            GameEvent::Ai(AiEvent::MainFleetNamed { fleet, .. }) => {
                if let (Some(a), Some(b)) = (s.fleets.get_mut(*fleet), exe.fleets.get(*fleet)) {
                    a[..0x14].copy_from_slice(&b[..0x14]);
                }
            }
            _ => {}
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
