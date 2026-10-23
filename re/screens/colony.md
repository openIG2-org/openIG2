# The colony screen: camera, terrain, buildings, units

How the original draws a planet's colony screen (`AktKep == 3`, planet `AktPlanet`). Evidence: the ELF oracle (`ig2`,
symbols) decompiled with `tools/ghidra/DecompileFunctions.java`; addresses are ELF unless marked exe. The exe was read
where offsets or values differ (camera globals, battlefield layout, tables) and the result was checked against
reference captures of the running exe (§8). `[inf]` marks inference.

Scope: the 3D view (camera, terrain object, buildings, roads, units, shots), what is hidden on planets the human
doesn't see into, the HUD's layout, the building orders (`MULTIGET_Colony`, §9) and the building screen's input with
the 2D parts it owns (§10), the ground war's input and 2D parts (§11). The rest of §7 is not specified here.

## 1. Frame structure

- `Colony_Belep` 0x4d8fd0 (exe 0x465b40): brushes `AktBrushok = ColonyBrushok` (0x115a570, 32 records), the
  virtual screen 480 lines high widened for the window (as the starmap, `re/screens/starmap.md` §2.3), sounds per
  surface/weather, then `Init3dColony`; reads the camera from the battlefield (§2.1) and, when `WF+0x26c` (exe
  `+0x184`) is set, `HaVanEpuletAkkorAzLegyenKozepen` 0x4d8d20 (§2.1); `Multi_Send(0x26)` tells the others which
  planet is shown.
- `Init3dColony` 0x4c6050 (exe 0x47d520): `VectorDir = vector/colony/`, `TextureDir = textures/colony/`; fog range
  (2000, 2200); lights; the background dome; the terrain object; the buildings; the scenery objects
  (`LoadScapeObj`), particles, weather, the roads (`RecreateRoadSystem`); the 2D pass hooks
  `ColonyGfxCallbackHordozo0..4` → `ColonyGfx2D_0..4`; `WF::LoadMesh` and `WF_Tank::LoadMesh` for the battlefield.
- Per drawn frame: `GFX_Colony` 0x4da050 (HUD faders, `ColonyMouse`, the 2D hooks) → `ShowColony` 0x4ced80 (exe
  0x49f130: visibility, building states, camera, `Update3D(…, 1)` → `renderscene(1)`), then a second
  `Update3D` into the radar panel's viewport (`HatarX1..` = 0x12..0xad × 0x24..0xa7, centre (95, 100)).
- `Colony_Kilep` 0x4d9b90 writes the camera back into the battlefield (`WF+0x2a8..+0x2b8`, exe `+0x1a8..+0x1b8`).
- Per sim tick nothing of the view changes; the battlefield's units move in `FRISSIT_Grwar` (re/rules/ground-combat.md
  §11) and buildings in the colony economy.

## 2. The 3D scene

### 2.1 Camera
State (globals, exe): `ColonyXStart` 0xBA90DC, `ColonyYStart` 0xBA9514, `ColonyZoom` 0xBA9E8C, `ColonyPitch`
0xBA9B64, `ColonyRotate` 0xBA983C. Per battlefield they are saved as `WF+0x1a8..+0x1b8` (exe; ELF `+0x2a8..`), in the
save's `WF[i]` chunks (`ig2-sim` `Field::tail.f1a8`). New games (`InitNewGame`, `WF::WF`) set (−517.5549, 499.8431,
−1100, −π/4, 0); `Colony_Belep` resets |x| or |y| > 5000 to 0.

Entering with `WF+0x184` (exe; ELF `+0x26c`, saved, set by the constructor) set: `HaVanEpuletAkkorAzLegyenKozepen`
selects the first building (`SelectBuildingRelative(1)`) and glides there: target `(ColTargetX, ColTargetY) =
(gx, −gy)` of `Grid2Real(building)` (§3.1, without the odd-size offsets), `x += (target − x)/2` for `ColTFrame` = 10
frames (exe `ShowColony` lines at 0x49f130: 0x9964d0/d4 targets, 0x9964d8 flag, 0x9964dc count), so it stops 1/1024
short; then the flag is cleared (not for a foreign planet: there `AktEpulet` goes back to 0).

`ShowColony` builds the camera every frame (exe matches the ELF):
```
x, y clamped to ±3800 (+ random(31) − 15 each during an earthquake on this planet, Colony_foldrenges)
pivot  = (0, x, −y)                       (h, x, y)
r      = Zoom − 800
eye    = pivot + (r·cos(Pitch), sin(Rotate)·r·sin(Pitch), cos(Rotate)·r·sin(Pitch))
MaximizeCamera(eye, pivot, 3900)          0x4ce5e0: eye pulled back to ±3900 along eye→pivot, pivot moved alike
g      = GetHeightCam(eye)                0x4be590: height under the eye, cells clamped 0.6..48.9
if g − 100 < eye.h: pivot.h = −(eye.h − (g − 100)); eye.h = g − 100
```
Heights are `(h, x, y)` with **up = −h**. `renderscene(1)` 0x6034f0-area uses the look-at path
`CameraRotate(eye, pivot)` 0x5fcf90: `d = normalise(pivot − eye)`, `a = normalise(d × (1, 0, 0))`,
`b = normalise(d × a)` (`CreateMatrix2v` 0x453690, `vektormul` 0x453440), camera space
`e = (a·q, −b·q, −d·q)`, `q = p − eye`; projection as the starmap's: visible for `e.z < zclip = −1.05·zbm`
(`zbm` = 25: −26.25), screen `(e.x·600/−e.z + 320, e.y·600/−e.z + 240)` on the 480-line virtual screen
(`xcenter` 320, `ycenter` 240). Scene objects are in units of 1/1024 (`MoveObj` divides by 1024;
`renderscene`/`dumpvertexlist` multiply the camera matrix by 1024): every world value below is in world units.

Mouse (`ShowColony`): both buttons held → `Rotate = last − dx/200`, `Pitch = last + dy/200` clamped
[−1.4922565, −0.05]; edge scrolling `Colony_Autoscroll` 0x4e2e40 (pointer within 8 px of a window edge, speed ramps
by 5 to 30, `Colony_Move`); zoom/pitch tweens `0x998244..` (×0.25 per frame for 15 frames, exe).

### 2.2 Terrain object
`Terepek[map]+0x11` (`terepleiro.txt` `[felszin]` model, `re/formats/colony-map.md`), loaded from
`vector/colony/terrainobj/` with `TextureDir = textures/colony/terrainmap/<surface>/` (surface = planet `+0x22`):
`AddToScene(…, mainxvb1, 1.0)`, `ScaleObj(4, 4, 4)`, `RotateObj(0, π/2, 0)`, `MoveObj(Terepek+0x8c, 0, 0)` (the map's
base height `200 − b`), then `TEREPMAXX1` = the largest vertex x (`dumpvertexlist`), `ScaleObj(k, k, k)` with
`k = 4096 / TEREPMAXX1` and the rotation again: the terrain spans ±4096 (`TEREPMIN/MAXX/Y`; asserted equal to
`TEREPXSIZE/YSIZE` = 8192). Child names: `Group*` (`FixLandscape`), `tt*` and `fak*` (trees: alpha-tested, no
lighting 0x100/0x200), `gejzir*`, `tavak*` (lakes: surfaces 1 and 5 swap in the dome's texture at transparency 0.28,
others 0.4), `lava`, `VIZ` (water, transparency 0.63).

**Verified** (`ig2-client --colony-check <map> <surface>`): with this transform the terrain's vertices sit on the
colony map's heights (`GetHeightFloat`, §2.3) — median difference ≈ 0 and p10/p90 within ±0.6 world units on maps 0,
1 and 30 (objects and the rim excepted).

### 2.3 Map coordinates
- `Grid2Real(gx, gy)` 0x45f950 (200 × 200 grid → world): `x = −((gx/2/99·100·S)/99 − S/2)`,
  `y = (gy/2/99·100·S)/99 − S/2`, S = 8192: **grid x runs against world x**.
- Height cells (50 × 50, `ShowColony`, `getbottomvertex`): `fx = 49 − ((x + S/2)/S·50·49)/50`,
  `fy = ((y + S/2)/S·50·49)/50`; `GetHeight(fx, fy)` 0x46bff0 (0 past 200) → `GetHeightFloat` 0x464090 (ported:
  `ig2-sim::ground::GroundMap::height`).

### 2.4 Object transforms (`re/formats/xv.md`)
A scene record holds a translation `t` (scene units), a scale matrix `S` (`+0x10`), a rotation `R` (`+0x64`) and their
product `R·S` (`+0x88`; `MultipleMatrix(a, b)` 0x4518f0 = `b·a`, column vectors). `Kocka2` 0x5fac80: a child sits at
`P·t + T` with matrix `P·(R·S)`; mesh vertices are `P·v + T`.
- `AddToScene(xvb, parent, s)` 0x4551d0: `S = diag(o/max(o))·s` (o = the root's half-extents `+4..+0xc`, `+0x4c` = 1).
- `AddToSceneGenesis(xvb, parent, s)` 0x455320: `S = diag(o)·s`.
- `ScaleObj(a, b, c)` 0x456eb0 scales `S`'s rows; `RotateObj(a, b, c)` 0x455a60: `R = CreateMatrix(a, b, c)`;
  `RotateObj2(axis, θ)` 0x455c40: `R = axis(θ)·R`; `SetObjMatrix(m)` 0x4573b0: `R = m`; `MoveObj(h, x, y)` 0x455f80:
  `t = (h, x, y)/1024`. `RotateObj(0, π/2, 0)` maps a model's `(x, y, z)` to `(−z, y, x)`: the models' +z is up.

### 2.5 Lights, fog, background
`Init3dColony`: ambient 0x969696 on `mainmeshid1`, 0x404040 on `mainmeshid2`, parallel lights; `SetEnvironment`
0x4caa10 every frame (fixed time of day 12): ambient `light1` 0x1e1e1e, the dome's `light3` 0x323232, the parallel
`light2` 0xc8c8c8 from (2, 2, −2), sun and aura particles at `−4500·(sin, sin, cos)` of the hour angle. Fog:
`setfogrange(2000, 2200)` 0x6062a0 → `globalfogtable` 0x603090 (none up to depth 2000, then linear over
(2200 − 2000)/0.03), colour `Hatterek[bg]+0x20..+0x28`; `SetBorderFog` 0x4be2c0 gives terrain vertices beyond 95 % of
the half-size fog 0.5 and beyond 99 % full fog; `fogalap.xvb` (scale 12.1, moved to h = 1600) is fully fogged. The
remaster's GL renderer shows **no distance fog** in the reference captures (§8) [obs]; the client keeps the border
fog only. The terrain and buildings sit under `mainmeshid1` (ambient `light1` 0x1e1e1e, the parallel `light2`
0xc8c8c8; the lightning `villamid` is black), the building panel's model under `mainmeshid2` (0x404040 and `light5x`
0xffffff from (10, 10, −10)); the remaster draws them in gamma space with the shadow map: render.md §4–§5.

The background: `melyikhatter = WF+0xa04` (exe `+0x8d4`, `Field::background`, `InitWeather` 0x4c1160), `LoadBox(Hatterek[bg])`
(`[hatter]` sections of `terepleiro.txt`), `AddToScene(…, 11.0)`, `ScaleObj(1, 1, 1.1)`, `RotateObj(0, π/2, 0)`,
`MoveObj(−3000, 0, 0)`, unlit and unfogged. Weather `WF+0xa00` (exe `+0x8d0`): 1 snow, 2 rain with lightning
(`OptionsKellIdojaras`).

## 3. Buildings and roads

### 3.1 Buildings (`Init3dColony` building loop)
For each used slot `Epuletek[p][k]` (32 bytes; kind `+0`, grid position `+0xc/+0x10` floats, work left `+8`, flags
`+0x19`): the model `Building[kind − 1]` (exe 0x8F1F28, stride 0x6c: name `+0`, preview name `+0x28`, scale `+0x50`,
preview scale `+0x54`, footprint `+0x58/+0x5c`; `ig2-rules::building_models`) by `LoadMaxLevelBox` (highest LOD,
`OptionsBuildingRes`), `AddToSceneGenesis(model, scale/1024/1000)`, `RotateObj(0, π/2, 0)`; children `_A_` get
transparency 0.6, `_B_` the dome's texture (environment-mapped, 0xc000), `_C_` both. Position:
`Grid2Real(x + (w odd)·0.5, y − (h odd)·0.5)` and height `getbottomvertex` 0x47c750 − 1: over the model's lowest
vertices (within 0.5 % of its height range) the largest `GetHeight(under the vertex) − vertex h` — the model is
lowered until every bottom corner touches the ground; the result is stored in the slot's `+0x14`. Then
`Allocate(x, y, w, h, true)` (§3.2), lights and effects (`SetLangPlaces`, `PlaceVillam`, `AddBuildingSpecEffect`).

`ShowColony` per frame: finished buildings are opaque, except flag `+0x19` bit 4 (`sin(ColonyCounter/15 + k)·0.25 +
0.26` transparency: blinking); unfinished ones are cut at `1 − work_left/(EpuletekLeiro[kind]+4 · 50)` (÷1.5 for
race 4) of their height (`SetEpuletVagas`) and get scaffolding (`EpuletAllvanyok::AddAllvanyok`, 10 steps). The client
draws unfinished buildings see-through instead [deviation, cosmetic].

### 3.2 Roads (`RecreateRoadSystem` 0x4bee90, option `OptionsKellUt`)
`MapAlloc` (200 × 200) = the map's build mask as 4 × 4 blocks (`Decode2LargeMap`, re/rules/colony-placement.md) plus
every building's interior (1) and frame ring (+0x20 per building) by `Allocate` 0x4bf6a0 in slot order. Rows: each
run of frame cells (≥ 0x20) longer than 3 becomes a road; an end steps in by one cell where the rows above and below
are frames too; `AddUtDarab(row, from, to, 0)` 0x4be960 marks its cells (bit 8, unless bit 0x80). Columns: runs of
frame cells without bit 8 → `AddUtDarab(col, from, to, 1)`. A run of 10+ cells is split into pieces of 9. A piece of
`n = min(to − from, 9)` cells is `ut<n>.xvb`, `AddToScene(…, utdarabsize[n − 1])` (exe 0x8EC968, 9 floats;
`ig2-rules::road_scale`), `RotateObj(0, π/2, 0)` (row pieces also `RotateObj2((1, 0, 0), π/2)`), centred at
`Grid2Real((from + to)/2, row + 0.5)` (columns: `(col + 0.5, (from + to)/2)`), at height
`GetHeight((from + to)/8, row/4) − 2.5` (columns swapped).

### 3.3 Planets the human doesn't see into
`ShowColony`/`GFX_Colony`: `WF+0xa1b = owner == EN || BolygoTipus::pMuholdakMennyi(EN) || WF+0x9dd+EN` (the last an
unsaved runtime flag) or the planet of an MP ground battle. `pMuholdakMennyi(e)` 0x4f0ea0 = planet `+0xc4 + e`
(satellites/spies; **empire 7 with 0 there counts as 1** [original quirk]). When clear: buildings and roads are
hidden (`HideObj`), so are the units (`WF_Tank::Draw3d`/`Update` test the flag), and `ColonyGfx2D_1` draws
`DrawBuildingBases` 0x4d2150: per building a quad of `hazalap.bmp` (`WF+0x220`), ±w·41/2 × ±h·41/2 around its
centre, colour 0x800000ff (red, half transparent). Verified by capture (§8).

## 4. Units (`WF_Tank`, re/rules/ground-combat.md §11)

`WF_Tank::LoadMesh` 0x46efc0, per frame `WF_Tank::Update` 0x47f9e0; visible units are alive, not mines (0x20000)
and placed (bit 2 clear). Exe unit offsets are ELF − 8 from `+0x164` on, −0x10 from `+0x1d0` (`ig2-sim` names).
- Hull: `TankAlap[hull]` (exe 0x8F50D8, 5 × 0x38: name, `+0x34` vector directory 0 `starmap/transfer`,
  1 `spacewar`, 2 `talalmanyok`, 3 `colony`; all 3), `AddToSceneGenesis(model, TankMeret[hull]/1024/10/5)`
  (`TankMeret` exe 0x8F1BC4); the first sub-mesh's texture becomes `tankok/<kerekes|kozep|lebeg|lanc|aknarak><owner ≤ 10>.bmp`.
- Placement each frame: `SetObjMatrix(body)` with `body` = the unit's orientation `+0x40` (`ModifyTankMatrix` from
  the smoothed terrain basis `+0x1c` and the body angle: `ig2-sim` `TankRt::orientation`), `MoveObj(P + body·(0, 0,
  TankPivotAdder[hull]))` (exe 0x8EC524) with `P = (z, x, y)` (`+0x1dc`, `+0x1d4`, `+0x1d8` exe); hovering hulls
  (flag 0x200000) add a bob, a dropship ride adds `+0x27c..+0x284`.
- Weapon (`+0x130` has a gun): the design's first pod `+0x51` (else `+0x52`; neither → 0x11 air-strike pod) − 0xa1
  indexes `TankTorony` (exe 0x8EC538, 19 × 0x38: `gepagyuv0 konnyuv0 laseragyu nehez megav0 raketas1 raketas2
  fagyaszto mani a1v0+ … fr4v2++ airstrikepod laserstrikepod`), scaled `TankToronySize[hull·9 + k]/1024/10` (exe
  0x8F1BD8; 0x11 → 1.953125e−5, 0x12 → 8.789063e−5); matrix `ModifyTankMatrixToTorony` = the turret orientation
  `+0x64`, at `P + body·(0, 0, pivot) + body·(0, −10, 12)` (`Forgasd` 0x47f780 = matrix × vector).
- Defence turrets (flag 0x4000000, `WF::AddBuildingTurrets`): `TankTorony[pod − 0x98]` (pods 0xa1..0xa8 →
  `a1v0+ … fr4v2++`), `TankToronyMeret[pod − 0xa1]/1024/10` (exe 0x8F1BA4), turned by the turret angle about h, at
  `P`; no hull.
- Own units being placed: transparency (`+0x190` bit 15); selection rings, targets, shadows and the route lines
  (`line3d`, colour 0x80be8882) are `WF_Tank::DrawSelection/DrawTarget/DrawShadow/Draw3d`.

## 5. Shots and effects (`WF::LoadMesh` 0x46c300)
The battlefield's projectile lists (shells `+0xd4`, missiles/bombs/debris `+0xd8`, bolts `+0xdc`; `ig2-sim`
`Lists`) are drawn with `rocket laser racs2 paralyze biglaser1..5 akna vad manipulator manipulated hordozo megashot
heavyshot lightshot repesz1..3` (`.xvb`) and particle textures (`star rcloud flare lens/* epuletkosz tankkosz`); the
fighters of an air strike (`WF+0x1f4 + 0x40k`) and dropships (`hordozo.xvb`) are models too. The client draws them as
glowing points (shells yellow, missiles orange, debris dim, bolts blue, fighters white) [deviation, cosmetic].

## 6. HUD layout (`ColonyBrushok` 0x115a570, exe 0x8ECA08; `COLONY_X.BMP` = kep 17)
Records `(src x, src y, dst x, dst y, w, h, step x, step y, frames per row, kep)`, a negative size = source in the
original's pixels on the doubled remaster picture (pinned in re/screens/hud.md §1, §3 with the passes and the radar):
12 = planet panel (dst (0, 18), 223 × 153),
13 = radar frame (491, 348; 149 × 132) with the minimap drawn by the second `Update3D` and `DrawRadar`, 14 = the
build/info panel (455, 23; 182 × 148), 1–11, 19–27, 29–30 = buttons (31 × 31, 39 × 20, 20 × 39, 72 × 19 cells),
17/22/23 `STAR_W.BMP` (kep 97), 18 `TALAJ.BMP`, 28 `FUV6.BMP`, 31 `ZAJ.BMP` (radar noise). `GFX_Colony` also draws
the faders behind the panels and `GFX_DateMoneyScrollFade` (only faders: the top bar itself is the persistent layers'
`GFX_IdospeedPenzDatum`, re/screens/hud.md §2). The 2D passes: `ColonyGfx2D_0` (lens flare), `_1` (landing zones, build grid, building
bases/selection, `WF::WriteStuff`, the radar, special-weapon buttons), `_2` (planet info text, rain/sandstorm),
`_3` (repair/demolish windows, prices), `_4` (texts, self-destruct). `ShowBuildings` 0x4c9220: building names
(`Talalmanyok[EpuletekLeiro[kind]].name`, mafia names for flagged ones) at the projected building position, faded
over depth 2300..2800.

## 7. Not specified here
The ground war's landing clicks, shadows, landing zone, unit picture and self-destruct window (§11.4), the planet info mode (`ColonyPlanetInfoMod`, `StarmapHosszuInfoKiir`, hot-spots
0x15..=0x1e: their tax / autobuild / priority drop-downs send starmap message 0x19), renaming the planet (`IOMode`),
`ColonyGestureHandler`, the small turning model beside the building info (`HandleKisInfoObject`), the self-destruct
buttons (hot-spots 8, 10, `WriteSelfDestruct`), the shadow map (`rendershadowmap`), scenery objects (`LoadScapeObj`), weather particles, lightning, lens flare, building
effects (`AddBuildingSpecEffect`, `PlaceVillam`), construction cuts and scaffolding, landing zones, the build grid.

## 8. Reference captures
`tools/trace/run_screen.py <install> <save> <out> --planet P [--cam X,Y,ZOOM,PITCH,ROTATE] --run` loads the save in
ig2.exe (`LoadSave`), opens the colony screen (`AktKepKell` 0xBAE250 = 3, `AktPlanetKell` 0xBAE254), keeps the
pointer in the window's middle (`WM_MOUSEMOVE`, else edge scrolling moves the camera) and grabs the window; it
reports the live camera globals and the camera points (`forgaspont` exe 0xDE0130, `kamerapont` 0xDE0148). quick-1.sav,
planet 42: the exe's camera after entering is pivot (0, 375.6696, 333.1013), eye (−1343.5028, 375.6696, 1676.6041);
`ig2-client --snapshot --colony 42` computes the same values; buildings, roads, terrain and rocks line up in the
images. Planet 54 of `replay-quick1-battles-ground-x16/0.sav` (owner AI 3, no satellites): red footprints only, as the
client draws them. Captures hold game imagery: `_local/snapshots/ref/`, never committed.

## 9. The colony screen's orders (`MULTIGET_Colony`)
`MULTIGET_Colony` (ELF 0x4ec2e0, exe 0x48a270; found by its `Multi_GetNextTipus(0x1f, 0x27)` / `(300, 399)` calls) runs
every frame like the starmap's handler (`re/screens/starmap.md` §10): each message is `Multi_Get(type, &issuer, …)`
with int values, applied on every peer. Port: `ig2_sim::game::ColonyOrder` (`from_message`/`to_message`),
`Game::colony_order`, `ig2_net::Command::Colony { kind, values }`; outcomes for the issuer's screen are
`GameEvent::ColonyOrder(ColonyOrderEvent)`. Values: planet p, building slot k (1-based), kind t.

| Type | Values | What the exe does |
|---|---|---|
| 0x1f | p, t, x, y | `NemSzabadEpiteni − 1` when from `EN` (UI); unless the research cheat 0xB7727C is set: `MoneyMennyi(e, 0) < GetGyartasAr(e, EpuletekLeiro[t].tech)` (f64 compare) → the line `GetString(3, 2, 10)` ("You don't have enough money to build a %s!") for the issuer, whose screen drops the cursor (`ColonyRemoveBuilding`), nothing else; else `MoneyHozzaadLog(e, 0, −price)`. Then `EpuletNewSearchLocation(p, t, false, 0, x, y)`: `LocateSuitableLocation` searched from (x, y) (colony-placement.md §3–§5); no room → no building and **the money stays spent** [original behaviour]. `EpuletNew` (exe 0x53c7d0: the tutorial's events, `AddBuildingTurrets(t, 100, owner, slot, p)`); the issuer's screen on p selects the new slot (`AktEpulet`, when `CanModifyAktEpulet`). |
| 0x20 | p, k | `MoneyHozzaadLog(e, 0, GetGyartasAr(e, tech of Epuletek[p][k]) / 2)` (int division: `cdq; sub; sar`), `EpuletDelete(p, k)` (exe 0x53c730): a `'+'` building resets the self-destruct, the colony screen on p drops `AktEpulet`, turrets removed, kind 0, `HanyEpulet[p] − 1`, `RemoveTarget(−k)`. No check of k: an empty slot is "deleted" again (`HanyEpulet` one too low until the next recount, below) and a slot past `HanyEpuletLefoglalva` reads the block's unused records; the port refuses k outside 1..=`HanyEpuletLefoglalva[p]` (the screen never sends one). |
| 0x21 | p, k | `+0x19 ^= 2`. The colony screen's code never sends it. |
| 0x22..=0x25, 0x12d, 0x12f, 0x130, 0x134 | p, … | the ground war's orders: §9.1. |
| 0x26 | p | `KiMelyikBolygonVan[e − 1] = p` (`(&0x9ABAE4)[e]`, unsaved; e = 0 writes `Colony_elozo_foldrenges`). `Colony_Belep` sends it. |
| 0x27 / 300 | p | the self-destruct starts (`WF+0x334 = 399`, ground-combat.md §11) / is called off (−1). |
| 0x132 | p, k | `+0x19 ^= 0x20`: the building's on/off switch (hot-spot 0x11 "Switch on/off", key E; `FRISSIT_Planet` keeps a switched-off building idle). |
| 0x133 | p, k | if the slot's kind ≠ 0: cost = `EpuletJavitasAr` (inlined) = `(int)((f32)Talalmanyok[tech]+0x14 · (f32)damage / (f32)EpuletekLeiro[t]+8)`, the tech's own price without the race discount (`mulss`, then `divss`); `MoneyMennyi < cost` → the same refusal line for the issuer; else `MoneyHozzaadLog(e, 0, −cost)` and six `EpuletDamage(p, k, −damage, false, 10000)` (the first repairs it all and shares the repair with its turrets, the other five pass 0). |

### 9.1 The ground war's orders
Sent by the colony screen for the selected units (`WF_Tank` flags bit 8) of a real-time ground war; `unit` is the
battlefield slot (`WF(p)+0x330`), `e` the sender. Port: `ig2_sim::ground::GroundOrder`, `War::order` (`ground/orders.rs`),
carried as `ColonyOrder::Ground { planet, order }`; outcomes for the screen are `GroundEvent::Order`. Exe handler 0x48a29a
(body of 0x48a270); the exe inlines `WT_WayPoint::Free/Add` and `WF_Tank::Stop`. No handler range-checks `p`, `unit` or
`t`, and the unit pointer is loaded before the alive test: the port refuses a unit slot that isn't alive.

| Type | Values | Sender (exe) | What the handler does |
|---|---|---|---|
| 0x22 | p, unit, x, y | `MoveSelected` 0x48be70 (click on the field; own units without flags 2/4) | unit flags & 0x80000000 = 0 and alive: waypoints freed (count `+0x22c`, cursor `+0x230` := 0, list `+0x234` freed), `MoveG(x, y)` (0x48bdc0, ground-combat.md §11) |
| 0x23 | p, unit, x, y | 0x45f300 (shift-click; e = each unit's owner) | alive: `WT_WayPoint::Add`: count + 1, `realloc`, the cell (x, y) as two i32; no limit, cursor untouched |
| 0x24 | p, unit, t | `WF::AttackWithSelected` 0x45f520 (t = the unit under the cursor or `−(slot)` a building) | `t < 0` or alive[t], and alive[unit]: waypoints freed, `SetTarget(t)` (0x49c670) |
| 0x25 | p, unit | `WF::StopSelected` 0x4a4d80 (button 0x12, its key) | alive: `Stop` inlined (unless flag 0x20000000: move state reset, flags `&= 0xfffff27f`, destination := cell, target := 10000); the waypoints stay |
| 0x12d | p, unit, big | 0x470cd0 (buttons 0xD/0xE: big = id − 0xD; units with weapon 0x40 resp. 0x80) | `WF_Tank::Detonate(big, WF, unit)` (exe 0x46dc40, ELF 0x4a0130), below; no alive test |
| 0x12f | p | `Colony_Main` / key R (only when no unit is still landing) | `LandingZone::Free` (zone 0xA34F50 + p·0xC0: the 200 × 200 levels and the outline `FUN_00477d40`); on the shown planet `ColonySpeaker.Play(1, e = EN ? 3 : 10)` (a picked line draws libc once); then the **retreat flag** of side e flips: `WF(AktPlanet)+0x89f + e − 1` [original bug: `AktPlanet`, the receiving machine's screen, not p; every machine shows p during a war (`StartGrWar`), the port uses p] |
| 0x130 | p, x, y, kind | `Colony_Main` button 0x14 after 0xB/0xC (kind 0 fighters: timer = 200, 1 big ships: 300) | `WF::ShotSpecWeapon(x, y, kind, e)` (0x49cb70, ground-combat.md §11.15), then the same charge timer `WF+0x7fc + 8(e − 1) + 4·kind` := 0 again; kind unchecked (the port refuses kind ∉ {0, 1}) |
| 0x134 | p, unit, v (1 byte) | 0x4a4e00 (button 0xF; units with weapon 0x10) | `+0x250` (mine laying) := v ≠ 0; the byte lands in a stack int: only its low byte counts |

`WF_Tank::Detonate(big, wf, i)`: `k = big + 1`. (1) `100k` particles, seven libc draws each (`%5, %0x1e, %0x32, %0x32,
%0x14, %0x14, %0x14`; render only). (2) On the shown planet (`AktPlanet` = unit `+0x280`, `AktKep` 3, no
`Atlepes`/`Belep`): a flash (one draw), 24 pieces of debris (`AddRepesz`: draws a, b, c; `vy = (a%0x3c/10 − 3)/1.2`,
`vx = (b%0x3c/10 − 3)/1.2`, `vh = (c%0x1e/10 − 4)/1.2`, f32). (3) One draw for `Blast07.wav + r%5` (played on the shown
planet when `WF+0x88c` = 0). (4) A scorch mark (`Koszfolt`, 80 × 80, list 0xA34FD4; render only) unless a turret.
(5) `RemoveTankGroup(i)`, `RemoveTarget(i)`. (6) Unless the unit had flag 0x8000 (a partner) at entry: every alive
unit without flags 2/0x40000000 and flags2 bit 2 at `d = (f32)sqrt((f64)(f32)(dx² + dy²))` cells (int cells) `≤ 6`
takes `WF_Tank::Damage(dmg, 10000)`, `dmg = (int)(f32)((f64)(80000k) − (f64)((f32)(80000k)·d) / 6.0)` (`mulss`, then
`divsd`/`subsd`, `cvtpd2ps`, `cvttss2si`); a dead group: not a partner and "on screen" a burst (one draw, 24 × 3
draws, `/1.5`), one sound draw, `RemoveTankGroup`, `RemoveTarget` (the loop re-reads the unit count). Then every
building slot `1..=HanyEpuletLefoglalva[p]` with kind ≠ 0 at `d ≤ w/2 + 6` (`w` the kind's width, building position
truncated) takes `WF::Damage(−slot, (int)(f32)((f64)(10000k) − (f64)((f32)(10000k)·d) / ((f64)(w/2) + 6.0)), 10000)`
and one sound draw. Stream 2 only inside the damage calls; the rest is libc.

[original bug] The victim burst's "on screen" test (exe 0x46f49d, `cmp eax, [ecx+0x280]`) reads the planet from the
detonator's record after `RemoveTankGroup` freed it: once the heap has handed the block to the debris allocated
meanwhile, the value is garbage and the burst (with its 73 draws and 24 objects) is skipped. The port reads the record
as left (`War::detonate`'s `freed_planet`); the tracer logs every value the exe read there and the replay feeds them
back (1 of 31 recorded detonations hit a reused block). In lockstep the burst is cosmetic but its libc draws aren't:
peers stay in step because they all run the port's rule.

`SetTarget` on an emptied building slot (a message sent before the building fell): the width is read for kind 0, out
of the table (`Building[−1]+0x58`, exe 0x8F1F14 = 10000): range `+0x14c` = 5000 + the weapon range
(`Rules::building_footprint0`, `GroundRules::kind0_width`).

Verified (`re/rules/traces/2026-10-22-groundwar-orders.md`): 407 messages staged through the exe's own handler in five
recordings of real-time battles, all exact (units, lists, retreat flags, timers, landing zone, voices, streams, libc).

`FRISSIT_Planet` (exe 0x474930, also its display refresh `FRISSIT_Planet(false, p)` that `FRISSIT_Epuletek` runs for
planets with repairs) recounts `HanyEpulet[p]` from the records (`= 0`, `+1` per non-empty slot); the port's refresh
returns the count (`colony::refresh`).

Verified (`re/rules/traces/2026-10-17-colony-orders.md`): 730 messages staged through the exe's own handler in three
save-replay recordings, byte-exact.

## 10. The building screen's input and its 2D parts (`ig2-client::colony_ui`)
Hot-spots (`Colony_MouseInit` ELF 0x4e0b80: `Mouse_Szoveg_Set(id, 0, x, y, w, h, tooltip GetString(3, 1, n))`;
enabled per frame by `Colony_MouseInit2` 0x4e1d90; the actions are `Colony_Main`'s, ELF 0x4dadf0 / exe 0x4666d0):

| Id | Rect | Enabled | Action |
|---|---|---|---|
| 1 | (179, 443) 31 × 31 | own planet not at war, a building selected, damaged, `+0x19` bit 4 clear | `ColonyRepairWindow` (drops the build cursor) |
| 2 | (212, 443) 31 × 31 | as 1 without the damage test (and not `TutorialDisableEpuletLerombolas`) | `ColonyDemolishWindow` |
| 4 | (0, 424) 166 × 56 | always | released with the right button: the starmap; with the left: the planet info mode |
| 5 | (522, 362) 100 × 100 | always | the camera glides to the radar point, `ColTargetX = (−(mx − 572)/50)·8192/2`, `ColTargetY = (−(my − 412)/50)·8192/2`, over 10 frames (`ColTFrame`), while held |
| 6 | (25, 39) 145 × 99 | own planet, not at war, not in info mode | the build menu's pick list (below), unless build orders are on their way (`NemSzabadEpiteni`) |
| 0x11 | (245, 443) 31 × 31 | as 2 | `Multi_Send(0x132, p, AktEpulet)` |
| 0x13 | (63, 148) 85 × 17 | | "Build": `colony_start_build` |
| 0x14 | (0, 20) 640 × 460 | always | the 3D view (below) |

The pick list (`ColonySelectEpuletMode`): pressing hot-spot 6 opens it at (mx + 12, my − 17) (x 163 when `x + 477 >
640`), drawn only after 300 ms (`SelectBuildingMikorVoltLenyomas`): `FadeKerettelKerekit(x − 6, y − 2, 4·120 + 3,
rows·13 + 18)` with fill (0, 10, 0) t 60 and edge (0, 5, 0) t 30; the headers `Epulet_Terulet_Nev[1..=4]`
(`StringReadAll`: the Hungarian txt17 §3, 35..=38) centred at x + 120(k − 1) + 54, y + 3, colour 6; the entries are the
techs 0x51..=0x90 the human owns (status 5; race 5 lacks 0x69, 0x81..=0x83), in column (t − 0x51)/16, one 13-pixel row
each from y + 17: the name at +8 (size 2, colour 3; 1 without a working colony hub, except 0x51; 6 under the pointer)
and the planet's count of that kind left of it (size 1, colour 0xc). While held, the entry under the pointer (x .. x +
114) is `colSelected` (only the hub's without a hub); hovering one drops the build cursor. Released under 301 ms
after the press: the build cursor toggles for the current kind; else the entry's building `Talalmanyok[t] + 0x27`
becomes `AktEpuletTipus` with the cursor up. Without a working hub (`SearchBuilding(p, 1, true)`) only the hub can be
picked.

The 3D view: without the build cursor the left button selects the building under the pointer (`GetNearestBuilding`'s
`selectedepulet + 1`, the model hit by the click; finished defence posts 0x20..=0x23 select their turrets and give 0)
and makes its kind `AktEpuletTipus`. With the cursor up (`ShowColony`): the pointer's ground cell `AktBGridX/Y`
(`Real2Grid` ELF 0x4717e0: `floor((S/2 − x)·99/S/100·99·2)`, `floor((y + S/2)·99/S/100·99·2)`) is searched for the
kind's first free spot (`LocateActBuildingSuitableLocation` exe 0x486460 / ELF 0x4bc820, over the colony screen's grid; differs from
the order's search: units at their own cell, no centroid; `rules/colony-placement.md` §8), and the half-transparent model `tempepulet` stands there
(`AktBGridXSarga/YSarga`), lit red when the spot is taken; a left click with a free spot (or the Build button) runs
`colony_start_build` (ELF 0x4dcbb0: `NemSzabadEpiteni + 1`, `Multi_Send(0x1f, p, AktEpuletTipus, x, y)`, the cursor
down, `CanModifyAktEpulet`); no spot → the line `GetString(3, 2, 2)` with the name; no hub → `GetString(3, 2, 1)`; the
right button drops the cursor.

The windows: `ShowRepairWindow`/`ShowDemolishWindow` (ELF 0x4bae90/0x4ba980) `FadeKerettelKerekit(222, 408, 6n, 56)` /
`(239, 420, 6n, 44)` with fill (0, 20, 30) t 60 and edge (0, 15, 25) t 30, n = strlen(title) + 2; the texts
(`WriteRepairWindow` 0x4baf50 / `WriteDemolishWindow` 0x4baa40): the title `GetString(3, 4, 4)` / `(3, 4, 1)` at
(x, 411) / (x, 423) in n cells, the repair's "The price: " `(3, 4, 5)` + `EpuletJavitasAr` at (x, 423), "Yes"/"No"
(`(3, 4, 2)`, `(3, 4, 3)`) centred at x + 3n, y 435 / 449, size 3, colour 2 under the pointer, else 1. They open on the
button's press and take their answer once the left button is up (`HandleRepairWindow`/`HandleDemolishWindow` in
`ColonyGfx2D_3`): row 1 = y 434..=446, row 2 = 448..=460, within x .. x + 6n; "Yes" sends 0x133 / 0x20 (the demolish
also drops `AktEpulet`), anything else shuts it: the answer is a press on the button, a drag to "Yes" and the
release.

Keys (`ColonyKeyboardHandler`): E = 0x132 for the selected building, Shift+Delete = 0x20 at once, PageUp/PageDown =
the previous/next planet the human owns and sees (`+0xd8 + 2·EN > 0`), I = the planet info mode.

The selection marker (`DrawBuildingSelection` ELF 0x482ca0): `epkij.bmp` on two flat triangles at the building's placed
height `+0x14` plus `allvanypoziciok[k − 1]` − 20 (exe 0x8F3878, 60 floats, `ig2-rules::scaffold_heights`), half sizes
`w·40·Building[k − 1].select + 2·selectionsize` and `h·40·… ` (exe `+0x68`, ELF `+0x6c`, `ig2-rules::BuildingModel::select`),
alpha `selectionatlatszosag`. `Frissit2_Colony` animates it per UI frame: a new selection starts at size 300, shrinking
×0.95 (alpha `(255 − (size − 3))/3`) until it reaches `sin(ColonyCounter/30)·3 + 3`, then it pulses with alpha
`−sin(ColonyCounter/30)·64 + 128`. (Each triangle draws a libc `rand()` into an unused counter: render-only.)

The short info (`ColonyGfx2D_2` → `StarmapRovidInfoKiir(50, 424)` with `StarmapMode` 1): `FRISSIT_Colony2` sets
`ColonyInfoMod` 1 with units selected, 2 with a building selected, else 0. 0 shows the planet's short info
(re/screens/starmap.md §11), 2 `ColonyKisInfo` (ELF 0x4edb40): the building's tech name at (50, 427), size 2, colour
0x22 (the mafia's name `MaffiaNev[tech − 0x51]` for a `+0x19` bit 4 building of techs 0x52..=0x90); "Damage :"
`(3, 2, 11)` at (50, 448), size 1, colour 0x23, with `DrawHpStripe(max − damage, max, 98, 448, 50)` (ELF 0x45f6a0: a
black 5-pixel bar, inside it `(w − 2)·v/max` pixels of colour 7 above half, 3 above a quarter, else 6); not working:
"Not working" `(3, 2, 14)` at y 439 and for a producer (`EpuletekLeiro[t]+1 ≠ 0`) "Product: nothing" `(3, 2, 15)` at
y 457, or under construction "Not completed" `(3, 2, 16)` and "Compl. :" `(3, 2, 17)` with the bar `(work − left/50,
work)` at (98, 457); working: "Working:   %" `(3, 2, 12)` without its '%' at y 439 and the efficiency `+0x1e·100/256`
with '%' right-aligned in 4 cells after the whole text (colour 6 below 100), and for a producer "Product:"
`(3, 2, 13)` and the icon byte (1 workers, 2 energy, 3 money) + `output · +0x18 · +0x1e / 256` (×
`GetBolygoGazdagsag(p)`/100 for class 3; kinds 6/7 show 5000/7000 with planet info −0x215c) at y 456, size 2.
`HandleKisInfoObject` (ELF 0x4ce840) also turns a small copy of the building's model beside it (not drawn by the
client).

The panel's turning building (`ShowColony`, `buildshown`): when `AktEpuletTipus` changes, the preview model
`Building[k − 1]+0x28` (`LoadMaxLevelBox`) is `AddToScene(…, mainxvb2, Building+0x54 · 3/2)` (1.5 at most below
`OptionsBuildingRes` 2), its `_A_`/`_B_`/`_C_` children as the buildings', `RotateObj(π/2, 0, 0)`,
`RotateObj2((1, 0, 0), −0.3)`, camera-fixed (`SetRelative(false)`) at (0, 0, −30000); `Frissit2_Colony` turns it 0.012
per UI frame about `(0, cos −0.3, sin −0.3)`; the second `Update3D` draws it in the viewport 18..173 × 36..167 centred
on (95, 101) (`ShowColony`'s `HatarX1..`, `xcenter`, `ycenter`). A unit being placed shows its hull instead
(`AktEpuletTipus = −10 − design`).

Deviations in the client: the building under the pointer is the one whose footprint holds the pointer's cell (the
original tests the model hit by the click); a left drag on the view pans the camera (the original scrolls at the
window's edges and box-selects units with it); the build cursor's red light is a tint.

Verified: `ig2-client`'s test `building_through_the_screen_reaches_the_sim` drives quick-1.sav's planet 42 through the
pick list, the cursor, a click (0x1f: the building stands where the cursor showed and is selected) and the demolish
window (0x20); the capture comparison is in `re/screens/diff-results.md` (`--colony-live`: the exe's state save is
written on the colony screen, so the camera is the live `ColonyXStart…` chunks; `--colony-ui pick|demolish|build`
renders those states).

## 11. The ground war's input and 2D parts (`ig2-client::colony_war`)
Evidence: the ELF decompiles of `Colony_MouseInit`/`_MouseInit2`/`Colony_Main`/`ColonyKeyboardHandler`/`ColonyGfx2D_0..4`,
`GetNearestBuilding`, `WarfareKeretesKijeloles`, the `WF::`/`WF_Tank::` selection, draw and order-sending functions and
`LandingZone::*` (`_local/decomp/colony-war/`), checked against the exe's (`Colony_Main` 0x4666d0, `Colony_MouseInit`
0x4680e0, `_MouseInit2` 0x467990, `ColonyKeyboardHandler` 0x4628c0, `ColonyGfx2D_1` 0x461190, `GetNearestBuilding`
0x47b350, `WF::CheckTanks` 0x460b70, `WF::MouseOver` 0x48bb00, `CheckPlacing` 0x4609d0, `GetSelNum` 0x47bb90 and the
senders of §9.1). Unit offsets are the exe's; EN = the local player.

**Selection** is unit flag 8 (`+0x188`), set and cleared only by the UI (`WF_Tank::SetSelection` ELF 0x47c9b0: skipped
for flag 0x40000000; a new selection restarts the ring's animation at size 300, alpha 0); no `MULTIGET_Colony` handler
and no sim pass reads it. `GetSelNum` = alive units with flags & 2 = 0 and flag 8. `LastSelectedTank` (exe 0x9964cc) =
the owner of the last unit selected. The port keeps the selection in the screen (a set of unit slots), not in the
sim's records: each machine selects for itself, as each exe does.

### 11.1 Hot-spots in a war
While the planet is at war (`+0x64` bit 0) the building buttons 1, 2, 0x11 and the pick list 6 are off; the war buttons
take the bottom row (`Colony_MouseInit` rects 31 × 31 at y 443; tooltip `GetString(3, 1, n)`; the brush is the
`ColonyBrushok` record on `COLONY_X.BMP`, frame k = the k-th cell of its row, 31 apart):

| Id | x | n | Enabled (`Colony_MouseInit2`) | Press (`Colony_Main`) | Brush, frame |
|---|---|---|---|---|---|
| 0xb | 344 | 8 Air Strike | `CheckFighterAttack(EN)` (exe 0x460770): owner ≠ EN and a unit of EN with weapon 0x20, flags & 2 = 0, `+0x288` > 0 | if the fighter timer `WF+0x7fc + 8(EN − 1)` = 200: `SpecWeaponMode` := 1 (0 when it was 1) | 20 (src 0, 31), frame `(mode = 1) + 1` |
| 0xc | 377 | 9 Laser Strike | `CheckBigShipAttack(EN)` (exe 0x45fc90): the same with weapon 8 | if the big-ship timer `+0x800 + 8(EN − 1)` = 300: mode := 2 (0 when it was 2) | 21 (0, 62), frame `(mode = 2) + 1` |
| 0xd | 245 | 10 Detonator | `CheckWeaponSelected(EN, 0x40)` (exe 0x460d00): a selected unit of EN with weapon 0x40 | `DetonateSelected(EN, 0)` (exe 0x470cd0): 0x12d (p, unit, 0) per such unit | 24 (0, 155), frame 1 |
| 0xe | 278 | 11 Devastator | the same with weapon 0x80 | 0x12d (p, unit, 1) | 25 (62, 0), frame 1 |
| 0xf | 212 | 12 Mine on/off | the same with weapon 0x10 | `CheckSelectedMine` (exe 0x460aa0) = 2·(a selected layer laying, `+0x250`) + (one not laying); 1 → 0x134 (p, unit, 1) per selected layer, 2 or 3 → 0x134 (…, 0) (`SwitchSelectedMineLayers` exe 0x4a4e00) | 26 (62, 31), frame = that value |
| 0x10 | 410 | 13 Retreat on/off | owner ≠ EN and at war | not while `TutorialPaused`; only when no alive unit is still to land (flag 0x8000000) and every one has `+0x278` bit 0: 0x12f (p) | 27 (62, 62), frame = EN's retreat flag (`WF+0x89f + EN − 1`) + 1 |
| 0x12 | 311 | 15 Cancel order | off without an own selected unit (`IsOwnTankSelected` exe 0x483c00); on while `AnyMovementInSelection` (exe 0x45f4b0: a selected own unit with flag 0x100 or 0x400); else left as it was | `StopSelected` (exe 0x4a4d80): 0x25 (p, unit) per selected unit of EN with flags & 6 = 0 | 30 (62, 124), frame 1 |

The war buttons act once, on the press, and each press plays `PlaySoundGlobal(0)`. Brushes 21, 24, 25, 26, 27 (and the
self-destruct's 19) and the retreat/landing texts are drawn only when `local_22` = not multiplayer, or the planet is the
MP war's and (owner = EN or side EN has units on the field, `WF+0x8b1 + EN`); in that MP case without units 0xb..=0x10
are off too. Brushes 20 and 30 have no such gate. The brushes are drawn only while their hot-spot is enabled.

Charge bars (`ColonyGfx2D_1`, while 0xb / 0xc is enabled): `FillToScreen(x, 427, 31, 5, 6)` and over it
`FillToScreen(x, 427, (int)((f64)timer·31.0/200.0), 5, 7)` at x 344 (fighters) and 377 (big ships: also /200.0, so its
bar runs past the cell above a charge of 200 [original quirk]); 6 and 7 are palette indices.

Hot-spot 4 released with the right button (the starmap) and PageUp/PageDown are blocked while
`GetWarfareStatus(planet) = 1` (the war runs on the shown planet).

### 11.2 Keys (`ColonyKeyboardHandler`)
| Key | Effect |
|---|---|
| Ctrl + 4..9 | `WF::SaveSelection(n − 4)`: each unit's byte `+0x13c + n` := its flag 8; the rings restart (`ForceSelectionAnim`) |
| 4..9 | `WF::LoadSelection(n − 4)`: flag 8 := the saved byte (no ring restart, `LastSelectedTank` untouched) |
| S | `StopSelected` (0x25 per selected own unit; no click sound) |
| a | the building selection goes; every alive unit with flags & 2 = 0, not 0x40000000, flags2 bit 2 clear (the exe's filter): selected when its owner is EN and it isn't a defence turret, else deselected; then the select voice |
| R | only with the debug switch (`DEBUGX` exe 0xBAE587); retreat is the button in normal play |
| E, Shift+Delete | building on/off, demolish: not at war |

### 11.3 The 3D view (hot-spot 0x14)
`ShowColony` each frame: `TankSelected` = `GetSelNum ≠ 0`, the grid cell under the pointer (`AktBGridX/Y`; the ray
missing the ground sets `InvalidBuildingPosition`), and `WarfareKeretesKijeloles` (the box). Then `Colony_Main`:

- **Hit test** (`WF::CheckTanks` exe 0x460b70): among the alive units with flags & 2 = 0 whose owner is EN, or all when
  side EN has units on the field (`WF+0x8b1 + EN`), those under the pointer (`WF_Tank::MouseOver` exe 0x48bc90 via
  `WF::MouseOver`'s test: not flag 0x20, not a defence turret of pods 0xa1..=0xa4; the object centre (`GetObjCenter` of
  the hull mesh, the gun mesh for a turret) projected; radius `TransformTav(35.0, z)` = 35·600/−z, at least 10.0; hit
  when `dx² + dy² < r²`, f32); of those the one whose projected centre is nearest to the pointer; −1 none.
- **Left press** (nothing armed, no build cursor): the box starts (`KeretesKijeloles`, `keretstart` = the pointer).
  `GetNearestBuilding` (exe 0x47b350): each unit's selection is first kept in `+0x146`; without Shift and outside a drag
  of `|dx| + |dy| ≥ 16`, everything but the unit under the pointer is deselected (`DeSelectAll`); the unit under the
  pointer is selected (any one without Shift; with Shift only an own one, when `LastSelectedTank = EN` or nothing is
  selected); nothing under it: the building rule of §10 (a defence post selects its turrets).
- **Box** (`WarfareKeretesKijeloles` exe 0x4a79e0, while the left button is held and `|dx| + |dy| > 15`): the building
  selection goes; `FadeKeret(start, size)` = four 1-pixel lines of (250, 80, 153) at t 128 (no fill); the unit under the
  start point stays selected; every unit whose projected centre is inside the rectangle (inclusive) is selected, every
  other deselected; then, unless exactly one is selected, the selected units of other sides are deselected; with Shift
  the kept selection (`+0x146`) is added back.
- **Left release**: the box ends; with the first selected unit EN's (not a mine) the select voice
  (`ColonySpeaker.Play(slot % 3 + 1, 1)`).
- **Right press** (the press frame only) with units selected; `t` = the unit under the pointer, `b` = the building
  under it; "the move" is `MoveSelectedG` (exe 0x48be70): 0x22 (p, unit, gx, gy) per selected unit of EN with flags & 6
  = 0 to the last cell under the pointer (also when the ray missed the ground), not with Ctrl or while `TutorialPaused`;
  the move voice (kind 0):
  - `t` not EN's (or EN's with Ctrl): no Alt → `AttackWithSelected(t)` (exe 0x45f520): 0x24 (p, unit, t) per selected
    unit of EN with flags & 6 = 0, the attack voice (kind 2); with Alt → the move.
  - else a building `b` and (owner ≠ EN or Ctrl): no Alt → `AttackWithSelected(−b)`; with Alt → the move.
  - else Shift: `AddWayPointSelectedGrid` (exe 0x45f300): 0x23 (p, unit, gx, gy) for every selected unit with flags & 2
    = 0, sent as that unit's owner (no owner filter [original quirk]).
  - else the move.
  With nothing selected the right press selects like the left one (without the box).
- **Special weapon** (`SpecWeaponMode` 1/2, exe 0x99649c; 0 on entering): selection and orders are skipped; the release
  of either button in the view (`Mouse_Felenged`) sends 0x130 (p, gx, gy, mode − 1) and disarms; the last cell is used
  even when the pointer is off the ground.
- **Landing** (owner ≠ EN while EN has units to place, `CheckPlacing(EN)` false): the left click places or picks up the
  unit at the cell (`UsedPosition`/`PlaceTank`/`UnplaceTank`), the right click places them all (`PlaceAllTanks`,
  ground-combat.md §11.16). These aren't `MULTIGET_Colony` messages [not ported yet].

### 11.4 Draw list
- **Rings** (`WF_Tank::DrawSelection` ELF 0x47df60, from `WF::WriteStuff`): every selected unit (mesh loaded, not flag
  0x20): `friend.bmp` for EN's, else `enemy.bmp`; a flat square at the unit's (x, y), 5 above its height (a turret's at
  it), half size `+0xec`, alpha `+0xf0`, two triangles. Animation (`WF_Tank::Refresh2` ELF 0x483400, per UI frame):
  `ph = ColonyCounter/30 + slot`, `s = sin(ph)·3 + 40`; zooming in from 300: size ×0.8, alpha
  `clamp((255 − (size − 45))/3, 0, 255)`, settled once size ≤ s; settled: size = s, alpha = `−sin(ph)·64 + 128`.
- **Targets** (`DrawTarget` ELF 0x47e5a0): a selected unit with flag 0x100 and a target ≠ 10000: on a unit `nyilak.bmp`
  at its position (5 up), on a building (`−slot`) `epuletkij.bmp` the size of the building's marker
  (`w·40·select + (size − 45)·2`); the same animation with `sin(ColonyCounter/30 − target)·3 + 50`; one marker per
  target per frame.
- **Route crosses** (`WF_Tank::Draw3d` ELF 0x481890): for each selected own unit each waiting waypoint (cursor ..
  count) two `line3d` (0x80be8882) crossing at the cell, 10 above the ground, half length `sin(gvalue/10)·3 + 6` along
  the map axes.
- **Group size** (`WF_Tank::WriteStuff` ELF 0x47dc30): a unit standing for more than one (`AliveTankNum > 1`) shows the
  number at its projected centre − (8, 8), size 2, colour 2; a retreating one (flag 4) a label below.
- **Texts** (`ColonyGfx2D_1`, owner ≠ EN, at war, `local_22`): EN's retreat flag on: `GetString(3, 2, 3)` at (220, 40);
  units still to land: `GetString(3, 2, 4)` at (220, 50); both `KiirStr(…, 70 cells, size 2, colour 2)`.
- Not specified in full or not ported yet: shadows (`DrawShadow`), the landing zone (`LandingZone::Draw`: 14 edge tiles
  `landz1..14` chosen by an 8-neighbour mask, `GenerateKorvonal` ELF 0x46acd0), the unit info panel (`ColonyKisInfo`,
  `ColonyInfoMod` 1), the self-destruct window (`ShowSelfDestruct`, hot-spots 8, 9, 10), the attack banner (`_2`), the
  radar's noise, the cursor shapes, the shots' and explosions' models and particles (§5), the voices. Each ring, target
  and shadow triangle draws a libc `rand()` into an unused counter (render only, as §10's marker).
- **The panel and the view in a war**: the planet panel (brush 12) and its faders only when the planet is EN's and not at
  war or EN still has units to land (`GFX_Colony`, `ColonyGfx2D_3`); the surplus line's two faders only with the line
  (EN's planet, not at war). The buildings and units show when EN owns the planet, has satellites there, or has units
  on the field (`WF+0x8b1 + EN`, as `GetStatus` sets it); else the red footprints of §3.3. A unit riding its dropship
  (`+0x278` bit 3) is drawn at its dropship's offset (`+0x27c..+0x284`, §4), its ring stays on the ground. The rings and
  target markers add their texture (black is clear).
- **Attack banner** (`ColonyGfx2D_2` exe 0x461900): while `TamadasSzovegCounter` (exe 0x9ABAE0; `StartGrWar` sets 900,
  `Frissit2_Colony` counts it down per UI frame on the colony screen, `Colony_Kilep` zeroes it) runs: EN's planet:
  `GetString(3, 2, 5)` with the attacking fleet's owner (`Flottak[WF+0x338]+0x15`, 1..=15) and the planet's name; else
  `GetString(3, 2, 6)` with the planet's name and its owner's (swapped in language 4); MP: `"%s (%s) vs. %s (%s)"` with
  the players' names; `KiirStr(…, 200, 30, 70, 2, 4, 2)` at `min(100, 2·counter)` % brightness.
- **Unit info** (`ColonyInfoMod` 1, in `StarmapRovidInfoKiir`'s place at (50, 424)): several units: `GetString(3, 2, 33)`
  ("%d units selected.") at (50, 430) size 1, the first one's empire at (50, 439) in colour owner + 0xf, its weapon's
  tech name (the pod, `+0x51` else `+0x52` of the design; a turret's own pod) at (50, 466); one unit: its design's name at
  (50, 427) size 2 in the owner's colour instead of the count. The exe also draws the unit's picture left of the lines
  (not specified yet).

### 11.5 Sounds
`PlaySoundGlobal(0)` on each war button press that acts (0xb, 0xc only with a full timer; 0x10 only once every unit has
landed and its dropship left; 0xd, 0xe, 0xf, 0x12 always) and when a press in the view changes the selected building;
none for the keys. Voices (`ColonySpeaker.Play(line, kind)`, ground-combat.md §11.12): select (left release, `a`)
kind 1, move kind 0, attack kind 2, detonate kind 4, line `slot % 3 + 1`. Recorded (`run_screen.py --grwar --script
tools/trace/scripts/grwar.json --sounds`, quick-1.sav planet 17): the right press on the ground → one `Speech::Play`
the next UI tick; the retreat presses before the dropships left, the `a`/S keys and the inactive buttons → nothing; the
ambient `Colony_Hangok` 43 and the ground-war track (`mfx/gwar3.ogg`) at entry. The client plays the button click
(`audio::CLICK`); the voices are played (ground war's voices, `audio.md` §8).

### 11.6 Verified
`ig2-client` tests `clicks_and_the_box_select`, `the_right_button_and_the_buttons_send_the_orders` (every order of
§9.1 through the screen's input), `rings_zoom_in_then_pulse`. Capture comparison `run_screendiff.py … grwar:17`
(`run_screen.py --grwar`: ig2.exe lands two of the human's vehicle designs on AI planet 17 with `AddTanksToPlanet` +
`StartGrWar`, `PlaceAllTanks(100, 100)`, sets the war bit and flag 8 on every human unit; the client renders the saved
moment with `--colony-ui war`): the attack banner, the retreat button, the unit info's lines and the selection rings
line up with the exe's; the rest is the colony renderer's known terrain shadow and building texture differences
(re/screens/diff-results.md).
