# ADR 0032: Quick save, quick load and autosaves kept in turns

Date: 2026-11-01. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
autosave kept as the default. Spec: `re/screens/options.md` §5.2, `re/rules/time-and-rng.md` (Calendar),
`re/screens/input.md` §1. Implemented by `ig2-client::options` (`autosave_due`, `rotate_autosaves`) and
`ig2-client::window` (`quick_save`, `quick_load`, `autosave_tick`). Row 442 in `docs/ENHANCEMENTS.md`.

## Context
The original saves only through the Options screen (`Options_EnterPressed` exe 0x51a090, `LoadSave(slot, name, 2)`)
and autosaves into slot 24 on day 1 of months 1 and 7 (`IdoNapValtozott` exe 0x53e970 / 0x620720: single player, not
while the tutorial is active or paused, with the game page's "Auto Save" row `OptionsKellAutoSave` on), each one
over the last. Slot 23 is never written (options.md §5.2: it can't be saved to and stays empty). A crash or a lost
battle half a year after the last autosave costs that half year.

Free keys: `DoKeyboard` (exe 0x4db1a0) takes F1..F11 for the screens; `remapKeyCodes` (exe 0x4e4670) queues F12
(0x7b) as a key down, but neither `DoKeyboard` nor a screen's handler acts on it (input.md §1), so F12 is free on
every screen.

## Decision
1. **F12 quick-saves, Shift+F12 quick-loads** (single player). The quick save is `quicksave.ig2s` beside the slots
   (`slot_dir`, `IG2_SAVE_DIR`), in the slot format (ADR 0007 state plus the slot title "Quicksave <year> <month>
   <day>"). The quick save has the Options screen's save gates: not under the tutorial, not in a battle (the space
   battle or a ground war, unless `DEBUGX`), not on the main menu or with the Options screen up. Quick load loads it
   as the load button loads a slot (a running ground war opens its colony). Each shows a line on the message bar.
2. **Option "Autosave every"** (Remake row on the game page, `AutosaveDays` in the remake's settings file):
   Original (0, the default), 10, 30 or 90 game days. The original's "Auto Save" row still turns all autosaving on
   or off, and the tutorial still has none.
   - **Original**: as the exe, slot 24 on day 1 of months 1 and 7, overwritten each time.
   - **N days**: an autosave once N game days have passed since the game was started or loaded or since the last
     autosave. Before each, the previous autosave in slot 24 moves to slot 23, so the load list holds the last two
     (slot 23 is otherwise unused). Only the remake's own `.ig2s` files move.
3. **Multiplayer**: no quick load. F12 and the autosave save on the host only, as the Options screen's multiplayer
   save does (the session's save, `World::mp_save`); the original's six-monthly autosave stays single-player only
   (the exe has none in multiplayer), so a multiplayer game autosaves only with an N-day cadence.

## Consequences
- With the defaults nothing changes but the free F12 key: recorded replays and capture comparisons are unaffected
  (the window reads the option, the sim doesn't).
- A quick load drops the running game without asking, as quick loads in other games do.
