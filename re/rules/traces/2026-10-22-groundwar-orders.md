# 2026-10-22: the ground war's orders (`MULTIGET_Colony` 0x22..0x25, 0x12d, 0x12f, 0x130, 0x134) against the exe

`tools/trace/run_ground_rt.py <install> <out> --mode battle --orders-after 3 --orders-every 2|3 [--human] [--small]
[--ai-fuzz] [--orders-detonate] --seed S` (Frida 17, GOG `ig2.exe`, `quick.scn`). From battle tick T, every N ticks
one ground-war message goes through the exe's own `MULTIGET_Colony` (0x48a270): its 2 `Multi_GetNextTipus` and 16
`Multi_Get` calls are pointed at stubs handing it the staged type and values (as `replay.js --colony`), libc pinned by
`srand`. The messages are shaped like the screen's (re/screens/colony.md §9.1): move/waypoint to cells −10..209,
target a live unit or `−(slot)`, stop, detonate (big 0/1), retreat, air strike (kind 0/1), mine laying; the unit
mostly the human's, the issuer mostly the human (sometimes a side of the war). `--orders-detonate` makes half of them
detonations. Each is a `{"k": "gorder", type, vals: [issuer, p, …], freed, pre, post, rand}` record with the
battle's byte-region diffs (`freed`: the values Detonate's victim-burst test read from the freed detonator, logged by
a call stub patched over `cmp eax, [ecx+0x280]` at 0x46f49d while the message runs; an Interceptor probe there
doesn't fire for calls from the agent's thread).

`engine/crates/ig2-sim/tests/ground_rt_trace.rs::transporters_match_trace` rebuilds the state before each message,
applies it with `War::order` (or `War::detonate` with the logged reads) and compares every unit, the projectile lists,
the retreat flags, call-in timers and landing zone, the voices, the status cache, streams 0 and 2 and libc; the other
four ground replays carry the records' diffs and still pass on these recordings.

| Recording | Options | Messages | Result |
|---|---|---|---|
| `grt-orders-human-81` | small, human attacker, every 3 | 97 | exact |
| `grt-orders-human-82` | full forces, human attacker, every 2 (cut at 540 s) | 35 | exact |
| `grt-orders-83` | full forces, AI vs AI | 135 | exact |
| `grt-orders-fuzz-85` | small, `--ai-fuzz` (carriers, charges) | 114 | exact |
| `grt-orders-det-86` | AI vs AI, `--orders-detonate`, the freed-record log | 26 | exact |

By type (all five): 0x22 77, 0x23 45, 0x24 87, 0x25 51, 0x12d 31 (victims killed, buildings damaged), 0x12f 14,
0x130 49 (mostly without a carrier: only the charge timer; `ShotSpecWeapon`'s strike itself is verified by the
AI replays, 2026-10-16-ground-air.md), 0x134 53. The issuers: 6 (the human) mostly, 1, 3, 5, 7.

What the recordings found in the port (fixed):
- 0x12f plays `ColonySpeaker.Play(1, 3 | 10)` directly: a picked line costs a libc draw (the replay now runs with the
  voices).
- Detonate's fall-off is computed in f64 from an f32 product (`mulss`, `divsd`, `subsd`, `cvtpd2ps`), and the
  distance's int sum goes through f32 first.
- `SetTarget` on an emptied building slot reads kind 0's width out of the table (`Building[−1]+0x58` = 0x8F1F14,
  10000): `Rules::building_footprint0` → `GroundRules::kind0_width`.
- [original bug] Detonate's victim bursts test "on screen" with the detonator's freed record: a recording without the
  log (seed 84, discarded) skipped a burst the port made; with the log, 1 of the 31 detonations read a reused block
  (`0x8000000` instead of the planet) and skipped its bursts. The port assumes the record intact.

Not covered: messages for dead or missing unit slots (the exe reads a stale pointer; the port refuses them; the screen
only sends live selected units) and the 0x12f flag on a battlefield other than the shown one (`AktPlanet` [original
bug]; every machine shows p during a war).
