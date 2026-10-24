//! The story engine's world inside the game: [`StoryWorld`] over [`Game`] (`re/rules/new-game.md` §15).
//!
//! The scripts read the save state as the exe's `st_*` helpers read its globals and records (the layouts are the
//! trait's), and what they create (`FlottaNew`, `TervNew`, `st_GetTerv`, `Spy_New`) is made in the game at once, so
//! the rest of the script sees it, as in the exe. The streams are the story's while a script runs and the game's
//! around each of its own calls ([`GameStory::with_game`]).

use super::{Game, Globals, TradeView};
use crate::campaign::CampaignTables;
use crate::colony_side::EMPIRES;
use crate::messages::cstr;
use crate::rng::Streams;
use crate::save::{va, PLANET_SIZE, TECH_SIZE};
use crate::story::{FieldUnit, FleetName, SpyRecord, StoryEvent, StoryWorld, DESIGN};

fn rd_i16(b: &[u8], o: usize) -> i32 {
    i16::from_le_bytes([b[o], b[o + 1]]) as i32
}
fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn rd_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
/// The story engine's view of the game while a script runs.
pub(super) struct GameStory<'a, 'r> {
    pub game: &'a mut Game<'r>,
    pub g: Globals,
    /// The streams while the story engine holds them.
    pub rng: Streams,
    pub events: Vec<StoryEvent>,
    /// The wormhole the last `AddWormhole` made (0 none): story 239's `RevealWormhole` shows it.
    pub made: i32,
}

impl<'a, 'r> GameStory<'a, 'r> {
    pub fn new(game: &'a mut Game<'r>) -> Self {
        let g = game.globals();
        let rng = game.state.rng();
        Self { game, g, rng, events: Vec::new(), made: 0 }
    }

    /// The events the scripts raised; the streams go back into the state.
    pub fn finish(self) -> Vec<StoryEvent> {
        self.game.state.store_rng(&self.rng);
        self.events
    }

    /// Run `f` on the game with the streams in its state.
    pub(super) fn with_game<R>(&mut self, f: impl FnOnce(&mut Game<'r>) -> R) -> R {
        self.game.state.store_rng(&self.rng);
        let r = f(self.game);
        self.rng = self.game.state.rng();
        r
    }

    pub(super) fn mem(&self) -> &crate::save::Memory {
        &self.game.state.mem
    }
    pub(super) fn planet(&self, p: i32) -> Vec<u8> {
        self.mem().read(va::PLANETS + (p.max(0) as usize * PLANET_SIZE) as u32, PLANET_SIZE)
    }
    pub(super) fn fleet(&self, f: i32) -> Option<&[u8; 0x80]> {
        self.game.state.fleets.get(usize::try_from(f).ok()?)
    }
    pub(super) fn fleet_i32(&self, f: i32, o: usize) -> i32 {
        self.fleet(f).map_or(0, |r| rd_i32(r, o))
    }
    pub(super) fn design(&self, e: i32, d: i32) -> Option<&[u8; DESIGN]> {
        self.game.state.designs.get(usize::try_from(e).ok()?)?.get(usize::try_from(d).ok()?)
    }
    pub(super) fn design_i32(&self, e: i32, d: i32, o: usize) -> i32 {
        self.design(e, d).map_or(0, |r| rd_i32(r, o))
    }
    pub(super) fn tech_at(t: i32) -> u32 {
        va::TECHS + (t.max(0) as usize * TECH_SIZE) as u32
    }
    pub(super) fn building_rec(&self, p: i32, k: i32) -> Option<&[u8; crate::save::BUILDING_SIZE]> {
        self.game.state.buildings.get(usize::try_from(p).ok()?)?.get(usize::try_from(k).ok()?)
    }
    pub(super) fn spy_at(e: i32, k: i32) -> Option<u32> {
        let (e, k) = (usize::try_from(e).ok()?, usize::try_from(k).ok()?);
        (e < crate::spy::ROSTERS && k < crate::spy::SLOTS).then(|| va::KEMEK + ((e * crate::spy::SLOTS + k) * crate::spy::RECORD) as u32)
    }
    pub(super) fn empire(e: i32) -> usize {
        e.clamp(0, EMPIRES as i32 - 1) as usize
    }
    /// A new design of empire `e` (`TervNew(e, 0)`) with `record` over it: its slot.
    pub(super) fn new_design_with(&mut self, e: i32, record: &[u8; DESIGN], recompute: bool) -> i32 {
        if !(0..EMPIRES as i32).contains(&e) {
            return 0;
        }
        let e = e as usize;
        self.with_game(|g| {
            let mut m = TradeView { game: g };
            let d = crate::trade::Market::new_design(&mut m, e, 0);
            crate::trade::Market::set_design(&mut m, e, d, record);
            if recompute {
                crate::trade::Market::recompute_design(&mut m, e, d, true);
            }
            d as i32
        })
    }
}

impl StoryWorld for GameStory<'_, '_> {
    fn ido(&self) -> i32 {
        self.g.tick
    }
    fn canonical(&self) -> bool {
        self.g.canonical
    }
    fn speed_mult(&self) -> i32 {
        self.g.speed
    }
    fn human(&self) -> i32 {
        self.g.en as i32
    }
    fn difficulty(&self) -> i32 {
        self.g.difficulty
    }
    fn multiplayer(&self) -> bool {
        self.g.multiplayer
    }
    fn rng(&mut self) -> &mut Streams {
        &mut self.rng
    }
    fn planet_count(&self) -> i32 {
        self.game.state.planet_count() as i32
    }
    fn planet_owner(&self, p: i32) -> i32 {
        self.planet(p)[0x20] as i32
    }
    fn planet_value(&self, p: i32) -> i32 {
        rd_i32(&self.planet(p), 0x28)
    }
    fn planet_visibility(&self, p: i32) -> i32 {
        rd_i16(&self.planet(p), 0xd8 + 2 * self.g.en)
    }
    fn fleet_count(&self) -> i32 {
        self.game.state.fleets.len() as i32 - 1
    }
    fn fleet_owner(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0xff, |r| r[0x15] as i32)
    }
    fn fleet_id(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0, |r| rd_i16(r, 0x16))
    }
    fn empire_alive(&self, e: i32) -> bool {
        self.g.alive.get(e.max(0) as usize).copied().unwrap_or(false)
    }
    fn planets_of(&self, e: i32) -> i32 {
        self.mem().i32(va::EMPIRE_PLANETS + 4 * e.max(0) as u32)
    }
    fn money(&self, e: i32) -> f64 {
        self.game.state.treasuries()[Self::empire(e)].available(0)
    }
    fn tech_status(&self, tech: i32, e: i32) -> i32 {
        self.mem().u8(Self::tech_at(tech) + 0x45 + e.max(0) as u32) as i32
    }
    fn relation_level(&self, a: i32, b: i32) -> i32 {
        self.mem().u8(va::RELATIONS + ((a.max(0) as usize * EMPIRES + b.max(0) as usize) * 0x26) as u32 + 0x25) as i32
    }
    fn planet_infected(&self, p: i32) -> bool {
        rd_i16(&self.planet(p), 0xb8) > 0
    }
    fn fleet_infected(&self, f: i32) -> bool {
        self.fleet(f).is_some_and(|r| r[0x15] != 0xff && r[0x52] & 4 != 0)
    }
    fn event(&mut self, ev: StoryEvent) {
        self.carry_out(&ev);
        self.events.push(ev);
    }

    fn campaign_type(&self) -> i32 {
        self.mem().i32(va::CAMPAIGN_TYPE)
    }
    fn campaign_tables(&self) -> &CampaignTables {
        &self.game.rules.story.campaign
    }
    fn race(&self, e: i32) -> i32 {
        self.g.race.get(e.max(0) as usize).copied().unwrap_or(0) as i32
    }
    fn knows(&self, a: i32, b: i32) -> bool {
        self.mem().u8(va::EMPIRE_KNOWS + (a.max(0) as usize * EMPIRES + b.max(0) as usize) as u32) != 0
    }
    fn akt_planet(&self) -> i32 {
        if self.g.canonical { 0 } else { self.mem().i32(va::AKT_PLANET) }
    }
    fn akt_fleet(&self) -> i32 {
        if self.g.canonical { 0 } else { self.mem().i32(va::AKT_FLEET) }
    }
    fn planet_xy(&self, p: i32) -> (f32, f32) {
        let r = self.planet(p);
        (rd_f32(&r, 0x18), rd_f32(&r, 0x1c))
    }
    fn planet_radar(&self, p: i32) -> i32 {
        self.planet(p)[0x6e] as i32
    }
    fn radar_range(&self, e: i32, level: i32) -> f32 {
        self.mem().f32(va::RADAR2 + 4 * (e.max(0) as u32 * 8 + level.max(0) as u32))
    }
    fn fleet_xy(&self, f: i32) -> (f32, f32) {
        self.fleet(f).map_or((0.0, 0.0), |r| (rd_f32(r, 0x30), rd_f32(r, 0x34)))
    }
    fn fleet_order(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0, |r| rd_i16(r, 0x42))
    }
    fn fleet_5c(&self, f: i32) -> i32 {
        self.fleet_i32(f, 0x5c)
    }
    fn ship_count(&self) -> i32 {
        self.game.state.ships.len() as i32 - 1
    }
    fn ship(&self, k: i32) -> (i32, i32) {
        usize::try_from(k).ok().and_then(|k| self.game.state.ships.get(k)).map_or((0, 0x7fff), |s| (rd_i16(s, 0), rd_i16(s, 2)))
    }
    fn design_upgrade(&self, e: i32, d: i32) -> i32 {
        self.design_i32(e, d, 0x1c)
    }
    fn design_price(&self, e: i32, d: i32) -> i32 {
        self.design_i32(e, d, 0x24)
    }
    fn design_is_ground(&self, e: i32, d: i32) -> bool {
        self.design(e, d).is_some_and(|r| rd_i16(r, 0x16) == 2 || r[0x36] != 0)
    }
    fn planet_has_info(&self, p: i32, info: i32) -> bool {
        let r = self.planet(p);
        (1..=5).any(|k| rd_i16(&r, 0x168 + 2 * k) == info)
    }
    fn empire_strength(&self, e: i32) -> f32 {
        self.mem().f32(va::EMPIRE_POWER + 4 * e.max(0) as u32)
    }
    fn ai_design(&self, e: i32, slot: i32) -> i32 {
        self.mem().i16(va::AI_DESIGNS + 2 * (e.max(0) as u32 * 25 + slot.max(0) as u32)) as i32
    }
    fn in_radar_range(&mut self, x: i32, y: i32, k: i32) -> bool {
        self.game.in_radar_range(x, y, k)
    }
    fn story_design(&mut self, e: i32, key: i32, default: i32) -> i32 {
        if !(0..EMPIRES as i32).contains(&e) {
            return default;
        }
        let designs = self.game.state.story_designs();
        self.with_game(|g| crate::trade::story_design(e as usize, key, default.max(0) as usize, &designs, &mut TradeView { game: g }) as i32)
    }
    fn new_fleet(&mut self, name: &FleetName, owner: i32, id: i32, x: f32, y: f32, speed: f32) -> i32 {
        let name = match *name {
            FleetName::Bytes(ref b) => b.clone(),
            FleetName::Text(t) => self.story_text(t),
            FleetName::Format(f, a) => crate::messages::printf(&self.story_text(f), &[crate::messages::Arg::Str(cstr(&self.story_text(a)))]),
            FleetName::Design { empire, design } => self.design(empire, design).map_or(Vec::new(), |r| cstr(&r[..0x16]).to_vec()),
        };
        self.with_game(|g| g.new_fleet(&name, owner as u8, id as i16, x, y, speed) as i32)
    }
    fn info_string(&self, info: i32) -> bool {
        self.game.info_string(info)
    }
    fn ctf_value(&self, side: i32) -> i32 {
        if (0..3).contains(&side) { self.mem().i32(va::CTF_VALUE + 4 * side as u32) } else { 0 }
    }
    fn new_design(&mut self, e: i32, record: &[u8; DESIGN]) -> i32 {
        self.new_design_with(e, record, true)
    }

    fn new_design_raw(&mut self, e: i32, record: &[u8; DESIGN]) -> i32 {
        self.new_design_with(e, record, false)
    }
    fn design_count(&self, e: i32) -> i32 {
        self.mem().i32(va::DESIGN_COUNT + 4 * e.max(0) as u32)
    }
    fn design_key(&self, e: i32, d: i32) -> i32 {
        self.design_i32(e, d, 0x80)
    }
    fn design_attack(&self, e: i32, d: i32) -> i32 {
        self.design_i32(e, d, 0x2c)
    }
    fn tech_prereqs(&self, t: i32) -> [i32; 3] {
        let at = Self::tech_at(t);
        std::array::from_fn(|k| self.mem().u8(at + 0x2d + k as u32) as i32)
    }
    fn battle_fleets(&self) -> (i32, i32) {
        (self.mem().i16(va::BATTLE_FLEETS) as i32, self.mem().i16(va::BATTLE_FLEETS + 6) as i32)
    }
    fn fleet_command(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0, |r| rd_i16(r, 0x40))
    }
    fn fleet_target(&self, f: i32) -> f32 {
        self.fleet(f).map_or(0.0, |r| rd_f32(r, 0x44))
    }
    fn fleet_state(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0, |r| r[0x18] as i32)
    }
    fn planet_production(&self, p: i32) -> (i32, i32) {
        let r = self.planet(p);
        (rd_i32(&r, 0x70), rd_i32(&r, 0x74))
    }
    fn building_workers(&self, kind: i32) -> i32 {
        self.game.rules.side.colony.buildings.get(kind.max(0) as usize).map_or(0, |b| b.workers)
    }
    fn wormhole_shown(&self) -> bool {
        (1..=self.wormhole_count()).any(|w| self.wormhole(w).iter().all(|e| e.0))
    }

    fn fleet_power(&self, f: i32) -> i32 {
        self.fleet_i32(f, 0x60)
    }
    fn building_count(&self, p: i32) -> i32 {
        self.mem().i32(va::BUILDING_SLOTS + 4 * p.max(0) as u32)
    }
    fn building(&self, p: i32, k: i32) -> (i32, i32, i32) {
        self.building_rec(p, k).map_or((0, 0, 0), |b| (b[0] as i32, rd_i32(b, 4), rd_i16(b, 8)))
    }
    fn building_hp(&self, kind: i32) -> i32 {
        self.game.rules.side.colony.buildings.get(kind.max(0) as usize).map_or(0, |b| b.max_hp)
    }
    fn building_tech(&self, kind: i32) -> i32 {
        self.game.rules.side.colony.buildings.get(kind.max(0) as usize).map_or(0, |b| b.tech as i32)
    }
    fn tank_count(&self) -> i32 {
        self.mem().i32(va::VEHICLE_COUNT)
    }
    fn tank(&self, k: i32) -> (i32, i32, i32) {
        if !(0..=self.tank_count()).contains(&k) {
            return (0, 0x7fff, 0);
        }
        let r = self.mem().read(va::VEHICLES + 8 * k as u32, 8);
        (rd_i16(&r, 0), rd_i16(&r, 2), rd_i32(&r, 4))
    }
    fn design_class(&self, e: i32, d: i32) -> i32 {
        self.design(e, d).map_or(0, |r| rd_i16(r, 0x16))
    }
    fn design_size(&self, e: i32, d: i32) -> i32 {
        self.design_i32(e, d, 0x44)
    }
    fn design_colony(&self, e: i32, d: i32) -> bool {
        self.design(e, d).is_some_and(|r| r[0x34] != 0)
    }
    fn trade_offer_count(&self) -> i32 {
        self.mem().i32(va::TRADE_OFFER_COUNT)
    }
    fn trade_offer(&self, k: i32) -> (i32, i32) {
        let r = self.mem().read(va::TRADE_OFFERS + (k.max(0) as usize * crate::trade::RECORD) as u32, crate::trade::RECORD);
        (rd_i16(&r, 0x16), rd_i32(&r, 0x80))
    }
    fn trade_agreement(&self, a: i32, b: i32) -> bool {
        self.mem().u8(va::RELATIONS + ((a.max(0) as usize * EMPIRES + b.max(0) as usize) * 0x26) as u32 + 0x24) != 0
    }
    fn planet_epidemic(&self, p: i32) -> i32 {
        rd_i16(&self.planet(p), 0xb8)
    }
    fn tech_priority(&self, t: i32) -> i32 {
        self.mem().u8(Self::tech_at(t) + 0xb8) as i32
    }
    fn tech_research(&self, t: i32) -> (i32, i32) {
        let at = Self::tech_at(t);
        (self.mem().i32(at + 0x34), self.mem().i32(at + 0x38))
    }
    fn researching(&self, e: i32) -> i32 {
        self.mem().i32(va::RESEARCHING + 4 * e.max(0) as u32)
    }
    fn spy_count(&self, e: i32) -> i32 {
        self.mem().i32(va::SPY_COUNT + 4 * e.max(0) as u32)
    }
    fn spy_record(&self, e: i32, k: i32) -> SpyRecord {
        let Some(at) = Self::spy_at(e, k) else { return SpyRecord::default() };
        let r = self.mem().read(at, crate::spy::RECORD);
        let agent = self.game.rules.messages.story_text(3092);
        SpyRecord {
            state: rd_i16(&r, 0x10),
            race: r[0x12] as i32,
            target: r[0x18] as i32,
            mission: rd_i16(&r, 0x1a),
            skill: (r[0x32] as i32, r[0x33] as i32),
            skill_31: r[0x31] as i32,
            active: rd_i16(&r, 0x16),
            agent: !agent.is_empty() && cstr(&r[..0x10]) == cstr(agent),
            name: r[..0x10].try_into().unwrap(),
        }
    }

    fn planet_morale(&self, p: i32) -> i32 {
        rd_i32(&self.planet(p), 0x30)
    }
    fn planet_surface(&self, p: i32) -> i32 {
        self.planet(p)[0x22] as i32
    }
    fn planet_richness(&self, p: i32) -> i32 {
        rd_i32(&self.planet(p), 0x24)
    }

    fn fleet_flags(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0, |r| u16::from_le_bytes([r[0x52], r[0x53]]) as i32)
    }
    fn spy_rank_xp(&self, rank: i32) -> i32 {
        self.game.rules.spy.rank_xp.get(rank.max(0) as usize).copied().unwrap_or(0)
    }
    fn new_spy(&mut self, e: i32) -> i32 {
        if !(0..crate::spy::ROSTERS as i32).contains(&e) {
            return 0;
        }
        self.with_game(|g| g.new_spy(e as usize))
    }
    fn lab_level(&self, e: i32, k: i32) -> i32 {
        self.mem().i32(va::LABS + 4 * (e.max(0) as u32 * 4 + k.clamp(0, 3) as u32))
    }
    fn wormhole_count(&self) -> i32 {
        self.mem().i32(va::WORMHOLE_COUNT)
    }
    fn wormhole(&self, w: i32) -> [(bool, i32); 2] {
        let r = self.mem().read(va::WORMHOLES + 0x1c * w.max(0) as u32, 0x1c);
        [(r[8] != 0, rd_i16(&r, 0xa)), (r[0x14] != 0, rd_i16(&r, 0x16))]
    }

    fn planet_extra(&self, p: i32) -> bool {
        self.planet(p)[0x187] != 0
    }
    fn tech_time_left(&self, t: i32, e: i32) -> i32 {
        self.mem().i32(Self::tech_at(t) + 0x58 + 4 * e.max(0) as u32)
    }
    fn building_bytes(&self, p: i32, k: i32) -> (i32, i32) {
        self.building_rec(p, k).map_or((0, 0), |b| (b[0x18] as i32, b[0x19] as i32))
    }
    fn conquest_ends_epidemic(&mut self, p: i32) -> bool {
        let owner = self.planet_owner(p);
        let (px, py) = self.planet_xy(p);
        let planets = (1..=self.planet_count()).any(|q| q != p && self.planet_infected(q));
        let fleets = (1..=self.fleet_count()).any(|f| {
            let (x, y) = self.fleet_xy(f);
            let (dx, dy) = (x - px, y - py);
            self.fleet_flags(f) & 4 != 0 && !(self.fleet_owner(f) == owner && dy * dy + dx * dx < 100.0)
        });
        !planets && !fleets
    }
    fn fleet_ship_count(&self, f: i32) -> i32 {
        self.fleet_i32(f, 0x54)
    }
    fn near_fleet(&mut self, p: i32, radius: f32) -> i32 {
        self.near_fleet_ex(p, radius, true, true, false, false, 0)
    }
    fn near_fleet_ex(&mut self, p: i32, radius: f32, own: bool, idle: bool, room: bool, calm: bool, last: i32) -> i32 {
        if !(0..=self.planet_count()).contains(&p) {
            return 0;
        }
        self.game.shipyard().delivery_fleet(p as usize, radius, own, idle, room, calm, last) as i32
    }
    fn design_kind(&self, e: i32, d: i32) -> i32 {
        self.design_class(e, d)
    }
    fn spy_rank(&self, e: i32, k: i32) -> i32 {
        Self::spy_at(e, k).map_or(0, |at| self.mem().u8(at + 0x40) as i32)
    }
    fn recruit_name(&self, k: i32) -> Vec<u8> {
        self.game.rules.story.recruit_names.get(k.max(0) as usize).cloned().unwrap_or_default()
    }
    fn spy_mission_on(&self, k: i32) -> bool {
        self.mem().u8(va::MISSIONS + 0x20 * k.max(0) as u32) != 0
    }
    fn fleet_named(&self, f: i32, text: i32) -> bool {
        let want = cstr(self.game.rules.messages.story_text(text));
        self.fleet(f).is_some_and(|r| cstr(&r[..0x15]) == want)
    }
    fn fleet_units(&self, f: i32) -> (i32, i32) {
        (self.fleet_i32(f, 0x5c), self.fleet_i32(f, 0x78))
    }

    fn battlefield_units(&self, p: i32) -> Vec<(bool, i32)> {
        self.battlefield_records(p).into_iter().map(|u| (u.alive, u.owner)).collect()
    }
    fn buildings_occupied(&self, p: i32) -> i32 {
        self.mem().i32(va::BUILDINGS_OCCUPIED + 4 * p.max(0) as u32)
    }

    fn planet_garrison(&self, p: i32) -> i32 {
        rd_i32(&self.planet(p), 0x58)
    }
    fn battlefield_records(&self, p: i32) -> Vec<FieldUnit> {
        let Some(field) = usize::try_from(p).ok().and_then(|p| self.game.state.ground_field(p)) else { return Vec::new() };
        field
            .units
            .iter()
            .zip(&field.alive)
            .map(|(u, &alive)| FieldUnit {
                alive,
                owner: u.owner,
                flags: u.flags,
                flags2: u.flags2 as u8,
                count: u.hp.len() as i32,
                design: u.design,
            })
            .collect()
    }
    fn copy_design_new(&mut self, e: i32, from: i32, d: i32) -> i32 {
        let Some(record) = self.design(from, d).copied() else { return 0 };
        self.new_design_with(e, &record, false)
    }

    fn story_text(&self, id: i32) -> Vec<u8> {
        self.game.rules.messages.story_text(id).to_vec()
    }
    fn design_hull(&self, e: i32, d: i32) -> i32 {
        self.design_i32(e, d, 0x48)
    }
    fn design_36(&self, e: i32, d: i32) -> i32 {
        self.design(e, d).map_or(0, |r| r[0x36] as i32)
    }

    fn fleet_fighters(&self, f: i32) -> i32 {
        self.fleet_i32(f, 0x58)
    }
    fn design_44(&self, e: i32, d: i32) -> i32 {
        self.design_i32(e, d, 0x44)
    }
    fn date(&self) -> [i32; 3] {
        [self.g.year, self.g.month, self.g.day]
    }
}

impl Game<'_> {
    /// `st_IsInRR(x, y, k)` (exe 0x5eb300, after `GetENRadarKifejlesztve`): the point is inside the radar of one of
    /// the human's planets it sees (`k & 1`; range `RadarHatotav2[EN][+0x6e]`, the level unclamped as the exe's
    /// index runs on into the next row and the squares) or of its fleets (`k & 2`; range `+0x7c`). f32, strict.
    pub(super) fn in_radar_range(&self, x: i32, y: i32, k: i32) -> bool {
        let en = self.globals().en;
        let t = self.radar_tables();
        let flat: Vec<f32> = t.range.iter().chain(&t.range_sq).flatten().copied().collect();
        let (x, y) = (x as f32, y as f32);
        let hit = |px: f32, py: f32, r: f32| {
            let (dx, dy) = (px - x, py - y);
            dy * dy + dx * dx < r * r
        };
        if k & 1 != 0 {
            for p in 1..=self.state.planet_count() {
                let r = self.state.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE);
                if r[0x20] as usize == en && rd_i16(&r, 0xd8 + 2 * en) >= 0 {
                    let range = flat.get(r[0x6e] as usize + en * 8).copied().unwrap_or(0.0);
                    if hit(rd_f32(&r, 0x18), rd_f32(&r, 0x1c), range) {
                        return true;
                    }
                }
            }
        }
        if k & 2 != 0 {
            for f in self.state.fleets.iter().skip(1) {
                if f[0x15] as usize == en && hit(rd_f32(f, 0x30), rd_f32(f, 0x34), rd_f32(f, 0x7c)) {
                    return true;
                }
            }
        }
        false
    }

    /// `InfoString_GetIndex(info) != 0` (exe 0x53ec40): `infostr.txt` has `info` past its first record.
    pub(super) fn info_string(&self, info: i32) -> bool {
        self.rules.story.info_strings.iter().position(|&(id, _)| id as i32 == info).is_some_and(|k| k > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameRules;
    use crate::save::SaveState;

    #[test]
    fn reads_follow_the_exe_layouts() {
        let rules = GameRules { story: crate::game::StoryRules { info_strings: vec![(0, false), (-1381, true), (-10011, true)], ..Default::default() }, ..Default::default() };
        let mut state = SaveState::from_parts(crate::save::Memory::default(), Vec::new());
        let m = &mut state.mem;
        m.write(va::PLANETS, &vec![0; 3 * PLANET_SIZE]);
        m.set_i32(va::PLANET_COUNT, 2);
        let p2 = va::PLANETS + 2 * PLANET_SIZE as u32;
        m.set_f32(p2 + 0x18, 12.5);
        m.set_f32(p2 + 0x1c, -3.0);
        m.write(p2 + 0x16a + 2 * 4, &101i16.to_le_bytes());
        m.write(va::VEHICLES, &[0; 16]);
        m.set_i32(va::VEHICLE_COUNT, 1);
        m.write(va::VEHICLES + 8, &[0xfe, 0xff, 7, 0, 5, 0, 0, 0]);
        m.write(va::BATTLE_FLEETS, &[3, 0, 0, 0, 0, 0, 9, 0]);
        m.set_i32s(va::CTF_VALUE, &[0, 40, 7]);
        m.write(va::WORMHOLES, &[0; 0x38]);
        m.set_u8(va::WORMHOLES + 0x1c + 8, 1);
        m.write(va::WORMHOLES + 0x1c + 0xa, &4i16.to_le_bytes());
        m.write(va::KEMEK, &[0; crate::spy::RECORD * 15]);
        let spy = va::KEMEK + ((crate::spy::SLOTS + 2) * crate::spy::RECORD) as u32;
        m.write(spy, b"Lolka\0");
        m.write(spy + 0x10, &3i16.to_le_bytes());
        m.set_u8(spy + 0x40, 4);
        let mut d = [0u8; DESIGN];
        d[0x16] = 2;
        d[0x80..0x84].copy_from_slice(&8401i32.to_le_bytes());
        state.designs = vec![Vec::new(), vec![[0; DESIGN], d]];
        let mut game = Game::new(state, &rules);
        let w = GameStory::new(&mut game);
        assert_eq!(w.planet_xy(2), (12.5, -3.0));
        assert!(w.planet_has_info(2, 101) && !w.planet_has_info(2, 100));
        assert_eq!(w.tank(1), (-2, 7, 5));
        assert_eq!(w.tank(2), (0, 0x7fff, 0));
        assert_eq!(w.battle_fleets(), (3, 9));
        assert_eq!((w.ctf_value(1), w.ctf_value(2)), (40, 7));
        assert_eq!(w.wormhole(1), [(true, 4), (false, 0)]);
        assert!(!w.wormhole_shown());
        let s = w.spy_record(1, 2);
        assert_eq!((s.state, &s.name[..5]), (3, &b"Lolka"[..]));
        assert_eq!(w.spy_rank(1, 2), 4);
        assert_eq!((w.design_class(1, 1), w.design_key(1, 1)), (2, 8401));
        assert!(w.design_is_ground(1, 1) && !w.design_is_ground(1, 0) && !w.design_is_ground(2, 1));
        // `InfoString_GetIndex`: index 0 ("none") counts as missing.
        assert!(game_info(&w, -10011) && !game_info(&w, 0) && !game_info(&w, 5));
    }

    fn game_info(w: &GameStory, info: i32) -> bool {
        w.info_string(info)
    }
}
