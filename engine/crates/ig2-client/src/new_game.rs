//! A new game from the user's install (re/rules/new-game.md): what `ig2_sim`'s start constructors take besides their
//! config ([`StartData`]: the exe image and the texts `StringReadAll` and the start read), and the campaign start
//! the race menu's `CampaignGameStart` (exe 0x4f5e90, re/screens/mainscr.md §9.1) runs. [`crate::world::World::new_game`]
//! turns the `Game` into the client's world.

use ig2_formats::strings::{KeyedStrings, UiStrings};
use ig2_formats::text::{self, Lang};
use ig2_formats::{dialogue, Install};
use ig2_sim::game::new_game::{NewCampaign, NewScenario, NewSkirmish, StartData};
use ig2_sim::game::{Game, GameRules};

/// The install's part of a start, owned (the sim's [`StartData`] borrows it).
pub struct StartTexts {
    exe: Vec<u8>,
    race_names: Vec<Vec<u8>>,
    constellations: Vec<Vec<u8>>,
    tech_names: Vec<Vec<u8>>,
    design_names: Vec<Vec<u8>>,
    fleet_name: Vec<u8>,
    design_texts: std::collections::BTreeMap<i32, Vec<u8>>,
    message_kinds: i32,
    /// The mods' rule tables over the exe image (ADR 0044 §7, ADR 0069): `Talalmanyok0` and `EpuletekLeiro`, which
    /// the start copies from the exe; empty without rule patches.
    tables: Vec<(u32, Vec<u8>)>,
}

impl StartTexts {
    /// `StringReadAll` (exe 0x531e40) writes the **Hungarian** `txt17` names into the exe's tables whatever the
    /// language (the race, tech and design names, `AlapFlottaNev`); the constellations `_s(0x1d, k)` and the message
    /// kinds (`LoadMessagesFile`) are the install's language.
    pub fn read(inst: &Install) -> Result<Self, String> {
        let exe = inst.read_raw("ig2.exe").map_err(|e| e.to_string())?;
        let mut hu = UiStrings::default();
        if let Ok(b) = inst.read("localization/hu/dataraw/text/txt17.txt").or_else(|_| inst.read("dataraw/text/txt17.txt")) {
            hu.add_file(&text::decode(&b, Lang::Hu));
        }
        let hu_bytes = |s: &str| Lang::Hu.encoding().encode(s).0.into_owned();
        let enc = |s: &str| inst.lang().encoding().encode(s).0.into_owned();
        let mut keyed = KeyedStrings::default();
        if let Ok(b) = inst.read("dataraw/text/s_stname.txt") {
            keyed.add_file(29, &text::decode(&b, inst.lang()));
        }
        let kinds = inst.read("dataraw/uzenetek/uzenetek.txt").map(|b| dialogue::parse_message_index(&text::decode(&b, inst.lang())).len()).unwrap_or(0);
        let mut tech_names: Vec<Vec<u8>> =
            (0..193u16).map(|t| if t == 0 { Vec::new() } else { hu_bytes(hu.get(0x11, 8 + (t - 1) / 48, (t - 1) % 48 + 1)) }).collect();
        let mut tables = Vec::new();
        if !inst.rule_patches().map_err(|e| e.to_string())?.is_empty() {
            let r = ig2_rules::Rules::from_install(inst).map_err(|e| e.to_string())?;
            for a in &r.added_techs {
                if matches!(&tech_names[a.index][..], b"" | b"-") {
                    tech_names[a.index] = hu_bytes(&a.name);
                }
            }
            let originals = ig2_rules::layout::BUILDINGS.2;
            tables.push((ig2_rules::layout::TECHS.0, r.tech_defaults));
            tables.push((ig2_rules::layout::BUILDINGS.0, r.buildings[..originals].iter().flat_map(|b| b.raw()).collect()));
        }
        Ok(Self {
            exe,
            race_names: (0..20).map(|r| hu_bytes(hu.get(0x11, 1, r + 5))).collect(),
            constellations: (1..=15).map(|k| enc(keyed.get(29, k).unwrap_or(""))).collect(),
            tech_names,
            design_names: (0..47u16).map(|k| hu_bytes(hu.get(0x11, 7, k + 1))).collect(),
            fleet_name: hu_bytes(hu.get(17, 5, 46)),
            design_texts: design_texts(inst),
            message_kinds: kinds as i32,
            tables,
        })
    }

    /// The sim's view of these, with the exe image (`PeImage::read_loaded`, BSS as zero) and `scenario/<n>.scn`.
    pub fn with<T>(&self, inst: &Install, f: impl FnOnce(&StartData) -> T) -> Result<T, String> {
        let pe = ig2_rules::PeImage::parse(&self.exe).map_err(|e| e.to_string())?;
        let image = |at: u32, len: usize| {
            let mut v = pe.read_loaded(at, len).unwrap_or_else(|_| vec![0; len]);
            for (va, t) in &self.tables {
                // The overlap of [at, at + len) and the table.
                let (lo, hi) = (at.max(*va) as usize, (at as usize + len).min(*va as usize + t.len()));
                if lo < hi {
                    v[lo - at as usize..hi - at as usize].copy_from_slice(&t[lo - *va as usize..hi - *va as usize]);
                }
            }
            v
        };
        let scenario = |n: i32| inst.read(&format!("dataraw/scenario/{n}.scn")).ok().map(|b| text::decode(&b, inst.lang()));
        let data = StartData {
            image: &image,
            scenario: &scenario,
            race_names: self.race_names.clone(),
            constellations: self.constellations.clone(),
            tech_names: self.tech_names.clone(),
            design_names: self.design_names.clone(),
            fleet_name: self.fleet_name.clone(),
            design_texts: self.design_texts.clone(),
            message_kinds: self.message_kinds,
        };
        Ok(f(&data))
    }
}

/// `_s(0x3b, n)`: `s_design.txt` in the install's language, encoded (`Game::design_texts`).
pub fn design_texts(inst: &Install) -> std::collections::BTreeMap<i32, Vec<u8>> {
    let mut keyed = KeyedStrings::default();
    if let Ok(b) = inst.read("dataraw/text/s_design.txt") {
        keyed.add_file(0x3b, &text::decode(&b, inst.lang()));
    }
    keyed.table(0x3b).map(|(n, s)| (n as i32, inst.lang().encoding().encode(s).0.into_owned())).collect()
}

/// `CampaignGameStart`'s `InitNewGame(seed, 0, race + 1, Nehezseg)`.
pub fn campaign<'r>(inst: &Install, cfg: &NewCampaign, rules: &'r GameRules) -> Result<Game<'r>, String> {
    StartTexts::read(inst)?.with(inst, |data| Game::new_campaign(cfg, data, rules))
}

/// `Betolt_Scenario(file)` (exe 0x4f5ba0): `scenario/<name>.scn` started with the wall clock's seeds and the config's
/// `Difficulty` (`GetParam`'s default 2).
pub fn scenario<'r>(inst: &Install, file: &str, seed: u32, rules: &'r GameRules) -> Result<Game<'r>, String> {
    let text = inst.read(&format!("dataraw/{file}")).map(|b| text::decode(&b, inst.lang())).map_err(|e| format!("{file}: {e}"))?;
    let cfg = NewScenario { text, seed, ticks: seed, difficulty: 2 };
    StartTexts::read(inst)?.with(inst, |data| {
        let mut g = Game::before_start(data, rules);
        g.start_scenario(&cfg, data);
        g
    })
}

/// `SetScirmishGameOptions(..)` (exe 0x503e80) → `InitNewGame(seed, 3, 0, Nehezseg)`: a generated galaxy.
pub fn skirmish<'r>(inst: &Install, cfg: &NewSkirmish, rules: &'r GameRules) -> Result<Game<'r>, String> {
    StartTexts::read(inst)?.with(inst, |data| {
        let mut g = Game::before_start(data, rules);
        g.start_skirmish(cfg, data);
        g
    })
}

/// Ours (ADR 0085): a custom game ([`Game::start_custom`]).
pub fn custom<'r>(inst: &Install, cfg: &ig2_sim::game::new_game::CustomGame, rules: &'r GameRules) -> Result<Game<'r>, String> {
    StartTexts::read(inst)?.with(inst, |data| {
        let mut g = Game::before_start(data, rules);
        g.start_custom(cfg, data);
        g
    })
}

/// `TutorialSetCampaignGameOptions(n)` + `TutorialLoadScenario(n)` (exe 0x504250, 0x611c80): chapter `n`'s
/// `tutorial/scn/tutorial<n>.scn`. The script (`TutorialStart("main", n)`) is the front end's.
pub fn tutorial<'r>(inst: &Install, n: u8, seed: u32, rules: &'r GameRules) -> Result<Game<'r>, String> {
    let file = format!("dataraw/tutorial/scn/tutorial{n}.scn");
    let text = inst.read(&file).map(|b| text::decode(&b, inst.lang())).map_err(|e| format!("{file}: {e}"))?;
    let cfg = NewScenario { text, seed, ticks: seed, difficulty: 2 };
    StartTexts::read(inst)?.with(inst, |data| {
        let mut g = Game::before_start(data, rules);
        g.start_tutorial(&cfg, data);
        g
    })
}

/// `SDL_GetTicks()` for a start's seeds: the milliseconds of the wall clock (the original's ticks since boot).
pub fn wall_ticks() -> u32 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u32)
}

/// `--new-campaign C[,D[,SEED[,TICKS]]]`: campaign 1..=3, difficulty 1/2/4 (default 2), `RandomInit`'s seed and
/// `NEWGAME_Diplomacy`'s `SDL_GetTicks()` (default 4242 and the seed); no config storyline, no registry entry.
pub fn parse_campaign(v: &str) -> Result<NewCampaign, String> {
    let n: Vec<i64> = v.split(',').map(|s| s.trim().parse().map_err(|_| format!("--new-campaign {v}: numbers"))).collect::<Result<_, _>>()?;
    let campaign = *n.first().ok_or("--new-campaign C[,D[,SEED[,TICKS]]]")? as i32;
    if !(1..=3).contains(&campaign) {
        return Err(format!("--new-campaign {v}: campaign 1, 2 or 3"));
    }
    let difficulty = n.get(1).copied().unwrap_or(2) as i32;
    if ![1, 2, 4].contains(&difficulty) {
        return Err(format!("--new-campaign {v}: difficulty 1, 2 or 4"));
    }
    let seed = n.get(2).copied().unwrap_or(4242) as u32;
    let ticks = n.get(3).map_or(seed, |&t| t as u32);
    Ok(NewCampaign { campaign, difficulty, seed, ticks, storyline: 0, storymap: 0, last_storyline: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A new campaign with the test mod (ADR 0069, `ig2-rules/tests/mods/fusion`): the start copies the mod's tech
    /// record (unknown to every empire, the mod's research cost) and patched building kinds from the rules, not the
    /// exe; without rule patches the start is the exe's (skips without the install).
    #[test]
    fn a_new_game_starts_from_the_mods_tables() {
        let Some(Ok(plain)) = Install::from_env(Lang::En) else { return };
        let fusion = ig2_formats::vfs::Mod { name: "fusion".into(), dir: std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ig2-rules/tests/mods/fusion") };
        let install = Install::open_with_mods(plain.root(), Lang::En, vec![fusion]).unwrap();
        let (rules, _) = crate::rules::load(&install).unwrap();
        let game = campaign(&install, &parse_campaign("1").unwrap(), &rules).unwrap();
        let techs = game.state.tech();
        assert!((1..ig2_sim::colony_side::EMPIRES).all(|e| techs.status(136, e) == ig2_sim::colony_side::status::UNKNOWN));
        assert_eq!((techs.research_cost(136), techs.byte(136, 0x27)), (40000, 56));
        assert!(StartTexts::read(&plain).unwrap().tables.is_empty());
    }

    /// A mod's race and planet type (ADR 0069 §13, §14, the test mod `ig2-rules/tests/mods/frontier`) in a generated
    /// galaxy: race 15 plays the human and an AI empire; a share of the free volcanic planets turns into planet type
    /// 9; race 15 starts with its original's (race 3's) techs and a home world of its own favourite type; the game
    /// plays on with the AI; the texts name both; a save of it refuses to load without the mod (skips without the
    /// install).
    #[test]
    fn a_mods_race_and_planet_type_play_in_a_new_galaxy() {
        use ig2_sim::colony_side::{status, TECHS};
        let Some(Ok(plain)) = Install::from_env(Lang::En) else { return };
        let frontier = ig2_formats::vfs::Mod { name: "frontier".into(), dir: std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ig2-rules/tests/mods/frontier") };
        let install = Install::open_with_mods(plain.root(), Lang::En, vec![frontier]).unwrap();
        let (rules, _) = crate::rules::load(&install).unwrap();
        let options = ig2_net::galaxy::GalaxyOptions { planets: 80, ..Default::default() };
        let start = ig2_net::galaxy::GalaxyStart::new(4242, options, [15, 2, 3, 15, 5, 6, 7, 8], vec![2, 3, 4]);
        let mut game = StartTexts::read(&install).unwrap().with(&install, |data| start.build(data, &rules, 1, &[1], 77)).unwrap();
        let races = game.state.races();
        assert_eq!((races[1], races[3], races[4]), (15, 3, 15));
        let planets = |g: &ig2_sim::game::Game| (1..=g.state.planet_count()).map(|p| g.state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, ig2_sim::save::PLANET_SIZE)).collect::<Vec<_>>();
        let start_planets = planets(&game);
        let ash = start_planets.iter().filter(|r| r[0x22] == 9).count();
        assert!(ash > 0 && start_planets.iter().all(|r| r[0x22] != 9 || r[0x20] == 0), "{ash} ash worlds, all free");
        // Its original's start techs; its home world the first type of its own row's best liking.
        let techs = game.state.tech();
        assert!((1..TECHS).all(|t| techs.status(t, 1) == techs.status(t, 3)));
        let row = &rules.side.colony.surface_growth[15];
        let best = (1..=8).max_by_key(|&k| (row[k], std::cmp::Reverse(k))).unwrap();
        let home = start_planets.iter().find(|r| r[0x20] == 1).unwrap();
        assert_eq!((home[0x21], row[best] >= row[home[0x22] as usize]), (15, true));
        assert_eq!(techs.status(1, 1) == status::OWNED, techs.status(1, 3) == status::OWNED);
        for _ in 0..3000 {
            game.run_tick();
        }
        assert!(planets(&game).iter().filter(|r| r[0x20] == 4).all(|r| r[0x21] == 15), "the AI's colonies are of its race");
        let texts = crate::ui::UiTexts::load(&install);
        assert_eq!((texts.race(15), texts.surface(9), texts.race(3)), (b"Vorlani".to_vec(), b"Ash world".to_vec(), crate::ui::UiTexts::load(&plain).race(3)));
        assert!(texts.playable_races().ends_with(&[8, 15]));
        assert_eq!(crate::world::check_kinds(&game.state, &rules), Ok(()));
        let (classic, _) = crate::rules::load(&plain).unwrap();
        assert!(crate::world::check_kinds(&game.state, &classic).unwrap_err().contains("race 15"));
    }

    #[test]
    fn campaign_argument() {

        let c = parse_campaign("2,4,77,9000").unwrap();
        assert_eq!((c.campaign, c.difficulty, c.seed, c.ticks, c.last_storyline), (2, 4, 77, 9000, None));
        let c = parse_campaign("1").unwrap();
        assert_eq!((c.difficulty, c.seed, c.ticks), (2, 4242, 4242));
        assert!(parse_campaign("4").is_err());
        assert!(parse_campaign("1,3").is_err());
    }
}
