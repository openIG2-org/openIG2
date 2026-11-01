//! Replays (ADR 0038): a game's starting state and its input stream, written as the game runs and played back
//! headless to the same end.
//!
//! A replay file is [`MAGIC`], [`FORMAT`] (u16), then length-prefixed (u32) blocks: the [`Meta`] (wire), the start
//! (our save file, [`Snapshot::to_file`]), then [`Chunk`]s (zlib of the wire encoding) until the end of the file. Each
//! chunk holds [`CHECK_EVERY`] steps (the last one fewer) and the state hash after its last step, so a playback
//! that drifts stops at the first chunk whose hash differs. Only whole chunks are read: a game that crashed loses
//! at most its last chunk.
//!
//! A step is one lockstep turn in multiplayer (the bundle every peer ran, [`TURN_TICKS`] ticks) and one sim tick in
//! single player, where the inputs are the player's commands and the screen state the window wrote between the
//! ticks ([`Input`], [`Locals`]).

use std::io::Write;

use ig2_sim::game::{Game, GameEvent, GameRules, LocalView};
use ig2_sim::save::va;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Stamped};
use crate::lockstep::{Bundle, Peer};
use crate::snapshot::{self, Snapshot};
use crate::wire;

pub const MAGIC: &[u8; 8] = b"IG2RPLAY";
/// Bumped whenever [`Meta`], [`Chunk`] or [`Input`] change (the start state carries its own save version).
pub const FORMAT: u16 = 1;
/// Steps per chunk: 10 s of multiplayer turns, 5 s of single-player ticks.
pub const CHECK_EVERY: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a replay (bad magic)")]
    Magic,
    #[error("replay format {0}; this build reads {FORMAT}")]
    Format(u16),
    #[error("truncated replay")]
    Truncated,
    #[error("corrupt replay: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Start(#[from] snapshot::Error),
    #[error(transparent)]
    Wire(#[from] wire::Error),
}

/// What the replay was recorded from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// The build that recorded it (`OPENIG2_VERSION`, or `VERSION` with `-dev`).
    pub build: String,
    /// The rules fingerprint of the install (0 when the recorder had none).
    pub rules: u64,
    /// A lockstep replay (a step = a turn); else single player (a step = a tick).
    pub multiplayer: bool,
    /// The recording machine's empire and lockstep slot.
    pub empire: u8,
    pub slot: u8,
    /// `SpBattles::live`: the single-player client shows its real-time battles.
    pub live_battles: bool,
}

/// One input of a step, applied in order before the step runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Input {
    /// A command as the lockstep or `World::pump` applies it.
    Order { issuer: u8, command: Command },
    /// Single player's screen state the window writes ([`Locals`]): `Game::set_view`, `set_screen`,
    /// `set_transfer`, the battle screen (`SpBattles`) and `set_pause`.
    View { empire: u8, starmap_mode: i32, planet: i32, fleet: i32 },
    Screen(i32),
    Transfer(Option<(i32, i32)>),
    BattleScreen { on_screen: bool, fade: f32, colony: Option<usize> },
    Pause(bool),
    /// `World::set_speed`: `Game::set_speed(level, false)`.
    Speed(i32),
    /// `World::pop_message`: the message line takes its next text.
    PopMessage,
}

impl Input {
    pub fn apply(&self, game: &mut Game) {
        match self {
            Input::Order { issuer, command } => command.apply(game, *issuer),
            &Input::View { empire, starmap_mode, planet, fleet } => game.set_view(LocalView { empire, starmap_mode, planet, fleet, ..game.local_view() }),
            &Input::Screen(kep) => game.set_screen(kep),
            &Input::Transfer(f) => game.set_transfer(f),
            &Input::BattleScreen { on_screen, fade, colony } => {
                game.sp_battles.on_screen = on_screen;
                game.sp_battles.fade = fade;
                game.sp_battles.colony = colony;
            }
            &Input::Pause(p) => game.set_pause(p, false),
            &Input::Speed(level) => game.set_speed(level, false),
            Input::PopMessage => {
                let mut t = game.state.ticker();
                t.pop();
                game.state.store_ticker(&t);
            }
        }
    }
}

impl From<&Stamped> for Input {
    fn from(s: &Stamped) -> Self {
        Input::Order { issuer: s.issuer, command: s.command.clone() }
    }
}

/// The single-player screen state the window writes outside the commands, compared between ticks: a change is
/// recorded as its [`Input`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Locals {
    view: (u8, i32, i32, i32),
    screen: i32,
    transfer: Option<(i32, i32)>,
    battle: (bool, f32, Option<usize>),
    paused: bool,
}

impl Locals {
    pub fn read(game: &Game) -> Self {
        let v = game.local_view();
        let m = &game.state.mem;
        let transfer = (m.u8(va::STARMAP_TRANSFER_MODE) != 0).then(|| (m.i32(va::STARMAP_TRANSFER_FLEETS.0), m.i32(va::STARMAP_TRANSFER_FLEETS.1)));
        let b = &game.sp_battles;
        Self { view: (v.empire, v.starmap_mode, v.planet, v.fleet), screen: m.i32(va::AKT_KEP), transfer, battle: (b.on_screen, b.fade, b.colony), paused: game.paused() }
    }

    /// The inputs that turn `self` into `now`.
    pub fn changes(&self, now: &Locals) -> Vec<Input> {
        let mut out = Vec::new();
        if self.view != now.view {
            let (empire, starmap_mode, planet, fleet) = now.view;
            out.push(Input::View { empire, starmap_mode, planet, fleet });
        }
        if self.screen != now.screen {
            out.push(Input::Screen(now.screen));
        }
        if self.transfer != now.transfer {
            out.push(Input::Transfer(now.transfer));
        }
        if self.battle.0 != now.battle.0 || self.battle.1.to_bits() != now.battle.1.to_bits() || self.battle.2 != now.battle.2 {
            let (on_screen, fade, colony) = now.battle;
            out.push(Input::BattleScreen { on_screen, fade, colony });
        }
        if self.paused != now.paused {
            out.push(Input::Pause(now.paused));
        }
        out
    }
}

/// [`CHECK_EVERY`] steps from `first` on, and the state hash after the last (`None`: the recorder was dropped
/// without the game, so the game's last steps go unchecked). The last chunk's `tail`: the inputs after the last
/// step (the game was left between ticks), part of the hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub first: u64,
    pub steps: Vec<Vec<Input>>,
    pub tail: Vec<Input>,
    pub hash: Option<u64>,
}

fn block(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
}

/// The sim-originated commands waiting in `events` for single player's next tick (`World::tick`).
fn sp_pending(game: &Game) -> Vec<Command> {
    game.events.iter().filter_map(Command::from_event).collect()
}

/// The state hash after a step: a peer's ([`Peer::hash`]), or single player's with the messages its next tick
/// delivers.
pub fn sp_hash(game: &Game) -> u64 {
    snapshot::state_hash(game, &sp_pending(game))
}

/// Single player's tick start (`World::tick`): the last tick's lockstep messages (`Command::from_event`), each
/// from `Game::sender`. Returns the events taken.
pub fn deliver(game: &mut Game) -> Vec<GameEvent> {
    let events = game.take_events();
    for c in events.iter().filter_map(Command::from_event) {
        let sender = game.sender();
        c.apply(game, sender);
    }
    events
}

// ------------------------------------------------------------------------------------------------ recording

/// Writes a replay as the game runs: the header and start at once, then a chunk every [`CHECK_EVERY`] steps.
pub struct Recorder {
    out: Box<dyn Write + Send>,
    /// The next step's inputs so far.
    inputs: Vec<Input>,
    steps: Vec<Vec<Input>>,
    first: u64,
    /// Single player: the screen state at the last input boundary.
    locals: Option<Locals>,
    /// The first write error (the game goes on; the replay stops).
    pub failed: Option<String>,
}

impl Recorder {
    /// A recording starting from `start` (a turn boundary, or a single-player game between ticks with its pending
    /// messages).
    pub fn new(mut out: Box<dyn Write + Send>, meta: &Meta, start: &Snapshot, rules: u64) -> std::io::Result<Self> {
        let mut head = Vec::new();
        head.extend_from_slice(MAGIC);
        head.extend_from_slice(&FORMAT.to_le_bytes());
        block(&mut head, &wire::to_vec(meta).map_err(std::io::Error::other)?);
        block(&mut head, &start.to_file(rules, meta.multiplayer));
        out.write_all(&head)?;
        out.flush()?;
        Ok(Self { out, inputs: Vec::new(), steps: Vec::new(), first: 0, locals: None, failed: None })
    }

    /// A single-player recording of `game` as it is now (between ticks).
    pub fn single_player(out: Box<dyn Write + Send>, game: &Game, build: &str) -> std::io::Result<Self> {
        let meta = Meta { build: build.into(), rules: 0, multiplayer: false, empire: game.local_view().empire, slot: 0, live_battles: game.sp_battles.live };
        let mut r = Self::new(out, &meta, &Snapshot::capture(game, &sp_pending(game), 0, 0), 0)?;
        r.locals = Some(Locals::default());
        r.watch(game);
        Ok(r)
    }

    /// A peer's recording from its state now (a turn boundary).
    pub fn peer(out: Box<dyn Write + Send>, peer: &Peer, build: &str, rules: u64) -> std::io::Result<Self> {
        let meta = Meta { build: build.into(), rules, multiplayer: true, empire: peer.empire, slot: peer.slot, live_battles: peer.game.sp_battles.live };
        Self::new(out, &meta, &peer.snapshot(), rules)
    }

    /// The steps recorded.
    pub fn steps(&self) -> u64 {
        self.first + self.steps.len() as u64
    }

    pub fn input(&mut self, i: Input) {
        self.inputs.push(i);
    }

    /// A lockstep turn's bundle as the step's inputs.
    pub fn inputs_from(&mut self, commands: &[Stamped]) {
        self.inputs.extend(commands.iter().map(Input::from));
    }

    /// Single player: record the screen state's changes since the last call (before the player's commands are
    /// applied and before each tick).
    pub fn watch(&mut self, game: &Game) {
        let Some(was) = self.locals else { return };
        let now = Locals::read(game);
        self.inputs.extend(was.changes(&now));
        self.locals = Some(now);
    }

    /// The step ran; `hash` (the state hash after it) is asked for at a chunk's end.
    pub fn step(&mut self, hash: impl FnOnce() -> u64) {
        self.steps.push(std::mem::take(&mut self.inputs));
        if self.steps.len() == CHECK_EVERY {
            self.write(Some(hash()));
        }
    }

    /// The single-player tick ran: the step, and the screen state the sim left as the next baseline.
    pub fn sp_step(&mut self, game: &Game) {
        self.step(|| sp_hash(game));
        if self.locals.is_some() {
            self.locals = Some(Locals::read(game));
        }
    }

    /// Write the steps since the last chunk (the game ends or is left); `hash` = the state hash now.
    pub fn flush(&mut self, hash: u64) {
        self.write(Some(hash));
    }

    fn write(&mut self, hash: Option<u64>) {
        if (self.steps.is_empty() && self.inputs.is_empty()) || self.failed.is_some() {
            return;
        }
        let steps = std::mem::take(&mut self.steps);
        // Only a game's end leaves inputs past a full chunk.
        let tail = if steps.len() < CHECK_EVERY { std::mem::take(&mut self.inputs) } else { Vec::new() };
        let n = steps.len() as u64;
        let chunk = Chunk { first: self.first, steps, tail, hash };
        self.first += n;
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&wire::to_vec(&chunk).expect("a chunk encodes"), 6);
        let mut b = Vec::with_capacity(z.len() + 4);
        block(&mut b, &z);
        if let Err(e) = self.out.write_all(&b).and_then(|()| self.out.flush()) {
            self.failed = Some(e.to_string());
        }
    }
}

impl Drop for Recorder {
    /// Dropped with its game (a game left without [`Recorder::flush`]): the last steps, unchecked.
    fn drop(&mut self) {
        self.write(None);
    }
}

/// A replay written into memory (the tests, the play-through harness).
#[derive(Debug, Clone, Default)]
pub struct Buffer(pub std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Buffer {
    pub fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

impl Write for Buffer {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------ playback

/// A replay file read back.
#[derive(Debug, Clone)]
pub struct Replay {
    pub meta: Meta,
    pub start: Snapshot,
    pub chunks: Vec<Chunk>,
    /// The bytes after the last whole chunk (a recording cut short).
    pub torn: usize,
}

impl Replay {
    pub fn read(b: &[u8]) -> Result<Self, Error> {
        if b.len() < 10 || &b[..8] != MAGIC {
            return Err(if b.len() < 10 && b.starts_with(&MAGIC[..b.len().min(8)]) { Error::Truncated } else { Error::Magic });
        }
        let format = u16::from_le_bytes([b[8], b[9]]);
        if format != FORMAT {
            return Err(Error::Format(format));
        }
        let mut at = 10;
        let next = |at: &mut usize| -> Option<&[u8]> {
            let len = u32::from_le_bytes(b.get(*at..*at + 4)?.try_into().unwrap()) as usize;
            let body = b.get(*at + 4..*at + 4 + len)?;
            *at += 4 + len;
            Some(body)
        };
        let meta: Meta = wire::from_slice(next(&mut at).ok_or(Error::Truncated)?)?;
        let (start, _) = Snapshot::from_file(next(&mut at).ok_or(Error::Truncated)?)?;
        let mut chunks: Vec<Chunk> = Vec::new();
        let mut whole = at;
        while let Some(z) = next(&mut at) {
            let raw = miniz_oxide::inflate::decompress_to_vec_zlib(z).map_err(|e| Error::Corrupt(format!("chunk {}: {e:?}", chunks.len())))?;
            let c: Chunk = wire::from_slice(&raw)?;
            let expected = chunks.last().map_or(0, |l| l.first + l.steps.len() as u64);
            if c.first != expected {
                return Err(Error::Corrupt(format!("chunk {} starts at step {}, expected {expected}", chunks.len(), c.first)));
            }
            chunks.push(c);
            whole = at;
        }
        Ok(Self { meta, start, chunks, torn: b.len() - whole })
    }

    /// The steps recorded.
    pub fn steps(&self) -> u64 {
        self.chunks.last().map_or(0, |c| c.first + c.steps.len() as u64)
    }

    /// Sim ticks per step.
    pub fn ticks_per_step(&self) -> u32 {
        if self.meta.multiplayer { crate::TURN_TICKS } else { 1 }
    }
}

/// A playback diverged from the recording: after `step`, the state hash `got` instead of `expected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the replay diverged after step {step} (hash {got:016x}, recorded {expected:016x})")]
pub struct Diverged {
    pub step: u64,
    pub expected: u64,
    pub got: u64,
}

/// A replay run headless, step by step: the recorded game's sim, fed the recorded inputs.
pub struct Playback<'r> {
    replay: std::sync::Arc<Replay>,
    /// The game (in multiplayer the peer's; its turn and tick count with it).
    pub peer: Peer<'r>,
    /// The next step and where it sits in the chunks.
    step: u64,
    chunk: usize,
    /// Single player: the start's pending messages, delivered with the first tick.
    pending: Vec<Command>,
}

impl<'r> Playback<'r> {
    /// The replay's start restored. The caller adds what a snapshot doesn't keep and the recording client had
    /// (`Game::design_texts` from the install).
    pub fn new(replay: std::sync::Arc<Replay>, rules: &'r GameRules) -> Result<Self, Error> {
        let m = &replay.meta;
        let (peer, pending) = if m.multiplayer {
            (Peer::from_snapshot(m.slot, m.empire, &replay.start, rules, 0, 0)?, Vec::new())
        } else {
            let (game, pending) = replay.start.restore(rules)?;
            (Peer::new(0, m.empire, game, 0), pending)
        };
        let mut peer = peer;
        peer.game.sp_battles.live = m.live_battles;
        Ok(Self { replay, peer, step: 0, chunk: 0, pending })
    }

    pub fn game(&self) -> &Game<'r> {
        &self.peer.game
    }

    pub fn game_mut(&mut self) -> &mut Game<'r> {
        &mut self.peer.game
    }

    /// The next step to run.
    pub fn position(&self) -> u64 {
        self.step
    }

    pub fn done(&self) -> bool {
        self.step >= self.replay.steps()
    }

    /// Run the next step; `Ok(false)` at the end. At a chunk's end the state hash is checked.
    pub fn step(&mut self) -> Result<bool, Diverged> {
        let replay = std::sync::Arc::clone(&self.replay);
        let Some(c) = replay.chunks.get(self.chunk) else { return Ok(false) };
        let k = (self.step - c.first) as usize;
        if c.steps.is_empty() {
            // Only a tail: the game was left before its first tick here.
            self.chunk += 1;
            self.end_chunk(c)?;
            return Ok(false);
        }
        let inputs = &c.steps[k];
        if self.replay.meta.multiplayer {
            let commands = inputs.iter().filter_map(|i| match i {
                Input::Order { issuer, command } => Some(Stamped { issuer: *issuer, command: command.clone() }),
                _ => None,
            });
            self.peer.run_bundle(&Bundle { turn: self.peer.turn, commands: commands.collect() });
        } else {
            let g = &mut self.peer.game;
            for i in inputs {
                i.apply(g);
            }
            for c in std::mem::take(&mut self.pending) {
                let sender = g.sender();
                c.apply(g, sender);
            }
            let events = deliver(g);
            self.peer.events.extend(events);
            g.advance_clock();
            g.run_tick();
            self.peer.tick += 1;
        }
        self.step += 1;
        if k + 1 == c.steps.len() {
            self.chunk += 1;
            self.end_chunk(c)?;
        }
        Ok(true)
    }

    /// A chunk ran: its tail, then the check.
    fn end_chunk(&mut self, c: &Chunk) -> Result<(), Diverged> {
        for i in &c.tail {
            i.apply(&mut self.peer.game);
        }
        let got = self.hash();
        match c.hash.filter(|&h| h != got) {
            Some(expected) => Err(Diverged { step: self.step, expected, got }),
            None => Ok(()),
        }
    }

    /// The state hash now (the one a chunk's end checks).
    pub fn hash(&self) -> u64 {
        if self.replay.meta.multiplayer { self.peer.hash() } else { sp_hash(&self.peer.game) }
    }

    /// Run to the end (the screens' events dropped); the final state hash.
    pub fn run_to_end(&mut self) -> Result<u64, Diverged> {
        while self.step()? {
            self.peer.events.clear();
        }
        Ok(self.hash())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ig2_sim::save::SaveState;

    fn game(rules: &GameRules) -> Game<'_> {
        Game::new(SaveState::from_parts(Default::default(), Vec::new()), rules)
    }

    #[test]
    fn the_file_reads_back_and_a_torn_tail_is_left_out() {
        let rules = GameRules::default();
        let g = game(&rules);
        let buf = Buffer::default();
        let mut r = Recorder::single_player(Box::new(buf.clone()), &g, "0.7-test").unwrap();
        for k in 0..250u64 {
            if k % 7 == 0 {
                r.input(Input::Speed(k as i32 % 3 + 1));
            }
            r.step(|| 1000 + k);
        }
        r.flush(42);
        drop(r);
        let bytes = buf.take();
        let rp = Replay::read(&bytes).unwrap();
        assert_eq!(rp.meta.build, "0.7-test");
        assert!(!rp.meta.multiplayer);
        assert_eq!((rp.steps(), rp.torn), (250, 0));
        assert_eq!(rp.chunks.iter().map(|c| (c.first, c.steps.len(), c.hash)).collect::<Vec<_>>(), [(0, 100, Some(1099)), (100, 100, Some(1199)), (200, 50, Some(42))]);
        assert_eq!(rp.chunks[0].steps[7], [Input::Speed(2)]);
        assert_eq!(rp.start.hash, Snapshot::capture(&g, &[], 0, 0).hash);
        // A crash mid-chunk: the whole chunks before it still read.
        let cut = Replay::read(&bytes[..bytes.len() - 3]).unwrap();
        assert_eq!(cut.steps(), 200);
        assert!(cut.torn > 0);
        assert!(matches!(Replay::read(b"IG2RSAVE.."), Err(Error::Magic)));
    }

    #[test]
    fn a_dropped_recorder_keeps_its_last_steps_unchecked() {
        let rules = GameRules::default();
        let buf = Buffer::default();
        let mut r = Recorder::single_player(Box::new(buf.clone()), &game(&rules), "x").unwrap();
        r.step(|| 0);
        drop(r);
        let rp = Replay::read(&buf.take()).unwrap();
        assert_eq!(rp.chunks.iter().map(|c| (c.steps.len(), c.hash)).collect::<Vec<_>>(), [(1, None)]);
    }

    #[test]
    fn the_screen_state_is_recorded_as_its_changes() {
        let rules = GameRules::default();
        let mut g = game(&rules);
        let was = Locals::read(&g);
        assert!(was.changes(&was).is_empty());
        g.set_view(LocalView { planet: 7, starmap_mode: 1, ..g.local_view() });
        g.set_pause(true, false);
        g.sp_battles.fade = 0.5;
        let now = Locals::read(&g);
        let changes = was.changes(&now);
        assert_eq!(changes, [Input::View { empire: 0, starmap_mode: 1, planet: 7, fleet: 0 }, Input::BattleScreen { on_screen: false, fade: 0.5, colony: None }, Input::Pause(true)]);
        let mut h = game(&rules);
        for i in &changes {
            i.apply(&mut h);
        }
        assert_eq!(Locals::read(&h), now);
    }
}
