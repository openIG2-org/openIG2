# Text, the scrolling message line and the message inbox

How the original draws text (`KiirStr`), the scrolling message line on top of the screens and the inbox screen
(`AktKep == 10`). Evidence: the ELF oracle decompiled with `tools/ghidra/DecompileFunctions.java` (addresses ELF
unless marked exe), exe tables found by their bytes, reference captures (`tools/trace/run_screen.py`). What gets
posted, the lists' state and the inbox's expiry are sim systems (`re/rules/info-messages.md` §2–§4,
`ig2-sim::messages`). `[inf]` marks inference.

## 1. Text (`KiirStr(text, x, y, len, size, flags, colour)` 0x4f8570)
A bitmap font: kep 0x54 `CHARSET.BMP` (the remaster's `screen/charset_bmp`, 512 × 512, addressed as a 256-unit atlas:
uv = pixels / 256). Per size 1..=4 (exe tables, `ig2-rules::Charset`):

| table | exe | size 1 | 2 | 3 | 4 |
|---|---|---|---|---|---|
| `CharXMeret` glyph width = advance | 0x912A50 | 6 | 6 | 8 | 12 |
| `CharYMeret` glyph height | 0x912A64 | 5 | 7 | 10 | 12 |
| `CharXTolas` step in the atlas | 0x912A78 | 6 | 6 | 8 | 12 |
| `CharYTolas` row step | 0x912A8C | 6 | 8 | 10 | 12 |
| `CharXKezdet` first row's y | 0x912AA0 | 0 | 24 | 62 | 126 |
| `CharYDarab` glyphs per row | 0x912AB4 | 42 | 42 | 32 | 21 |

A byte's glyph index comes from the order string (162 bytes: exe 0x8527F0 for Hungarian, 0x852748 for the other
languages, `ig2_getlocal()`); bytes not in it map to 76 (a space). Glyph `g` is the atlas cell
`((g % perRow)·stepX, (g / perRow)·stepY + startY)` of `width × height`, drawn at `(x + i·width, y)` as a quad with
blend (1, 1) (additive), colour `DDColor[colour]` × `KiirStrSzinSzorzo`/100 (or `KiirStrR/G/B`); spaces draw
nothing. `len` caps the bytes; flags & 3: 1 centres in `len` cells, 2 right-aligns, 3 centres on x; flag 8 clips to
`HatarX1..X2 × HatarY1..Y2` by whole glyphs. `KiirInt` writes a number the same way. Texts are raw bytes in the
game's code page (`uzenet.txt`, planet names).

## 2. The scrolling message line
`FRISSIT_Scroll2` 0x66ebd0 runs in the 10 ms UI frame (not while entering/leaving a screen); `GFX_Scroll` 0x66f270
draws it. Both run when the screen is the messages screen (10), the space battle in SP (0xb) or options (0xd), or
when no message window is up (`VanUzenet()` false), or in a few battle states (a planet at war on the colony
screen, options during a battle); not on the colony screen while `StoryAdat+0xb4` is set (the tests at the head of
both functions). An error text (`HibaUzenetSzoveg`) replaces the line for 300 frames.

Motion, in characters (`UzenetStart` `s`, `UzenetSebesseg` `v`, `UzenetHatar` `h`):
- When the current text has left (`s ≥ len`) or there is none: take `ScrollUzenetek[1]` (text, `+0x3c` important →
  `UzenetFontos`), close the list (`ig2-sim::messages::Ticker::pop`), `s = −53`, `v = √((len + 53)·0.0146)`,
  `h = −(53 − len)/2` (texts are ≤ 53 bytes); play `SOUNDS/SCROLL/{HUMAN|BRUTAL}/%04d.WAV` (`EN == 5`: BRUTAL) for
  the id unless 1, 0x14, 0x15, 0x1a. No entry → the line is empty.
- `s < h`: `s += v`, `v = max(v − 0.0146, 0.02)`; on reaching `h`: `s = h`, `v = 400` (the hold counter).
- `s == h`: `v −= 1`; at 0: `s += 0.01`, `v = 0.2`. `s > h`: `s += v`, `v += 0.0146`.
So a text slides in from the right, slows to a stop (centred when shorter than the line), stays 400 frames (4 s),
then speeds away to the left.

Drawing: offset `o = (int)((s + 53)·6)`; the window is 53 spaces + the text (at most 0xb4 bytes), 0x36 bytes from
`o / 6`; `KiirStrAlap(window, 280 − o % 6, 7, 0x34, size 2, flags 0xc, colour)`, colour 0x21, or 6 for an important
text blinking with `AktIdo300Szazalek`; clipped to x 270..593, y 5..16 (+40 in x with `IG2_TOOLBAR`). The history
(`GFX_ScrollHistory` 0x66eb30, the top bar's drop-down): entries 1..=`UzenetekDbHistory` at `(268, 16 + 10k)`,
size 2, colour 0x21, over a fader 0x146 wide.

## 3. The inbox (`Messages_Belep` 0x5a8a20, `MessagesGfx2D_2` 0x5a1d30)
The list shows four entries of `Uzenetek` (0x88 each: `+0` the message id, `+4` kept/read flag, `+8` expiry, `+0xc`
the date as an `Ido`) from `AktUzenetPointer`: indices `ptr + 3` down to `ptr` fill rows 0..=3; with fewer than four
messages they take the bottom rows, newest first. Row `r` is at `y = UBAY − (3 − r)·45` (globals in the ELF's data:
`UBAX` 3, `UBAY` 352, `UBAFont1` 2, `UBAFont2` 3, `UBASzin1` 1, `UBASzin2` 2, `UBASzin4` 34; `UBAXSzsz` 157,
`UBAYSz` 218 for the selection frame):
- the sender `UzenetFajtak[KerUz(id)]+7` (`uzenetek/uzenetek.txt`, `ig2-formats::dialogue::MessageKind::sender`;
  `KerUz` 0x5a5b90 = the kind with that id), via `AliasConv`, 0x13 bytes, size `UBAFont2`, colour `UBASzin4`, or
  `UBASzin1` when `+4` is set;
- the date at `y + 13`: year `Ido / 18000` (`Ido2Ev` 0x6206a0, 4 digits), month `Ido / 1500 % 12 + 1`, day
  `Ido / 50 % 30 + 1` with dots, size `UBAFont1`, colour `UBASzin2`;
- the two subject lines `+0x1b`, `+0x2f` at `y + 23`, `y + 32`, 0x14 bytes;
- a frame around the open message (`UAktUzenet`).
The open message's text (`ParbeszPlace`, typed out with `|C`/`~` colour codes) and its answers (`Valasztasok`,
`AnswFont`) follow below; the panel slides in (`mesframe` to 30). The capture of quick-1.sav (empty inbox) shows the
panel at the left half-way in [obs].

## 4. Not specified here
The top bar (money, date, speed buttons, the line's frame) is in re/screens/hud.md §2. The messages screen's 3D frame and animation, talking heads and videos
(`uzenetanim_%d`), the dialogue text and answers (`dialogue.rs` parses them), `AliasConv`'s aliases, input. The
client draws §1–§3 over plain panels.
