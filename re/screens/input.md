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
held Space toggles the pause, held 1..3 sound again (FX1, row 162; the client does the same, SpaceWar included: row 270, no repeat test in 0x4e2db0; spacewar.md §9.14).

Not editing (`DAT_00bae2c3 == 0`):

| event | key | effect |
|---|---|---|
| char | `o`/`O` | options (`AktKepKell = 0xd`), or back to `DAT_00bae2b4` from it (0xb → 2); not on the messages screen unless idle (`DAT_00dabe3c == 0 && DAT_00da6607`), not with `KepernyokLetiltva[0xd]` (0xf66f39, also with `DEBUGX`), `AktKep < 0xe`, not while a screen change is pending (`bae24c/24d/250`); lines 54-76 |
| char | `p`/`P` | pause toggle, as Space |
| char | `m`/`M` | the messages screen (`AktKepKell = 10`), not from it, `AktKep < 0xe`; SpaceWar, a ground-war colony and the options screen when load-only or in a battle only with `DEBUGX`; not while `AktKepKell` is set (`Belep`/`Atlepes` aren't tested) or `KepernyokLetiltva[10]` (0xf66f36, also with `DEBUGX`); refused, the character goes on to the screen's handler; lines 92-114 |
| char | `v`/`V` | `DAT_00b772c0 = SDL_GetTicks()` (no visible effect) |
| char | `` ` `` | the console entry (`ExecuteString` on Enter), with `DEBUGLEHET` or in MP, `AktKep < 0xd` |
| char | any other | `FUN_004db040(0, ch, 1)` |
| down | F1..F11 (0x70..0x7a) | `AktKepKell = key − 0x6f` (1 help … 10 messages, 11 SpaceWar), lines 191-265, also while a text entry is up (the F-key block comes before the `IOMode` branch; the key then reaches no entry); gates in order: no change pending (`Belep` 0xbae24c, `Atlepes` 0xbae24d, `AktKepKell` 0xbae250 all 0); `AktKep` 0xb or 3 with `help_new_available(AktPlanet) == 1` (a ground war) only with `DEBUGX` 0xbae587; options (0xd): not load-only (`bae24e`, never, even with `DEBUGX`) and in a battle (`bae24f`) only with `DEBUGX`; messages (10) only idle (`dabe3c == 0 && da6607`, leaving it; entering it has no idle test); else `AktKep < 0xe`; then not the screen it is on, and `KepernyokLetiltva[n]` (`(&DAT_00f66ebd)[vk]` = 0xf66f2c + n) clear unless `DEBUGX`; F11 only with a battle-pending flag (bytes from 0xe436ee, stride 0x2d9c, 10 slots; `spw[b]+0x1f2e`, set by `AddFleet` 0x5646b0, cleared by `EndSpaceWar` 0x566c30; `SpaceWarIndit` sets `AktKepKell` = 0xb at once and `SpaceWar_Belep` disables the screens for the whole battle, so in practice only with DEBUGX, ADR 0016 §4); F3 while `TutorialEvent` (0xf66f40) is 0x1c: the planet `AktPlanet` named `Tutorial_MainBolygoNeve` (0xf66f88) asks for nothing, any other raises `TutorialCheckEvent(0x1c)` first. The key then also goes to the screen's handler (`FUN_004db040(vk, 0, 1)`) |
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

Frame order (`IG2_gameUpdate` lines 38-53): the frame's ticks, the draw, the mouse poll and hit-test, then the screen's `_Main`, so a press's 2D effect is drawn one rendered frame later than in the client (kept, ADR 0016 §5, row 229).

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
travels as `SessionMsg::Say` and the host relays `Said{empire,text}` to everyone, sender included. The timer
details (0x5ab310) are untraced.

### 5.1 `ExecuteString` (exe 0x4dc140, ELF 0x57a980; FX15)

Enter in the entry runs the line (at most 0x33 bytes) through two stages; decompile and asm in
`_local/decomp/fx15-console/` (`exe/ExecuteString_head.asm` = 0x4dc140..0x4dc400).

Stage A, only in MP (`MultiPlayerMode` 0xbae2c0), first match wins:

| line | gate | effect |
|---|---|---|
| `/clearlog` (`_stricmp`, exact, any case; str 0x854258) | `DAT_00b7726c == 0` (asm 0x4dc192) | `Multi_Send(0x405, EN, 0)`; every machine's receiver runs `ClearLogfiles` (exe 0x53c230: truncates `senddump.!`, `truedump.!`, `recvdump.!`, `hexdump.!`, again only with `b7726c == 0`) |
| `/kick ` or `/kill ` (`_strnicmp` 6, the same code; str 0x854264 / 0x85426c) | `SuperVisor` (0x912a3a, the host) | the target below; nothing is sent to the others |
| anything else, a non-host's `/kick` and a `/clearlog` with `b7726c != 0` included | | the chat: `Multi_Send(0x3fc, EN, strlen + 1, line)` (asm 0x4dc34c) |

`b7726c` = (`DAT_00fe6530` == 0) in `IG2_gameInit` (exe 0x4dfae5); no writer of 0xfe6530 was found in any
decompile, so the packet dumps are off and `/clearlog` goes out as an ordinary chat line [inf: 0xfe6530 is a
zero-initialised switch]. Ours: always the chat.

The kick target: skip spaces after index 6; a first byte `'1'..='8'` (asm 0x4dc212 `cmp al,0x31; jl`, `cmp al,0x38;
jg`) is the empire number (only that one digit is read); else the first empire `i` in 1..`ProgramPlayers` (0x912acc)
with `PlayerKi[i]` (0xbae560 + i) == 1 (a human still playing) whose `Faj_Nev[i]` (the race name list indexed by the
empire number, `GetString(0x11, 1, i + 5)`, as `diplomacy.rs` already does) equals the rest of the line
(`_stricmp`: the whole rest, any case); else the same walk against `PlayerNev[i]` (0xbae588 + i·0xb). Then: no match
or `PlayerKi[i] != 1` → `AddSzoveg("No such player.", 6)` (str 0x85428c); `i == EN` → `AddSzoveg("Cannot kick
yourself.", 6)` (0x854274, asm 0x4dc305); else `MultiKickClient(PlayerHanyasKliens[i] (short 0xbae65c + 2i), 2)`
(exe 0x512410, asm 0x4dc31e-0x4dc329). Both lines are local and red (colour 6; `dsz`'s test lines: 6 red, 2 white,
3 chat, 7 system). `MultiKickClient` sends socket packet 99 with the reason (2 = console; 1 = the DrawOver player
menu, 7 = lobby full) and drops the socket; the kicked machine's `ReceiveData` (exe 0x515910, state 10) sets
`DAT_00db94f4 = 1`, `DAT_00db94f8 = reason` (its disconnect), and the host's drop path makes the empire a computer
one (0x404, `PlayerKi = 2`), so a second kick of it is "No such player.".

Ours (`ig2-client::console`, `World::say`, `HostSession::kick`): a kickable empire is a session human still
connected, `PlayerNev` its lobby name, `Faj_Nev` the language's race names. The host sends `SessionMsg::Kicked`
and lets the link go; the empire is away at once, as a player who left (`PlayerAway` in a running battle, no
grace: the original's computer takeover); its name is refused at the door from then on (`Refusal::Kicked`; the
original has no way back at all; not kept in an MP save or across a host migration). The kicked joiner's
`ClientSession::kicked()`: closed, no successors (it never migrates); its message line says so (ours). The kick
runs on the state at the next turn, so the client swaps its game into the peer around it (as for the MP save).

Stage B, only with `DEBUGLEHET` (0xbae659) or `DEBUGX` (0xbae587), runs after stage A (a chat line too): a
`_strnicmp` prefix chain of about 90 cheats (`debug` toggles `DEBUGX` and sends the 0x3fd line "enabled/disabled
cheat mode"; `allweapons` and `destroy` send 0x75/0x76 and a 0x3fd line; `win`, `m `, `kill`, `r`, `story`, `ctut`,
`seten`, toggles of debug draws, …; `dest` (3 bytes) shadows `desynch` and `destroy`). `DEBUGLEHET` is set only
from `init/config`'s `debugmode` (exe 0x4dfb6a); the GOG install has no `init/config`, and `DEBUGX` is set only by
stage B's `debug`, so on a GOG install stage B and every 0x3fd line are unreachable. Not ported (a keep, ADR 0016:
the cheats act on one machine and would desync the lockstep). The 0x3fd receiver (`MULTIGET_Starmap`, asm
0x59ec41): `AddSzoveg(PlayerNev[sender] + " " + text, 7)`.

### 5.2 MP pause and speed (0x406/0x407; FX15)

The sender alone is gated: `DoKeyboard` (Space and `p`/`P` lines 80-90/301-311, `1`..`3` lines 316-331; the same in
the ELF at lines 83, 409, 429) and `IG2_gameUpdate`'s top-bar spots 2..5 (exe 0x4e0831, 0x4e08e9) send only when
`OnlyServerCanModifyTime` (0xbae55c) == 0 or `SuperVisor`. Pause: `Multi_Send(0x407, EN, 1, &(IdoPause == 0))` (the
new value from the sender's own `IdoPause`); a speed: `Multi_Send(0x406, EN, 4, &n)` then 0x407 with 0. No screen or
`nemlehetidotallitani` test in MP. The receiver (`MULTIGET_Starmap` asm 0x59f364/0x59f3a7) runs `SetIdoSpeed(n, 1)`
/ `SetIdoPause(b, 1)` with no sender test; their own gate is `ProgramSebesseg` (4 = the players set the speed;
otherwise the room's fixed speed and no pause; ported in `Game::set_speed`/`set_pause`). The flag is the room's
(`Start_Main` ids 0x28/0x29 → 0/1, sent in the start status, saved; ours `GalaxyOptions::host_time`). Ours:
`ig2-client` `Window::time_mp_allowed` (`do_keyboard::mp_time_gate`) reads the flag from the shared state and the
host from the session; the keys, the top bar and SpaceWar's keys send through `toggle_pause`/`set_speed`.

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
only with `DEBUGX`); no sound. FX1: the left button is tested every held frame (no press edge) while the hold began on the line (`bar_line_held`), so a message arriving while held, or the pointer coming back onto the line, asks for the messages screen (`IG2_gameUpdate` 0x4e0440 l.51-53 re-hits `AktIkon` only while `Mouse_Megfogva` is 0; a hold that began over no hot-spot (`AktIkon` 0) and reaches the line asks too, client `bar_free_hold`, never over the starmap view: deviations 340, closed). The right button: `ScreensScrollHistoryMode` (0xe40c5e) while held, cleared on the
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
| research (`Research_Main` 0x523c10) | every spot: start/stop, urgent, categories, the running project 0x19 (it tests no button; `Felenged` = "not a release frame") | the list rows and model pick (`ResearchGfx2D_2` 0x5221a0, 0x51d2d0 need `Lenyom`&&`Bal`) |
| spy (`Spy_Main` 0x58d410) | every spot (it tests only `Lenyom`/`Felenged`): kill 0x47 (spot 2), recall 0x48 (4), send 0xe, cancel 0xf, the lists 0x10..0x13 (3 is a list), verdict 0x14..0x17, skills 0x19..0x1c, hire/reject/later 0x1e..0x20, repeat 0x21 | |

Probes (FX3 rest, 2026-10-25, `_local/traces/`, scripts `tools/trace/scripts/research-right.json`, `research-left.json`, `spy-right.json`): research start/stop, urgent, the power category and the running project act identically for both buttons, a list row picks only with the left; spy: a right press on "later" 0x20 clicks, sends msg 0x4b and plays `Spy_Hangok[1]`, on the mission list 0x10 it clicks. Client: `window.rs` research arm sends either press to `research_press` (`Research::press(.., left)` gates rows and models); the spy arm presses on the first button down and releases on the last up. `Mouse_Lekerdez` (exe 0x510694..0x510717) takes the edges from the any-button `Mouse_Gomb`, so a second button's edge while one is held is none (deviations row 351, settled). The category click is `Forgotabla`'s (exe 0x51e360, `PlaySoundGlobal(0)` while open), reached only by the middle wheel's categories 1..=5 (`choose_area`); the off-wheel TT_* handlers (0x524a50..) are silent.

**Held frames (FX3 rest).** `IG2_gameUpdate` (exe 0x4e0440) calls a screen's `_Main` only on frames a button is down over a
hot-spot (every held frame for a release-flag-0 spot, the press frame only for flag 1) and once on the release over the
same flag-0 spot; never for the pointer alone (a spot rested on 40 frames with no button: nothing). Info: the star-log
arrows 35/36 (exe ids 0x23/0x24) step `StarLogPointer` once a held frame, the release frame included (36 held 15 frames:
16 steps), either button; click only on the press. The scroll bar 34 (0x22) in list mode follows `Mouse_y` every held
frame, past the bar's edges too (`Mouse_Megfogva`), formula = the client's `scroll_to` (y 348 -> 0, 357 -> 1 with 5
entries); the release moves nothing. Main menu: a right press acts like the left on the scenario panel boxes 0x15..0x27
(row pick, arrows with their click, cancel, the high-score toggle 0x27), the race spheres and the difficulty rings; never
on a menu ring (`mainscr-right.json`, `mainscr-right-boxes.json`, `mainscr-right-race.json`). Detail: mainscr.md §6.3,
info.md §5. Client: `mainscr.rs` `press_right`/`release_right`, `window.rs` front-end right-button arm.

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
  it (`MainScr_Kilep`'s loop, the tutorial's parked pointer). The client sets `pointer_in` on every CursorMoved/CursorEntered before the per-screen arms (FX17b; the main menu, Options and Help lacked it, so no pointer showed). `MouseFazis` (0xb77204) +1 every 10 ms UI tick Stills (row 277, 2026-10-25): screen.js records it per grab (`ui.json` mouse_phase), run_screendiff.py passes `--mouse-phase N`, the snapshot pointer draws row (N/5)%40; script sequences still start at their own phase.
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

## 9. The MP wait overlay (`GFX_DrawOver`; row 296, ported 2026-10-25: `ig2-client::mp_wait`)

An in-game multiplayer modal, not part of the main menu. `MultiSendPackets` (exe 0x5126b0) sets `WaitingForVector` 0xdb9748
once a turn (≥ 14) has lacked its data for 5000 ms (1000 ms with a join pending, `db94b4 > 0`) and clears it when the data
comes. `MOUSE_DrawOver` (exe 0x4c95b0, ELF 0x5306a0; from `IG2_gameUpdate` 0x4e0440) then opens the overlay (`DrawOverMode`
aba970 = 1, = 2 with a join pending; `acttime` aba978 = `SDL_GetTicks()` at the open; never with `DEBUGX` bae587) and runs its
own loop (`WaitForDrawing`, `Fake_FRISSITES`: the sim and `MultiSendPackets` still run, `UpdateFrame`, the mouse poll) until it
closes: the screens' handlers take no input. The boxes are `FUN_004c8b80` (fader pass 3, `FUN_004d1860` =
`FadeKerettelKerekit` untextured), the texts `GFX_DrawOver` (exe 0x4c8f80, ELF 0x52f920; `KiirStr(s, x, y, len, size, flags,
colour)`, flags & 3 = alignment). Geometry: literals (`DrawOverX/Y/Xm/Ym` = 0x28, 0x32, 0x230, 0x10e; 0x5a in mode 2).
- Panel (40, 50, 560, 270), fill (0, 0x14, 0x1e) 0x3c, frame (0, 0xf, 0x19) 0x1e; the exit strip (40, 330, 560, 30) after
  4000 ms, the same colours.
- Title `GetString(18, 5, 1)` "Connecting..." if `SocketConnectingMode` e4162c (no writer in the exe), else (18, 5, 2) "Waiting
  for players to reconnect..." if `MultiPlayerReconnectingMode` db94e8 (`OnCloseSocket` 0x5142b0's reconnect wait), at (40, 40),
  len 0x28, size 4, left, colour 3; none otherwise.
- Mode 1, a cell per late slot (`FUN_00510fd0(i)`: on the host the slots whose data is missing, on a joiner the host's late
  list, else the host), k-th at (50 + 140·(k % 4), 60 + 140·(k / 4)), box 120 × 70, fill (0, 10, 0) 0x3c, frame (0, 5, 0) 0x1e:
  (18, 5, 3) "Slow connection!" at +10, the name at +0x1e, the slot's text at +0x32, len 0x14, size 2, centred in the cells,
  colour 0x25. After 4000 ms on the host a button box (cx, cy + 0x50, 120, 30) with (18, 5, 4) "Close connection" at +0x5a,
  colour 0x25 hovered, else 1. After 4000 ms (18, 5, 5) "EXIT GAME" at (320, 340), len 0x46, size 3, centred on x, colour
  0x25 hovered, else 1.
- Hit test (only after 4000 ms): the host's buttons (the last hit wins: item = the slot), the exit strip x 40..600, y 330..360
  (item 9). `Mouse_Tipus = DrawOverSelected > 0`. A press closes it: a slot → `MultiKickClient(bae65c[slot], 1)`, 9 →
  `ExitToMainScreen`, the main menu. While the stall lasts it opens again at once (armed 4000 ms later). No sound.
- Mode 2 (a joiner asks to enter a running game, `FUN_005bfa60`): the question (18, 5, 6..8), the joiner's slot and race
  (9, 10), accept / reject strips (0xb, 0xc; reject → `Multi_KickJoiningPlayer` 0x513630, reason 8), exit (0xd).

Ours (`mp_wait`): open after 5000 ms without a local turn while no player's pause holds the session; the cells are
`mp_lobby::Session::late` (the empires the session awaits, dropped in a battle or a migration's survivors, plus on the host
those whose input for the next turn hasn't come, `lockstep::Host::late`; a joiner that knows of none waits for the host); the
title (18, 5, 2) while the session awaits a reconnect; the second line is the empire's race (the exe's per-slot text 0xbae593 is
unknown); "Close connection" is `HostSession::drop_player` (the drop path: the player may come back through the door; the exe's
reason-1 kick only ends the link too); either button held acts, as the exe's level test; the title shows whenever a reconnect is awaited, the overlay open or not, the screens below take no mouse input, the keys stay live (`MOUSE_DrawOver`'s loop runs `Fake_FRISSITES` → `DoKeyboard` 0x4de890). Mode 2 has no counterpart: the
door admits a returning human by name and refuses newcomers to a started game (deviation 296).
Compared against ig2.exe 2026-10-25 (`run_screendiff.py … mpwait`): a real two-instance stall can't be driven, so
`tools/trace/probes/mpwait.js` stages the exe's own state for one (two late players, the host, the reconnect title,
`acttime` 5 s back) in a single-player game and the exe's code draws; the client draws the same with `--mp-wait`. Title,
panel, cells, buttons and exit strip match (mean ≤ 2.3; the exe's OS-drawn pointer aside).

## 10. Modifier keys and the middle button (row 379, fixed 2026-10-25: `ig2-client::window`)

- `MainCallBackEventHandler` (exe 0x4e2db0): a KEYDOWN (0x300) of Shift/Ctrl/Alt sets `KeyShift` 0xbae295 / `KeyCtrl` 0xbae27f / `KeyAlt` 0xbae294 (asm 0x4e3099 / 0x4e30bf / 0x4e30e9) after `remapKeyCodes` 0x4e4670 (LSHIFT/RSHIFT e1/e5 to VK 0x10, LCTRL/RCTRL e0/e4 to 0x11, LALT/RALT e2/e6 to 0x12); no repeat filter; gated by `TutorialActive` 0xf6915d == 0 or `AktKep` 0xbae0fc == 0xd. A KEYUP (0x301) of either key of a pair clears it. Nothing reads `SDL_GetModState` or `keysym.mod`.
- A button event (0x401/0x402) with a button other than 1/3 (middle, X1, X2), and event type 0xa (meaning unknown), jump to block 0x4e345a: with `IsActive` 0x912a38 set it zeroes the three modifier bytes and the held bytes 0xdb467d/e/f (any/left/right), then calls `SetWindowPos(NULL, HWND_TOPMOST, ...)` (hWnd NULL: no effect; HWND_BOTTOM when inactive, and then nothing is cleared). The tutorial gate is inside the button-1/3 branches only, so the middle clear also works in the tutorial.
- `Mouse_Lekerdez` (0x5105d0) then sets `Mouse_Felenged` 0xdb465c and the release edges 0xdb465d/e for every button that was held; the later real release of that button is no edge.
- Readers: Starmap_Main 0x5b2b70, StarmapKeyboardHandler 0x5aeab0, Starmap_MouseInit2 0x5b8950, ColonyKeyboardHandler 0x4628c0, Colony_MouseInit2 0x467990, ground war 0x47b350/0x4a79e0, SpaceWarKeyboardHandler 0x561ef0, FUN_0057c4c0; TutorialCheckEvent 0x60bdf0/0x60be40 zero Shift; FUN_00510930 records/plays them in replays.
- Client: `modifier_key` sets/clears a modifier on its own key's down/up (repeats included, AltGr = Alt, synthetic focus presses skipped, not while `tutorial_owns_input()`); `ModifiersChanged` only clears, so a pressed Ctrl doesn't bring back a dropped Shift; `middle_clears` (pre-dispatch in `window_event`) tracks left/right held over every route and, on any other button with the window focused, drops the modifiers, re-dispatches a Released for each held button and swallows that button's real release. Not run in the live window (the capture path calls `mouse()`/`key()` directly).
