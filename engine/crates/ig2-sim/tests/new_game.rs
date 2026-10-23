//! A new campaign (`ig2_sim::game::new_game`, spec `re/rules/new-game.md`) against ig2.exe: tools/trace/run_newgame.py
//! starts campaigns in the GOG exe the way the race menu does (and scenarios the way the scenario menu does,
//! `--scenario`; skirmishes the way the skirmish menu does, `--skirmish`; their seeds recovered by `tools/trace/scenario_seeds.py`) and saves at tick 0 (`_local/traces/newgame/` or
//! `IG2_TRACE_DIR/newgame/`: `newgame-*.sav` + `.json` with the seed, the srand seeds and the config/registry reads); the
//! port starts the same campaign and must write the same save, chunk by chunk. Traces hold game state, so they stay
//! in `_local/`; the rule tables come from the user's exe. The test skips without them.

#[path = "../../ig2-net/tests/common/mod.rs"]
#[allow(dead_code)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use ig2_formats::save::SaveGame;
use ig2_formats::strings::{KeyedStrings, UiStrings};
use ig2_formats::text::{self, Lang};
use ig2_formats::{dialogue, Install};
use ig2_sim::game::new_game::{NewCampaign, NewMpGalaxy, NewScenario, NewSkirmish, StartData};
use ig2_sim::ai::AiEvent;
use ig2_sim::game::{Fresh, Game, GameEvent};
use ig2_sim::save::group_chunks;
use serde_json::Value;

fn trace_dir() -> PathBuf {
    std::env::var_os("IG2_TRACE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces")).join("newgame")
}

/// The differing chunk groups (first differing byte and a few more of each).
fn diff(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut out = Vec::new();
    for n in a.keys().chain(b.keys().filter(|k| !a.contains_key(*k))) {
        let (x, y) = (a.get(n).map_or(&[][..], |v| &v[..]), b.get(n).map_or(&[][..], |v| &v[..]));
        if x != y {
            let at: Vec<usize> = (0..x.len().max(y.len())).filter(|&i| x.get(i) != y.get(i)).collect();
            let more: Vec<String> = at.iter().take(6).map(|&j| format!("{j:#x}: {:?}/{:?}", x.get(j), y.get(j))).collect();
            out.push(format!("{n} (len {}/{}, {} bytes differ): {}", x.len(), y.len(), at.len(), more.join(", ")));
        }
    }
    out
}

/// Bytes the original leaves uninitialised (fresh heap): slot 0 of the fleet and ship arrays, and the building
/// records past each planet's used slots (`HanyEpuletLefoglalva`) with slot 0.
fn mask_uninit(a: &mut BTreeMap<String, Vec<u8>>, b: &mut BTreeMap<String, Vec<u8>>) {
    for m in [&mut *a, &mut *b] {
        if let Some(f) = m.get_mut("Flottak") {
            f[..0x80].fill(0);
            // A new fleet's name buffer past the name's NUL (`FlottaNew`'s strcpy leaves the heap's bytes).
            for r in f.chunks_exact_mut(0x80) {
                let n = r[..0x14].iter().position(|&c| c == 0).unwrap_or(0x14);
                r[n..0x14].fill(0);
            }
        }
        if let Some(h) = m.get_mut("Hajok") {
            h[..6].fill(0);
        }
    }
    let used = |m: &BTreeMap<String, Vec<u8>>| m.get("HanyEpuletLefoglalva").map(|v| v.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>());
    let (ua, ub) = (used(a), used(b));
    for p in 1..=200usize {
        let n = format!("Epuletek[{p}]");
        for (m, u) in [(&mut *a, &ua), (&mut *b, &ub)] {
            let k = u.as_ref().and_then(|u| u.get(p)).copied().unwrap_or(0).max(0) as usize;
            if let Some(r) = m.get_mut(&n) {
                let n0 = r.len().min(0x20);
                r[..n0].fill(0);
                if r.len() > (k + 1) * 0x20 {
                    r[(k + 1) * 0x20..].fill(0);
                }
            }
        }
    }
}

#[test]
fn new_campaign_matches_the_exe() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let dir = trace_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("{} missing; skipping", dir.display());
        return;
    };
    let mut runs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    runs.sort();
    if let Ok(only) = std::env::var("NEWGAME_ONLY") {
        runs.retain(|p| p.file_stem().is_some_and(|n| n.to_string_lossy().contains(&only)));
    }
    let rules = common::game_rules(&inst);
    let exe = std::fs::read(inst.root().join("ig2.exe")).unwrap();
    let pe = ig2_rules::PeImage::parse(&exe).unwrap();
    let image = |at: u32, len: usize| pe.read_loaded(at, len).unwrap_or_else(|_| vec![0; len]);
    let scenario = |n: i32| inst.read(&format!("dataraw/scenario/{n}.scn")).ok().map(|b| text::decode(&b, inst.lang()));
    let mut hu = UiStrings::default();
    if let Ok(b) = inst.read("localization/hu/dataraw/text/txt17.txt").or_else(|_| inst.read("dataraw/text/txt17.txt")) {
        hu.add_file(&text::decode(&b, Lang::Hu));
    }
    let hu_bytes = |s: &str| Lang::Hu.encoding().encode(s).0.into_owned();
    let mut ui = UiStrings::default();
    ui.add_file(&text::decode(&inst.read("dataraw/text/txt17.txt").unwrap(), inst.lang()));
    let enc = |s: &str| inst.lang().encoding().encode(s).0.into_owned();
    let mut keyed = KeyedStrings::default();
    keyed.add_file(29, &text::decode(&inst.read("dataraw/text/s_stname.txt").unwrap(), inst.lang()));
    let kinds = dialogue::parse_message_index(&text::decode(&inst.read("dataraw/uzenetek/uzenetek.txt").unwrap(), inst.lang()));
    let data = StartData {
        image: &image,
        scenario: &scenario,
        race_names: (0..20).map(|r| hu_bytes(hu.get(0x11, 1, r + 5))).collect(),
        constellations: (1..=15).map(|k| enc(keyed.get(29, k).unwrap_or(""))).collect(),
        tech_names: (0..193u16).map(|t| if t == 0 { Vec::new() } else { hu_bytes(hu.get(0x11, 8 + (t - 1) / 48, (t - 1) % 48 + 1)) }).collect(),
        design_names: (0..47u16).map(|k| hu_bytes(hu.get(0x11, 7, k + 1))).collect(),
        // `StringReadAll` sets `AlapFlottaNev` from the Hungarian txt17 too.
        fleet_name: hu_bytes(hu.get(17, 5, 46)),
        message_kinds: kinds.len() as i32,
    };
    let mut bad = Vec::new();
    let mut checked = 0;
    for json in &runs {
        let sav = json.with_extension("sav");
        let Ok(bytes) = std::fs::read(&sav) else { continue };
        let want = SaveGame::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", sav.display()));
        let j: Value = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
        let int = |v: &Value| v.as_i64().unwrap_or(0) as i32;
        let scenario = j["scenario"].as_str().map(|f| NewScenario {
            text: text::decode(&inst.read(&format!("dataraw/{f}")).unwrap(), inst.lang()),
            seed: j["seeds"]["seed"].as_u64().unwrap() as u32,
            ticks: j["seeds"]["ticks"].as_u64().unwrap() as u32,
            // The config has no `Difficulty`: GetParam's default "2".
            difficulty: 2,
        });
        let cfg = || NewCampaign {
            campaign: int(&j["campaign"]),
            difficulty: int(&j["log"]["init"][3]),
            seed: j["log"]["srand"][0].as_u64().unwrap() as u32,
            ticks: j["log"]["srand"][1].as_u64().unwrap() as u32,
            storyline: int(&j["storyline"]),
            storymap: int(&j["storymap"]),
            last_storyline: j["registry"].as_i64().or_else(|| j["registry"].as_str().and_then(|s| s.parse().ok())).map(|v| v as i32),
        };
        let mut g = Game::before_start(&data, &rules);
        // Appended fleet, ship, building, design and ground-unit records hold the heap's bytes in the original: the
        // exe's own.
        let s = want.clone();
        g.uninit = Some(Box::new(move |f: Fresh| match f {
            Fresh::Fleet(i) => s.fleets.get(i).map_or_else(Vec::new, |r| r.to_vec()),
            Fresh::Ship(i) => s.ships.get(i).map_or_else(Vec::new, |r| r.to_vec()),
            Fresh::Building { planet, slot } => s.buildings.get(planet).and_then(|b| b.get(slot)).map_or_else(Vec::new, |r| r.to_vec()),
            Fresh::Design { empire, design } => s.designs.get(empire).and_then(|v| v.get(design)).map_or_else(Vec::new, |r| r.to_vec()),
            // A scenario's garrisons: the new unit objects' uninitialised fields (as in `save_replay.rs`).
            Fresh::GroundUnit { planet, slot } => {
                let mut pairs = Vec::new();
                let u = format!("WF[{}].unit[{slot}]", planet - 1);
                for o in ig2_sim::ground::UNINIT {
                    let b = if (0x264..0x27c).contains(&o) { s.chunk(&format!("{u}+0x264")).map(|b| &b[o - 0x264..]) } else { s.chunk(&format!("{u}+{o:#x}")) };
                    if let Some(b) = b {
                        pairs.extend((o as u32).to_le_bytes());
                        pairs.extend_from_slice(&b[..4]);
                    }
                }
                pairs
            }
            _ => Vec::new(),
        }));
        let mp = j["mp"].as_object().map(|mp| NewMpGalaxy {
            seed: mp["seed"].as_u64().unwrap() as u32,
            ticks: j["seeds"]["ticks"].as_u64().unwrap() as u32,
            planets: int(&mp["planets"]),
            en: int(&mp["en"]),
            humans: mp["humans"].as_array().unwrap().iter().map(int).collect(),
            alive: mp["alive"].as_array().unwrap().iter().map(int).collect(),
            speed: int(&mp["speed"]),
            // Start_Init's races with the lobby's picks.
            races: (0..19u8).map(|i| {
                let pick = mp.get("races").and_then(|r| r.get(i.to_string())).and_then(Value::as_u64);
                pick.map_or(match i {
                    15 | 17 | 18 => 1,
                    16 => 3,
                    _ => i,
                }, |r| r as u8)
            }).collect(),
        });
        let skirmish = j["skirmish"].as_object().map(|s| NewSkirmish {
            en: int(&s["en"]),
            ais: int(&s["ais"]),
            random_id: s.get("randomId").map_or(0, int),
            tech: int(&s["tech"]),
            planets: int(&s["planets"]),
            money: int(&s["money"]),
            difficulty: int(&s["difficulty"]),
            population: int(&s["population"]),
            seed: j["seeds"]["seed"].as_u64().unwrap() as u32,
            ticks: j["seeds"]["ticks"].as_u64().unwrap() as u32,
        });
        match (&scenario, &mp, &skirmish) {
            (Some(sc), _, _) => g.start_scenario(sc, &data),
            (_, Some(mp), _) => g.start_mp_galaxy(mp, &data),
            (_, _, Some(sk)) => g.start_skirmish(sk, &data),
            _ => g.start_campaign(&cfg(), &data),
        }
        // The AI's design names (`DesignPoenosanElnevez`, localised tables the port doesn't render): the exe's.
        for ev in &g.events {
            if let GameEvent::Ai(AiEvent::DesignNamed { empire, design, .. }) = *ev {
                let (e, d) = (empire as usize, design);
                if let (Some(a), Some(b)) = (g.state.designs.get_mut(e).and_then(|v| v.get_mut(d)), want.designs.get(e).and_then(|v| v.get(d))) {
                    a[..0x15].copy_from_slice(&b[..0x15]);
                }
            }
        }
        let mut got = g.state.chunks();
        let mut want = group_chunks(&want);
        mask_uninit(&mut want, &mut got);
        if let Ok(show) = std::env::var("NEWGAME_SHOW") {
            for n in show.split(',') {
                let hex = |m: &BTreeMap<String, Vec<u8>>| m.get(n).map_or(String::new(), |b| b.iter().map(|x| format!("{x:02x}")).collect());
                eprintln!("{n}
  exe  {}
  port {}", hex(&want), hex(&got));
            }
        }
        let d = diff(&want, &got);
        let name = json.file_stem().unwrap().to_string_lossy().to_string();
        eprintln!("{name}: {} differing chunk groups", d.len());
        let lines = std::env::var("NEWGAME_LINES").ok().and_then(|v| v.parse().ok()).unwrap_or(80);
        for line in d.iter().take(lines) {
            eprintln!("  {line}");
        }
        if !d.is_empty() {
            bad.push(name);
        }
        checked += 1;
    }
    eprintln!("{checked} new games checked");
    assert!(bad.is_empty(), "differ: {bad:?}");
}
