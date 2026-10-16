//! Deterministic simulation core for the IG2 reimplementation.
//!
//! Specs: `re/rules/` (each module cites its spec). Decisions: `docs/decisions/0003-sim-clock-and-rng.md`.

pub mod ai;
pub mod colony;
pub mod colony_side;
pub mod diplomacy;
pub mod fleet;
pub mod placement;
pub mod production;
pub mod radar;
pub mod research;
pub mod rng;
pub mod spacebattle;
pub mod spacewar;
pub mod spy;
pub mod time;
