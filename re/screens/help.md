# The Help screen (`AktKep` 1)

The encyclopedia F1 opens. From the ELF decompiles (`_local/decomp/help/elf`, `elf2`) and the exe's
(`_local/decomp/help/exe`); the exe wins. Addresses are exe unless marked ELF. `[elf]` = read in the ELF only; `[asm]`
= checked in the exe; `[obs]` = seen in a capture (§9). Coordinates are 640 × 480 virtual pixels. "Tick" = one 10 ms
UI tick. Strings are `GetString(1, section, n)` (`txt1`). Port: `ig2-client::help` (§10).

## 0. Functions

| Part | ELF | exe |
|---|---|---|
| `Help_Belep` | 0x54db70 | 0x4d7060 |
| `Help_Kilep` (closing loop, `hlpRemoveNullBoxes` inlined) | 0x54e0e0 | 0x4d7440 |
| `Help_Main` (per frame, input) | 0x54e260 | 0x4d7510 |
| `Help_MouseInit` / `Help_MouseInit2` (empty) | 0x54e510 / 0x54e8b0 | 0x4d7640 / folded `WaitForDrawing` |
| `Help3DInit` | 0x54aaf0 | 0x4d59e0 |
| `GFX_Help` (per drawn frame) | 0x54f160 | 0x4d5540 |
| `HelpGfx2D_1` (faders) / `_2` (texts, topic list, pictures) / `_3` (empty) | 0x54ad10 / 0x54b300 / 0x54c040 | 0x4d5b80 / 0x4d5f00 / – |
| `FRISSIT_Help2` (tick) | 0x54ea30 | 0x4d5150 |
| `HelpPutCsuszkaFader` (scroll thumb) | 0x54eca0 | 0x4d6e40 |
| `TopicBetolto`, `hLoadFileEsTordel`, `hTordelveAttesz`, `hAtteszSorvegeig`, `hstrxlen` | 0x54dac0, 0x54c7e0, 0x54c1e0, 0x54c760, 0x54c170 | 0x4d78f0, 0x4d7980, 0x4d7b00, inlined, inlined |
| `GetNumberOfLines`, `hlpFindNextLine`, `FindSorVege`, `ScanLastFnt`, `ScanLastCol` | 0x54c9f0, 0x54cb30, 0x54cbc0, 0x54cc90, 0x54cd60 | 0x4d5880, inlined, inlined, 0x4d7890, 0x4d7830 |
| `HelpPregeneralo`, `FindCommand`, `FindSubTopic`, `GetRemainingPixels` | 0x54ce30, 0x54d040, 0x54d7b0, 0x54e1f0 | 0x4d6d40, 0x4d52c0, 0x4d5490, 0x4d58b0 |
| `HelpKikeresKepek`, `HelpKirakPrevPictures`, `HelpKirakOnScreenPictures` | 0x54d8f0, 0x54d300, 0x54d560 | 0x4d66d0, 0x4d6aa0, 0x4d6800 |
| `help_press_f1` / `help_callback` / `help_new_available` (remaster toolbar) | 0x550bb0 / 0x550c10 / 0x550ac0 | 0x4d8b40 / 0x4d8650 / 0x4d8af0 (**the ported name on 0x47c150 is wrong**: that is `GetWarfareStatus`) |

Data (static values read from the exe [asm]): `MAXFRAMES` 100.0f (0x90af68), `hlpKELLKINYITAS` default 1 (0x90af6c),
`HELP_WIDTH` 69 (0x90af70), `Barky` 47 (0x90af74), `HelpFilesPath` `"help/"` (0x90af78), `TopicKezdY` 60 / `TopicKezdX`
30 (0x90af80/84), `TopicMaxLines` 415 / `TopicTextKezdY` 55 / `SorKoz` 3 (0x90af98..a0), `TopicBox` (19, 47, 131, 383),
`TextBox` (170, 46, 426, 385), `HelpX1/Y1/X2/Y2Vagas` (178, 48, 596, 429) (0x90afac..d8), `CharXMeret` [_, 6, 6, 8,
12] / `CharYMeret` [_, 5, 7, 10, 12] (0x912a50/64). `TopicTav` = `SubTopicTav` = 15, `SubTopicKezdX` = 38 [elf:
`__cxx_global_var_init`]. State: `helpframe` 0xb49e7c, `hlpELINDULT/BECSUKAS/CsuszkaMode/KINYITVA` 0xb49e78..7b,
`OldTopicSel` 0xb49e80, `BarOldH/Y` 0xb49e84/88, `TopicSel` 0x90af94, `SubTopicSel/Pos` 0x90afa4/a8, `TopicActSorPtr`
0xb76a34, `FSzor` 0xb76a78, `helpf` 0xb76a80, `HelpElottiScreen` 0xb77058, `HelpTopicPlace` 0xb4af30, `HelpTopicSor`
0xb57280 (stride 0x14), `AllTopics` 0xb6f958 (stride 0x5a4), `MaxTopic` 0xb6f954.

## 1. Entry and exit

Opened by F1 (`DoKeyboard`: F*n* asks for `AktKep` *n*; not on `AktKep` 1 itself, not while a screen change runs; the
screen-enable rules are `GetScreenCanBeChanged` ELF 0x550990, deviation 72) and by the remaster's toolbar help button
(`help_callback`: on screens where `help_new_available` (starmap without fleet/transfer mode, 3..9, 0xb) it shows a
`help_new\help_<screen>.bmp` overlay first; a second press, or a screen without one, sends F1 through
`help_press_f1`, which pushes F1, or Esc on `AktKep` 1). The overlay is the toolbar's (not ported here: §11).

`Help_Belep`:
1. `hlpELINDULT = hlpBECSUKAS = 0`. `HelpElottiScreen = OldKep` unless `OldKep == 1`; from Messages (10)
   `MessagesElottiScreen`, from Options (0xd) `OptionsElottiKep`.
2. `hlpKELLKINYITAS = OldKep != 1 && default (1)`; `hlpKINYITVA = !KELLKINYITAS`; `helpframe = KINYITVA ? 100 : 0`;
   `IdoPauseMertAtlepesAlattVan = !KINYITVA`: the game time stands from here until `Help_Kilep` clears it (nothing in
   between does), so for the whole visit when it opened animated.
3. `BarOldH = 10`, `BarOldY = 0`, `FSzor = 0`, `HelpCsuszkaMode = 0`.
4. `help/helpmain.txt` (`helpmain_mobile.txt` when `ControlConfig[0] & 1`; 0 in the GOG exe 0x90b520 [asm]):
   `<TOPICname,file>` starts a topic (name up to the comma, file up to `>`), each `<SUBTOPICname>` up to the next
   `<T` adds a subtopic (at most 20). Everything else is commentary.
5. `TopicSel = OldTopicSel` (the last topic clicked, 0 at first), `SubTopicSel = −1`; `TopicBetolto`,
   `GetNumberOfLines`, `TopicActSorPtr = 0`, `HelpPregeneralo` (§4).
6. `InitFaderDesignStruktura(helpf)`, `Help3DInit` (§2).

No music change, no sound anywhere in the Help code.

`Help_Kilep`: `IdoPause… = 1`; if `hlpKELLKINYITAS`, the screen stays on (`AktKep = 1` for the loop) with
`hlpBECSUKAS = 1` and `UpdateFrame` + `FRISSITES` run until `helpframe` has been 0 for two passes; then
`hlpBECSUKAS = hlpKINYITVA = 0`, `IdoPause… = 0`, the NullBoxes removed.

## 2. `Help3DInit`

No model: a fresh scene, `fadertexture = textures/design/screen/dt.bmp`, the background darkening fader, and the
render list `GFX_AllandoInit1` (top bar), `HelpGfx2D_1` + `InitFader1`, `HelpGfx2D_2` + `InitFader2`, `HelpGfx2D_3`,
`GFX_AllandoInit2` + `InitFader3`. The widescreen frame: `e = (W·480/H − 640)/2` (int), `Hatar` = (−e, 0)..(639 + e,
479).

## 3. Drawing

`f = helpframe / 100` (f32), `g = 1 − f`.

`GFX_Help`: `hlpELINDULT = 1`; a release ends the drag (`HelpCsuszkaMode = 0`); `GFX_DateMoneyScrollFade` (the top bar);
`Copy3hatterkep(0x35, grey 255 − min(128, (int)(f·128)))` (`DESIG_U3.BMP`'s slot; black in the GOG captures, as on
Options); the scene's 2D passes; then, while not closing, the hover glows of the text and topic boxes:
`FadeKerettelKerekitDesign` with `helpf`'s colour scale `s0` / `s1` (the box colours `(0, s·54, s·90)` fill and
`(0, s·90, s·130)` edge, fill transparency `g·155 + 100`, edge `g·225 + 30`), the scan band at `fmod(helpf clock, 1)`,
and each box's glow `1 − glow[i]` with its style while it runs (`helpf` +0x3e0/+0x458).

`HelpGfx2D_1`: `LaserPos = (639, 479)`; `KirakFader(id, helpf, x, y, w, h, fill (0, k·54, k·90), g·155 + 100, edge
(0, k·90, k·130), g·225 + 30, 0.005, 0.005)`, `k = helpf s[id]`:

| id | rect | when |
|---|---|---|
| 0 | `TextBox` (170, 46, 426, 385) | always |
| 1 | (612, `Barky` − 1 = 46, 16, 386), the scroll track | `f > 0.2` (f64 compare) |
| 2 | `TopicBox` (19, 47, 131, 383) | always |
| 3 | (24, 401, 122, 25), the exit button | always |

then `HelpPutCsuszkaFader` (the thumb, once per **drawn frame**) [elf, constants asm]: `N` = the topic's line count;
target `th = 387, ty = 0` when `N ≤ 36`, else `th = 13932 / N`, `ty = TopicActSorPtr·(387 − th)/(N − 36)` (int);
`BarOldH` steps ±20 towards `th`, `BarOldY` ±(15 + 120·dragging) towards `ty` (both clamped on arrival); `FSzor`
±0.1 towards 1 while dragging, 0 otherwise; height `h = BarOldH`, or `457 − Barky − BarOldY` when `Barky + BarOldY +
BarOldH − 4 > 453`; `FadeKerettelKerekit(613, Barky + BarOldY, 14, h − 3, fill (F·12, F·12 + 54, F·12 + 90), g·155 +
100, edge (F·12, F·12 + 90, F·12 + 130), g·225 + 30)` (`F = FSzor`, every channel an `(int)` of its f32).

`HelpGfx2D_2`:
1. While `0 < f < 1`: `DrawLaser(TextBox, 76, 76, 76, f)` from (639, 479) and `DrawLaser(TopicBox, …)` from (0, 0);
   `DrawSparks` while `f != 0`.
2. Only open and not closing: the clip `Hatar` = (178, 48)..(596, 429) for the pictures; the exit label
   `GetString(1, 2, 1)` at `KiirStr(·, 82, 410, 10, size 2, flags 7 (centred on x), colour 2)`.
3. The topic list (and its clicks, §5) from y = 60, x = 30, step 15: the selected topic in colour 2, its subtopics
   below it at x = 38 (colour 0x25 when selected or hovered, else 1); the others in 0x22, 2 when hovered. A row is
   hovered when `30 ≤ x ≤ 148`, `pos − 3 < y ≤ pos + 11` and no drag holds the mouse.
4. The text (§4.3), then `HelpKirakPrevPictures`, `HelpKirakOnScreenPictures` (§4.4); `Hatar` back to the widescreen
   frame.

`KiirStr` flags 4: draw (no clip, no centring); colours are `DDColor` indices.

## 4. The text

### 4.1 Loading and wrapping (`TopicBetolto` → `hLoadFileEsTordel`)
`help/<file>` (≤ 100000 bytes) becomes `HelpTopicPlace`, lines ending in a lone `\r`. The parse runs while the
current byte is `-`, `+` or `*` (and the file isn't empty); anything else ends it.
- `X\r…` (an empty `*`, `+` or `-` line): one `\r`, the input moves 3.
- `*` line: the bytes from offset 2 up to the `\n` (the `\r` included) are copied verbatim (no wrapping).
- `+` / `-` line: `hTordelveAttesz(line + 2, out, o, plus)` [elf, read 2026-10-23]:
  - `first = 1`. On a `-` line: if `out[o − 1] == \r` it is dropped and `first = 0`; again for `out[o − 2]` (an empty
    line before is dropped too). So **`-` continues the previous line's paragraph with a space**; `+` starts a new
    line whose first word gets no space.
  - The width already on the output line: from `o − 1` back to the previous `\r`, `+1` a byte, `−1` for `<` and `>`.
  - The source line (to `\n`, `\r` included) is cut into words at spaces (leading spaces skipped). For each word with
    `w = hstrxlen(word)` (+1 a byte, −1 for `~`):
    - `width + w + 1 > 69`: write `\r` and the word, `width = w` (`first` stays as it is);
    - else write a space first unless `first` (which then clears), then the word, `width += w + (space)`.
  - The last word carries the line's `\r`.
- `GetNumberOfLines`: `N` = the `\r`s + 1 (the last line is empty and never drawn).

### 4.2 The line table (`HelpPregeneralo`)
Entry 0 = (colour 1, font 1, offset 0); entry `i` (1..N−1): offset = after the `i`-th `\r`, colour / font = the last
`<Cn>`/`<Fn>` (one or two digits) before it (`ScanLastCol`/`ScanLastFnt`, from 1/1), height = `CharY[font] + 3`. Entry
0's height is never written (it keeps the value a longer topic left there; 0 at first), and the picture flag (a `<P`
before the line's `\r`) is never cleared.

### 4.3 Drawing the lines
`m = SubTopicPos` when both `SubTopicPos` and `SubTopicSel` are set, else `TopicActSorPtr`; `TopicActSorPtr = m`.
Colour and font from entry `m`; `y = 55`; while `y < 415 && m < N − 1`: walk the line from its offset: `FindCommand`
(`<Cd>`/`<Cdd>` colour, `<Fd>`/`<Fdd>` font, `<Pname>` picture, `<SUBTOPIC>` (S, B, O at +1, +3, +5) length 7; any
`<` skips `length + 3` bytes, an unknown tag with the last length); `FindSorVege` = the bytes to the next `\r` or `<`;
those bytes go out with `KiirStr(·, 178 + col·CharX[font], y, len, font, 4, colour)` (`col` = the bytes drawn so far on
the line); until the `\r`. Then `y += 3 + CharY[font]` (the font at the line's **end**), `m += 1`. `KepernyoAljan =
y ≥ 415`; `TopicLastSorOnScreen` = the lines drawn.

### 4.4 Pictures
`HelpKikeresKepek` (topic load): every `<Pname>` in order becomes kep 0x23 + i, loaded from `../help/name` (the
remaster: `help/<name>_bmp.pvr`, `.` → `_`, lower case). `KepToScreen(kep, x, y)` draws it at its size, clipped to
`Hatar`. In both passes a flagged line is scanned from its offset to its `\r`; `col` counts bytes, a `<P…>` tag counts 0
and the byte after it is skipped; `x = 178 + col·CharX[font at the line's start]`:
- `HelpKirakPrevPictures(first)` (lines `0..first`): `s = Σ height[line..first)`; drawn at `y = 55 − s` when `s <`
  the picture's height (a picture whose line scrolled off stays partly in view).
- `HelpKirakOnScreenPictures(first)` (lines `first..first + TopicLastSorOnScreen`): `s = Σ height(first..line]`;
  drawn at `y = 55 + s`.

## 5. Input

`Help_MouseInit`: six hot-spots (`AktIkon = id + 30`), tooltip `GetString(1, 1, id)`:

| id | rect | |
|---|---|---|
| 1 | 610, 45, 27, 23 | scroll up: switched off (`Mouse_Szoveg_Off_Range(1, 2)`), dead |
| 2 | 610, 409, 27, 23 | scroll down: off, dead |
| 3 | 17, 44, 139, 358 | the topic list (tooltip only) |
| 4 | 610, 45, 22, 387 | the scroll track |
| 5 | 171, 46, 426, 382 | the text (tooltip only; one-shot) |
| 6 | 24, 401, 122, 25 | exit (one-shot) |

`Help_Main` [asm constants]: over the track (0x22) the mouse is held (`Mouse_Megfogva`), a press starts the drag,
`SubTopicSel = −1`; while dragging `TopicActSorPtr = 0` for `N < 37`, else `h = 13212/N`, `d = 367 − h`, `m = N −
36`, `ptr = ((y − 66 − h/2)·m + d/2)/d` clamped to `0..=m`. Exit (6, or Esc, which `MainCallBackEventHandler` maps to
`AktIkon 0x24` on this screen): `AktKep = HelpElottiScreen`. F1 does nothing here.

Clicks on the list (`HelpGfx2D_2`, at draw time; left button, the press frame, no drag): a topic → `TopicSel =
OldTopicSel = i`, `SubTopicSel = −1`, reload (§4), `TopicActSorPtr = 0` (the current topic too); a subtopic `k` →
`SubTopicSel = k`, `SubTopicPos = FindSubTopic(k)` (the line of the `k`-th `<SUBTOPIC>`), and if fewer than 383 px
remain below it, `SubTopicPos` = the last line from which ≥ 383 px remain (`j` from `N − 1` down while `rest(j) <
383 && j > 0`). The view then follows `SubTopicPos` until the track is touched.

No mouse wheel and no keys besides Esc in the Help code (the touch scroll of `FRISSIT_Help2`, `(y0 − y)/10` lines over
the text box, is the mobile build's).

## 6. Clock

`FRISSIT_Help2` (a tick, on `AktKep` 1): `FaderDesignFrissito(helpf)`; opening (`KELLKINYITAS && !BECSUKAS && ELINDULT
&& !KINYITVA`): `helpframe += 2.0` up to 100 (then `KINYITVA`): 50 ticks; closing: `helpframe −= 2.6` down to 0
(about 39 ticks, then two more frames). Constants f64 0x852338 = 2.0, 0x853338 = 2.6 [asm].

## 9. Captures

`run_screendiff.py … quick-1.sav help --script scripts/help.json` (2026-10-24): rest, list clicks, subtopic jump,
the track drag and the close all match (text 1.8–3.4, topics 3.0–4.9, after the release frame fix below). What stays:
- The exe draws the pointer and its tooltip one frame late in the harness (t200/t240 topics 14, text 12–16: the
  exe's pointer and "Súgó szöveg" box where the client's pointer already moved; the client draws no pointer).
- Mid-drag (t370) the exe trails by one tick: the scripted press reaches `Help_Main` at UI tick 361 for script t 360
  [obs: `probes/help.js`], so its `TopicActSorPtr` is one 10 px step behind.
- The exit box's shine (t382/410/495) is the fader's random shine (`FaderDesignFrissito`'s `rand`).
- Not compared: the opening. `Help_Belep` calls `FRISSITES` every 10 bytes of `helpmain.txt`, which in the scripted
  clock holds the first frame back ~85 ticks. `probes/help.js` on a real-clock run shows `helpframe` 0, 2, 4, … a UI
  tick from the first `GFX_Help` (`hlpELINDULT`) to 100, as ported.

[obs: `probes/help.js`] `AktIkon` (0xbae23c) names the hot-spot under the pointer only while the button is down and on
the release frame; on that frame `Help_Main` still drags (`HelpCsuszkaMode` ends in `GFX_Help`, after it), so the view
stops at the release point.

## 10. Port
`ig2-client::help`: `wrap`/`Text` (§4.1–§4.2, unit tests on hand-made lines), `HelpScreen` (§1, §3, §5, §6),
`--snapshot --screen help [--script S]`. The window opens it with F1 over the starmap, the colony and the overlay
screens and draws it on its own frame path (black, the screen, the top bar) without game ticks; Esc and the exit
button close it and the screen before comes back with the game clock restarted. The line table and `OldTopicSel`
live on between visits (`help::Memory`).

## 11. Not ported / open
- The remaster's toolbar help button and its `help_new` overlays (`help_callback`, `ig2_toolbar_*`).
- The mobile touch scroll.
