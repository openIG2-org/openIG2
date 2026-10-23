# 2026-10-23: the build cursor's search (`LocateActBuildingSuitableLocation`) against the exe

Spec: `re/rules/colony-placement.md` §8. Port: `ig2-sim::placement::PlacementRules::{screen_grid, locate_cursor}`, used by
`ig2-client::colony_ui` (`hover`, start cell `cursor_cell_under`). Test: `engine/crates/ig2-sim/tests/save_replay.rs`
`build_cursor_search_matches_the_exe`. Tracer: `tools/trace/run_save.py --cursor` + `save.js` `fuzzUnits()`/`cursorProbes()`.

## Evidence
The exe's cursor search is 0x486460 (found as a caller of `LocatePos` 0x486610 that passes `CheckAllocation` 0x45fba0;
`ShowColony` 0x49f130 calls it at 0x4a0089 and 0x4a1225). Unlike the ELF -O0 build (which tests only the used flag), it
filters the units like `LocateSuitableLocation` (`+0x188` bits 1 and 30, `+0x18c` bit 2) and adds 8 at
`MapAlloc[+0x1a4 · 200 + +0x1a0]` (exe `MapAlloc` 0x998260). `MapAlloc`'s writers (FindRefs over 0x998260..+40000):
`Init3dColony`, `Allocate` 0x45f3d0, `RemoveAllocation` 0x497960, `AddUtDarab` 0x45ede0 (road bit 8, cleared by
`RecreateRoadSystem`), the cursor search itself; the landing zone and `ShowGrid` only read it.

## Recording
`run_save.py --cursor --days 2,20,60,120 --seed 919 --tag cursor919` on `scenario/quick.scn`: at each day half the
battlefield units move to a building cell ± 6, a save is written; then on every owned planet the agent builds
`MapAlloc` with the exe's own `Decode2LargeMap(m, MapAlloc, 0, p)` and `Allocate((int)x, (int)y, w, h, 1)` per building
(as `Init3dColony`), and calls 0x486460 forty times (`AktPlanet = p`, `AktEpuletTipus` 1..=60; starts: 70 % a building
cell ± 8, 20 % anywhere, 10 % at the edges and outside), restoring `MapAlloc`, `AktPlanet` and `AktEpuletTipus`.
Recordings in `_local/traces/cursor/` (`cursor919-<k>.sav` + `.json` with each grid and `[kind, x, y, ok, x', y']`; never
committed). Units moved: 25, 30, 24, 33.

## Result
4 saves, 40 grids, 1,600 searches (1,173 moved off the start, 211 refused, all refusals starts outside 10..=189): the
port's `screen_grid` equals every exe grid and `locate_cursor` (with `SaveState::vehicle_cells`) every result.
Mutations caught: the unit stamp at `(x, y − 1)` as in the order's search (41 mismatches), a stamp of 0x20 instead of 8
(ignored by the frame test; 60). Not exercised: the kept grid after builds/demolitions on the screen (`RemoveAllocation`,
equal to a fresh build by the placement invariant, spec §8.1), and `ShowColony`'s int truncation of the ground point
(asm reading; the client follows it).
