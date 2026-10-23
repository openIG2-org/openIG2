# Keyboard path (all screens)

The exe's keyboard input, common to every screen, read for the M4 input audit (2026-10-23). The per-screen keys
are in each screen's spec; the differences from `ig2-client` are `re/deviations.md` #158-#198.

## 1. `DoKeyboard` (exe 0x4db1a0, ELF 0x57fd20)

A loop over the event queue (`FUN_004e3660` 0x4e3660, one event per call, 0 = empty). `DAT_00b771e2` is the
event's type (0 a character, 1 a key down, 2 a key up), `DAT_00b772a8` its value. There is no held-key state in
`DoKeyboard`: a held key repeats only if the queue's producer repeats it (not traced).

Not editing (`DAT_00bae2c3 == 0`):

| event | key | effect |
|---|---|---|
| char | `o`/`O` | options (`AktKepKell = 0xd`), or back to `DAT_00bae2b4` from it (0xb → 2); not on the messages screen unless idle (`DAT_00dabe3c == 0 && DAT_00da6607`), not while a screen change is pending (`bae24c/24d/250`) |
| char | `p`/`P` | pause toggle, as Space |
| char | `m`/`M` | the messages screen (`AktKepKell = 10`), not from it; on SpaceWar and a help-pending colony only with `DAT_00bae587` |
| char | `v`/`V` | `DAT_00b772c0 = SDL_GetTicks()` (no visible effect) |
| char | `` ` `` | the console entry (`ExecuteString` on Enter), with `DEBUGLEHET` or in MP, `AktKep < 0xd` |
| char | any other | `FUN_004db040(0, ch, 1)` |
| down | F1..F11 (0x70..0x7a) | `AktKepKell = key − 0x6f` (1 help … 10 messages, 11 SpaceWar); gates: no change pending, `AktKep < 0xe`, not the screen it is on, the per-VK table `DAT_00f66ebd` (tutorial) unless `DAT_00bae587`; SpaceWar and a help-pending colony need `DAT_00bae587`; the messages screen only when idle; F11 only with a battle-pending flag (bytes from 0xe436ee, stride 0x2d9c); F3 raises `TutorialCheckEvent(0x1c)` |
| down | Space | pause toggle: SP `SetIdoPause` when `AktKep != 10`, `< 0xd`, `DAT_00bad0d4 == 0`; MP `Multi_Send(0x407)` |
| down | `1`..`3` | SP: sound 10, `SetIdoSpeed(n)`, unpause (same gates); MP `Multi_Send(0x406)` then 0x407 |
| down | `0` | `FUN_00614700()`, `DAT_00b77268 = 1` (unidentified) |
| down | Enter | MainScr: `DAT_00bbd144 = 0`; the chat entry with `DEBUGLEHET` or in MP; then as below |
| down | any other | `FUN_004db040(vk, 0, 1)` |
| up | any | `FUN_004db040(vk, 0, 0)` |

Esc is not a `DoKeyboard` key outside an entry: it reaches the screens through `FUN_004db040` and
`MainCallBackEventHandler` (`ESC_Pressed`, options.md §5.1).

Editing (`DAT_00bae2c3 != 0`, `IOMode`): characters ≥ 0x20 are inserted (a name field 0x98 takes `A-Z a-z 0-9 _`,
Space as `_`); Backspace, Delete (the byte at the cursor), Home, End, ←, → edit; Enter ends it per screen (options
`FUN_0051a090`, design `DesignEntertNyomott`, MainScr `MainScr_IPEntered`, the starmap's rename with tutorial
events 3/4); Esc cancels it per screen.

## 2. `FUN_004db040(vk, ch, down)` (exe 0x4db040): the screen's handler

| `AktKep` | handler |
|---|---|
| 2 | `StarmapKeyboardHandler` (exe 0x5aeab0, ELF 0x704580) |
| 3 | `ColonyKeyboardHandler` 0x4628c0 |
| 4 | `ResearchKeyboardHandler` (exe 0x523020, ELF 0x5e9aa0) |
| 5 | `DesignKeyboardHandler` 0x4b1840 |
| 7 | `SpyKeyboardHandler` (exe 0x58cb20, ELF 0x6c5770) |
| 8 | the trade screen's handler 0x609ae0 (§3) |
| 10 | `MessagesKeyboardHandler` (exe 0x507eb0, ELF 0x5ab100; §4) |
| 0xb | `SpaceWarKeyboardHandler` 0x561ef0 |
| 0xf | `MainscrKeyboardHandler` 0x5000c0 |

The help, diplomacy, info, options, start and IRC screens have none.

## 3. Trade (exe 0x609ae0)

On key down only (`vk`): numpad + (0x6b) adds one to the quantity while something is chosen and it is below the
amount left; numpad − (0x6d) takes one off while above 0. ↓/↑ step `TradeWhichVehicle` by ±1 in the list in use
(`TradeIgazitShipPoz` 0x609a00 / `TankPoz` 0x609a70, then `TradeSelectShip` 0x60a080 / `Tank` 0x60a130, which
reset the quantity); → switches to the vehicle list when it has rows, ← to the ship list, keeping the row (the
same fit and select). Every key but ± ends in `FUN_00607b60` (a new row loads or turns the model, trade.md §6).
Port: `ig2-client::trade::Trade::key`.

## 4. Messages (exe 0x507eb0)

The character: `a`..`d` take that answer (`AktValasz0to3`, `FUN_00509d10`) when the text is typed
(`DAT_00da6607 == 1`) and the answer exists. `DEBUGX`: q/w/a/s nudge two layout values by 0.0005.
Port: `ig2-client::messages_screen::MessagesScreen::key`.
