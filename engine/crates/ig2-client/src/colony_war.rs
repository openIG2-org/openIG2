//! The colony screen in a real-time ground war (re/screens/colony.md §11): the war buttons of `Colony_MouseInit`
//! (exe 0x4680e0) and `Colony_Main` (exe 0x4666d0), unit selection (`GetNearestBuilding` exe 0x47b350, the box of
//! `WarfareKeretesKijeloles` exe 0x4a79e0, the groups of `ColonyKeyboardHandler`), the right button's orders, the
//! armed air strike, and the 2D/3D marks (`WF::WriteStuff`: rings, target markers, route crosses, group sizes).
//! Orders leave as `ig2_sim::game::ColonyOrder::Ground` (`MULTIGET_Colony`'s 0x22..=0x25, 0x12d, 0x12f, 0x130, 0x134).

use std::collections::{BTreeSet, HashMap};

use ig2_net::Command;
use ig2_sim::game::ColonyOrder;
use ig2_sim::ground::{flag, weapon, GroundMap, GroundOrder};

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
        if !in_view || !c.snap.war_flag {
            return out;
        }
        out.taken = true;
        if self.armed != 0 {
            return out;
        }
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
            for u in self.chosen(c).filter(|u| u.owner == c.en && u.flags & 6 == 0) {
                out.commands.push(order(c, GroundOrder::Target { unit: u.slot as i32, target }));
            }
        };
        // `MoveSelectedG` to the last cell under the pointer (also when the ray missed the ground), not with Ctrl or
        // while `TutorialPaused`.
        let move_to = |out: &mut WarOutcome| {
            if !ctrl && !c.tutorial_paused {
                for u in self.chosen(c).filter(|u| u.owner == c.en && u.flags & 4 == 0) {
                    out.commands.push(order(c, GroundOrder::Move { unit: u.slot as i32, x: self.cell.0, y: self.cell.1 }));
                }
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
        }
        out
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
            if !self.enabled(c, id) {
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
        // `ColonyGfx2D_1`: the retreat and landing lines (owner ≠ EN).
        if c.foreign() {
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
            to_land: Vec::new(),
            attacker: 0,
            planet_name: b"Sol 10".to_vec(),
            all_landed: true,
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
        let o = orders(&w.press(&c, [220.0, 200.0], false, true));
        assert_eq!(o, (0..3).map(|u| GroundOrder::Target { unit: u, target: 3 }).collect::<Vec<_>>());
        let o = orders(&w.press(&c, [400.0, 300.0], false, true));
        assert_eq!(o, (0..3).map(|u| GroundOrder::Move { unit: u, x: 50, y: 60 }).collect::<Vec<_>>());
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
}
