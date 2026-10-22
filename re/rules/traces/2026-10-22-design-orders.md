# 2026-10-22: the design orders (`MULTIGET_Design`) against the exe

`tools/trace/run_replay.py <install> _local/saves/quick-1.sav <out> --ticks 60 --design 4 --design-every 2 --seed S`
(S = 2, 3, 4; `--slot 83 --slot-base 760 --pre-slot 82` kept it off the slots other recordings used that day). Before save
0 the tracer runs the starmap orders' set-up (stations on the human's planets, two fleets of its ships and vehicles).
At tick 2 the fleets' vehicles go to the human's first planet through the exe's own `MULTIGET_Starmap` 0x1c
(reported as `sorder`: planet holders −p for the design orders). From tick 4, every second `IdoNapValtozott` runs four
random messages through the exe's own `MULTIGET_Design` (exe 0x4b4d60: its `Multi_GetNextTipus` calls 0x4b4d7a,
0x4b5740 and `Multi_Get` calls 0x4b4dd4, 0x4b5328, 0x4b565e patched to the starmap orders' stubs; 0x33's flag is a
one-byte field, so the stub writes it with a byte store) before the tick's save. Each is a
`{"k": "dorder", "before", "libc", "type", "vals"}` record. The messages: 0x33 an issuer's design stored back (mode 3
in place or with flag 1 as a new upgrade copy, else a new design), its price and build time now and then changed, and
now and then an upgrade target of a design with units edited at about the old design's price (so the in-place refit
runs); 0x34 a design's units of one holder sold (mode 2, now and then another mode); 0x35 a refit. Mostly the human
(empire 6), one in six an AI empire 1..8.

`engine/crates/ig2-sim/tests/save_replay.rs` applies them with `Game::design_order` after the tick's clock step and
compares the whole state with the exe's save:

| Recording | Ticks | Messages (0x33 / 0x34 / 0x35) | Result |
|---|---|---|---|
| s2 | 60 | 116 (45 / 44 / 27) | byte-exact |
| s3 | 60 | 116 (48 / 35 / 33) | byte-exact |
| s4 | 60 | 116 (38 / 51 / 27) | byte-exact |

(Recordings: `_local/traces/replay-quick1-design-s{2,3,4}-x16`; an earlier set of the same seeds had about 129 / 132
/ 87 of the three types, 48 sales and 2 refits on planet holders.)

What the recordings found:
- Two garrison set-ups failed for reasons outside the handler. `TankNew(−p)` called by the tracer before save 0: at
  tick 1 the new battlefield units' `+0xe0` (f32) is 0.65 in the exe and 0.7 in the port (staging only; not
  followed up). The transfers at tick 1: the replay test has no source for new battlefield units' uninitialised bytes
  at tick 1 (its `next` save is set at the end of tick 1). The transfers moved to tick 2.
- A recording whose run shared its save slot with another recording's loaded a different game (Ido2 0); recorded again on
  other slots.

Mutations (each run on the three recordings), all caught: the queue rescale's old/new prices swapped, no rescale, the
market offer's `rndlog(11, 110, 131)`, no market offer, no `Story_Special(0x17)`, `TervDelete` without the mark, the
refit cost ×3, no refit, an in-place edit without `TervAzOsszesHajotAtalalitja`, an upgrade copy that doesn't move the
other designs' upgrade targets, stations left out of the count, the rescale only on the head design itself, the
sale credit dropped.

Not covered: the battlefield half (0x33's in-place loop over `WF` units, `0x4a78d0` for a planet's refit) needs a
ground war holding units of the design; the story hero's ship (`+0x44 == 5` with `StoryAdat+0xac`); a full market
(40 offers).
