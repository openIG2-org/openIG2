# The HUD: brushes, the top bar, the colony panels and radar, names on the maps

How the original draws its 2D HUD over the screens: brush records (`BrushToScreen`), the top bar that every screen
shows (money, date, speed buttons, the message-line frame), the colony screen's panels and its minimap
(`DrawRadar`), and the names written over the starmap and the colony. Evidence: the ELF oracle decompiled with
`tools/ghidra/DecompileFunctions.java` (addresses ELF unless marked exe; the exe's versions checked where noted),
the exe's tables found by their bytes, reference captures of ig2.exe (`tools/trace/run_screen.py`, 800 × 600 =
the 640 × 480 virtual screen × 1.25). Text is `KiirStr` (re/screens/messages.md §1). `[inf]` marks inference,
`[obs]` what the captures show.

## 1. Pictures and brushes
- `Kepek` (ELF 0x1171ea0, 0x130 bytes; exe 0x903558, 0x124 bytes, 107 entries): the 2D pictures ("kep") by index,
  `+0` the file name (`COLONY_X.BMP`, …), loaded on first use (`LoadKepToMemory` 0x533db0). The remaster's file is
  `dataraw/screen/<name lowercased, '.' → '_'>.pvr` at twice the original's resolution (`COLONY_X` 1232 × 832).
  `HACK_SetBrushok` (0x569860, exe 0x4ef690) renames kep 14 to `BRUSHOK_ORIG.BMP` when `IG2_TOOLBAR` (exe byte
  0xFE6532) is clear, which it is in the GOG build [obs: nothing is shifted by the toolbar's 40 pixels].
- A brush record is 14 dwords (0x38 bytes): `src x, src y, dst x, dst y, w, h, step x, step y, per row, kep`, then 4
  unused words. Tables: `Brushok` (ELF 0x117b140, exe 0x912AD8, 20 records: the top bar, ids 1000+),
  `ColonyBrushok` (ELF 0x115a570, exe 0x8ECA08, 33), `StarmapBrushok` (ELF 0x119a450, exe 0x927130, 13). A
  screen's own table is `AktBrushok` (ids below 1000).
- `BrushToScreen(id)` 0x5352c0: `DDBrushToScreen3d(kep, src, dst, w, h)` 0x4f9b40. `BrushToScreenMelyik(id, n)`
  0x535500 draws frame `n` (1-based): src + `(step x·((n − 1) % per row), step y·((n − 1) / per row))`.
  `…XY(id, n, x, y)` puts it at `(x, y)` instead of dst; `BrushClipToScreenMelyikXY` clips it to the `Hatar` box.
  `CopyToScreen(id)` 0x534da0 is `BrushToScreen` without the frame.
- `DDBrushToScreen3d` 0x4f9b40: a quad at `(dst x, dst y)` of `|w| × |h|` virtual pixels, alpha-blended (src alpha,
  1 − src alpha), no depth. Texture coordinates `u = src x·k / W`, `v = src y·k / H` and size `|w|·k / W`,
  `|h|·k / H`, where `W × H` is the loaded picture's size and `k = 2` for a negative `w` (`h` for v), else 1: a
  negative size marks a record whose source rectangle is in the original's pixels on a doubled remaster picture.

### 1.1 Colour space and sampling of the 2D pass
The exe's 2D quads (`DDBrushToScreen3d` ELF 0x4f9b40 / exe 0x4aae80, `KiirStr`'s glyphs) are pre-transformed
vertices (`x, y, 0.5, 1`, colour, `u, v`) handed to `gl_draw_tlv` 0x4f68f0, which queues them for the render thread
(`ig2_pushcommand`; `command_glDrawElements` 0x4269f0 draws them with the stateblock's blend, `command_glBlendFunc`
0x426780). Textures come from `command_glLoadTexture` (ELF 0x426e30 / exe 0x733fe0: `dataraw/<name>.pvr` through
`sgres`, the font sheet in the `FONT` pass) → `sg3d::texture_t::load_pvr` (ELF 0x86db50 / exe 0x728d50, checked):
- **No sRGB anywhere**: the screen pictures' formats map to plain `GL_RGB`/`GL_RGBA` (`charset_bmp` is 8-bit
  luminance, `brushok_orig_bmp` RGB 565, `colony_x_bmp` RGBA 4444), and the exe never enables
  `GL_FRAMEBUFFER_SRGB` (no 0x8DB9 immediate in ig2.exe). So texels, the vertex colour (`DDColor[i]`, the brush
  tint) and the blend (src alpha, 1 − src alpha for brushes; additive for text) all work on the stored gamma-space
  values. [obs: the money text's core is (160, 160, 20) = `DDColor[0x21]` exactly.]
- **One level, bilinear**: min filter `GL_LINEAR_MIPMAP_NEAREST` only with more than one mip level, else
  `GL_LINEAR`; mag `GL_LINEAR`; wrap from the PVR's `SGLB`/`TXNV` metadata word (bit 0/1 repeat in s/t, bit 2/3
  trilinear (+anisotropy), bit 4 nearest). The screen pictures have one level and `TXNV` = 11: bilinear on level 0,
  repeat (at 800 × 600 the 2× pictures are minified 1.6×, so the glyphs alias thin rather than blur).

The client (ADR 0006) renders into an sRGB frame with sRGB textures: its 2D blends ran in linear light, which turned
a tint `c` into `c^(1/2.2)` (text (203, 203, 77)) and brightened every translucent or filtered edge; with its mip
chains the glyphs also blurred into their neighbours' atlas cells. `Blend::GammaAlpha`/`GammaAdditive`
(`ig2-client::render`) are the exe's pass: drawn through the frame's non-sRGB view with the textures' raw views and a
level-0 bilinear repeating sampler; `hud::Pictures::brush` and `font::Font::text` use them. Before/after:
re/screens/diff-results.md (the top bar 7.7 → 0.1 on every screen).

### 1.2 Faders
Every translucent box (`KirakFader` ELF 0x547920, `FadeKerettelKerekit` 0x538330, `FadeScreen` 0x537890) is queued
into `fade` (0x34 B records) and drawn by `PutDarkening` 0x5391d0 → `PutDarkeningFace` 0x538de0 →
`polygonsfader` 0x5f16a0 in the same 2D pass (checked):
- **Shape**: `FadeKerettelKerekit` = `FadeKerekit` 0x537bd0's rounded fill (five strips: `(x + 4, y + 1, w − 8, 1)`,
  `(x + 2, y + 2, w − 4, 2)`, `(x + 1, y + 4, w − 2, h − 8)` and their mirrors) + `FadeKeretKerekit` 0x537e70's
  one-pixel rounded frame (four sides inset 4, eight corner pieces).
- **Colour**: `FadeScreen` packs `0xTTBBGGRR`; `polygonsfader` turns the fourth value into `255 − t` (a
  transparency) and blends src alpha / 1 − src alpha (its additive mode is unused here).
- **Texture**: `polygonsfader` binds `fadertexture`, which every screen's 3D init loads from
  `textures/design/screen/dt.bmp` (`Init3dStarmap`, `Init3dColony`, `Research3DInit`, `Diplomacy3DInit`,
  `Infoing3DInit`, `Design3DInit`, …: `FindRefs` on 0x169e518); the texel multiplies the vertex colour. The
  coordinates are `((x + u)·su, (y + v)·sv)` per virtual pixel. `GLOBALKELLDESIGN` (ELF .data 0x1171e88) is 1 and
  never written, so `KirakFader` passes `su = sv = 0.005` and the fader's own offsets (`faderstruktura +0x1e0/+0x258
  + 4i`, 0 after `InitFaderDesignStruktura` 0x547cf0, then drifting by `rand()` in `FaderDesignFrissito` 0x547e10):
  its fill shows the texture (mean 0.50); its frame and every direct `FadeKerettelKerekit`/`FadeScreen` call pass a
  scale of 0, so all their vertices sample (0, 0): with repeat, the mean of the four corner texels (0.65 in the GOG
  `dt.bmp`). The callers' colours are × `s` = `+0x168 + 4i` = 0.9 + an occasional "shine" (`FaderDesignFrissito`).
  [obs: across the edges of the transfer panel and the long info in the exe captures, inside = A + B·outside with
  B = 0.37–0.40 (= 1 − 155/255) and A = (0, 14, 24) (= (0, 48.6, 81)·0.61·0.48), which the port now reproduces to
  half a level.]

The port: `starmap_panels::fader` (scale 0) and `kit::fader::kirak_fader` (the texture at the fader's offsets, §1.2.1)
on `Blend::GammaAlpha`.

### 1.2.1 Fader design: the shine, the drift and the hover glow (`GLOBALKELLDESIGN` = 1)
Each screen keeps a `faderstruktura` (0x5cc B: `optf`, `resf`, `desf`, `starf`, `mainf`, `fifo`, …) for up to 30 faders
(`i` < 0x1e, larger `i` is ignored). Offsets `+off + 4i`, all f32 unless noted:

| off | meaning | `InitFaderDesignStruktura` ELF 0x547cf0 (exe 0x4d3590), per `i`, in RNG order |
|---|---|---|
| +0x000 | shine wait, −1 a tick | `random(1000)` |
| +0x078 | shine time left | 0 |
| +0x0f0 | shine 0..1 | 0 |
| +0x168 | `s`, the callers' colour scale | 0 (0.9 after the first tick) |
| +0x1e0 / +0x258 | the fill texture's u / v offset (`KirakFader`'s last two) | 0 |
| +0x2d0 | v drift a tick | `(random(10) + 1) / 30.0f` (f32 `divss`) |
| +0x348 (s32) | shine axis: 0 jitters v, 1 u | – |
| +0x3c0 (u8) | set on entering; not read | 0 |
| +0x3e0 | glow countdown `g`: 1 on entering, 0 idle | 0 |
| +0x458 (s32) | glow style 1..5 | – |
| +0x4d0 (s32) | the last fader entered | −1 |
| +0x4d8 | clock, +0.003 a tick (the scan bands, the info heads' pulse) | 0 |

**`FaderDesignFrissito`** ELF 0x547e10, once per UI tick (`Frissit2_Options` 0x5da990, `FRISSIT_Colony2`, design
0x50d890, `FRISSIT_Diplomacy2`, `FRISSIT_Info2`, research, starmap): `+0x4d8 = (f32)((f64)+0x4d8 + 0.003)`; then per `i`:
1. `if g > 0 { g = (f32)((f64)g − 0.01); if g < 0 { g = 0 } }` (about 100 ticks).
2. If `+0x78 > 0`: `if +0xf0 < 1.0f { +0xf0 = (f32)((f64)+0xf0 + 0.1) }`; `rand() % 100 − 50` (signed) added to `+0x258`
   (axis 0) or `+0x1e0` (axis 1).
3. `if +0xf0 > 0 { +0xf0 = (f32)((f64)+0xf0 − 0.02); clamp ≥ 0 }`.
4. If `+0x000 <= 0`: `+0x78 <= 0` → `+0x000 = random(1000) + 1000`, else `+0x78 −= 1.0f`. Else `+0x000 −= 1.0f`, and
   if it is then exactly 0: `+0x78 = random(50) + 50`, `+0x348 = random(2)`.
5. `+0x258 += +0x2d0`.
6. `+0x168 = (f32)((f64)(+0xf0 · 1.0f) + 0.9)`: 0.9..1.9.

**`KirakFader(i, fs, x, y, w, h, fill, t, edge, t', false, 0.005, 0.005)`** ELF 0x547920: draws the fader
(`FadeKerettelKerekit` with the texture offsets `(+0x1e0, +0x258)`, §1.2), then starts a glow when, in f32 with the
mouse as floats, `x < mx < x + w && y < my < y + h` (strict) and `(!Mouse_Bal && !Mouse_Jobb && !Mouse_Felenged) ||
Mouse_Lenyom` and `+0x4d0 != i` and `g == 0`: `+0x4d0 = i`, `g = 1.0`, `+0x458 = random(5) + 1`, `+0x3c0 = 1`. Nothing
resets `+0x4d0`, so a box glows again only after another box of the same screen has glowed. `random(n)` (ELF
0x4f0200) is `rand() % n` (0 for n < 1), the one libc stream.

**`FadeKerettelKerekitDesign(x, y, w, h, r, g, b, a, …4 unused, t, 1.0, style)`** ELF 0x545b60 draws the glow, if
`r > 1 || g > 1 || b > 1`, as one-pixel lines (`TscVonal` 0x5371e0, coordinates truncated to int; `line2d` 0x5f0700
additive, src·1 + dst·1, so `a` doesn't matter), colour `C = (r, g, b)`, on the 640 × 480 virtual screen.
`blink` = `AktIdo200` (on for 200 ms, off for 200 ms):

| style | lines |
|---|---|
| 0 | the scan band (not from a hover; `t` = `fmod(clock·k, 1)`): rows `yo = 0, 4, 8, … < h`, `d = |t − yo/h|`; if `d < 0.1f`, `a = (0.1f − d)/0.1f`, ÷ `(h·0.1f)/yo` if `yo < 0.1f·h` (0 at `yo = 0`), × `(h − yo)/(h·0.1f)` if `yo > h − 0.1f·h`; drawn if `(f64)a ≥ 0.1` with `c = (int)(C·a/2.0f)`: `(x, Y)→(x + w/6, Y)` from black to `c`, then `c` to `x + w − w/6`, then `c` to black at `x + w` (`Y = y + yo`, `w/6` integer) |
| 1 | a point runs round the box: `t < 0.25`: `(x + w·t·4, y)`; `< 0.5`: `(x + w, y + h(t − 0.25)·4)`; `< 0.75`: `(x + w − w(t − 0.5)·4, y + h)`; `< 1`: `(x, y + h − h(t − 0.75)·4)` (only the first in f32, the rest f64). Colour `(int)(C/1.2)` (f64). The lines `(Px, 0)→(Px, 479)` and `(0, Py)→(639, Py)` and a square of half-size `s = 10 + (int)(sin(t·3.1415·10.0)·5.0)` round P |
| 2 | four lines across the screen close in on the box: `t < 0.5`: `c = (int)(C·t·2.0f)`, `o = (int)((0.5 − t)·200.0)`; else `o = 0`, `c = C` if blink else black. `x − o` and `x + w + o` vertical 0..479, `y − o` and `y + h + o` horizontal 0..639 |
| 3 | corner marks shrinking onto the box: `d = t < 0.5 ? (f32)((0.5 − t)·200.0) : 0`; at `(x − d, y − d)`, `(x + w + d, y − d)`, `(x + w + d, y + h + d)`, `(x − d, y + h + d)` two 20-pixel strokes pointing out (horizontal and vertical, away from the box), colour `C`, when `blink || t < 0.5` |
| 4 | the four sides slide in: `q = (f32)(0.5 − t)`; `t < 0.5`: `x0 = t·x·2 + q(x − 300)·2`, `y0 = t·y·2 + q(y − 300)·2`, `x1 = t(x + w)·2 + q(x + w + 300)·2`, `y1 = t(y + h)·2 + q(y + w + 300)·2` (**`y + w`**: the exe's slip, asm 0x5471d1); else the box. Lines `(x0, y)→(x0 + w, y)`, `(x + w, y0)→(x + w, y0 + h)`, `(x1, y + h)→(x1 − w, y + h)`, `(x, y1)→(x, y1 − h)`, colour `C`, when `blink || t < 0.5` |
| 5 | the outline morphs from the last box that glowed past `t > 0.8` (`fade_oldx/y/sz/ma`, one set for the process, 0 at start): `X0 = x·t + (1 − t)·ox`, `X1 = (x + w)·t + (1 − t)(ox + osz)`, likewise Y; colour `C`, always |

After the lines, `style != 0 && t > 0.8` (f64) saves the box as `fade_old*`. `line2d` draws at once, while
`KirakFader`/`FadeKerettelKerekit` only queue their boxes, drawn after the screen's 2D pass: every fader of the frame
covers the glows and scan bands [obs: Options, a style-1 line stops at the slot list's edge; the slot list's scan band
isn't visible in the exe captures].

Runtime check (2026-10-23, `run_screen.py --kep 13 --script scripts/options.json --probe probes/fader.js`,
`_local/traces-faderglow`): the clock grows 0.003 a UI tick (one `FaderDesignFrissito` a tick); the glows start on the
ticks and boxes the spec gives (the save button on entering; the left panel and a tab on the same press, `+0x4d0` = the
later one; the panel again once its glow ended with the pointer still in it; quit; the slot list), each 100 ticks
(`g` 0.99, 0.98, … 6.6e−7, then 0); the drift steps are `k/30`; a shine climbs 0.08 a tick to 1.04 (`s` 1.94), holds
there ±0.02, decays 0.02 a tick to 0.9; all bit-exact in `kit::fader_design`'s test. The styles and the shine times
come from the one `rand()` stream, so they differ from run to run (and from the client's).

The screens call it right after their faders, unless leaving, with `t = 1 − g`, `C = (0, s·54, s·90)` (truncated):
- **Options** (`OptionsGfx2D_1` 0x5d49a0, `optf`): glows for faders 0 (load), 1 (save/YES), 2 (quit; in the tutorial
  both quit-row boxes are fader 2, but the glow tests and draws index 3 at the first box), 3 (the slot list); the
  left panel (4) and the tabs (6, 7) start glows (and spend `rand()`) but never draw them. Their colours use `s`
  of `optf` index `local_34 + k`: the quit box 2 (the tutorial's second row 3), the slot list 3, the panel 4 (5 in
  the tutorial), both tabs 5 (6). At full fade-in the slot list also
  draws the scan band: style 0, `t = fmod(clock·1.8, 1)`.
- **Research** (`Research3DUpdate` 0x5e4e10, `resf`): scan bands on the two side boxes (`fmod(clock, 1)`,
  `fmod(clock/2, 1)`), `C = (0, 54, 90)`; glows for faders 0 and 1 (the side boxes), `C` = `(0, 54, 90)`.
- **Design** (0x503ea0, `desf`): scan bands on the five boxes (`fmod(clock·k, 1)`, `k` = 1, 1/2 (f32), 1/1.5 (f64),
  1.8, 1.2), `C = (0, s·54, s·90)` of each; glows for faders 0..4.
- Starmap, info, colony/main screen (`mainf`): `KirakFader` only (the shine and the drift; their glows aren't drawn).
  `Starmap_Belep` inits `fifo` then `starf`; `FRISSIT_Starmap2` (ELF 0x719070) ticks `fifo` only while the fleet
  info, the transfer or the tanks info is up, `starf` always. `starf`'s colours use `s` of the fader's own index;
  `StarmapGfx2D_1` counts from 0 (the planet info box (319, 33, 302, 326) when shown, then the radar box (5, 423,
  162, 52)), `StarmapGfx2D_2` from 6 (the product mode's slide-in boxes 6..15, its static boxes and the two (32/532,
  86, 76, 35) after, then the info-off box and the tanks box: each the next count). Exact: `StarmapGfx2D_1` ELF 0x6dcb30 (planet
  info box 0 when shown, radar next), `StarmapGfx2D_2` 0x6de1b0 from 6: slide-in 10 boxes, product 10 boxes, exit
  button (only when `StarmapInfoOffHoltart` > 0, which the tanks info leaves at 0), then the tanks-info box (445, 380, 176, 30). Port: `starmap_panels::StarmapFaders { starf, fifo }` in `Panels`
  (`FaderDesign::next` draws from the one rand() stream, fifo then starf, re-inited on every starmap entry; glows'
  rand() draw skipped). `fifo`: transfer (`ShowStarmap` 0x6eca50) 0..5 + scroll track/thumb 6, fleet info
  (`ArrangeFleetInfo` 0x6e5c30) 0..5 + bar 6, tanks info (`StarmapTanksInfoFaderKirakas` 0x6ea720) 0..4 + bar 5
  (`fifo`'s fill (0, 54s, 90s) at 100, edge (0, 60s, 110s) at
  70). The capture tools dump the exe's `fifo` (0xF152F0) and `starf` (0xF168E8), 0x5cc B each, into `<state>.ui.json` ("faders"); the client loads them with `--faders` (FX13a, row 208). Info (`PutFaders` ELF 0x55b2c0): `infof` faders 0 (387, 271, 0xe2 − slide, 183) and 1 (280, 209, 100, 8),
  constant colours (no `s`); `Info_Belep` inits `InfoFejF` then `infof`, `FRISSIT_Info2` ticks both.

## 2. The top bar (every screen below 0xe)
Two persistent 2D layers (`GFX_AllandoInit1/2` 0x66d080/0x66d0f0, added by every screen's 3D init) draw it after the
screen's own passes, unless options (0xd) runs as the self-playing demo or the load menu, or the game is leaving.
- `GFX_AllandoCallBack1` 0x66c750: the bar's frame `BrushToScreenMelyik(1003, 1 | 2)` (2 during a space battle, a
  ground war on the shown colony, the options' load/battle states, an open dialogue on the messages screen; the
  record has one frame, so both look the same), then the research button 1005: frame 3 while the human researches
  something (`ResearchAktFejlesztett[EN] > 0`), frame 1 when nothing is researched and some tech is researchable
  (status 3 or 4 in `Talalmanyok[t].+0x45 + EN`), else 2 (and 2 in the battle states above).
- `GFX_AllandoCallBack2` 0x66caf0: the history drop-down (`GFX_ScrollHistory`, when open), the message line
  (`GFX_Scroll`, messages.md §2), **`GFX_IdospeedPenzDatum` 0x66a590** (exe 0x554990, checked), the screen-select
  menu (`GFX_SelectScreens`, when open), then texts: "message waiting" `GetString(0x12, 2)` at (268, 7), 0x36 bytes,
  size 2, flags 5, colour 6, pulsing with `KiirStrSzinSzorzo = sin(AktIdo/100)·50 + 50` when a message window is up
  (`VanUzenet`); an empire with no planets left gets the countdown texts at (10, 30) and (10, 50), size 4, colour 3.
  (`GFX_DateMoneyScrollFade` 0x66d160 only draws faders: the tooltip box, the select menus' boxes, the history box
  (268, 22, 0x146 × `10·n + 5`).)
- `GFX_IdospeedPenzDatum`: the speed buttons `BrushToScreen(1007 + i)` for the selected `i` and `1011 + i` for the
  others, i = 1..=4, selected = 1 when paused (`IdoPause`), else `IdoSpeed + 1`; the money `"$" + floor(MoneyMennyi(EN,
  0) + 0.5)` at (78, 4), 10 cells, size 3, flags 5 (centred in the cells), colour 0x21; the date `Ev` at (171, 4)
  4 cells, `:` at (205, 4) colour 2, `Honap` at (215, 4) 2 cells, `:` at (233, 4), `Nap` at (243, 4) 2 cells, size 3,
  flags 6 (right-aligned in the cells), colour 0x21. All x + 40 with `IG2_TOOLBAR`. Debug builds add the money
  pools per area. The text-input field (`IOMode`) is drawn here too.
- `HACK_SetBrushok`'s records (the GOG build's bar; `src → dst, w × h`, all on kep 14 with negative sizes): 1003
  (0, 0) → (75, 0), 525 × 20 (the three boxes: money, date, message line); 1005 (0, 40) → (600, 0), 40 × 20, frames
  43 × 18 apart, 3 per row; speed buttons selected 1008..=1011 = (1, 20), (16, 20), (30, 20), (51, 20) → x 0, 15, 29,
  50, widths 15, 14, 21, 23, height 20; the others 1012..=1015 = src x 77, 92, 106, 127, same places. [obs: the
  capture's bar matches: money box from x 75, "$109090" centred at x 90, the year at 172.]

### 2.1 The two drop-downs: screens and quick research (2026-10-24)
Input `MOUSE_SelectScreens` (ELF 0x66ba60, exe 0x555470), texts `GFX_SelectScreens` (ELF 0x66ace0, exe 0x554d90; no
difference), boxes `GFX_DateMoneyScrollFade` (ELF 0x66d160, exe 0x554500, fader pass 2). Exe globals: open flags
0xe40c5c (screens), 0xe40c5d (research), 0xe40c5e (history); `SelectX/Y/Sz/Ma` 0x912ad4/0x912f38/0x912f3c/0x912f40,
`Selected` 0xbaf330. Both draw on every screen 1..=0xd (not the load-only options), after the bar.
- **Hot-spots** (`Mouse_Init` exe 0x555df0 / ELF 0x6697c0, `Mouse_Szoveg_Set`; `AktIkon` = slot + 0x1e; the GOG build,
  `IG2_TOOLBAR` 0 (exe 0xfe6532)): 6 = (619, 0, 21, 20) the arrow, tooltip `GetString(0x12, 1, 5)`; 7 = (270, 0, 323,
  20) the message line (`GetString(0x12, 1, 6)`); 8 = (598, 0, 21, 20) the lightning, `GetString(0x12, 1, 7)`. None
  fires on release; no screen switches them off. While a menu is open `Mouse_Megfogva` keeps `AktIkon` (the menu owns
  the pointer until the release). **No sound** in any of these functions (the target screen's `Belep` may play one).
- **Screens menu** (hot-spot 6): a press with the left button not held (a right press) asks for the starmap
  (`AktKepKell = 2`) unless `AktKep` is 2 or `KepernyokLetiltva[2]`. A left press opens it at `SelectX = Mouse_x −
  0x81`, `SelectY = 0x14` (frozen while open). Rows: screens i = 1..=`HanyTaskbarElem` (13, static) without 11 and 12
  (the battles), row r counted without them. Each held frame `Selected` = the row whose `[SelectX, SelectX + 0x7e] ×
  [SelectY + 16r, SelectY + 16r + 16)` holds the pointer, else −1. The release asks for `Selected` when > 0, not the
  current screen and not `KepernyokLetiltva[Selected]` (the tutorial's event 0x1c gate: the switch raises
  `TutorialCheckEvent(0x1c)` unless the shown planet is `Tutorial_MainBolygoNeve`), and always closes.
  Box `FadeKerettelKerekit(SelectX − 6, SelectY − 2, 0x88, 16n + 4)` (n = 11 rows), fill (0, 10, 0) at 60, edge (0, 5,
  0) at 30 (the info screen's select boxes' colours). Per row: colour 0x21, 6 when `Selected`, 1 when disabled (wins);
  the key `Taskbar_Key_Names[i]` (static: "F1-".."F9-", "F10", 13 = "o -") at (SelectX, SelectY + 16r + 6), 13 cells,
  size 1, left; the name `Taskbar_Names[i]` = `GetString(0x12, 6, i)` at (SelectX + 0x13, SelectY + 16r + 3), 13
  cells, size 3, left. `KepernyokLetiltva` (exe 0xf66f2c, 20 bytes): the tutorial's DISABLE/ENABLE SCREEN commands;
  all set during a real-time space battle (`SpaceWar_Belep`/`_Kilep` save and restore it); per frame (`Fake_FRISSITES`
  exe 0x4de720) 6 = no other players met (`!KigyujtPlayerek()`, while 2 is enabled), 6/7/8 for race 5 and when
  `Diplomacia/Spy/TradeEngedelyezve` is 0.
- **Quick-research menu** (hot-spot 8): on the press and every held frame: `SelectSz = 3`, `SelectMa` = the most
  researchable techs (status 3 at `Talalmanyok[t].+0x45 + EN`) in one column, `SelectX = 0x140`, `SelectY = 0x14`
  (`0x1de − 16·Ma` if `16·Ma + 0x16 > 0x1e0`); it opens only when `Ma > 0`. Columns: `FastResearchHanyadikSor[(t −
  1)/16]` (static: categories 1..5 → 0 ships, 6..9 → 1 buildings, 10..12 → 2 tanks); row = the count of earlier
  status-3 techs in that column. `Selected` = the tech whose `[0x140 + 0x66c, +0x60] × [SelectY + 16row + 0xe, +16)`
  holds the pointer (the text is at +0x11). The release: with `Selected > 0`, nothing researched, status 3 and the
  labs enough (or the cheat 0xb7727c): `ResearchAtvaltTalalmanyra(t)` (the research screen's category and field jump
  to it), `Multi_Send(0x2a, t)` (start, `MULTIGET_Research`); on the research screen showing t, its start button
  goes in (`ResMelyikGombot 0xcc`, `ResGombNew`, `ButtonPressed`). With `Selected < 1` and a project running: a
  release inside the STOP rect sends `Multi_Send(0x29, current)` (stop). Then it closes; it also closes whenever no
  button is held (`Mouse_Gomb`).
  Texts: headers `GetString(0x11, 3, 0x26 + c)` (SPACESHIPS, BUILDINGS, TANKS) at (SelectX + 0x66(c − 1), SelectY +
  4), 16 cells, size 2, centred in the cells, colour 6; items the tech's name (`Talalmanyok[t]` +0, 16 bytes) at
  (SelectX + 0x66col, SelectY + 16row + 0x11), 16 cells, size 2, left; colour 3, 6 when `Selected`, 0x21 while a
  project runs, then 1 when a lab level is short (no cheat). Footer, `M = 16·Ma + 0x30`: idle with `Selected > 0`
  and its labs enough: `GetString(0x12, 3, 4) + ":   %d"` (the cost) at (SelectX + 9, SelectY + M − 0x17), size 2,
  colour 3. Researching: `GetString(0x12, 3, 1) + ":  " + name + "  " + pct + "%"` at (SelectX + 9, SelectY + M −
  0x17), 0x28 cells, colour 3; `GetString(0x12, 3, 2)` (STOP) centred on x = SelectX + 0x66·Sz − 0x22, same y, 10
  cells, colour 6 under the pointer in `[SelectX + 0x66Sz − 0x3f, +0x37] × [SelectY + M − 0x20, +0x19]`, else 3;
  `GetString(0x12, 3, 3) + "   %d"` (money left) at (SelectX + 9, SelectY + M + 7). pct = `(int)((f64)((f32)(prog·100)
  / ((f32)time · IdoSzorzo)) + 0.5)`; amounts `(int)((Ido·t − prog)·(cost·Money)/(Ido·t))` in f32 (the research
  screen's daily cost formula), doubled when urgent; the tech is the running one, or `Selected` when idle.
  Boxes (fill/edge as above), `h = 16Ma + 0x12`, `16Ma + 0x30` when idle and `Selected` can start, `16Ma + 0x4e`
  while researching (exe 0x554500's `iVar5`: replaced, not added; the texts sit 9 px inside the strips):
  (SelectX − 6, SelectY − 2, 0x66Sz + 3, h); idle-and-startable the cost strip (SelectX − 1, SelectY + h − 0x20,
  0x66Sz − 7, 0x19); researching the progress strip (SelectX − 1, SelectY + h − 0x3e, 0x66Sz − 0x43, 0x19), the STOP
  button (SelectX + 0x66Sz − 0x3f, SelectY + h − 0x3e, 0x37, 0x19), the cost strip (SelectX − 1, SelectY + h − 0x20,
  0x66Sz − 0x43, 0x19).

## 3. The colony screen's HUD (`AktKep` 3)
`GFX_Colony` 0x4da050 draws faders (`KirakFader` (6, 403, 77 × 17), (89, 403, 77 × 17), (5, 423, 162 × 52) for the
bottom-left info, the build menu's when open), `GFX_DateMoneyScrollFade`, then hooks the 2D passes
`ColonyGfx2D_0..4`. Records in `ColonyBrushok` (`src → dst, w × h`, kep 17 = `COLONY_X.BMP` unless noted):

| id | what | src → dst, size | frames |
|---|---|---|---|
| 1, 2 | building buttons (repair, demolish; colony.md §10) | (0, 93) → (179, 443), (0, 124) → (212, 443), 31 × 31 | 2 per row |
| 3..=5 | build-menu tabs | → (37/81/125, 331), 39 × 20 | 3 |
| 6, 7 | side tabs | → (176, 361/405), 20 × 39 | 3 |
| 8, 9 | 72 × 19 bars | → (33, 116), (115, 146) | 3 |
| 10, 11 | arrows | → (175, 39/121), 20 × 17 | 3 |
| 12 | the planet panel | (0, 200) → (0, 18), 223 × 153 | 1 |
| 13 | the radar frame | (238, 226) → (491, 348), 149 × 132 | 1 |
| 14 | the build/info panel | (458, 0) → (455, 23), 182 × 148 | 1 (not drawn by the passes below) |
| 17, 22, 23 | kep 97 `STAR_W.BMP` buttons | 38/40 × 37 | 1 |
| 18 | kep 3 `TALAJ.BMP` | → (522, 362), 100 × 100 | |
| 19..=21, 24..=27, 29, 30 | 31 × 31 buttons along y 443 | | 2 or 3 |
| 28 | kep 1 `FUV6.BMP`, the radar picture | (0, 0) → (522, 362), 100 × 100 (positive: picture pixels) | |
| 31 | kep 2 `ZAJ.BMP`, radar noise | → (522, 362), 100 × 100 | 4 |

- `ColonyGfx2D_1` 0x4e3150 (exe 0x461190, checked): `ShowBuildings` (§4.1), landing zones, the build grid, building
  bases/selection, `WF::WriteStuff`, the select menus' faders, then the radar: `BrushToScreen(13)`, then
  `CopyToScreen(28)` (or noise `BrushToScreenMelyik(31, counter % 4 + 1)` while an enemy radar jammer works and the
  human has no radar turret), then `DrawRadar(turret || owner == EN)`; then the ground-war buttons (27, 24, 25, 30,
  26, 19, 20, 21 with the strike charge bars `FillToScreen(344/377, 427, …)`).
- `ColonyGfx2D_2` 0x4e3f60: rain/sandstorm, the attack text at (200, 30), then for the human's planet not at war
  (`+0x64` bit 0 clear): `"\x01:%+d"` (workers: planet `+0x28 − +0x50`) at (4, 406) and `"\x02:%+d"` (energy:
  `+0x48 − +0x4c`) at (87, 406), 10 cells, size 3, flags 5, colour 6 when negative, else 1 (bytes 1 and 2 are the
  font's worker and energy icons); the selected building's buttons 1, 2 and 29 (the on/off switch: `+0x19` bit 5 →
  frame 2); the short info (`StarmapRovidInfoKiir(50, 424)`: the planet's, or `ColonyKisInfo`'s for a selected
  building, colony.md §10), the long one with `ColonyPlanetInfoMod`.
- `ColonyGfx2D_3` 0x4e4f00: the planet panel `BrushToScreen(12)` (the human's planet not at war, or the human not
  placing units), then for the human's planet not at war the build menu's building `AktEpuletTipus`: its name
  (`Talalmanyok[EpuletekLeiro[t].tech]`) at (54, 141), 16 cells, size 2, flags 5, colour 0x25, and at x 160, size 2,
  colour 0x25: `"$:" + GetGyartasAr(EN, tech)` at y 61, `"\x02:" + EpuletekLeiro[t].+0xc` (energy) at y 92,
  `"\x01:" + +0x10` (workers) at y 123; the BUILD button animation `Copy3(17, …)` at (59, 148), 91 × 23. The panel's
  turning building is a second 3D render (`Update3D` into the panel's viewport, colony.md §10); then the repair and
  demolish windows (colony.md §10).
- `ColonyGfx2D_4` 0x4e5680: the self-destruct countdown, the rename field, the pick list's and the drop-downs'
  texts, the windows' texts (colony.md §10).

### 3.1 The radar (`DrawRadar(bool)` 0x4cb220)
The radar is the 100 × 100 square at (522, 362), half a grid cell per pixel (grid 0..200):
- buildings (non-empty slots, except finished kinds 0x20..=0x23): `FillToScreen(gx/2 + 522, gy/2 + 362, 2 × 2)` in
  colour 1, or 2 when `+0x19` bit 0 is set, blinking (`AktIdo100Szazalek`) for the selected one; not while jammed;
- units, when the human sees inside (its planet, satellites, or a battle there): a pixel at `GetGPos/2 + (522,
  362)`, green for the human's, red for others, blinking when selected (unit `+400` bit 3);
- the view: the screen corners (±320, −220) and (±320, 240) around (320, 240) cast to the ground (`get_cradarpos`
  0x4cb1a0 = `SikMetszespont` on h = 0), mapped like the terrain's height cells: `x = 2·(49 − ((wx + S/2)/S·50·49)/50)
  + 522`, `y = 2·(((wy + S/2)/S·50·49)/50) + 362` (S = 8192), four white lines clipped to 522..621 × 362..461.
So radar x 522 is world x = +S/2 (grid x 0) and radar y 362 is world y = −S/2.

The picture under it is kep 1 (`FUV6.BMP`, a static grass map) in the exe's code, but the captures show the shown
planet's own terrain from above [obs: quick-1.sav planet 42's radar is the brown of its ground; how the GOG build
fills kep 1 is open, unk]. `Init3dColony` builds a 100 × 100 `FinalRadar` from `maptexture` (mirrored in x) that
nothing in the ELF reads.

## 4. Names
### 4.1 Buildings (`ShowBuildings` 0x4c9220)
With `KellEpuletnevKiiras` set and the human seeing inside (`WF+0xa1b`, colony.md §3): for each building slot, the
name `Talalmanyok[EpuletekLeiro[kind].tech].name` (the mafia's name `MaffiaNev[tech − 0x51]` for a flagged
`+0x19` bit 4 building whose tech is 0x52..=0x90), at the point `(+0x14, Grid2Real(x + (w odd)·0.5, y − (h odd)·0.5))`
(`+0x14` = the building's placed height, colony.md §3.1) through the camera; only in front (`z < 0`), faded by depth:
`KiirStrSzinSzorzo` = 100 nearer than 2300, 0 beyond 2800, `((z + 2300)/500 + 1)·100` between; written at
`(x − 4, y)`, size 2, flags 0xf (centred on x, clipped), colour 2.

The names are the techs' localised ones: the game overwrites `Talalmanyok[t]` bytes 0..15 at start-up
(rules/tables.md); `txt17.txt` section `8 + (t − 1)/48`, index `(t − 1) % 48 + 1` holds them [inf: equals the exe's
built-in English names for 185 of the 192 techs; the rest are unnamed slots].

### 4.2 Planets (`Show_Stars2d3d` 0x6e22e0, in `StarmapGfx2D`)
For each planet the human sees (`+0xd8 + 2·EN > 0`): its point `(0, x, y)` through the starmap camera; in front
only. Its name is written when it has an owner (or story flag `StoryAdat+0x4c`, the tutorial, or the blink flag
`+0xba`): at `(x, y + (int)sqrt(Zoom/5))`, 0x14 bytes, size 1, flags 0xf, colour `owner + 15`; blinking
(`AktIdo300Szazalek`) with `+0xba`; faded from `StarmapZoom` 590 (`100 − (Zoom − 590)/9.1·100`). A planet with
`+0xb8` gets the animated marker `StarmapBrushok` 4 at (x − 13, y − 13). Then the visible wormholes' arrows.

### 4.3 Fleets (`Show_Flottak2d` 0x6e3d30)
For each fleet on the map (owner ≠ −1, seen by the human or always visible, `+0x52` bit 10 clear) in front: the
sprite (starmap.md §4.2), then, when `+0x52` bit 11 is set (in MP only the human's own), its name `+0` twice:
colour 0 at `(x + 1, y + 10)` (the shadow) and `owner + 15` at `(x + 1, y + 9)`, 0x14 bytes, size 1, flags 0xf. The
debug build adds the fleet's numbers under it. On the starmap, fleets with `+0x52` bit 2 or 7 get the animated
marker `StarmapBrushok` 6 at (x − 15, y − 13).

## 5. Not specified here
The screen-select menu and tooltips, the long planet info (`StarmapHosszuInfoKiir`), the ground-war buttons'
conditions in detail, faders' look
(`KirakFader`, `FadeKerettelKerekit`), the starmap's own panels (`StarmapGfx2D_1..4`, `kezel.xvb`), input.

## 6. Reference captures
`tools/trace/run_screen.py <install> quick-1.sav <out> --kep 2 --slot 91` (the starmap; the clock stays paused, so
the pause button is the selected one) and `--kep 3 --planet 42` (re/screens/colony.md §8), compared with
`ig2-client --snapshot … --size 800x600` [obs, 2026-10-16]: the planet names TRA 2, TRA 5, TRA 8, MAGELLAN 8,
MAGELLAN 10, ANDROMEDA 8 cover the same pixels (bounding boxes within 1 pixel), in the same owner colours; the
money "$109090", the year/month/day digits and the bar's boxes sit on the same columns; on planet 42 the panel's
"$ 10000", energy 0, workers 500 lines, the "+4895" / "+1700" surplus line, the demolish and on/off buttons, the
radar frame, its building dots, unit dots and view outline line up. Captures hold game imagery: `_local/snapshots/`,
never committed.
