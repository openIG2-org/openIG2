# ADR 0021: Desyncs and resyncs are told to the players

Date: 2026-10-31. Status: accepted (issue #26). An enhancement of our own multiplayer (ADR 0005, 0007); the
original's MP set a `Desync` flag (0xB771E8, re/rules/game-end.md) that no screen showed, so there is no classic
behaviour to keep. Implemented in `ig2-client::world::World::mp_tick`, with `ClientSession::resyncs` (ig2-net) and
`mp_lobby::Session::resyncs`.

## Context
ADR 0007 §3 detects a desync from the per-turn state hashes, and the host restarts every diverged peer from its own
state (re/rules/multiplayer.md §7.2). Both happened silently: a player whose galaxy jumped back a few turns, or
whose last orders vanished, saw no reason, and a bug report had nothing to point at.

## Decision
Each resync puts one English line on the machine's top-bar message line (the `local_lines` queue the "has left the
game" lines use, shown before the sim's messages):
- The host, for each diverged peer it restarts (`SessionEvent::Resynced`): "<player>'s game was out of step and has
  been resynchronised."
- The resynced joiner: "This game was out of step with the host and has been resynchronised." A joiner counts a
  `Start` on its running link as a resync (`ClientSession::resyncs`); a rejoin comes on a new link and a host
  migration's restart goes through the migration, so neither shows the line.

Both machines also write a log line (stderr) with the empire and the turns, for bug reports.

Always on in multiplayer; single player has no lockstep and is unchanged. No classic option: the classic client is
a single-player fidelity target here, and the original's MP had no lockstep of ours to report on.

## Consequences
- A desync the host itself caused still resyncs the others (the host's state wins); the lines name the joiners.
- The lines are English only, like the other session lines: the game has no strings for them.
- Keeping the player's selection across a resync, and a per-part hash log, stay open.
