# Transporters of the real-time ground war: runtime verification (2026-10-16)

Spec: `re/rules/ground-combat.md` §11.16. Port: `engine/crates/ig2-sim/src/ground/transport.rs` (`War::dropships`,
`dropship`, `place_all_tanks`, `place_tank`, `unplace_tank`), `placement::locate_pos` (the spiral with a test).
Replay: `tests/ground_rt_trace.rs::transporters_match_trace` (the battle-tick, `FRISSIT_Grwar` and AI replays
also run the new traces). Tracer: `tools/trace/run_ground_rt.py --mode battle --transport --place-after T
--retreat-after T` + `ground_rt.js` (Frida 17, GOG `ig2.exe`, `quick.scn`).

## Method
- The tracer hooks the game's own `WF_Hordozo::Refresh2` (exe 0x48e0a0) on the battle planet: every pass of a
  dropship that isn't away is a `hor` record like the `WF::MI`/`WF::Refresh` ones (libc pinned by `srand`,
  region diffs of the battlefield, the units, the lists and the streams; `cc` = `ColonyCounter`, `ret` = shot
  down). The passability maps and the landing zone are carried over unread in this hook (a pass can't touch
  them), which made it fast enough for big battles.
- `--transport`: the first attacking design gets the transporter pod 0x9f (slot 0, sometimes slot 1 too) and
  lands as **one** vehicle, so `RearrangeTankGroups` never splits its partner (the split partner with a garbage
  carrier is what crashed the exe, §8). Every staged carrier had its partner linked both ways; no crash in
  23 battles.
- `--place-after T` (with `--human`): from tick T, every 5 ticks while the human has a unit whose dropship is
  away, a landing order between two ticks (`order`: `PlaceAllTanks` 40 %, `PlaceTank` 30 %, `UnplaceTank`
  30 %, random cells including out-of-range ones), called through the exe and recorded like a call.
- `--retreat-after T`: the attacker's retreat flag `WF+0x89f + side − 1` is set between ticks; `WF::Refresh`
  then calls `Retreat` and the dropships come for the units.

| Trace | Options | battles / ticks | dropship passes | orders |
|---|---|---|---|---|
| `grt-transport-1` | `--small --human --transport --place-after 10 --retreat-after 200 --ticks 400 --seed 7101` | 4 / 1,577 | 2,057 | 12 |
| `grt-transport-ai-2` | `--transport --fuzz --retreat-after 120 --ticks 350 --seed 7202` (time limit) | 2 / 370 | 2,060 | – |
| `grt-transport-human-3` | `--small --human --transport --fuzz --place-after 5 --retreat-after 260 --ticks 420 --seed 7303` | 4 / 1,695 | 3,308 | 30 |
| `grt-transport-ai-4` | `--transport --fuzz --retreat-after 120 --ticks 300 --seed 7404` | 3 / 767 | 9,598 | – |
| `grt-transport-human-5` | `--human --transport --fuzz --place-after 5 --retreat-after 200 --ticks 320 --seed 7505` | 4 / 1,016 | 8,134 | 30 |

## Result
- **25,157 dropship passes and 72 landing orders (35 `PlaceAllTanks`, 24 `PlaceTank`, 13 `UnplaceTank`),
  all exact**: every unit field (flags, owner, the dropship's offsets, height and state), the lists, the status
  cache, streams 0 and 2, the libc sequence. State changes seen: 0x1a → 0x14 (delivered, 100), 0x14 → 0x15
  (away again, 81), 0x42 → 0x4c (retreat pick-up, 55), 0x42 → shot down (15, with the wreck's debris), 0x4c →
  0x4d (gone with the unit, 36), 0x22 → 0x2c (collected, 4), 0x2c → 0x2d (1), 0x62 → 0x6c (79: collected while
  a retreat was pending).
- Every pass ran at `(int)ColonyCounter % 5 == 0` (the UI cadence of §11.16).
- The other replays over all traces stay exact: 29,461 battle ticks, 16,324 `FRISSIT_Grwar` passes,
  16,953 `WF::MI` calls (768 with the known heap over-read target pick, `ground-ai.md` §8).
- The one port error the traces found: the order bits are tested deliver → collect → retreat (the first port
  tested retreat before collect; 23 mismatches on state 0x62).

## Not covered
MP (the arrival half doesn't run there; untraced), whether a `PlaceTank` failed only its landing-zone test (unit left moved
but away) is not singled out, a dropship's NaN speed (only after a positive
height, which no code writes).
