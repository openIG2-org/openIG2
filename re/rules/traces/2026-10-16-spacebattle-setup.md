# Real-time space battle, setup and end: runtime verification (2026-10-16)

Spec: `re/rules/space-combat.md` §7.10 (setup), §7.11 (end). Port: `engine/crates/ig2-sim/src/spacebattle.rs`
(`new_battle`, `Battle::finish`, `Battle::ground_war_due`). Replay: `tests/spacebattle_trace.rs`
(`battle_setup_matches_trace`; the tick replay `battle_ticks_match_trace` runs over the same traces). Tracer:
`tools/trace/run_spwc.py` + `spwc.js` (Frida 17, GOG `ig2.exe`, `scenario/quick.scn`).

## Method
Every `SpaceWarIndit` call the agent makes is wrapped in two snapshots: `presetup` just before it and `setup`
right after it. Each holds the battle slot `SpaceWar_NewBattle` will use (the battle object, units, mounts, ship
lists, fighters, guns, pools, squads), the streams, `Hajok`/`Flottak`/`Tankok`, the battle planet and its
buildings, the designs, the live weapon table, and the globals the setup reads: relations, `PlayerKi`, `EN`, the
MP flag, the asteroid and editor switches, `spFlottak`/`spKozeliBolygo` (0xBAA1C0..), `spFoldiharcElottiUrharc`,
the story pacts (0xF26150..0xF261EF), the ship-type table `tipusnevek` (0x91F488, 0x2d00 bytes), `alapfegyverek`
(0xE5E08C), the building techs and the 48 bytes before `Hajok`. The hooks can't see inside the agent's own
`NativeFunction` call, so the replay takes the strategic state from `setup` (`SpaceWarIndit` only clears orders,
rolls the story and the diplomatic incident before `SpaceWar_NewBattle`; none of that is read by the setup and
the setup changes none of it) and stream 6 and the slot from `presetup`. Leaving the battle screen records the
end's globals (`SpWarKiNyert`, `SpWarVesztesVisszaVonult`, `SpWarYouWasInBattle`, `SpwFlottaNum`).

Modes: `--join` lets another AI fleet join right after a battle is created (a join while the battle is shown
reloads the graphics from inside our frame hook and crashed the game), `--planets` arms human planets with
defence ships and guns, `--fuzz` turns asteroids on (and fuzzes the battle after its setup), `--fuzz-designs`
writes random weapons, shields and special pods into the participants' designs (quick.scn's have none), raises
some fighter wings to 30..99 (more than 60 in a battle scales the groups), and adds armour buildings, skipped
techs and switched-off buildings to the planet.

The replay rebuilds the ten slots (the slot before a new battle from its kept fields, a joined battle decoded
whole), runs `new_battle` with the formations from the install and the ship-type table from `presetup`, and
compares the slot field by field with the exe's decoded `setup`, plus all 21 streams and the whole ship-type
table (the footprints `AddUjUrhajo` writes). Masked as uninitialised in the exe: the fourth word of cells and
vectors, waypoints beyond the count, unit +0x194, the fighters' turn, an asteroid's model, the timers of
armour-only guns, the bytes of free projectile slots (checked as free). The ig2-rules table equals the live one
at each session's first battle.

| Trace | Mode | Setups (joins) | Ticks |
|---|---|---|---|
| `spwc-setup-101` | `--planets --join --ai-human` | 27 (13) | 4,471 |
| `spwc-setup-202` | `--fuzz --planets --join --ai-human` | 23 (9) | 5,139 |
| `spwc-setup-303` | the same as 101, long battles | 9 (4) | 8,176 |
| `spwc-setup-404` | `--fuzz --planets --join --ai-human` | 21 (9) | 3,655 |
| `spwc-setup-505` | `--fuzz-designs --fuzz --planets --join --ai-human` | 7 (3) | 907 |
| `spwc-setup-606` | `--fuzz-designs --planets --join --ai-human` | 31 (13) | 3,700 |
| `spwc-setup-707` | the same, fighter wings raised | 27 (11) | 3,644 |

## Result
**145 setups, bit-exact** (62 joins; 2,298 units, 594 of them with several ships, 687 defence ships; 2,792
fighter groups, the 60-fighter scaling seen; 725 planet guns including armour-only ones; 353 asteroids placed;
shields 0x31..0x36 and all six pods; formation turns for one and two empires already on the field). **5 natural
ends** (winner, participants, the human's presence) match. One setup is undecidable (below). The tick replay
over the same traces: 29,692 more ticks, bit-exact (61,930 with the earlier traces).

Fixes the replay found:
- `AddFleet`'s first loop also zeroes each side's fighter count (+0x64), so a join counts only the newcomers'
  fighters (the decompile's pointer arithmetic hid it).
- A defence ship (`−k`) without a formation slot merges by `Hajok[−k].fleet`, which reads the heap bytes before
  the array: two traces decided it two different ways. Undefined; the port uses `−k` and the replay passes the
  recorded bytes when it has them (`SetupInput::defence_fleets`; one setup in `spwc-setup-303` predates them).
- The tick (found by the new traces): weapon 11's area damage and weapon 12's burn hit a group once per ship
  and re-read the ship count every round: a group wiped by the first round takes no more damage. The port had
  fixed the count before the loop.
- The tick: an asteroid taken over by weapon 10 that then dies runs `clearship` on its ship 0, `HajoDelete(0)`
  and the fleet in the unused `Hajok[0]` (garbage, past `Flottak`) [original bug]; the replay's world is
  bounds-tolerant there.

Quirks pinned by the traces: the AI and aim timers and the winner carry over from the slot's previous battle
(seen 10.0 after a battle); joins at a planet duplicate its guns and defence ships; `SpaceWarIndit` picks a
nearby planet of a participant for a fleet battle (`spKozeliBolygo` = 19 for a fleet-only start); the story pact
is one-sided when the pact empire's index is below the human's (row order); `alapfegyverek` is all zero.

Not exercised: Zachkin's pact, three or more empires on the field before a side's placement (the exe's
direction is uninitialised), the editor, MP, `GroundWarIndit` after a won planet battle (stubbed by the
tracer), a full unit array (119+).

## Mutation pass
35 single-point mutations of the setup code; **27 caught** by the setup replay (from 1 to 139 of the 145
setups: the footprint, slot classes, design types, `helykeres`'s class range, the formation turn and both
direction cases, slot closing, ship merging, the look roll, the shortest-range rule, shield shares, both sorts,
the fighter split, scale and scatter, gun placement, hits and armour, side centres, the fighter-count reset, the
asteroid draws, the kept AI timer, the asteroid placement box, the join's clearing of the list). Missed:
- Equivalent on these inputs: `cell_z` vs `cell` in the vertical footprint (both 3), the last vs first mount of a
  weapon (a unit never holds two mounts of one weapon), the merge's model test (every candidate of the same
  type had the same model).
- Boundaries and inputs the traces never hit: `<` vs `≤` when slots close (distances are multiples of 3, the
  limits aren't), Zachkin's and the story pacts (never set in quick.scn), more than 6 defence ships (the tracer
  arms planets with 1..6), the defence-ship merge fallback (the one undecidable setup is masked).
These stay pinned by the asm and the unit tests.
