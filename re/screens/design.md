# The design screen (`AktKep` 5): ship / vehicle designer (from the ELF decompile, exe checked where noted; the viewing mode verified on captures, §14)

Source: ELF decompiles `_local/decomp/design/0x*.c` (ELF, -O0) and the exe decompiles `_local/decomp/design/exe/`.
Addresses are ELF unless marked exe. Exe data addresses are read off the exe decompile and are marked (i) = inferred
by matching code. Not verified at runtime. Port target: `ig2-client::design` + `ig2-sim` (the `0x33/0x34/0x35` orders).

## 1. Overview and functions
The screen shows the empire's designs (`Tervek[EN]`, 0x84-byte records) as a list on the left (15 rows + scroll bar),
the chosen design's data (bottom panels, top bar), a rotating 3D hull (`Alapok` model) with pod slots on the right
(pod list at x = 508) and four mask buttons (new / redesign / modify / delete, or OK / cancel / auto / clear while
editing). Editing works on a scratch copy (`DesignTervezesMod` != 0); OK sends the copy to the sim (`Multi_Send 0x33`).

| Part | ELF | exe |
|---|---|---|
| `Design_Belep` / `Design_Kilep` | 0x5074e0 / 0x507bb0 | 0x4b1be0 / 0x4b1f20 |
| `Design3DInit` / `Design3DUpdate` (per frame) | 0x5035b0 / 0x503ea0 | 0x4ad360 / 0x4ad8e0 (i: `GFX_Design`'s 2nd call) |
| `GFX_Design` (per frame) | 0x507ed0 | 0x4b4090 |
| `DesignGfx2D_1` (texts, pod rows) / `_2` (popup texts) | 0x4fb840 / 0x4feea0 | 0x4af440 / 0x4b1220 |
| `Design_MouseInit` (hot-spot table) | 0x50b430 | 0x4b3810 |
| `Design_MouseInit2` (per-frame enable/disable) | 0x50be70 | **0x4b34f0** (`FUN_004b34f0`; the README address is right). 0x4b6b90 is NOT it (below) |
| `Design_Main` (input) | 0x509140 | 0x4b2190 |
| `DesignKeyboardHandler` / `DesignKeyboardEscPressed` | 0x508dc0 / 0x509040 | 0x4b1840 / 0x4b17a0 |
| `MULTIGET_Design` | 0x50c430 | **0x4b4d60** (the loop's `Multi_GetNextTipus(0x33,0x3b)` calls at exe 0x4b4d76 and 0x4b573c) |
| `FRISSIT_Design2` (UI tick) / `FRISSIT_Design` (empty `ret`) | 0x50d890 / 0x50db10 | inlined / unknown |
| `DesignCancel` / `RedesignPlan` / `ModifyPlan` | 0x508d30 / 0x508a60 / 0x508b70 | 0x4aee90 / 0x4b6940 / 0x4b5f40 |
| `DesignElnevez` / `DesignPoenosanElnevez` (names) | 0x5083a0 / 0x5087c0 | 0x4aef90 / 0x4b19d0 |
| `DesignTervezesOk` / `DesignEntertNyomott` | 0x50b310 / 0x50c3a0 | 0x4b1b20 / 0x4af2c0 |
| `Design_UjraRendez` / `DesignENValtozott` | 0x5078d0 / 0x507af0 | 0x4b3cb0 (i, Belep call) / ? |
| `NEWGAME_Design` (pod type names) | 0x508210 | ? |
| `DesignSetKiirStrSzorzo` / `DesignGetFaderCoordinates` | 0x4fb2d0 / 0x4ff870 | inlined |
| `InitFaderDesignStruktura` / `FaderDesignFrissito` | 0x547cf0 / 0x547e10 | 0x4d3590 / ? |
| `DesignKeret1` (list frame; research.md 5.4) / `FadeKerettelKerekitDesign` | 0x53f780 / 0x545b60 | ? / ? |

Exe address traps: `0x4b6b90` (ported-names `Design_MouseInit2`) is a different function `(race, base)`: it loops
the `POD%02d` boxes of the base model (`findbox`), `HideObj`s the ones past `GetMaxPodok`, else
`VannakTalalmanyokAPodon` + a pod recolour call (`(race, i, mesh, ..)`; ELF counterpart not identified).
The `FrissitGombTexturak` that `Design_MouseInit2` calls is exe `FUN_004b3f50` (ELF 0x500b30); the ported name 0x4b4690 is another function.

## 2. State (ELF; exe in brackets where known (i))
Design records: `Tervek[EN]` (`Tervek` 0x1432390 [exe 0xb9aa18 + EN*4], `OsszHanyTerv` 0x141b060); record 0x84 bytes (fields seen here):
+0x00 name[]; +0x16 i16 (<0 = deleted); +0x18 u8 listed in the screen; +0x19 u8 obsolete (cannot be modified/redesigned);
+0x1a u8 flag set on sale/replace; +0x1c i32 successor design; +0x20 i32 (build effort; shown as `/0.03/1000/50`);
+0x24 i32 price per ship; +0x28 u8 (extra stat, shown if non-zero); +0x2c, +0x30 i32 (stats);
+0x36 u8; +0x37 u8 flag (one-option modify dialog); +0x38 u8; +0x3c f32 (speed; shown `(int)(v*100+0.5)`, `/4` if +0x38);
+0x40 i16 race; +0x44 i32 base index into `Alapok`; +0x48 i32 hull tech id; +0x4c i32 (pod count; `_76_4_`);
+0x50[1..] u8 tech id per pod slot; +0x5d[1..] u8 count per pod slot; +0x6c i32 units for sale, +0x70 u8, +0x74/78/7c date; +0x80 i32.
Scratch record: `DesignTervTervezett` 0x1432278 [exe 0xb9aa68] (same layout; "-1" in the `Terv*` calls).
`Alapok` 0x117c310 [exe 0x90bbf8 (i)], stride 0x24: +8 model index, +0xc hull tech id, +0x14[pod] pod class per slot.
`Talalmanyok` 0x1433960 [exe 0xb9c0c8], stride 0x114: +0x24 u8 max count, +0x26 bit0 disabled/obsolete, +0x45+EN status (5 = available).

| Name | ELF [exe] | Meaning |
|---|---|---|
| `AktTerv` 0x141b100 [0xb9aa64]; `AktTervMutato` 0x141b104 [0xb9aaec]; `AktHanyTerv` 0x141b0fc [0xb9aa14] | | chosen design index; its rank (1..) in `DesignRendezo`; count listed |
| `DesignRendezo` 0x12c37d0 [0xa3f0a8] | | rank -> design index, `[0]=0`, `[1..]` |
| `DesignTervKezdet` 0x1432270 [0xb9aa0c] | | list scroll offset (first shown row - 1) |
| `DesignTervezesMod` 0x12c37b8 [0xa3f0b4] | | 0 view, 1 new, 2 redesign copy, 3 modify |
| `DesignAktPod` 0x12c37b4 [0xa3f0bc] | | chosen pod slot (0 = none) |
| `CanModify` 0x12c37c8 [0xa3f0c1] | | pods editable; `ModifyPlan` sets it; for modes 1/2 set elsewhere (not found, 13) |
| `DesigndSelect{Pod,Pod2,Alap,Delete,RedesignOk}Mode` 0x12c37bc/bd/be/bf/c0 [Pod2 0xa3f0b2]; `dSelected` 0x12c37c4; `dSelectX/Y/M` 0x1171538/3c/40 | | popup menus (pod tech list at the 3D pod / at the row, base list, delete, modify-ok) |
| `AktAlap` 0x12c67c0, `OldAlap` 0x12c67c4 | | model index of the shown base (`Alapok[base]+8`) / the loaded one |
| `SelectedPod` 0x12c67c8, `OldPod` 0x12c67cc, `KivalasztottTalalmany` 0x12c4114 | | 3D pod pick |
| `desKINYITVA` 0x12c66c1 [0xa3ef8c] open; `desBECSUKAS` 0x12c66c0 [0xa3ef73] closing; `desKELLKINYITAS` 0x11715c0 [0x8f8915] animate; `desELINDULT` 0x12c381d started; `desKilepesHova` 0x12c3824; `DesignElottiKep` 0x12c3820 | | open/close anim |
| `desframe` 0x12c67a0 [0xa3f01c] f32 | | 0..60 open anim frame |
| `deswireframe` 0x12c67a4; `deskeringeto` 0x12c3e18; `des_xy` 0x12c67b0; `desatlatszosag` 0x12c67ac; `desmozgato` 0x12c67a8 | | wire sphere frame, light angle, pulse |
| `AddZoomDes` 0x12c3df4 [0xa3f10c], `ADDZOOMMAXDES` 0x11715bc [0x902f5c] = **321.0f** (0x43a08000), `AddZoomDirDes` 0x12c3e00 | | base slides in/out (z) |
| `SubZoomDes` 0x12c3df8 [0xa3f6dc], `SubZoom2Des` 0x12c3dfc [0xa3f6e0], `SubZoomDirDes` 0x12c3e04, `SubZoom2Lecsokkentve` 0x12c4112 | | tech-model expansion (0..1000, +-140 a frame) |
| `InitiateDelay` 0x12c4110, `DelayVar` 0x12c3e14 | | 30 ticks of hover before the tech models open |
| `LEGELSOBEJOVES` 0x12c381c, `desMessagesUtaniLegelsoBejoves` 0x12c37c1, `NemkellTexturaCsere` 0x12c3e10 | | |
| `DesMelyikGombot` 0x12c67d4, `...Old` 0x12c67d8, `DesGombNew` 0x12c67dc, `DesGombFrame` 0x12c67d0, `DesGombFrameCsokkent` 0x12c67dd | | 3D button press animation (buttons 0..5) |
| `desf` 0x12c3828 [0xa3f110] | | fader struct (8) |
| `design_fader_*` 0x12c3e1c..0x12c3e60 | | five fader rects (5.1) |
| `DesignSzinallitoDesignTablazat` 0x12c37e0 [0xa3f0c8] i32[15] | | per-row text fade-in (-10*i -> 100) |
| `Kord1X/Y` 0x117154c/50 = 15/100; `Kord2X` 0x1171554 = 15; `Kord2Y/Y2/Y3` 0x1171558/5c/60 = 375/390/405; `Kord3X` 0x1171564 = 62; `CsuszkaX/Y` 0x1171568/6c = 137/92; `podkezdx/y` 0x1171544/48 = 508/80; `PodMode2x` 0x1171530 = 400 | [0x902ee4..0x902f0c] (i) | static initial values read from the ELF image |
| `Des_Hangok` 0x11710c0 [exe 0x902a68] (4 `Hang`, stride 0x11c) | | [0] screen21.wav, [1] be1.wav, [2] ki1.wav, [3] screen23.wav (all `screens/design/`) |
| `DesignBrushok` 0x1170fa0 (`AktBrushok`, 4 used; id n at +0x38*n), picture kep 0x13 | | 9x9 cells at src x 0/9/18/27 = minus, plus, greyed minus, greyed plus (ids 1..4) |
| `GLOBALKELLDESIGN` 0x1171e88 = 1; `KELLKINYITASDEFAULT` 0x117b054 [0x912a39] = 1 | | |
| `AktIkon2` 0x1414b50 | | hot-spot id under the mouse |
| `HanyasFajPodTablazatatHasznalja` 0x1171570 | | race -> pod table set: 0,1,2,3,4,5,6,7,8,1,1,... |
| `DDColor` 0x1166cd0 (4 B each) | | 1 grey 0x7f, 2 white, 3 yellow, 4 magenta, 5 cyan, 6 red, 7 light green, 8 blue, 0xc dark grey, 0x22 (127,200,255), 0x27 grey 0x32 (if byte 0 = R) |

## 3. Entry / exit
`Design_Belep`: `desELINDULT=0`; `DesignElottiKep=OldKep`; `desKELLKINYITAS` = 0 if `OldKep` in {10 messages, 1 help, 0xd options}
else `KELLKINYITASDEFAULT & 1`; `desKINYITVA = !desKELLKINYITAS`; `desmozgato` = 29.0f (animate) / 3.0f; `IdoPauseMertAtlepesAlattVan = !desKINYITVA`;
`InitFaderDesignStruktura(desf)`; `DesignSzinallitoDesignTablazat[i] = -10*i` (i<15); `NemkellTexturaCsere=0`; `InitSounds(Des_Hangok,4)`;
the two callback carriers cleared; `AktpodCuccai[i].flag=0` (i<40). Coming back from messages/help/options when their "before" screen was 5:
`AddZoomDirDes=0; AktAlap=OldAlap; desMessagesUtaniLegelsoBejoves=1`; else `des_xy=deskeringeto=deswireframe=0; AddZoomDirDes=AktAlap=-1`.
`OldAlap=-1; AddZoomDes=ADDZOOMMAXDES; VannakBetoltveTalalmanyok=0; InitAlapok2(); PodLightId+8=10000;` then
`SubZoom2Lecsokkentve=CanModify=SubZoom2Des=VoltmarkijelolveEbbenaframeban=VoltPodEbbenaframeban=InitiateDelay=LEGELSOBEJOVES=0`,
`DesMelyikGombot(Old)=-1, DesGombNew=0, SelectedPod=OldPod=-1, desLenyomta=desBECSUKAS=DesignPodKiuritomod=0, DesignAktPodListabolkivalasztott=-1`;
`AktBrushok=DesignBrushok; AktBrushHany=4`; all `Designd*Mode=0; DesignTervezesMod=0; DesignTervKezdet=0; DesignAktPod=0`;
`Design_UjraRendez(); AktTerv = DesignRendezo[AktTervMutato]` (the old `AktTervMutato` is kept); `Design3DInit()`; if not `desKINYITVA` play `Des_Hangok[0]` (screen21.wav);
SelfPlaying demo: `FRISSITES(); DemoMode_Mode=1; DemoMode_Init()`.

`Design3DInit`: dirs `vector/design/screen/`, `textures/design/screen/`; `FreeMainXVB/CreateMainXVB`; `fadertexture = loadtexture("dt.bmp")`; `InitHatterFader`;
`LoadBox("desbck.xvb")` -> `AddToScene(.., mainxvb, 1.1, DesMainMesh)`; sub-mesh 6/0/0 material copied to `dGombok[0..5]` (0x60 B each, `[0x5c]=0`);
the six buttons = sub-mesh 6 -> i -> 0 get `dGombok[i]` (originals kept in `Gombokp[i]`, restored by `Kilep`); `DesWireGomb` = sub-mesh 0 (props +0x40, -4);
`MoveObj(Main, 0,0,-650)`; `RotateObj(Main,0,0,0)`; textures `des_panel1/2/3.bmp` (`MainText1..3`); `fxn=30.0f, fyn=25.0f, fzn=-210.0f, zbm=0.1f`;
viewport `HatarX1 = -k, HatarX2 = k+639` with `k = ((W*480/H) - 640)/2` (ints), `HatarY 0..479`, `HatarXm = X2-X1+1`, `HatarYm = 480`, `ModifyViewPort`;
lights: ambient 0x141414, parallel 0x808080 dir (0,0,-10); sub-meshes 1..5 of Main hidden; dirs back to `vector/`, `textures/`; `desOldalsoFenyekBe`;
`desInitNullBoxes` (the exe inlines it: `GFX_AllandoInit1; InitFader1; AddNullBox+SetRenderOther(DesignGfx2D_1); InitFader2; AddNullBox+SetRenderOther(DesignGfx2D_2); GFX_AllandoInit2; InitFader3`
= the layer order: fader 1, 2D_1, fader 2, 2D_2, top bar, fader 3); `desframe = desKELLKINYITAS ? 0 : 60.0f`; `desGyX=desGyY=0`.

`Design_Kilep` (target is not messages/help/options, `KELLKINYITASDEFAULT`=1): `IdoPauseMertAtlepesAlattVan=1; AddZoomDirDes=1`; demo mode 2; remember `AktKep` in
`desKilepesHova`; `AktKep=5; desBECSUKAS=1`; play `Des_Hangok[3]` (screen23.wav); loop `{UpdateFrame; FRISSITES}` (forced) until `desframe` has been 0 for two passes
(`VoltNulla` static: +1 per pass at `desframe==0`; at 2 -> `UnjelolPod()`); then `AktKep` = the target. If the target is 10/1/0xd: `KilepesElott=1; GFX_Design(); SaveScreen(2); KilepesElott=0`
(background kept for those screens), else `desOldalsoFenyekKi`. Always: restore the six button materials from `Gombokp`, `UnLoadKep(0x5f)`, `DeInitSounds`, `RemovePodLightok`,
`FRISSITES_IGAZIT`, `IdoPauseMertAtlepesAlattVan=0; desKINYITVA=0; desRemoveNullBoxes`.

## 4. Hot-spots (`Design_MouseInit` 0x50b430 / exe 0x4b3810; `Mouse_Szoveg_Set(id, colour, x, y, w, h, tooltip, onRelease, 1)`)
`MouseSzovegSzam=0x56`, mask picture kep `0x5f` (`MouseHanyasIkonKep`; `LoadKepToMemory`, `GetMouseKepAddr`); `SetFirstMenu`. Colour != 0 = hit only where the mask has that colour. Tooltip = `GetString(5, 1, n)` (n read from the exe).
`AktIkon2` = id; ids with `onRelease`=1 fire on release (mainscr.md).

| id | colour | rect x,y,w,h | tip n | rel. | `Design_Main` action |
|---|---|---|---|---|---|
| 1 | 2 | 0,0x128,0x42,0x30 | 1 | 0 | new design: base popup (6.1) |
| 2 | 3 | 0x41,0x127,0x43,0x31 | 2 | 1 | `RedesignPlan()` |
| 3 | 4 | 0,0x157,0x42,0x31 | 3 | 1 | `ModifyPlan()` |
| 4 | 5 | 0x41,0x157,0x44,0x32 | 4 | 0 | delete/sell popup |
| 5 | 2 | 0,0x128,0x42,0x30 | 5 | 0 | OK |
| 6 | 3 | 0x41,0x127,0x43,0x31 | 6 | 1 | `DesignCancel()` |
| 7 | 4 | 0,0x157,0x42,0x31 | 7 | 1 | `TervAutoDesign(EN,-1,0)` + recalc |
| 8 | 5 | 0x41,0x157,0x44,0x32 | 8 | 1 | `TervClear(EN,-1)` + recalc |
| 9 | 0 | 5,0x5d,0x7b,0xe9 | 9 | 0 | design list click (select) |
| 10 | 0 | CsuszkaX,CsuszkaY,10,0xea (=137,92) | 10 | 0 | scroll bar |
| 0x1f | 6 | 0,0,0x20,9 (set again by `Design_MouseInit2`; moved at run time) | 0xe | 0 | pod count + |
| 0x20 | 1 | 0,0,0x20,9 | 0xd | 0 | pod count - |
| 0x33+2k | 0 | podkezdx+0x55, podkezdy+1+15k, 0x14, 0xe | 0xd | 0 | pod k count - (k = 0..15) |
| 0x34+2k | 0 | podkezdx+0x6e, same y, 0x14, 0xe | 0xe | 0 | pod k count + |
| 0x54 | 0 | 0x1f2,0x5c,0x8a,0xea (498,92,138,234) | 0xb | 0 | pod list panel (right) |
| 0x55 | 0 | 0x1f2,0x5c,0x8a,0xea, empty text; re-set each frame by `PodKivalaszto` at the hovered 3D pod (centre = mouse, size 2r; tip `GetString(5,1,0xf)` clickable / `0x10` locked) | - | 0 | 3D pod pick (in `PodKivalaszto`, not `Design_Main`) |
| 0x56 | 0 | Kord3X-3, Kord2Y-3 (59,372), 0x66, 0xe | 0xc | 0 | name field: starts text input |
Then `Mouse_Szoveg_Off_Range(0xb,0x52)`, `Mouse_Szoveg_Off(0x55)`. Exe `MouseInit` also does `if (PodLightId+8 != 10000) { resOldalsoFenyekKi(&PodLightId); PodLightId+8 = 10000 }` (ELF none).

`Design_MouseInit2` (every frame; ELF 0x50be70, exe 0x4b34f0): if `SubZoomDes != 0`: `Mouse_Szoveg_Off_Range(1, MouseSzovegSzam+1)` (all off); else with `d` = view/scratch design:
- mod 0: 1 on; 2,3,4 on only if `AktHanyTerv>0 && Talalmanyok[Alapok[d.base]+0xc].+0x26 bit0 == 0 && d.+0x19 == 0`, else off; 5..8 off; 9,10 on.
- mod != 0: 1..4 off; 5..8 on; 9,10 off.
- mod 0: 0x1f..0x32 off. mod != 0: 0x1f,0x20 off, then if `SubZoomDes==SubZoom2Des==AddZoomDes==0 && DesignAktPod != 0 && CanModify` and
  `min(GetMaxRarakhato(race,base,pod), Talalmanyok[d.tech[pod]].+0x24) > 1`: set 0x20 (colour 1, text `GetString(5,1,0xd)`) and 0x1f (colour 6, `0xe`) at (0,0,0x20,9); then `Off_Range(GetMaxPodok+0x1f, 0x32)`.
- if `desKINYITVA && !desBECSUKAS`: 0x54 on iff `SubZoomDes==0 && GetMaxPodok(race,base) > 0`, else off.
- `FrissitGombTexturak()`, `Mouse_Szoveg_Off(0xc)`.
`FrissitGombTexturak` (exe 0x4b3f50; skipped while `DAT_00a3f6ec`): mod 0: the 3D buttons 0..3 (ids 1..4) take the hover colour `DAT_00a3f010` when `FUN_00510860(id)` (hover test) else `...18`; mod != 0: ids 5..8 take `DAT_00a3f014`; ids 0x1f/0x20 likewise (buttons 4, 5).

## 5. Draw list
### 5.1 `Design3DUpdate` (3D scene, per frame; ELF 0x503ea0) in order
1. `race` = `d.+0x40`. If `OldAlap != AktAlap || NemkellTexturaCsere`: `AddZoomDirDes=1; SubZoomDirDes=0; SubZoomDes=0`.
2. `DelayVar >= 30`: `SubZoomDirDes=1; DelayVar=0; InitiateDelay=0;` if `CanModify && VannakTalalmanyok()`: `RemoveHozzavaloTalalmanyok(); BetoltHozzavaloTalalmanyok()`.
3. If `desKINYITVA`: `SubZoomDirDes==-1`: `SubZoomDes -= 140` (<=0 -> 0, dir 0); `==1`: `+= 140` (cap 1000); `AddZoomDirDes==-1`: `AddZoomDes -= 40` (<=0 -> 0, dir 0, `LEGELSOBEJOVES=1`); `==1`: `+= 40`; at `>= ADDZOOMMAXDES` (and not closing): `RemoveHozzavaloTalalmanyok; HideAlap(OldAlap); OldAlap=AktAlap; LoadAlapVektor(AktAlap);` if `desMessagesUtani...`: `PodElsotetito(race)`; then (flag clear) dir = -1 and `OldPod=SelectedPod=-1`, else dir 0, `AddZoomDes=0`. `UnHideAlap(OldAlap)`.
4. `SetFrame(DesMainMesh, desframe)`; `desELINDULT=1`; at `desframe==60`: `deswireframe += 0.25` wrapping to 0 at 10; `SetFrame(DesWireGomb, desframe+deswireframe)`.
5. Opening finished (`(!desKINYITVA || desBECSUKAS) && desframe>=60 && AktKep==5`): `desKINYITVA=1; FRISSITES_IGAZIT(); IdoPauseMertAtlepesAlattVan=0`.
6. Button animation (open, not closing, `desframe>=60`): on `DesGombNew`: clear it, `DesGombFrameCsokkent=0`, `desSetGombFrame(DesMelyikGombotOld,0)` (if != -1), `Old=Mel`; if `DesMelyikGombot != -1`: `DesGombFrame += 7` (cap 10; at 10 -> falling), falling `-7` to 0; `desSetGombFrame(DesMelyikGombot, DesGombFrame)`.
7. `DesignGetFaderCoordinates()`: `t` = 1 (not closing) or `desframe/60*2 - 1` (closing); fader rects (x, y, sz, ma): F0 (5, 209-117t, 124, 234t); F1 (471, 398-33t, 166, 72t); F2 (5, 398-33t, 174, 72t); F3 (499, F0.y, 136, F0.ma); F4 (320-160t, 51-29t, 320t, 57t) (y/ma/F4 f32; x/sz ints).
8. `DesWireGomb` transparency: `desframe<=30 ? 1.35 - 2*desframe/60 : 0`.
9. Faders (if `desframe>30 || !desBECSUKAS`): `KirakFader(i, desf, rect_i, 0, b_i*54, b_i*90, a1, 0, b_i*90, b_i*130, a2, false, 0.005, 0.005)`, `b_i = desf[0x168+4i]` (8), `s = desBECSUKAS ? 0 : 1 - desframe/60`, `a1 = s*155+100`, `a2 = s*225+30`.
10. Mouse down over the base (`AlapfolottLenyomva`) -> `desMouseOld = mouse; desLenyomta=1`. `desGyX,Y *= 0.8`; `SubZoom2Des` -140 (<=0: 0, `SubZoom2Lecsokkentve=1`); sub-zoom rules: `SubZoom2Lecsokkentve && dir==1 && Kival != -1` -> dir -1; on release with dir 1: `Kival == -1` -> -1; `Kival != -1 && SubZoom2Des==0 && SubZoomDes==1000` -> `SubZoom2Des = 1000`; on release `InitiateDelay = 0`.
11. Open, not closing, `LEGELSOBEJOVES`: `MoveObj(DesMainMesh, 0, 0, AddZoomDes - 650)`. `CanModify && VannakTalalmanyok()`: `MegkeresHozzavaloTalalmanyok(); CheckIfTalalmanyfolottvan()`.
12. `desKELLKINYITAS`: `MoveObj(DesWireGomb, desInitPos1(0.0), 0.0, 200.0 + (1 - desframe/60)*1000)`; wire transparency `1 - desframe/60`.
13. Camera: `kamerapont=(0,0,600)`, `forgaspont=0`, `xcenter=320`, `ycenter = desKELLKINYITAS ? 250 - desframe*10/60 : 240` (*479/479); `desMozgatMainFenyek()` (side lights, ELF 0x500010); `AddZoomDes==ADDZOOMMAXDES` -> `NemkellTexturaCsere=0; FrissitGombTexturak()`.
14. `desMeshKiBe(7, ...)`: sub-mesh 7 hidden when open and not closing; otherwise shown unless `(closing && desKilepesHova==9) || (!desKINYITVA && DesignElottiKep==9)`; `Copy3hatterkep(0x34, 0xff000000 | c*0x010101)` with `c = 255 - (int)(desframe/60*128)`.
15. `Update3D(0,0,0)` (draws the scene incl. the two callbacks 5.3/5.4); `SzinezPodok(race, OldAlap)` if `OldAlap != -1`.
16. Base spin: release clears `desLenyomta`; each frame `matrixforgat(M, out, desGyX, desGyY)` on the base (`AlapokTsc + OldAlap*0x140`: obj +8, matrix +0x14); dragging: `desGyX = 0.9*desGyX + 0.1*(oldx-mx)/30` (same y), old = mouse; idle (no drag): `matrixforgat(.., 0.002, 0)` per frame.
17. `GLOBALKELLDESIGN` (=1) overlays: five `FadeKerettelKerekitDesign(rect_i, 0, 54b,90b,a1, 0, 90b,130b,a2, fmod(desf[0x4d8]*k_i, 1), 1.0, 0)` with `k` = 1, 0.5, 1/1.5, 1.8, 1.2 (case 0 = a moving scan band, 4 px steps) and, for each fader with `desf[0x3e0+4i] != 0`, the same with `param13 = 1 - desf[0x3e0+4i]`, `param15 = desf[0x458+4i]` (cases 1..5: line / corner effects). `desMessagesUtaniLegelsoBejoves=0`.

### 5.2 `GFX_Design` (per frame) 0x507ed0
`GFX_DateMoneyScrollFade(); Design3DUpdate(); if Mouse_Felenged: DesigndSelectPodMode2=0;` race = scratch race (mod != 0) or the design race. If `!desKINYITVA || desBECSUKAS`: `PodElsotetito(race)` only.
Else: scroll bar: `AktHanyTerv<16`: thumb 234 px full; else `h = 3510 / AktHanyTerv` (int64 division), `off = DesignTervKezdet*(234-h)/(AktHanyTerv-15)`; if `h<234`: track `FadeKerettelKerekit(137, 92, 10, 234, 0, 54, 90, a1, 0, 90, 130, a2, false, 0...)` and thumb `(138, 93+off, 8, h-2, same)`, with s, a1, a2 as 5.1 (s=0 when closing), `Mouse_Szoveg_On(10)`; else `Mouse_Szoveg_Off(10)`. Then `PodKivalaszto(race)` (3D pod pick, exe 0x4b6260) and `CheckIfTalalmanyfolottvan()`.

### 5.3 `DesignGfx2D_1` (callback; 0x4fb840 / exe 0x4af440): `WaitForDrawing()`; `d` = view/scratch design, `base = Alapok[d.base]`
`KiirStr(text, x, y, maxChars, 2, 4, colourIdx)` (size 2, flags 4); `KiirInt(v, x, y, width, 2, flags 4 or 6, colourIdx)`. `DesignSetKiirStrSzorzo(n)`:
`KiirStrSzinSzorzo = max(0, DesignSzinallitoDesignTablazat[n])` for n<15 else 100 (colour brightness %, fades each text in); the number in parentheses below is `n`. Labels = `GetString(5, 2, k) + ":"` (":" at exe 0x852aec).
Only if `!desBECSUKAS`:

| # | Content | Pos | Colour |
|---|---|---|---|
| 1 | rows `i = DesignTervKezdet+1 .. min(AktHanyTerv, DesignTervKezdet+15)`: design name `Tervek[DesignRendezo[i]]` (16 chars); n = i-Kezdet-1; mod != 0: factor forced 50 | (Kord1X, Kord1Y + (i-Kezdet)*15 - 15) = (15, 100 + 15*row) | 1; 0xc if hull tech `Talalmanyok[d_i.+0x48].+0x26&1` or `d_i.+0x19` (obsolete) |
| 2 | mod 0, `Kezdet+1 <= AktTervMutato <= min(AktHanyTerv, Kezdet+15)`, open: `DesignKeret1(Kord1X-5, Kord1Y + (AktTervMutato-Kezdet)*15 - 0x12, 0x72, 0xd, 0x22)` | | frame colour 0x22 |
| 3 | `AktAlap = base+8`; label k=1 (8) | (Kord2X, Kord2Y) 7 ch | 1 |
| 4 | name (9) | (Kord3X, Kord2Y) 16 ch | 2 |
| 5 | label k=2 (10) | (Kord2X, Kord2Y2) 7 | 1 |
| 6 | (11) `d.+0x37` -> "-" else `(int)(f32 d.+0x3c*100+0.5)` (`/4.0` double if `+0x38`) | KiirInt (Kord3X, Kord2Y2), w 4 | 2 |
| 7 | (12/13) label k=3 + `d.+0x2c` | (Kord2X, Kord2Y2+15) 10 ch; value (Kord2X+0x46, same) w 5 | 1 / 2 |
| 8 | (12/13) label k=4 + `d.+0x30` | y = Kord2Y2+30 | 1 / 2 |
| 9 | (0) label k=5 | (0xa8, 0x1b) 6 | 1 |
| 10 | (1) hull tech name `Talalmanyok[d.+0x48]` | (0xd7, 0x1b) 16 | 2 |
| 11 | (2) label k=6 | (0x147, 0x1b) 16 | 1 |
| 12 | (3) `(int)(((double)d.+0x20 / 0.03) / 1000.0 / 50.0)` | (0x1a7, 0x1b) w 8, flags 6 | 2 |
| 13 | (4,5) label k=7 + `d.+0x24` (price) | (0xa8,0x2a) 14; (0xd7,0x2a) w 10 | 1 / 2 |
| 14 | (6,7) label k=8 + `HanyHajoVanATervbol(EN,0,AktTerv)` (ships of the design) | (0x147,0x2a) 19; (0x1bd,0x2a) w 4 flags 6 | 1 / 2 |
| 15 | `Tervek[EN][AktTerv].+0x28 != 0`: label k=9 + that byte | (0x147,0x39) 19; (0x1bd,0x39) w 4 flags 6 | 1 / 2 |
| 16 | `DesignAktPod < 1`: label k=10 (7) + "---" (8); label k=0xb (9) + hull tech name (10) | (0x1ea,Kord2Y) 4 / (0x208,Kord2Y) 16; (0x1ea,Kord2Y2) 5 / (0x208,Kord2Y2) 16 | 1 / 2 |
| 17 | `DesignAktPod >= 1`: label k=10 (6 ch, 7) + pod type name = table at 0x1166fc4 (stride 0x54) `[KeresPod(PodFajok[(DesignAktPod-1)*8 + AktAlap*0x60 + (HanyasFaj...[race]-1)*0xa20])]` (8); names filled by `NEWGAME_Design` from `GetString(0x13, 1)` for class < 0x33 else `(0x13, 2)` | (0x1ea,Kord2Y), (0x208,Kord2Y) | 1 / 2 |
| 18 | pod empty (`d.+0x50[pod]==0`): k=0xb "---" (9, 10) at (0x1ea/0x208, Kord2Y2); k=0xc (11) at (0x1ea, Kord2Y3) 6 ch, "---" (12) at 0x212 w 2; k=0xd (13) at 0x230 4 ch, "---" (14) at 0x24f | | 1 / 2 |
| 19 | pod fitted: k=0xb + tech name (0x208, Kord2Y2); k=0xc + count `d.+0x5d[pod]` at (0x212, Kord2Y3) w 2; k=0xd + max = `min(GetMaxRarakhato(race,base,pod), Talalmanyok[tech].+0x24)` at (0x24f, Kord2Y3) | | 1 / 2 |

Then (open, not closing): `VoltPodEbbenaframeban=0; Mouse_Szoveg_Off_Range(0x33,0x52)`;
- `SubZoomDes != 0` (tech models open): `UnjelolMindet()`; for each `AktpodCuccai[i]` (stride 0x28, flag +0, tech id +0x24; `Osszhanycucc` entries): row `(podkezdx, y = podkezdy + (i+1)*15)`: tech name at (podkezdx, y+5), colour 2; if the mouse is in the row (`y < my <= y+15`, `x < mx < x+0x73`) and `SubZoomDes==1000 && SubZoom2Des==0 && !VoltmarkijelolveEbbenaframeban`: `KijelolTalalmany(i)`, `DesignKeret2(x-3, y+2, 0x73, 0xd, 1)`. Then the "clear" row after the last: `GetString(5,2,0xf)` at (podkezdx, podkezdy+0x2d+last*15 + 5) 14 ch colour 2; hovered: `DesignKeret2(.., 2)` and `DesignPodKiuritomod=1` else 0.
- else rows `k = 1..GetMaxPodok(race, base)`, `y = podkezdy + 15k`, `z = AddZoomDes/ADDZOOMMAXDES` (f32):
  - empty slot: `n1 = (1-z)*13` (>=1: pod type name at x `podkezdx + z*50 - 4`, y+5, n1 chars, colour 0xc); `n2 = (1-z)*5` (>=1: `GetString(5,2,0xe)` at x `podkezdx - 6 + 96 - 35z + 4`, colour 2).
  - fitted: `n = (1-z)*16` chars of the tech name at `(podkezdx + 50z - 4, y+5)` colour 1; count (`z<0.6`): "-" (2 ch) when 0, else `KiirInt(count)` colour 1, or 0x22 when `count >= max`; x `podkezdx + 96 + 12 - (cells*6)/2 - 35z` with cells = 1 (count<10) or 2; if mod != 0 and max > 1: hot-spots `2k+0x33`/`2k+0x34` on; brush 1 (z<0.8, x `podkezdx+96-4-35z`, y+4) and brush 2 (z<0.2, x `podkezdx+96+19-35z`); else brushes 3 / 4 (greyed) at the same positions.
  - after the loop: `!VoltPodEbbenaframeban && DesignAktPod != 0`: `DesignKeret2(podkezdx-7, podkezdy + DesignAktPod*15 + 2, 0x84, 0xd, 1)`.
- Popup frames (`FadeKerettelKerekit(x,y,w,h, 0, 10,0,0x3c, 0, 5,0,0x1e, false, 0...)` between `FaderHanyadikMenet=2` and 1): pod-tech popup (`DesigndSelectPodMode`, at (dSelectX-6, dSelectY-2), 0x82 x (n*13+4)); row popup (`...Mode2`, at (PodMode2x-6, podkezdy + DesignAktPod*15), same size; n = valid techs); base popup (`AlapMode`, at dSelect, n = bases available); delete popup (`(dSelectX-6, dSelectY-2, 6*len+10, 0x11)`); modify-ok popup (`(dSelectX-6, dSelectY-2, 8*dSelectM+10, (((d.+0x37&1)==0)+1)*15+4)`, `dSelectM` = the longer of the two option strings in chars).
- Opening/closing (`desframe/60` not 0/1, `!desKINYITVA`): five `DrawLaser(x, y, sz, ma, 0x4c,0x4c,0x4c, f)` with corner origins `LaserPos` = (0,0), (639,479), (0,479), (639,0), (320,0) for faders 0..4 (F3 uses F0 y and ma), `f = desframe/60`; `desframe != 60`: `DrawSparks()`.

### 5.4 `DesignGfx2D_2` (callback 0x4feea0 / exe 0x4b1220): popup texts, open and not closing
Items `KiirStr(name, x, y, 16, 2, 4, c)`, `c` = 2 if the item is `dSelected` else 1, 13 px pitch:
pod-tech popup: valid techs (`TalalmanyRarakhato(base, t, DesignAktPod, 5, EN, race)`, t = 0..0xc0) at (dSelectX, dSelectY + 3 + 13n); row popup at (PodMode2x, podkezdy + 5 + 15*DesignAktPod + 13n); base popup: bases 1..24 whose hull tech has status `Talalmanyok[t]+0x45+EN == 5` and `+0x26&1 == 0`, hull tech name, at (dSelectX, dSelectY+3+13n); delete popup (1 item, 40 chars): `DesignTorlesValasztasNev[0] + str(TervHajoinakCsokkentettAra(EN,0,AktTerv))` at (dSelectX, dSelectY + 3); modify-ok popup (size 3 flags 4, strlen chars): items 1..(`d.+0x37` ? 1 : 2) at (dSelectX, dSelectY - 12 + 15*i): item 2 (or item 1 when `+0x37`): option text + " - $" + cost (`DesignModifyOkValasztasNev + 8` if affordable, `+ 0x10` if not), `cost = max(0, (HanyHajo*scratch.+0x24 - TervHajoinakAra)*2)`, affordable = `cost <= MoneyMennyi(EN,0) || cost < 1`; other item = `DesignModifyOkValasztasNev[0]`.

## 6. Input (`Design_Main` 0x509140 / exe 0x4b2190)
Each call: `old = AktTerv; DesignPodKiuritomod = 0`. Click sound = `PlaySoundGlobal(0)` on `Mouse_Lenyom` for every button below (exe agrees). `KezdoIdo` = one function-static counter shared by all repeat buttons: reset to 0 on press, +1 per call while held; acts when `== 1 || > 7`.

### 6.1 per hot-spot
| id / condition | Action |
|---|---|
| (open, !closing) `AktIkon2==0x54 && SubZoomDes==0` | pod list. Press: `dSelected=-1`. Release (`Mouse_Felenged`): if `dSelected != -1 && CanModify`: 0 -> `d.+0x50[pod]=0; d.+0x5d[pod]=0; TervUjraSzamol(EN,-1,false,true)`; else (t in 0..0xc0 with `TalalmanyRarakhato` true and `t==dSelected`): tutorial event 0xb if `t==0x12 && Alapok[base]+0xc == 5`; `+0x50[pod]=t; +0x5d[pod]=min(GetMaxRarakhato(race,base,pod), Talalmanyok[t].+0x24)`; `TervUjraSzamol`. Then `dSelected=-1`. Rows `k=1..GetMaxPodok`: left held, `!Felenged`, (`Lenyom || !CanModify`): mouse in `(podkezdx, +0x73) x (podkezdy+15k, +15]` -> `DesignAktPod=k` (always) and, if `CanModify`, `DesigndSelectPodMode2=1; Mouse_Megfogva=1`. While `Bal && Megfogva && !Lenyom`: `dSelected=-1`; for the valid techs in order (`n` = 0,1,..): x in `(PodMode2x, +0x6e)` and y in `[n*13 + podkezdy + DesignAktPod*15 + 2, +0xf)` -> `dSelected = t`. (The 3D pod pick sets `DesigndSelectPodMode` in `PodKivalaszto`.) |
| `AktIkon2==1 && (AlapMode or Lenyom)` | new design. `DesMelyikGombot=0; DesGombNew=1`. While held: first frame `n` = bases `Alapok[1..24]` with hull-tech status `Talalmanyok[t]+0x45+EN == 5` and `+0x26&1==0`; popup origin `dSelectX = mx+12`, `dSelectY = 0x16e - 13n`; `dSelected` = base index of the item under the mouse (x in `[dSelectX, +0x78]`, rows 13 px); `AlapMode=1; Mouse_Megfogva=1`. On release: if `dSelected > 0`: `SetSecondMenu(); DesignTervezesMod=1; TervClear(EN,-1); scratch.+0x44 = dSelected; scratch.+0x40 = PlayerFaj[EN] (byte); TervUjraSzamol; DesignElnevez(EN, Talalmanyok[Alapok[sel]+0xc].name, scratch)`; name edit. `AlapMode=0`. |
| 2 | `RedesignPlan()` (0x508a60): sound if pressed; `DesMelyikGombot=3; DesGombNew=1; SetSecondMenu(); DesignTervezesMod=2; TervCopy(EN, AktTerv, -1); DesignElnevez(EN, name(AktTerv), scratch)`; name edit. |
| 3 | `ModifyPlan()` (exe 0x4b5f40; ELF 0x508b70 not read): sound; `DesignTervezesMod=3; TervCopy(EN,AktTerv,-1); CanModify=1;` (exe: drops a pod light, `PodLightId+8=10000`); `DesignAktPod=0`; other pod-select vars reset (exe `DAT_00a3f040/44/4c`, `DAT_00a3ef8e=1`); the scratch name is version-bumped: trailing digits parsed (stopping at v/V), `if (digits != 0 && the char before them is v or V) sprintf("v%d", n+1)`; `IOString` pointer = scratch (`DAT_00bae2c8`; edit not started). |
| 4 `!TutorialPaused && (DeleteMode or Lenyom)` | delete: `DesMelyikGombot=5; DesGombNew=1`. While held: first frame `dSelectX = mx+16; dSelectY = my-8`; `dSelected=-1`; text = `DesignTorlesValasztasNev[0] + str(TervHajoinakCsokkentettAra(EN,0,AktTerv))` (len*6 px wide, 13 high) -> item 1 `dSelected=1`; `DeleteMode=1; Megfogva=1`. Release: `dSelected>0` -> `Multi_Send(0x34, EN, 4,&AktTerv, 4,&(dSelected+1)=2, 4,&zero)`; `DeleteMode=0; Design_UjraRendez()` (always). |
| 5 `&& SubZoomDes==0` | OK. Mod 3 (pressed or held): `DesMelyikGombot=0; DesGombNew=1; TervUjraSzamol`; popup at (mx+12 capped to `(dSelectM+1)*-8 + 0x27c`, my-0x22 or my-0x13 if `Tervek[EN][AktTerv].+0x37&1`), width `dSelectM*8`; `cost = max(0,(HanyHajoVanATervbol(EN,0,AktTerv)*scratch.+0x24 - TervHajoinakAra(EN,0,AktTerv))*2)` (`dSelectM` = longer of the option strings); items 1..2 (1 if `+0x37`), 15 px; release: `dSelected==2` or (`+0x37` and `dSelected==1`): if `cost <= money or cost<1` -> `DesignTervezesOk(false)`; else `dSelected==1` -> `DesignTervezesOk(true)`; `RedesignOkMode=0`. Mod != 3: on press: `DesMelyikGombot=0; DesGombNew=1; TervUjraSzamol;` if `AktAlap==6` tutorial event 0xc per filled pod; `DesignTervezesOk(false); Mouse_FelengedesMegvar=1`. |
| 6 | `DesignCancel()` |
| 7 | `DesMelyikGombot=4; DesGombNew=1; TervAutoDesign(EN,-1,0); TervUjraSzamol(EN,-1,false,true)` |
| 8 | `DesMelyikGombot=5; DesGombNew=1; TervClear(EN,-1); TervUjraSzamol` |
| 9 `!Felenged && (Megfogva or Lenyom) && Bal` | list click/drag: on press `PlaySoundGlobal(0)` and `PlaySound(Des_Hangok[2])` (ki1.wav). `AktTervMutato = (my - Kord1Y + 5)/15 + DesignTervKezdet + 1`, clamped to `> Kezdet`, `<= AktHanyTerv`, `<= Kezdet+15`; `AktTerv = DesignRendezo[AktTervMutato]`; `Megfogva=1`. |
| 10 `Megfogva or Lenyom` | scroll bar: `AktHanyTerv<16` -> `Kezdet=0`; else `h = 3585/AktHanyTerv` (int64; the draw uses 3510); `Kezdet = (((my-0x22) - h/2)*(n-15) + (0xef-h)/2) / (0xef-h)` clamped `[0, n-15]`; `AktTervMutato` clamped into `[Kezdet+1, Kezdet+15]`, `AktTerv` follows; `Megfogva=1`. |
| 0x1f (+) / 0x20 (-) `SubZoomDes==SubZoom2Des==AddZoomDes==0 && DesignAktPod != 0` | sound on press; held, `!Felenged`: `KezdoIdo` rule; 0x1f: `if count < min(GetMaxRarakhato(race,base,pod), Talalmanyok[tech].+0x24) count++` (race/base from the scratch); 0x20: `if count != 0 count--`; `TervUjraSzamol`; `DesMelyikGombot` 2 (+) / 1 (-); `Megfogva=1`. (pod = `DesignAktPod` at entry.) |
| 0x33..0x52 even (-) / odd (+) | per pod `k = (id-0x33)/2`: same rule on pod k (+ limited like above); no `DesMelyikGombot`. |
| 0x56 | name edit: `IOMode=1; IOAzonosito=0x33; IOString = (mod 0 ? Tervek[EN][AktTerv].name : scratch.name)`; `IOMaxLen=0x10; IOX=Kord3X; IOY=Kord2Y; IOCharSize=2; IOColor=2; IOKurzor=strlen; IOGombnyomasraTorles=1` (first key replaces). **In mod 0 this edits the stored design name directly, with no `Multi_Send`.** |
End of `Design_Main`: scroll window fix (`Kezdet = Mutato-1` if `Mutato <= Kezdet`; `Kezdet = Mutato-15` if `Kezdet+15 <= Mutato`; `Kezdet>=0`); if `AktTerv != old`: `DesignAktPod=0; NemkellTexturaCsere=1`.
The name edit also starts from `RedesignPlan` and the new-design release (same `IO*` values, `IOString = scratch`). `DesignEntertNyomott` (Enter in the text input): `IOMode=0` if `IOAzonosito==0x33`.

### 6.2 Keyboard (`DesignKeyboardHandler(vk, ch, down)` 0x508dc0 / exe 0x4b1840)
| key | Action |
|---|---|
| Up (0x26) `down && AktTervMutato>1` | `m = AktTervMutato-1; if m <= Kezdet: Kezdet = AktTervMutato-2; AktTerv = DesignRendezo[m]; AktTervMutato=m` |
| Down (0x28) `down && Mutato < AktHanyTerv` | `Mutato++; if Kezdet+15 < Mutato: Kezdet++; AktTerv = DesignRendezo[Mutato]` |
| char 'r' (0x72) `down && mod==0 && AktHanyTerv>0 && hull tech +0x26 bit0==0 && d.+0x19==0` | `RedesignPlan()` |
| Enter (0xd), same conditions | `ModifyPlan()` (Enter in the text input is `DesignEntertNyomott`) |
| Esc (0x1b) `down && mod != 0` | `DesignCancel()` (Esc in mod 0 leaves the screen: main loop) |
`DesignKeyboardEscPressed` (0x509040; mod != 0 only): `SubZoomDirDes==1 && KivalasztottTalalmany==-1 -> -1`; `SubZoomDirDes==1 && Kival != -1 && SubZoom2Des==0 && SubZoomDes==1000 -> SubZoom2Des=1000`; `InitiateDelay=0; RemoveHozzavaloTalalmanyok(); DesigndSelectPodMode=Mode2=0; DesignCancel(); DeleteMenus()`. Always: `IOMode=0` if `IOAzonosito==0x33`.
`DesignCancel` (0x508d30): sound if pressed; `TervUjraSzamol(EN,-1,false,true); DesMelyikGombot=3; DesGombNew=1; SetFirstMenu(); DesignTervezesMod=0; DesignAktPod=0; Design_UjraRendez();` IO off if id 0x33.
`DesignTervezesOk(keepOld)` (0x50b310): `SetFirstMenu(); TervUjraSzamol(EN,-1,false,true);` if mod != 0: `if (mod != 2 || !TervEqual(EN, AktTerv, -1)) { TervClearEmpty(EN,-1); Multi_Send(0x33, EN, 4,&mod, 4,&AktTerv, 0x84,scratch, 1,&keepOld, 0) }; DesignTervezesMod = 0`.

## 7. Orders and `MULTIGET_Design` (0x50c430 / exe 0x4b4d60)
`Multi_Send(type, from, [len, ptr]..., 0)`; `Multi_Get(type, &from, [&val, &len]..., 0)`; loop `Multi_GetNextTipus(0x33, 0x3b)` until <= 0. Only 0x33, 0x34, 0x35 are handled here (0x36..0x3b belong to other modules; check what `Multi_GetNextTipus` does with an unhandled type, ELF 0x5c1100 / exe 0x513340).
`s` = the first field (the sender EN, 4 B). `Tervek[s]` = that empire table. Money: `MoneyHozzaadLog(s, 0, amount_f64, "../sources_base/design.cpp", line)`; `MoneyMennyi(s,0)` f64.

| Type | Sent by | Payload (after `from`) |
|---|---|---|
| 0x33 | `DesignTervezesOk` (OK button) | `mode` i32 (1 new, 2 redesign, 3 modify); `slot` i32 (`AktTerv`); `design` 0x84 B (the scratch); `flag` u8 (`keepOld`) |
| 0x34 | delete popup (id 4) | `slot` i32 (`AktTerv`); `option` i32 (`dSelected+1` = 2); `fleet` i32 (`zero` global = 0) |
| 0x35 | not in this set (starmap/fleet panel "upgrade ships" (guess)) | `slot` i32; `fleet` i32 (0 = all) |

**0x33, mode 3 (modify)**: `f9 = (f32)Tervek[s][slot].+0x24` (old price, read before the copy), `f10 = (f32)design.+0x24` (new price); `bc = 0`.
- `flag == 0` (refit existing ships): `n = HanyHajoVanATervbol(s,0,slot)`; `cost = (n*design.+0x24 - TervHajoinakAra(s,0,slot))*2` (i32), `cost = max(cost,0)`; if `(double)cost <= MoneyMennyi(s,0) || cost < 1`: `MoneyHozzaadLog(s,0,-cost,...,0x969)`; `TervCopy2(s, design, slot)`; if `s==EN && AktKep==5 && DesignTervezesMod==0 && !DeleteMode`: `Design_UjraRendez()`; `TervAzOsszesHajotAtalalitja(s,0,slot)`; `bc = slot`. If `bc > 0`: for every planet `p` in `0..HanyBolygo-1` (`Warfare[p]`, stride 0xa20; unit count at +0x268, unit pointers at +0x458, active flag bytes at +0x270): each unit `u` with active bit, `(u+400 & 0x20000)==0`, `(u+400 & 0x4000000)==0`, `u+0x19c == s`, `u+0x164 == slot`: `u+0x164 = bc` (same value), `WF::CheckDesign(Warfare[p], idx)`. Unaffordable: nothing is written and `bc` stays 0.
- `flag == 1` (keep the old ships, new design): `bc = TervNew(s,0)`; `TervCopy2(s, design, bc)`; old record: `+0x18 = 0` (leaves the list), `+0x19 = 1` (obsolete), `+0x1a = 1`, `+0x1c = bc`; every live design `i` (1..OsszHanyTerv[s], `+0x16 >= 0`) with `+0x1c == slot` gets `+0x1c = bc`; UI: `s==EN && AktKep==5 && mod==0 && !DeleteMode` -> `Design_UjraRendez(); AktTerv = bc; AktTervMutato = TervRendezoKeres(bc)`.
- Both flags, then for each planet `f8` in `1..HanyBolygo` with `Bolygok[f8*0x188 + 0x20] == s` (owner byte), for each production-queue slot `fc` in 0..1 (shorts at `Bolygok + 0x141ee28 + f8*0x188 + fc*8 + 2j`, j<4; ints at `... + 0x141ee50 + fc*4`): `iVar3 = Tervek[s][slot].+0x20` (after the copy): if `short[fc][0] == slot || Tervek[s][short[fc][0]].+0x1c == bc`: `v = (f32)iVar3 - ((f32)(iVar3 - progress)*f9)/f10; v = max(v, 0); v = min(v, (f32)iVar3); progress = (int)v`. If `flag == 1`: for j in 0..3: if `short[fc][j] == slot || Tervek[s][short[fc][j]].+0x1c == bc`: `short[fc][j] = bc`. (If `bc == 0` any queued design with `+0x1c == 0` matches the second test; negative queue entries index `Tervek[s][-1...]`, a read before the array.)

**0x33, modes 1 and 2**: `i = TervNew(s,0); TervCopy2(s, design, i)`; UI: if `s==EN && AktKep==5 && mod==0 && !DeleteMode`: `Design_UjraRendez(); AktTerv=i; AktTervMutato=TervRendezoKeres(i)`. (`mode`/`slot`/`flag` otherwise ignored.) A message whose `Multi_Get` returns false is dropped.

**0x34**: `n = HanyHajoVanATervbol(s, fleet, slot)`; `price = TervHajoinakCsokkentettAra(s, fleet, slot)`. If `option == 2` and `n > 0`: `MoneyHozzaadLog(s, 0, +price, ..., 0x9d8)`; then, unless (`Tervek[s][slot].+0x44 == 5 && StoryAdat+0xac != 0`): `+0x6c = n; +0x70 = 0; +0x74 = Ev; +0x78 = Honap; +0x7c = Nap; +0x1a = 1`; `idx` = first `TradeTervek` index whose `+0x16 <= 0` (loop `while idx < TradeHanyTradeCuccVan && +0x16 > 0`); if `idx < count || count < 40`: `if idx >= count { idx = count; count++ }`; `TradeTervek[idx] = Tervek[s][slot]` (0x84 B); `TradeTervek[idx].+0x24 = (old +0x24 * rndlog(11, 110, 130)) / 100` (i32; stream 11, 110..130; site 0x9f0); `st_CheckTradeTervAr(idx)`; `Story_Special(0x17, s, slot, n, 0,0,0,0)`.
Then always: `fleet == 0`: `TervDelete(s, slot, 0, true)`; UI (`s==EN && AktKep==5 && mod==0 && !DeleteMode`): `if AktTerv == slot { if AktHanyTerv <= AktTervMutato: AktTervMutato--; AktTerv = DesignRendezo[AktTervMutato] }; Design_UjraRendez()`. `fleet != 0`: `TervDelete(s, slot, fleet, true)`.

**0x35**: `n = HanyHajoVanATervbol(s, fleet, slot)`; `newprice = Tervek[s][Tervek[s][slot].+0x1c].+0x24`; `cost = max(0, (n*newprice - TervHajoinakAra(s, fleet, slot))*2)`; if `(double)cost <= MoneyMennyi(s,0) || cost < 1`: `MoneyHozzaadLog(s,0,-cost,...,0xa19); TervAzOsszesHajorolAtalalitja(s, fleet, slot); StarmapAktFlottaValtozott = 1`.

**After the loop** (UI repair of the starmap chosen fleet): if `StarmapMode == 2` and `Flottak[AktFleet]` (0x80 B) lacks `+0x52 bit 10` and (`+0x15 == -1` or ((`+0x19+EN` bit0 == 0 or `+0x18` bit0) and `!DEBUGX` and `!fMindigLathato`)): `AktFleet` = the first fleet owned by EN (`+0x15 == EN`) with `+0x18` bit0 == 0; else the first owned by EN; else the first with `+0x15 != -1` that is visible (`+0x19+EN` bit0 or `fMindigLathato`); else `StarmapMode = 1`.

`Design_UjraRendez` (0x5078d0): list = designs `i` in `1..OsszHanyTerv[EN]` with `+0x16 >= 0 && (+0x18 & 1)`; ordered by `+0x44` ascending with this exact pass: `for i = n down to 2 { j = i-1; while j < n && key(R[j+1]) < key(R[j]) { swap(R[j], R[j+1]); j++ } }`; `R[0]=0`; `AktTervMutato = 0` if the list is empty; `AktTerv = R[AktTervMutato]`. `DesignENValtozott` (0x507af0): `AktTervMutato = 1; if AktKep==5 { AktTerv = R[1]; AktAlap = Alapok[(mod 0 ? design : scratch).+0x44]+8; LoadAlapVektor(AktAlap) }`.
`DesignElnevez(emp, base, out)` (0x5083a0): `stem` = `base` minus trailing digits; `n = atoi(digits)+1`; `used = n > 1`; for each live design of `emp` with `strncmp(stem, name, strlen(stem)) == 0`: `k = atoi(its trailing digits)`; `n = max(n, k+1)`; `used = 1`. Output: if not `used` the `base`, else `stem` (cut to 12 chars) + `itoa(n)`.

## 8. Timers / animations
- `FRISSIT_Design2` (0x50d890, per UI tick, only `AktKep == 5`): `InitiateDelay -> DelayVar += 1.0`; `FaderDesignFrissito(desf)`; `deskeringeto += 0.0006`; `des_xy += 0.02`; `desatlatszosag = (f32)((sin((double)des_xy)+1)/8 + 0.75)`; open anim (`desKELLKINYITAS && !desBECSUKAS && desELINDULT && !desKINYITVA`): `desframe += 0.8` (cap 60); closing (`desKELLKINYITAS && desBECSUKAS`): `desframe -= 1.2` (floor 0); `DesignSzinallitoDesignTablazat[i]` (i<15): if `KELLKINYITASDEFAULT == 0` then 100, else `++` up to 100. (`FRISSIT_Design` is empty.)
- `FaderDesignFrissito` (0x547e10; `desf` = 30 slots; float[30] arrays at byte offsets 0x000 countdown, 0x078 active, 0x0f0 glow, 0x168 out brightness, 0x1e0/0x258 jitter, 0x2d0 speed, 0x348 axis (int), 0x3c0 bytes, 0x3e0 flash, 0x4d0/0x4d4 = -1, 0x4d8 time): `time += 0.003`; per slot: flash `0x3e0 -= 0.01` if > 0 (<=0 -> 0); if `active > 0`: glow `+= 0.1` while < 1, axis 0: `0x258 += rand()%100 - 50`, axis 1: `0x1e0 += ...` (libc `rand`); glow `-= 0.02` if > 0 (<=0 -> 0); countdown `<= 0`: if `active <= 0` countdown = `random(1000) + 1000` else `active -= 1`; else `countdown -= 1` and at exactly 0: `active = random(50) + 50; axis = random(2)`; `0x258 += speed`; out brightness `0x168 = glow * (GLOBALKELLDESIGN & 1) + 0.9`. `InitFaderDesignStruktura` (0x547cf0): `countdown = (f32)random(1000)`, `speed = (random(10)+1)/30`, rest 0, `0x4d0 = 0x4d4 = -1`, `time = 0`. (`random(n)` stream not checked.)
- Opening / closing: `desframe` 0..60; `AddZoomDes` +-40 per frame; `SubZoomDes`/`SubZoom2Des` +-140; button frame +-7 up to 10; wire sphere `+0.25` wrap 10; base idle spin `matrixforgat(.., 0.002, 0)`; tech models open after `DelayVar >= 30` ticks of `InitiateDelay`.
- `Design_Main` `KezdoIdo` repeat: first call, then every call from the 8th held call on.

## 9. Sounds
| Event | Sound |
|---|---|
| A press (`Mouse_Lenyom`) on new, delete, OK, auto, clear, the pod +/- buttons, the list, a pod row (0x54) [obs]; redesign and modify act on the release (`onRelease`) and `RedesignPlan`/`ModifyPlan` click only on a press: silent; the scroll bar (10) has no sound | `PlaySoundGlobal(0)` |
| Design list press (id 9) | also `PlaySound(Des_Hangok[2])` = `screens/design/ki1.wav` |
| Screen opens (not from messages/help/options) | `PlaySound(Des_Hangok[0])` = `screens/design/screen21.wav` |
| Screen closes | `PlaySound(Des_Hangok[3])` = `screens/design/screen23.wav` |
| 3D pod pick (`PodKivalaszto`, exe 0x4b6260) | `PlaySoundGlobal(2)` |
| A hull loads (`LoadAlapVektor` 0x5006c0: every slide-in, the first one and each design change) | `PlaySound(Des_Hangok[1])` = `be1.wav` |

## 10. Exe vs ELF differences seen
- `Design_MouseInit`: the exe also frees a pod light (`resOldalsoFenyekKi`, `PodLightId+8 = 10000`) when set.
- `Design3DInit`: the ELF `desInitNullBoxes` is inlined in the exe (`GFX_AllandoInit1/2` + `InitFader1/2/3` + the two `SetRenderOther(DesignGfx2D_1/_2)` boxes).
- `GFX_Design`: the exe first calls `FUN_00554500` (`GFX_DateMoneyScrollFade`) and `FUN_004ad8e0` (`Design3DUpdate`).
- `Design_Belep`: the exe vectorises the `DesignSzinallitoDesignTablazat[i] = -10*i` fill (`pmulld`); same values.
- Strings: the exe passes the label index as a 4th `GetString` argument (the ELF decompile drops it); `KiirStr` is `FUN_004a9e60`, `operator+` is `FUN_004a92a0`.
- `DesignGfx2D_1`: the `KeresPod` lookup is inlined as a loop over a table at exe 0x8f8918 (stride 9 dwords).
- Both binaries: `Design_Main` scroll uses 3585 (0xe01), `GFX_Design` uses 3510 (0xdb6).

## 11. Check in the asm (ELF address; exe where known)
1. `Design_Main` 0x509140 (exe 0x4b2190, line ~529): `0xe01 / AktHanyTerv` is int64 division (`longlong`): check `idiv` width, that `(0xef - h)/2` is integer, and the 3585 vs 3510 constants (`GFX_Design` 0x507ed0 / exe 0x4b4090).
2. `MULTIGET_Design` 0x50c430 (exe 0x4b4d60): the production-queue progress rescale: `f9`, `f10` f32 (`cvtsi2ss`), the order of `(f32)(iVar3 - progress) * f9 / f10`, truncation `cvttss2si`; the clamp chain (computed twice in the decompile); signedness of the `short` queue entries and the `Tervek[-1]` read.
3. `MULTIGET_Design` mode 3: confirm `f9` is read from `+0x24` before `TervCopy2` and that the loop `iVar3` comes from `+0x20` after it (the decompile re-reads `+0x20` every iteration).
4. `MULTIGET_Design` 0x34: `rndlog(11, 110, 130)` argument order and the `(old*r)/100` integer ops; the comma expression `MoneyHozzaadLog(...) , +0x44 != 5 || StoryAdat+0xac == 0` (money is added whenever `n > 0`).
5. `MULTIGET_Design`: loop behaviour for an unhandled type in 0x36..0x3b; exe 0x4b573c second `Multi_GetNextTipus`; the `Multi_Get` payload order (mode, slot, design, flag) against the `Multi_Send` order.
6. Modify-ok dialog (0x509140, 0x4fb840, 0x4feea0; exe 0x4b2190, 0x4af440, 0x4b1220): string concat order (option text, " - $", number) and `dSelectM` (max strlen) logic; the `(((d.+0x37&1)==0)+1)*15+4` height.
7. `DesignGfx2D_1` 0x4fb840 (exe 0x4af440): speed `(int)(f32 v*100+0.5)` f32 (`mulss`/`addss`, cvttss2si), the `/4.0` branch is double; `/0.03/1000/50` double (exe 0x4af766..0x4afa36, `divsd`); `z = AddZoomDes/ADDZOOMMAXDES` f32 (`divss [0x902f5c]`); `(1-z)*13` etc. truncated by `cvttss2si`; x expressions `(float)int + z*50.0 - 4.0` in f32 (constants 50.0 / 35.0 / 0.6 / 0.8 / 0.2).
8. `Design3DUpdate` 0x503ea0 (exe 0x4ad8e0): f32 vs f64 for `desframe` steps and `ycenter = (250 - desframe*10/60)*479/479`; `fmod((double)(f32 time*k), 1.0)` in the overlays (double, then to float); `1.35 - (desframe*2)/60`.
9. `FRISSIT_Design2` 0x50d890: `desatlatszosag` is an f32 cast of an f64 expression; `desframe` +0.8 / -1.2 f32 with the `60.0` compare.
10. `FaderDesignFrissito` 0x547e10: libc `rand() % 100` vs the game `random()` (exe 0x819978); the `<= 1.0 && != 1.0` patterns are plain `< 1.0`.
11. `DesignKeret1` 0x53f780: `sin(x + const)` + `fmod` chain, integer shift idioms for `/4`, `/2` of `local_14 - 8`; colour truncations `(int)(f32*f32) & 0xff`; same routine as research.md 5.4.
12. `DesignElnevez` 0x5083a0: the 12-char cut (`local_f8[12]` written past a 12-byte buffer) and the `isdigit` scans (char signedness, decrementing index).
13. `DesignPoenosanElnevez` 0x5087c0 (exe 0x4b19d0): the decompile calls `_s(0x3b)` and ignores `a*1000 + b*10 + d`; the real string id is probably that sum (argument hidden): check the asm. Fallback `Talalmanyok[Alapok[b]+0xc].name`.
14. `Design_MouseInit2` 0x50be70 (exe 0x4b34f0): the real positions of ids 0x1f/0x20 (set in `PodKivalaszto`); `FrissitGombTexturak` hover logic (exe 0x4b3f50, `FUN_00510860`).
15. `GFX_Design` 0x507ed0: `(int)(local_30*155.0 + 100.0)` f32 to int; thumb `fVar3 + 1.0 + (float)local_18` f32.
16. `Design_Kilep` 0x507bb0: the `VoltNulla` static and the `SelfPlaying` truth table of `local_e`.
17. `Design_Belep` 0x5074e0 (exe 0x4b1be0): `desmozgato` stores 29.0f / 3.0f; `KELLKINYITASDEFAULT & 1` on a byte.
18. `DesignTervezesOk` 0x50b310: `Multi_Send` payload lengths (4, 4, 0x84, 1) and that the flag is a 1-byte value.

## 12. Helpers not decompiled yet (name, ELF [exe])
UI: `PodKivalaszto` 0x502a60 [0x4b6260; exe read in part: 3D pod pick, sets id 0x55, `PlaySoundGlobal(2)`, sets `DesigndSelectPodMode`], `PodElsotetito` 0x5028d0 [0x4b60e0], `FrissitGombTexturak` 0x500b30 [0x4b3f50], `desSetGombFrame` 0x503bb0, `desMeshKiBe` 0x4fb6e0, `desMozgatMainFenyek` 0x500010, `desOldalsoFenyekBe/Ki` 0x4fff30 [0x4b7790] / 0x5001b0, `desInitNullBoxes` 0x4fb7b0, `desRemoveNullBoxes` 0x4fb750, `LoadAlapVektor` 0x5006c0 [0x4b4a60], `HideAlap` 0x502820, `UnHideAlap` 0x503490, `InitAlapok2` 0x500470, `SzinezPodok` 0x500510, `AlapfolottLenyomva` 0x503d20, `KijelolTalalmany` 0x500dc0, `UnjelolPod` 0x5027e0, `UnjelolMindet` 0x501310, `MegkeresHozzavaloTalalmanyok` 0x501cc0 [0x4a3730], `BetoltHozzavaloTalalmanyok` 0x501650, `RemoveHozzavaloTalalmanyok` 0x501be0 [0x4b6a70], `VannakTalalmanyok` 0x501ad0, `VannakTalalmanyokAPodon` 0x4ffb70 [0x4b7530], `CheckIfTalalmanyfolottvan` 0x501380 [0x4ad0d0], `RemovePodLightok` 0x500370, `TalalmanyRarakhato` 0x4fb540 [0x4b6ca0], `GetMaxRarakhato` 0x4fb4c0 [0x4b42f0], `GetMaxPodok` 0x4fb390 [0x4b4290], `KeresPod` 0x4fb320, `DesignKeret2` 0x541940 [0x4cd4c0 (i)], `FadeKerettelKerekit` 0x538330 [0x4d1860 (i)], `KirakFader` 0x547920, `DrawLaser` 0x53c200, `DrawSparks` 0x53b870, `SetFirstMenu` / `SetSecondMenu` 0x507470 / 0x507490 [0x4b6b30].
Sim side: `TervUjraSzamol` 0x61b5f0 [0x5423b0], `TervCopy` 0x61a840 [0x541cd0], `TervCopy2` 0x61aa80, `TervNew` 0x619b80 [0x5422f0], `TervDelete` 0x619f70 [0x541d50], `TervClear` 0x619e80 [0x541be0], `TervClearEmpty` 0x61b070 [0x541c60], `TervEqual` 0x61ada0 [0x542260], `TervAutoDesign` 0x61b100 [0x541920], `TervHajoinakAra` 0x506880, `TervHajoinakCsokkentettAra` 0x506e00, `HanyHajoVanATervbol` 0x506520, `TervAzOsszesHajotAtalalitja` 0x506e70, `TervAzOsszesHajorolAtalalitja` 0x5071d0, `TervRendezoKeres` 0x50c3d0, `WF::CheckDesign` 0x46dd90 [0x45fd20], `st_CheckTradeTervAr` 0x7372c0, `Story_Special` 0x73e8b0 [0x5c9550], `MoneyHozzaadLog` 0x623240, `MoneyMennyi` 0x622720, `rndlog` 0x61e170 [0x542da0], `Multi_GetNextTipus` 0x5c1100 [0x513340], `Multi_Get` 0x5c11e0, `Multi_Send` 0x5ba960 [0x513840].

## 13. Unknowns
- Where `DesigndSelectPodMode` (3D pod pick popup), the 0x1f/0x20 rectangles and `DesignAktPodListabolkivalasztott` are set: `PodKivalaszto` not fully read.
- Who sends `0x35`; what sets `CanModify` for modes 1 and 2 (not in `Design_Main` for mode 1; `ModifyPlan` sets it for 3; look in `SetSecondMenu` ELF 0x507490, `TervUjraSzamol`, `PodKivalaszto`).
- `DDColor` byte order in `TscVonal` (D3DCOLOR is BGR in memory: byte 0 may be B, then the table 1..8 reads B,G,R).
- `KiirStr` flags 4 vs 6 and the 5th argument (size 2/3): research.md says size 2, flags 4 = left; 6 = right/centre (guess).
- Deviation candidates: stored-design rename without a command (MP desync), 3585 vs 3510 in the scroll bar, the unaffordable-modify `bc = 0` quirk, `Tervek[-1]` queue reads, `DesignElnevez` buffer overrun.

## 14. Verified (viewing mode, 2026-10-22) and the client
Captures: `run_screendiff.py <install> quick-1.sav design` (a still after 600 UI ticks) and `--script
tools/trace/scripts/design-sounds.json` (the opening, a list press, down, up, a tooltip rest, F2 and the closing; 13
grabs). Every region matches at rest (mean 0.05–3.7) but where the exe's faders flash (below); the 8 sounds match the
exe's in order and tick (screen21 at 0, be1 at 76, Click1 + ki1 at the press, be1 at +9 ticks after each design
change, screen23 at F2).
- Hull models (`LoadAlapVektor` 0x5006c0): `AlapNevek[m]` exe 0x9028e8 and `AlapAranyok[m]` exe 0x8f88b0 (25 each),
  `m = Alapok[d.+0x44]+8`; file from `vector/design/alapok/`, the race digit at [4] for m = 5..11, textures from
  `talalmanyok` (m < 5), `spacewar` (5..14), `design/urbazis` (15..18), `design/tankalapok` (19..24); scale
  `AlapAranyok·0.5` (EN 1: ×0.5 for m 5, 6, ×0.6 for 7, 8, ×0.8 for 9, 10); `MoveObj(0, −15, −1400)`,
  `RotateObj(1.0471667, 0.785375, 0)`; then `UnHideAlap` (0x503490) moves it to `z = AddZoomDes − 1400 − SubZoomDes`.
- `Design3DUpdate`'s `AddZoomDes += 40` caps at `ADDZOOMMAXDES` before its `!desBECSUKAS` test: closing, the hull
  slides back to −1079 and fades (`1 − f/60`) [obs: t540/t560 diff 22.9/11.9 → 6.3/3.1 with the cap].
- `AktAlap` is set by `DesignGfx2D_1`, which `Update3D` draws after the zoom logic: a design change slides the old
  hull out from the next frame (be1 at press + 9) [obs].
- Lights reach only their node's subtree (`lighting`): `desOldalsoFenyekBe` (0x4fff30) parents the blue 0x7f5d00
  to child 6 (the buttons) and the orange 0x004e7f to the scene; the hull and the console take ambient 0x141414,
  white 0x808080 (0, 0, −10) and the orange one [obs: the hull's tan]. The wire sphere (child 0, `racsszin`) takes
  only the ambient [obs: (13, 17, 19) on its lines].
- Pod slots: `PodFajok` exe 0x8f9b28 (8 tables × 27 hulls × 12 slots of (class, max)), the race's table
  `HanyasFajPodTablazatatHasznalja` exe 0x902f10; `GetMaxRarakhato` (0x4fb4c0) = the slot's second word; the class's
  name `GetString(0x13, 1, c)` for c < 0x33, else `(0x13, 2, c − 0x32)` (`NEWGAME_Design`, the index in ECX).
  `DesignBrushok[1..4]` exe 0x902988: kep 0x13 (`DESIGN_X.BMP`), 9 × 9 at src x 0/9/18/27.
- `HanyHajoVanATervbol(e, fleet, t)` (0x506520): ships (`Hajok`, design ≠ 0x7fff, the fleet's owner), vehicles by
  count (`Tankok`, owner from the fleet or the planet `−f`) and planet stations (`+0xfe + 2k`, k ≤ `+0x114`) of design
  `t` or of a design whose successor (`+0x1c`) is `t`; `fleet ≠ 0` limits ships and vehicles to that fleet.
- Esc in the viewing mode (`DoKeyboard` → `DesignKeyboardEscPressed`, nothing in mode 0) leaves through the main loop
  to the options screen (`AktKep` 0xd) at once: no closing, no sound [obs: kep 13 at the next grab].
- The first frame is drawn (`desELINDULT`) before the first UI tick: the opening runs 75 ticks (be1 at 76).
- The exe's faders flash at random (`FaderDesignFrissito`'s glow, ~50 frames, and `FadeKerettelKerekitDesign`'s scan
  band): the only regions over the threshold at rest (one panel at a time, 11–18).

Client (`ig2-client::design`, `design_scene`; F5 opens it, F2 closes it animated, Esc at once): the opening and
closing, the list (press, drag, up/down), the scroll bar, the right-button hull drag, the panels' texts, the pod rows
with their greyed −/+ cells, the faders, the tooltips, the 3D buttons' textures, the sounds. Not yet (part 2): the
editing modes 1–3 and their popups, the name input, the orders 0x33/0x34 (`MULTIGET_Design`, sim port), the
3D button press frames, the pods' lights (`SzinezPodok`/`KijelolPod`), the 3D pod pick (`PodKivalaszto`), the tech
models opening out of a pod (`SubZoomDes`), the fader flashes and scan bands, the lasers and sparks of the opening and
closing.
