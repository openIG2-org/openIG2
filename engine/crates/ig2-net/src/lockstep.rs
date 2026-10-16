//! Deterministic lockstep (ADR 0007 §2–§4): peers run the same ticks from the same commands; the host orders
//! the commands into one bundle per turn and compares the peers' state hashes.

use std::collections::BTreeMap;

use ig2_sim::game::{Game, GameEvent, GameRules, Libc, LocalView, ViewMode};
use serde::{Deserialize, Serialize};

use crate::command::{Command, Stamped};
use crate::lobby::LobbyMsg;
use crate::snapshot::{self, Snapshot};
use crate::transport::Transport;
use crate::wire;

/// Sim ticks per turn (100 ms at 20 Hz).
pub const TURN_TICKS: u32 = 2;
/// The default input delay in turns (200 ms).
pub const DEFAULT_DELAY: u64 = 2;

/// The commands every peer applies at the start of `turn`, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle {
    pub turn: u64,
    pub commands: Vec<Stamped>,
}

/// A lockstep message. Which peer sent it is known from the link it came over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Msg {
    /// A peer's commands for `turn` (one per turn, empty or not: the heartbeat).
    Input { turn: u64, commands: Vec<Command> },
    Bundle(Bundle),
    /// A peer's state hash after running `turn`.
    Hash { turn: u64, hash: u64 },
    /// The host saw different hashes after `turn`.
    Desync { turn: u64, slots: Vec<u8> },
    /// Before the start: the lobby (ADR 0008 §3). Ignored by the lockstep.
    Lobby(LobbyMsg),
}

/// A desync the host saw: after `turn`, `slots` disagreed with the first hash reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desync {
    pub turn: u64,
    pub slots: Vec<u8>,
}

// ------------------------------------------------------------------------------------------------ host

struct Slot {
    empire: u8,
    link: Box<dyn Transport>,
    /// The first turn whose input this peer owes.
    from: u64,
}

/// The session's relay: admission, the turn order and desync detection. It doesn't run the sim (its player is an
/// ordinary [`Peer`] on a loopback link).
pub struct Host {
    delay: u64,
    slots: BTreeMap<u8, Slot>,
    next: u64,
    inputs: BTreeMap<u64, BTreeMap<u8, Vec<Command>>>,
    /// Every bundle sent (rejoin replays them).
    pub log: Vec<Bundle>,
    hashes: BTreeMap<u64, Vec<(u8, u64)>>,
    pub desyncs: Vec<Desync>,
}

impl Host {
    /// A session whose peers send their commands `delay` turns ahead (turns before `delay` have none).
    pub fn new(delay: u64) -> Self {
        Self { delay, slots: BTreeMap::new(), next: 0, inputs: BTreeMap::new(), log: Vec::new(), hashes: BTreeMap::new(), desyncs: Vec::new() }
    }

    /// The next turn without a bundle.
    pub fn next_turn(&self) -> u64 {
        self.next
    }

    /// Admit a peer playing `empire` in `slot`. Returns the first turn it must send an input for: the joiner's
    /// [`Peer`] starts its inputs there. A rejoining peer first gets the bundles since its snapshot
    /// ([`Host::send_log`]).
    pub fn add_peer(&mut self, slot: u8, empire: u8, link: Box<dyn Transport>) -> u64 {
        let from = self.next.max(self.delay);
        self.slots.insert(slot, Slot { empire, link, from });
        from
    }

    /// A peer left: later turns no longer wait for it.
    pub fn remove_peer(&mut self, slot: u8) {
        self.slots.remove(&slot);
        for inputs in self.inputs.values_mut() {
            inputs.remove(&slot);
        }
    }

    /// Send `slot` every bundle from `turn` on.
    pub fn send_log(&mut self, slot: u8, turn: u64) {
        let Some(s) = self.slots.get_mut(&slot) else { return };
        for b in self.log.iter().filter(|b| b.turn >= turn) {
            s.link.send(&Msg::Bundle(b.clone()));
        }
    }

    /// Receive what the peers sent, send every bundle that's complete, compare hashes.
    pub fn pump(&mut self) -> Result<(), wire::Error> {
        let mut got = Vec::new();
        for (&slot, s) in self.slots.iter_mut() {
            while let Some(m) = s.link.recv()? {
                got.push((slot, m));
            }
        }
        for (slot, m) in got {
            match m {
                Msg::Input { turn, commands } => {
                    // Late or unowed inputs are dropped: the turn went out without them.
                    if turn >= self.next && self.slots.get(&slot).is_some_and(|s| turn >= s.from) {
                        self.inputs.entry(turn).or_default().insert(slot, commands);
                    }
                }
                Msg::Hash { turn, hash } => self.hash(slot, turn, hash),
                Msg::Bundle(_) | Msg::Desync { .. } | Msg::Lobby(_) => {}
            }
        }
        while self.ready(self.next) {
            let inputs = self.inputs.remove(&self.next).unwrap_or_default();
            let commands = inputs
                .into_iter()
                .flat_map(|(slot, cmds)| {
                    let issuer = self.slots[&slot].empire;
                    cmds.into_iter().map(move |command| Stamped { issuer, command })
                })
                .collect();
            let b = Bundle { turn: self.next, commands };
            for s in self.slots.values_mut() {
                s.link.send(&Msg::Bundle(b.clone()));
            }
            self.log.push(b);
            self.next += 1;
        }
        Ok(())
    }

    fn ready(&self, turn: u64) -> bool {
        if self.slots.is_empty() {
            return false;
        }
        let have = self.inputs.get(&turn);
        self.slots.iter().all(|(slot, s)| turn < s.from || have.is_some_and(|i| i.contains_key(slot)))
    }

    fn hash(&mut self, slot: u8, turn: u64, hash: u64) {
        let seen = self.hashes.entry(turn).or_default();
        seen.push((slot, hash));
        let first = seen[0].1;
        if hash != first {
            match self.desyncs.iter_mut().find(|d| d.turn == turn) {
                Some(d) => d.slots.push(slot),
                None => self.desyncs.push(Desync { turn, slots: vec![slot] }),
            }
            let d = Msg::Desync { turn, slots: vec![slot] };
            for s in self.slots.values_mut() {
                s.link.send(&d);
            }
        }
        // Keep the hashes of the last few hundred turns (a rejoiner reports old turns while catching up).
        while self.hashes.len() > 512 {
            self.hashes.pop_first();
        }
    }
}

// ------------------------------------------------------------------------------------------------ peer

/// A player's machine: the whole sim, run turn by turn as the bundles arrive.
pub struct Peer<'r> {
    pub slot: u8,
    pub empire: u8,
    pub game: Game<'r>,
    /// The next turn to run, and the ticks run so far.
    pub turn: u64,
    pub tick: u64,
    /// Sim-originated commands for the next tick boundary.
    pending: Vec<Command>,
    bundles: BTreeMap<u64, Bundle>,
    outbox: Vec<Command>,
    delay: u64,
    next_input: u64,
    /// `(turn, hash)` after each turn run.
    pub hashes: Vec<(u64, u64)>,
    /// The ticks' events since the caller last took them (the UI's).
    pub events: Vec<GameEvent>,
    pub desyncs: Vec<Desync>,
}

impl<'r> Peer<'r> {
    /// A peer starting a session at turn 0. A game in the canonical view (multiplayer, ADR 0007 §5) gets this peer's
    /// local view (its empire, nothing shown) and cosmetic stream; a classic game is left as it is.
    pub fn new(slot: u8, empire: u8, mut game: Game<'r>, delay: u64) -> Self {
        if game.view != ViewMode::Classic {
            game.local = LocalView { empire, ..LocalView::default() };
            game.cosmetic = Libc(0x1_0000 * slot as u32 + empire as u32);
        }
        Self {
            slot,
            empire,
            game,
            turn: 0,
            tick: 0,
            pending: Vec::new(),
            bundles: BTreeMap::new(),
            outbox: Vec::new(),
            delay,
            next_input: delay,
            hashes: Vec::new(),
            events: Vec::new(),
            desyncs: Vec::new(),
        }
    }

    /// A peer (re)joining from a snapshot; `first_input` = [`Host::add_peer`]'s answer.
    pub fn from_snapshot(slot: u8, empire: u8, snap: &Snapshot, rules: &'r GameRules, delay: u64, first_input: u64) -> Result<Self, snapshot::Error> {
        let (game, pending) = snap.restore(rules)?;
        let mut p = Self::new(slot, empire, game, delay);
        p.turn = snap.turn;
        p.tick = snap.tick;
        p.pending = pending;
        p.next_input = first_input;
        Ok(p)
    }

    /// The synced state now (a turn boundary).
    pub fn snapshot(&self) -> Snapshot {
        Snapshot::capture(&self.game, &self.pending, self.turn, self.tick)
    }

    pub fn hash(&self) -> u64 {
        snapshot::state_hash(&self.game, &self.pending)
    }

    /// The local player looks elsewhere (screen, planet, selection). Canonical view: per-peer only, never in the
    /// synced state. Classic view: the original's globals in the state, so only for single player.
    pub fn show(&mut self, v: LocalView) {
        self.game.set_view(v);
    }

    /// A local player action, sent with the next input (it runs `delay` turns later on every peer).
    pub fn issue(&mut self, c: Command) {
        self.outbox.push(c);
    }

    /// Receive, send the inputs that are due and run up to `max_turns` turns whose bundles are here. Returns
    /// the turns run.
    pub fn poll(&mut self, link: &mut dyn Transport, max_turns: usize) -> Result<usize, wire::Error> {
        while let Some(m) = link.recv()? {
            match m {
                Msg::Bundle(b) if b.turn >= self.turn => {
                    self.bundles.insert(b.turn, b);
                }
                Msg::Desync { turn, slots } => self.desyncs.push(Desync { turn, slots }),
                _ => {}
            }
        }
        let mut run = 0;
        loop {
            self.send_inputs(link);
            if run == max_turns {
                break;
            }
            let Some(b) = self.bundles.remove(&self.turn) else { break };
            self.run_turn(&b);
            let hash = self.hash();
            self.hashes.push((b.turn, hash));
            link.send(&Msg::Hash { turn: b.turn, hash });
            run += 1;
        }
        Ok(run)
    }

    fn send_inputs(&mut self, link: &mut dyn Transport) {
        let due = self.turn + self.delay;
        while self.next_input <= due {
            let commands = if self.next_input == due { std::mem::take(&mut self.outbox) } else { Vec::new() };
            link.send(&Msg::Input { turn: self.next_input, commands });
            self.next_input += 1;
        }
    }

    /// One turn: at each tick boundary the sim's own pending messages, then (first tick) the bundle; then the tick.
    fn run_turn(&mut self, b: &Bundle) {
        for k in 0..TURN_TICKS {
            for c in std::mem::take(&mut self.pending) {
                c.apply(&mut self.game, self.empire);
            }
            if k == 0 {
                for s in &b.commands {
                    s.command.apply(&mut self.game, s.issuer);
                }
            }
            let from = self.game.events.len();
            self.game.advance_clock();
            self.game.run_tick();
            self.pending.extend(self.game.events[from..].iter().filter_map(Command::from_event));
            self.events.append(&mut self.game.events);
            self.tick += 1;
        }
        self.turn += 1;
    }
}
