# ADR 0022: Resume a multiplayer save without every saved player

Date: 2026-10-31. Status: accepted (issue #25). Extends ADR 0005 §4 and `re/rules/multiplayer.md` §7.2. The original
has no multiplayer save (`MultiPlayerMode` 0xbae2c0 turns Load and Save off), so there is no classic behaviour to
keep. Implemented in `ig2-net::session::HostSession::resume`.

## Context
A resumed MP save needed the room's members to hold exactly the saved humans' empires. One player who couldn't come
back (or was kicked) made the save unplayable for everyone else.

## Decision
The room starts when every member holds one of the saved humans' empires; it no longer needs all of them. Each saved
human nobody holds is handed to the computer as the original hands a dropped player (message 0x404: `PlayerKi` 2,
`BirodalomMIzik` on, `INIT_MI_KIESETT`; `Game::autopilot`). The host applies that to the restored state before the
start, so every machine starts from the same snapshot. With every saved player back, the saved snapshot goes out
unchanged. A member on an empire that wasn't a saved human's is still refused ("Pick one of the saved empires").

No option: there is no classic behaviour to keep, and resuming with everyone present is unchanged.

## Consequences
- A player absent at the resume can't take the empire back: the rejoin door knows only the session's humans, and
  the next save lists only them. The empire stays with the computer for the rest of the game.
- The computer plays such an empire with the dropped-player AI, not a fresh AI empire's start.
- Tested by `ig2-net/tests/session.rs::a_save_resumes_without_a_saved_player_whose_empire_the_computer_plays`.
