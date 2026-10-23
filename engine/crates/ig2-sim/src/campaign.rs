//! The campaign on the story engine: `InitStory` (exe 0x5c2940) and the stories `Story_Check` (exe 0x5c5610)
//! holds inline: 400 (the campaign driver: heroes, storyline, random stories), 401 (the opening episodes),
//! 402 (a bomb), 403 (an escorted base), 404 (a refit trader), 405/406 (scripted win and loss), 902–918
//! (one-shot actions). Spec: `re/rules/campaign.md` §8.
//!
//! The scripts (`st_Begin_include` and the others) start most of these; the campaign's tables come from the
//! user's exe ([`CampaignTables`]). What the stories do to the rest of the game goes out as
//! [`StoryEvent`]s and [`StoryWorld`] calls, like the scripts'.

use crate::story::{last_fn, Abort, ScenarioStory, Story, StoryEvent, StoryWorld, STREAM};

/// The campaign tables, the exe's words from VA [`CampaignTables::VA`] (`StoryEleje` 0x928340, `StoryElejeSeq`,
/// `StoryLine`, `StoryScenario`, `RndStoryk` 0x928540, `RndStoryValasztas`, `StoryHosok`, `RndHosok` 0x9288B0,
/// `StoryKuldetes` 0x9289AC; spec §8.1). The accessors index them the way the exe does, campaign rows by `CampaignType` 1..=3, so the
/// rows overlap where the exe's do (campaign 3's opening sequence is campaign 1's storyline row).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CampaignTables {
    words: Vec<i32>,
}

/// The tables of a world without a campaign: every read is 0.
pub static NO_TABLES: CampaignTables = CampaignTables { words: Vec::new() };

impl CampaignTables {
    pub const VA: u32 = 0x928340;
    pub const WORDS: usize = 423;
    const RND_STORYK: u32 = 0x928540;

    /// From the exe's words; `RndStoryk`'s row counts (word 0 of each of the 5 rows, −1 in the image) are
    /// filled in as `Story_Check` does the first time it needs them: the run of non-zero entries from 1.
    pub fn new(mut words: Vec<i32>) -> Self {
        let at = |va: u32| ((va - Self::VA) / 4) as usize;
        if words.get(at(Self::RND_STORYK)).is_some_and(|&c| c < 0) {
            for row in 0..5 {
                let base = at(Self::RND_STORYK) + 35 * row;
                let n = (1..35).take_while(|&i| words.get(base + i).is_some_and(|&v| v != 0)).count();
                words[base] = n as i32;
            }
        }
        Self { words }
    }

    fn at(&self, va: u32) -> i32 {
        va.checked_sub(Self::VA).and_then(|o| self.words.get((o / 4) as usize)).copied().unwrap_or(0)
    }

    fn word(&self, base: u32, row: i32, stride: u32, i: i32) -> i32 {
        self.at(base.wrapping_add((row as u32).wrapping_mul(stride)).wrapping_add((i as u32).wrapping_mul(4)))
    }

    /// `StoryEleje[k][i]`: opening episode group `k` (0..=5): `[0]` = count, then story ids.
    pub fn opening(&self, k: i32, i: i32) -> i32 {
        self.word(0x928340, k, 0x28, i)
    }

    /// `StoryElejeSeq[campaign][step]`: the group of opening step 0..=3.
    pub fn opening_seq(&self, campaign: i32, step: i32) -> i32 {
        self.word(0x928420, campaign, 0x10, step)
    }

    /// `StoryLine[campaign][i]`: `[0]` = count, then the storyline stories.
    pub fn storyline(&self, campaign: i32, i: i32) -> i32 {
        self.word(0x928428, campaign, 0x28, i)
    }

    /// `StoryScenario[campaign][i]`: `[0]` = count, then the campaign maps (`scenario/<n>.scn`).
    pub fn scenario(&self, campaign: i32, i: i32) -> i32 {
        self.word(0x9284a0, campaign, 0x28, i)
    }

    /// `RndStoryk[row][i]` (5 rows of 35): `[0]` = count, then random stories (`≤ 0` = disabled).
    pub fn random_story(&self, row: i32, i: i32) -> i32 {
        self.word(Self::RND_STORYK, row, 0x8c, i)
    }

    /// `RndStoryValasztas[campaign][i]`: `[0]` = count, then the `RndStoryk` rows the campaign draws from.
    pub fn random_rows(&self, campaign: i32, i: i32) -> i32 {
        self.word(0x9287e8, campaign, 0x14, i)
    }

    /// `StoryHosok[campaign][i]`: `[0]` = candidates, `[1]` = how many heroes to start, `[2..]` the stories.
    pub fn heroes(&self, campaign: i32, i: i32) -> i32 {
        self.word(0x928810, campaign, 0x28, i)
    }

    /// `RndHosok[k].id` (9 records of 0x1c bytes): the hero ships' story design ids.
    pub fn hero_design(&self, k: i32) -> i32 {
        self.word(0x9288b0, k, 0x1c, 0)
    }

    /// `RndHosok[k]` word `i` (0..=6): id, tech, the three battle bonuses (`StoryAdat+0x84`, `+0x90`, `+0x7c`),
    /// the fee in thousands, the strength (`st_Rnd` 242).
    pub fn hero(&self, k: i32, i: i32) -> i32 {
        self.word(0x9288b0, k, 0x1c, i)
    }

    /// `StoryKuldetes[k]` (12 words): the story whose spy mission is `KemKuldetesek[14 + k]` (`st_Rnd` 127).
    pub fn story_mission(&self, k: i32) -> i32 {
        self.word(0x9289ac, 0, 0, k)
    }
}

/// The story variables, counters and planet ids `Story_Check` uses (decimal = the exe's hex).
pub mod id {
    pub const DRIVER: i32 = 400;
    pub const OPENING: i32 = 401;
    /// Variables 4001..=4003: storyline index, campaign map, storyline story (`InitStory`); counters
    /// 4001..=4003: story 400's timers.
    pub const V_LINE: i32 = 4001;
    pub const V_MAP: i32 = 4002;
    pub const V_LINE_STORY: i32 = 4003;
    /// Story 401: variable/counter 4011 (the step and its timer), variable 4012 (the episode started).
    pub const OPENING_STEP: i32 = 4011;
    pub const OPENING_EPISODE: i32 = 4012;
}

fn rnd(w: &mut dyn StoryWorld, lo: i32, hi: i32) -> i32 {
    w.rng().range(STREAM, lo, hi)
}

/// `RndClass(5)` + `Next(1)`: reservoir sampling, the `n`-th candidate replaces the pick with chance `1/n`.
#[derive(Default)]
struct Pick {
    n: i32,
}

impl Pick {
    fn offer(&mut self, w: &mut dyn StoryWorld) -> bool {
        self.n += 1;
        rnd(w, 1, self.n) <= 1
    }
}

// ---------------------------------------------------------------- InitStory (spec §8.2)

impl Story {
    /// `InitStory(mode)` (exe 0x5c2940), `mode == 0` for a campaign (`InitNewGame` passes its game mode):
    /// clears the buckets, `StoryAdat` and the history, finish and start queues; in a campaign picks the
    /// storyline (`storyline` config, else the registry's last one → `3 − v`, else stream 5) and the map
    /// (`storymap` config, else stream 5), loads `scenario/<map>.scn`, sets variables 4001..=4003, rolls
    /// the two mirror flags (stream 5), gives the human's planets their first buildings (2 and 6, campaign 2
    /// also 0x32 and 0x34), names the star systems and sets `StoryAdat+0xbc` (campaigns 1, 2) and `+0xb8`
    /// (campaigns 2, 3).
    pub fn init_story(&mut self, mode: i32, w: &mut dyn StoryWorld) {
        self.init_story_cleared();
        if mode != 0 {
            return;
        }
        self.init_story_campaign(w);
    }

    /// `InitStory`'s part for every mode: the buckets, `StoryAdat`, the history, finish and start queues cleared.
    pub fn init_story_cleared(&mut self) {
        for b in self.buckets.iter_mut() {
            b.entries.clear();
        }
        self.adat = [0; crate::story::ADAT];
        self.history.entries.clear();
        self.finished.entries.clear();
        self.starting.entries.clear();
        self.line_story = 0;
    }

    fn init_story_campaign(&mut self, w: &mut dyn StoryWorld) {
        let ct = w.campaign_type();
        let t = w.campaign_tables().clone();
        let mut line = w.config_int("storyline");
        if line == 0 {
            line = match w.registry_storyline(ct) {
                Some(v) if v != 0 => 3 - v,
                _ => rnd(w, 1, t.storyline(ct, 0)),
            };
        }
        self.line_story = t.storyline(ct, line);
        let mut map = w.config_int("storymap");
        if map == 0 {
            map = rnd(w, 1, t.scenario(ct, 0));
        }
        let scenario = t.scenario(ct, map);
        for op in w.load_scenario(scenario) {
            match op {
                ScenarioStory::Var(k, v) => self.v_set(k, v),
                ScenarioStory::Planet(p, k) => self.b_add_id(p, k),
                ScenarioStory::Start(id) => {
                    self.start_story(id, false);
                }
                ScenarioStory::SzStart(id, days) => self.sz_start(id, days),
            }
        }
        self.v_set(id::V_LINE, line);
        self.v_set(id::V_MAP, scenario);
        self.v_set(id::V_LINE_STORY, self.line_story);
        self.scenario = scenario;
        let mirror_x = rnd(w, 0, 1) != 0;
        let mirror_y = rnd(w, 0, 1) != 0;
        w.event(StoryEvent::Mirror { x: mirror_x, y: mirror_y });
        let en = w.human();
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) == en {
                let kinds: &[i32] = if ct == 2 { &[2, 6, 0x32, 0x34] } else { &[2, 6] };
                for &kind in kinds {
                    w.event(StoryEvent::PlaceBuilding { planet: p, kind });
                }
            }
        }
        let (names, systems) = w.generate_star_names();
        for (p, k) in names {
            self.b_add_id(p, k);
        }
        self.v_set(500, systems);
        if ct == 1 || ct == 2 {
            self.set_adat_i32(0xbc, 1);
        }
        if ct == 2 || ct == 3 {
            self.set_adat_i32(0xb8, 1);
        }
    }

    /// `st_AddKolonizalo(count)` (exe 0x5c9dc0): every other live empire 1..=8 with a planet the human can
    /// see gets a fleet there (`st_fNewEx("", e, 0, 0, 2, planet)`) with `count` of its colony ships
    /// (`MiTervMutato[e][13]`, design 4 when unset).
    pub fn add_colonizers(&mut self, count: i32, w: &mut dyn StoryWorld) {
        for e in 1..=8 {
            if !w.empire_alive(e) || e == w.human() {
                continue;
            }
            let p = self.b_search(-e, 1, w);
            if p != 0 {
                let f = self.f_new_ex("", e, 0, 0.0, 2, p, w);
                let design = match w.ai_design(e, 13) {
                    d if d < 1 => 4,
                    d => d,
                };
                self.f_add_ship(f.wrapping_neg(), design, count, w);
            }
        }
    }

    /// `st_AddDiplomacia(id, e, owner)`: a diplomacy special and its undo entry (type 3) for `owner`
    /// (−1: `id / 10`; 0: none).
    pub fn add_diplomacy(&mut self, id: i32, e: i32, owner: i32, w: &mut dyn StoryWorld) {
        let owner = if owner == -1 { id / 10 } else { owner };
        w.event(StoryEvent::DiplomacySpecial { id, empire: e });
        if owner != 0 {
            self.add_undo(3, (id & 0xffff) | (e << 16), owner);
        }
    }

    /// `st_SetBolygoTulaj(p, owner, -1)` (inlined): colonised for `owner` with its race when free, else
    /// conquered (and the race written when non-zero).
    pub fn set_planet_owner(&mut self, p: i32, owner: i32, w: &mut dyn StoryWorld) {
        let race = w.race(owner);
        let conquest = w.planet_owner(p) != 0;
        w.event(StoryEvent::SetPlanetOwner { planet: p, owner, race });
        // `BolygoElfoglal` cures the planet (exe 0x542980) and, with nothing infected left, ends the epidemic:
        // `StoryAdat+0x30 := 0` and info 0x65 off every planet.
        if conquest && w.conquest_ends_epidemic(p) {
            self.set_adat_i32(crate::story::adat::EPIDEMIC, 0);
            w.event(StoryEvent::EpidemicOver);
        }
    }

    /// `st_bUnhide(x)` (exe 0x604040): planet `−x` (`x ≤ 0`) or the planet with story id `x`; every empire's
    /// unknown view of it becomes 0. `StoryError` = no planet.
    pub fn b_unhide(&mut self, x: i32, w: &mut dyn StoryWorld) {
        let p = if x > 0 { self.b_search(x, 0, w) } else { x.wrapping_neg() };
        if p > 0 {
            self.error = 0;
            w.event(StoryEvent::UnhidePlanet(p));
        } else {
            self.error = 1;
        }
    }

    /// `st_bSetBarat(id, on)`: planet `id`'s `+0xbc`. `StoryError` = no planet.
    pub fn b_set_friendly(&mut self, id: i32, on: bool, w: &mut dyn StoryWorld) {
        let p = self.b_search(id, 0, w);
        if p < 1 {
            self.error = 1;
        } else {
            self.error = 0;
            w.event(StoryEvent::SetPlanetFriendly { planet: p, friendly: on });
        }
    }

    /// `StoryInit8160()` (exe 0x5c4d40): planet 8160 goes to empire 10 and is shown to everybody.
    pub fn init_8160(&mut self, w: &mut dyn StoryWorld) {
        let p = self.b_search(0x1fe0, 1, w);
        self.set_planet_owner(p, 10, w);
        self.b_unhide(p.wrapping_neg(), w);
    }

    /// `StoryInit804()` (exe 0x5c49c0): planets 8040..=8042 are shown to everybody and go to empire 10.
    pub fn init_804(&mut self, w: &mut dyn StoryWorld) {
        let p: Vec<i32> = (0x1f68..=0x1f6a).map(|id| self.b_search(id, 1, w)).collect();
        for &q in &p {
            self.b_unhide(q.wrapping_neg(), w);
        }
        for &q in &p {
            self.set_planet_owner(q, 10, w);
        }
    }

    /// `st_GetBirodalom808hoz()` = `…852hoz` (exe 0x5ea400): from a random planet of the human's (stream 5),
    /// the owner of the nearest planet (f32 `dy² + dx²` from the start's truncated coordinates, the first of
    /// equals) of an empire 2, 4, 6, 7 or 8 other than the human's at peace with it (`Viszony` level > 2);
    /// once a known empire's planet is taken, only known empires' count. The planet goes to
    /// `nearest_planet`. 0 when there is none (or the human has no planet).
    pub fn empire_near_human(&mut self, w: &mut dyn StoryWorld) -> i32 {
        let en = w.human();
        let start = self.b_search(en.wrapping_neg(), 0, w);
        if start == 0 {
            return 0;
        }
        let (sx, sy) = w.planet_xy(start);
        let (sx, sy) = (sx as i32 as f32, sy as i32 as f32);
        let (mut best, mut best_d, mut known) = (0, 0, false);
        for q in 1..=w.planet_count() {
            let o = w.planet_owner(q);
            if o == 0 || o == en || o > 8 || o == 1 || o == 5 || o == 3 || w.relation_level(en, o) <= 2 {
                continue;
            }
            let (qx, qy) = w.planet_xy(q);
            let (dx, dy) = (sx - qx, sy - qy);
            // `comiss` + `jbe`: an unordered distance doesn't beat the best either.
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if best != 0 && !(dy * dy + dx * dx < best_d as f32) {
                continue;
            }
            if known && !w.knows(en, o) {
                continue;
            }
            best = q;
            best_d = (dy * dy + dx * dx) as i32;
            known |= w.knows(en, o);
        }
        self.nearest_planet = best;
        if best == 0 { 0 } else { w.planet_owner(best) }
    }
}

/// `st_GetLegerosebbEllenseg(a, b)` (exe 0x5ea790): the strongest live empire 1..=8 other than the human,
/// `a` and `b` (`BirodalomEro`, f32; the first of equals); `known`: only those the human knows
/// (`st_GetIsmertLegerosebbEllenseg`, 0x5ea6a0). 0 when there is none.
pub fn strongest_enemy(a: i32, b: i32, known: bool, w: &dyn StoryWorld) -> i32 {
    let en = w.human();
    let mut best = 0;
    for e in 1..9 {
        if e == en || e == a || e == b || !w.empire_alive(e) || (known && !w.knows(en, e)) {
            continue;
        }
        if best == 0 || w.empire_strength(e) > w.empire_strength(best) {
            best = e;
        }
    }
    best
}

/// `st_GetMasodikLegerosebbEllenseg(x)` (exe 0x5ea7e0): the strongest known empire other than empire `x` and the
/// strongest (`GetIsmert…(x, GetLegerosebb(0, 0))`).
pub fn second_strongest_enemy(x: i32, w: &dyn StoryWorld) -> i32 {
    strongest_enemy(x, strongest_enemy(0, 0, false, w), true, w)
}

// ---------------------------------------------------------------- the inline stories (spec §8.3–§8.5)

/// The stories of `Story_Check` after the scripts, in the exe's order. A story a search aborts is
/// `WIN(0, 1)`ed (the catch handler) and the next one runs.
pub fn inline_stories(s: &mut Story, w: &mut dyn StoryWorld) {
    type Inline = fn(&mut Story, &mut dyn StoryWorld) -> Result<(), Abort>;
    let stories: [(i32, Inline); 19] = [
        (400, driver),
        (401, opening),
        (402, bomb),
        (403, escorts),
        (404, refit),
        (405, scripted_win),
        (406, scripted_loss),
        (902, wormhole),
        (903, base),
        (904, ships_904),
        (905, fleet_905),
        (906, money_906),
        (908, reset_designs),
        (909, volcano),
        (910, diplomacy_910),
        (911, auto_ships_911),
        (913, tutorial_913),
        (917, heroes_917),
        (918, fleets_918),
    ];
    for (id, story) in stories {
        if s.running(id) && story(s, w).is_err() {
            s.win(0, true, w);
        }
    }
}

/// 400: the campaign driver.
fn driver(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let ct = w.campaign_type();
    if s.starting_now() {
        let t = w.campaign_tables().clone();
        if ct != 3 {
            s.start_story(118, false);
        }
        s.start_story(142, false);
        s.start_story(199, false);
        if ct == 2 {
            s.start_story(865, false);
        }
        if ct == 3 {
            s.start_story(837, false);
        }
        // The heroes: each time one of the candidates that isn't running (one may come twice: the start
        // queue isn't searched).
        for _ in 0..t.heroes(ct, 1) {
            let mut pick = Pick::default();
            let mut chosen = 0;
            for i in 2..=t.heroes(ct, 0) + 1 {
                if !s.is_running(t.heroes(ct, i)) && pick.offer(w) {
                    chosen = i;
                }
            }
            if chosen != 0 {
                s.start_story(t.heroes(ct, chosen), false);
            }
        }
        if ct == 3 {
            let d = rnd(w, 300, 450);
            s.sz_start(id::V_LINE, d);
        } else {
            s.start_story(id::OPENING, false);
            s.start_story(242, false);
        }
        if ct == 3 && s.v_get(id::V_LINE) == 1 {
            s.start_story(842, false);
        }
    }
    if s.story_finished(id::OPENING) || s.counter(4001) {
        let d = rnd(w, 120, 180);
        s.sz_start(4002, d);
        let d = rnd(w, 600, 650);
        s.sz_start(4003, d);
    }
    // The storyline.
    if s.counter(4002) {
        let t = w.campaign_tables().clone();
        let mut line = s.v_get(id::V_LINE);
        if line == 0 {
            line = rnd(w, 1, t.storyline(ct, 0));
        }
        s.start_story(t.storyline(ct, line), false);
        w.event(StoryEvent::SaveStoryline { campaign: ct, value: line });
    }
    // A random story the campaign hasn't had.
    if s.counter(4003) {
        let t = w.campaign_tables().clone();
        let mut pick = Pick::default();
        let mut chosen = 0;
        for k in 1..=t.random_rows(ct, 0) {
            let row = t.random_rows(ct, k);
            for i in 1..=t.random_story(row, 0) {
                let story = t.random_story(row, i);
                if story > 0 && pick.offer(w) {
                    chosen = story;
                }
            }
        }
        if chosen > 0 && !s.history.contains(chosen) {
            s.start_story(chosen, false);
        }
        let d = rnd(w, 600, 900);
        s.sz_start(4003, d);
    }
    Ok(())
}

/// 401: the opening episodes, one per step 0..=3 from the campaign's groups, `values.txt` days apart.
fn opening(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let d = rnd(w, 220, 280);
        s.sz_start(id::OPENING_STEP, d);
        s.v_set(id::OPENING_STEP, 0);
    }
    if s.counter(id::OPENING_STEP) {
        let step = s.v_get(id::OPENING_STEP);
        s.v_set(id::OPENING_STEP, step + 1);
        let difficulty = w.difficulty();
        if step == 2 && difficulty > 1 {
            s.add_colonizers(if difficulty > 2 { 2 } else { 1 }, w);
        }
        if step > 3 {
            s.win(0, true, w);
            return Ok(());
        }
        let t = w.campaign_tables().clone();
        let group = t.opening_seq(w.campaign_type(), step);
        let en = w.human();
        let episode = loop {
            let r = rnd(w, 1, t.opening(group, 0));
            let episode = t.opening(group, r);
            if episode != 9 {
                break episode;
            }
            // Episode 9 needs an empire the human knows (the scan runs on past 8 into the neutrals).
            let mut e = 1;
            while e < 64 && ((e < 9 && e == en) || !w.empire_alive(e) || !w.knows(en, e)) {
                e += 1;
            }
            if e <= 8 {
                break episode;
            }
        };
        s.v_set(id::OPENING_EPISODE, episode);
        s.start_story(episode, false);
        let days = Story::get_value(&s.values, group + 1, difficulty);
        s.sz_start(id::OPENING_STEP, days);
    }
    Ok(())
}

/// 402: a planet bombed (special 0x12 on the selected planet of another empire) blows up after 60 days
/// unless it changes hands to nobody first; a countdown info line.
fn bomb(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const V: i32 = 4021;
    if s.starting_now() {
        s.set_adat_i32(0x44, 1);
        s.v_set(V, 0);
    }
    if s.flags[crate::story::flag::SPECIAL] && s.special[0] == 0x12 {
        let p = w.akt_planet();
        let owner = w.planet_owner(p);
        if owner != 0 && owner != w.human() {
            s.set_adat_i32(0x44, 0);
            s.sz_start(4022, 60);
            s.v_set(V, 1);
            s.b_add_id(p, V);
            s.add_game_info(V, 60, -1, w);
        }
    }
    if s.daily() && s.v_get(V) != 0 {
        s.remove_game_info(V, w);
        let days = s.sz_get(4022);
        let p = s.b_search(V, 0, w);
        if w.planet_owner(p) == 0 {
            s.win(0, true, w);
        } else if days == 0 {
            s.error = (p < 1) as i32;
            w.event(StoryEvent::DestroyPlanet(p));
            w.event(StoryEvent::Anim(6));
            s.win(0, true, w);
        } else {
            s.add_game_info(V, days, -1, w);
        }
    }
    Ok(())
}

/// 403: planet 4031 (owner 10) guarded by up to four fleets 4030..=4033; they chase the human's fleets that
/// come within 50 (weighted by closeness), circle the planet, are rebuilt every 150 days, and the story ends
/// when the planet is lost and every escort is home.
fn escorts(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const BASE: i32 = 4031;
    const ESCORT: i32 = 4030;
    const OWNER: i32 = 10;
    if s.starting_now() {
        s.sz_start(4039, 25);
        s.sz_start(4038, 12);
    }
    if s.daily() {
        let mut escort = [0i32; 4];
        let mut target = [0i32; 4];
        let mut weights = [(0i32, 0i32); 4];
        for f in 1..=w.fleet_count() {
            let id = w.fleet_id(f);
            if w.fleet_owner(f) == OWNER && (ESCORT..ESCORT + 5).contains(&id) {
                if let Some(e) = escort.get_mut((id - ESCORT) as usize) {
                    *e = f;
                }
            }
        }
        let p = s.b_search(BASE, 0, w);
        let mut guard = true;
        if w.planet_owner(p) == OWNER {
            if s.v_get(4035) != 0 {
                s.sz_stop(4035);
                s.v_set(4035, 0);
            }
        } else {
            guard = false;
            let mut sent = false;
            for k in 0..4 {
                if escort[k] == 0 {
                    continue;
                }
                if w.fleet_5c(escort[k]) == 0 {
                    guard = true;
                } else {
                    sent = true;
                    w.event(StoryEvent::MoveFleet { fleet: escort[k], kind: 2, mode: 1, x: p as f32, y: 0.0 });
                    target[k] = -1;
                }
            }
            if !sent && !guard {
                s.win(0, true, w);
            } else if s.v_get(4035) == 0 {
                s.sz_start(4035, 60);
                s.v_set(4035, 1);
            }
        }
        if s.counter(4035) {
            for &f in &escort {
                if f != 0 {
                    w.event(StoryEvent::DeleteFleet(f));
                }
            }
            s.win(0, true, w);
        }
        if guard {
            let en = w.human();
            let (px, py) = w.planet_xy(p);
            let dist = |a: (f32, f32), b: (f32, f32)| {
                let (dx, dy) = (a.0 - b.0, a.1 - b.1);
                (dy * dy + dx * dx) as i32
            };
            for f in 1..=w.fleet_count() {
                if w.fleet_owner(f) != en {
                    continue;
                }
                let at = w.fleet_xy(f);
                if dist(at, (px, py)) >= 2500 {
                    continue;
                }
                for k in 0..4 {
                    if escort[k] == 0 || target[k] < 0 {
                        continue;
                    }
                    let d = dist(at, w.fleet_xy(escort[k]));
                    if d < 2500 {
                        let weight = 2505 - d;
                        weights[k].0 += 1;
                        weights[k].1 += weight;
                        if rnd(w, 1, weights[k].1) <= weight {
                            target[k] = f;
                        }
                    }
                }
            }
            for k in 0..4 {
                let f = escort[k];
                if f == 0 {
                    continue;
                }
                if target[k] < 1 {
                    if target[k] == 0 && w.fleet_order(f) == 2 {
                        w.event(StoryEvent::MoveFleet { fleet: f, kind: 0, mode: 0, x: 0.0, y: 0.0 });
                    }
                } else if w.fleet_order(f) != 2 {
                    w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 2, x: target[k] as f32, y: 0.0 });
                }
            }
        }
    }
    // The patrol: every 30 days the idle escorts move to the next of 8 points round the planet.
    if s.counter(4038) {
        s.sz_start(4038, 30);
        let turn = (s.v_get(4039) + 1) % 8;
        s.v_set(4039, turn);
        let p = s.b_search(BASE, 0, w);
        if w.planet_owner(p) == OWNER {
            const PHASE: [i32; 4] = [0, 4, 2, 6];
            for (k, phase) in PHASE.into_iter().enumerate() {
                let f = s.f_search(ESCORT + k as i32, w);
                if f != 0 && w.fleet_order(f) == 0 {
                    let a = (phase + turn) as f64 * std::f64::consts::PI * 0.25;
                    let (px, py) = w.planet_xy(p);
                    let x = (a.cos() * 60.0 + px as f64) as i32;
                    let y = (a.sin() * 60.0 + py as f64) as i32;
                    w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x: x as f32, y: y as f32 });
                }
            }
        }
    }
    // Every 150 days one missing escort is rebuilt on the planet.
    if s.counter(4039) {
        s.sz_start(4039, 150);
        let mut pick = Pick::default();
        let mut missing = -1;
        for k in 0..4 {
            if s.f_search(ESCORT + k, w) == 0 && pick.offer(w) {
                missing = k;
            }
        }
        let home = if missing >= 0 { s.b_search(BASE, 0, w) } else { 0 };
        if missing >= 0 && w.planet_owner(home) == OWNER {
            let id = ESCORT + missing;
            let p = s.b_search(BASE, 0, w);
            s.f_new_ex("", OWNER, id, 0.08, 2, p, w);
            s.f_add_ship(id, 3, 3, w);
            s.f_add_ship(id, 2, 8, w);
            s.f_add_tank(id, 1, 3, w);
        }
    }
    Ok(())
}

/// 404: trader fleet 4041 visits the human's refittable ships; the human's budget grows 40 a day.
fn refit(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const TRADER: i32 = 4041;
    if s.starting_now() {
        let f = s.f_search(TRADER, w);
        if f == 0 {
            s.f_new_ex("", 9, TRADER, 0.1, 1, 0, w);
        } else {
            w.event(StoryEvent::SetFleetSpeed { fleet: f, speed: 0.1 });
        }
        s.v_set(TRADER, w.ido() / 50);
    }
    let en = w.human();
    if s.daily() {
        let f = s.f_search(TRADER, w);
        if f == 0 {
            s.win(0, true, w);
        } else {
            let budget = (w.ido() / 50 - s.v_get(TRADER)) * 40;
            let at = w.fleet_xy(f);
            let (mut best, mut best_d) = (0, 0);
            for k in 1..=w.ship_count() {
                let (sf, design) = w.ship(k);
                if design == 0x7fff || w.fleet_owner(sf) != en {
                    continue;
                }
                let up = w.design_upgrade(en, design);
                if up == 0 {
                    continue;
                }
                let other = w.fleet_xy(sf);
                let (dx, dy) = (at.0 - other.0, at.1 - other.1);
                let d = (dy * dy + dx * dx) as i32;
                let cost = w.design_price(en, up) - w.design_price(en, design);
                if 0 < cost && cost <= budget && (best == 0 || d < best_d) {
                    best = sf;
                    best_d = d;
                }
            }
            if best != 0 {
                w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 2, x: best as f32, y: 0.0 });
            }
        }
    }
    if s.arrived_fleet(TRADER, 0) {
        let mut budget = (w.ido() / 50 - s.v_get(TRADER)) * 40;
        let target = s.arrival.target;
        for k in 1..=w.ship_count() {
            let (sf, design) = w.ship(k);
            if design == 0x7fff || sf != target {
                continue;
            }
            let up = w.design_upgrade(en, design);
            if up == 0 {
                continue;
            }
            let cost = w.design_price(en, up) - w.design_price(en, design);
            if cost <= budget {
                budget -= cost;
                w.event(StoryEvent::UpgradeShip { ship: k, design: up });
            }
        }
        s.v_set(TRADER, w.ido() / 50 - budget / 40);
        w.event(StoryEvent::RecalcFleets);
        let f = s.f_search(TRADER, w);
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 0, mode: 0, x: 0.0, y: 0.0 });
    }
    Ok(())
}

/// 405: unless variable 4059 is set, empire `StoryAdat+0xcc` sends message 0x23f0 (none: the story ends);
/// answering it (part 2) wins the game a day later.
fn scripted_win(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() && s.v_get(4059) == 0 {
        let e = s.adat_i32(0xcc);
        if e == 0 {
            return Err(Abort);
        }
        w.event(StoryEvent::Message { id: 0x23f0, empire: e });
    }
    if s.flags[crate::story::flag::MESSAGE] && s.message == (0x23f0, 2) {
        s.sz_start(4051, 1);
    }
    if s.counter(4051) {
        w.event(StoryEvent::Won);
    }
    Ok(())
}

/// 406: the human dies a day after the start.
fn scripted_loss(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.sz_start(4061, 1);
    }
    if s.counter(4061) {
        let en = w.human();
        w.event(StoryEvent::EmpireDies(en));
        s.win(0, true, w);
    }
    Ok(())
}

/// 902: a ship of design 0x1c for the selected fleet; special 0x11 opens a wormhole from fleet `data1` to
/// 20..=28 from planet 9021 (one of the human's).
fn wormhole(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const PLANET: i32 = 9021;
    if s.starting_now() {
        w.event(StoryEvent::NewShip { fleet: w.akt_fleet(), design: 0x1c });
        let p = s.b_search(w.human().wrapping_neg(), 0, w);
        s.b_add_id(p, PLANET);
    }
    if s.special_is(0x11, 0, 0, 0) {
        let p = s.b_search(PLANET, 0, w);
        let r = rnd(w, 20, 28);
        let q = rnd(w, 1, 360);
        let a = q as f64 * std::f64::consts::PI / 180.0;
        let dx = (a.cos() * r as f64) as i32;
        let dy = (a.sin() * r as f64) as i32;
        let (fx, fy) = w.fleet_xy(s.special[1]);
        let (px, py) = w.planet_xy(p);
        w.event(StoryEvent::Wormhole { x1: fx as i32, y1: fy as i32, x2: (dx as f32 + px) as i32, y2: (dy as f32 + py) as i32 });
    }
    Ok(())
}

/// 903: a free planet the human can see becomes planet 4031 of empire 10, then story 403 runs.
fn base(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let p = s.b_search_try(0, 0, w)?;
        s.b_add_id(p, 4031);
        w.event(StoryEvent::SetPlanetOwner { planet: p, owner: 10, race: w.race(10) });
        s.start_story(403, false);
        s.win(0, true, w);
    }
    Ok(())
}

/// 904: five ships of design 4 for the selected fleet.
fn ships_904(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        for _ in 0..5 {
            w.event(StoryEvent::NewShip { fleet: w.akt_fleet(), design: 4 });
        }
    }
    Ok(())
}

/// 905: fleet 8072 of empire 10 appears near one of the human's planets and heads for it.
fn fleet_905(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let f = s.f_new_ex("", 10, 0x1f88, 0.0, 1, 0, w);
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: s.last_fn[last_fn::B] as f32, y: 0.0 });
        s.win(0, true, w);
    }
    Ok(())
}

/// 906: the human loses 100,000.
fn money_906(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        w.event(StoryEvent::AddMoney { empire: w.human(), amount: -100_000.0 });
        s.win(0, true, w);
    }
    Ok(())
}

/// 908: every empire's designs are reset.
fn reset_designs(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        w.event(StoryEvent::ResetDesigns);
        s.win(0, true, w);
    }
    Ok(())
}

/// 909: a volcano erupts on the selected planet.
fn volcano(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        w.event(StoryEvent::Volcano(w.akt_planet()));
        s.win(0, true, w);
    }
    Ok(())
}

/// 910: diplomacy special 0x217b with empire 2 (undo entry for story 857).
fn diplomacy_910(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.add_diplomacy(0x217b, 2, -1, w);
    }
    Ok(())
}

/// 911: `AutoAddship(AktFleet, 9999)` when the selected fleet is the human's.
fn auto_ships_911(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let f = w.akt_fleet();
        if w.fleet_owner(f) == w.human() {
            s.auto_add_ship(f, 9999, w);
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 913: the trade tutorial's destroyer.
fn tutorial_913(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        w.event(StoryEvent::DestroyerTutorial);
        s.win(0, true, w);
    }
    Ok(())
}

/// 917: the nine hero ships for the selected fleet.
fn heroes_917(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let t = w.campaign_tables().clone();
        for k in 0..9 {
            let en = w.human();
            let design = w.story_design(en, t.hero_design(k), 3);
            w.event(StoryEvent::NewShip { fleet: w.akt_fleet(), design });
        }
    }
    Ok(())
}

/// 918: a one-ship fleet for each empire 1..=17 near the human.
fn fleets_918(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        for e in 1..=17 {
            let f = s.f_new_ex("Flotta", e, 0, 0.0, 1, 0, w);
            s.f_add_ship(f.wrapping_neg(), 3, 1, w);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Streams;
    use crate::story::ScenarioStories;

    /// Synthetic tables with the exe's layout (not the shipped values).
    fn tables() -> CampaignTables {
        let mut w = vec![0i32; CampaignTables::WORDS];
        let at = |va: u32| ((va - CampaignTables::VA) / 4) as usize;
        // StoryLine[1] = 2 storylines; StoryScenario[1] = 1 map; StoryHosok[1]: 2 candidates, start 1.
        w[at(0x928428 + 0x28)..at(0x928428 + 0x28) + 3].copy_from_slice(&[2, 700, 701]);
        w[at(0x9284a0 + 0x28)..at(0x9284a0 + 0x28) + 2].copy_from_slice(&[1, 555]);
        w[at(0x928810 + 0x28)..at(0x928810 + 0x28) + 4].copy_from_slice(&[2, 1, 176, 177]);
        // RndStoryk rows: −1 counts (as in the image), row 0 = 300, −301, 302.
        for r in 0..5 {
            w[at(0x928540) + 35 * r] = -1;
        }
        w[at(0x928540) + 1..at(0x928540) + 4].copy_from_slice(&[300, -301, 302]);
        CampaignTables::new(w)
    }

    struct W {
        t: CampaignTables,
        rng: Streams,
        events: Vec<StoryEvent>,
        owners: Vec<i32>,
    }

    impl StoryWorld for W {
        fn ido(&self) -> i32 {
            5000
        }
        fn speed_mult(&self) -> i32 {
            1
        }
        fn human(&self) -> i32 {
            1
        }
        fn difficulty(&self) -> i32 {
            2
        }
        fn multiplayer(&self) -> bool {
            false
        }
        fn rng(&mut self) -> &mut Streams {
            &mut self.rng
        }
        fn planet_count(&self) -> i32 {
            self.owners.len() as i32 - 1
        }
        fn planet_owner(&self, p: i32) -> i32 {
            self.owners[p as usize]
        }
        fn planet_value(&self, _p: i32) -> i32 {
            0
        }
        fn planet_visibility(&self, _p: i32) -> i32 {
            0
        }
        fn fleet_count(&self) -> i32 {
            0
        }
        fn fleet_owner(&self, _f: i32) -> i32 {
            0xff
        }
        fn fleet_id(&self, _f: i32) -> i32 {
            0
        }
        fn empire_alive(&self, _e: i32) -> bool {
            true
        }
        fn planets_of(&self, _e: i32) -> i32 {
            3
        }
        fn money(&self, _e: i32) -> f64 {
            0.0
        }
        fn tech_status(&self, _t: i32, _e: i32) -> i32 {
            0
        }
        fn relation_level(&self, _a: i32, _b: i32) -> i32 {
            3
        }
        fn event(&mut self, ev: StoryEvent) {
            self.events.push(ev);
        }
        fn campaign_type(&self) -> i32 {
            1
        }
        fn campaign_tables(&self) -> &CampaignTables {
            &self.t
        }
        fn registry_storyline(&self, _c: i32) -> Option<i32> {
            Some(1)
        }
        fn load_scenario(&mut self, _n: i32) -> Vec<ScenarioStory> {
            vec![ScenarioStory::Var(500, 10), ScenarioStory::Planet(2, 8020)]
        }
    }

    fn world() -> W {
        W { t: tables(), rng: Streams::default(), events: Vec::new(), owners: vec![0, 1, 2, 1] }
    }

    #[test]
    fn random_story_counts_are_filled_in() {
        let t = tables();
        assert_eq!(t.random_story(0, 0), 3, "the run of non-zero entries from 1, disabled ones included");
        assert_eq!(t.random_story(1, 0), 0);
        assert_eq!(t.storyline(1, 2), 701);
        assert_eq!(NO_TABLES.storyline(1, 0), 0);
    }

    #[test]
    fn init_story_takes_the_other_storyline_and_mirrors() {
        let mut s = Story::default();
        let mut w = world();
        s.start_story(360, true);
        s.init_story(0, &mut w);
        assert!(!s.is_running(360), "the buckets are cleared");
        // Registry 1 → storyline 3 − 1 = 2; the one map; two mirror draws on stream 5.
        assert_eq!(s.line_story, 701);
        assert_eq!(s.scenario, 555);
        assert_eq!(s.vars.live(), &[(500, 0), (4001, 2), (4002, 555), (4003, 701)]);
        assert_eq!(s.planets, vec![(8020, 2)]);
        assert_eq!(s.adat_i32(0xbc), 1);
        assert_eq!(s.adat_i32(0xb8), 0);
        let buildings: Vec<_> = w.events.iter().filter(|e| matches!(e, StoryEvent::PlaceBuilding { .. })).collect();
        assert_eq!(buildings.len(), 4, "buildings 2 and 6 on the human's planets 1 and 3");
        // The map pick (one map, still a draw) and the two mirror flags on stream 5.
        let mut r = Streams::default();
        r.range(STREAM, 1, 1);
        r.range(STREAM, 0, 1);
        r.range(STREAM, 0, 1);
        assert_eq!(w.rng.state(STREAM), r.state(STREAM));
    }

    #[test]
    fn driver_starts_heroes_and_the_opening() {
        let mut s = Story::default();
        let mut w = world();
        s.start_story(400, false);
        s.check(true, &ScenarioStories, &mut w);
        for id in [118, 142, 199, 401, 242] {
            assert!(s.is_running(id), "story {id}");
        }
        assert!(s.is_running(176) || s.is_running(177), "one hero");
        assert_eq!(s.counters.get(id::OPENING_STEP).map(|t| (220..=280).contains(&(t / 50))), Some(true));
        assert_eq!(s.vars.get(id::OPENING_STEP), Some(0));
    }

    #[test]
    fn story_405_without_a_sender_aborts_into_win() {
        let mut s = Story::default();
        let mut w = world();
        s.start_story(405, true);
        s.check(true, &ScenarioStories, &mut w);
        assert!(!s.is_running(405));
        assert_eq!(s.history.entries, vec![(405, 5000)]);
        assert!(w.events.is_empty());
    }
}
