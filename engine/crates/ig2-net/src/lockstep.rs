//! Deterministic lockstep (ADR 0007 §2–§4): peers run the same ticks from the same commands; the host orders
//! the commands into one bundle per turn and compares the peers' state hashes.

use std::collections::{BTreeMap, VecDeque};

use ig2_sim::game::{Game, GameEvent, GameRules, Libc, LocalView, ViewMode};
use serde::{Deserialize, Serialize};

use crate::command::{Command, Stamped};
use crate::lobby::LobbyMsg;
use crate::session::SessionMsg;
use crate::snapshot::{self, Snapshot};
use crate::transport::Transport;
use crate::wire;

/// Sim ticks per turn (100 ms at 20 Hz).
pub const TURN_TICKS: u32 = 2;
/// The default input delay in turns (200 ms).
pub const DEFAULT_DELAY: u64 = 2;
/// The bundles a peer keeps after running them, for a host migration (ADR 0012 §3).
pub const KEPT_BUNDLES: usize = 128;

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
    /// Before the start: the lobby (ADR 0008 §3); after it, a (re)start from a snapshot (rejoin, resync). Ignored by
    /// the lockstep.
    Lobby(LobbyMsg),
    /// The session layer's pause requests and hold status (ADR 0011 §4). Ignored by the lockstep.
    Session(SessionMsg),
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
    /// The turn it joined at: its hashes for earlier turns are a former link's, not compared.
    since: u64,
    /// Messages received from it (the session's keep-alive watches this change).
    heard: u64,
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
    /// The host's own commands for the next bundle, each with the empire it acts for (ADR 0011 §5).
    injected: Vec<Stamped>,
    /// No bundles go out while held (ADR 0011 §4).
    held: bool,
    /// The peers' session messages, for the session layer.
    inbox: Vec<(u8, SessionMsg)>,
}

impl Host {
    /// A session whose peers send their commands `delay` turns ahead (turns before `delay` have none).
    pub fn new(delay: u64) -> Self {
        Self::at(delay, 0)
    }

    /// A migrated session (ADR 0012 §3): its next bundle is `next`'s, `log` the bundles the new host's peer kept
    /// (contiguous, ending at `next`).
    pub fn resumed(delay: u64, next: u64, log: Vec<Bundle>) -> Self {
        debug_assert!(log.last().is_none_or(|b| b.turn + 1 == next));
        Self { log, ..Self::at(delay, next) }
    }

    /// A session whose first bundle is `turn`'s (resumed from a save at that turn).
    pub fn at(delay: u64, turn: u64) -> Self {
        Self {
            delay,
            slots: BTreeMap::new(),
            next: turn,
            inputs: BTreeMap::new(),
            log: Vec::new(),
            hashes: BTreeMap::new(),
            desyncs: Vec::new(),
            injected: Vec::new(),
            held: false,
            inbox: Vec::new(),
        }
    }

    /// Stop (`true`) or resume the turn clock: while held no bundle goes out, so every peer stalls at the same turn
    /// and the sim doesn't change (a pause, or a dropped player awaited during a battle; ADR 0011 §4).
    pub fn hold(&mut self, held: bool) {
        self.held = held;
    }

    pub fn held(&self) -> bool {
        self.held
    }

    /// A host command for `empire` (`Command::PlayerAway`/`PlayerBack`), first in the next bundle that goes out.
    pub fn inject(&mut self, empire: u8, command: Command) {
        self.injected.push(Stamped { issuer: empire, command });
    }

    /// The slots whose input for the next bundle hasn't come: the relay waits for them (the exe's late test,
    /// `FUN_00510fd0`).
    pub fn late(&self) -> Vec<u8> {
        let have = self.inputs.get(&self.next);
        self.slots.iter().filter(|(slot, s)| self.next >= s.from && !have.is_some_and(|i| i.contains_key(slot))).map(|(&slot, _)| slot).collect()
    }

    /// The address `slot`'s link comes from ([`Transport::remote`]).
    pub fn remote(&self, slot: u8) -> Option<std::net::IpAddr> {
        self.slots.get(&slot)?.link.remote()
    }

    /// The empire `slot` plays, while it's in the session.
    pub fn empire_of(&self, slot: u8) -> Option<u8> {
        self.slots.get(&slot).map(|s| s.empire)
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
        self.slots.insert(slot, Slot { empire, link, from, since: self.next, heard: 0 });
        from
    }

    /// A peer left: later turns no longer wait for it. Returns its link (a resync reuses it).
    pub fn remove_peer(&mut self, slot: u8) -> Option<Box<dyn Transport>> {
        for inputs in self.inputs.values_mut() {
            inputs.remove(&slot);
        }
        self.slots.remove(&slot).map(|s| s.link)
    }

    /// The slots whose link is known to be gone.
    pub fn closed(&self) -> Vec<u8> {
        self.slots.iter().filter(|(_, s)| s.link.closed()).map(|(&slot, _)| slot).collect()
    }

    /// How many messages `slot` sent since it was added (any kind: the session's keep-alive counts them).
    pub fn heard(&self, slot: u8) -> Option<u64> {
        self.slots.get(&slot).map(|s| s.heard)
    }

    /// The turn `slot` joined at (its hashes count from there on).
    pub fn joined_at(&self, slot: u8) -> Option<u64> {
        self.slots.get(&slot).map(|s| s.since)
    }

    /// The `(slot, hash)` reports for `turn` the host still holds, in arrival order.
    pub fn reported(&self, turn: u64) -> &[(u8, u64)] {
        self.hashes.get(&turn).map_or(&[], |v| v.as_slice())
    }

    /// Send `msg` to `slot`.
    pub fn send_to(&mut self, slot: u8, msg: &Msg) {
        if let Some(s) = self.slots.get_mut(&slot) {
            s.link.send(msg);
        }
    }

    /// Send `msg` to every peer.
    pub fn broadcast(&mut self, msg: &Msg) {
        for s in self.slots.values_mut() {
            s.link.send(msg);
        }
    }

    /// The session messages the peers sent since the last call, with their slots.
    pub fn take_inbox(&mut self) -> Vec<(u8, SessionMsg)> {
        std::mem::take(&mut self.inbox)
    }

    /// A bundle of the former host's for the next turn (a survivor had it, ADR 0012 §3): logged and sent to every
    /// peer as if this host had made it. False (nothing done) unless it is `next_turn`'s.
    pub fn adopt(&mut self, b: Bundle) -> bool {
        if b.turn != self.next {
            return false;
        }
        self.inputs.remove(&b.turn);
        for s in self.slots.values_mut() {
            s.link.send(&Msg::Bundle(b.clone()));
        }
        self.log.push(b);
        self.next += 1;
        true
    }

    /// [`Host::send_log`] from `turn` gives a peer every bundle it lacks (the log reaches back that far).
    pub fn log_reaches(&self, turn: u64) -> bool {
        turn <= self.next && (turn == self.next || self.log.first().is_some_and(|b| b.turn <= turn))
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
                s.heard += 1;
                got.push((slot, m));
            }
        }
        for (slot, m) in got {
            match m {
                Msg::Input { turn, commands } => {
                    // Late or unowed inputs are dropped: the turn went out without them.
                    if turn >= self.next && self.slots.get(&slot).is_some_and(|s| turn >= s.from) {
                        let commands = commands.into_iter().filter(|c| !c.host_only()).collect();
                        self.inputs.entry(turn).or_default().insert(slot, commands);
                    }
                }
                Msg::Hash { turn, hash } => {
                    if self.slots.get(&slot).is_some_and(|s| turn >= s.since) {
                        self.hash(slot, turn, hash);
                    }
                }
                Msg::Session(SessionMsg::Alive) => {}
                Msg::Session(m) => self.inbox.push((slot, m)),
                Msg::Bundle(_) | Msg::Desync { .. } | Msg::Lobby(_) => {}
            }
        }
        while !self.held && self.ready(self.next) {
            let inputs = self.inputs.remove(&self.next).unwrap_or_default();
            let commands = std::mem::take(&mut self.injected)
                .into_iter()
                .chain(inputs.into_iter().flat_map(|(slot, cmds)| {
                    let issuer = self.slots[&slot].empire;
                    cmds.into_iter().map(move |command| Stamped { issuer, command })
                }))
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
    /// The last [`KEPT_BUNDLES`] bundles run (a host migration hands them on).
    kept: VecDeque<Bundle>,
    outbox: Vec<Command>,
    delay: u64,
    next_input: u64,
    /// The inputs sent for turns no bundle has come for yet (resent to a new host).
    unacked: BTreeMap<u64, Vec<Command>>,
    /// Resend `unacked` with the next inputs.
    resend: bool,
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
            kept: VecDeque::new(),
            outbox: Vec::new(),
            delay,
            next_input: delay,
            unacked: BTreeMap::new(),
            resend: false,
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

    /// A peer starting at turn 0 from a state it built itself (a generated galaxy, ADR 0014 §2); `first_input` =
    /// [`Host::add_peer`]'s answer.
    pub fn from_game(slot: u8, empire: u8, game: Game<'r>, delay: u64, first_input: u64) -> Self {
        let mut p = Self::new(slot, empire, game, delay);
        p.next_input = first_input;
        p
    }

    /// The synced state now (a turn boundary).
    pub fn snapshot(&self) -> Snapshot {
        Snapshot::capture(&self.game, &self.pending, self.turn, self.tick)
    }

    pub fn hash(&self) -> u64 {
        snapshot::state_hash(&self.game, &self.pending)
    }

    /// The next turn this peer holds no bundle for (run or buffered).
    pub fn frontier(&self) -> u64 {
        let mut t = self.turn;
        while self.bundles.contains_key(&t) {
            t += 1;
        }
        t
    }

    /// The bundles this peer holds up to its frontier: the last [`KEPT_BUNDLES`] run, then the buffered ones
    /// (contiguous, in turn order).
    pub fn held_bundles(&self) -> Vec<Bundle> {
        let buffered = (self.turn..self.frontier()).map(|t| self.bundles[&t].clone());
        self.kept.iter().cloned().chain(buffered).collect()
    }

    /// Go on with a new host (ADR 0012 §3) that wants inputs from turn `from` on: the inputs sent for turns `>= from`
    /// no bundle covered go out again with the next ones.
    pub fn resume(&mut self, from: u64) {
        self.unacked.retain(|&t, _| t >= from);
        self.resend = true;
        self.next_input = self.next_input.max(from);
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
                Msg::Bundle(b) => {
                    self.unacked.retain(|&t, _| t > b.turn);
                    if b.turn >= self.turn {
                        self.bundles.insert(b.turn, b);
                    }
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
            if self.kept.len() == KEPT_BUNDLES {
                self.kept.pop_front();
            }
            self.kept.push_back(b);
            run += 1;
        }
        Ok(run)
    }

    fn send_inputs(&mut self, link: &mut dyn Transport) {
        if std::mem::take(&mut self.resend) {
            for (&turn, commands) in &self.unacked {
                link.send(&Msg::Input { turn, commands: commands.clone() });
            }
        }
        let due = self.turn + self.delay;
        while self.next_input <= due {
            let commands = if self.next_input == due { std::mem::take(&mut self.outbox) } else { Vec::new() };
            link.send(&Msg::Input { turn: self.next_input, commands: commands.clone() });
            self.unacked.insert(self.next_input, commands);
            self.next_input += 1;
        }
    }

    /// One turn: at each tick boundary the sim's own pending messages, then (first tick) the bundle; then the tick.
    fn run_turn(&mut self, b: &Bundle) {
        for k in 0..TURN_TICKS {
            for c in std::mem::take(&mut self.pending) {
                let sender = self.game.sender();
                c.apply(&mut self.game, sender);
            }
            // The sim's own messages from this tick and from the bundle's orders (an answer's 0x66 on the messages
            // screen: the original's UI loop sends it, delivered after the next tick).
            let from = self.game.events.len();
            if k == 0 {
                for s in &b.commands {
                    s.command.apply(&mut self.game, s.issuer);
                }
            }
            self.game.advance_clock();
            self.game.run_tick();
            self.pending.extend(self.game.events[from..].iter().filter_map(Command::from_event));
            self.events.append(&mut self.game.events);
            self.tick += 1;
        }
        self.turn += 1;
    }
}
