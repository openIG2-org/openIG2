# ADR 0030: W/A/S/D pan the camera

Date: 2026-10-31. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
keys kept as an option. Spec: `re/screens/starmap.md` §10.6, `re/screens/colony.md` §11.2 and §11.9,
`re/screens/input.md` §1. Implemented by `ig2-client::starmap::pan_target`, `ig2-client::colony::CameraKeys` and
`ig2-client::window` (`wasd_key`).

## Context
The original pans the starmap and the colony view only with the pointer at a window edge (`Starmap_Autoscroll` ELF
0x6fe890, `Colony_Autoscroll` exe 0x465940); its arrows turn and tilt the camera (`StarmapKeyboardHandler` exe
0x5aeab0, `ColonyKeyboardHandler` exe 0x4628c0). Players used to strategy games expect W/A/S/D to move the view.

The letters reach the original's handlers only as characters (`remapKeyCodes` exe 0x4e4670 queues no key down for
a letter, input.md §1). What they do today:
- Every screen (`DoKeyboard` exe 0x4db1a0): none of W, A, S, D (it takes `o`, `p`, `m`, `v`, `` ` ``).
- Starmap: none (the handler takes `c`, `=`, `-`; `l` only with the debug switch).
- Colony screen: `a` (lower case) selects all own units (at war or not), `s` and `S` stop the selected units; `A`,
  `d` and a second `s` only with the debug switch (`DEBUGX`, `ColonyKeyboardHandler` lines 370, 854, 898).
- Text entries (planet and fleet rename, MP chat, save name, design name, console) type them.

## Decision
1. **Option "WASD camera"** (Remake row on the Options screen's game page, `WasdCamera` in the remake's settings
   file): Off (0), Starmap (1, the default), Both (2: the starmap and the colony screen).
2. A held key aims the edge scroll as the pointer at that edge does: A left, D right, W up, S down (by their place
   on the keyboard, so Z/Q/S/D on AZERTY). Same target speed 30, the same ramp of 5 per 10 ms UI tick and the same
   slide, so the speed matches the edge scroll at any frame rate; the arrows keep turning and tilting.
3. Not while a text entry has focus (the letters type), not under an overlay screen, the options or a space battle;
   the edge scroll's own gates (long info, zoom, a playing tutorial) apply too. A key up ends the pan whatever screen
   has the keyboard; a pan held when an entry opens goes on until its key is released.
4. **Default Starmap only**: on the colony screen the keys would take the original's `a` (select all) and `s`
   (stop), which a ground war uses (and the debug switch's `A`, `d`, `s`); players who want W/A/S/D there choose Both and lose those two keys
   (the stop and select-all actions stay reachable with the mouse).

## Consequences
- Recorded replays and capture comparisons are unaffected: the exe never queues a letter's key down, and the option
  is read by the window only.
