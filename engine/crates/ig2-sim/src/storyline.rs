//! The campaign's storylines: `st_Line_include` (exe 0x5eb6f0, ELF 0x7771e0; `st_line.cpp`), run by
//! `Story_Check` after `st_Scen_include`. Spec: `re/rules/campaign.md` §8.9.
//!
//! Stories 501 and 502 relay timed messages; 530, 550, 560, 680, 710 and 720 are the storylines story 400
//! starts (`StoryLine`): chains of `values.txt` timers that start the episodes (stories 8xx of the other
//! scripts), send messages and react to their answers, and end with `WIN` when the last episode finishes. The
//! messages, planet changes and the empires met go out as [`StoryEvent`]s.

use crate::campaign::second_strongest_enemy;
use crate::story::{flag, Abort, MessageText, Script, Story, StoryEvent, StoryWorld, STREAM};

/// `st_Line_include`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LineStories;

impl Script for LineStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        type Line = fn(&mut Story, &mut dyn StoryWorld) -> Result<(), Abort>;
        let stories: [(i32, Line); 8] = [
            (501, messages_501),
            (502, messages_502),
            (530, line_530),
            (550, line_550),
            (560, line_560),
            (680, line_680),
            (710, line_710),
            (720, line_720),
        ];
        for (id, story) in stories {
            // The catch handler of each story block: `WIN(0, 1)`, then the next story.
            if s.running(id) && story(s, w).is_err() {
                s.win(0, true, w);
            }
        }
    }
}

// ---------------------------------------------------------------- helpers

/// `st_Message(b, r)`: the human answered part `r` of message `b`.
fn answered(s: &Story, b: i32, r: i32) -> bool {
    s.flags[flag::MESSAGE] && s.message.0 == b && s.message.1 == r
}

/// `st_Elevul(b)`: message `b` was promoted (flag 7).
fn promoted(s: &Story, b: i32) -> bool {
    s.flags[flag::PROMOTED] && s.message.0 == b
}

/// `st_Letilt(b, 0)`: message `b` asks which of its answers to disable (flag 8).
fn disabling(s: &Story, b: i32) -> bool {
    s.flags[flag::DISABLED] && s.message.0 == b
}

fn message(w: &mut dyn StoryWorld, id: i32) {
    w.event(StoryEvent::NewMessage { id, arg: 0 });
}

fn param(w: &mut dyn StoryWorld, slot: i32, text: MessageText) {
    w.event(StoryEvent::MessageParam { slot, text });
}

/// `szStart(id, st_GetValue(id))`: a timer whose length is the `values.txt` entry of the same id.
fn timer(s: &mut Story, w: &dyn StoryWorld, id: i32) {
    let days = Story::get_value(&s.values, id, w.difficulty());
    s.sz_start(id, days);
}

/// The scripts' `throw CStoryWinException(StoryLastChecked)` unless all of `empires` live.
fn need_alive(w: &dyn StoryWorld, empires: &[i32]) -> Result<(), Abort> {
    if empires.iter().all(|&e| w.empire_alive(e)) { Ok(()) } else { Err(Abort) }
}

fn knows_both(w: &dyn StoryWorld) -> bool {
    let en = w.human();
    w.knows(en, 5) && w.knows(en, 1)
}

// ---------------------------------------------------------------- 501, 502: timed messages

/// 501: counters 5011..=5017 each send one of the messages 0x2166..=0x216c.
fn messages_501(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    for k in 0..7 {
        if s.counter(0x1393 + k) {
            message(w, 0x2166 + k);
        }
    }
    Ok(())
}

/// 502: counters 5021..=5023 send 0x218e, 0x2192, 0x2193 while empire 1 lives and the human knows it.
fn messages_502(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    for (counter, id) in [(0x139d, 0x218e), (0x139e, 0x2192), (0x139f, 0x2193)] {
        let en = w.human();
        if s.counter(counter) && w.knows(en, 1) && w.empire_alive(1) {
            message(w, id);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- the storylines

/// `st_StoryFinish(id)` → `WIN(0, 1)`: the storyline ends with episode `id`.
fn finish_on(s: &mut Story, w: &mut dyn StoryWorld, id: i32) {
    if s.story_finished(id) {
        s.win(0, true, w);
    }
}

/// 530.
fn line_530(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() && s.skip_first == 0 {
        s.start_story(502, false);
        s.start_story(801, false);
        timer(s, w, 0x14b5);
    }
    // An empire near the human offers a planet (message 0x189c).
    if s.counter(0x14b5) {
        s.sz_start(0x139d, 300);
        s.v_set(0x781, 1);
        let e = s.empire_near_human(w);
        if e == 0 {
            s.sz_start(0x206c, 100);
        } else {
            s.v_set(0x1458, e);
            s.v_set(0x1457, 1);
            let p = s.b_search(e.wrapping_neg(), 0, w);
            s.b_add_id(p, 0x1f90);
            if p != 0 {
                w.event(StoryEvent::PlaceBuilding { planet: p, kind: 0x10 });
                w.event(StoryEvent::PlaceBuilding { planet: p, kind: 0xe });
                w.event(StoryEvent::NewMessage { id: 0x189c, arg: 200 });
                w.event(StoryEvent::MessageRace(e));
                param(w, 1, MessageText::Empire(e));
                param(w, 2, MessageText::Planet(p));
            }
        }
        timer(s, w, 0x14b6);
    }
    if s.story_finished(801) {
        s.b_set_friendly(0x1f4a, false, w);
        s.b_set_friendly(0x1f4b, false, w);
    }
    // The offer lapses when the planet changes hands.
    if s.daily() && s.v_get(0x1457) != 0 {
        let p = s.b_search(0x1f90, 0, w);
        if w.planet_owner(p) != s.v_get(0x1458) {
            w.event(StoryEvent::RemoveMessage(0x189c));
            s.v_set(0x1457, 0);
        }
    }
    if answered(s, 0x189c, 2) || answered(s, 0x189c, 3) {
        s.v_set(0x1457, 0);
        if answered(s, 0x189c, 2) {
            s.v_set(0x1f99, 1);
        }
        s.start_story(808, false);
    }
    if promoted(s, 0x189c) {
        s.v_set(0x1457, 0);
        message(w, 0x189d);
        let e = s.v_get(0x1458);
        w.event(StoryEvent::MessageRace(e));
        let e = s.v_get(0x1458);
        param(w, 1, MessageText::Empire(e));
        s.start_story(808, false);
    }
    if s.counter(0x14b6) {
        s.start_story(824, false);
        timer(s, w, 0x14b7);
    }
    if s.counter(0x14b7) {
        s.sz_start(0x139e, 300);
        if s.v_get(0x1f41) == 2 {
            message(w, 0x1f67);
        } else {
            message(w, 0x1f65);
            s.start_story(817, false);
            timer(s, w, 0x14b8);
        }
    }
    // Message 0x1f67: pay 500,000 (4) or gamble (5) to get rid of the marked planets; without one, both
    // answers are disabled.
    let en = w.human();
    let marked = |w: &dyn StoryWorld, p: i32| w.planet_owner(p) == en && (w.planet_has_info(p, 0x1f41) || w.planet_has_info(p, 0x1f42));
    if disabling(s, 0x1f67) && !(1..=w.planet_count()).any(|p| marked(&*w, p)) {
        w.event(StoryEvent::DisablePart(4));
        w.event(StoryEvent::DisablePart(5));
    }
    let mut cleared = false;
    if answered(s, 0x1f67, 4) {
        w.event(StoryEvent::AddMoney { empire: en, amount: -500_000.0 });
        cleared = true;
        timer(s, w, 0x14b8);
    }
    if answered(s, 0x1f67, 5) {
        cleared = true;
        if w.rng().range(STREAM, 0, 99) < 50 {
            s.sz_start(0x14bc, 30);
        } else {
            timer(s, w, 0x14b8);
        }
    }
    if s.counter(0x14bc) {
        message(w, 0x1f66);
        s.start_story(817, false);
        timer(s, w, 0x14b8);
    }
    // Planets marked 0x1f41 are given up; 0x1f42 lose 9200 of `+0x28` (given up below 300).
    if cleared {
        for p in 1..=w.planet_count() {
            if !marked(&*w, p) {
                continue;
            }
            if w.planet_has_info(p, 0x1f41) {
                s.set_planet_owner(p, 0, w);
                w.event(StoryEvent::RemovePlanetInfo { planet: p, info: 0x1f41 });
            } else {
                let mut value = w.planet_value(p).wrapping_sub(0x23f0);
                let lost = value < 300;
                if lost {
                    value = 0;
                }
                w.event(StoryEvent::SetPlanetValue { planet: p, value });
                if lost {
                    s.set_planet_owner(p, 0, w);
                }
                w.event(StoryEvent::RemovePlanetInfo { planet: p, info: 0x1f42 });
            }
        }
    }
    if answered(s, 0x1f67, 3) || answered(s, 0x1f67, 6) || promoted(s, 0x1f67) {
        s.start_story(817, false);
        timer(s, w, 0x14b8);
        s.sz_start(0x14bd, 30);
    }
    if s.counter(0x14bd) {
        message(w, 0x13a7);
    }
    if s.counter(0x14b8) {
        need_alive(w, &[5, 1])?;
        s.start_story(850, false);
        s.sz_start(0x139f, 300);
        timer(s, w, 0x14b9);
    }
    if s.counter(0x14b9) {
        s.sz_start(0x14be, 5);
    }
    if s.counter(0x14be) {
        need_alive(w, &[5, 1])?;
        if knows_both(w) {
            s.start_story(861, false);
        } else {
            s.sz_start(0x14be, 10);
        }
    }
    if s.story_finished(861) {
        s.sz_start(0x14ba, 60);
    }
    if s.counter(0x14ba) {
        need_alive(w, &[5, 1])?;
        let next = if s.v_get(0x21a3) < 1 { 864 } else { 862 };
        s.start_story(next, false);
    }
    if s.story_finished(862) {
        s.sz_start(0x14bb, 90);
    }
    if s.counter(0x14bb) {
        need_alive(w, &[5, 1])?;
        s.start_story(863, false);
    }
    finish_on(s, w, 863);
    finish_on(s, w, 864);
    Ok(())
}

/// 550.
fn line_550(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() && s.skip_first == 0 {
        s.start_story(502, false);
        s.start_story(802, false);
        timer(s, w, 0x157d);
    }
    if s.counter(0x157d) {
        s.sz_start(0x139d, 300);
        let e = s.empire_near_human(w);
        if e != 0 {
            let en = w.human();
            if !w.knows(en, e) {
                w.event(StoryEvent::DiscoverEmpire(e));
            }
            message(w, 0x19c9);
            w.event(StoryEvent::MessageRace(e));
            param(w, 1, MessageText::Empire(e));
            s.v_set(0x2149, e);
        }
        timer(s, w, 0x157f);
    }
    if answered(s, 0x19c9, 2) {
        s.start_story(852, false);
    }
    if s.counter(0x157f) {
        s.sz_start(0x139e, 300);
        message(w, 0x139d);
        s.init_8160(w);
        s.start_story(816, false);
        timer(s, w, 0x1581);
    }
    if s.story_finished(816) {
        message(w, 0x159c);
        s.start_story(813, false);
    }
    if s.story_finished(813) {
        s.v_set(0x781, 1);
    }
    if s.counter(0x1581) {
        message(w, 0x15a4);
        let p = s.b_search(0x1fcc, 0, w);
        param(w, 1, MessageText::Planet(p));
        timer(s, w, 0x1582);
        s.sz_start(0x139f, 300);
    }
    if answered(s, 0x15a4, 4) || answered(s, 0x15a5, 4) {
        let en = w.human();
        w.event(StoryEvent::AddMoney { empire: en, amount: 200_000.0 });
        s.start_story(814, false);
    }
    if s.counter(0x1582) {
        s.start_story(860, false);
        timer(s, w, 0x1583);
    }
    if s.counter(0x1583) {
        s.sz_start(0x158f, 5);
    }
    if s.counter(0x158f) {
        need_alive(w, &[5, 1])?;
        if knows_both(w) {
            s.start_story(861, false);
        } else {
            // 530's timer (a copy-and-paste slip in the original).
            s.sz_start(0x14be, 10);
        }
    }
    if s.story_finished(861) {
        s.sz_start(0x1586, 60);
    }
    if s.counter(0x1586) {
        if s.v_get(0x21a3) < 1 {
            s.start_story(864, false);
        } else {
            s.start_story(862, false);
            timer(s, w, 0x1584);
        }
    }
    if s.counter(0x1584) {
        s.start_story(863, false);
    }
    finish_on(s, w, 863);
    if s.story_finished(864) || (s.skip_first == 8 && s.starting_now()) {
        s.win(0, true, w);
    }
    Ok(())
}

/// 560.
fn line_560(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() && s.skip_first == 0 {
        s.start_story(501, false);
        s.start_story(802, false);
        s.sz_start(0x1393, 300);
        timer(s, w, 0x15e9);
    }
    if s.counter(0x15e9) {
        s.sz_start(0x1394, 300);
        if s.v_get(0x1f54) == 1 {
            let v = s.adat_i32(0x6c).wrapping_sub(25);
            s.set_adat_i32(0x6c, v);
            s.v_set(0x1ffd, 1);
            s.init_8160(w);
            message(w, 0x15ea);
            let p = s.b_search(0x1fe0, 0, w);
            param(w, 1, MessageText::Planet(p));
            s.start_story(816, false);
        } else {
            s.init_804(w);
            message(w, 0x15eb);
            s.start_story(804, false);
        }
        timer(s, w, 0x15e2);
    }
    if s.story_finished(816) {
        match s.v_get(0x1ff5) {
            0 => {
                message(w, 0x15ec);
                let v = s.adat_i32(0x6c).wrapping_add(25);
                s.set_adat_i32(0x6c, v);
                s.start_story(402, false);
            }
            // The exe reads the variable a second time.
            _ if s.v_get(0x1ff5) == 1 => {
                message(w, 0x15ed);
                s.start_story(402, false);
            }
            _ => {
                message(w, 0x15f1);
                let v = s.adat_i32(0x6c).wrapping_add(25);
                s.set_adat_i32(0x6c, v);
            }
        }
    }
    if s.counter(0x15e2) {
        s.start_story(810, false);
        s.sz_start(0x1396, 300);
        timer(s, w, 0x15e3);
    }
    if s.counter(0x15e3) {
        s.sz_start(0x1398, 300);
        message(w, 0x163b);
        w.event(StoryEvent::Anim(6));
        s.start_story(809, false);
        timer(s, w, 0x15e4);
    }
    episodes(s, w, [0x15e4, 0x15e5, 0x15e6]);
    Ok(())
}

/// The last three episodes of 560 and 680: counter `t[0]` starts 856; its end starts timer `t[1]` → 857;
/// its end timer `t[2]` → 858; its end finishes the storyline.
fn episodes(s: &mut Story, w: &mut dyn StoryWorld, t: [i32; 3]) {
    if s.counter(t[0]) {
        s.start_story(856, false);
    }
    if s.story_finished(856) {
        timer(s, w, t[1]);
    }
    if s.counter(t[1]) {
        s.start_story(857, false);
    }
    if s.story_finished(857) {
        timer(s, w, t[2]);
    }
    if s.counter(t[2]) {
        s.start_story(858, false);
    }
    finish_on(s, w, 858);
}

/// 680.
fn line_680(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.start_story(501, false);
        s.start_story(854, false);
        s.sz_start(0x1393, 300);
        timer(s, w, 0x1a91);
    }
    for (counter, story, msg, next) in [(0x1a91, 806, 0x1395, 0x1a92), (0x1a92, 820, 0x1397, 0x1a93)] {
        if s.counter(counter) {
            s.start_story(story, false);
            s.sz_start(msg, 300);
            timer(s, w, next);
        }
    }
    if s.counter(0x1a93) {
        s.sz_start(0x1398, 300);
        s.init_8160(w);
        message(w, 0x1ab1);
        s.start_story(816, false);
        timer(s, w, 0x1a95);
    }
    if s.story_finished(816) {
        s.start_story(807, false);
    }
    episodes(s, w, [0x1a95, 0x1a96, 0x1a97]);
    Ok(())
}

/// 710 and 720's war on the second strongest enemy: `vSet(0x20a7, e)`, message `id` names it, story 835.
fn second_enemy(s: &mut Story, w: &mut dyn StoryWorld, id: i32, slots: &[i32]) {
    let e = second_strongest_enemy(1, &*w);
    if e == 0 {
        s.win(0, true, w);
        return;
    }
    message(w, id);
    s.v_set(0x20a7, e);
    for &slot in slots {
        let e = s.v_get(0x20a7);
        param(w, slot, MessageText::Empire(e));
    }
    s.start_story(835, false);
}

/// 710.
fn line_710(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        timer(s, w, 0x1bbd);
    }
    for (counter, story, next) in [(0x1bbd, 841, 0x1bbe), (0x1bbe, 830, 0x1bbf), (0x1bbf, 831, 0x1bc0), (0x1bc0, 832, 0x1bc1)] {
        if s.counter(counter) {
            s.start_story(story, false);
            timer(s, w, next);
        }
    }
    if s.story_finished(832) {
        timer(s, w, 0x1bc2);
    }
    if s.counter(0x1bc2) {
        message(w, 0x208a);
        s.start_story(815, false);
    }
    if s.counter(0x1bc1) {
        message(w, 0x1c52);
        s.v_set(0x1c52, 1);
        s.start_story(817, false);
        timer(s, w, 0x1bc3);
    }
    if s.counter(0x1bc3) {
        need_alive(w, &[1])?;
        message(w, 0x2197);
        s.start_story(834, false);
        timer(s, w, 0x1bc4);
    }
    if s.counter(0x1bc4) {
        need_alive(w, &[1])?;
        second_enemy(s, w, 0x209e, &[1, 2]);
    }
    war_end(s, w, 0x1bc5)
}

/// 835 finished → timer `t` → message 0x20a8 and story 836; 837 finished ends the storyline (710, 720).
fn war_end(s: &mut Story, w: &mut dyn StoryWorld, t: i32) -> Result<(), Abort> {
    if s.story_finished(835) {
        timer(s, w, t);
    }
    if s.counter(t) {
        need_alive(w, &[1])?;
        message(w, 0x20a8);
        s.start_story(836, false);
    }
    finish_on(s, w, 837);
    Ok(())
}

/// 720.
fn line_720(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.start_story(839, false);
        timer(s, w, 0x1c21);
    }
    for (counter, story, next) in [(0x1c21, 841, 0x1c22), (0x1c22, 840, 0x1c23)] {
        if s.counter(counter) {
            s.start_story(story, false);
            timer(s, w, next);
        }
    }
    if s.counter(0x1c23) {
        message(w, 0x1c8e);
        s.init_8160(w);
        s.start_story(816, false);
        timer(s, w, 0x1c24);
    }
    if s.story_finished(816) {
        message(w, 0x1c98);
        s.init_804(w);
        s.start_story(804, false);
    }
    if s.signal(804, 2) {
        message(w, 0x1ca2);
        s.start_story(843, false);
    }
    if s.counter(0x1c24) {
        s.start_story(844, false);
        timer(s, w, 0x1c25);
    }
    if s.counter(0x1c25) {
        message(w, 0x2197);
        s.sz_start(0x1c48, 40);
    }
    if s.counter(0x1c48) {
        need_alive(w, &[1])?;
        second_enemy(s, w, 0x1c48, &[2]);
    }
    war_end(s, w, 0x1c26)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Streams;
    use crate::story::ScenarioStories;

    #[derive(Default)]
    struct W {
        rng: Streams,
        alive: [bool; 19],
        events: Vec<StoryEvent>,
    }

    impl StoryWorld for W {
        fn ido(&self) -> i32 {
            1000
        }
        fn speed_mult(&self) -> i32 {
            1
        }
        fn human(&self) -> i32 {
            2
        }
        fn difficulty(&self) -> i32 {
            4
        }
        fn multiplayer(&self) -> bool {
            false
        }
        fn rng(&mut self) -> &mut Streams {
            &mut self.rng
        }
        fn planet_count(&self) -> i32 {
            0
        }
        fn planet_owner(&self, _p: i32) -> i32 {
            0
        }
        fn planet_value(&self, _p: i32) -> i32 {
            0
        }
        fn planet_visibility(&self, _p: i32) -> i32 {
            -1
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
        fn empire_alive(&self, e: i32) -> bool {
            self.alive[e as usize]
        }
        fn planets_of(&self, _e: i32) -> i32 {
            0
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
    }

    fn fire(s: &mut Story, w: &mut W, counter: i32) {
        s.flags[flag::COUNTER] = true;
        s.counter_done = counter;
        s.check(true, &(ScenarioStories, LineStories), w);
    }

    #[test]
    fn story_501_relays_its_timers_as_messages() {
        let mut s = Story::default();
        let mut w = W::default();
        s.start_story(501, true);
        fire(&mut s, &mut w, 0x1395);
        assert_eq!(w.events, vec![StoryEvent::NewMessage { id: 0x2168, arg: 0 }]);
    }

    #[test]
    fn story_530_aborts_when_empire_5_is_dead_and_starts_the_next_episode_otherwise() {
        let mut s = Story::default();
        let mut w = W::default();
        w.alive[1] = true;
        s.start_story(530, true);
        s.buckets[530 & 0xff].entries[0].1 = 0;
        fire(&mut s, &mut w, 0x14b8);
        assert!(!s.is_running(530), "WIN(0, 1) from the catch handler");
        assert_eq!(s.history.entries, vec![(530, 1000)]);
        let mut s = Story::default();
        w.alive[5] = true;
        s.start_story(530, true);
        s.buckets[530 & 0xff].entries[0].1 = 0;
        fire(&mut s, &mut w, 0x14b8);
        assert!(s.is_running(850), "story 850 started");
        assert_eq!(s.counters.get(0x139f), Some(300 * crate::story::DAY));
        assert_eq!(s.counters.get(0x14b9), Some(10 * crate::story::DAY), "no values.txt entry: 10 days");
    }
}
