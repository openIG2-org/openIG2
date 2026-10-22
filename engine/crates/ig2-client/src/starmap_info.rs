//! The starmap's fleet and vehicle info panels (re/screens/starmap.md §19–§20): `StarmapFleetMode` with the selected
//! fleet's rows (`CopyFleets` ELF 0x6e59e0, `FlottaRendezo` 0x6e5830) and `StarmapTanksInfo` with the planet's vehicles
//! (`GetNthTank` 0x6ea600, `StarmapTanksInfoSetup` 0x6ec7d0); their faders (`ArrangeFleetInfo` 0x6e5c30,
//! `StarmapTanksInfoFaderKirakas` 0x6ea720), texts (`FlottaInfoText2d` 0x6e74b0, `StarmapTanksInfoSzovegKiirasok`
//! 0x6ec6a0, `DrawSpecificInfoOnTank` 0x6eb210), models (`InfoBetoltAlap` 0x6e5b20, `InfoForgatAlap` 0x6d3840,
//! `SetTanksInfoObjects` 0x6ebbb0, `SetSpecificMeshId` 0x6ec190) and the sale menus (`SellShipMenuSor` 0x6f4220,
//! `SellShipMehet` 0x6f40c0, `SellTankMenuSor` 0x6f4910, `SellTankMehet` 0x6f4800) whose release sends
//! `MULTIGET_Design`'s 0x34 (sell) and 0x35 (refit).

use ig2_sim::save::{va, SaveState, PLANET_SIZE, TECH_SIZE};

use crate::kit::design_frame::DesignFrame;
use crate::kit::fader::{kirak_fader, S};
use crate::kit::inside;
use crate::orders::Selection;
use crate::render::Frame;
use crate::scene::{self, M3, V3};
use crate::starmap_panels::Draw2d;

fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

fn en(state: &SaveState) -> u8 {
    state.mem.i32(va::EN).clamp(0, 18) as u8
}

fn planet(state: &SaveState, p: usize) -> Vec<u8> {
    state.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE)
}

fn fleet(state: &SaveState, f: i32) -> Option<&[u8]> {
    usize::try_from(f).ok().and_then(|f| state.fleets.get(f)).map(|r| &r[..])
}

/// `Tervek[e][d]`.
fn design(state: &SaveState, e: u8, d: usize) -> Option<&[u8]> {
    state.designs.get(e as usize)?.get(d).map(|r| &r[..])
}

/// `Tankok[1..=OsszHanyTank]` (8 bytes: holder i16 (a fleet, or −p), design i16 (0x7fff free), count i32), with
/// their indices.
fn vehicles(state: &SaveState) -> Vec<(usize, [u8; 8])> {
    let n = state.mem.i32(va::VEHICLE_COUNT).max(0) as usize;
    let raw = state.mem.read(va::VEHICLES, 8 * (n + 1));
    raw.chunks_exact(8).enumerate().skip(1).map(|(i, c)| (i, c.try_into().unwrap())).collect()
}

/// One row: a design and how many units of it (ships one each, a vehicle entry its count); for the vehicle panel the
/// `Tankok` entry it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoRow {
    pub design: usize,
    pub count: i32,
    pub tank: usize,
}

/// `CopyFleets`: the `Hajok` entries of fleet `f`, then its `Tankok` entries, one row per design (`getterv`), sorted by
/// `FlottaRendezo` on `Tervek[EN]`: class `+0x16` (1 ship, 2 vehicle), non-fighters (`+0x36` bit 0) first, design.
pub fn fleet_rows(state: &SaveState, f: i32) -> Vec<InfoRow> {
    let mut rows: Vec<InfoRow> = Vec::new();
    let mut add = |d: i16, c: i32| {
        if d == 0x7fff || d < 0 {
            return;
        }
        match rows.iter_mut().find(|r| r.design == d as usize) {
            Some(r) => r.count += c,
            None => rows.push(InfoRow { design: d as usize, count: c, tank: 0 }),
        }
    };
    for s in state.ships.iter().skip(1) {
        if i16_at(s, 0) as i32 == f {
            add(i16_at(s, 2), 1);
        }
    }
    for (_, v) in vehicles(state) {
        if i16_at(&v, 0) as i32 == f {
            add(i16_at(&v, 2), i32_at(&v, 4));
        }
    }
    let e = en(state);
    let key = |d: usize| design(state, e, d).map_or((0, 0), |r| (i16_at(r, 0x16), r[0x36] & 1));
    rows.sort_by(|a, b| {
        let (ka, kb) = (key(a.design), key(b.design));
        ka.cmp(&kb).then(a.design.cmp(&b.design))
    });
    rows
}

/// The vehicle panel's list: `GetNthTank(n, −p)` for n = 1.., the `Tankok` entries held by planet `p` in table order.
pub fn tank_rows(state: &SaveState, p: usize) -> Vec<InfoRow> {
    vehicles(state)
        .into_iter()
        .filter(|(_, v)| i16_at(v, 2) != 0x7fff && i16_at(v, 0) as i32 == -(p as i32))
        .map(|(i, v)| InfoRow { design: i16_at(&v, 2) as usize, count: i32_at(&v, 4), tank: i })
        .collect()
}

/// `GetTervEladasOszto(e, d)` (ELF 0x506c40): 2, 10 for fighters, 0.5 for the story's cheap sale, −1 for a story
/// design that can't be sold.
pub fn sale_divisor(state: &SaveState, e: u8, d: usize) -> f32 {
    let Some(r) = design(state, e, d) else { return 2.0 };
    let mut f: f32 = if r[0x36] & 1 != 0 { 10.0 } else { 2.0 };
    let story = i32_at(&state.mem.read(va::STORY_ADAT + 0xac, 4), 0) != 0;
    if story && i32_at(r, 0x44) == 5 && i16_at(r, 0x40) as i32 == e as i32 {
        f = 0.5;
    }
    let id = i32_at(r, 0x80);
    if (0x6a5..3000).contains(&id) && ![0x8fc, 0x8fd, 0x906].contains(&id) && !(0x924..=0x937).contains(&id) {
        return -1.0;
    }
    f
}

/// `HanyHajoVanATervbol(e, holder, d)` and `TervHajoinakAra(e, holder, d)` (ELF 0x506520, 0x506880): the units of
/// design `d` or of a design upgrading to it (`+0x1c == d`) that empire `e` has in `holder` (0 = anywhere, stations
/// included), and their designs' prices summed.
pub fn units_of(state: &SaveState, e: u8, holder: i32, d: usize) -> (i32, i32) {
    let rec = |x: i16| design(state, e, x.max(0) as usize);
    let matches = |x: i16| x as i32 == d as i32 || rec(x).is_some_and(|r| i32_at(r, 0x1c) == d as i32);
    let price = |x: i16| rec(x).map_or(0, |r| i32_at(r, 0x24));
    let (mut n, mut sum) = (0, 0);
    for s in state.ships.iter().skip(1) {
        let (h, x) = (i16_at(s, 0), i16_at(s, 2));
        if x != 0x7fff && fleet(state, h as i32).is_some_and(|f| f[0x15] == e) && matches(x) && (holder == 0 || h as i32 == holder) {
            n += 1;
            sum += price(x);
        }
    }
    for (_, v) in vehicles(state) {
        let (h, x, c) = (i16_at(&v, 0), i16_at(&v, 2), i32_at(&v, 4));
        if x == 0x7fff {
            continue;
        }
        let owner = if h < 1 { planet(state, (-(h as i32)) as usize)[0x20] } else { fleet(state, h as i32).map_or(0xff, |f| f[0x15]) };
        if owner == e && matches(x) && (holder == 0 || h as i32 == holder) {
            n += c;
            sum += c * price(x);
        }
    }
    if holder == 0 {
        for p in 1..=state.planet_count() {
            let r = planet(state, p);
            if r[0x20] != e {
                continue;
            }
            for k in 1..=i32_at(&r, 0x114).clamp(0, 10) as usize {
                let x = i16_at(&r, 0xfe + 2 * k);
                if matches(x) {
                    n += 1;
                    sum += price(x);
                }
            }
        }
    }
    (n, sum)
}

/// What a sale menu's release sends (`MULTIGET_Design` exe 0x4b4d60, re/rules/design-orders.md): 0x34 sells the
/// holder's units of the design (`mode` 2), 0x35 refits them to its upgrade target.
pub use ig2_sim::game::DesignOrder;

/// Which panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `StarmapFleetMode` on `AktFleet`.
    Fleet,
    /// `StarmapTanksInfo` on `AktPlanet`.
    Tanks,
}

/// The sale menu (`StarmapSelectDeleteMode` / `StarmapSelectTankDeleteMode`): `smSelectX/Y`, `smSelected`, the rows'
/// texts and whether each is allowed (`SellShipMehet`).
#[derive(Debug, Clone, PartialEq)]
pub struct Sale {
    pub x: i32,
    pub y: i32,
    pub hovered: i32,
    pub rows: Vec<(Vec<u8>, bool)>,
    /// The row's design, its owner and the holder (`AktFleet` or −p).
    pub design: usize,
    pub owner: u8,
    pub holder: i32,
    /// `GetTervEladasOszto ≥ 0`.
    pub sellable: bool,
}

impl Sale {
    /// `SellShipHossz` / `SellTankHossz`: the longest row.
    pub fn len(&self) -> i32 {
        self.rows.iter().map(|r| r.0.len()).max().unwrap_or(0) as i32
    }

    fn hover(&mut self, at: [f32; 2]) {
        let (mx, my, w) = (at[0] as i32, at[1] as i32, 6 * self.len());
        self.hovered = -1;
        for k in 1..=self.rows.len() as i32 {
            if self.x <= mx && mx <= self.x + w && self.y + 13 * (k - 1) <= my && my < self.y + 13 * k {
                self.hovered = k;
            }
        }
    }
}

/// The texts the menus are made of: `DesignTorlesValasztasNev[0..4]` (`GetString(0x11, 3, 0x1c..=0x1f)`) and
/// `GetString(2, 4, 7)` ("unable to sell").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SaleNames {
    pub names: [Vec<u8>; 4],
    pub unable: Vec<u8>,
}

impl SaleNames {
    pub fn load(t: &crate::ui::UiTexts) -> Self {
        Self { names: std::array::from_fn(|k| t.get(0x11, 3, 0x1c + k as u16)), unable: t.get(2, 4, 7) }
    }
}

/// The cells' hot-spots (`Starmap_MouseInit`'s even ids 0x43..=0x51: the left panel's columns).
fn cell_rect(k: i32) -> [i32; 4] {
    let y = 60 + 70 * (k / 2);
    if k % 2 == 0 {
        [28, y, 124, 70]
    } else {
        [152, y, 139, 70]
    }
}

/// An open info panel (the original's screen globals).
#[derive(Debug, Clone, PartialEq)]
pub struct InfoPanel {
    pub kind: Kind,
    /// `StarmapTransferKezdet` / `StarmapTanksInfoFirst`: the first item shown (even).
    pub first: i32,
    /// `ActInfoItem` / `StarmapTanksInfoSelected`: the highlighted item.
    pub selected: i32,
    /// `ActInfoItem2`: the fleet's previewed item; for the vehicle panel the `Tankok` entry of the big model
    /// (`SetSpecificMeshId`), 0 for none.
    pub preview: i32,
    pub sale: Option<Sale>,
    /// A cell held (`Mouse_Megfogva`): its place k.
    pub held: Option<i32>,
    /// The scroll bar held.
    pub scrolling: bool,
    /// `StarmapInfoOffLastZoom`: the zoom the fly started from (the exit button slides in over it).
    pub zoom_from: f32,
    /// `StarmapInfoOffHoltart`.
    pub holtart: f32,
    /// The time the panel opened and the last advanced to (the cells turn per drawn frame, `InfoAlapRotation` per
    /// UI frame).
    pub opened: Option<f32>,
    pub now: f32,
    pub frame: DesignFrame,
    /// `InfoAlapRotation` given outright (a capture's value) instead of the clock's.
    pub turn: Option<f32>,
    names: SaleNames,
}

impl InfoPanel {
    pub fn new(kind: Kind, names: SaleNames, zoom_from: f32) -> Self {
        let holtart = if kind == Kind::Tanks { 1.0 } else { 0.0 };
        Self { kind, first: 0, selected: 0, preview: 0, sale: None, held: None, scrolling: false, zoom_from, holtart, opened: None, now: 0.0, frame: DesignFrame::default(), turn: None, names }
    }

    /// The rows for the selection.
    pub fn rows(&self, state: &SaveState, sel: &Selection) -> Vec<InfoRow> {
        match self.kind {
            Kind::Fleet => fleet_rows(state, sel.fleet as i32),
            Kind::Tanks => tank_rows(state, sel.planet),
        }
    }

    /// `StarmapTanksInfoHanyItemVan`: every row while `first` is within them, else 0; the fleet's `InfoHanyHely`.
    fn count(&self, rows: &[InfoRow]) -> i32 {
        let n = rows.len() as i32;
        if self.kind == Kind::Tanks && self.first >= n {
            0
        } else {
            n
        }
    }

    /// The owner whose designs the rows are (the fleet's, the planet's).
    pub fn owner(&self, state: &SaveState, sel: &Selection) -> u8 {
        match self.kind {
            Kind::Fleet => fleet(state, sel.fleet as i32).map_or(0, |r| r[0x15]),
            Kind::Tanks => planet(state, sel.planet)[0x20],
        }
    }

    fn holder(&self, sel: &Selection) -> i32 {
        match self.kind {
            Kind::Fleet => sel.fleet as i32,
            Kind::Tanks => -(sel.planet as i32),
        }
    }

    /// The hot-spot at `at` among the panel's own (9, the cells 0x43 + 2k, 0x57, 0x58); the lowest id wins.
    pub fn spot(&self, state: &SaveState, sel: &Selection, at: [f32; 2]) -> Option<u32> {
        let n = self.count(&self.rows(state, sel));
        // `Starmap_MouseInit2`: the fleet's cells are `min(n, 8)` whatever `Kezdet` (an empty cell of a scrolled list
        // still answers), the vehicles' `min(n − First, 8)`.
        let shown = if self.kind == Kind::Fleet { n.min(8) } else { (n - self.first).clamp(0, 8) };
        if inside([445, 380, 164, 60], at) {
            return Some(9);
        }
        if let Some(k) = (0..shown).find(|&k| inside(cell_rect(k), at)) {
            return Some(0x43 + 2 * k as u32);
        }
        if inside([307, 30, 26, 329], at) {
            return Some(0x57);
        }
        inside([24, 24, 313, 339], at).then_some(0x58)
    }

    /// A press on one of the panel's hot-spots (`Mouse_Lenyom`). `Some(true)` = close the panel (hot-spot 9).
    pub fn press(&mut self, state: &SaveState, sel: &Selection, id: u32, at: [f32; 2]) -> bool {
        match id {
            9 => return true,
            0x43..=0x52 => {
                let k = ((id - 0x43) / 2) as i32;
                self.held = Some(k);
                self.cell(state, sel, k, at, true);
            }
            0x57 => {
                self.scrolling = true;
                self.scroll(state, sel, at);
            }
            _ => {}
        }
        false
    }

    /// The cell branch of `Starmap_Main` (each UI frame while held, and on the press): the item becomes the selected
    /// one, and on the owner's units the sale menu follows the pointer (placed on its first frame).
    fn cell(&mut self, state: &SaveState, sel: &Selection, k: i32, at: [f32; 2], first_frame: bool) {
        let rows = self.rows(state, sel);
        let n = self.count(&rows);
        if self.first + k < n {
            self.selected = self.first + k;
            if self.kind == Kind::Fleet {
                self.preview = self.selected;
            }
        }
        let e = en(state);
        let owner = self.owner(state, sel);
        let tutorial = self.kind == Kind::Fleet && state.mem.u8(0xF66FFD) != 0;
        if owner != e || tutorial {
            return;
        }
        // The fleet's menu is of `ActInfoItem2`'s row, the vehicles' of `GetNthTank(Selected + 1)`'s.
        let item = if self.kind == Kind::Fleet { self.preview } else { self.selected };
        let Some(row) = rows.get(item.max(0) as usize).copied() else {
            self.sale = None;
            return;
        };
        let holder = self.holder(sel);
        let mut sale = self.sale_rows(state, sel, row, owner, holder);
        match &self.sale {
            Some(s) if !first_frame => (sale.x, sale.y) = (s.x, s.y),
            _ => {
                (sale.x, sale.y) = (at[0] as i32 + 14, at[1] as i32 - 6);
            }
        }
        let w = 6 * sale.len();
        if sale.x + w + 4 > 0x27f {
            sale.x = 0x280 - (w + 4);
        }
        sale.hover(at);
        self.sale = Some(sale);
    }

    /// `SellShipMenuSor` / `SellTankMenuSor` and `SellShipMehet` / `SellTankMehet` for the row.
    fn sale_rows(&self, state: &SaveState, sel: &Selection, row: InfoRow, owner: u8, holder: i32) -> Sale {
        let d = row.design;
        let rec = design(state, owner, d);
        let target = rec.map_or(0, |r| i32_at(r, 0x1c));
        let oszto = sale_divisor(state, owner, d);
        let resale = if oszto < 0.0 { 0 } else { (rec.map_or(0, |r| i32_at(r, 0x24)) as f32 / oszto) as i32 };
        let number = |s: &[u8], v: i32| [s, v.to_string().as_bytes()].concat();
        let names = &self.names.names;
        let fits = match self.kind {
            Kind::Fleet => {
                let f = fleet(state, sel.fleet as i32).unwrap_or(&[0u8; 0x80][..]);
                row.count * rec.map_or(0, |r| r[0x28] as i32) <= i32_at(f, 0x78) - i32_at(f, 0x5c)
            }
            Kind::Tanks => true,
        };
        let mut one = number(&names[0], row.count * resale);
        if oszto < 0.0 {
            // `GetHosElkuldesStr` (the story heroes' texts) [the port shows "unable to sell"].
            one = self.names.unable.clone();
        } else if !fits {
            one = names[3].clone();
        }
        let mut rows = vec![(one, fits)];
        if target > 0 {
            let (n, sum) = units_of(state, owner, holder, d);
            let price = design(state, owner, target.max(0) as usize).map_or(0, |r| i32_at(r, 0x24));
            let cost = ((n * price - sum) * 2).max(0);
            let money = state.treasuries()[en(state) as usize].available(0);
            let ok = (cost as f64) <= money || cost < 1;
            rows.push((number(if ok { &names[1] } else { &names[2] }, cost), ok));
        }
        Sale { x: 0, y: 0, hovered: -1, rows, design: d, owner, holder, sellable: oszto >= 0.0 }
    }

    /// One 10 ms UI frame with the button held: the cell's branch again, or the scroll bar.
    pub fn hold_frame(&mut self, state: &SaveState, sel: &Selection, at: [f32; 2]) {
        if let Some(k) = self.held {
            self.cell(state, sel, k, at, false);
        }
        if self.scrolling {
            self.scroll(state, sel, at);
        }
    }

    /// The button went up: a sale row under the pointer that is allowed sends 0x34 (row 1, mode 2) or 0x35 (row 2);
    /// the menu closes.
    pub fn release(&mut self) -> Option<DesignOrder> {
        self.held = None;
        self.scrolling = false;
        let s = self.sale.take()?;
        let row = usize::try_from(s.hovered - 1).ok().and_then(|i| s.rows.get(i))?;
        if !row.1 || !s.sellable {
            // `Story_Special(0x19, AktFleet, d)` for a story design (single player only, not ported).
            return None;
        }
        Some(if s.hovered < 2 {
            DesignOrder::Sell { design: s.design as i32, mode: s.hovered + 1, holder: s.holder }
        } else {
            DesignOrder::Refit { design: s.design as i32, holder: s.holder }
        })
    }

    /// The pointer moved: the open menu follows it.
    pub fn hover(&mut self, at: [f32; 2]) {
        if let Some(s) = &mut self.sale {
            s.hover(at);
        }
    }

    /// Hot-spot 0x57 (§13.4's formula): 2632/n pixels a thumb, even rows only.
    fn scroll(&mut self, state: &SaveState, sel: &Selection, at: [f32; 2]) {
        let n = self.count(&self.rows(state, sel));
        if n < 9 {
            self.first = 0;
            return;
        }
        let h = 0xa48 / n;
        let first = (((at[1] as i32 - 30) - h / 2) * (n - 8) + (0x149 - h) / 2) / (0x149 - h);
        self.first = (first.clamp(0, n - 8) as f64 / 2.0).ceil() as i32 * 2;
    }

    /// Advance the clocks to `time` (seconds) and the exit button to the camera's `zoom`; `ui_frames` 10 ms frames ran
    /// (the selection frame's morph). The vehicle panel's big model follows the selection while it is shown.
    pub fn advance(&mut self, state: &SaveState, sel: &Selection, time: f32, zoom: f32, ui_frames: u32) {
        self.opened.get_or_insert(time);
        self.now = time;
        if self.kind == Kind::Fleet {
            // `(Zoom − OffLastZoom)/(TargetZoom − OffLastZoom)`.
            self.holtart = if self.zoom_from >= 590.0 { 1.0 } else { ((zoom - self.zoom_from) / (590.0 - self.zoom_from)).clamp(0.0, 1.0) };
        }
        let rows = self.rows(state, sel);
        let n = self.count(&rows);
        if self.kind == Kind::Tanks {
            let k = self.selected - self.first;
            if n == 0 {
                self.preview = 0;
            } else if (0..8).contains(&k) {
                if let Some(r) = rows.get(self.selected as usize) {
                    self.preview = r.tank as i32;
                }
            }
        }
        let k = self.selected - self.first;
        if self.kind == Kind::Fleet && (0..8).contains(&k) && self.selected < n {
            self.frame.ask([39 + (k % 2) * 129, 121 + (k / 2) * 70, 120, 13]);
        }
        for _ in 0..ui_frames.min(200) {
            self.frame.tick();
        }
    }

    /// Drawn frames since the panel opened (the cells' spin, 60 Hz [inf]).
    fn drawn_frames(&self) -> f32 {
        ((self.now - self.opened.unwrap_or(self.now)) * 60.0).floor()
    }

    /// The models to draw: per shown cell the row's hull at scale 0.04 turning `(i % 3 + 1)·0.03` a drawn frame, and
    /// the selected design (the fleet's `InfoForgatAlap` pose at 0.08, the vehicles' at 0.1).
    pub fn poses(&self, state: &SaveState, sel: &Selection) -> Vec<Pose> {
        let rows = self.rows(state, sel);
        let n = self.count(&rows);
        let owner = self.owner(state, sel);
        let e = en(state);
        let mut out = Vec::new();
        let unit = |d: usize| -> Option<(usize, u8)> {
            let r = design(state, owner, d)?;
            let hull = i32_at(r, 0x44).max(0) as usize;
            Some((hull, i16_at(r, 0x40).clamp(0, 9) as u8))
        };
        // A vehicle's skin is `min(owner, 10)`: the fleet's owner, `EN` on the vehicle panel.
        let skin = if self.kind == Kind::Fleet { owner } else { e };
        let frames = self.drawn_frames();
        let spin_axis = [0.0, (-0.5f32).cos(), (-0.5f32).sin()];
        for (i, row) in rows.iter().enumerate().take(n.max(0) as usize) {
            let k = i as i32 - self.first;
            if !(0..8).contains(&k) {
                continue;
            }
            let Some((hull, digit)) = unit(row.design) else { continue };
            let turn = ((i % 3 + 1) as f64 * 0.03) as f32;
            let rot = scene::mul(&scene::axis_matrix(spin_axis, turn * frames), &crate::starmap_panels::tilt());
            let at = [((k % 2) * 0x81 - 0xe5) as f32, ((k / 2) * 0x46 - 0x9c) as f32, -600.0];
            out.push(Pose { hull, digit, owner: skin, rot, scale: 0.04, at });
        }
        let big = match self.kind {
            Kind::Fleet => rows.get(self.preview.max(0) as usize).filter(|_| n > 0).map(|r| r.design),
            Kind::Tanks => (self.preview > 0).then(|| rows.iter().find(|r| r.tank as i32 == self.preview).map(|r| r.design)).flatten(),
        };
        if let Some((hull, digit)) = big.and_then(unit) {
            let (rot, scale) = match self.kind {
                // `RotateObj(0, 0, InfoAlapRotation)`, `RotateObj2((1, 0, 0), 1.3)`; +0.01 a 10 ms UI frame.
                Kind::Fleet => {
                    let turn = self.turn.unwrap_or((self.now - self.opened.unwrap_or(self.now)) * 100.0 * 0.01);
                    // [obs: the exe's view at `InfoAlapRotation` t is the port's at π − t (captures at t = 0.91 and
                    // 2.69, re/screens/starmap.md §19.3); the model axes' handedness is the likely cause.]
                    let yaw = std::f32::consts::PI - turn;
                    (scene::mul(&scene::axis_matrix([1.0, 0.0, 0.0], 1.3), &scene::create_matrix(0.0, 0.0, yaw)), 0.08)
                }
                Kind::Tanks => (scene::mul(&scene::axis_matrix(spin_axis, 0.03 * frames), &crate::starmap_panels::tilt()), 0.1),
            };
            out.push(Pose { hull, digit, owner: skin, rot, scale, at: [150.0, -150.0, -600.0] });
        }
        out
    }
}

/// A camera-fixed model of the panel: the hull (`TransfAlap`), the race digit and the vehicle skin's owner, its
/// rotation, `AddToScene`'s scale and the place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub hull: usize,
    pub digit: u8,
    pub owner: u8,
    pub rot: M3,
    pub scale: f32,
    pub at: V3,
}

/// `fifo`'s look (`KirakFader(…, 0, 54s, 90s, 100, 0, 60s, 110s, 70)`).
fn fifo(f: &mut Frame, d: &Draw2d, r: [f32; 4]) {
    kirak_fader(f, d.white, &d.v, r, [0.0, 54.0 * S, 90.0 * S], 100.0, [0.0, 60.0 * S, 110.0 * S], 70.0);
}

/// The sale menu's box (§16's look).
fn menu_box(f: &mut Frame, d: &Draw2d, r: [f32; 4]) {
    crate::kit::fader::fader(f, d.white, &d.v, r, [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
}

impl InfoPanel {
    /// The faders under the models (`ArrangeFleetInfo` / `StarmapTanksInfoFaderKirakas`).
    pub fn draw_under(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, sel: &Selection) {
        let mut rects = vec![[28.0, 27.0, 273.0, 334.0], [338.0, 186.0, 273.0, 175.0], [338.0, 27.0, 273.0, 152.0], [30.0, 29.0, 269.0, 17.0]];
        if self.kind == Kind::Fleet {
            rects.push([30.0, 342.0, 269.0, 17.0]);
        }
        rects.push([340.0, 188.0, 269.0, 17.0]);
        for r in rects {
            fifo(f, d, r);
        }
        let n = self.count(&self.rows(state, sel));
        let (mut h, mut top) = if n < 9 { (0x14b, 0) } else { (0xa58 / n, self.first * (0x14b - 0xa58 / n) / (n - 8)) };
        if top + h > 0x14b {
            let over = top + h - 0x14b;
            h -= over / 2;
            top -= over / 2;
        }
        fifo(f, d, [306.0, 29.0, 28.0, 333.0]);
        fifo(f, d, [310.0, (top + 0x22) as f32, 20.0, (h - 8) as f32]);
    }

    /// The texts and the sale menu (`FlottaInfoText2d` / `StarmapTanksInfoSzovegKiirasok`, `StarmapGfx2D_3`'s menu box
    /// and `StarmapGfx2D_4`'s rows), over the models.
    pub fn draw_over(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, sel: &Selection, rules: &ig2_rules::Rules) {
        let rows = self.rows(state, sel);
        let n = self.count(&rows);
        let owner = self.owner(state, sel);
        let title = match self.kind {
            Kind::Fleet => fleet(state, sel.fleet as i32).map(|r| cstr(&r[..0x14]).to_vec()).unwrap_or_default(),
            Kind::Tanks => cstr(&planet(state, sel.planet)[..0x18]).to_vec(),
        };
        d.text(f, &title, 156.0, 32.0, 0x13, 3, 7, 3);
        let suffix = |r: &[u8], n: u16| -> Vec<u8> {
            let mut s = cstr(&r[..0x14]).to_vec();
            if i32_at(r, 0x1c) > 0 {
                s.extend_from_slice(b" (");
                s.extend(d.texts.get(2, 4, n));
                s.extend_from_slice(b")");
            }
            s
        };
        for (i, row) in rows.iter().enumerate().take(n.max(0) as usize) {
            let k = i as i32 - self.first;
            if !(0..8).contains(&k) {
                continue;
            }
            let Some(rec) = design(state, owner, row.design) else { continue };
            let (cx, cy) = ((k % 2 * 0x81) as f32, (k / 2 * 0x46) as f32);
            let col = if i as i32 == self.selected { 2 } else { 1 };
            match self.kind {
                Kind::Fleet => {
                    d.text(f, &suffix(rec, 1), cx + 91.0, cy + 124.0, 0x19, 2, 7, col);
                    d.text(f, row.count.to_string().as_bytes(), cx + 91.0, cy + 112.0, 0xe, 2, 7, col);
                }
                Kind::Tanks => {
                    d.text(f, cstr(&rec[..0x14]), cx + 91.0, cy + 124.0, 0x19, 2, 7, col);
                    d.text(f, row.count.to_string().as_bytes(), cx + 116.0, cy + 114.0, 4, 2, 7, col);
                }
            }
        }
        if self.kind == Kind::Fleet {
            // The selected row's frame (`DesignKeret1(…, 0x22)`).
            let k = self.selected - self.first;
            if (0..8).contains(&k) && self.selected < n {
                self.frame.draw(f, d.white, &d.v, d.palette.get(0x22).copied().unwrap_or([1.0; 3]));
            }
            // `ShowFlottaTankCap(110, 347, AktFleet)`.
            let r = fleet(state, sel.fleet as i32).unwrap_or(&[0u8; 0x80][..]);
            let cap = if i32_at(r, 0x78) == 0 {
                [d.texts.get(2, 5, 2), d.texts.get(2, 5, 3)].concat()
            } else {
                [d.texts.get(2, 5, 2), format!(" {}/{}", i32_at(r, 0x5c), i32_at(r, 0x78)).into_bytes()].concat()
            };
            d.text(f, &cap, 164.0, 347.0, 0x28, 2, 7, 1);
        }
        let shown = match self.kind {
            Kind::Fleet => (n > 0).then(|| rows.get(self.selected.max(0) as usize)).flatten(),
            Kind::Tanks => (0..8).contains(&(self.selected - self.first)).then(|| rows.get(self.selected.max(0) as usize)).flatten(),
        };
        if let Some(rec) = shown.and_then(|r| design(state, owner, r.design)) {
            self.draw_data(f, d, state, rec, rules);
        }
        if let Some(s) = &self.sale {
            let (x, y, len) = (s.x as f32, s.y as f32, s.len());
            menu_box(f, d, [x - 6.0, y - 2.0, (len * 6 + 10) as f32, (s.rows.len() * 13 + 4) as f32]);
            for (c, (text, ok)) in s.rows.iter().enumerate() {
                let c = c as i32 + 1;
                let col = if !ok { 1 } else if c == s.hovered { 6 } else { 3 };
                d.text(f, text, x, y + (13 * c - 10) as f32, text.len(), 2, 4, col);
            }
        }
    }

    /// The selected design's data (`FlottaInfoText2d`'s lower half, `DrawSpecificInfoOnTank`).
    fn draw_data(&self, f: &mut Frame, d: &Draw2d, state: &SaveState, rec: &[u8], rules: &ig2_rules::Rules) {
        let race = i16_at(rec, 0x40);
        let hull = i32_at(rec, 0x44).max(0) as usize;
        let table = rules.race_pod_table.get(race.max(0) as usize).copied().unwrap_or(0) - 1;
        let slots = rules.pod_slots.get(table.max(0) as usize).and_then(|t| t.get(hull));
        // `GetMaxPodok`: the slots with a pod type.
        let pods = slots.map_or(0, |s| s.iter().filter(|p| p.pod_type > 0).count());
        for p in 1..=pods {
            let y = (200 + 12 * p) as f32;
            let tech = rec[0x50 + p] as usize;
            if tech == 0 {
                d.text(f, &d.texts.get(2, 5, 5), 485.0, y, 0, 2, 4, 2);
                continue;
            }
            d.text(f, &crate::hud::tech_name(d.texts, tech), 485.0, y, 0x10, 2, 4, 3);
            // `min(GetMaxRarakhato(pt, h, p), tech +0x24)`.
            let slot_max = slots.and_then(|s| s.get(p - 1)).map_or(0, |s| s.max_count);
            let tech_max = state.mem.u8(va::TECHS + (tech * TECH_SIZE) as u32 + 0x24) as i32;
            let max = slot_max.min(tech_max);
            let cnt = rec[0x5d + p] as i32;
            let digits = if cnt < 10 { 1 } else { 2 };
            let x = (485 + 0x72 - digits * 6 / 2) as f32;
            if cnt == 0 {
                d.text(f, b"-", x, y, 2, 2, 4, 6);
            } else {
                d.text(f, cnt.to_string().as_bytes(), x, y, 2, 2, 4, if cnt < max { 3 } else { 0x22 });
            }
        }
        let name = {
            let mut s = cstr(&rec[..0x14]).to_vec();
            if i32_at(rec, 0x1c) > 0 {
                s.extend_from_slice(b" (");
                s.extend(d.texts.get(2, 4, 5));
                s.extend_from_slice(b")");
            }
            s
        };
        d.text(f, &name, 472.0, 193.0, 0x1f, 2, 7, 2);
        let ship = i16_at(rec, 0x16) == 1 && rec[0x36] & 1 == 0;
        let (lx, lflags, llen, vx) = match self.kind {
            Kind::Fleet => (340.0, 4, 0x10, 441.0),
            Kind::Tanks => (385.0, 7, 0xf, 435.0),
        };
        d.text(f, &d.texts.get(2, 5, 6), 420.0, 212.0, llen, 2, 7, 1);
        for (n, y) in [(7u16, 248.0), (8, 260.0), (9, 272.0)] {
            d.text(f, &d.texts.get(2, 5, n), lx, y, llen, 2, lflags, 1);
        }
        if ship {
            d.text(f, &d.texts.get(2, 5, 10), lx, 284.0, llen, 2, lflags, 1);
        }
        d.text(f, &crate::hud::tech_name(d.texts, i32_at(rec, 0x48).max(0) as usize), 400.0, 224.0, 0x10, 2, 7, 2);
        d.text(f, i32_at(rec, 0x30).to_string().as_bytes(), vx, 248.0, 5, 2, 7, 2);
        d.text(f, i32_at(rec, 0x2c).to_string().as_bytes(), vx, 260.0, 5, 2, 7, 2);
        let speed = match self.kind {
            // `(int)((f64)(+0x3c ·f32 100) + 0.5)`, a quarter for `+0x38` bit 0 (ELF 0x6e8557..0x6e85d2).
            Kind::Fleet => {
                let v = ((f32_at(rec, 0x3c) * 100.0) as f64 + 0.5) as i32;
                if rec[0x38] & 1 != 0 {
                    v / 4
                } else {
                    v
                }
            }
            Kind::Tanks => f32_at(rec, 0x3c) as i32,
        };
        d.text(f, speed.to_string().as_bytes(), vx, 272.0, 5, 2, 7, 2);
        if ship {
            d.text(f, rec[0x28].to_string().as_bytes(), vx, 284.0, 5, 2, 7, 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_follow_the_left_panels_hot_spots() {
        assert_eq!(cell_rect(0), [28, 60, 124, 70]);
        assert_eq!(cell_rect(1), [152, 60, 139, 70]);
        assert_eq!(cell_rect(6), [28, 270, 124, 70]);
        assert_eq!(cell_rect(7), [152, 270, 139, 70]);
    }

    /// The vehicle panel on two staged `Tankok` entries of the player's planet and the fleet panel on a fleet built
    /// through production (quick-1.sav's player has none): selection, the sale menu and its orders, closing.
    #[test]
    fn info_panels_select_and_sell() {
        use crate::starmap_panels::{Action, Panels};
        use ig2_net::{Command, StarmapCmd};
        use ig2_sim::game::StarmapOrder;
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let Some(dir) = std::env::var_os("IG2_SAVE_DIR") else { return };
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join("quick-1.sav")) else { return };
        let save = ig2_formats::save::SaveGame::parse(&bytes).unwrap();
        let (rules, defaults) = crate::rules::load(&install).unwrap();
        let volatile = crate::rules::scenario_globals(&install).unwrap();
        let mut w = crate::world::World::load(&save, &rules, &defaults, &volatile);
        let e = en(&w.game.state);
        let p = (1..=w.game.state.planet_count()).find(|&p| planet(&w.game.state, p)[0x20] == e).unwrap();
        let names = SaleNames { names: [b"S".to_vec(), b"U".to_vec(), b"N".to_vec(), b"R".to_vec()], unable: b"X".to_vec() };
        // Two vehicle entries on the planet.
        let state = &mut w.game.state;
        let all = state.designs[e as usize].len();
        let vd: Vec<usize> = (1..all).filter(|&d| design(state, e, d).is_some_and(|r| i16_at(r, 0x16) == 2)).take(2).collect();
        let (d0, d1) = (vd[0], *vd.get(1).unwrap_or(&vd[0]));
        let n = state.mem.i32(va::VEHICLE_COUNT);
        for (k, (d, c)) in [(d0, 3i32), (d1, 5)].into_iter().enumerate() {
            let mut rec = [0u8; 8];
            rec[0..2].copy_from_slice(&(-(p as i16)).to_le_bytes());
            rec[2..4].copy_from_slice(&(d as i16).to_le_bytes());
            rec[4..8].copy_from_slice(&c.to_le_bytes());
            state.mem.write(va::VEHICLES + 8 * (n as u32 + 1 + k as u32), &rec);
        }
        state.mem.set_i32(va::VEHICLE_COUNT, n + 2);
        let sel = Selection { mode: 1, planet: p, fleet: 0 };
        let mut panels = Panels::default();
        panels.open_tanks_info(names.clone());
        let rows = tank_rows(&w.game.state, p);
        assert_eq!(rows.iter().map(|r| (r.design, r.count)).collect::<Vec<_>>(), vec![(d0, 3), (d1, 5)]);
        // Cell 1 (hot-spot 0x45): selected, the menu at the pointer + (14, −6).
        assert_eq!(panels.press(&w.game.state, &sel, [200.0, 100.0], true), Some(vec![]));
        let info = panels.info.as_ref().unwrap();
        assert_eq!(info.selected, 1);
        let s = info.sale.clone().unwrap();
        assert_eq!((s.x, s.y), (214, 94));
        let resale = crate::starmap_menus::resale(&w.game.state, e, d1);
        assert_eq!(s.rows[0].0, [b"S".to_vec(), (5 * resale).to_string().into_bytes()].concat());
        panels.hover([220.0, 98.0]);
        let acts = panels.release(&w.game.state, &sel);
        assert_eq!(acts, vec![Action::Design(DesignOrder::Sell { design: d1 as i32, mode: 2, holder: -(p as i32) })]);
        // It reaches the sim (re/rules/design-orders.md §3): the planet's vehicles of d1 go, their price comes in
        // (`trunc(Σ price / div)`, the menu's `count·CsokkentettTervAr` give or take the rounding) and the design
        // stays (a holder's sale).
        let money = w.game.state.treasuries()[e as usize].total;
        for a in acts {
            if let Action::Design(o) = a {
                w.send(Command::from(o));
            }
        }
        w.pump();
        assert_eq!(tank_rows(&w.game.state, p).iter().map(|r| (r.design, r.count)).collect::<Vec<_>>(), vec![(d0, 3)]);
        let got = w.game.state.treasuries()[e as usize].total - money;
        assert!(got >= (5 * resale) as f64 && got < (5 * resale + 5) as f64, "credited {got}, the menu said {}", 5 * resale);
        assert!(design(&w.game.state, e, d1).is_some_and(|r| i16_at(r, 0x16) == 2));
        // Hot-spot 9 closes it; nothing flies.
        assert_eq!(panels.press(&w.game.state, &sel, [500.0, 400.0], true), Some(vec![]));
        assert!(panels.info.is_none());

        // A fleet: a warship queued and finished (as `starmap_panels::panel_clicks_reach_the_sim`).
        let send = |w: &mut crate::world::World, acts: Vec<Action>| {
            for a in acts {
                if let Action::Send(o) = a {
                    w.send(Command::Starmap { order: StarmapCmd::from(o) });
                }
            }
            w.pump();
        };
        let mut prod = Panels::opened("production").unwrap();
        prod.press(&w.game.state, &sel, [50.0, 150.0], true);
        let list = prod.list.clone().unwrap();
        let state = &w.game.state;
        let (i, d) = list.designs.iter().copied().enumerate().find(|&(_, d)| d > 0 && design(state, e, d).is_some_and(|x| x[0x36] == 0 && x[0x37] == 0)).map(|(i, d)| (i as i32, d)).unwrap();
        prod.hover([(list.x + 126 * (i / list.per_col) + 5) as f32, (list.y + 13 * (i % list.per_col) + 5) as f32]);
        let acts = prod.release(&w.game.state, &sel);
        send(&mut w, acts);
        w.send(Command::Starmap { order: StarmapCmd::from(StarmapOrder::FinishHeads { planet: p as i32 }) });
        w.pump();
        w.tick();
        let f = crate::starmap_panels::near_fleet(&w.game.state, p);
        assert!(f > 0, "no fleet built");
        let sel = Selection { mode: 2, planet: p, fleet: f as usize };
        let mut panels = Panels::default();
        let acts = panels.info_box_fleet(&w.game.state, &sel, names.clone(), 300.0);
        assert!(matches!(acts[..], [Action::ZoomIn { zoom, .. }] if zoom == 590.0));
        assert_eq!(fleet_rows(&w.game.state, f), vec![InfoRow { design: d, count: 1, tank: 0 }]);
        panels.press(&w.game.state, &sel, [60.0, 100.0], true);
        panels.hover([80.0, 98.0]);
        let acts = panels.release(&w.game.state, &sel);
        assert_eq!(acts, vec![Action::Design(DesignOrder::Sell { design: d as i32, mode: 2, holder: f })]);
        let sold = acts;
        // The info box again: the panel goes and the camera flies back.
        assert_eq!(panels.info_box_fleet(&w.game.state, &sel, names, 300.0), vec![Action::ZoomOut]);
        assert!(panels.info.is_none());
        // The sale reaches the sim: the ship goes, and its emptied fleet with it (`TervDelete(e, d, f, 1)`).
        for a in sold {
            if let Action::Design(o) = a {
                w.send(Command::from(o));
            }
        }
        w.pump();
        assert!(fleet_rows(&w.game.state, f).is_empty());
        assert_eq!(crate::starmap_panels::near_fleet(&w.game.state, p), 0);
    }

    #[test]
    fn the_menu_keeps_inside_the_screen() {
        let mut s = Sale { x: 600, y: 100, hovered: -1, rows: vec![(vec![b'x'; 20], true)], design: 1, owner: 1, holder: 1, sellable: true };
        s.hover([610.0, 105.0]);
        assert_eq!(s.hovered, 1);
        // `smSelectX = 0x280 − (6·len + 4)` when `smSelectX + 6·len + 4 > 0x27f`.
        let w = 6 * s.len();
        if s.x + w + 4 > 0x27f {
            s.x = 0x280 - (w + 4);
        }
        assert_eq!(s.x, 516);
    }
}
