# ADR 0026: Spectators in a multiplayer game

Date: 2026-10-31. Status: accepted (issue #37). An enhanced option of our multiplayer (ADR 0007, 0011); off by
default, so the classic rules keep the original's end. Implemented in `ig2_net` (`GalaxyOptions::spectators`,
`Peer::run_turn`'s dead-issuer check, snapshot version 8, `HostSession`'s spectator door, `Host::add_watcher`),
`ig2_sim` (`Game::spectators`, `LocalView::spectating`) and the client's room, world and window.

## Context
In the original a player whose empire dies leaves the game: `BirodalomMeghal(EN)` runs `ExitToMainScreen` 0x5117f0
(ELF 0x5bc0b0), which in multiplayer sends `nam:, stat:0`, closes the info socket and calls `MultiCloseClient(1)` (a
joiner) or `MultiCloseServer(-1, true)` (the host), then plays the defeat videos and shows the main menu
(`re/rules/game-end.md` §2). The port does the same (`exit_to_main_menu` → `Session::leave`). A defeated player can't
see how the game ends, and a defeated host ends it for everyone unless a successor takes over. Once a game has
started, its door admits only its own humans coming back (`Refused{Started}` for anyone else; the original has no
joining after the start either).

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

The same option opens the running game to spectators: a connection whose name is none of the game's humans, which
the door would turn away as `Refused{Started}`, is admitted after the usual checks (version, rules, join code,
password) as a spectator, at most eight at a time (`Refused{Full}` beyond):
- it gets `Welcome` with a spectator slot (0xF0..=0xF7, `SPECTATOR_SLOTS`), then the host's state now as a `Start` for
  empire 0 and the bundles since, like a rejoiner; the players' message lines say "<name> is watching the game.";
- the host relays every bundle and message to it but never waits for it: it owes no input (`first_input` =
  `u64::MAX`), anything it sends but chat and leave is dropped, and its hashes aren't compared (no resync);
- its machine sees the whole map as a defeated player's does, its game never ends (`Game::over` set), and its chat
  lines are signed "Spectator";
- it takes no part in a host migration: the game ends for it when the host leaves.

The option lives in the generated galaxy's room and in the state (snapshot version 8; older saves read it as off),
so a rejoin and a host migration keep it. A resumed save keeps the setting it was started with.

## Consequences
- The others still wait for a defeated player's turns, as they did while it played; a late spectator holds nobody
  up (it only receives).
- Watching the whole map lets a spectator tell an ally about the others: that is why the host decides.
- The lobby protocol (version 7) and the save format (version 8) changed; older builds can't join a room of this one.
- A late spectator plays empire 0, an empty slot: the top bar and the other screens show that slot's data.
- Not covered: an own-empire "follow" view, resyncing a spectator that drifts (it would have to leave and join
  again).
