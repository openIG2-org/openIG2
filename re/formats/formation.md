# Space-combat formations: `dataraw/spacewar/felallas<0-9>.bin`

Status: **layout verified** on all 10 files (`ig2_formats::formation`, golden test
`formations_parse`). Field meanings partly inferred.

## Evidence (ELF `ig2`, `../sources_base/spacewar.cpp`)

| Symbol | Address | Role |
|---|---|---|
| `SpwC::FelallasLoad(char*, felallastipus*)` | 0x691700 | game load: `SPACEWAR/%s.bin`, header u32, then records of 0x10 + 4 + 0x40 bytes into a 0x54-byte array |
| `SpwC::loadfelallas(char*)` | 0x6a5a60 | editor load: per record `racspos2pos` → `AddUjUrhajo` → `setshippos`; u32 → `urhajo+0x30`, 64 bytes → `urhajo+0x78` |
| `SpwC::savefelallas` | — | editor save (same layout) |
| `felallasforgat`, `tipus2felallashely` | — | rotate a formation / formation slot by ship type |

## Layout
```
u32 count
count × 84 bytes:
  i32 cell[4]     RACSPONTTYPE: battle-grid point (x, y, z/layer, w) [inf]; e.g. (64, 55, 5, 3)
  u32 kind        → urhajo+0x30 [inf: slot class; 1 in all shipped files]
  u8  extra[64]   → urhajo+0x78: cell again, a second point (i32 ×3 at +16) [inf: target/facing],
                  rest uninitialised (MSVC 0xCD fill) in most records
```
Slot counts: felallas0 13, 1–3 30, 4 29, 5–6 30, 7–8 31, 9 32.

## Open
- Grid coordinate units versus the space-combat world; meaning of cell[2..3] and `kind`.
- How `felallastipus` selects which slots a fleet uses (`tipus2felallashely`, `felallasforgat`): Phase 2 space-combat spec.
