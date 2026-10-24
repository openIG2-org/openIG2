# Keyboard path (all screens)

The exe's keyboard input, common to every screen, read for the M4 input audit (2026-10-23). The per-screen keys
are in each screen's spec; the differences from `ig2-client` are `re/deviations.md` #158-#198.

## 1. `DoKeyboard` (exe 0x4db1a0, ELF 0x57fd20)

A loop over the event queue (`FUN_004e3660` 0x4e3660, one event per call, 0 = empty). `DAT_00b771e2` is the
event's type (0 a character, 1 a key down, 2 a key up), `DAT_00b772a8` its value. The producer is
`MainCallBackEventHandler` (exe 0x4e2db0): SDL_KEYDOWN (0x300) queues the virtual key from `remapKeyCodes(SDL_Keysym&)`
(exe 0x4e4670, the keysym only; FX5 decompile `_local/decomp/fx5/0x4e4670.c`) **only** for Backspace, Enter,
Esc, Space, the top row's 1..9, F1..F12 (0x70..0x7b), PgUp/PgDn (swapped: PgUp → vk 0x22, PgDn → 0x21), the arrows,
numpad +/− (0x6b/0x6d), Ctrl, Shift and Alt; Delete, Home, End, Insert, Tab, '0', numpad Enter, the numpad digits and
the letters queue no key down (so `DoKeyboard`'s `0` case and the entries' Delete/Home/End never run), SDL_KEYUP (0x301) a key up, SDL_TEXTINPUT (0x303) `text[0]` as a character (multi-byte text is
dropped); the characters `DoKeyboard` and the screens test come only from the text events. No path reads the event's
repeat byte (+0xd), so an OS key repeat is a new key down (and a new character): a held O toggles the options screen,
held Space toggles the pause, held 1..3 sound again (FX1, row 162; the client does the same).

Not editing (`DAT_00bae2c3 == 0`):

| event | key | effect |
|---|---|---|
| char | `o`/`O` | options (`AktKepKell = 0xd`), or back to `DAT_00bae2b4` from it (0xb → 2); not on the messages screen unless idle (`DAT_00dabe3c == 0 && DAT_00da6607`), not with `KepernyokLetiltva[0xd]` (0xf66f39, also with `DEBUGX`), `AktKep < 0xe`, not while a screen change is pending (`bae24c/24d/250`); lines 54-76 |
| char | `p`/`P` | pause toggle, as Space |
| char | `m`/`M` | the messages screen (`AktKepKell = 10`), not from it, `AktKep < 0xe`; SpaceWar, a ground-war colony and the options screen when load-only or in a battle only with `DEBUGX`; not while `AktKepKell` is set (`Belep`/`Atlepes` aren't tested) or `KepernyokLetiltva[10]` (0xf66f36, also with `DEBUGX`); refused, the character goes on to the screen's handler; lines 92-114 |
| char | `v`/`V` | `DAT_00b772c0 = SDL_GetTicks()` (no visible effect) |
| char | `` ` `` | the console entry (`ExecuteString` on Enter), with `DEBUGLEHET` or in MP, `AktKep < 0xd` |
| char | any other | `FUN_004db040(0, ch, 1)` |
| down | F1..F11 (0x70..0x7a) | `AktKepKell = key − 0x6f` (1 help … 10 messages, 11 SpaceWar), lines 191-265, also while a text entry is up (the F-key block comes before the `IOMode` branch; the key then reaches no entry); gates in order: no change pending (`Belep` 0xbae24c, `Atlepes` 0xbae24d, `AktKepKell` 0xbae250 all 0); `AktKep` 0xb or 3 with `help_new_available(AktPlanet) == 1` (a ground war) only with `DEBUGX` 0xbae587; options (0xd): not load-only (`bae24e`, never, even with `DEBUGX`) and in a battle (`bae24f`) only with `DEBUGX`; messages (10) only idle (`dabe3c == 0 && da6607`, leaving it; entering it has no idle test); else `AktKep < 0xe`; then not the screen it is on, and `KepernyokLetiltva[n]` (`(&DAT_00f66ebd)[vk]` = 0xf66f2c + n) clear unless `DEBUGX`; F11 only with a battle-pending flag (bytes from 0xe436ee, stride 0x2d9c, 10 slots); F3 while `TutorialEvent` (0xf66f40) is 0x1c: the planet `AktPlanet` named `Tutorial_MainBolygoNeve` (0xf66f88) asks for nothing, any other raises `TutorialCheckEvent(0x1c)` first. The key then also goes to the screen's handler (`FUN_004db040(vk, 0, 1)`) |
| down | Space | pause toggle: SP `SetIdoPause` when `AktKep != 10`, `< 0xd`, `DAT_00bad0d4 == 0`; MP `Multi_Send(0x407)` |
| down | `1`..`3` | SP: sound 10, `SetIdoSpeed(n)`, unpause (same gates); MP `Multi_Send(0x406)` then 0x407 |
| down | `0` | `FUN_00614700()`, `DAT_00b77268 = 1` (unidentified) |
| down | Enter | MainScr: `DAT_00bbd144 = 0`; the chat entry with `DEBUGLEHET` or in MP; then as below |
| down | any other | `FUN_004db040(vk, 0, 1)` |
| up | any | `FUN_004db040(vk, 0, 0)` |

Esc is not a `DoKeyboard` key outside an entry: it reaches the screens through `FUN_004db040` and
`MainCallBackEventHandler` (exe 0x4e2db0, on the key down, not in an entry or a change): the messages screen when idle
`AktIkonKell = 0x25` (hot-spot 7), Help 0x24 (6), the starmap with the production view, the transfer, the fleet's or
vehicles' panel or the long info up (`StarmapProductMode` 0xb9933a, `StarmapTransferMode` 0xb9a304, `StarmapFleetMode`
0xf16eef, `StarmapPlanetInfoMod` 0xf16ef7, `StarmapTanksInfo` 0xf176ed) 0x27 = hot-spot 9, the panel's exit; the
colony in its planet info mode (0x9963a0) 0x3d = hot-spot 0x1f, the info panel's dead spot (nothing happens, no
options); the designer's edit mode `DesignKeyboardEscPressed`; else `ESC_Pressed` (options.md §5.1).
(`AktIkonKell` = hot-spot + 0x1e, which `IG2_gameUpdate` turns into `AktIkon2`.)

### 1.1 The screen change (`IG2_gameUpdate` exe 0x4e0440) and the client

`AktKepKell` becomes `AktKep` once `Belep` and `Atlepes` are clear; the screen up leaves through its `_Kilep` (the
research, design, trade, diplomacy, spy and messages screens run a closing animation first; towards the messages,
help or options screen trade and diplomacy go at once and silently), then the new screen's `_Belep` runs. Help goes
over the screen up (`HelpElottiScreen`). A screen entered from the messages, help or options screen (`OldKep` 10, 1,
0xd) opens without its animation (`KELLKINYITASDEFAULT`; design.md §3, trade.md §5, research.md §1; FX1 trace: research
from the options screen plays no `Res_Hangok[0]`, only `Res_Hangok[1]` 31 ticks in). Port: `ig2-client`
`window/do_keyboard.rs` (`f_key`, `request_screen`, `screen_change_step`; the request waits for a closing to run out).

FX1 traces (`_local/traces/fx1-input/`, quick-1.sav, `scripts/fkeys.json`, `fkeys2.json`, `fkeys-probe.json` with
`probes/fx1_keys.js`): F5 → design, F8 → trade, F2, F9, F10, F6 (diplomacy; towards messages quiet; from messages no
opening), F2 from diplomacy, M, O on the idle messages screen, Esc back, F1 help and F2 from it all act as above; the
entry sounds come 1-2 ticks after the key in the exe (the client's at the key). F-keys pressed while a screen's entry
or closing ran were refused (`Belep` was still 1 ~120 ticks after a load onto the starmap; the messages screen's closing
took ~80 ticks). `Belep` (0xbae24c) is set from the old screen's `_Kilep` (its closing loop: `UpdateFrame` +
`FRISSITES` until the frame float reaches 0, only when `KELLKINYITASDEFAULT` and not towards 1, 10, 0xd) through the
new screen's synchronous `_Belep` load to its first `Main` (`IG2_gameUpdate` lines 82-103, 388-392); the loads call
`FRISSITES` (e.g. `Init3dStarmap` exe 0x59c850, once per planet), whose catch-up ticks run `DoKeyboard` and refuse the
keys. The client loads without those ticks: only the closings refuse keys (re/deviations.md).

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

## 5. MP chat entry (exe `DoKeyboard` 0x4db1a0; FX15)

Enter / backquote open the entry in MP on game screens (`IOMaxLen` 0x34 at (0x112, 6), size 2, colour 3; cursor
`FillToScreen` colour 1, height 2, blinking on `AktIdo200`); editing keys edit, Esc drops, F-keys close it and switch.
Enter sends `ExecuteString`'s 0x3fc chat (exe 0x4dc140). Receiver `MULTIGET_Starmap` (0x59d930) calls `AddSzoveg`
(0x591360, `name + ": " + text`, colour 3): a ring of 8 drawn on the starmap only by `StarmapGfx2D_3` (0x5aaba0) at
(0x14, i*10+0x32-shift), one pixel per 100 ms, the top slot faded (9-shift)*10 %. Port: `ig2-client::chat`; the text
travels as `SessionMsg::Say` and the host relays `Said{empire,text}` to everyone, sender included. Not ported:
`ExecuteString`'s debug commands (DEBUGLEHET/DEBUGX), `/kick` `/kill` `/clearlog`, the 0x3fd system lines, MP
pause/speed 0x407/0x406 with `OnlyServerCanModifyTime` (0xbae55c); the timer details (0x5ab310) are untraced.

## 6. The top bar's spots on every screen (FX1 part 2)

`Mouse_Init` (exe 0x555df0, from `IG2_gameInit`) registers the bar once: `Mouse_Szoveg_Set(id, 0, x, y, w, h,
GetString(0x12, 1, n), onRelease, flag)`, `AktIkon = id + 0x1e`. Each screen's `_MouseInit` only sets the count
(`db4680`), and the hit test (exe 0x510390: the pointer clamped to 0..639 × 0..479, `x0 ≤ x ≤ x0 + w − 1`, the lowest
index wins) walks indices from the count down to 1, so the bar's spots (2..8) are live over every screen's, Help and
Options included (`AktKep < 0xe`; the main menu not probed).

| `AktIkon` | rect | tooltip (18, 1, n) | flag | effect |
|---|---|---|---|---|
| 2 | 0, 0, 15, 20 | 1 | 1 | pause toggle (`SetIdoPause`, silent) |
| 3..5 | 16/30/50, 0, 14/21/23, 20 | 2..4 | 1 | speed 1..3 when paused or at another speed: `PlaySoundGlobal(10)`, `SetIdoSpeed`, unpause |
| 6 | 619, 0, 21, 20 | 5 | 0 | the screen menu (left), the starmap (right): `crate::topbar_menus` |
| 7 | 270, 0, 323, 20 | 6 | 0 | the message line: left = the messages screen, right = the history |
| 8 | 598, 0, 21, 20 | 7 | 0 | the quick-research drop-down (`Multi_Send` 0x29/0x2a): open |

Flag 1 (`Mouse_FelengedesMajdMegvar`) acts once on the press frame. Pause and speeds (`IG2_gameUpdate` exe 0x4e0440
lines 147-181): single player not on the messages (10) or options (0xd) screen and not with `nemlehetidotallitani`
(0xbad0d4); MP `Multi_Send(0x407/0x406)` with no screen gate. Spot 7 (`MOUSE_SelectScreens` exe 0x555470 lines 90-121):
the left button held, the history closed, `AktKep != 10` and `VanUzenet` (exe 0x509e00: an inbox entry with `+4 == 0`,
unread) → `AktKepKell = 10` (the space battle, a colony at war and the options screen when load-only or in a battle
only with `DEBUGX`); no sound. The right button: `ScreensScrollHistoryMode` (0xe40c5e) while held, cleared on the
release; `GFX_DateMoneyScrollFade` draws `FadeKerettelKerekit(0x10c, 0x16, 0x146, n·10 + 5, 0, 10, 0, 0x3c, 0, 5, 0,
0x1e)` (fader pass 2) and `GFX_ScrollHistory` (exe 0x556ac0) entry i (`UzenetekDbHistory` 0xe413d8 entries of 0x40
bytes from 0xe40fb0) as `KiirStrAlap(s, 0x10c, 0x1a + 10(i − 1), 0x36, 2, 5, 0x21)` (size 2, centred in 0x36 cells;
x + 0x28 with the remaster toolbar). The remaster toolbar mode (`IG2_TOOLBAR` 0xfe6532) registers only 2..5 (28 × 18
at x 0, 0x1c, 0x38, 0x54) and 7 (x 310, 323 × 18).

Port: `ig2-client` `window/top_bar.rs` (`top_bar_mouse`, ahead of every screen's input, Help and Options included; a
press it takes also takes that button's release), the history in `ui::Hud::draw`. Verified with `scripts/topbar.json`
(quick-1.sav): the speeds sound on the starmap, the info screen and over Help, nothing on the messages screen; the right
button shows the (empty) history box in the exe; the client's sounds match at the same ticks.

## 7. The right button as the left (FX3)

`IG2_gameUpdate` (exe 0x4e0440) computes `AktIkon`/`AktIkon2` for either held button, and `Mouse_Lenyom` (0xdb465f) /
`Mouse_Felenged` (0xdb465c) are either button's edges; only `Mouse_Bal` (0xdb4651) is the left button. So a hot-spot
whose handler tests only the edge acts on a right press too. Probes (`_local/traces/fx1-input/`, quick-1.sav):
`scripts/rightbutton.json` (a right press on the bar's ×1 and ×3 sounds G10 and sets the speed, like the left),
`colony-right.json` (a right press on the colony's radar, spot 5, clicks G0 and glides the camera),
`info-right.json` (a right press on chart button 17 clicks and picks the chart).

Per screen (the spots that test `Mouse_Bal` or the button keep their own rule):

| screen | right press acts like the left on | not |
|---|---|---|
| top bar | 2..5 (pause, speeds) | 6 (right = the starmap), 7 (right = the history) |
| colony (`Colony_Main` 0x4666d0) | 1, 2, 5, 6, 8, 10, 0xb..=0x13, the info mode's spots | 4 (left/right differ), the view 0x14 (its own rules) |
| design (`Design_Main` 0x509140) | every spot but 9 and 0x54; its release (`Mouse_Felenged`) ends the hold and lets a popup act | 9, 0x54 (`Mouse_Bal`) |
| diplomacy (`DiplomGfx2D_1` 0x4b89e0) | the menus' click (`InitAlmenuk`) | the cascade (`ActFomenu` needs `Mouse_Bal`) |
| info (`Info_Main` 0x4e81c0) | 9..=12, 14..=25, 30/31, 35/36, 37 | 13 and the triangle/menus (their own right rules) |
| messages (`Messages_Main` 0x508560) | the rows, arrows, answers, leave and the text box's hurry; plus `SkipAll` (0xd90dc4 = 1, FX14) | |

Port: `ig2-client` `colony_ui::press`/`colony_war::press` (no left test on the buttons), `design::MainInput::PressRight`,
`diplomacy::Input::right_pressed`, `window.rs` `info_right_press` and the messages arm, `window/top_bar.rs`.

## 8. The pointer and the hot-spot tooltip (FX2)

`UpdateFrame` (exe 0x4e3e90, ELF 0x578e40) runs only while no screen changes (`Belep`/`Atlepes` clear, unless
`ForceUpdateFrame` 0xbae2ac) and draws, after the screen's `GFX_*`: the hot-spot tooltip, then the pointer, last.

- **Pointer:** `MOUSE.BMP` (`DDBrushToScreen3d_last`), the 20 × 20 cell of column `t` and row `(MouseFazis / 5) % 40`
  at the raw pointer (`Mouse_Akt_x/y` 0x916008/0x91600c) minus `Hot[t]` (exe 0x90b608: 0, 0, then 10 for 2..9); drawn
  when `ControlConfig[0]` (0x90b520) and `UseSystemMouse` (0xb772ad) are 0 and `Mouse_Tipus` (0xdb9414) > −1; then
  `Mouse_Tipus = −1`. −1 takes the type of the hot-spot under the pointer: `Mouse_Szoveg_Set`'s last argument, kept in
  the slot's `+0x78` (which is also its enable flag, on while < 0x40); no spot → 0; a type outside 0..9 → 0. −2 hides
  it (`MainScr_Kilep`'s loop, the tutorial's parked pointer). `MouseFazis` (0xb77204) +1 every 10 ms UI tick
  (`Fake_FRISSITES` 0x4de720), on every screen. Screen writers: the main menu's rings 1 (`Mainscreen_Update3D`
  0x500270) and every main-menu box (`Mouse_Szoveg_Set(…, 1)`: the scenario panel's 0x15..=0x27, the race spheres
  0xe..=0x10), the starmap's view spot 0x35 (whole screen, type 0; FX2 rest below), the colony's view 0x32 and build
  window 0x31 (colony.md §11.10), SpaceWar's `MousePointerType`, design 1 over a pickable pod (`PodKivalaszto` exe
  0x4b6260: within `R`, no button, `AktIkon` 0, `CanModify`, techs fit it) or a tech model within 50 px
  (`CheckIfTalalmanyfolottvan` exe 0x4ad0d0, every one in range, marked or not), research 1 over a list row (the row's
  rect in `ResearchGfx2D_2` exe 0x5221a0, the same as its press) or a model within 30 px with the models in
  (`CheckIfTalalmanyokFolottVan` ELF 0x5e2720), the MP wait overlay (`GFX_DrawOver` 0x4c8f80: while `0xaba970 > 0`
  (1 the players list `GetString(18, 5, 3..5)`, 2 the drop question 6..10) the type is `0xaba974 > 0`, its hovered item:
  the hand over an item; not the tutorial bubble). The top bar's spots pass 1.
- **The view's types** (`Starmap_MouseInit2` exe 0x5b8950 / ELF 0x7024b0, with `AktIkon2 == 0x35`): the unit under the
  pointer is `SelectNearestUnit(Mouse_Akt, 1.0, AktFleet | −AktPlanet, &kind, 0, 0)`, or `SelectNearestPlanet` in
  production mode. Something under it → 5. With the player's fleet selected (`StarmapMode` 2): nothing → 2; a foreign
  fleet → 6 (2 with `KeyAlt` 0xbae294); a planet not the player's → 2 if free or the fleet's `+0x5c` is 0, else 6 (2
  with Alt). A foreign fleet selected: nothing set (the spot's 0). A planet selected (1) with the redirect pick on
  (`StarmapAtiranyitasMode` 0xe75a58 > 0) → 6, 7 over the player's planet.
- **The rest after a move** comes from `Mouse_Lekerdez` (exe 0x5105d0): a move of more than a pixel between `Mouse_x`
  and `Mouse_Akt` sets `LastMouseMove` (0xbae268) = `AktIdo`, then `Mouse_x = Mouse_Akt`. A capture that writes both
  (the old `screen.js`) never shows a move, so the exe's tooltip came at once (row 10's 20 ticks); `screen.js` script runs
  now write only `Mouse_Akt` and the exe shows it 40 ticks after the move, as the client [obs: research-fx2.json, t 338
  none, 345 shown in both]. The physical pointer `Starmap_Autoscroll` (exe 0x5b17d0) reads is 0xdb9408/0xdb940c (window
  pixels, within 8 px of the edge scrolls); a capture window reads (0, 0) and the map drifts, so script runs write the
  script's point there too.
- **Tooltip:** `Mouse_Szoveg_Set(id, mask, x, y, w, h, text, wait, cursor)` (exe 0x5108c0) keeps the text in the slot
  (`+0x10`); `UpdateFrame` shows the lowest enabled slot under the pointer (`AktIkonKiir`) after a 400 ms rest
  (spy.md §6.1), as `KiirStrAlap(text, x + 0x15, y + 2, 99, 0x1a, 0xc, 2)`, only below `AktKep` 0xe, not with
  `ControlConfig[0]` or the options screen's quit question (0xdb975a); the box (exe 0x554500, each screen's `GFX_*`)
  only for a non-empty text. The main menu (0xf) never shows one: its rings' texts (15, 1, n) are dead. The top bar's
  texts are `GetString(18, 1, 1..7)` on its slots 2..8, on every screen below 0xe. The colony's (`Colony_MouseInit` exe 0x4680e0,
  `GetString(3, 1, n)`): 1 → 1, 2 → 2, 5 → 4, 6 → 5, 8 → 6, 0xa..=0x12 → 7..=0xf (the war's buttons), 0x15..=0x1e →
  0x10..=0x19, 0x20 → 0x1a, 0x13 the literal "Build", 3, 7, 9, 0x14, 0x1f none; hot-spot 4's text is rebuilt every
  frame (`Colony_MouseInit2` exe 0x467990): `(3, 3, 1)`, or `(3, 3, 2)` in the planet info mode, and with nothing
  selected in single player `(3, 3, 3)` appended (`strncat` to 0x46); their enables are the hot-spots' own (colony.md
  §11, §12). The starmap's (`Starmap_MouseInit` exe 0x5b99d0,
  `GetString(2, 1, n)`, switched by `Starmap_MouseInit2` 0x5b8950; FX2 part 2): 1..=7 (the console's and minimap's
  mask spots) and 0xa..=0xd → n = id; 0xe..=0x15 → 0xe; 0x16..=0x19 → 0xf; 0x1a..=0x21 → 0x10; 0x22..=0x29 → 0x11;
  0x2a..=0x31 → 0x12; 0x32 → 0x13; 0x36 → 0x14; the panels' cells 0x43..=0x52 → 0x27 (vehicles, fleet), 0x25
  (transfer), else 0x15; 0x53/0x55 → 0x16, 0x54/0x56 → 0x17; 0x57 → 0x18; the long info's 0x5a..=0x63 → 0x19..=0x22;
  0x33, 0x34, 0x35, 0x58, 0x59, 0x64, 0x6f none; the exit 9 → 0x28 (vehicles, long info), 0x26 (fleet), 0x24
  (transfer), 0x23 (production, or its opening), else 9; the info box 8 → planet mode 0x29 (0x2a with the long info),
  fleet mode 0x2b (the fleet's panel), else for the player's fleet 0x2c (transfer), 0x2d / 0x2e (`+0x52` bit 0 clear /
  set), a foreign fleet nothing; then in single player, in planet mode without the long info or on the player's fleet,
  "  " and `(2, 1, 0x2f)` (`strncat` to 0x50). Enables: everything off, then 1..=7 only without the long info and the
  fleet's zoom (the tutorial's starmap lock keeps only these), 8 always, 9 with a panel or the long info, 0xa / 0xb with
  the tanks / production buttons, 0x35 without the long info. With the radial menu open, the icon under the pointer
  (`AktRopIkon2 > 0 && StarmapRopIkonMod`) shows `StarmapRopIkon_Nev[icon]` (`txt17` §6) at once, without the rest
  (`UpdateFrame`'s special case) [the icons' names: not checked against a capture]. During the starmap's entry
  (`Belep`, ~100 ticks after a load) `UpdateFrame` doesn't run and no tooltip shows [obs: FX2 trace `stips`].

Port: `ig2-client` `window/pointer.rs` (`frame_overlay`: the top bar's tooltip and the pointer over every frame, the
window's own pointer hidden; none while a screen closes, nor where the screen draws its own: the colony, the space
battle, the tutorial's recording); the hand over the bar's spots and the main menu's rings; snapshots draw both at
`--mouse` (`Hud::snapshot_pointer`; verified: research held at frame 60 with the pointer at (300, 250) and on the
×2 button (40, 8): the pointer and the "Normál idő" box match the exe's capture). The colony's tooltips (FX2 part 3):
`ColonyUi::tip`, shown through the same overlay (the bar's slots win), and the colony's pointer moved into it (drawn
last, the hand over the bar); verified on colony:42 in the planet info mode with the pointer on an "ALL" button: the
"Minden bolygón" box and the pointer match the exe's capture. The starmap's (FX2 part 2): `crate::starmap_tip` (the console mask,
the panels' and the long info's spots, the radial icons), through the same overlay; the starmap snapshot draws it at
`--mouse` when no menu is staged. Verified (`scripts/starmap-tips.json`, quick-1.sav): the info box ("Bal:Info
Jobb:Kolónia Mindkettő:Átnevezés"), the minimap ("Radar"), the production button ("Gyártás") and the disabled tanks
button (nothing) match the exe's grabs.

FX9: `run_screendiff.py info --info-mouse X,Y` stages the pointer on kep 9 (`screen.js`) for the info tooltips.

FX2 rest: every screen's `*_MouseInit` passes cursor 1 to its spots (Help, Info, Irc, Messages, Research, Spy, Start,
Trade, SpaceWar, Design all 1; the colony's 3, 7, 9, 0x13, 0x14, 0x1f and the starmap's 0x33, 0x34, 0x35, 0x58, 0x59,
100, 0x6f pass 0), so the hand shows over every enabled spot. Port: `kit::Tooltip::under` (the spot the screen's
tooltip saw under the pointer) gives the hand on research, design, trade, spy, help and options, the info spots on
info, the starmap tip's id. The screens' own types in `window/pointer.rs` (`frame_overlay`): the main menu's `MainScr::hand`, research's
`Research::hand`, design's `Pods::hand`, the starmap's `orders::view_pointer` (over 0x35: no long info, info or transfer
panel, no lower spot); the main-menu, research and design script sequences and the starmap still at `--mouse` draw the
pointer too (phase 0, row 277). Verified: `starmap-fx2.json` (quick-1.sav, Magellan 8 selected): 5 over two planets, the
arrow on empty space, as the exe; `research-fx2.json` the tooltip's rest as above and the pointer per grab (the hand on
the urgent button, a row and the bottom ring, the arrow on an empty row); `design-pods.json` (stored exe run) and
`mainscr-race.json` / `mainscr-scenario.json` (stored FX11 runs): arrow/hand per grab as the exe.
