//! The trade screen (`AktKep` 8, re/screens/trade.md), part 1: the market browsed and bought from. `Trade_Belep`
//! (ELF 0x7ace10 / exe 0x60a1e0) and `Trade_Kilep` (0x7ad040 / exe 0x60a630) with the opening and closing
//! (`traframe`), the per-tick `FRISSIT_Trade2` (0x7b0180 / exe 0x605b60) and the state half of `Trade3DUpdate`
//! (0x7abfc0 / exe 0x608340), the hot-spots of `Trade_MouseInit`/`MouseInit2` (exe 0x60b110 / 0x60aed0), the input
//! `Trade_Main` (exe 0x60a7f0: the two lists, their scroll bars, the quantity, the destination pick list, OK), and the
//! 2D passes: `GFX_Trade`'s boxes (exe 0x606b50), `TradeGfx2D_2` (exe 0x608c70: the lists, the purchase line, the
//! deliveries), `TradeGfx2D_3` (exe 0x6098a0: the destinations) and the bottom ticker (`TozsdeScrollFrissit*`).
//! OK sends message 0x51 (`MULTIGET_Trade`, [`ig2_net::Command::Buy`]). The 3D frame is [`crate::trade_scene`].
//!
//! Not yet (the roadmap's "Trade screen part 2"): the chosen offer's hull model (`TradeLoadAlap`'s file table,
//! `ForgatTradeAlap`'s turn and fade are kept as state only), its info card (`PlaceVehicleInfo`, block C's boxes) and
//! the pod list (`Trade_KiirPodNagyLista`, `TradeGfx2D_3`'s box), the tutorial event 0x27.

use ig2_rules::sounds::Table;
use ig2_sim::save::{va, SaveState};
use ig2_sim::trade::{Record, DELIVERY, OFFERS, RECORD, TRADE_PORT};

use crate::audio::{SoundId, CLICK};
use crate::font::Align;
use crate::kit::{DesignFrame, Tooltip};
use crate::render::Frame;
use crate::starmap_panels::Draw2d;

/// `TradeWhichVehicle` with nothing chosen.
pub const NONE: i32 = 50000;
/// Rows each list shows.
const ROWS: i32 = 8;
/// `Gombokx1/y1`, `Gombokx2/y2` (exe 0x928da0..): the purchase line's two halves.
const G1: [i32; 2] = [137, 195];
const G2: [i32; 2] = [352, 195];
/// `SzallX` (exe 0x928dc8): the deliveries' first column.
const SZALL_X: f32 = 270.0;
/// `TRADEXAMAX` (exe 0x928ae8).
const XA_MAX: f32 = 185.0;
/// The lists' place (`TradeDisplayVehicle(0x69 | 0x15b, 0x3f, …)`) and row height.
const LIST_X: [i32; 2] = [0x69, 0x15b];
const LIST_Y: i32 = 0x3f;
const ROW_H: i32 = 11;
/// `Bolygok` stride and the owner byte.
const PLANET_SIZE: u32 = 0x188;
/// The destination list's row height and the width in characters (`traSelectChr`).
const SEL_ROW: i32 = 13;
const SEL_CHR: i32 = 0x16;

/// `Trade_Hangok` (`InitSounds(Trade_Hangok, 2)`): screen19 (opening), scrn19ki (closing).
fn tra(i: u16) -> SoundId {
    SoundId::new(Table::Trade, i)
}

/// `Trade_MouseInit`'s hot-spots (`Mouse_Szoveg_Set(id, 0, x, y, w, h, GetString(8, 1, tip), release, 1)`):
/// `(id, rect, tip, fires on the release)`. 0x25's x follows the hull model (`Trade_MouseInit2`).
pub const SPOTS: [(u32, [i32; 4], u16, bool); 9] = [
    (0x19, [G1[0] + 0xf, G1[1] + 0x14, 0x28, 0xf], 1, false),
    (0x1a, [G1[0] + 0x41, G1[1] + 0x14, 0x28, 0xf], 2, false),
    (0x1b, [G2[0] - 5, G1[1] + 0x14, 0x37, 0xf], 3, false),
    (0x1c, [G1[0] + 0x73, G1[1] + 0x14, 0x28, 0xf], 4, true),
    (0x21, [300, 0x3f, 0x12, 0x56], 5, false),
    (0x22, [0x143, 0x3f, 0x12, 0x56], 5, false),
    (0x23, [0x69, 0x3f, 0xbb, 0x58], 6, false),
    (0x24, [0x15b, 0x3f, 0xbb, 0x58], 7, false),
    (0x25, [200, 300, 0xf0, 0xaa], 8, false),
];

/// The tooltip texts by hot-spot id (`GetString(8, 1, tip)`).
pub fn tips(texts: &crate::ui::UiTexts) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(); 0x26];
    for &(id, _, k, _) in &SPOTS {
        out[id as usize] = texts.get(8, 1, k);
    }
    out
}

/// The ticker's lines (`ScrollSorok`, `TozsdeScrollLoad` exe 0x607fe0): line 0 asks for a trade port.
pub fn ticker_lines(texts: &crate::ui::UiTexts) -> Vec<Vec<u8>> {
    vec![texts.get(8, 4, 1)]
}

fn en(state: &SaveState) -> usize {
    state.mem.i32(va::EN).clamp(0, 18) as usize
}

fn i16_at(r: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([r[o], r[o + 1]])
}

fn i32_at(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}

/// The market as the screen reads it: `TradeTervek[0..=40]` (the lookups read one past the offers, as the exe's do)
/// and `TradeHanyTradeCuccVan`.
pub struct Market {
    pub offers: Vec<Record>,
    pub n: usize,
}

impl Market {
    pub fn of(state: &SaveState) -> Self {
        let n = (state.mem.i32(va::TRADE_OFFER_COUNT).max(0) as usize).min(OFFERS);
        let offers = (0..=OFFERS).map(|i| state.mem.read(va::TRADE_OFFERS + (i * RECORD) as u32, RECORD).try_into().unwrap()).collect();
        Self { offers, n }
    }

    fn class(&self, i: usize) -> i16 {
        self.offers.get(i).map_or(0, |r| i16_at(r, 0x16))
    }

    /// `TradeNextTerv(t, from)` (ELF 0x7a71c0): the first offer from `from` on of class `t`, `N` if none (the class
    /// is tested before the bound).
    fn next(&self, t: i16, from: usize) -> usize {
        let mut i = from;
        while i <= OFFERS && self.class(i) != t && i < self.n {
            i += 1;
        }
        i
    }

    /// The offers of class 1 and 2 (`TradeCalculateVehicleNum`'s counts).
    fn counts(&self) -> [i32; 2] {
        let mut c = [0; 2];
        for i in 0..self.n {
            match self.class(i) {
                1 => c[0] += 1,
                2 => c[1] += 1,
                _ => {}
            }
        }
        c
    }

    /// `TradeAr(i, n)` (exe 0x608830).
    fn price(&self, i: usize, n: i32) -> i32 {
        self.offers.get(i).map_or(0, |r| ig2_sim::trade::price(r, n))
    }
}

/// The human's planets with a working trade port, or all of them with `DEBUGX` or `TradeCheat` (`HanyTradePortVanAkarmi`
/// exe 0x606f80).
pub fn ports(state: &SaveState) -> Vec<i32> {
    let e = en(state);
    let n = state.mem.i32(va::PLANET_COUNT).max(0);
    let any = state.mem.u8(va::DEBUGX) != 0 || state.mem.u8(va::TRADE_CHEAT) != 0;
    (1..=n).filter(|&p| state.mem.u8(va::PLANETS + p as u32 * PLANET_SIZE + 0x20) as usize == e && (any || state.working_buildings(p as usize, TRADE_PORT) > 0)).collect()
}

/// `Bolygok[p]`'s name.
fn planet_name(state: &SaveState, p: i32) -> Vec<u8> {
    let mut s = state.mem.read(va::PLANETS + p.max(0) as u32 * PLANET_SIZE, 0x20);
    s.truncate(s.iter().position(|&b| b == 0).unwrap_or(s.len()));
    s
}

/// `MoneyMennyi(EN, 0)`.
fn money(state: &SaveState) -> f64 {
    state.mem.f64(va::MONEY + 8 * en(state) as u32)
}

/// The pointer between two UI frames: where it is, the buttons held (`Mouse_Bal`, `Mouse_Jobb`) and this frame's
/// edges (`Mouse_Lenyom`, `Mouse_Felenged`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Mouse {
    pub at: [f32; 2],
    pub left: bool,
    pub right: bool,
    pub press: bool,
    pub release: bool,
}

/// The screen's state (the exe's globals, re/screens/trade.md §2).
#[derive(Debug, Clone)]
pub struct Trade {
    /// `traframe` (0..=50), `traKINYITVA`, `traBECSUKAS`, `traKELLKINYITAS`, `traELINDULT`.
    pub frame: f32,
    pub opened: bool,
    pub closing: bool,
    leaving: bool,
    pub animated: bool,
    started: bool,
    /// `TradeElottiKep` and, while closing, `traKilepesHova`: 9 hides the frame's part 4.
    pub before: i32,
    pub towards: i32,
    /// `trakeringeto`, `trawireframe`.
    pub keringeto: f32,
    pub wire: f32,
    /// `TRADExa` (the model and card block's slide: 0 with deliveries under way).
    pub xa: f32,
    /// `TradeWhichVehicle` (`NONE`), `TradeWhichVehicleType`, `TradeVehicleOld`/`OldType`.
    pub which: i32,
    pub kind: usize,
    old: i32,
    old_kind: usize,
    /// `TradeVehiclePos` per list, `TradeAktualisCucc`, `TradeVehicleMennyi`, `…MennyiOsszesen`,
    /// `TradeActVehiclePrice`.
    pub pos: [i32; 2],
    pub current: usize,
    pub qty: i32,
    pub left: i32,
    pub unit_price: i32,
    /// `TradeSelectedPlanet`, `…Temp`, `AktEtaDays2`.
    pub planet: i32,
    planet_temp: i32,
    pub eta: i32,
    /// `SelectDestinationMode`, `traSelected`, `traSelectX/Y`.
    pub selecting: bool,
    selected: i32,
    sel_at: [i32; 2],
    /// The hull model's bookkeeping (part 2 draws it): loaded, `init`, `dir`, done, `angle`, its target,
    /// `traOldTervmut`, `traLastTransparency`.
    hull: bool,
    hull_init: bool,
    hull_dir: bool,
    hull_done: bool,
    angle: f32,
    angle_to: f32,
    pub shown_offer: i32,
    pub last_transparency: f32,
    /// The ticker: `ScrollQueue[1..]`, `ScrollHova`, its lines, the font's advance (`CharXMeret[ScrollFont]`).
    queue: Vec<usize>,
    hova: f32,
    lines: Vec<Vec<u8>>,
    advance: f32,
    /// The pointer and this frame's edges, the hot-spot `Mouse_Megfogva` holds, the release box pressed.
    pub mouse: Mouse,
    held: Option<u32>,
    armed: Option<u32>,
    /// The hot-spots `Trade_MouseInit2` leaves on, and 0x25's x.
    on: [bool; 0x26],
    pod_x: i32,
    pub list_frame: DesignFrame,
    pub tip: Tooltip,
    pub frames: u64,
    pub sounds: Vec<SoundId>,
    /// Message 0x51's payloads (`TradeSzallitasTemp`, 0x98 bytes) sent this frame, and the human's delivery count when
    /// the last one went (its acceptance shows as a new delivery: `MULTIGET_Trade`'s screen side, [`Trade::bought`]).
    pub orders: Vec<Vec<u8>>,
    pending: Option<i32>,
}

impl Trade {
    /// `Trade_Belep`: `animated` = `traKELLKINYITAS` (not coming back from the messages, help or options screen),
    /// `before` = `OldKep`, `advance` = the ticker font's character width.
    pub fn open(state: &SaveState, animated: bool, before: i32, lines: Vec<Vec<u8>>, advance: f32) -> Self {
        let deliveries = state.mem.i32(va::TRADE_DELIVERY_COUNTS + 4 * en(state) as u32);
        Self::with(animated, before, lines, advance, deliveries, !ports(state).is_empty())
    }

    /// [`Trade::open`] on the human's delivery count and whether it has a trade port.
    fn with(animated: bool, before: i32, lines: Vec<Vec<u8>>, advance: f32, deliveries: i32, port: bool) -> Self {
        let xa = if deliveries < 1 { XA_MAX } else { 0.0 };
        let mut s = Self {
            frame: if animated { 0.0 } else { 50.0 },
            opened: !animated,
            closing: false,
            leaving: false,
            animated,
            started: false,
            before,
            towards: 0,
            keringeto: 0.0,
            wire: 0.0,
            xa,
            which: NONE,
            kind: 0,
            old: -1,
            old_kind: usize::MAX,
            pos: [0; 2],
            current: 1,
            qty: 0,
            left: 0,
            unit_price: 0,
            planet: -1,
            planet_temp: -1,
            eta: 0,
            selecting: false,
            selected: -1,
            sel_at: [90, 28],
            hull: false,
            hull_init: false,
            hull_dir: false,
            hull_done: true,
            angle: 0.0,
            angle_to: 0.0,
            shown_offer: -1,
            last_transparency: 0.0,
            queue: Vec::new(),
            hova: 0.0,
            lines,
            advance,
            mouse: Mouse { at: [320.0, 240.0], ..Mouse::default() },
            held: None,
            armed: None,
            on: [false; 0x26],
            pod_x: 200,
            list_frame: DesignFrame::default(),
            tip: Tooltip::default(),
            frames: 0,
            sounds: Vec::new(),
            orders: Vec::new(),
            pending: None,
        };
        // `TozsdeScroll(0, 2)` without a trade port: line 0 twice.
        if !port {
            s.queue.extend([0, 0]);
        }
        if animated {
            s.sounds.push(tra(0));
        }
        s
    }

    /// `Trade_Kilep` towards another game screen `to`: the closing runs out (`traframe` down by 0.6 a tick) before
    /// the screen goes; true = gone now (towards the messages, help or options screen it goes at once, silently).
    pub fn leave(&mut self, to: i32) -> bool {
        if !self.animated || matches!(to, 10 | 1 | 0xd) {
            return true;
        }
        if !self.closing && !self.leaving {
            self.leaving = true;
            self.towards = to;
            self.sounds.push(tra(1));
        }
        false
    }

    /// The closing has finished.
    pub fn closed(&self) -> bool {
        self.closing && self.frame <= 0.0
    }

    /// Hold the screen at rest (snapshots).
    pub fn hold_open(&mut self) {
        self.frame = 50.0;
        self.opened = true;
        self.started = true;
    }

    /// `Trade3DUpdate` step 7: the frame's part 4 is hidden while the info screen is the neighbour.
    pub fn hides_early(&self) -> bool {
        (self.closing && self.towards == 9) || (!self.opened && self.before == 9)
    }

    /// `Trade3DUpdate` step 4: the wire sphere's place `(x, y, z)` and turn.
    pub fn wire_pose(&self) -> ([f32; 3], crate::scene::M3) {
        if !self.animated {
            return ([0.0, 0.0, 200.0], crate::scene::create_matrix(0.0, 0.0, 0.0));
        }
        let x = 1.0 - self.frame / 50.0;
        // `(float)sin((double)(x·2.0f))·3.0f` in f32, `(float)(sin((double)x·2.1)·2.0)` in f64.
        let a = (((x * 2.0) as f64).sin() as f32) * 3.0;
        let b = ((x as f64 * 2.1).sin() * 2.0) as f32;
        ([0.0, x * 200.0, 200.0 - x * 1000.0], crate::scene::create_matrix(a, b, 0.0))
    }

    /// `Trade3DUpdate` step 4: the wire sphere's transparency.
    pub fn wire_transparency(&self) -> f32 {
        if self.frame <= 25.0 {
            let v = 1.35 - (self.frame as f64 * 2.0) / 50.0;
            if v < 0.0001 {
                1e-6
            } else if v > 0.9 {
                0.999_999_9
            } else {
                v as f32
            }
        } else {
            1e-6
        }
    }

    /// `KeresAktualisTervSorszamat` (exe 0x607020): the chosen row's offer, −1 without one.
    fn offer(&self, m: &Market) -> i32 {
        if self.which == NONE {
            return -1;
        }
        let t = self.kind as i16 + 1;
        let mut i = m.next(t, 0);
        for _ in 0..self.which.max(0) {
            i = m.next(t, i + 1);
        }
        i as i32
    }

    /// The OK button's condition (`Trade_MouseInit2`, `GFX_Trade`, `TradeGfx2D_2`): a destination, a quantity, and
    /// `(double)TradeAr(offer, qty) <= MoneyMennyi(EN, 0)`.
    fn can_buy(&self, state: &SaveState, m: &Market) -> bool {
        self.planet != -1 && self.qty != 0 && m.price(self.offer(m).max(0) as usize, self.qty) as f64 <= money(state)
    }

    /// `TradeSelectShip` / `TradeSelectTank` (exe 0x60a080 / 0x60a130).
    fn select(&mut self, m: &Market, counts: [i32; 2]) {
        let c = counts[self.kind];
        if self.which >= c {
            self.which = c - 1;
        }
        if c == 0 {
            self.which = NONE;
        }
        if self.which == NONE {
            self.left = 0;
        } else {
            let i = self.offer(m).max(0) as usize;
            self.left = m.offers.get(i).map_or(0, |r| i32_at(r, 0x6c));
            self.unit_price = m.price(i, 1);
            self.current = i;
        }
        self.qty = 0;
    }

    /// `TradeLoadAlap(base, race)`'s state (exe 0x609c60): a new hull model (part 2 loads and draws it).
    fn load_hull(&mut self, m: &Market) {
        if self.which == NONE {
            return;
        }
        self.hull = true;
        self.hull_init = true;
        self.shown_offer = self.offer(m);
    }

    /// The pointer moved.
    pub fn moved(&mut self, at: [f32; 2]) {
        self.mouse.at = at;
        self.tip.moved(at);
    }

    /// A button went down (`left` or the right one).
    pub fn press(&mut self, left: bool, at: [f32; 2]) {
        self.moved(at);
        self.mouse.press = true;
        if left {
            self.mouse.left = true;
        } else {
            self.mouse.right = true;
        }
    }

    /// A button went up.
    pub fn release(&mut self, left: bool, at: [f32; 2]) {
        self.moved(at);
        self.mouse.release = true;
        if left {
            self.mouse.left = false;
        } else {
            self.mouse.right = false;
        }
    }

    /// The hot-spot under `at` that is on (`AktIkon2`; `Mouse_Darabol`: the lowest id wins, `x ≤ px ≤ x + w − 1`
    /// with the pointer clamped to the screen).
    pub fn spot(&self, at: [f32; 2]) -> u32 {
        let (x, y) = ((at[0] as i32).clamp(0, 639), (at[1] as i32).clamp(0, 479));
        for &(id, r, _, _) in &SPOTS {
            let r = if id == 0x25 { [self.pod_x, r[1], r[2], r[3]] } else { r };
            if self.on[id as usize] && x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3] {
                return id;
            }
        }
        0
    }

    /// `Trade_MouseInit2` (every frame): which hot-spots are on, and 0x25 following the hull model.
    fn mouse_init2(&mut self, state: &SaveState, m: &Market) {
        let c = (self.angle as f64).cos() as f32;
        let off = if self.hull_dir { (1.0 - c) * 300.0 } else { (1.0 - c) * -200.0 };
        self.pod_x = (off as i32 as f32 + self.xa) as i32;
        self.on = [false; 0x26];
        for id in [0x1b, 0x21, 0x22, 0x23, 0x24] {
            self.on[id] = true;
        }
        if self.which != NONE {
            for id in [0x19, 0x1a, 0x25] {
                self.on[id] = true;
            }
            self.on[0x1c] = self.can_buy(state, m);
        }
        if !self.opened || self.closing {
            self.on = [false; 0x26];
        }
    }

    /// One UI frame: `FRISSIT_Trade2`, `Trade_MouseInit2`, the state half of `Trade3DUpdate`, then `Trade_Main` on
    /// the frame's input (the exe runs the last three per drawn frame; one frame a tick here, as the captures run).
    pub fn step(&mut self, state: &SaveState) {
        self.frames += 1;
        let m = Market::of(state);
        let e = en(state);
        let deliveries = state.mem.i32(va::TRADE_DELIVERY_COUNTS + 4 * e as u32);
        let eta_days = state.mem.i32(va::TRADE_ETA_DAYS);
        if self.pending.is_some_and(|c| c != deliveries) {
            self.pending = None;
            self.bought(state);
        }
        // `FRISSIT_Trade2`: f64 steps on f32 globals.
        let add = |v: f32, d: f64| (v as f64 + d) as f32;
        if !self.queue.is_empty() {
            self.hova = add(self.hova, 0.8);
        }
        let xa_to = if deliveries < 1 { XA_MAX } else { 0.0 };
        if xa_to == 0.0 && self.xa != 0.0 {
            self.xa = add(self.xa, -3.6).max(0.0);
        } else if xa_to == XA_MAX && self.xa != XA_MAX {
            self.xa = add(self.xa, 3.6).min(XA_MAX);
        }
        self.keringeto = add(self.keringeto, 0.0006);
        if self.animated && !self.closing && !self.opened && self.started {
            self.frame = add(self.frame, 4.0 / 11.0).min(50.0);
        }
        if self.animated && self.closing {
            self.frame = add(self.frame, -0.6).max(0.0);
        }
        if self.angle_to != self.angle {
            if self.angle < self.angle_to {
                self.angle = add(self.angle, 0.05).min(self.angle_to);
            }
            if self.angle_to < self.angle {
                self.angle = add(self.angle, -0.05).max(self.angle_to);
            }
        }
        let counts = m.counts();
        // The offers moved under the selection (a sale, an expiry, a new offer): the same offer is chosen again.
        let cur_class = m.class(self.current);
        if self.which != NONE && self.offer(&m) != self.current as i32 && cur_class > 0 {
            self.old_kind = (cur_class - 1) as usize;
            self.which = (0..self.current).filter(|&i| m.class(i) - 1 == cur_class - 1).count() as i32;
            self.old = self.which;
            self.kind = self.old_kind;
        }
        for (p, &c) in self.pos.iter_mut().zip(&counts) {
            if *p > c - ROWS {
                *p = c - ROWS;
            }
            *p = (*p).max(0);
        }
        self.list_frame.tick();
        // `Trade_MouseInit2` (before `GFX_Trade` in `UpdateFrame`).
        self.mouse_init2(state, &m);
        // `GFX_Trade`: a destination that lost its port or owner is dropped; `TradeCalculateVehicleNum`.
        if self.planet != -1 && !ports(state).contains(&self.planet) {
            self.planet = -1;
        }
        if self.opened && !self.closing {
            for (p, &c) in self.pos.iter_mut().zip(&counts) {
                if c < *p + ROWS {
                    *p -= ROWS;
                }
                *p = (*p).max(0);
            }
        }
        // `Trade3DUpdate`.
        if !self.opened && self.frame >= 50.0 {
            self.opened = true;
        }
        self.started = true;
        if self.frame == 50.0 {
            self.wire = add(self.wire, 0.25);
            if self.wire >= 10.0 {
                self.wire = 0.0;
            }
        }
        // `TozsdeScrollFrissit`: a line that has run past the left edge leaves the queue.
        if let Some(&l) = self.queue.first() {
            let len = self.lines.get(l).map_or(0, |s| s.len()) as f32 * self.advance;
            if self.hova > len + 640.0 {
                self.hova = 0.0;
                self.queue.remove(0);
            }
        }
        // `ForgatTradeAlap`.
        if self.hull {
            if self.hull_init {
                self.angle_to = 0.0;
                self.angle = std::f32::consts::FRAC_PI_2;
                self.hull_init = false;
                self.hull_dir = deliveries < 1;
            }
            let c = (self.angle as f64).cos() as f32;
            let s = if self.hull_dir { (1.0 - c) * 300.0 } else { (1.0 - c) * -200.0 };
            let s0 = if self.hull_dir { (1.0 - std::f64::consts::FRAC_PI_2.cos()) * 300.0 } else { (1.0 - std::f64::consts::FRAC_PI_2.cos()) * -200.0 };
            self.last_transparency = s / s0 as f32;
        }
        // Step 11: the turned-away model is replaced once it arrived.
        if !self.hull_done && self.angle == self.angle_to {
            self.hull_done = true;
            self.load_hull(&m);
        }
        // `TradeDrawSelection` asks for the row frame (`TradeGfx2D_2`, while open).
        if self.opened && !self.closing && self.which != NONE {
            let p = self.pos[self.kind];
            if (p..p + ROWS).contains(&self.which) {
                self.list_frame.ask([LIST_X[self.kind] - 4, LIST_Y + (self.which - p) * ROW_H - 1, 0xc4, 10]);
            }
        }
        self.main(state, &m, counts, deliveries, eta_days);
        if self.leaving {
            self.leaving = false;
            self.closing = true;
        }
    }

    /// `Trade_Main` (exe 0x60a7f0) on this frame's input; `AktIkon` is the hot-spot under a held or pressed button
    /// (`Mouse_Megfogva` keeps the one a handler grabbed). A release box (OK) fires on the press and then gives
    /// nothing until the button is up (`Mouse_FelengedesMajdMegvar`) [obs: the exe's Click3 on OK's press].
    fn main(&mut self, state: &SaveState, m: &Market, counts: [i32; 2], deliveries: i32, eta_days: i32) {
        let ms = self.mouse;
        let under = self.spot(ms.at);
        let release_box = |id: u32| SPOTS.iter().any(|&(i, _, _, r)| i == id && r);
        let ikon = if let Some(h) = self.held {
            h
        } else if self.armed.is_some() {
            0
        } else if ms.press || ((ms.left || ms.right) && !release_box(under)) {
            under
        } else {
            0
        };
        if ms.press && release_box(under) {
            self.armed = Some(under);
        }
        if !ms.left && !ms.right {
            self.armed = None;
        }
        let (press, release) = (ms.press, ms.release);
        let (bal, jobb) = (ms.left || (ms.press && ms.release), ms.right);
        let mut grab = None;
        if release {
            // The pod list closes with any release (part 2 draws it).
        }
        match ikon {
            0x19 if !release => {
                if press {
                    self.sounds.push(CLICK);
                }
                if self.qty != 0 && ((press && bal) || jobb) {
                    self.qty -= 1;
                }
            }
            0x1a if !release => {
                if press {
                    self.sounds.push(CLICK);
                }
                if self.qty < self.left && ((press && bal) || jobb) {
                    self.qty += 1;
                }
            }
            0x1b if press || self.selecting => {
                if press {
                    self.sounds.push(CLICK);
                }
                if !release {
                    let ports = ports(state);
                    let at = [ms.at[0] as i32, ms.at[1] as i32];
                    if !self.selecting {
                        let n = ports.len().max(1) as i32;
                        let mut sel = [at[0] + 12, at[1] - (n * SEL_ROW) / 2];
                        if at[0] + 6 < 0 {
                            sel[0] = 6;
                        }
                        if sel[1] - 2 < 0 {
                            sel[1] = 2;
                        }
                        if sel[0] + SEL_CHR * 6 + 4 > 0x280 {
                            sel[0] = 0x27c - SEL_CHR * 6;
                        }
                        if sel[1] + n * SEL_ROW + 2 > 0x1e0 {
                            sel[1] = 0x1de - n * SEL_ROW;
                        }
                        self.sel_at = sel;
                    }
                    self.selected = -1;
                    for (j, &p) in ports.iter().enumerate() {
                        let y = self.sel_at[1] + j as i32 * SEL_ROW;
                        if (self.sel_at[0]..=self.sel_at[0] + 0x84).contains(&at[0]) && (y..y + SEL_ROW).contains(&at[1]) {
                            self.selected = j as i32;
                            self.planet_temp = p;
                        }
                    }
                    self.selecting = true;
                    grab = Some(0x1b);
                } else {
                    self.selecting = false;
                    self.planet = self.planet_temp;
                    self.eta = eta_days;
                }
            }
            0x1c => {
                if press {
                    self.sounds.push(SoundId::global(2));
                }
                let i = self.offer(m);
                let mut msg = Vec::with_capacity(DELIVERY);
                for v in [self.planet, self.eta, i, self.qty, m.price(i.max(0) as usize, self.qty)] {
                    msg.extend(v.to_le_bytes());
                }
                msg.extend_from_slice(m.offers.get(i.max(0) as usize).map_or(&[0; RECORD][..], |r| &r[..]));
                self.orders.push(msg);
                self.pending = Some(deliveries);
            }
            0x21 | 0x22 => {
                let k = (ikon - 0x21) as usize;
                // f32: `(int)(((float)my − 63.0)/86.0·(float)count − 4.0)`.
                let p = ((ms.at[1] as i32 as f32 - 63.0) / 86.0 * counts[k] as f32 - 4.0) as i32;
                self.pos[k] = p.max(0);
                if counts[k] < self.pos[k] + ROWS {
                    self.pos[k] = counts[k] - ROWS;
                }
                grab = Some(ikon);
            }
            0x23 | 0x24 if counts[(ikon - 0x23) as usize] > 0 => {
                if press {
                    self.sounds.push(CLICK);
                }
                let k = (ikon - 0x23) as usize;
                self.kind = k;
                let mut sel = self.pos[k] + (ms.at[1] as i32 - LIST_Y) / ROW_H;
                self.fit_row(k, &mut sel, counts[k]);
                self.which = sel;
                self.select(m, counts);
                grab = Some(ikon);
            }
            _ => {}
        }
        // A new row: the model is loaded at once the first time, else the shown one turns away first.
        if matches!(ikon, 0x23 | 0x24) && (bal || jobb) && !release && (self.old != -1 || self.which != NONE) && (self.which != self.old || self.old_kind != self.kind) {
            if self.old == -1 {
                self.load_hull(m);
            } else {
                self.hull_done = false;
                self.angle_to = std::f32::consts::FRAC_PI_2;
                self.hull_dir = deliveries < 1;
            }
            self.old = self.which;
            self.old_kind = self.kind;
        }
        if let Some(g) = grab {
            self.held = Some(g);
        }
        if !ms.left && !ms.right {
            self.held = None;
        }
        self.mouse.press = false;
        self.mouse.release = false;
    }

    /// `TradeIgazitShipPoz` / `TankPoz` (ELF 0x7ad8d0 / 0x7ada00): a row outside the window scrolls it by one.
    fn fit_row(&mut self, k: usize, sel: &mut i32, count: i32) {
        let p = &mut self.pos[k];
        if *sel >= *p + ROWS || *sel < *p {
            if *sel < *p {
                *p -= 1;
                *sel = *p;
            }
            if *sel > *p + 7 {
                *sel = *p + 8;
                *p += 1;
            }
            if *sel >= *p + ROWS {
                *sel = *p + 7;
            }
            if *p > count - ROWS {
                *p = count - ROWS;
            }
            if *sel < 0 {
                *sel = 0;
            }
            if *p < 0 {
                *p = 0;
            }
        }
    }

    /// `MULTIGET_Trade`'s screen side for the human's accepted purchase (re/screens/trade.md §6.3).
    pub fn bought(&mut self, state: &SaveState) {
        let m = Market::of(state);
        let deliveries = state.mem.i32(va::TRADE_DELIVERY_COUNTS + 4 * en(state) as u32);
        let i = self.offer(&m);
        if i >= 0 && m.class(i as usize) < 0 {
            self.which = NONE;
            self.left = 0;
            if self.old != NONE {
                self.hull_done = false;
                self.hull_dir = deliveries < 1;
                self.old = NONE;
                self.old_kind = self.kind;
            }
        } else if i >= 0 {
            self.left = i32_at(&m.offers[i as usize], 0x6c);
            self.unit_price = m.price(i as usize, 1);
        }
        self.qty = 0;
    }

    /// The tooltip: the hot-spot under the pointer once it rested (`UpdateFrame`).
    pub fn tooltip(&mut self, tips: &[Vec<u8>], now: f64) {
        let under = self.spot(self.tip.mouse);
        let len = tips.get(under as usize).map_or(0, |t| t.len());
        self.tip.update(now, under, len, self.held.is_some());
    }

    /// The 2D passes over the scene: fader pass 1 (`GFX_Trade`'s boxes, the ticker's bar), `TradeGfx2D_2`'s texts,
    /// fader pass 2 (the destination list's box), `TradeGfx2D_3` (the destinations), the ticker's text.
    pub fn draw(&self, f: &mut Frame, d: &Draw2d, state: &SaveState) {
        let m = Market::of(state);
        let e = en(state);
        let counts = m.counts();
        let open = self.opened && !self.closing;
        let col = |c: usize, pct: f32| d.palette.get(c).copied().unwrap_or([1.0; 3]).map(|x| x * pct / 100.0);
        let fade = |f: &mut Frame, r: [f32; 4], c1: [f32; 4], c2: [f32; 4]| {
            crate::kit::fader::fader_untinted(f, d.white, &d.v, r, [c1[0], c1[1], c1[2]], c1[3], [c2[0], c2[1], c2[2]], c2[3]);
        };
        let ports = ports(state);
        // `GFX_Trade`'s boxes (fader pass 1).
        if open {
            let dark = |a: f32| [0xb as f32, 0xd as f32, 0xd as f32, a];
            for k in 0..2 {
                self.scroll_bar(f, d, [[0x12f, 0x3f], [0x143, 0x3f]][k], counts[k], self.pos[k]);
            }
            let y = (G1[1] + 0x14) as f32;
            if self.which != NONE {
                fade(f, [(G1[0] + 0xf) as f32, y, 40.0, 15.0], dark(0x7f as f32), dark(0x61 as f32));
                fade(f, [(G1[0] + 0x41) as f32, y, 40.0, 15.0], dark(0x7f as f32), dark(0x61 as f32));
                if self.can_buy(state, &m) {
                    fade(f, [(G1[0] + 0x73) as f32, y, 40.0, 15.0], dark(0x7f as f32), dark(0x61 as f32));
                }
            }
            fade(f, [(G2[0] - 5) as f32, y, 55.0, 15.0], dark(0x7f as f32), dark(0x61 as f32));
        }
        let fr = self.frame / 50.0;
        if self.xa == 0.0 {
            let y = (((1.0 - fr) * 139.0) * 0.5 + 295.0) as i32 as f32;
            let g = |a: f32| [0x21 as f32, 0x29 as f32, 0x28 as f32, a as i32 as f32];
            fade(f, [0x106 as f32, y, 0x166 as f32, (fr * 169.0) as i32 as f32], g(255.0 - fr * 128.0), g(255.0 - fr * 228.0));
        }
        // `TozsdeScrollFrissitAlsoBar` (in `Trade3DUpdate`): the ticker's bar, its factors in f64.
        let line = self.queue.first().and_then(|&l| self.lines.get(l));
        let len = line.map_or(0, |s| s.len()) as f32 * self.advance;
        let mut bf = fr as f64;
        if self.hova < 200.0 {
            bf *= self.hova as f64 / 200.0;
        }
        if self.hova > len + 440.0 {
            bf *= -(((self.hova - 640.0) - len) as f64) / 200.0;
        }
        let bf = (bf as f32).max(0.0);
        fade(f, [30.0, 457.0, 575.0, 15.0], [27.0, 76.0, 122.0, (250.0 - bf * 38.0) as i32 as f32], [54.0, 152.0, 244.0, (255.0 - bf * 98.0) as i32 as f32]);
        // `TradeGfx2D_2`.
        let label = |k: u16| {
            let mut s = d.texts.get(8, 2, k);
            s.push(b':');
            s
        };
        let text = |f: &mut Frame, s: &[u8], x: f32, y: f32, len: usize, align: Align, c: usize| d.font.text(f, &d.v, s, x, y, len, 2, align, col(c, 100.0), None);
        let int = |f: &mut Frame, n: i64, x: f32, y: f32, len: usize, align: Align, c: usize| d.font.int(f, &d.v, n, x, y, len, 2, align, col(c, 100.0));
        if open {
            for (k, x) in LIST_X.iter().map(|&x| x as f32).enumerate() {
                let (title, n) = if k == 0 { (3, 8) } else { (6, 12) };
                text(f, &label(title), x, 0x31 as f32, n, Align::Left, 1);
                text(f, &label(4), x + 0x77 as f32, 0x31 as f32, 6, Align::Left, 1);
                text(f, &label(5), x + 0xa4 as f32, 0x31 as f32, 5, Align::Left, 1);
                // `TradeDisplayVehicle` (exe 0x608a60): the class's offers from the `first`-th, eight rows.
                let t = k as i16 + 1;
                let mut i = m.next(t, 0);
                for _ in 0..self.pos[k] {
                    i = m.next(t, i + 1);
                }
                for row in 0..ROWS {
                    if self.pos[k] + row >= counts[k] {
                        break;
                    }
                    let y = (LIST_Y + ROW_H * row) as f32;
                    if let Some(r) = m.offers.get(i) {
                        text(f, crate::design::name(r), x, y, 16, Align::Left, 0x25);
                        int(f, m.price(i, 1) as i64, x + 0x78 as f32, y, 7, Align::Left, 0x25);
                        int(f, i32_at(r, 0x6c) as i64, x + 0xa5 as f32, y, 3, Align::Left, 0x25);
                    }
                    i = m.next(t, i + 1);
                }
            }
            if self.which != NONE {
                let p = self.pos[self.kind];
                if (p..p + ROWS).contains(&self.which) {
                    self.list_frame.draw(f, d.white, &d.v, col(1, 100.0));
                }
                let (x1, y1, x2) = (G1[0] as f32, G1[1] as f32, G2[0] as f32);
                text(f, &label(7), x1, y1, 7, Align::Left, 1);
                int(f, self.qty as i64, x1 + 0x28 as f32, y1, 3, Align::Right, 0x25);
                text(f, b"/", x1 + 0x41 as f32, y1, 1, Align::Left, 1);
                int(f, self.left as i64, x1 + 0x50 as f32, y1, 3, Align::Left, 0x25);
                text(f, &label(4), x2, y1, 6, Align::Left, 1);
                int(f, self.unit_price.wrapping_mul(self.qty) as i64, x2 + 0x28 as f32, y1, 8, Align::Left, 0x25);
                text(f, b"-", x1 + 0x15 as f32, y1 + 0x19 as f32, 5, Align::Field, 0);
                text(f, b"-", x1 + 0x14 as f32, y1 + 0x18 as f32, 5, Align::Field, 0x25);
                text(f, b"+", x1 + 0x47 as f32, y1 + 0x19 as f32, 5, Align::Field, 0);
                text(f, b"+", x1 + 0x46 as f32, y1 + 0x18 as f32, 5, Align::Field, 0x25);
                if self.can_buy(state, &m) {
                    text(f, &d.texts.get(8, 2, 8), x1 + 0x78 as f32, y1 + 0x18 as f32, 5, Align::Field, 0x25);
                }
                if self.planet != -1 {
                    text(f, &label(9), x2 + 0x5c as f32, y1, 4, Align::Left, 1);
                    int(f, self.eta as i64, x2 + 0x82 as f32, y1, 3, Align::Left, 0x25);
                }
            }
            let (x2, y) = (G2[0] as f32, (G1[1] + 0x18) as f32);
            text(f, &label(10), x2, y, 8, Align::Field, 0x25);
            if self.planet == -1 {
                text(f, b"---", x2 + 0x3c as f32, y, 3, Align::Field, 0x25);
            } else {
                text(f, &planet_name(state, self.planet), x2 + 0x3c as f32, y, 0x14, Align::Left, 0x25);
            }
            if self.xa == 0.0 {
                for (k, x, n) in [(11, SZALL_X, 12), (9, 370.0, 4), (12, 420.0, 10), (13, 570.0, 8)] {
                    text(f, &label(k), x, 300.0, n, Align::Left, 1);
                }
            }
        }
        // The deliveries (block B), clipped to `HatarY1 = 0x127 .. HatarY2` as the panel opens (all f32).
        if self.xa == 0.0 {
            let count = state.mem.i32(va::TRADE_DELIVERY_COUNTS + 4 * e as u32).clamp(0, 50);
            let n = (count + 1).min(11);
            let y2 = (((((n * 15 + 8) as f32 * fr + 320.0) - 7.0) - 25.0) * 479.0 / 479.0) as i32;
            let h = d.font.height(2) as i32;
            let base = va::TRADE_DELIVERIES + e as u32 * ig2_sim::save::TRADE_EMPIRE_STRIDE;
            for i in 0..count.min(10) {
                let y = i * 15 + 0x13e;
                if y > 0x1d3 || !(0x127..=y2 - h + 1).contains(&y) {
                    continue;
                }
                let at = base + (i as usize * DELIVERY) as u32;
                let rec = state.mem.read(at + 0x14, RECORD);
                let yf = y as f32;
                text(f, &planet_name(state, state.mem.i32(at)), SZALL_X, yf, 16, Align::Left, 0x25);
                int(f, state.mem.i32(at + 4) as i64, 370.0, yf, 4, Align::Left, 0x25);
                text(f, crate::design::name(&rec), 420.0, yf, 16, Align::Left, 0x25);
                int(f, state.mem.i32(at + 0xc) as i64, 570.0, yf, 7, Align::Left, 0x25);
            }
        }
        // Fader pass 2 and `TradeGfx2D_3`: the destination list.
        if open && self.selecting {
            let n = ports.len().max(1) as f32;
            let [x, y] = self.sel_at.map(|c| c as f32);
            fade(f, [x - 5.0, y - 2.0, 0x8e as f32, n * 13.0 + 4.0], [0.0, 10.0, 0.0, 0x3c as f32], [0.0, 5.0, 0.0, 0x1e as f32]);
            for (j, &p) in ports.iter().enumerate() {
                text(f, &planet_name(state, p), x, y + 3.0 + j as f32 * 13.0, 0x16, Align::Left, if j as i32 == self.selected { 0x25 } else { 1 });
            }
            if ports.is_empty() {
                text(f, &d.texts.get(8, 2, 14), x, y + 3.0, 0x16, Align::Left, 0x25);
            }
        }
        // `TozsdeScrollFrissit`: the ticker's text, clipped to x 35..600, `KiirStrSzinSzorzo = traframe + 50` faded
        // in and out over its first and last 200 pixels (f64).
        if let Some(s) = line {
            let mut pct = ((self.frame + 50.0) as i32) as f64;
            if self.hova < 200.0 {
                pct *= self.hova as f64 / 200.0;
            }
            if self.hova > len + 440.0 {
                pct *= -(((self.hova - 640.0) - len) as f64) / 200.0;
            }
            let extra = if self.opened { 700.0 - fr * 700.0 } else { 0.0 };
            let x = (640.0 - self.hova - extra) as i32 as f32;
            d.font.text(f, &d.v, s, x, 0x1cc as f32, s.len(), 2, Align::Left, col(0x22, pct as i32 as f32), Some((35.0, 600.0)));
        }
    }

    /// `TradeDisplayScrollBar(x, y, total, pos, 8, 0x56)` (exe 0x608900, f64 with 1.00001): the track and the thumb.
    fn scroll_bar(&self, f: &mut Frame, d: &Draw2d, at: [i32; 2], total: i32, pos: i32) {
        let h = 0x56 as f64;
        let (thumb, off) = if total == 0 {
            (h as f32, 0.0f32)
        } else {
            let thumb = ((ROWS as f64 * 1.00001 / total as f64) * h).min(h) as f32;
            let off = ((pos as f64 / (total as f64 * 1.00001 - ROWS as f64)) * (h - thumb as f64)) as f32;
            (thumb, off.max(0.0))
        };
        let [x, y] = at.map(|c| c as f32);
        let g = |a: i32| [0x21 as f32, 0x29 as f32, 0x28 as f32, a as f32];
        let t = |a: i32| [0x2c as f32, 0x36 as f32, 0x34 as f32, a as f32];
        let fade = |f: &mut Frame, r: [f32; 4], c1: [f32; 4], c2: [f32; 4]| {
            crate::kit::fader::fader_untinted(f, d.white, &d.v, r, [c1[0], c1[1], c1[2]], c1[3], [c2[0], c2[1], c2[2]], c2[3]);
        };
        fade(f, [x, y, 13.0, h as f32], g(0x44), g(0x80));
        fade(f, [x, (y + off) as i32 as f32, 13.0, thumb as i32 as f32], t(0x80), t(0x9e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> Trade {
        let mut s = Trade::with(true, 2, Vec::new(), 6.0, 0, false);
        s.sounds.clear();
        s
    }

    #[test]
    fn the_opening_takes_138_ticks_and_queues_the_port_line() {
        // `traframe += 0.36363636…` (f64 on f32): frame 50 on the 138th tick.
        let mut f = 0.0f32;
        let mut n = 0;
        while f < 50.0 {
            f = ((f as f64 + 4.0 / 11.0) as f32).min(50.0);
            n += 1;
        }
        assert_eq!(n, 138);
        let s = blank();
        assert_eq!((s.queue.clone(), s.xa), (vec![0, 0], XA_MAX));
    }

    #[test]
    fn the_message_is_planet_eta_offer_units_price_then_the_record() {
        let mut m = Market { offers: vec![[0; RECORD]; OFFERS + 1], n: 2 };
        m.offers[1][0x16] = 1;
        m.offers[1][0x24..0x28].copy_from_slice(&940i32.to_le_bytes());
        m.offers[1][0x6c] = 5;
        let mut s = blank();
        (s.which, s.kind, s.planet, s.eta, s.qty) = (0, 0, 43, 31, 2);
        assert_eq!(s.offer(&m), 1);
        assert_eq!(m.price(1, 2), 1880);
        // A fighter offer (`+0x36`) costs a fifth per unit, after the multiply.
        m.offers[1][0x36] = 1;
        assert_eq!(m.price(1, 3), 940 * 3 / 5);
    }

    #[test]
    fn a_row_outside_the_window_scrolls_it_by_one() {
        let mut s = blank();
        s.pos[0] = 3;
        let mut sel = 2;
        s.fit_row(0, &mut sel, 20);
        assert_eq!((sel, s.pos[0]), (2, 2));
        let mut sel = 11;
        s.fit_row(0, &mut sel, 20);
        assert_eq!((sel, s.pos[0]), (10, 3));
        // Near the end the window stops at count − 8; the row past the last is left to `TradeSelectShip`'s clamp.
        s.pos[0] = 12;
        let mut sel = 20;
        s.fit_row(0, &mut sel, 20);
        assert_eq!((sel, s.pos[0]), (20, 12));
    }

    #[test]
    fn the_wire_sphere_fades_in_by_frame_25() {
        let mut s = blank();
        s.frame = 0.0;
        assert_eq!(s.wire_transparency(), 0.999_999_9);
        s.frame = 25.0;
        assert!((s.wire_transparency() - 0.35).abs() < 1e-6);
        s.frame = 30.0;
        assert_eq!(s.wire_transparency(), 1e-6);
    }
}
