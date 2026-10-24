# The info screen: charts and race names

How the original draws the info screen (`AktKep == 9`). Evidence: the ELF oracle decompiled with
`tools/ghidra/DecompileFunctions.java` (addresses ELF unless marked exe), exe tables found by their bytes, and a
reference capture of ig2.exe (`tools/trace/run_screen.py … --kep 9`). The data behind it — the ten statistics charts
— is a sim system, specified and verified in `re/rules/info-messages.md` §1 (`ig2-sim::info`). `[inf]` marks
inference.

Scope: the chart and the race-name row (§2–§3), the 3D frame (§1.1), the empire panel, the spending limit, the money
triangle's texts and the lines (§4), the hot-spots and input (§5), the planet/fleet list with its rows, faders, globes
and scrolling (§6), the detail mode (§6.6), the slots' matrix swap (§6.7), the star log (§6.8) and the orders: the
planet settings menus, the money triangle and the spending limit (§7).

## 1. Frame structure
- `Info_Belep` 0x563fc0: `Info_Bir1/2` = 1, 2, chart mode on (`Info_ChartMode` = 1, `Info_SelectedChart` = 0,
  `Info_ChartFelbontasSelected` = 0), `InfoSelected = EN`, `Infoing3DInit`, the chart sweeps
  `InfoChartSzamlalok[k] = 0` with speeds `InfoChartSzamlaloAdderek[k] = rand() % 400 + 200` (10 libc draws, UI-only),
  the opening animation unless coming back from the messages/help/options screens.
- Per frame: `Infoing3DUpdate` 0x559980 (3D frame, `GFX_DateMoneyScrollFade`, `Update3D`), the 2D passes
  `InfoGfx2D_1..3`: `_2` 0x556500 = planet/fleet list, star log, **`Info_PutChart(Info_SelectedChart)`**, the empire
  panel; `_3` 0x556590 = **`KirakFajNeveket(0x28, 0x75)`** (and debug tables behind `InfoKellenekOldDolgok`).
- `FRISSIT_Info2` 0x5686f0 (UI tick, 10 ms): `InfoVillogtato += 0.04` (the chart marker's blink), the list's motion,
  `InfoHaromszogNovelo` +1 up to 100, `infoFelfadeles` +1 up to 100 once open (the texts fade in over 1 s), the chart
  sweeps `InfoChartSzamlalok[k] = ((int)(infokeringeto·adder[k]) % 55 << 1) − 27` with `infokeringeto += 0.0006`;
  opening `infoframe += 0.3` up to 45 (`infoMAXFRAMES`, ELF 0x117abac), the star log opens the frame on to 75 and
  back at 0.3 a frame, closing −1.2.

### 1.1 The 3D frame (`Infoing3DInit` 0x555b50, `Infoing3DUpdate` 0x559980)
- Projection: `xcenter` 320, `ycenter` 240, the focal 600 (`kamerapont.z`): `vx = 600·x/w + 320`, `vy = 600·y/w + 240`
  (w = −z).
- Lights (the engine's per-vertex model, research.md §2.1; exe `Infoing3DInit` 0x4ea8a0 matches the ELF): on
  `mainmeshid` (every node) ambient 0x1e1e1e, parallel 0x9e9e9e from (0, 0, −10) and the side light 0x004e7f
  (`InfoMainLight4`); on `InfoMainMesh` (the frame only, not the chart marker or the triangle's knob) the side light
  0x7f5d00 (`InfoMainLight3`). Both side lights (`infoOldalsoFenyekBe` 0x554dc0, created at (200, 0, −650)) move
  every frame (`infoMozgatMainFenyek` 0x554e80, inlined at exe 0x4ef450 into `Infoing3DUpdate`) to
  `(sin a·300, cos a·200, −650)`, `a = infokeringeto·20` for light 3 and `a + 1.57075` for light 4. `infokeringeto`
  (ELF 0x14132b0, exe 0xB85304) gains 0.0006 a UI frame (`FRISSIT_Info2`) and is never reset (it also drives the
  chart sweeps); the capture tool records it (`info_turn` in `<state>.ui.json`, the client's `--info-turn`). With the
  main light the lit faces saturate at 255 (texture as is), so the side lights' phase barely shows [obs: no pixel
  changed with their x or y mirrored]. The exe's ported names for the side lights are wrong: exe 0x4c89e0
  ("`infoOldalsoFenyekBe`") is the diplomacy screen's (`Dip_MainLight3/4` 0xABA91C/0xABA930, moved in 0x4be2d0).
- The heads' marks (`RarakFenyekFejekre` 0x554790, every frame after `Update3D`; `RarakFenytFejre` 0x554570 adds an
  ambient light to `InfoFejek[e]` = node 3/ring/1/0, slot `s` at `InfoFejek + 0x3c·e + 0xc + 0xc·s`, flag at
  `+8 + s`): slot 0 is removed from every head and put back on `InfoSelected` in grey `(int)(sin(t·20)·127/2 + 192)`,
  `t` = the heads' fader clock `InfoFejF+0x4d8` (`InitFaderDesignStruktura` in `Info_Belep`, +0.003 a frame in
  `FaderDesignFrissito` 0x547e10); slot 1, on every other head and never removed while the screen is open, is red
  (200, 50, 50) for a dead empire, else `(100 − x, 100 − x/2, 100 + x)`, `x = (int)(InfoFejF[0x168 + 4e]·100) − 89`
  clamped to 0..100 (the fader's `s` = 0.9 + its random shine: x = 0 with the ELF's f64 product [inf: the exe's f32
  product would give 1]). Alive heads that were selected at some point keep their slot 1 too. With
  `GLOBALKELLDESIGN` the shining heads also get two `FadeKerettelKerekitDesign` frames [not ported].
- `info.xvb` (`vector/info/`, `textures/info/`) `AddToScene(…, 1.22)`, `RotateObj(π/2, 0, 0)`, `MoveObj(0, 15, −650)`,
  property 0x200, posed by `SetFrame(InfoMainMesh, infoframe)`. Hidden: node 1's children 0xe..0x13 (init) and
  `GombMeshek[11]` = 1/7/0/5 (`InfoShowSelectedGomb` 0x554190).
- Race heads: for `k` 0..8, node 3 / `InfoMeshSorrend[k]` (ELF 0x117abb0: 7, 5, 4, 2, 3, 1, 6, 8) / 1 / 0 gets
  `textures/diplomac/<PlayerFaj[k + 1]>a.bmp` (`InfoTexturaNevek` 1a..8a), 1 % transparent; `UnhideIsmertFajFejeket`
  0x557870 shows 3/ring/1 only for the empires the human knows.
- The chart marker `negyzet1.xvb` ×0.031 at the centre of the selected chart button's mesh (`GombMeshek` ELF 0x117aca0,
  index `Info_SelectedChart`, +1 above 4: 1/1..1/5 then 1/7/0/6, 1/7/0/1..4) moved by (−1, −3), transparency
  `(sin(InfoVillogtato) + 1)/4`, hidden in star-log mode. `Infoing3DInit` turns its properties 8, 4 and 2 off: no
  depth test or write and additive blending (render.md §2.3), so `fa.bmp`'s black inside adds nothing and only its
  blue rim shows [obs: drawn alpha-blended the port covered the selected button with a black square; `buttons` 7.56
  → 5.02].
- The triangle's knob `hszog0.xvb` ×0.03 at `(InfoTriX·4.5 − 1438, InfoTriY·4.5 − 1050, −2650)`, transparency
  `1 − infoframe/45`; `Info_InitializeTriangleToMidPosition` 0x55e5d0 puts it at (319, `(int)(50 + 2√2700/5.6)`).
- Under the frame `Copy3hatterkep(0x34, 0xff − infoframe/45·128)`: picture `0x33 + n` is `SaveScreen(n)` (0x5deb20,
  a copy of the screen); black only after a bare jump, otherwise the starmap without HUD (see [`overlay-background.md`](overlay-background.md)).

## 2. The chart (`Info_PutChart` 0x55dbf0)
Drawn only while the screen is open (`infoKINYITVA`, not closing) and the opening is done or in its last 15 frames.
For chart `c` (0..=9, `re/rules/info-messages.md` §1.2: income, population, planets, space strength, ground strength,
research, spies, battles won, diplomacy, aggregate; titles `GetString(9, 1, 5 + c)`), on the 640 × 480 virtual screen:
1. `peak` = the largest `InfoChartsSmoothed[c][e − 1][4..=50]` over the empires `e` = 1..=8 the human knows
   (`BirodalomIsmeri[EN][e]`; the human knows itself); `scale = peak / 153`, 1 when 0.
2. For each known `e` and `k` = 4..`n − 1` (`n` = 50, fewer during the opening's last frames): a line from
   `(4k + 385, (int)(426 − v_k / scale))` to `(4(k + 1) + 385, (int)(426 − v_{k+1} / scale))` (`InfoTscVonal`
   0x559af0 → `line2d`), `v` = the smoothed cell (`+ Info_ChartFelbontasSelected` cells, 0 here).
3. Colour `DDColor[e + 15]` times `b = clamp(1 − |InfoChartSzamlalok[e − 1] − k| / 55 · 3, 0.1, 1)`: a bright spot
   running along each row (`FRISSIT_Info2` advances the counters [inf]).
So the chart spans x 401..585 and y 273..426 (values ≤ 0 sit on the bottom line; −1 = no data yet). The capture shows
the chart inside the frame at about (388..612, 272..452).

## 3. Race names (`KirakFajNeveket(x, y)` 0x563cf0, called with (40, 117))
For `e` = 1..=8 known by the human: `Faj_Nev[e]` (`GetString(0x11, 1, race + 5)`, `StringReadAll`) at
`(FajnevX[e − 1], 117)` (exe 0x90B6BC: 41, 97, 157, 214, 382, 439, 496, 552; `ig2-rules::race_name_x`), size 2,
colour by the relation `Viszony[InfoSelected][e].+0x25`: 1 (war) → 6, 2 → 3, 4 → 0x29, 5 → 7, else 1; `InfoSelected`
itself 2 with a frame (`DesignKeret1`), a dead empire 0xc. In MP the player names go 0x14 above (`PlayernevX`).
Verified on the capture (quick-1.sav: Shinari red on column 3, the human's Godan white and framed on column 6).

## 4. The panels (`InfoGfx2D_2` 0x556500)
All drawn only while open and not closing, the texts times `infoFelfadeles`.
- **Empire panel** (`KirakBirodalomInfo` 0x559df0) for `InfoSelected`, labels `GetString(9, 3, k) + ":"` at x 180
  (size 2, colour 1, 10 bytes), values at x 240 (colour 2), rows from y 150 by 15: `(9,3,2)` the empire's name
  (`Birodalom_Nev`); `(9,3,5)` + `sprintf((9,3,9), planets, fleets)` = the planets the human sees of that empire
  (`+0xd8 + 2·EN` > 0) and its fleets the human sees (`+0x19 + EN` & 1); for the human only: `(9,3,3)` +
  `(9,3,6) ": %d - " (9,3,7) ", %d - " (9,3,8)` with `TradeIncomeLastYear`, `TaxIncomeLastYear`, and on the next
  row `(9,3,4) ": %d"` with its planets' `+0x44` summed (indices from the asm: the ELF decompile drops `GetString`'s
  last argument).
- **Spending limit** (`PutLimitCucc` 0x5683b0, the human only): `(9,3,1) ":"` at (180, 210), `"$" + MoneyHatar[EN][0]`
  at (386, 210) size 2; 48 ticks at x `c + 282` (c = 1, 3, …, 95), y 210..215 (the end ticks 2 shorter), colour 0x25
  below `((int)(ln(limit/10500 + 1)/3·200000)/2000·100)/100`, else 0xc.
- **The money triangle** (`PutHaromszog` 0x55a8b0): size 1, colour 2: `(9,3,10)` at (290, 127), `(9,3,11)` at
  (269, 40), `(9,3,12)` at (325, 40), the shares `MoneyTeruletSzorzo[EN][1..=3]·100` + "%" at (310, 118), (243, 40),
  (380, 40) (blinking with `AktIdo300Szazalek` while `InfoHaromszogNovelo` < 100).
- **Lines** (`MeghuzIzeVonalak` 0x55c7c0, `InfoTscVonal` → `line2d`): the four list rows' top and bottom (x 29..260,
  y `249 + 55k` and `301 + 55k`) in 0x322200, times the fade; under each race column a line from `FajnevX[e−1]` to +54
  at y 107, black → `DDColor[e + 15]` → black; the triangle's drag trail. `line2d` takes the colour words as
  0x00BBGGRR (as `DDColor` holds them): the row lines are (0, 34, 50) on the capture, one screen pixel high on row
  `⌊y·k⌋`, the vertex colours interpolated in the gamma pass (the capture shows the bytes as given: the race lines peak
  at the race colour itself).
- **Race names** (§3): the colour follows `Viszony[InfoSelected][e]`; the selected one is white in `DesignKeret1(x − 5,
  114, 0x36, 0xc, 2)` (a morphing gradient box; the port draws its settled outline [inf]).

## 5. Hot-spots and input (`Info_MouseInit` 0x5667b0, `Info_Main` 0x5647f0)
`Mouse_Szoveg_Set(id, 0, x, y, w, h, tooltip)` in id order (tooltips `GetString(9, 1, k)`): 1..=8 the race columns
`(FajnevX − 2, 48, 50, 79)`; 9..=12 the list rows `(60, 249 + 56k, 200, 53)`; 13 the list `(60, 249, 200, 56·min(n, 4))`;
14 charts `(588, 430, 45, 20)`; 15 star log `(350, 430, 45, 20)`; 16..=20 charts 0..4 `(350, 283 + 27k, 45, 20)`,
21..=25 charts 5..9 `(588, 283 + 27k, 45, 20)`; 26..=28 tax / auto-build / priority `(135, 354 + 14k, 100, 13)`; 29 the
triangle; 30/31 list up/down `(290, 247 / 450, 18, 19)`; 32 the limit `(280, 209, 100, 8)`; 33 `(0, 249, 55, …)`; 34 the
scroll bar `(290, 266, 18, 203)`; 35/36 star log up/down `(621, 274 / 431, 18, 19)`; 37 back from the detail.
Heights (`Info_MouseInit` exe 0x4e9900, `Info_MouseInit2` 0x4e9530 every frame): 13 and 37 are a fixed 224 (13 always
off, 37 only in detail mode); 33 is `56·min(HanyBolygoFlotta, 4)` (0xB82988), on only outside the detail mode while
open. Tooltips (`GetString(9, 1, k)`, drawn by `UpdateFrame` as every screen's, input.md §8): 1..=8 the race's name;
9..=12 k 1; 13 k 2; 14..=31 k = id − 11 (15 in chart mode 0x1a); 32 none; 33 k 21 (27 when `CheckMelyikFolottVan()`
is 0 [u]); 34..=37 k = id − 12 [obs: 16 → "Teljes bevétel", exe and client alike, `info --info-mouse 370,290`].
`Mouse_Megfogva` is set by 26..=28 (a menu open or held), 29 (`Lenyom && Bal`), 32 (every frame it is `AktIkon2`) and
34: while it holds, passing over a race column doesn't switch the empire.
`Info_Main`: the mouse **over** 1..=8 selects that empire (`InfoSelected`, no click needed); a press on 14 / 15
switches chart / star-log mode; 16..=25 picks `Info_SelectedChart = id − 16` (the button animates, sound 0);
9..=12 open a row's detail (`Info_NagyInfoMode`); 26..=28 open the select menus and send `Multi_Send(0x19, …)`
(planet settings 8/9/10); 29 drags the triangle and sends 0x5b (the three shares); 32 sends 0x5c (the limit). §7
has these orders. `AktIkon2` is the hot-spot under the pointer whether a button is down or not (`Mouse_Darabol`
ELF 0x5b7a40 in `IG2_gameUpdate`), except while `Mouse_Megfogva` (exe 0xDB466A) holds the last one: a handler sets
it, `IG2_gameUpdate` clears it once no button is down (after picking, so the release frame still sees the held
hot-spot with `Mouse_Felenged`).

## 6. The planet/fleet list
`GFX_Info` (exe 0x4e5c30, ELF 0x564580) runs every frame: `FeltoltBolygoFlottaTomb` (exe 0x4e5a20, ELF 0x55efd0: the
exe rebuilds the list every frame, the ELF only in `Info_Belep`), `PutFaders` (exe 0x4ed410, ELF 0x55b2c0), then
`Infoing3DUpdate` (exe 0x4eb020). The exe's helpers are unnamed in the ported table: `KirakBolygoFlottakat` exe 0x4eb700
calls `KirakBolygoInfo` exe 0x4eb840, `KirakFlottaInfo` exe 0x4ebfc0, `VanNagyProblema` exe 0x4ef3f0,
`RarakPirosFenytBolygora` exe 0x4ee5d0 and `LeszedPirosFenytBolygorol` exe 0x4ecb10; `Infoing3DUpdate` calls
`RotateBolygoFlottak` exe 0x4ee750 and `KisGolyokAtlatszosaga` exe 0x4ec820 (all checked against the ELF's).

### 6.1 The list (`FeltoltBolygoFlottaTomb`)
`BolygoFlottaTomb` (exe 0xB8298C, `[kind, index]` × `HanyBolygoFlotta` 0xB82988): first the planets 1..=`HanyBolygo`
owned by `InfoSelected` (exe 0xB79FB4), then the fleets 1..=`OsszHanyFlotta` of `InfoSelected` without flag `+0x52 &
0x20`; for another empire than the human only those the human sees (planet `+0xd8 + 2·EN` > 0; fleet `+0x19 + EN` ≠ 0
in the exe, `& 1` in the ELF), or all while the tutorial runs (exe 0xF6915D / 0xF66F2A). `BolygoFlottaPointer` (exe
0xB82984) is the first row shown; 0 on entry and when the mouse picks another empire (which also reloads the fleet
models with `LoadBolygoUrhajok(InfoSelected)`).

### 6.2 The rows (`KirakBolygoFlottakat(70, 249)`, `InfoGfx2D_2`)
Only while open and not closing, the texts times `infoFelfadeles`. For row `k` = 0..3 with `k + pointer < n`, at
`y = 249 + 56k` (`KiirStrAlap(s, 70, y + dy, 0x13, size, 0xc, colour)`: 19 characters, left-aligned):
- **A planet** (`KirakBolygoInfo`): first `VanNagyProblema(p)` (below) puts the red light on slot `k` or takes it off.
  dy 3: the name (`+0`), size 2, colour 0x22; dy 15: `BirodalomNev[owner]`, size 1, colour 0x23; dy 24 (a free planet:
  `FajBolygoViszony_Nev + FajBolygoViszony[race][surface] + "% "`, unreachable in the list) for the human's planet, or
  race 7 seeing a planet without `+0xcb`, or a satellite (`+0xc4 + EN`), or `DEBUGX`: `GetString(2, 3, 1) + ": %d
  (%+d)"` with the population `+0x28` and its growth `+0x2c`. For the human's own planets only: dy 33 the morale or a
  problem, dy 42 the production (colour 1), as the starmap's short info (`StarmapRovidInfoKiir`, starmap.md §11):
  `GetString(2, 3, 2) + ": %s"` with `Moral_Nev[+0x30 / 100]` at step 0, else `"%d%% %s"` with problem
  `InfoProblemaRendezo[h]`'s `+0x118 + 4i` / 10000 and `Problemak_Nev[i]`; the production line shows queue head 0
  (`+0x78`) or 1 (`+0x80`), alternating every 30 `Counter` steps when both are set: `"%s - %d%%"` (then `"%s-%d%%"`,
  `"%s%d%%"` above 19 characters) with the design's name and `100 − (+0xa0 + 4l)·100 / design +0x20`.
- The problem step `h` is one static of the function (exe 0xB85E88, with the last `Counter` 0xB85E84 and the planet
  0xB85E8C it was reset for: a new `AktPlanet` resets it to 0): every 30 `Counter` (exe 0xBAE05C; +1 per 50 ms tick
  unless the screen pauses the clock for its opening, so it runs while the game is paused) it steps on and wraps after
  `InfoProblemaHanyKiirt`. All rows share it, so a row whose planet has fewer problems than `h` resets it to 0 for
  itself and the rows below [quirk, kept].
- `InfoProblemaRendez(p)` (exe 0x4e75c0, ELF 0x55f450): the problems `i` = 1..=19 at 100000 or more, sorted worst
  first into `InfoProblemaRendezo[1..=4]` (exe 0xB84C20; count `InfoProblemaHanyKiirt` 0xB84C18), as
  `StarmapProblemaRendez`; slots past the count keep the last planet's entries.
- `VanNagyProblema(p)`: true when `InfoProblemaRendezo[1]`'s problem is above 50 % (`/10000 > 0x32`; with no problems
  that slot is the previous planet's [quirk]) or the morale `+0x30 / 100` is below 2.
- **A fleet** (`KirakFlottaInfo`): colours by `+0x53`: bit 2 → name and lines 6, bit 6 → 1, else the name 3 and the
  lines 0x21. dy 3 the name (size 2); dy 15 `BirodalomNev[owner]`; for the human's fleet (or `DEBUGX`) dy 24
  `GetString(2, 3, 3)` when idle (`+0x40` = 0), else `GetString(2, 3, 5 | 6)` (move / attack) + `":%s"` with the
  target (`+0x42`: 0 `GetString(2, 3, 4)`, 1 the planet `(int)+0x44`'s name, 2 that fleet's name); for the human's
  fleet or a radar level (`GetENRadarKifejlesztve`) above 1: dy 33 `(2,3,7) ":%d " (2,3,8) ":%d " (2,3,9) ":%d/%d"`
  with ships − fighters (`+0x54 − +0x58`), fighters, vehicles `+0x5c` and capacity `+0x78` (above 19 characters
  `(2,3,10..12)`, then without the capacity); above 2: dy 42 `GetString(2, 3, 13) + ": %d"` with `(int)(+0x38·100 + 0.5)`.
- In detail mode (`Info_NagyInfoMode`, a click on a row) `InfoNagyKiirBolygo`/`InfoNagyKiirFlotta` replace the rows
  (§6.6); `VanNagyProblema` isn't called then, so the slots' red lights stay as the rows left them.

### 6.3 The faders under the frame (`PutFaders`)
Queued with `FaderHanyadikMenet` 1, so `PutDarkening(1)` (the null box `fader1` added before `info.xvb`) draws them
under the frame, whose panel textures are transparent inside (re/screens/hud.md §1.2 for the drawing). From
`infoframe ≥ infoMAXFRAMES − 15`; `t = (int)((1 − infoFelfadeles/100)·255)` (0 once open: opaque), every fill textured
by `dt.bmp` at 0.005 a pixel with the offsets given (`c` = the fader clock `infof+0x4d8`, exe 0xB85160, +0.003 a frame
from `Info_Belep`, like the heads' `InfoFejF`):
- The chart: `KirakFader(0, infof, 387, 271, 226 − d, 183, (0, 54, 90) t 100, edge (0, 90, 130) t 30)`, `d` = the
  star log's slide (0 open).
- Per list row `k` = 0..3 (`y0 = 55k`, `x0 = (int)(o + 15)`, `o` = 0 open): `FadeKerekit(x0, y0 + 248, 271, 53, (0, 34,
  50), t)` with offsets (0, 20c), `FadeKerekit(x0 − 8, y0 + 252, 10, 45, …)` (20c, 20c), `FadeKerekit(x0 − 15, y0 + 262,
  9, 25, …)` (20c, 20c); in detail mode the first is 58 high (53 for row 3, at y 413) and the offsets (20c, 0).
- The empire panel: `FadeKerekit(157, 146, 330, 83)`, `(135, 161, 26, 62)`, `(483, 161, 19, 62)`, `(498, 166, 8, 52)`,
  (0, 34, 50), t, offsets (0, 18c).
- The list's scroll bar: `FadeKerettelKerekit(295, 267, 10, 180, (0, 54, 90) t 100, (0, 90, 130) t 30)` and its thumb
  `(296, (int)(a + 267), 8, (int)b)` the same: `a` = 1, `b` = 178 for up to 4 entries, else `b = 4/n·180`,
  `a = 180/n·pointer + 1`.
- The human only: the limit `KirakFader(1, infof, 280, 209, 100, 8, (0, 34, 50) t 155f + 100, (0, 60, 100) t 225f +
  30)` (`f = 1 − infoFelfadeles/100`); in detail mode on the human's planet three boxes (133, 354/368/382, 100, 13) for
  the selectors; an open select menu's box (layer 2).

### 6.4 The globes (`LoadBolygoUrhajok(1)`, `RotateBolygoFlottak`)
- `LoadBolygoUrhajok(e)` (exe 0x4ecb40, ELF 0x555070, from `Infoing3DInit` with e = 1): for slot `k` = 0..3 the planet
  `vector/info/bolygo_k.xvb` `AddToScene(…, 0.029)` and the fleet `vector/spacewar/flag<e>0.xvb` ×0.025, both
  `SetRelative(false)`, `RotateObj(π/2, 0, 0)`, at `(−310, 59k + 40, −650)` (`InfoBF` exe 0xB85B90, 0xB0 per slot
  (ELF 0x1412890, 0xB8): matrices `+0x30` / `+0x54` (3 × 3, from `GetObjMatrix`), spin `+0x78` = `((rand() % 10) +
  1)/100 + 0.01` (one libc draw per slot, UI-only), position `+0x7c`, home `+0x88`, target `+0xa0`, steps `+0xac`).
- `InfoLoadBolygoTexturak` (exe 0x4e64c0): `BolygoTexturak[i]` = `textures/starmap/p<i+1>.bmp` (i = 0..7; the ninth
  is `p1.bmp` again).
- The models as the renderer draws them [obs, the detail and fleet captures]: `flag10.xvb` is only the head of a
  level-of-detail chain (`flag11` → `flag12`, the cruiser `cirkalv04` + `Object05` in `cirk1`/`cirk3`), drawn at its
  last level and scaled by the head's half-extents (as the starmap's planets, starmap.md §3.3); the faces turned away
  are culled (a translucent globe shows no back hemisphere); the lighting normals carry the child record's scale
  (`flag12` 2.84, `bolygo_k` 0.85) and light by their direction only.
- `RotateBolygoFlottak` (every frame in `Infoing3DUpdate`): hides every slot; while open, not closing and without the
  debug tables, for each shown row `k` sets the slot's model matrix (`SetObjMatrix`, the rotation after the scale),
  unhides it (in detail mode only the selected one, or while it fades), gives a planet's child 0 the texture
  `BolygoTexturak[surface +0x22 − 1]`, its transparency (0 outside the detail mode's fade) and `MoveObj` to `+0x7c`.
- `RotateBolygoFlottak2` (ELF 0x558d60, every UI frame in `FRISSIT_Info2`): `matrixforgat(m, m, spin/8, 0)` for each
  shown row's model: `m = axis(0, −1, 0)(spin/8)·m` (`creatematrixfromaxis`).
- `KellMatrixCsere` (set by the list arrows) swaps the slots' matrices in `AtteszMatrixokat` (§6.7); the detail mode
  slides the selected globe to (−43, 28, −150) (§6.6).
- Lights: the globes hang on `mainmeshid` (the ambient, the parallel and the blue side light, §1.1), not on
  `InfoMainMesh`. `RarakPirosFenytBolygora(k)` adds an ambient light `((int)((sin(InfoVillogtato) + 1)·150·0.5 + 14),
  14, 14)` to slot `k`'s planet (created once, then recoloured), `LeszedPirosFenytBolygorol` removes it.
- `KisGolyokAtlatszosaga`: the frame's list discs `1/10..13/0` (`racsszin` cylinders under the globes) are hidden from
  `infoframe ≥ infoMAXFRAMES` (transparency 1); below they are 0.7 (and `0.7 + (f − 30)/15/3.3333` in the last 15
  frames) transparent.

### 6.5 Scrolling (`Info_Main`)
Hot-spot 30 (up) / 31 (down) on a press: `pointer − 1` if > 0 / `pointer + 1` if `< n − 4` (detail mode: the design
list instead), `KellMatrixCsere` with the direction, sound 0, and the arrow's mesh `GombMeshek[12/13]` (1/9, 1/0)
animates; 34 (the bar) held: `pointer = ((y − 266 − s/2)·(n − 4) + (180 − s)/2) / (180 − s)` with `s = 720/n`, clamped
to 0..n − 4 (0 when `n < 5`).

### 6.6 The detail mode (`Info_NagyInfoMode`)
Exe globals (from `Info_Main` exe 0x4e81c0, ELF 0x5647f0): `Info_NagyInfoMode` 0xB79FB1, `Info_KilepesNagyinforol`
0xB85258, `InfoBolygoFlottaSelected` 0xB82980 (the list entry), `InfoNKivalasztott` 0xB852B8 (its row slot),
`Info_NagyInfoKivalasztott` 0xB852C0 / `…Tip` 0xB852BC (the entry's index and kind), `InfoMaxTervekPtr` 0xB84C08,
`InfoMaxTervek` 0xB84C0C. `Info_MouseInit2` (exe 0x4e9530, ELF 0x567ce0) switches the hot-spots every frame: in
detail mode 9..=12 off and 37 on (the list's rectangle), else 37 off and `9 + k` on for the rows shown; 26..=28 (the
selectors) only in detail mode on the human's own planet.
- **Enter** (`Info_Main`, a press on 9..=12 while `Info_KilepesNagyinforol` is clear): mode on, `InfoMaxTervekPtr = 0`,
  entry `id − 9 + BolygoFlottaPointer`, slot `s = id − 9`, the entry's index and kind remembered; slot `s`'s target
  `+0xa0` = (−43, 28, −150) (0xc22c0000, 0x41e00000, 0xc3160000), `+0x94` = that target, steps `+0xac` = (target −
  position `+0x7c`)/40 per axis.
- **Back** (hot-spot 37 pressed, `Info_KilepesNagyinforol` clear): that flag set, target = home `+0x88`, steps =
  (home − position)/40. Hot-spot 13 pressed clears the mode at once (`Info_MouseInit2` switches 13 off: unreachable).
  Picking another empire (1..=8) clears it too (the list restarts, `LoadBolygoUrhajok` resets the slots).
- **Motion** (`FRISSIT_Info2`, every 10 ms UI frame, before `RotateBolygoFlottak2`): if the mode is on and the entry
  `InfoBolygoFlottaSelected` of the list no longer holds the remembered index and kind (and no screen change is
  running), the mode and the back flag clear and slot `s` snaps home (target and position = `+0x88`). Then every slot
  moves per axis: a position above its target steps (`+= step`) and stops at the target; one below steps and stops at
  the target, and on the x axis that arrival also clears the mode and the back flag (the globe is home: the list is
  back). Equal positions don't move. All in f32.
- **The globes** (`RotateBolygoFlottak` ELF 0x558580 / exe 0x4ee750): in detail mode the selected slot's transparency
  is `f − 0.3` (at least 0.01) with `f = (x − home)/(target − home)` while its x target lies right of home (going
  out), else `1 − (start − x)/(start − home)` (`start` = `+0x94`, coming back); every other slot takes `f` of the
  selected slot without the −0.3 and is shown only while `f < 0.99`. So the selected globe settles at (−43, 28,
  −150), 70 % transparent (`SetObjTransparency(0.7)`), the others hidden [obs: solving `exe = a·globe + (1 − a)·faders`
  per pixel against the port's renders without and with the globe gives a = 0.28..0.30]. Outside the detail mode the
  transparency is 0.
- **The rows** (`KirakBolygoFlottakat(70, 249)`): only `InfoNagyKiirBolygo(61, 253, p)` / `InfoNagyKiirFlotta(61, 253,
  f)` for the entry; `MeghuzIzeVonalak` draws only the top and bottom lines (y 249 and 466); `PutFaders` §6.3.
- **A planet** (`InfoNagyKiirBolygo` exe 0x4e6500, ELF 0x562390; size 2, align left, rows 13 apart from y 253; labels
  `GetString(2, 2, k)` [len] at x 71 in colour 1, values at x 141; second column: label x 176, value x 229):
  1. (2,2,1) [8] the name [0x1a] colour 3.
  2. (2,2,2) [0xb] `"%s (%d%% %s)"`: `Felszin_Nev[+0x22]`, `GetBolygoGazdagsag(p)`, `Gazdagsag_Nev` [0x1a] 0x21.
  3. Owned and (the human's, `EN` = 7 without `+0xcb`, a satellite `+0xc4 + EN`, or `DEBUGX`): (2,2,5) `"%d (%+d)"`
     with `+0x28`, `+0x2c`.
  4. Owned by the human (or `DEBUGX`): (2,2,6) `Moral_Nev[+0x30 / 100]`; (2,2,7) `"%d"` of `+0x44`; (2,2,8) `+0x28`
     and (2,2,9) `+0x50` (`KiirInt`, colour 6 when `+0x28 < +0x50`, else 0x21); (2,2,10) `+0x48` and (2,2,11) `+0x4c`
     (colour 6 when `+0x48 < +0x4c`).
  5. Owned and (the human's, or (`EN` ≠ 7 or `+0xcb`) with a satellite level `+0xc4 + EN` above 1, or `DEBUGX`):
     (2,2,12) `+0x58` [0xb].
  6. The human's (or `DEBUGX`): (2,2,15..17) [0x20] on three rows, values `AdoMertek_Nev[+0x34]`,
     `AutoBuild_Nev[+0x6c]`, `BolygoPrioritas_Nev[+0x6d]` [0x10] 0x21 (the hot-spots 26..=28 lie over them).
  7. (2,2,13) and the planet's info texts in turn, as the long info (starmap.md §15.2) but with their own statics
     `oldplanet` 0xB85E98, `oldcounter` 0xB85E94, `hanyadik` 0x90B8A0 (1 initially and for a new planet): every 30
     `Counter` the next slot `h % 5 + 1` with a code at `+0x168 + 2h`; `InfoString_Get(code)` [0x1a] colour 6.
  8. The human's (or `DEBUGX`): (2,2,14) on the next row, then per sorted problem (`InfoProblemaRendez`, §6.2) a row:
     the percentage (`KiirInt(v/10000, x 141, …, 2, 2, right, 6)`), `"%"` at x 154 (colour 6), `Problemak_Nev[i]` at
     x 167 [0x20] 0x21.
  The names are `StringReadAll`'s Hungarian `txt17` tables (starmap.md §15.2): surface `(5, s)`, `Gazdagsag_Nev`
  `(5, 20)`, morale `(4, 41 + m)`, problems `(4, 21 + i)`, tax `(5, 22 + k)`, auto-build `(5, 30 + k)`, priority
  `(5, 41 + k)`.
- **A fleet** (`InfoNagyKiirFlotta` exe 0x4e6ea0, ELF 0x5602a0; labels `GetString(9, 2, k)` at x 61 [0xc] colour 1,
  values at x 137 colour 2, rows 13 apart from y 253; `r` = `GetENRadarKifejlesztve`):
  1. (9,2,1) the name [0x14].
  2. The human's, `DEBUGX` or `r` > 1: (9,2,2) `+0x54` and (9,2,3) `+0x5c` (`KiirInt` [10]).
  3. The human's, `DEBUGX` or `r` > 2: (9,2,4) `(int)(+0x38·100 + 0.5)`.
  4. The human's only: (9,2,5) `CalcSpacePower(f)` (`ig2-sim::info`); with an order (`+0x40` ≠ 0) (9,2,6) and
     `sprintf((9,2,8 | 9 | 10), …)` [0x1e] by the target kind `+0x42` (0; the planet `(int)+0x44`'s name; the fleet's);
     (9,2,7) and, starting on the same row, the designs: the ships' (`Hajok` 1..`OsszHanyHajo` of the fleet, design ≠
     0x7fff) in order of first appearance with their counts, then the vehicles' (`Tankok`, deduplicated among the
     vehicles only) with their summed `+4` counts, `"%s (%d)"` with `Tervek[InfoSelected][d]`'s name [0x18], one a row.
     It shows entries 0, 1, … while `i < 9` and `i + InfoMaxTervekPtr < n` [quirk: scrolling shortens it from the
     bottom, it never moves]; `InfoMaxTervek = n` (else kept from the last human fleet).
- In detail mode the list arrows 30/31 step `InfoMaxTervekPtr` (down while `< InfoMaxTervek − 9`), not the list; the
  scroll bar 34 is ignored.

### 6.7 The slots' matrix swap (`AtteszMatrixokat` ELF 0x558f10, exe 0x4e4a20)
The list arrows set `KellMatrixCsere` (exe 0xB8525D) and `MatrixCsereMire` (0xB8525E: 1 = down, the pointer grew; 0 =
up); `Infoing3DUpdate` (exe 0x4eb020) calls `AtteszMatrixokat(MatrixCsereMire)` before `RotateBolygoFlottak` and
clears the flag. Down: every slot `k` = 0..2 takes slot `k + 1`'s planet and fleet matrices (`+0x30`, `+0x54`) and
spin `+0x78`, slot 3 takes slot 0's old ones; up: the other way round (slot 0 takes slot 3's). So each globe keeps its
turn and spin with its entry. Positions and the red lights stay with the slots. The scroll bar (34) moves the pointer
without a swap.

### 6.8 The star log (`Info_StarLogMode` exe 0xB79FB2)
- `StarLog` exe 0xB82990 (ELF 0x1410600): 200 records of 0x2c (i32 colour `+0`, text `+8`), the newest last;
  `StarlogHanyszoveg` 0xB84BF0, `StarlogHanysorfeltoltve` 0xB84BF4 (lines filled, ≤ 200), `StarLogPointer` 0xB84C10.
  `InitializeStarLog` (ELF 0x55e540) clears it; `Info_AddStarLog(text, colour, a, b)` (ELF 0x55e850) shifts in an
  empty line and a dated line (`GetString(9, 3, …)`, colour 1), then the text with `#1`/`#2` replaced by `a`/`b`,
  word-wrapped at 33 characters, and sets the pointer to 0. It is not saved: after a load the log holds what the
  running exe added (diplomacy's `DipCallStarLog` ELF 0x50e190 and others) [the producers aren't ported].
- Hot-spot 15 (pressed, chart mode): star-log mode, `SetUresTexturak` (ELF 0x555980: the left buttons
  `GombMeshek[0..5]`' child 0 gets `textures/info/info_ures.bmp`, the sixth `info_j6.bmp`; `BaloldaliGombok` ELF
  0x117ac60 = `info_b1..6`, `info_ures`, `info_j6`), hot-spot 14 moves to (350, 430) and 15 goes off, 16..=25 off,
  35/36 on (`Info_MouseInit2`). 14 (pressed): chart mode, `SetRegiTexturak` (0x555a80: `info_b1..6` back).
- `FRISSIT_Info2`: in star-log mode and open, `infoframe` runs on from 45 to 75 by 0.3 a frame, calling
  `SetJobbAtl((infoframe − 45)/30)` (ELF 0x555750) first: the right buttons `GombMeshek[6..11]` get that
  transparency, `[11]` stays hidden; back in chart mode it runs down to 45 the same way.
- `Info_PutChart` draws only at `infoframe` 45 in chart mode, or between 45 and 60 (fading `1 − (f − 45)/15`).
- `Info_PutStarLog` (ELF 0x5645b0): from `infoframe ≥ 60` (or in star-log mode before the screen is open), the 16
  lines `c` = 0..15 of records `184 + c − StarLogPointer` at (410, 275 + 11c), size 2, colour the record's `+0`, at
  most `(int)((f − 60)·33/15)` characters, brightness `(int)((f − 60)·100/15)·infoFelfadeles/100` % (33 characters and
  100 % at 75).
- 35 / 36 (held): `StarLogPointer` +1 (older) while more than 16 lines are filled and it is below `filled − 16` / −1
  while above 0; sound 0 on a press.
- `PutFaders`: the chart's fader narrows by `d` (`(1 − (f − 60)/15)·200` from 60, `(f − 45)·200/15` between 45 and 60).

### 6.9 Capturing these states
`tools/trace/run_screen.py --kep 9 --info-press IDS` presses hot-spots through the game's input: `AktIkonKell` exe
0xB77208 = id + 0x1e (`IG2_gameUpdate` turns it into `AktIkon2`), and since that sets no press edge, the agent sets
`Mouse_Lenyom` (0xDB465F) at `Info_Main`'s entry (exe 0x4e81c0) in that frame. `--fleets N` gives the human fleets
first (fleet rows), `--info-starlog N` stages star-log lines. The grab's `<state>.ui.json` carries the list, the
slots (matrices, positions, steps), the detail mode and the star log; `run_screendiff.py … info:IDS [--fleets N]
[--starlog N]` hands them to the client (`--info-detail`, `--info-texts`, `--info-star`, `--info-log`).

## 7. The orders (`Info_Main` exe 0x4e81c0, ELF 0x5647f0)
### 7.1 The planet settings menus (hot-spots 26..=28)
On only in detail mode on the human's own planet (§6.6). Globals: `InfoSelectTax`/`InfoSelectAutoBuild`/
`InfoSelectPriority` exe 0xB79F8A/0xB79F8B/0xB79FB0 (the menu open), `infoSelectX/Y` 0x90B6FC/0x90B700 (where),
`infosmSelected` 0xB84C14 (the item under the pointer, −1 none; shared). For hot-spot `k` while its menu is open or
on a press (`Mouse_Lenyom`):
- No release (`Mouse_Felenged` clear): a menu not yet open opens at (`Mouse_x + 12`, `Mouse_y − 20`); the item is
  the last `i` with `X ≤ Mouse_x ≤ X + 120` and `Y + 13(i − f) ≤ Mouse_y < Y + 13(i − f + 1)`, over tax 0..=7,
  auto-build 0..=8 (0..=6 for race 5, `PlayerFaj[EN]`) and priority 1..=3 (`f` = 1 for the priority, else 0); the
  flag is set and `Mouse_Megfogva` holds the hot-spot.
- On the release: with an item, sound 0 and `Multi_Send(0x19, EN, BolygoFlottaTomb[InfoBolygoFlottaSelected].index,
  8 | 9 | 10, item)` (the starmap's planet settings order, starmap.md §10: tax `+0x34`, auto-build `+0x6c`, priority
  `+0x6d`); the menu closes.
- Drawing: `PutFaders` puts the box on fader pass 2 (`FaderHanyadikMenet` 2: after `InfoGfx2D_2`, so over the rows):
  `FadeKerettelKerekit(X − 6, Y − 2, 130, h, (0, 10, 0) 60, (0, 5, 0) 30)` with texture scale 0, `h` = 108 (tax),
  `(n + 1)·13 + 4` (auto-build, `n` the last mode), 43 (priority). `InfoGfx2D_3` writes the items `KiirStr(name, X,
  Y + 13(i − f) + 3, 20, 2, …, colour 6 for the item under the pointer, else 3)`: `AdoMertek_Nev`, `AutoBuild_Nev`,
  `BolygoPrioritas_Nev` (the Hungarian `txt17` tables of §6.6). Verified on captures (§7.4).

### 7.2 The money triangle (hot-spot 29)
A left press grabs it (`InfoHaromszogMegfogva` exe 0xB85259, `Mouse_Megfogva`); a right press acts once. While
grabbed or on the right press, in the exe's f32 with `h = (f32)√2700`, `top = h + 50`, `base = top − h`, corners
A (319, top), B (271.9, base), C (366.1, base) and the signed edge distances (`PontEgyenesTavolsaga` inlined, the
lengths `(f32)sqrt((f64)(a² + d²))` with `a = −(top − base)`, `d` = 47.100006 for AB/AC, `a` = −0 and `d` = 94.20001
for BC):
1. `InfoTriX/Y` (exe 0xB84C00/0xB84C04, ints) = the pointer, the old position kept; y clamps to `(int)top` above top
   and to `(int)(base + 1)` at or below base.
2. AB < 0 (left of AB): without a press or a right click, x walks right in 0.1 steps (f32) until AB ≥ 0 and is
   truncated; else the old position comes back and the move is refused. Then AC > 0 (right of AC) the same way, x
   walking left.
3. Refused: nothing is sent, the grab ends. Else a right click resets the knob to (319, `(int)(2h/5.6 + base)`), and
   `Multi_Send(0x5b, EN, s1, s2, s3)` (f64) with `s1 = BC/h`, `s2 = −AC/h/1.25`, `s3 = AB/h/1.25` (each f32
   distance widened), each below 0.03 sent as 0; the knob model moves to `(x·4.5 − 1438, y·4.5 − 1050, −2650)`.
The exe sends this every frame of a drag (the port only when the shares change). The release (`Mouse_Felenged`)
ends the grab and sets `InfoHaromszogNovelo` to 0 (`FRISSIT_Info2` +1 a UI frame up to 100; the shares blink with
`AktIdo300Szazalek` until then). `InfoTriX/Y` aren't reset by `Info_Belep`.

### 7.3 The spending limit (hot-spot 32) and the messages
On only for the human's own empire (`Info_MouseInit2`). Every frame the pointer is over it (no button needed, from
the code) `v = clamp(((Mouse_x − 280)·100)/100, 0, 100)` (int division, then f64), `l = (int)((exp(v·2000·3/200000)
− 1)·10500)/100·100`, above 15,000 `l/1000·1000`; `Multi_Send(0x5c, EN, (f64)l)`; `Mouse_Megfogva` set (a held
button keeps it while dragging off the bar).

`MULTIGET_Info` (exe 0x4ece60, ELF 0x569360; the lockstep handler of types 0x5b..0x63) applies them for issuer `e`:
- 0x5b: `MoneyTeruletCsuszka[e][1..=3]` (exe 0xBAD920 + 0x30e, f64) = the three values, `[4]` = `[5]` = 0, then
  `MoneyInit(false, e)` (exe 0x540060): `MoneyTeruletSzorzoCalculate` (every empire's shares from its sliders,
  re/rules/ai-empire.md §4), then for `e` (every empire for 0) `MoneyTeruletenkent[e][k] = Money[e] ×
  MoneyTeruletSzorzo[e][k]` and `MoneyHatar[e][k]` = 0 for k = 1..=5 (the pools re-dealt, the production and
  research reserves cleared).
- 0x5c: `MoneyHatar[e][0]` (the reserve `MoneyMennyi` keeps back) = the value.
Port: `ig2_sim::game::InfoOrder` / `Game::info_order`, `ig2_net::Command::Info`; verified byte-exact in the save
replay on 520 staged messages (re/rules/traces/2026-10-17-info-orders.md).

### 7.4 Capturing an open menu
`run_screen.py --info-select K,X,Y,I` (and `run_screendiff.py info:9 --select K,X,Y,I`) holds menu K open at (X, Y) on
item I by writing the globals after every `Info_Main` once the presses are done; the grab's `<state>.ui.json` carries
them and the client draws them (`--info-select`).

## 8. Not specified here
The opening and closing animation, the debug tables (`InfoKellenekOldDolgok`), the star log's producers
(`Info_AddStarLog`'s callers). Open
[obs]: in chart mode the exe's sixth left button (1/6, the star log's) shows a blank plate (`info_ures`'s look) where
its material is `Info_b6.bmp` (the computer) and no code found here retextures it (`Info_Belep`, `Infoing3DInit`,
`SetRegiTexturak` exe 0x4eeaf0 / `SetUresTexturak` 0x4eebc0 are only called from `Info_Main`); the port shows the
computer. The chart's fade between `infoframe` 45 and 60 isn't drawn.
