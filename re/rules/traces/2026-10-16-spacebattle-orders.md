# The human's battle orders (`MULTIGET_SpaceWar`): runtime verification (2026-10-16)

Spec: `re/rules/space-combat.md` §7.12 (the MP flow: §7.13, `multiplayer.md` §7). Port:
`engine/crates/ig2-sim/src/spacebattle.rs` (`Order`, `Battle::order`, `Battle::arm_all`, `Battle::screen_point`),
`Game::space_order`, `ig2_net::Command::SpaceOrder`. Replay: `tests/spacebattle_trace.rs` (records of kind `order` in
`spwo-*.jsonl`, together with the ticks around them). Tracer: `tools/trace/run_spwc.py --orders N --order-every K`
+ `spwc.js` (Frida 17, GOG `ig2.exe`, `scenario/quick.scn`).

## Method
The real-time battle tracer (`traces/2026-10-16-spacebattle.md`) starts battles between the human and an AI fleet and
hooks `SpwC::SpaceWarFrissit`. With `--orders N`, every K-th tick, before the tick, it stages N random orders through
the exe's own `MULTIGET_SpaceWar` (exe 0x5615b0) on the game thread: the handler's four `Multi_GetNextTipus` calls
(0x5615d9, 0x561a9e, 0x561ab9, 0x561c0b) are pointed at a stub that returns the staged type once, and only to a
call whose range holds it (so 0x44e/0x44f reach the second loop), and its twelve `Multi_Get` calls at a stub that
writes the staged values to the out-parameters (`[esp + 8 + 8i]`) until the list's terminating 0. Each order is an
`order` record with its type, values and the byte diff of every battle region (units, mounts, fighters, guns, pools,
squads, streams, `Hajok`/`Flottak`/`Tankok`, the battle planet and its buildings). Orders: moves to points the screen
can send (inside the arena less two cells, as `JobbClick` clamps), attacks (1 in 10 on −1), retreats, gun orders
(with `--planets`), the two cheats, fighter stances for any side, direct fire from a random mount, weapon masks; the
units are live five times in six, any owner.

The replay decodes the battle before the order, applies `Battle::order` (or `arm_all` for 0x75; −1 targets are
no-ops), decodes the exe's state after it and compares every field, all 21 streams and the strategic records, as for
the ticks; the ticks between the orders replay as before.

| Trace | Options | Orders | Ticks |
|---|---|---|---|
| `spwo-2` | `--orders 3 --order-every 5 --planets`, seed 202 | 2,046 | 3,395 |
| `spwo-3` | the same with `--fuzz`, every 4, seed 303 (the game froze after 69 ticks) | 53 | 69 |
| `spwo-4` | `--orders 2 --order-every 6 --fuzz`, seed 404 | 409 | 1,228 |

## Result
**2,508 orders byte-exact, and the 4,692 ticks between them**: move 555, attack 509, retreat 84, attack a gun 127,
gun target 98, the weapon cheat 160, the explode cheat 42, fighter stance 181, direct fire 427, weapon mask 325;
71 of them were −1 targets the handler ignores. Seen around them: cloaked, jammed, decoyed and taken-over units,
attached beams, chain lightning, planet guns firing and destroyed (63 building hits), scenery, 81 ships lost, seven
battles over.

A first recording (`spwo-1`, not kept) also sent points outside the arena; the handler doesn't clamp, and in the
next ticks the exe's path search diverged from the port's (path-search bookkeeping of the unit). The screen never
sends such a point (`JobbClick` clamps), so it isn't a reachable input; `Game::space_order` applies the screen's clamp
to every peer's move (`multiplayer.md` §7).

## Mutation pass
18 single-point mutations of the order code; **16 caught** (from 3 to 236 mismatches, or a panic). Missed:
- equivalent here: `fegyverhozzaad`'s last-match search vs the first (a unit never holds two mounts of one weapon);
- not reached: `moveship`'s disabled-unit guard (no move order went to a unit disabled by weapon 9). It is the same
  guard the battle AI's moves pass through, pinned by the asm (exe 0x57c7fb).

## Not traced
The MP side itself (0x44d and the in-progress flag need two machines: tested in lockstep by
`ig2-net/tests/lockstep.rs`), the editor branches of `moveship`, message 0x6f (no sender), several battles at once.
