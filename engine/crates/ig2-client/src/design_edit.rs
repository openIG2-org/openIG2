//! The design screen, part 2: editing (re/screens/design.md §6, §7). The scratch design `DesignTervTervezett`
//! (`DesignTervezesMod` 1 new, 2 redesign, 3 modify), the popups of `Design_Main` (ELF 0x509140 / exe 0x4b2190:
//! the hull list of "new", the sale of "delete", the pay-or-keep choice of OK while modifying, the pod row's tech
//! list), the pod counts' −/+ with the `KezdoIdo` repeat, the name input (`IOMode`, `IOAzonosito` 0x33), the keys of
//! `DesignKeyboardHandler` (0x508dc0) and `DesignKeyboardEscPressed` (0x509040), and the orders 0x33
//! (`DesignTervezesOk` 0x50b310) and 0x34 as [`DesignOrder`]s, whose events re-sort the list (`MULTIGET_Design`).

use ig2_sim::ai::design::{auto_design_record, recompute_record, slot_max, tech_fits};
use ig2_sim::ai::AiRules;
use ig2_sim::colony_side::TechTable;
use ig2_sim::game::{DesignOrder, OrderEvent};
use ig2_sim::save::{va, SaveState};

use crate::audio::CLICK;
use crate::design::{field, name, Design, POD};
use crate::font::Align;
use crate::render::Frame;
use crate::starmap_panels::Draw2d;

/// `PodMode2x`: the pod row's tech list.
const POD_MODE2_X: i32 = 400;

/// What the screen reads to edit: the save, its techs, the client's tables and the sim's design rules.
pub struct Ctx<'a> {
    pub install: &'a ig2_formats::Install,
    pub state: &'a SaveState,
    pub techs: &'a TechTable,
    pub rules: &'a ig2_rules::Rules,
    pub sim: &'a AiRules,
    /// `DesignDebug`.
    pub debug: bool,
    pub texts: Option<&'a crate::ui::UiTexts>,
}

impl Ctx<'_> {
    pub(crate) fn en(&self) -> usize {
        self.state.mem.i32(va::EN).clamp(0, 18) as usize
    }

    /// `PlayerFaj[EN]`.
    fn race(&self) -> u8 {
        self.state.mem.u8(va::PLAYER_FAJ + self.en() as u32)
    }

    /// `MoneyMennyi(EN, 0)`.
    fn money(&self) -> f64 {
        self.state.treasuries()[self.en()].available(0)
    }

    fn stored(&self, d: usize) -> Option<&[u8; 0x84]> {
        self.state.designs.get(self.en())?.get(d).filter(|_| d > 0)
    }
}

/// The popup texts: `DesignTorlesValasztasNev[0]` (`GetString(0x11, 3, 0x1c)`, the sale) and
/// `DesignModifyOkValasztasNev[0..3]` (`GetString(0x11, 3, 0x20..=0x22)`: keep the old units, refit them, not
/// enough money).
#[derive(Debug, Clone, Default)]
pub struct Names {
    pub sell: Vec<u8>,
    pub modify: [Vec<u8>; 3],
}

impl Names {
    pub fn read(texts: &crate::ui::UiTexts) -> Self {
        Self { sell: texts.get(0x11, 3, 0x1c), modify: [texts.get(0x11, 3, 0x20), texts.get(0x11, 3, 0x21), texts.get(0x11, 3, 0x22)] }
    }
}

/// A popup menu held open by the left button (`Designd*Mode`) at `dSelectX/Y`.
#[derive(Debug, Clone, PartialEq)]
pub enum Popup {
    /// `DesigndSelectAlapMode` (spot 1): the hulls the empire can build.
    Base { x: i32, y: i32, items: Vec<usize> },
    /// `DesigndSelectDeleteMode` (spot 4): one row, the sale and its price.
    Delete { x: i32, y: i32, text: Vec<u8> },
    /// `DesigndSelectRedesignOkMode` (spot 5 while modifying): keep the old units (unless `+0x37`), refit them for
    /// `cost`. `m` = `dSelectM`, the longer row in characters.
    ModifyOk { x: i32, y: i32, m: i32, rows: Vec<Vec<u8>> },
    /// `DesigndSelectPodMode2` (spot 0x54 on a row while `CanModify`): the techs that fit the chosen pod.
    PodRow { items: Vec<usize> },
}

/// The name input (`IOMode` with `IOAzonosito` 0x33): the scratch design's name, or in the viewing mode the stored
/// design's (written in place, with no order: re/deviations.md).
#[derive(Debug, Clone, PartialEq)]
pub struct NameEntry {
    pub text: Vec<u8>,
    pub cursor: usize,
    /// `IOGombnyomasraTorles`: the first key replaces the text.
    pub clear: bool,
    /// The stored design it edits (`None` = the scratch).
    pub stored: Option<usize>,
}

/// A key of the name input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKey {
    Char(u8),
    Back,
    Delete,
    Left,
    Right,
    Home,
    End,
}

/// `IOMaxLen`.
const NAME_MAX: usize = 0x10;

fn rd_i32(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}

fn rd_i16(r: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([r[o], r[o + 1]])
}

/// `strlen`-style name bytes written into a record (NUL-ended).
fn set_name(r: &mut [u8; 0x84], s: &[u8]) {
    let n = s.len().min(0x15);
    r[..n].copy_from_slice(&s[..n]);
    r[n] = 0;
}

/// The trailing ASCII digits of `s` (`isdigit` from the end).
fn split_digits(s: &[u8]) -> (&[u8], &[u8]) {
    let k = s.iter().rposition(|b| !b.is_ascii_digit()).map_or(0, |i| i + 1);
    s.split_at(k)
}

fn atoi(s: &[u8]) -> i32 {
    s.iter().fold(0i32, |n, &b| n.wrapping_mul(10).wrapping_add((b - b'0') as i32))
}

/// `DesignElnevez(emp, base, out)` (ELF 0x5083a0 / exe 0x4aef90): `base` without its trailing number, numbered past
/// every live design of `emp` whose name starts with that stem; `base` itself when nothing matches and it had no
/// number above 0.
pub fn numbered_name(designs: &[[u8; 0x84]], base: &[u8]) -> Vec<u8> {
    let (stem, digits) = split_digits(base);
    let mut n = atoi(digits) + 1;
    let mut used = n > 1;
    for r in designs.iter().skip(1).filter(|r| rd_i16(&r[..], field::DELETED) >= 0) {
        let other = name(r);
        if other.starts_with(stem) {
            n = n.max(atoi(split_digits(other).1) + 1);
            used = true;
        }
    }
    if !used {
        return base.to_vec();
    }
    let mut out = stem[..stem.len().min(12)].to_vec();
    out.extend_from_slice(n.to_string().as_bytes());
    out
}

/// `ModifyPlan` (exe 0x4b5f40)'s version bump: a non-zero trailing number after a `v` or `V` (not the first
/// character) goes up by one, written `v%d`.
pub fn bumped_name(s: &[u8]) -> Vec<u8> {
    let (head, digits) = split_digits(s);
    match head.last() {
        Some(b'v' | b'V') if head.len() > 1 && atoi(digits) != 0 => {
            let mut out = head[..head.len() - 1].to_vec();
            out.extend_from_slice(format!("v{}", atoi(digits) + 1).as_bytes());
            out
        }
        _ => s.to_vec(),
    }
}

impl Design {
    /// The record the screen shows and edits: the scratch while editing (`DesignTervezesMod != 0`).
    pub fn edited<'a>(&'a self, state: &'a SaveState) -> Option<&'a [u8; 0x84]> {
        if self.mode != 0 {
            Some(&self.scratch)
        } else {
            self.record(state)
        }
    }

    /// `TervUjraSzamol(EN, −1, false, true)`.
    pub(crate) fn recompute(&mut self, c: &Ctx) {
        recompute_record(&c.sim.design, c.race(), &mut self.scratch);
    }

    /// `TalalmanyRarakhato(base, t, pod, 5, EN, race)` for t = 0..=0xc0: the techs that fit pod `k` of the scratch.
    fn fitting(&self, c: &Ctx, k: usize) -> Vec<usize> {
        let (hull, pt) = (rd_i32(&self.scratch, field::HULL).max(0) as usize, rd_i16(&self.scratch, field::RACE));
        (0..0xc1).filter(|&t| tech_fits(c.sim, c.techs, c.debug, hull, t, k, 5, c.en(), pt)).collect()
    }

    /// `min(GetMaxRarakhato(race, base, k), Talalmanyok[tech].+0x24)` on the scratch.
    pub(crate) fn scratch_max(&self, c: &Ctx, k: usize) -> i32 {
        let (hull, pt) = (rd_i32(&self.scratch, field::HULL).max(0) as usize, rd_i16(&self.scratch, field::RACE));
        slot_max(c.sim, pt, hull, k).min(c.techs.byte(self.scratch[0x50 + k] as usize, 0x24) as i32)
    }

    /// The bases the empire can build (`Alapok[1..25]` whose hull tech has status 5 for `EN` and isn't obsolete).
    fn bases(&self, c: &Ctx) -> Vec<usize> {
        (1..25)
            .filter(|&b| {
                let t = c.rules.hulls.get(b).map_or(0, |h| h.tech).clamp(0, 192) as usize;
                c.techs.status(t, c.en()) == 5 && c.techs.byte(t, 0x26) & 1 == 0
            })
            .collect()
    }

    /// The name input on `stored` (the viewing mode's design) or the scratch.
    fn start_entry(&mut self, text: Vec<u8>, stored: Option<usize>) {
        self.entry = Some(NameEntry { cursor: text.len(), text, clear: true, stored });
    }

    /// `RedesignPlan` (ELF 0x508a60 / exe 0x4b6940): a copy of the chosen design, renamed, as a new design.
    pub fn redesign(&mut self, c: &Ctx) {
        let Some(r) = c.stored(self.current()).copied() else { return };
        self.push_button(3);
        self.can_modify = true;
        self.pod = 0;
        self.mode = 2;
        self.scratch = r;
        let n = numbered_name(&c.state.designs[c.en()], name(&r));
        set_name(&mut self.scratch, &n);
        self.start_entry(n, None);
    }

    /// `ModifyPlan` (exe 0x4b5f40): the chosen design edited in place, its name's version bumped; the pods can be
    /// changed (`CanModify`).
    pub fn modify(&mut self, c: &Ctx) {
        let Some(r) = c.stored(self.current()).copied() else { return };
        self.mode = 3;
        self.scratch = r;
        self.can_modify = true;
        self.pod = 0;
        self.push_button(4);
        let n = bumped_name(name(&r));
        set_name(&mut self.scratch, &n);
    }

    /// `DesignCancel` (ELF 0x508d30 / exe 0x4aee90).
    pub fn cancel(&mut self, c: &Ctx) {
        self.recompute(c);
        self.push_button(3);
        self.can_modify = false;
        self.mode = 0;
        self.pod = 0;
        self.resort(c.state);
        self.entry = None;
    }

    /// `DesignTervezesOk(keepOld)` (ELF 0x50b310 / exe 0x4b1b20): the scratch goes to the sim as order 0x33 unless
    /// a redesign left it equal to its original (`TervEqual`).
    fn store(&mut self, c: &Ctx, keep_old: bool) {
        self.can_modify = false;
        self.recompute(c);
        let mode = self.mode;
        if mode != 2 || c.stored(self.current()).is_none_or(|r| !design_equal(r, &self.scratch)) {
            clear_empty(&mut self.scratch);
            self.orders.push(DesignOrder::Store { mode: mode as i32, design: self.current() as i32, record: self.scratch, copy: keep_old });
        }
        self.mode = 0;
    }

    /// `Design_UjraRendez` with the chosen rank kept (`AktTerv = R[AktTervMutato]`).
    pub fn resort(&mut self, state: &SaveState) {
        self.order = crate::design::sorted(state, state.mem.i32(va::EN).clamp(0, 18) as usize);
        if self.count() == 0 {
            self.pointer = 0;
        }
        self.pointer = self.pointer.min(self.count());
    }

    /// `MULTIGET_Design`'s follow-ups for the human (`AktKep == 5 && DesignTervezesMod == 0 && !DeleteMode`).
    /// An order's event (`MULTIGET_Design`). The exe handles the frame's messages before `Design_Main`, so an
    /// order sent this frame acts on the next: the frame after the input is still drawn with the old `AktTerv`
    /// (OK on a new design slides the old hull out first) [obs: be1 after OK, design-edit.json].
    pub fn event(&mut self, e: &OrderEvent) {
        self.events.push(*e);
    }

    /// The events of the orders sent before the last frame (`step`'s start).
    pub(crate) fn apply_events(&mut self, state: &SaveState) {
        for e in std::mem::take(&mut self.events_due) {
            self.apply_event(state, &e);
        }
        self.events_due = std::mem::take(&mut self.events);
    }

    fn apply_event(&mut self, state: &SaveState, e: &OrderEvent) {
        let en = state.mem.i32(va::EN).clamp(0, 18) as u8;
        if self.mode != 0 || matches!(self.popup, Some(Popup::Delete { .. })) {
            return;
        }
        match *e {
            OrderEvent::DesignStored { empire, design } if empire == en => {
                self.resort(state);
                if let Some(k) = self.order.iter().position(|&d| d == design) {
                    self.pointer = k;
                }
            }
            OrderEvent::DesignSold { empire, design, holder: 0 } if empire == en => {
                if self.current() == design && self.count() <= self.pointer {
                    self.pointer = self.pointer.saturating_sub(1);
                }
                self.resort(state);
            }
            _ => return,
        }
        self.scroll_to_pointer();
    }

    /// The left button went down on `id` (`Design_Main` with `Mouse_Lenyom`). True = handled here.
    pub fn edit_press(&mut self, c: &Ctx, id: u32, at: [f32; 2]) -> bool {
        let (mx, my) = (at[0] as i32, at[1] as i32);
        match id {
            1 => {
                self.sounds.push(CLICK);
                self.push_button(0);
                let items = self.bases(c);
                let y = 0x16e - 13 * items.len() as i32;
                self.popup = Some(Popup::Base { x: mx + 12, y, items });
                self.held = Some(1);
                self.hover_popup(at);
            }
            4 if !self.tutorial_paused => {
                self.sounds.push(CLICK);
                self.push_button(5);
                let (_, sum) = crate::starmap_info::units_of(c.state, c.en() as u8, 0, self.current());
                let div = crate::starmap_info::sale_divisor(c.state, c.en() as u8, self.current());
                let price = if div < 0.0 { 0 } else { (sum as f32 / div) as i32 };
                let mut text = self.names.sell.clone();
                text.extend_from_slice(price.to_string().as_bytes());
                self.popup = Some(Popup::Delete { x: mx + 16, y: my - 8, text });
                self.held = Some(4);
                self.hover_popup(at);
            }
            5 if self.sub_zoom == 0.0 => {
                self.sounds.push(CLICK);
                self.push_button(0);
                self.recompute(c);
                if self.mode == 3 {
                    self.popup = Some(self.modify_ok(c, at));
                    self.held = Some(5);
                    self.hover_popup(at);
                } else {
                    // The tutorial's event 0xc per fitted pod on the first hull (`AktAlap == 6`) isn't raised here.
                    self.store(c, false);
                }
            }
            0x1f | 0x20 | 0x35..=0x52 => {
                self.sounds.push(CLICK);
                self.held = Some(id);
                self.repeat = 1;
                self.step_count(c, id);
            }
            // The `onRelease` spots: `IG2_gameUpdate` (exe 0x4e0440) hands them to `Design_Main` once, on the press
            // (`Mouse_Lenyom`: their click), then runs frames until the button is up.
            2 | 3 => {
                self.sounds.push(CLICK);
                if id == 2 {
                    self.redesign(c);
                } else {
                    self.modify(c);
                }
            }
            6 => {
                self.sounds.push(CLICK);
                self.cancel(c);
            }
            7 => {
                self.sounds.push(CLICK);
                self.push_button(4);
                let (en, race) = (c.en(), c.race());
                auto_design_record(c.sim, c.techs, c.debug, &mut self.scratch, en, race, 0, false);
                self.recompute(c);
            }
            8 => {
                self.sounds.push(CLICK);
                self.push_button(5);
                ig2_sim::spacewar::clear_design_record(&mut self.scratch);
                self.recompute(c);
            }
            _ => return false,
        }
        true
    }

    /// `Design_Main` on OK while modifying: the rows and where they open.
    fn modify_ok(&self, c: &Ctx, at: [f32; 2]) -> Popup {
        let one = c.stored(self.current()).is_some_and(|r| r[field::NO_SPEED] & 1 != 0);
        let cost = self.refit_cost(c);
        let mut refit = if (cost as f64) <= c.money() || cost < 1 { self.names.modify[1].clone() } else { self.names.modify[2].clone() };
        refit.extend_from_slice(b" - $");
        refit.extend_from_slice(cost.to_string().as_bytes());
        let keep = if one { 0 } else { self.names.modify[0].len() };
        let m = refit.len().max(keep) as i32;
        let x = (at[0] as i32 + 12).min((m + 1) * -8 + 0x27c);
        let y = at[1] as i32 - if one { 0x13 } else { 0x22 };
        let rows = if one { vec![refit] } else { vec![self.names.modify[0].clone(), refit] };
        Popup::ModifyOk { x, y, m, rows }
    }

    /// `max(0, (HanyHajoVanATervbol(EN, 0, AktTerv)·scratch.+0x24 − TervHajoinakAra(EN, 0, AktTerv))·2)`.
    fn refit_cost(&self, c: &Ctx) -> i32 {
        let (n, sum) = crate::starmap_info::units_of(c.state, c.en() as u8, 0, self.current());
        (n.wrapping_mul(rd_i32(&self.scratch, field::PRICE)).wrapping_sub(sum)).wrapping_mul(2).max(0)
    }

    /// While held: the popup row under the pointer (`dSelected`).
    pub fn hover_popup(&mut self, at: [f32; 2]) {
        let (mx, my) = (at[0] as i32, at[1] as i32);
        self.selected = -1;
        match &self.popup {
            Some(Popup::Base { x, y, items }) => {
                for (n, &b) in items.iter().enumerate() {
                    let n = n as i32;
                    if *x <= mx && mx <= x + 0x78 && y + 13 * n <= my && my < y + 13 * (n + 1) {
                        self.selected = b as i32;
                    }
                }
            }
            Some(Popup::Delete { x, y, text }) => {
                if *x <= mx && mx <= x + 6 * text.len() as i32 && *y <= my && my < y + 13 {
                    self.selected = 1;
                }
            }
            Some(Popup::ModifyOk { x, y, m, rows }) => {
                for i in 1..=rows.len() as i32 {
                    if *x <= mx && mx <= x + m * 8 && y + 15 * (i - 1) <= my && my < y + 15 * i {
                        self.selected = i;
                    }
                }
            }
            Some(Popup::PodRow { items }) => {
                let top = POD[1] as i32 + self.pod as i32 * 15;
                for (n, &t) in items.iter().enumerate() {
                    let n = n as i32;
                    if POD_MODE2_X < mx && mx < POD_MODE2_X + 0x6e && n * 13 + top + 2 <= my && my < n * 13 + top + 0xf {
                        self.selected = t as i32;
                    }
                }
            }
            None => {}
        }
    }

    /// Spot 0x54 pressed while `CanModify`: the row picks the pod and opens its tech list.
    pub fn pod_row_press(&mut self, c: &Ctx) {
        if self.can_modify && self.pod > 0 && self.mode != 0 {
            let items = self.fitting(c, self.pod);
            self.popup = Some(Popup::PodRow { items });
            self.selected = -1;
        }
    }

    /// One `KezdoIdo` step of a held −/+ (`== 1 || > 7`): the count of pod `k` down or up to what it takes (the 3D
    /// buttons 0x20 −, 0x1f + on `DesignAktPod`, `DesMelyikGombot` 1 / 2; the cells on their row).
    fn step_count(&mut self, c: &Ctx, id: u32) {
        let (k, minus) = match id {
            0x1f => (self.pod, false),
            0x20 => (self.pod, true),
            _ => (((id - 0x33) / 2) as usize, (id - 0x33) % 2 == 0),
        };
        if id == 0x1f || id == 0x20 {
            self.push_button(if minus { 1 } else { 2 });
        }
        if k == 0 || k > 12 {
            return;
        }
        if minus {
            self.scratch[0x5d + k] = self.scratch[0x5d + k].saturating_sub(1);
        } else if (self.scratch[0x5d + k] as i32) < self.scratch_max(c, k) {
            self.scratch[0x5d + k] += 1;
        }
        self.recompute(c);
    }

    /// Each UI tick while a −/+ is held.
    pub fn edit_held(&mut self, c: &Ctx) {
        if let Some(id @ (0x1f | 0x20 | 0x35..=0x52)) = self.held {
            self.repeat += 1;
            if self.repeat > 7 {
                self.step_count(c, id);
            }
        }
    }

    /// The left button went up (`Mouse_Felenged`): the held popup acts on its row (`dSelected`) and closes.
    pub fn edit_release(&mut self, c: &Ctx) {
        let popup = self.popup.take();
        match popup {
            Some(Popup::Base { .. }) if self.selected > 0 => {
                let b = self.selected as usize;
                // `SetSecondMenu` (its pod light and `SelectedPod` are part 3); `DesignAktPod` stays as it was.
                self.mode = 1;
                self.can_modify = true;
                ig2_sim::spacewar::clear_design_record(&mut self.scratch);
                self.scratch[field::HULL..field::HULL + 4].copy_from_slice(&(b as i32).to_le_bytes());
                self.scratch[field::RACE..field::RACE + 2].copy_from_slice(&(c.race() as u16).to_le_bytes());
                self.recompute(c);
                let t = c.rules.hulls.get(b).map_or(0, |h| h.tech).clamp(0, 192) as usize;
                let n = numbered_name(&c.state.designs[c.en()], &c.texts.map_or_else(Vec::new, |x| crate::hud::tech_name(x, t)));
                set_name(&mut self.scratch, &n);
                self.start_entry(n, None);
            }
            Some(Popup::Delete { .. }) => {
                if self.selected > 0 {
                    self.orders.push(DesignOrder::Sell { design: self.current() as i32, mode: self.selected + 1, holder: 0 });
                }
                self.resort(c.state);
            }
            Some(Popup::ModifyOk { rows, .. }) => {
                let one = rows.len() == 1;
                if self.selected == 2 || (one && self.selected == 1) {
                    let cost = self.refit_cost(c);
                    if (cost as f64) <= c.money() || cost < 1 {
                        self.store(c, false);
                    }
                } else if self.selected == 1 {
                    self.store(c, true);
                }
            }
            Some(Popup::PodRow { items }) => {
                if self.selected != -1 && self.can_modify {
                    let k = self.pod;
                    if self.selected == 0 {
                        self.scratch[0x50 + k] = 0;
                        self.scratch[0x5d + k] = 0;
                    } else if items.contains(&(self.selected as usize)) {
                        self.scratch[0x50 + k] = self.selected as u8;
                        self.scratch[0x5d + k] = self.scratch_max(c, k).clamp(0, 255) as u8;
                    }
                    self.recompute(c);
                }
            }
            Some(Popup::Base { .. }) | None => {}
        }
        self.selected = -1;
    }

    /// The name input took a key (`DoKeyboard` with `IOMode`): the name is written as typed.
    pub fn entry_key(&mut self, k: EntryKey) -> Option<(usize, Vec<u8>)> {
        let e = self.entry.as_mut()?;
        let clear = std::mem::take(&mut e.clear);
        if clear && matches!(k, EntryKey::Char(_) | EntryKey::Back | EntryKey::Delete) {
            e.text.clear();
            e.cursor = 0;
        }
        match k {
            EntryKey::Char(ch) if e.text.len() < NAME_MAX => {
                e.text.insert(e.cursor.min(e.text.len()), ch);
                e.cursor += 1;
            }
            EntryKey::Back if !clear && e.cursor > 0 => {
                e.cursor -= 1;
                e.text.remove(e.cursor);
            }
            EntryKey::Delete if !clear && e.cursor < e.text.len() => {
                e.text.remove(e.cursor);
            }
            EntryKey::Left => e.cursor = e.cursor.saturating_sub(1),
            EntryKey::Right => e.cursor = (e.cursor + 1).min(e.text.len()),
            EntryKey::Home => e.cursor = 0,
            EntryKey::End => e.cursor = e.text.len(),
            _ => {}
        }
        let (text, stored) = (e.text.clone(), e.stored);
        match stored {
            None => {
                set_name(&mut self.scratch, &text);
                None
            }
            Some(d) => Some((d, text)),
        }
    }

    /// Spot 0x56: the name input on the shown design (`IOString` = the stored record while viewing).
    pub fn name_press(&mut self, c: &Ctx) {
        if self.mode != 0 {
            let n = name(&self.scratch).to_vec();
            self.start_entry(n, None);
        } else if let Some(r) = c.stored(self.current()) {
            let n = name(r).to_vec();
            self.start_entry(n, Some(self.current()));
        }
    }

    /// `DesignKeyboardHandler`'s 'r' and Enter (viewing, a design whose hull and itself aren't obsolete).
    pub fn edit_key(&mut self, c: &Ctx, redesign: bool) {
        if self.mode != 0 || self.count() == 0 || !self.spot_state(c.state, c.techs, c.rules, 2) {
            return;
        }
        if redesign {
            self.redesign(c);
        } else {
            self.modify(c);
        }
    }

    /// `DesignKeyboardEscPressed` (0x509040): the input ends; while editing the edit is cancelled and the menus
    /// close. True = it was the screen's (the viewing mode's Esc leaves the screen).
    pub fn escape(&mut self, c: &Ctx) -> bool {
        let had = self.entry.take().is_some();
        if self.mode != 0 {
            self.popup = None;
            self.cancel(c);
            return true;
        }
        had
    }

    /// Popups' frames (`FadeKerettelKerekit(.., 0, 10, 0, 0x3c, 0, 5, 0, 0x1e, ..)`) and their rows
    /// (`DesignGfx2D_2`: colour 2 on `dSelected`, else 1).
    pub fn draw_popups(&self, f: &mut Frame, d: &Draw2d, rules: &ig2_rules::Rules) {
        let Some(p) = &self.popup else { return };
        let col = |k: usize| d.palette.get(k).copied().unwrap_or([1.0; 3]);
        let frame = |f: &mut Frame, r: [i32; 4]| {
            crate::kit::fader::fader(f, d.white, &d.v, r.map(|x| x as f32), [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
        };
        let item = |f: &mut Frame, s: &[u8], x: i32, y: i32, len: usize, size: usize, on: bool| {
            d.font.text(f, &d.v, s, x as f32, y as f32, len, size, Align::Left, col(if on { 2 } else { 1 }), None);
        };
        match p {
            Popup::Base { x, y, items } => {
                frame(f, [x - 6, y - 2, 0x82, items.len() as i32 * 13 + 4]);
                for (n, &b) in items.iter().enumerate() {
                    let t = rules.hulls.get(b).map_or(0, |h| h.tech).clamp(0, 192) as usize;
                    item(f, &crate::hud::tech_name(d.texts, t), *x, y + 3 + 13 * n as i32, 16, 2, self.selected == b as i32);
                }
            }
            Popup::Delete { x, y, text } => {
                frame(f, [x - 6, y - 2, 6 * text.len() as i32 + 10, 0x11]);
                item(f, text, *x, y + 3, 40, 2, self.selected == 1);
            }
            Popup::ModifyOk { x, y, m, rows } => {
                frame(f, [x - 6, y - 2, 8 * m + 10, rows.len() as i32 * 15 + 4]);
                for (i, r) in rows.iter().enumerate() {
                    let i = i as i32 + 1;
                    item(f, r, *x, y - 12 + 15 * i, r.len(), 3, self.selected == i);
                }
            }
            Popup::PodRow { items } => {
                let top = POD[1] as i32 + self.pod as i32 * 15;
                frame(f, [POD_MODE2_X - 6, top, 0x82, items.len() as i32 * 13 + 4]);
                for (n, &t) in items.iter().enumerate() {
                    item(f, &crate::hud::tech_name(d.texts, t), POD_MODE2_X, top + 5 + 13 * n as i32, 16, 2, self.selected == t as i32);
                }
            }
        }
    }
}

/// `TervEqual(EN, AktTerv, −1)` (exe 0x542260 → `TervEqual2` 0x5421e0): the same class, owner race, hull, hull tech
/// and slot count, and the same tech and count in every slot (the name and the price aside).
fn design_equal(a: &[u8; 0x84], b: &[u8; 0x84]) -> bool {
    let n = rd_i32(a, 0x4c).clamp(0, 12) as usize;
    rd_i16(a, 0x16) == rd_i16(b, 0x16)
        && rd_i16(a, 0x40) == rd_i16(b, 0x40)
        && [0x44, 0x48, 0x4c].iter().all(|&o| rd_i32(a, o) == rd_i32(b, o))
        && (1..=n).all(|i| a[0x50 + i] == b[0x50 + i] && a[0x5d + i] == b[0x5d + i])
}

/// `TervClearEmpty(EN, −1)` (exe 0x541c60): a slot with no units loses its tech.
fn clear_empty(r: &mut [u8; 0x84]) {
    for i in 1..=12 {
        if r[0x5d + i] == 0 {
            r[0x50 + i] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_number_past_the_designs_with_the_same_stem() {
        let mut t = vec![[0u8; 0x84]; 3];
        set_name(&mut t[1], b"Cruiser2");
        set_name(&mut t[2], b"Other");
        assert_eq!(numbered_name(&t, b"Cruiser"), b"Cruiser3");
        assert_eq!(numbered_name(&t, b"Fighter"), b"Fighter");
        assert_eq!(numbered_name(&t, b"Fighter4"), b"Fighter5");
    }

    #[test]
    fn modify_bumps_a_v_number() {
        assert_eq!(bumped_name(b"Hawk v2"), b"Hawk v3");
        assert_eq!(bumped_name(b"HawkV9"), b"Hawkv10");
        assert_eq!(bumped_name(b"Hawk2"), b"Hawk2");
        assert_eq!(bumped_name(b"v3"), b"v3");
        assert_eq!(bumped_name(b"Hawk v0"), b"Hawk v0");
    }
}
