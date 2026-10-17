# 2026-10-17: the research screen's orders (`MULTIGET_Research`) against the exe

`tools/trace/run_replay.py <install> _local/saves/quick-1.sav <out> --ticks 60|80 --research 4|5 --research-every 2
--seed S` (S = 2, 3, 4). Before save 0 the tracer turns on "always simulate" (0xB77230). From tick 1, every second
`IdoNapValtozott` runs N random messages through the exe's own `MULTIGET_Research` (exe 0x51ee70: its two
`Multi_GetNextTipus` calls 0x51ee7a/0x51f02d and four `Multi_Get` calls 0x51eea6, 0x51ef01, 0x51ef3b, 0x51ef74
patched to the starmap orders' stubs, one staged message each) before the tick's save. Messages are shaped like the
screen's: 0x2a starts mostly a researchable tech (status 3) of the issuer, now and then any tech or with a project
running; 0x29 stops mostly the running project; 0x2b toggles urgency; 0x31 (develop every tech some empire knows)
rarely; the issuer is mostly the human (empire 6), one in six an AI empire 1..8 (alive or not). Each is a
`{"k": "rorder", "before", "libc", "type", "vals"}` record.

`engine/crates/ig2-sim/tests/save_replay.rs` applies them with `Game::research_order` after the tick's clock step and
compares the whole state with the exe's save:

| Recording | Ticks | Messages (0x29 / 0x2a / 0x2b / 0x31) | Result |
|---|---|---|---|
| s2 | 60 | 120 (46 / 41 / 30 / 3) | byte-exact |
| s3 | 80 | 200 (68 / 80 / 46 / 6) | byte-exact |
| s4 | 80 | 200 (64 / 81 / 51 / 4) | byte-exact |

(Recordings: `_local/traces/replay-quick1-research-s{2,3,4}-x16`.)

What the recordings found (fixed):
- `TalalmanyKifejlesztve` gives a human's hull tech its automatic design (`TervezAutomatikus` exe 0x4b72b0: a new
  design of that hull named after the tech, fitted by `TervAutoDesign`); the port only reported it. Message 0x31
  on the human made 12 designs at once. Now `ai::World::automatic_design`, also for the research tick's
  completions.
- `TervAutoDesign`'s allow list for techs 0x41..=0x43 (exe 0x5419ec..0x5419fa, on the stack) had its variant-1 half
  shifted by one in the port (`T41_OK`); ai-empire.md §10.3 had it right. Found by 0x31 handing a dead AI empire
  every tech: its next `FRISSIT_MI_DESIGN` picked tech 0x41 where the port picked a shield.

Mutations (each run on the three recordings): stop → status 2, urgent set instead of toggled, no
`Story_Special(0x0d)` after 0x31, 0x31 walking the techs upwards, the automatic design without its second
`TervUjraSzamol`, the design on the next hull: all caught. Dropping 0x31's final `Research_TalalmanyUjraSzamol` is
not caught: `TalalmanyKifejlesztve` already ran it after the last tech (equivalent).
