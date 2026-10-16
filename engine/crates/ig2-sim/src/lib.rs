//! Deterministic simulation core for the IG2 reimplementation.
//!
//! Specs: `re/rules/` (each module cites its spec). Decisions: `docs/decisions/0003-sim-clock-and-rng.md`.

pub mod ai;
pub mod begin;
pub mod brnd;
pub mod campaign;
pub mod colony;
pub mod colony_side;
pub mod diplomacy;
pub mod fleet;
pub mod game;
pub mod ground;
pub mod info;
pub mod messages;
pub mod multi;
pub mod placement;
pub mod production;
pub mod radar;
pub mod research;
pub mod rnd;
pub mod rng;
pub mod save;
pub mod spacebattle;
pub mod spacewar;
pub mod spy;
pub mod story;
pub mod storyline;
pub mod time;
pub mod trade;
