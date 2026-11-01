# ADR 0050: a working high-score entry after a won scenario

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::window` (`won_scenario`, `exit_to_main_menu`, the
`HighScore` act), `ig2-client::mainscr` (`HighScore`, `open_high_score`, `high_score_key`),
`ig2-client::mainscr_scenario` (`select_registry`, `draw_high_score`, the tables read through `LoadHighscores`) and
`ig2-client::display` (the "High-score entry" row).

## Context
Every scenario has a ten-slot high-score table (re/rules/game-end.md §4–§5) and the main menu has the entry that
fills it (`MainScr_Belep` step 7, `EnterHighScoreNameMode`, IO field 0x99, `AddHighscore` exe 0x543150). But the
entry is dead code: nothing sets `scenario_nevbevitelkell` (0xBAD10C) and nothing writes `scenario_score`
(0xBAD110). A scan of the exe's code for the two addresses finds 0xBAD10C only at `InitNewGame`'s clear (0x4e1290)
and its test in `MainScr_Belep` (0x4fe620), and 0xBAD110 only read: the dialog's `score / 50` (`imul` at 0x4fdbcd)
and the `push` into `AddHighscore` (0x4feb21). The ELF's `ElertEHighscore` (0x629e40) has no caller. So the tables
only ever show the `fabscr.txt` seed.

## Decision
1. **A "High-score entry" row** (`HighScoreEntry`, on by default). Off = the exe: no entry.
2. **The score** is `Ido − scenario_kezdesi_ido`: the game time from the scenario's start to the win, in `Ido`
   units (50 a day). What is pinned: the unit (the dialog prints `score / 50` as "You managed to win in %d days",
   the table shows `score / 50` days, lower is better, and `fabscr.txt`'s seeds are days · 50), and the start time
   (`MainScr_Kilep` stores `Ido` into `scenario_kezdesi_ido` 0xBAD108 when a scenario starts, exe 0x4fef64; its
   only other references are the clear 0x4e1196 and the save field 0x5442fb, so nothing else reads it). What is a
   guess: that the original designers meant exactly this difference (no code computes it). The port now writes
   `scenario_kezdesi_ido` and `scenario_registry_name` at a scenario's start as the exe does (classic too: they
   are saved fields).
3. **When**: a single-player scenario this machine won (`MEGNYERES(true)`), on the way back to the main menu
   (after the end videos and, with ADR 0034 on, under the statistics screen): the scenario list opens on the won
   entry in high-score view (`ScenarioGombraRanyomtak(scenario_registry_name)`) under the entry dialog, as
   `MainScr_Belep` step 7 would: fader (60, 134, 330 × 130), the two lines of `txt15` section 3, "Player" in 20
   cells at (0x91, 0xdf). Enter with a non-empty name runs `AddHighscore`; Esc does nothing (the exe's
   `MainScr_CancelIPEnter` skips 0x99); the list's hot-spots are off meanwhile.
4. **Storage**: `LoadHighscores` reads `options.dat` with our settings file over it; `SaveHighscores` writes the
   21 keys into our settings file only (`options.dat` stays the original's, as for every other setting of ours).

## Rejected
- **A score from the statistics (ADR 0034)**: a new formula with no trace in the original; the table's unit is days.
- **Multiplayer scores**: scenarios are single player; a multiplayer win has no registry name.
