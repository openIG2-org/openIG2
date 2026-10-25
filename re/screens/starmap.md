# The starmap screen: camera and drawing

How the original draws the starmap (screen `AktKep == 2`). Evidence: the ELF oracle (`ig2`, symbols) decompiled with
`tools/ghidra/DecompileFunctions.java`; addresses are ELF unless marked exe. The exe was checked where values matter
(palette, saved camera). `[inf]` marks inference.

Scope: the 3D view, systems, planets, fleets, routes, selection, background, colours, the minimap, the orders the
screen sends and the lockstep handler that carries them out (§10), the control panel and the short info (§11), the
production view (§12), the transfer panel (§13), the radial icon menu (§14), the planet's long info (§15), the select
menus (§16) and the production view's planet list (§17). The names (§3.5, §4.2) and the top bar are in re/screens/hud.md
§2, §4; what is left is in §9.

## 1. Frame structure

- `Starmap_Belep` 0x6f1e10: brushes `AktBrushok = StarmapBrushok` (0x119a450, 12 records), loads keps 0x1c–0x1f, calls
  `Init3dStarmap`.
- `Init3dStarmap` 0x6db470: builds the 3D scene (`mainxvb`): lights (ambient 0x404040, parallel 0x808080 from
  (10, 10, −10)), one planet mesh + two cloud meshes per planet, the particle sets (§4.1, §6), the radar-range rings
  (`rrange.xvb`, option `OptionsKellRadarRange`, §6.3), the sky sphere `gomb.xvb` (§6.5), the production panel, the
  control pointer `kezel.xvb`; registers the
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
- Operand widths (exe asm; all five camera values are f32): zoom is f32 throughout (`(640 − z) / 10 / 5`, two
  `divss`, exe 0x5ae8c9); rotate adds the f64 0.02, rounds, below 0 adds the f64 2π, rounds, then `fmod` by f32 2π as
  f64 (0x5ae9ec); pitch adds the f64 0.00628318 and rounds, the clamps compare in f64 and store f32 −0.2 / 0xbfc7c821
  (0x5aea5f); the pan (`Starmap_Move` exe 0x5bb230, `Move0` inlined) rounds sin/cos to f32, turns and scales in f32
  (`mulss`, `/ 600`), then `XStart − v·0.85` in f64 (an f32-only pan is off by an ulp now and then); the minimap pivot
  divides in f32 and does `(v − 0.5)·−600` in f64 (0x5b2ec9). Bit-exact against runtime probes
  (`probes/starmap_cam.js`, `scripts/starmap-cam.json`, quick-1.sav). `Starmap_Autoscroll`'s normalisation divides in
  f32 in the exe (f64 in the ELF); the integer results are the same (30, 21, 24).
- `StarmapSetSideView` with no planet known or owned (exe 0x5b0c60): Zoom, Rotate and Pitch targets are written,
  `TargetXStart/YStart` and `StarmapMaxKiZoom` are not (stale); `StarmapSetTopView` (0x5b0e00) sets only rotate and
  pitch and starts no move. Unreachable: a planet the human has seen stays known (`+0xd8 + 2EN`), the home planet from
  the start. The box's filter (both views, exe 0x5b0cc0 / 0x5b0e50, planets 1..count): `+0xd8 + 2EN > 0` or
  `+0x20 == EN`, no `DEBUGX` term.
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
One particle per planet in `DiffStars[(s−1)%8]` (texture `textures/starmap/n1..n8`, additive), at (0, px, py), white,
size `StarSize` pulsing (§6.4; `rotate_stars` 0x6e1a00 first sets `StarSize·starmapszorzo` (= 1) when the human sees
the planet, else 0). A textured particle is a screen-aligned quad of half-size `size·fov/−e.z` around the projection
(`Particle` 0x5f4ff0).

### 3.3 The planet
Mesh `PlanetMeshes[(s−1)%8]` = `b{k}0.xvb` scaled `(bolygomeret/200000 + 0.002)/8` (`AddToScene` 0x4551d0; exe
`Init3dStarmap` 0x59c850 is the same), placed by `rotate_stars` on its orbit around the star:

```
K = StarmapPlanetKering, r = Dist/20 + 2, T = Speed/100 + 0.1
planet = (0, px − sin(K/T)·r, py + cos(K/T)·r)
```

`ShowStarmap` turns it every frame by `RotateObj(0, 0, PlanetRotate·spin·6)` (spin = `SpinSpeed/100`;
`PlanetRotate` exe 0xE85028 steps 1/150 per UI tick with `futofeny`, so it is `futofeny/150`: 1.58667 at
`futofeny` 238 in a capture; the exe's `GetObjMatrix` gives exactly `CreateMatrix(0, 0, PlanetRotate·spin·6)`).
- **Level of detail.** `b{k}0` is a 4-face tetrahedron whose node names `b{k}1` as `lod_archive`, which names
  `b{k}2`, then `b{k}3` (48, 168, 528 faces, radius 1 × child scale 0.846). `Kocka` (ELF 0x5f6a40) steps down the
  chain while `valtashatar·30^level < size·lod²` (`lod` 5 in each): the long info's planet is at `b{k}3`. On screen
  its radius is `1024·0.846·scale` world units (Magellan 10, size 7: 0.218 at distance ≈ 1, 163 pixels at 800 × 600,
  as in the exe).
- **Texture.** The material's `p{k+1}.bmp` (= `p<surface>` for surfaces 1..8) holds the map twice, one above the
  other; the remaster's PVR carries a metadata block `SGLB`/`CNST` with `uv_mul = (1, 0.5, 0, 0)` and
  `render/ig2_vertex.shader` multiplies the mesh UVs by it (`var_uv = sg3d_texcoord0 · uv_mul.xy`), so the sphere
  shows one copy (`ig2_formats::pvr::Pvr::constant`).
- **Light.** `Init3dStarmap` (ELF 0x6db470, exe 0x59c850) puts an ambient light 0x404040
  and a parallel one 0x808080 along (10, 10, −10) (normalised, the world's `MoveObj` axes) on the scene root
  `mainmeshid`; every planet and cloud inherits them (`Kocka`). `ShowStarmap`'s `StmapLightX/Y/Z` → `fxn/fyn/fzn` (exe
  0x5a4464..0x5a448c) are read only by `setlightnormal`, which nothing calls. `fenyekszamolasa` (research.md §2.1)
  lights in the object's space: `Kocka` turns `L` by the camera, `fenyekszamolasa` back by the
  node's chain (camera · `RotateObj`), so `Lo = Rᵀ·L` with `R` the object's own `RotateObj` (the planet's `(0, 0,
  PlanetRotate·spin·6)`, the clouds' `(−PlanetRotate·spin·2, 5, 0)`): the lit side is fixed in the galaxy and differs
  between the planet and its clouds. Per vertex `0x40 + 0x80·shade[|n' − Lo|²·2047] >> 8` with `n'` the file normal
  (mirrored in x and y as stored, z negated); both have property 8 (planet flags 0xf, `CloudId` 0xb), so the bytes
  aren't scaled by the opacity [obs: `--light-probe`, `light-planetinfo.jsonl`: every vertex exact with the exe's own
  `Mᵀ·L`, within one byte from `Rᵀ·L`]. `fenyekszamolasa` caches the colours in the shared XVO and recomputes only for
  the first drawn object of a mesh per frame (`xvo+0xbc`), so same-surface planets would share one shading [inf, not
  run].
- **Order.** The remaster draws the starmap's meshes without the depth test (`render/video.state`) in the old
  engine's sorted order, the particle pass (`DrawStarmapParticles`, a `SetRenderOther` callback) before the planets:
  the background's additive clouds don't show through a planet.
- **Clouds** (`HideClouds` exe 0x59c600: only the long info's planet `AktPlanet` keeps them). `felhoo_0.xvb` (a LOD
  chain to `felhoo_3`, 528 faces, radius ≈ 1.0 × (0.855, 0.854, 0.856)) at `(scale + 0.0001)/8`, turned by
  `RotateObj(−PlanetRotate·spin·2, 5, 0)`; a second copy at scale 0 (`CloudId2`, never visible). Its texture
  `ethcldbp.bmp` only picks the remaster's `CLOUD` material (`command_glLoadTexture` ELF 0x426e30;
  `render/ig2.material`, `ig2_cloud.program`): `render/ig2_fragment_cloud.shader` draws procedural clouds,
  `cloud = smoothstep(0, 0.4, Σ₄ (noise(uv') − 0.4)·½ᵏ · edge fades)` with `uv' = (u, v/2)·0.5 + sg3d_time.x·0.01`
  scaled 3.3 per octave, colour `min(2·cloud, 1)` × the vertex light, alpha `cloud`, alpha-blended
  (`SetObjTransparency(0.5)` is ignored). `sg3d_time.x = game::acttime/1000` (`ig2screen::render` ELF 0x423b80; exe
  0x7370f0, `acttime` exe 0xFD1520 in ms). The noise is `sg3d::create_plasma(7, 5)` (ELF 0x87dbb0, made once by
  `ig2screen::initialise`): `create_noise_optim(5)` fills 32 × 32 bytes from sg3d's own generator (`srand` ELF
  0x7d2a40: Park–Miller `seed·16807 mod 2³¹ − 1`, `rand_seed` ELF 0x119dfb0 = 1 [inf: nothing draws from it
  earlier]), then each of 128 × 128 texels sums 3 octaves of `(2·s − 1)·amp` (amp ½, ¼, ⅛, uv doubling) of
  `sample_optim` (ELF 0x870920: smoothstep-weighted bilinear, wrapping), stored as `(int)(sum·127.5 + 127.5)`, all
  f32; sampled with mipmaps and repeat.

### 3.4 Which planets are drawn
Only planets the human sees: `Bolygok[p] + 0xd8 + 2·EN` (i16) **> 0** (or the debug switch `DEBUGX`); others are
hidden (`rotate_stars`, `Show_Stars2d3d` 0x6e22e0) and planets beyond `HanyBolygo` too. A campaign starts with dozens at −1
(never seen) and most others at 0 [obs: the exe's start saves, `newgame-c1-d1-s5150`: 31 at −1, 84 at 0, 3 above].
The side view's box also takes the human's own planets (§2.4).

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
(`AktFleet`, `StarmapMode == 2`, `KellSelect`; also story fleet 0x1f88's and the DEBUGX views') also gets its waypoint
chain from `+0x50` (`Waypointek` +0/+4, next +8): one arrow a segment, the first from the order's target, each
`0x805f4b46` except segment `i` (0 first) with `(waypointvillogo − i) % 15 == 0`, `0x80be968c`; `waypointvillogo`
counts every second call (`waypointvillogok`), so the lit segment walks along the chain.

## 5. Selection (`GFX_Starmap`, `rotate_stars`, `ShowStarmap`)
40 `select.bmp` particles (`Selection`, additive) on a circle of radius `10/√(600/D)` world units around the selected
planet's star (`AktPlanet` in `StarmapMode` 1) — `10·√(600/D)` virtual pixels. Every third frame a light runs one
step: particles `h` and `h + 20` (h = `futofeny2 % 40`) get size 2.0, the others lose 0.2 down to ≈ 0.4; sizes are
divided by `√(0.3·600/D)`. Colour `0xff8020` (blue) for a planet, `0x00ffff` (yellow) in fleet mode, faded by the
planet-info zoom.

## 6. Background
Particle records: `FStars` exe 0xF16F38, 11 sets × 0x38 = {+0 records (0x4c bytes: +0 h, +4 x, +8 y, +0xc colour
`0xTTBBGGRR`, +0x14 size, +0x48 colour backup), +4 texture, +0x10 count}; `DiffStars` 0xF171A0, 8 sets (read live
with `run_screen.py --particles`; §6.1 and §6.3 give all 3,010 background particles and quick-1.sav's 52 radar dots
bit for bit). Sets: 0 `FarStar` (no texture: points), 2
`RadarRange` (`radar.bmp`), 3 `Selection`, 6 `FlottaMozgPart`, 7 the wormholes, 8 `Clouds1` (`felho3.bmp`), 9
`Clouds2` and 10 `KodPamacsok` (`felho2.bmp`). `Particle` (ELF 0x5f4ff0) draws a textured particle as a
screen-aligned quad of half size `size·fov/−e.z` with colour × `(255 − TT)/256` and blend (1, 1) (alpha blending when
the set's `+0x20` is 99), skipping colour 0 and size ≤ 0; untextured sets as GL points. The whole pass is gamma-space
GL (re/screens/hud.md §1.1).

### 6.1 Generation (`GenerateStarParticles` exe 0x599cf0, ELF 0x6d8e90; once, `srand(1)`)
`random(n) = n > 0 ? rand() % n : 0` with MSVC's `rand`. Positions are (h, x, y). The exe rounds the square roots and
the sines and cosines to f32 and mixes f32 and f64 steps where the ELF works in f64 (asm).
- `FarStar` (2500, colour `v·0x010101`, v = `random(100)`, overwritten by the twinkle, §6.4): 0..1749 a two-armed
  spiral: arm = `random(2)`, height `h0 = random(30) − 15`, along the arm `random(600) − 300 + random(50) − 25` (for
  both axes, one of them × 0), the colour draw, `r = √(x² + y²)` (f32), `turn0 = r/100` (f32), height
  `((1 − r/√180000) (f32) + 0.3)·h0` (f64), `turn = (random(⌊turn0·100⌋)/100 − turn0·0.5)/2.5 + turn0` (f64), turned by
  `turn` with f32 sine and cosine; 1500..2499 then overwritten by a flat disc (radius `random(600) − 300`, angle
  `random(400)/400·π·2` in f64 with π exact), 1750..2499 by a wide ring (radius `random(1800) − 900`, height
  `random(600) − 300`), 2000..2499 by a shell (`random(100) − 50` per axis, normalised, × `900 + random(500)`).
- `Clouds1` (105, size 5 + `random(6)`) and `Clouds2` (105, size 12 + `random(20)`): spiral points with a tint
  `random(50) + 50` drawn after the position, colour `0xc8BBGGRR`, B = `random(100) + tint + 0x37`, G =
  `random(50) + 50`, R = `random(50) + 50`.
- `AddColor` (exe 0x590f60, ELF 0x6d8650): the first five `Clouds2` move to five new spiral points; each point adds
  `(R, G, B)·(1 − d/√720900)^25` to every cloud of both sets (d its distance; f32 with the root and the power in f64;
  channels capped at 255). The exe passes the three `random(50)` right to left: its first draw is blue (the ELF's is
  red).
- `KodPamacsok` (30 clumps × 10, `felho2`, size 25 + `random(25)`): clump centre h = `(random(900) − 450)·1.5`, x =
  `(random(900) − 450)·1.5`, y = `±random(300)·1.5`, bases R, G, B = `random(100) + 50`; members ±50 per axis, colour
  base + `random(50)` (B, G, R order). **Only 30 of the 300 are lit**: both binaries copy the colour to the backup
  `+0x48` of the clump's index instead of the member's, and `Init3dStarmap` then restores every colour from the
  backups, so members 30..299 are black.

### 6.2 Clouds drawn and drifting
`Init3dStarmap` gives both cloud sets the count `HANYFELHOPARTICLE = OptionsFogResolution·50` (100 with the option at
2; `%LOCALAPPDATA%/GyroscopeGames/ig2/options.dat`, key/value strings, `ig2_formats::options`), so their last 5 are
never drawn. `GalaxisGomolygas` (exe 0x5999c0, ELF 0x6d8ac0) runs every starmap UI tick (`FRISSIT_Starmap2`):
`GomolygasCounter` (exe 0xE75918) + 1, and on multiples of 5 the first `HANYFELHOPARTICLE` clouds move by
`cos|sin(counter/(i + k) + i)/40` (`Clouds1`: h cos 319, x sin 210, y sin 250; size cos 400 /70) and `/9` (`Clouds2`:
h sin 279, x cos 210, y sin 150; size sin 220 /20), in f64 written back to f32. The counter isn't saved.

### 6.3 Radar ranges (`OptionsKellRadarRange`, on)
`RadarRangeKi` (ELF 0x70e030) first runs `GetENRadarKifejlesztve`, then collects circles (`RadarKorok`, ≤ 400): for
each empire allied with the human (`Viszony` level 5, the human included, 1..`ProgramPlayers2`), each planet with
radar level `+0x6e` 1 or ≥ 3 that this empire owns, or of level ≥ 3 whoever owns it (once per ally), radius
`RadarHatotav2[owner][level]`; then the human's fleets with `+0x52` bit 5 clear and the story's tracked fleet
(`StoryAdat+0x28`, `+0x18` bit 0 clear) with radar `+0x7c` > 0. A loaded save doesn't carry `RadarHatotav2`; the
exe's rows hold `RadarHatotav2Init` (radius 30 at level 3 in quick-1.sav): `Game::radar_tables`.
`RadarRangeReszKi` (0x70dc80) puts `8n` dots on each circle (n = 2 below radius 35, 3 below 70, else 4; angles
`π/(8n) + k·π/(4n)` mirrored 8 ways, offsets `(f32)cos·r`; ≤ 4000 dots). `CopyRadarParticles` (0x70caa0) then takes
each live circle c against every other live o that touches it: when c is no bigger than o or o pokes out of c, the
dots of o inside c (`d² ≤ r_c² − 1`) go; else c swallows o (its dots and its disc go); two planets' equal circles
holding each other's centre add an arc of c, and arcs covering c all round remove c (joined from a "first" arc the
original picks by comparing the last arc's start each time, kept). The surviving dots (≤ 999) are `FStars` set 2:
colour `0xff` (red) for planets', `0xc04000` for fleets', TT = `255 − starmaperosseg·255` (`starmaperosseg` is 1 but
in `starmapszarok`), size 1.3. The surviving circles show their discs: planet p's `RRange[p − 1]` = `rrange.xvb` (a
15-segment unit fan, `rrange.bmp` pure red), moved and scaled to the circle by `rotate_stars`; fleets' `rrang2.xvb`
(`rrange2.bmp`, blue-violet); transparency `1 − starmaperosseg·0.1·fade` (fade 1 up to zoom 580, 0 at 590).
[obs, captures: a disc adds 10 % of its colour in gamma space (red +25.5, green and blue unchanged), and overlapping
discs add once (coplanar and depth-tested).]

### 6.4 Stars (`PulzaljKisCsillag` exe 0x5a05a0, `PulzaljNagyCsillag` exe 0x5a0810; every `ShowStarmap`)
- The far stars twinkle. Tables at the first call: `rnd1[i] = ((sin(i·82 (f32))/3 + 1)·0.25·180 (f32))/3.1415`,
  `rnd2[i] = sin(i·415)·30 + 120` (f32), `sine[j] = sin((j·0.0078125) (f32)·3.141592)·90`. Each frame a quarter of
  the stars (i ≡ `melyikkiscsill` exe 0xE75914 mod 4) get colour `0x646464` with TT = `sine[((i + ⌊futofeny/10⌋)·
  rnd1[i]) & 0xff] + rnd2[i]` (with `starmaperosseg` ≤ 0.99: `255 − (255 − TT)·starmaperosseg`). `futofeny` (exe
  0xF1736C, + 1 per UI tick; also the fleet sprites' row clock) is unsaved like the drift counter.
- System stars: colour **white** `0xffffff` (not the planet's `szin`), size `StarSize[c] + sin(c + futofeny·0.25/13)/2.5`
  (c = planet − 1, f64) for the planets the human sees.

### 6.5 The sky sphere
`Init3dStarmap` loads `gomb.xvb` (`AddToScene(…, mainxvb, 2.0)`, properties 0xc0000 on and 6 off, never moved): a
sphere of radius ≈ 2048 around the galaxy's origin, two parts textured `texcso3.bmp` (orange and teal nebulae) and
`ujtext.bmp`, seen from inside, lit by the scene's ambient 0x404040 and parallel light 0x808080 (towards (−1, −1, 1),
the colony's convention). It makes most of the exe's broad haze. Open: the sub-meshes' own ambient 0x404040 and the
property bits are unexplained; the light levels above fit the captures best (a sun of 0.25 or the ambients summed fit
worse).

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
Wormhole particles, `starmapszarok`'s brightness change. The hot-spots' tooltips are in re/screens/input.md §8.

## 10. Input → orders (lockstep messages)
The screen never changes the game state itself: every order goes out as `Multi_Send(type, EN, …)` and is carried
out by **`MULTIGET_Starmap`** (exe 0x59d930, ELF 0x719680) when the message comes back, on every machine, in the
next frame's message pass (also in single player, also while paused). The handler drains the types in ranges
(`Multi_GetNextTipus(lo, hi)`): 1..9, 0x14..0x1d, 0x3fc..0x40f, 2000; then fixes the selection (§10.4).
`Multi_Get(type, &issuer, marker, &v1, marker, &v2, …, 0)`: the values below follow the issuer.

### 10.1 The messages
| Type | Values | What the handler does (exe) | Sent by |
|---|---|---|---|
| 0x14 | fleet | Split, only the issuer's fleet: `FlottaNew(AlapFlottaNev, e, its story id, x, y, 0)` (the exe's `FlottaNew` clears `+0x2a..+0x2d` with one dword, past the visibility bytes), then `FlottaMozgat(fleet, 0, 0, 0, 0)`; for `EN` the screen selects the new fleet (`AktFleet`, saved) and opens the transfer between the two | rop icon 0xd |
| 0x15 | fleet, order, kind, x (f32), y (f32) | `FlottaMozgat(fleet, order, kind, x, y)` for **any** fleet (the screen also steers story fleet 0x1f88); then the tutorial's event 0x15 | right click (§10.3), rop icons |
| 0x16 | planet, kind | kind 1: `DeleteKolonizalo(FindNearestKolonizalo((int)px, (int)py, e), planet)`; kind 2: `DeleteTerraformalo(FindNearestTerraformalo(…), planet)` (exe 0x53ceb0/0x53c560: the first terraformer of the fleet leaves in a new fleet "Terraformer", always a new one, the old fleet stays even when emptied; flags and pools as `FlottaMozgat`, order 1/1 to the planet, flag 0x10, pool − 1) | rop icons 7, 8 |
| 0x17 | planet, index | Sell station `index` (1-based, `+0x100 + 2(index − 1)`) if `0 < index ≤ +0x114`: `MoneyHozzaadLog(e, 0, CsokkentettTervAr(e, design))`, the list closes up, count − 1 (no owner check) | `StarmapSelectRemoveSpaceBase` menu |
| 0x18 | planet, kind | Satellite: `MuholdAr[kind]` (exe 0x9138D0, i32 × 3); if `MoneyMennyi(e, 0)` < price, `ScrollHibaUzenet` for `EN`; else spend it and `+0xc4 + e = kind` | rop icons 5, 6 |
| 0x19 | planet, what, value | 1/2 tax `+0x34` −/+ (0..=7), 3/4 aggressiveness `+0x6d` −/+ (0..=3), 5/6/7 copy tax / autobuild `+0x6c` / aggressiveness to every planet of the issuer it sees (`+0xd8 + 2e ≥ 0`), 8/9/10 set them to the value's low byte (no owner check) | select menus |
| 0x1a | planet, 64 bytes | The whole production block `+0x78..+0xb8` if the planet is the issuer's | `StarmapKeyboardHandler` (a queue copied) |
| 0x1b | fleet | `FlottaDelete` if `+0x54 < 1` (sim-originated: `fleet-movement.md` §4.3, §10.2 here) | the sim |
| 0x1c | from, to, design, ship, count | Both holders the issuer's (fleet > 0, planet −p). Ships of `from` with that design (only ship `ship` when non-zero) move to `to` (`HajoAtpakol`) while `count` lasts; for a vehicle design (`+0x16 == 2`) into a fleet the count is capped by its room (`+0x78 − +0x5c`); the first vehicle entry of `from`/design moves `min(count, entry)` (`TankAtpakol`); then the ground vehicles a fleet `from` can no longer carry (`+0x5c − +0x78`) go to `to`, each entry capped by `to`'s room. A ship onto a planet makes the exe write outside `Flottak` (the screen never sends it) | transfer panel |
| 0x1d | op, planet, line, (unused), slot, value | The issuer's planet, queue line `L` (0 ships, 1 vehicles), the slot whose UI byte `+0x98 + 4L + k` is `slot` (the last match): 1 count − `value` times (min 1; then `+0xa8 + L` stays below slot 0's count), 2 count + (max 100), 3 toggle repeat `+0x90` unless a station (`+0x37`; the exe tests `== 0`), 4 put design `value` there (0 = remove: the slots close up with their counts, repeats and `+0xad` bytes, the UI bytes swapped along; slot 0 refunds the started unit `(1 − left/time)·price` in f32 to pool 1; a new design drops to the first free slot before it; slot 0 resets `+0xa0 + 4L` to its build time and `+0xa8 + L`), 5 toggle `+0xad` (into a fleet) | production view |
| 0x400 | planet, target | `+0xac` (rally planet) = target (no owner check) | redirect mode (rop icon 3, then a planet) |
| 0x403 | value | `Multiplayer_speed` (exe 0xBAE0AC) | MP |
| 0x406 | level | `SetIdoSpeed(level, true)` (exe 0x541330): SP 1/2/3 → `IdoSpeedSzorzo` 1/4/16, battles 1/2/4, unless `nemlehetidotallitani`; MP follows `ProgramSebesseg` (1/2/3 → 4/8/16, battles 1/2/2), a message picks only when it is 4 (4/8/16, 1/2/4); `IdoSpeed` = the level | speed buttons |
| 0x407 | paused (byte) | `SetIdoPause(p, true)` (exe 0x541300): MP never pauses unless `ProgramSebesseg` is 4 | pause button |
| 0x40f | planet | The head units of both queues are finished (`+0xa0 + 4L = −1`) | a debug key |
| 1, 2, 0x3fc, 0x3fd, 0x401, 0x402, 0x404, 0x405, 2000 | | session: money/join, chat, desync checks, a player dropped (→ AI), log files, a player left | `ig2-net` |

`IG2_gameUpdate` (exe 0x4e0440) handles the top bar's hot-spots: 2 = pause toggle, 3..5 = speeds 1..3. In SP it
calls `SetIdoPause(!IdoPause, false)` (not on screens 10 and ≥ 0xd, nor when the speed is locked) and, for a speed
button when paused or at another speed, `SetIdoSpeed(n, false)` + `SetIdoPause(false, false)`; in MP it sends
0x406 + 0x407(0) (or 0x407 alone) instead. [inf: the buttons' rectangles are the speed brushes 1012..=1015.]

### 10.2 The sim's own 0x1b
At the end of every fleet's step `FRISSIT_Starmap` (exe 0x5970d5) sends `Multi_Send(0x1b, EN, f)` for the local
player's fleet without ships (`+0x54 < 1`, neither flag 0x400 nor 0x4000) unless the transfer panel shows it
(`AktKep == 2`, `StarmapTransferMode` 0xB9A304, `StarmapTransferFlotta1/2` 0xB9A62C/0xB9A954); an AI's is deleted at
once. The next frame's handler deletes it if it is still empty (a split's new fleet left empty goes this way).

### 10.3 Mouse (`Starmap_Main` ELF 0x6f50a0)
- Picks: `SelectNearestUnit(sx, sy, r, exclude, &kind, skip_selected, enemies)` (ELF 0x6e46b0): fleets the human sees
  (`+0x19 + EN` bit 0 and `+0x18` bit 0 clear, or always visible) and planets it knows (`+0xd8 + 2EN > 0`) whose
  projection is within `max(225, (Zoom/20)²)·r²` squared virtual pixels (`Tavolsag` 0x6e45f0), at most 10, ranked
  fleet `id + ships·10000 + own·10⁸`, planet 1; with `enemies` only others' fleets without flag 0x200 and owned
  others' planets without `+0xbc` bit 0. The pick is the highest rank below `exclude`'s (a fleet's rank, 1 for a
  planet −p, unless `exclude` is out of reach), else the highest within 1.1 × the nearest distance: clicking again
  cycles through a stack.
- Left press: `SelectNearestUnit(…, 1.0, AktFleet or −AktPlanet, false, false)` selects (`StarmapMode` 1 planet / 2
  fleet, `AktPlanet`/`AktFleet`) and opens the radial menu (§14); the release carries out its icon.
- Right press in planet mode on the player's planet (and a left one in the redirect mode): the rally order 0x400
  (exe 0x5b53e2: `StarmapMode == 1`, the player's planet, `Bal == 0 || StarmapVillogasMode || StarmapAtiranyitasMode`,
  `StarmapRopIkonMod == 0`; in fleet mode the fleet branch takes the press, redirect mode or not; §14.5).
  `StarmapVillogasMode` (0xb9a964) is the pending fleet order's kind (1 move, 2 attack, 0xd point, 0xe on the fleet
  itself), set and cleared inside the right-click arm (0x5b53d3), cleared by `MULTIGET` 0x15 (0x59db19), `Starmap_Belep`
  and `Starmap_MouseInit2`; its left-held setter (0x5b4538) needs `ControlConfig[0]` (touch), off in GOG: the gate's
  Villogas term is always 0 there. The 100-tick order-line blink is `f176f0`/`e836bc`, not it.
- Right click with the human's fleet selected (or story fleet 0x1f88): Ctrl = attack mode; the pick at r = 0.5
  (skipping the fleet itself, its target excluded); Alt = no pick. A hostile pick (owned by someone other than the
  human and the fleet's owner; fleets without 0x200, planets without `+0xbc` bit 0) turns a move into an attack (2).
  The order is sent unless the fleet is in battle (`+0x52` bit 0) or the move is onto itself:
  `0x15(fleet, mode, kind, (f32)target, 0)`, or for no pick (or Shift) a point: `get_pos` (the galaxy point under
  the cursor, `SikMetszespont` on h = 0) pulled within 600 of the centre, mode 13 (append a waypoint) with Shift,
  14 (patrol) when the click is on the fleet itself (`Tavolsag < max(225, (Zoom/20)²)/4`).
- Minimap (hot-spot 7, `STAR_M.RAW` colour 0xd, §10.6): on the press and every frame while held (`Mouse_Megfogva`,
  not on the release frame) the view's target pivot `XStart = −((x − 274)/92 − 0.5)·600`, `YStart = −((y − 388)/65 −
  0.5)·600` (±299.9), tweened over 4 frames (`TargetFrame` 4, zoom/rotate/pitch kept): a drag; `PlaySoundGlobal(0)` on
  the press.

### 10.6 Keys, console and edge scroll
- `StarmapKeyboardHandler(vk, ch, down)` (ELF 0x704580, exe 0x5aeab0; `DoKeyboard` passes a key down as `(vk, 0, 1)`,
  a key up as `(vk, 0, 0)`, a character as `(0, ch, 1)`, re/screens/input.md §1):
  - With no long info (`StarmapPlanetInfoMod`), zoom (`StarmapZoomMode`) or fleet panel (`StarmapFleetMode`), down and
    up: numpad + / − (0x6b / 0x6d) set `KeyboardZoomBe` / `Ki` (and clear `…2`), ← → ↑ ↓ set `KeyboardRotateBal` /
    `Jobb`, `KeyboardPitchFel` / `Le`; the characters `=` / `-` set `KeyboardZoomBe2` / `Ki2 = 20` unless the held flag
    is on (so numpad −'s own `-` is ignored).
  - Fleet panel up, down: ↑ ↓ move `ActInfoItem2` by ∓2, ← → by ∓1, clamped to `[0, InfoHanyHely − 1]`; `Kezdet`
    steps by 2 to keep it in the 8 shown; changed → `InfoBetoltAlap`, `ActInfoItem = ActInfoItem2`.
  - `DAT_0144c4cc` (debug) moves the planet by ±5 with the arrows.
  - Unless `TutorialStarmapMindenLetiltvaKiveveAKurzorMozgatoBillentyuket`, down only: PgUp / PgDn (0x21 / 0x22) with
    no select menu (tax, auto-build, priority, station sale), long info fly (`StarmapPlanetInfoZoomMod`), fleet panel
    or transfer: planet mode (the human owns a planet, or Shift): `AktPlanet` steps back / on (cyclic, 1-based) to the
    next planet the human owns (any with Shift) and knows (`+0xd8 + 2EN ≥ 1`, or `DEBUGX`); the long info's
    `StarSize` hiding moves with it; `AktEpulet = 0`, `StarmapProductPlanetChange`, `StarmapKozepreAllit(+0x18, +0x1c)`,
    `TanksInfoFirst/Selected = 0`, `DeleteMenus`. Fleet mode (the human has a fleet): `AktFleet` steps to the next
    own fleet (`+0x15 == EN`), `StarmapKozepreAllit(+0x30, +0x34)`, `DeleteMenus`. `c` (99): planet mode, a free planet,
    `!StarmapKolonizaloElkuldve`, `OsszHanyKolonizalo[EN] > 0`, `!IndultEKolonizalo`, no info −0x2094 →
    `Multi_Send(0x16, planet, kind 1)`, `StarmapKolonizaloElkuldve = 1`. `DEBUGX` only: Home (copies the queues to the
    empty own planets, 0x1a), End (`HanyEpuletLefoglalva = 0`), `l` (+10000 population).
- `StarmapKeyboardDoModifies` (ELF 0x705630), every 10 ms UI frame on every screen (`Fake_FRISSITES`): a running
  `…2` step zooms one key step (in when `ZoomBe2`) and counts both down; a held zoom flag one step; rotate ±0.02
  (`Bal` −), pitch ±0.00628318 (`Le` +) with the §2.4 clamps.
- The console and the minimap are mask hot-spots (`Starmap_MouseInit` 0x6feb70: `MouseHanyasIkonKep = 0x1f`, kep
  0x1f = `STAR_M.RAW`, 640 × 480 colours; `Mouse_Darabol` takes the colour under the pointer, the rectangles are
  unused; on together, `Mouse_Szoveg_On_Range(1, 7)`, off under the long info, the production view, the transfer
  and the fleet panel). Colours and boxes: 1 top view 0xf (244..276 × 376..461), 2 rotate right 0x10 (218..271 ×
  384..461), 3 rotate left 0x11 (385..421 × 384..453), 4 side view 0x12 (361..395 × 377..461), 5 zoom 0xb (278..361 ×
  366..386), 6 pitch 0xc (277..362 × 452..473), 7 minimap 0xd (272..367 × 387..452). `Starmap_Main` lines 163-257:
  1 (on the release) `PlaySoundGlobal(0)`, `StarmapSetTopView` (§2.4, tweened 8 frames when a planet counts); 4 (on
  the release) the sound, `TargetMozogj`, 8 frames, `StarmapSetSideView`; 2 / 3 while held `KeyboardRotateJobb` /
  `Bal` (the other cleared), cleared on the release; 5 while held left `KeyboardZoomBe`, right `KeyboardZoomKi`; 6
  left `KeyboardPitchFel`, right `Le`; 2, 3, 5, 6 click on the press. Not with `ControlConfig[0]`.
- `Starmap_Autoscroll` (ELF 0x6fe890), every UI frame unless entering/leaving a screen: on the starmap (`AktKep` 2),
  no `ControlConfig[0]`, no tutorial, long info or zoom; no button held and the physical pointer within 8 px of the
  window's edge → a target ±30 per axis, normalised to length 30 (f64, the second axis seeing the first's new value);
  `xSpeed`/`ySpeed` step 5 towards it; non-zero → `Starmap_Move(xSpeed/5, ySpeed/5)` (§2.4).
- Console press animation: hot-spot → sub-mesh 1 → 4, 2 → 5, 3 → 2, 4 → 1, 5 → 0, 6 → 3 (`Starmap_Main` lines
  163-257). `ShowStarmap` (ELF 0x6eca50 lines 379-381, 465-473 / exe 0x5a3250) steps ±5 per drawn frame (a UI tick);
  `Mouse_Felenged` → phase 2; only `MelyikGombot`'s sub-mesh is set; held 2/3/5/6 re-set frame 0 each frame so they
  show frame 5; views 1/4 act on the release frame and never sink; `Starmap_Belep` (exe 0x5b19a0) zeroes the six
  sub-mesh frames only. Tooltips of 1..=7: `GetString(2, 1, N)` (`Starmap_MouseInit` exe 0x5b99d0).
- `StarmapProductPlanetChange` (ELF 0x6fe2b0) only queues the production view's head-model swap. `Starmap_MouseInit2`
  (ELF 0x7024b0 / exe 0x5b8950) turns 0x35 off under the long info, tanks, fleet panel and transfer.
- The edge scroll reads the last physical pointer (`Mouse_Phys_x/y`, exe 0xdb9408/0c), so it goes on with the pointer
  outside the window. Both start at 0 and only SDL_MOUSEMOTION writes them (`MainCallBackEventHandler` exe 0x4e34e1;
  nothing warps the pointer), so before the first move the exe scrolls up-left.
- Buttons (`Mouse_Lekerdez` exe 0x5105d0): `Mouse_Gomb` is any button held, `Mouse_Felenged` the last one let go,
  `Mouse_Lenyom` the first one down; a held spot stays latched (`Mouse_Megfogva`) until every button is up and a second
  button's press goes to it (no new click); 5 and 6 read the buttons every frame, left (in, up) then right (out, down),
  so the right wins with both held; 2 and 3 ignore which button. Every held frame of the map view (hot-spot 0x35) sets
  `Mouse_Megfogva` (exe 0x5b54a2; also spots 2, 3, 5, 6, 7 and the panel cells, never 1 or 4), and the pick runs only
  on the first held frame (0xdb9418, set while no button is down), so after a press on the map a second button starts
  no console spot, pick or order until both are up (`scripts/starmap-second-button.json`: the exe's view doesn't zoom
  then). An open menu holds `Mouse_Megfogva`, so `IG2_gameUpdate` (exe 0x4e0440 line 52) doesn't re-read `AktIkon` and
  the release arm runs under the lock.
- [obs, `scripts/starmap-console.json`: the exe's first ~100 script ticks draw nothing and lose their events; the
  console's blue rim is its shading, the same at rest.]

### 10.7 The rename (`IOMode`, `IOAzonosito` 0x15 planet / 0x16 fleet)
- Start (`Starmap_Main` ELF 0x6f50a0, the info box branch, `ControlConfig[0]` clear): hot-spot 8 with both buttons held.
  Planet mode, any planet (after the branch's `PlaySoundGlobal(5)`): `IOString` = the planet's name (record +0),
  `IOMaxLen` 0xf, colour 0x22. Fleet mode, `EN`'s fleet (not `fSpec`): `PlaySoundGlobal(0)`, the fleet's name (+0),
  `IOMaxLen` 0x13, colour 3, the fleet's `+0x52 |= 0x800` (exe 0x5b62be / 0x5b6cf4). Both: `IOStringBackup` = the old name, `IOX`
  0x32, `IOY` 0x1ab, `IOCharSize` 2, `IOKurzor` = its length, `IOGombnyomasraTorles` 1, `Mouse_FelengedesMajdMegvar`.
- The name is edited in place (no message; every machine keeps its own). Keys: `DoKeyboard`'s editing branch
  (re/screens/input.md §1); Enter ends it (`TutorialCheckEvent(4)` fleet, `(3)` planet); Esc restores the backup.
  Any press ends it (`Starmap_Main` line 157), the backup restored unless the press is on hot-spot 8. `FlottaDelete` /
  `FlottaOsszevon` end a fleet's rename when the fleet goes.
- Drawn by `StarmapGfx2D_3` (ELF 0x6d4a30): until the first key a colour-0x27 box (`FillToScreen(IOX − 1, IOY − 1,
  len·cw + 2, ch + 2)`), `KiirStr(IOString, IOX, IOY, IOMaxLen, 2, 4, IOColor)`, on `AktIdo200` odd the cursor bar
  `FillToScreen(IOX + IOKurzor·cw, IOY + ch, cw, 2, 1)` (on the last cell once full).
- `0x800` is the fleet's custom-name bit (ELF `FlottaNevLatszik` 0x615470 = `strcmp(name, AlapFlottaNev) != 0`;
  fleet-movement.md: bit 11), not an editing flag: nothing at the rename's end clears it (Esc, a press elsewhere, Enter),
  so a fleet once renamed keeps its name drawn by `Show_Flottak2d` (exe 0x5a5080, test 0x5a5370: the name and its
  shadow under the marker). Recomputed from the name by `FlottaNew` (exe 0x53d755) and `FlottaOsszevon` (0x53d94d),
  cleared by `InitNewGame` and the AI's main-fleet change, set/cleared by the stories. Its only readers are that draw
  and the Elem story (0x5d52b6, host only), so the local edit (no message, like the name) can't desync.

### 10.4 The selection fix-up
After every message pass: with `StarmapMode` 2 and `AktFleet` gone (owner 0xFF) or no longer seen (`+0x19 + EN`
bit 0 clear or `+0x18` bit 0, and not always visible, not `DEBUGX`), unless it has flag 0x400: the first own fleet
with `+0x18` bit 0 clear, else the first own one, else the first seen one; none → `StarmapMode` 1.

### 10.5 Verification
`run_replay.py --orders N --orders-every T` stages N random messages of every type before each T-th tick's save
through the exe's own handler (its `Multi_GetNextTipus`/`Multi_Get` calls pointed at stubs, libc pinned per batch);
the trace report is `rules/traces/2026-10-17-starmap-orders.md`.

## 11. The HUD (`StarmapGfx2D_1`, `_3`, the control panel)
Drawn in the original's order: the 3D pass with the control panel, then the 2D passes.
- The control panel `kezel.xvb` (`Init3dStarmap`: `AddToScene(…, 0.17)`, `SetRelative(false)`, `MoveObj(0, 170,
  −600)`, not rotated): a camera-fixed model with half-extents 1024, so a vertex `v` sits at `174.08·v + (0, 170,
  −600)` in camera space, over virtual 214..426 × 366..474. Texture `textures/starmap/starpan`; no faces over its
  window. Lights: the scene's ambient 0x404040 and parallel 0x808080, plus two parallel lights of its own
  (`smOldalsoFenyekBe` ELF 0x6d81e0: colours 0x7f5d00 and 0x004e7f) that `smMozgatMainFenyek` circles in front of
  it (`(sin·300, cos·200, 0)` of `smkeringeto·20`). `KezeloGombFrame`/`MelyikGombot` animate its buttons (§10.6).
- `StarmapGfx2D_1` (ELF 0x6dcb30): `KirakFader(…, 5, 423, 162, 52, 0, 54s, 90s, 155l + 100, 0, 90s, 130s, 185l + 70)`
  behind the short info (`FadeKerettelKerekit`: rounded fill, then frame; s = 0.9, l = 0 at rest; the fader rules,
  texture included, are re/screens/hud.md §1.2), then `FadeScreen(271, 385, 98, 71, black, 0x40)` over the
  minimap, 0x40 being the transparency (75 % black [obs]).
- `StarmapGfx2D_3` (ELF 0x6d4a30, exe 0x5aaba0): a selected fleet's mark (brush 11 at (6, 430) for flag 0x4000,
  else brush 3 at (6, 429) for 0x400), `StarmapRovidInfoKiir(50, 424, 0)`, the buttons `BrushClipToScreenMelyikXY(9,
  Starmap_RopIkonKonvert[k], …)` (exe 0x927088: `[0, 13, 9, 10, 8, 1, 2, 6, 7, 5, 0, 0, 0, 3, 4, 0, 11, 12, 14, 0]`):
  tanks (k 1) at (171, 430) and production (k 2, frame 9) at (419, 430) when `Starmap_MouseInit2` (ELF 0x7024b0) set
  `StarmapTanksGomb` / `StarmapProductGomb` (the latter: planet mode on the human's planet), the redirect mark (k 3)
  at (471, 430) blinking, then `GFX_StarmapRadar` (§8). `StarmapBrushok` 9 is kep 33, −49 × −49, 14 per row.
- `StarmapRovidInfoKiir` (exe 0x5afc50; `KiirStrAlap(text, x, y, 0x13, size, 0xc, colour)`):
  - planet mode: the name at (50, 427) size 2 colour 0x22; when the planet is the human's, or it has a satellite
    there (`+0xc4 + EN`), or the human is race 7 and `+0xcb` is 0, or `DEBUGX`: `"\x03:"` (size 2) and
    `GetBolygoGazdagsag(p)` + `%` (size 1) at (8, 427)/(18, 428), `"\x01:"` and `FajBolygoViszony[race][surface]` + `%`
    at (8, 466)/(18, 467) (race = the owner's, the human's for a free planet), colour 1; for a planet not the
    human's with a satellite: the satellite's tech name (`Talalmanyok[level + 0xe]`) at (50, 457); the owner's
    empire name `BirodalomNev[owner]` at (50, 439) colour 0x23; for an owned, seen planet `GetString(2, 3, 1) +
    ": %d (%+d)"` (population `+0x28`, change `+0x2c`) at (50, 448); for the human's planet the line at (50, 457)
    steps every 30 `Counter` frames from the planet's selection through `MOR` (`GetString(2, 3, 2) + ": %s"` with
    `Moral_Nev[(int)(f32)(+0x30 / 100)]`) and the problems `StarmapProblemaRendez` sorts (ELF 0x7082c0: the four
    worst of 1..=19 at ≥ 100000, `"%d%% %s"` with `+0x118 + 4p / 10000` and `Problemak_Nev[p]`), and the
    production line at (50, 466) colour 1: the head ship (or vehicle; alternating every 30 frames when both)
    `"%s - %d%%"` with `100 − left·100/time`, shortened to `"%s-%d%%"`, then `"%s%d%%"` beyond 19 bytes.
  - fleet mode: the name at (50, 427) size 2 colour 3 (6 for flag 0x400, 1 for 0x4000), the empire at (50, 439)
    colour 0x21 (6, 1); for the human's: "Stay in space" (`GetString(2, 3, 3)`), or `MOVE`/`TARG` (3, 5/6) + `":%s"`
    with "to space" (3, 4), the planet's or the fleet's name, at (50, 448); unless flags 0x4400
    `"SH:%d FI:%d TA:%d/%d"` (3, 7..9: ships − fighters, fighters `+0x58`, vehicles `+0x5c`, capacity `+0x78`),
    beyond 19 bytes `"S:… F:… T:…/…"` (3, 10..12), then without the capacity, at (50, 457); unless flag 0x400
    `SPEED` (3, 13) + `": %d"` of `+0x38·100 + 0.5` at (50, 466).
  - `BirodalomNev`, `Moral_Nev`, `Problemak_Nev` come from `StringReadAll` out of the **Hungarian** `txt17.txt`
    whatever the language [obs: "GODAN KIRALYSAG", "MOR: Jó" in an English run]: section 1 index 33 + e, section 4
    index 21 + p and 41 + k.
  - The picture left of the text (between the two icon lines, virtual ≈ 6..38 × 434..466) is a 3D object, not a
    2D pass: `StarmapLoadInfoBolygo` (ELF 0x6d3950, from `Starmap_Belep`) loads `vector/info/bolygo_1.xvb`
    (`VectorDir` `vector/info/`, `TextureDir` `textures/info/`) as `SBolygoId`: `AddToScene(…, 0.029)`,
    `SetRelative(false)`, `RotateObj(π/2, 0, 0)`, `MoveObj(−545, 365, −1100)` (camera-fixed, projecting to ≈ (23,
    449)), `PropertiesOn(0x2000000)`, `PropertiesOff(2)`, hidden, its matrix kept in `SBolygoMatrix`.
    `StarmapFrissitBolygoUrhajok` (ELF 0x6d3d40, every `ShowStarmap`) hides it and the fleet picture `SFlottaId`,
    and while `spacewarelfade` < 1 and not `KilepesElott` (transparency `spacewarelfade`): in planet mode unhides it,
    `SetObjMatrix(SBolygoMatrix)` then `matrixforgat(SBolygoMatrix, SBolygoMatrix, 0.05, 0)` (a turn a drawn frame)
    and gives sub-mesh 0 `BolygoTexturak[surface − 1]`; in fleet mode (not a special fleet) it unhides `SFlottaId`
    (`StarmapLoadInfoFlottak(race of the fleet's owner)` when `AktFleet` changed, turned the same way). The globe is
    drawn over `StarmapGfx2D_1`'s fader, undimmed by the fill [obs, quick-1.sav planet 42]; the fleet picture the
    same.
  - `StarmapLoadInfoFlottak(race)` ELF 0x6d3b10 (race > 8 → 6): `vector/spacewar/flag<race>0.xvb`
    (textures/spacewar), `AddToScene(…, 0.03)`, `SetRelative(false)`, `PropertiesOn(0x2000000)` (no `PropertiesOff(2)`,
    unlike the globe), `RotateObj(π/2, 0, 0)`, `MoveObj(−535, 365, −1100)`, hidden, matrix kept in `SFlottaMatrix`;
    `StarmapInfoRemoveUrhajok` 0x6d3930 = `RemoveFromScene(SFlottaId)`. `StarmapFrissitBolygoUrhajok` reloads it when
    `AktFleet` ≠ `SInfoOldFleet`, the fleet isn't `fSpec` and its owner (+0x15) isn't 0xff, with race
    `PlayerFaj[owner]`, so a new fleet restarts the turn; `SFlottaMatrix` turns only while drawn in fleet mode, so the
    phase counts from the selection [obs, light probe: flags 0x200000F, ambient 0x404040 + parallel 0x808080, turning
    −0.05 a frame about y like the globe].

### 11.1 Reference captures
`run_screen.py <install> quick-1.sav <out> --kep 2 --planet 71` (the tool sets `AktPlanetKell`, so `--planet` picks
the shown planet); the fleet box: `--fleets 2 --rop-fleet 1`.

## 12. The production view
The selected planet's two build queues over the starmap (ELF `StarmapGfx2D_2` 0x6de1b0, `StarmapGfx2D_3` 0x6d4a30 /
exe 0x5aaba0, `ProductGFX` 0x6e8870, `ShowStarmap` 0x6eca50, `Starmap_Main` 0x6f50a0 / exe 0x5b2b70, hot-spots
`Starmap_MouseInit` exe 0x5b99d0 and `Starmap_MouseInit2` 0x7024b0).

### 12.1 State and animation
`StarmapProductMode` (exe 0xB9933A, up), `StarmapPModeIn` (moving), `StarmapPDir` (opening), `StarmapPFrame`
(`gyartas.xvb`'s animation frame). `AktRopIkon == 2` (the production button, hot-spot 11; also the radial icon)
opens it (`PDir` 1, frame 0) or, when up or opening, closes it (`PDir` 0, frame 0x11, `ProductMode` 0); so do
hot-spot 9 (the exit button), the tanks button, the planet info, and `FRISSIT_Starmap2` 0x719070 when `StarmapMode`
isn't 1 or the planet isn't `EN`'s (not with `DEBUGX`). `ShowStarmap` steps once per drawn frame [inf: 60 Hz]:
opening frame 0 → 12, then `ProductMode` 1 (and `PrShipAdd`/`PrTankAdd` for the head units); closing 17 → 30, then
off. Frame 0 hides the model. A transfer panel open at the press closes first (`StarmapKiZoomol` + `TransferOff`).

### 12.2 Pictures
- `gyartas.xvb` (`Init3dStarmap`: `AddToScene(…, 0.6336)`, camera-fixed at (0, −50, −600), an ambient light 0x0a0a0a
  of its own): nine groups; the eight slot clusters fly in from z ≈ 1 to the root's pivot by frames 9..12 and shrink
  to 0 over frames 19..30 (node keys, `Kocka`); group 50 is
  the name plate. `GetProductionPanelSubMeshIDs` 0x6d4010 maps slot `r = k + 4L` to its minus, plus, "into a fleet"
  and repeat meshes (`Group%02d`: r0 9/8/11/10, r1 17/16/19/18, r2 25/24/27/26, r3 33/32/35/34, r4 13/12/15/14,
  r5 21/20/23/22, r6 29/28/31/30, r7 37/36/39/38). `ProductGFX` textures a repeat lamp `gyart_gr_3` when
  `+0x90 + 4L + k` is set, else `gyart_gr`, and line 1's "into a fleet" lamps `gyart_gr_3` / `gyart_gr_2`
  (`+0xad + 4 + k`). A pressed button sinks 40 units (`ProductionButtons` 0x6ea3d0 / `…2` 0x6ea450 around the
  render).
- The head units (`ProductGFX`): the ship line's head design's hull model `TransfAlap[+0x44]` (exe 0x927410, 25 ×
  0x38: name, `+0x34` 0/2 `talalmanyok`, 1 `spacewar`, 3 `colony`; a spacewar hull < 12 gets
  `'0' + PlayerFaj[design +0x40]` (at most '8') as its name's 5th character) at (−540, −350, −1600) ×0.375, the
  vehicle line's at (540, −350, −1600) ×0.28 (race digit of `EN`; a ground hull > 0x12 gets
  `tankok/<kerekes|kozep|lebeg|lanc|aknarak><owner ≤ 10>.bmp` on its first sub-mesh); both `RotateObj(π/2, 0, 0)` +
  `RotateObj2((1, 0, 0), −0.5)`, then `RotateObj2((0, cos −0.5, sin −0.5), 0.05)` per frame,
  `SetObjTransparency(1 − p)` with `p` = the bar's fraction clamped to 0.05..0.95, plus a wireframe twin at `p`. `RotateObj2(axis, θ)` makes `R = creatematrixfromaxis(axis, θ)·R` (0x4520a0: the quaternion
  `(cos θ/2, sin θ/2·axis)`).
- Faders (`StarmapGfx2D_2`; `KirakFader` → `FadeKerettelKerekit` 0x538330 → `FadeKerekit`/`FadeKeretKerekit` →
  `FadeScreen` 0x537890, which queues `0xTTBBGGRR`: **the fourth colour value is a transparency**, the colour is
  blended at opacity `(255 − t)/255`, tinted by `dt.bmp` (re/screens/hud.md §1.2); `s` = the fader's `+0x168 + 4i`
  = 0.9 (`FaderDesignFrissito` 0x547e10, plus a short shine since `GLOBALKELLDESIGN` is set)): when up, `FadeKerettelKerekit(5, 80, 190, 292, 0, 20, 20, 200, 0, 15, 15, 150)` and
  (445, 80, 191, 292), the slot boxes `KirakFader(11 | 511, y, 120, h, 0, 54s, 90s, 100, 0, 90s, 130s, 70)` for
  (y, h) = (125, 60), (206, 50), (261, 50), (316, 50), the capacity boxes (32, 86, 76, 35) and (532, 86, 76, 35).
  Moving (`PModeIn`; f = the frame, or 30 − frame past 12): slot box j slides in by `cos((f − 2j)/6·π/2)·130` (130
  before its start, 0 after `6 + 2j`), the capacity boxes grow from (70 | 570, 103) to `(f + 1)/13` of their size,
  the big panels come in by `(1 − sin(f/12·π/2))·200`.
- The exit button (`StarmapInfoOffHoltart` H: f/12 while moving, 1 when up or with the transfer panel):
  `KirakFader((1 − sin(H·π/2))·200 + 445, 380, 176 + addo, 30)` with `addo` stepping by 1 per frame towards 11
  (production), −10 (transfer) or 0; its text `GetString(2, 4, 4)` at x + 30, 390 (len 13, size 3, field-centred,
  colour 0x25).

### 12.3 Texts (`StarmapGfx2D_3` when `ProductMode`)
The planet's name at (320, 48) size 3 (len 12) or, from 13 characters, (320, 50) size 2 (len 20), centred, colour 1.
Per line L (0 ships, 1 vehicles): the capacity `+0x70 + 4L` at (69 + 500L, 105) size 3 centred (below 1:
`GetString(2, 4, 2)`, "none"), its label `GetString(2, 4, 3)` at (41 + 500L, 94) size 2, field-centred in 10. Per
slot k with a design `d = +0x78 + 8L + 2k` > 0 (y = 141, 214, 271, 326): the design's name at (31 + 501L, y + 2)
size 2 len 18, colour 3 (6 while its design list is open); the count `+0x88 + 4L + k` (slot 0 less the started units
`+0xa8 + L`, ×5 for fighters `+0x36`) at (163 + 315L, y + 12) centred; `"$"` + count × price `+0x24` at
(31 + 501L, y + 20 + 2·(L = 0)). The head unit's bar: `w = 76 − left·76/time` (`+0xa0 + 4L`, design `+0x20`),
`FillToScreen(32 + 500L, 152, w, 4, 3)`, then the rest in colour 6 times `AktIdo300Szazalek` without capacity, else 0
(black).

### 12.4 Input → orders
Hot-spots (`Mouse_Szoveg_Set(id, …, x, y, w, h)`; `Starmap_MouseInit2` turns 0xc..0x36 on in production mode,
without the buttons of empty slots; the lowest id under the pointer wins [inf]):

| Hot-spot | Rectangle | `Starmap_Main` |
|---|---|---|
| 0xc / 0xd | (254, 37, 22, 37) / (363, 37, 22, 37) | previous / next planet of `EN` (`StarmapKozepreAllit`, `StarmapProductPlanetChange`) |
| 0xe..0x15 | (153 or 468, 164 / 238 / 293 / 349, 21, 24 or 21) | `0x1d(3, planet, L, k, ui, 0)`: toggle repeat |
| 0x16..0x19 | (469, 134 / 208 / 263 / 318, 19, 13 or 14) | `0x1d(5, planet, 1, k, ui, 0)`: toggle "into a fleet" |
| 0x1a..0x29 | (135 or 450, y, 28, 51) and (163 or 478, y, 28, 51), y = 132, 205, 259, 314 | held: `0x1d(1 or 2, planet, L, k, ui, n)` on the first frame and every 10 ms frame after the 7th, `n = (KezdoIdo − 7)/8 + 1` ≤ 5; minus while `Mouse_x` < 164 (L 0) / 479 (L 1); not for stations (`+0x37`) |
| 0x2a..0x31 | (11 or 511, 125 / 206 / 261 / 316, 120, 60 or 50) | press: the design list at the pointer; release: `0x1d(4, planet, L, k, ui, d)` (§12.5) |
| 0x32 | (276, 32, 87, 47) | the list of `EN`'s planets (`StarmapSelectPlanet`, §17) |
| 0x33 / 0x34 | (0, 80, 195, 292) / (445, 80, 195, 292) | the panels (they catch clicks) |
| 9 | (445, 380, 164, 60) | close |

`ui` = the slot's UI byte `+0x98 + 4L + k` (the handler finds the slot by it, §10.1). With the view up the map picks
planets only (`SelectNearestPlanet`) and the minimap is off.

### 12.5 The design list (`StarmapProductSelectMode`, drawn by `StarmapGfx2D_4` 0x6d6b20 and `_3`)
Design 0 (remove) and `EN`'s designs of the line's class (`+0x16 == L + 1`) that are visible (`+0x18`), 32 to a
column (`sz = max(1, ⌈n/32⌉)` columns, `ma = ⌈n/sz⌉` rows), at `smSelectY = Mouse_y − 14` and `smSelectX =
Mouse_x + 12` (line 0) or `Mouse_x − 120·sz − 8` (line 1), kept on screen (x ≥ 6, y ≥ 2, right edge 643, bottom 450).
Box `FadeKerettelKerekit(x − 6, y − 2, 126·sz + 3, 13·ma + 4, 0, 10, 0, 60, 0, 5, 0, 30)`; names at
(x + 126·col, y + 3 + 13·row) size 2 len 18, colour 3, 6 under the pointer (hit box x..x + 120 × y..y + 13), 1 for a
station when the planet has 8 (`+0x114` plus the line's queued stations). On release the hovered design is sent
unless it is a ninth station or already in the slot.

## 13. The transfer panel
Units between two holders (`StarmapTransferMode` exe 0xB9A304, `StarmapTransferTankMode` 0xB9A305,
`StarmapTransferEgyesMode` 0xB9A306, `StarmapTransferFlotta1/2` 0xB9A62C/0xB9A954 (a planet is −p),
`StarmapTransferHanyHely` 0xB9A958, `StarmapTransferKezdet` 0xB9A95C). The sim reads the mode and the fleets for its
0x1b (§10.2).

### 13.1 Opening and closing
- Split (0x14; the info box right-clicked on `EN`'s fleet not in battle, or the radial icon 0xd): the handler
  selects the new fleet and opens fleet 1 = the old, fleet 2 = the new (`MULTIGET_Starmap`, §10.1); the click zooms
  in to 590 (`StarmapBeZoomol`) unless it closed a panel.
- `AktRopIkon == 1` (the tanks button, hot-spot 10 at (174, 432, 44, 44), shown when `StarmapTanksGomb`: planet mode
  on `EN`'s unprotected planet with `SearchBolygohozKozeliFlotta(p, 20, true, false, false, true, 0)` > 0, or `EN`'s
  fleet not in battle with `SearchFlottahozKozeliBolygo(f, 20, true, true)` an `EN` planet): vehicle mode between
  the fleet (fleet 1) and the planet (−p), zoomed in to 590, `0x15(fleet 1, 0, 0, 0, 0)` stops the fleet. Pressed
  with a vehicle panel up it closes it; a fleet-to-fleet panel closes and the vehicle panel opens.
- `AktRopIkon == 0xe` (the radial icon): fleet 1 = `StarmapAktFleethezKozeliSajatFlotta` (`EN`'s
  fleet nearest to `AktFleet`, not in battle, `(int)` Manhattan distance < 20), fleet 2 = `AktFleet`, both stopped
  (0x15), zoomed in.
- Closed by hot-spot 9, the info box, the production button, a planet turning protected (`+0x64` bit 0,
  `FRISSIT_Starmap2`): `StarmapKiZoomol` + `TransferOff` 0x6f2810 (an emptied `AktFleet` hands the selection to the
  other fleet; in single player the player's emptied fleets of the panel are deleted at once with `FlottaDelete`,
  multiplayer leaves it to 0x1b).

### 13.2 Rows
`StarmapTransferDarab` and its twin: per design of `EN` the ships (`Hajok` entries) and vehicles (`Tankok` counts)
each holder has. Rows: group 1 ships (`+0x16 == 1`, no fighter flag `+0x36`), group 2 fighters (both not in vehicle
mode), group 3 vehicles (`+0x16 == 2`), designs ascending, only designs either side holds. Item i sits in cell
`i − Kezdet` (0..7): column `% 2`, row `/ 2`. (`StarmapTransferEgyesMode`, ship by ship, is set by nothing found.)

### 13.3 Pictures and texts
- Faders (`ShowStarmap`, `fifo`, `KirakFader(…, 0, 54s, 90s, 100, 0, 60s, 110s, 70)`): the panels (28 or 338, 27,
  273, 334), title bars (30 or 340, 29, 269, 17), bottom bars (30 or 340, 342, 269, 17), the scroll bar
  (306, 29, 28, 333) and its thumb (310, top + 34, 20, h − 8) with h = 331 (n < 9) or 2648/n,
  top = Kezdet·(331 − h)/(n − 8) (an overflow is taken half off both).
- Models (`TransferObjs`, two per row): `TransfAlap[design +0x44]` (here the race digit is `'0' + (design +0x40)`
  and a vehicle skin's owner is `AktFleet`'s), `AddToScene(…, 0.04)`, camera-fixed at ((c·129 − 229) or
  (c·129 + 99), r·70 − 156, −600), `RotateObj(π/2, 0, 0)` + `RotateObj2((1, 0, 0), −0.5)`, turning
  `RotateObj2((0, cos −0.5, sin −0.5), (i % 3 + 1)·0.03)` per frame; a side holding none is three quarters
  transparent. Phase [obs, `light-transfer.jsonl`]: both copies of a row turn in step (identical rotation chains and
  geometry matrices; what looks out of step is perspective, x −229/−100 vs 99/228 at z −600); cell i = 0/1/2 turns
  0.03/0.06/0.09 a frame, and at drawn frame n after the icon press the cells have turned n − 1 frames (`ropFrames`
  in the grab); `InfoAlapRotation` is not in step with them here.
- Texts (`StarmapGfx2D_3`): fleet 1's name at (156, 32), fleet 2's (or the planet's) at (484, 32), size 3 centred,
  colour 3; `ShowFlottaTankCap(110 or 420, 347, f)` (0x6d4560: `GetString(2, 5, 2)` + `" %d/%d"` of `+0x5c`/`+0x78`,
  or `+ GetString(2, 5, 3)` without capacity) / `ShowBolygoTankCap` (`GetString(2, 5, 4)` + `" %d"` of planet
  `+0x58`) at x + 54, size 2 centred, colour 1; per cell the design's name (+ `" (" + GetString(2, 4, 1) + ")"` when
  it has an upgrade target `+0x1c`) at (91 or 419 + c·129, 124 + r·70) size 2 centred, colour 3 / 7 / 1 by group,
  and the counts 12 above (colour 6 for the vehicles of a full fleet, `+0x78 ≤ +0x5c`).

### 13.4 Input → orders
Hot-spots 0x43..0x52 (the cells; `Starmap_MouseInit2` turns on `2·min(n, 8)` of them): left panel column 0
(28, y, 124, 70), column 1 (152, y, 139, 70), right panel column 0 (338, y, 139, 70), column 1 (487, y, 121, 70),
y = 60, 130, 200, 270; item = `(id − 0x43)/2 + Kezdet`; an even id moves left → right (fleet 1 → 2), an odd one
right → left. Held: the first frame and every 10 ms frame after the 7th send `0x1c(from, to, design, 0, n)` with
`n = (KezdoIdo − 7)/8 + 1` (no cap); the right button sends 1,000,000 (all). 0x57 (307, 30, 26, 329):
`Kezdet = ((Mouse_y − 30 − h/2)·(n − 8) + (329 − h)/2)/(329 − h)` with h = 2632/n, clamped to 0..n − 8, rounded up
to even. 0x58 (24, 24, 592, 339) covers the map; the minimap is off. Hot-spot 9 closes.

### 13.5 Verification
`run_screen.py … --kep 2 --planet 71 --rop 2` (production) and `--fleets 2 --rop 0xe --rop-fleet 1` (two new
fleets of the human beside planet 71, the transfer between them) press the icons in ig2.exe itself (`screen.js`
writes `AktRopIkon` 0xE83654 right after `Starmap_Main` clears it) and save the moment.

## 14. The radial icon menu ("rop" icons)
Pressing the left button on the map picks a unit and opens up to four icons around the pointer; releasing the button
over one carries it out (ELF `Starmap_Main` 0x6f50a0 / exe 0x5b2b70, `Starmap_MouseInit2` ELF 0x7024b0 / exe
0x5b8950, drawing `StarmapGfx2D_3` exe 0x5aaba0, frame counter `FRISSIT_Starmap2` ELF 0x719070).

### 14.1 State (exe)
`StarmapRopIkonMod` 0xF16F18 (open), `RopIkonFrame` f32 0xE8634C, `RopIkonFazis` 0xEF1154 (1 opening, 2 closed),
`LastMouseX/Y` 0xE75A74/0xE75A78, `StarmapRopIkonok[1..=4]` 0xE75A30.. (the icons of the four places),
`AktRopIkon2` 0xF16F14 (the icon under the pointer), `AktRopIkon` 0xE83654 (the icon to carry out this frame, cleared
at the head of `Starmap_Main`), `StarmapVoltRopikonAlap` 0xE75A73. Tables (exe .data, the same values in the ELF):

| table | exe | [1], [2], [3], [4] |
|---|---|---|
| `Starmap_RopIkonX` / `Y` (hit centres) | 0x9270d4 / 0x9270e8 | (29, 86), (81, 125), (132, 86), (81, 49) |
| `Starmap_RopIkonX2` / `Y2` (icon top-left) | 0x9270fc / 0x927110 | (5, 62), (56, 100), (107, 62), (56, 24) |

`Starmap_RopIkonKonvert` (exe 0x927088, 19 entries) maps an icon to its frame of `StarmapBrushok` 9 (§11).

### 14.2 Opening, choosing, firing
- Hot-spot 0x35 (the whole screen, the lowest priority) pressed with the left button while no fleet order or redirect
  is pending (`StarmapVillogasMode`, `StarmapAtiranyitasMode` 0) and the menu is shut: on the press's first frame
  (`Mouse_VoltFelengedes`) `SelectNearestUnit(Mouse, 1.0, AktFleet or −AktPlanet, false, false)` (§10.3; with the
  production view up `SelectNearestPlanet`, below) picks; a pick becomes the selection (`StarmapMode`, `AktPlanet` +
  `StarmapProductPlanetChange`, or `AktFleet` + the fleet's voice for the player's own) and opens the menu:
  `StarmapRopIkonMod` 1, frame 0, phase 1. `LastMouseX/Y = Mouse − (82, 84)` either way. **Selection happens on the
  press**, not on the release.
- `FRISSIT_Starmap2` (the 100 Hz UI frame) steps the frame +1 up to 50 in phase 1, −1 down to 0 in phase 2.
- From frame 50 every `Starmap_Main` sets `AktRopIkon2` to the icon of the last place `k` whose hit centre
  `LastMouse + (X[k], Y[k])` is within 30 pixels of the pointer (`dx² + dy² ≤ 900`); it starts each frame at 0.
- The left button's release (hot-spot 0x35, `Mouse_Felenged`) with the menu open: `StarmapRopIkonMod` 0, phase 2,
  frame 0, `AktRopIkon = AktRopIkon2`. Releasing before frame 50, or away from the icons, does nothing.
- The tanks and production buttons (hot-spots 10 and 11) set `AktRopIkon` 1 and 2 directly (`AktIkon2 − 9`).
- `SelectNearestPlanet(x, y, r, &kind)` (ELF 0x6e5150): the player's own planets it sees (`+0xd8 + 2EN > 0`), the
  nearest star projection within `max(225, (Zoom/20)²)·r²` squared virtual pixels, no cycling.

### 14.3 Which icons (`Starmap_MouseInit2`, with no production view, transfer panel, fleet info or tank info up)
All four places cleared, then:
- Planet mode: place 2 = 4 (the colony screen); the player's planet: place 4 = 9 (tank info), place 1 = 2
  (production), place 3 = 3 (redirect). Someone else's planet (or a free one): place 1 = 5 (satellite) when the player
  has none there (`+0xc4 + EN` = 0), hasn't sent one (`StarmapSatelliteElkuldve` 0xE75A72) and tech 15 (Satellite)
  is researched (status `+0x45 + EN` = 5); place 1 = 6 (spy satellite) instead when that level is below 2, nothing is
  sent and tech 16 is researched; place 3 = 0x12 (gigabomb) for an owned planet of empire 1..=8 when the story allows
  it (`StoryAdat + 0x44` and `st_bRakhatoNovabomb(p)`). A free planet (owner 0) without a colony ship sent
  (`StarmapKolonizaloElkuldve` 0xE75A71), none heading there (`IndultEKolonizalo(EN, p)`: an `EN` fleet ordered to the
  planet with flag 0x8 or 0x10) and no "don't colonise" mark (`BolygoInfoIs(p, −0x2094)`): place 4 = 8 (colonise)
  with colony ships in the pool (`OsszHanyKolonizalo[EN]` exe 0xBB4108), place 3 = 7 (terraform) with terraformers
  (`OsszHanyTerraformalo[EN]` 0xBB4158).
- Fleet mode, the player's fleet: place 1 = 0xd (split) with more than one ship (`+0x54`), place 2 = 0xe (join) when
  `StarmapAktFleethezKozeliSajatFlotta` (§13.1) exists, place 3 = 1 (tanks) when the player's planet is near
  (`SearchFlottahozKozeliBolygo(f, 20, true, true)`), 0x11 (virus alert) instead for flag 0x2000. A story fleet of
  empire 9 (`StoryAdat + 0x30`, story id 10000..10999) heading for a planet with `+0xb8 > 0` within the player's radar
  range: place 1 = 0x10 (wormhole). A fleet in battle (`+0x52` bit 0) loses 0xd, 0xe and 1.
- The redirect mode (`StarmapAtiranyitasMode` 0xE75A58) on the player's planet puts 3 in place 2 every other blink
  (`AktIdo100` bit 0); on another planet it ends the mode.

### 14.4 Drawing
`StarmapGfx2D_1` sets `StarmapVoltRopikonAlap` when the menu is open, the frame ≥ 30 and a button is held
(`Mouse_Akt_Gomb`); `StarmapGfx2D_3` then draws each place's icon `> 0` as brush 9 frame `Konvert[icon]` with its
top-left at `LastMouse + (X2[k], Y2[k])`: relative to the press (−77, −22) left, (−26, 16) below, (25, −22) right,
(−26, −60) above (49 × 49 cells). So the icons appear after 0.3 s of holding and answer from 0.5 s. With an icon
under the pointer the tooltip shows `StarmapRopIkon_Nev[AktRopIkon2]` (`txt17` §6 index k of the Hungarian file;
`GFX_DateMoneyScrollFade` ELF 0x66d160 at the tooltip place).

### 14.5 What the icons do (`Starmap_Main`, `AktRopIkon`)
| icon | frame | does |
|---|---|---|
| 1 | 13 | the tanks button: the vehicle transfer panel (§13.1) |
| 2 | 9 | the production view on/off (§12.1) |
| 3 | 10 | toggles the redirect mode: the next left press, or any right press, on the map picks a planet (`SelectNearestPlanet(…, 1.0)`) and, if it is the player's, sends `0x400(AktPlanet, target)` (0 when it is the planet itself: no rally); the mode ends. The blinking mark at (471, 430) (§11) shows it |
| 4 | 8 | the colony screen of the planet (`AktKepKell` 3) |
| 5, 6 | 1, 2 | `0x18(AktPlanet, 1 or 2)` (satellite, spy satellite), `StarmapSatelliteElkuldve` set until the handler has run |
| 7, 8 | 6, 7 | `0x16(AktPlanet, 2 or 1)` (terraform, colonise), `StarmapKolonizaloElkuldve` set until the handler has run |
| 9 | 5 | the tank info panel (`StarmapTanksInfo`, §20) |
| 0xd | 3 | split: zoom in to 590, `0x14(AktFleet)` unless one is pending (`StarmapTransferOnElkuldve`) |
| 0xe | 4 | the transfer between the selected fleet and the nearest own one (§13.1) |
| 0xf | 0 | the long info (§15; no place offers it, the info box's left click does the same) |
| 0x10 | 11 | SP only: the story fleet `FlottaMozgat`s to a random owned planet (`RndValasztas`) |
| 0x11 | 12 | SP only: `Story_Special(0x11, AktFleet)` and `HajoDelete` of its first design-0x1c ship |
| 0x12 | 14 | `Story_Special(0x12, AktPlanet)` |

The story icons call the sim directly, outside the lockstep queue.

## 15. The planet's long info (`StarmapPlanetInfoMod` exe 0xF16EF7)
The info box (hot-spot 8) left-clicked in planet mode (or `AktRopIkon` 0xf) zooms onto the planet and shows its long
info with the tax, auto-build and priority settings (ELF `Starmap_Main`, `GFX_Starmap` 0x6f36c0, `StarmapGfx2D_1`
0x6dcb30, `StarmapHosszuInfoKiir` 0x708440 / exe 0x5ac7a0).

### 15.1 Entering and leaving
- Enter (the left button's release on hot-spot 8, `Mouse_Felenged_Bal`; both buttons start a rename instead): the
  production view closes, a transfer panel goes (`TransferOff`); `Backup{Zoom, XStart, YStart}` and the star's size
  are kept; the zoom runs `Zoom += (600 − Zoom)/2` per drawn frame up to 599 (a blocking loop of `FRISSITES` +
  `UpdateFrame`) with the pivot moving from the old one towards the planet's (below) in proportion, and the star
  shrinking to nothing (`StarSize = Backup·(1 − progress)`). The right button's release opens the colony screen.
- While on, `GFX_Starmap` pins `Zoom = 599` and the pivot to the orbiting planet, offset towards the screen's left:
  `XStart = planet x − (cα·0.35 − sα·0.12)/1.3`, `YStart = −planet y − (cα·0.12 + sα·0.35)/1.3` (planet = §3.3, α =
  `StarmapRotate`).
- Leave by hot-spot 8 (left button), 9 (the exit button) or 11 (the production button, which then opens the view):
  `Zoom −= (600 − Zoom)/2` per frame down to the backup with the pivot moving back in proportion and the star growing
  again; then the mode ends.
- With the mode on, hot-spots 1..=7 (the console, the minimap) and 0x35 are off; 0x5a..=0x63 are on for the player's
  planet, 100 (the panel, it catches clicks) and 0x6f (the screen) always, 0x65..=0x64 + `+0x114` (one per station).

### 15.2 Pictures
`q = (Zoom − L)/(600 − L)` with `L = (int)max(540, BackupZoom)`; nothing while `Zoom ≤ L`.
- `StarmapGfx2D_1`: the panel `KirakFader(319, 33, 302, 326)` (the `starf` look, §12.2); for the player's planet per
  row r = 0..=2 at y = 288 + 22r: `FadeKerettelKerekit(399, y, 102, 18)` (the value), `(505, y, 32, 18)` ("ALL") and
  for rows 0 and 2 `(541, y, 22, 18)`, `(567, y, 22, 18)` (−, +), all black with transparency `255 − 100q` (fill) and
  `255 − 180q` (frame). `StarmapInfoOffHoltart = (Zoom − BackupZoom)/(600 − BackupZoom)` brings the exit button in
  (§12.2).
- `StarmapGfx2D_3` → `StarmapHosszuInfoKiir(320, 34)`: texts at brightness `KiirStrSzinSzorzo = 80q² + 20` %;
  `GetString(2, 2, n)` labels at x 330 (size 3, len 8, colour 1), values at x 398 (size 3, colour 0x21), a line every
  16 pixels from y = 44:
  1. NAME (n 1): the planet's name, colour 3.
  2. SURFACE (2): `Felszin_Nev[+0x22]`; for the player's planet, a planet with the player's satellite or `DEBUGX`:
     + `" (" + GetBolygoGazdagsag(p) + "% " + Gazdagsag_Nev + ")"`.
  3. EMPIRE (3), owned planets: `Birodalom_Nev[owner]`.
  4. RACE (4): `Faj_Nev[+0x21]` + `" "` for an owned planet (else empty), then for the player's or a satellite's
     planet `"(" + FajBolygoViszony[race][surface] + "% " + FajBolygoViszony_Nev + ")"` (race = the owner's, `EN`'s
     for a free planet).
  5. POPUL (5), owned and (own or satellite): `"\x01%d (%+d)"` of `+0x28`, `+0x2c`.
  6. The player's planet only: MORALE (6) `Moral_Nev[(int)(f32)(+0x30/100)]`; INCOME (7) `"$%d"` of `+0x44`; WORKERS
     (8) `"\x01" + +0x28` and NEEDED (9) at x 460 / `"\x01" + +0x50` at x 528, colour 6 when `+0x28 < +0x50`;
     ENERGY (10) `"\x02" + +0x48`, NEEDED (11) `"\x02" + +0x4c`, colour 6 when `+0x48 < +0x4c`.
  7. TANKS (12), owned and (own or a spy satellite, level ≥ 2): `+0x58` (`KiirInt`, len 8).
  8. SPECIAL (13): the planet's info texts (`InfoString_Get(+0x168 + 2h)`, `infostr.txt`) for h = 1..=5, the next
     non-empty one every 30 `Counter` frames (restarting at 1 for a new planet), colour 6.
  9. PROBLEMS (14, len 9), the player's planet: per problem `StarmapProblemaRendez` keeps (§11), 12 pixels apart from
     the same row + 2: `+0x118 + 4p` / 10000 right-aligned in 2 at x 406 and `"%"` at 419 (colour 6),
     `Problemak_Nev[p]` at 432 (size 2, colour 0x21).
  10. The player's planet: TAX (15), AUTO BUILD (16), PRIORITY (17) at (330, 294 / 316 / 338) size 2; their values
     `AdoMertek_Nev[+0x34]`, `AutoBuild_Nev[+0x6c]`, `BolygoPrioritas_Nev[+0x6d]` field-centred in 16 at x 402, colour
     0x21; per row "ALL" (18) at 506 (len 5, field-centred, colour 2 while the button is held over it, else 1) and for
     rows 0 and 2 `"-"` / `"+"` at 548 / 574, y − 2, size 3.
- The name tables come from the Hungarian `txt17.txt` (`StringReadAll`): `Felszin_Nev[s]` §5 s, `Gazdagsag_Nev` §5 20,
  `FajBolygoViszony_Nev` §5 21, `AdoMertek_Nev[k]` §5 22 + k (8), `AutoBuild_Nev[k]` §5 30 + k (11),
  `BolygoPrioritas_Nev[k]` §5 41 + k (4), `RemoveSpaceBase_Nev[1]` §5 45, `StarmapRopIkon_Nev[k]` §6 k.
- The stations (`StarmapLoadUrallomasok` 0x6d3310, `StarmapForgatEsMozgatUrallomasok` 0x6d2c00) circle the planet as
  their designs' models; their hot-spots 0x65.. are the strips (0, 40 + 30i, 100, 20),
  i = 0..=9 (the design's name as the tooltip).

### 15.3 Input → orders
| Hot-spot | Rectangle | `Starmap_Main` |
|---|---|---|
| 0x5a / 0x5e / 0x60 | (400, 289 / 311 / 333, 100, 16) | the tax / auto-build / priority menu (§16) → `0x19(planet, 8 / 9 / 10, value)` |
| 0x5b / 0x5f / 0x61 | (506, …, 30, 16) | "ALL": `0x19(planet, 5 / 6 / 7, 0)` (copy to every planet of the player it sees) |
| 0x5c / 0x5d | (542 / 568, 289, 20, 16) | tax − / +: `0x19(planet, 1 / 2, 0)` |
| 0x62 / 0x63 | (542 / 568, 333, 20, 16) | priority − / +: `0x19(planet, 3 / 4, 0)` |
| 0x65 + i | (0, 40 + 30i, 100, 20) | the sale menu of station i + 1 (§16) → `0x17(planet, i + 1)` |
| 8 (left), 9, 11 | | leave (§15.1) |

The buttons send on the press; every message carries `AktPlanet`.

## 16. The select menus (`StarmapGfx2D_2` ELF 0x6de1b0 boxes, `StarmapGfx2D_4` 0x6d6b20 texts)
A menu opens on its hot-spot's press at the pointer, follows the held button (the item under the pointer is
`smSelected` 0xE75A68, colour 6, the others 3) and sends on the release when an item is under it. Flags (exe):
`StarmapSelectTax` 0xF1737F, `…AutoBuild` 0xF176E8, `…Priority` 0xF176E9, `…RemoveSpaceBase` 0xF176EA, `…Planet`
0xF176EB; the place `smSelectX/Y` 0x927078/0x92707C (`smSelectChr/Ma/Sz` 0xE75A5C/60/64). Each box is
`FadeKerettelKerekit(x − 6, y − 2, w, h, 0, 10, 0, 60, 0, 5, 0, 30)` (fill (0, 10, 0) at transparency 60, frame
(0, 5, 0) at 30), drawn after the other faders; the texts are size 2.

| menu | place (pointer +) | items at y | box w × h | hit row |
|---|---|---|---|---|
| tax | (12, −20) | `AdoMertek_Nev[0..=7]` at y + 3 + 13i, len 20 | 130 × 108 | `y + 13i ≤ my < y + 13(i + 1)`, `x ≤ mx ≤ x + 120` |
| auto-build | (12, −20) | `AutoBuild_Nev[0..=n]`, n = 6 for race 5 (`PlayerFaj[EN]`), else 8 | 130 × (13(n + 1) + 4) | the same |
| priority | (12, −20) | `BolygoPrioritas_Nev[1..=3]` at y + 13k − 10 | 130 × 43 | `y + 13(k − 1) ≤ my < y + 13k` |
| station sale | (−12, 16), x ≥ 6 | one line: `RemoveSpaceBase_Nev[1]` ("Sell %s. Credits: $%d") with the design's name and `CsokkentettTervAr(EN, d)`, at y + 3, len = its length | (6·len + 10) × 17 | `x ≤ mx ≤ x + 6·len`, `y ≤ my < y + 13` |

The place is taken on the press only. The release sends `0x19(AktPlanet, 8 / 9 / 10, item)`, or
`0x17(AktPlanet, i + 1)` for the sale; nothing without an item.

## 17. The production view's planet list (hot-spot 0x32, `StarmapSelectPlanet`)
Pressing the planet's name plate lists the player's planets (ascending index) to jump to (`Starmap_Main`,
`StarmapGfx2D_2`/`_4`):
- Per planet its name and, for one building a ship (`+0x78 > 0`), `pct + "%-" + design name` with `pct = 100 −
  left·100/time` (`+0xa0`, the design's `+0x20`; 99 at most) padded to two characters with a leading space; `a` = the
  longest name, `b` = the longest such text (`smSelectChr = a + b`).
- `sz = max(1, ⌈n/32⌉)` columns of `ma = ⌈n/sz⌉`; place = pointer + (12, 6) on the press, then x ≥ 6, y ≥ 22,
  `x + 6(a + b + 1)·sz − 3 ≤ 640`, `y + 13·ma + 2 ≤ 480`.
- Box `(x − 6, y − 2, 6(a + b + 1)·sz + 3, 13·ma + 4)` (the menus' look); item i in column `c = i/ma`, row `i % ma`:
  the name at `(x + 6(a + b + 1)c, y + 3 + 13·row)` len a, colour 3 / 6; the production text at
  `x + 6(a + b + 1)c + 2 + 6a`, `y + 4 + 13·row`, len b, size 1, colour 1. Hit: the column's width `6(a + b + 1)` and
  the row.
- Release on a planet: it becomes `AktPlanet`, `StarmapBeZoomol(its star, Zoom)` (flies there at the same zoom),
  `StarmapZoomMode` 0, `StarmapProductPlanetChange` (the view shows its queues).

## 18. Verification
`run_screen.py … --kep 2 --planet P` stages the states in ig2.exe and saves them (`--state-out`): `--rop 0xf` (the
long info), `--rop-menu X,Y` (the radial menu held open at the virtual point), `--menu tax|autobuild|priority|sale:N|
planets@X,Y` and `--mouse X,Y` (the pointer, for the hover).

## 19. The fleet's info panel (`StarmapFleetMode` exe 0xF16EEF, ELF 0x1728948)
The info box (hot-spot 8) left-clicked in fleet mode zooms onto the fleet and lists its ships and vehicles per design
(one left panel of 2 × 4 cells, as §13's), the selected design's turning model and its data, and a sale menu per
cell. ELF `Starmap_Main` 0x6f50a0, `ArrangeFleetInfo` 0x6e5c30 (models, faders; called from `ShowStarmap` 0x6eca50),
`FlottaInfoText2d` 0x6e74b0 (texts, the end of `StarmapGfx2D_3`), `CopyFleets` 0x6e59e0, `InfoBetoltAlap` 0x6e5b20,
`InfoForgatAlap` 0x6d3840, `SellShipMehet` 0x6f40c0, `SellShipMenuSor` 0x6f4220, `SellShipHossz` 0x6f4650. Exe:
`ClickedInfoItem` 0x927124, `ActInfoItem2` 0xE83650, `FlottaInf` 0xE83708 (12-byte rows), `StarmapInfoHanyHely`
0xE8370C, `StarmapSelectDeleteMode` 0xF176EC, the scroll `StarmapTransferKezdet` 0xB9A95C (shared with §13).

### 19.1 Entering and leaving
- `StarmapMode == 2`, not `FlottaTipus::fSpec(AktFleet)`, and hot-spot 8 released with the left button (or
  `AktRopIkon` 0xf): `PlaySoundGlobal(0)`, the vehicle panel (§20) goes, then the first of:
  1. the fleet panel already up: `StarmapKiZoomol` (a left release only), `InfoRemoveAlap`, the mode off (the box
     toggles);
  2. a transfer panel up: `PlaySoundGlobal(0)`, `TransferOff` (no zoom-out, except in vehicle mode);
  3. `EN`'s fleet (or `DEBUGX`): `StarmapBeZoomol(fleet +0x30, +0x34, 590)` unless 2. ran (the zoom is still in), the
     mode on, `Kezdet` 0, the rows rebuilt.
  Both buttons start the rename (§11); the right release splits (§13.1).
- `StarmapBeZoomol` (0x6fe450) is §2.4's non-blocking fly (`PlaySoundGlobal(5)`, 9 halving frames to zoom 590 at the
  fleet); no pivot offset as in §15.
- Leave: hot-spot 9 (`PlaySoundGlobal(0)`, the mode off, `StarmapKiZoomol` with `PlaySoundGlobal(5)`), the info box
  again, the tanks button (icon 1, no zoom-out: the vehicle transfer opens), leaving the starmap. A fleet destroyed
  under the panel isn't checked here.
- With the mode on: hot-spots 1..=7 off while zoomed (`StarmapZoomMode`), 0x35 and the radial menu off.

### 19.2 Rows (`CopyFleets`)
`FlottaInf`: one row per design `(design, count, unit indices)`: the `Hajok` entries of `AktFleet`, then its `Tankok`
entries (a vehicle row counts its entry's `+4`). Sorted (`qsort`, `FlottaRendezo` 0x6e5830, fields from
`Tervek[EN]`): `+0x16` ascending (1 ship, 2 vehicle), non-fighters (`+0x36` bit 0) first, design ascending.
`StarmapAktFlottaValtozott` (set by `HajoAtpakol`, `HajoDelete`, the AI, the tank functions, `MULTIGET_Design` 0x35)
makes `GFX_Starmap` re-run `CopyFleets`: the same designs → only the counts change, else the models reload.

### 19.3 Models
- Cells: per row `TransfAlap[D +0x44]` (`D = Tervek[fleet owner][design]`; the race digit `'0' + D +0x40` capped at
  `'8'` for spacewar hulls < 12; a vehicle hull > 0x12 gets `tankok/<kind><min(owner, 10)>.bmp`), `AddToScene(…,
  0.04)`, camera-fixed at `((k % 2)·129 − 229, (k / 2)·70 − 156, −600)` for `k = i − Kezdet` in 0..8 (the others
  hidden), `RotateObj(π/2, 0, 0)` + `RotateObj2((1, 0, 0), −0.5)`, turning `RotateObj2((0, cos −0.5, sin −0.5),
  (i % 3 + 1)·0.03)` per drawn frame: §13.3's left panel.
- The selected design (`InfoBetoltAlap(item)`: row 0 on load, `ActInfoItem2` on every cell press): the same model at
  scale 0.08, posed every frame by `InfoForgatAlap`: `RotateObj(0, 0, InfoAlapRotation)`, `RotateObj2((1, 0, 0),
  1.3)`, `MoveObj(150 + fifo slide x, −150 + slide y, −600)` (screen ≈ (470, 90)); `InfoAlapRotation += 0.01` per
  100 Hz UI frame (`FRISSIT_Starmap2`, also for §13 and §20). Exe: `InfoForgatAlap` 0x59C780, `InfoAlapRotation`
  0xF158C0, `FlottaInfoTscAlapIdBetoltve` 0xE83701, the slides 0xF1584C / 0xF157D4 (`fifo` 0xF15368). [obs, light
  probe (`screen.js --light-probe` on kep 2; sort record `raw[0..9]` = geometry matrix = rotation·diag(scale)): the
  rotation is `axis(x, 1.3)·create(0, 0, t)` with `t = InfoAlapRotation` (not π − t) exactly at t 1.62/1.64/1.66.]
  `InfoAlapRotation` is reset only in `Starmap_Belep` and steps while a panel is up; it and the cells' turns are in
  step (at t = 1.66 the cells had turned 164.6 frames, so t ↔ round(100·t) − 1 drawn frames). Open: the colony ship's
  cell model (`kolonizalo.xvb`) turns along the model's local x in the exe; the exe's `AddToScene` 0x451720 box after
  `BoxUjraszamol` doesn't explain a y axis; suspect the per-child re-normalisation `boxujraszamol` 0x5ed9f0.
- Lighting (all the panels' models: §13's cells and heads, §19, §20, `gyartas`) [obs: `run_screen.py --kep 2
  --light-probe` on the fleet panel, `_local/traces/light-fleetinfo.jsonl`]: per vertex (`fenyekszamolasa`) by the
  scene's ambient 0x404040 and parallel 0x808080, whose camera-space vector is the starmap camera's rotation times
  (10, 10, −10) (fitted to 5e−4; the console's side lights only `kezel` gets); `gyartas` adds its own ambient
  0x0a0a0a. One rotation chain per model, e.g. an 87-normal model at [0.96, 0.30, 0; 0, 0, −1; −0.30, 0.96, 0].
- Level of detail: the panels' hulls step down the model's LOD chain like `Kocka` (`romb61` in the cells, `romb62` in
  the big preview) and draw the self-illumination pass: the rule is in `re/formats/xv.md` "Level of detail".
- The selection ring around the fleet (`ShowStarmap`) is 75 % transparent (alpha 0x40) while the mode is on and fades
  back over the exit zoom (`(7 − TargetFrame)·0x17 + 0x40`).

### 19.4 Pictures (`ArrangeFleetInfo`, `fifo`, `KirakFader(k, …, 0, 54s, 90s, 100, 0, 60s, 110s, 70)` as §13.3)
| k | rect | |
|---|---|---|
| 0 | 28, 27, 273, 334 | the list |
| 1 | 338, 186, 273, 175 | the data |
| 2 | 338, 27, 273, 152 | the model |
| 3 | 30, 29, 269, 17 | the list's title bar |
| 4 | 30, 342, 269, 17 | the list's bottom bar |
| 5 | 340, 188, 269, 17 | the data's title bar |
| 6 | 306, 29, 28, 333 and 310, top + 34, 20, h − 8 | the scroll bar and its thumb (§13.3) |

The selected cell: `DesignKeret1(39 + c·129, 121 + r·70, 120, 13, 0x22)` (the morphing frame).
The exit button (hot-spot 9) is §12.2's: `StarmapGfx2D_2`'s `starf` fader `((1 − sin(h·π/2))·200 + 445, 380, addo +
176, 30)` with `h = StarmapInfoOffHoltart = (Zoom − OffLastZoom)/(TargetZoom − OffLastZoom)` and `addo` stepping to
−10, its text `GetString(2, 4, 4)` ("OFF") at `((1 − sin)·200 + 475, 390)` len 13 size 3 flags 0xd colour 0x25.

### 19.5 Texts (`FlottaInfoText2d`; `KiirStr(text, x, y, len, size, flags, colour)`, flags 7 centred, 4 left)
- `ClickedInfoItem` (−1 = none, reset at the end) makes `ActInfoItem = Kezdet + Clicked` when below n.
- The fleet's name at (156, 32) len 19 size 3 flags 7 colour 3.
- Per cell (item i, `x = 91 + 129c`, colour 2 for `ActInfoItem`, else 1): the design's name (+ `" (" + GetString(2, 4,
  1) + ")"` with an upgrade target `+0x1c > 0`) at (x, 124 + 70r) len 25 size 2 flags 7; the count at (x, 112 + 70r)
  len 14.
- `ShowFlottaTankCap(110, 347, AktFleet)` (§13.3).
- The data (`D` = the `ActInfoItem` row's design, of the fleet's owner):
  - the slots `p = 1..=GetMaxPodok(D +0x40, D +0x44)` at (485, 200 + 12p): `D +0x50 + p == 0` → `GetString(2, 5, 5)`
    (len 0, flags 4, colour 2); else the tech name `Talalmanyok[D +0x50 + p]` (len 16, flags 4, colour 3) and its
    count `D +0x5d + p` at `599 − 3·digits` (len 2, flags 4): `"-"` colour 6 for 0, else colour 3 below
    `min(GetMaxRarakhato(D +0x40, D +0x44, p), tech +0x24)`, 0x22 at it;
  - the header: the name (+ `" (" + GetString(2, 4, 5) + ")"` with an upgrade target) at (472, 193) len 31 size 2
    flags 7 colour 2;
  - labels colour 1 size 2 len 16: `GetString(2, 5, 6)` at (420, 212) flags 7; `GetString(2, 5, 7..=9)` at (340, 248 /
    260 / 272) flags 4; `GetString(2, 5, 10)` at (340, 284) only for a ship (`+0x16 == 1`) without the fighter bit;
  - values colour 2 size 2 flags 7: `Talalmanyok[D +0x48]` (the hull tech) at (400, 224) len 16; `D +0x30` at (441,
    248), `D +0x2c` at (441, 260) len 5; at (441, 272) `(int)((f64)(D +0x3c ·f32 100) + 0.5)` (`mulss`, `cvtss2sd`,
    `addsd`, `cvttsd2si` ELF 0x6e8557..0x6e8564), signed `/ 4` when `D +0x38` bit 0; `D +0x28` (byte) at (441, 284)
    under label 10's condition.

### 19.6 Hot-spots (`Starmap_MouseInit2`; rectangles `Starmap_MouseInit` 0x6feb70)
| id | rect | when | does |
|---|---|---|---|
| 8 | 0, 424, 166, 56 | always | §19.1 |
| 9 | 445, 380, 164, 60 | always | close (tooltip `GetString(2, 1, 0x26)`) |
| 0x43 + 2k, k < min(n, 8) (not less `Kezdet`: a scrolled list's empty cell still answers, with the old `ActInfoItem2`) | k even (28, 60 + 70⌊k/2⌋, 124, 70), odd (152, …, 139, 70) | | the cell of item `Kezdet + k` (tooltip `GetString(2, 1, 0x27)`) |
| 0x57 | 307, 30, 26, 329 | | the scroll bar (§13.4's formula) |
| 0x58 | 24, 24, 313, 339 | | catches the clicks under the panel |

### 19.7 The sale menu (`StarmapSelectDeleteMode`)
- A cell hot-spot under a held button (`Mouse_Lenyom`, or the menu open): `ClickedInfoItem = k`, `ActInfoItem2 =
  Kezdet + k` when below n, `InfoBetoltAlap(ActInfoItem2)`, `PlaySoundGlobal(0)` on the press. Unless
  `TutorialDisableHajokEladasa` (exe 0xF66FFD), for `EN`'s fleet (or `DEBUGX`), while held: the menu (§16's look),
  `n = 1` row, or 2 with an upgrade target; on its first frame `smSelectX = Mouse_x + 14`, `smSelectY = Mouse_y − 6`,
  then `smSelectX = 636 − 6·len` when `smSelectX + 6·len + 4 > 639` (`len = SellShipHossz()`, the longest row);
  `smSelected` = the row `k` with `smSelectX ≤ mx ≤ smSelectX + 6·len`, `smSelectY + 13(k − 1) ≤ my < smSelectY + 13k`.
- Box `FadeKerettelKerekit(smSelectX − 6, smSelectY − 2, 6·len + 10, 13n + 4, …)` (§16); row c at `(smSelectX,
  smSelectY + 13c − 10)` size 2 flags 4, colour 6 hovered, 1 when `!SellShipMehet(c)`, else 3. Texts
  (`DesignTorlesValasztasNev[0..4]` = `GetString(0x11, 3, 0x1c..=0x1f)`, the number appended by `CString::operator+` after the text's own `- $`): row 1
  `Name[0]` with `count·CsokkentettTervAr(owner, d)` (`GetHosElkuldesStr` instead when `GetTervEladasOszto < 0`,
  `Name[3]` when the vehicles wouldn't fit); row 2 `Name[1]` (`Name[2]` when unaffordable) with `cost = max(0,
  (count·Tervek[owner][D +0x1c] +0x24 − TervHajoinakAra(owner, fleet, d))·2)`.
- `SellShipMehet(1)`: `count·D +0x28 ≤ fleet +0x78 − fleet +0x5c`; `(2)`: `cost < 1` or `cost ≤ MoneyMennyi(EN, 0)`.
- `GetHosElkuldesStr(owner, d, SellShipMehet(1))` (ELF 0x73e960): for `EN`'s design with story key (`+0x80`) / 10 = 180
  `_s(5, 0x70a)` ("Send back to academy"; 0x70b "Unable to …" when it doesn't fit), / 10 = 235 `_s(5, 0x930)`
  ("Unable to sell"); else `_s(5, 0x6e2)` ("Terminate contract"; 0x6e3 "Unable to …").
- The release on a row it allows: `GetTervEladasOszto < 0` → `Story_Special(0x19, AktFleet, d)` (SP; no message);
  row 1 → `0x34(d, 2, AktFleet)`; row 2 → `0x35(d, AktFleet)` (`MULTIGET_Design` ELF 0x50c430: 0x34 sells the fleet's
  units of the design for `TervHajoinakCsokkentettAra`, logs the design's retirement, offers a copy on the market,
  `TervDelete`; 0x35 refits them to the upgrade target for the cost; `re/rules/design-orders.md`). The menu closes. No
  sound.

### 19.8 Sounds
`PlaySoundGlobal(0)`: opening, closing (8, 9), a cell press; `PlaySoundGlobal(5)`: the zooms in and out. [obs,
`tools/trace/scripts/fleetinfo.json` with `--fleets 2 --rop-fleet 1`: the info box's release Global[0] + Global[5]
(Click1, benyilas1), a cell press Global[0], the exit button Global[0] + Global[5].] The vehicle panel's
(`PlaySoundGlobal(0)` on opening and a cell press, none on hot-spot 9) are from the decompile.

## 20. The vehicle info panel (`StarmapTanksInfo` exe 0xF176ED, ELF 0x1728e98)
The vehicles on the player's planet: up to 8 designs per page with their counts, the selected one's model and data, a
sale menu. ELF `StarmapTanksInfoSetup` 0x6ec7d0 (every `GFX_Starmap`), `SetTanksInfoObjects` 0x6ebbb0,
`SetSpecificMeshId` 0x6ec190, `StarmapTanksInfoFaderKirakas` 0x6ea720 (from `ShowStarmap`),
`StarmapTanksInfoSzovegKiirasok` 0x6ec6a0, `DrawStarmapTankInfoSzoveg` 0x6eb0f0, `DrawSpecificInfoOnTank` 0x6eb210,
`GetNthTank` 0x6ea600, `SellTankMehet` 0x6f4800, `SellTankMenuSor` 0x6f4910, `SellTankHossz` 0x6f4d60. Exe:
`StarmapTanksInfoFirst/Selected/HanyItemVan` 0xE836C0/C4/C8, `StarmapSelectTankDeleteMode` 0xE75A70.

### 20.1 Opening, closing, scrolling
- `AktRopIkon == 9` (the radial icon on the player's planet, §14.3): `PlaySoundGlobal(0)`, the panel on, First,
  Selected and the count 0. No zoom.
- Closed by hot-spot 9 (no sound), the tanks / production buttons (icons 1, 2, with `PlaySoundGlobal(0)`; then the
  transfer panel or the production view opens), icon 0xe, the info box (the long info / the fleet panel), leaving the
  starmap. With the panel up the radial menu offers nothing and 0x35 is off; hot-spots 1..=7 stay on.
- The list: the `Tankok` entries held by `−AktPlanet`, in table order (`GetNthTank(n, holder)`: the n-th used one,
  1-based); n = how many from `First + 1` on (`StarmapTanksInfoSetup`, every frame).
- Scroll: hot-spot 0x57 only, §13.4's formula (2632/n and 329; the thumb 2648/n and 331). Nothing re-clamps First or
  Selected after a sale.

### 20.2 Models
Per cell i the hull `m = D +0x44 − 0x13` (`D = Tervek[planet owner][design]`): `TankAlap[m]` (`vector/colony/`
kerekes, kozepes, lebego, lanctalp, aknarako) with sub-mesh 0 skinned `textures/colony/tankok/{kerekes, kozep, lebeg,
lanc, aknarak}<min(EN, 10)>.bmp`, `AddToScene(…, 0.04)` at `((i % 2)·129 − 229, (i / 2)·70 − 156, −600)`, posed and
turning as §19.3's cells; the selected one (`Selected − First` in 0..8) also at scale 0.1, `MoveObj(150, −150, −600)`,
`RotateObj2((1, 0, 0), −0.5)` once, then `RotateObj2((0, cos −0.5, sin −0.5), 0.03)` per frame (not
`InfoForgatAlap`'s pose). The preview (`SetSpecificMeshId` ELF 0x6ec190) carries cell 0's rotation chain exactly
[obs, `light-tanks.jsonl`]: its apparent pitch is the spin phase (tilted axis), and §13.3's phase rule holds (n − 1
frames after the press).

### 20.3 Pictures and texts
- Faders (`StarmapTanksInfoFaderKirakas`, §13.3's call): (28, 27, 273, 334), (338, 186, 273, 175), (338, 27, 273,
  152), the title bars (30, 29, 269, 17) and (340, 188, 269, 17), the scroll bar and thumb (§19.4); no bottom bar. The
  exit button as §19.4 without the slide (`starf` (445, 380, 176, 30)) and the same text.
- Texts: the planet's name at (156, 32) len 19 size 3 flags 7 colour 3; per cell (colour 2 when `i == Selected −
  First`, else 1) the design's name at (91 + 129c, 124 + 70r) len 25 and the count at (116 + 129c, 114 + 70r) len 4,
  size 2 flags 7 (no frame, no upgrade suffix).
- The selected one's data (`DrawSpecificInfoOnTank`): the slots as §19.5; the header at (472, 193) the same; labels
  `GetString(2, 5, 6)` at (420, 212), `GetString(2, 5, 7..=10)` at (385, 248 / 260 / 272 / 284), len 15, flags 7
  (centred), colour 1, label 10 under §19.5's condition; values colour 2: the hull tech at (400, 224) len 16, `D
  +0x30`, `D +0x2c`, `(int)D +0x3c` (truncated, not ×100) and `D +0x28` at x 435, len 5.

### 20.4 Hot-spots and the sale menu
- Hot-spots 8, 9 (tooltip `GetString(2, 1, 0x28)`), the tanks / production buttons (10, 11), the cells 0x43 + 2k as
  §19.6, 0x57, 0x58 (24, 24, 313, 339).
- A cell under a held button: `Selected = First + k` when below n, `PlaySoundGlobal(0)` on the press; on the player's
  planet (or `DEBUGX`) §19.7's menu with `SellTankHossz` / `SellTankMenuSor(r, owner, design)`: row 1 `Name[0]` with
  `count·CsokkentettTervAr` (`GetString(2, 4, 7)` "Unable to sell" when it can't be sold), row 2 (with an upgrade
  target) as §19.7 over the planet's units; `SellTankMehet(1)` is always true. The release sends `0x34(d, 2,
  −AktPlanet)` or `0x35(d, −AktPlanet)` when `GetTervEladasOszto ≥ 0` (else nothing); the sim side is `re/rules/design-orders.md`.

### 20.5 Verification
`run_screen.py … --kep 2 --planet P --rop 9` (the vehicle panel) and `--fleets 1 --rop-fleet 1 --rop 0xf` (the fleet
panel) open them in ig2.exe through `AktRopIkon`.

## Client status
Modules (`engine/crates/ig2-client/src/`): `starmap.rs` (camera, keys, background, routes, fleet sprites),
`starmap_planet.rs` (planets, clouds), `starmap_radar.rs` (§6.3), `starmap_hud.rs` (console, side lights, button
animation), `starmap_panels.rs` (§12, §13), `starmap_menus.rs` (radial menu, long info, select menus §16, planet list
§17, rename), `starmap_info.rs` (fleet panel §19, vehicle panel §20, sale menus), `starmap_tip.rs` (tooltips),
`topbar_menus.rs`. Orders: `ig2_sim` `StarmapOrder` / `Game::starmap_order`, `ig2_net::Command::Starmap`; §10.5's
replay is byte-exact. Camera steps, background particles and radar dots are bit-exact (tests
`camera_steps_match_the_exe_bits`, `the_background_matches_the_exes_records`, `quick1_rings_match_the_exes_particles`).
Missing: the radial menu's story icons 0x10, 0x11, 0x12 (story wormhole, virus alert, gigabomb) do nothing, and 0x10 /
0x12 are not offered (`rop_places`); planets and clouds always at the last LOD level, no per-mesh light cache (§3.3);
the production view's sinking buttons and wireframe twins (§12.2); the long info's stations (§15.2); an emptied fleet
always goes by 0x1b, also in SP (§13.1). Open deviations: 47, 317.
Captures: `run_screen.py … --kep 2 --planet P` with the flags of §11.1, §13.5, §18, §20.5 against `ig2-client
--snapshot` (`--panel`, `--info-panel`, `--rop-menu`, `--planet-info`, `--menu`, `--mouse`); unsaved UI clocks go over
as `--clouds`, `--futofeny`, `--acttime`, `--spin-frames`, `--info-alap`; scores in re/screens/diff-results.md.
