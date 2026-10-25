# The tutorial overlay

The interpreter itself is `re/rules/tutorial.md` (`ig2_sim::tutorial`); this file is the overlay the original draws
and the input and events around it. Client status: §5.

Sources: exe decompiles in `_local/decomp/tutorial-overlay/` (`0x611ba0`, `0x612740`, `0x4ab250` `FillToScreen`,
`0x4a9e60` `KiirStr`, `0x52cd10` `Update3D`, `0x4e2db0` `MainCallBackEventHandler`, `0x4e3e90` `UpdateFrame`,
`0x4e3660` `ReadNext`, `0x5105d0` `Mouse_Lekerdez`, `0x60be40` `TutorialCheckEvents`; the call sites in `precall.txt`).

## 1. Driver (per 10 ms UI tick, `Fake_FRISSITES` exe 0x4de720)

`DoKeyboard`, `Mouse_Tutorial_Kezel` (exe 0x510930: the sample index steps, the recorded pointer and buttons go into
`Mouse_Akt_*`), the screens' `*2` updates (which raise events), `TutorialFrissit` (exe 0x60c020), `TutorialCounter++`
unless paused. The game state the screens read: `TutorialActive` 0xf6915d, `TutorialPaused` 0xf66f2a, `TutorialEvent`
0xf66f40. Effects: in-place game writes; `CENTER*`/`FIND*` select and centre (`StarmapKozepreAllit`);
`GAMESPEED`/`PAUSE`; `RETURNTOMAINMENU`/`QUIT` leave (`ExitToMainScreen` stops it); `END` the message line. Started by
the main menu's tutorial ring (`TutorialStart("main", n)`, the script `dataraw/tutorial/main/script/main.tut`, the
language's copy). The install ships no `mfx/tutorial/` speech: `PLAYWAV` finds a length of 0 and nothing plays
(typing at 15 chars/s).

## 2. The window (`TutorialKeretFrissit` exe 0x611ba0, `TutorialSzovegFrissit` exe 0x612740)

* **Box** (a real function in the exe, not inlined: called from `GFX_DateMoneyScrollFade` exe 0x554500 with
  `FaderHanyadikMenet = 3`, after the screen's 2D passes and the top bar): when `AktKep != 0xd && !DisableTextWindow &&
  TextShown && (Active || Paused)`: `FadeKerettelKerekit((int)(TKActX − 5.0), (int)(TKActY − 5.0), w, CharY·8 + 0x22,
  (0, 0x36, 0x5a) at 0x1e, (0, 0x5a, 0x82) at 0)`, `w = CharX·0x28 + 10`, clamped to `0x27f − x` when `0x27f < x + w`.
  CharX/CharY = `CharXMeret[2]`/`CharYMeret[2]` (6, 7): 250 × 90.
* **Text** (`Update3D` exe 0x52cd10 draws it after the scene, so over the box and every pass): nothing on `AktKep` 0xd;
  with neither Active nor Paused only the recorder's debug line. Else, with the window on: every cell of `TKTomb` whose
  char isn't a space: `KiirStr(cell, (int)(TKActX + (float)(c·CharX)), (int)(TKActY + (float)(r·(CharY + 3))), 1, 2, 4,
  colour)` (int products, then f32 adds, truncated). Colour byte (signed) < 0x40: that palette index; else index − 0x40
  with `KiirStrSzinSzorzo = TutorialSzinVillogtato` (30..100, ±1 a tick), reset to 100 after the cell (`KiirStr`: each
  channel `·m/100`, integer).
* **Cursor**: `FillToScreen(x of SorPtr on row 7, CharX, CharY, colour 8, false)` when `(QLen == 0 ? AktIdo200 :
  TKTomb[7][SorPtr − 1] != ' ') && !Paused`; `AktIdo200` = `(AktIdo / 200)` even. Opaque, on whole pixels: corner
  rounded up, size truncated [obs: 7 × 8 pixels at (554, 363) for the cell (443, 290) at 800 × 600].

## 3. Input while the script plays

* `MainCallBackEventHandler` (exe 0x4e2db0), G = `TutorialActive && AktKep != 0xd` (Active only, not Paused): mouse
  motion (the position isn't stored), buttons 1/3 and the Shift/Ctrl/Alt flags are dropped. With `Active || Paused`: `t`
  toggles `TextShown`, `o` is `ESC_Pressed` (the options screen), `j` stops the tutorial only with DEBUGLEHET
  (0xbae659, off in the shipped game). Other keys go into the 20-slot ring; `ReadNext` (exe 0x4e3660; the spec's
  0x57f9b0 is the ELF's) returns only the recorded keys under G and leaves the ring alone, which `ClearKeyboardBuffer`
  empties on unpause; Esc keeps its live path (not tutorial-gated). `MainCallBackEventHandler` handles no
  `SDL_MOUSEWHEEL` (0x403) at all (the classic game ignores the wheel). The modifiers come from the recording (Alt 1,
  Shift 2, Ctrl 4). When the replay stops (paused, options), a recorded button still held is let go (`Mouse_Lekerdez`
  reads the live buttons again).
* Recorded keys: `.mou` slots `(kind, code)`: 0 a character, 1 a key down, 2 up; Windows key codes.
* **Pointer** (`UpdateFrame` exe 0x4e3e90, last): `MOUSE.BMP` cell `Mouse_Tipus` at `Mouse_x/y` minus the hot point when
  `Mouse_Tipus > −1`. While a sample plays it is the normal pointer at the recorded spot (its shape from the hot-spot
  under it); parked (no sample: `Mouse_x/y` = (0x140, 200), `Mouse_Tipus = −2`) nothing is drawn [obs: no pointer in
  the captures]. `Mouse_Move` (exe 0x510790) only stores the position; `UpdateFrame` takes `Mouse_Tipus` from the
  hot-spot under it when it is −1.

## 4. Events the screens raise (`TutorialCheckEvent` exe 0x60bdf0)

From the screens: 1 (the planet sighting, `FRISSIT_Starmap`), 0x27 (`Trade_Main`'s purchase), 0x22 (the countdown),
2/0x19/0x1e (`TutorialCheckEvents`' polling while paused), and from the screen id's change: 0x1a (`Starmap_Belep`
0x5b1cc3), 7 (`Colony_Belep` 0x465cf0), 0x28 (`Diplomacy_Belep` 0x4bf7f0); entering the battle screen ends
`WAITFORBATTLE` (`SpaceWar_Belep`). Further sites (exe address, id): `ColonyKeyboardHandler` 0x462caa/0x462df6 0x1b
(PgUp/PgDn); `Colony_Main` 0x46745b/0x467482/0x467586 0x24 (tax), 0x4675ea 0x25 (auto-build); `Design_Main` 0x4b2304
0xb (every frame of the flight), 0x4b29e0 0xc; 0x4b4a14 0xb; `TervAutoDesign` 0x541a7f 0xb; `FeldolgozValasz`
0x4c16ff 0xd or 0x2a; `DoKeyboard` 0x4db3eb 0x1c ('r'), 0x4db9a7 4, 0x4db9c0 3 (the renames' Enter);
`FRISSIT_Research` 0x51e200 9, 0x51e219 0xa; `TalalmanyKivalasztva` 0x524fdb 8 (research pick); `Spy_Main` 0x58d4e3
0xf, 0x58d4f7 0x10; `FRISSIT_Spy` 0x5862b5 0xa; `MOUSE_SelectScreens` 0x55555f 0x1c; `SpaceWar_Kilep` 0x562b68 0x20;
`Starmap_Main` 0x5b3035 0x16 (leaving the planet info), 0x5b39c2 6 (production pick of a kind-5 design),
0x5b4ca6/0x5b4d11 0x11 then 0x1f (attack), 0x5b5b51/0x5b6533 0x1c (into a non-main colony; the main planet stays
shut), 0x5b5b9a/0x5b5be0 0x2b (satellites); `TransferOff` 0x5bbcda 5 (`Tutorial_Split_Ok`). In the sim:
`GroundWarFinished` (exe 0x47c5d0, the names file's "KiszamolErtek") 0x21 when EN wins (simulated or not);
`EpuletNew` 0x53c8eb by building type on an EN planet (2→0x13, 7→0x1d, 0x34→0x14, 0x13→0x17, 0x15→0x18, 0xe→0x23,
0x32→0x26, 4→0x29); `FRISSIT_Starmap` 0x596964 0x1c; `MULTIGET_Starmap` 0x59dd1f 0x15 (own fleet, order ≠ 0, farther
than `RadarHatotav[radar level]` from the last own planet); `FRISSIT_Research`/`FRISSIT_Spy` 9/0xa (EN completes, or
any thief steals, tech 0x12/6).

## 5. Client status
`ig2-client::tutorial_ui` (`TutorialUi`) and `window.rs` (`App::tutorial_*`): the driver (`replay_step` feeds the
recorded pointer, button edges and keys to the screens; `pull`, `tick`, `push` to the game's copies), the effects via
`Game::tutorial_effect`, the window, text and cursor (§2), §3's input drops and `t`/`o`/Esc, the pointer through
`window/pointer.rs`, Options' tutorial layout. Every §4 site is raised: the sim's in the sim (`EpuletNew` when the
colony world is stored), the screens' from the commands they send (`World::send`); approximations: 0x11/0x1f fire only
when an order is really sent, the 0x24/0x25/6/0x2b/0xf/0x10 sites from the command, design 0xb once on the pick.
Missing: the tutorial input locks (rows 141, 174, 198). Verified: `run_screen.py <install> quick-1.sav <out> --kep 2
--script scripts/tutorial.json --probe probes/tutorial.js --tutorial 1 --state-out S` logs the exe's window per tick
(`<out>-tutwin.jsonl`); the client draws the logged state (`--snapshot … --tutorial-window <record>`) and the window
region matches; `tutorial_ui::tests::chapter_1_window_matches_the_exe` reproduces chapter 1's window exactly (the save
load's transition holds the first `WAITFORATLEPES` to t 102). Not compared: the replayed pointer and clicks, a whole
chapter played through.
