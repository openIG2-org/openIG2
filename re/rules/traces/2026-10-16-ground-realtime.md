# Ground combat (real-time battle): runtime verification (2026-10-16)

Spec: `re/rules/ground-combat.md` §11. Port: `engine/crates/ig2-sim/src/ground/` (`realtime.rs`, `battle.rs`,
`tick.rs`, `path.rs`, `voices.rs`) and `placement::{landing_zone, locate_tank}`. Replay:
`tests/ground_rt_trace.rs`. Tracer: `tools/trace/run_ground_rt.py` + `ground_rt.js` (Frida 17, GOG
`ig2.exe`, `scenario/quick.scn`).

## Method
- **`place`** (`grt-place-1`, `grt-place-fuzz-3`): stages landings as in the strategic tracer
  (`2026-10-16-ground.md`), but real-time (`AddTanksToPlanet(f, p, 0)`), then `StartGrWar(p)`, with
  snapshots of every group (positions, cells, heights, matrices, speeds, fire ranges, reloads), the landing
  zone and the field. `--fuzz` adds ground pods and on-screen landings (`LoadMesh`).
- **`turret`** (`grt-turret-1/2/4`): builds defence buildings with `EpuletNewSearch` and the exe's
  `EpuletNew` path, then `AddBuildingTurrets`/`BuildingBuilt`, and snapshots the turrets.
- **`battle`**: stages a landing, starts the war and opens the planet's colony screen; from then on the
  game runs its own 20 Hz tick. The agent hooks `FRISSIT_Grwar` (0x4747c0), `WF::MI` (0x486d40) and
  `WF::Refresh` (0x490890) and records **byte-region diffs** on entry and exit of each: the battlefield, every
  alive group with its hit-point, origin and waypoint arrays, the three projectile lists and every object in
  them, the five passability maps, the planet, its buildings, the landing zone, `Tankok`, the fleets, the
  stream seeds, the per-empire "watching" table and the colony speaker's voice lines (speaker, kind, samples,
  cooldown; `Lang`; the mute byte 0x9B2EBC). libc `rand()` is pinned with `srand(S)` at each call, and each
  call also records the two values it would have seen (so `FRISSIT_Grwar`'s own draws can be counted).
  Options: `--small` (small forces), `--human` (the human attacks: the end countdown runs), `--end-after T`
  (between two ticks the attacker's groups are removed with the exe's own `RemoveTankGroup`/`RemoveTarget`),
  `--destruct-after T` (the self-destruct timer `WF+0x334` set to 3), `--fuzz`, `--speed`.
  Two game instances record in parallel without trouble.

| Trace | Options | Battles | `Refresh` calls | Speed |
|---|---|---|---|---|
| `grt-battle-3` | full forces | 1 | 2,223 | 4 |
| `grt-battle-fuzz-51` | full, fuzzed | 1 | 1,049 | 4 |
| `grt-battle-52` | full | 1 | 947 | 2 |
| `grt-small-21` | small | 3 | 1,590 | 1 |
| `grt-small-fuzz-22` | small, fuzzed | 4 | 3,397 | 1, 4 |
| `grt-small-end-31` | small, `--end-after 150` | 4 | 2,300 | 1 |
| `grt-small-end-fuzz-32` | small, fuzzed, `--end-after 150` | 4 | 3,635 | 1, 4 |
| `grt-small-human-41` | small, human attacker, `--end-after 150` | 5 | 952 | 1 |
| `grt-small-destruct-42` | small, `--destruct-after 120` | 2 | 1,404 | 1, 4 |

The replay rebuilds the state before every call from the init snapshot and the diffs, runs the port's
`War::refresh` (or `War::grwar_planet` for `FRISSIT_Grwar`'s own part, up to the first nested call), and
compares every unit field (cell, position, height, the three matrices, angles, speeds, reloads and their
counters, orders, targets, auto-aim, path counters, waypoints, flags, hit points and origins, moved/fired),
the list objects slot by slot, the field globals (countdown, status, finished flag, empire timers, watching
flags), the buildings, the voice lines, stream 2 and the libc sequence after the call.

## Result
- **Part 1: 140 real-time landings (4,688 groups placed, 70 `StartGrWar`) and 170 defence buildings (413
  turrets) exact.**
- **17,497 `WF::Refresh` ticks and 4,354 `FRISSIT_Grwar` passes exact**: 25 battles at speeds 1, 2 and 4;
  33 groups destroyed, 109 projectiles and 586 debris pieces made, 73 voice lines played, buildings fallen
  to fire, 2 self-destructs (every building and group gone), 4 end countdowns into `GroundWarFinished`
  (owner checked), `WF::Collapse` passes, idle fire, attack-moves and path searches throughout.
- Tolerated by documented cause, nowhere else:
  - 6 ticks where the exe freed fresh debris in the same tick: debris lifetimes are **uninitialised heap
    bytes** in the exe (§11.12); everything else in those ticks matches (the other list-B objects as a
    multiset, the units, stream 2, libc).
  - 69 passability maps built inside a call (the planet's first path search, `GenerateMap` not ported,
    §11.11): the replay takes them from after the call. (Since superseded: the port generates them,
    `2026-10-16-atjaras.md`.)
  - 4 of ~14,400 gun-elevation solutions differ by 3–67 ulps (`+0x1c8`, render-only: no battle code reads
    it). The inputs are identical and the chain is the asm's (all scalar SSE, constants checked); one ulp
    anywhere upstream of the two cancellations reproduces the exe's value, but which one is not known
    (√q is not near a rounding boundary; `tan` can't move it that far). Open.

## Findings (spec errors the traces caught)
- `RemoveTarget` also marks the affected units **moved** and clears the projectiles' targets and shooters
  (not for bombs); without it `GetNearestEnemy` picks nothing in the tick a target dies.
- The colony screen's **voice lines** cost a libc draw only off cooldown (`Speeches::Play`; cooldown 10,
  counted down per UI frame): hit voices (`rand() % 15 == 0`) and death voices (`RemoveTankGroup`, muted
  while groups merge). The death voice needs the alive cache, which `WF_Tank::Damage` clears at a death.
- `EpuletDelete` on the colony screen leaves a ruin (`Remove3dBuilding` → `Rom::Rom`: 24 pieces of debris);
  a turret's building falls with 16 slow pieces, a building hit directly with 24 (not 20 as assumed).
- f64 where the decompile showed f32: `ShotOne`'s tolerance, ±π wrap, snap test and turret step; the
  rocket speed factors; the stealth jitter product.
- Attackers removed between ticks don't end an AI-against-AI battle: `GetStatus` starts the countdown only
  when the human is involved (§5.1). `FRISSIT_Grwar` doesn't end it either.

## Mutation pass
40 single-point mutations of the tick code; **28 caught** (unit tests or the replay of 15,501 ticks; up to
14,641 ticks wrong). Missed:
- Not exercised by these traces: stealth targets (the jitter), slow rockets (kind 6), the self-destruct
  countdown's `> −1` vs `> 0` (equivalent from 3), the end countdown's length (it started outside the
  replayed calls), the voices' race offset (the human is race 6: no offset), the path search's limit floor
  and its full heap, the `ShotOne` snap and ±π boundaries, the shortest-turn tie, `GetNearestEnemy`'s
  "has fired" test (every unit had fired).
- Not observable by design: the burst's slow factor (debris is compared by kind only: its velocity is
  cosmetic and its lifetime garbage).
These stay pinned by the asm.

## Not covered
Air strikes (fighters, bombs), transporters, `WF::MI` (separate report), `GenerateMap`, the laser, the
story mad tank, MP, `Refresh2`.
