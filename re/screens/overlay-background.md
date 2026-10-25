# The game behind the overlay screens

Research (`AktKep` 4), design (5), diplomacy (6), spy (7), trade (8), info (9), messages (10), options (0xd) and
help (1) don't draw the starmap or the colony underneath. Each lays a screenshot of the screen it came from over the
viewport, dimmed to about half as it opens. The starmap or colony is not drawn again while the screen is up. The game
keeps running underneath (except where noted in §3). From the exe's decompiles (`_local/decomp/screens`, `design`,
`infomsg2`, `spw-setup2`, `topbar`, `fader/Research3DUpdate.c`, `spyscr_exe`) and captures of a running game (§3).
Addresses are exe unless marked ELF. Client status: §4; the "live background" option is ADR 0015.

## 1. The pictures: `SaveScreen(n)` (0x524700, ELF 0x5deb20)
- `SaveScreen(n)` fills the picture slot `Kepek[0x33 + n]` (exe 0x903558 + i·0x124: loaded byte +0x101, GL texture
  +0x114, brush w/h +0x118/+0x11c, resize stamp +0x120). The slots' static names (`_ESIG_U2/U3/U4.BMP`, i = 52–54)
  are files the GOG install doesn't ship, so the slots are only ever screenshots.
- First use: `glGenTextures`, brush 640 × 480, `LefoglalKep` (0x4d3a30: `FRISSITES()`, loaded byte := 1; no file).
  Then `glTexImage2D(RGB, window size)` when the window was resized since (stamp ≠ 0xB77098), and
  `glCopyTexSubImage2D` of the window rect (0xB770C8/CC, 0xB77090/94): **the back buffer as the GFX call just before
  left it**, before any swap. RGB, no alpha.
- A slot stays loaded for the session: a later screen draws the last picture taken there, however old.

Who takes which picture (the screen being left runs its `GFX_X` first, with `KilepesElott` 0xBAE296 = 1 hiding
the top bar (`GFX_AllandoCallBack1/2`, `GFX_DateMoneyScrollFade`) and the starmap/colony panels (`StarmapGfx2D_*`)):

| picture | taken by | when, of what |
|---|---|---|
| 0x34 (`SaveScreen(1)`) | `Starmap_Kilep` 0x5b2200 (call 0x5b28ed) | every exit except to the battle (0xb): `GFX_Starmap` with `KilepesElott` = 1, **the starmap without its HUD** (the scene, its radar zones and fleets; no names, top bar or panels) |
| 0x35 (`SaveScreen(2)`) | `Starmap_Kilep` (call 0x5b28c5) | to options, messages or help, before the above: `KilepesElott` = 0, **the starmap with its HUD** |
| 0x35 | `Colony_Kilep` 0x466390 (0x4663db) | to 10, 1, 0xd: `GFX_Colony` with `KilepesElott` = 1 (no HUD) |
| 0x35 | `Research_Kilep` 0x5239b0, `Design_Kilep` 0x4b1f20, `Diplomacy_Kilep` 0x4bf810, `Spy_Kilep` 0x58d250, `Trade_Kilep` 0x60a630, `Info_Kilep` 0x4e7fa0 | (to 10, 1, 0xd; research, diplomacy, spy, info on every exit [the gates: check in the asm]): their own `GFX_X`, the screen as it stood |
| 0x35 | `SpaceWar_Kilep` 0x562b50 (0x562bf0) | to options only: `GFX_Starmap` with `KilepesElott` = 0 |
| 0x35 | `Messages_Belep` 0x507f90 (0x508052) | when the screen before (0xBAE19C) is not the starmap or help: the back buffer as it is |
| 0x36 (`SaveScreen(3)`) | `Messages3DUpdate` ELF 0x5a3ad0 | at `mesframe` 30 (`MessagesKellKepMentes`): the messages screen itself, then drawn instead of its model (messages.md §4) |

`IG2_gameUpdate` (0x4e0440) runs a switch as `ScreenFelszabadit(old)` → `<old>_Kilep` (the pictures) →
`ScreenBetolt(new)` → `<new>_Belep` → its mouse init → `_Main`. `Starmap_Kilep` returns before taking any picture
while a zoom transition runs (`DAT_00b9a304`) or with `DAT_00f16eef` set.

## 2. Drawing: `Copy3hatterkep(n, colour)` (0x4cb690, ELF 0x534ac0)
Nothing when `n = 0` or the slot's loaded byte is clear; else the picture 1:1 over the viewport (widescreen.md §3)
through `DDBrushToScreen3d`, alpha-blended, colour `0xFFgggggg` (a grey multiplied into the picture's bytes in gamma
space). Each screen's 3D update calls it **first**, before `Update3D` and its 2D passes: the picture is the bottom
layer, under the screen's own meshes.

| screen | picture | grey `255 − c` |
|---|---|---|
| research (`Research3DUpdate` ELF 0x5e4e10) | 0x34 | `c = (int)(resframe/60·128)` |
| design (0x4ad8e0) | 0x34 | `c = (int)(desframe/60·128)` |
| diplomacy (0x4be2d0) | 0x34 | `c = (int)(dipframe/80·128)` |
| spy (`Spying3DUpdate` 0x590410) | 0x34 | `c = min(128, (int)(spyframe/50·128))` |
| trade (`Trade3DUpdate` 0x608340) | 0x34 | `c = min(128, (int)(traframe/50·128))` |
| info (0x4eb020) | 0x34 | `c = (int)(infoframe/45·128)` (the star log's frames past 45 dim it further) |
| messages (ELF 0x5a3ad0) | 0x35, then 0x36 | `c = (int)(195·min(mesframe, 15)/15)`; 0x36 at `0xffffffff` |
| options (`GFX_Options` 0x516f30) | 0x35 | `c = min(128, (int)(F/100·128))`, `F` = `OptionsFelfadeles` (`KilepesValtozo` while leaving) |
| help (`GFX_Help` ELF 0x54f160) | 0x35 | `c = min(128, (int)(helpframe/100·128))` |

`UpdateFrame` (0x4e3e90) draws only the active screen's `GFX_X`: no starmap or colony behind the overlays.

Consequences of the slot rules (original behaviour, kept in classic mode):
- From the colony, research and the other 0x34 screens draw the **last starmap exit's** picture (stale), or nothing
  (black) if the starmap was never left this session.
- Options, messages and help opened over research and the like draw that screen as it was (§3: options and
  messages over the info screen).
- A slot whose loaded byte is set but whose texture is missing draws with the last bound texture: the "circuit"
  side strips of widescreen.md §7.2 (deviations.md 4).

## 3. The game keeps running
`Fake_FRISSITES` (0x4de720) runs the 20 Hz sim ticks (`while now − DAT_bae274 > 50`) whatever `AktKep` is: no gate
on screens 4–10 or 0xd. The ticks need `IdoPause` (0xBAE264) clear and `IdoPauseMertAtlepesAlattVan` (0xBAE265)
clear, unless multiplayer (`MultiPlayerMode` 0xBAE2C0):
- research, design, diplomacy, spy and trade set 0xBAE265 while they animate open or closed, so the game stops for
  the animation and runs again once they're open; info sets it while opening (then `DAT_0090b704`);
- messages and options pause the clock (`SetIdoPause(1, 0)` in `Messages_Belep` / `Options_Belep`, the old value
  back in their `_Kilep`); help keeps 0xBAE265 set while it opened animated.

So the original freezes the **picture**, not the game: the date runs on behind research, info and the rest [obs:
`run_screen.py quick-1.sav --kep 2 --run --script scripts/overlay-bg.json`, 1920 × 1080: the info screen over the
dimmed starmap without names, options and messages over the dimmed info screen (Esc and F10 from it), the date
2327-10-24 → 10-26 behind the info screen; F4 is gated on that save, research came up through `AktKepKell`].

A bare `run_screen.py --kep N` capture goes from the main menu (`AktKep` 15 after `LoadSave`) straight to the screen:
no `Starmap_Kilep`, no picture, a black background. `--via-starmap F` opens the starmap first and the screen F frames
later, the original's own path.

## 4. Client status
`ig2-client::backdrop` (`Backdrop`, `window.rs` `App::backdrop_switch`, `Frame::backdrop` in render.rs): the two
pictures (0x34, 0x35) as render targets taken per §1's table when the shown `AktKep` changes, drawn under everything in
gamma space with §2's grey. **Classic** (`--background classic`, settings `LiveBackground` 0): the pictures as taken.
**Live** (default, ADR 0015): a picture of the starmap or colony is replaced by that scene drawn anew every frame (no
HUD); a picture of another overlay screen stays; the sim's rules (§3) don't change. Cost (`IG2_PROFILE`, release,
vsync off, 800 × 600, `quick-2.sav`, medians): live mode's `backdrop` phase 0.6–0.7 ms a frame (worst ≈ 2 ms); worst
frames classic → live: info 2.4 → 5.4 ms, research 3.4 → 4.2, design 2.8 → 3.5, trade 2.5 → 3.6 (the 3D pass keeps
a depth buffer per target size, 4 at most). Capture: `--snapshot --backdrop` draws the save's starmap as the picture,
`run_screendiff.py … <screen> --via-starmap 120` passes it on. Without `--backdrop` the snapshot is black,
as the bare captures.
