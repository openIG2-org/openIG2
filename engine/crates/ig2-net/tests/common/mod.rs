//! Shared test set-up: the rule tables from the user's install (as `ig2-sim`'s save replay builds them) and the
//! test saves. Saves hold game state, so they stay in `_local/saves` (never committed; `IG2_SAVE_DIR` points
//! elsewhere). The tests skip without `IG2_INSTALL_DIR` or the save.

use std::path::PathBuf;

use ig2_formats::save::SaveGame;
use ig2_formats::text::{self, Lang};
use ig2_formats::{colony_map, misc_text, Install};
use ig2_sim::ai::{AiRules, PodRule};
use ig2_sim::colony::{BuildingKind, ColonyRules, Curve};
use ig2_sim::colony_side::{tech_strength, AutobuildMode, AutobuildRules, SideRules};
use ig2_sim::diplomacy::DiplomacyRules;
use ig2_sim::game::GameRules;
use ig2_sim::ground::{GroundMap, GroundRules};
use ig2_sim::messages::Texts;
use ig2_sim::placement::{PlacementRules, Terrain};
use ig2_sim::radar::RadarRules;
use ig2_sim::research::ResearchRules;
use ig2_sim::save::{va, SaveState, PLANET_SIZE};
use ig2_sim::spacewar::{DesignRules, GunRules, Hull};
use ig2_sim::spacebattle::{FormRules, FormationSlot, ShipType, SpaceRules};
use ig2_sim::spy::{MissionRule, Spy, SpyRules};

/// The rules and `_local/saves/<name>` as sim state, if both are there.
pub fn setup(name: &str) -> Option<(GameRules, SaveState)> {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return None;
    };
    let dir = std::env::var_os("IG2_SAVE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/saves"));
    let path = dir.join(name);
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("{} missing; skipping", path.display());
        return None;
    };
    let save = SaveGame::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let tech_defaults = ig2_rules::Rules::from_install(&inst).expect("catalogued exe").tech_defaults;
    Some((game_rules(&inst), SaveState::from_save(&save, &tech_defaults)))
}

pub fn game_rules(inst: &Install) -> GameRules {
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
        space: space_rules(inst, &r).expect("battle rules"),
        satellite_prices: r.satellite_prices.clone(),
        inbox: ig2_sim::messages::InboxRules::from_install(inst),
        story: ig2_sim::game::StoryRules::new(r.campaign_words.clone(), r.story_recruit_names.clone(), &r.design_templates, r.production_template.clone(), inst),
    }
}

/// The real-time space battle's rules: the exe's weapon, projectile-form and ship-type tables, the buildings' tech
/// and hit points, and the two formation files.
pub fn space_rules(inst: &Install, r: &ig2_rules::Rules) -> Result<SpaceRules, String> {
    let (weapons, weapon_words) = SpaceRules::weapons_from_table(&r.space_weapon_table());
    let formation = |n: usize| -> Result<Vec<FormationSlot>, String> {
        let p = format!("dataraw/spacewar/felallas{n}.bin");
        let b = inst.read(&p).map_err(|e| format!("{p}: {e}"))?;
        let slots = ig2_formats::formation::parse(&b).map_err(|e| format!("{p}: {e}"))?;
        Ok(slots.iter().map(|s| FormationSlot::from_record(s.cell, s.kind, &s.extra)).collect())
    };
    Ok(SpaceRules {
        weapons,
        weapon_words,
        forms: r.shot_forms.iter().map(|f| FormRules { loaded: !f.model.is_empty(), slots: f.slots.max(0) as usize, size: f.size }).collect(),
        formations: (0..ig2_sim::spacebattle::FORMATION_FILES).map(formation).collect::<Result<_, _>>()?,
        ship_types: r
            .ship_types
            .iter()
            .map(|t| ShipType { scale: t.scale, top_speed: t.top_speed, accel: t.accel, turn: t.turn, size_xy: t.size_xy, size_z: t.size_z })
            .collect(),
        building_tech: r.buildings.iter().map(|b| b.tech).collect(),
        building_hp: r.buildings.iter().map(|b| b.max_hp).collect(),
    })
}

/// Empire 3 becomes a second human (the session runs the MP rules) and gets a fleet of warships next to a fleet of
/// 6's beside one of 6's planets, ordered to attack it: a real-time space battle between the two humans.
#[allow(dead_code)]
pub fn space_clash(mut state: SaveState) -> SaveState {
    state.mem.set_u8(va::PLAYER_KI + 3, 1);
    state.mem.set_u8(va::MULTIPLAYER, 1);
    state.mem.set_i32(0xBAE558, 0);
    // MP normal speed (×4; the save holds 0): the battles step once per tick (`SetIdoSpeed`).
    state.mem.set_i32(va::IDO_SPEED, 4);
    let combat = state.combat();
    let warship = |e: usize| {
        (1..combat.designs[e].len())
            .find(|&d| {
                let x = &combat.designs[e][d];
                x.class == 1 && !x.is_fighter() && x.attack > 0 && x.hp > 0
            })
            .unwrap_or_else(|| panic!("empire {e} has a warship design"))
    };
    let (d6, d3) = (warship(6), warship(3));
    // A fleet for each next to one of 6's planets (records like fleet 1's, a plain fleet), 3's ordered to attack 6's.
    let home = (1..=state.planet_count()).find(|&p| state.mem.u8(va::PLANETS + (p * PLANET_SIZE) as u32 + 0x20) == 6).expect("a planet of 6");
    let at = va::PLANETS + (home * PLANET_SIZE) as u32;
    let (hx, hy) = (f32::from_le_bytes(state.mem.read(at + 0x18, 4).try_into().unwrap()), f32::from_le_bytes(state.mem.read(at + 0x1c, 4).try_into().unwrap()));
    for owner in [6u8, 3] {
        let mut rec = state.fleets[1];
        rec[0x15] = owner;
        state.fleets.push(rec);
    }
    let (a, b) = (state.fleets.len() - 2, state.fleets.len() - 1);
    state.mem.set_i32(va::FLEET_COUNT, b as i32);
    for (f, d, n) in [(a, d6, 6), (b, d3, 6)] {
        for _ in 0..n {
            let mut s = [0u8; 6];
            s[0..2].copy_from_slice(&(f as i16).to_le_bytes());
            s[2..4].copy_from_slice(&(d as i16).to_le_bytes());
            state.ships.push(s);
        }
    }
    state.mem.set_i32(va::SHIP_COUNT, state.ships.len() as i32 - 1);
    let mut map = state.starmap();
    for f in [a, b] {
        (map.fleets[f].x, map.fleets[f].y, map.fleets[f].order, map.fleets[f].flags) = (hx + 3.0, hy + 3.0, 0, 0);
        map.fleets[f].name = [0; 21];
        map.fleets[f].name[..5].copy_from_slice(b"Probe");
    }
    map.fleets[b].x += 1.0;
    (map.fleets[b].order, map.fleets[b].kind, map.fleets[b].tx) = (ig2_sim::fleet::order::ATTACK, ig2_sim::fleet::order::FLEET, a as f32);
    state.store_starmap(&map);
    state
}

/// The install's start data with placeholder texts (both ends use the same, so the states agree).
#[allow(dead_code)]
pub fn with_start_data<T>(inst: &ig2_formats::Install, f: impl FnOnce(&ig2_sim::game::new_game::StartData) -> T) -> T {
    let exe = inst.read_raw("ig2.exe").unwrap();
    let pe = ig2_rules::PeImage::parse(&exe).unwrap();
    let image = |at: u32, len: usize| pe.read_loaded(at, len).unwrap_or_else(|_| vec![0; len]);
    let scenario = |_: i32| None;
    let names = |n: usize, p: &str| (0..n).map(|i| format!("{p}{i}").into_bytes()).collect::<Vec<_>>();
    let data = ig2_sim::game::new_game::StartData {
        image: &image,
        scenario: &scenario,
        race_names: names(20, "race"),
        constellations: names(15, "Const"),
        tech_names: names(193, "tech"),
        design_names: names(47, "design"),
        fleet_name: b"Fleet".to_vec(),
        design_texts: Default::default(),
        message_kinds: 0,
    };
    f(&data)
}
