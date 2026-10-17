# 2026-10-17: the info screen's orders (`MULTIGET_Info`) against the exe

`tools/trace/run_replay.py <install> _local/saves/quick-1.sav <out> --ticks 60|80 --info 4|5 --info-every 2 --seed S`
(S = 2, 3, 4). Before save 0 the tracer turns on "always simulate" (0xB77230). From tick 1, every second
`IdoNapValtozott` runs N random messages through the exe's own `MULTIGET_Info` (exe 0x4ece60: its two
`Multi_GetNextTipus` calls 0x4ece6a/0x4ecf45 and two `Multi_Get` calls 0x4eceac/0x4ecf1f patched to the starmap
orders' stubs, one staged message each) before the tick's save. Messages are shaped like the screen's: 0x5b the
three shares of a point of the money triangle (`Info_Main`'s distances to the three edges over √2700, two of them
/1.25, below 0.03 → 0), one in six arbitrary values; 0x5c the spending limit bar's rounded values, one in five
arbitrary (or 0). The issuer is mostly the human (empire 6), one in five an AI empire 1..8. Each is a
`{"k": "iorder", "before", "libc", "type", "vals"}` record (each f64 as its low and high dwords).

`engine/crates/ig2-sim/tests/save_replay.rs` applies them with `Game::info_order` after the tick's clock step and
compares the whole state with the exe's save:

| Recording | Ticks | Messages (0x5b / 0x5c; an AI's) | Result |
|---|---|---|---|
| s2 | 60 | 120 (78 / 42; 20) | byte-exact |
| s3 | 80 | 200 (114 / 86; 38) | byte-exact |
| s4 | 80 | 200 (123 / 77; 41) | byte-exact |

(Recordings: `_local/traces/replay-quick1-info-s{2,3,4}-x16`.) The ticks after the messages cover their effect on
the economy (pools re-dealt by `MoneyInit`, the new reserve read by `MoneyMennyi`) and the AI's daily slider reset.
