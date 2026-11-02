//! Mods add techs and building kinds (ADR 0069), against the user's own ig2.exe (IG2_INSTALL_DIR).

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Rules;

const MOD: &str = include_str!("mods/fusion/rules.toml");

fn rules() -> Option<Rules> {
    let inst = Install::from_env(Lang::En)?.expect("valid install");
    Some(Rules::from_install(&inst).expect("catalogued exe"))
}

#[test]
fn the_test_mod_adds_a_tech_in_a_free_field_and_a_kind_after_the_originals() {
    let Some(mut r) = rules() else { return };
    let before = r.clone();
    assert_eq!(r.apply_patch(MOD).unwrap(), 12);
    // Category 9's first free field is 8 (tech 136); kind 56 is the first after the originals.
    assert_eq!(r.added_techs.len(), 1);
    let t = &r.added_techs[0];
    assert_eq!((t.index, t.name.as_str(), t.description.len()), (136, "Fusion reactor", 2));
    assert_eq!(r.techs.len(), 193);
    let tech = &r.techs[136];
    assert_eq!((tech.prerequisites, tech.research_cost, tech.price, tech.building), ([135, 0, 0], 40000, 9000, 56));
    assert_eq!(r.buildings.len(), 57);
    let b = &r.buildings[56];
    assert_eq!((b.tech, b.like, b.role(), b.output, b.priority), (136, 55, 55, 400, before.buildings[55].priority));
    assert_eq!(r.building_models[56], before.building_models[55]);
    assert_eq!((r.building_footprints[56], r.scaffold_heights[55]), (before.building_footprints[55], before.scaffold_heights[54]));
    // The raw record: unknown to every race, AI priority 8, the building, the patched fields.
    let raw = &r.tech_defaults[136 * 276..137 * 276];
    assert_eq!((&raw[0x45..0x48], &raw[0xb7..0xba], raw[0xbf], raw[0xc0], raw[39]), (&[0, 1, 1][..], &[0, 8, 8][..], 8, 0, 56));
    assert_eq!(i32::from_le_bytes(raw[52..56].try_into().unwrap()), 40000);
    // Nothing else moved: every other raw record and every original kind.
    let other = |d: &[u8]| [&d[..136 * 276], &d[137 * 276..]].concat();
    assert_eq!(other(&r.tech_defaults), other(&before.tech_defaults));
    assert!(r.buildings[..56].iter().zip(&before.buildings).all(|(a, b)| a.raw() == b.raw()));
}

#[test]
fn a_second_mod_takes_the_next_field_and_can_name_the_first_mods_entries() {
    let Some(mut r) = rules() else { return };
    r.apply_patch(MOD).unwrap();
    let more = "[[techs.new]]\nkey = \"hot_fusion\"\ncategory = 9\nprerequisites = [\"fusion_reactor\"]\n\
                [[buildings.new]]\nkey = \"hot_fusion\"\nlike = 55\ntech = \"hot_fusion\"\n";
    r.apply_patch(more).unwrap();
    assert_eq!((r.added_techs[1].index, r.techs[137].prerequisites[0], r.techs[137].building), (137, 136, 57));
    assert_eq!((r.buildings[57].tech, r.added_buildings[1].kind), (137, 57));
    // Defaults: the category's latest AI priority, the key as the name.
    assert_eq!((r.tech_defaults[137 * 276 + 0xb8], r.added_techs[1].name.as_str()), (8, "hot_fusion"));
}

#[test]
fn bad_entries_name_what_is_wrong_and_change_nothing() {
    let Some(r) = rules() else { return };
    let err = |t: &str| r.clone().apply_patch(t).unwrap_err();
    assert_eq!(err("[[techs.new]]\ncategory = 9\n"), "techs.new[0]: needs a key = \"…\"");
    assert_eq!(err("[[techs.new]]\nkey = \"a\"\ncategory = 13\n"), "techs.new \"a\".category: expected an integer 1..=12");
    assert_eq!(err("[[techs.new]]\nkey = \"a\"\ncategory = 1\n"), "techs.new \"a\": category 1 has no free field");
    assert_eq!(err("[[techs.new]]\nkey = \"a\"\ncategory = 9\nfield = 7\n"), "techs.new \"a\".field: category 9 field 7 is tech 135 (-Mezon plant), not a free one");
    assert_eq!(err("[[techs.new]]\nkey = \"a\"\ncategory = 9\n[[techs.new]]\nkey = \"a\"\ncategory = 9\n"), "techs.new \"a\": the key is used twice");
    assert_eq!(err("[[techs.new]]\nkey = \"a\"\ncategory = 9\nprerequisites = [\"b\"]\n"), "techs.new \"a\".prerequisites[0]: no added tech \"b\"");
    assert!(err("[[techs.new]]\nkey = \"a\"\ncategory = 9\nprise = 1\n").starts_with("techs.new \"a\".prise: unknown field (fields: category,"));
    assert_eq!(err("[[buildings.new]]\nkey = \"b\"\nlike = 30\n"), "buildings.new \"b\".like: kind 30 can't be copied (the defences 28..=43 and the unused kinds can't)");
    assert_eq!(err("[[buildings.new]]\nkey = \"b\"\nlike = 56\n"), "buildings.new \"b\".like: expected an integer 1..=55");
    assert_eq!(err("[[buildings.new]]\nkey = \"b\"\nlike = 55\ntech = 3\n"), "buildings.new \"b\".tech: tech 3 is not a building tech (81..=144, research categories 6..=9)");
    assert_eq!(err("[[buildings.new]]\nkey = \"b\"\nlike = 55\npriority = 7\n"), "buildings.new \"b\": needs priority 0..=6 and max_hp above 0");
    assert_eq!(err("[techs]\nnew = 3\n"), "techs.new: expected [[techs.new]] entries");
    // An id patch can still reach a new entry by its number.
    let mut m = r.clone();
    m.apply_patch(&format!("{MOD}[techs.136]\nprice = 1\n[buildings.56]\nenergy = 5\n")).unwrap();
    assert_eq!((m.techs[136].price, m.buildings[56].energy), (1, 5));
}

const ARMADA: &str = include_str!("mods/armada/rules.toml");

#[test]
fn the_ship_mod_appends_a_hull_a_weapon_and_a_tank_and_fits_its_parts() {
    let Some(mut r) = rules() else { return };
    let before = r.clone();
    r.apply_patch(ARMADA).unwrap();
    // The research entries: first free fields of categories 5, 2, 11 and 4.
    let idx: Vec<usize> = r.added_techs.iter().map(|a| a.index).collect();
    assert_eq!(idx, [71, 32, 174, 62]);
    // Hulls 25 (cruiser) and 26 (tank) follow the originals and play their roles.
    assert_eq!(r.hulls.len(), 27);
    let (h, t) = (&r.hulls[25], &r.hulls[26]);
    assert_eq!((h.like, h.tech, h.id, h.class, r.hull_role(25)), (9, 71, 9, 1, 9));
    assert_eq!((t.like, t.tech, t.id, t.class, r.hull_role(26)), (22, 174, 22, 2, 22));
    assert_eq!((r.tech_role(71), r.tech_role(174), r.tech_role(62), r.tech_role(32)), (9, 148, 61, 20));
    assert_eq!((r.ground_hull_hp[26], r.ground_hull_hp[25]), (90000, before.ground_hull_hp[9]));
    assert_eq!(r.unit_models.transfer[26], before.unit_models.transfer[22]);
    // Weapon 40 is the beam laser's record with the mod's reload and range.
    let w = &r.space_weapons[40];
    let b = &before.space_weapons[3];
    assert_eq!((w.reload, w.range, w.graphic, w.accuracy), (120, 80, b.graphic, b.accuracy));
    assert_eq!(r.added_weapons[0].tech, 32);
    assert_eq!(r.space_weapon_table().len(), 41 * 0x5c);
    // Parts fit wherever their original fits; the tank's tech is no pod.
    let lists = |t: i32| r.pod_types.iter().filter(|p| p.techs.contains(&t)).map(|p| p.id).collect::<Vec<_>>();
    assert_eq!(lists(32), lists(20));
    assert_eq!(lists(62), lists(61));
    assert!(lists(174).is_empty());
    assert_eq!(r.research_screen.models[61], before.research_screen.models[60]);
    // Nothing original moved.
    assert!(r.hulls[..25].iter().zip(&before.hulls).all(|(a, b)| (a.tech, a.like) == (b.tech, 0)));
    assert_eq!(r.space_weapon_table()[..40 * 0x5c], before.space_weapon_table()[..]);
}

#[test]
fn bad_items_name_what_is_wrong() {
    let Some(r) = rules() else { return };
    let err = |t: &str| r.clone().apply_patch(t).unwrap_err();
    let tech = "[[techs.new]]\nkey = \"a\"\ncategory = 5\n";
    assert_eq!(err("[[hulls.new]]\nkey = \"a\"\nlike = 24\n"), "hulls.new \"a\".like: hull 24 is unused");
    assert_eq!(err("[[hulls.new]]\nkey = \"a\"\nlike = 9\n"), "hulls.new \"a\": needs tech = <an added tech>");
    assert_eq!(err("[[hulls.new]]\nkey = \"a\"\nlike = 9\ntech = 9\n"), "hulls.new \"a\".tech: tech 9 is not a mod's tech");
    assert_eq!(err(&format!("{tech}[[hulls.new]]\nkey = \"a\"\nlike = 9\n[[hulls.new]]\nkey = \"b\"\nlike = 9\ntech = \"a\"\n")), "hulls.new \"b\".tech: tech 71 already is another hull's or weapon's");
    assert_eq!(err(&format!("{tech}[[space_weapons.new]]\nkey = \"a\"\nlike = 30\n")), "space_weapons.new \"a\".tech: weapon 30 has no pod tech: give tech 71 a like");
    assert_eq!(err(&format!("{tech}like = 100\n")), "techs.new \"a\".like: tech 100 is not an original ship or tank item (1..=80, 145..=176)");
    assert!(err(&format!("{tech}[[space_weapons.new]]\nkey = \"a\"\nlike = 3\nrelaod = 1\n")).starts_with("space_weapons.new \"a\".relaod: unknown field (fields: like, tech, kind,"));
}
