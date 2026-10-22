//! The spy screen (`AktKep` 7, re/screens/spy.md): the roster carousel, the selected spy's stats, the mission
//! planner and its pick lists, the reports and the applicant/verdict dialogs. `Spy_Belep` (ELF 0x6c5a70, exe
//! 0x58cd50) sets the screen up, `FRISSIT_Spy2` (ELF 0x6d1e20) runs on the 100 Hz UI clock, `Spy_Main` (ELF
//! 0x6c7360, exe 0x58d410) takes the hot-spots of `Spy_MouseInit`/`Spy_MouseInit2` (ELF 0x6ca930/0x6c99c0) and
//! sends the orders through the lockstep queue (`Command::Spy`, `MULTIGET_Spy` exe 0x5883c0), `GFX_Spy` (ELF
//! 0x6c6080, exe 0x586aa0) and `SpyGfx2D_1..3` (ELF 0x6bb6e0/0x6bb7c0/0x6c3a10, exe –/0x588bd0/0x58c1d0) draw it.
//!
//! Everything here is UI state; what the orders change is the sim's (`ig2_sim::spy`).

use ig2_net::{Command, SpyOrder};
use ig2_sim::save::{va, SaveState, PLANET_SIZE, TECH_SIZE};
use ig2_sim::spy::{self, Spies, Spy};

use crate::render::{Frame, TexId};
use crate::spy_scene::{Pose, Spot};

mod draw;
pub use draw::Draw2d;

/// The texts the screen uses: `GetString(7, …)` in the game's language, the name tables `StringReadAll` fills
/// from the Hungarian `txt17` (re/screens/spy.md §4).
pub struct SpyTexts {
    /// `KemKuldetes_Nev[1..=26]` = (17, 2, k + 20), `Rang_Nev[1..=20]` = (17, 2, k), `KemStatusz_Nev[1..=4]` =
    /// (17, 3, k), `KemBeszolas_Szoveg[1..=5]` = (17, 3, k + 4), `KemCsaladtag_Nev[1..=5]` = (17, 3, k + 9),
    /// `KemMitortentAFellazadtBolygoval_Szoveg[1..=4]` = (17, 3, k + 14), `SpyAreYouSure_Nev[0..3]` = (17, 6,
    /// 43..=45), `Faj_Nev[1..=14]` = (17, 1, k + 5); `TrainingKuldetesSzoveg[1..=5]` = (7, 6, k),
    /// `SpyCelTipusNevek[1..=5]` = (7, 6, k + 5) (`Spy_Belep`).
    pub missions: Vec<Vec<u8>>,
    pub ranks: Vec<Vec<u8>>,
    pub statuses: Vec<Vec<u8>>,
    pub outcomes: Vec<Vec<u8>>,
    pub family: Vec<Vec<u8>>,
    pub rebellion: Vec<Vec<u8>>,
    pub sure: Vec<Vec<u8>>,
    pub races: Vec<Vec<u8>>,
    pub training: Vec<Vec<u8>>,
    pub target_kinds: Vec<Vec<u8>>,
    /// `Moral_Nev[k]` = (17, 4, 41 + k) (as `starmap_hud`).
    pub morale: Vec<Vec<u8>>,
    /// `Birodalom_Nev[1..=18]` = (17, 1, k + 33).
    pub empires: Vec<Vec<u8>>,
    /// The hot-spots' tooltips (`Spy_MouseInit`'s `Mouse_Szoveg_Set(id, …, GetString(7, 1, k))`, indices from the
    /// asm): by hot-spot id.
    pub tips: Vec<Vec<u8>>,
    /// `Spy_Main`'s lines for `ScrollHibaUzenet`: a mission the treasury can't pay (7, 2, 1), a repeat it can't pay
    /// (7, 2, 2).
    pub no_money: Vec<u8>,
    pub no_money_repeat: Vec<u8>,
}

/// `Spy_MouseInit`'s tooltip text `GetString(7, 1, k)` of hot-spot `id` (0: none): 1..=4 → 1..=4, 0xe..=0x18 →
/// 5..=15, the four skill buttons 0x19..=0x1c all 16, 0x1e..=0x25 → 17..=24.
pub fn tip_index(id: u32) -> u16 {
    match id {
        1..=4 => id as u16,
        0xe..=0x18 => (id - 9) as u16,
        0x19..=0x1c => 16,
        0x1e..=0x25 => (id - 13) as u16,
        _ => 0,
    }
}

impl SpyTexts {
    pub fn new(texts: &crate::ui::UiTexts, hu17: &ig2_formats::strings::UiStrings) -> Self {
        let h = |s: u16, i: u16| crate::ui::hu_bytes(hu17.get(17, s, i));
        let tbl = |n: u16, f: &dyn Fn(u16) -> Vec<u8>| (0..=n).map(|k| if k == 0 { Vec::new() } else { f(k) }).collect::<Vec<_>>();
        Self {
            missions: tbl(26, &|k| h(2, k + 20)),
            ranks: tbl(20, &|k| h(2, k)),
            statuses: tbl(4, &|k| h(3, k)),
            outcomes: tbl(5, &|k| h(3, k + 4)),
            family: tbl(5, &|k| h(3, k + 9)),
            rebellion: tbl(4, &|k| h(3, k + 14)),
            sure: (43..=45).map(|i| h(6, i)).collect(),
            races: tbl(14, &|k| h(1, k + 5)),
            training: tbl(5, &|k| texts.get(7, 6, k)),
            target_kinds: tbl(5, &|k| texts.get(7, 6, k + 5)),
            morale: (0..=20).map(|k| crate::ui::hu_bytes(hu17.get(17, 4, 41 + k))).collect(),
            empires: crate::ui::empire_names(hu17),
            tips: (0..=0x25).map(|id| match tip_index(id) {
                0 => Vec::new(),
                k => texts.get(7, 1, k),
            }).collect(),
            no_money: texts.get(7, 2, 1),
            no_money_repeat: texts.get(7, 2, 2),
        }
    }
}

fn get(v: &[Vec<u8>], i: i64) -> &[u8] {
    usize::try_from(i).ok().and_then(|i| v.get(i)).map_or(&[], |x| x.as_slice())
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// The first report of the queue (`KemBeszolasok[1]`, re/rules/info-messages.md §3.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    pub kind: u8,
    pub a: i32,
    pub b: u8,
    pub c: u8,
    pub x: [i32; 9],
}

impl Report {
    fn read(q: &[u8]) -> Option<Self> {
        let r = q.get(..spy::REPORT)?;
        let i = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        Some(Self { kind: r[0], a: i(4), b: r[8], c: r[9], x: std::array::from_fn(|k| i(0xc + 4 * k)) })
    }
    /// `x6`: the mission, `x7` the target empire, `x8`/`x9` its parameters, `x10..x13` the result's values, `x14` the
    /// fail code (997 target dead, 998 lost the duel, 999 nothing to do, 0x3e5 target beaten).
    fn mission(&self) -> i32 {
        self.x[0]
    }
}

/// What the screen reads from the sim each frame.
pub struct SpyData<'a> {
    pub state: &'a SaveState,
    pub en: usize,
    pub spies: Spies,
    pub reports: Vec<u8>,
    pub money: f64,
    pub rules: &'a ig2_sim::game::GameRules,
    pub tables: &'a ig2_rules::Rules,
    pub alive: [bool; 19],
    pub races: [u8; 19],
    pub known: Vec<bool>,
    /// `HanyKemVan[EN]`, `HanyKemLehet[EN]`.
    pub working: i32,
    pub capacity: i32,
    pub multiplayer: bool,
}

impl<'a> SpyData<'a> {
    pub fn read(state: &'a SaveState, en: usize, rules: &'a ig2_sim::game::GameRules, tables: &'a ig2_rules::Rules) -> Self {
        let spies = state.spies();
        let canon = state.mem.i32(va::EN).clamp(0, 18) as usize;
        Self {
            state,
            en,
            working: spies.working.get(en).copied().unwrap_or(0),
            capacity: state.mem.i32(va::SPY_CAPACITY + 4 * en as u32),
            reports: state.spy_reports(en, canon),
            spies,
            money: state.treasuries().get(en).map_or(0.0, |t| t.available(0)),
            rules,
            tables,
            alive: state.alive(),
            races: state.races(),
            known: (0..19).map(|e| state.mem.u8(va::EMPIRE_KNOWS + (en * 19 + e) as u32) != 0).collect(),
            multiplayer: state.mem.u8(va::MULTIPLAYER) != 0,
        }
    }
    fn spy(&self, k: usize) -> Spy {
        self.spies.roster.get(self.en).and_then(|r| r.get(k)).copied().unwrap_or_default()
    }
    fn count(&self) -> usize {
        self.spies.count.get(self.en).copied().unwrap_or(0).clamp(0, spy::MAX_SPIES) as usize
    }
    fn mission(&self, m: i32) -> spy::MissionRule {
        self.rules.spy.missions.get(m.max(0) as usize).copied().unwrap_or_default()
    }
    /// `KemKuldetesek[m].+0`: the mission is on offer (the story toggles them).
    fn enabled(&self, m: i32) -> bool {
        self.spies.enabled.get(m.max(0) as usize).copied().unwrap_or(0) != 0
    }
    fn report(&self) -> Option<Report> {
        Report::read(&self.reports)
    }
    /// `KemKuldetesPenzSzorzoFajonkent[PlayerFaj[EN]]` × the cost (`TrainingKuldetesAr[level]` for training).
    fn cost(&self, m: i32, level: i32) -> f32 {
        let c = if m == 1 { self.rules.ai.training_cost.get(level.max(0) as usize).copied().unwrap_or(0) } else { self.mission(m).cost };
        c as f32 * self.rules.ai.mission_cost_mult.get(self.races[self.en] as usize).copied().unwrap_or(0.0)
    }
    fn planet_name(&self, p: i32) -> Vec<u8> {
        cstr(&self.state.mem.read(va::PLANETS + (p.max(0) as usize * PLANET_SIZE) as u32, 0x20)).to_vec()
    }
    fn tech_name(&self, t: i32) -> Vec<u8> {
        cstr(&self.state.mem.read(va::TECHS + (t.max(0) as usize * TECH_SIZE) as u32, 0x20)).to_vec()
    }
    fn planet_i32(&self, p: i32, off: u32) -> i32 {
        self.state.mem.i32(va::PLANETS + p.max(0) as u32 * PLANET_SIZE as u32 + off)
    }
    fn tech_u8(&self, t: usize, off: usize) -> u8 {
        self.state.mem.u8(va::TECHS + (t * TECH_SIZE + off) as u32)
    }
    /// The empires the human can target: alive, known (or `DEBUGX` in SP), not itself, 1..=8.
    fn targets(&self) -> Vec<usize> {
        (1..9).filter(|&e| self.alive[e] && self.known.get(e).copied().unwrap_or(false) && e != self.en).collect()
    }
    /// `KemKuldetesek[m].+8`: the second parameter's kind (2 = a tech).
    fn second_kind(&self, m: i32) -> i32 {
        self.tables.spy_missions.get(m.max(0) as usize).map_or(0, |x| x.w8)
    }
    /// `HanyBolygo`.
    fn planets(&self) -> usize {
        self.state.mem.i32(va::PLANET_COUNT).clamp(0, 300) as usize
    }
}

/// The open pick list (`SpyNewKuldetesMode`, `SpyNewRaceMode`, `SpyNewTargetMode`, `SpyAreYouSureToKillMode`) with
/// its box (`spySelectX/Y`, columns `spySelectSz`, rows `spySelectMa`, width `spySelectChr`) and the row under the
/// pointer (`spySelected`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum List {
    Mission,
    Race,
    /// `SpyNewTargetMode` 1..=5 (planet, tech for theft, hull, tech for sabotage, training level); `second` = the
    /// second parameter (hot-spot 0x13: always techs).
    Target { mode: i32, second: bool },
    Kill,
}

#[derive(Debug, Clone, Copy)]
pub struct Popup {
    pub list: List,
    pub x: i32,
    pub y: i32,
    pub cols: i32,
    pub rows: i32,
    pub chars: i32,
    pub selected: i32,
}

/// `SpyNewMission*`: the mission planner.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Plan {
    pub on: bool,
    pub mission: i32,
    pub race: i32,
    pub target: i32,
    pub target2: i32,
    pub days: i32,
    pub cost: i32,
    pub chance1: i32,
    pub chance2: i32,
    pub recompute: bool,
}

impl Plan {
    /// `SpyInitNewMission` (ELF 0x6b92a0).
    fn init() -> Self {
        Self { on: true, days: -1, cost: -1, chance1: -1, chance2: -1, recompute: true, ..Default::default() }
    }
    /// `SpyDeleteNewMission` (ELF 0x6b9340).
    fn delete() -> Self {
        Self { days: -1, cost: -1, chance1: -1, chance2: -1, ..Default::default() }
    }
}

/// The screen's state (`Spy_Belep`'s globals).
#[derive(Debug, Clone)]
pub struct SpyScreen {
    /// `AktKem`, `SpyBeszolasKemBackup`, `SpyOldKem`.
    pub akt: usize,
    backup: usize,
    old_kem: i32,
    pub plan: Plan,
    pub popup: Option<Popup>,
    /// `SpyBeszolasMi`: the dialog shown (the first report's kind), 0 none.
    pub dialog: u8,
    /// `SpyEppKuldetesElkuldeseAlattVan`: a mission was just sent (no new plan until the spy leaves home).
    sending: bool,
    /// Orders in flight: the report on show changes only when they are applied (the next frame's pump).
    awaiting: bool,
    /// `KemNevVillogtato`/`Dir`, `spyKiirSzinSzorzo`, `spykeringeto`, `SpyPalastRotation`, `ElfordulasiSebesseg`.
    blink: i32,
    blink_up: bool,
    pub text_fade: f32,
    pub orbit: f32,
    pub rotation: f64,
    turn_speed: f32,
    /// `spyframe`, `spywireframe`, `spyFrameValtozo`, `spyAtfordulas`, `spyAtvaneFordulva`, `RaalltKem`, `SpyOldMe`.
    pub frame: f32,
    pub wire: f32,
    /// `spyKELLKINYITAS` (`KELLKINYITASDEFAULT`: the screen opens and closes animated), `spyKINYITVA` (open: set
    /// by `Spying3DUpdate` at frame 50), `spyBECSUKAS` (closing, `Spy_Kilep`) and `Spy_Kilep::voltnulla` (frame 0
    /// was drawn once).
    pub animated: bool,
    pub opened: bool,
    pub closing: bool,
    closed_once: bool,
    /// `spyELINDULT`: a frame was drawn (the opening runs from then on).
    started: bool,
    /// A `spyframe` held for a capture comparison (the tracer pins the exe's the same way).
    pub pin: Option<f32>,
    /// The head-video disc is shown (`UnhideObj(spyKorId)`: by `Spying3DInit` when not animated, else once a
    /// flip passes its frame 15).
    pub disc: bool,
    pub spin: i32,
    pub flip: f32,
    pub flipped: f32,
    settled: i32,
    old_me: usize,
    /// The pointer in virtual pixels and the left button.
    pub mouse: [f32; 2],
    held: bool,
    /// The tooltip (`UpdateFrame`).
    pub tip: crate::kit::Tooltip,
    /// Lines for the message ticker (`ScrollHibaUzenet`), taken by the window.
    pub errors: Vec<Vec<u8>>,
}

impl SpyScreen {
    /// `Spy_Belep`: the cursor on slot 1, no plan or list, then the first report if there is one. `opening` =
    /// `spyKELLKINYITAS`: the frame opens from `spyframe` 0 (else it is open at once, re/screens/spy.md §1.1).
    pub fn enter(d: &SpyData, opening: bool) -> Self {
        let mut s = Self {
            akt: 1,
            backup: 0,
            old_kem: -1,
            plan: Plan::delete(),
            popup: None,
            dialog: 0,
            sending: false,
            awaiting: false,
            blink: 0,
            blink_up: true,
            text_fade: 0.0,
            orbit: 0.0,
            rotation: 0.0,
            turn_speed: 0.0,
            frame: if opening { 0.0 } else { 50.0 },
            wire: 0.0,
            animated: opening,
            opened: !opening,
            closing: false,
            closed_once: false,
            started: false,
            pin: None,
            disc: !opening,
            spin: 0,
            flip: 30.0,
            flipped: 0.0,
            settled: -1,
            old_me: 0,
            mouse: [320.0, 240.0],
            held: false,
            tip: crate::kit::Tooltip::default(),
            errors: Vec::new(),
        };
        s.plan = Plan::init();
        s.fix_cursor(d);
        s.check_reports(d);
        s
    }

    /// `KigyujtKemek` (ELF 0x6b9450): the working slots in roster order onto pedestals 0..; the rest empty.
    pub fn pedestals(d: &SpyData) -> [Option<usize>; 12] {
        let mut out = [None; 12];
        let mut n = 0;
        for k in 1..=d.count() {
            if d.spy(k).state > 0 && n < 12 {
                out[n] = Some(k);
                n += 1;
            }
        }
        out
    }

    /// `KikeresKem(AktKem)`.
    pub fn selected_pedestal(&self, d: &SpyData) -> Option<usize> {
        Self::pedestals(d).iter().position(|&k| k == Some(self.akt))
    }

    /// `SpyEsemenyEllenoriz` (ELF 0x6c5d30): with no plan in progress and no dialog, show the first report: its
    /// spy becomes `AktKem` (the old one is kept for afterwards).
    fn check_reports(&mut self, d: &SpyData) {
        if self.awaiting || self.dialog != 0 || (self.plan.on && self.plan.mission > 0) || matches!(self.popup, Some(Popup { list: List::Mission, .. })) {
            return;
        }
        if let Some(r) = d.report() {
            self.backup = self.akt;
            self.dialog = r.kind;
            self.akt = r.a.clamp(0, spy::SLOTS as i32 - 1) as usize;
        }
    }

    /// The cursor fix-ups of `Spy_Main`/`FRISSIT_Spy2`: within the roster, then off empty slots (up, then down).
    fn fix_cursor(&mut self, d: &SpyData) {
        let n = d.count();
        self.akt = self.akt.min(n).max(1);
        while self.akt < n && d.spy(self.akt).state == 0 {
            self.akt += 1;
        }
        while self.akt > 0 && d.spy(self.akt).state == 0 {
            self.akt -= 1;
        }
    }

    /// The frame's start: orders sent last frame have been applied (`World::pump`), so the report on show is
    /// read again.
    pub fn applied(&mut self, d: &SpyData) {
        if self.awaiting {
            self.awaiting = false;
            self.dialog = 0;
            self.check_reports(d);
            if self.dialog == 0 {
                self.akt = self.backup.max(self.akt.min(1));
                if self.backup > 0 {
                    self.akt = self.backup;
                }
                self.fix_cursor(d);
            }
        }
    }

    /// `FRISSIT_Spy2` (ELF 0x6d1e20): one 10 ms UI frame.
    pub fn ui_frame(&mut self, d: &SpyData) {
        if self.blink_up {
            self.blink += 1;
            if self.blink > 99 {
                self.blink_up = false;
                self.blink = 100;
            }
        } else {
            self.blink -= 1;
            if self.blink < 1 {
                self.blink_up = true;
                self.blink = 0;
            }
        }
        let open = self.opened;
        if open && self.text_fade < 100.0 {
            self.text_fade += 1.0;
        }
        self.orbit += 0.0006;
        let tau = std::f64::consts::TAU;
        while self.rotation < tau {
            self.rotation += tau / 5.0;
        }
        while self.rotation >= 2.0 * tau {
            self.rotation -= tau / 5.0;
        }
        // The opening (`spyELINDULT`: after the first drawn frame) and the closing.
        if self.animated && !self.closing && !self.opened && self.started {
            self.frame = (self.frame + 1.0 / 3.0).min(50.0);
        }
        if self.animated && self.closing {
            self.frame = (self.frame - 0.6).max(0.0);
        }
        if let Some(f) = self.pin {
            self.frame = f;
        }
        if self.frame >= 25.0 && self.flip as i32 != 30 {
            self.flip += 0.2;
            if self.flip as i32 > 29 {
                self.flip = 30.0;
                self.flipped = if self.flipped == 0.0 { std::f32::consts::PI } else { 0.0 };
            }
        }
        if open {
            let [mx, my] = self.mouse;
            if my > 50.0 && my < 240.0 {
                if mx < 5.0 && self.turn_speed > -0.1 {
                    self.turn_speed -= 0.006;
                }
                if mx > 635.0 && self.turn_speed < 0.1 {
                    self.turn_speed += 0.006;
                }
            }
            self.rotation += self.turn_speed as f64 / 5.0;
            if (mx < 5.0 || mx > 635.0) && my > 49.0 && my < 241.0 {
                self.settled = self.selected_pedestal(d).map_or(-1, |p| p as i32);
            } else {
                self.turn_speed *= 0.92;
            }
            while self.rotation < tau {
                self.rotation += tau;
            }
            while self.rotation >= 2.0 * tau {
                self.rotation -= tau;
            }
        }
        if d.spy(self.akt).state == 0 {
            self.plan = Plan::delete();
        }
        self.fix_cursor(d);
        if !self.plan.on && self.dialog == 0 && d.report().is_none() && self.akt > 0 && d.spy(self.akt).state == 1 && !self.sending {
            self.plan = Plan::init();
        }
        if self.plan.on && (self.akt == 0 || d.spy(self.akt).state != 1) {
            self.plan = Plan::delete();
        }
        if d.spy(self.akt).state != 1 {
            self.sending = false;
        }
        if self.plan.on && self.plan.mission > 0 && self.plan.race > 0 && !d.alive[self.plan.race as usize] {
            self.plan.race = 0;
        }
        if self.plan.on && self.plan.mission > 0 && self.plan.race > 0 && d.mission(self.plan.mission).kind == 1 && d.planet_i32(self.plan.target, 0x20) & 0xff != self.plan.race {
            self.plan.target = 0;
        }
    }

    /// `KozelitAktKemre` (ELF 0x6baa60): turn the carousel until the selected spy stands in the middle.
    pub fn settle(&mut self, d: &SpyData, spots: &[Spot; 12]) {
        if self.frame < 50.0 {
            return;
        }
        let Some(i) = self.selected_pedestal(d) else { return };
        if self.akt != self.old_me {
            self.settled = -1;
            self.old_me = self.akt;
        }
        if self.settled == i as i32 {
            return;
        }
        let x = spots[i].screen[0];
        if (316.0..=324.0).contains(&x) {
            self.settled = i as i32;
        } else {
            self.turn_speed = (if x < 320.0 { -1.0 } else { 1.0 }) * ((x - 320.0).abs() / 320.0) * 0.1;
        }
    }

    /// The 3D pose for this frame.
    pub fn pose(&self, d: &SpyData, head: Option<TexId>) -> Pose {
        let peds = Self::pedestals(d);
        Pose {
            frame: self.frame,
            wire: self.wire,
            animated: self.animated,
            opened: self.opened,
            closing: self.closing,
            rotation: self.rotation,
            spin: self.spin,
            flip: self.flip,
            flipped: self.flipped,
            orbit: self.orbit,
            slots: std::array::from_fn(|i| peds[i].map(|k| (d.spy(k).race, d.spy(k).state))),
            selected: self.selected_pedestal(d),
            head,
        }
    }

    /// The race whose head video shows (`MakeAnimName`: the selected spy's, `ures.ani` without one).
    pub fn head_race(&self, d: &SpyData) -> u8 {
        if self.akt < 1 {
            0
        } else {
            d.spy(self.akt).race
        }
    }

    /// A new spy under the cursor starts the ring's flip (`Spying3DUpdate`: `spyAtfordulas` back to 0 when the
    /// cursor changed and the last flip ended).
    /// Also: the screen counts as open from frame 50 (`spyKINYITVA`), the ring spins past frame 25 unless closing
    /// (`spyFrameValtozo`), the sphere's wire frames run at rest (`spywireframe`).
    pub fn draw_frame(&mut self) {
        if self.frame >= 25.0 && self.akt as i32 != self.old_kem && self.flip as i32 != 0 {
            self.flip = 0.0;
            self.old_kem = self.akt as i32;
        }
        if self.frame >= 25.0 && !self.closing && self.flip as i32 != 30 && self.flip as i32 > 15 {
            self.disc = true;
        }
        if !self.opened && self.frame >= 50.0 {
            self.opened = true;
        }
        if self.frame >= 25.0 && !self.closing {
            self.spin += 1;
        }
        if self.frame == 50.0 {
            self.wire += 0.25;
            if self.wire >= 10.0 {
                self.wire = 0.0;
            }
        }
        self.started = true;
    }

    /// `Spy_Kilep`: leaving the screen. Animated, the frame closes first (`spyBECSUKAS`, frame 50 → 0); returns
    /// whether the screen can go now. The original loops `UpdateFrame` + `FRISSITES` until frame 0 was drawn once.
    pub fn leave(&mut self) -> bool {
        if !self.animated {
            return true;
        }
        self.closing = true;
        self.closed()
    }

    /// The closing has drawn frame 0 (`Spy_Kilep::voltnulla`) and one more frame.
    pub fn closed(&mut self) -> bool {
        if !self.closing || self.frame != 0.0 {
            return false;
        }
        if self.closed_once {
            return true;
        }
        self.closed_once = true;
        false
    }

    // ------------------------------------------------------------------------------------------------ input

    /// The enabled hot-spots (`Spy_MouseInit` positions, `Spy_MouseInit2` switches): id → (x, y, w, h).
    pub fn hot_spots(&self, d: &SpyData) -> Vec<(u32, [i32; 4])> {
        let mut out = Vec::new();
        let s = d.spy(self.akt);
        let mut on = |id: u32, r: [i32; 4], cond: bool| {
            if cond {
                out.push((id, r));
            }
        };
        let button = [475, 424, 60, 25];
        let ok = [565, 424, 60, 25];
        let skills = self.akt > 0 && s.points > 0 && matches!(s.state, 1..=3);
        for i in 0..4 {
            on(0x19 + i, [75, 322 + 16 * i as i32, 96, 10], skills && self.dialog == 0);
            // The skills' names: tooltips only (`Spy_MouseInit2`: on while open with a spy on show).
            on(0x22 + i, [12, 320 + 16 * i as i32, 86, 15], self.opened && self.akt > 0);
        }
        if self.dialog == 0 {
            if !self.plan.on {
                on(4, button, s.state == 3);
            } else {
                on(0xf, button, self.plan.mission > 0);
                on(0x10, [470, 269, 170, 16], true);
                let rule = d.mission(self.plan.mission);
                let can = self.plan.mission > 0 && (self.plan.race > 0 || !rule.needs_target) && self.plan.cost as f64 <= d.money;
                on(0xe, ok, can);
                on(0x11, [470, 303, 170, 16], self.plan.mission > 0 && rule.needs_target);
                on(0x12, [522, 320, 118, 16], self.plan.mission > 0 && rule.kind > 0 && (self.plan.race > 0 || self.plan.mission == 1));
                on(0x13, [522, 337, 118, 16], self.plan.mission > 0 && d.second_kind(self.plan.mission) > 0 && self.plan.race > 0);
                on(3, button, self.plan.mission < 1 && s.state > 0 && s.state != 3 && s.state != 4);
            }
        } else {
            on(0x18, ok, self.dialog != 2 && self.dialog != 3);
            on(0x21, [475, 424, 60, 25], self.repeatable(d));
            if self.dialog == 2 {
                let cost = applicant_cost(d, self.akt);
                let free = d.working < d.capacity;
                on(0x1e, [475, 298, 150, 25], ((cost as f64) <= d.money || cost == 0) && free || (!d.multiplayer && d.state.mem.i32(va::SPY_FORCED) == 3));
                on(0x1f, [475, 332, 150, 25], true);
                on(0x20, [475, 366, 150, 25], true);
            }
            if self.dialog == 3 {
                for (i, y) in [298, 332, 366, 400].into_iter().enumerate() {
                    on(0x14 + i as u32, [475, y, 150, 25], true);
                }
            }
        }
        out
    }

    /// Hot-spot 0x21 (`ÚJRA`, repeat the mission): a finished report of the spy on show, now at home, whose target
    /// and planet still stand.
    fn repeatable(&self, d: &SpyData) -> bool {
        let Some(r) = d.report() else { return false };
        let m = r.mission();
        let rule = d.mission(m);
        self.dialog == 1
            && r.b > 3
            && self.akt > 0
            && d.spy(self.akt).state == 1
            && self.akt as i32 == r.a
            && (r.x[1] == 0 || d.alive.get(r.x[1].max(0) as usize).copied().unwrap_or(false))
            && (rule.kind != 1 || r.x[2] == 0 || d.planet_i32(r.x[2], 0x20) & 0xff == r.x[1])
            && (m != 8 || r.x[2] == 0 || r.c == 0)
            && d.enabled(m)
            && r.x[8] != 999
            && r.x[8] != 0x3e5
    }

    /// `Mouse_Darabol` (ELF 0x5b7a40): the enabled hot-spot under the pointer (clamped to the screen), the lowest id
    /// of several.
    fn spot_at(&self, d: &SpyData) -> Option<u32> {
        let [x, y] = [self.mouse[0].clamp(0.0, 639.0) as i32, self.mouse[1].clamp(0.0, 479.0) as i32];
        self.hot_spots(d).into_iter().filter(|(_, r)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]).map(|(id, _)| id).min()
    }

    /// The tooltip, once per drawn frame (`UpdateFrame` ELF 0x578e40): the hot-spot under the pointer shows its
    /// text once the pointer rested 400 ms on it (or stays on the one shown); a held button hides it. It follows
    /// the pointer, kept on the 640-pixel screen (`len·6 + 0x18` wide). `now` is the clock in ms (`AktIdo`).
    pub fn tooltip(&mut self, d: &SpyData, t: &SpyTexts, now: f64) {
        let akt = self.spot_at(d).unwrap_or(0);
        let len = t.tips.get(akt as usize).map_or(0, |s| cstr(s).len());
        self.tip.mouse = self.mouse;
        self.tip.update(now, akt, len, self.held);
    }

    /// The left button went down at virtual `at` (`Mouse_Lenyom`): a hot-spot, or a spy on the carousel
    /// (`BeallitKemekAtlatszosaga`: the nearest pedestal whose box holds the pointer). Returns the orders.
    pub fn press(&mut self, d: &SpyData, t: &SpyTexts, spots: &[Spot; 12], at: [f32; 2]) -> Vec<Command> {
        self.mouse = at;
        self.held = true;
        let mut out = Vec::new();
        if self.popup.is_some() {
            return out;
        }
        match self.spot_at(d) {
            Some(id) => self.icon(d, t, id, &mut out),
            None => self.pick_spy(d, spots),
        }
        self.after(d, &mut out);
        out
    }

    /// The pointer moved (a list follows it while the button is held).
    pub fn hover(&mut self, d: &SpyData, t: &SpyTexts, at: [f32; 2]) {
        // `Mouse_Lekerdez`: a move of more than a pixel restarts the tooltip's wait.
        self.tip.moved(at);
        self.mouse = at;
        if let (Some(p), true) = (self.popup, self.held) {
            self.popup = Some(Popup { selected: self.row_at(d, t, &p), ..p });
        }
    }

    /// The left button went up (`Mouse_Felenged`): a list takes its row.
    pub fn release(&mut self, d: &SpyData, t: &SpyTexts) -> Vec<Command> {
        self.held = false;
        let mut out = Vec::new();
        if let Some(p) = self.popup.take() {
            let p = Popup { selected: self.row_at(d, t, &p), ..p };
            self.choose(d, t, p, &mut out);
        }
        self.after(d, &mut out);
        out
    }

    /// `SpyKeyboardHandler` (ELF 0x6c5770): ← the next spy, → the previous one.
    pub fn key(&mut self, d: &SpyData, left: bool) {
        if self.dialog != 0 {
            return;
        }
        let n = d.count();
        if !left && self.akt > 1 {
            let mut k = self.akt;
            loop {
                k -= 1;
                if d.spy(k).state != 0 || k <= 1 {
                    break;
                }
            }
            if d.spy(k).state != 0 {
                self.akt = k;
            }
        }
        if left && self.akt < n {
            let mut k = self.akt;
            loop {
                k += 1;
                if !(d.spy(k).state == 0 && k <= n) {
                    break;
                }
            }
            if d.spy(k).state != 0 {
                self.akt = k;
            }
        }
        self.fix_cursor(d);
    }

    fn pick_spy(&mut self, d: &SpyData, spots: &[Spot; 12]) {
        if self.frame < 50.0 || self.dialog != 0 {
            return;
        }
        let [mx, my] = self.mouse;
        let mut best: Option<(usize, f32)> = None;
        for (i, s) in spots.iter().enumerate() {
            let f = 20.0 - (s.centre[2] + 1000.0) / 40.0;
            let (x0, y0) = (s.screen[0] - (50.0 - f), s.screen[1] - (70.0 - f));
            if mx > x0 && mx < x0 + 100.0 - 2.0 * f && my > y0 && my < y0 + 180.0 - 2.0 * f && best.is_none_or(|(_, z)| s.centre[2] > z) {
                best = Some((i, s.centre[2]));
            }
        }
        let Some((i, z)) = best else { return };
        let Some(k) = Self::pedestals(d)[i] else { return };
        if z > -880.0 && d.spy(k).state > 0 {
            self.akt = k;
            if d.spy(k).state == 1 {
                self.plan = Plan::init();
            }
        }
    }

    /// `Spy_Main`'s branches for a pressed hot-spot.
    fn icon(&mut self, d: &SpyData, t: &SpyTexts, id: u32, out: &mut Vec<Command>) {
        let spy = self.akt as u8;
        match id {
            1 => self.plan = Plan::init(),
            2 => out.push(Command::Spy { order: SpyOrder::Dismiss { spy } }),
            4 => out.push(Command::Spy { order: SpyOrder::Recall { spy } }),
            0xe => {
                let p = self.plan;
                out.push(Command::Spy {
                    order: SpyOrder::Send { spy, mission: p.mission as u16, target: p.race as u8, param1: p.target as i16, param2: p.target2 as i16, days: p.days },
                });
                self.sending = true;
                self.plan = Plan::delete();
                self.dialog = 0;
            }
            0xf => {
                self.plan.mission = 0;
                self.plan.race = 0;
                self.dialog = 0;
            }
            0x10 => self.open_list(d, t, List::Mission),
            3 => self.open_list(d, t, List::Kill),
            0x11 => self.open_list(d, t, List::Race),
            0x12 | 0x13 => {
                let mode = if id == 0x13 { 2 } else { d.mission(self.plan.mission).kind };
                self.open_list(d, t, List::Target { mode, second: id == 0x13 });
            }
            0x14..=0x17 => {
                // The verdict: the roll on stream 0 and the follow-up report travel with the order (§7).
                if let Some(r) = d.report() {
                    out.push(Command::Spy { order: SpyOrder::Judge { choice: id as i32 - 0x13, owner: r.x[1] as u8, spy: r.x[2] as u8 } });
                    self.awaiting = true;
                }
            }
            0x18 => {
                out.push(Command::Spy { order: SpyOrder::Acknowledge });
                self.awaiting = true;
            }
            0x19..=0x1c => out.push(Command::Spy { order: SpyOrder::Skill { spy, skill: (id - 0x18) as u8 } }),
            0x1e..=0x20 => {
                if let Some(r) = d.report() {
                    out.push(Command::Spy { order: SpyOrder::Applicant { choice: id as i32 - 0x1d, spy: r.a as u8 } });
                    out.push(Command::Spy { order: SpyOrder::Acknowledge });
                    self.awaiting = true;
                }
            }
            0x21 => {
                if let Some(r) = d.report() {
                    let m = r.mission();
                    let cost = d.cost(m, r.x[2]) as i32;
                    if (cost as f64) > d.money {
                        self.errors.push(t.no_money_repeat.clone());
                    } else {
                        let days = if m == 1 { d.tables.spy_training_days.get(r.x[2].max(0) as usize).copied().unwrap_or(0) } else { d.mission(m).days };
                        out.push(Command::Spy {
                            order: SpyOrder::Send { spy, mission: m as u16, target: r.x[1] as u8, param1: r.x[2] as i16, param2: r.x[3] as i16, days },
                        });
                        self.sending = true;
                    }
                    out.push(Command::Spy { order: SpyOrder::Acknowledge });
                    self.awaiting = true;
                }
            }
            _ => {}
        }
    }

    /// The rows of a list in order: (value, text, colour) — `SpyGfx2D_3` draws them, `Spy_Main` hit-tests them.
    pub fn rows(&self, d: &SpyData, t: &SpyTexts, list: List) -> Vec<(i32, Vec<u8>, usize)> {
        let rank = d.spy(self.akt).rank;
        let mut out = Vec::new();
        match list {
            List::Mission => {
                for m in 1..0x1b {
                    let rule = d.mission(m);
                    if d.enabled(m) && rule.min_rank <= rank {
                        out.push((m, get(&t.missions, m as i64).to_vec(), 0));
                    }
                }
            }
            List::Race => {
                for e in d.targets() {
                    out.push((e as i32, get(&t.empires, e as i64).to_vec(), 0));
                }
            }
            List::Target { mode, .. } => {
                if mode != 5 {
                    out.push((0, Vec::new(), 0));
                }
                let race = self.plan.race;
                let en = d.en;
                match mode {
                    1 => {
                        for p in 1..=d.planets() {
                            let vis = d.state.mem.i16(va::PLANETS + (p * PLANET_SIZE) as u32 + 0xd8 + 2 * en as u32);
                            if d.planet_i32(p as i32, 0x20) & 0xff == race && vis > 0 {
                                out.push((p as i32, d.planet_name(p as i32), 0));
                            }
                        }
                    }
                    2 => {
                        for x in 0x51..0x91 {
                            if d.tech_u8(x, 0xc0 + 9 * en + race.max(0) as usize) & 1 != 0 {
                                out.push((x as i32, d.tech_name(x as i32), 0));
                            }
                        }
                    }
                    3 => {
                        for h in d.tables.hulls.iter().filter(|h| (1..0x19).contains(&h.index)) {
                            if d.tech_u8(h.tech.max(0) as usize, 0xc0 + 9 * en + race.max(0) as usize) & 1 != 0 && h.class == 1 && h.flags[2] & 1 == 0 {
                                out.push((h.index as i32, d.tech_name(h.tech), 0));
                            }
                        }
                    }
                    4 => {
                        for x in 1..0xc1 {
                            if d.tech_u8(x, 0xc0 + 9 * en + race.max(0) as usize) & 1 != 0 && d.tech_u8(x, 0x45 + en) < 5 {
                                out.push((x as i32, d.tech_name(x as i32), 0));
                            }
                        }
                    }
                    _ => {
                        for l in 1..6 {
                            out.push((l, get(&t.training, l as i64).to_vec(), 0));
                        }
                    }
                }
            }
            List::Kill => {
                // The question (row 0), then "yes" and "no".
                for (i, s) in t.sure.iter().enumerate() {
                    out.push((i as i32, s.clone(), 0));
                }
            }
        }
        out
    }

    /// Open a list at the pointer (`Spy_Main`'s first press: placed by the pointer, kept on the screen).
    fn open_list(&mut self, d: &SpyData, t: &SpyTexts, list: List) {
        let [mx, my] = [self.mouse[0] as i32, self.mouse[1] as i32];
        let (mut x, mut y, cols, rows, chars);
        match list {
            List::Mission => {
                let n = (1..0x1b).filter(|&m| d.enabled(m) && d.mission(m).min_rank <= d.spy(self.akt).rank).count() as i32;
                chars = 0x16;
                (x, y) = (mx - 0x8c, my - n * 13 / 2);
                x = x.max(6);
                y = y.max(2);
                if x + 0x88 > 0x280 {
                    x = 0x1f8;
                }
                if y + n * 13 + 2 > 0x1e0 {
                    y = 0x1de - n * 13;
                }
                (cols, rows) = (1, n);
            }
            List::Race => {
                let n = d.targets().len() as i32;
                chars = 0x18;
                (x, y) = (mx - 0x98, my - n * 13 / 2);
                x = x.max(6);
                y = y.max(2);
                if x + 0x94 > 0x280 {
                    x = 0x1ec;
                }
                if y + n * 13 + 2 > 0x1e0 {
                    y = 0x1de - n * 13;
                }
                (cols, rows) = (1, n.max(1));
            }
            List::Target { mode, .. } => {
                // Rows: "BÁRMELY" first (not for training), then the candidates; at most 32 per column.
                let n = if mode == 5 { 6 } else { self.rows(d, t, list).len() as i32 };
                let c = ((n + 0x1f) / 0x20).max(1);
                let mut r = (n + c - 1) / c;
                if mode == 5 {
                    r -= 1;
                }
                chars = 0x12;
                (x, y) = (mx - c * 0x72 - 3, my - r * 13 / 2);
                x = x.max(6);
                y = y.max(2);
                if x + c * 0x72 - 3 > 0x280 {
                    x = 0x283 - c * 0x72;
                }
                if y + r * 13 + 2 > 0x1e0 {
                    y = 0x1de - r * 13;
                }
                (cols, rows) = (c, r);
            }
            List::Kill => {
                let n = 2;
                chars = t.sure.iter().map(|x| x.len()).max().unwrap_or(0) as i32;
                (x, y) = (mx - chars * 6 - 12, my - 13);
                x = x.max(6);
                y = y.max(2);
                if x + chars * 6 + 4 > 0x280 {
                    x = 0x27c - chars * 6;
                }
                if y + n * 13 + 2 > 0x1e0 {
                    y = 0x1de - n * 13;
                }
                (cols, rows) = (1, n);
            }
        }
        let p = Popup { list, x, y, cols, rows, chars, selected: -1 };
        self.popup = Some(Popup { selected: self.row_at(d, t, &p), ..p });
    }

    /// The row under the pointer (`spySelected`): a list's value + 1 for the target lists (−1 none).
    fn row_at(&self, d: &SpyData, t: &SpyTexts, p: &Popup) -> i32 {
        let [mx, my] = [self.mouse[0] as i32, self.mouse[1] as i32];
        match p.list {
            List::Mission | List::Race => {
                let w = if p.list == List::Mission { 0x84 } else { 0x90 };
                let vals: Vec<i32> = match p.list {
                    List::Mission => (1..0x1b).filter(|&m| d.enabled(m) && d.mission(m).min_rank <= d.spy(self.akt).rank).collect(),
                    _ => d.targets().into_iter().map(|e| e as i32).collect(),
                };
                for (n, v) in vals.into_iter().enumerate() {
                    let n = n as i32;
                    if mx >= p.x && mx <= p.x + w && my >= p.y + n * 13 && my < p.y + (n + 1) * 13 {
                        return v;
                    }
                }
                -1
            }
            List::Target { mode, .. } => {
                for (n, (v, _, _)) in self.rows(d, t, p.list).into_iter().enumerate() {
                    let n = n as i32;
                    let (cx, cy) = (p.x + (n / p.rows.max(1)) * 0x72, p.y + (n % p.rows.max(1)) * 13);
                    if mx >= cx && mx <= cx + 0x6c && my >= cy && my < cy + 13 {
                        return if mode == 5 { v } else { v + 1 };
                    }
                }
                -1
            }
            List::Kill => {
                for i in 1..3 {
                    if mx >= p.x && mx <= p.x + p.chars * 6 && my >= p.y + i * 13 && my < p.y + (i + 1) * 13 {
                        return i;
                    }
                }
                -1
            }
        }
    }

    /// `Spy_Main`'s release branch of a list.
    fn choose(&mut self, d: &SpyData, t: &SpyTexts, p: Popup, out: &mut Vec<Command>) {
        let s = p.selected;
        match p.list {
            List::Mission => {
                if s > 0 && (d.money) < d.cost(s, 1) as f64 {
                    // "Nincs elég pénzed." on the message line (`ScrollHibaUzenet`).
                    self.errors.push(t.no_money.clone());
                } else if s > 0 {
                    self.plan.mission = s;
                    self.plan.target2 = 0;
                    self.plan.target = (s == 1) as i32;
                    self.plan.recompute = true;
                }
                if s < 1 {
                    self.dialog = 0;
                }
            }
            List::Kill => {
                if s == 1 {
                    out.push(Command::Spy { order: SpyOrder::Dismiss { spy: self.akt as u8 } });
                }
            }
            List::Race => {
                if s > 0 && d.races[s as usize] != 5 {
                    self.plan.race = s;
                    if d.mission(self.plan.mission).kind == 1 {
                        self.plan.target = 0;
                    }
                    self.plan.recompute = true;
                }
            }
            List::Target { mode, second } => {
                let cost = d.cost(self.plan.mission, (s - if mode == 5 { 0 } else { 1 }).max(0)) as i32;
                if (mode != 5 || cost as f64 <= d.money) && s > 0 {
                    if second {
                        self.plan.target2 = s - 1;
                    } else {
                        self.plan.target = if mode == 5 { s } else { s - 1 };
                    }
                    self.plan.recompute = true;
                }
            }
        }
    }

    /// The end of `Spy_Main`: the cursor kept on a working spy, the planner's figures recomputed.
    fn after(&mut self, d: &SpyData, out: &mut Vec<Command>) {
        self.fix_cursor(d);
        if self.plan.on && d.spy(self.akt).state != 1 {
            self.plan = Plan::delete();
        }
        if self.plan.recompute {
            self.plan.recompute = false;
            let rule = d.mission(self.plan.mission);
            if self.plan.mission < 1 || (self.plan.race < 1 && rule.needs_target) {
                (self.plan.days, self.plan.cost, self.plan.chance1, self.plan.chance2) = (-1, -1, -1, -1);
            } else {
                let m = self.plan.mission;
                self.plan.days = if m == 1 { d.tables.spy_training_days.get(self.plan.target.max(0) as usize).copied().unwrap_or(0) } else { rule.days };
                self.plan.cost = d.cost(m, self.plan.target) as i32;
                // `KemEselySzamolas1(EN, AktKem, m, t, 1000, 0)` on a copy of stream 0; the sim makes the same draws
                // when the order arrives (re/screens/spy.md §7).
                let mut rng = d.state.rng();
                let ctx = spy_context(d);
                let (k, t) = (self.akt, self.plan.race.max(0) as usize);
                self.plan.chance1 = d.spies.escape_chance(&ctx, &d.rules.spy, &mut rng, d.en, k, m as usize, t, spy::ESTIMATE_ROUNDS, spy::UI_STREAM);
                self.plan.chance2 = d.spies.success_chance(&ctx, &d.rules.spy, d.en, k, m as usize, t, d.state.kidnap().get(t).copied().unwrap_or(0));
                if rule.needs_target {
                    out.push(Command::Spy { order: SpyOrder::Estimate { spy: k as u8, mission: m as u16, target: t as u8 } });
                }
            }
        }
        self.check_reports(d);
    }
}

/// `Spy_MennyiPenzbeKerul(EN, k)` (ELF 0x6cb9c0): an applicant's wage.
fn applicant_cost(d: &SpyData, k: usize) -> i32 {
    let s = d.spy(k);
    if s.race == 14 {
        0
    } else {
        (d.rules.spy.hire_cost_mult.get(d.races[d.en] as usize).copied().unwrap_or(0.0) * 3000.0 * s.rank as f32) as i32
    }
}

/// What `KemEselySzamolas1/2` read: the races, the difficulty, MP and who is human.
fn spy_context(d: &SpyData) -> spy::SpyContext {
    let m = &d.state.mem;
    spy::SpyContext {
        alive: d.alive,
        race: d.races,
        human: std::array::from_fn(|e| m.u8(va::PLAYER_KI + e as u32) == 1),
        human_empire: d.en,
        multiplayer: d.multiplayer,
        difficulty: m.i32(va::NEHEZSEG),
        ..Default::default()
    }
}

/// The open spy screen: its state, 3D scene, texts and the head video.
pub struct SpyView {
    pub screen: SpyScreen,
    scene: crate::spy_scene::SpyScene,
    texts: SpyTexts,
    head: Option<crate::spy_scene::HeadVideo>,
    head_race: Option<u8>,
    /// When the head video started (seconds).
    head_start: f32,
    /// The remaster's clock in seconds (`sg3d_time`: the head video's scan lines), when pinned (`--acttime`).
    pub acttime: Option<f32>,
}

impl SpyView {
    pub fn open(r: &mut crate::render::Renderer, install: &ig2_formats::Install, hud: &crate::ui::Hud, d: &SpyData, opening: bool) -> Result<Self, String> {
        Ok(Self {
            screen: SpyScreen::enter(d, opening),
            scene: crate::spy_scene::SpyScene::load(r, install)?,
            texts: SpyTexts::new(&hud.texts, &hud.hu17),
            head: None,
            head_race: None,
            head_start: 0.0,
            acttime: None,
        })
    }

    /// The frame's UI work: `ui_frames` frames of `FRISSIT_Spy2`, the carousel settling on the selected spy, the
    /// head video.
    pub fn advance(&mut self, r: &mut crate::render::Renderer, install: &ig2_formats::Install, d: &SpyData, ui_frames: u64, time: f32) {
        self.screen.applied(d);
        // `Spying3DUpdate` runs per drawn frame (`spyFrameValtozo` counts them): ours once per UI frame.
        for _ in 0..ui_frames {
            self.screen.ui_frame(d);
            let spots = self.scene.spots(&self.screen.pose(d, None));
            self.screen.settle(d, &spots);
            self.screen.draw_frame();
        }
        self.screen.tooltip(d, &self.texts, time as f64 * 1000.0);
        let race = self.screen.head_race(d);
        if self.head_race != Some(race) && self.screen.flip >= 15.0 && self.screen.disc {
            self.head = Some(crate::spy_scene::HeadVideo::load(install, race));
            self.head_race = Some(race);
            self.head_start = time;
        }
        if let Some(h) = &mut self.head {
            h.show(r, self.scene.head_tex, time - self.head_start);
        }
    }

    /// The screen's frame: the 3D pass, then the 2D layers.
    pub fn build(&self, hud: &crate::ui::Hud, d: &SpyData, size: [f32; 2], hungarian: bool) -> Frame {
        let pose = self.screen.pose(d, self.head.as_ref().filter(|_| self.screen.disc).map(|_| self.scene.head_tex));
        let spots = self.scene.spots(&pose);
        let mut f = Frame::new([0.0, 0.0, 0.0, 1.0]);
        let mut scene = self.scene.build(&pose, size);
        scene.uniforms.eye[3] = self.acttime.unwrap_or(0.0);
        f.scene = Some(scene);
        if let Some(font) = &hud.font {
            let g = Draw2d { font, white: hud.white, v: crate::font::Virtual::new(size), palette: &hud.palette, ui: &hud.texts, t: &self.texts, hungarian };
            self.screen.draw(&mut f, d, &g, &spots);
        }
        f
    }

    pub fn press(&mut self, d: &SpyData, at: [f32; 2]) -> Vec<Command> {
        let spots = self.scene.spots(&self.screen.pose(d, None));
        self.screen.press(d, &self.texts, &spots, at)
    }
    pub fn hover(&mut self, d: &SpyData, at: [f32; 2]) {
        self.screen.hover(d, &self.texts, at);
    }
    pub fn release(&mut self, d: &SpyData) -> Vec<Command> {
        self.screen.release(d, &self.texts)
    }
    pub fn key(&mut self, d: &SpyData, left: bool) {
        self.screen.key(d, left);
    }
    /// Start leaving (`Spy_Kilep`): true when the screen can go at once (not animated).
    pub fn leave(&mut self) -> bool {
        self.screen.leave()
    }
    /// The closing animation is over.
    pub fn closed(&mut self) -> bool {
        self.screen.closed()
    }
    /// The lines for the message ticker since the last call.
    pub fn take_errors(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.screen.errors)
    }
}

/// `--snapshot --screen spy`: the screen as the save left it, after `--ui-frames` UI frames (the carousel settles,
/// the names fade in).
pub fn snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut crate::render::Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let en = world.curr.human as usize;
    let d = SpyData::read(&world.game.state, en, &l.rules, &l.tables);
    // Animated like the exe (`KELLKINYITASDEFAULT` 1): at rest the sphere's `MoveObj` still applies (§2.2). The
    // save's moment is the open screen, or `--spy-frame F` held while opening (closing).
    let mut v = SpyView::open(r, &l.install, hud, &d, true)?;
    match o.spy_frame {
        Some(f) => {
            v.screen.pin = Some(f);
            v.screen.frame = f;
            v.screen.closing = o.spy_closing;
            // Closing starts from the open screen (`spyKINYITVA` stays set).
            v.screen.opened = o.spy_closing;
        }
        None => {
            v.screen.frame = 50.0;
            v.screen.opened = true;
        }
    }
    // `--mouse X,Y`: the pointer rests there (a hot-spot's tooltip shows after 400 ms).
    if let Some(m) = o.mouse {
        v.screen.mouse = m;
    }
    let t = world.curr.tick as f32 * 0.05;
    v.acttime = o.acttime;
    v.advance(r, &l.install, &d, o.ui_frames.max(1) as u64, t);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let mut frame = v.build(hud, &d, size, l.install.lang() == ig2_formats::text::Lang::Hu);
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Spy, t);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    let s = &v.screen;
    println!(
        "{}: spy screen | EN {en} spy {} dialog {} plan {:?} | roster {:?} | rotation {:.3}",
        out.display(),
        s.akt,
        s.dialog,
        s.plan,
        SpyScreen::pedestals(&d),
        s.rotation
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use ig2_formats::save::SaveGame;
    use ig2_sim::spy::state;

    type Loaded = (ig2_formats::Install, SaveGame, ig2_sim::game::GameRules, ig2_rules::Rules, Vec<u8>, Vec<(u32, Vec<u8>)>);

    /// The install, quick-1.sav and its rules (skips without `IG2_INSTALL_DIR` and `IG2_SAVE_DIR`).
    fn quick1() -> Option<Loaded> {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return None;
        };
        let dir = std::env::var_os("IG2_SAVE_DIR")?;
        let bytes = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")).ok()?;
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let tables = ig2_rules::Rules::from_install(&install).unwrap();
        Some((install, SaveGame::parse(&bytes).unwrap(), rules, tables, defaults, volatile))
    }

    fn spy(name: &[u8], st: i16, race: u8, rank: u8) -> Spy {
        let mut s = Spy::default();
        s.name[..name.len()].copy_from_slice(name);
        (s.state, s.race, s.rank, s.skills, s.points, s.xp) = (st, race, rank, [60, 40, 30, 20], 2, 9000);
        s
    }

    /// The human's roster: slot 1 at home (rank 5, skill points), slot 2 at home, slot 3 an applicant; AI 3's slot 1
    /// a prisoner; the human knows AI 3 and has room for twelve spies.
    fn stage(w: &mut World) -> usize {
        let en = w.curr.human as usize;
        let mut sp = w.game.state.spies();
        sp.roster[en][1] = spy(b"Alpha", state::HOME, 1, 5);
        sp.roster[en][2] = spy(b"Bravo", state::HOME, 4, 2);
        sp.roster[en][3] = Spy { applicant: 1, ..spy(b"Charlie", state::HOME, 6, 2) };
        sp.count[en] = 3;
        sp.roster[3][1] = Spy { skills: [50, 50, 50, 1], ..spy(b"Spectre", state::CAPTURED, 3, 4) };
        sp.count[3] = sp.count[3].max(1);
        w.game.state.store_spies(&sp);
        let m = &mut w.game.state.mem;
        m.set_u8(va::EMPIRE_KNOWS + (en * 19 + 3) as u32, 1);
        m.set_i32(va::SPY_CAPACITY + 4 * en as u32, 12);
        en
    }

    struct Bench<'a> {
        rules: &'a ig2_sim::game::GameRules,
        tables: &'a ig2_rules::Rules,
        t: SpyTexts,
        en: usize,
    }

    impl Bench<'_> {
        fn data<'s>(&'s self, w: &'s World) -> SpyData<'s> {
            SpyData::read(&w.game.state, self.en, self.rules, self.tables)
        }
        /// Press the left button at `at`, move to `to`, release (virtual pixels); the orders through the lockstep
        /// queue, then a UI frame.
        fn click(&self, w: &mut World, s: &mut SpyScreen, at: [f32; 2], to: [f32; 2]) {
            let orders = {
                let d = self.data(w);
                let mut o = s.press(&d, &self.t, &[Spot::default(); 12], at);
                s.hover(&d, &self.t, to);
                o.extend(s.release(&d, &self.t));
                o
            };
            for c in orders {
                w.send(c);
            }
            w.pump();
            let d = self.data(w);
            s.applied(&d);
            s.ui_frame(&d);
        }
        /// Where row `value` of the list `at` opens is (to release on).
        fn row(&self, w: &World, s: &SpyScreen, at: [f32; 2], value: i32) -> [f32; 2] {
            let d = self.data(w);
            let mut s = s.clone();
            s.press(&d, &self.t, &[Spot::default(); 12], at);
            let p = s.popup.expect("a list is open");
            let rows = s.rows(&d, &self.t, p.list);
            let n = rows.iter().position(|r| r.0 == value).expect("the row is offered") as i32;
            let ma = p.rows.max(1);
            [(p.x + (n / ma) * 0x72 + 10) as f32, (p.y + (n % ma) * 13 + 5) as f32]
        }
    }

    /// The planner sends a mission, the kill dialog dismisses, the reports' buttons judge, hire and close: every
    /// order through `Command::Spy` into the sim.
    #[test]
    fn the_screen_drives_the_spies_through_the_lockstep() {
        let Some((install, save, rules, tables, defaults, volatile)) = quick1() else { return };
        let mut w = World::load(&save, &rules, &defaults, &volatile);
        let en = stage(&mut w);
        let b = Bench { rules: &rules, tables: &tables, t: SpyTexts::new(&crate::ui::UiTexts::load(&install), &crate::ui::hungarian_txt17(&install)), en };
        let mut s = SpyScreen::enter(&b.data(&w), false);
        assert_eq!((s.akt, s.dialog), (1, 0));
        assert!(s.plan.on, "a spy at home gets a plan");
        // The mission list (hot-spot 0x10): the empire report.
        let to = b.row(&w, &s, [480.0, 275.0], 2);
        b.click(&mut w, &mut s, [480.0, 275.0], to);
        assert_eq!(s.plan.mission, 2);
        // The target empire (0x11): AI 3; the chance estimate's stream-0 draws reach the sim with the order.
        let rng0 = w.game.state.rng().state(0);
        let to = b.row(&w, &s, [480.0, 308.0], 3);
        b.click(&mut w, &mut s, [480.0, 308.0], to);
        assert_eq!(s.plan.race, 3);
        assert!(s.plan.cost > 0 && s.plan.days > 0 && s.plan.chance1 >= 0, "{:?}", s.plan);
        assert_ne!(w.game.state.rng().state(0), rng0, "the estimate drew on stream 0");
        // OK (0xe): the spy leaves.
        let money = w.game.state.treasuries()[en].available(0);
        let cost = s.plan.cost;
        b.click(&mut w, &mut s, [570.0, 430.0], [570.0, 430.0]);
        let sp = w.game.state.spies();
        assert_eq!((sp.roster[en][1].state, sp.roster[en][1].mission, sp.roster[en][1].target), (state::MISSION, 2, 3));
        assert_eq!(w.game.state.treasuries()[en].available(0), money - cost as f64);
        // A skill point (0x19..0x1c) for the spy on show.
        let before = sp.roster[en][1].skills[0];
        b.click(&mut w, &mut s, [80.0, 325.0], [80.0, 325.0]);
        assert_eq!(w.game.state.spies().roster[en][1].skills[0], before + 1);
        // The next spy (←), then dismiss it (3, "Igen").
        {
            let d = b.data(&w);
            s.key(&d, true);
            s.ui_frame(&d);
        }
        assert_eq!(s.akt, 2);
        let to = b.row(&w, &s, [480.0, 430.0], 1);
        b.click(&mut w, &mut s, [480.0, 430.0], to);
        assert_eq!(w.game.state.spies().roster[en][2].state, state::NONE);
        // A verdict on AI 3's prisoner (dialog 3): turn him (0x15); the roll on stream 0 picks the follow-up.
        w.game.state.push_spy_report(&spy::Report { kind: 3, a: 1, x: [0, 3, 1, 0, 0, 0, 0, 0, 0], ..Default::default() });
        let mut s = SpyScreen::enter(&b.data(&w), false);
        assert_eq!(s.dialog, 3);
        b.click(&mut w, &mut s, [480.0, 340.0], [480.0, 340.0]);
        let follow = s.dialog;
        assert!(matches!(follow, 8..=11), "follow-up {follow}");
        assert_eq!(Report::read(&w.game.state.spy_reports(en, en)).map(|r| r.kind), Some(follow));
        // OK closes it.
        b.click(&mut w, &mut s, [570.0, 430.0], [570.0, 430.0]);
        assert!(w.game.state.spy_reports(en, en).is_empty());
        assert_eq!(s.dialog, 0);
        // An applicant (dialog 2): hire (0x1e).
        w.game.state.push_spy_report(&spy::Report { kind: 2, a: 3, x: [0, en as i32, 0, 0, 0, 0, 0, 0, 0], ..Default::default() });
        let mut s = SpyScreen::enter(&b.data(&w), false);
        assert_eq!((s.dialog, s.akt), (2, 3));
        b.click(&mut w, &mut s, [480.0, 305.0], [480.0, 305.0]);
        assert_eq!(w.game.state.spies().roster[en][3].applicant, 0);
        assert!(w.game.state.spy_reports(en, en).is_empty());
    }

    /// `KemEselySzamolas1` with no counter-spies: every round gets through; a mission without a target: 100 and no
    /// draws.
    #[test]
    fn the_estimate_without_guards_is_certain() {
        let mut sp = Spies::default();
        sp.roster[1][1] = Spy { state: state::HOME, skills: [50, 0, 0, 0], ..Default::default() };
        let rules = spy::SpyRules { missions: (0..14).map(|m| spy::MissionRule { needs_target: m == 2, ..Default::default() }).collect(), ..Default::default() };
        let ctx = spy::SpyContext::default();
        let mut rng = ig2_sim::rng::Streams::default();
        assert_eq!(sp.escape_chance(&ctx, &rules, &mut rng, 1, 1, 2, 3, 1000, 0), 100);
        assert_ne!(rng.state(0), 0, "two draws a round");
        let mut quiet = ig2_sim::rng::Streams::default();
        assert_eq!(sp.escape_chance(&ctx, &rules, &mut quiet, 1, 1, 1, 3, 1000, 0), 100);
        assert_eq!(quiet.state(0), 0);
    }

    /// `Spy_MouseInit`'s tooltip indices (the asm's `GetString(7, 1, k)` immediates).
    #[test]
    fn tooltips_follow_the_mouse_init() {
        let k: Vec<u16> = [1, 4, 5, 0xd, 0xe, 0x10, 0x14, 0x17, 0x18, 0x19, 0x1c, 0x1d, 0x1e, 0x21, 0x22, 0x25, 0x26].iter().map(|&id| tip_index(id)).collect();
        assert_eq!(k, [1, 4, 0, 0, 5, 7, 11, 14, 15, 16, 16, 0, 17, 20, 21, 24, 0]);
    }
}
