# The starmap screen: camera and drawing

How the original draws the starmap (screen `AktKep == 2`). Evidence: the ELF oracle (`ig2`, symbols) decompiled with
`tools/ghidra/DecompileFunctions.java`; addresses are ELF unless marked exe. The exe was checked where values matter
(palette, saved camera). `[inf]` marks inference.

Scope: the 3D view, systems, planets, fleets, routes, selection, background, colours, the minimap. The HUD panels
(`starpan`, `screen/star_*` brushes 1–3, 8–11), planet names (text), info panels and input → commands are listed in §9
but not specified.

## 1. Frame structure

- `Starmap_Belep` 0x6f1e10: brushes `AktBrushok = StarmapBrushok` (0x119a450, 12 records), loads keps 0x1c–0x1f, calls
  `Init3dStarmap`.
- `Init3dStarmap` 0x6db470: builds the 3D scene (`mainxvb`): lights (ambient 0x404040, parallel 0x808080 from
  (10, 10, −10)), one planet mesh + two cloud meshes per planet, the particle sets (§4.1, §6), the radar-range rings
  (`rrange.xvb`, option `OptionsKellRadarRange`), the production panel, the control pointer `kezel.xvb`; registers the
  2D passes `StarmapGfx2D_1` (0x6dcb30: names, routes, fleets), `StarmapGfx2D_2` (panels), `DrawStarmapParticles`.
- Per UI tick (100 Hz: `Fake_FRISSITES` runs it every 10 ms of `SDL_GetTicks`): `FRISSIT_Starmap2` 0x719070 (counters: `PlanetRotate += 1/150`, `futofeny += 1`,
  `smkeringeto += 0.0002`; cloud drift `GalaxisGomolygas` 0x6d8ac0), `Starmap_Main` 0x6f50a0 (input, camera tweens),
  then per drawn frame `GFX_Starmap` 0x6f36c0 (selection ring, `rotate_stars`) → `ShowStarmap` 0x6eca50 (camera set-up, planet spin,
  particle sizes) → the engine's `renderscene` 0x6034f0-area → 2D passes.
- Per sim tick: `FRISSIT_Starmap` (exe 0x594ae0) ends with `StarmapPlanetKering += IdoSpeedSzorzo / 1500`
  (ELF `FRISSIT_Starmap` 0x7132b0; planet orbits run on game time, so they stop when paused).

## 2. The camera

### 2.1 State
Saved globals (exe 0xB9A968.. in the save's chunks `StarmapXStart`, `StarmapYStart`, `StarmapRotate`,
`StarmapPitch`, `StarmapZoom`, `StarmapPlanetKering`; ELF 0x14321b0..cc):

| name | meaning |
|---|---|
| `StarmapXStart`, `StarmapYStart` | pivot: the galaxy point `(XStart, −YStart)` the camera looks at |
| `StarmapRotate` | yaw α, kept in [0, 2π) (`Starmap_Main` end, `StarmapKeyboardDoModifies` 0x705630) |
| `StarmapPitch` | pitch β, keyboard clamp [−1.560795, −0.2]; side view −0.95 (0xbf733333), top view −1.5608 (0xbfc7c821) |
| `StarmapZoom` | camera distance `D = 600 − Zoom`; keyboard/gesture clamp [2, 580] (planet zoom-ins go to 599) |
| `StarmapPlanetKering` | orbit clock (§3.3) |

`LoadSave` resets NaN or |value| > 100000 to 0 and clamps the zoom to 580. quick-1.sav: (43.68, 26.94), α −0.26
(unnormalised), β −0.95, Zoom 400, K 7.397.

### 2.2 Projection
`ShowStarmap` sets the engine camera each frame: `forgaspont` (pivot) = (0, XStart, −YStart), `kamerapont` =
(0, 0, 600 − Zoom), `elforgatas` = α, `+0x74` = β, `+0x78` = π/2 (0x3fc90fdb), `xcenter` = 320, `ycenter` = 250.
`createcameramatrix` 0x603480 → `CameraRotate2` 0x5fd340 → `CreateMatrix(α, β, γ)` 0x451ee0 (f32 sin/cos):

```
M = | cγ·cβ   −cγ·sβ·sα − sγ·cα   −cγ·sβ·cα + sγ·sα |
    | sγ·cβ   −sγ·sβ·sα + cγ·cα   −sγ·sβ·cα − cγ·sα |
    | sβ       cβ·sα                cβ·cα            |
```

Points are `(h, x, y)`: height (0 for everything on the map), galaxy x, galaxy y. A point `p` goes to camera space as
`e = M·(p − pivot) − (0, 0, D)` (`smRotatePoint` 0x6d8420 with `Kozeppont` = the transformed origin), is visible when
`e.z < 0` (2D overlays; particles need `e.z < zclip = −1.05·zbm = −0.42`, `zbm` = 0.4 from `ShowStarmap`), and lands on
the virtual screen at `(e.x·f + 320, e.y·f + 250)`, `f = fov / −e.z` (`TransformPoint` 0x5ee370). `fov = zoom·600`
(`calculateclipplanes` 0x6034f0) with `zoom` = 1.0 (`DDSetMode`): **fov = 600**. A world length `s` at depth `e.z` is
`s·600/−e.z` virtual pixels (`TransformTav` 0x5eeb20).

With γ = π/2 this is `X = −cα·dx + sα·dy`, `Y = −sβ·(sα·dx + cα·dy)`, `Z = cβ·(sα·dx + cα·dy) − D` (d = p − pivot):
**galaxy +x points left on screen at α = 0** and +y down (a mirror image of the galaxy coordinates). The minimap
confirms it (§8: screen x = −x).

### 2.3 Virtual screen
480 lines high, widened for the window's aspect: `ModifyViewPort` 0x4f6df0 sets `HatarX1 = −((W·480/H − 640)/2)`,
`HatarX2 = 639 + (W·480/H − 640)/2`, `HatarY1..2 = 0..479`. Virtual (vx, vy) → window pixels
`(W/2 + (vx − 320)·H/480, vy·H/480)`. 2D brushes and the minimap are drawn in the same virtual pixels.

### 2.4 Moves
- Tweens (`Mozgasd` 0x6e8750): every frame each of Zoom, XStart, YStart, Rotate, Pitch moves half-way to its
  `Target*` for `TargetFrame` = 8 frames.
- `StarmapSetSideView` 0x6fde80 (new game: `InitNewGame` copies the targets): over the planets the human sees
  (`+0xd8 + 2·EN` > 0) or owns, bounding box (min/max starting at ±300 inverted), pivot = box centre (YStart =
  −centre y), α = 0, β = −0.95, `Zoom = min(|w·600/320 − 600|, |h·600/240 − 600|) / 1.5` with `w`, `h` the half
  extents; also `StarmapMaxKiZoom`. `StarmapSetTopView` 0x6fdae0: same box (initial ±1000), β = −1.5608, ÷1.7, tweened.
- Keys (`StarmapKeyboardDoModifies`): zoom `Zoom ± (640 − Zoom)/50` per step, clamp [2, 580]; rotate ±0.02 per
  frame; pitch ±0.00628318 per frame, clamp [−1.560795, −0.2].
- Pan (`Starmap_Move0` 0x6fe570): `XStart −= (cα·mx − sα·my)·(600 − Zoom)/600·0.85`, `YStart −= (cα·my + sα·mx)·
  (600 − Zoom)/600·0.85`; a move that would leave [−300, 300]² is refused (`Starmap_Move` 0x6fe780 retries with
  1 unit less until it fits). So the pivot never leaves the ±300 square.
- Zoom to a planet: `StarmapBeZoomol` 0x6fe450 (saves `Last*`, targets the point at the given zoom),
  `StarmapKiZoomol` 0x6f3610 back; `StarmapKozepreAllit` 0x6fe220 centres. In planet-info mode `GFX_Starmap` pins
  Zoom = 599 and follows the orbiting planet.

## 3. Systems and planets

### 3.1 Per-planet look (`.scn` `PLANET` keys, planet record)
| offset | key | use |
|---|---|---|
| +0x18/+0x1c | `px`/`py` | the system's galaxy position (the star) |
| +0x22 | `pFelszin` | surface 1..; mesh `b{(s−1)%8}0.xvb`, star texture `n{(s−1)%8+1}` |
| +0x17c | `szin` | star colour `0xTTBBGGRR` (TT = transparency, see §7) |
| +0x180 | `bolygomeret` | planet size |
| +0x181 | `csillagmeret` | star size |
| +0x182 | `SpinSpeed` | `spin = v/100` |
| +0x183 | `Yaw` | `v/50 − 1` (stored, no starmap reader found) |
| +0x184 | `Dist` | orbit radius `v/20 + 2` |
| +0x185 | `Speed` | orbit period `v/100 + 0.1` |

(`Init3dStarmap` 0x6db470 → `PlanetProperties[p−1]` +4/+8/+0xc/+0x10, `StarSize[p−1] = csillagmeret >> 1`.)

### 3.2 The star
One particle per planet in `DiffStars[(s−1)%8]` (texture `textures/starmap/n1..n8`, additive), at (0, px, py), colour
`szin`, size `StarSize` (`rotate_stars` 0x6e1a00 sets size `StarSize·starmapszorzo` (= 1) when the human sees the
planet, else 0). A textured particle is a screen-aligned quad of half-size `size·fov/−e.z` around the projection
(`Particle` 0x5f4ff0).

### 3.3 The planet
Mesh `PlanetMeshes[(s−1)%8]` scaled `(bolygomeret/200000 + 0.002)/8` (`AddToScene` 0x4551d0), placed by
`rotate_stars` on its orbit around the star:

```
K = StarmapPlanetKering, r = Dist/20 + 2, T = Speed/100 + 0.1
planet = (0, px − sin(K/T)·r, py + cos(K/T)·r)
```

Two cloud meshes (`felhoo_0.xvb`, 50 % transparent) share the position. `ShowStarmap` spins them every frame:
planet `RotateObj(0, 0, PlanetRotate·spin·6)`, clouds `(−PlanetRotate·spin·2, 5, 0)` and `(PlanetRotate·spin·3, 0, 0)`.
World radius `[inf]`: the meshes are unit spheres with a ×1.1327 child scale and the scene's mesh unit appears to be
1000 world units (the radar ring `rrange.xvb`, a unit circle, is scaled `range·0.001` to show a radar range), so
`r_planet ≈ 1.1327·(bolygomeret/1600 + 0.25)`: a fraction of a unit, visible only when zoomed in. The planet texture
is the surface map (`p<surface>`; the client already uses it).

### 3.4 Which planets are drawn
Only planets the human sees: `Bolygok[p] + 0xd8 + 2·EN` (i16) **> 0** (or the debug switch `DEBUGX`); others are
hidden (`rotate_stars`, `Show_Stars2d3d` 0x6e22e0) and planets beyond `HanyBolygo` too.

**(−300, −300)**: quick.scn (and so quick-1.sav) holds the home worlds of all eight races; the six whose races aren't
in play (`pFaj` 1, 2, 4, 5, 7, 8; human 6, AI 3) are parked at (−300, −300), the corner of the pan square (§2.4), with
`pFelderites −1` (visibility −1 for everyone), so they are never drawn. The traders' exit point is the same corner.

### 3.5 Names and markers (2D, `Show_Stars2d3d`)
For each drawn planet whose star projects in front (`e.z < 0`) and that is owned (or story `StoryAdat+0x4c`, tutorial,
or `+0xba` set): the name centred at `(sx, sy + ⌊√(Zoom/5)⌋)` (`KiirStr`, colour `DDColor[owner + 15]`, blinking with
`AktIdo300Szazalek` when `+0xba`; faded out over Zoom 590..599.1). A planet with `+0xb8 ≠ 0` gets brush 4
(`kijelol` frames 1–4, `(AktIdo/200) % 4`) at `(sx − 13, sy − 13)`. Wormholes known to the human: two arrows between
their ends (`DrawArrow`, colour from the record, alpha 0x40).

## 4. Fleets

### 4.1 Which fleets
`Show_Flottak2d` 0x6e3d30: owner ≠ 0xFF, `+0x52` bit 10 clear, and (`+0x19 + EN` set and `+0x18` bit 0 clear, or
`FlottaTipus::fMindigLathato`, or `DEBUGX`).

### 4.2 Sprites (2D brushes)
The fleet's position (0, x, y) is projected (§2.2); when in front:
- default: brush 5 = `screen/star_pl` (kep 0x1c, 510×1056): cells 30×22, 17 columns × 48 rows. Frame
  `owner + row·17` → column `owner − 1` (one ship per empire colour), row `(⌊futofeny/5⌋ + slot·5) % 48` (a slowly
  turning ship; `futofeny` +1 per UI frame). Bit 14 of `+0x52` → column 16 (the special sprite). Top-left at
  `(sx − 14, sy − 10)`.
- `+0x52` bit 5: brush 7 = `screen/star_pl2` (kep 0x1d, 240×528): cells 15×11, 16 columns, frame `owner + row·16`,
  top-left `(sx − 7, sy − 5)`.
- Fleet names (`+0x52` bit 11) under it at `(sx + 1, sy + 9)`, `DDColor[owner + 15]` with a black shadow.
- Attack/arrival warnings (`+0x52` bits 2 or 7): brush 6 (`kijelol`) at `(sx − 15, sy − 13)`.

Brush records (`StarmapBrushok`, 0x38 bytes: x0, y0, ?, ?, w, h, step x, step y, columns, kep): 1–3 and 11 kep 97,
4/6 kep 0x1e (`KIJELOL.BMP`, 33×27 step 34, 4 columns), 5 kep 0x1c (`STAR_PL.BMP`), 7 kep 0x1d (`STAR_PL2.BMP`),
8 kep 34, 9 kep 33, 10 kep 1. Kep names are in `Kepek` (0x1171ea0, 0x130 bytes per kep, name at +0).

Neither binary names `flotta.bmp` (nor does any starmap model): `textures/starmap/flotta` looks unused. The fleet
transfer panel (`ShowStarmap`'s transfer mode) shows the designs' own models from `vector/talalmanyok|spacewar|colony`.
`starpan` is the texture of the control-pointer model `kezel.xvb` (its material), the HUD knob of §1.

### 4.3 Routes (`Show_Flottak3d` 0x6e3640)
For the human's fleets (and debug views) with an order (`+0x40` > 0) and `+0x52` bit 5 clear: an arrow from the fleet
to its target (`+0x42`: 0 = point `+0x44/+0x48`, 1 = planet `(int)+0x44`'s star, 2 = fleet `(int)+0x44`), colour
`0x800000ff` (red, half transparent) when the order is 2 (attack), else `0x805f4b46`; `DrawArrow` 0x6e2930 projects both
ends, draws the `nyilszar` texture as a strip 2·1.5 virtual pixels wide and a 10-pixel head. The selected own fleet
(`AktFleet`, `StarmapMode == 2`) also gets its waypoint chain (`Waypointek` +0/+4, next +8), blinking `0x80be968c`
every 15th segment.

## 5. Selection (`GFX_Starmap`, `rotate_stars`, `ShowStarmap`)
40 `select.bmp` particles (`Selection`, additive) on a circle of radius `10/√(600/D)` world units around the selected
planet's star (`AktPlanet` in `StarmapMode` 1) — `10·√(600/D)` virtual pixels. Every third frame a light runs one
step: particles `h` and `h + 20` (h = `futofeny2 % 40`) get size 2.0, the others lose 0.2 down to ≈ 0.4; sizes are
divided by `√(0.3·600/D)`. Colour `0xff8020` (blue) for a planet, `0x00ffff` (yellow) in fleet mode, faded by the
planet-info zoom.

## 6. Background (`GenerateStarParticles` 0x6d8e90, once, `srand(1)`, `random(n) = n > 0 ? rand() % n : 0`)
All in (h, x, y); the exe's `rand` is MSVC's.
- `FarStar` (2500 untextured points, colour `v·0x010101`, v = `random(100)`): 0..1749 a two-armed spiral (arm =
  `random(2)`, offset along the arm `random(600) − 300 + random(50) − 25`, height `(random(30) − 15)·(1 − r/√180000 +
  0.3)`, rotated by `r/100 + (random(r)/100 − r/200)/2.5` with r = |(x, y)|·… as in the code); 1500..2499 then
  overwritten by a flat disc (radius `random(600) − 300`, angle `random(400)/400·2π`), 1750..2499 by a wide ring
  (radius `random(1800) − 900`, height `random(600) − 300`), 2000..2499 by a shell (`random(100) − 50` per axis,
  normalised, × `900 + random(500)`).
- `Clouds1` (`felho3`, 105, size 5 + `random(6)`) and `Clouds2` (`felho2`, 105, size 12 + `random(20)`): spiral
  positions like the stars, colour `0xc8BBGGRR` with R = `random(50) + 50`, G = `random(50) + 50`,
  B = `random(100) + 50 + random(50) + 55` (transparency 0xc8 → ×55/256). Then 5 spiral points recoloured with
  `AddColor` (unmapped). Drawn up to `HANYFELHOPARTICLE = OptionsFogResolution·50`; they drift (`GalaxisGomolygas`).
- `KodPamacsok` (`felho2`, 30 clumps × 10): clump centre `(random(900) − 450)·1.5` in x and y, height
  `±random(300)·1.5`; members ±50, size 25 + `random(25)`, colour like the clouds with per-clump bases.
Particles are drawn additively (blend 1, 1; `+0x20` = 100 set by `ShowStarmap`); untextured ones as points.

## 7. Colours
`DDColor` (exe 0x8F8710, ELF 0x1166cd0): 43 × `0x00BBGGRR` (red in the low byte: GL vertex colours). Empire `e`'s
colour is `DDColor[e + 15]` (names, minimap, fleets' name text); unowned planets on the minimap use `DDColor[12]`.
The 17 `star_pl` columns are drawn in the same colours (column e − 1). Particle colours `0xTTBBGGRR` multiply RGB by
`(255 − TT)/256` (`Particle`). Arrow/vertex colours are `0xAABBGGRR`.

## 8. Minimap (`GFX_StarmapRadar` 0x70e490)
In the panel at virtual (274..366, 388..453): planets the human sees as 2×2 dots at
`(−x·92/600 + 320, y·65/600 + 420)` (exactly `((−x/100/3)·92)/2 + 46 + 274`, `((y/100/3)·65)/2 + 32 + 388`),
`DDColor[owner + 15]` (unowned `DDColor[12]`), the selected one blinking; visible fleets as 2×1 dots; the view's
footprint as four lines between `get_radarpos` of the virtual corners (−320, −220), (320, −220), (−320, 240),
(320, 240).

## 9. Not specified here
The HUD panels (`kezel.xvb` with `starpan`, `screen/star_ico`, `star_w`, `kijelol` uses beyond §3.5/§4.2), `StarmapGfx2D_2..4`
(info, production, transfer, sell panels), planet/fleet info text, `StarmapKeyboardHandler`/`StarmapGestureHandler`
input → commands, the production view, radar-range rings, wormhole particles, `AddColor`, cloud drift.
