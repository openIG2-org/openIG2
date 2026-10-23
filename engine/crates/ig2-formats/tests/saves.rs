//! Original save games against live memory. `tools/trace/run_save.py` makes `<tag>-<n>.sav` with the exe's own
//! `LoadSave` and snapshots the saved globals at the same moment (`<tag>-<n>.json`); `--mp` adds the multiplayer
//! state blob (`<tag>-<n>.mpstate`). Every `.sav` in the directory must parse completely; those with a snapshot
//! must match it section by section. Directory: `IG2_SAVE_DIR`, else `_local/saves` of the repository (the
//! files hold game state and are never committed); without saves the test skips.

use std::path::PathBuf;

use ig2_formats::save::{self, SaveGame};
use serde_json::Value;

fn save_dir() -> Option<PathBuf> {
    let dir = match std::env::var_os("IG2_SAVE_DIR") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/saves"),
    };
    dir.is_dir().then_some(dir)
}

fn files(ext: &str) -> Vec<PathBuf> {
    let Some(dir) = save_dir() else { return Vec::new() };
    let mut out = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn hex(v: &Value) -> Vec<u8> {
    let s = v.as_str().unwrap();
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn flat<const N: usize>(recs: &[[u8; N]]) -> Vec<u8> {
    recs.iter().flatten().copied().collect()
}

fn check_snapshot(s: &SaveGame, snap: &Value, what: &str) {
    let i = |k: &str| snap[k].as_i64().unwrap() as i32;
    let h = s.header.as_ref().unwrap();
    for (k, got, hdr) in [("ev", s.year, h.year), ("honap", s.month, h.month), ("nap", s.day, h.day), ("ido", s.ido, h.ido),
        ("old_nap", s.old_day, h.old_day)] {
        assert_eq!(got, i(k), "{what}: {k}");
        assert_eq!(hdr, i(k), "{what}: header {k}");
    }
    assert_eq!(s.ido2, i("ido2"), "{what}: Ido2");
    assert_eq!(s.difficulty, i("difficulty"), "{what}: Nehezseg");
    assert_eq!(s.en, i("en"), "{what}: EN");
    assert_eq!(h.en, Some(i("en")), "{what}: header EN");
    assert_eq!(s.campaign_type, i("campaign_type"), "{what}: CampaignType");

    let raw = |name: &str| s.chunk(name).unwrap_or_else(|| panic!("{what}: no chunk {name}")).to_vec();
    for (key, chunk) in [("money", "Money"), ("money_pools", "MoneyTeruletenkent"), ("rand_seed", "RandSeed"),
        ("player_ki", "PlayerKi"), ("player_faj", "PlayerFaj"), ("birodalom_el", "BirodalomEl"),
        ("birodalom_ismeri", "BirodalomIsmeri"), ("viszony", "Viszony"), ("story_adat", "StoryAdat"),
        ("ossz_hany_kem", "OsszHanyKem"), ("design_counts", "OsszHanyTerv"), ("hany_epulet", "HanyEpulet"),
        ("hany_epulet_lefoglalva", "HanyEpuletLefoglalva"), ("hany_epulet_blokk", "HanyEpuletBlokk"),
        ("mi_gyarthato", "MiGyarthatoCuccok")] {
        assert_eq!(raw(chunk), hex(&snap[key]), "{what}: {chunk}");
    }
    let money: Vec<u8> = s.money.iter().flat_map(|m| m.to_le_bytes()).collect();
    assert_eq!(money, hex(&snap["money"]), "{what}: typed Money");
    let seeds: Vec<u8> = s.rand_seed.iter().flat_map(|m| m.to_le_bytes()).collect();
    assert_eq!(seeds, hex(&snap["rand_seed"]), "{what}: typed RandSeed");

    assert_eq!(s.fleets.len() as i64, snap["n_fleets"].as_i64().unwrap() + 1, "{what}: fleet count");
    assert_eq!(flat(&s.fleets), hex(&snap["fleets"]), "{what}: Flottak");
    assert_eq!(flat(&s.ships), hex(&snap["ships"]), "{what}: Hajok");
    assert_eq!(flat(&s.vehicles), hex(&snap["tanks"]), "{what}: Tankok");
    if snap["n_waypoints"].as_i64().unwrap() != 0 {
        assert_eq!(flat(&s.waypoints), hex(&snap["waypoints"]), "{what}: Waypointek");
    }
    assert_eq!(s.planets.len() as i64, snap["n_planets"].as_i64().unwrap() + 1, "{what}: planet count");
    assert_eq!(flat(&s.planets), hex(&snap["planets"]), "{what}: Bolygok");
    for e in 1..=8 {
        assert_eq!(flat(&s.spies[e]), hex(&snap[format!("kemek{e}")]), "{what}: Kemek[{e}]");
    }
    for e in 1..save::EMPIRES {
        assert_eq!(flat(&s.designs[e]), hex(&snap[format!("designs{e}")]), "{what}: Tervek[{e}]");
    }
    for (p, b) in snap["buildings"].as_array().unwrap().iter().enumerate() {
        assert_eq!(flat(&s.buildings[p + 1]), hex(b), "{what}: Epuletek[{}]", p + 1);
    }
    for (t, v) in snap["techs"].as_array().unwrap().iter().enumerate() {
        let x = &s.techs[t + 1];
        let mut b = x.status.to_vec();
        b.extend(x.time.iter().flat_map(|v| v.to_le_bytes()));
        b.extend(x.given);
        b.extend(x.fields.iter().flat_map(|v| v.to_le_bytes()));
        b.extend(x.seen);
        assert_eq!(b, hex(v), "{what}: Talalmanyok[{}]", t + 1);
    }
    // The views agree with the raw records.
    let en = s.en as u8;
    let owned = (1..s.planets.len()).filter(|&p| s.planet(p).owner() == en).count();
    assert!(owned > 0, "{what}: the human owns no planet");
    for f in 1..s.fleets.len() {
        let v = s.fleet(f);
        assert_eq!(v.owner(), s.fleets[f][0x15]);
    }
}

#[test]
fn saves_parse_and_match_snapshots() {
    let saves = files("sav");
    if saves.is_empty() {
        eprintln!("no saves (IG2_SAVE_DIR / _local/saves); skipping");
        return;
    }
    let mut checked = 0;
    for path in &saves {
        let what = path.display().to_string();
        let file = std::fs::read(path).unwrap();
        let s = SaveGame::parse(&file).unwrap_or_else(|e| panic!("{what}: {e}"));
        let units: usize = s.battlefields.iter().map(|b| b.units.len()).sum();
        eprintln!("{what}: v{:#x} {} {}-{}-{}, {} chunks, {} of {} bytes, {} planets, {} fleets, {} ships, {} vehicle entries, {} ground groups",
            s.version, s.header.as_ref().unwrap().name_str(), s.year, s.month, s.day, s.chunks.len(), s.used, s.body.len(),
            s.planets.len() - 1, s.fleets.len() - 1, s.ships.len() - 1, s.vehicles.len() - 1, units);
        // A rebuilt file decodes to the same stream.
        let again = SaveGame::parse(&s.to_file().unwrap()).unwrap();
        assert_eq!(again.body, s.body, "{what}: rebuild");
        // Writing the chunks back in schema order gives the stream without its padding.
        let mut it = s.chunks.iter();
        let written = SaveGame::write_stream(false, &mut |name, len| {
            let c = it.next().unwrap();
            assert_eq!((c.name.as_str(), c.range.len()), (name, len), "{what}: write order");
            s.body[c.range.clone()].to_vec()
        }).unwrap();
        assert_eq!(written.body, s.body[..s.used], "{what}: write_stream");
        let json = path.with_extension("json");
        if json.exists() {
            let ev: Value = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
            check_snapshot(&s, &ev["snap"], &what);
            assert_eq!(s.header.as_ref().unwrap().name_str(), ev["name"].as_str().unwrap());
            checked += 1;
        }
        let mp = path.with_extension("mpstate");
        if mp.exists() {
            let m = SaveGame::parse_multiplayer(&std::fs::read(&mp).unwrap()).unwrap_or_else(|e| panic!("{what} (mp): {e}"));
            assert!(m.multiplayer && m.mp_time.is_some());
            // The same moment as the file save: identical apart from the multiplayer-only chunks.
            assert_eq!(m.en, s.en, "{what}: mp player");
            assert_eq!(flat(&m.planets), flat(&s.planets), "{what}: mp planets");
            assert_eq!(flat(&m.fleets), flat(&s.fleets), "{what}: mp fleets");
            assert_eq!(m.chunks.len(), s.chunks.len() + 2, "{what}: mp chunk count");
        }
    }
    eprintln!("{} saves parsed, {checked} matched snapshots", saves.len());
}
