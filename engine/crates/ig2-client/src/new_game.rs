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
    message_kinds: i32,
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
        Ok(Self {
            exe,
            race_names: (0..20).map(|r| hu_bytes(hu.get(0x11, 1, r + 5))).collect(),
            constellations: (1..=15).map(|k| enc(keyed.get(29, k).unwrap_or(""))).collect(),
            tech_names: (0..193u16).map(|t| if t == 0 { Vec::new() } else { hu_bytes(hu.get(0x11, 8 + (t - 1) / 48, (t - 1) % 48 + 1)) }).collect(),
            design_names: (0..47u16).map(|k| hu_bytes(hu.get(0x11, 7, k + 1))).collect(),
            fleet_name: hu_bytes(hu.get(17, 5, 46)),
            message_kinds: kinds as i32,
        })
    }

    /// The sim's view of these, with the exe image (`PeImage::read_loaded`, BSS as zero) and `scenario/<n>.scn`.
    pub fn with<T>(&self, inst: &Install, f: impl FnOnce(&StartData) -> T) -> Result<T, String> {
        let pe = ig2_rules::PeImage::parse(&self.exe).map_err(|e| e.to_string())?;
        let image = |at: u32, len: usize| pe.read_loaded(at, len).unwrap_or_else(|_| vec![0; len]);
        let scenario = |n: i32| inst.read(&format!("dataraw/scenario/{n}.scn")).ok().map(|b| text::decode(&b, inst.lang()));
        let data = StartData {
            image: &image,
            scenario: &scenario,
            race_names: self.race_names.clone(),
            constellations: self.constellations.clone(),
            tech_names: self.tech_names.clone(),
            design_names: self.design_names.clone(),
            fleet_name: self.fleet_name.clone(),
            message_kinds: self.message_kinds,
        };
        Ok(f(&data))
    }
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
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u32)
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
