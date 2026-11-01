# ADR 0033: A difficulty level per computer player in a multiplayer room

Date: 2026-11-01. Status: accepted. An enhanced option of our multiplayer room (ADR 0014); its default is the
original's level, so a room left alone plays as the GOG release. Implemented in `ig2_net` (`GalaxyOptions::difficulty`,
`galaxy::nehezseg`, `GalaxyStart::build`, lobby protocol 8, snapshot version 9), `ig2_sim`
(`NewMpGalaxy::ai_difficulty`, `SaveState::ai_difficulty` / `difficulties`, the per-empire difficulty of the AI's
contexts) and the client's room (`mp_lobby.rs`).

## Context
The original's multiplayer galaxy has no difficulty option: `ReceiveStartData` (exe 0x5bc9d0) and the host's
`FRISSIT_Start` (0x5bc590) call `InitNewGame(seed, 2, 0, 1)`, which sets `Nehezseg` (exe 0x92827C) to 1, the
easiest level, for the whole game (`re/screens/start.md` §1). Every computer empire in the room plays at that level.
Single player has three levels (the skirmish menu's strings 15/4/1..3; `SetScirmishGameOptions` exe 0x503e80 maps
the menu's 3, hard, to `Nehezseg` 4).

`Nehezseg` is one global, but every reader that shapes a computer empire's play asks it on that empire's behalf:
- its income, research and production speed: `NehezsegPenzSzorzo`, `FejlesztesSebessegSzorzo`,
  `NehezsegGyartasSzorzo` (`MoneyHozzaad`, the research and production ticks; `re/rules/colony-side.md`,
  `re/rules/production-research.md`);
- its fleets: `CheckVeszelyesHely`'s threat margin, `MI_KitAkarTamadni`'s target choice, the flee pass's idle
  check, the war start, the colonisation boldness and the scout draw (`re/rules/ai-empire.md`'s fleet pass, §12);
- its spies: the mission draw `rndlog(10, 1, 500 − 200·Nehezseg)` (`ai-empire.md` §6.2);
- its diplomacy: the attack target (`diplomacy.md` §6.2), the provocation of a neutral, a paid deal's refusal at 4
  (`diplomacy.md` §6.4);
- the start: `BirodalomIsmeriMiotaKi` of each empire's row (`rules/new-game.md` §13).
The story's readers (deadlines, waves, crises) are the game's, not one computer's: they keep the global.

## Decision
The host sets a level for each computer slot of a generated galaxy's room: easy, normal or hard (the skirmish
menu's three, strings 15/4/1..3), easy by default (the original's `Nehezseg` 1). The level shows after "Computer"
in the slot's row; the host's click raises it, a right click lowers it (round). Joiners see it.

- The levels travel in the room's options (`GalaxyOptions::difficulty`, empires 1..=8; lobby protocol 8), so every
  machine builds the same start from them.
- At the start each computer empire gets its level's `Nehezseg` (1, 2, 4) as its own (`SaveState::ai_difficulty`;
  humans and the empires past 8 keep the game's). The global `Nehezseg` stays 1.
- Every reader in the list above asks the empire it works for (`SaveState::difficulties`, the contexts'
  per-empire `difficulty`); single player has no own levels, so each reader gets the global as before.
- The levels are part of the synced state (snapshot version 9; older saves have none: every empire the game's), so a
  rejoin, a host migration and a resumed save keep them.

## Consequences
- A room left at the defaults plays bit-exact as before (the per-empire value equals the global).
- The level is the original's AI at another `Nehezseg`: no new AI logic, no "personality" choice. A hard computer
  gets the hard income, production and research multipliers and the hard decisions; the humans' start (money, fleets)
  is the multiplayer one as before.
- The lobby protocol (8) and the save format (9) changed; older builds can't join a room of this one, and an older
  build can't read a newer save.
- The play-through's multiplayer run uses mixed levels, so its desync checks cover every level.
