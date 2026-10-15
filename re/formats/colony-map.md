# Colony terrain maps: `dataraw/colony/gen_maps/<n>.map`

Status: **layout verified** on all 75 files (`ig2_formats::colony_map`, golden test
`every_colony_map_parses`). Field meanings are partly inferred.

## Evidence (ELF `ig2`)

| Symbol | Address | Role |
|---|---|---|
| `InitHatterekEsTerepek()` | 0x4bd5b0 | `sprintf("colony/gen_maps/%d.map", i)` for `i < *(int*)0x12795a0`, then sequential `DatFile::dread` into `Terepek[i]` |
| `CheckTerepTargyak(XVBStruct*, meshid, int)` | 0x4bd150 | marks terrain-object footprints into both masks |
| `Real2Grid(float, float, int*, int*)` | 0x4717e0 | world → 200×200 grid |
| `TestHeightMapDifferences` | — | only reader of `+0x1394` besides the loader |
| `GetHeightFloat`, `Init3dColony`, `PlanetAtjarasMap::GenerateMap` | — | readers of `+0x8c` |

`Terepek` is a pointer (`0x1279360`) to an array of 0x2720-byte terrain structs. The map file
fills part of each struct. The rest comes from `terepleiro.txt` ("terrain description": background
`.xvb`, fog colour, allowed weather, time of day), parsed in the same function.

## Layout (little-endian, no header)

| Offset | Size | Struct field | Content |
|---|---|---|---|
| 0 | 650 | `+0xae` | mask A: 50 rows × 13 bytes; 2 bits per cell, 4 cells per byte (cell x at bits `(x%4)*2`) |
| 650 | 650 | `+0x338` | mask B: same addressing |
| 1300 | 2500 | `+0x9d0` | 50×50 bytes, row-major [inf: tile/texture IDs] |
| 3800 | 5000 | `+0x1394` | 50×50 u16, row-major [inf: heightmap, values ≈ 11200–17413] |
| 8800 | 4 | `+0x9cc` | u32 N (78–209 in shipped files) |
| 8804 | 4N | `+0x5cc` | N × f32 [unk] |
| 8804+4N | 4 | `+0x8c` | f32 [inf: base height; `GetHeightFloat` subtracts a constant from it] |

Total = 8808 + 4N bytes. This matches every shipped file exactly.

### Mask addressing (from `CheckTerepTargyak`)
```
(gx, gy) = Real2Grid(world_x, world_z)        // 0..200
row = gy / 4; col = gx / 4                   // 0..50
byte = row * 13 + col / 4
mask[byte] |= 3 << ((col % 4) * 2)
```
Both masks are set together for terrain objects. [inf] One is "can't build", the other "can't pass"
(`PlanetAtjarasMap` = passability map), but which is which hasn't been established.

## Visual check (2026-10-15)
Rendered side by side (heights | tile IDs | mask A | mask B) for maps 0, 1 and 30:
- heights: a smooth terrain field (craters, ridges, roads visible) → heightmap ✔
- tile IDs: uniform over flat ground, varied in rough areas → terrain texture tiles ✔
- mask B: small scattered blobs → terrain-object footprints ✔ (matches `CheckTerepTargyak`)
- mask A: large contiguous regions plus objects; not simply height or tile → **the build mask**: it's
  what `Decode2LargeMap` rasterises for building placement (`rules/colony-placement.md` §2; verified by
  the placement replay, where ignoring it gives 54–79 mismatches)
  (unbuildable/impassable candidates)

## Open questions
- Mask columns 50–51 (inside the 13-byte stride) are set in shipped files: real cells or junk?
- The meaning of the N-float list, and whether N is even (pairs → points/paths?).
- Height units versus world units (check in the asset viewer: render as a heightfield and compare with the game).
- Other colony files: `landzone.map` (2500 B = 50×50), `talaj.b` (30000 B, "talaj" = soil),
  `bf.b` (10000 B), `a.m` (650 B, mask-sized), `obj*.res`, `first.res`. Loaders not yet located.
