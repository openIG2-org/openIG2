//! What the story scripts do to the rest of the game (`StoryEvent`, `re/rules/new-game.md` §15), carried out the
//! moment a script asks, as the exe calls the system there: later reads of the same script see it.

use super::story_world::GameStory;
use super::{Game, GameEvent};
use crate::story::StoryEvent;

impl GameStory<'_, '_> {
    /// Carry out what a script asked of the rest of the game, at once (the exe calls the system there).
    pub(super) fn carry_out(&mut self, ev: &StoryEvent) {
        match *ev {
            StoryEvent::StopFleet(f) => self.with_game(|g| g.stop_fleet(f)),
            StoryEvent::Wormhole { x1, y1, x2, y2 } => self.made = self.with_game(|g| g.add_wormhole(x1, y1, x2, y2)),
            StoryEvent::RevealWormhole if self.made != 0 => {
                let w = self.made;
                self.with_game(|g| g.reveal_wormhole(w));
            }
            _ => {}
        }
    }
}

impl Game<'_> {
    /// `FlottaMozgat(f, 0, 0, 0, 0)`: the fleet stops.
    pub(super) fn stop_fleet(&mut self, f: i32) {
        let g = self.globals();
        let ctx = self.starmap_context(&g);
        let mut map = self.state.starmap();
        let mut ev = Vec::new();
        map.order(f.max(0) as usize, 0, 0, 0.0, 0.0, &ctx, &mut ev);
        self.state.store_starmap(&map);
    }

    /// `FlottaNew(name, owner, id, x, y, speed)` (exe 0x53d400, `production-research.md` §5.1): the new fleet's slot.
    pub(super) fn new_fleet(&mut self, name: &[u8], owner: u8, id: i16, x: f32, y: f32, speed: f32) -> usize {
        let g = self.globals();
        let ctx = self.production_context(&g, g.speed);
        let mut yard = self.shipyard();
        let mut ev = Vec::new();
        let f = yard.new_fleet(name, owner, id, x, y, speed, &ctx, &mut ev);
        self.store_shipyard(&yard, &ev);
        self.events.extend(ev.into_iter().map(GameEvent::Production));
        f
    }

    /// `Spy_New(e)` (exe 0x58f830, `espionage.md` §6.1): the new spy's slot (−1 no room).
    pub(super) fn new_spy(&mut self, e: usize) -> i32 {
        let mut slot = 0;
        self.with_spies(|sp, w| slot = sp.new_spy(w, e));
        slot
    }
}
