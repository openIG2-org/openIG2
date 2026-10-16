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
