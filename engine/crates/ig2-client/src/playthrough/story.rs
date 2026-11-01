//! The story-aware part of the play-through driver (`docs/BUILDING.md` → Play-through harness): what a player does
//! for the campaign's episodes that the sim's AI, playing the human's empire, doesn't (re/rules/campaign.md §8,
//! campaign/line.md, campaign/elem.md, campaign/begin.md). It acts only through the player's commands
//! ([`World::send`]: fleet orders, research, spy missions, the messages screen's answers) and reads the state, so the
//! same driver plays a peer of a multiplayer game.

use ig2_net::{Command, ResearchCmd, SpyOrder, StarmapCmd, Terms};
use ig2_sim::game::Game;
use ig2_sim::save::va;
use ig2_sim::story::Story;

use crate::world::World;

/// The story empires whose story planets a player takes: rebels and pirates (10), the episodes' invaders (11..=13),
/// the colonists of 840 (17).
const STORY_FOES: [u8; 5] = [10, 11, 12, 13, 17];

/// `FlottaMozgat`'s orders and targets (re/rules/fleet-movement.md §2, §4.3).
const MOVE: i32 = 1;
const ATTACK: i32 = 2;
const PLANET: i32 = 1;
const FLEET: i32 = 2;

/// The techs an episode asks the player to research while it runs: (story, tech) (elem.md).
const RESEARCH: &[(i32, i32)] = &[
    // 802: the volcano of planet 8020 is stopped with tech 183.
    (802, 183),
    // 815: the human's fleets inside the triangle of 8150..=8152 are lost until tech 177 is researched.
    (815, 177),
    // 831: one of techs 161..=171 researched while their research is halved.
    (831, 162),
    // 841: tech 6 at half the time and price until the human has it.
    (841, 6),
];

/// How a player answers a story message block (the parts are the `- PP NN` lines' `PP`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Part(i32),
    /// Leave it unread until it expires (the story takes the expiry as its own answer).
    Leave,
}

/// The answers a player takes where the first allowed one would stall or lose the storyline (elem.md, line.md).
const ANSWERS: &[(i32, Answer)] = &[
    // 530: the price of 801's lost colonists: 3 pays nothing and the storyline goes on.
    (8039, Answer::Part(3)),
    // 530: planet 8080's offer; 3 (as the expiry) leaves 808 without its closing message.
    (6300, Answer::Part(3)),
    // 810 (and 550's 0x19c9): 3 ends 810 at once.
    (6601, Answer::Part(3)),
    // 550: 852's protection; part 2 leads to war.
    (6701, Answer::Leave),
    // 807: the pirates' planet; 5 ends it.
    (6840, Answer::Part(5)),
    // 824: the weak planet's offer; 4 ends it quietly (8241), 8242's answers all lead to war.
    (8241, Answer::Part(4)),
    (8242, Answer::Leave),
    // 850: the protection; 2 pays monthly and ends it.
    (5310, Answer::Part(2)),
    (5311, Answer::Part(2)),
    (8112, Answer::Part(3)),
    // 550: planet 8140's doomsday device: 4 pays 200,000 and starts 814.
    (5540, Answer::Part(4)),
    (5541, Answer::Part(4)),
    // 856: the toll of planet 8560: pay 30,000, then lose up to 5 fighters; 8567's 2 sends the fighters.
    (8562, Answer::Part(3)),
    (8563, Answer::Part(5)),
    (8567, Answer::Part(2)),
    // 861: sell the evidence (+200,000), then side with empire 1 against empire 5 (862, 863: the campaign's end).
    (8611, Answer::Part(2)),
    (8613, Answer::Part(2)),
    // 864: empire 5's demand: refuse.
    (8641, Answer::Part(4)),
];

/// The answer for message block `id`, when the first allowed one isn't the player's.
pub fn answer(id: i32) -> Option<Answer> {
    ANSWERS.iter().find(|a| a.0 == id).map(|a| a.1)
}

/// The story spy missions a player runs: (story, mission, target empire) (elem.md 861: the evidence against
/// empire 1).
const MISSIONS: &[(i32, i32, u8)] = &[(861, 8611, 1)];

/// The story diplomacy specials a player sends (menu 0x32, re/screens/diplomacy.md §6.1): (story, special) (elem.md
/// 857: buy the alliance, 858: the pacts, 860: the tribute).
const SPECIALS: &[(i32, i32)] = &[(857, 8571), (858, 8581), (860, 8601)];

/// The fleets the story may take from the AI at once.
const FLEETS: usize = 2;

/// What the driver did, for the report.
#[derive(Debug, Default)]
pub struct Driver {
    pub orders: u32,
    /// The specials sent: (id, empire).
    sent: Vec<(i32, i32)>,
}

/// One fleet goal: what the story wants a fleet to do.
#[derive(Debug, Clone, Copy)]
enum Goal {
    /// Attack (and land on) a planet.
    Take(usize),
    /// Destroy a fleet.
    Hunt(usize),
    /// Stand at a planet.
    Guard(usize),
}

impl Driver {
    /// One look at the game as a player takes it every few game days: the story's targets get a fleet each, the
    /// episodes' techs are researched, the story spy missions get a spy.
    pub fn step(&mut self, w: &mut World, en: usize, notes: &mut Vec<String>) {
        let story = w.game.state.story();
        for &(s, tech) in RESEARCH {
            if story.is_running(s) && tech_status(&w.game, en, tech) < 5 {
                w.send(Command::Research { order: ResearchCmd::Start { tech } });
            }
        }
        self.spies(w, en, &story, notes);
        self.specials(w, en, &story, notes);
        self.war(w, en, notes);
        // 840: five colonies in time, or the human loses its planets (or dies).
        if story.is_running(840) {
            self.build_colony_ship(w, en, &story, notes);
            self.colonise(w, en, notes);
        }
        let goals = goals(&w.game, en, &story);
        let free = own_fleets(&w.game, en);
        // The rest of the fleets stay the AI's (its defence and its own wars), unless the AI won't fight the only
        // enemies there are: then every fleet follows the goals, the ones beyond them the last goal.
        let n = if guarded_wars_only(&w.game, en) { free.len() } else { FLEETS.min(goals.len()) };
        for (k, &f) in free.iter().take(n).enumerate() {
            let Some(&g) = goals.get(k.min(goals.len().saturating_sub(1))) else { break };
            if self.order(w, f, g) {
                notes.push(format!("{g:?}"));
            }
        }
        w.pump();
    }

    /// Fleet `f` follows goal `g` unless that is its order already (the AI may have given it another).
    fn order(&mut self, w: &mut World, f: usize, g: Goal) -> bool {
        let (order, kind, target) = match g {
            Goal::Take(p) => (ATTACK, PLANET, p),
            Goal::Hunt(t) => (ATTACK, FLEET, t),
            Goal::Guard(p) => (MOVE, PLANET, p),
        };
        let map = w.game.state.starmap();
        let fl = &map.fleets[f];
        if fl.order as i32 == order && fl.kind as i32 == kind && fl.tx as usize == target {
            return false;
        }
        if let Goal::Guard(p) = g {
            let c = w.game.state.colony();
            let q = &c.planets[p];
            if (fl.x - q.x).hypot(fl.y - q.y) < 10.0 {
                return false;
            }
        }
        w.send(Command::Starmap { order: StarmapCmd::Move { fleet: f as i32, order, kind, x: (target as f32).to_bits(), y: 0f32.to_bits() } });
        self.orders += 1;
        true
    }

    /// Each fleet with a colony ship that isn't colonising goes to the nearest free planet the human sees (one each).
    fn colonise(&mut self, w: &mut World, en: usize, notes: &mut Vec<String>) {
        let map = w.game.state.starmap();
        let colony = w.game.state.colony();
        let mut taken: Vec<usize> = map.fleets.iter().filter(|f| f.owner as usize == en && f.flags & ig2_sim::fleet::flag::COLONISING != 0).map(|f| f.tx as usize).collect();
        for f in map.fleets.iter().skip(1).filter(|f| f.owner as usize == en && f.colony_ships > 0 && f.flags & ig2_sim::fleet::flag::COLONISING == 0) {
            let d = |p: usize| (colony.planets[p].x - f.x).hypot(colony.planets[p].y - f.y);
            let free = (1..colony.planets.len()).filter(|&p| colony.planets[p].owner == 0 && colony.planets[p].visibility[en] >= 0 && !taken.contains(&p));
            let Some(p) = free.min_by(|&a, &b| d(a).total_cmp(&d(b))) else { break };
            taken.push(p);
            w.send(Command::Starmap { order: StarmapCmd::Colonise { planet: p as i32, kind: 1 } });
            self.orders += 1;
            notes.push(format!("colonise {p}"));
        }
    }

    /// Each story special offered (`SpecialLista`, exe 0xA42DE8) is sent once while its story runs (cat 4, `sub` =
    /// the `SpecialLeiro` index); 8571 only with the 2,000,000 it costs (the menu disables it below that).
    fn specials(&mut self, w: &mut World, en: usize, story: &Story, notes: &mut Vec<String>) {
        let m = &w.game.state.mem;
        let n = m.i32(0xA41DC4).clamp(0, 200) as u32;
        let money = m.f64(va::MONEY + 8 * en as u32);
        let list: Vec<[i32; 3]> = (0..n).map(|i| std::array::from_fn(|f| m.i32(0xA42DE8 + 12 * i + 4 * f as u32))).collect();
        for [id, e, k] in list {
            let running = SPECIALS.iter().any(|&(s, x)| x == id && story.is_running(s));
            if !running || self.sent.contains(&(id, e)) || (id == 8571 && money < 2_000_000.0) {
                continue;
            }
            self.sent.push((id, e));
            w.send(Command::DiplomacyMessage { to: e, declaration: false, terms: Terms { cat: 4, sub: k, ..Default::default() } });
            notes.push(format!("special {id} to {e}"));
        }
    }

    /// While fewer colony ships fly and are queued than 840 still needs colonies (`5 − v(8400)`), the cheapest colony
    /// ship design the human can build goes into the ship line of its planet with the most ship capacity that isn't
    /// building one: the production screen's "add" (`Production` op 4, exe order 0x1d: the design goes to the first
    /// empty slot, re-timed when it is the head; production-research.md §1, space-combat.md §2.3 for the design's
    /// colony flag `+0x34`). One at a time misses the goal: building one takes most of a year on a broke economy.
    fn build_colony_ship(&mut self, w: &mut World, en: usize, story: &Story, notes: &mut Vec<String>) {
        let map = w.game.state.starmap();
        let flying: i32 = map.fleets.iter().skip(1).filter(|f| f.owner as usize == en).map(|f| f.colony_ships as i32).sum();
        let need = 5 - story.vars.get(8400).unwrap_or(0) - flying;
        if need <= 0 {
            return;
        }
        let combat = w.game.state.combat();
        let Some(designs) = combat.designs.get(en) else { return };
        let price = |d: &ig2_sim::spacewar::Design| i32::from_le_bytes(d.raw[0x24..0x28].try_into().unwrap());
        let hull = |d: &ig2_sim::spacewar::Design| i32::from_le_bytes(d.raw[0x48..0x4c].try_into().unwrap());
        let colony = |d: &ig2_sim::spacewar::Design| d.class == 1 && d.raw[0x34] != 0 && d.raw[0x18] != 0 && tech_status(&w.game, en, hull(d)) == 5;
        let Some(design) = (1..designs.len()).filter(|&d| colony(&designs[d])).min_by_key(|&d| price(&designs[d])) else { return };
        let c = w.game.state.colony();
        let slot = |q: &[u8; 64], k: usize| i16::from_le_bytes([q[2 * k], q[2 * k + 1]]);
        let mine = |p: usize| c.planets[p].owner as usize == en;
        // Queued: what the AI put ahead of it is deleted (op 4 without a design on the head), so it is built next.
        let queued: Vec<usize> = (1..c.planets.len()).filter(|&p| mine(p) && (0..4).any(|k| slot(&c.planets[p].production, k) == design as i16)).collect();
        for &p in &queued {
            let q = &c.planets[p].production;
            if slot(q, 0) != design as i16 {
                w.send(Command::Starmap { order: StarmapCmd::Production { op: 4, planet: p as i32, line: 0, slot: q[0x20] as i32, value: 0 } });
                self.orders += 1;
            }
        }
        // The ship line (0): designs at +0x78 + 2k, the UI bytes at +0x98 + k that the order finds the slot by.
        let mut free: Vec<usize> = (1..c.planets.len()).filter(|&p| mine(p) && c.planets[p].ship_capacity > 0 && slot(&c.planets[p].production, 3) == 0 && !queued.contains(&p)).collect();
        free.sort_by_key(|&p| std::cmp::Reverse(c.planets[p].ship_capacity));
        for p in free.into_iter().take((need - queued.len() as i32).max(0) as usize) {
            let ui = c.planets[p].production[0x20 + 3];
            w.send(Command::Starmap { order: StarmapCmd::Production { op: 4, planet: p as i32, line: 0, slot: ui as i32, value: design as i32 } });
            self.orders += 1;
            notes.push(format!("colony ship queued at {p}"));
        }
    }

    /// At war with nobody, the player declares war on the empire it knows with the fewest planets
    /// (`BirodalomHanyBolygo`), when that is under half its own: the AI playing
    /// the human waits for a war target its rules allow (ai-empire.md §6.2, the story's guards §11.1) and stops
    /// growing once the free planets are gone. At war with two rivals or fewer left, it still declares on a non-ally with
    /// under a quarter of its planets: the campaign is won with one non-ally left that the human outweighs twice (story 405,
    /// re/rules/game-end.md), not while a small bystander lives. The diplomacy screen's declaration
    /// (re/rules/diplomacy.md §6.3).
    fn war(&mut self, w: &mut World, en: usize, notes: &mut Vec<String>) {
        let m = &w.game.state.mem;
        let alive = w.game.state.alive();
        let knows = w.game.state.colony().knows;
        let level = |e: usize| m.u8(va::RELATIONS + ((en * 19 + e) * 0x26) as u32 + 0x25);
        let rivals: Vec<usize> = (1..9).filter(|&e| e != en && alive[e] && knows[en][e] != 0).collect();
        let planets: [i32; 19] = m.i32s(va::EMPIRE_PLANETS);
        let at_war = rivals.iter().any(|&e| level(e) == 1);
        if at_war && (1..9).filter(|&e| e != en && alive[e]).count() > 2 {
            return;
        }
        let small = |e: usize| if at_war { level(e) != 1 && level(e) != 5 && planets[e] * 4 < planets[en] } else { planets[e] * 2 < planets[en] };
        let Some(e) = rivals.into_iter().filter(|&e| small(e)).min_by_key(|&e| planets[e]) else { return };
        let terms = ig2_net::Terms { cat: 0, sub: 2, value: -1, sub2: -1, value2: -1 };
        w.send(Command::DiplomacyMessage { to: e as i32, declaration: true, terms });
        notes.push(format!("war on {e}"));
    }

    /// A spy at home goes on each story mission that is open.
    fn spies(&mut self, w: &mut World, en: usize, story: &Story, notes: &mut Vec<String>) {
        let spies = w.game.state.spies();
        let tables = &w.game.rules.story.campaign;
        for &(s, id, target) in MISSIONS {
            let Some(k) = (0..12).find(|&k| tables.story_mission(k) == id) else { continue };
            let m = 14 + k as usize;
            if spies.enabled.get(m).copied().unwrap_or(0) == 0 || !story.is_running(s) {
                continue;
            }
            let roster = &spies.roster[en];
            if roster.iter().any(|s| s.state == ig2_sim::spy::state::MISSION && s.mission as usize == m) {
                continue;
            }
            let count = spies.count[en].clamp(0, ig2_sim::spy::SLOTS as i32 - 1) as usize;
            if let Some(spy) = (1..=count).find(|&s| roster[s].state == ig2_sim::spy::state::HOME) {
                w.send(Command::Spy { order: SpyOrder::Send { spy: spy as u8, mission: m as u16, target, param1: 0, param2: 0, days: 10 } });
                notes.push(format!("spy {spy} on story mission {id}"));
            }
        }
    }
}

/// `IG2_PLAYTHROUGH_DEBUG`: the story planets (id, planet, owner, the human's view) and 840's colonies so far.
pub fn debug(g: &Game, en: usize) -> String {
    let story = g.state.story();
    let colony = g.state.colony();
    let planets: Vec<String> = story.planets.iter().filter(|e| e.0 > 0 && e.1 > 0).filter_map(|&(id, p)| colony.planets.get(p as usize).map(|q| format!("{id}:{p}/{}/{}", q.owner, q.visibility[en]))).collect();
    format!("story planets {} 840 v8400 {:?}", planets.join(" "), story.is_running(840).then(|| story.vars.get(8400)))
}

/// The story's fleet goals, the most urgent first (elem.md, begin.md).
fn goals(g: &Game, en: usize, story: &Story) -> Vec<Goal> {
    let colony = g.state.colony();
    let map = g.state.starmap();
    let planet = |id: i32| story.planets.iter().find(|e| e.0 == id && e.1 > 0).map(|e| e.1 as usize);
    let fleet = |id: i32| map.fleets.iter().enumerate().skip(1).find(|(_, f)| f.story_id as i32 == id && f.ship_count > 0).map(|(k, _)| k);
    let owner = |p: usize| colony.planets.get(p).map_or(0, |x| x.owner as usize);
    let seen = |p: usize| colony.planets.get(p).is_some_and(|x| x.visibility[en] >= 0);
    let running = |id: i32| story.is_running(id);
    let mut out = Vec::new();
    // 834: a fleet of the human's within 20 of planet 8340 when each of empire 1's fleets arrives, or the human dies.
    if running(834) {
        if let Some(p) = planet(8340) {
            out.push(Goal::Guard(p));
        }
    }
    // 862: empire 5's raiders (8621) at planet 8620; 817: empire 11's fleet 8170.
    for (s, f) in [(862, 8621), (817, 8170)] {
        if running(s) {
            out.extend(fleet(f).map(Goal::Hunt));
        }
    }
    // The story planets a story empire holds (begin 6; 804, 809, 814, 815, 816, 820, 832, 840, …: each finishes
    // when the planet is no longer that empire's).
    for &(id, p) in &story.planets {
        let p = p as usize;
        if id > 0 && p > 0 && STORY_FOES.contains(&(owner(p) as u8)) && seen(p) && !out.iter().any(|g| matches!(g, Goal::Take(q) if *q == p)) {
            out.push(Goal::Take(p));
        }
    }
    // 840 missed: empire 17 took two of the human's planets (no story ids); 900 days to take them back or die.
    if running(840) {
        let taken: Vec<usize> = (1..colony.planets.len()).filter(|&p| owner(p) == 17 && seen(p) && !out.iter().any(|g| matches!(g, Goal::Take(q) if *q == p))).collect();
        out.extend(taken.into_iter().map(Goal::Take));
    }
    // 856: a fleet at planet 8560 pays the toll.
    if running(856) {
        out.extend(planet(8560).filter(|&p| owner(p) != en).map(Goal::Guard));
    }
    // 858: empire 5's planet 8580; 835: every planet of empire v(8359) the human sees.
    if running(858) {
        out.extend(planet(8580).filter(|&p| owner(p) == 5).map(Goal::Take));
    }
    // The story's guards (`StoryAdat+0xb8`: empire 1, `+0xbc`: empire 5; ai-empire.md §11.1): no AI attacks a
    // guarded empire's planets while it isn't among the two strongest, the AI playing the human included. A player
    // isn't bound by it: at war with one, its least defended planet the human sees.
    for (at, e) in [(0xb8, 1usize), (0xbc, 5)] {
        let m = &g.state.mem;
        let war = m.u8(va::RELATIONS + ((en * 19 + e) * 0x26) as u32 + 0x25) == 1;
        if e != en && m.i32(va::STORY_ADAT + at) != 0 && war {
            let t = (1..colony.planets.len()).filter(|&p| owner(p) == e && colony.planets[p].visibility[en] > 0).min_by_key(|&p| colony.planets[p].field_5c);
            out.extend(t.map(Goal::Take));
        }
    }
    if running(835) {
        let e = story.vars.get(8359).unwrap_or(0) as usize;
        out.extend((1..colony.planets.len()).filter(|&p| e > 0 && owner(p) == e && seen(p)).take(2).map(Goal::Take));
    }
    out
}

/// The human is at war, and only with empires the story guards from the AI: the AI playing the human attacks nobody
/// then.
fn guarded_wars_only(g: &Game, en: usize) -> bool {
    let m = &g.state.mem;
    let alive = g.state.alive();
    let war = |e: usize| m.u8(va::RELATIONS + ((en * 19 + e) * 0x26) as u32 + 0x25) == 1;
    let wars: Vec<usize> = (1..9).filter(|&e| e != en && alive[e] && war(e)).collect();
    !wars.is_empty() && wars.iter().all(|&e| guarded(g, e))
}

/// No AI attacks empire `e`'s planets right now: the story guards it (`StoryAdat+0xb8`: 1, `+0xbc`: 5) and it isn't
/// among the strongest (ai-empire.md §11.1).
fn guarded(g: &Game, e: usize) -> bool {
    let m = &g.state.mem;
    let alive = g.state.alive();
    let power: [f32; 19] = m.f32s(va::EMPIRE_POWER);
    // `s1`, `s2`: the first alive empire with the strictly greatest power, then likewise among the others.
    let strongest = |skip: usize| (1..9).filter(|&c| alive[c] && c != skip).fold(0, |s, c| if s == 0 || power[s] < power[c] { c } else { s });
    let (s1, flags) = (strongest(usize::MAX), [m.i32(va::STORY_ADAT + 0xb8) != 0, m.i32(va::STORY_ADAT + 0xbc) != 0]);
    let s2 = strongest(s1);
    match (e, flags) {
        (1, [true, true]) | (5, [true, true]) => s1 != e && s2 != e,
        (1, [true, _]) | (5, [_, true]) => s1 != e,
        _ => false,
    }
}

/// The human's fleets that aren't story fleets, the strongest (by ground units, then strength) first.
fn own_fleets(g: &Game, en: usize) -> Vec<usize> {
    let map = g.state.starmap();
    let mut v: Vec<(usize, (bool, i32))> = map
        .fleets
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, f)| f.owner as usize == en && f.story_id == 0 && f.ship_count > 0 && f.flags & ig2_sim::fleet::flag::WORMHOLE == 0)
        .map(|(k, f)| (k, (f.ground_strength > 0, f.strength)))
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    v.into_iter().map(|x| x.0).collect()
}

/// The tech's status for empire `en` (`+0x45 + e`; 5 = researched).
fn tech_status(g: &Game, en: usize, tech: i32) -> i32 {
    g.state.mem.u8(va::TECHS + (tech as usize * ig2_sim::save::TECH_SIZE) as u32 + 0x45 + en as u32) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_unique_and_found() {
        let mut ids: Vec<i32> = ANSWERS.iter().map(|a| a.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ANSWERS.len());
        assert_eq!(answer(8613), Some(Answer::Part(2)));
        assert_eq!(answer(6701), Some(Answer::Leave));
        assert_eq!(answer(1), None);
    }
}
