//! Player commands (ADR 0007 §1): one variant per lockstep message of the original, applied at a tick boundary
//! as its `MULTIGET_*` handler applies it.

use ig2_sim::diplomacy::{Answer, Letter, Message};
use ig2_sim::game::{Game, GameEvent};
use ig2_sim::spacebattle::Order;
use ig2_sim::spy;
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

impl From<Terms> for Message {
    fn from(t: Terms) -> Self {
        Message { cat: t.cat, sub: t.sub, value: t.value, sub2: t.sub2, value2: t.value2 }
    }
}

/// A spy-screen order (`ig2_sim::spy::Order`, `MULTIGET_Spy`'s messages 0x47..0x4c; `re/rules/espionage.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpyOrder {
    Dismiss { spy: u8 },
    Recall { spy: u8 },
    Send { spy: u8, mission: u16, target: u8, param1: i16, param2: i16, days: i32 },
    Verdict { fate: i32, owner: u8, spy: u8 },
    Applicant { choice: i32, spy: u8 },
    Skill { spy: u8, skill: u8 },
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
}

/// A command with its issuer, as a bundle carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamped {
    pub issuer: u8,
    pub command: Command,
}

impl Command {
    /// The sim-originated command a tick's event sends (the sim's own `Multi_Send`), if any.
    pub fn from_event(e: &GameEvent) -> Option<Command> {
        e.emptied_fleet().map(|f| Command::FleetEmptied { fleet: f as u32 })
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
            Command::DiplomacyMessage { to, declaration, terms } => {
                game.diplomacy_message(Letter { from: issuer as i32, to: *to, declaration: *declaration, msg: (*terms).into() });
            }
            Command::DiplomacyAnswer { to, terms, counter, reply, yes } => {
                game.diplomacy_answer(Answer { from: issuer as i32, to: *to, msg: (*terms).into(), counter: *counter, reply: *reply, yes: *yes });
            }
            Command::StoryChoice { id } => game.story_special([15, issuer as i32, *id, 0, 0, 0, 0, 0]),
            Command::FleetEmptied { fleet } => game.fleet_emptied(*fleet as usize),
            Command::Spy { order } => game.spy_order(issuer as usize, (*order).into()),
            Command::StoryStart { id } => game.story_start(*id),
            Command::SpaceOrder { battle, order } => {
                game.space_order(issuer, *battle as usize, (*order).into());
            }
            Command::SpaceBattleLeft => game.space_battle_left(issuer),
        }
    }
}
