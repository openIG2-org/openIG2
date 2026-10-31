# ADR 0026: Defeated players watch a multiplayer game

Date: 2026-10-31. Status: accepted (issue #37). An enhanced option of our multiplayer (ADR 0007, 0011); off by
default, so the classic rules keep the original's end. Implemented in `ig2_net` (`GalaxyOptions::spectators`,
`Command::apply`'s dead-issuer check, snapshot version 8), `ig2_sim` (`Game::spectators`,
`LocalView::spectating`) and the client's room, world and window.

## Context
In the original a player whose empire dies leaves the game: `BirodalomMeghal(EN)` runs `ExitToMainScreen` 0x5117f0
(ELF 0x5bc0b0), which in multiplayer sends `nam:, stat:0`, closes the info socket and calls `MultiCloseClient(1)` (a
joiner) or `MultiCloseServer(-1, true)` (the host), then plays the defeat videos and shows the main menu
(`re/rules/game-end.md` §2). The port does the same (`exit_to_main_menu` → `Session::leave`). A defeated player can't
see how the game ends, and a defeated host ends it for everyone unless a successor takes over.

## Decision
The room's host has an option, "Defeated players" (off: leave, the original; on: watch). With it on, a machine
whose own empire dies:
- plays the defeat videos as before, then goes back to the starmap instead of the main menu, and stays in the
  session: it keeps sending its (empty) turns and its state hashes, so the lockstep and the desync checks run as
  for any peer. A host keeps hosting.
- sends no orders: the client drops whatever its screens queue (the pause and speed buttons too); chat still works.
  With the option on, every peer also ignores a player order whose issuer's empire is dead (`EMPIRE_ALIVE` 0), so
  a modified client can't act for a dead empire either. The sim's own lockstep messages (`Command::from_event`, away/back) are kept.
- sees the whole map: every planet and fleet, every name (as `DebugX` shows it), drawn from the shared state on
  that machine only (`LocalView::spectating`, not synced).
- leaves with the menu's exit as any player does (the session's `Leave`).

The option lives in the generated galaxy's room and in the state (snapshot version 8; older saves read it as off),
so a rejoin and a host migration keep it. A resumed save keeps the setting it was started with.

## Consequences
- The others still wait for a slow spectator's turns, as they did while it played.
- Watching the whole map lets a spectator tell an ally about the others: that is why the host decides.
- The lobby protocol (version 7) and the save format (version 8) changed; older builds can't join a room of this one.
- Not covered: joining a running game as a spectator (the door admits only the game's humans), an own-empire
  "follow" view.
