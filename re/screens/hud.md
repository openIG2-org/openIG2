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
