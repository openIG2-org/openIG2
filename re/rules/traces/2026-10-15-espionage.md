# Espionage: runtime verification (2026-10-15)

Spec: `re/rules/espionage.md`. Port: `engine/crates/ig2-sim/src/spy.rs`. Replay: `tests/spy_trace.rs`.
Tracer: `tools/trace/run_spy.py` + `spy.js` (Frida 17, GOG `ig2.exe`, `scenario/quick.scn`).

## Method
The game calls `FRISSIT_Spy` (exe 0x584b30) once a day. The agent hooks it and records the byte regions it
reads or writes before and after every call: spy rosters, counts, the mission table, relations, contacts,
techs, planets and buildings, money pools, kidnappings, stream seeds, fleets, ships, vehicles, designs and
research. It also logs the callees reached (rebellions, building and ship deletions, techs, recruits, captor
verdicts, money). `--fuzz` revives empires 1..8 with built-up planets and rewrites 3–10 spy records before
each call: missions 1..13 due today, catch windows, double agents, applicants, captured spies,
counter-intelligence. Now and then it also sets the kidnap table, unseen planets and techs, the SP debug
override 0xB77250, `DEBUGX`, `Nap = 1` and the difficulty (restored after the call). `--ships` adds warships
and hands the human's empire to the AI, because real-time battles stop the clock on screen 0xB.

| Trace | Mode | Calls |
|---|---|---|
| `spy-plain` | unfuzzed (AI-launched missions only) | 300 |
| `spy-fuzz-21`, `spy-fuzz-22` | fuzzed, human side | 400 + 400 |
| `spy-ships-23` | fuzzed, warships, human empire as AI | 293 (time limit) |

## Result
**1,393 calls, byte-exact.** Covered: 1,382 mission results across all 12 effect missions plus training
(successes: 121 training, 67 empire reports, 34 planet reports, 40 tech reports, 41 thefts, 30 sabotages,
9 ship strikes, 10 tech thefts, 6 rebellions, 13 assassinations, 19 kidnappings, 16 mass unrests).
346 catch checks: outcome 4 ×149, caught ×187 (AI verdicts: turned 21, double agent 19, executed 60,
released 38, killed 39), spotted 6, lost 4. Also 26 double agents exposed, 78 recruits (35 `Spy_New`,
43 `Spy_Copy`), 2,598 rank-ups, 9 discoveries.

The first replay found one spec error: `HanyKemVan` is at 0xBB23D0 (the exe's loop writes from 0xBB23D4 for
empire 1), not 0xBB23D4.

## Mutation pass
61 single-point mutations of `spy.rs`; **50 caught**. Missed:
- Equality boundaries the traces never hit exactly: the monthly-experience `≤` (a draw equal to
  `0.03·xp + 1`), the double-agent clamp at 20 and the `< 50` roll, the assassination tie `a == b`, the
  rank floor 2 vs 3, the unrest weight 100000 vs 100001 (the same integer weight on the traced morale).
- States the traces don't contain: hidden designs (`+0x80 != 0`) for ship strikes, loyal or AI-proof
  planets for rebellions, a chance above `max + 10` (it needs ×4 multipliers).
- Equivalent on the traced data: the tech-report dedupe `a == c` (three picks never collide like that),
  and the sabotage fallback order (no call had both `(p1, 0)` and `(0, p2)` find a building).
Those are asm/decompile-pinned (spec §3–§6).

## Not traced
Story missions (`+0x1d`: `StoryEredmenyKem*` is a hook in the port), MP, the tutorial, `ProgramPlayers > 8`
(recruitment for empires ≥ 9 would index past `Kemek`), and the human's own decisions (`MULTIGET_Spy`).
