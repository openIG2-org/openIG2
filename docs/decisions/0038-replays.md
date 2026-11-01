# ADR 0038: Replays

Date: 2026-11-01. Status: accepted. An enhancement over the classic client (ADR 0009): the original records no
replays. Implemented by `ig2-net::replay` (the file, the recorder, the headless playback), `ig2-net::lockstep::Peer`
(multiplayer recording) and `ig2-client::world` (single-player recording). Row 452 in `docs/ENHANCEMENTS.md`.
This ADR covers the file and the headless playback; the viewer (play, pause, speed in the client) and the list in
the main menu come next.

## Context
The lockstep sim is deterministic (ADR 0007): the same start and the same commands give the same state hash on
every peer. A game is therefore fully described by its starting state and the inputs each step consumed, which is
far smaller than periodic saves.

Single player doesn't run through the lockstep: `World::pump` applies the player's commands between ticks and
`World::tick` runs one sim tick. Besides the commands, the window writes some state the sim reads between ticks:
the classic view's globals (`Game::set_view`, `set_screen`, `set_transfer`), the battle screen (`SpBattles`
`on_screen`, `fade`, `colony`), the pause, the speed and the message line's pop.

## Decision
1. **What a replay is.** The starting state (our save format, ADR 0007 §4, with the pending sim-originated
   commands) and, per step, the inputs applied before it. A multiplayer step is a lockstep turn: the bundle every
   peer ran (`Bundle::commands`). A single-player step is one sim tick: the player's commands (`Input::Order`, with
   the issuer `World::pump` uses), the speed button (`Input::Speed`), the message line's pop (`Input::PopMessage`),
   and the window's screen state, recorded as its changes since the last tick or command pass (`Locals`: view,
   screen, transfer panel, battle screen, pause). Playback applies them in the same order, then runs the step the
   same way (`Peer::run_bundle`; `replay::deliver`, `Game::advance_clock`, `Game::run_tick`).
2. **The file** (`.ig2r`): magic `IG2RPLAY`, a format version (u16, now 1), then u32-length-prefixed blocks:
   - `Meta` (our wire encoding): the build version that recorded it, the rules fingerprint, multiplayer or not, the
     recording machine's empire and slot, and `SpBattles::live`;
   - the start: a whole save file (`Snapshot::to_file`, which carries its own version and hash);
   - chunks until the end of the file, each zlib-compressed: the first step, up to 100 steps (10 s of turns, 5 s of
     ticks) and the state hash after the last one (`Peer::hash`, or for single player the hash with the messages
     the next tick delivers).
   The file is written as the game runs: the header and start at once, a chunk every 100 steps, the last partial
   chunk when the game is left. A crash loses at most one chunk; the reader ignores a torn tail.
3. **Divergence.** Playback compares the state hash at each chunk's end and stops at the first one that differs
   (`replay::Diverged`): a build whose sim changed, or an input that wasn't recorded, shows at once and not as a
   quietly different game. A replay is only expected to play back on the build that recorded it (`Meta::build`).
4. **Where and when.** Every machine writes its own replay into `replays/` beside the save slots (`slot_dir`,
   `IG2_SLOT_DIR`), one file per game started or loaded, named by the start time; in multiplayer every peer records
   (the bundles are the same on all of them). A rejoin or resync starts the peer over from a snapshot and ends its
   replay. The tutorial records none (its script writes the state outside the inputs). The 50 newest replays are
   kept.
5. **The option.** "Record replays" (Remake row on the game page, `RecordReplays` in the settings file), **on by
   default**: a replay can't be made after the fact, and it costs a save file plus a few KB a minute. Off = the
   original (nothing written).

## Consequences
- The sim is untouched: recording only reads the state and the inputs the client already applies.
- Inputs the window writes outside this list (the colony screen's unit groups, the tutorial's effects) aren't
  recorded; a replay that crosses one diverges and says so at that chunk.
- A replay outlives a save/load only as two files: loading a save starts a new replay from the loaded state.
- Playback API for the viewer: `Replay::read`, `Playback::new` (then `game_mut().design_texts` as the window sets
  them), `Playback::step` (one step; `Replay::ticks_per_step` sim ticks), `Playback::run_to_end`, `Playback::game`
  and `peer.events` for the screens.
