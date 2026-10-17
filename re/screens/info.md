# The info screen: charts and race names

How the original draws the info screen (`AktKep == 9`). Evidence: the ELF oracle decompiled with
`tools/ghidra/DecompileFunctions.java` (addresses ELF unless marked exe), exe tables found by their bytes, and a
reference capture of ig2.exe (`tools/trace/run_screen.py … --kep 9`). The data behind it — the ten statistics charts
— is a sim system, specified and verified in `re/rules/info-messages.md` §1 (`ig2-sim::info`). `[inf]` marks
inference.

Scope: the chart and the race-name row (§2–§3), the 3D frame (§1.1), the empire panel, the spending limit, the money
triangle's texts and the lines (§4), the hot-spots and input (§5). Not yet: the planet/fleet list, the star log, the
selectors and sliders' orders (§6).

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
  (w = −z). Lights: ambient 0x1e1e1e, parallel 0x9e9e9e from (0, 0, −10), and the coloured side lights
  (`infoOldalsoFenyekBe` 0x554dc0: 0x7f5d00 and 0x004e7f from (200, 0, −650), moved by `infoMozgatMainFenyek`).
  The frame's faces point away from the eye: the parallel light lights them from behind, so the port lights both
  sides (the capture diff halved with it).
- `info.xvb` (`vector/info/`, `textures/info/`) `AddToScene(…, 1.22)`, `RotateObj(π/2, 0, 0)`, `MoveObj(0, 15, −650)`,
  property 0x200, posed by `SetFrame(InfoMainMesh, infoframe)`. Hidden: node 1's children 0xe..0x13 (init) and
  `GombMeshek[11]` = 1/7/0/5 (`InfoShowSelectedGomb` 0x554190).
- Race heads: for `k` 0..8, node 3 / `InfoMeshSorrend[k]` (ELF 0x117abb0: 7, 5, 4, 2, 3, 1, 6, 8) / 1 / 0 gets
  `textures/diplomac/<PlayerFaj[k + 1]>a.bmp` (`InfoTexturaNevek` 1a..8a), 1 % transparent; `UnhideIsmertFajFejeket`
  0x557870 shows 3/ring/1 only for the empires the human knows.
- The chart marker `negyzet1.xvb` ×0.031 at the centre of the selected chart button's mesh (`GombMeshek` ELF 0x117aca0,
  index `Info_SelectedChart`, +1 above 4: 1/1..1/5 then 1/7/0/6, 1/7/0/1..4) moved by (−1, −3), transparency
  `(sin(InfoVillogtato) + 1)/4`, hidden in star-log mode.
- The triangle's knob `hszog0.xvb` ×0.03 at `(InfoTriX·4.5 − 1438, InfoTriY·4.5 − 1050, −2650)`, transparency
  `1 − infoframe/45`; `Info_InitializeTriangleToMidPosition` 0x55e5d0 puts it at (319, `(int)(50 + 2√2700/5.6)`).
- Under the frame `Copy3hatterkep(0x34, 0xff − infoframe/45·128)`: picture `0x33 + n` is `SaveScreen(n)` (0x5deb20,
  a copy of the screen); in the captures it is black.

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
  y `249 + 55k` and `301 + 55k`) in 0x322200 (R 50, G 34), times the fade; under each race column a line from
  `FajnevX[e−1]` to +54 at y 107, black → `DDColor[e + 15]` → black; the triangle's drag trail.
- **Race names** (§3): the colour follows `Viszony[InfoSelected][e]`; the selected one is white in `DesignKeret1(x − 5,
  114, 0x36, 0xc, 2)` (a morphing gradient box; the port draws its settled outline [inf]).

## 5. Hot-spots and input (`Info_MouseInit` 0x5667b0, `Info_Main` 0x5647f0)
`Mouse_Szoveg_Set(id, 0, x, y, w, h, tooltip)` in id order (tooltips `GetString(9, 1, k)`): 1..=8 the race columns
`(FajnevX − 2, 48, 50, 79)`; 9..=12 the list rows `(60, 249 + 56k, 200, 53)`; 13 the list `(60, 249, 200, 56·min(n, 4))`;
14 charts `(588, 430, 45, 20)`; 15 star log `(350, 430, 45, 20)`; 16..=20 charts 0..4 `(350, 283 + 27k, 45, 20)`,
21..=25 charts 5..9 `(588, 283 + 27k, 45, 20)`; 26..=28 tax / auto-build / priority `(135, 354 + 14k, 100, 13)`; 29 the
triangle; 30/31 list up/down `(290, 247 / 450, 18, 19)`; 32 the limit `(280, 209, 100, 8)`; 33 `(0, 249, 55, …)`; 34 the
scroll bar `(290, 266, 18, 203)`; 35/36 star log up/down `(621, 274 / 431, 18, 19)`; 37 back from the detail.
`Info_Main`: the mouse **over** 1..=8 selects that empire (`InfoSelected`, no click needed); a press on 14 / 15
switches chart / star-log mode; 16..=25 picks `Info_SelectedChart = id − 16` (the button animates, sound 0);
9..=12 open a row's detail (`Info_NagyInfoMode`); 26..=28 open the select menus and send `Multi_Send(0x19, …)`
(planet settings 8/9/10); 29 drags the triangle and sends 0x5b (the three shares); 32 sends 0x5c (the limit
`(exp(3·2000·x/200000) − 1)·10500` rounded to 100, above 15,000 to 1,000).

## 6. Not specified here
The planet/fleet list (`KirakBolygoFlottakat(70, 249)` → `KirakBolygoInfo` 0x55f5d0 / `KirakFlottaInfo` 0x561250, the
planet and ship models `LoadBolygoUrhajok`, the detail `InfoNagyKiirBolygo`), the star log (`Info_PutStarLog`
0x5645b0: 16 lines of `StarLog`, unsaved), the select menus, the triangle and limit orders (0x5b, 0x5c: not in the
sim's commands yet), the opening and closing animation, the side lights, the debug tables (`InfoKellenekOldDolgok`).
