# ADR 0023: Formations before a multiplayer space battle

Date: 2026-10-31. Status: accepted (issue #9). An enhanced option of our multiplayer (ADR 0007, 0011); off by
default, so the classic rules keep the original's start. Implemented in `ig2_sim::game::realtime` (`MpFormation`,
`Game::space_formation`, `Game::space_ready`), `ig2_net` (`Command::SpaceFormation`, `Command::SpaceReady`,
`GalaxyOptions::formation`, snapshot version 7) and the client's battle screen and room.

## Context
A single-player space battle opens with a formation phase: with at least 6 ships the human picks one of nine
formations and presses start (`space-combat.md` §7.10 step 10, `+0x2d85`; re/screens/spacewar.md §7.1.3). The
original skips it in multiplayer (`AddFleet` sets `+0x2d85` only outside MP), so an MP battle starts at once from
the default formation, and a player with a big fleet has no say in its placement.

## Decision
The room's host has an option, "Space battle formations" (off: the original; on: every player picks). With it on:
- A new real-time space battle gives each human side with at least 6 ships a formation phase, as in SP: the
  weapon column shows the nine formations and the start button. The battle doesn't run while any of those sides
  hasn't started.
- A pick (`Command::SpaceFormation`) places only the issuer's side anew, on every machine (`felallit` of that side,
  the SP placement); start (`Command::SpaceReady`) ends that side's picking. The screen then says "Waiting for the
  other players" and keeps the column until the battle's first tick, when it slides out as in SP. Orders wait for
  the phase's end, as SP's do. The pause and speed keys keep their MP meaning (the shared clock) and don't end
  the phase (SP's unpause does); a pick once the column slides out is refused.
- The battle runs once every side has started or its player is away (dropped or left), or after 1200 ticks
  (60 s at the ADR 0007 pace), whichever comes first.
- The phase is synced state: it travels in MP saves (snapshot version 7; older saves read it as none) and through a
  host migration.

The option lives in the generated galaxy's room; a resumed save keeps the setting it was started with. A fleet that
joins a battle during the phase is placed as the original places it.

## Consequences
- Both players' screens wait on the slower player for up to a minute.
- The lobby protocol (version 6) and the save format (version 7) changed; older builds can't join a room of this
  one.
- Computer sides keep the original's placement; a human with fewer than 6 ships has no phase (as in SP).
