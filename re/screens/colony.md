# The colony screen: camera, terrain, buildings, units

How the original draws a planet's colony screen (`AktKep == 3`, planet `AktPlanet`). Evidence: the ELF oracle (`ig2`,
symbols) decompiled with `tools/ghidra/DecompileFunctions.java`; addresses are ELF unless marked exe. The exe was read
where offsets or values differ (camera globals, battlefield layout, tables) and the result was checked against
reference captures of the running exe (§8). `[inf]` marks inference.

Scope: the 3D view (camera, terrain object, buildings, roads, units, shots), what is hidden on planets the human
doesn't see into, and the HUD's layout. Text, the HUD's contents and input → commands are listed in §7 but not specified.

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
fog only.

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
The HUD's contents and texts, `ColonyKeyboardHandler`/`ColonyGestureHandler` and the build/selection input, the
shadow map (`rendershadowmap`), scenery objects (`LoadScapeObj`), weather particles, lightning, lens flare, building
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
