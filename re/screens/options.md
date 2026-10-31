# The Options screen (`AktKep` 13)

From the ELF decompiles (`_local/decomp/options/`) and the exe decompiles (`_local/decomp/options-exe/`); the exe wins.
Addresses are exe unless marked ELF. `[elf]` = read in the ELF only; `[guess]` = inferred; `[asm]` = checked in the exe
disassembly; `[obs]` = observed at runtime (section 13). Coordinates are 640 × 480 virtual pixels. "Tick" = one 10 ms
UI tick (`Frissit2_Options`). **T** = the tutorial layout: `(TutorialActive 0xf6915d || TutorialPaused 0xf66f2a) &&
!SelfPlaying 0xbae267`; with T several hot-spot ids shift by `s = 1`. Strings are `GetString(13, section, n)` =
`txt13` (the install's text file 13). Client status: section 14.

## 0. Functions

| Part | ELF | exe |
|---|---|---|
| `Options_Belep` | 0x5d9830 | 0x519790 |
| `Options_Kilep` | 0x5daa80 | 0x51a100 |
| `Options_Main` (per frame, input) | 0x5dad20 | 0x51a320 |
| `Options_MouseInit` (once) / `Options_MouseInit2` (per frame, from `UpdateFrame` 0x4e3e90) | 0x5db920 / 0x5dbf60 | 0x51ace0 / 0x51a8a0 |
| `Options_EnterPressed` / `Options_EscPressed` (text entry) | 0x5dc740 / 0x5dc7d0 | 0x51a090 / 0x51a0f0 (`IOMode = 0; jmp OptionsRefreshSlots`) [asm] |
| `GFX_Options` (per drawn frame: background) | 0x5d6f30 | not located |
| `OptionsGfx2D_1` (faders, lasers) / `_2` (texts, slots, sliders) / `_3` (empty) | 0x5d49a0 / 0x5d71c0 / 0x5d95b0 | – / 0x518a30 / – |
| `Frissit2_Options` (tick) | 0x5da990 | not located |
| `OptionsRefreshSlots` | 0x5d95c0 | 0x5196c0 |
| `Options3DInit` | 0x5d96a0 | 0x517460 |
| `OptionsInitNullBoxes` / `OptionsRemoveNullBoxes` | 0x5d7100 / 0x5d7070 | 0x519640 / 0x519700 |
| `OptionsCsuszkakKiBe` (slider visibility) | 0x5d48c0 | inlined |
| `CheckOptionsDependencies` | 0x5d2b20 | 0x516e10 |
| `KirakMenuPontok` (option rows) | 0x5d4040 | 0x5171f0 (**ported name `WaitForFlag` is wrong**) |
| `OptionsVonal` (line vertex) | 0x5d4690 | 0x4e7860 |
| `Options_LoadStrings` | 0x5d26a0 | 0x51a2c0 |
| `SaveGlobalRegistrySettings` / `SaveOptions` / `setRegKey` / `save_options_new` | 0x5d3e40 / 0x5d3490 / 0x5d1f00 / 0x5d2320 | 0x51b4e0 / 0x51b640 / 0x51c6b0 / 0x51c4d0 |
| `LoadSave(slot, name, mode)` / `LoadTitle` / `GetSavePath` | 0x628180 / 0x628dc0 / 0x624d80 | 0x543c80 / 0x553590 / 0x5579c0 |
| `ExitToMainScreen` | 0x5bc0b0 | 0x5117f0 |
| `SetGlobalMusicVolume` / `SetGlobalSoundVolume` | 0x670940 / 0x670a50 | 0x55d0f0 / 0x559ad0 (**ported name `WaitForDrawing` is wrong**) [guess] |
| `GetWarfareStatus` (ELF name) | 0x4a4f10 | 0x47c150 (**ported name `help_new_available` is wrong**) |

## 1. State (exe addresses)

| Name | exe | Meaning |
|---|---|---|
| `OptionsMelyikMod` | 0x91609c | Page: 0 sound (music, sound sliders), 1 graphics (gamma + page-1 rows), 2 game (page-2 rows). Saved. |
| `OptCsuszkak` | 0xdba258 | 3 sliders × 0x2a0 (section 2). |
| `SaveAktivSlot` | 0x916058 | Selected slot 1..24 (default 1). Saved as `Save slot`. |
| `SaveLeiro` | 0xdb9768 | Slot titles, `[slot] × 0x34` (entry 0 unused; section 5.3). |
| `OptionsCsakLoadLehet` | 0xbae24e | Load only (from the main menu's load ring). |
| `OptionsHarcFolyik` | 0xbae24f | A battle is running (no save without `DEBUGX` 0xbae587). Set by `SpaceWar_Kilep` 0x562b50 towards Options (0x562c0f, also gated by `PTR_DAT_0091a830[0x1fb8]` = the battle's leaving flag, set by `Starmapvisszaallit` 0x570c10, so only after the zoom-out: spacewar.md §4.1.2) and by every `Colony_Kilep` 0x466390 as `WF::GetStatus(Warfare[VisiblePlanet−1]) == −1` (0x466631, the planet's ground war runs; `GetWarfareStatus` inlined). Belep (0x51a074) clears it unless `OptionsElottiKep` is 3 or 0xb; Kilep (0x51a2aa) clears it. Readers: Options_MouseInit2 (id 2), the button pass 0x517580, `OptionsGfx2D_2`'s Save label (0x518da7), `DoKeyboard`'s F-key gate, the colony handlers. |
| `OptionsAreYouSureToQuitMode` | 0xdb975a | The quit question is up (Save → YES, Quit → NO). |
| `OptionsElottiKep` | 0xbae2b4 | The screen to go back to. |
| `OptionsFelfadeles` (f32) | 0xdb9c84 | Fade-in 0..100. |
| `OptionsKilepes` / `OptionsKilepesValtozo` (int) / `OptionsVoltNulla` | 0xdb9763 / 0xdbaa44 / 0xdbaa40 | Leaving, its fade-out, frames at 0. |
| `OptionsVoltElsoframe` | 0xdb9762 | Set by the first `Gfx2D_2`. |
| `OptionsOldIdoPause` / `OptionsUtanPauseModbanInduljon` | 0xdb9758 / 0xdb9759 | The pause to restore / pause after leaving. |
| `OptionsBlokk1Atlatszosag` / `Blokk2` (f32) | 0xdbaa38 / 0xdbaa3c | Text opacity of the slot block / the left panel (`KiirStrSzinSzorzo` 0x903510). |
| `OptionsGraphicsDetail` | 0xdc469c | Saved as `OptionsGfxDetail`; 3 (custom) after a row click. |
| `GlobalMusicVolume` / `GlobalSoundVolume` / `GlobalGamma` / `GlobalGraphicDetail` | 0x919928 / 0x91992c / 0x903518 / 0x90af64 | Settings (default 100, 100, –, –). |
| `enablegammacorrection` | 0xde82e9 | The gamma slider is enabled. |
| `IOString` / `IOMode` / `IOAzonosito` / `IOMaxLen` / `IOX`,`IOY` / `IOCharSize`,`IOColor` / `IOKurzor` / `IOGombnyomasraTorles` | 0xbae2c8 / 0xbae2c3 / 0xbae2c4 / 0xbae2cc / 0xbae398,0xbae39c / 0xbae3a0,0xbae3a4 / 0xbae3a8 / 0xbae3ac | The text entry (save name). |
| name buffer | 0xdc46cc | `char[0x21]`, the save name. |
| `MultiPlayerMode` | 0xbae2c0 | No load, no save. |
| `STARTSCREEN` | 0x927a88 | 2 = starmap (where a load goes). |
| option table | 0x9160a0 | 17 rows × 0x22c (section 6). |

## 2. Sliders (`OptCsuszkak`)

Record 0x2a0: `+0 x`, `+4 y`, `+8 width` (220), `+0xc value` 0..100, `+0x10 + 100k` six level strings, `+0x268` title,
`+0x29a` continuous, `+0x29b` visible, `+0x29c` enabled.

| # | x, y | Value | Level strings | Title |
|---|---|---|---|---|
| 0 music | 30, 110 | `GlobalMusicVolume` | (13,4,1..6) | (13,4,7) |
| 1 sound | 30, 150 | `GlobalSoundVolume` | (13,4,1..6) | (13,4,8) |
| 2 gamma | 30, 110 | `GlobalGamma` | (13,4,9), (13,4,2..6) | (13,4,10) |

- Each level string is `"- "` (exe 0x8574f4) + the text; the shown level is `value * 6 / 101`-style banding `[guess]`
  — see the port's `level()` and section 13 for the observed bands.
- Belep sets `+0x29a = 1` on all three, so `Options_Main`'s `value / 33 * 33` quantisation (`imul 0x3e0f83e1`,
  `sar 3`) never runs [asm 0x51a725].
- Visibility (`OptionsCsuszkakKiBe`, Belep and the head of `Gfx2D_1`): all hidden, enabled; page 0 shows 0 and 1;
  page 1 shows 2 with enabled = `enablegammacorrection`; page 2 none.
- Every `Gfx2D_2` copies the visible sliders' values to the globals (gamma only when enabled) and calls
  `SetGlobalMusicVolume` and the sound setter: the volume changes live.
- A disabled slider: its title in colour 1, then (13,4,11) at `x + CharXMeret[2]·len(title) + 10`, colour 0x25.
- **Drag** [asm 0x51a641..0x51a76d]: ids 5..7, rect `(x−10, y−5, 230, 30)`. On the press (`Mouse_Lenyom`) over it:
  `Mouse_Megfogva` (0xdb466a) = 1, `CsuszXStart` (0xdb9c7c) = `Mouse_x`, `StartBarPos` (0xdb9c80) = value. While held
  and still over its hot-spot: `value = trunc((f64)((f32)(x − start) / (f32)width) · 100.0 + (f64)StartBarPos)`
  (`divss`, `cvtps2pd`, `mulsd [0x852428]`, `addsd`, `cvttsd2si`), clamped 0..100. The release (0xdb465c) clears the hold.
  The drag is relative to where it was grabbed.

## 3. Enter, leave, timers

**Belep** (0x519790), in order: if `OldKep == 0xb` (from a space battle) `RemoveFader`, `GFX_AllandoRemove`;
`Mouse_FelengedesMajdMegvar = 1`; `PauseTutorialMp2` if a tutorial runs; `InitFaderDesignStruktura(optf)`;
`OptionsElottiKep = OldKep` unless `OldKep == 0xd` (`HelpElottiScreen` when it was 1, `MessagesElottiScreen` when 10);
clear the leave state; fill the sliders; `OptionsOldIdoPause = IdoPause & 1`, `SetIdoPause(1, 0)` (**the game is paused
while the screen is up**); `OptionsRemoveNullBoxes`; `OptionsRefreshSlots`; `Options3DInit`; `OptionsInitNullBoxes`;
`OptionsFelfadeles = 0`, `VoltElsoframe = 0`; slider visibility; `AreYouSure = 0`; `HarcFolyik` rule above;
`CheckOptionsDependencies`. No sound, no music change.

- `Options3DInit` (0x517460): unless load only, `FreeMainXVB`/`CreateMainXVB`/`InitHatterFader`, the camera
  (f = (30, 25, −210), `zbm` 0.1, centre (320, 250)); always `TextureDir = "textures/design/screen/"`,
  `fadertexture = dt.bmp`.
- `GFX_Options` [elf]: load only (or `Options_Kell3DHatter`): `Mainscreen_Update3D()` — **the main menu's scene runs on
  behind**. Else the frozen game picture `Copy3hatterkep(0x35, grey c)` with `c = 0xff − min(0x80, (int)(F/100·128))`,
  `F` = `OptionsFelfadeles` (or `KilepesValtozo` while leaving): it dims to half as the screen fades in, then `Update3D`. (The picture is black only in bare-jump captures; see [`overlay-background.md`](overlay-background.md).)
- `Frissit2_Options` [elf], per tick: `FaderDesignFrissito(optf)`; once `VoltElsoframe`, `OptionsFelfadeles += 0.6`
  up to 100 (≈ 167 ticks); while leaving, `KilepesValtozo −= 2` down to 0.
- **Kilep** (0x51a100): `OptionsKilepes = 1`, `KilepesValtozo = (int)OptionsFelfadeles`; `SaveGlobalRegistrySettings`
  (section 7); then a blocking loop of `UpdateFrame` + `FRISSITES` with `AktKep` held at 0xd until the fade-out is 0 for
  2 frames (the fade-out plays before the next screen). Back to the main menu (`AktKep` 0xf): `ReleaseTutorialMp2`,
  `TutorialStop(1)`, `ExitToMainScreen` (closes a multiplayer session). Else `UnPauseTutorialMp2`. Then
  `SetIdoPause(OptionsOldIdoPause)` (and pause if `UtanPauseModbanInduljon`), clear `HarcFolyik`.

### 3.1 The screen before (`IG2_gameUpdate` exe 0x4e0440: the old screen's `_Kilep`, then the new one's `_Belep`)
Every game screen's `_Kilep` / `_Belep` special-cases `AktKep` / `OldKep` ∈ {1, 10, 0xd}:

| Screen | `_Kilep` towards Options | `_Belep` back (`OldKep` 0xd) |
|---|---|---|
| Starmap 2 (0x5b2200 / 0x5b19a0) | no animation or sound; a long info zooms out first (blocking); a transfer, fleet or production panel cancels the change | full re-init (faders `fifo`/`starf`, modes off), no opening; the camera kept; `PlayMusicMain` keeps a main track playing |
| Colony 3 (0x466390 / 0x465b40) | no animation; everything freed, the camera written to `Warfare[p]` | full rebuild from `Warfare[p]`'s camera; the planet's ambience sound again (`random`) |
| Research 4, Diplomacy 6, Info 9 (0x5239b0 / 0x523540, 0x4bf810 / 0x4bf530, 0x4e7fa0 / 0x4e7d30) | no closing animation or sound (`SaveScreen(2)`) | open at once, no sound; the selection and menus reset (Info: `InfoSelected = EN`, the chart counters `rand() % 400 + 200`) |
| Design 5 (0x4b1f20 / 0x4b1be0) | no closing animation or sound | open at once; keeps `des_xy`, the spin, wireframe and `AktAlap` (`OptionsElottiKep == 5`) |
| Spy 7 (0x58d250 / 0x58cd50) | no closing animation or sound | `OldKep` not read: the opening animation and sound again |
| Trade 8 (0x60a630 / 0x60a1e0) | no closing animation or sound | open at once, the selection reset |
| Messages 10 (0x5083d0 / 0x507f90), Help 1 (0x4d7440 / 0x4d7060) | the closing animation plays (no `AktKep` test) | Options goes back to the screen before them (`MessagesElottiScreen`, `HelpElottiScreen`) |
| SpaceWar 0xb (0x562b50) | no exit, `OptionsHarcFolyik` | back via the starmap (`starmaphovaleptovabb = 0xb`, the battle view restored) |

O on Help runs `Help_Kilep`'s closing (0x4d7440) first; from Messages, `Messages_Kilep` (0x5083d0) closes first.
The design-return flags (`resMessagesUtaniLegelsoBejoves` exe 0xdc471c, set at 0x523810; `infoMessagesUtaniLegelsoBejoves`
0xb8525c, set at 0x4e7ed2) are set on `(MessagesElottiScreen == 5 && OldKep == 10) || (OldKep == 1 && HelpElottiScreen == 5) ||
(OldKep == 0xd && OptionsElottiKep == 5)`. Neither has a reader (only `desMessagesUtaniLegelsoBejoves`, Design's own, is
read); set, research skips resetting `reswireframe` (0xdc537c) and `res_xy` (0xdc5378, 2.0), so they go on from the last
visit; info has no such branch. The DEBUGX reads (0x51a927, 0x518d9c, 0x51790b) and `!SelfPlaying` (0x51a8a0) can't
change while the modal Options is up.

### 3.2 Multiplayer (`MultiPlayerMode` 0xbae2c0)
Read in Options only by `CheckOptionsDependencies` 0x516e10 (rows without the MP flag off), `Options_MouseInit2`
0x51a8a0 (ids 1 and 2 off, 0x51a8fd / 0x51a932), the button pass 0x517580 (no Load button, 0x5175d9; the Save
button only with `AreYouSure`, 0x51790b), `OptionsGfx2D_2` (no "Load" label 0x518cc6, no "Save" label 0x518da7) and
`ExitToMainScreen` 0x5117f0 (YES on the quit question: `ig2server_sendinfo("nam:,stat:0")`, then
`MultiCloseClient(1)` for a joiner or the host's teardown, `MultiPlayerMode = 0`). Belep, Main, Kilep and the slot
code have none; Belep's `SetIdoPause(1, 0)` is ignored in MP (`SetIdoPause` 0x541272).

## 4. Hot-spots

`Mouse_Szoveg_Set(id, 0, x, y, w, h, tooltip, wait_release, 1)`; `AktIkon = id + 0x1e`; tooltips (13,1,n).
**Every action fires on the press** (`Options_Main` tests `AktIkon2` with `Mouse_Lenyom`, or hover alone): the flag only
makes `IG2_gameUpdate` (ELF 0x587400) wait for the button to come up afterwards (`Mouse_FelengedesMajdMegvar`).

| id | Rect | Tooltip | Waits for release | Active |
|---|---|---|---|---|
| 1 Load | 366,62,78,37 | 1 | yes | slot filled, not T, not AreYouSure, not MP |
| 2 Save / YES | 452,62,78,37 | 2 | no | AreYouSure, or `!LoadOnly && !(HarcFolyik && !DEBUGX) && !MP && slot ≤ 22 && !T` |
| 3 Quit / BACK / NO | 541,62,78,37 (T: 441,62,178,37) | 3; load only 13; T "" | no | always (T: plays the click, goes to the main menu without the question: `Options_Kilep` 0x51a100 `TutorialStop(1)` + `ExitToMainScreen`) |
| 9 (T) continue tutorial | 441,102,178,37 | "" | yes | T |
| 4 slot list | 358,113,269,336 | 4 | no | `!IOMode && !T` |
| 10+s tab sound | 13,66,90,20 | 5 | yes | always |
| 11+s tab game | 108,66,90,20 | 7 | yes | always |
| 5..7 sliders | (x−10, y−5, 230, 30) | the slider's title | no | set each `Gfx2D_2`; off when hidden |
| 17+s left panel | 9,62,288,384 | "" | no | always (swallows clicks) |
| 18+s back | 0,0,345,479 | 12 | yes | always |

`MouseSzovegSzam = 0x12 + s`. The exe's `MouseInit2` also turns Load and Save off in multiplayer. `Mouse_Darabol`
(ELF 0x5b7a40) runs from the highest id down and keeps the last hit, so **the lowest id under the pointer wins** (the
panel 17 covers the back area 18 under it); boxes take `x <= px <= x + w - 1` with the pointer clamped to 0..639 x 0..479.
While a handler holds `Mouse_Megfogva` (a slider grab) `AktIkon` isn't recomputed: the drag goes on outside the box.

**Tutorial layout (T)**: `Options_MouseInit2` 0x51a8a0 turns load, save and the slots off, quit becomes
(441,62,178,37), continue id 9 (441,102,178,37), empty tooltips. Continue leaves on the hover alone, tested every tick
(asm 0x51a7e3, hover id 0xbae240, no button or fade test). Texts (13,5,7)/(13,5,8) left-aligned at x 464, y 72/112
(`Gfx2D_2` 0x518a30). Faders per the ELF `OptionsGfx2D_1` 0x5d49a0 (the exe's isn't located): quit 441/178 wide,
continue under it on fader 2 with optf index 3, lasers for both, the slot-list fader skipped, later optf indices +1, the
hover glow reads optf 3 at the quit rect (an ELF quirk, unverified in the exe).

## 5. Input, slots, saving

### 5.1 `Options_Main` (0x51a320)

| Input | Effect |
|---|---|
| id 3 press | AreYouSure: cancel it (**no sound**: the cancel jumps past the call). Else `PlaySoundGlobal(0)` (**the screen's only sound**); load only (or T and not self-playing): `AktKep = 0xf` (main menu), `Atlepes`; else `AreYouSure = 1`. |
| id 2 press, AreYouSure | Not load only: `AktKep = 0xf`, `Atlepes` (YES: quit to the main menu). |
| id 2 press | The name entry: the slot's name (≤ 0x20) or `"Save Game %02d"`; `IOMaxLen 0x20`, `IOAzonosito 0x83`, `IOMode 1`, colour 2, font 2, the first key clears the text, at (0x17c, (slot−1)·12 + 0x82). |
| id 1 press (Load) | `LoadSave(slot, "", 1)`, `ReadUzenetAnimLista`, `SetIdoSpeed(1, 0)`; `OptionsOldIdoPause = 0` (**a loaded game runs unpaused**), `OptionsCsakLoadLehet = 0`, `AktKep = STARTSCREEN` (starmap). If a planet's ground war is on (`GetWarfareStatus(p) == 1`, the last such planet): straight to its colony screen (`AktKepKell 3`). |
| id 4, every frame while over it | `slot = (Mouse_y − 0x82) / 12 + 1` (C division), taken if filled or `< 23`; clamp 1..24. **Hover alone selects** (no button test). A press grabs (`Mouse_Megfogva`, `AktIkon` stays 4): while held the pick follows the pointer's y outside the list. |
| ids 5..7 | the slider drag (section 2). |
| id 18+s press with `OptionsFelfadeles == 100` | back: `AktKep = OptionsElottiKep` (0xb → 2). |
| id 9 (T) | the same back. |
| tab 10+s / 11+s | `OptionsMelyikMod = 0` / `2`. |
| Esc / `O` (`ESC_Pressed` ELF 0x581440 / `DoKeyboard`, outside the text entry) | Toggle: on Options, back to `OptionsElottiKep` (no `OptionsFelfadeles == 100` test, unlike id 18; load only: the main menu, `OptionsElottiKep` 0xf); on a game screen (`AktKep < 0xe`) open Options. Esc reaches `ESC_Pressed` from every game screen except the starmap with a panel up (`AktIkonKell 0x27`), the colony with its planet info (0x3d), messages (0x25) and the designer's edit mode (`MainCallBackEventHandler`). Blocked by `KepernyokLetiltva[0xd]`. F-keys / screen keys are refused while load only or a battle runs. |
| Esc / Enter in the text entry | `Options_EscPressed`: `IOMode = 0`, refresh. `Options_EnterPressed` (0x51a090): `IOMode = 0`, `LoadSave(slot, IOString, 2)`, refresh; if `OptionsElottiKep < 0xb` go back there; its inner `== 0xb` arm (→ 2, `starmaphovaleptovabb = 0xb`) is dead code, so a save opened from the space battle leaves Options up (ELF 0x5dc740 the same). |

### 5.2 The slot list (`Gfx2D_2`)

24 slots. Row i at y = 0x82 + (i−1)·12 (130 + 12(i−1)); drawn unless T.
- Name: `KiirStr` at x 380, font 2, max 0x1e, colour 2 if selected else 1. Empty slots 1..22 show 30 `-`; empty 23,
  24 nothing.
- Date of a filled slot: year `KiirInt` at x 0x238 (568), month at 0x253 (595), day at 0x262 (610), y + 1, font 1.
- The selected row: `DesignKeret1(0x178, (i−1)·12 + 0x7f, 0xbb, 0xc, 0x22)`.
- Slot 24 is the autosave (`IdoNapValtozott` exe 0x53e970: `LoadSave(24, "Autosave <year> <month> <day>", 2)`).

### 5.3 `OptionsRefreshSlots` (0x5196c0), `LoadTitle` (0x553590)

`LoadTitle(n)` opens `<GetSavePath>save<n>.sav` (`"%ssave%d.sav"`, exe 0x858b3c) and reads the header
(`re/formats/save.md` §2): the 9-byte `VERS0002`, the name (0x1f bytes), year, month, day, `Ido`, `OldNap`. Entry
`+0` filled, `+1` name, `+0x20` year, `+0x24` month, `+0x28` day, `+0x2c` `Ido`, `+0x30` `OldNap`.
`GetSavePath` = `%LOCALAPPDATA%/GyroscopeGames/ig2/` (the folder of `options.dat`).

### 5.4 `LoadSave` (0x543c80)

Mode 2 writes `safesave.sav`, then copies it to `save<slot>.sav` (1 KiB blocks); mode 1 runs `InitNewGame(0,5,0,0)` and
reads the slot. Both end with `FRISSITES_IGAZIT`, `FlottakUjraSzamol`; a failed save puts "Failed to save game!" on the
message line. The format is `re/formats/save.md`.

## 6. Option rows (`KirakMenuPontok` 0x5171f0)

17 rows × 0x22c at 0x9160a0: `+0` display name, `+0x100` key, `+0x200` enabled, `+0x204` value, `+0x208` states,
`+0x20c` page, `+0x210` id, `+0x214` dependency, `+0x228` allowed in multiplayer. Names: `Options_LoadStrings`
(0x51a2c0) `GetString(13, 2, k+1)` for k = 0..16, skipping 6 and 7 (rows 5 and 6 keep an empty name [guess]).

| k | Key | Default | States | Page | Id | Dep |
|---|---|---|---|---|---|---|
| 0 | `OptionsKellAutoSave` | 1 | 1 | 2 | 101 | 0 |
| 1 | `DebugData_grBattle` | 0 | 1 | 2 | 102 | 0 |
| 2 | `DebugData_GrNoOverrun` | 1 | 1 | 2 | 103 | −102 |
| 3 | `DebugData_Battle` | 0 | 1 | 2 | 104 | 0 |
| 4 | `DebugData_SpNoOverrun` | 1 | 1 | 2 | 105 | −104 |
| 5 | `Cutscenes` | 1 | 1 | 2 | 106 | 0 |
| 6 | `VROT` | 0 | 1 | 2 | 107 | 0 |
| 7..16 | `KELLKINYITASDEFAULT`, `Lowrestextures`, `KellEpuletnevKiiras`, `OptionsKellArnyek`, `OptionsKellUt`, `OptionsKellSpecFX`, `OptionsKellIdojaras`, `OptionsBuildingRes` (2 states), `OptionsHiresVectors`, `OptionsFogResolution` (2 states) | 1,0,1,1,1,1,1,2,1,2 | | 1 | 201, 211, 202..209 | 0 |

(The default and state values are the exe's .data; the port reads them from the user's exe, not from this table.)
`Options_LoadStrings` (exe 0x51a2c0 decompiled): row k takes `GetString(13, 2, k + 1)` except k = 5, 6, which keep the
name compiled into the table; so strings 6 and 7 are never shown and the names after them follow the string numbers,
not their keys' meaning (as the exe has it; the port copies it). A page's rows draw at y = `n·15 + 0x5c` (+100 on page 1): name at x 0x1e, value at x 0xe6; hover =
`y..y+15`, `31 ≤ x ≤ 279`, no button held. Colours: name 0x22, value 0x25; both 2 hovered; both 1 disabled. A press on
an enabled row: `value + 1`, back to 0 past `states`; `OptionsGraphicsDetail = 3`; `SaveGlobalRegistrySettings`. Value
text: 1 state (13,3,1) on / (13,3,2) off; 2 states (13,3,3..5) low / medium / high. `CheckOptionsDependencies`: dep 0
enabled; dep > 0 enabled iff that id's value ≠ 0; dep < 0 iff it is 0; not MP-allowed in multiplayer: disabled.

## 7. Settings file (`SaveGlobalRegistrySettings` 0x51b4e0)

`setRegKey(key, "%d")` into the in-memory map for `Sound Volume`, `Music Volume`, `Gamma Correction`,
`Graphics Detail`, `Save slot`, `OptionsMelyikMod`, `OptionsGfxDetail`, `Player name`, `IRC Address`, then
`SaveOptions` (the 17 rows, each also applied to its global) and `save_options_new` (0x51c4d0): the whole map to
`<GetSavePath>options.dat` (`wb`; `ig2_formats::options` reads it). Called from Kilep and from a row press.

## 8. Draw list

### `Gfx2D_1` [elf 0x5d49a0] (faders and lasers)
`F` = `OptionsFelfadeles`. Each block fades in over its window of F (`V` 0..100), with `KirakFader` and, while
`0 < V < 100` and not leaving, `DrawLaser(rect, 0x3c, 0x50, 0xfa, V/100)`.

| Block | Rect | V | Drawn when |
|---|---|---|---|
| Load | 366,62,78,33 | `3F` (F < 33) | slot filled, not MP, not T, not AreYouSure |
| Save / YES | 452,62,78,33 | `3(F−33)` | as id 2 is active |
| Quit | 541,62,78,33 (T: 441,62,178,33) | `3(F−66)` | always |
| Continue tutorial | 441,102,178,33 | as Quit | T |
| Slot list | 361,116,265,330 | `2F` (F < 50), laser from (639, 479) | not T |
| Left panel | 9,62,288,384 | `2(F−50)`, laser from (0, 0) | always |
| Tabs | 13,66,90,20; 108,66,90,20 | with the panel | always |

Then per visible slider `FadeKerettelKerekit(x, y−3, 220, 10, …)` (brighter when hovered), `DrawSparks`,
`GFX_DateMoneyScrollFade` (the top bar).

### `Gfx2D_2` (0x518a30) (texts)
1. The slider hot-spots; the volumes applied. 2. Slider titles and level texts. 3. The page's rows. 4. Button labels,
   `KiirStr(text, x, 0x48, 9, font 3, …)`, colour 2 hovered else 1: Load (13,5,1) at 0x195; Save (13,5,3) / YES (13,5,2)
   at 0x1eb; Quit (13,5,6) / BACK (13,5,4, load only) at 0x244; NO (13,5,5) at 0x249; T: (13,5,7) at y 0x48 and (13,5,8)
   at y 0x70, x 0x234 − 100s. Labels and tabs use align 7 (centred on x: the boxes' middles). Tabs at y 0x46: (13,5,9)
   at 0x3a, (13,5,11) at 0x99, colour 2 if hovered or its page is up (the second tab tests page **1**, though its press
   sets 2: an exe slip, so the game tab never stays lit [obs]). 5. The slot list (5.2). 6. The
   text entry box: `FillToScreen(IOX−1, IOY−1, len·charW+2, charH+2, 0x27)`, the text, a cursor block blinking on
   `AktIdo200`. 7. Slider tracks (`line2d`): per visible slider, columns `j = 0..value+9` at `x + 2j` from
   `y − 2 + d` to `y + 6 − d` (`d` 1 at `j = value+8`, 3 at `value+9`), colour by `f = j/220·2·Blokk2/100` (`Blokk2`
   reloaded each column) as `0xAABBGGRR`: top (r, g, b) = `(10f, 10f, 200f)`, bottom `(210f, 210f, 200f)` [obs: blue];
   the thumb: five grey (`Blokk2·140/100`) strokes at `x14 = x + 2·value + 0x12`: `(x14−3, y+4)–(x14−1, y+4)`,
   `(x14−1, y+2)–(x14−1, y+4)`, `(x14, y)–(x14, y+3)`, `(x14−1, y−1)–(x14−1, y)`, `(x14−3, y−2)–(x14−1, y−2)`.
   Slider texts: the title at `(x, y + 0xb)` (font 2, colour 2, 1 when disabled), the level `levels[value / 20]` at
   `x + 10 + CharXMeret[2]·len(title)`, colour 0x25. Slot rows: names at `(0x17c, 0x82 + 12(i−1))`, dates `KiirInt` at
   `y + 1` right-aligned in 4/2/2 cells (font 1, align 6). Text opacity `KiirStrSzinSzorzo`: `Blokk2` for the left
   panel, buttons and tabs, `Blokk1` for the slots.

## 9. Sounds

Only `PlaySoundGlobal(0)` on the Quit/BACK/NO press. Load, save, tabs, rows and slots are silent.

## 10. Exe vs ELF

- `Options_Main`: the exe ends the id-3 handler early after a cancel; the slot y as `slot·12 + 0x76`; no guard on
  `SaveLeiro`. `MouseInit2`: the multiplayer check on ids 1, 2. `Gfx2D_2`: the load-only label branches, the tab string
  (13,5,11). `LoadSave`: an extra `DAT_009130a8 = 0`. `SaveGlobalRegistrySettings`: `sprintf` + `setRegKey` inline.

## 11. Check in the asm

| Spot | exe | Status |
|---|---|---|
| Slider drag precision | 0x51a6c0..0x51a6f7 | [asm] f32 divide, f64 × 100 + start, truncate |
| Quantisation skipped | 0x51a6cd / 0x51a701 | [asm] `+0x29a ≠ 0` jumps over it |
| Fade step 0.6, cap 100 | `Frissit2_Options` (ELF 0x5da990) | exe not located |
| Background dim | `GFX_Options` (ELF 0x5d6f30) | exe not located |
| Slot pick division | 0x51a320 | C division toward zero |
| Tab highlight vs press | 0x518a30 / 0x51a320 | open (section 13) |
| `SetGlobalSoundVolume` = 0x559ad0 | 0x518a30 | [guess] |

## 12. Unknowns

1. Page 1 (graphics, gamma) is not reachable by the two tabs (the second sets page 2); a saved `OptionsMelyikMod` 1
   would show it.
2. The exe addresses of `Gfx2D_1`, `Frissit2_Options`, `GFX_Options` (the port follows the ELF there).
3. The registry loader (where `options.dat` is read back into the globals at boot); the port applies the file's keys.

## 13. Runtime observations

`run_screen.py --kep 13` on `quick-1.sav` (Hungarian install): page 0; the music and sound sliders at y 110/150 with
"Zene hangereje – Magas" / "Hangerő – Magas"; Save and Quit buttons (no Load: slot 1 empty); the 24-row list with 22
dashed rows, row 23 empty and row 24 "Autosave 2328 1 1"; slot 1 framed; the back-area tooltip under the pointer; the
lasers' corner glows.

Load only (`--boot --run`, `tools/trace/scripts/options-loadonly.json`): the load ring's release opens Options at once
(no sound; the menu music plays on); the rings fly away (`Cel` 6) and the main menu's scene runs on behind; no top bar;
Quit reads BACK with the tooltip (13,1,13). Sounds (`--sounds`, `tools/trace/scripts/options.json`): only
`Global_Hangok[0]` (`ingame/Click1.wav`) on the quit press, one UI tick after it; none on NO, tabs, rows, slots.

## 14. Client status

`ig2-client::options` (`options.rs`, opened by `window.rs` from the load ring and with Esc/`O`): sliders, rows and
dependencies, slots, name entry, quit question, tutorial layout, MP states (the MP host keeps Save for ADR 0007's MP
save, a joiner has neither), Back re-entering the screen before as its `_Belep` with OldKep 0xd. Load reads an original
`save<n>.sav` or our `save<n>.ig2s`; save writes ADR 0007's format plus the live starmap camera chunk (row 70); settings
go to `ig2-rework-options.dat` (row 69). The game page adds six "Remake" rows below the original's seven (enhanced,
row 402; `display.rs`): display mode, resolution, vsync, letterbox, live background, one-language names. Missing:
the faders' hover glow `FadeKerettelKerekitDesign`, the research design-return flags (row 359, kept); load into a war colony and the held slot pick are decompile-only (rows
190/191). Capture: `run_screendiff.py options --script scripts/options.json` and `options-load --script
scripts/options-loadonly.json` (`IG2_OPTIONS_DIR` pinned to a copy of the exe's options.dat and save*.sav); open
differences are the random sparks and the hover glow.
During the tutorial the two "simulate" rows read off and are greyed (`Row.locked`, `OptionsScreen::lock_battle_simulate`; row 410, ADR 0019); the stored values stay in the settings file. Known bug (row 411): the four battle rows' values are not applied to exe 0xB77230..0xB7723C (`SaveOptions` 0x51b640 / `SetOptions` not ported), so switching them does not change battles.
