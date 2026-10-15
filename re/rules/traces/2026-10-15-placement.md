# Colony-map building placement: runtime verification (2026-10-15)

Spec: `../colony-placement.md`. Target: GOG `ig2.exe` (sha256 2baa530b…0442), `LocateSuitableLocation`
@ 0x4869d0 via `EpuletNewSearchLocation` @ 0x53cca0.

## Method
No new recording: the colony-side traces (`traces/2026-10-15-colony-side.md`) already hold every
planet's raw record (colony map `+0x23`) and all building slots before and after each call. The replay
`engine/crates/ig2-sim/tests/colony_side_trace.rs` now places new buildings with the port's search
(`PlacementRules::place`) instead of copying the game's position, built from the install: mask A of
`colony/gen_maps/<n>.map`, the AI build starts of `colony/terepleiro.txt` and the footprints
(`Building` +0x58/+0x5c, exe 0x8F1F28) read by `ig2-rules`. Whole building records (positions included)
and the "no room" outcome are compared with the game's `after` state. Ground-war vehicles: none on the
traced planets (input left empty).

## Results
| Trace set | Placements by the search | "No room" | Result |
|---|---|---|---|
| `side-colonise.jsonl` (colony hub of a colonisation) | 60 | 0 | exact |
| `side-economy*.jsonl` (autobuild purchases in 431 economy steps, plain + 4 fuzzed) | 161 | 9 | exact |

## Mutations (scratchpad script, one change at a time)
Caught: terrain ignored (54/79 mismatches), no frame test (53/52), odd heights on the high side (29),
footprint width/height swapped (20), spiral order changed (4/17), start point +1 (1/44).
Not caught:
- per-step truncation of the centroid accumulator: every traced building sits on integer coordinates
  (all placed by this search), so it can't show; pinned by the asm (`cvtsi2ss`/`addss`/`cvttss2si`).
- interior test treating frame cells (0x20) as free: logically redundant (an interior over another
  building's frame always puts that building's interior on the new frame or interior).
- the 15..=184 edge band (vs 10..=189): no placement reached the edges; pinned by the asm
  (`cmp 0xe` / `sub 0xf; cmp 0xaa` / `cmp 0xb9` @ 0x46db30).

## Not covered
Ground-war vehicles on the map, a caller-given start (`EpuleteketJoHelyreTesz`, ELF only), fractional
building positions (scenario/UI-placed), the landing-zone variant of `Decode2LargeMap`.
