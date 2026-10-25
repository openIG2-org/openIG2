# The trade screen (`AktKep` 8): the merchants' market (from the ELF and exe decompiles)

Sources: ELF decompiles `_local/decomp/trade-screen-elf/` (`DecompileTree` depth 1 from the screen's functions), exe
decompiles `_local/decomp/trade-screen-exe/`, plus exe `0x605b60 0x607500 0x607c60 0x607e90 0x6058e0 0x6068a0 0x608a60
0x608be0 0x608c30 0x608c70 0x6098a0 0x60a130 0x60b450 0x554500` and ELF `0x7a74c0 0x7a7910 0x7a7c10 0x7a7d40 0x7a7ee0
0x7a80e0 0x7a8aa0 0x7a8e40 0x7a7f60 0x66d160`. The sim side (offers, deliveries, `MULTIGET_Trade`) is
[`../rules/trade.md`](../rules/trade.md). Constants checked in the exe image: `FRISSIT_Trade2`'s steps
are f64 constants added to f32 globals (0.8, 3.6, 0.03, 0.0006, 4/11 = 0.36363636…, 0.6, 0.05 at 0x852278, 0x861dc8,
0x8521e0, 0x852b20, 0x861dc0, 0x852250, 0x848598); the scroll drag's 63.0, 86.0, 4.0 are f32 (0x861dd0/d4, 0x852480);
`TRADEXAMAX` 185.0, `Gombok*` 137/195/352/195, `SzallX` 270, `traSelectX/Y/Chr` 90/28/22, 1.00001 (f64 0x856af0),
1.57075 (f64 0x852b30), 1e-6 and 0.9999999 (f32 0x848518, 0x85ab00).
Addresses are exe unless marked ELF. UI tick = 100 Hz (`Fake_FRISSITES` exe 0x4de720). Strings are `GetString(8, section, idx)` (`txt08.txt`).
Conventions: `KiirStr(text,x,y,maxChars,size,flags,colour)` / `KiirInt(v,x,y,width,size,flags,colour)` (messages.md section 1; flags&3: 1 centre in cells, 2 right, 3 centre on x; 8 = clip to `HatarX1..Y2`; 4 = plain left);
`FadeKerettelKerekit(x,y,w,h, r1,g1,b1,a1, r2,g2,b2,a2, false, 0,0,0,0)` (exe 0x4d1860) = a gradient box; `FaderHanyadikMenet` (exe 0x903514) = fader pass 1/2/3.
`AktIkon = hot-spot id + 0x1e`, `AktIkon2 = id` (mainscr.md).

## 1. Overview and functions

The market of the merchants: the left list shows the offers of class 1 (ships), the right list the class-2 offers (vehicles / tanks); the player picks a row, a quantity, a destination
planet (one of his own planets with building 0x32, the trade port) and presses OK, which sends message 0x51. The chosen design hull is shown as a rotating 3D model with an
info card (Base / Value / Firepower / Speed / Weapons). Own deliveries on the way are listed in a panel at the bottom.

| Function | ELF | exe | Notes |
|---|---|---|---|
| `Trade_Belep` | 0x7ace10 | 0x60a1e0 | `TozsdeScroll` inlined in exe |
| `Trade_Kilep` | 0x7ad040 | 0x60a630 | `RemoveTradeAlap`, `traOldalsoFenyekKi` (4x `resOldalsoFenyekKi` exe 0x452810) inlined |
| `Trade_MouseInit` | 0x7aed30 | 0x60b110 | once, after Belep |
| `Trade_MouseInit2` | 0x7af340 | 0x60aed0 | every rendered frame, from `UpdateFrame` (exe 0x4e3e90, before `GFX_Trade`) |
| `Trade_Main` | 0x7adcd0 | 0x60a7f0 | every rendered frame; `TradeIgazitShipPoz/TankPoz` (ELF 0x7ad8d0/0x7ada00) inlined |
| `GFX_Trade` (per frame, from `UpdateFrame`) | 0x7ad240 | 0x606b50 | `CheckKivalasztottTradePortosBolygoNemElfoglaltak` (ELF 0x7aeca0) inlined at its top |
| `Trade3DInit` | 0x7aa0e0 | 0x608030 | |
| `Trade3DUpdate` (called by `GFX_Trade`) | 0x7abfc0 | 0x608340 | `traMeshKiBe` (ELF 0x7a7c10), `traSetObjTransparencyH` (0x7a7ee0) inlined |
| `TradeGfx2D_1` (empty) | 0x7aa4c0 | 0x559ad0 (folded empty `ret`, ported name `WaitForDrawing`) | |
| `TradeGfx2D_2` (texts + lists) | 0x7aa4d0 | 0x608c70 | not in the symbol table |
| `TradeGfx2D_3` (destination popup, pod popup box) | 0x7abdb0 | 0x6098a0 | not in the symbol table |
| `FRISSIT_Trade2` (UI tick) | 0x7b0180 | 0x605b60 | 7th call after `AktKep < 0xe` in `Fake_FRISSITES` |
| `Trade_KiirPodNagyLista` | 0x7ae710 | 0x60a390 | called by `GFX_Trade` |
| `TradeDisplayVehicle` | 0x7ac690 | 0x608a60 | |
| `TradeDrawSelection` | 0x7a7130 | 0x608be0 (calls `DesignKeret1`, exe 0x4cbac0) | |
| `TradeDisplayScrollBar` | 0x7ac990 | 0x608900 | |
| `TradeCalculateVehicleNum` | 0x7ac850 | 0x608870 | |
| `TradeAr` | 0x7ac600 | 0x608830 | |
| `KeresAktualisTervSorszamat` | 0x7a7230 | 0x607020 | `TradeNextTerv` (ELF 0x7a71c0) inlined |
| `HanyTradePortVanAkarmi` | 0x7acc80 | 0x606f80 | also DRAWS the destination list (5.3) |
| `TradeGetHeightAdder` | 0x7ae670 | 0x608c30 | |
| `TradeSelectShip` / `TradeSelectTank` | 0x7adb30 / 0x7adc00 | 0x60a080 / 0x60a130 | |
| `TradeLoadAlap` / `RemoveTradeAlap` | 0x7a9b10 / 0x7a9a90 | 0x609c60 / 0x607c10 | |
| `ForgatTradeAlap` | 0x7a8aa0 | 0x6068a0 | |
| `PlaceVehicleInfo` | 0x7a8e40 | 0x607500 | info card text, called from `Trade3DUpdate` |
| `traMozgatMainFenyek` | 0x7a7d40 | 0x60b450 | orbiting side lights |
| `traOldalsoFenyekBe` | 0x7a7f60 | 0x60b540 | |
| `EltuntetGombok` | 0x7a80e0 | 0x6058e0 | hides the built-in button meshes of the frame model |
| `TozsdeScroll` / `TozsdeScrollFrissit` / `...AlsoBar` | 0x7a73a0 / 0x7a74c0 / 0x7a7910 | inlined in Belep / 0x607c60 / 0x607e90 | bottom ticker |
| `GFX_DateMoneyScrollFade` (top bar, hud.md section 2) | 0x66d160 | 0x554500 | |
| `TradeSetGombMousePoziciok` | 0x7a8190 | (none) | NEVER CALLED (no `call` in the ELF disassembly `_local/decomp/screens/elf.dis`): dead |
| `MULTIGET_Trade` (sim, trade.md section 4) | 0x7afa70 | 0x6070b0 | UI side effect in 6.3 |

Dispatcher: `UpdateFrame` runs `Trade_MouseInit2()` (when `AktKep == 8`) and later `GFX_Trade()`; `Trade_Main` runs once per rendered frame (not per 100 Hz tick), so everything "per frame" below runs at the display rate.

## 2. State (exe address; ELF name)

| exe | Name | Type / initial | Meaning |
|---|---|---|---|
| 0xF43433 | `traKINYITVA` | u8; Belep: `= !traKELLKINYITAS` | screen open. `Trade3DUpdate` sets it when `traframe >= 50` and clears `IdoPauseMertAtlepesAlattVan` (0xBAE265) |
| 0xF43432 | `traBECSUKAS` | u8, 0 | closing animation running (set by `Trade_Kilep`) |
| 0xF43460 | `traKELLKINYITAS` | u8 | animate: `KELLKINYITASDEFAULT` (0x912A39, image value 1); forced 0 when `OldKep` is 10, 1 or 0xd |
| 0xF43461 | `traELINDULT` | u8 | set to 1 by `Trade3DUpdate` (first frame drawn); the opening waits for it |
| 0xF434B8 | `traframe` | f32 | 0..50: animation frame of the frame model (opening 0->50, closing 50->0); 50 when no animation |
| 0xF434BC | `trakeringeto` | f32 | +0.0006 per tick: phase of the side lights and of the cycling weapon line |
| 0xF434C0 | `trawireframe` | f32 | wire sphere extra frame, +0.25 per DRAWN frame once `traframe == 50`, wraps at 10 |
| 0xF434C4 | `traLastTransparency` | f32 | last transparency ratio of the hull model (`ForgatTradeAlap`); drives the info-card fade |
| 0xF434C8 | `traOldTervmut` | i32 | offer index of the model currently loaded (set by `TradeLoadAlap`) |
| 0xF43380 | `TradeElottiKep` | i32 | `OldKep` at Belep |
| 0xF43464 | `traKilepesHova` | i32 | target screen while closing |
| 0xF43431 | `VoltMarToltes` | u8 | set to 1 when the selection is re-found (FRISSIT); otherwise unused here |
| 0xF43378 / 0xF4337C | `TRADExa` / `TRADExacel` | f32 | x slide of the model/info block: 0 when the player has deliveries, else `TRADEXAMAX`; moves 3.6/tick |
| 0x928AE8 | `TRADEXAMAX` | f32 = 185.0 | |
| 0xF43370 / 0xF43374 | `TRADEnyitottsag` / `...cel` | f32 | 0..1 fade, +-0.03/tick; written in Belep/FRISSIT only, read nowhere else in this screen (dead?) |
| 0x928DBC | `TradeWhichVehicle` | i32, 50000 = none | selected row index within its class list |
| 0xF43448 | `TradeWhichVehicleType` | i32 | 0 ships (class 1), 1 vehicles (class 2) |
| 0xF43420 / 0xF43424 | `TradeVehicleOld` / `...OldType` | i32, -1 | the selection the model was last (re)loaded for |
| 0x928DB4 / 0x928DB8 | `TradeVehicleNum` / tank count | i32 | offers per class, recounted each frame |
| 0xF4343C / 0xF43440 | `TradeVehiclePos` / tank pos | i32 | first visible row of each list |
| 0xF43444 | `TradeAktualisCucc` | i32, Belep 1 | offer index of the current selection (set in `TradeSelect*`) |
| 0xF43450 | `TradeVehicleMennyi` | i32 | chosen quantity; NOT reset at Belep |
| 0xF4344C | `TradeVehicleMennyiOsszesen` | i32 | units left in the offer (`+0x6c`) |
| 0xF43458 | `TradeActVehiclePrice` | i32 | `TradeAr(i,1)` |
| 0xF43454 | `TradeTotalVehiclePrice` | i32 | `qty * ActVehiclePrice`, recomputed in `TradeGfx2D_2` (NOT `TradeAr(i,qty)`, see 10) |
| 0x928DC0 / 0x928DC4 | `TradeSelectedPlanet` / `...Temp` | i32, -1 | chosen / hovered destination (Belep sets both -1) |
| 0xF43428 / 0xF4342C | `AktEtaDays` / `AktEtaDays2` | i32 | delivery time of the month (20..39, sim) / the value frozen when the destination is chosen |
| 0xF43836 | `SelectDestinationMode` | u8 | destination popup open |
| 0xF43384 | `traSelected` | i32 | row under the pointer in that popup |
| 0x928AEC / 0x928D98 / 0x928D9C | `traSelectX` / `traSelectY` / `traSelectChr` | i32 (image 90 / 28 / 22) | popup origin / width in chars |
| 0xF43430 / 0xF43434 / 0xF43438 | `Trade_GetPodListaMode` / `...X` / `...Y` | u8 / i32 / i32 | pod list popup on while the model area is held |
| 0x928DA0 / A4 / A8 / AC | `Gombokx1 / Gomboky1 / Gombokx2 / Gomboky2` | i32 = 137 / 195 / 352 / 195 (ELF 0x119d81c.. same) | positions of the button row (static data) |
| 0x928DC8 | `SzallX` | i32 = 270 | x of the deliveries table |
| 0x928DCC..D4 | `traInitPos1` + 2 | f32 = 0, 0, 200.0 | wire sphere rest position |
| 0x928DDC | `ScrollFont` | i32 = 2 | ticker font size |
| 0xF439C8 / 0xF43838 | `ScrollQueueLength` / `ScrollQueue[1..]` | i32 | ticker queue (1-based, max 99) |
| 0xF43820 / 0xF43824 | `ScrollHova` / `ScrollHossz` | f32 / i32 | ticker x offset / text width px |
| 0xF434D8 | `ScrollSorok` | 0x54-byte lines | line 0 = `GetString(8,4,1)` (NEWGAME_Trade / `TozsdeScrollLoad` exe 0x607fe0, rules/new-game.md) |
| 0xF66E98 / 0xF66E9C | `TradeAlap` xvb / mesh | | hull model; 0xF66EC8 loaded (ELF `DAT_0180dde4`), 0xF66EC9 init flag (`dde5`), 0xF66ECA dir (`dde6`), 0xF66ECB done flag (`dde7`), 0xF66ECC angle (`dde8`, f32), 0xF66ED0 target angle (`ddec`, f32) |
| 0xF66ED4 | second-model flag (`tf_konnyut.xvb`) | u8 | set to 0 in `TradeLoadAlap`, never set to 1: DEAD branch |
| 0xF66E54 | `TraMainMesh` (tozs1bck.xvb) | meshid | 0xF66E80 `TraWireGomb` (submesh 0), 0xF66F10 `TraSubMesh` scratch, 0xF66E64 scratch for the button pairs |
| 0xF66E88 / E48 / E6C / E34 | `Tra_MainLight1..4` | lightid | |
| 0xF66E40 / F66F08 / F66E5C | null boxes for `Gfx2D_1/2/3` | meshid | |
| 0xF4345C, 0xF66E78 | `mainxvbtradeobject` xvb, mesh | | parent of the hull model |
| 0xDE003C | `mainxvb` | | |
| 0xB9AAF0 / 0xF439CC | `TradeTervek` / `TradeHanyTradeCuccVan` | | offers (0x84 each): `+0` name, `+0x16` (short) class 1 ship / 2 vehicle, `+0x24` price, `+0x2c` int shown as Firepower, `+0x36` fighter flag, `+0x3c` f32 speed, `+0x40` (short) race, `+0x44` base (Alapok index), `+0x48` hull tech, `+0x4c` pod count, `+0x50+k` pod tech k (1-based), `+0x5d+k` pod amount, `+0x6c` units left |
| 0xF66DE0 / 0xF439D0 | `TradeHanySzallitas` / `TradeSzallitas` | | deliveries, trade.md section 1 |
| 0xB85EA8 | `Bolygok` (planet 0), stride 0x188 | | name at `+0`, owner byte `+0x20` |
| 0x912AC8 / 0x912A3C | `HanyBolygo` / `EN` | | |
| 0xF43835 / 0xBAE587 | `TradeCheat` / `DEBUGX` | u8 | make every own planet a trade port |
| 0xDB4680 / 0xDB4684 | `MouseSzovegSzam` / `MouseHanyasIkonKep` | | 0x25 / 0 |
| 0xDB4654 / 0xDB4658 | `Mouse_x` / `Mouse_y` | | |
| 0xDB4651 / 4652 / 465C / 465F / 466A | `Mouse_Bal` / `Mouse_Jobb` / `Mouse_Felenged` / `Mouse_Lenyom` / `Mouse_Megfogva` | u8 | left held / right held / released this frame / pressed this frame / grab |
| 0xB9A980.. | `HatarX1, X2, Y1, Y2, Xm, Ym` | i32 | clip rect (0xB9A980 / 984 / 988 / 98C / 990 / 994) |

Offer lookups: `TradeNextTerv(t, from)` (ELF 0x7a71c0) = first index `>= from` with class `== t`, stopping at `N` (returns `N` if none). `KeresAktualisTervSorszamat` (exe 0x607020) =
`i = TradeNextTerv(type+1, 0); repeat Which times: i = TradeNextTerv(type+1, i+1)`; `-1` if `Which == 50000`.
`TradeAr(i, n)` (0x608830) = `rec.+0x36 == 0 ? rec.+0x24 * n : (rec.+0x24 * n) / 5` (i32, signed division).

## 3. Entry / exit

### 3.1 `Trade_Belep` (ELF 0x7ace10 / exe 0x60a1e0), in order
1. `TradeElottiKep = OldKep` (0xBAE19C); `traELINDULT = 0; TradeVehicleOld = TradeVehicleOldType = -1; VoltMarToltes = 0; Trade_GetPodListaMode = 0; TRADEnyitottsag = 0; TRADEnyitottsagcel = 1.0; TradeWhichVehicle = 50000; TradeAktualisCucc = 1`.
2. `TRADExacel = (TradeHanySzallitas[EN] < 1) ? TRADEXAMAX (185.0) : 0; TRADExa = TRADExacel` (starts at the target: no slide on entry).
3. `InitSounds(Trade_Hangok, 2)`: record 0 = `sounds/screens/trade/screen19.wav` (exe 0x928AF0), record 1 = `screens/trade/scrn19ki.wav` (0x928C0C). Hang records are 0x11c bytes; loop field 0 in the image.
4. `AktBrushok = TradeBrushok` (exe 0x928D28), `AktBrushHany = 1`. The table is one record with an empty name and rect-like fields 40, 295, 200, 140, 200, 140, 1, 1; nothing in the trade code references a brush id (purpose unknown).
5. `if (HanyTradePortVanAkarmi() == 0) TozsdeScroll(0, 2)`: queues ticker line 0 twice (the build-trade-ports text, `GetString(8,4,1)`) when the player has no trade port. Note `HanyTradePortVanAkarmi` also calls `KiirStr` for each port name (a side effect at Belep, harmless).
   `TozsdeScroll(l, n)`: `for k in 0..n: if ++ScrollQueueLength == 100 stop; ScrollQueue[ScrollQueueLength] = l`.
6. `traKELLKINYITAS = (OldKep in {10, 1, 0xd}) ? 0 : KELLKINYITASDEFAULT & 1`; `trakeringeto = 0; trawireframe = 0; traBECSUKAS = 0; TraGombNew = 0; TradeMelyikGomb = TradeMelyikGombOld = -1` (exe 0xF434CC / 0xF434D0 / 0xF43462, no other use).
7. `traKINYITVA = !traKELLKINYITAS; IdoPauseMertAtlepesAlattVan = !traKINYITVA` (game time paused while the screen opens).
8. `Trade3DInit()` (3.2). `Mouse_Szoveg_Off_Range(0x14,0x17); Mouse_Szoveg_Off_Range(0x1d,0x20)`.
9. `SelectDestinationMode = 0; TradeSelectedPlanet = TradeSelectedPlanetTemp = -1`.
10. `if (!traKINYITVA) PlaySound(Trade_Hangok[0])` (screen19.wav): only when the opening animation runs.
Not reset: quantity, list scroll positions, `AktEtaDays2` (and the row frame's morph phase, §5.2 row 9).

### 3.2 `Trade3DInit` (ELF 0x7aa0e0 / exe 0x608030)
`VectorDir = "vector/trade/"; TextureDir = "textures/trade/"; FreeMainXVB(); CreateMainXVB(); hull-loaded flag = 0; InitHatterFader()` (exe 0x4d3650).
Then `tozs1bck.xvb` (4.1), the view globals, viewport, the two lights, the two side lights, `VectorDir = "vector/"; TextureDir = "textures/"`, then the 2D layers:
`GFX_AllandoInit1(); AddNullBox(mainxvb, cb1); SetRenderOther(cb1, TradeGfx2D_1, 0); InitFader1(); AddNullBox(mainxvb, cb2); SetRenderOther(cb2, TradeGfx2D_2, 0); InitFader2(); AddNullBox(mainxvb, cb3); SetRenderOther(cb3, TradeGfx2D_3, 0); GFX_AllandoInit2(); mainxvbtradeobject = CreateXVB(); AddToScene(it, mainxvb, 1.0, mesh); InitFader3(); done flag (0xF66ECB) = 1`.
Draw order = creation order (an inference from the other screens, spy.md section 1): callback 1, fader pass 1, callback 2 (`Gfx2D_2` texts), fader pass 2, callback 3 (`Gfx2D_3`), the common HUD (`GFX_AllandoInit2`), the hull model under `mainxvbtradeobject`, fader pass 3.

### 3.3 `Trade_Kilep` (ELF 0x7ad040 / exe 0x60a630), in order
1. `IdoPauseMertAtlepesAlattVan = 1; Trade_GetPodListaMode = 0; Mouse_Szoveg_Off_Range(0x14,0x17); (0x1d,0x20); traKELLKINYITAS = KELLKINYITASDEFAULT & 1; RemoveTradeAlap()` (if loaded: `RemoveFromScene(mesh)` exe 0x452790, `FreeXVB(xvb, true)` exe 0x452180; the dead second model likewise).
2. If `traKELLKINYITAS && AktKep not in {10, 1, 0xd}` (AktKep is already the TARGET screen here): `traKilepesHova = AktKep; AktKep = 8; traBECSUKAS = 1`; ELF `OroklodikVisszaallitas(&TraMainMesh)` (ELF 0x456bb0) is an empty `ret` in the exe (the exe call shows as `WaitForDrawing(&TraMainMesh)`); `PlaySound(Trade_Hangok[1])` (scrn19ki.wav); loop `{ ForceUpdateFrame (exe 0xBAE2AC) = 1; UpdateFrame(); FRISSITES(); ForceUpdateFrame = 0 }` until `traframe == 0` has been seen on two consecutive passes (the static `VoltNulla`: the first pass that sees 0 sets it, the next exits); then `AktKep` = the saved target.
   (`FRISSIT_Trade2` runs its body only when `AktKep == 8`, hence the temporary reset.)
3. If the target is 10, 1 or 0xd (messages, help, options): `KilepesElott (exe 0xBAE296) = 1; GFX_Trade(); SaveScreen(2); KilepesElott = 0` (keeps the screen as the background of that screen; no closing animation).
4. Always after: `traOldalsoFenyekKi` = `RemoveLight` of `Tra_MainLight1..4`; `DeInitSounds(Trade_Hangok, 2); traKilepesHova = 0; IdoPauseMertAtlepesAlattVan = 0; traKINYITVA = 0; RemoveFader(); GFX_AllandoRemove()`.

## 4. The 3D frame

### 4.1 Models and objects (`Trade3DInit`)
| Object | Model | Parent | Scale | Pos | Rot | Notes |
|---|---|---|---|---|---|---|
| `TraMainMesh` | `tozs1bck.xvb` (dirs `vector/trade/`, `textures/trade/`) | `mainxvb` | 1.15 (0x3f933333) | 0, 15, -650 | 0, 0, 0 | `SetRelative(false)`; the frame, with the opening/closing animation `SetFrame(traframe)` |
| `TraWireGomb` | submesh 0 of `TraMainMesh` | | | | | `PropertiesOn(0x40)`; the wire sphere |
| scratch `TraSubMesh` | submeshes 2 and 4 of `TraMainMesh`; button pairs (below) | | | | | |
| hull model | `TradeLoadAlap` (4.4) | `mainxvbtradeobject` | k*0.08 | TRADExa-190, 105, -650 | 1.57075, 0.785375, 0 | hidden until `ForgatTradeAlap` unhides it |
| dead: `tf_konnyut.xvb` | | `mainxvb` | 0.03 | TRADExa-190, 5, -650 | same | never loaded (flag stays 0) |

Globals set in `Trade3DInit`: `fxn = 30.0, fyn = 25.0, fzn = -210.0` (exe 0x919658, 0xDE01CC, 0xDE01D4), `zbm = 0.1` (0x91965C) (meaning not checked: view/projection globals used by `Update3D`).
Viewport: `m = ((IG2_WINDOW_WIDTH * 480) / IG2_WINDOW_HEIGHT - 640) / 2` (ints; exe 0xB77090/94); `HatarX1 = (m * -0x27f) / 0x27f; HatarX2 = ((m + 0x27f) * 0x27f) / 0x27f; HatarY1 = 0; HatarY2 = 0x1df; HatarXm = HatarX2 - HatarX1 + 1; HatarYm = 0x1e0; ModifyViewPort()` (exe 0x4ac4f0).
`traframe = traKELLKINYITAS ? 0 : 50.0`.
Lights: `CreateLightAmbient(0x141414, &mainmeshid, Tra_MainLight1)`; `CreateLightParalell(0x808080, dir 0, 0, -10.0, &mainmeshid, Tra_MainLight2)`; `traOldalsoFenyekBe` (exe 0x60b540): two `CreateLightParalell` with dir 200.0, 0, -650.0: light 3 colour 0x7f5d00 (amber if RGB), light 4 colour 0x004e7f (blue if RGB); their positions are overwritten every frame (4.2 step 6).

Button meshes hidden each frame (`EltuntetGombok`, exe 0x6058e0): for each of the 8 pairs (parent submesh a, child submesh b) of `TraMainMesh`: (1,3) (1,2) (1,1) (1,0) (1,8) (1,9) (1,10) (1,11) (exe data 0x928DE0..0x928E1C): `GetSubMeshid(main, tmp, a); GetSubMeshid(tmp, sub, b); HideObj(sub); SetFrame(sub, traframe)`. So the 3D buttons of the model are hidden; the buttons are the 2D boxes of 5.1.

### 4.2 `Trade3DUpdate` per frame (ELF 0x7abfc0 / exe 0x608340), in order
1. If `!traKINYITVA && traframe >= 50.0`: `traKINYITVA = 1; IdoPauseMertAtlepesAlattVan = 0`.
2. `SetFrame(TraMainMesh, traframe); traELINDULT = 1`.
3. Submesh 2: visible iff `traframe >= 35.0`; submesh 4: visible iff `traframe <= 25.0` (each followed by `SetFrame(sub, traframe)`).
4. Wire sphere: if `!traKELLKINYITAS`: `MoveObj(ball, 0, 0, 200.0)` (traInitPos1 and the next two); `RotateObj(ball, 0,0,0)`. Else with `x = 1 - traframe/50`: `MoveObj(ball, 0, 0 + x*200, 200 - x*1000)`; `RotateObj(ball, (float)sin((double)(x*2.0f)) * 3.0f, (float)(sin((double)x * 2.1) * 2.0), 0)` (exe 0x6084d6..0x608517; the ELF multiplies the first by double 3.0).
   If `traframe == 50.0`: `trawireframe += 0.25; if >= 10 -> 0`. `SetFrame(ball, traframe + trawireframe)`.
   Ball transparency: if `traframe <= 25`: `v = 1.35 - (traframe*2)/50` (double), `v < 0.0001 -> 1e-6; v > 0.9 -> 0.9999999` (f32 constants); else `1e-6`; `SetObjTransparency(ball, v)` (exe 0x4533a0).
5. View globals: `forgaspont = 0,0,0`, `kamerapont = 0, 0, 600.0`, `xcenter = 320.0`, `ycenter = 240.0`, `elforgatas = 0,0,0` (exe 0xDE0130.., 0xDE0148.., 0x919650/54, 0xDE0158..).
6. `traMozgatMainFenyek` (exe 0x60b450): `k = trakeringeto*20`; light 3 position `sin(k)*300, cos(k)*200, -850.0`; light 4 `sin(k+1.57075)*300, cos(k+1.57075)*200, -850.0` (`SetLightPos` exe 0x453050).
7. Submesh 4 again: `hide4 = (traBECSUKAS && traKilepesHova == 9) || (!traKINYITVA && TradeElottiKep == 9)`; visible iff `!hide4` (part of the frame is hidden while the Info screen 9 is the neighbour); `SetFrame(sub4, traframe)`.
8. `GFX_DateMoneyScrollFade()` (top bar, exe 0x554500); `TozsdeScrollFrissitAlsoBar` (ticker bar, 4.5); `EltuntetGombok`; `ForgatTradeAlap` (4.3).
9. Background: `c = (int)((traframe/50) * 128.0); if c > 128: c = 128; Copy3hatterkep(0x34, colour 0xFF000000 + (255-c)*0x10101)` (grey of level 255-c, opaque; exe 0x4cb690; the stored background picture 0x34 is tinted from white to 50% grey while opening; the picture is the saved starmap: see [`overlay-background.md`](overlay-background.md)); `Update3D(0, 0, 0)` (exe 0x52cd10).
10. `PlaceVehicleInfo((int)((TRADExa + 210.0f) - 190.0f), 0x1b5)` (4.6); `TozsdeScrollFrissit` (ticker text, 4.5).
11. If the done flag (0xF66ECB) is 0 and `angle == target` (floats, exact): flag = 1; `i = KeresAktualisTervSorszamat(); TradeLoadAlap(TradeTervek[i].+0x44, (short)TradeTervek[i].+0x40)`. NOTE: when the selection was cleared (`Which == 50000`) `i = -1` and the arguments are read at record index -1 (the 0x84 bytes before `TradeTervek`); harmless because `TradeLoadAlap` returns first when `Which == 50000` (the old model stays, fully transparent).

### 4.3 `ForgatTradeAlap` (ELF 0x7a8aa0 / exe 0x6068a0), only while a hull model is loaded (0xF66EC8)
- If the init flag (0xF66EC9) is set (set by `TradeLoadAlap`): `target = 0; angle = 1.5707964; init = 0; dir = (TradeHanySzallitas[EN] < 1)`.
- `s = dir ? (1 - cos(angle)) * 300 : (1 - cos(angle)) * -200` (float result; the exe computes it in f32, the ELF in double); `s0` = the same with `angle = 1.5707963267948966` (double in both).
- `UnhideObj(model); GetObjMatrix(model, M); matrixforgat(M, out, 0.03, 0.0); SetObjMatrix(model, out)` (spins the hull by 0.03 per DRAWN frame; `matrixforgat` exe 0x44fbc0); `MoveObj(model, (TRADExa + 0) - 190 + s, 145.0, -650.0)`; `SetObjTransparency(model, s / s0)` (1.0 at angle pi/2 = invisible, 0 at angle 0 = opaque); `traLastTransparency = s / s0`.
- (dead second model: Y 112.0, same otherwise).

### 4.4 `TradeLoadAlap(base, race)` (ELF 0x7a9b10 / exe 0x609c60); returns at once if `Which == 50000`
`RemoveTradeAlap(); second-model flag = 0; k = 1.0`; file name and directories by `base`:
| `base` | file name | `TextureDir` / `VectorDir` | `k` |
|---|---|---|---|
| < 5 | `AlapNevek[base]` (exe 0x9028E8: 1 vad1, 2 vad2, 3 vad4, 4 vad3, all `.xvb`) | `textures/talalmanyok/` / `vector/talalmanyok/` | 1 |
| 5..11 | `tipusnevek[(base-5)/2 + race*12]` (entries of 0x30 bytes, 0x240 per race; exe 0x91F488) | `textures/spacewar/` / `vector/spacewar/` | 1 |
| 12..14 | `AlapNevek[base]` (zault, kolonizalo, terra2) | `textures/spacewar/` / `vector/design/alapok/` | 2.0 |
| 15..18 | `TTeruletek[Alapok[base].+0xc - 1]` (exe pointer table 0x919294 + 4*v, `Alapok` 0x90BC04 stride 0x24) | `textures/talalmanyok/` / `vector/talalmanyok/` | 1 |
| 19..24 | same | same | 0.5 for base 19, else 1 |
| 25 (0x19) | `tipusnevek[race*0x240]` | `textures/spacewar/` / `vector/spacewar/` | 1 |
`TradeAlap = LoadBox(name, false)` (the exe's `LoadBox` 0x452520 takes two arguments as well; the supposed third doesn't exist, `k` only scales `AddToScene`); `AddToScene(TradeAlap, mainxvbtradeobject, k * 0.08, mesh); SetRelative(false); MoveObj(mesh, (TRADExa + 0) - 190, 105.0, -650.0); RotateObj(mesh, 1.57075, 0.785375, 0); HideObj(mesh)`; loaded = 1; init flag = 1; `traOldTervmut = KeresAktualisTervSorszamat()`; dirs back to `vector/`, `textures/`; `Trade_GetPodListaMode = 0`; `reloadtextures()` (ELF 0x5ff750; a folded no-op in the exe).
`RemoveTradeAlap`: if loaded: `RemoveFromScene(mesh); FreeXVB(xvb, true); loaded = 0`.

### 4.5 The ticker (bottom line)
- `FRISSIT_Trade2`: `if (ScrollQueueLength != 0) ScrollHova += 0.8` per tick.
- `TozsdeScrollFrissitAlsoBar` (exe 0x607e90): `f = traframe/50; ScrollHossz = strlen(text) * CharXMeret[ScrollFont]; if ScrollHova < 200: f *= ScrollHova/200; if ScrollHova > ScrollHossz + 440: f *= -((ScrollHova - 640) - ScrollHossz)/200; f = max(f, 0); FadeKerettelKerekit(30, 457, 575, 15, (27, 76, 122, (int)(250 - f*38)), (54, 152, 244, (int)(255 - f*98)))` (0x1b8 = 440).
- `TozsdeScrollFrissit` (exe 0x607c60): clip `HatarX1..X2 = 35..600, Y 0..479`; if the queue is not empty: if `ScrollHova > ScrollHossz + 640`: `ScrollHova = 0; ScrollQueueLength--; shift the queue down by one`; text = `ScrollSorok[ScrollQueue[1]]`; `ScrollHossz` as above; `KiirStrSzinSzorzo = (int)(traframe + 50)`; if `ScrollHova < 200`: `*= ScrollHova/200`; if `> ScrollHossz + 440`: `*= -((ScrollHova - 640) - ScrollHossz)/200`; `KiirStr(text, (int)(640 - ScrollHova - extra), 0x1cc, strlen, 2, 0xc, 0x22)` with `extra = traKINYITVA ? 700 - traframe/50*700 : 0`; restore the clip, `KiirStrSzinSzorzo = 100`.

### 4.6 `PlaceVehicleInfo(x, y)` (ELF 0x7a8e40 / exe 0x607500), x = `TRADExa + 20` (int), y = 437; only if a hull model is loaded and `Which != 50000`
`t = (1 - traLastTransparency) * 100; off = 0; if (!traKINYITVA) { t = t * traframe/50; off = (1 - traframe/50) * 100 }; KiirStrSzinSzorzo = (int)t`; `rec = TradeTervek[traOldTervmut]`. Labels = `GetString(8,3,k)` plus ":" (":" at exe 0x852AEC). Positions are `(int)((float)(x+dx) + off)`.
| What | Position | maxChars | colour |
|---|---|---|---|
| label (8,3,1) Base | x, y | 6 | 1 |
| `Talalmanyok[rec.+0x48].name` (exe 0xB9C0C8 + t*0x114) | x+0x28, y | 16 | 2 |
| label (8,3,2) Value | x+0x96, y | 14 | 1 |
| `KiirInt(value, x+0xbe, y, 10, 2, 4, 2)`; value = GetGyartasAr(race, hull tech) + sum over pods k=1..pods(+0x4c) with tech != 0 of GetGyartasAr(race, tech) * amount(+0x5d+k); then divided by 5 if `rec.+0x36` | | | 2 |
| label (8,3,3) Firepower | x, y+15 | 12 | 1 |
| `KiirInt(rec.+0x2c, x+0x41, y+15, 16, 2, 4, 2)` | | | 2 |
| label (8,3,4) Speed | x+0x96, y+15 | 19 | 1 |
| `KiirInt((int)(rec.+0x3c as f32 * 100.0 + 0.5), x+0xbe, y+15, 4, 2, 6, 2)` | | | 2 |
| label (8,3,6) Weapons | x - off, y-0x89 | 13 | 1 |
| no pod used: (8,3,7) None at (x+0x3c-off, y-0x88), 13, colour 0x25 | | | 0x25 |
| else: the list of non-empty pods; line n = GetString(8,3,5) followed by the format " d - " with d = n (1-based count of used pods) and then `Talalmanyok[pod tech].name`; the line shown is `(int)(trakeringeto*10) mod count`, at (x+0x3c-off, y-0x88), maxChars = strlen | | | 0x25 |
`GetMaxPodok(race, base)` exe 0x4b4290, `GetGyartasAr` exe 0x4acaf0. Exe decompile: the value is `GetGyartasAr((short)+0x40 = an empire index, looked up via PlayerFaj)`; the weapons lines loop `GetMaxPodok`'s slots; the tech names are the localised txt17 ones. In the ELF the pod lines use strcat of the tech name; the exe copies the same bytes.

### 4.7 Rendering rules seen in the exe [obs]
- **Material transparency**: the `.xvm` material's last word is an f32 transparency (`re/formats/xv.md` → Material
  transparency): `Racsszin` (the four panels) and `tozs_hk` (the backdrop) hold 0.65 (0x3f266666), the sphere's
  `Gombracs` 0.25. The exe draws the panel object at opacity 89/255: the panels are a flat racsszin × 0.35 and keep
  their depth write, so neither the backdrop nor the wire sphere shows through them.
- **The wire sphere** is drawn by `wirekirak` (ELF 0x6083f0): every face's edges, no culling (the far side
  ambient-lit), aliased one-pixel lines in the vertex's lit colour, alpha = opacity (property 8). At rest its far
  faces read as the dark grid over the lit backdrop.
- **Hull normals**: the exe's hull normals are the file normals exactly (`vad1.xv`: 305 of 305, normalised and
  deduplicated by `optimizeobject` ELF 0x5ff9d0, first-occurrence order). `LoadXVB` (ELF, the child loop) copies the
  child record's matrix to child `+100`, then resets `+100` with `CreateMatrix(0, 0, 0)` (identity) and sets
  `+0x88 = record·+100`; `Kocka` builds the lighting chain `sp+0x24` from the `+100` matrices (and a keyed child's
  rotation key) only. So a child record's matrix places and scales the points but never turns or scales the normals
  (`vad1`'s only child carries diag(0.85, 1.90, 0.18)). With the screen's lights (§3.2), 200 of `vad1`'s 305 vertices
  get the ambient 0x141414 alone (near-black faces turned from the light); the wings are amber-lit.

## 5. The 2D passes (draw list)

Positions in the 640x480 virtual screen (centre / letterbox: widescreen.md). `FaderHanyadikMenet` is 1 unless said.
`Trade3DInit` loads no `dt.bmp`: every `FadeKerettelKerekit` here is the bare colour at `(255 − t)/255` over what lies below
(the scroll-bar thumb (34, 42, 40) = ½·(0x2c, 0x36, 0x34) + ½·track; the deliveries panel (17, 24, 27) = ½·fill + ½·backdrop) [obs].
`AktIkon` is set only while a button is down or on the press (the list and scroll-bar handlers test no button); `Mouse_Megfogva` holds the grabbed hot-spot.

### 5.1 `GFX_Trade` (per frame; ELF 0x7ad240 / exe 0x606b50), boxes
0. Inlined first: `if (TradeSelectedPlanet != -1 && (Bolygok[p].owner != EN || (SearchBuilding(p, 0x32, true) < 1 && !DEBUGX && !TradeCheat))) TradeSelectedPlanet = -1` (`SearchBuilding` exe 0x541150; building type 0x32 = trade port).
If `traKINYITVA && !traBECSUKAS`:
1. If `SelectDestinationMode`: `n` = number of own planets with a port (min 1); `FaderHanyadikMenet = 2; FadeKerettelKerekit(traSelectX-5, traSelectY-2, 0x8e, n*13+4, (0,10,0,0x3c), (0,5,0,0x1e)); FaderHanyadikMenet = 1`.
2. `TradeCalculateVehicleNum()`; `TradeDisplayScrollBar(0x12f, 0x3f, shipCount, shipPos, 8, 0x56)` and `(0x143, 0x3f, tankCount, tankPos, 8, 0x56)`.
3. If `Which != 50000`: boxes `FadeKerettelKerekit(Gombokx1+0xf, Gomboky1+0x14, 0x28, 0xf, (0xb,0xd,0xd,0x7f), (0xb,0xd,0xd,0x61))` (minus button, x 152, y 215) and the same at `Gombokx1+0x41` (plus, x 202); if `TradeSelectedPlanet != -1 && qty != 0 && (double)TradeAr(KeresAktualisTervSorszamat(), qty) <= MoneyMennyi(EN, 0)` also at `Gombokx1+0x73` (OK, x 252).
4. Always (inside the open test): `(Gombokx2-5, Gomboky1+0x14, 0x37, 0xf, same colours)` (destination button, x 347, w 55).
Then (always, even while closing): `f = traframe/50; if TRADExa == 0: FadeKerettelKerekit(0x106, (int)(((1-f)*139)*0.5 + 295), 0x166, (int)(f*169), (0x21,0x29,0x28,(int)(255-f*128)), (0x21,0x29,0x28,(int)(255-f*228)))` (the deliveries panel growing out of its middle: x 262..620, y 295..464).
Then `Trade3DUpdate()`, `Trade_KiirPodNagyLista(Trade_GetPodListaX, Trade_GetPodListaY)`.
`TradeDisplayScrollBar(x, y, total, pos, vis, h)` (ELF 0x7ac990 / exe 0x608900): if `total == 0`: thumb = `h`, off = 0; else (exe, double arithmetic) `thumb = (float)(((double)vis * 1.00001 / (double)total) * (double)h)`, clamped to `h`; `off = (float)(((double)pos / ((double)total * 1.00001 - (double)vis)) * (double)(h - thumb))`, `off = max(off, 0)`. Track `FadeKerettelKerekit(x, y, 0xd, h, (0x21,0x29,0x28,0x44), (0x21,0x29,0x28,0x80))`; thumb `FadeKerettelKerekit(x, (int)(y + off), 0xd, (int)thumb, (0x2c,0x36,0x34,0x80), (0x2c,0x36,0x34,0x9e))`.
`TradeCalculateVehicleNum` (exe 0x608870): counts the offers with class 1 and class 2; then for each list `if count < pos + 8: pos -= 8` (a subtraction, not a clamp), then `pos = max(pos, 0)`.

### 5.2 `TradeGfx2D_2` (exe 0x608c70, ELF 0x7aa4d0): first `WaitForDrawing()` (no-op)
Block A only if `traKINYITVA && !traBECSUKAS`; block B (deliveries rows) if `TRADExa == 0`; block C (card boxes). Label text = `GetString(8,2,k)` plus ":". All `KiirStr` here use size 2.
Block A (flags 4 unless stated):
| # | Text | Pos | maxChars | colour |
|---|---|---|---|---|
| 0 | `TradeCalculateVehicleNum()` | | | |
| 1 | (8,2,3) Ships: | 0x69, 0x31 | 8 | 1 |
| 2 | (8,2,4) Price: | 0xe0, 0x31 | 6 | 1 |
| 3 | (8,2,5) Qty: | 0x10d, 0x31 | 5 | 1 |
| 4 | `TradeDisplayVehicle(0x69, 0x3f, 0, shipPos, 8)` | rows y = 63 + 11k | | |
| 5 | (8,2,6) Vehicles: | 0x15b, 0x31 | 12 | 1 |
| 6 | (8,2,4) Price: | 0x1d2, 0x31 | 6 | 1 |
| 7 | (8,2,5) Qty: | 0x1ff, 0x31 | 5 | 1 |
| 8 | `TradeDisplayVehicle(0x15b, 0x3f, 1, tankPos, 8)` | | | |
| 9 | if `Which != 50000`: `TradeDrawSelection(Type == 0 ? 0x69 : 0x15b, 0x3f, Type, Which, 8)`: if `pos[Type] <= Which < pos[Type]+8`: `DesignKeret1(x-4, 63 + (Which-pos)*11 - 1, 0xc4, 10, 1)` (animated frame, exe 0x4cbac0; its morph state `dkm` 0xB34C0C, `dkmcel` 0x903548, `GfxNovelo` 0xB34BAC is global and never reset between screens: only the places reset on a screen change, `Frissit2_Grafika`; 0xAD0EF5 = `MainScr_Main`'s jump-without-morph flag) | | | |
| 10 | `TradeTotalVehiclePrice = TradeActVehiclePrice * TradeVehicleMennyi` (computed here) | | | |
`TradeDisplayVehicle(x, y, t, first, 8)` (exe 0x608a60): rows `k = 0..7`, a row is shown if `first + k < count[t]`: name `KiirStr(rec.name, x, y+11k, 16, 2, 4, 0x25)`; price `KiirInt(TradeAr(i,1), x+0x78, y+11k, 7, 2, 4, 0x25)`; units `KiirInt(rec.+0x6c, x+0xa5, y+11k, 3, 2, 4, 0x25)`; the rows are consecutive offers of class `t+1` (`TradeNextTerv`), starting at the `first`-th.
If `Which != 50000` (block A continued):
| # | Text | Pos | maxChars | colour |
|---|---|---|---|---|
| 11 | (8,2,7) Buy: | 137, 195 (Gombokx1, Gomboky1) | 7 | 1 |
| 12 | `KiirInt(qty, 137+0x28, 195, 3, 2, 6, 0x25)` | | | 0x25 |
| 13 | "/" (exe 0x852DD8) | 137+0x41, 195 | 1 | 1 |
| 14 | `KiirInt(units left, 137+0x50, 195, 3, 2, 4, 0x25)` | | | 0x25 |
| 15 | (8,2,4) Price: | 352, 195 (Gombokx2, Gomboky2) | 6 | 1 |
| 16 | `KiirInt(TradeTotalVehiclePrice, 352+0x28, 195, 8, 2, 4, 0x25)` | | | 0x25 |
| 17 | "-": shadow `KiirStr("-", 137+0x15, 195+0x19, 5, 2, 5, 0)` then `KiirStr("-", 137+0x14, 195+0x18, 5, 2, 5, 0x25)` | | 5 | 0 / 0x25 |
| 18 | "+": shadow `(137+0x47, 195+0x19, 5, 2, 5, colour 0)` then `(137+0x46, 195+0x18, 5, 2, 5, 0x25)` | | 5 | 0 / 0x25 |
| 19 | only if `planet != -1 && qty != 0 && (double)TradeAr(...) <= MoneyMennyi(EN,0)`: (8,2,8) OK (no colon) at 137+0x78, 195+0x18, size 2, flags 5 (centred in 5), colour 0x25 | | 5 | 0x25 |
| 20 | only if `planet != -1`: (8,2,9) ETA: at 352+0x5c, 195 (4, colour 1); `KiirInt(AktEtaDays2, 352+0x82, 195, 3, 2, 4, 0x25)` | | | |
Then always in block A:
| # | Text | Pos | maxChars | colour |
|---|---|---|---|---|
| 21 | (8,2,10) DEST.: flags 5 | 352, 195+0x18 = 219 | 8 | 0x25 |
| 22 | `planet == -1`: "---" (exe 0x852B00) flags 5 at 352+0x3c, 219, maxChars 3, colour 0x25; else `KiirStr(Bolygok[planet].name, 352+0x3c, 219, 0x14, 2, 4, 0x25)` | | | 0x25 |
| 23 | only if `TRADExa == 0`: column headers at y = 300 (flags 4, colour 1): (8,2,11) Destination: x 270 (SzallX), 12; (8,2,9) ETA: x 370, 4; (8,2,12) Item: x 420, 10; (8,2,13) Amount: x 570, 8 | | | 1 |
Block B (if `TRADExa == 0`, outside the open test): save `Hatar*`; `HatarX1 = 0; HatarX2 = 0x27f; HatarY1 = 0x127; n = min(TradeHanySzallitas[EN] + 1, 11); HatarY2 = (int)(((((float)(n*15+8) * (traframe/50) + 320.0) - 7.0) - 25.0) * 479.0 / 479.0)` (all f32); `HatarXm = 0x280; HatarYm = HatarY2 - 0x126`; for `i = 0 .. min(count, 10)-1` (the exe stops when `y > 0x1d3`), `y = i*15 + 0x13e`, delivery `d = TradeSzallitas[EN][i]` (0x98 each): `KiirStr(Bolygok[d.+0].name, 270, y, 16, 2, 0xc, 0x25)`; `KiirInt(d.+4 days, 370, y, 4, 2, 0xc, 0x25)`; `KiirStr(d.+0x14 design name, 420, y, 16, 2, 0xc, 0x25)`; `KiirInt(d.+0xc units, 570, y, 7, 2, 0xc, 0x25)`; then restore `Hatar*` and `HatarXm/Ym = (X2-X1)+1 / (Y2-Y1)+1`.
Block C (info-card boxes): if the hull model is loaded (0xF66EC8) and `Which != 50000`: `FaderHanyadikMenet = 3; a = 1 - traLastTransparency; off = 0; if (!traKINYITVA) { off = (1 - traframe/50)*100; a *= traframe/50 }; X = ((TRADExa + 210) - 190) - 5`; `FadeKerettelKerekit((int)(X + off), 0x1b0, 0xf0, 0x20, (0x21,0x29,0x28,(int)(255 - a*128)), (0x21,0x29,0x28,(int)(255 - a*228)))` (under the Base/Value/Firepower/Speed lines) and `FadeKerettelKerekit((int)(X - off), 0x127, 0xf0, 0x13, same colours)` (under the Weapons line); `FaderHanyadikMenet = 1`.

### 5.3 `TradeGfx2D_3` (exe 0x6098a0, ELF 0x7abdb0)
- If `SelectDestinationMode`: `WaitForDrawing()`; `n = HanyTradePortVanAkarmi()` (this call draws the list, below); if `n == 0`: (8,2,14) No trade ports! at (traSelectX, traSelectY+3), maxChars 0x16, size 2, flags 4, colour 0x25.
  `HanyTradePortVanAkarmi` (exe 0x606f80): for `p = 1..HanyBolygo` with `owner == EN && (SearchBuilding(p, 0x32, true) > 0 || DEBUGX || TradeCheat)`: `KiirStr(Bolygok[p].name, traSelectX, traSelectY + 3 + n*13, 0x16, 2, 4, n == traSelected ? 0x25 : 1)`; `n++`; returns `n`.
- If `Trade_GetPodListaMode`: `FaderHanyadikMenet = 3; h = TradeGetHeightAdder()` (exe 0x608c30: pods == 0 ? 7 : pods*14/2, with pods = GetMaxPodok(rec[traOldTervmut].+0x40, .+0x44)); `FadeKerettelKerekit(PodX-5, (PodY - h) - 5, 0x10e, h*2 + 5, (0x21,0x29,0x28,0x7f), (0x21,0x29,0x28,0x1b)); FaderHanyadikMenet = 1` (the not-minus-one test in the ELF is always true).

### 5.4 `Trade_KiirPodNagyLista(x, y)` (exe 0x60a390, ELF 0x7ae710), only while `Trade_GetPodListaMode`
`rec = TradeTervek[traOldTervmut]; n = GetMaxPodok(race, base)`. If `n == 0`: GetString(8,2,1) followed by an exclamation mark (exe 0x85DA0C) at (x, y-7), maxChars 0x1e, size 2, flags 4, colour 1. Else `top = y - n*14/2`; for `k = 1..n` a line at `(x, top + (k-1)*14)`, maxChars = strlen, 2, 4, colour 1:
- pod empty (`rec.+0x50+k == 0`): text = k, " - ", slotname, " - ", GetString(8,2,2), exclamation mark (format `"%d - %s - "` + GetString(8,2,2) + `"!"`; k in decimal; slotname from `GetPodName(rec, k)`, exe 0x4b4320 = the `PodTipusok` name);
- else: text = k, " - ", slotname, " - ", techname, " (", amount, " / ", max, ")" (`"%d - %s - %s (%d / %d)"`): techname = `Talalmanyok[t].name` with `t = rec.+0x50+k`, amount = `rec.+0x5d+k`, max = `min(GetMaxRarakhato(race, base, k), Talalmanyok[t].+0x24)` (`GetMaxRarakhato` exe 0x4b42f0; `Talalmanyok` exe 0xB9C0C8, stride 0x114, `+0x24` is a byte).

## 6. Input

### 6.1 Hot-spots (`Trade_MouseInit` exe 0x60b110 / ELF 0x7aed30; `MouseSzovegSzam = 0x25`, `MouseHanyasIkonKep = 0`): `Mouse_Szoveg_Set(id, 0, x, y, w, h, GetString(8,1,tip), onRelease, 1)`
| id | `AktIkon` | rect x,y,w,h | tip (8,1,n) | onRelease | `Trade_Main` action |
|---|---|---|---|---|---|
| 0x19 | 0x37 | Gombokx1+0xf = 152, Gomboky1+0x14 = 215, 0x28, 0xf | 1 Reduce quantity | 0 | qty - 1 |
| 0x1a | 0x38 | Gombokx1+0x41 = 202, 215, 0x28, 0xf | 2 Increase quantity | 0 | qty + 1 |
| 0x1b | 0x39 | Gombokx2-5 = 347, 215, 0x37, 0xf | 3 Destination planet | 0 | destination popup |
| 0x1c | 0x3a | Gombokx1+0x73 = 252, 215, 0x28, 0xf | 4 Ok | 1 | send the purchase |
| 0x21 | 0x3f | 300, 0x3f, 0x12, 0x56 | 5 Scrollbar | 0 | ship list scroll |
| 0x22 | 0x40 | 0x143 = 323, 0x3f, 0x12, 0x56 | 5 Scrollbar | 0 | vehicle list scroll |
| 0x23 | 0x41 | 0x69 = 105, 0x3f, 0xbb, 0x58 | 6 Ships | 0 | ship list click |
| 0x24 | 0x42 | 0x15b = 347, 0x3f, 0xbb, 0x58 | 7 Tanks | 0 | vehicle list click |
| 0x25 | 0x43 (read as `AktIkon2`) | 200, 300, 0xf0, 0xaa | 8 List of pods | 0 | pod list popup (while held) |

`Trade_MouseInit2` (exe 0x60aed0, every frame): re-sets 0x25 at x = `(int)((float)(int)off + TRADExa)` where `off = dir ? (1 - cos(angle)) * 300 : (1 - cos(angle)) * -200` (f32; `angle` 0xF66ECC, `dir` 0xF66ECA), y 300, 0xf0 x 0xaa (follows the sliding model); then `Off(0xc), Off(0x18), On(0x21..0x24), Off(1,2,3,4,8,0xb,0xe,0x10,0x13,0x25), On(0x1b)`; if `Which == 50000`: `Off(0x19), Off(0x1a), Off(0x1c)`; else `On(0x19), On(0x1a), On(0x25)`, and 0x1c on iff `planet != -1 && qty != 0 && (double)TradeAr(offer, qty) <= MoneyMennyi(EN, 0)` (else off); then `Mouse_Szoveg_Off_Range(1, 0x18)` and `(0x1d, 0x20)` (so the common ids 2..5 for pause / speed are off here); and if `!traKINYITVA || traBECSUKAS`: all ids `1..MouseSzovegSzam` off.
No keyboard handler for the trade screen (screens README row: none).

### 6.2 `Trade_Main` (exe 0x60a7f0 / ELF 0x7adcd0), every rendered frame, in order
0. `if Mouse_Felenged: Trade_GetPodListaMode = 0`.
1. `AktIkon == 0x37` (and not `Felenged`): on `Lenyom`: `PlaySoundGlobal(0)`; if `qty != 0 && ((Lenyom && Bal) || Jobb)`: `qty -= 1`.
2. `AktIkon == 0x38` (not `Felenged`): `Lenyom` -> `PlaySoundGlobal(0)`; if `qty < MennyiOsszesen && ((Lenyom && Bal) || Jobb)`: `qty += 1`. (The right button repeats EVERY rendered frame while held; the left button steps once per press.)
3. `AktIkon == 0x39 && (Lenyom || SelectDestinationMode)` (destination): `Lenyom` -> `PlaySoundGlobal(0)`. If not `Felenged`:
   - first frame (`SelectDestinationMode == 0`): `traSelectChr = 0x16`; `n` = ports (min 1); `traSelectX = Mouse_x + 12; traSelectY = Mouse_y - (n*13)/2` (signed division); clamps in this order: `Mouse_x + 6 < 0 -> X = 6` (never true); `Y - 2 < 0 -> Y = 2`; `X + 22*6 + 4 > 640 -> X = 0x27c - 22*6 (= 504)`; `Y + n*13 + 2 > 480 -> Y = 0x1de - n*13`.
   - every frame: `traSelected = -1`; walk the ports in order (index j): if `traSelectX <= Mouse_x <= traSelectX + 0x84` and `traSelectY + j*13 <= Mouse_y < traSelectY + (j+1)*13`: `traSelected = j; TradeSelectedPlanetTemp = planet`. (Temp keeps its last value when the pointer is on no row.) Then `SelectDestinationMode = 1; Mouse_Megfogva = 1`.
   - on `Felenged`: `SelectDestinationMode = 0; TradeSelectedPlanet = TradeSelectedPlanetTemp; AktEtaDays2 = AktEtaDays` (so a release outside the list re-picks the last hovered planet, or -1 if none was hovered since Belep).
4. `AktIkon == 0x3a` (OK; an `onRelease` box reaches `_Main` once, on the press, then nothing until the button is up, `Mouse_FelengedesMajdMegvar`): `if Lenyom: PlaySoundGlobal(2)` (Click3 and the message both come on the press [obs]). Build `TradeSzallitasTemp` (exe 0xF43388): `+0 TradeSelectedPlanet, +4 AktEtaDays2, +8 i = KeresAktualisTervSorszamat(), +0xc qty, +0x10 TradeAr(i, qty), +0x14.. a copy of the 0x84-byte record TradeTervek[i]`; `Multi_Send(0x51, EN, 0x98, &TradeSzallitasTemp, 0)` (exe 0x513840); if `copy.+0x48` (hull tech) `== 5`: `TutorialCheckEvent(0x27)` (exe 0x60bdf0). No further checks in `Trade_Main` (the button is only enabled by `Trade_MouseInit2`); the handler (trade.md section 4) validates.
5. `AktIkon == 0x3f` (ship scrollbar, `Mouse_Megfogva = 1`): `pos = (int)(((float)Mouse_y - 63.0) / 86.0 * (float)shipCount - 4.0)` (f32; asm 0x60aba9..0x60abc8); `pos < 0 -> 0`; `shipCount < pos + 8 -> pos = shipCount - 8` (can be negative; clamped to >= 0 by the next `TradeCalculateVehicleNum` / FRISSIT). `AktIkon == 0x40`: the same with tank count and pos.
6. `AktIkon == 0x41 && shipCount > 0` (ship list): `Lenyom` -> `PlaySoundGlobal(0)`; `Type = 0; sel = shipPos + (Mouse_y - 0x3f)/0xb` (signed division: for `Mouse_y` 53..62 the quotient is 0, not -1); `TradeIgazitShipPoz(&sel)`: if `sel < pos` or `sel >= pos+8`: `sel < pos -> pos -= 1, sel = pos`; `sel > pos+7 -> sel = pos+8, pos += 1`; `sel >= pos+8 -> sel = pos+7`; `pos > count-8 -> pos = count-8`; `sel < 0 -> 0`; `pos < 0 -> 0` (auto-scroll one row per frame while dragging outside; `Mouse_Megfogva` keeps the hot-spot); `Which = sel; TradeSelectShip(); Mouse_Megfogva = 1`.
   `TradeSelectShip` (exe 0x60a080): `Which >= count -> count - 1; count == 0 -> Which = 50000`; if `Which == 50000`: `MennyiOsszesen = 0`, else `i = KeresAktualisTervSorszamat(); MennyiOsszesen = rec.+0x6c; ActVehiclePrice = TradeAr(i, 1); TradeAktualisCucc = i`; then `qty = 0` (every frame the mouse is held on a row, the quantity is reset to 0).
   `AktIkon == 0x42 && tankCount > 0`: the same for tanks (`Type = 1`, `TradeIgazitTankPoz`, `TradeSelectTank` exe 0x60a130).
7. Model reload trigger: `(AktIkon == 0x41 || 0x42) && (Bal || Jobb) && !Felenged && (TradeVehicleOld != -1 || Which != 50000) && (Which != TradeVehicleOld || TradeVehicleOldType != Type)`: if `TradeVehicleOld == -1` (first selection): `if Which != 50000: TradeLoadAlap(rec.+0x44, (short)rec.+0x40)` at once; else (a model is shown): done flag (0xF66ECB) = 0, `target angle = 1.5707964 (pi/2)`, `dir = TradeHanySzallitas[EN] < 1` (the old model turns and fades out; `Trade3DUpdate` step 11 loads the new one when the angle has arrived). Then `TradeVehicleOld = Which; TradeVehicleOldType = Type`.
8. `AktIkon2 == 0x25 && Lenyom`: `Trade_GetPodListaMode = 1; X = Mouse_x; Y = Mouse_y` (the popup shows while held; step 0 clears it on release).
Exe-only: when `Felenged` and `AktIkon` is 0x37, 0x38 or 0x39 the exe jumps to the end (step 8 not evaluated): same result.

### 6.3 `MULTIGET_Trade` UI side effects (ELF 0x7afa70; rules in trade.md section 4), for `e == EN` after a purchase is accepted
Sold out (`class < 0`): `Which = 50000; MennyiOsszesen = 0; if TradeVehicleOld != 50000: done flag = 0, dir = TradeHanySzallitas[EN] < 1, TradeVehicleOld = 50000, TradeVehicleOldType = Type`. Otherwise `MennyiOsszesen = units left; ActVehiclePrice = TradeAr(i, 1)`. Always `qty = 0`.

## 7. Orders
Only message 0x51 (`MULTIGET_Trade`), payload 0x98 bytes as in 6.2 step 4; sender byte `EN`. No other `Multi_Send` in the screen.

## 8. Timers / animations (`FRISSIT_Trade2`, ELF 0x7b0180 / exe 0x605b60, per 100 Hz tick; the body runs only when `AktKep == 8`, the ticker line before that check)
1. `if (ScrollQueueLength != 0) ScrollHova += 0.8`.
2. `TRADExacel = (TradeHanySzallitas[EN] < 1) ? TRADEXAMAX : 0`; move `TRADExa` toward it by 3.6 (down: clamp at 0; up: clamp at TRADEXAMAX); 185 to 0 takes 52 ticks.
3. `TRADEnyitottsag`: if `cel == 0 && value != 0`: `value -= 0.03; if value <= 0: value = 0, cel = 1`; else if `cel == 1 && value != 1`: `value += 0.03; if >= 1: 1`.
4. `trakeringeto += 0.0006`.
5. Opening: `traKELLKINYITAS && !traBECSUKAS && !traKINYITVA && traELINDULT`: `traframe += 0.36363637` (4/11), cap 50 (137.5 ticks = 1.4 s). Closing (`traKELLKINYITAS && traBECSUKAS`): `traframe -= 0.6`, floor 0 (83 ticks).
6. Hull model angle: if `target != angle`: `angle < target: angle += 0.05 (clamp to target)`; `angle > target: angle -= 0.05 (clamp)`.
7. Selection follow: if `Which != 50000 && KeresAktualisTervSorszamat() != TradeAktualisCucc && TradeTervek[TradeAktualisCucc].class > 0`: the offer list changed under the selection (sell-out, expiry, new offer): `Type = class(TradeAktualisCucc) - 1; Which = number of offers before index TradeAktualisCucc with the same class` (re-select the same slot position), `VoltMarToltes = 1`, `TRADEnyitottsagcel = (TRADEnyitottsag != 0) ? 0 : 1`, `TradeVehicleOld = Which; TradeVehicleOldType = Type` (so no model reload).
8. Clamp `shipPos`, `tankPos` to at most `count-8`, then to at least 0.
Per rendered frame (not per tick): hull spin 0.03 (`ForgatTradeAlap`), `trawireframe += 0.25`, the right-button quantity repeat.

## 9. Sounds
| Event | Sound |
|---|---|
| Screen opens (only with the opening animation) | `PlaySound(Trade_Hangok[0])` = `sounds/screens/trade/screen19.wav` |
| Screen closes (only with the closing animation) | `PlaySound(Trade_Hangok[1])` = `screens/trade/scrn19ki.wav` |
| Press on minus, plus, destination, a list row | `PlaySoundGlobal(0)` = `ingame/Click1.wav` (`Global_Hangok` exe 0x919A60, stride 0x11c) |
| OK, on the press [obs] | `PlaySoundGlobal(2)` = `ingame/Click3.wav` |
| scroll bars, pod list, ticker | none |

## 10. Exe vs ELF differences seen (and quirks)
- Inlined in the exe: `TozsdeScroll` (Belep), `RemoveTradeAlap` + `traOldalsoFenyekKi` (Kilep), `CheckKivalasztottTradePortosBolygoNemElfoglaltak` (top of `GFX_Trade`), `TradeIgazitShipPoz/TankPoz` (Main), `traMeshKiBe`, `traSetObjTransparencyH`, `traSubMeshKiBe` (in `Trade3DUpdate` / `EltuntetGombok`), `TradeNextTerv`.
- Folded: `TradeGfx2D_1` = exe 0x559ad0 (empty); `OroklodikVisszaallitas` and `reloadtextures` are empty no-ops (shown as `WaitForDrawing(...)`).
- `TradeLoadAlap`: the exe passes `k` as a third `LoadBox` argument (the ELF has two); same partition of `base`.
- `Trade3DUpdate` rotation: exe first term `(float)sin((double)(x*2.0f)) * 3.0f` (f32 multiply), ELF `(float)(sin(...) * 3.0)` (double). `traMozgatMainFenyek`: exe light 3 `(float)sin * 300.0f`, `(float)cos * 200.0f` (f32), light 4 in double; ELF all double. `ForgatTradeAlap` and `Trade_MouseInit2`: exe `(1 - (float)cos(angle)) * 300.0f` or `* -200.0f` in f32, ELF in double.
- `FRISSIT_Trade2`: the exe adds double constants (0.8, 3.6, 0.03, 0.0006, 4/11, 0.6, 0.05) to f32 variables and rounds back (`CVTPS2PD; ADDSD; CVTPD2PS`); the ELF listing prints f32 constants (0.36363637).
- `TradeDisplayScrollBar`: the exe works in double (1.00001, `MULSD/DIVSD`), the ELF decompile looks f32.
- `TradeGfx2D_2` delivery rows: ELF `i < 10`, exe stops when `y > 0x1d3` (the same for rows 0..9).
- `Trade_Main` exe: early jump to the end when `Felenged` for ids 0x37 / 0x38 (no practical effect).
- Quirk: the shown total price is `qty * TradeAr(i,1)`, the charged and sent price is `TradeAr(i,qty)`; for fighters (a unit = price/5) `TradeAr(i,1)` is truncated first, so the displayed total can be lower than the charge (the OK-enable test uses `TradeAr(i,qty)`).
- Quirk: right button quantity repeat runs once per rendered frame; the pod list popup is bound to `Lenyom` of hot-spot 0x25 (the whole model block).
- Quirk: `TradeCalculateVehicleNum` subtracts 8 from the list position instead of clamping.
- The `tf_konnyut.xvb` second model, `TRADEnyitottsag` and `TradeSetGombMousePoziciok` are dead code.

## 11. Check in the asm (exe addresses)
| Where | What |
|---|---|
| 0x6084d6..0x608517 (`Trade3DUpdate`) | wire sphere rotation: `MULSD 2.1` (0x852340), `MULSD 2.0` (0x852338), `MULSS 2.0f` (0x848540), `MULSS 3.0f` (0x848544): f32 vs f64 |
| 0x6085fa..0x608656 | ball transparency: double `1.35 - (traframe*2)/50`, clamps 0.0001 / 0.9, f32 1e-6 (0x848518), 0.9999999 (0x85ab00) |
| 0x6085a0 | `trawireframe + 0.25` in double then f32 |
| 0x60876f..0x60879c | `(int)(traframe/50 * 128.0f)` and the grey colour build (`Copy3hatterkep`) |
| 0x6087ac..0x6087c9 | `PlaceVehicleInfo` x: `(int)((TRADExa + 210.0f) - 190.0f)` (f32) |
| 0x605b74, 0x605be4, 0x605c1e, 0x605c6c, 0x605cae, 0x605ce1, 0x605d1d, 0x605d51, 0x605d9d, 0x605dc5 (`FRISSIT_Trade2`) | double constant adds; check the 0.05 constant in `XMM2` and the compare orders around 0x605d84..0x605dbd |
| 0x606905..0x606b37 (`ForgatTradeAlap`) | f32 `SUBSS/MULSS` (0x606919, 0x606925, 0x606b0d, 0x606b19) vs double `SUBSD/MULSD` (0x60693f, 0x606943, 0x606b33, 0x606b37); the ratio `DIVSS` at 0x6069e1 |
| 0x60b468..0x60b4af, 0x60b4d4..0x60b504 (`traMozgatMainFenyek`) | f32 first light, double second light (+1.57075 at 0x852b30) |
| 0x60af19..0x60af6f (`Trade_MouseInit2`) | f32 `(1-cos)*300` / `*-200`, `CVTTSS2SI`, then `(float)(int) + TRADExa` truncation |
| 0x60aba9..0x60abc8, 0x60ac11..0x60ac30 (`Trade_Main` scroll drag) | `SUBSS 63.0 (0x861dd0)`, `DIVSS 86.0 (0x861dd4)`, `MULSS count`, `SUBSS 4.0 (0x852480)`, `CVTTSS2SI`: check the constants and f32 |
| 0x60a7f0 (list click) | the signed division by 0xb (magic multiply) of `Mouse_y - 0x3f` and division by 2 of `n*13` (signed truncation toward zero) |
| 0x608900..0x608a40 (`TradeDisplayScrollBar`) | double math at 0x608946..0x608999 with 1.00001 (0x856af0), `CVTTSS2SI` on the thumb height and `y + off` |
| 0x609599..0x6095c8 (`Gfx2D_2` `HatarY2`) | all f32: `n*15+8`, times `traframe/50`, `+320 -7 -25`, `*479/479`, `CVTTSS2SI` |
| 0x609198..0x6091b1, 0x606dc1..0x606dda | `(double)price <= MoneyMennyi` compare through x87 `FSTP double` (the OK condition) |
| 0x606e98..0x606f49 (`GFX_Trade` deliveries panel) | f32: `(1-f)*139*0.5+295`, `f*169`, `255 - f*128` / `f*228` |
| 0x607d4d..0x607e3f, 0x607ee0..0x607f95 (ticker) | the `ScrollHova/200` factors in DOUBLE (`CVTSS2SD/DIVSD/MULSD`) vs f32; `(int)(traframe + 50)` |
| 0x6078c5..0x6078dc (`PlaceVehicleInfo` speed) | `MULSS 100.0f` then `ADDSD 0.5` then `CVTTSD2SI` |
| 0x607020 / 0x608a60 (list walks) | read `TradeTervek` records past `N` when `Which` exceeds the class count (returns `N`) |
| 0x608340 end (step 11) | `TradeLoadAlap` arguments read at record index -1 when nothing is selected |
| 0x609c60 (`TradeLoadAlap`) | the third `LoadBox` argument (`k`, f32) and the 0.08 scale: f32 `MULSS` or double |
| `Gfx2D_2` call sites | flags 4 / 5 / 6 / 0xc and the colour argument at each `KiirStr` / `KiirInt` call: read from the decompile and the ELF asm, the exe call sites were not re-read one by one |

## 12. Unknowns
- Exact draw layering of the `KiirStr` calls issued from `GFX_Trade` itself (`Trade_KiirPodNagyLista`, the ticker text, `PlaceVehicleInfo`) relative to the null-box callbacks and fader passes (they are not in a render callback; the `KiirStr` queueing was not decompiled). The pod popup box is pass 3, its text is drawn from `GFX_Trade`.
- Palette indices 0, 1, 2, 0x22, 0x25 (`DDColor`, hud.md); the flag meaning is taken from messages.md section 1.
- Semantics of `fxn/fyn/fzn/zbm` and of `xcenter/ycenter/kamerapont`.
- The `TradeBrushok` record (empty name; no brush id used) and the 9th argument (1) of `Mouse_Szoveg_Set`.
- `TRADEnyitottsag` is written but never read in the screen.
- The Hang record flag at `+0x114` is 0 in the exe image (set at run time? see sound.md).
- Where `TradeVehicleMennyi` is capped when the offer shrinks under the selection through ANOTHER empire purchase (FRISSIT does not clamp it; `MULTIGET_Trade` resets it for the buyer only): not found, so qty may exceed the units left until the next row click.
- `ScrollSorok[0]` content assumed = `GetString(8,4,1)` from rules/new-game.md row 41 (not re-read).
- Which submesh of `tozs1bck.xvb` is which (wire sphere = submesh 0, parts 2 and 4, button pairs) was not checked against the xvb file.

## 13. Client status
Modules: `ig2-client/src/trade.rs` (screen state, 2D passes, input, hull file choice `hull_file`, the order) and
`trade_scene.rs` (tozs1bck frame, wire sphere, hull model, lights). F8 opens it (`DoKeyboard`), F2 closes it with
the closing (scrn19ki), Esc goes to the options screen at once; Options "back" re-enters as `Trade_Belep` with
`OldKep` 0xd (`Trade::back`). OK sends 0x51 as `ig2_net::Command::Buy` (the 0x98-byte `TradeSzallitasTemp`); the
§6.3 side effects run when the human's delivery count changes; tutorial event 0x27 is raised in `ig2_sim::Game::buy`.
Done: §3–§9 (lists, scroll bars, buttons, destination popup, deliveries panel, ticker, hull model and its card,
pod list, sounds in order and tick), the glass panels, the wire sphere, the hull's lighting.
Not captured: the multi-line pod list and a second offer's turn-away/reload (quick-1 has one ship offer without pods).
Open `re/deviations.md` rows: 107 (tick-bound repeat rate, ADR 0016), 229, 250. The row frame's morph phase is one
global as in the exe.
Captures: `ig2-client --snapshot --screen trade [--script S] [--trade-cheat] [--trade-eta N]`; `run_screendiff.py
<install> quick-1.sav trade` / `trade-buy` (scripts `tools/trace/scripts/trade.json`, `trade-buy.json`,
`trade-pods.json`). No save has a trade port: `--trade-cheat` sets the exe's `TradeCheat` (0xF43835) in both runs;
`AktEtaDays` (unsaved) and the `DesignKeret1` phase globals go from the exe's script start to the client through the
state save's `ui.json` (`trade_eta`, `dkm` → `run_screendiff --dkm M,CEL,NOVELO`). Light probe: `run_screen.py --kep 8
--script tools/trace/scripts/trade-pods.json --light-probe --jsonl`.
