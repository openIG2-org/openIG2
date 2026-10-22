//! Golden tests against the user's own ig2.exe (IG2_INSTALL_DIR). Values checked here were also
//! observed in live game memory with Frida (re/rules/traces/2026-10-15-tables.md).

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Rules;

fn rules() -> Option<Rules> {
    let inst = Install::from_env(Lang::En)?.expect("valid install");
    Some(Rules::from_install(&inst).expect("catalogued exe"))
}

#[test]
fn tables_have_catalogued_shapes() {
    let Some(r) = rules() else { return };
    assert_eq!(r.techs.len(), 193);
    assert_eq!(r.buildings.len(), 56);
    // Footprints by kind: 1..=55 are real buildings (positive sizes), 56..=60 empty records.
    assert_eq!(r.building_footprints.len(), 61);
    assert!(r.building_footprints[1..=55].iter().all(|&(w, h)| w > 0 && h > 0 && w <= 20 && h <= 20));
    assert!(r.building_footprints[56..].iter().all(|&f| f == (0, 0)));
    // `allvanypoziciok` (ELF 0x115f740) and `Building[0]+0x68` (ELF +0x6c): the selection marker of the colony hub.
    assert_eq!(r.scaffold_heights.len(), 60);
    assert_eq!(&r.scaffold_heights[..4], &[170.0, 15.0, 205.0, 50.0]);
    assert_eq!(r.building_models[1].select, 0.59);
    assert_eq!(r.pod_types.len(), 55);
    assert_eq!(r.pod_slots.len(), 14);
    assert!(r.pod_slots.iter().all(|t| t.len() == 27 && t.iter().all(|h| h.len() == 12)));
    assert_eq!(r.hulls.len(), 25);
    assert_eq!(r.designs.len(), 201);
    assert_eq!(r.space_weapons.len(), 40);
    assert_eq!(r.race_surface_affinity.len(), 15);
    assert_eq!(r.problem_curves.len(), 20);
    assert_eq!(r.kidnap_morale.len(), 6);
    assert_eq!((r.radar_ranges.len(), r.planet_radar_ranges.len()), (6, 8));
    // Radar ranges grow with the tech level; planet radar levels 3..=7 reuse tech levels 1..=5.
    assert!(r.radar_ranges.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(&r.planet_radar_ranges[3..], &r.radar_ranges[1..]);
    // Every building's priority class indexes the 7 energy/worker pools (colony-economy.md §4).
    assert!(r.buildings.iter().all(|b| b.priority < 7));
}

#[test]
fn cross_references_are_consistent() {
    let Some(r) = rules() else { return };
    // Every prerequisite is a valid tech index.
    for t in &r.techs {
        for &p in &t.prerequisites {
            assert!((p as usize) < r.techs.len(), "tech {} prerequisite {p}", t.index);
        }
    }
    // Every slot's pod type exists in PodTipusok (or is 0 = empty).
    let ids: std::collections::BTreeSet<i32> = r.pod_types.iter().map(|p| p.id).collect();
    for table in &r.pod_slots {
        for hull in table {
            for s in hull {
                assert!(s.pod_type == 0 || ids.contains(&s.pod_type), "unknown pod type {}", s.pod_type);
            }
        }
    }
    // Every pod type's allowed techs exist; every building's cost tech exists.
    assert!(r.pod_types.iter().all(|p| p.techs.iter().all(|&t| t > 0 && (t as usize) < r.techs.len())));
    assert!(r.buildings.iter().all(|b| (b.tech as usize) < r.techs.len()));
}

#[test]
fn spot_values_match_live_game() {
    let Some(r) = rules() else { return };
    // Tech 1 ("Fighter"): firepower 10, as res_info.txt also states; price/time read by GetGyartasAr/Ido.
    assert!(r.techs[1].name.ends_with("Fighter"));
    assert_eq!(r.techs[1].firepower, 10);
    assert_eq!((r.techs[1].price, r.techs[1].build_time), (6000, 300000));
    assert_eq!((r.techs[1].research_cost, r.techs[1].research_time), (10000, 6000));
    // Race multipliers: index 1 (Solarian) researches 30% faster, matching s_races.txt
    // ("Solarian Empire: Their research is 30% faster").
    assert!((r.race_research_time_mult[1] - 0.7).abs() < 1e-6);
}

#[test]
fn hull_weapon_design_fields_are_consistent() {
    let Some(r) = rules() else { return };
    // Hull +8 is its own index; +0xc is a valid tech; classes are 0..=2 (space-combat.md §2.1).
    assert!(r.hulls.iter().all(|h| h.id as usize == h.index && (h.tech as usize) < r.techs.len() && (0..=2).contains(&h.class)));
    // Ground hulls (class 2) are exactly the hulls whose tech has a TankAlapSpeed entry.
    assert!(r.hulls.iter().filter(|h| h.class == 2).all(|h| (145..150).contains(&h.tech)));
    assert!(r.tank_speeds.iter().all(|&s| s > 0.0));
    // Default designs: hull index (+0x44) and slot count (+0x4c) are in range, pod techs valid.
    for d in r.designs.iter().filter(|d| !d.name.is_empty()) {
        let hull = i32::from_le_bytes(d.raw[0x44..0x48].try_into().unwrap());
        let slots = i32::from_le_bytes(d.raw[0x4c..0x50].try_into().unwrap());
        assert!((0..25).contains(&hull) && (0..=12).contains(&slots), "design {}", d.index);
        assert!(d.raw[0x51..0x5d].iter().all(|&t| (t as usize) < r.techs.len()));
    }
    // Weapons: positive reload; range is the field fegyverhozzaad tracks (> 0 for used records).
    assert!(r.space_weapons.iter().filter(|w| w.reload > 0).all(|w| w.range > 0 && w.accuracy <= 100));
}

#[test]
fn hud_pictures_and_brushes_line_up() {
    let Some(r) = rules() else { return };
    // re/screens/hud.md §1, §3: kep 17 is the colony HUD's sheet; its brushes 12 (planet panel) and 13 (radar frame)
    // sit where the reference captures show them; 28 is the radar picture on kep 1.
    assert_eq!(r.pictures.len(), 107);
    assert_eq!(r.pictures[17], "COLONY_X.BMP");
    assert_eq!(r.pictures[14], "BRUSHOK.BMP");
    assert_eq!(r.colony_brushes.len(), 33);
    let (p, f) = (r.colony_brushes[12], r.colony_brushes[13]);
    assert_eq!((p.kep, p.dst, p.size), (17, [0, 18], [-223, -153]));
    assert_eq!((f.kep, f.dst, f.size), (17, [491, 348], [-149, -132]));
    assert_eq!((r.colony_brushes[28].kep, r.colony_brushes[28].dst), (1, [522, 362]));
    assert_eq!(r.starmap_brushes.len(), 13);
    assert_eq!(r.starmap_brushes[4].kep, 30);
}

#[test]
fn research_screen_tables_line_up() {
    let Some(r) = rules() else { return };
    // re/screens/research.md §2: the middle wheel starts radar, weapons, shields, bombs, ships (mask colours 9, 16,
    // 11, 5, 12); every tech has a model slot; the mask picture is kep 0x58.
    let s = &r.research_screen;
    assert_eq!(s.wheel_kep, [9, 16, 11, 5, 12]);
    assert_eq!(s.wheel_area, [3, 2, 4, 5, 1]);
    assert_eq!(s.wheel_button, [100, 102, 103, 104, 105]);
    assert_eq!(s.wheel_line, [[322, 353]; 5]);
    assert_eq!(s.models.len(), 192);
    assert_eq!(s.orbit.len(), 17);
    assert_eq!(r.pictures[0x58], "RESE_MSK.RAW");
    eprintln!("models {:?}\norbit {:?}\nlabs {:?} {:?} {:?}", &s.models[..32], s.orbit, s.lab_x, s.lab_x2, s.lab_y);
}

#[test]
fn design_screen_hull_models_line_up() {
    let Some(r) = rules() else { return };
    // re/screens/design.md §5.1: 25 hull models; model 0 has none, the others are .xvb files with a positive scale.
    let s = &r.design_screen;
    assert_eq!((s.models.len(), s.scale.len()), (25, 25));
    assert!(s.models[0].is_empty());
    assert!(s.models[1..].iter().all(|m| m.to_ascii_lowercase().ends_with(".xvb")));
    assert!(s.scale[1..].iter().all(|&k| k > 0.0));
    // Every race uses one of the eight pod tables; a hull's slots are filled from the first.
    assert!(s.pod_table[1..=8].iter().all(|t| (1..=8).contains(t)));
    // The four 9 × 9 cells side by side on kep 0x13 (`DESIGN_X.BMP`).
    assert_eq!(s.brushes.iter().map(|b| (b.src, b.size, b.kep)).collect::<Vec<_>>(), (0..4).map(|i| ([9 * i, 0], [9, 9], 0x13)).collect::<Vec<_>>());
    for race in 1..=8 {
        for hull in 0..25 {
            let p = s.pod_slots(race, hull);
            let n = p.iter().filter(|&&c| c > 0).count();
            assert!(p[..n].iter().all(|&c| c > 0), "race {race} hull {hull}: {p:?}");
        }
    }
}

#[test]
fn sound_tables_name_the_installs_files() {
    use ig2_rules::sounds::{install_paths, SoundTables, Table};
    let Some(inst) = Install::from_env(Lang::En) else { return };
    let inst = inst.expect("valid install");
    let exe = inst.read_raw("ig2.exe").expect("ig2.exe");
    let t = SoundTables::from_exe(&exe).expect("catalogued exe");
    let counts: Vec<usize> = t.tables.iter().map(Vec::len).collect();
    assert_eq!(counts, [11, 58, 44, 4, 3, 6, 19, 4, 42, 3, 2]);
    // Every record has its file except Colony_Hangok[27..=36], ten night bird calls the GOG install lacks
    // (re/screens/sound.md §1).
    for table in Table::ALL {
        for (i, h) in t.tables[table as usize].iter().enumerate() {
            let found = install_paths(&h.name).iter().any(|p| inst.resolve(p).is_some());
            assert_eq!(found, !(table == Table::Colony && (27..=36).contains(&i)), "{}[{i}] {}", table.name(), h.name);
            assert!([0, 4, 6].contains(&h.looping) && (1..=100).contains(&h.volume) && !h.preload);
        }
    }
    assert_eq!(t.music.len(), 19);
    assert!(t.music.iter().all(|m| m.starts_with("mfx/") && inst.resolve(m).is_some()));
    assert_eq!(t.music[12], "mfx/menu.ogg");
}

/// The video name tables (`re/screens/video.md` §5): every name resolves to a file of the install.
#[test]
fn video_tables_resolve() {
    let Some(r) = rules() else { return };
    let inst = ig2_formats::Install::from_env(ig2_formats::text::Lang::En).unwrap().unwrap();
    let v = &r.videos;
    assert_eq!((v.loss.len(), v.win.len(), v.message_clips.len()), (9, 9, 15));
    let names = v.loss.iter().chain(&v.win).chain(v.message_clips.iter().flat_map(|(a, b)| [a, b]));
    let mut n = 0;
    for name in names.filter(|n| !n.is_empty()) {
        let mp4 = format!("anim/mpeg4/{}.mp4", name.strip_suffix(".bik").unwrap());
        assert!(inst.resolve(&mp4).is_some(), "{name}");
        n += 1;
    }
    assert_eq!(n, 16 + 20);
}
