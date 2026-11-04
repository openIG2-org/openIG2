//! The `st_*` helpers every campaign script module uses: the story's random draw, the message predicates and a
//! new message.

use crate::story::{flag, Story, StoryEvent, StoryWorld, STREAM};

/// A story draw in `lo..=hi` (`rndlog(STREAM, lo, hi)`).
pub(crate) fn rnd(w: &mut dyn StoryWorld, lo: i32, hi: i32) -> i32 {
    w.rng().range(STREAM, lo, hi)
}

/// `st_Message(b, r)`: the human answered part `r` of message `b`.
pub(crate) fn answered(s: &Story, b: i32, r: i32) -> bool {
    s.flags[flag::MESSAGE] && s.message == (b, r)
}

/// `st_Elevul(b)`: message `b` was promoted (flag 7).
pub(crate) fn promoted(s: &Story, b: i32) -> bool {
    s.flags[flag::PROMOTED] && s.message.0 == b
}

/// `st_Olvas(b)`: message `b` was read (flag 9, `MessageSelectedBlokk`).
pub(crate) fn read(s: &Story, b: i32) -> bool {
    s.flags[flag::READ] && s.message.0 == b
}

/// `st_NewMessage(id, arg)`.
pub(crate) fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

/// The story wormhole from the fleet of the special just reported (`special[1]`) to a random point 20..=28 from
/// the planet with story id `planet` (stories 903 and 9021).
pub(crate) fn wormhole_near(s: &mut Story, w: &mut dyn StoryWorld, planet: i32) {
    let p = s.b_search(planet, 0, w);
    let r = rnd(w, 20, 28);
    let q = rnd(w, 1, 360);
    let a = q as f64 * std::f64::consts::PI / 180.0;
    let dx = (a.cos() * r as f64) as i32;
    let dy = (a.sin() * r as f64) as i32;
    let (fx, fy) = w.fleet_xy(s.special[1]);
    let (px, py) = w.planet_xy(p);
    w.event(StoryEvent::Wormhole { x1: fx as i32, y1: fy as i32, x2: (dx as f32 + px) as i32, y2: (dy as f32 + py) as i32 });
}
