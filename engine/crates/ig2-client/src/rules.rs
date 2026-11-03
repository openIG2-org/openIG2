//! The sim's [`GameRules`] from the user's own install: the exe's rule tables (ADR 0004) and the text files the
//! systems read. Mirrors `ig2-sim/tests/save_replay.rs` (`game_rules`), which the replay verifies.

use ig2_formats::{colony_map, misc_text, text, Install};
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
use ig2_sim::spacewar::{DesignRules, GunRules};
use ig2_sim::spacebattle::{FormRules, FormationSlot, ShipType, SpaceRules};
use ig2_sim::spy::{MissionRule, Spy, SpyRules};

/// The rules with the install's mods; none (and no rules read) without rule patches.
fn mod_rules(inst: &Install) -> Option<ig2_rules::Rules> {
    match inst.rule_patches() {
        Ok(p) if !p.is_empty() => ig2_rules::Rules::from_install(inst).ok(),
        _ => None,
    }
}

/// The techs the install's mods add (ADR 0069), for their texts.
pub fn added_techs(inst: &Install) -> Vec<ig2_rules::AddedTech> {
    mod_rules(inst).map(|r| r.added_techs).unwrap_or_default()
}

/// The races and planet types the install's mods add (ADR 0069 §13, §14), for their names.
pub fn added_names(inst: &Install) -> (Vec<ig2_rules::AddedRace>, Vec<ig2_rules::AddedPlanetType>) {
    mod_rules(inst).map(|r| (r.added_races, r.added_planet_types)).unwrap_or_default()
}


/// The rule tables and the `Talalmanyok0` defaults a save is loaded over.
pub fn load(inst: &Install) -> Result<(GameRules, Vec<u8>), String> {
    let r = ig2_rules::Rules::from_install(inst)?;
    let read = |p: &str| inst.read(p).map_err(|e| format!("{p}: {e}"));
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
                like: b.like,
            })
            .collect(),
        curves: r.problem_curves.iter().map(|c| Curve { a: c.a, b: c.b, c: c.c, d: c.d, rate: c.rate, weight: c.weight }).collect(),
        surface_growth: r.race_surface_affinity.clone(),
        self_destruct_fear: r.race_fear_threshold.clone(),
        kidnap_morale: r.kidnap_morale.clone(),
        race_production: r.race_production_mult.clone(),
        surface_like: match r.added_planet_types.last() {
            Some(a) => (0..=a.surface).map(|s| r.surface_role(s) as u8).collect(),
            None => Vec::new(),
        },
        surface_share: match r.added_planet_types.last() {
            Some(a) => (0..=a.surface).map(|s| r.added_planet_types.iter().find(|t| t.surface == s).map_or(0, |t| t.share)).collect(),
            None => Vec::new(),
        },
    };

    let mut modes = vec![AutobuildMode::default()];
    for m in 1..=10 {
        let raw = read(&format!("dataraw/text/autobld{m}.txt"))?;
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
    let raw = read("dataraw/colony/terepleiro.txt")?;
    let surfaces = colony_map::parse_surfaces(&text::decode(&raw, inst.lang())).map_err(|e| e.to_string())?;
    let backgrounds = colony_map::parse_backgrounds(&text::decode(&raw, inst.lang())).map_err(|e| e.to_string())?;
    let maps = surfaces
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let m = colony_map::ColonyMap::parse(&read(&format!("dataraw/colony/gen_maps/{n}.map"))?).map_err(|e| e.to_string())?;
            let terrain = Terrain { mask: m.mask_a, kind: s.kind as u8, start: s.ai_build_start };
            Ok(GroundMap { terrain, heights: m.heights.to_vec(), base: -(m.base - 200.0), footprints: r.building_footprints.clone() })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let terrains = maps.iter().map(|m| m.terrain.clone()).collect();
    let design = DesignRules::from_rules(&r);
    let story = |p: &str| read(p).unwrap_or_default();
    let messages = read("dataraw/text/uzenet.txt")
        .map(|raw| Texts::parse(&raw).with_story(&story("dataraw/text/s_story.txt"), &story("dataraw/text/txt02.txt"), inst.lang()))
        .unwrap_or_default();
    let rules = GameRules {
        side,
        placement: PlacementRules { terrains, footprints: r.building_footprints.clone() },
        research: ResearchRules {
            race_time: r.race_research_time_mult.clone(),
            race_money: r.race_research_money_mult.clone(),
            difficulty_speed: r.difficulty_development_mult.clone(),
            difficulty_money: r.difficulty_money_mult.clone(),
            added_techs: ResearchRules::added(r.added_techs.iter().map(|a| a.index), &r.tech_defaults),
        },
        diplomacy: DiplomacyRules {
            surface_affinity: r.race_surface_affinity.clone(),
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
        messages,
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
        ground_maps: maps,
        space: space_rules(inst, &r)?,
        satellite_prices: r.satellite_prices.clone(),
        inbox: ig2_sim::messages::InboxRules::from_install(inst),
        story: ig2_sim::game::StoryRules {
            mods: ig2_sim::story_mods::load(inst)?.into(),
            ..ig2_sim::game::StoryRules::new(r.campaign_words.clone(), r.story_recruit_names.clone(), &r.design_templates, r.production_template.clone(), inst)
        },
    };
    Ok((rules, r.tech_defaults))
}

/// Globals a save doesn't carry that the scenario normally sets (`replay.js` `VOLATILE`), from the user's exe
/// image: `BirodalomMIzik` 0x915F58 (the AI plays the empire; without it the AI skips everyone) and
/// `BirodalomTamadhat` 0x915F44 (may attack), 19 bytes each.
pub fn scenario_globals(inst: &Install) -> Result<Vec<(u32, Vec<u8>)>, String> {
    let exe = inst.read_raw("ig2.exe").map_err(|e| format!("ig2.exe: {e}"))?;
    let pe = ig2_rules::PeImage::parse(&exe).map_err(|e| e.to_string())?;
    [ig2_sim::save::va::AI_EMPIRE, ig2_sim::save::va::MAY_ATTACK]
        .into_iter()
        .map(|at| pe.read(at, 19).map(|b| (at, b.to_vec())).map_err(|e| e.to_string()))
        .collect()
}

/// The real-time space battle's rules: the exe's weapon, projectile-form and ship-type tables, the buildings' tech
/// and hit points, and the ten formation files.
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
        tech_like: ig2_sim::spacewar::DesignRules::from_rules(r).tech_like,
        added_weapons: r.added_weapons.iter().map(|a| (a.tech, a.index)).collect(),
    })
}
