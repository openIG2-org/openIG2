# The human's spy orders (`MULTIGET_Spy`): runtime verification (2026-10-16)

Spec: `re/rules/espionage.md` §8. Port: `engine/crates/ig2-sim/src/spy.rs` (`Order`, `Spies::order`),
`Game::spy_order`, `ig2_net::Command::Spy`. Replay: `tests/spy_trace.rs` (records of kind `order`).
Tracer: `tools/trace/run_spy.py --orders N` + `spy.js` (Frida 17, GOG `ig2.exe`, `scenario/quick.scn`, `--fuzz`).

## Method
After every traced `FRISSIT_Spy` call the agent stages N random orders and runs them through the exe's own
`MULTIGET_Spy` (0x5883c0): its two `Multi_GetNextTipus` calls (0x5883cd, 0x5888f2) and six `Multi_Get` calls
(0x588400 … 0x5888b5) are patched for the duration of the call to stubs that hand it one staged message (the type
once, then the payload ints into `Multi_Get`'s out-parameters). Before each order the target spy is prepared so the
branches come up (state 1/3/2/0 for recall and send, an applicant for 0x4b, sometimes the "free" race 14, skill
points, a captured spy for the verdict) and sometimes the sender's money is set low or to 0; all of that is in the
record's `pre` diff. Sender: the human two times in three, else any empire 1..=8; slots 0..=12; missions 0..=13
(story missions would make the next daily pass ask the story engine, which the replay doesn't trace), training
levels 0..=5, days 0..=40, verdicts 0..=7, applicant choices 0..=4, skill bytes `+0x30..+0x35`. The replay rebuilds
the rosters, money, stream seeds and the rest as for the daily pass, applies `Spies::order` and compares every
region byte for byte (money as parsed treasuries).

| Trace | Seed | Daily calls | Orders |
|---|---|---|---|
| `spy-orders-1` | 911 | 132 (the clock stalled on a story screen after that) | 528 |
| `spy-orders-2` | 2024 | 100 | 400 |

## Result
**928 orders byte-exact** (dismiss 154, recall 151, send 150, verdict 157, applicant 163, skill point 153), and the
232 daily calls in between still exact. Mutation checks, each caught: the money test off by 1,000 (15
mismatches), the catch moment's upper bound `days·50 − 29` (84), the turn-away roll `> 2` (1), the hire price with
rank + 1 (7).

Notes: Frida's own hooks don't fire inside the agent's `NativeFunction` call, so the record's callee list is empty
for orders; the state diff is the evidence. The recount of `HanyKemVan` after the queue is part of every record.
