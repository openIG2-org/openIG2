# The info screen: charts and race names

How the original draws the info screen (`AktKep == 9`). Evidence: the ELF oracle decompiled with
`tools/ghidra/DecompileFunctions.java` (addresses ELF unless marked exe), exe tables found by their bytes, and a
reference capture of ig2.exe (`tools/trace/run_screen.py … --kep 9`). The data behind it — the ten statistics charts
— is a sim system, specified and verified in `re/rules/info-messages.md` §1 (`ig2-sim::info`). `[inf]` marks
inference.

Scope: the chart and the race-name row. The screen's 3D frame (`Infoing3DInit` 0x555b50: race heads in rings, the
triangle with the money sliders, the planet/fleet list, the panels), its opening animation (`infoframe` up to
`infoMAXFRAMES`, `infoFelfadeles` fading the text in), the planet/fleet list (`KirakBolygoFlottakat(0x46, 0xf9)`),
the empire panel (`KirakBirodalomInfo`), the star log and input are listed in §4 but not specified.

## 1. Frame structure
- `Info_Belep` 0x563fc0: `Info_Bir1/2` = 1, 2, chart mode on (`Info_ChartMode` = 1, `Info_SelectedChart` = 0,
  `Info_ChartFelbontasSelected` = 0), `InfoSelected = EN`, `Infoing3DInit`, the chart sweeps
  `InfoChartSzamlalok[k] = 0` with speeds `InfoChartSzamlaloAdderek[k] = rand() % 400 + 200` (10 libc draws, UI-only),
  the opening animation unless coming back from the messages/help/options screens.
- Per frame: `Infoing3DUpdate` 0x559980 (3D frame, `GFX_DateMoneyScrollFade`, `Update3D`), the 2D passes
  `InfoGfx2D_1..3`: `_2` 0x556500 = planet/fleet list, star log, **`Info_PutChart(Info_SelectedChart)`**, the empire
  panel; `_3` 0x556590 = **`KirakFajNeveket(0x28, 0x75)`** (and debug tables behind `InfoKellenekOldDolgok`).
- `FRISSIT_Info2` 0x5686f0 (UI tick): `InfoVillogtato += 0.04` (the selected empire's blink), the list's motion.

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

## 4. Not specified here
The 3D frame and its animation, the planet/fleet list and its detail mode (`Info_NagyInfoMode`), the empire panel
(`KirakBirodalomInfo`: name, strength, income, spending limit), the tax/auto-build/priority selectors, the star log
(`Info_PutStarLog`), the money sliders (`PutLimitCucc`, `PutHaromszog`), chart buttons and input
(`Info_MouseInit`), the debug tables (`InfoKellenekOldDolgok`). The client draws §2 and §3 over a plain panel, `Tab`
picks the chart.
