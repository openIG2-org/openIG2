# 2026-10-17: the colony screen's orders (`MULTIGET_Colony`) against the exe

`tools/trace/run_replay.py <install> _local/saves/quick-1.sav _local/traces/replay-quick1-colony-s<S>-x16 --ticks
60|80 --colony 6|8 --orders-every 3|2 --seed S` (S = 2, 3, 4; `--slot 61 --pre-slot 60 --slot-base 400` keeps clear of
other recordings). Before save 0 the tracer damages a third of the human's buildings (random amounts below the kind's
hit points, for the repairs) and lists the building kinds standing anywhere on the map. From tick 1, every
`ordersEvery`-th `IdoNapValtozott` runs `colony` random messages through the exe's own `MULTIGET_Colony` (exe
0x48a270: its 2 `Multi_GetNextTipus` calls and 16 `Multi_Get` calls pointed at the `--orders` stubs; libc
`srand`-pinned per batch, the seed reported) before the tick's save. The messages are shaped like the screen's
(re/screens/colony.md §9–§10): builds of the listed kinds from a random cell (−1, −1 now and then) on the human's
planets and sometimes any planet; demolish, switch, priority and repair of the planet's used slots (mostly live
buildings, now and then an empty one); the shown planet (0x26); the self-destruct switch (0x27/300). Each is a
`{"k": "corder", "before", "libc", "type", "vals"}` record.

`engine/crates/ig2-sim/tests/save_replay.rs` applies them with `Game::colony_order` after the tick's clock step and
compares the whole state with the exe's save:

| Recording | Ticks | Messages | Result |
|---|---|---|---|
| s2 | 60 | 113 | byte-exact |
| s3 | 80 | 316 | byte-exact |
| s4 | 80 | 301 | byte-exact |

Message counts over the three: 0x1f 266 (the replay counted 80 `Built` and 113 money refusals among its events), 0x20 81, 0x21 126, 0x26 24, 0x27 11, 300 13, 0x132 79, 0x133 130 (3 money refusals counted). They also exercise the knock-on
systems: building blocks grown by 20 (uninitialised records), new defence posts' turrets on the battlefields, repairs
shared with turrets, `FRISSIT_Epuletek`'s repairs and refreshes, construction finishing, autobuild beside the orders.

What the recordings found in the port (fixed):
- `FRISSIT_Planet`'s display refresh (`FRISSIT_Planet(false, p)`, run by `FRISSIT_Epuletek` for planets with
  repairs) recounts `HanyEpulet[p]` from the building records; the port's refresh didn't write the count back, so a
  demolish of an already empty slot (which counts it down once more) stayed one too low.
- A build that grows the planet's building block takes the heap's bytes for the new records at once
  (`Game::fill_fresh_buildings` after the order, as after a tick).

Not reproducible and not sent by the screen: a demolish of a slot past `HanyEpuletLefoglalva[p]` (the exe reads the
block's unused records and counts `HanyEpulet` down): the tracer only names used slots and the port refuses others.
Unsaved and so not compared: 0x26's `KiMelyikBolygonVan`.
