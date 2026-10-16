# Real-time space battle: runtime verification (2026-10-16)

Spec: `re/rules/space-combat.md` §7. Port: `engine/crates/ig2-sim/src/spacebattle.rs`. Replay:
`tests/spacebattle_trace.rs`. Tracer: `tools/trace/run_spwc.py` + `spwc.js` (Frida 17, GOG `ig2.exe`,
`scenario/quick.scn`).

## Method
The agent gives every empire warships and fighters (half the fleets repeat one design; some hold 20–44
ships, so the battle packs several ships into one unit), then starts real-time battles between an AI fleet
and the human's (`SpaceWarIndit` with the "never lopsided" switch 0xB77238) and, with `--planets`, at a
human planet armed with defence ships and finished planet guns (building types 28..31). The game switches to
the battle screen and runs its own tick; the agent hooks `SpwC::SpaceWarFrissit` (exe 0x56dff0). The first
call of a battle records every region the tick reads or writes: the battle object (0x2D9C bytes), the units,
their weapon mounts and ship lists, fighters, planet guns, all 40 projectile forms with their pools and
sprites, the AI squads and their member lists, the 21 streams, `Hajok`, `Flottak`, `Tankok`, the battle
planet and its buildings, and the globals the tick reads (pause, MP, debug switches, story bonuses and
retreat flags, the screen fade, empire centres); static data: the live weapon table, the ship-type records,
the designs, the building hit points. Every call then records the changed byte runs ("step"), and the changes
between calls ("between": the UI frame). One battle at a time (the pools are global), battle speed 4
(`IdoSpeedSzorzoBattle` 0xBAE260), abandoned after 1,500 ticks. `--ai-human` lets the battle AI play the
human's side too; `--fuzz` randomises special pods, weapons (a third of the mounts get weapons 9..12),
reload timers, barrels, shields, experience, the sides' fighter aggression and losses (to provoke
retreats) and the story bonuses, and turns asteroids on (`asteroidak` 0xE72DD2). Ground wars after a won
planet battle are stubbed out.

The replay decodes each tick's memory into a `Battle` (pointers → indices), steps the port with a
`BattleWorld` over the recorded strategic records (`spacewar::CombatWorld` for `HajoDelete`/`FlottaDelete`/
`TankDelete`, the planet's ship list and buildings as raw bytes) and compares the decoded next state field by
field, plus all streams, ships, vehicles, fleets, the planet record and its buildings. Resync every tick.
Not compared: the uninitialised fourth float of copied vectors, the drawn orientation matrix and sprites
(only weapon 11 reads one back: compared for it), selection (UI), the ship lists' stale tails.

| Trace | Mode | Ticks (battles) |
|---|---|---|
| `spwc-ai-1` | `--ai-human` | 6,000 (2) |
| `spwc-fuzz-7` | `--ai-human --fuzz --planets` | 8,557 (2) |
| `spwc-fuzz-11` | the same, multi-ship fleets | 8,977 (7, 4 at planets) |
| `spwc-fuzz-23` | the same, weapons 9..12 favoured, ground troops | 8,704 (8) |

## Result
**32,238 ticks, bit-exact** (every field, every stream, every strategic record). 1,077 shots fired, 562
hits, 109 ships lost, 71 hits on planet buildings, 17 end checks that closed a battle. Seen at least once:
fighters in all states (chasing, overshooting, retreating, doomed), multi-ship units losing ships, planet
guns firing and destroyed, asteroids hit, shields, experience, all six pods (jammed, scanned, cloaked,
decoyed missiles 1,763 projectile-ticks), disabling (weapon 9) and takeover (weapon 10), attached beams,
weapon 11's area pass, AI squads gathering and attacking, retreats of units and fighters with the starmap
back-off, 31 of the 40 weapons in flight.

Fixes the replay found (all now in the spec):
- The end check is also gated by the screen fade (0xE72E14) and skipped once the battle is closed
  (+0x1fb8).
- `creatematrixfromaxis` computes `zz = z2·(z·s)`; the decompile showed `(z2·z)·s` (1 ulp in a heading).
- The path is kept as u16: a cell that wrapped below z = 0 is checked for line of sight as z + 65536.
- The experience bonus divides by the current ship count (+0x1b0); +0x1b4 is not an initial ship count but a
  hit-point divisor that stays 1.
- Between ticks the UI frame rewrites unit +0x184..+0x190 (overlapping the attacker pointer, which the
  tick re-derives before use).

Not exercised: chain-lightning arcs (weapon 12 fires but its hit chance is 0), the victory flight
(+0x1f30), MP, several battles at once.

## Mutation pass
38 single-point mutations of `spacebattle.rs`; **28 caught** (from 1 to all 32,238 ticks). Missed:
- Equivalent on these inputs: the summation order of `tavnegyzet` and the 2.0 cap in `vektorszog` (unit
  vectors never differ by more than 2), `(f32)((f64)v/2.5)` vs `v/2.5f` on the traced ship types; the battle
  AI's "own damage %" term, which is the same for every candidate (dropping it can't change the pick).
- Boundaries the traces never hit: the shield's last point, the doomed fighter's `<` vs `≤`, the path
  heuristic floor 0x8c vs 0x80, the last pool slot (no pool filled up), the gathering spread's 9/100
  thresholds, the end check by original vs current owner (no taken-over unit was among the last).
These stay pinned by the asm.
