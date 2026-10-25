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

**The glyph quad as drawn** (row 27, 2026-10-24): every vertex (exe 0x4a9e60, ELF 0x4f8570) also carries the cell
inset by half a texel, `(u0 + 0.5/W, v0 + 0.5/H)` in `z, rhw` and `(u1 − 0.5/W, v1 − 0.5/H)` in the second uv
(`W × H` = the sheet's pixels); the `FONT` pass (`ig2_vertex_font`/`ig2_fragment_font`) clamps the sampled uv into it.
Quad and uv are exactly `width × height` virtual pixels (no rounding). At 800 × 600 a glyph is 8.75 px tall, and when
its bottom edge lands on a pixel centre (`y + height ≡ 2 mod 4`, e.g. the SpaceWar info box's title at y 427, edge
542.5) the exe's GL fills that row (the clamped last texel row: the descenders of `y`, `g`), where wgpu's top-left
rule leaves it out [obs: `_local/screendiff/spacewar` grab3 t600 rows 542 and 587; every other row equal]. The client
draws glyphs as `kind::GLYPH` (the clamp) moved 1/64 px down with their uv, so ties fall as in the exe.

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
  `KerUz` 0x5a5b90 = the kind with that id; an id not in the table gives `HanyUzenetFajta`, the 1000-entry table's
  first unused, zeroed entry, so the sender and both subject lines are empty [obs: 230.sav's ids 30578/30579, no
  caption in the exe]), via `AliasConv`, 0x13 bytes, size `UBAFont2`, colour `UBASzin4`, or `UBASzin1` when `+4` is
  set;
- the date at `y + 13`: year `Ido / 18000` (`Ido2Ev` 0x6206a0, 4 digits), month `Ido / 1500 % 12 + 1`, day
  `Ido / 50 % 30 + 1` with dots, size `UBAFont1`, colour `UBASzin2`;
- the two subject lines `+0x1b`, `+0x2f` at `y + 23`, `y + 32`, 0x14 bytes;
- a frame around the open message (`UAktUzenet`).
The open message's text (`ParbeszPlace`, typed out with `|C`/`~` colour codes) and its answers (`Valasztasok`,
`AnswFont`) follow below; the panel slides in (`mesframe` to 30). The capture of quick-1.sav (empty inbox) shows the
panel at the left half-way in [obs].

Globals (ELF data 0x1186088..): `UBASorkoz` 3, `UBASzin3` 8, `UBAYSzsz` 176; the answers `AnswX` 210, `AnswY` 430,
`AnswXSz` 340, `AnswYSz` 46, `AnswFont` 2, `AnswSzin` 2 / `AnswSzin2` 6 (under the mouse) / `AnswSzin3` 1 (disabled);
the text `MsgStartX` 210, `MsgStartY` 18, `MsgXBekezd` 10, `MsgYKoz` 4, `MsgFont` 2. The row frame is
`Keret(UBAXSz + 1, UBAYSz + slot·45 − 1, UBAXSzsz − 2, 43, 0x26)` (`Keret` 0x5374e0: four 1-pixel lines) for the open
message while it types or its dialogue runs (`UBAXSz` is set at run time [inf: 0]).

## 4. The frame (`Messages3DInit` 0x5a19a0, `Messages3DUpdate` 0x5a3ad0, `FRISSIT_Messages2` 0x5aad60)
`messbck.xvb` (`vector/messages/`, `textures/messages/`) ×0.9 at (33, 10, −650), not rotated, posed by `mesframe`;
projection as the info screen's (`xcenter` 320, `ycenter` 240, focal 600); lit per vertex by the engine's model
(research.md §2.1): ambient 0x141414, parallel 0x808080 from (0, 0, −10), side lights 0x7f5d00 / 0x004e7f from
(200, 0, −350), all on `mainmeshid` (`mesOldalsoFenyekBe` ELF 0x5a1280 = exe 0x50a000, which the ported names call
`resOldalsoFenyekBe`; the exe's "`mesOldalsoFenyekBe`" 0x5047d0 is another screen's). The side lights never move:
`mesMozgatMainFenyek` (0x5a0f40) has no caller and nothing writes `meskeringeto` but `Messages_Belep`'s reset
[obs: no `call` to it in the ELF, no reference to the exe's lights 0xDABEBC/0xDABE70 outside their creation; the
capture matches to 0.3]. Sub-meshes 1/6, 2/7 and 4/0 are 40 % transparent. `mesframe` runs to 15 at +1/3 a UI frame (the list panel); opening a message
(`KELLKINYITAS2`) runs it on to 30 (the text and answer panels), where the exe saves the screen (`SaveScreen(3)`) and
from then on draws that picture (0x36) instead of the model. Under the model `Copy3hatterkep(0x35, 0xff − 195·min(
mesframe, 15)/15)` (the screen before, `SaveScreen(2)` in `Messages_Belep`; black only in bare-jump captures, see [`overlay-background.md`](overlay-background.md)). The buttons 0 and 3
press in by moving their sub-mesh 0 in z (`mesSetGombFrame` 0x5a0d60).

### 4.1 The opening and the closing (`mesframe`, exe 0xdabe44, f32)
Exe flags: `mesKELLKINYITAS` 0x914943 (= the options' screen-animation switch 0x912a39, default 1), `mesKINYITVA`
0xda65e5 (open), `mesBECSUKAS` 0xda65cf (closing), `KELLKINYITAS2` 0xda65e6, `ELINDULT1` 0xda65fd (the first
render done), `MessagesKellKepMentes` 0xd909ae (save the screen).
- **Enter** (`Messages_Belep` exe 0x507f90, `Messages3DInit` 0x506970): `mesframe` 0, or 15 with the switch off;
  `mesKINYITVA = !switch`, the other flags 0; `KELLKINYITAS2 = 1` at once when a message is already selected
  (`DAT_00dabda0 != −1`), else only when an entry is opened (`KiertekelUjuzenet` exe 0x505d60). The game clock stays
  (`IdoPauseMertAtlepesAlattVan` 0xbae265) until the screen is open. Sound `screen23.wav` (with the switch on).
- **Each UI tick** (`FRISSIT_Messages2` exe 0x505180, 10 ms; asm 0x5052c0..0x5053a3, every step
  `(float)((double)mesframe ± c)`):
  1. switch on, not closing, `ELINDULT1`, `mesframe < 15`: `+ 1/3` (the double 0x857038), over 15 → 15;
  2. `mesKINYITVA`, `KELLKINYITAS2`, not closing, `mesframe < 30`: `+ 1/3`; `≥ 30` → 30 and `MessagesKellKepMentes = 1`;
  3. switch on and closing: `− 0.6` (0x852250), below 0 → 0.
  From 0: 46 ticks to 15, 45 more to 30 (30.0000248 clamps). Nothing takes `mesframe` back from 30 to 15: a second
  message opened at 30 replays nothing.
- **Each render** (`Messages3DUpdate` = exe 0x506c50): `ELINDULT1 = 1`; `mesKINYITVA = 1` once `mesframe ≥ 15` (and
  the game clock runs); `SetFrame(MesMainMesh, mesframe)` (exe 0x452f30: 8.8 fixed point, the key `⌊f⌋` blended to the
  next by the 1/256 steps); the background `Copy3hatterkep(0x35, 255 − c)`, `c = mesframe ≥ 15 ? 195 :
  (int)(mesframe / 15 · 195)` (f32 `divss`, `mulss`, `cvttss2si`, asm 0x506e2e..0x506e56). At 30 the render with
  `MessagesKellKepMentes` draws the model without the top bar and the 2D pass, then `SaveScreen(3)`; from then on
  picture 0x36 instead of the model until the closing, which draws the live model again.
- **2D** (`MessagesGfx2D_2` exe 0x507020, skipped in the save render): the list rows while `mesKINYITVA` and not
  closing; the message's text, its typing and the answers only at `mesframe == 30` and not closing; the clip gate
  (§5.3, exe 0x506f60) likewise. `MessagesAnimFrissit` draws the clip before the typing pass, so a clip the typer's
  end opens (`StartAnim_EndOfMessage`, `VegeAzElsoAnimnak`) shows from the next frame and its clock starts there
  [obs: end clip first drawn at t 133].
- **Leave** (`Messages_Kilep` exe 0x5083d0): the typer, the speech and the alien voice stop, the clip closes, sub-mesh
  5/0 shown again; with the switch on: `mesBECSUKAS = 1`, `scrn23ki.wav`, then a loop of `UpdateFrame` +
  `FRISSITES` (one render and the UI ticks due, wall-clock) until a pass begins at `mesframe == 0` a second time
  (asm 0x5084b1..0x50850e: one more render at 0). From 30: 50 ticks; from wherever the opening stood. Then the screen
  asked for (`MessagesElottiScreen` 0xda65b4, or the key's).
- **Input**: hot-spot 3 only when `mesKINYITVA`; 4 and 6 at `mesframe == 30`; 7..=9 (and Esc, `MainCallBackEventHandler`
  exe 0x4e2db0, no `mesframe` test) live during the opening: leaving mid-opening closes from where it stood. Inside
  the closing loop the main loop's mouse pass doesn't run and `bae24d` refuses Esc and the screen keys. A button pressed
  inside the closing loop never reaches the next screen [obs: `scripts/messages-close-press.json` with
  `--via-starmap 60`, 2026-10-25: a left down on the starmap's screens arrow during the closing, released after the
  starmap is up, opens no menu].
- **Measured** (`run_screen.py --script scripts/messages-{open,esc}.json` over a replay save with inbox messages,
  `mesframe`/`mesKINYITVA`/`mesBECSUKAS` per grab, 2026-10-24): 0 → 15 in 46 ticks (`mesKINYITVA` at the next render);
  an Esc or a press at UI tick t is taken after the render of t + 1 (`IG2_gameUpdate`/`Messages_Main` come after the
  frame's ticks and render), so the closing's first −0.6 is tick t + 2; an inbox row pressed at t starts 15 → 30 at
  tick t + 3 (`KELLKINYITAS2` a tick after the open; `Kiirashang` at t + 1); Esc at `mesframe` 10 closes from 10.333
  and the screen is gone at the second pass at 0. `ig2-client --snapshot --screen messages --script` matches every
  grab's `mesframe` (screendiff `messages --script`).

## 5. The open message (`KiertekelUjuzenet` 0x5a8550 and the dialogue)
Opening inbox entry `UAktUzenet = AktUzenet0to3 + AktUzenetPointer` loads `uzenetek/msg%04d.txt` and `txt%04d.txt`
(`BetoltFajlok` 0x5a83d0, each made to end in `
`) and builds the text in `ParbeszPlace` (byte offsets throughout):
- A plain message (kind `+4` clear) prints part 1 (`KiirReszletek(1)`), sets the entry's `+4` (read) and, once,
  `+5` + `Story_MessageMegnezes(id)`; the dialogue is over (`ParbeszedVege`).
- A conversation evaluates block 0 (`KeresBlokk` 0x5a69c0: the line after the `%02d` header; `KiertekelBlokk`
  0x5a6ac0: each `- PP NN label` line is an answer `Valasztasok` (0x5c: label from column 8 to the line end with
  `#k#` replaced, `+0x50` part, `+0x54` next block, `+0x58` disabled from `Tiltolista` by `Story_LetiltasCheck`
  via `BlokkBelepes`)). If its first answer prints a part, that part is printed and its next block evaluated at
  once; if the part is −1 the next block is evaluated. Answers played are kept in `UzenetPlaybackek` (0x54 per entry)
  to replay a read message.
- `KiirReszlet` 0x5a80b0: `KeresReszlet` finds the next line starting with the part's `%02d`; `GetTxtFileCommands`
  0x5a7c70 reads `PP AAAA SS WW[ #m]` (speed `SS`: `ParbeszTyperPtrNovelo = SS/165`; `WW` "EN" = the player speaks
  (1), else the other side (2), which also looks up the talking-head clip); `PutColorCommand` writes `|C` + 1/2; then
  every following `+`/`-` line (the text after the marker and a space) goes through `TordelveAttesz`.
- `TordelveAttesz` 0x5a51a0 (src, `ParbeszPlace`, length, plus, other): a `-` line first takes back up to two line
  ends at the text's end (so it flows on, and an empty `+` line right before it goes too [obs: decompile only]); the
  current line's visible width is counted back to the last `` (`~` −1, else +1; a line starting with `|C` −3); the
  source to its `
` (the `` kept) gets `#k#` = the entry's parameter `k` (`+0x10 + 0x14·k`; "#=#" →
  "(~verror~s)"; in `~f…~s` when the other side speaks); `a2az` converts it for Hungarian (`Lang` 4); then each word
  follows a space if `width + strxlen(word) + 1 < 0x36`, else a `` and the word starts the line.
- Typing (`MessagesGfx2D_2`): `MsgElteltMp += (MsgActTime − MsgOldTime)/500` (`SDL_GetTicks`), one more byte per
  `ParbeszTyperPtrNovelo`, then `MsgOldTime = MsgActTime`. Only that pass (at `mesframe` 30) and the open
  (`KiertekelUjuzenet`) or `Messages_Belep` set `MsgOldTime` (the ELF's reset in `Messages3DUpdate` tests
  `mesframe < 30 && mesframe == 30`, never true, and the exe has none), so the first pass takes the whole 15 → 30
  wait at once: ~460 ms, a short message typed out in that frame [obs: `messages-open.json`, opened at t 86, typed
  out at t 133]. At the end
  `KiirtaMar`, `Novelo = 0.4`, a plain message's end-of-message clip, and a dialogue whose answer led to −1 is over
  (`ParbeszedVege`, the entry's `+4` set). Drawing from line `ParbeszKezdoSor` (its colour by
  `ScanLastParbeszedColor`): `|C` n sets colour n, `~c` a colour code (`SzinKarakter` "spkfzv " → `SzinTomb` 2, 6, 8,
  3, 7, −1 = blinking 6/2 with `AktIdo200`, else 2; a digit indexes `SzinTomb`), a `` starts the next line; byte
  `MsgX` of line `MsgY` at `(220 + 6·MsgX, 22 + 11·MsgY)`, 13 lines, beyond line 12 the text scrolls a line; the
  typer's cell blinks in colour 8 while typing or waiting for an answer.
- Answers (once typed, the entry not read): at `(220, 434 + 11k)` up to the ``, colour by state; one answer alone is
  taken as soon as the text is typed (`Melyikvalaszfolottvanazeger` 0x5a9920); taking one (`ValaszKivalasztva`
  0x5a97a0) calls `Story_MessageSelected(id, part)` (`ReszletValasztas`), records it, prints its part and evaluates
  its next block (−1: the dialogue ends with this text).

## 6. Hot-spots and input (`Messages_MouseInit` 0x5aa340, `Messages_Main` 0x5a9c50)
1 `(160, 189, 13, 19)` and 2 `(160, 400, 13, 19)`: the list one newer / older (`AktUzenetPointer` ±1, kept in range;
only after `LenyomasValtozo` reached 5 UI frames); 3 the inbox `(UBAXSz, 218, 157, 176)`: the row by quarters (top =
`AktUzenet0to3` 3), only rows holding a message, only when no dialogue is running and no message picked still types (`DAT_00dabda0 == −1`, reset by `MessagesGfx2D_2`
once typed; `FolyamatbanU == −1` and
`ParbeszedVege`); 4 the answers `(210, 430, 340, 46)` by quarters; 5 the map `(201, 204, 334, 188)`; 6 the text
`(210, 18, 333, 156)`; 7 `(0, 0, 195, 180)`, 8 `(586, 0, 54, 392)`, 9 `(0, 410, 143, 70)`, 10 the whole screen: back to
`MessagesElottiScreen`. A press inside x 186..512, y 1..167 types the rest at once (`Novelo` 0.001; right button:
`SkipAll` too) and stops the voice. `FRISSIT_Messages2` (every 10th UI frame) starts the automatic messages
(`KeresAutomataUzenet`).
Tooltips (`Messages_MouseInit` exe 0x508b10: `Mouse_Szoveg_Set(id, 0, rect, GetString(10, 1, n), wait, 1)`, the hand
on every spot): n = id for 1..=6, 7 for 7..=10; shown by `UpdateFrame` after the 400 ms rest (input.md §8) on the lowest
enabled spot under the pointer; the "lit frame" over a hovered row is that tooltip's fader box [obs: messages-open.json
t 129, the inbox's "Üzenetek" box at the pointer]. Client: `MessagesScreen::{spot_at, tip_text}`.
`Messages_MouseInit2` (exe 0x508940, every frame): 3 on per `DAT_00da65e5`; 4 and 6 only at `mesframe == 30`; 5 and 10
off; 7..=9 only with no choices pending and the text typed (`AktValasztasok == 0 && KiirtaMar`). Esc
(`MainCallBackEventHandler` exe 0x4e2db0) in that idle state sets `AktIkonKell = 0x25`, a press on hot-spot 7
(`IG2_gameUpdate` 0x4e0440): the screen leaves for `MessagesElottiScreen` as a click there does; otherwise Esc does
nothing. An answer asked for (hot-spot 4, a..d, `Melyikvalaszfolottvanazeger`) sets `VoltUjValasz` (0xDABDB4);
`ValaszKivalasztva` (0x509d10) runs at the end of every `Messages_Main` and takes it once the choice isn't disabled,
so a disabled answer stays asked for (only `Messages_Belep` clears it). Client: `MessagesScreen::{pending, answer}`.

## 7. Not specified here
The top bar (money, date, speed buttons, the line's frame) is in re/screens/hud.md §2. The talking heads and videos
(`StartAnim` 0x5a1340, `uzenetanim_%d`), the voices and typing sounds, the story map (`CreateStoryMap`), the
automatic messages, `AliasConv`'s aliases beyond `#k#`, the read message's replay (`UzenetPlaybackek`). The story hand-offs (`Story_MessageMegnezes`, `Story_MessageSelected`,
`Story_LetiltasCheck`) and the entry's `+4`/`+5` flags are in rules/info-messages.md §7 and sent as `MessageOrder`s.
