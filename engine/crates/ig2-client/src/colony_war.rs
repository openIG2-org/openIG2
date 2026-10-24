//! The colony screen in a real-time ground war (re/screens/colony.md §11): the war buttons of `Colony_MouseInit`
//! (exe 0x4680e0) and `Colony_Main` (exe 0x4666d0), unit selection (`GetNearestBuilding` exe 0x47b350, the box of
//! `WarfareKeretesKijeloles` exe 0x4a79e0, the groups of `ColonyKeyboardHandler`), the right button's orders, the
//! armed air strike, and the 2D/3D marks (`WF::WriteStuff`: rings, target markers, route crosses, group sizes).
//! Orders leave as `ig2_sim::game::ColonyOrder::Ground` (`MULTIGET_Colony`'s 0x22..=0x25, 0x12d, 0x12f, 0x130, 0x134).

use std::collections::{BTreeSet, HashMap};

use ig2_net::Command;
use ig2_sim::game::ColonyOrder;
use ig2_sim::ground::{flag, weapon, GroundMap, GroundOrder};

use crate::audio::speech::colony as speech;
use crate::colony::{ColonySnap, Mark, MarkAt, MarkTex, UnitView, View};
use crate::font::{Align, Font, Virtual};
use crate::render::{Blend, Frame, Instance, TexId};

/// The war buttons (`Colony_MouseInit`): hot-spot id, x (all 31 × 31 at y 443) and `ColonyBrushok` record.
pub const BUTTONS: [(u32, f32, usize); 7] = [(0xb, 344.0, 20), (0xc, 377.0, 21), (0xd, 245.0, 24), (0xe, 278.0, 25), (0xf, 212.0, 26), (0x10, 410.0, 27), (0x12, 311.0, 30)];

/// Shift, Ctrl and Alt as `KeyShift`/`KeyCtrl`/`KeyAlt` read them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// A ring's or target marker's zoom and pulse (`+0xec/+0xf0/+0xf4`, `+0xf8/+0xfc/+0x100`; `WF_Tank::Refresh2`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pulse {
    size: f32,
    alpha: i32,
    settled: bool,
}

impl Pulse {
    /// `SetSelection(true)`, `ForceSelectionAnim`: size 300, alpha 0, zooming in.
    const NEW: Self = Self { size: 300.0, alpha: 0, settled: false };

    /// One UI frame with phase `ph` and resting size `base` (40 for rings, 50 for targets).
    fn step(&mut self, ph: f64, base: f64) {
        let rest = (ph.sin() * 3.0 + base) as f32;
        if !self.settled {
            self.size *= 0.8;
            self.alpha = (((255.0 - (self.size - 45.0)) / 3.0) as i32).clamp(0, 255);
            if self.size <= rest {
                self.settled = true;
            }
        }
        if self.settled {
            self.size = rest;
            self.alpha = (-ph.sin() * 64.0 + 128.0) as i32;
        }
    }
}

/// A unit as the war input sees it this frame: its projected centre and the hit radius there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OnScreen {
    pub slot: usize,
    pub at: [f32; 2],
    /// `TransformTav(35.0, z)` = 35·600/z, at least 10 (`WF::MouseOver`); `None` when `WF_Tank::MouseOver` never
    /// hits it (flag 0x20, a defence turret of pods 0xa1..=0xa4 or without a gun).
    pub radius: Option<f32>,
}

/// What the screen needs to know beyond the snapshot.
pub struct WarCtx<'a> {
    pub snap: &'a ColonySnap,
    /// EN, the local player.
    pub en: i32,
    /// `TutorialPaused`.
    pub tutorial_paused: bool,
}

impl WarCtx<'_> {
    fn unit(&self, slot: usize) -> Option<&UnitView> {
        self.snap.units.iter().find(|u| u.slot == slot)
    }
    /// The planet's owner isn't EN (EN is the attacker or a bystander).
    fn foreign(&self) -> bool {
        self.snap.owner as i32 != self.en
    }
    /// `+0x8b1 + EN`: side EN has units on the field.
    fn present(&self) -> bool {
        self.snap.present.get(self.en as usize).copied().unwrap_or(0) != 0
    }
    /// `ColonyGfx2D_1`'s `local_22`: single player, or the MP war's planet with EN its owner or on the field.
    fn shows(&self) -> bool {
        !self.snap.multiplayer || (self.snap.mp_war_here && (!self.foreign() || self.present()))
    }
    /// `Colony_MouseInit2`'s tail: a bystander of the MP war on its planet gets no war buttons 0xb..=0x10.
    fn bystander(&self) -> bool {
        self.snap.multiplayer && self.snap.mp_war_here && self.foreign() && !self.present()
    }
    fn timers(&self) -> (i32, i32) {
        self.snap.timers.get((self.en - 1).max(0) as usize).copied().unwrap_or((0, 0))
    }
    fn retreating(&self) -> bool {
        self.snap.retreat.get((self.en - 1).max(0) as usize).copied().unwrap_or(0) != 0
    }
}

/// What a press or key asks of the window.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WarOutcome {
    pub commands: Vec<Command>,
    /// `PlaySoundGlobal(0)`.
    pub click: bool,
    /// The press was taken (a war button, or the view while the war runs).
    pub taken: bool,
    /// `AktEpulet = 0`: a unit took the selection.
    pub drop_building: bool,
    /// `ColonySpeaker.Play(line, kind)`: a unit's voice for the selection, a move or an attack (ground-combat.md
    /// §11.12).
    pub voice: Option<(i32, i32)>,
}

/// The war input state.
#[derive(Debug, Clone, Default)]
pub struct WarUi {
    /// The selected unit slots (flag 8).
    pub selected: BTreeSet<usize>,
    rings: HashMap<usize, Pulse>,
    targets: HashMap<i32, Pulse>,
    /// `WF::SaveSelection` groups 0..=5 (keys 4..9).
    groups: [BTreeSet<usize>; 6],
    /// `LastSelectedTank`: the owner of the last unit selected.
    last_owner: i32,
    /// `SpecWeaponMode`: 1 fighters armed, 2 big ships.
    pub armed: u8,
    /// `KeretesKijeloles`, `keretstartx/y`: the box's corner while the left button is held.
    pub box_start: Option<[f32; 2]>,
    /// `+0x146`: each unit's selection when the press came (Shift keeps it).
    kept: BTreeSet<usize>,
    /// The pointer's grid cell (`AktBGridX/Y`), the last one when it is off the ground.
    cell: (i32, i32),
    /// `ColonyCounter`.
    counter: f32,
    /// Hot-spot 0x12 (`Colony_MouseInit2`): off without an own selected unit, on while one moves (flag 0x100 or
    /// 0x400), else left as it was.
    stop_on: bool,
    /// `TamadasSzovegCounter` (exe 0x9ABAE0): the attack banner's frames left (900 from the war's start).
    pub banner: i32,
    /// The planet was at war last frame (the banner starts when the war does).
    was_war: bool,
    pub screen: Vec<OnScreen>,
    pub mods: Mods,
    /// The info model's unit (`GetSelected(0)`) and the UI frames it has turned since it was put up.
    info_turn: (Option<usize>, f32),
}

fn planet_of(c: &WarCtx) -> i32 {
    c.snap.planet as i32
}

fn order(c: &WarCtx, o: GroundOrder) -> Command {
    ColonyOrder::Ground { planet: planet_of(c), order: o }.into()
}

impl WarUi {
    /// The units' projected centres for this frame's input (`TransformPoint` of the object centre) [inf: the hull's
    /// base point stands for `GetObjCenter`'s mesh centre].
    pub fn project(&mut self, view: &View, gm: &GroundMap, snap: &ColonySnap) {
        let basis = view.camera.basis(gm);
        self.screen = snap
            .units
            .iter()
            .filter_map(|u| {
                let (at, z) = view.camera.project_virtual(&basis, u.pos)?;
                let turret = u.flags & flag::TURRET != 0;
                let blind = u.flags & 0x20 != 0 || (turret && ((0xa1..=0xa4).contains(&u.design) || !u.has_gun));
                Some(OnScreen { slot: u.slot, at, radius: (!blind).then(|| (35.0 * 600.0 / z).max(10.0)) })
            })
            .collect();
    }

    /// Selects `slots` as `SetSelection(true)` does (rings from size 300), e.g. from a save's flag 8.
    pub fn seed(&mut self, slots: impl IntoIterator<Item = usize>) {
        for s in slots {
            self.selected.insert(s);
            self.rings.insert(s, Pulse::NEW);
        }
    }

    /// The pointer's cell (`ShowColony`): `None` off the ground (`InvalidBuildingPosition`; the last cell stays).
    pub fn pointer(&mut self, cell: Option<(i32, i32)>) {
        if let Some(c) = cell {
            self.cell = c;
        }
    }

    /// `WF::CheckTanks` (exe 0x460b70): of the units under `p` (own ones, or all while side EN is on the field), the
    /// one whose centre is nearest.
    fn hit(&self, c: &WarCtx, p: [f32; 2]) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for s in &self.screen {
            let Some(r) = s.radius else { continue };
            let Some(u) = c.unit(s.slot) else { continue };
            if u.owner != c.en && !c.present() {
                continue;
            }
            let d = (p[0] - s.at[0]) * (p[0] - s.at[0]) + (p[1] - s.at[1]) * (p[1] - s.at[1]);
            if d < r * r && best.is_none_or(|(_, b)| d < b) {
                best = Some((s.slot, d));
            }
        }
        best.map(|(s, _)| s)
    }

    /// `WF::MouseOver` (exe 0x48bb00, `MelyikTankFolottVanACursor`): the unit under `p` whose centre is nearest, any
    /// side.
    fn under(&self, p: [f32; 2]) -> Option<usize> {
        let d = |s: &OnScreen| (p[0] - s.at[0]) * (p[0] - s.at[0]) + (p[1] - s.at[1]) * (p[1] - s.at[1]);
        self.screen.iter().filter(|s| s.radius.is_some_and(|r| d(s) < r * r)).min_by(|a, b| d(a).total_cmp(&d(b))).map(|s| s.slot)
    }

    /// `Colony_MouseInit2`'s pointer shape over the 3D view (hot-spot 0x14; spec §11.10), else `None` (the
    /// hot-spot's own): no unit selected: 5 over a unit or a building, 3 with the build cursor; units selected: 2 over
    /// the ground, over an own unit (or a building on EN's planet) 5, with Ctrl 6; over another's 6, with Alt 2; both
    /// a unit and a building under the pointer: left as it was (`None`).
    pub fn cursor(&self, c: &WarCtx, p: [f32; 2], building: bool, build_mode: bool) -> Option<i32> {
        let t = self.under(p);
        if self.selected.is_empty() {
            let mut k = None;
            if building || t.is_some() {
                k = Some(5);
            }
            if build_mode {
                k = Some(3);
            }
            return k;
        }
        let pick = |own: bool| match (own, self.mods.ctrl, self.mods.alt) {
            (true, false, _) => 5,
            (true, true, _) => 6,
            (false, _, false) => 6,
            (false, _, true) => 2,
        };
        match (t, building) {
            (None, false) => Some(2),
            (Some(t), false) => Some(pick(c.unit(t).is_some_and(|u| u.owner == c.en))),
            (None, true) => Some(pick(!c.foreign())),
            (Some(_), true) => None,
        }
    }

    /// `WF_Tank::SetSelection`: a new selection restarts its ring.
    fn select(&mut self, c: &WarCtx, slot: usize, on: bool) {
        let Some(u) = c.unit(slot) else { return };
        if u.flags & flag::NOT_COUNTED != 0 {
            return;
        }
        if on {
            if self.selected.insert(slot) {
                self.rings.insert(slot, Pulse::NEW);
            }
            self.last_owner = u.owner;
        } else {
            self.selected.remove(&slot);
        }
    }

    /// The selected units (`GetSelNum`'s count).
    fn chosen<'a>(&'a self, c: &'a WarCtx) -> impl Iterator<Item = &'a UnitView> + 'a {
        self.selected.iter().filter_map(|&s| c.unit(s))
    }

    /// `CheckWeaponSelected(EN, w)`.
    fn armed_with(&self, c: &WarCtx, w: u32) -> bool {
        self.chosen(c).any(|u| u.owner == c.en && u.weapons & w != 0)
    }

    /// `CheckSelectedMine`: 2·(a layer laying) + (one not laying).
    fn mines(&self, c: &WarCtx) -> u8 {
        let layers = || self.chosen(c).filter(|u| u.owner == c.en && u.weapons & weapon::AMMO != 0);
        2 * layers().any(|u| u.laying) as u8 + layers().any(|u| !u.laying) as u8
    }

    /// `CheckFighterAttack`/`CheckBigShipAttack`: owner ≠ EN and a placed unit of EN with the weapon and air units.
    fn call_in(&self, c: &WarCtx, w: u32) -> bool {
        c.foreign() && c.snap.units.iter().any(|u| u.owner == c.en && u.weapons & w != 0 && u.air > 0)
    }

    /// `Colony_MouseInit2`: the war button `id` is enabled.
    pub fn enabled(&self, c: &WarCtx, id: u32) -> bool {
        if (0xb..=0x10).contains(&id) && c.bystander() {
            return false;
        }
        match id {
            0xb => self.call_in(c, weapon::FIGHTER),
            0xc => self.call_in(c, weapon::BIG_SHIP),
            0xd => self.armed_with(c, weapon::KAMIKAZE),
            0xe => self.armed_with(c, weapon::BIG_KAMIKAZE),
            0xf => self.armed_with(c, weapon::AMMO),
            0x10 => c.foreign() && c.snap.war_flag,
            // `IsOwnTankSelected` and `AnyMovementInSelection` (flag 0x100 or 0x400).
            0x12 => self.stop_on,
            _ => false,
        }
    }

    /// The enabled war button under `p`.
    pub fn button(&self, c: &WarCtx, p: [f32; 2]) -> Option<u32> {
        BUTTONS.iter().find(|&&(id, x, _)| crate::kit::inside_f([x, 443.0, 31.0, 31.0], p) && self.enabled(c, id)).map(|&(id, _, _)| id)
    }

    /// A button pressed at `p`: the war buttons (once, on the press), then the 3D view.
    pub fn press(&mut self, c: &WarCtx, p: [f32; 2], left: bool, in_view: bool) -> WarOutcome {
        let mut out = WarOutcome::default();
        if let Some(id) = self.button(c, p) {
            out.taken = true;
            if left {
                self.button_press(c, id, &mut out);
            }
            return out;
        }
        if !in_view || self.armed != 0 {
            return out;
        }
        if self.landing(c, left, &mut out) {
            return out;
        }
        if !c.snap.war_flag {
            return out;
        }
        out.taken = true;
        // The right button with units selected gives orders (the press frame only); otherwise either button selects
        // (`GetNearestBuilding`: the order block needs `TankSelected`).
        if !left && !self.selected.is_empty() {
            self.right_press(c, p, &mut out);
            return out;
        }
        {
            if left {
                self.box_start = Some(p);
            }
            // `GetNearestBuilding`'s unit half.
            self.kept = self.selected.clone();
            let t = self.hit(c, p);
            if !self.mods.shift {
                self.selected.retain(|&s| Some(s) == t);
            }
            match t {
                Some(t) => {
                    let own = c.unit(t).is_some_and(|u| u.owner == c.en);
                    if !self.mods.shift || (own && (self.last_owner == c.en || self.selected.is_empty())) {
                        self.select(c, t, true);
                    }
                    out.drop_building = true;
                }
                // Nothing under the pointer: the building rule (`colony_ui`) takes the press.
                None => out.taken = false,
            }
        }
        out
    }

    /// The unit info's model (`ColonyInfoMod` 1): the first selected unit and its turn in UI frames.
    pub fn info_model<'a>(&self, c: &'a WarCtx) -> Option<(&'a UnitView, f32)> {
        let u = c.unit(*self.selected.first()?)?;
        Some((u, if self.info_turn.0 == Some(u.slot) { self.info_turn.1 } else { 0.0 }))
    }

    /// `CheckPlacing(EN)` false: a unit of EN still waits on board (flag 0x8000000).
    pub fn placing(c: &WarCtx) -> bool {
        c.snap.to_land.iter().any(|&(o, _)| o == c.en)
    }

    /// `GetNearestBuilding`'s landing clicks (spec §11.7): the left button on a foreign planet lands EN's next unit on
    /// the cell (`PlaceTank`) or picks up EN's unit standing there (`UnplaceTank`; another side's unit there lets the
    /// press through to the selection); the right button lands them all near it (`PlaceAllTanks(…, false)`). The exe
    /// then waits for the release (`Mouse_FelengedesMajdMegvar`): one order per click.
    fn landing(&mut self, c: &WarCtx, left: bool, out: &mut WarOutcome) -> bool {
        if !Self::placing(c) {
            return false;
        }
        let (x, y) = self.cell;
        let o = if left {
            if !c.foreign() {
                return false;
            }
            // `WF::UsedPosition` (exe 0x4a7950): the first unit off board on the cell decides.
            match c.snap.placed.iter().find(|&&(_, _, at)| at == (x, y)) {
                None => GroundOrder::Place { x, y },
                Some(&(slot, owner, _)) if owner == c.en => GroundOrder::Unplace { unit: slot as i32 },
                Some(_) => return false,
            }
        } else {
            GroundOrder::PlaceAll { x, y }
        };
        out.commands.push(order(c, o));
        out.taken = true;
        // `GetNearestBuilding` returned 0: `Colony_Main` drops `AktEpulet` while side EN is on the field.
        out.drop_building = c.foreign() && c.present();
        if left {
            self.box_start = None;
        }
        true
    }

    fn button_press(&mut self, c: &WarCtx, id: u32, out: &mut WarOutcome) {
        let (fighters, ships) = c.timers();
        match id {
            0xb if fighters == 200 => self.armed = if self.armed == 1 { 0 } else { 1 },
            0xc if ships == 300 => self.armed = if self.armed == 2 { 0 } else { 2 },
            0xb | 0xc => return,
            // `DetonateSelected(EN, big)`: every selected unit of EN with the pod.
            0xd | 0xe => {
                let (w, big) = if id == 0xd { (weapon::KAMIKAZE, 0) } else { (weapon::BIG_KAMIKAZE, 1) };
                for u in self.chosen(c).filter(|u| u.owner == c.en && u.weapons & w != 0) {
                    out.commands.push(order(c, GroundOrder::Detonate { unit: u.slot as i32, big }));
                }
            }
            // `SwitchSelectedMineLayers(EN, on)`.
            0xf => {
                // 1 (none laying) → on; 2, 3 → off.
                let on = self.mines(c) == 1;
                for u in self.chosen(c).filter(|u| u.owner == c.en && u.weapons & weapon::AMMO != 0) {
                    out.commands.push(order(c, GroundOrder::Mines { unit: u.slot as i32, on }));
                }
            }
            // Only once every unit has landed and its dropship is gone (`+0x278` bit 0).
            0x10 => {
                if c.tutorial_paused || !c.snap.all_landed {
                    return;
                }
                out.commands.push(order(c, GroundOrder::Retreat));
            }
            0x12 => self.stop(c, out),
            _ => return,
        }
        out.click = true;
    }

    /// `StopSelected`: 0x25 per selected unit of EN with flags & 6 = 0.
    fn stop(&self, c: &WarCtx, out: &mut WarOutcome) {
        for u in self.chosen(c).filter(|u| u.owner == c.en && u.flags & 6 == 0) {
            out.commands.push(order(c, GroundOrder::Stop { unit: u.slot as i32 }));
        }
    }

    /// The right button in the view (`Colony_Main`, the press frame): attack the unit or building under the pointer,
    /// queue a waypoint (Shift) or drive there.
    fn right_press(&mut self, c: &WarCtx, p: [f32; 2], out: &mut WarOutcome) {
        let Mods { shift, ctrl, alt } = self.mods;
        let t = self.hit(c, p);
        let building = c.snap.buildings.iter().find(|b| {
            // [deviation, as §10: the building whose footprint holds the cell, not the model hit]
            let (x, y) = (b.x as i32, b.y as i32);
            (self.cell.0 - x).abs() <= 1 && (self.cell.1 - y).abs() <= 1
        });
        let attack = |target: i32, out: &mut WarOutcome| {
            // `AttackWithSelected` (exe 0x45f520): `GetSelected(0)`'s voice first, whoever's it is, unless a turret.
            out.voice = self.selected.first().and_then(|&s| c.unit(s)).filter(|u| u.flags & flag::TURRET == 0).map(|u| (u.slot as i32 % 3 + 1, speech::ATTACK));
            for u in self.chosen(c).filter(|u| u.owner == c.en && u.flags & 6 == 0) {
                out.commands.push(order(c, GroundOrder::Target { unit: u.slot as i32, target }));
            }
        };
        // `MoveSelectedG` to the last cell under the pointer (also when the ray missed the ground), not with Ctrl or
        // while `TutorialPaused`.
        let move_to = |out: &mut WarOutcome| {
            if !ctrl && !c.tutorial_paused {
                // `MoveSelectedG` (inlined at exe 0x48be70): the voice of the last unit sent that isn't a turret.
                let mut last = None;
                for u in self.chosen(c).filter(|u| u.owner == c.en && u.flags & 4 == 0) {
                    out.commands.push(order(c, GroundOrder::Move { unit: u.slot as i32, x: self.cell.0, y: self.cell.1 }));
                    if u.flags & flag::TURRET == 0 {
                        last = Some(u.slot as i32);
                    }
                }
                out.voice = last.map(|s| (s % 3 + 1, speech::MOVE));
            }
        };
        // exe 0x47b350: a foreign unit (an own one with Ctrl) is attacked unless Alt; else a building (foreign planet,
        // or Ctrl) is attacked unless Alt, which moves instead; else Shift queues a waypoint; else the move.
        let hostile = t.filter(|&t| c.unit(t).is_some_and(|u| u.owner != c.en) || ctrl);
        match (hostile, building.filter(|_| c.foreign() || ctrl)) {
            (Some(t), _) if !alt => attack(t as i32, out),
            (Some(_), _) => move_to(out),
            (None, Some(b)) if !alt => attack(-(b.slot as i32), out),
            (None, Some(_)) => move_to(out),
            (None, None) if shift => {
                // `AddWayPointSelectedGrid`: every selected unit (sent as the unit's owner in the exe; the handler
                // doesn't look at the sender).
                for u in self.chosen(c) {
                    out.commands.push(order(c, GroundOrder::Waypoint { unit: u.slot as i32, x: self.cell.0, y: self.cell.1 }));
                }
            }
            (None, None) => move_to(out),
        }
    }

    /// The pointer moved to `p` with the left button held: the box (`WarfareKeretesKijeloles`).
    pub fn drag(&mut self, c: &WarCtx, p: [f32; 2]) -> bool {
        let Some(s) = self.box_start else { return false };
        if (p[0] - s[0]).abs() + (p[1] - s[1]).abs() <= 15.0 {
            return false;
        }
        let first = self.hit(c, s);
        let (x0, x1) = (s[0].min(p[0]), s[0].max(p[0]));
        let (y0, y1) = (s[1].min(p[1]), s[1].max(p[1]));
        let inside: Vec<(usize, bool)> = self.screen.iter().map(|o| (o.slot, o.at[0] >= x0 && o.at[0] <= x1 && o.at[1] >= y0 && o.at[1] <= y1)).collect();
        for (slot, on) in inside {
            if self.selected.contains(&slot) != on {
                self.select(c, slot, on);
            }
        }
        if let Some(f) = first {
            self.select(c, f, true);
        }
        if self.selected.len() != 1 {
            let foreign: Vec<usize> = self.chosen(c).filter(|u| u.owner != c.en).map(|u| u.slot).collect();
            for s in foreign {
                self.selected.remove(&s);
            }
        }
        if self.mods.shift {
            let kept: Vec<usize> = self.kept.iter().copied().collect();
            for s in kept {
                self.select(c, s, true);
            }
        }
        true
    }

    /// A button released: the box ends; an armed air strike goes to the pointer's cell (0x130).
    /// `Colony_Main` fires on the release of either button (`Mouse_Felenged`).
    pub fn release(&mut self, c: &WarCtx, left: bool, in_view: bool) -> WarOutcome {
        let mut out = WarOutcome::default();
        if left {
            self.box_start = None;
        }
        if self.armed != 0 && in_view {
            out.commands.push(order(c, GroundOrder::AirStrike { x: self.cell.0, y: self.cell.1, kind: self.armed as i32 - 1 }));
            self.armed = 0;
            out.taken = true;
        } else if left && in_view {
            // `Colony_Main` (ELF 0x4dc4c7): the left release on the view with no strike armed.
            out.voice = self.selection_voice(c);
        }
        out
    }

    /// `WF::GetSelected(0)` when it is an own unit that isn't a mine: `ColonySpeaker`'s line `sel % 3 + 1`.
    fn selection_voice(&self, c: &WarCtx) -> Option<(i32, i32)> {
        let u = c.unit(*self.selected.first()?)?;
        (u.owner == c.en && u.flags & flag::MINE == 0).then_some((u.slot as i32 % 3 + 1, speech::SELECTION))
    }

    /// The war's keys (`ColonyKeyboardHandler`): Ctrl+4..9 keep a group, 4..9 call it back, S stops, `a` selects
    /// every own unit.
    pub fn key(&mut self, c: &WarCtx, k: WarKey) -> WarOutcome {
        let mut out = WarOutcome::default();
        match k {
            WarKey::Group(n) if n < 6 => {
                if self.mods.ctrl {
                    self.groups[n] = self.selected.clone();
                    for s in &self.selected {
                        self.rings.insert(*s, Pulse::NEW);
                    }
                } else {
                    let alive: BTreeSet<usize> = c.snap.units.iter().map(|u| u.slot).collect();
                    self.selected = self.groups[n].intersection(&alive).copied().collect();
                    for s in &self.selected {
                        self.rings.entry(*s).or_insert(Pulse::NEW);
                    }
                }
            }
            WarKey::Stop => self.stop(c, &mut out),
            WarKey::All => {
                out.drop_building = true;
                let units: Vec<(usize, bool)> = c
                    .snap
                    .units
                    .iter()
                    .filter(|u| u.flags & flag::NOT_COUNTED == 0)
                    .map(|u| (u.slot, u.owner == c.en && u.flags & flag::TURRET == 0))
                    .collect();
                for (s, on) in units {
                    self.select(c, s, on);
                }
                // `ColonyKeyboardHandler` (ELF 0x4ddb61).
                out.voice = self.selection_voice(c);
            }
            _ => return out,
        }
        out.taken = true;
        out
    }

    /// One UI frame (`Frissit2_Colony`, `WF_Tank::Refresh2`): the selection keeps alive units only; the rings and
    /// target markers pulse.
    pub fn ui_frame(&mut self, c: &WarCtx) {
        self.counter += 1.0;
        // `StartGrWar` sets the counter to 900, `Frissit2_Colony` counts it down on the colony screen, `Colony_Kilep`
        // zeroes it [deviation: the port starts it when the screen first sees the war, colony.md §11.4].
        if c.snap.war_flag && !self.was_war {
            self.banner = 900;
        }
        self.was_war = c.snap.war_flag;
        if !self.chosen(c).any(|u| u.owner == c.en) {
            self.stop_on = false;
        } else if self.chosen(c).any(|u| u.owner == c.en && u.flags & 0x500 != 0) {
            self.stop_on = true;
        }
        self.banner = (self.banner - 1).max(0);
        self.selected.retain(|&s| c.unit(s).is_some());
        // `HandleKisInfoObject` puts the model up again when the first selected unit changes.
        let first = self.selected.first().copied();
        self.info_turn = if first == self.info_turn.0 { (first, self.info_turn.1 + 1.0) } else { (first, 0.0) };
        if !c.snap.war_flag {
            self.armed = 0;
        }
        let ph0 = self.counter as f64 / 30.0;
        let wanted: BTreeSet<i32> = self.chosen(c).filter(|u| u.flags & flag::HAS_TARGET != 0 && u.target != 10000).map(|u| u.target).collect();
        let selected = &self.selected;
        self.rings.retain(|s, _| selected.contains(s));
        for (&s, p) in self.rings.iter_mut() {
            p.step(ph0 + s as f64, 40.0);
        }
        self.targets.retain(|t, _| wanted.contains(t));
        for t in wanted {
            self.targets.entry(t).or_insert(Pulse::NEW).step(ph0 - t as f64, 50.0);
        }
    }

    /// The 3D pass's rings and target markers (`DrawSelection`, `DrawTarget`).
    pub fn marks(&self, c: &WarCtx) -> Vec<Mark> {
        let mut out = Vec::new();
        for u in self.chosen(c).filter(|u| u.flags & 0x20 == 0) {
            let Some(p) = self.rings.get(&u.slot) else { continue };
            let turret = u.flags & flag::TURRET != 0;
            let tex = if u.owner == c.en { MarkTex::Friend } else { MarkTex::Enemy };
            out.push(Mark { tex, at: MarkAt::Unit { slot: u.slot, half: p.size, lift: if turret { 0.0 } else { 5.0 } }, alpha: p.alpha });
        }
        for (&t, p) in &self.targets {
            let at = if t < 0 { MarkAt::Building { slot: (-t) as usize, grow: (p.size - 45.0) * 2.0 } } else { MarkAt::Unit { slot: t as usize, half: p.size, lift: 5.0 } };
            let tex = if t < 0 { MarkTex::Building } else { MarkTex::Arrow };
            out.push(Mark { tex, at, alpha: p.alpha });
        }
        out
    }

    /// `ColonyKisInfo` with `ColonyInfoMod` 1 (units selected; `StarmapRovidInfoKiir(50, 424)`'s place): several units:
    /// the count line, the first one's empire, its weapon; one unit: its design's name, empire and hit points. Returns
    /// false with nothing selected.
    pub fn draw_unit_info(&self, f: &mut Frame, c: &WarCtx, d: &Draw2d) -> bool {
        let units: Vec<&UnitView> = self.chosen(c).collect();
        let Some(first) = units.first() else { return false };
        let col = |i: usize| d.palette.get(i).copied().unwrap_or([1.0; 3]);
        let (x, y) = (50.0, 424.0);
        let empire = d.empire_names.get(first.owner.clamp(0, 18) as usize).cloned().unwrap_or_default();
        let owner_col = col((first.owner + 0xf).max(0) as usize);
        if units.len() > 1 {
            let line = crate::ui::sprintf(&d.line(33), &[units.len() as i64]);
            d.font.text(f, d.v, &line, x, y + 6.0, 19, 1, Align::Left, col(0x23), None);
            d.font.text(f, d.v, &empire, x, y + 15.0, 19, 1, Align::Left, owner_col, None);
            let weapon = (d.weapon_name)(first);
            d.font.text(f, d.v, &weapon, x, y + 42.0, 20, 1, Align::Left, col(0x23), None);
        } else {
            d.font.text(f, d.v, &(d.design_name)(first), x, y + 3.0, 19, 2, Align::Left, owner_col, None);
            d.font.text(f, d.v, &empire, x, y + 15.0, 19, 1, Align::Left, owner_col, None);
            let weapon = (d.weapon_name)(first);
            d.font.text(f, d.v, &weapon, x, y + 42.0, 20, 1, Align::Left, col(0x23), None);
        }
        true
    }

    /// The war's 2D parts: the buttons and charge bars (`ColonyGfx2D_1`), the route crosses and group sizes
    /// (`WF::WriteStuff`), the box, the retreat/landing lines.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, c: &WarCtx, d: &Draw2d, view: &View, gm: &GroundMap, p: [f32; 2]) {
        let col = |i: usize| d.palette.get(i).copied().unwrap_or([1.0; 3]);
        let v = d.v;
        let fill = |f: &mut Frame, x: f32, y: f32, w: f32, h: f32, k: usize| {
            if w > 0.0 {
                let c = col(k);
                f.push(d.white, Blend::GammaAlpha, Instance::new(crate::render::kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
            }
        };
        // `ColonyGfx2D_2`: the attack banner while `TamadasSzovegCounter` runs, its brightness `min(100, 2·counter)` %
        // (`KiirStrSzinSzorzo`): EN's planet: "The %s attacked your planet %s!" (3, 2, 5) with the attacker and the
        // planet; else "You attacked the planet %s of the %s!" (3, 2, 6) with the planet and its owner (the other
        // way round in language 4) [MP: "%s (%s) vs. %s (%s)" with the players' names, not ported].
        if self.banner > 0 {
            let k = (2 * self.banner).min(100) as f32 / 100.0;
            let owner = d.empire_names.get(c.snap.owner as usize).cloned().unwrap_or_default();
            let text = if c.foreign() {
                Some(sprintf_s(&d.line(6), &[&c.snap.planet_name, &owner]))
            } else {
                (1..0x10).contains(&c.snap.attacker).then(|| sprintf_s(&d.line(5), &[&d.empire_names[c.snap.attacker as usize], &c.snap.planet_name]))
            };
            if let Some(t) = text {
                let rgb = col(2).map(|x| x * k);
                d.font.text(f, v, &t, 200.0, 30.0, 70, 2, Align::Left, rgb, None);
            }
        }
        if !c.snap.war_flag && self.selected.is_empty() {
            return;
        }
        // Route crosses: `line3d(0x80be8882)` around each waiting waypoint, 10 above the ground.
        let basis = view.camera.basis(gm);
        let gvalue = self.counter as f64;
        let half = ((gvalue / 10.0).sin() * 3.0 + 6.0) as f32;
        let line_col = [0xbe as f32 / 255.0, 0x88 as f32 / 255.0, 0x82 as f32 / 255.0, 0x80 as f32 / 255.0];
        for u in self.chosen(c).filter(|u| u.owner == c.en) {
            for &(gx, gy) in &u.route {
                let (x, y) = crate::colony::grid_to_real(gx as f32, gy as f32);
                let h = crate::colony::height(gm, x, y) - 10.0;
                let ends = [([h, x - half, y], [h, x + half, y]), ([h, x, y - half], [h, x, y + half])];
                for (a, b) in ends {
                    if let (Some(a), Some(b)) = (view.camera.project(&basis, a), view.camera.project(&basis, b)) {
                        f.push(d.white, Blend::Alpha, Instance::line(a, b, v.k.max(1.0), line_col));
                    }
                }
            }
        }
        // Group sizes at the projected centre − (8, 8) (`WF_Tank::WriteStuff`).
        for s in &self.screen {
            let Some(u) = c.unit(s.slot) else { continue };
            if u.members > 1 {
                d.font.int(f, v, u.members as i64, s.at[0] - 8.0, s.at[1] - 8.0, 4, 2, Align::Left, col(2));
            }
        }
        // The box: four 1-pixel lines of (250, 80, 153) at t 128 (`FadeKeret`).
        if let Some(s) = self.box_start.filter(|s| (p[0] - s[0]).abs() + (p[1] - s[1]).abs() > 15.0) {
            let rgb = [250.0 / 255.0, 80.0 / 255.0, 153.0 / 255.0, 128.0 / 255.0];
            for (a, b) in [([s[0], s[1]], [p[0], s[1]]), ([s[0], p[1]], [p[0], p[1]]), ([s[0], s[1]], [s[0], p[1]]), ([p[0], s[1]], [p[0], p[1]])] {
                f.push(d.white, Blend::Alpha, Instance::line(v.px(a[0], a[1]), v.px(b[0], b[1]), v.k.max(1.0), rgb));
            }
        }
        if !c.snap.war_flag {
            return;
        }
        // The buttons, drawn while enabled; the charge bars under 0xb and 0xc.
        let full = [1.0; 4];
        let (fighters, ships) = c.timers();
        for &(id, x, brush) in &BUTTONS {
            // Brushes 21, 24..=27 only with `local_22`; 20 (0xb) and 30 (0x12) without.
            if !self.enabled(c, id) || ((0xc..=0x10).contains(&id) && !c.shows()) {
                continue;
            }
            let frame = match id {
                0xb => (self.armed == 1) as usize + 1,
                0xc => (self.armed == 2) as usize + 1,
                0xf => self.mines(c).max(1) as usize,
                0x10 => c.retreating() as usize + 1,
                _ => 1,
            };
            if let Some(b) = d.brushes.get(brush) {
                d.pics.brush(f, v, b, frame as i32, None, full);
            }
            if id == 0xb || id == 0xc {
                let t = if id == 0xb { fighters } else { ships };
                fill(f, x, 427.0, 31.0, 5.0, 6);
                fill(f, x, 427.0, (t as f64 * 31.0 / 200.0) as i32 as f32, 5.0, 7);
            }
        }
        // `ColonyGfx2D_1`: the retreat and landing lines (owner ≠ EN, `local_22`).
        if c.foreign() && c.shows() {
            if c.retreating() {
                d.font.text(f, v, &d.line(3), 220.0, 40.0, 70, 2, Align::Left, col(2), None);
            }
            if c.snap.to_land.iter().any(|&(o, _)| o == c.en) {
                d.font.text(f, v, &d.line(4), 220.0, 50.0, 70, 2, Align::Left, col(2), None);
            }
        }
    }
}

/// The war's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarKey {
    /// 4..9 → 0..=5.
    Group(usize),
    Stop,
    All,
}

/// `CheckSelfDestructBuilding(p)` (exe 0x460b20) on EN's planet: a working `'+'` building (`+0x19` bit 0): hot-spot
/// 10 and its brush (spec §11.8).
pub fn self_destruct_ready(c: &WarCtx) -> bool {
    !c.foreign() && c.snap.buildings.iter().any(|b| b.kind == b'+' && b.flags & 1 != 0)
}

/// The self-destruct's parts (spec §11.8): button 10's brush 19 (`ColonyGfx2D_1`), and while the countdown runs the
/// window (`ShowSelfDestruct` exe 0x4a3640) and its texts (`WriteSelfDestruct` exe 0x4a82f0): the cancel button and
/// its label only on EN's planet.
pub fn draw_self_destruct(f: &mut Frame, c: &WarCtx, d: &Draw2d) {
    let col = |i: usize| d.palette.get(i).copied().unwrap_or([1.0; 3]);
    if self_destruct_ready(c)
        && c.shows()
        && let Some(b) = d.brushes.get(19)
    {
        d.pics.brush(f, d.v, b, 1, None, [1.0; 4]);
    }
    let n = c.snap.self_destruct;
    if n == -1 {
        return;
    }
    let own = !c.foreign();
    crate::kit::fader::fader(f, d.white, d.v, [380.0, 70.0, 180.0, 100.0], [0.0, 20.0, 30.0], 60.0, [0.0, 15.0, 25.0], 30.0);
    if own {
        crate::kit::fader::fader(f, d.white, d.v, [440.0, 135.0, 60.0, 25.0], [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
    }
    let line = |k: u16| d.texts.get(3, 5, k);
    d.font.text(f, d.v, &line(1), 415.0, 78.0, 18, 2, Align::Field, col(2), None);
    d.font.text(f, d.v, &line(2), 400.0, 90.0, 18, 3, Align::Field, col(2), None);
    d.font.int(f, d.v, (n / 20) as i64, 450.0, 112.0, 3, 4, Align::Field, col(6));
    if own {
        d.font.text(f, d.v, &line(3), 452.0, 143.0, 6, 2, Align::Field, col(2), None);
    }
}

/// The 2D pass's tools.
pub struct Draw2d<'a> {
    pub font: &'a Font,
    pub pics: &'a crate::hud::Pictures,
    pub brushes: &'a [ig2_rules::Brush],
    pub white: TexId,
    pub v: &'a Virtual,
    pub palette: &'a [[f32; 3]],
    pub texts: &'a crate::ui::UiTexts,
    pub empire_names: &'a [Vec<u8>],
    /// `Tervek[side] + 0x84·design`'s name; the weapon's tech name (`GiveOneTankWeaponName`).
    pub design_name: &'a dyn Fn(&UnitView) -> Vec<u8>,
    pub weapon_name: &'a dyn Fn(&UnitView) -> Vec<u8>,
}

/// `sprintf` with `%s` arguments in order.
fn sprintf_s(fmt: &[u8], args: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut i, mut k) = (0, 0);
    while i < fmt.len() {
        if fmt[i] == b'%' && fmt.get(i + 1) == Some(&b's') {
            out.extend_from_slice(args.get(k).copied().unwrap_or(&[]));
            k += 1;
            i += 2;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    out
}

impl Draw2d<'_> {
    /// `GetString(3, 2, n)`.
    fn line(&self, n: u16) -> Vec<u8> {
        self.texts.get(3, 2, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_zoom_in_then_pulse() {
        let mut p = Pulse::NEW;
        let mut frames = 0;
        while !p.settled {
            p.step(0.0, 40.0);
            frames += 1;
        }
        // 300·0.8^9 = 40.27 > 40, 300·0.8^10 = 32.2: settled on the tenth frame at the resting size.
        assert_eq!(frames, 10);
        assert_eq!(p.size, 40.0);
        assert_eq!(p.alpha, 128);
    }

    fn unit(slot: usize, owner: i32, weapons: u32) -> UnitView {
        UnitView {
            slot,
            owner,
            hull: 0,
            flags: flag::MESH,
            has_gun: true,
            design: 1,
            side: owner,
            pos: [0.0; 3],
            body: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            turret: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            turret_angle: 0.0,
            angle: 0.0,
            gun: None,
            weapons,
            air: if weapons & weapon::FIGHTER != 0 { 3 } else { 0 },
            laying: false,
            transport: 1,
            target: 10000,
            route: Vec::new(),
            members: 1,
            ride: None,
        }
    }

    /// Planet 7 of empire 2 at war; EN = 1 attacks with units 0 (fighters), 1 (mine layer), 2 (kamikaze); unit 3 is 2's.
    fn field() -> ColonySnap {
        let mut present = [0; 19];
        present[1] = 1;
        present[2] = 1;
        let mut timers = [(0, 0); 18];
        timers[0] = (200, 120);
        ColonySnap {
            planet: 7,
            tick: 0,
            owner: 2,
            surface: 1,
            map: 0,
            buildings: Vec::new(),
            units: vec![unit(0, 1, weapon::FIGHTER), unit(1, 1, weapon::AMMO), unit(2, 1, weapon::KAMIKAZE), unit(3, 2, 0)],
            shots: Vec::new(),
            camera: crate::colony::SavedCamera::NEW_GAME,
            weather: 0,
            background: 0,
            at_war: true,
            timers,
            retreat: [0; 18],
            present,
            war_flag: true,
            multiplayer: false,
            mp_war_here: false,
            to_land: Vec::new(),
            attacker: 0,
            planet_name: b"Sol 10".to_vec(),
            all_landed: true,
            placed: (0..4).map(|i| (i, if i < 3 { 1 } else { 2 }, (10 + i as i32, 20))).collect(),
            self_destruct: -1,
            visible: true,
        }
    }

    fn ui() -> WarUi {
        // Units 0..=3 side by side on the screen, 40 pixels apart, radius 12.
        let screen = (0..4).map(|i| OnScreen { slot: i, at: [100.0 + 40.0 * i as f32, 200.0], radius: Some(12.0) }).collect();
        let mut w = WarUi { screen, ..WarUi::default() };
        w.pointer(Some((50, 60)));
        w
    }

    fn orders(out: &WarOutcome) -> Vec<GroundOrder> {
        out.commands
            .iter()
            .filter_map(|c| match c {
                Command::Colony { kind, values } => match ColonyOrder::from_message(*kind, values) {
                    Some(ColonyOrder::Ground { planet: 7, order }) => Some(order),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    #[test]
    fn clicks_and_the_box_select() {
        let s = field();
        let c = WarCtx { snap: &s, en: 1, tutorial_paused: false };
        let mut w = ui();
        // A click on unit 1 selects it alone; Shift + click adds unit 2; a click on the enemy (side 1 is on the
        // field, so it can be picked) selects it alone.
        assert!(w.press(&c, [141.0, 202.0], true, true).taken);
        assert_eq!(w.selected, BTreeSet::from([1]));
        w.release(&c, true, true);
        w.mods.shift = true;
        w.press(&c, [180.0, 200.0], true, true);
        assert_eq!(w.selected, BTreeSet::from([1, 2]));
        w.mods.shift = false;
        w.press(&c, [220.0, 200.0], true, true);
        assert_eq!(w.selected, BTreeSet::from([3]));
        // A box over all four: the enemy goes again (more than one selected).
        w.press(&c, [60.0, 150.0], true, true);
        assert!(w.drag(&c, [260.0, 250.0]));
        assert_eq!(w.selected, BTreeSet::from([0, 1, 2]));
        // A box under 16 pixels isn't one.
        w.release(&c, true, true);
        w.press(&c, [60.0, 150.0], true, true);
        assert!(!w.drag(&c, [70.0, 155.0]));
    }

    #[test]
    fn the_right_button_and_the_buttons_send_the_orders() {
        let s = field();
        let c = WarCtx { snap: &s, en: 1, tutorial_paused: false };
        let mut w = ui();
        w.key(&c, WarKey::All);
        assert_eq!(w.selected, BTreeSet::from([0, 1, 2]));
        // Right on the enemy: 0x24 for each own selected unit; on the ground: 0x22; with Shift: 0x23.
        // The voices (ground-combat.md §11.12): the attack `GetSelected(0)`'s (unit 0: line 1, kind 2), the move the last
        // unit sent's (unit 2: line 3, kind 0; ig2.exe, quick-1.sav planet 17: 0x22 last for unit 12 → line 1).
        let out = w.press(&c, [220.0, 200.0], false, true);
        assert_eq!(out.voice, Some((1, 2)));
        assert_eq!(orders(&out), (0..3).map(|u| GroundOrder::Target { unit: u, target: 3 }).collect::<Vec<_>>());
        let out = w.press(&c, [400.0, 300.0], false, true);
        assert_eq!(out.voice, Some((3, 0)));
        assert_eq!(orders(&out), (0..3).map(|u| GroundOrder::Move { unit: u, x: 50, y: 60 }).collect::<Vec<_>>());
        w.mods.shift = true;
        let o = orders(&w.press(&c, [400.0, 300.0], false, true));
        assert_eq!(o[0], GroundOrder::Waypoint { unit: 0, x: 50, y: 60 });
        w.mods.shift = false;
        // Off the ground: the move goes to the last cell; with Ctrl none; Alt on the enemy moves instead.
        w.pointer(None);
        assert_eq!(orders(&w.press(&c, [400.0, 300.0], false, true))[0], GroundOrder::Move { unit: 0, x: 50, y: 60 });
        w.mods.ctrl = true;
        assert!(orders(&w.press(&c, [400.0, 300.0], false, true)).is_empty());
        w.mods = Mods { alt: true, ..Mods::default() };
        assert_eq!(orders(&w.press(&c, [220.0, 200.0], false, true))[0], GroundOrder::Move { unit: 0, x: 50, y: 60 });
        w.mods = Mods::default();
        // The buttons: detonate (unit 2), mines on (unit 1), the air strike armed and fired at the last cell; the big
        // ships' timer isn't full.
        assert!(w.enabled(&c, 0xd) && !w.enabled(&c, 0xe) && w.enabled(&c, 0xf) && w.enabled(&c, 0xb) && !w.enabled(&c, 0xc));
        assert_eq!(orders(&w.press(&c, [250.0, 450.0], true, false)), [GroundOrder::Detonate { unit: 2, big: 0 }]);
        assert_eq!(orders(&w.press(&c, [220.0, 450.0], true, false)), [GroundOrder::Mines { unit: 1, on: true }]);
        assert!(w.press(&c, [350.0, 450.0], true, false).click);
        assert_eq!(w.armed, 1);
        assert_eq!(orders(&w.release(&c, true, true)), [GroundOrder::AirStrike { x: 50, y: 60, kind: 0 }]);
        assert_eq!(w.armed, 0);
        // Retreat (0x12f), and groups: Ctrl+4 keeps {0, 1, 2}, `a`… then 4 calls it back.
        assert_eq!(orders(&w.press(&c, [415.0, 450.0], true, false)), [GroundOrder::Retreat]);
        w.mods.ctrl = true;
        w.key(&c, WarKey::Group(0));
        w.mods.ctrl = false;
        w.press(&c, [220.0, 200.0], true, true);
        assert_eq!(w.selected, BTreeSet::from([3]));
        w.key(&c, WarKey::Group(0));
        assert_eq!(w.selected, BTreeSet::from([0, 1, 2]));
    }

    #[test]
    fn pointer_shapes_over_the_view() {
        let s = field();
        let c = WarCtx { snap: &s, en: 1, tutorial_paused: false };
        let mut w = ui();
        // Nothing selected: 5 over a unit, none over the ground, 3 with the build cursor.
        assert_eq!(w.cursor(&c, [100.0, 200.0], false, false), Some(5));
        assert_eq!(w.cursor(&c, [400.0, 300.0], false, false), None);
        assert_eq!(w.cursor(&c, [100.0, 200.0], false, true), Some(3));
        // Units selected: 2 over the ground; own unit 5 (Ctrl 6); side 2's unit 6 (Alt 2); a building of the foreign
        // planet 6; a unit and a building at once: left alone.
        w.seed([0]);
        assert_eq!(w.cursor(&c, [400.0, 300.0], false, false), Some(2));
        assert_eq!(w.cursor(&c, [140.0, 200.0], false, false), Some(5));
        w.mods.ctrl = true;
        assert_eq!(w.cursor(&c, [140.0, 200.0], false, false), Some(6));
        w.mods = Mods { alt: true, ..Mods::default() };
        assert_eq!(w.cursor(&c, [220.0, 200.0], false, false), Some(2));
        w.mods = Mods::default();
        assert_eq!(w.cursor(&c, [220.0, 200.0], false, false), Some(6));
        assert_eq!(w.cursor(&c, [400.0, 300.0], true, false), Some(6));
        assert_eq!(w.cursor(&c, [220.0, 200.0], true, false), None);
    }

    #[test]
    fn landing_clicks_place_pick_up_and_land_all() {
        let mut s = field();
        s.to_land = vec![(1, 5)];
        let c = WarCtx { snap: &s, en: 1, tutorial_paused: false };
        let mut w = ui();
        // A free cell: `PlaceTank`; EN's unit 1 on (11, 20): `UnplaceTank(1)`; side 2's unit 3 on (13, 20): the
        // selection takes the press.
        w.pointer(Some((50, 60)));
        assert_eq!(orders(&w.press(&c, [500.0, 300.0], true, true)), [GroundOrder::Place { x: 50, y: 60 }]);
        w.pointer(Some((11, 20)));
        assert_eq!(orders(&w.press(&c, [500.0, 300.0], true, true)), [GroundOrder::Unplace { unit: 1 }]);
        w.pointer(Some((13, 20)));
        assert!(orders(&w.press(&c, [500.0, 300.0], true, true)).is_empty());
        assert_eq!(orders(&w.press(&c, [500.0, 300.0], false, true)), [GroundOrder::PlaceAll { x: 13, y: 20 }]);
        // Nothing to land, or the planet is EN's (left button): no landing.
        let own = ColonySnap { owner: 1, ..s.clone() };
        let c = WarCtx { snap: &own, en: 1, tutorial_paused: false };
        assert!(orders(&w.press(&c, [500.0, 300.0], true, true)).is_empty());
        assert_eq!(orders(&w.press(&c, [500.0, 300.0], false, true)), [GroundOrder::PlaceAll { x: 13, y: 20 }]);
    }

    /// `Colony_MouseInit2`'s MP tail and `ColonyGfx2D_1`'s `local_22` (spec §11.1): a bystander of the MP war on its
    /// planet (not the owner, no units on the field) gets no war buttons; a participant keeps them; single player has
    /// no gate.
    #[test]
    fn mp_bystanders_get_no_war_buttons() {
        let mut w = ui();
        w.selected = BTreeSet::from([0, 1, 2]);
        let s = ColonySnap { multiplayer: true, mp_war_here: true, ..field() };
        let on = |w: &WarUi, c: &WarCtx| (0xb..=0x10).filter(|&id| w.enabled(c, id)).collect::<Vec<_>>();
        let c = WarCtx { snap: &s, en: 1, tutorial_paused: false };
        let all = on(&w, &c);
        assert!(all.contains(&0x10) && all.contains(&0xb), "{all:x?}");
        assert!(c.shows() && !c.bystander());
        let mut present = s.present;
        present[1] = 0;
        let by = ColonySnap { present, ..s.clone() };
        let c = WarCtx { snap: &by, en: 1, tutorial_paused: false };
        assert!(on(&w, &c).is_empty() && !c.shows());
        // Another planet than the MP war's: the buttons stay, the gated brushes and lines don't show.
        let elsewhere = ColonySnap { mp_war_here: false, ..s.clone() };
        let c = WarCtx { snap: &elsewhere, en: 1, tutorial_paused: false };
        assert_eq!(on(&w, &c), all);
        assert!(!c.shows());
        let sp = ColonySnap { present, multiplayer: false, ..s };
        let c = WarCtx { snap: &sp, en: 1, tutorial_paused: false };
        assert!(c.shows() && !c.bystander());
    }
}
