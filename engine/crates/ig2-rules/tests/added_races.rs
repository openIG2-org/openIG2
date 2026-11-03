//! Mods add races and planet types (ADR 0069 §13, §14), against the user's own ig2.exe (IG2_INSTALL_DIR).

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Rules;

const MOD: &str = include_str!("mods/frontier/rules.toml");

fn rules() -> Option<Rules> {
    let inst = Install::from_env(Lang::En)?.expect("valid install");
    Some(Rules::from_install(&inst).expect("catalogued exe"))
}

#[test]
fn the_test_mod_adds_race_15_like_an_original_and_planet_type_9() {
    let Some(mut r) = rules() else { return };
    let before = r.clone();
    assert_eq!(r.apply_patch(MOD).unwrap(), 10);
    let (a, t) = (&r.added_races[0], &r.added_planet_types[0]);
    assert_eq!((a.race, a.like, a.ai, a.name.as_str(), a.plural.as_str()), (15, 3, 5, "Vorlani", "Vorlanis"));
    assert_eq!((t.surface, t.like, t.share, t.name.as_str()), (9, 4, 40, "Ash world"));
    assert_eq!((r.race_role(15), r.race_role(3), r.surface_role(9), r.surface_role(4)), (3, 3, 4, 4));
    // The like's entries in every race table, the AI's from `ai`, the traits the entry sets.
    assert_eq!((r.race_pod_table[15], r.race_fear_threshold[15], r.spy_race_cost_mult[15]), (before.race_pod_table[3], before.race_fear_threshold[3], before.spy_race_cost_mult[3]));
    assert_eq!((r.race_ai_production_share[15], r.race_ai_military_autobuild[15]), (before.race_ai_production_share[5], before.race_ai_military_autobuild[5]));
    assert_eq!((r.race_production_mult[15], r.race_research_time_mult[15], r.race_research_money_mult[15]), (1.25, 0.9, before.race_research_money_mult[3]));
    let types = |r: &Rules, race: usize| r.ship_types[race * 12..race * 12 + 12].iter().map(|s| s.top_speed.to_bits()).collect::<Vec<_>>();
    assert_eq!(types(&r, 15), types(&before, 3));
    assert_eq!(r.diplomacy_screen.portraits[28..30], before.diplomacy_screen.portraits[4..6]);
    // The affinity table: a column 9 (the like's, then the entry's cells) and a row 15 (race 3's, then its cells).
    assert!(r.race_surface_affinity.iter().all(|row| row.len() == 10));
    assert_eq!((r.race_surface_affinity[1][9], r.race_surface_affinity[4][9], r.race_surface_affinity[2][9]), (60, 90, before.race_surface_affinity[2][4]));
    assert_eq!((r.race_surface_affinity[15][2], r.race_surface_affinity[15][9], r.race_surface_affinity[15][7]), (130, 160, before.race_surface_affinity[3][7]));
    // The originals' rows keep their 9 columns.
    assert!(r.race_surface_affinity[..15].iter().zip(&before.race_surface_affinity).all(|(a, b)| a[..9] == b[..]));
}

#[test]
fn a_second_mod_takes_the_next_numbers_and_names_the_first_mods_keys() {
    let Some(mut r) = rules() else { return };
    r.apply_patch(MOD).unwrap();
    r.apply_patch("[[planet_types.new]]\nkey = \"reef\"\nlike = 1\naffinity = { vorlani = 10 }\n[[races.new]]\nkey = \"kith\"\nlike = 8\naffinity = { ash_world = 5 }\n").unwrap();
    assert_eq!((r.added_planet_types[1].surface, r.added_races[1].race, r.added_races[1].ai), (10, 16, 8));
    assert_eq!((r.race_surface_affinity[15][10], r.race_surface_affinity[16][9], r.race_surface_affinity[16].len()), (10, 5, 11));
    // Defaults: the key as the name and the plural.
    assert_eq!((r.added_races[1].name.as_str(), r.added_races[1].plural.as_str(), r.added_planet_types[1].share), ("kith", "kith", 0));
}

#[test]
fn bad_races_and_planet_types_name_what_is_wrong_and_change_nothing() {
    let Some(r) = rules() else { return };
    let err = |t: &str| r.clone().apply_patch(t).unwrap_err();
    assert_eq!(err("[[races.new]]\nlike = 3\n"), "races.new[0]: needs a key = \"…\"");
    assert_eq!(err("[[races.new]]\nkey = \"a\"\n"), "races.new \"a\": needs like = <an original race 1..=14>");
    assert_eq!(err("[[races.new]]\nkey = \"a\"\nlike = 15\n"), "races.new \"a\".like: expected an integer 1..=14");
    assert_eq!(err("[[races.new]]\nkey = \"a\"\nlike = 1\nai = 0\n"), "races.new \"a\".ai: expected an integer 1..=14");
    assert_eq!(err("[[races.new]]\nkey = \"a\"\nlike = 1\n[[races.new]]\nkey = \"a\"\nlike = 2\n"), "races.new \"a\": the key is used twice");
    assert!(err("[[races.new]]\nkey = \"a\"\nlike = 1\ncolour = 3\n").starts_with("races.new \"a\".colour: unknown field (fields: like, ai, name,"));
    assert_eq!(err("[[races.new]]\nkey = \"a\"\nlike = 1\naffinity = { lava = 1 }\n"), "races.new \"a\".affinity.lava: no added planet type \"lava\"");
    assert_eq!(err("[[races.new]]\nkey = \"a\"\nlike = 1\naffinity = { 9 = 1 }\n"), "races.new \"a\".affinity.9: expected a planet type 1..=8 or an added type's key");
    assert_eq!(err("[[races.new]]\nkey = \"a\"\nlike = 1\naffinity = 3\n"), "races.new \"a\".affinity: expected { <key> = <growth %>, … }");
    assert_eq!(err("[[races.new]]\nkey = \"a\"\nlike = 1\nproduction = \"x\"\n"), "races.new \"a\".production: expected a number, got string");
    assert_eq!(err("[[planet_types.new]]\nkey = \"p\"\nlike = 9\n"), "planet_types.new \"p\".like: expected an integer 1..=8");
    assert_eq!(err("[[planet_types.new]]\nkey = \"p\"\nlike = 1\nshare = 101\n"), "planet_types.new \"p\".share: expected an integer 0..=100");
    assert_eq!(err("[[planet_types.new]]\nkey = \"p\"\nlike = 1\naffinity = { 0 = 1 }\n"), "planet_types.new \"p\".affinity.0: expected a race 1..=14 or an added race's key");
    assert_eq!(err("[races.3]\nproduction = 1\n"), "races.3: only [[races.new]] entries (the race tables are patched by name, ADR 0044 §6)");
}
