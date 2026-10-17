# The research screen (`AktKep` 4)

The original's research screen as `ig2-client` rebuilds it (ADR 0009). Addresses are ELF unless marked exe; the
exe's own versions were checked where they differ. Port: `engine/crates/ig2-client/src/research.rs`; the orders are
`ig2_sim::game::ResearchOrder` (re/rules/production-research.md §2.4).

| Part | ELF | exe |
|---|---|---|
| `Research_Belep` / `Research_Kilep` | 0x5e8780 / 0x5e8e70 | 0x523540 / 0x5239b0 |
| `Research3DInit` / `Research3DUpdate` (= `GFX_Research`, per drawn frame) | 0x5e3bb0 / 0x5e4e10 | 0x51f7c0 / 0x51fd10 |
| `ResearchGfx2D_1` (opening lasers) / `_2` (texts) / `_3` (empty) | 0x5deeb0 / 0x5df030 / 0x5e0ea0 | — / 0x5221a0 / — |
| `FRISSIT_Research2` (100 Hz UI tick) | 0x5eb780 | — |
| `Research_Main` (input) / `Research_MouseInit` / `…2` | 0x5ea1c0 / 0x5ea4d0 / 0x5eb060 | 0x523c10 / 0x524080 / 0x523e20 |
| `ResearchKeyboardHandler` | 0x5e9aa0 | — |
| `ResearchStartStop` / `ResearchDoubleGomb` / `ResearchAtvaltTalalmanyra` | 0x5e9190 / 0x5e93c0 / 0x5ea060 | 0x523420 / — / — |
| `Forgotabla` (middle wheel) / `rotate_table` | 0x5e8230 / 0x5e8080 | 0x51e360 / 0x525740 |
| `LoadTalalmanyVectorok` / `ElforgatTalalmanyok` / `ForgatObjectek` | 0x5e1520 / 0x5e3270 / 0x5e2290 | 0x51e900 / — / — |
| `TalalmanyKivalasztva` / `IgazitTeruletAkt` / `KellMozgatni` | 0x5e24a0 / 0x5e86d0 / 0x5e36e0 | 0x524f60 / 0x51e730 / — |
| `MULTIGET_Research` | 0x5ec6e0 | 0x51ee70 |

## 1. State
Saved (`Research_Save` exe 0x524590 and the research chunks, formats/save.md §4.3): `ResearchTerulet` 0xB9A9B4 (the
shown category 1..=12), `ResearchMezo` 0xB9AA04 (field 1..=16), `ResearchTalalmany` 0xB9AA08 (tech =
`(category − 1)·16 + field`), `Terulet_Akt[13]` 0x919200 (each category's remembered field, −1 none),
`ResearchOldForgato` 0x919598 (the middle wheel's front category, 3 = radars at start). Categories: 1 ships,
2 weapons, 3 radars, 4 shields, 5 bombs, 6 civil buildings, 7 research, 8 military buildings, 9 power plants,
10 tanks, 11 turrets, 12 special.

Screen state (one peer's UI): `resframe` (the opening 0..60, +0.4 a UI frame), `resKINYITVA` (open), `kezeloframe`
→ `kezeloframeto` (the middle wheel), `maxhossz[16]` (characters out per list row: −(k % 8) on entry, +1.8 a frame
up to 20), `InfoSorok[4]+0xb4` (the info lines' characters out: +1.4 a frame), `reswireframe`, the ball's matrix,
`AddZoom`/`TalalmanyDir` (the models flying out and in), `Kozelites`/`KozelitesDirection` (the chosen model coming
forward), `ResearchTalalmanyForgas[17]` (+0.02 a frame), `reskeringeto` (+0.0006), `res_xy` (2 on entry, +0.02),
`GombocTransparencyk[3]` (the lab cones' fade, ±0.02), `Double_Lenyomva`, `ButtonPressed`.

`Research_Belep`: `KELLKINYITASDEFAULT` (exe 0x912a39, 1) makes it open animated unless coming back from the
messages, help or options screens; `Forgotabla(ResearchOldForgato)`; `LoadTalalmanyVectorok(ResearchTerulet)`;
`IgazitTeruletAkt(1..=12)` moves each remembered field to the first tech the human knows (status ≥ 2, from the
remembered one on; −1 if none); `Double_Lenyomva = KutatasSurgos[EN]`.

## 2. The 3D scene (`Research3DInit`, `Research3DUpdate`)
Camera as the starmap's panels: `vx = 600·x/w + 320`, `vy = 600·y/w + 250` (w = −z; `kamerapont` z 600,
`xcenter`/`ycenter` 320/250). Lights: ambient 0x202020, parallel 0x808080 from (0, 0, −10), two side lights
(0xffba00, 0x009cff) circling with `reskeringeto` (`resMozgatMainFenyek`).

- `resbck.xvb` (`vector/research/screen/`), `AddToScene(…, 0.95)` at (0, `resmozgato`, −650), `resmozgato =
  (1 − resframe/60)·26 + 3`, posed at `resframe` (121-frame node animation; open = 60). Its 17 children
  (`GetSubMeshid` index):
  - 0..=4 / 5..=9: the lab cones left / right (`Cone01..10`, materials AI, MECH, COMP, CIV, MIL). 3, 4, 8, 9 are
    hidden; 0/5, 1/6, 2/7 show (fading at 0.02 a frame) when the shown tech needs lab 1, 2, 3 (`+0x31..+0x33`).
  - 10 (`Group114`) and 12 (`Group116`): the left and right button wheels (4 buttons each).
  - 11 (`Group115`) and 13 (`Group133`): the middle wheel (6 buttons + SPEC). 11 is the closed one (hidden while
    open), 13 turns: `SetFrame(kezeloframe + 70)`.
  - 14 (`Group66`, `Sphere01` "Gombracs"): the background wire sphere (property 0x40 = wireframe), frame
    `resframe + reswireframe` (0..10 by 0.25 at the open frame).
  - 15 (`Group85`): the info box's glass (`Box01` "Kekes", child 0 → 0; transparency `resatlatszosag =
    sin(res_xy)·0.04 + 0.85`), the double-money button (child 1 → 0 `Circle02`: `fejl_double_a.bmp` /
    `fejl_double.bmp`) and the start/stop button (child 4 → 0 `Circle01`: `fejl_start.bmp` when the shown tech can
    start, `fejl_stop.bmp` while a project runs, `fejl_emp.bmp` else, `SetStartStopTexture`).
  - 16 (`Kezelo_radar`): the starmap console, shown only during the transition.
- `focilabda.xvb`, `AddToScene(…, 0.22)` at (0, −35, −950), wireframe; turns 0.002 a frame
  (`matrixforgat(m, 0.002, 0)`), the right button drags it (`resGyX/Y`, within 160 pixels of its centre).
- The category's tech models (`LoadTalalmanyVectorok(c)`), field k = 1..=16, tech `(c − 1)·16 + k`:
  name `TTeruletek[(c − 1)·16 + k − 1]` (exe 0x919298, `char*`) from `vector/talalmanyok/`; ships' fields 5..=11
  from `vector/spacewar/` with the race digit in the name's 5th character; categories 6..=9 the building of the
  tech's `+0x27` (`Building[b]` exe 0x8F1F28: the model at +0, the military category's preview at +0x28,
  `vector/colony/`); turrets without a name fall back to `TankTorony[k − 1]`. `AddToScene(…,
  Research_MeretSzorzok[t − 1]·0.04)`. Place (`ElforgatTalalmanyok`): `TPozX/Y/Z[k]` (exe 0x9190F8, 0x919140,
  0x919188) ×2.2, turned by the ball's matrix, ×`(sin(AddZoom)·5 + 1)`, + (0, −35, −950); the chosen one
  (`Kozelites` +80 a frame up to 700) moves towards (0, −50, −1000) and is scaled ×`4·Kozelites/700`. Rotation
  (`ForgatObjectek`): `RotateObj(0, 0, spin)` then `RotateObj2((1, 0, 0), 1.3)`. Shown when status ≥ 2: owned (5)
  solid, else wireframe (properties 0x40 | 0x800) lit by an ambient light per model (`FenyHozzarendel`: seen
  (0, 32, 32), researchable (0, 64, 64), researching (64, 64, 0)).
- A new category: `TalalmanyDir = 1`, `AddZoom` rises 0.01 a frame (`FRISSIT_Research2`) to `ADDZOOMMAX` (exe
  0x918bd0, π/4): the models fly out; then they are reloaded and fly in (−0.15 a frame to `ADDZOOMMIN` = 0), and
  the remembered field is chosen (`TalalmanyKivalasztva`).

## 3. The 2D passes
`Research3DUpdate`'s faders (`KirakFader`, s = 0.9; open: fill transparency 105, edge 50): the side panels (3, 81,
135, 106) and (500, 81, 135, 106), their bars (5 / 502, 172, 131, 13), the running project's box (1, 26, 108, 42).

`ResearchGfx2D_2` (size 2, flags 4 = left):
- The info box: `res_info.txt`'s three lines for the tech (`LoadTalalmanyInfok`: per tech a `;` header line and three
  lines) and a fourth: `GetString(4, 2, 1) + ": "` and the prerequisites' names joined by ", " (or `GetString(4, 2,
  2)` when none), at (175, 28 + 13i), colour 2, `InfoSorok[i]+0xb4` characters with a colour-0xc cursor cell.
- The lists: the category's techs the human knows, field k at (10, 86 + 10k) for k < 8, (507, 86 + 10(k − 8)) for
  the rest: `"%-16s %2d%%"` (or `"%-16s    "` at 0 or 100 %) of the localised name (`txt17`, hud.md §4.1) and
  `progress·100 / (time·race time multiplier) + 0.5`, `maxhossz[k]` characters, colour by status (5 → 2,
  4 → 3, 3 → 1, 2 → 0x27). A click on a row (21 cells × 10 lines) chooses it. The chosen row's frame
  `DesignKeret1(x − 5, y − 2, 20 cells + 10, h + 4)`.
- The labs (`Tudomany_X/X2/Y` exe 0x919268/0x919278/0x919288): for each lab the tech needs, its need at
  (33 − 6, y) in colour 1 (short) or 2 and the human's level at (600 − 6, y), y 210/265/320, `KiirInt(…, 3, 2, 5,
  …)` (centred in 3 cells).
- The running project (pulsing: `KiirStrSzinSzorzo = (sin(reskeringeto·100) + 1)·20 + 40`), colour 0x22: its name
  at (4, 30), the daily cost `GetString(4, 2, 3)` at (4, 42), the days left `GetString(4, 2, 4)` at (4, 54).
- The shown tech, unless owned: days `GetString(4, 2, 5)` at (507, 175), daily cost `GetString(4, 2, 6)` at (10, 175).
- Days: speed = `(Σ max(0, lab − need)·[need ≠ 0] + 5) / 5` (f64), ×2 when urgent, ×`(StoryAdat+0x6c + 100)/100`
  in SP, at most the work left, at least 1; days = `(int)left / speed / 50`. Cost per day = `price × money
  multiplier × left / total` (f32), ×2 when urgent.

`ResearchGfx2D_1` draws the opening lasers and sparks (not ported); `ResearchGfx2D_3` is empty.

## 4. Input
Hot-spots come from the mask picture `RESE_MSK.RAW` (kep 0x58; `mhwanh` header 640 × 480, a palette, then one byte
per virtual pixel): `Mouse_Darabol` (ELF 0x5b7a40) takes the pixel's value and finds the hot-spot registered with
that colour (`Mouse_Szoveg_Set(id, colour, …)`; colour 0 = the rectangle). `Research_MouseInit`:
- 1 (colour 0x11): start/stop (`ResearchStartStop`); on only while a project runs or the shown tech can start.
- 4 (0x12): urgent (`ResearchDoubleGomb`: the button's texture flips at once, message 0x2b).
- 6, 7, 8, 10, 0xd, 0xe, 0xf (their own colours): civil, power, military, research, special, tanks, turrets.
- 9, 0x10, 0xb, 5, 0xc: radars, weapons, shields, bombs, ships on the middle wheel. The wheel's five slots keep
  their colours (9, 16, 11, 5, 12); `Forgotabla(n)` turns category n to the front slot: s = its slot,
  `forgastabla2/3/4x/4y` rotate right `5 − s` times (exe 0x525740), `kezeloframeto += (5 − s)·10` (mod 50),
  backwards (−2 a frame) for s = 1, 2, else forwards (+2).
- Category hot-spots are off while the wheel turns or the models fly (`Research3DUpdate`'s `Mouse_Szoveg_OnOff`).
- 0x12 rectangle (175, 29, 298, 50): the info box (all lines out). 0x19 (4, 16, 104, 46), on while a project runs:
  `ResearchAtvaltTalalmanyra(ResearchAktFejlesztett[EN])` shows the running project. 3 and 0x13..0x18 only carry
  tooltips.
- A model's projected centre within 30 pixels (left press, models in place) chooses it
  (`CheckIfTalalmanyokFolottVan`).

`ResearchStartStop`: unless `ButtonPressed`: nothing running, status 3 and every needed lab level reached (or the
research cheat 0xB7727C) → `Multi_Send(0x2a, EN, 4, ResearchTalalmany)`; a project running →
`Multi_Send(0x29, EN, 4, ResearchAktFejlesztett[EN])`. `ButtonPressed` until the handler clears it.

Keys (`ResearchKeyboardHandler`): ↓/↑ next/previous known field, →/← the field 8 on/back in the other column,
Enter start/stop, D urgent, PgUp/PgDn the previous/next category in `Terulet_Melyik` order (exe 0x919234: tanks,
turrets, radars, ships, bombs, shields, weapons, special, civil, power, research, military).

The client opens the screen with the top bar's research button (1005, (600, 0) 40 × 20) and K, closes it with Esc
or the button.

## 5. Not ported yet
The opening/closing animation (`resframe` 0..60: the console flying in, `ResearchGfx2D_1`'s lasers and sparks, the
fade of the starmap console), the buttons' press animations (`resSetGombFrame`), `DesignKeret1`'s animated frame
(drawn as a plain box), the lines from the wheel button to the ball (`ResDrawVonalak`), tooltips, sounds, the ball
drag, the screen-select menu entry.

## 6. Orders
`MULTIGET_Research`: re/rules/production-research.md §2.4 (verified byte-exact, traces/2026-10-17-research-orders.md).

## 7. Capture comparison
`tools/trace/run_screendiff.py <install> quick-1.sav research` (the client in Hungarian, `IG2_LANG=hu`, like the
exe): results in diff-results.md.
