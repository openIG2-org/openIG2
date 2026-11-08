//! A custom game (ADR 0085, spec `re/rules/new-game.md` §13.2): each value's effect on the skirmish start. The
//! original's values are `tests/new_game.rs`'s recorded skirmish starts (`start_skirmish` is a custom game).

#[path = "../../ig2-net/tests/common/mod.rs"]
#[allow(dead_code)]
mod common;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::game::new_game::{CustomGame, NewSkirmish, Side, StartData};
use ig2_sim::game::{Game, GameRules};
use ig2_sim::save::{va, PLANET_SIZE, TECH_SIZE, TECHS};

const HOME: u32 = 0xF17394;

fn skirmish() -> NewSkirmish {
    NewSkirmish { en: 1, ais: 3, tech: 1, planets: 60, money: 50000, difficulty: 2, population: 1, random_id: 7, seed: 12345, ticks: 999 }
}

/// Empire 1 the human, 2 and 3 the computer's, the rest off.
fn chosen() -> CustomGame {
    let mut c = CustomGame::from(&skirmish());
    c.drawn_opponents = None;
    for e in &mut c.empires[3..] {
        e.side = Side::Off;
    }
    c
}

fn start<'r>(c: &CustomGame, data: &StartData, rules: &'r GameRules) -> Game<'r> {
    let mut g = Game::before_start(data, rules);
    g.start_custom(c, data);
    g
}

fn planet(g: &Game, p: usize) -> Vec<u8> {
    g.state.mem.read(va::PLANETS + (PLANET_SIZE * p) as u32, PLANET_SIZE)
}

/// The planets `e` owns, with their people.
fn owned(g: &Game, e: u8) -> Vec<(usize, i32)> {
    (1..=g.state.planet_count()).map(|p| (p, planet(g, p))).filter(|(_, r)| r[0x20] == e).map(|(p, r)| (p, i32::from_le_bytes(r[0x28..0x2c].try_into().unwrap()))).collect()
}

fn techs(g: &Game, e: usize) -> usize {
    (1..TECHS).filter(|t| g.state.mem.u8(va::TECHS + (t * TECH_SIZE + 0x45 + e) as u32) == 5).count()
}

fn with(f: impl FnOnce(&StartData, &GameRules)) {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| f(data, &rules));
}

#[test]
fn the_skirmish_maps_to_the_original_values() {
    let c = CustomGame::from(&NewSkirmish { population: 3, planets: 100, ais: 1, ..skirmish() });
    assert_eq!(c.drawn_opponents, Some(1));
    assert_eq!(c.empires[0].side, Side::Human);
    assert!(c.empires[1..].iter().all(|e| e.side == Side::Computer && e.level == 0 && e.start.is_none()));
    assert_eq!(c.empires.iter().map(|e| e.race).collect::<Vec<_>>(), (1..=8).collect::<Vec<_>>());
    // `InitScirmishBolygok`: HanyBolygo / (HanyGepiJatekos + 3) start planets.
    assert!(c.empires.iter().all(|e| e.planets == 25 && e.money == 50000 && e.tech == 1));
    assert_eq!(c.clamped(), c, "the original's values are in range");
}

#[test]
fn the_galaxy_and_the_opponents() {
    with(|data, rules| {
        let mut c = chosen();
        c.planets = 130;
        let g = start(&c, data, rules);
        assert_eq!(g.state.planet_count(), 130);
        let alive = g.state.alive();
        assert_eq!(&alive[1..=8], &[true, true, true, false, false, false, false, false], "the chosen opponents, no draws");
        assert_eq!(g.state.mem.i32(0xF21488), 2, "HanyGepiJatekos counts them");
        assert_eq!(g.state.custom.as_ref(), Some(&c.clamped()), "kept for the save");
        c.planets = 300;
        assert_eq!(start(&c, data, rules).state.planet_count(), 150, "the cap");
    });
}

#[test]
fn race_name_money_and_level() {
    with(|data, rules| {
        let mut c = chosen();
        c.empires[1].race = 5;
        c.empires[1].name = Some(b"Zorgons of the Rim".to_vec());
        c.empires[2].money = 123456;
        c.empires[2].level = 4;
        let g = start(&c, data, rules);
        assert_eq!(g.state.races()[2], 5);
        assert_eq!(g.state.mem.read(0xBAE588 + 22, 11), b"Zorgons of\0", "cut to the 11-byte slot");
        let home = g.state.mem.i32(HOME + 8) as usize;
        assert_eq!(planet(&g, home)[0x21], 5, "the home world's people are the empire's race");
        assert_eq!(g.state.mem.f64(va::MONEY + 24), 123456.0);
        assert_eq!(g.state.mem.f64(va::MONEY + 16), 50000.0, "the others keep theirs");
        assert_eq!(g.state.ai_difficulty[3], 4);
        assert_eq!(g.state.ai_difficulty[1], 0, "a human has the game's");
        // A hard computer's head start is one tech level more (`InitAdvancedTalalmanyok`).
        let base = start(&chosen(), data, rules);
        assert!(techs(&g, 3) > techs(&base, 3));
        assert_eq!(techs(&g, 1), techs(&base, 1));
    });
}

#[test]
fn technology_per_empire() {
    with(|data, rules| {
        let mut c = chosen();
        c.empires[1].tech = 3;
        c.empires[2].tech = 0;
        let (g, base) = (start(&c, data, rules), start(&chosen(), data, rules));
        assert!(techs(&g, 2) > techs(&base, 2));
        assert!(techs(&g, 3) < techs(&base, 3));
        assert_eq!(techs(&g, 1), techs(&base, 1), "the human keeps level 1");
    });
}

#[test]
fn start_planets_draw_on_their_own_streams() {
    with(|data, rules| {
        let base = start(&chosen(), data, rules);
        // 60 planets, population 1: two start planets each.
        assert_eq!(owned(&base, 2).len(), 2);
        let mut c = chosen();
        c.empires[1].planets = 6;
        let g = start(&c, data, rules);
        assert_eq!(owned(&g, 2).len(), 6);
        let planets = |g: &Game| owned(g, 2).into_iter().map(|(p, _)| p).collect::<Vec<_>>();
        assert!(planets(&base).iter().all(|p| planets(&g).contains(p)), "the original's colonies first");
        assert_eq!(owned(&g, 3), owned(&base, 3), "the later empire's colonies stay the seed's");
        c.empires[1].planets = 1;
        assert_eq!(owned(&start(&c, data, rules), 2).len(), 1);
    });
}

#[test]
fn a_chosen_start_planet() {
    with(|data, rules| {
        let base = start(&chosen(), data, rules);
        let home = |g: &Game, e: u32| g.state.mem.i32(HOME + 4 * e) as usize;
        let (h1, h3) = (home(&base, 1), home(&base, 3));
        // Empire 1 takes empire 3's start: they swap.
        let mut c = chosen();
        c.empires[0].start = Some(h3);
        let g = start(&c, data, rules);
        assert_eq!((home(&g, 1), home(&g, 3)), (h3, h1));
        assert_eq!(planet(&g, h3)[0x20], 1);
        assert_eq!(planet(&g, h1)[0x20], 3);
        // A free planet: only empire 1 moves.
        let free = (1..=60).find(|&p| planet(&base, p)[0x20] == 0 && (1..=8).all(|e| home(&base, e) != p)).unwrap();
        c.empires[0].start = Some(free);
        let g = start(&c, data, rules);
        assert_eq!((home(&g, 1), home(&g, 3)), (free, h3));
        assert_eq!(planet(&g, free)[0x20], 1);
    });
}
