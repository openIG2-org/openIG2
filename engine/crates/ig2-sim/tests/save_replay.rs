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
use ig2_sim::game::{BattleEvent, Fresh, Game, GameEvent, GameRules, Libc};
use ig2_sim::messages::{cstr, Texts, ENTRY};
use ig2_sim::placement::{PlacementRules, Terrain};
use ig2_sim::radar::RadarRules;
use ig2_sim::research::ResearchRules;
use ig2_sim::save::{group_chunks, modelled, va, SaveState};
use ig2_sim::save::va::TRADE_OFFERS;
use ig2_sim::ground::GroundRules;
use ig2_sim::spacewar::{DesignRules, GunRules, Hull};
use ig2_sim::spy::{MissionRule, Spy, SpyRules};
use ig2_sim::trade::{Delivery, TradeEvent, DELIVERY};
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
}

fn recordings() -> Vec<Recording> {
    let dir = local("IG2_TRACE_DIR", "traces");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    v.retain(|p| p.is_dir() && p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with("replay-")));
    v.sort();
    v.into_iter()
        .filter_map(|dir| {
            let text = std::fs::read_to_string(dir.join("replay.jsonl")).ok()?;
            let all: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
            let (events, rest): (Vec<Value>, Vec<Value>) = all.into_iter().partition(|e| e["k"] == "save");
            let buys = rest.into_iter().filter(|e| e["k"] == "buy").collect();
            Some(Recording { dir, events, buys })
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
            design: design.clone(),
            pod_slot_max: r.pod_slots.iter().map(|t| t.iter().map(|h| h.iter().map(|s| s.max_count).collect()).collect()).collect(),
            pods: r.pod_types.iter().map(|p| PodRule { id: p.id, techs: std::array::from_fn(|i| p.techs.get(i).copied().unwrap_or(0)) }).collect(),
            ground_priority: r.ai_ground_priority.clone(),
        },
        radar: RadarRules { ranges: r.radar_ranges[..6].try_into().unwrap(), planet_ranges: r.planet_radar_ranges[..8].try_into().unwrap() },
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
        },
    }
}

/// `uzenet.txt` in the language the recordings' exe ran in: the one whose texts begin the messages on the
/// recordings' scrolling lines (English when none shows one).
fn message_texts(recs: &[Recording]) -> Texts {
    let mut shown: Vec<(i32, Vec<u8>)> = Vec::new();
    for rec in recs {
        let last = load(&rec.dir.join(format!("{}.sav", rec.events.len() - 1)));
        let main = last.chunk("ScrollUzenetek").unwrap();
        for e in main.chunks(ENTRY).skip(1) {
            shown.push((i32::from_le_bytes(e[0x38..0x3c].try_into().unwrap()), cstr(e).to_vec()));
        }
    }
    let texts = |lang| Install::from_env(lang).and_then(|i| i.ok()).and_then(|i| i.read("dataraw/text/uzenet.txt").ok()).map(|raw| Texts::parse(&raw));
    for lang in Lang::ALL {
        let Some(t) = texts(lang) else { continue };
        let fits = |(id, text): &(i32, Vec<u8>)| {
            t.format(*id, &[]).is_some_and(|f| text.starts_with(&f[..f.windows(2).position(|w| w[0] == b'%').unwrap_or(f.len())]))
        };
        if !shown.is_empty() && shown.iter().all(fits) {
            return t;
        }
    }
    texts(Lang::En).unwrap_or_default()
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
    let rules = GameRules { messages: message_texts(&recs), ..game_rules(&inst) };
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
        if let Some(names) = rec.events[0]["vol"]["empnames"].as_array() {
            game.empire_names = names.iter().map(|n| unhex(n.as_str().unwrap_or_default())).collect();
        }
        if let Some(names) = rec.events[0]["vol"]["famnames"].as_array() {
            game.family_names = names.iter().map(|n| unhex(n.as_str().unwrap_or_default())).collect();
        }
        let source = next.clone();
        game.uninit = Some(Box::new(move |what| {
            let s = source.borrow();
            let Some(s) = s.as_ref() else { return Vec::new() };
            let rec = match what {
                Fresh::Fleet(f) => s.fleets.get(f).map(|r| r.to_vec()),
                Fresh::Ship(i) => s.ships.get(i).map(|r| r.to_vec()),
                Fresh::Building { planet, slot } => s.buildings.get(planet).and_then(|b| b.get(slot)).map(|r| r.to_vec()),
                Fresh::Design { empire, design } => s.designs.get(empire).and_then(|d| d.get(design)).map(|r| r.to_vec()),
            };
            rec.unwrap_or_default()
        }));
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
        for (k, ev) in rec.events.iter().enumerate().skip(1) {
            // The lockstep messages the last tick sent reach their handlers in the next frame, before this tick.
            let emptied: Vec<usize> = game.events[sent..].iter().filter_map(GameEvent::emptied_fleet).collect();
            for f in emptied {
                game.fleet_emptied(f);
            }
            sent = game.events.len();
            game.advance_clock();
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
            let before = game.events.len();
            game.run_tick();
            for e in game.events[before..].iter().filter(|e| matches!(e, GameEvent::Battle(BattleEvent::Space { .. } | BattleEvent::Ground { .. }))) {
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
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}

/// The unsaved globals of `replay.js`'s dump that ported systems write (the rest are inputs or UI state).
const COMPARED_VOLATILE: &[&str] = &[
    "speed", "rnd", "rndossz", "radar2", "radar2sq", "radarsz", "radarszsq", "dipnap", "dipstart", "st_flags", "st_last",
    "st_oldido", "st_szvegzett", "st_delayed", "st_restart", "st_skip", "st_err", "st_signal", "st_addship", "st_special",
    "st_radar", "st_dipl", "st_diplvalasz", "st_arr", "st_gr", "st_youwas", "st_grwin", "st_kem", "abmitepit", "mapcursor",
    "tradeports", "etahonap", "etadays", "infomonth", "spw_fleets", "spw_planetflag",
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

/// Localised names given in the last tick (the AI's and the story merchant's `DesignPoenosanElnevez`,
/// `MiFoFlottaNev`: text tables the port doesn't render), taken from the exe's save.
fn import_names(s: &mut SaveState, exe: &SaveGame, events: &[GameEvent]) {
    for e in events {
        match e {
            GameEvent::Ai(AiEvent::DesignNamed { empire, design, .. }) => {
                let (e, d) = (*empire as usize, *design);
                if let (Some(a), Some(b)) = (s.designs.get_mut(e).and_then(|v| v.get_mut(d)), exe.designs.get(e).and_then(|v| v.get(d))) {
                    a[..0x15].copy_from_slice(&b[..0x15]);
                }
            }
            GameEvent::Trade(TradeEvent::OfferNamed { slot, .. }) => {
                // The whole name as the exe built it (`name + " " + design name`, which may run past +0x14), over
                // as many bytes as the longer of the two strings covers.
                let theirs = exe.chunk("TradeTervek").unwrap();
                let at = TRADE_OFFERS + (slot * 0x84) as u32;
                if let Some(b) = theirs.get(slot * 0x84..(slot + 1) * 0x84) {
                    let ours = s.mem.read(at, 0x84);
                    let end = |r: &[u8]| r.iter().position(|&c| c == 0).map_or(0x84, |i| i + 1);
                    s.mem.write(at, &b[..end(b).max(end(&ours))]);
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
