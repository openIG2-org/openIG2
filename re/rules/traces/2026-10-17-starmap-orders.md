# 2026-10-17: the starmap's orders (`MULTIGET_Starmap`) against the exe

`tools/trace/run_replay.py <install> _local/saves/quick-1.sav _local/traces/replay-quick1-orders-s<S>-x16 --ticks
60|80 --orders 6|8 --orders-every 3|2 --seed S` (S = 2, 3, 4). Before save 0 the tracer turns on "always simulate"
(0xB77230, so attacks don't stop the clock on the battle screen), puts 2..5 stations on the human's first three
planets and makes two fleets of three ships and two vehicles each beside its home planet. From tick 1, every
`ordersEvery`-th `IdoNapValtozott` runs `ordersN` random messages through the exe's own `MULTIGET_Starmap`
(exe 0x59d930: its 8 `Multi_GetNextTipus` calls and 24 `Multi_Get` calls patched to stubs that hand over one staged
message; libc `srand`-pinned per batch, the seed reported) before the tick's save. The values are random but shaped
like the screen's (own fleets and planets mostly, any planet or fleet now and then; designs of the right class for
the queues; ships only between fleets). Each message is a `{"k": "sorder", "before", "libc", "type", "vals"}` record.

`engine/crates/ig2-sim/tests/save_replay.rs` applies them with `Game::starmap_order` after the tick's clock step and
compares the whole state with the exe's save:

| Recording | Ticks | Messages | Result |
|---|---|---|---|
| s2 | 60 | 120 | byte-exact |
| s3 | 80 | 320 | byte-exact |
| s4 | 80 | 320 | byte-exact |

Message counts over the three: 0x14 43, 0x15 175, 0x16 37, 0x17 50, 0x18 34, 0x19 71, 0x1a 16, 0x1c 113, 0x1d 134,
0x400 23, 0x403 6, 0x406 22, 0x407 17, 0x40f 19 (they also exercise the knock-on systems: splits, emptied fleets,
waypoints and patrols, colony ships and terraformers sent off, garrisons landing on and leaving battlefields,
production finishing heads early).

What the recordings found in the port (fixed):
- `FRISSIT_Starmap` sends `Multi_Send(0x1b, EN, f)` for the local player's empty fleets at each fleet's step end
  (`re/screens/starmap.md` §10.2); the port only deleted the AI's. A split's new fleet left empty is deleted by it.
- The exe's `FlottaNew` clears `+0x2a..+0x2d` with one dword: `+0x2c..+0x2d` (past the visibility bytes) were kept
  from a reused slot (`Fleet::field_2c`).
- A save holds `Waypointek` only when there are waypoints: a state that grew its first ones never wrote the chunk;
  new waypoint records carry uninitialised bytes (all of record 0, `+0xa..+0xc` of the others; `Fresh::Waypoint`).
- Garrison groups that grow (`RearrangeTankGroups` joins, `AddTank` into a group) may have their member arrays moved
  by `realloc`: the saved origin pointer `+0x19c` comes from the exe's save (`FieldTail::grown`).
- `DeleteTerraformalo` was missing (`World::send_terraformer`), and so were `FindNearestTerraformalo` and the
  station sale.

Not reproducible and not sent by the screen: a ship moved onto a planet (`0x1c` with `to < 0`): `HajoAtpakol(i, −p)`
counts it into `Flottak[−p]`, outside the table (the exe crashed a few ticks later in the first recording); the
tracer no longer stages it and the port skips it.
