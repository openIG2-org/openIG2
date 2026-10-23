# Colony-map building placement (`LocateSuitableLocation`)

Status: **verified at runtime** (§7); ELF -O0 and the shipped exe agree on every step below.
Port: `engine/crates/ig2-sim/src/placement.rs`. No rule values here; inline code constants are listed.

Evidence (ELF `ig2` / exe):
| Function | ELF | exe | Role |
|---|---|---|---|
| `EpuletNewSearchLocation(p, kind, done, dmg%, x, y)` | 0x61ca80 | 0x53cca0 | search, then `EpuletNew(p, kind, done, (f32)x, (f32)y, dmg%)` on success, else 0 |
| `LocateSuitableLocation(p, kind, &x, &y)` | 0x4bc4a0 | 0x4869d0 | builds the occupancy grid, picks the start, runs `LocatePos` |
| `Decode2LargeMap(map, grid, landing, p)` | 0x4bc040 | 0x46d8a0 | terrain mask → grid (`Set4x4Block` 0x4bbfd0 inlined in the exe) |
| `DecodedAllocate(x, y, w, h, ring)` | 0x4bbd10 | 0x46da50 | stamps a footprint |
| `DecodedCheckAllocation(x, y, w, h)` | 0x4bbb40 | 0x46db30 | "is this spot taken?" (passed to `LocatePos` as a pointer, `push 0x46db30` @ 0x486b76) |
| `CheckKeretAllocation(grid, x, y, w, h)` | 0x4bb8e0 | 0x460800 | frame test (keret = frame) |
| `LocatePos(check, cx, cy, &x, &y, w, h)` | 0x4885c0 | 0x486610 | square-spiral search |
| `EpuleteketJoHelyreTesz()` | 0x61cd10 | 0x53cd00 | re-places every building (load fix-up, §6) |

Callers: every scripted/automatic new building goes through `EpuletNewSearchLocation(p, kind, done, dmg, −1, −1)`
(colonisation's colony hub, autobuild, AI build orders, story/scenario buildings; ~40 call sites in the ELF). The exe
calls `LocateSuitableLocation` from three places: `EpuletNewSearchLocation`, `EpuleteketJoHelyreTesz` (§6) and a story
helper 0x5c95b0 (`Story_Check`, `st_Elem`).
The human's own placement in the colony screen: the build cursor's search (§8) over the screen's grid, then the order 0x1f
(`re/screens/colony.md` §9) runs `EpuletNewSearchLocation(p, t, false, 0, x, y)` from the cursor's spot (§3).

## 1. Data
- **Grid** `DecodedAlloc` (ELF 0x11e99a0, exe 0x9a1ea0): `u8[200 × 200]`, index `y × 200 + x`. Values:
  bit-pattern `0x01` = blocked (terrain, footprint interior, vehicle), multiples of `0x20` = frame
  rings of placed buildings (added, so they wrap after 8 overlaps).
- **Terrain** `Terepek[m]` (ELF stride 0x2720 / exe 0x2714, exe pointer 0x99825c): mask A (`colony-map.md`,
  ELF +0xae / exe +0xa6, 50 rows × 13 bytes, 2 bits per cell) and the **AI build start**
  (ELF +0x5c4/+0x5c8, exe +0x5bc/+0x5c0: two `int`s, the 10th positional value of the `[felszin]`
  section `m` of `colony/terepleiro.txt`, commented "Az MI itt kezd el epiteni"; 100,100 in all 75 shipped).
  `m` = planet `+0x23` (the colony map, `colony/gen_maps/<m>.map`).
- **Footprint** of building kind `k` (1-based): `Building[k−1]` (ELF .data 0x1164980, 60 × 0x70; exe
  0x8F1F28, 60 × **0x6c**): `+0x58 i32` width, `+0x5c i32` height, in grid cells (`+0` is the model name).
  Read by `ig2-rules` (`building_footprints`).
- **Buildings** of planet `p`: slots 1..=`HanyEpuletLefoglalva[p]` (`colony-economy.md` §2), kind `+0`
  (0 = empty), position `+0xc/+0x10` f32.
- **Vehicles** (ground war, `Warfare[p]`; exe stride 0x8ec from 0x9c33c4 = count, `+8` used flags,
  0x9c3574 + 0x8ec·p unit pointers): a used unit whose `+0x188` has neither bit 1 nor bit 30 and whose
  `+0x18c` lacks bit 2 (alive, on the map) blocks its cell `(+0x1a0, +0x1a4)` (ints).

## 2. Building the grid (`LocateSuitableLocation`, first part)
1. `Decode2LargeMap(m, grid, false, p)`: for mask row `r` 0..50 and cell `c` 0..50 (columns 50–51 of
   the 13-byte row are skipped), the 4 × 4 block `x ∈ 4c..4c+4, y ∈ 4r..4r+4` = `(cell != 0) as u8`
   (written, not OR-ed). The landing-zone branch (`landing = true`: every cell whose `LandingZones[p − 1]` level is
   above 0 becomes 1, the zone generated first if needed) is `LocateTankSuitablePosition`'s (exe 0x486bd3), ported as
   `placement::{landing_zone, locate_tank}` and verified with the real-time ground war (`ground-combat.md` §11.1,
   `traces/2026-10-16-ground-realtime.md`); `Init3dColony` (0x47e6e4) is the colony screen's.
2. Every building slot `j` in 1..=n with kind `k ≠ 0`, in slot order: `DecodedAllocate((int)x_j, (int)y_j, w_k, h_k, ring = true)`;
   `n += 1`; `sx = (int)((f32)sx + x_j)`, `sy = (int)((f32)sy + y_j)` (an int accumulator re-truncated after every
   f32 add: `cvtsi2ss`/`addss`/`cvttss2si`).
3. Every vehicle (§1): `DecodedAllocate(vx, vy, 1, 1, ring = false)`.

`DecodedAllocate(x, y, w, h, ring)` (all divisions C-truncating): only if `0 ≤ x < 200` and `0 ≤ y < 200`.
```
x0 = x − w/2 − 1,  x1 = x + w/2 + w%2         (inclusive)
y0 = y − h/2 − h%2 − 1,  y1 = y + h/2          (inclusive; note the odd extra row goes to the low side in y, the high side in x)
cell on the border (ix ∈ {x0, x1} or iy ∈ {y0, y1}): ring ? grid += 0x20 (u8 wrap) : unchanged
interior: grid = 1
```
So a footprint at `(x, y)` covers `x − w/2 ..= x + w/2 + w%2 − 1` × `y − h/2 − h%2 ..= y + h/2 − 1`: odd
widths lean to high x, odd heights to low y (a 1 × 1 vehicle or building at `(x, y)` fills cell `(x, y − 1)`).
No per-cell clipping (a footprint near the edge writes outside the row; the port drops writes outside the buffer).

## 3. Start point
`start` = the map's AI build start (§1); with `n > 0` buildings: `(sx / n, sy / n)` (int division);
if the caller passed `x ≥ 0`: `(x, y)` as given. (`EpuletNewSearchLocation` always passes −1, −1.)

## 4. The spot test (`DecodedCheckAllocation(x, y, w, h)`, true = taken)
- `x < 15 || y < 15 || x > 184 || y > 184` → taken.
- **Frame** `CheckKeretAllocation(grid, x, y, W = w + 2, H = h + 2)`: false if `W == 3 && H == 3` (1 × 1
  buildings skip it) or the centre is outside 0..200. Else the outer ring of the `W × H` box
  (columns `x − W/2` and `x − W/2 + W − 1`, rows `y − H/2 − H%2` and that `+ H − 1`; i.e. exactly the ring
  `DecodedAllocate` would draw) is scanned; any cell with `grid & 0x1f ≠ 0` → taken.
  (Neighbouring frames may overlap: `0x20` multiples are ignored here.)
- **Interior** `x ∈ x − w/2 ..= x + w/2 + w%2 − 1`, `y ∈ y − h/2 − h%2 ..= y + h/2 − 1`: only if the box lies
  within 0..200 (else the interior isn't checked at all); any cell ≠ 0 (frames included) → taken.
- Else free.

So a new footprint may not cover terrain, interiors or frames of other buildings, and its own frame may
not touch terrain or interiors: buildings end up exactly one shared frame cell apart at best.

## 5. The search (`LocatePos(check, cx, cy, &x, &y, w, h)`)
Fails at once unless `10 ≤ cx ≤ 189` and `10 ≤ cy ≤ 189`. With `a = cx − (w/2 + 1)`, `b = cy − (h/2 + 1)`,
`c = 200 − (w/2 + 1) − cx`, `d = 200 − (h/2 + 1) − cy`, `R = max(0, a, b, c, d)`:
```
for r in 0..R:  for k in 0..=r:   (first candidate that check() reports free wins)
  if r <= a:  k <= b → (cx − r, cy − k);   k <= d → (cx − r, cy + k)
  if r <= c:  k <= b → (cx + r, cy − k);   k <= d → (cx + r, cy + k)
  if r <= b:  k <= a → (cx − k, cy − r);   k <= c → (cx + k, cy − r)
  if r <= d:  k <= a → (cx − k, cy + r);   k <= c → (cx + k, cy + r)
```
The written `(x, y)` (ints) become the building's f32 position. No spot: returns 0, the outputs keep
their input (−1, −1); `EpuletNewSearchLocation` then creates nothing and returns 0 (autobuild's
"no room", `colony-side.md` §3).

## 6. `EpuleteketJoHelyreTesz` (load-time fix-up; exe 0x53cd00)
The last call of `LoadScenario` (exe 0x552d8f, unconditional: every scenario and campaign map load) and a console
command (`ExecuteString` 0x4ddb7c). For every planet with an owner (`+0x20`), slots `j` = 1.. while
`j ≤ HanyEpuletLefoglalva[p]` (re-read each step) with kind `k ≠ 0`: remember `k` and the `(int)` position,
`EpuletDelete(p, j)` (turrets removed, the `'+'` building's self-destruct reset, `HanyEpulet[p] − 1`),
`LocateSuitableLocation(p, k, &x, &y)` with the old cell as the start (so it stays if it's free; with no spot the
outputs keep the old cell), then `EpuletNew(p, k, done = true, (f32)x, (f32)y, 0)`: a fresh record (damage 0, no
work left, output factor recomputed) with `AddBuildingTurrets` and `BuildingBuilt`. `EpuletNew` takes the planet's
**first empty slot**, so a building behind a hole moves into it (and isn't visited again) [original behaviour].
The search sees the ground units on the map (§1) and the buildings not yet re-placed. Port: `Game::refit_buildings`.

## 7. Verification
The vehicle pass (§1, §2.3) now gets the planet's battlefield (`SaveState::vehicle_cells` =
`ground::Field::placed_cells`) in the economy step and colonisation arrivals; the refit (§6) is verified with it in
`traces/2026-10-16-refit.md` (10 fuzzed refits, 2,060 buildings, 569 moved; mutations of the vehicle pass, the start
hint, `BuildingBuilt` and the damage reset are each caught). The build cursor's search (§8) is verified in
`traces/2026-10-23-build-cursor.md`.

`traces/2026-10-15-placement.md`: the colony-side replays place every new building with the port
(`ig2-sim::placement`): 60 colonisation hubs and 161 autobuild purchases land exactly where the game put
them, and the 9 "no room" outcomes match. Mutations caught 6/9; the per-step centroid truncation and the
15..=184 edge band rest on the asm (no fractional positions or edge placements in the traces), and the
interior's frame-cell test is logically redundant (§4).

## 8. The build cursor (`LocateActBuildingSuitableLocation`; the human's placement)
Evidence (ELF / exe):
| Function | ELF | exe | Role |
|---|---|---|---|
| `LocateActBuildingSuitableLocation(&x, &y)` | 0x4bc820 | 0x486460 | the cursor's search (`ShowColony` 0x49f130 calls it at 0x4a0089 and 0x4a1225) |
| `CheckAllocation(x, y, w, h)` | 0x4bc2d0 | 0x45fba0 | `DecodedCheckAllocation` (§4) over `MapAlloc` instead of `DecodedAlloc` |
| `Allocate(x, y, w, h, ring)` | 0x4bf6a0 | 0x45f3d0 | `DecodedAllocate` (§2) into `MapAlloc` |
| `RemoveAllocation(x, y, w, h, ring)` | 0x4bf8d0 | 0x497960 | the reverse: ring cells `− 0x20`, interior `:= 0` (and `hombar = 10` when `w == 6`) |
| `Build3dBuilding` / `Remove3dBuilding` | 0x4ca0b0 / 0x4ca7c0 | 0x49c780 / 0x497760 | keep `MapAlloc` current on the shown planet |

### 8.1 The grid `MapAlloc` (ELF 0x11dfd60, exe 0x998260; `u8[200 × 200]`)
`Init3dColony` (exe 0x47d520, from `Colony_Belep`) builds it like §2.1–§2.2 without the centroid and the vehicles:
`Decode2LargeMap(m, MapAlloc, false, p)`, then `Allocate((int)x, (int)y, w, h, true)` per building in slot order. While the
screen shows the planet, `EpuletNew` (when `AktKep == 3`, `p == AktPlanet`, no `Atlepes`) runs `Build3dBuilding` →
`Allocate`, and `EpuletDelete` (`AktKep == 3`, `p == AktPlanet`) runs `Remove3dBuilding` → `RemoveAllocation` when the
slot has a 3D model. `RecreateRoadSystem` only sets bit 8 on road cells and clears it again at its end; the landing zone
reads the grid. As footprints never share an interior cell with terrain or another footprint (§4), the kept grid equals a
fresh build of the current buildings; a building made during a screen transition (`Atlepes`) or a terrain change while
the screen is up would leave it stale until the next entry [edge case, not modelled]. Port: `PlacementRules::screen_grid`.

### 8.2 The search
For every battlefield unit of `AktPlanet` that is used and on the map (the filter of §1; the ELF -O0 build tests only the
used flag), `MapAlloc[y·200 + x] += 8` at the unit's own cell `(+0x1a0, +0x1a4)` (no bounds check), then
`LocatePos(CheckAllocation, x, y, &x, &y, w, h)` (§5) with the footprint of `AktEpuletTipus`, then the `+8`s are taken
back. Differences from the order's search (§2–§3): no building centroid (the start is the pointer's cell), and a unit
blocks its own cell `(x, y)` (8: caught by both the frame test's `& 0x1f` and the interior test), where
`LocateSuitableLocation`'s 1 × 1 `DecodedAllocate` blocks `(x, y − 1)`. So the order can move a building off the spot the
cursor showed, next to a unit [original behaviour]. Port: `PlacementRules::locate_cursor`.

### 8.3 In `ShowColony`
With the build cursor up: the pointer's world point `(wx, wy)` (the 3D pick) is kept as `ColonyActBPosX/Y = (int)wx,
(int)wy`; each frame `Real2Grid((f32)ColonyActBPosX, (f32)ColonyActBPosY)` is searched; no spot → `InvalidBuildingPosition`,
the cursor model hidden, `Epitheto = 0`; else the spot is `AktBGridXSarga/YSarga`, where the half-transparent model
stands. A second search after `Update3D` from `Real2Grid(wx, wy)` (untruncated) only sets `InvalidBuildingPosition`.
`colony_start_build` sends `(AktBGridXSarga, AktBGridYSarga)` with 0x1f.
