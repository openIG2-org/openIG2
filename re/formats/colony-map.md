# Colony terrain maps: `dataraw/colony/gen_maps/<n>.map`

Status: **verified** on all 75 files (`ig2_formats::colony_map`, golden test
`every_colony_map_parses`). Every field's use (or non-use) is pinned in the ELF: the 16 functions that
touch `Terepek` were decompiled (2026-10-16).

## Evidence (ELF `ig2`)

| Symbol | Address | Role |
|---|---|---|
| `InitHatterekEsTerepek()` | 0x4bd5b0 | `sprintf("colony/gen_maps/%d.map", i)` for `i < *(int*)0x12795a0`, then sequential `DatFile::dread` into `Terepek[i]` |
| `CheckTerepTargyak(XVBStruct*, meshid, int)` | 0x4bd150 | marks terrain-object footprints into both masks |
| `Real2Grid(float, float, int*, int*)` | 0x4717e0 | world → 200×200 grid |
| `TestHeightMapDifferences` | 0x4bd040 | editor helper: writes `+0x1394` from `GetHeight2` |
| `GetHeightFloat` | 0x464090 | height sample: triangle of the 50×50 grid, `fix2float(h) + Terepek+0x8c` |
| `fix2float(short)` | 0x4bb7a0 | `(f32)((f64)h / 16.0) − 700.0`, `h` sign-extended |
| `Decode2LargeMap` | 0x4bc040 | mask A → 200×200 byte grid (`Set4x4Block`), + landing zone |
| `PlanetAtjarasMap::GenerateMap` | 0x4ae090 | passability layers from slopes; mask B blocks layers 1..4 |
| `Init3dColony` | 0x4c6050 | sets `TEREPXSIZE/YSIZE` from the terrain mesh box; moves the mesh by `+0x8c` |

`Terepek` is a pointer (`0x1279360`) to an array of 0x2720-byte terrain structs. The map file
fills part of each struct. The rest comes from `terepleiro.txt` ("terrain description": background
`.xvb`, fog colour, allowed weather, time of day), parsed in the same function.

## Layout (little-endian, no header)

| Offset | Size | Struct field | Content |
|---|---|---|---|
| 0 | 650 | `+0xae` | mask A: 50 rows × 13 bytes; 2 bits per cell, 4 cells per byte (cell x at bits `(x%4)*2`) |
| 650 | 650 | `+0x338` | mask B: same addressing |
| 1300 | 2500 | `+0x9d0` | 50×50 bytes, row-major [inf: tile/texture IDs]; **never read** |
| 3800 | 5000 | `+0x1394` | 50×50 **i16**, row-major: heights (map 56 has negative values) |
| 8800 | 4 | `+0x9cc` | u32 N (78–209 in shipped files); **never read** after the load |
| 8804 | 4N | `+0x5cc` | N × f32; **never read** (dead editor data) |
| 8804+4N | 4 | `+0x8c` | f32 `b`; the loader stores `200 − b` (`-(b − 200)`) as the base height |

Total = 8808 + 4N bytes. This matches every shipped file exactly.

### Mask addressing (from `CheckTerepTargyak`)
```
(gx, gy) = Real2Grid(world_x, world_z)        // 0..200
row = gy / 4; col = gx / 4                   // 0..50
byte = row * 13 + col / 4
mask[byte] |= 3 << ((col % 4) * 2)
```
Both masks are set together for terrain objects. Only cells 0..49 of a row are used: the readers
(`Decode2LargeMap`, `GenerateMap`) take 12 full bytes plus the low nibble of byte 12, so **columns
50–51 are padding** (set in 2,430 of the 3,750 shipped rows, never read).

### Mask roles (`PlanetAtjarasMap::GenerateMap`, 0x4ae090)
- **Mask A = build mask.** `Decode2LargeMap` rasterises it for building placement
  (`rules/colony-placement.md`). In editor mode (`GenerateMap(p, true)`) the game *generates* mask A
  from passability layer 0 and writes it back, and sets `+0x8c` from the height at the search result.
- **Mask B = terrain-object obstacles.** In game mode `GenerateMap(p, false)` builds five 200×200
  passability layers from the height slopes (8-neighbour differences × 0.0488, clamped ±2.9, compared
  with `TankMeredekseg[layer]`, layer 3 = everything passable), then marks mask B's cells blocked in
  layers 1..4 (`Set4x4(…, 1)`); layer 0 ignores it.

### Heights and units
`GetHeightFloat(x, y, planet)` (x, y in height cells, 0..50) intersects the cell's triangle and returns
`fix2float(h) + base`, with `fix2float(h) = (f32)((f64)(i16)h / 16.0) − 700.0` and
`base = Terepek+0x8c = 200 − b` (the file's last f32). So heights are 1/16 world unit steps around
−700. Horizontally a height cell is 4 `Real2Grid` cells; `Grid2Real` maps the 200-cell grid onto
`TEREPXSIZE × TEREPYSIZE` world units (default 8192; `Init3dColony` sets them to the terrain mesh's
bounding box). Shipped world heights span −1522..927 (golden test). `terepleiro.txt`'s "special
height" also goes to `+0x8c` (+ 110) but the map file overwrites it.

## Visual check (2026-10-15)
Rendered side by side (heights | tile IDs | mask A | mask B) for maps 0, 1 and 30:
- heights: a smooth terrain field (craters, ridges, roads visible) → heightmap ✔
- tile IDs: uniform over flat ground, varied in rough areas → terrain texture tiles ✔
- mask B: small scattered blobs → terrain-object footprints ✔ (matches `CheckTerepTargyak`)
- mask A: large contiguous regions plus objects; not simply height or tile → **the build mask**: it's
  what `Decode2LargeMap` rasterises for building placement (`rules/colony-placement.md` §2; verified by
  the placement replay, where ignoring it gives 54–79 mismatches)

## Other colony files: unused by the shipped code
`landzone.map` (2500 B), `talaj.b` (30000 B), `bf.b` (10000 B), `a.m` (650 B), `obj3.res`, `obj64*.res`,
`first.res`: none of these names occurs in the ELF. They come only from `terepleiro.txt`, and
`InitHatterekEsTerepek` reads the passability map (`a.m`) and the landing-zone map into a scratch
buffer and drops them, stores the radar heightmap (`+0x4d`) and the object description (`+0x90`)
without any reader, and `Colony_Belep` copies the radar pattern (`+0x6b`) into a global nothing reads.
Landing zones are generated at runtime (`LandingZone::Generate` 0x46b610). The port can ignore them.

## Open questions
- None for the layout. The tile bytes and the N-float list are dead in the shipped code; their 1999
  meaning (editor data) doesn't matter for the remake.
