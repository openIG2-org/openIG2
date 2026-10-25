//! Multiplayer core (ADR 0007): player commands applied at tick boundaries, deterministic lockstep with input
//! delay, per-turn state hashes, snapshots for rejoin and our own save format.
//!
//! ```text
//! Peer::issue(cmd) ─ Input{turn+delay} ─▶ Host ─ Bundle{turn} ─▶ every Peer: apply, run TURN_TICKS ticks
//!                    ◀──────────────── Hash{turn} ─────────────── (Host compares → Desync)
//! rejoin: Snapshot (to_file/from_file) + Host::send_log(turn)
//! session: HostSession / ClientSession (start, drops and grace, rejoin, desync resync, pause, MP saves, host migration)
//! galaxy: the generated MP galaxy's room options and the start every machine builds (ADR 0014)
//! ```
//!
//! Transports: the in-process [`transport::Loopback`] (no sockets) and, with feature `quic`, `quic::QuicHost` /
//! `quic::connect` (`quinn`, one UDP port, a pinned self-signed session certificate).

pub mod command;
pub mod discovery;
pub mod galaxy;
pub mod joincode;
pub mod lobby;
pub mod lockstep;
pub mod portmap;
#[cfg(feature = "quic")]
pub mod quic;
pub mod session;
pub mod snapshot;
pub mod transport;
pub mod wire;

pub use command::{BattleOrder, Command, DiplomacyScreenOp, ResearchCmd, SpyOrder, Stamped, StarmapCmd, Terms};
pub use lockstep::{Bundle, Desync, Host, Msg, Peer, DEFAULT_DELAY, KEPT_BUNDLES, TURN_TICKS};
pub use session::{ClientSession, HoldStatus, HostSession, MpSave, SessionEvent, SessionMsg, SessionSettings};
pub use snapshot::{part_hashes, state_hash, Header, Snapshot};
pub use transport::{loopback, Loopback, Transport};
