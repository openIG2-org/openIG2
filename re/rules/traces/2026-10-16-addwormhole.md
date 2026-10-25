# Opening a wormhole (`AddWormhole`) (2026-10-16)

Spec: `re/rules/fleet-movement.md` "Opening a wormhole" (before §11). Port: `Game::add_wormhole` (`ig2-sim::game`),
with the fleets made by the AI port's `FlottaNew` (`ai::World::new_fleet`, `production-research.md` §5.1) and the tint
table `ig2-rules` `wormhole_colours` (`i32 [6][3]` at exe 0x913850, `game::GameRules::radar.wormhole_colours`).
`Game` now carries out the stories' `StoryEvent::Wormhole` (902, 239) in the story pass's hand-offs, and 239's
`StoryEvent::RevealWormhole` (`Game::reveal_wormhole`) when the call made one. Test:
`engine/crates/ig2-sim/tests/save_replay.rs` `add_wormhole_matches_the_exe`. Tracer: `tools/trace/run_save.py --wormholes`.

## Evidence

Exe `AddWormhole` 0x53aec0 (ELF 0x615cb0): `OsszHanyWormhole` 0xBB61A0 `> 4` → return 0; `w = ++count`; record
`Wormhole` 0xBB60F8 + 0x1c·w: `+8 = 0`, `+0/+4 = (f32)x1/(f32)y1`, `+0xa = FlottaNew("- Wormhole -", 0, 0, x1, y1, 0)`,
that fleet's `+0x53 |= 4` then `|= 2`; end B the same at `+0xc/+0x10/+0x14/+0x16`; `+0x18 + i = -1 − (char)((0xff −
(byte)c[w][i]) / 2)` with `c` = `0x913850 + 0xc·w + 4i` (`= 255 − (255 − c) / 2` as a byte); then four `WormHole` scene
objects (`FUN_0053abf0`, list 0xF18B68: render only, not saved). Story 239's follow-up (exe 0x5ceb86..0x5cebc8): when
the call returned `w ≠ 0`, `+8 = 1` / `+0x14 = 1` and both fleets' `+0x53 |= 1`.

## Recording

`run_save.py --wormholes --days 3,20,45,80,120 --seed 7311 --tag wh7311` on `scenario/quick.scn`: at each day the agent
sets `OsszHanyWormhole` to a start value (0 on the first day, else `rnd(6)`), writes a save, calls the exe's
`AddWormhole` from the frame hook with coordinates in −200..1199 (a quarter of the calls with both ends on the same
point) until a call returns 0, and writes a second save; the calls and the results go into the second save's JSON.
The port loads the first save, runs `Game::saved()`, makes the same calls (the results must match) and must write
the second save (every modelled chunk group: `OsszHanyWormhole`, `Wormhole`, `Flottak`, fleet counters, pools).

Recordings in `_local/traces/addwormhole/` (`<tag>-<2k>.sav` before, `<tag>-<2k+1>.sav` after, `.json` beside them;
never committed).

| Pair | Start count | Calls (x1, y1, x2, y2 → w) |
|---|---|---|
| 0/1 | 0 | five made (1..5, one with both ends on one point, one with negative coordinates), the sixth → 0 |
| 2/3 | 5 | → 0 |
| 4/5 | 5 | → 0 |
| 6/7 | 4 | → 5, → 0 |
| 8/9 | 3 | → 4 (same point, negative x), → 5, → 0 |

## Result

All 5 pairs exact: 8 wormholes made, 5 calls refused, every return value equal, every modelled chunk equal (the new
fleets in the last free slot or appended with the exe's uninitialised bytes via `Game::uninit`).

Mutation check (each alone fails the test): the tint divisor 2 → 3; dropping `+0x53` bit 1 (fleet bit 9); the
limit `> 4` → `> 5` (return values and counts differ); end A's x + 0.5.

Not traced: `RevealWormhole` (story 239's own writes; the test checks them on the last wormhole of each pair) and the
scene objects (render only).
