# ADR 0019: Battle simulate switches locked in the tutorial

Date: 2026-10-31. Status: accepted (a maintainer's play report of tutorial chapter 4, issue #28). A deliberate
deviation from the classic client (ADR 0009). Implemented by `ig2-client::options::OptionsScreen::lock_battle_simulate`,
called from `open_options` in `window.rs`.

## Context
The Options screen's game page has "Ground battle simulate" (`DebugData_grBattle`, row 1) and "Space battle simulate"
(`DebugData_Battle`, row 3) (re/screens/options.md §6). ig2.exe doesn't grey them in the tutorial: `KirakMenuPontok`
(0x5171f0) has no tutorial test and `CheckOptionsDependencies` (0x516e10) only checks the row dependencies and
multiplayer. Before the battle lesson, chapter 4's `DISABLE OVERRUN` (re/rules/tutorial.md §3 row 12) sets the four
battle flags 0xB77230..0xB7723C to 0, 0, 1, 1 and `TutorialStop` puts them back; the rows don't show it, and a row
press (`SaveOptions` 0x51b640, every row applied to its global) undoes it. So in the original a player can switch the
lesson's battles to simulated by accident, and the rows can show a state the battle doesn't use.

## Decision
While the tutorial runs (`TutorialActive || TutorialPaused`, the screen's tutorial layout), both simulate rows show
"off" and are greyed: no hover, no press. Their stored values stay as the player left them, so the settings file and
later games keep the player's choice. The overrun rows depend on them (dependency −102 / −104) and read them as off.

No classic option: the original's switch only matters by undoing the tutorial's own override, which is a trap, not a
feature.

## Consequences
- Multiplayer is unaffected (there is no multiplayer tutorial).
- The capture comparisons of the Options screen run outside the tutorial and are unchanged.
