# The starmap screen: camera and drawing

How the original draws the starmap (screen `AktKep == 2`). Evidence: the ELF oracle (`ig2`, symbols) decompiled with
`tools/ghidra/DecompileFunctions.java`; addresses are ELF unless marked exe. The exe was checked where values matter
(palette, saved camera). `[inf]` marks inference.

Scope: the 3D view, systems, planets, fleets, routes, selection, background, colours, the minimap, the orders the
screen sends and the lockstep handler that carries them out (§10), the control panel and the short info (§11), the
production view (§12) and the transfer panel (§13). The names (§3.5, §4.2) and the top bar are in re/screens/hud.md
§2, §4; what is left is in §9.

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
The select menus of `StarmapGfx2D_4` (tax, autobuild, priority, station sale, the production view's planet list),
the fleet and vehicle info panels (`StarmapFleetMode`, `StarmapTanksInfo`: the info box's left click), the planet's
long info (`StarmapHosszuInfoKiir`), the radial "rop" icon menu (`StarmapRopIkonok`, brush 9 around the cursor),
`StarmapKeyboardHandler`'s keys beyond the camera, the console's own buttons (hot-spots 4..6: side view, zoom and
pitch while held), radar-range rings (`rrange.xvb`), wormhole particles, `AddColor`, cloud drift.

## 10. Input → orders (lockstep messages)
The screen never changes the game state itself: every order goes out as `Multi_Send(type, EN, …)` and is carried
out by **`MULTIGET_Starmap`** (exe 0x59d930, ELF 0x719680) when the message comes back, on every machine, in the
next frame's message pass (also in single player, also while paused). The handler drains the types in ranges
(`Multi_GetNextTipus(lo, hi)`): 1..9, 0x14..0x1d, 0x3fc..0x40f, 2000; then fixes the selection (§10.4).
`Multi_Get(type, &issuer, marker, &v1, marker, &v2, …, 0)`: the values below follow the issuer. The port:
`ig2_sim::game::StarmapOrder` + `Game::starmap_order(issuer, o)` (`game/orders.rs`), `StarmapOrder::from_message`;
in the lockstep protocol `ig2_net::Command::Starmap { order: StarmapCmd }`.

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
| 0x1c | from, to, design, ship, count | Both holders the issuer's (fleet > 0, planet −p). Ships of `from` with that design (only ship `ship` when non-zero) move to `to` (`HajoAtpakol`) while `count` lasts; for a vehicle design (`+0x16 == 2`) into a fleet the count is capped by its room (`+0x78 − +0x5c`); the first vehicle entry of `from`/design moves `min(count, entry)` (`TankAtpakol`); then the ground vehicles a fleet `from` can no longer carry (`+0x5c − +0x78`) go to `to`, each entry capped by `to`'s room. A ship onto a planet makes the exe write outside `Flottak` (the screen never sends it; the port skips it) | transfer panel |
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
Port: `StarmapEvent::FleetEmptied` (`StarmapContext::sends`, `transfer`).

### 10.3 Mouse (`Starmap_Main` ELF 0x6f50a0)
- Picks: `SelectNearestUnit(sx, sy, r, exclude, &kind, skip_selected, enemies)` (ELF 0x6e46b0): fleets the human sees
  (`+0x19 + EN` bit 0 and `+0x18` bit 0 clear, or always visible) and planets it knows (`+0xd8 + 2EN > 0`) whose
  projection is within `max(225, (Zoom/20)²)·r²` squared virtual pixels (`Tavolsag` 0x6e45f0), at most 10, ranked
  fleet `id + ships·10000 + own·10⁸`, planet 1; with `enemies` only others' fleets without flag 0x200 and owned
  others' planets without `+0xbc` bit 0. The pick is the highest rank below `exclude`'s (a fleet's rank, 1 for a
  planet −p, unless `exclude` is out of reach), else the highest within 1.1 × the nearest distance: clicking again
  cycles through a stack.
- Left click (on release): `SelectNearestUnit(…, 1.0, AktFleet or −AktPlanet, false, false)` selects
  (`StarmapMode` 1 planet / 2 fleet, `AktPlanet`/`AktFleet`).
- Right click with the human's fleet selected (or story fleet 0x1f88): Ctrl = attack mode; the pick at r = 0.5
  (skipping the fleet itself, its target excluded); Alt = no pick. A hostile pick (owned by someone other than the
  human and the fleet's owner; fleets without 0x200, planets without `+0xbc` bit 0) turns a move into an attack (2).
  The order is sent unless the fleet is in battle (`+0x52` bit 0) or the move is onto itself:
  `0x15(fleet, mode, kind, (f32)target, 0)`, or for no pick (or Shift) a point: `get_pos` (the galaxy point under
  the cursor, `SikMetszespont` on h = 0) pulled within 600 of the centre, mode 13 (append a waypoint) with Shift,
  14 (patrol) when the click is on the fleet itself (`Tavolsag < max(225, (Zoom/20)²)/4`).
- Minimap (hot-spot 7, `Mouse_Szoveg_Set(7, …, 261, 367, 118, 83)`): the view's target pivot
  `XStart = −((x − 274)/92 − 0.5)·600`, `YStart = −((y − 388)/65 − 0.5)·600` (±299.9), tweened over 4 frames.

### 10.4 The selection fix-up
After every message pass: with `StarmapMode` 2 and `AktFleet` gone (owner 0xFF) or no longer seen (`+0x19 + EN`
bit 0 clear or `+0x18` bit 0, and not always visible, not `DEBUGX`), unless it has flag 0x400: the first own fleet
with `+0x18` bit 0 clear, else the first own one, else the first seen one; none → `StarmapMode` 1. Port: the client
(`World::fix_selection`).

### 10.5 Verification
`run_replay.py --orders N --orders-every T` stages N random messages of every type before each T-th tick's save
through the exe's own handler (its `Multi_GetNextTipus`/`Multi_Get` calls pointed at stubs, libc pinned per batch)
and the save replay applies them with `Game::starmap_order`: 760 messages over 3 recordings (220 ticks),
byte-exact (`rules/traces/2026-10-17-starmap-orders.md`).

## 11. The HUD (`StarmapGfx2D_1`, `_3`, the control panel)
Drawn in the original's order: the 3D pass with the control panel, then the 2D passes.
- The control panel `kezel.xvb` (`Init3dStarmap`: `AddToScene(…, 0.17)`, `SetRelative(false)`, `MoveObj(0, 170,
  −600)`, not rotated): a camera-fixed model with half-extents 1024, so a vertex `v` sits at `174.08·v + (0, 170,
  −600)` in camera space, over virtual 214..426 × 366..474. Texture `textures/starmap/starpan`; no faces over its
  window. Lights: the scene's ambient 0x404040 and parallel 0x808080, plus two parallel lights of its own
  (`smOldalsoFenyekBe` ELF 0x6d81e0: colours 0x7f5d00 and 0x004e7f) that `smMozgatMainFenyek` circles in front of
  it (`(sin·300, cos·200, 0)` of `smkeringeto·20`). `KezeloGombFrame`/`MelyikGombot` animate its buttons. [The port
  renders it once with one warm front light.]
- `StarmapGfx2D_1` (ELF 0x6dcb30): `KirakFader(…, 5, 423, 162, 52, 0, 54s, 90s, 155l + 100, 0, 90s, 130s, 185l + 70)`
  behind the short info (`FadeKerettelKerekit`: rounded fill, then frame; s = 1, l = 0 at rest: fill (0, 54, 90) at
  opacity 100/255, frame (0, 90, 130) at 70/255 [obs: exe pixels]), then `FadeScreen(271, 385, 98, 71, black,
  0x40)` over the minimap, 0x40 being the transparency (75 % black [obs]).
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
  - Open: the planet picture left of the text in the GOG build (between the two icon lines, virtual ≈ 6..38 ×
    434..466) isn't drawn by these passes (a remaster addition, not found).

### 11.1 Reference captures
`run_screen.py <install> quick-1.sav <out> --kep 2 --planet 71` (the tool sets `AktPlanetKell`, so `--planet` picks
the shown planet) against `ig2-client --snapshot … --size 800x600 --paused` [obs, 2026-10-17, `ig2-screendiff` mean
difference per pixel]: top bar 5.7, the production button 7.6, the minimap 8.0, the panel 9.8; the short info box
14.5 (the missing planet picture, square corners). The texts, colours and places agree; the rest is the background
clouds behind the translucent parts.

## 12. The production view
The selected planet's two build queues over the starmap (ELF `StarmapGfx2D_2` 0x6de1b0, `StarmapGfx2D_3` 0x6d4a30 /
exe 0x5aaba0, `ProductGFX` 0x6e8870, `ShowStarmap` 0x6eca50, `Starmap_Main` 0x6f50a0 / exe 0x5b2b70, hot-spots
`Starmap_MouseInit` exe 0x5b99d0 and `Starmap_MouseInit2` 0x7024b0). Port: `ig2-client::starmap_panels`.

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
  to 0 over frames 19..30 (node keys, `Kocka`; the client poses the model with `scene::Model::load_at`); group 50 is
  the name plate. `GetProductionPanelSubMeshIDs` 0x6d4010 maps slot `r = k + 4L` to its minus, plus, "into a fleet"
  and repeat meshes (`Group%02d`: r0 9/8/11/10, r1 17/16/19/18, r2 25/24/27/26, r3 33/32/35/34, r4 13/12/15/14,
  r5 21/20/23/22, r6 29/28/31/30, r7 37/36/39/38). `ProductGFX` textures a repeat lamp `gyart_gr_3` when
  `+0x90 + 4L + k` is set, else `gyart_gr`, and line 1's "into a fleet" lamps `gyart_gr_3` / `gyart_gr_2`
  (`+0xad + 4 + k`). A pressed button sinks 40 units (`ProductionButtons` 0x6ea3d0 / `…2` 0x6ea450 around the
  render; not drawn by the port).
- The head units (`ProductGFX`): the ship line's head design's hull model `TransfAlap[+0x44]` (exe 0x927410, 25 ×
  0x38: name, `+0x34` 0/2 `talalmanyok`, 1 `spacewar`, 3 `colony`; a spacewar hull < 12 gets
  `'0' + PlayerFaj[design +0x40]` (at most '8') as its name's 5th character) at (−540, −350, −1600) ×0.375, the
  vehicle line's at (540, −350, −1600) ×0.28 (race digit of `EN`; a ground hull > 0x12 gets
  `tankok/<kerekes|kozep|lebeg|lanc|aknarak><owner ≤ 10>.bmp` on its first sub-mesh); both `RotateObj(π/2, 0, 0)` +
  `RotateObj2((1, 0, 0), −0.5)`, then `RotateObj2((0, cos −0.5, sin −0.5), 0.05)` per frame,
  `SetObjTransparency(1 − p)` with `p` = the bar's fraction clamped to 0.05..0.95, plus a wireframe twin at `p` (not
  drawn by the port). `RotateObj2(axis, θ)` makes `R = creatematrixfromaxis(axis, θ)·R` (0x4520a0: the quaternion
  `(cos θ/2, sin θ/2·axis)`).
- Faders (`StarmapGfx2D_2`; `KirakFader` → `FadeKerettelKerekit` 0x538330 → `FadeKerekit`/`FadeKeretKerekit` →
  `FadeScreen` 0x537890, which queues `0xTTBBGGRR`: **the fourth colour value is a transparency**, the colour is
  blended at opacity `(255 − t)/255` [obs: the panels' edges in the exe capture give 0.22 for t = 200, the transfer
  faders over black match]; `s` = the fader's `+0x168 + 4i` = 0.9 (`FaderDesignFrissito` 0x547e10; more only with
  `GLOBALKELLDESIGN`)): when up, `FadeKerettelKerekit(5, 80, 190, 292, 0, 20, 20, 200, 0, 15, 15, 150)` and
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
| 0x32 | (276, 32, 87, 47) | the list of `EN`'s planets (`StarmapSelectPlanet`, not ported) |
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
`StarmapTransferHanyHely` 0xB9A958, `StarmapTransferKezdet` 0xB9A95C). Port: `ig2-client::starmap_panels`; the sim
reads the mode and the fleets for its 0x1b (§10.2, `Game::set_transfer`).

### 13.1 Opening and closing
- Split (0x14; the info box right-clicked on `EN`'s fleet not in battle, or the radial icon 0xd): the handler
  selects the new fleet and opens fleet 1 = the old, fleet 2 = the new (`MULTIGET_Starmap`, §10.1); the click zooms
  in to 590 (`StarmapBeZoomol`) unless it closed a panel.
- `AktRopIkon == 1` (the tanks button, hot-spot 10 at (174, 432, 44, 44), shown when `StarmapTanksGomb`: planet mode
  on `EN`'s unprotected planet with `SearchBolygohozKozeliFlotta(p, 20, true, false, false, true, 0)` > 0, or `EN`'s
  fleet not in battle with `SearchFlottahozKozeliBolygo(f, 20, true, true)` an `EN` planet): vehicle mode between
  the fleet (fleet 1) and the planet (−p), zoomed in to 590, `0x15(fleet 1, 0, 0, 0, 0)` stops the fleet. Pressed
  with a vehicle panel up it closes it; a fleet-to-fleet panel closes and the vehicle panel opens.
- `AktRopIkon == 0xe` (the radial icon; the port's key T): fleet 1 = `StarmapAktFleethezKozeliSajatFlotta` (`EN`'s
  fleet nearest to `AktFleet`, not in battle, `(int)` Manhattan distance < 20), fleet 2 = `AktFleet`, both stopped
  (0x15), zoomed in.
- Closed by hot-spot 9, the info box, the production button, a planet turning protected (`+0x64` bit 0,
  `FRISSIT_Starmap2`): `StarmapKiZoomol` + `TransferOff` 0x6f2810 (an emptied `AktFleet` hands the selection to the
  other fleet; in single player the player's emptied fleets of the panel are deleted at once with `FlottaDelete`,
  multiplayer leaves it to 0x1b; the port always does the latter).

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
  transparent.
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
writes `AktRopIkon` 0xE83654 right after `Starmap_Main` clears it) and save the moment; `run_screendiff.py …
production | transfer` compares `ig2-client --snapshot --panel production | transfer:9,10`
(re/screens/diff-results.md). The client test `starmap_panels::panel_clicks_reach_the_sim` drives the hot-spots on
`quick-1.sav`: a design picked from the list, its count raised, the unit finished (0x40f), the fleet split, its
ship moved across, the emptied fleet kept while the panel is up.
