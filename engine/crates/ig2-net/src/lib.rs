//! Multiplayer core (ADR 0007): player commands applied at tick boundaries, deterministic lockstep with input
//! delay, per-turn state hashes, snapshots for rejoin and our own save format.
//!
//! ```text
//! Peer::issue(cmd) ─ Input{turn+delay} ─▶ Host ─ Bundle{turn} ─▶ every Peer: apply, run TURN_TICKS ticks
//!                    ◀──────────────── Hash{turn} ─────────────── (Host compares → Desync)
//! rejoin: Snapshot (to_file/from_file) + Host::send_log(turn)
//! ```
//!
//! Transports: the in-process [`transport::Loopback`] (no sockets) and, with feature `quic`, `quic::QuicHost` /
//! `quic::connect` (`quinn`, one UDP port, a pinned self-signed session certificate).

pub mod command;
pub mod discovery;
pub mod lobby;
pub mod lockstep;
pub mod portmap;
#[cfg(feature = "quic")]
pub mod quic;
pub mod snapshot;
pub mod transport;
pub mod wire;

pub use command::{Command, Stamped, Terms};
pub use lockstep::{Bundle, Desync, Host, Msg, Peer, DEFAULT_DELAY, TURN_TICKS};
pub use snapshot::{state_hash, Header, Snapshot};
pub use transport::{loopback, Loopback, Transport};
