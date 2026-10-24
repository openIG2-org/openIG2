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
    /// The weapon loops' channels (the shots' handles `+0x84..`), the window's.
    pub loops: std::collections::HashMap<(usize, usize, u8), usize>,
    /// The hot-spots' tooltip.
    tip: crate::kit::tooltip::Tooltip,
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
        Some(Self { ui, gfx: None, seen_tick, mask, left: false, right: false, zoomed: false, ended: false, pointer: [320, 240], window: [640, 480], ticks100: now as u64 / 10, fx, loops: Default::default(), tip: Default::default() })
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
            if k == 0 {
                for e in &events {
                    self.ui.battle_voice(b, e);
                }
            }
            if let Some(fx) = self.fx.as_mut() {
                let (eye, target) = self.ui.eye_target();
                let back = crate::scene::normalise([eye[0] - target[0], eye[1] - target[1], eye[2] - target[2]]);
                let matrices: Vec<_> = self.ui.units.iter().map(|g| g.matrix).collect();
                fx.tick(b, &world.game.rules.space.weapons, if k == 0 { &events } else { &[] }, &matrices, back);
                out.extend(fx.sounds.drain(..).map(Out::Sound));
            }
        }
        // `setsounds` after `Update3D`: the loops once a rendered frame.
        if let Some(fx) = self.fx.as_mut() {
            fx.frame_loops(b, &world.game.rules.space.weapons);
            out.extend(fx.loop_out.drain(..).map(Out::Loop));
        }
        self.seen_tick = b.tick;
        // `Fake_FRISSITES`' 100 Hz loop (exe 0x4de850, `while now − stamp > 10`) runs `FRISSIT_SpaceWar2` and then
        // `SpaceWar_Autoscroll` (off while the tutorial runs) once per UI tick (§7.2).
        let due = (now as u64 / 10).saturating_sub(self.ticks100).min(10);
        self.ticks100 += due;
        if due > 0 {
            let combat = world.game.state.combat();
            let tech = &world.game.rules.space.building_tech;
            let blocks = world.game.state.buildings.get(b.planet.max(0) as usize);
            let gun_tech = b.guns.iter().map(|g| blocks.and_then(|x| x.get(g.building.max(0) as usize)).filter(|_| g.building > 0).and_then(|x| tech.get(x[0] as usize)).map_or(-1, |&t| t as i32)).collect();
            let hp = &world.game.rules.space.building_hp;
            let gun_hp = b.guns.iter().map(|g| blocks.and_then(|x| x.get(g.building.max(0) as usize)).filter(|_| g.building > 0).map_or([0; 2], |x| [i32::from_le_bytes(x[4..8].try_into().unwrap()), hp.get(x[0] as usize).copied().unwrap_or(0)])).collect();
            let plans = crate::spacewar_ui::Plans { designs: &combat.designs, ships: &combat.ships, gun_tech, gun_hp, firepower: &world.game.rules.ai.design.firepower, fleets: &world.game.state.fleets };
            let scroll = world.game.state.mem.u8(ig2_sim::save::va::TUTORIAL_ACTIVE) == 0;
            self.ui.story_no_retreat = world.game.state.mem.i32(ig2_sim::save::va::STORY_ADAT + 0x98) != 0;
            for _ in 0..due {
                self.ui.tick(b, &plans, &mut out);
                if scroll {
                    self.ui.autoscroll(self.pointer, self.window, self.left || self.right);
                }
            }
        }
        self.ui.frame(b, now, &mut out);
        // Without a formation phase nothing on this screen unpauses the battle: the top bar's pause button does
        // (spec §7.4; `IG2_gameUpdate` exe 0x4e0440), and not during the zoom (`nemlehetidotallitani`).
        if !self.zoomed && !self.ui.entering && !self.ui.restoring && self.ui.fade >= 1.0 {
            self.zoomed = true;
        }
        if due > 0 {
            if let Some(pick) = self.ui.formation_due() {
                world.game.pick_formation(self.ui.slot, pick);
                if let Some(b) = world.game.realtime.slots.get(self.ui.slot) {
                    self.ui.placed(b);
                }
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
        // The guns' buildings' damaged fraction (`building +4 / EpuletekLeiro[type] +8`).
        let blocks = world.game.state.buildings.get(b.planet.max(0) as usize);
        let hp = &world.game.rules.space.building_hp;
        shown.gun_damage = b
            .guns
            .iter()
            .map(|g| {
                let Some(x) = blocks.and_then(|x| x.get(g.building.max(0) as usize)).filter(|_| g.building > 0) else { return 0.0 };
                let max = hp.get(x[0] as usize).copied().unwrap_or(0);
                if max == 0 { 0.0 } else { i32::from_le_bytes(x[4..8].try_into().unwrap()) as f32 / max as f32 }
            })
            .collect();
        // A selected gun's building's model (`kisinfogfxfrissit` ELF 0x6860c0): `TTeruletek[t − 1]` of its
        // `EpuletekLeiro` tech `t` (+0), with 0x71 and 0x73 swapped.
        if self.ui.info_type >= 50000 {
            let kind = blocks.and_then(|x| x.get((self.ui.info_type - 50000) as usize)).map_or(0, |x| x[0]);
            let tech = tables.buildings.get(kind as usize).filter(|_| kind != 0).map(|x| match x.tech { 0x71 => 0x73, 0x73 => 0x71, t => t });
            shown.info_other = tech.and_then(|t| tables.research_screen.models.get((t as usize).wrapping_sub(1)).filter(|n| !n.is_empty()).map(|n| (n.clone(), true)));
        }
        let gfx = self.gfx.get_or_insert_with(|| Gfx::load(r, install, &shown));
        crate::spacewar_screen::prepare(v, gfx, assets.white, &shown);
        v.battle_fade = Some(shown.erosseg);
        let mut frame = v.build(assets, &world.prev, &world.curr, alpha, t);
        let surface = world.curr.planets.iter().find(|p| p.index as i32 == b.planet).map(|p| p.surface);
        // `StarmapGfx2D_1`'s names while `starmaphideflottak` is clear (e > 0; spec §9.10), under the console.
        if shown.erosseg > 0.0 {
            hud.draw_starmap_names(&mut frame, v, assets, &world.prev, &world.curr, alpha, t);
        }
        crate::spacewar_screen::draw(&mut frame, r, install, gfx, assets, &v.camera, hud, &shown, surface, size);
        let lv = world.game.local_view();
        let sel = crate::orders::Selection { mode: lv.starmap_mode, planet: lv.planet.max(0) as usize, fleet: lv.fleet.max(0) as usize };
        hud.draw_battle_starmap(&mut frame, &world.game.state, tables, v, &world.prev, &world.curr, &sel, alpha, t, shown.fade);
        let virt = crate::font::Virtual::new(size);
        if let Some(big) = &shown.big {
            crate::spacewar_hud::big_box(&mut frame, hud, &virt, &shown, big);
        }
        if self.ui.retreat_window {
            crate::spacewar_hud::retreat_window(&mut frame, hud, &virt, mouse);
        }
        // The MP versus banner (exe 0x57f800): with more than one empire present (`+0xe30`), the first two of the
        // list (`+0xe09`, `+0xe0a`) with their players' names (`PlayerNev` 0xBAE588, 11 bytes each).
        if world.game.state.mem.u8(ig2_sim::save::va::MULTIPLAYER) != 0
            && b.present.empires.len() > 1
            && let Some(t0) = self.ui.start_ms
        {
            let name = |e: usize| {
                let r = world.game.state.mem.read(0xBAE588 + 0xb * e as u32, 0xb);
                r[..r.iter().position(|&c| c == 0).unwrap_or(r.len())].to_vec()
            };
            let emp = |e: usize| hud.empire_names.get(e).cloned().unwrap_or_default();
            let (a, c) = (b.present.empires[0].0 as usize, b.present.empires[1].0 as usize);
            let mut t = emp(a);
            t.extend(b" (");
            t.extend(name(a));
            t.extend(b") vs. ");
            t.extend(emp(c));
            t.extend(b" (");
            t.extend(name(c));
            t.extend(b")");
            // `a2az` (Hungarian articles; the other languages' texts have no markers).
            let t = if hud.texts.lang == ig2_formats::text::Lang::Hu { crate::diplomacy::a2az(&t) } else { t };
            crate::spacewar_hud::banner(&mut frame, hud, &virt, &t, now.saturating_sub(t0), self.ui.fade);
        }
        // The tooltip (`GFX_DateMoneyScrollFade`'s fader pass 3) and the cursor (`UpdateFrame`): `MousePointerType` 0 → 1,
        // 2 → 5, 3 → 7, 4 → 6; 1 keeps the hot-spots' type 1.
        let tips = crate::spacewar_hud::tooltips(hud, &shown, self.ui.formation(b));
        self.tip.moved(mouse);
        let under = raw.map_or(0, |a| a.saturating_sub(0x1e));
        let len = tips.get(under as usize).map_or(0, |t| t.len());
        self.tip.update(now as f64, if len == 0 { 0 } else { under }, len, self.left || self.right);
        if let (Some(t), Some(font)) = (tips.get(self.tip.shown as usize), hud.font.as_ref()) {
            self.tip.draw(&mut frame, font, hud.white, &virt, &hud.palette, t);
        }
        let kind = match shown.pointer {
            0 | 1 => 1,
            2 => 5,
            3 => 7,
            4 => 6,
            _ => 1,
        };
        crate::spacewar_hud::cursor(&mut frame, &virt, gfx.cursor, kind, self.ticks100, mouse);
        let (units, guns) = crate::spacewar_screen::pick(gfx, assets, &shown, &v.camera);
        let plane = crate::spacewar_screen::plane_point(&shown, mouse, crate::spacewar_ui::PLANE_H * shown.view.scale);
        let cam = v.camera;
        let pole = |p: [f32; 2], y: f32| crate::spacewar_screen::pole_height(&shown, &cam, p, y);
        self.ui.mouse_frame(b, &units, &guns, plane, pole, shift, spot == Some(0x4c), &mut out);
        (Some(frame), out)
    }

    /// A left press at `at` (virtual pixels) on the top bar's speed buttons (`IG2_gameUpdate` exe 0x4e07d6, hot-spots
    /// 2..=5 at x 0, 15, 29, 50, y < 20) once the opening zoom is over: 0 = pause (the caller toggles it), 1..=3 a
    /// speed (the caller's `set_speed`).
    /// In MP the zoom doesn't hold them (`IG2_gameUpdate` tests `nemlehetidotallitani` in its SP branch only).
    pub fn bar_press(&self, at: [f32; 2], multiplayer: bool) -> Option<i32> {
        if !(multiplayer || self.clock_free()) || at[1] >= 20.0 || at[0] >= 73.0 {
            return None;
        }
        Some(match at[0] {
            x if x < 15.0 => 0,
            x if x < 29.0 => 1,
            x if x < 50.0 => 2,
            _ => 3,
        })
    }

    /// `nemlehetidotallitani` (exe 0xBAD0D4) clear: `SpaceWar_Belep` sets it and the opening zoom's end clears it (spec
    /// §2.5), so the pause and speed keys and buttons wait for the zoom (`DoKeyboard`, `IG2_gameUpdate`).
    pub fn clock_free(&self) -> bool {
        self.zoomed
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
            Out::Click | Out::Sound(_) | Out::Loop(_) => Vec::new(),
        }
    }
}

/// The screen's view after leaving: the starmap's own camera again.
pub fn restore_view(v: &mut crate::starmap::View) {
    v.camera.look = None;
    v.battle_fade = None;
    v.backdrop.clear();
}
