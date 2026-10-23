//! The battle screen on a live battle slot (re/screens/spacewar.md §9.4): what runs it each frame, shared by the
//! window and the scripted snapshot (`--screen spacewar --battle FILE --script S`). The sim's battle advances with the
//! world's ticks; the screen keeps its own state ([`SpaceWarUi`]) and sends its orders as lockstep commands.

use ig2_net::Command;

use crate::spacewar_screen::Gfx;
use crate::spacewar_ui::{Out, SpaceWarUi};
use crate::world::World;

pub struct Live {
    pub ui: SpaceWarUi,
    gfx: Option<Gfx>,
    /// The battle tick the poses last saw.
    seen_tick: i32,
    /// `STAR_M.RAW` (kep 0x1f): the camera cluster's hot-spot mask.
    mask: Vec<u8>,
    /// The mouse buttons held.
    pub left: bool,
    pub right: bool,
    /// The opening zoom has ended.
    zoomed: bool,
    /// The end check closed the battle (`SpaceWarVege` clears `spacewarbanvanbarkacs`, exe 0xF176EE).
    pub ended: bool,
    /// The pointer in window pixels and the window's size (`SpaceWar_Autoscroll` reads the physical window), the
    /// UI ticks done (`Fake_FRISSITES`' 100 Hz loop).
    pub pointer: [i32; 2],
    pub window: [i32; 2],
    ticks100: u64,
    /// The effects (`None` without the exe's tables).
    pub fx: Option<crate::spacewar_fx::Fx>,
}

impl Live {
    /// `SpaceWar_Belep` on battle slot `slot`, from the starmap camera `cam`.
    pub fn enter(world: &mut World, slot: usize, cam: &crate::starmap::Camera, install: &ig2_formats::Install, now: u32) -> Option<Self> {
        let b = world.game.realtime.slots.get(slot)?;
        let ui = SpaceWarUi::enter(slot, b, cam, now);
        let mask = install.read("dataraw/screen/star_m.raw").ok().and_then(|b| b.get(800..800 + 640 * 480).map(|x| x.to_vec())).unwrap_or_default();
        let seen_tick = b.tick;
        world.game.sp_battles.on_screen = true;
        world.game.realtime.fx.clear();
        let fx = crate::spacewar_fx::Tables::read(install).map_err(|e| eprintln!("ig2-client: the battle effects' tables: {e}")).ok().map(crate::spacewar_fx::Fx::new);
        Some(Self { ui, gfx: None, seen_tick, mask, left: false, right: false, zoomed: false, ended: false, pointer: [320, 240], window: [640, 480], ticks100: now as u64 / 10, fx })
    }

    /// The end check closed the battle (`AktKepKell = 2`): `SpaceWar_Kilep`'s zoom-out.
    pub fn over(&mut self, world: &World, now: u32) {
        self.ended = true;
        if let Some(b) = world.game.realtime.slots.get(self.ui.slot) {
            self.ui.leave(b, now);
        }
    }

    /// The frame's update: the poses after the battle ticks, the UI ticks (`FRISSIT_SpaceWar2`, `SpaceWar_Autoscroll`), the view animation,
    /// the fade the end check reads.
    pub fn update(&mut self, world: &mut World, now: u32) -> Vec<Out> {
        let mut out = Vec::new();
        let events: Vec<_> = std::mem::take(&mut world.game.realtime.fx).into_iter().filter(|e| e.0 == self.ui.slot).map(|e| e.1).collect();
        let Some(b) = world.game.realtime.slots.get(self.ui.slot) else { return out };
        let ticks = (b.tick - self.seen_tick).clamp(0, 8);
        for k in 0..ticks {
            self.ui.battle_ticked(b);
            if let Some(fx) = self.fx.as_mut() {
                let (eye, target) = self.ui.eye_target();
                let back = crate::scene::normalise([eye[0] - target[0], eye[1] - target[1], eye[2] - target[2]]);
                let matrices: Vec<_> = self.ui.units.iter().map(|g| g.matrix).collect();
                fx.tick(b, &world.game.rules.space.weapons, if k == 0 { &events } else { &[] }, &matrices, back);
                out.extend(fx.sounds.drain(..).map(Out::Sound));
            }
        }
        self.seen_tick = b.tick;
        // `Fake_FRISSITES`' 100 Hz loop (exe 0x4de850, `while now − stamp > 10`) runs `FRISSIT_SpaceWar2` and then
        // `SpaceWar_Autoscroll` (off while the tutorial runs) once per UI tick (§7.2).
        let due = (now as u64 / 10).saturating_sub(self.ticks100).min(10);
        self.ticks100 += due;
        if due > 0 {
            let combat = world.game.state.combat();
            let plans = crate::spacewar_ui::Plans { designs: &combat.designs, ships: &combat.ships };
            let scroll = world.game.state.mem.u8(ig2_sim::save::va::TUTORIAL_ACTIVE) == 0;
            for _ in 0..due {
                self.ui.tick(b, &plans, &mut out);
                if scroll {
                    self.ui.autoscroll(self.pointer, self.window, self.left || self.right);
                }
            }
        }
        self.ui.frame(b, now, &mut out);
        // The opening zoom ended: the battle runs, unless the human picks a formation first [inf].
        if !self.zoomed && !self.ui.entering && !self.ui.restoring && self.ui.fade >= 1.0 {
            self.zoomed = true;
            if !b.formation_choice {
                out.push(Out::Start);
            }
        }
        world.game.sp_battles.fade = self.ui.fade;
        out
    }

    /// The frame's input (`SpwC::main`, the retreat window, then `SpaceWarMouse` after the picture) and its picture
    /// on the starmap's view `v` (the top bar is the caller's).
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, world: &World, r: &mut crate::render::Renderer, install: &ig2_formats::Install, assets: &crate::starmap::Assets, v: &mut crate::starmap::View, hud: &crate::ui::Hud, tables: &ig2_rules::Rules, mouse: [f32; 2], shift: bool, now: u32, alpha: f32, t: f32, size: [f32; 2]) -> (Option<crate::render::Frame>, Vec<Out>) {
        let mut out = Vec::new();
        let Some(b) = world.game.realtime.slots.get(self.ui.slot) else { return (None, out) };
        self.ui.mouse = mouse;
        let raw = self.ui.spot(&self.mask, mouse);
        let spot = self.ui.latch(raw);
        self.ui.buttons(self.left, self.right, spot == Some(0x4c));
        if !self.left && self.ui.retreat_window {
            let yes = crate::spacewar_hud::retreat_hover(hud, mouse) == 1;
            let paused = world.game.state.mem.u8(ig2_sim::save::va::TUTORIAL_PAUSED) != 0;
            self.ui.retreat_window_frame(yes, paused, &mut out);
        }
        self.ui.hot_spots(b, spot, now, &mut out);
        let mut shown = self.ui.shown(b, tables);
        shown.fx = self.fx.as_ref().map(|fx| (fx, world.game.rules.space.weapons.as_slice()));
        let gfx = self.gfx.get_or_insert_with(|| Gfx::load(r, install, &shown));
        crate::spacewar_screen::prepare(v, gfx, assets.white, &shown);
        let mut frame = v.build(assets, &world.prev, &world.curr, alpha, t);
        let surface = world.curr.planets.iter().find(|p| p.index as i32 == b.planet).map(|p| p.surface);
        crate::spacewar_screen::draw(&mut frame, r, install, gfx, assets, &v.camera, hud, &shown, surface, size);
        if self.ui.retreat_window {
            crate::spacewar_hud::retreat_window(&mut frame, hud, &crate::font::Virtual::new(size), mouse);
        }
        let (units, guns) = crate::spacewar_screen::pick(gfx, assets, &shown, &v.camera);
        let plane = crate::spacewar_screen::plane_point(&shown, mouse, crate::spacewar_ui::PLANE_H * shown.view.scale);
        let cam = v.camera;
        let pole = |p: [f32; 2], y: f32| crate::spacewar_screen::pole_height(&shown, &cam, p, y);
        self.ui.mouse_frame(b, &units, &guns, plane, pole, shift, spot == Some(0x4c), &mut out);
        (Some(frame), out)
    }

    /// The commands an output becomes (`Multi_Send(.., EN, ..)`); `Out::Click` and `Out::Left` are the caller's.
    pub fn commands(&self, o: &Out) -> Vec<Command> {
        use ig2_net::StarmapCmd;
        use ig2_sim::game::StarmapOrder;
        match o {
            Out::Order(order) => vec![Command::SpaceOrder { battle: self.ui.slot as u8, order: (*order).into() }],
            Out::Start => vec![
                Command::Starmap { order: StarmapCmd::from(StarmapOrder::Speed { level: 1 }) },
                Command::Starmap { order: StarmapCmd::from(StarmapOrder::Pause { paused: false }) },
            ],
            Out::Left => vec![Command::SpaceBattleLeft],
            Out::Click | Out::Sound(_) => Vec::new(),
        }
    }
}

/// The screen's view after leaving: the starmap's own camera again.
pub fn restore_view(v: &mut crate::starmap::View) {
    v.camera.look = None;
    v.backdrop.clear();
}
