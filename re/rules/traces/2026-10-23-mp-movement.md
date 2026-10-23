# Movement orders and sightings with a second human in MP (2026-10-23)

Spec: `re/rules/fleet-movement.md` §6.4 (U16). Port: `ig2-sim::radar` (`Starmap::refresh` with
`StarmapContext::viewers`), `ig2-sim::fleet` (`ContactLostNetwork` for every human), `Game::messages_for`.
Tracer: `tools/trace/run_fleet.py --mp-move --fuzz` + `fleet.js` `mpMove()`. Tests:
`engine/crates/ig2-sim/tests/fleet_trace.rs` (now also `fleet-mpmove.jsonl`),
`radar::tests::every_human_gets_its_own_machines_sightings_in_canonical_mp`,
`ig2-net/tests/lockstep.rs::two_humans_give_movement_orders_in_lockstep`.

## What the handler does in MP
`MULTIGET_Starmap` (ELF 0x719680, exe 0x59d930) never reads `MultiPlayerMode`; its `EN` tests are the issuer's screen
only (selection and transfer panel after 0x14, the 0x18 refusal line, the tutorial's event 0x15, UI flags). 0x15 has no
owner or issuer check. The starmap orders were already verified byte-exact through the exe's own handler
(`2026-10-17-starmap-orders.md`), so the MP question is the movement tick's local-human code.

## The other machine staged in SP
`--mp-move` plays the second human's machine: on half the refresh ticks and a quarter of the others, for the
`FRISSIT_Starmap` call only, `MultiPlayerMode` = 1, `EN` = another live empire with `PlayerKi` = 1 (empire 3 in
quick.scn; the real human is 6), `ProgramSebesseg` random 1..=4, and on refresh ticks staged attacks on that `EN`
(as `--fuzz-approach`). At the first staged tick two wormholes are opened with the exe's `AddWormhole` (0x53aec0) near
live fleets (quick.scn starts with traders only) and revealed, one with end B hidden. `--fuzz` adds random orders,
waypoints, patrols and follows every 40 ticks. Everything is restored after the call; the snapshot records `EN`,
`MultiPlayerMode`, `PlayerKi` and the speed inputs, so the replay runs the classic port with that tick's `EN`.

| Recording | Ticks | Staged MP ticks (refresh) | MP slow-downs | Wormholes | `+0x19+EN` 0→1 on MP refreshes |
|---|---|---|---|---|---|
| `fleet-mpmove.jsonl` | 1,500 | 380 (73) | 19 (×4/×8/×16 by `ProgramSebesseg`) | 2 (3 ends revealed) | 35 |

Result: every tick matches. Mutation: dropping the wormhole ends' sightings gives 174 mismatches.

## Lockstep (ours)
The exe only ever has one `EN`; canonical MP gives every human the sightings its own machine would give it
(`fleet-movement.md` §6.4). The unit test checks the projection (each human's bytes and warnings equal the classic pass
with `EN` = that human); the `ig2-net` test runs two humans (6 and 3) on `quick-1.sav` with move and waypoint orders
on both fleets (one order on the other human's fleet, which the handler carries out too) and a revealed wormhole:
20 turns hash-identical, both orders applied on both peers, both humans see the wormhole's ends. Mutation: classic
viewers (`EN` only) fails it.
