//! Player commands (ADR 0007 §1): one variant per lockstep message of the original, applied at a tick boundary
//! as its `MULTIGET_*` handler applies it.

use ig2_sim::diplomacy::{Answer, Letter, Message, ScreenOp};
use ig2_sim::game::{ColonyOrder, DesignOrder, Game, GameEvent, InfoOrder, MessageOrder, ResearchOrder, StarmapOrder};
use ig2_sim::spacebattle::Order;
use ig2_sim::spy;
use ig2_sim::story::StoryEvent;
use ig2_sim::trade::{Delivery, DELIVERY};
use serde::{Deserialize, Serialize};

/// A diplomatic message's terms (`ig2_sim::diplomacy::Message`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terms {
    pub cat: i32,
    pub sub: i32,
    pub value: i32,
    pub sub2: i32,
    pub value2: i32,
}

impl Terms {
    /// Terms a diplomacy screen can build (`re/rules/diplomacy.md` §6.3): every item's index in range. The original
    /// only ever receives what its own screen built; a peer's terms are checked here, so a malformed message is
    /// dropped on every machine alike instead of indexing past the empire, planet or tech tables.
    pub fn in_range(&self, planets: usize) -> bool {
        use ig2_sim::diplomacy::{cat, item};
        let empire = |v: i32| (1..=8).contains(&v);
        let item = |sub: i32, value: i32| match sub {
            item::MONEY => value >= 0,
            item::RADAR | item::WAR | item::PEACE => empire(value),
            item::PLANET => (1..=planets as i32).contains(&value),
            item::TECH => (0..ig2_sim::colony_side::TECHS as i32).contains(&value),
            item::HOSTAGE => (1..=5).contains(&value),
            _ => false,
        };
        match self.cat {
            // The third empire (`WAR_TOGETHER`, the AI's `target`), any of the 19 empire slots.
            cat::NEGOTIATE => (0..=8).contains(&self.sub) && (-1..19).contains(&self.value),
            cat::OFFER_FOR => item(self.sub, self.value) && item(self.sub2, self.value2),
            cat::DEMAND | cat::OFFER => item(self.sub, self.value),
            cat::SPECIAL => (0..0x100).contains(&self.sub),
            cat::TALK => (0..=9).contains(&self.sub),
            _ => false,
        }
    }
}

impl From<Terms> for Message {
    fn from(t: Terms) -> Self {
        Message { cat: t.cat, sub: t.sub, value: t.value, sub2: t.sub2, value2: t.value2 }
    }
}

/// What the diplomacy screen writes to its own contact-list slot (`ig2_sim::diplomacy::ScreenOp`,
/// `re/screens/diplomacy.md` §9): the original writes these flags in the frame it sends the message or answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiplomacyScreenOp {
    Sent,
    Answered,
    AnswerSeen,
    CounterSeen,
    Cancelled,
    Greeted,
}

impl From<DiplomacyScreenOp> for ScreenOp {
    fn from(o: DiplomacyScreenOp) -> Self {
        match o {
            DiplomacyScreenOp::Sent => ScreenOp::Sent,
            DiplomacyScreenOp::Answered => ScreenOp::Answered,
            DiplomacyScreenOp::AnswerSeen => ScreenOp::AnswerSeen,
            DiplomacyScreenOp::CounterSeen => ScreenOp::CounterSeen,
            DiplomacyScreenOp::Cancelled => ScreenOp::Cancelled,
            DiplomacyScreenOp::Greeted => ScreenOp::Greeted,
        }
    }
}

/// A spy-screen order (`ig2_sim::spy::Order`, `MULTIGET_Spy`'s messages 0x47..0x4c; `re/rules/espionage.md` §8;
/// the screen's own steps, re/screens/spy.md §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpyOrder {
    Dismiss { spy: u8 },
    Recall { spy: u8 },
    Send { spy: u8, mission: u16, target: u8, param1: i16, param2: i16, days: i32 },
    Verdict { fate: i32, owner: u8, spy: u8 },
    Applicant { choice: i32, spy: u8 },
    Skill { spy: u8, skill: u8 },
    /// Ours (re/screens/spy.md §7): the verdict buttons with the UI's stream-0 roll, the report closed, the chance
    /// estimate's stream-0 draws.
    Judge { choice: i32, owner: u8, spy: u8 },
    Acknowledge,
    Estimate { spy: u8, mission: u16, target: u8 },
}

impl From<SpyOrder> for spy::Order {
    fn from(o: SpyOrder) -> Self {
        use spy::Order;
        match o {
            SpyOrder::Dismiss { spy } => Order::Dismiss { spy },
            SpyOrder::Recall { spy } => Order::Recall { spy },
            SpyOrder::Send { spy, mission, target, param1, param2, days } => Order::Send { spy, mission, target, param1, param2, days },
            SpyOrder::Verdict { fate, owner, spy } => Order::Verdict { fate, owner, spy },
            SpyOrder::Applicant { choice, spy } => Order::Applicant { choice, spy },
            SpyOrder::Skill { spy, skill } => Order::Skill { spy, skill },
            SpyOrder::Judge { choice, owner, spy } => Order::Judge { choice, owner, spy },
            SpyOrder::Acknowledge => Order::Acknowledge,
            SpyOrder::Estimate { spy, mission, target } => Order::Estimate { spy, mission, target },
        }
    }
}

/// A battle-screen order (`ig2_sim::spacebattle::Order`, `MULTIGET_SpaceWar`'s messages 0x70..0x77, 0x44e, 0x44f;
/// `re/rules/space-combat.md` §7.12). Coordinates travel as f32 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleOrder {
    Move { unit: u16, x: u32, y: u32, z: u32 },
    Attack { unit: u16, target: u16 },
    Retreat,
    AttackGun { unit: u16, gun: u16 },
    GunTarget { gun: u16, unit: u16 },
    ArmAll,
    Explode { unit: u16 },
    FighterStance { side: u8, value: i32 },
    DirectFire { unit: u16, target: u16, mount: u16 },
    Weapons { unit: u16, keep: u32, set: u32 },
    Strengths { values: [i32; ig2_sim::spacebattle::EMPIRES] },
    Outgunned,
}

impl From<BattleOrder> for Order {
    fn from(o: BattleOrder) -> Self {
        let u = |x: u16| x as usize;
        match o {
            BattleOrder::Move { unit, x, y, z } => Order::Move { unit: u(unit), x: f32::from_bits(x), y: f32::from_bits(y), z: f32::from_bits(z) },
            BattleOrder::Attack { unit, target } => Order::Attack { unit: u(unit), target: u(target) },
            BattleOrder::Retreat => Order::Retreat,
            BattleOrder::AttackGun { unit, gun } => Order::AttackGun { unit: u(unit), gun: u(gun) },
            BattleOrder::GunTarget { gun, unit } => Order::GunTarget { gun: u(gun), unit: u(unit) },
            BattleOrder::ArmAll => Order::ArmAll,
            BattleOrder::Explode { unit } => Order::Explode { unit: u(unit) },
            BattleOrder::FighterStance { side, value } => Order::FighterStance { side: side as usize, value },
            BattleOrder::DirectFire { unit, target, mount } => Order::DirectFire { unit: u(unit), target: u(target), mount: u(mount) },
            BattleOrder::Weapons { unit, keep, set } => Order::Weapons { unit: u(unit), keep, set },
            BattleOrder::Strengths { values } => Order::Strengths { values },
            BattleOrder::Outgunned => Order::Outgunned,
        }
    }
}

impl From<Order> for BattleOrder {
    fn from(o: Order) -> Self {
        let u = |x: usize| x as u16;
        match o {
            Order::Move { unit, x, y, z } => BattleOrder::Move { unit: u(unit), x: x.to_bits(), y: y.to_bits(), z: z.to_bits() },
            Order::Attack { unit, target } => BattleOrder::Attack { unit: u(unit), target: u(target) },
            Order::Retreat => BattleOrder::Retreat,
            Order::AttackGun { unit, gun } => BattleOrder::AttackGun { unit: u(unit), gun: u(gun) },
            Order::GunTarget { gun, unit } => BattleOrder::GunTarget { gun: u(gun), unit: u(unit) },
            Order::ArmAll => BattleOrder::ArmAll,
            Order::Explode { unit } => BattleOrder::Explode { unit: u(unit) },
            Order::FighterStance { side, value } => BattleOrder::FighterStance { side: side as u8, value },
            Order::DirectFire { unit, target, mount } => BattleOrder::DirectFire { unit: u(unit), target: u(target), mount: u(mount) },
            Order::Weapons { unit, keep, set } => BattleOrder::Weapons { unit: u(unit), keep, set },
            Order::Strengths { values } => BattleOrder::Strengths { values },
            Order::Outgunned => BattleOrder::Outgunned,
        }
    }
}

/// A starmap order (`ig2_sim::game::StarmapOrder`, `MULTIGET_Starmap`'s messages 0x14..0x1d, 0x400, 0x403, 0x406,
/// 0x407, 0x40f; re/screens/starmap.md §10). Coordinates travel as f32 bits, the production block as its 64 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StarmapCmd {
    Split { fleet: i32 },
    Move { fleet: i32, order: i32, kind: i32, x: u32, y: u32 },
    Colonise { planet: i32, kind: i32 },
    SellStation { planet: i32, index: i32 },
    Satellite { planet: i32, kind: i32 },
    Setting { planet: i32, what: i32, value: i32 },
    Queue { planet: i32, block: Vec<u8> },
    Transfer { from: i32, to: i32, design: i32, ship: i32, count: i32 },
    Production { op: i32, planet: i32, line: i32, slot: i32, value: i32 },
    Rally { planet: i32, target: i32 },
    FinishHeads { planet: i32 },
    MpSpeed { value: i32 },
    Speed { level: i32 },
    Pause { paused: bool },
}

impl StarmapCmd {
    /// The sim's order; `None` for a malformed production block.
    pub fn order(&self) -> Option<StarmapOrder> {
        use StarmapCmd as C;
        use StarmapOrder as O;
        Some(match *self {
            C::Split { fleet } => O::Split { fleet },
            C::Move { fleet, order, kind, x, y } => O::Move { fleet, order, kind, x: f32::from_bits(x), y: f32::from_bits(y) },
            C::Colonise { planet, kind } => O::Colonise { planet, kind },
            C::SellStation { planet, index } => O::SellStation { planet, index },
            C::Satellite { planet, kind } => O::Satellite { planet, kind },
            C::Setting { planet, what, value } => O::Setting { planet, what, value },
            C::Queue { planet, ref block } => O::Queue { planet, block: block[..].try_into().ok()? },
            C::Transfer { from, to, design, ship, count } => O::Transfer { from, to, design, ship, count },
            C::Production { op, planet, line, slot, value } => O::Production { op, planet, line, slot, value },
            C::Rally { planet, target } => O::Rally { planet, target },
            C::FinishHeads { planet } => O::FinishHeads { planet },
            C::MpSpeed { value } => O::MpSpeed { value },
            C::Speed { level } => O::Speed { level },
            C::Pause { paused } => O::Pause { paused },
        })
    }
}

impl From<StarmapOrder> for StarmapCmd {
    fn from(o: StarmapOrder) -> Self {
        use StarmapCmd as C;
        use StarmapOrder as O;
        match o {
            O::Split { fleet } => C::Split { fleet },
            O::Move { fleet, order, kind, x, y } => C::Move { fleet, order, kind, x: x.to_bits(), y: y.to_bits() },
            O::Colonise { planet, kind } => C::Colonise { planet, kind },
            O::SellStation { planet, index } => C::SellStation { planet, index },
            O::Satellite { planet, kind } => C::Satellite { planet, kind },
            O::Setting { planet, what, value } => C::Setting { planet, what, value },
            O::Queue { planet, block } => C::Queue { planet, block: block.to_vec() },
            O::Transfer { from, to, design, ship, count } => C::Transfer { from, to, design, ship, count },
            O::Production { op, planet, line, slot, value } => C::Production { op, planet, line, slot, value },
            O::Rally { planet, target } => C::Rally { planet, target },
            O::FinishHeads { planet } => C::FinishHeads { planet },
            O::MpSpeed { value } => C::MpSpeed { value },
            O::Speed { level } => C::Speed { level },
            O::Pause { paused } => C::Pause { paused },
        }
    }
}

/// A research-screen order (`ig2_sim::game::ResearchOrder`, `MULTIGET_Research`'s messages 0x29, 0x2a, 0x2b, 0x31;
/// re/screens/research.md §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchCmd {
    Stop { tech: i32 },
    Start { tech: i32 },
    Urgent,
    DevelopAll,
}

impl From<ResearchCmd> for ResearchOrder {
    fn from(o: ResearchCmd) -> Self {
        match o {
            ResearchCmd::Stop { tech } => ResearchOrder::Stop { tech },
            ResearchCmd::Start { tech } => ResearchOrder::Start { tech },
            ResearchCmd::Urgent => ResearchOrder::Urgent,
            ResearchCmd::DevelopAll => ResearchOrder::DevelopAll,
        }
    }
}

impl From<ResearchOrder> for ResearchCmd {
    fn from(o: ResearchOrder) -> Self {
        match o {
            ResearchOrder::Stop { tech } => ResearchCmd::Stop { tech },
            ResearchOrder::Start { tech } => ResearchCmd::Start { tech },
            ResearchOrder::Urgent => ResearchCmd::Urgent,
            ResearchOrder::DevelopAll => ResearchCmd::DevelopAll,
        }
    }
}

/// A player's action. The acting empire (the issuer) isn't part of it: the host stamps it from the sender's slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Packet 0x51, `MULTIGET_Trade` exe 0x6070b0: buy an offer (`trade::Delivery`, `DELIVERY` bytes).
    Buy { delivery: Vec<u8> },
    /// Packet 0x3e, `MULTIGET_Diplomacy` exe 0x4c7880 → `FeldolgozAjanlat`: a message to empire `to`.
    DiplomacyMessage { to: i32, declaration: bool, terms: Terms },
    /// Packet 0x3f → `FeldolgozValasz`: an answer to proposer `to`.
    DiplomacyAnswer { to: i32, terms: Terms, counter: bool, reply: bool, yes: bool },
    /// Packet 0x66, `MULTIGET_Story`: `Story_Special(15, issuer, id)` (a story's choice sent to every player).
    StoryChoice { id: i32 },
    /// Packet 0x1b, `MULTIGET_Starmap` ELF 0x719680: delete fleet `fleet` if it's still empty. Sim-originated: every
    /// peer queues it from its own tick ([`Command::from_event`]).
    FleetEmptied { fleet: u32 },
    /// Packets 0x47..0x4c, `MULTIGET_Spy` exe 0x5883c0: an order from the spy screen.
    Spy { order: SpyOrder },
    /// Packet 0x65, `MULTIGET_Story`: `StartStory(id, false)`.
    StoryStart { id: i32 },
    /// Packets 0x70..0x77, 0x44e, 0x44f, `MULTIGET_SpaceWar` exe 0x5615b0: an order on the battle screen of slot
    /// `battle`.
    SpaceOrder { battle: u8, order: BattleOrder },
    /// Packet 0x44d: the issuer left the finished battle's screen (`SpaceWarVege` once every human has).
    SpaceBattleLeft,
    /// Packets 0x14..0x1d, 0x400, 0x403, 0x406, 0x407, 0x40f, `MULTIGET_Starmap` exe 0x59d930: a starmap order.
    Starmap { order: StarmapCmd },
    /// Packets 0x1f..0x21, 0x26, 0x27, 300, 0x132, 0x133, `MULTIGET_Colony` exe 0x48a270: a colony-screen order
    /// (`ig2_sim::game::ColonyOrder`) as its message type and values.
    Colony { kind: i32, values: Vec<i32> },
    /// The diplomacy screen's own writes to the issuer's contact-list slot of `empire` (no packet of its own in the
    /// original: `DiplomGfx2D_1` writes them before `ElkuldUzenet`/`ElkuldValasz`).
    DiplomacyScreen { empire: u8, op: DiplomacyScreenOp },
    /// Packets 0x29, 0x2a, 0x2b, 0x31, `MULTIGET_Research` exe 0x51ee70: a research-screen order.
    Research { order: ResearchCmd },
    /// Packets 0x5b, 0x5c, `MULTIGET_Info` exe 0x4ece60: the info screen's money shares or spending limit
    /// (`ig2_sim::game::InfoOrder`) as its message type and dwords (each f64 as its two words).
    Info { kind: i32, values: Vec<i32> },
    /// Ours (ADR 0011 §5), sent only by the host ([`crate::lockstep::Host::inject`]): the issuer's player dropped
    /// and didn't come back in time; it has left every running space battle's screen.
    PlayerAway,
    /// Ours: the issuer's player rejoined; on the running space battle's screen again.
    PlayerBack,
    /// Packets 0x33..0x35, `MULTIGET_Design` exe 0x4b4d60: a design stored, a design's units sold or refitted
    /// (`ig2_sim::game::DesignOrder`) as its message type and dwords.
    Design { kind: i32, values: Vec<i32> },
    /// Ours (no packet in the original): the messages screen's writes to the human's inbox and their story
    /// hand-offs (`ig2_sim::game::MessageOrder`, re/rules/info-messages.md §7) as its kind and values.
    Message { kind: i32, values: Vec<i32> },
    /// Ours (no packet: the exe calls it straight from the fleet panel's sale menu, single player only): a story hero's
    /// design sent back (`Story_Special(0x19, fleet, design)`, re/screens/starmap.md §19.7).
    HeroSentBack { fleet: i32, design: i32 },
    /// Ours (no packet: the exe calls the sim straight from `Starmap_Main`): the radial menu's story icon `icon`
    /// (0x10 story fleet's wormhole, 0x11 virus alert, 0x12 gigabomb) on fleet or planet `target`
    /// ([`Game::story_icon`], re/screens/starmap.md §14.5).
    StoryIcon { icon: i32, target: i32 },
    /// The story's own 0x66 (`SendStorySpecial`) with its sender: `Story_Special(15, sender, id)`. Sim-originated (the
    /// answering human's, `re/rules/multiplayer.md` §5.3); a peer's copy is dropped ([`Command::host_only`]).
    StoryFrom { sender: u8, id: i32 },
    /// A fleet's or a planet's rename (ours, deviation 395: the original's is its own machine's only) with the
    /// fleet's custom-name bit.
    Rename { fleet: bool, index: u32, name: Vec<u8>, custom: bool },
    /// Ours (the original's panel is its own machine's): the issuer's starmap transfer panel shows `fleets` or
    /// closed, which spares its emptied fleets the 0x1b ([`Game::transfer_panel`], re/screens/starmap.md §10.2).
    TransferPanel { fleets: Option<(i32, i32)> },
}

impl From<MessageOrder> for Command {
    fn from(o: MessageOrder) -> Self {
        let (kind, values) = o.to_message();
        Command::Message { kind, values }
    }
}

impl From<DesignOrder> for Command {
    fn from(o: DesignOrder) -> Self {
        let (kind, values) = o.to_message();
        Command::Design { kind, values }
    }
}

impl From<InfoOrder> for Command {
    fn from(o: InfoOrder) -> Self {
        let (kind, values) = o.to_message();
        Command::Info { kind, values }
    }
}

impl From<ColonyOrder> for Command {
    fn from(o: ColonyOrder) -> Self {
        let (kind, values) = o.to_message();
        Command::Colony { kind, values }
    }
}

/// A command with its issuer, as a bundle carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamped {
    pub issuer: u8,
    pub command: Command,
}

impl Command {
    /// Only the host sends it ([`crate::lockstep::Host::inject`]); a peer's copy is dropped.
    pub fn host_only(&self) -> bool {
        matches!(self, Command::PlayerAway | Command::PlayerBack | Command::StoryFrom { .. })
    }

    /// The sim-originated command a tick's event sends (the sim's own `Multi_Send`), if any; the driver applies it
    /// for [`Game::sender`] before the next tick, where the original's `MultiSendPackets` hands it to the handlers
    /// (rules/multiplayer.md §5.5): 0x1b (an emptied fleet), the story's 0x66 (`MULTIGET_Story` →
    /// `Story_Special(15, sender, id)`) and 0x49 (`MULTIGET_Spy`: the spy screen's send with target 0 and 10 days).
    pub fn from_event(e: &GameEvent) -> Option<Command> {
        match e {
            GameEvent::Story(StoryEvent::SendStorySpecial { id, sender }) => Some(Command::StoryFrom { sender: (*sender).clamp(0, 255) as u8, id: *id }),
            GameEvent::Story(StoryEvent::SpyOrder { spy, mission }) => Some(Command::Spy {
                order: SpyOrder::Send { spy: *spy as u8, mission: *mission as u16, target: 0, param1: 0, param2: 0, days: 10 },
            }),
            _ => e.emptied_fleet().map(|f| Command::FleetEmptied { fleet: f as u32 }),
        }
    }

    /// Apply it for empire `issuer`, as the handler does. Invalid input is refused by the sim's own checks (or
    /// here, when it can't even be decoded), identically on every peer.
    pub fn apply(&self, game: &mut Game, issuer: u8) {
        match self {
            Command::Buy { delivery } => {
                if let Ok(raw) = <[u8; DELIVERY]>::try_from(&delivery[..]) {
                    game.buy(issuer as usize, &Delivery(raw));
                }
            }
            Command::DiplomacyMessage { to, terms, .. } | Command::DiplomacyAnswer { to, terms, .. }
                if !(1..=8).contains(to) || !terms.in_range(game.state.planet_count()) => {}
            Command::DiplomacyMessage { to, declaration, terms } => {
                game.diplomacy_message(Letter { from: issuer as i32, to: *to, declaration: *declaration, msg: (*terms).into() });
            }
            Command::DiplomacyAnswer { to, terms, counter, reply, yes } => {
                game.diplomacy_answer(Answer { from: issuer as i32, to: *to, msg: (*terms).into(), counter: *counter, reply: *reply, yes: *yes });
            }
            Command::StoryChoice { id } => game.story_special([15, issuer as i32, *id, 0, 0, 0, 0, 0]),
            Command::StoryFrom { sender, id } => game.story_special([15, *sender as i32, *id, 0, 0, 0, 0, 0]),
            Command::Rename { fleet, index, name, custom } => game.rename(issuer, *fleet, *index as usize, name, *custom),
            Command::HeroSentBack { fleet, design } => game.story_special([0x19, *fleet, *design, 0, 0, 0, 0, 0]),
            Command::StoryIcon { icon, target } => game.story_icon(*icon, *target),
            Command::FleetEmptied { fleet } => game.fleet_emptied(*fleet as usize),
            Command::Spy { order } => game.spy_order(issuer as usize, (*order).into()),
            Command::StoryStart { id } => game.story_start(*id),
            Command::SpaceOrder { battle, order } => {
                game.space_order(issuer, *battle as usize, (*order).into());
            }
            Command::SpaceBattleLeft => game.space_battle_left(issuer),
            Command::Starmap { order } => {
                if let Some(o) = order.order() {
                    game.starmap_order(issuer, o);
                }
            }
            Command::Colony { kind, values } => {
                if let Some(o) = ColonyOrder::from_message(*kind, values) {
                    game.colony_order(issuer, o);
                }
            }
            Command::DiplomacyScreen { empire, op } => game.diplomacy_screen(issuer as usize, *empire as usize, (*op).into()),
            Command::Research { order } => game.research_order(issuer, (*order).into()),
            Command::Info { kind, values } => {
                if let Some(o) = InfoOrder::from_message(*kind, values) {
                    game.info_order(issuer, o);
                }
            }
            Command::PlayerAway => game.player_away(issuer),
            Command::PlayerBack => game.player_back(issuer),
            Command::Design { kind, values } => {
                if let Some(o) = DesignOrder::from_message(*kind, values) {
                    game.design_order(issuer, o);
                }
            }
            Command::Message { kind, values } => {
                if let Some(o) = MessageOrder::from_message(*kind, values) {
                    game.message_order(issuer, o);
                }
            }
            Command::TransferPanel { fleets } => game.transfer_panel(issuer, *fleets),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starmap_orders_survive_the_wire() {
        let orders = [
            StarmapOrder::Move { fleet: 4, order: 14, kind: 0, x: -0.0, y: 123.456 },
            StarmapOrder::Queue { planet: 42, block: std::array::from_fn(|i| i as u8) },
            StarmapOrder::Production { op: 4, planet: 7, line: 1, slot: 2, value: 33 },
            StarmapOrder::Pause { paused: true },
        ];
        for o in orders {
            let c = Command::Starmap { order: StarmapCmd::from(o) };
            let back: Command = crate::wire::from_slice(&crate::wire::to_vec(&c).unwrap()).unwrap();
            let Command::Starmap { order } = back else { panic!() };
            let got = order.order().unwrap();
            assert_eq!(format!("{got:?}"), format!("{o:?}"));
            if let (StarmapOrder::Move { x: a, .. }, StarmapOrder::Move { x: b, .. }) = (got, o) {
                assert_eq!(a.to_bits(), b.to_bits());
            }
        }
        let o = ColonyOrder::Build { planet: 42, kind: 7, x: 100, y: -1 };
        let back: Command = crate::wire::from_slice(&crate::wire::to_vec(&Command::from(o)).unwrap()).unwrap();
        let Command::Colony { kind, values } = back else { panic!() };
        assert_eq!(ColonyOrder::from_message(kind, &values), Some(o));
        let o = DesignOrder::Sell { design: 9, mode: 2, holder: -17 };
        let back: Command = crate::wire::from_slice(&crate::wire::to_vec(&Command::from(o)).unwrap()).unwrap();
        let Command::Design { kind, values } = back else { panic!() };
        assert_eq!(DesignOrder::from_message(kind, &values), Some(o));
        // A block that isn't 64 bytes is refused.
        assert!(StarmapCmd::Queue { planet: 1, block: vec![0; 3] }.order().is_none());
    }

    /// The story's own `Multi_Send`s become commands the driver applies before the next tick, as
    /// `MultiSendPackets` hands them to `MULTIGET_Story` (0x66 → `Story_Special(15, sender, id)`) and `MULTIGET_Spy`
    /// (0x49: the elem stories' `Multi_Send(0x49, EN, spy, mission, 0, 0, 0, 10)`, exe 0x5d4b70).
    #[test]
    fn story_messages_become_commands() {
        use ig2_sim::story::StoryEvent;
        assert_eq!(Command::from_event(&GameEvent::Story(StoryEvent::SendStorySpecial { id: 10031, sender: 3 })), Some(Command::StoryFrom { sender: 3, id: 10031 }));
        assert_eq!(
            Command::from_event(&GameEvent::Story(StoryEvent::SpyOrder { spy: 3, mission: 17 })),
            Some(Command::Spy { order: SpyOrder::Send { spy: 3, mission: 17, target: 0, param1: 0, param2: 0, days: 10 } })
        );
        assert_eq!(Command::from_event(&GameEvent::FleetEmptied { fleet: 4 }), Some(Command::FleetEmptied { fleet: 4 }));
        assert_eq!(Command::from_event(&GameEvent::Story(StoryEvent::ShowPlanet(2))), None);
    }

    #[test]
    fn terms_out_of_range_are_refused() {
        let t = |cat, sub, value, sub2, value2| Terms { cat, sub, value, sub2, value2 }.in_range(20);
        assert!(t(6, 8, 0, -1, -1), "a greeting");
        assert!(t(0, 1, -1, -1, -1) && t(0, 7, 4, -1, -1), "negotiations, with or without a third empire");
        assert!(t(3, 3, 192, -1, -1) && !t(3, 3, 193, -1, -1), "techs 0..193");
        assert!(t(2, 2, 20, -1, -1) && !t(2, 2, 21, -1, -1) && !t(2, 2, 0, -1, -1), "planets 1..=20");
        assert!(t(1, 0, 500, 6, 3) && !t(1, 0, 500, 6, 1069), "both sides of an exchange");
        assert!(!t(3, 0, -5, -1, -1) && !t(3, 7, 0, -1, -1) && !t(5, 0, 0, -1, -1) && !t(0, 9, 0, -1, -1));
    }
}
