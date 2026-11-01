# ADR 0058: key rebinding, a voice volume and extra game speeds

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::bindings` (the keys), `ig2-client::options::modern`
(the Controls tab), `ig2-client::display` (the rows), `ig2-client::audio` (the voice volume) and `ig2-client::window`
(the key remap, the extra speeds' ticks).

## Context
The original's keys are fixed (`DoKeyboard` exe 0x4db1a0 and the screens' keyboard handlers, re/screens/input.md §1,
§2), its two volumes are music and sound, where sound covers the effects and every spoken line, and its fastest speed
is ×16 (`SetIdoSpeed` exe 0x541330: 1/2/3 → `IdoSpeedSzorzo` 1/4/16 in single player). Players ask for their own
keys, quieter or louder speech than effects, and a faster run through quiet years.

## Decision
1. **Key rebinding.** One table of actions (`bindings::ACTIONS`): each is one of the original's hotkeys or the
   remake's (W/A/S/D camera ADR 0030, the event log ADR 0040, the quick save ADR 0032), with the place it applies
   (everywhere, or one screen: starmap, colony, research, design, messages, space battle; `Ctx::of_screen` of
   `AktKep`) and the original's key as its default. Esc, Enter, the arrows, the group digits 4..9 and the
   modifiers stay fixed: text entries, menus and Ctrl+digit share them.
2. **One remap point.** A key event is rewritten at the top of the window's event handler: a key bound to an action
   of the place becomes that action's default key (its physical code, logical key and text as a US layout gives
   them; a typed capital stays one), so every screen keeps matching the original's keys; the default key of an
   action bound to another key does nothing there; any other key passes. Key up runs through the same table, so a
   held key ends. Not rewritten: the main menu, a video, the main menu's Options screen, a text entry, the open event
   log. Synthetic keys (the tutorial's recording, the profiling tour) never pass the handler and stay the original's.
3. **Clashes.** Two actions clash when they share a key where both apply (the same screen, or one of them
   everywhere). A clash is allowed but shown: the key in the warning colour and, on its line, "Also the key of:" and
   the other action. The defaults clash nowhere (a test).
4. **Controls tab.** The main menu's Options screen (ADR 0053) gains a Controls tab: a line per action (a screen's
   name before its own actions), the key at the right, and "Reset all keys". Enter or a click waits for the new
   key ("Press a key"); Esc gives up; a key outside the bindable set (letters, digits, F1..F12, Space, PgUp/PgDn,
   Home/End/Ins/Del, Tab, punctuation, the numpad) is ignored. The list scrolls (the focus stays in view; the wheel).
   Apply and Back hold the keys as the other settings. Stored as `Bind<id>` = the key's name in the remake's settings
   file; a missing or unknown one is the default.
5. **Voice volume.** A remake row "Voice volume" (Sound group; the main menu's Sound tab, the in-game Remake page):
   "Same as sound" (the default, the original's behaviour) or 0..100 % in steps of 10. It sets the global volume of
   the spoken lines in place of the sound volume: the speakers and the start voice (`PlayWav`), the message speech
   (`PlayMessagesHang`) and the aliens' voices (`Mes_Hangok[3..19]`). Music keeps its slider; the sound slider is
   then the effects'. Heard as it is changed on the screen.
6. **Extra game speeds.** A remake row "Extra game speeds" (Game, off by default). With it on, in single player, the
   fastest speed asked for again (key 3, or its top-bar button) while it runs doubles the sim ticks per tick, then
   quadruples them (×32 and ×64 of the original's time); the top bar shows "x2" or "x4" by the fastest button. Any
   other speed, a slow-down (`SetIdoLelassit`), a space battle, the row off end it. The sim is unchanged:
   the ticks are the original's, only more of them per frame, so saves, replays and the AI see nothing new.
   Multiplayer keeps the host's lockstep speeds (ADR 0011), a replay its recorded speed.

## Rejected
- **Rebinding inside each screen's handler**: thirty places to change and to keep in step with the specs; one remap
  in front of them keeps the ported handlers as the original's.
- **Bindings with modifiers (Ctrl+K)**: the original's keys carry none (bar Shift on PgUp/PgDn and F12); the
  modifiers stay what they are.
- **A larger time step for the extra speeds** (`IdoSpeedSzorzo` 32 or 64): the sim's per-tick work (moves, battles'
  timers, the daily update) assumes the original's steps; more ticks of the original's size keep it exact.
- **A third slider for speech**: the in-game screen's three sliders are the exe's layout; a row keeps both screens
  on one model (ADR 0048).
