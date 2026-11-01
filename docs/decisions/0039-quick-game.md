# ADR 0039: A quick game from the main menu

Date: 2026-11-01. Status: accepted. A deliberate deviation from the classic client (ADR 0009), switched with the
main menu's other remake entries (ADR 0031). Spec: `re/screens/mainscr.md` §9.3, §14. Implemented by
`ig2-client::mainscr` (`Act::QuickGame`, the label, `MainScr::open_quick`), `ig2-client::mainscr_scenario`
(`Skirmish::config`, `Skirmish::from_setting`, `ScenarioList::quick`) and `ig2-client::window` (the start, the
remembered settings).

## Context
A generated galaxy (a skirmish) takes four steps in the original: the single-player ring, the scenario ring, the list
(its first entry, "Skirmish", is selected), then the seven settings and Start (`ScenarioGombraRanyomtak` exe
0x503bf0, `MainScr_Main` exe 0x4fefc0, `SetScirmishGameOptions` exe 0x503e80; spec §9.3). The settings start from
`InitScirmishValtozok` (exe 0x4f8f80: race 1, 3 computers, the rest 2) on every visit: the exe keeps no last choice.

## Decision
1. While the main menu's main ring set is up and idle and "Settings in menu" (`MenuSettings`, ADR 0031) is on, a
   "Quick game" label shows above the Settings label (bottom right). Off (0): the original's menu, neither label.
2. A press on it opens the scenario panel holding only the skirmish entry, selected, with its seven settings (race,
   computers, difficulty, planets, technology, money, population) and Start: the original's panel, input and draw,
   so the second click is Start. Esc or the panel's close goes back to the main ring set.
3. The settings are the last skirmish's: every skirmish start (quick or from the full list) stores them as
   `QuickGame` (seven integers, comma-separated) in the remake's own settings file; without the key, or with a
   value out of its range, the setting is `InitScirmishValtozok`'s default. The full list keeps the original's
   defaults on every visit.
4. Start is the full list's start (`Start::Skirmish`, kind 1, `SetScirmishGameOptions` with the same tables): the
   same choices and seed make the same galaxy (test `quick_game_matches_the_full_setup`).

## Consequences
- The original's single-player path and the skirmish panel are unchanged; capture comparisons of the main menu set
  `MenuSettings` 0 (the labels aren't the exe's).
- Recorded replays are unaffected: the quick game starts the sim exactly as the full setup does.
