//! Player commands (ADR 0007 §1): one variant per lockstep message of the original, applied at a tick boundary
//! as its `MULTIGET_*` handler applies it.

use ig2_sim::diplomacy::{Answer, Letter, Message};
use ig2_sim::game::{Game, GameEvent};
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
        }
    }
}
