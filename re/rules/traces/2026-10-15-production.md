# 2026-10-15: production and research queues, runtime check

Spec: `../production-research.md`. Port: `engine/crates/ig2-sim/src/{research,production}.rs`, replay
`engine/crates/ig2-sim/tests/production_trace.rs`. Traces (game state, local only):
`_local/traces/prod-research-*.jsonl`, `_local/traces/prod-production-*.jsonl`.

## Method
`tools/trace/run_prodres.py` spawns the GOG `ig2.exe` under Frida 17, loads `scenario/quick.scn` from the
frame hook and runs at speed 3. The agent (`prodres.js`) hooks
- `FRISSIT_Research` (exe 0x51ddd0) enter/leave (`--mode research`);
- the production block inside `FRISSIT_Starmap`: instruction hooks at 0x597322 (after the fleet loop) and
  0x597aac (after the planet loop) (`--mode production`).

State is recorded as byte regions (tech table, research targets, urgency, labs, treasuries, `PlayerKi`/
`PlayerFaj`; or planets, fleets, ships, vehicles, every empire's designs, delivery fleets, fleet name
counters, colony-ship pools): one full dump, then per record the changes before the call ("pre": other
systems and the fuzzer) and the changes the call made ("post"), plus the globals (day, speed,
difficulty, EN, MP, cheat and all-techs switches, story bonus, `nemlehetidotallitani`, `ProgramSebesseg`,
the default fleet name). The replay rebuilds the state, runs the port, writes its result back into the raw
records and compares **every byte** of every region with the recorded "after".

`--fuzz` (research): random research targets (any status), urgency values 0..=2, lab levels, progress
next to the limit, pool-3 money, races, difficulty, the story bonus, the research cheat, game speed, extra
researchable techs for idle AIs. `--fuzz` (production): random queue lines (designs of any class, counts,
repeat values 0..=2, UI and "into fleet" bytes, short build times), line capacities, rally planets, the
halt byte, delivery-fleet indices, station counts, pool-1 money, difficulty, speed, and idle or
planet-bound fleets spawned next to planets with `FlottaNew`/`HajoNew`/`FlottaMozgat`.

## Results
| Trace | Ticks | Exercised | Result |
|---|---|---|---|
| research plain | 3,000 | 878 payments, 2 completions, 1 AI pick | exact |
| research fuzz (seed 7) | 3,000 | 5,962 payments, 1,701 completions, 1,344 AI picks, 1 auto design | exact |
| research fuzz (seed 99) | 3,000 | 5,164 payments, 1,647 completions, 1,275 AI picks, 2 auto designs | exact |
| production plain | 3,000 | 1,456 payments, 26 units (AI queues), 14 fleets, 10 garrisons | exact |
| production fuzz (seed 7) | 3,000 | 4,320 payments, 1,171 units, 475 fleets, 603 ships, 284 garrisons | exact |
| production fuzz (seed 99) | 919 (session ended at the time limit) | 1,355 payments, 378 units, 144 fleets, 170 ships, 105 garrisons | exact |

Found by the fuzzed traces:
- **The human's completion slows the clock mid-loop.** `SetIdoLelassit` (exe 0x541270) sets
  `IdoSpeedSzorzo = 1` at once in SP, so empires after the human in the same `FRISSIT_Research` loop (and the
  rest of the tick) run at speed 1. Three ticks with the cheat on mismatched until the port applied it
  (`ResearchContext::slow_down`; MP returns to the `ProgramSebesseg` multiplier).
- `FlottaNew` leaves the name bytes after the NUL and `+0x18` bits 2–7 untouched: appended fleet slots carry
  uninitialised memory there (the replay takes those bytes from the exe, like `vis[0]`, `+0x2e/+0x2f`, bit 15).

## Mutation check
A scratchpad script swaps one expression at a time and reruns the replay.
- Research: **18/23 caught.** Survivors: the f64 speed sum and the f64 story bonus (small integer sums
  and bonus values round the same; the asm pins f64: `addsd`/`divsd`/`mulsd` 0x51dea0..0x51dee3,
  0x51df41..0x51df54), the f64 progress add (equivalent for these values), `>=` vs `>` on the money test
  (no exact ties), the 0x95 → 0xAA link (never developed in the traces).
- Production: **21/27 caught.** Survivors: the `cost <= 0` path (no zero-price designs), `>=` vs `>` on the
  money test, a station design's rally planet (no station built with a rally planet set), the last vs
  first matching vehicle entry (no duplicate entries arise), `FlottaNew`'s f64 speed division (0.06 rounds
  the same) and the f32 garrison √ input (products below 2²⁴).

## Not covered
MP (production halving, the MP slow-down path), `FlottaDelete` of an emptied delivery fleet (unreachable
with consistent sums), `TervezAutomatikus` and `FRISSIT_MI_DESIGN` themselves (events), station overflow
past 10 entries.
