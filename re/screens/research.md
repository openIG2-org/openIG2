# The research screen (`AktKep` 4)

The original's research screen. Addresses are ELF unless marked exe; the exe's own versions were checked where they
differ. The orders are re/rules/production-research.md §2.4. The picture behind the scene (the saved starmap,
dimmed): [`overlay-background.md`](overlay-background.md).

| Part | ELF | exe |
|---|---|---|
| `Research_Belep` / `Research_Kilep` | 0x5e8780 / 0x5e8e70 | 0x523540 / 0x5239b0 |
| `Research3DInit` / `Research3DUpdate` (= `GFX_Research`, per drawn frame) | 0x5e3bb0 / 0x5e4e10 | 0x51f7c0 / 0x51fd10 |
| `ResearchGfx2D_1` (opening lasers) / `_2` (texts) / `_3` (empty) | 0x5deeb0 / 0x5df030 / 0x5e0ea0 | — / 0x5221a0 / — |
| `FRISSIT_Research2` (100 Hz UI tick) | 0x5eb780 | — |
| `Research_Main` (input) / `Research_MouseInit` / `…2` | 0x5ea1c0 / 0x5ea4d0 / 0x5eb060 | 0x523c10 / 0x524080 / 0x523e20 |
| `ResearchKeyboardHandler` | 0x5e9aa0 | 0x523020 |
| `ResearchStartStop` / `ResearchDoubleGomb` / `ResearchAtvaltTalalmanyra` | 0x5e9190 / 0x5e93c0 / 0x5ea060 | 0x523420 / — / — |
| `Forgotabla` (middle wheel) / `rotate_table` | 0x5e8230 / 0x5e8080 | 0x51e360 / 0x525740 |
| `LoadTalalmanyVectorok` / `ElforgatTalalmanyok` / `ForgatObjectek` | 0x5e1520 / 0x5e3270 / 0x5e2290 | 0x51e900 / 0x51d600 / — |
| `SetTTransp` / `FenyHozzarendel` (= `UnHideMeglevoTalalmanyok`'s body) | 0x5e2af0 / 0x5e2c30 | 0x5248a0 / 0x525060 |
| `TalalmanyKivalasztva` / `IgazitTeruletAkt` / `KellMozgatni` | 0x5e24a0 / 0x5e86d0 / 0x5e36e0 | 0x524f60 / 0x51e730 / 0x51e790 |
| `MULTIGET_Research` | 0x5ec6e0 | 0x51ee70 |

## 1. State
Saved (`Research_Save` exe 0x524590 and the research chunks, formats/save.md §4.3): `ResearchTerulet` 0xB9A9B4 (the
shown category 1..=12), `ResearchMezo` 0xB9AA04 (field 1..=16), `ResearchTalalmany` 0xB9AA08 (tech =
`(category − 1)·16 + field`), `Terulet_Akt[13]` 0x919200 (each category's remembered field, −1 none),
`ResearchOldForgato` 0x919598 (the middle wheel's front category, 3 = radars at start). Categories: 1 ships,
2 weapons, 3 radars, 4 shields, 5 bombs, 6 civil buildings, 7 research, 8 military buildings, 9 power plants,
10 tanks, 11 turrets, 12 special.

Screen state (one peer's UI): `resframe` (the opening 0..60, +0.4 a UI frame; §5), `resKINYITVA` (open), `kezeloframe`
→ `kezeloframeto` (the middle wheel), `maxhossz[16]` (characters out per list row: −(k % 8) on entry, +1.8 a frame
up to 20), `InfoSorok[4]+0xb4` (the info lines' characters out: +1.4 a frame), `reswireframe`, the ball's matrix,
`AddZoom`/`TalalmanyDir` (the models flying out and in), `Kozelites`/`KozelitesDirection` (the chosen model coming
forward), `ResearchTalalmanyForgas[17]` (+0.02 a frame), `reskeringeto` (+0.0006), `res_xy` (2 on entry, +0.02),
`GombocTransparencyk[3]` (the lab cones' fade, ±0.02), `Double_Lenyomva`, `ButtonPressed`.

`Research_Belep`: `KELLKINYITASDEFAULT` (exe 0x912a39, 1) makes it open animated unless coming back from the
messages, help or options screens; `Forgotabla(ResearchOldForgato)`; `LoadTalalmanyVectorok(ResearchTerulet)`;
`IgazitTeruletAkt(1..=12)` moves each remembered field to the first tech the human knows (status ≥ 2, from the
remembered one on; −1 if none); `Double_Lenyomva = KutatasSurgos[EN]`. Entered without the animation, the exe plays
`Res_Hangok[1]` ~31 ticks in and no `Res_Hangok[0]`.

## 2. The 3D scene (`Research3DInit`, `Research3DUpdate`)
Camera as the starmap's panels: `vx = 600·x/w + 320`, `vy = 600·y/w + 250` (w = −z; `kamerapont` z 600,
`xcenter`/`ycenter` 320/250). Lights: ambient 0x202020, parallel 0x808080 from (0, 0, −10), two side lights
(0xffba00, 0x009cff) circling with `reskeringeto` (exe 0xDDF6BC; `resMozgatMainFenyek`, inlined in the exe's
`Research3DUpdate`: `(sin(20k)·300, cos(20k)·200, −950)` and the same at +1.57075).
Engine behaviour seen on the opening's captures [obs]: back faces are culled (solid parts and the ball's wireframe;
the wire sphere draws every edge); geometry close to the camera still draws (the info box's glass passes within a few
units while closing); an opaque wireframe is lit per triangle (the wire sphere: its cells facing the camera full
(46, 144, 239), the far side the ambient's (5, 18, 30)); edges crossing the camera plane are clipped, not dropped; the
2D faders and lasers draw over the models.

### 2.1 The engine's lights
The 3D engine colours the vertices of lit objects (property 1) on the CPU; the shader only multiplies the texture
by the vertex colour (`gl_FragColor = texture2D(…)·var_color`, `var_color.xyz *= var_color.w`; no doubling). Shared
by every 3D screen.
- Lights live on a scene node (`CreateLightAmbient` ELF 0x4575c0 / exe 0x451cb0, type 0; `CreateLightParalell`
  0x4577a0 / 0x451ef0, type 1, stores `normalizevektor(pos)` at +0x18; `CreateLightOmni` 0x457980, type 2;
  records of 0x48 bytes: type +0, colour word +4, `SetLightStrength`'s strength +0x3c (omni only), specular flag
  +0x44). `SetLightPos` (0x457c50 / 0x453050) stores a parallel light's normalised position (an omni's raw one) and
  flags the node's subtree for relighting (`fenyvaltozikbeir` 0x454490) only when it changed; `MoveObj`,
  `RotateObj`, `SetFrame`, `ScaleObj` and `AddToScene` flag theirs too.
- `Kocka` (ELF 0x5f6a40) gives every node the parent's lights, then its own (at most 15, colour ≠ 0); a parallel
  light's vector is turned by the parent's rotation chain into +0x28 (the camera's `cmatrix` at the root: identity on
  the 3D-framed screens). Each node carries two matrices: the placement (scale × rotation, +0) and the rotation
  chain (`spsorttype+0x24`): `AddToXVB` keeps the `AddToScene` scale apart (+0x10) from the rotation (+0x64,
  `CreateMatrix`, `RotateObj`); a file child contributes its record matrix, an animated one its key's rotation
  matrix (key floats 9..17; `Kocka` blends two keys linearly without renormalising).
- `fenyekszamolasa` (ELF 0x606530, exe 0x52f190; only when a light or the node changed: the colours are cached in
  the mesh's `vertexnormal` array, +0xc): the colour starts at 0; ambient lights add their colour bytes; a parallel
  light adds `colour·shade[i] >> 8` per byte with `i = float2int(|n' − Mᵀ·L|²·2047)`, `n'` = the vertex's file normal
  (normalised by `LoadObject`) with z negated, `M` the node's rotation chain (`InvertMatrix` 0x451b80 is a transpose),
  `L` the light's +0x28; with the specular flag (`SetLightSpecular`) and a material specular table also
  `colour·specular[i] >> 8`. Every add saturates per byte (`addcolor` 0x450c70). Omni lights add nothing here.
  Without property 8 the bytes are scaled by the opacity `(255 − 255·transparency)` `>> 8` and alpha is 0xff; with
  property 8 (the research screen's parts) the bytes stay and alpha = the opacity. Unlit objects (no property 1,
  no 0x100) get 0xffffff.
- `shade` = `SetLights(2, 0.15)` (ELF 0x602ea0, from `Init3DVariables`): `N = (int)(√2/4·8192)` = 2896, `shade[i] =
  (int)(sqrt(1 − i/N)·255)` below N, 0 from N to 8191; `specular[i] = (int)(cos(i·3.141/N/2)^64·255)` likewise.
  `float2int` (exe 0x542d80) adds 2²³ in f32 and keeps the mantissa (round to nearest even). So a parallel light
  reaches a normal within ~73° of it (`cos θ > 1 − N/4094`), full (255) face-on, `sqrt` falloff.
- Colour words are bytes `[r, g, b]`: `0xffba00` is (0, 186, 255), sky blue; `0x009cff` (255, 156, 0), orange
  [obs: the cyan left wheel at opening 45].
- `wirekirak` (ELF 0x6083f0) draws a lit wireframe's edges with each end's vertex colour times the texture there.
- Runtime check: `run_screen.py --kep 4 --res-frame 60 --res-keringeto 0.01 --light-probe --jsonl
  _local/traces/light-research.jsonl` hooks the exe's `fenyekszamolasa` on the first lit frames and records each
  node's rotation chain, lights and every vertex's normal and colour: the formula above gives all 6,096 colours
  exactly (`lighting::tests::research_probe_matches_the_exe`; the shade table matches the exe's copy).

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
  (`ForgatObjectek`): `RotateObj(0, 0, spin)` then `RotateObj2((1, 0, 0), 1.3)`; `spin` (`ResearchTalalmanyForgas`)
  +0.02 a UI tick, reset to 0 for every model `LoadTalalmanyVectorok` loads. Shown when status ≥ 2: owned (5)
  solid, else wireframe (properties 0x40 | 0x800) lit by an ambient light per model (`FenyHozzarendel` exe 0x525060:
  colour words 0x202000 seen, 0x404000 researchable, 0x004040 researching, i.e. bytes (0, 32, 32), (0, 64, 64),
  (64, 64, 0)).
- **Depth fade** (`SetTTransp` exe 0x5248a0; the ELF's 0x5e2af0 adds to the old transparency instead, the exe's
  runs): inside each step of the exe's `ElforgatTalalmanyok` (0x51d600, before that model's `MoveObj`), for all 16
  models from the z of their last `MoveObj` (×1024): `t = 0.01` if z > −950 (in front of the ball's centre), `−(z +
  950)/100` for −1050 ≤ z ≤ −950, 1 behind; `t += AddZoom/ADDZOOMMAX`, at most 1; `HideAlboxok(t ≤ 0.99)` (above 0.99
  every sub-mesh hidden); `SetObjTransparency(t)` (< 0.0001 → 0, > 0.99 → 1). So the far half of the orbit is
  invisible, the models near the ball's centre half faded, and the chosen one (pulled to z −1000) is drawn at 0.5
  [obs: `probes/research_items.js`: opacity bytes 252 in front, 128 / 134 near the centre]. The solid models take the
  opacity as alpha (property 8); the wireframes' lit colours are scaled by it.
- **Wireframes as drawn** [obs, a probe of `fenyekszamolasa`]: the ball's vertices are white (the four lights
  saturate) or 0x202020 and every edge draws (the far ones dim), in the texture's colour (49, 154, 255); the tech
  models' edges take the lit vertex colour alone (0xfbfbfb white, 0x5e5e1f teal), no texture colour. Lines are aliased
  GL lines (one pixel, full colour; an edge two faces share draws once: the second fails the depth test). The ball's
  chain at t 250 is (0.9797, 0, −0.2006 | 0, 1, 0 | 0.2006, 0, 0.9797) (row-major): `matrixforgat(m, 0.002, 0)` turns
  it −0.002 about y in `axis_matrix`'s sense.
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

`ResearchGfx2D_1` draws the opening lasers and sparks (§5.2); `ResearchGfx2D_3` is empty.

## 4. Input
Hot-spots come from the mask picture `RESE_MSK.RAW` (kep 0x58; `mhwanh` header 640 × 480, a palette, then one byte
per virtual pixel): `Mouse_Darabol` (ELF 0x5b7a40) takes the pixel's value and finds the hot-spot registered with
that colour (`Mouse_Szoveg_Set(id, colour, …)`; colour 0 = the rectangle). `Research_MouseInit`:
- 1 (colour 0x11): start/stop (`ResearchStartStop`); on only while a project runs or the shown tech can start.
- 4 (0x12): urgent (`ResearchDoubleGomb`: the button's texture flips at once, message 0x2b).
- 6, 7, 8, 10, 0xd, 0xe, 0xf (their own colours): civil, power, military, research, special, tanks, turrets.
- 0x13..=0x18 (rectangles): the labs' tooltips only (§5.5).
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

Keys (`ResearchKeyboardHandler`, ELF 0x5e9aa0 / exe 0x523020): ↓/↑ next/previous known field, →/← the field 8
on/back in the other column, Enter start/stop, PgUp/PgDn the previous/next category in `Terulet_Melyik` order (exe
0x919234: tanks, turrets, radars, ships, bombs, shields, weapons, special, civil, power, research, military), each
`TT_*` → `Forgotabla` (ELF 0x5e8230), which clicks (`PlaySoundGlobal(0)`) while `resKINYITVA`. `remapKeyCodes` swaps
the keys (input.md §1): the PageUp key is vk 0x22, so it steps to the **next** category [obs: turrets 11 → radars 3 →
civil 1, a click each]. The handler's vk 0x44 (D, `ResearchDoubleGomb`) never arrives (no letter key down is queued).
Enter fires only when the handler's last call was not a key down: `oldEnterlenyomva`/`oldDlenyomva` (exe
0xDDFFEE/0xDDFFED) take the down flag of **every** call (every key up, every character, every queued key down), so a
held Enter's OS repeats do nothing until any key goes up [obs: Enter at 210 starts tech 2, the lone repeat at 225 does
nothing, ← up at 233, Enter at 245 stops it]. `ResearchStartStop` (ELF 0x5e9190) clicks only on `Mouse_Lenyom`:
Enter is silent [obs].

Esc is `ESC_Pressed` (options.md §5.1): the options screen at once, no closing, no sound; the top bar's spots are over
the screen's (input.md §6).

### 4.1 The mouse: the ball drag, the model grab, the faders' hover
The frame reads the buttons of the last `Mouse_Lekerdez` poll (ELF 0x5b7720, once per `IG2_gameUpdate` loop **after**
`UpdateFrame`): `Mouse_Gomb` any button, `Mouse_Lenyom` (exe 0xDB465F) its rising and `Mouse_Felenged` (0xDB465C) its
falling edge, `Mouse_Bal`/`Mouse_Jobb` (0xDB4651/52). `Research3DUpdate` (exe 0x51fd10; ELF 0x5e4e10 lines 25-28,
222-231, 434-473), per drawn frame: `resGyX/Y` (exe 0xDC4728/0xDC472C) ×0.8; `Felenged` drops the model's grip;
open and the grab test → `MouseOld = Mouse` (0xDC538C/0xDDF31C); `Update3D`, `KellMozgatni`; open: `matrixforgat(ball,
resGyX, resGyY)` (exe 0x4523b0: axis `normalize(b, −a, 0)`, angle `√(a² + b²)`), `Felenged` drops the ball's grip
(`ResearchGombMegfogva` 0xDDF321), the grab test takes it; gripped and no model held: `resGy = 0.9·resGy +
(MouseOld − Mouse)/30·0.1` and `MouseOld = Mouse`; not gripped: the idle `matrixforgat(0.002, 0)`. The grab test
(`GombfolottLenyomva` ELF 0x5e2190, inlined in the exe): the ball's projected centre within 160 pixels, `Mouse_Lenyom`
and `Mouse_Jobb` (the right button pressed first; a left press, or the left already down, never grabs).
A model press (`CheckIfTalalmanyokFolottVan` ELF 0x5e2720: within 30 pixels, `Lenyom && Bal`) →
`TalalmanyKivalasztva` (exe 0x524f60): a model not on its way (`KozelitesDirection` 0) is held
(`Research_Talalmany_Megfogva` 0xDC471D), and every call takes `MouseOld = Mouse`; `KellMozgatni` (exe 0x51e790) sets
the held model's turn to `(MouseOldX − Mouse_x)/100` (absolute) every frame; `FRISSIT_Research2` turns every model
+0.02 a tick but the held one. `KirakFader` (exe 0x4d3880) lights a fader under the pointer (strict rectangle) when
`(!Bal && !Jobb && !Felenged) || Lenyom` and it isn't the last lit one.

## 5. Opening, closing and presses
### 5.1 The opening and closing (`Research3DUpdate`, `FRISSIT_Research2`, `Research_Kilep`)
State (exe): `resframe` f32 0xDC5368, `resKINYITVA` 0xDDF322 (open; set by `Research_Belep` 0x523540 when unanimated,
by `Research3DUpdate` at `resframe ≥ 60`; cleared by `Research_Kilep` 0x5239b0), `resBECSUKAS` 0xDDF323 (closing),
`resKELLKINYITAS` 0x918BD5, `reswireframe` 0xDC537C, `resELINDULT` 0xDDF628 (a frame was drawn).
- `FRISSIT_Research2` (100 Hz): animated and started, `resframe += 0.4` up to 60 (150 UI frames); closing,
  `−0.72` down to 0. `AddZoom` +0.01 (open, models flying out) or +0.02 (closing).
- `Research3DUpdate` (per drawn frame): `SetFrame(ResMainMesh, resframe)` (the whole console's node animation
  0..=60), `MoveObj(ResMainMesh, 0, (int)resmozgato, −650)`. Not open: at frame 60 (not closing) the screen opens
  (`resKINYITVA`, `InitKozepso`: the closed middle wheel `Group115` hidden, the turning `Group133` shown); before
  frame 30 the ball is hidden and the wire sphere's transparency is `1.35 − 2f/60` (the exe clamps < 0.0001 to 0,
  > 0.99 to 1); from 30 the ball flies in: `g = (60 − f)·200`, `MoveObj(FociMesh, sin(g/1000)·200·g/2000,
  cos(g/1000)·150·g/2000 − 35, −950 − g/10)`, `RotateObj(sin(g/1000), 0, 0)` then turned 0.002 (its transparency
  `g/12000 − 0.6` is always clamped to 0), the sphere opaque.
- While `resKELLKINYITAS` (also at rest), with `g = 1 − f/60`: the info box `Group85` (child 0xf) is moved to
  `InitPos2 + (0, g·700, g·1400)` (exe 0x9185c0: (0.036, −233.77, −8.66)) and turned `(sin(2g)·2, 0, 0)`; the wire
  sphere `Group66` (0xe) to `resInitPos1 + (0, g·200, −g·1000)` (exe 0x9185b0: (0, 0, 200)) and turned
  `(sin(2g)·3, sin(2g)·2, 0)`. `MoveObj` on a child replaces its record translation (`v/1024`), `RotateObj` its
  matrix (`P·K·R`, spy.md §2.2): at rest the sphere sits at z 200/1024, not its file's −0.069.
- `Kezelo_radar` (0x10, the starmap console) shows while not open or closing (the exe also hides it on the one frame
  where `resframe == 20` exactly, and for the transitions to/from screen 9).
- The 2D faders (`KirakFader` 0..4) only past frame 30: `l = f/30 − 1`; opening, whole panels with fill
  transparency `(1 − l)·150 + 105` and edge `(1 − l)·205 + 50` (opaque at 30, the open values at 60); closing,
  the open transparencies with the panels folding to y 124 (`y = 124 − 43l`, `h = 106l`).
- `ResearchGfx2D_2` writes nothing until open; the lists and labs stop when closing. The info lines restart at
  `InfoSorok[i]+0xb8` (0, −20, −40, −60: `NEWGAME_Research`) for a new tech and type on only while open.
- The tech models: `LoadTalalmanyVectorok` hides them; they show after the zoom-in that follows the opening
  (`UnHideMeglevoTalalmanyok` when open and `TalalmanyDir == 0`) or at a category's reload. They keep drawing while
  flying out (`TalalmanyDir == 1`, fading with `AddZoom`) and hide only at the reload. The list/frame shine is
  the `resf` faders (exe 0xDC4758); `ResearchGfx2D_2` restarts the info lines on a new tech before `Research_Main`.
- `Research_Kilep` (animated, not to screens 10/1/13): `resBECSUKAS`, the turning wheel hidden and the closed one
  shown, the models fly out (`TalalmanyDir` 1, from `ADDZOOMMIN` when they stood), sound `scrn01ki.wav`; then
  `resKINYITVA = 0`, `UpdateFrame` + `FRISSITES` in a loop until frame 0 was drawn once (`VoltNulla`). Closing at
  the max zoom hides the models (`HideAllTalalmany`); at frame 0 the sphere is hidden.
- `Research_Belep` starts `kezeloframe = kezeloframeto = 0` and then `Forgotabla(ResearchOldForgato)`: the wheel
  turns to the saved front category once open.

### 5.2 The lasers (`ResearchGfx2D_1` ELF 0x5deeb0)
Started and not closing: while opening (frame not 0 or 60, not open) `DrawLaser` (ELF 0x53c200) runs round both side
faders (3, 81, 135, 106) and (500, 81, 135, 106) from `LaserPos` (0, 0) and (639, 0) with colour (0, 0x70, 0xe3) at
`t = f/60`; `DrawSparks` unless at frame 60. `DrawLaser`: opacity `a = 255 − fade` (`fade = (0.1 − t)·2550` below
0.1, `(t − 0.9)·2550` above 0.9); four heads (top from the left `(x + wt, y)`, bottom from the right
`(x + w(1 − t), y + h)`, left from the bottom `(x, y + h(1 − t))`, right from the top `(x + w, y + ht)`); the top
and bottom heads drag a tail of additive pixels from the head back to its start, the colour less 2 a pixel
(`DrawFadingLineH`; `DrawFadingLineV` ELF 0x53c040 fills its pixel list but never draws it, so the side heads have
none [obs]); 12 additive lines (`line2d`) from the source to the heads in `(r/5 + random(15), g/5 +
random(15), b/5 + random(15))`, and at half colour from one pixel left and one up; per head `AddSpark` and a glow
(`laservago.bmp`, `InitFader1`) of half-size `(random(10) + 15)/1.5` in the colour and `(… + 5)/1.5` white, both × `a`,
additive (`polygons(…, 1)`). `AddSpark` (0x53bcd0): 10 sparks in free slots of 1500, speed `(random(50)/5 − 2.5)/3`
each axis; `UpdateSparks` (0x53b9a0, every UI frame from `Frissit2_Grafika`): move, ×0.93, +0.04 down, −3 a channel,
gone below a colour sum of 50 or off (1..638, 1..478); `DrawSparks`: additive pixels. `random(n)` = `rand() % n`
(libc, cosmetic).

### 5.3 Presses (`resSetGombFrame` ELF 0x5e4360)
`ResMelyikGombot` (the button, −1 none) and `ResGombNew` are set by `ResearchStartStop` (0xcc), the `TT_*`
category handlers (middle wheel: `forgastabla3[slot]` = 100 + child, set by `Forgotabla` before it rotates; off the
wheel: tanks 1, turrets 3 (left wheel), civil 0x12d, military 0x12e, research 0x12f, power 0x130 (right wheel),
special 0x6a). In `Research3DUpdate` from frame 60: `ResGombNew` clears `KeszALenyomas`; then `ResGombFrame` (shared)
goes +7 to 10 and back −7 to 0 (`ResGombFrameCsokkent`), `KeszALenyomas` again at 0; the middle wheel only turns
with `ResGombFrame == 0`, `KeszALenyomas` and the models not flying out, and the category hot-spots are off while
it turns or a press isn't over. `resSetGombFrame(id, f)`: id < 100 → the left wheel's child id, z = `−30f − 6000`
(1) / `−5300` (3); 100..199 → the middle wheel's child id − 100, z = its start z (`GetKezdoZposok`) − 30f; 200..299
→ the info box's child id − 200 `SetFrame(f + 60)` (the buttons' own keys push them in over frames 60..70);
300..399 → the right wheel's, z = `−30f − 3250/3060/2950/3260` for 0x12d/0x12e/0x12f/0x130 (`MoveObj`, v units: the
first press moves a side button to that depth for good). The urgent button doesn't use it: `ResearchDoubleGomb`
puts it in (10, `SetDoubleTexture`) or out (0, `DoubleGombFelenged`) at once; `Research_Belep` puts it in when urgent.

### 5.4 The chosen row's frame (`DesignKeret1` ELF 0x53f780)
`DesignKeret1(x − 5, y − 2, 20 cells + 10, h + 4, 1)` every drawn frame while open. A new place starts a morph
(`dkmcel = π/2`); `Frissit2_Grafika` (ELF 0x5481c0, every UI frame) runs `dkm` +0.05 to π/2, jumps to the new place,
then back to 0 (`dkoldx == 0` after a screen change keeps it hidden until the first place arrives; asked on the very
first frame it would never show). Boot values (.data): dkm 0xB34C0C = 0, dkmcel 0x903548 = π/2, GfxNovelo 0xB34BAC =
0; with no place asked `Frissit2_Grafika` (exe 0x4d2ed0) saw-tooths dkm π/2 → 0, so the first ask morphs in unless it
lands on dkm == π/2 (1 tick in ~32). `DesignKeretAzonnalMorph` (0xad0ef5): the next new place jumps without a morph
(the main menu's scroll drag). Drawn with `f = 1 − sin(dkm)`, `w' = w(2 − f)`, `x' = x − w(1 − f)/3`, colour
`DDColor[1]` × `KiirStrSzinSzorzo` (100): per row of `h + 1` from `y − 1` a glow of half the colour times a triangle
peaking mid-height, dark at both ends over a tenth of the width (`line3` gives a segment's first colour to its far
end); the border as 18 one-pixel segments (top right to left, the left corner, side and corner, bottom left to
right, the right corner, side and corner), segment k `frac((sin(GfxNovelo + k·20°) + 1)/2)` bright (`GfxNovelo`
+0.02 a UI frame, a light running round); all additive (`line3(…, true)`). Device rows: each virtual pixel fills its
own device rectangle, but a device row whose centre lies exactly on a virtual row's top edge (every 5th at 800x600)
takes the row above's value in the exe.

### 5.5 Tooltips
`Research_MouseInit` (exe 0x524080) gives each hot-spot `GetString(4, 1, k)` (from the asm's immediates): 1 → 6 (and
0x18 from `Research_MouseInit2` every frame), 3 → 7, 4 → 8, 6 → 9, 7 → 0xa, 8 → 0xb, 10 → 0xc, 0xd..=0xf → 0xd..=0xf,
0x12 → 0x10, 0x13..=0x18 → 0x11..=0x16 (the labs: rectangles (10 | 0x23f, 0xbd/0xf5/0x129, 0x37, 0x32)), 0x19 →
0x17; `Forgotabla` the middle wheel's 9, 0x10, 0xb, 5, 0xc → 1..=5. `Research_MouseInit2` (exe 0x523e20, from
`UpdateFrame`): 3 and 0x19 only while a project runs, 1 as `ResearchStartStop` would act, 0x11 never, 0x12 while
open and not closing, the labs while their cone shows (`GombocTransparencyk[i] == 0`: 0x13/0x16 lab 1, 0x14/0x17
lab 2, 0x15/0x18 lab 3). Shown like every screen's (`UpdateFrame` 400 ms rest, lowest id, spy.md §6.1).

Model tooltip 0x11 (`CheckIfTalalmanyokFolottVan`, exe 0x51d2d0): over a model the text is the tech's name
(`Talalmanyok` name, 0xb9c1dc + i·0x114), rect 30 × 30 at that frame's pointer, release flag 0, cursor 1; off with no
model under it unless one flies (`FUN_00525280`); runs only while `resKINYITVA`. Model hit: `resKINYITVA` (open or
closing), `AddZoom` == min (0xddfc78 == 0xdc4754), entry +0x2a loaded and +0x28 shown, `KozelitesDirection[k] == 0`,
`MoveObj` z·1024 > −1050 (asm 0x51d34d), f32 squared distance, < 30 strict; the centre is `GetObjCenter` (0x452280).
The list-row pick and the hand need open && !closing.

### 5.6 The button lines (`ResDrawVonalak` ELF 0x5e4910 / exe 0x51f340)
Not lines from the button to the ball: a ring of ten arrowheads pointing in at the last pressed category button.
ELF and exe agree in every constant and precision (exe: the helpers inlined or unrolled).
- **Where**: `ResLastGombX/Y` (ELF 0x118a0d0, exe 0x9185a8; static −1, never set back). `Forgotabla` (ELF 0x5e8230,
  exe 0x51e360) sets them from `forgastabla4x/4y[slot]` (exe 0x918c50/0x918c64, all five (322, 353): the middle
  wheel's front button), so every middle-wheel category and the screen's entry (`Research_Belep` calls
  `Forgotabla(ResearchOldForgato)`) put it there. The off-wheel `TT_*` set constants (ELF 0x5e94b0.., exe
  0x524a50..): civil 6 → (0x1ec, 0x172), power 9 → (0x1c0, 0x199), military 8 → (0x218, 0x19e), research 7 → (0x1ed,
  0x1c8), special 12 → (0x140, 0x19b), tanks 10 → (0x96, 0x172), turrets 11 → (0x96, 0x1ca).
- **State**: `ResVonalkak[10]` (f32, exe 0xddf794), `ResVonalakNovelo` (f32, exe 0xddf674), `ResKellVonalTavolodas`
  (exe 0xddf62b). Init (`ResInitVonalkak`): `v[i] = (i/10)·50 + 100` (100, 105, …, 145), Kell 0. Hide
  (`ResEltuntetVonalkak`): `v[i] = 100000`, Kell 0. `LoadTalalmanyVectorok` (ELF 0x5e1520, exe 0x51e900): Novelo 0,
  Init, Kell 1 (it runs from `Research_Belep` and when a new category's models are loaded after the old ones flew
  out). `Research3DUpdate`: the first drawn frame after `ResearchTerulet` changes (≠ `OldResearchTerulet`) hides them.
- **Tick** (`FRISSIT_Research2`, every UI tick on the screen, also while opening): `v = v ≤ 0 ? 0 : v − 0.5`; then
  `if 500 ≤ Novelo { Novelo = 0; Init } else { Novelo += 1 }` (a 501-tick period, the restart without fly-in).
- **Draw** (`ResearchGfx2D_2`'s first step, when `ResLastGombX ≠ −1` and `resKINYITVA`), centre `c = (X·639/639,
  Y·479/479)` (f32), for each `v < 100`:
  - `f = (v ≤ 50 ? 0 : v − 50)/12` (f64 → f32), 0 unless Kell (the fly-in from radius ~103 px).
  - alpha `A = (int)(v > 50 ? (100 − v)·2.56 : v > 20 ? 128 : max(v·6.4, 0.001))`: in over 50 ticks, 128 for 60,
    out over 40 (after Init line i starts 10i + 1 ticks late).
  - `θ = (f32)(Novelo/500·3.1415·2) + (f32)(i/10f)·3.1415·2` (f64), `s = ((f32)(sin θ·20)·639/639, (f32)(cos θ·20)
    ·479/479)` (f32); tip `p1 = c + s·(f + 1)` (f32), tail `p0 = c + s·(f + 1.5)` (f64 → f32).
  - `DrawArrowAbsolute(p0, p1, A << 24 | 0x735f5f, 2.0)` (ELF 0x6efa10, exe 0x593cb0): `d = (p1 − p0)/|p1 − p0|`
    (f32 squares, f64 sqrt), `n = (−d.y, d.x)·w`; the shaft `nyilszar` quad p0 ± n to p1 ± n − 10d, the head
    `nyilhegy` triangle tip p1 (uv 0.5, 1), base `p1 ± 2.5n − 10d` (uv (0, 0) / (1, 0)); z = rhw = 0.5;
    `polygons(…, 1)`: colour premultiplied (`byte·A >> 8`), A 255, additive. |p1 − p0| is always 10, so the shaft
    has no area: a 10 px head, 10 px across, colour (0x73, 0x5f, 0x5f)·A/256 ≤ (57, 47, 47) added.
- The exe draws them over the wheel's buttons [obs] (no occlusion by the console). `nyilhegy` is a white triangle on
  opaque black (base along v = 0, apex at v = 1).

### 5.7 Open questions
Who reads `resLenyomta` (0xDC5388, written by the ball's grab) [u].

## 6. Orders
`MULTIGET_Research`: re/rules/production-research.md §2.4 (runtime report traces/2026-10-17-research-orders.md).

## 7. Client status
`ig2-client::research` (scene, lists, labs, lasers, presses, button lines `Lines`/`draw_arrow`, tooltips
`model_tip`, `ResearchModels::centres`/`models_under`, input `Research::{buttons, poll, edge, gy, mouse_old,
ball_grip, model_grip}`, `Research::press(.., left)`, sounds per audio.md §6, keys `window.rs` `research_key`/`research_key_seen`); lights
`ig2-client::lighting` (all four lights per vertex; the bytes mapped into the sRGB frame by `spy_scene::gamma`), the
row frame `kit::design_frame` (`ROW_EDGE` nudge for the device-row rule, `DesignFrame::at_once`), faders
`kit::fader`/`fader_design`, lasers `kit::laser` (sparks from a local generator). Orders:
`ig2_sim::game::ResearchOrder` (verified byte-exact). The button lines match a probe run bit for bit
(`probes/research_lines.js`, `button_lines_match_the_exe`); the ball and models turn with the right-button drag as
the exe's. Missing: the panels' `FadeKerettelKerekitDesign` frames (`GLOBALKELLDESIGN`), the screen-select menu
entry; the info box glass's depth against the left wheel while closing differs; the model hit uses the orbit point,
not `GetObjCenter`. Open deviation rows: 84 (glow/shine rand stream, keep), 359 (keep).
Captures: `run_screendiff.py <install> quick-1.sav research [--script research.json]` (the client in the exe's
language); `--anim-frame F [--anim-closing]` holds `resframe` in both (the tracer writes it before every exe
`Research3DUpdate`, `run_screen.py --res-frame`; closing also sets `resBECSUKAS`, clears `resKINYITVA`); held, the
light clock `reskeringeto` is pinned at 0 (or `--res-keringeto K`) and `resatlatszosag` (exe 0xDC5370) at 0.85;
`--anim-mouse X,Y` rests the pointer. Without a held frame the exe grab waits 1000 frames. Results: diff-results.md.
