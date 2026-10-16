# Air strikes of the real-time ground war: runtime verification (2026-10-16)

Spec: `re/rules/ground-combat.md` §11.15 (and §5.5 `WF::DamageBuilding`), `re/rules/ground-ai.md` §5.7.
Port: `engine/crates/ig2-sim/src/ground/battle.rs` (`air_strike_objects`, `add_bomb`, `add_fighters`,
`fighters_step`), `ground/ai.rs` (`shot_spec_weapon`), `ground.rs` (`turrets_share`). Replays:
`tests/ground_rt_trace.rs::{ground_ai_matches_trace, battle_tick_matches_trace}`. Tracer:
`tools/trace/run_ground_rt.py --mode battle --ai-fuzz` + `ground_rt.js` (Frida 17, GOG `ig2.exe`, `quick.scn`).

## Method
The tracer already recorded the whole battlefield `WF(p)` (0x8ec bytes: the five fighters at `+0x1f4`, the
wave flag `+0x32c`) and every list-B object, so the three `grt-ai` traces of the ground-AI recordings hold their
air strikes. The replays now read the fighters and compare them with the projectile lists after every
`WF::MI` call (the strike's objects) and every `WF::Refresh` call (flight, bombs, blasts); the battle-tick
replay now also runs the `grt-ai` traces. Two new traces with strikes armed often:

| Trace | Options | `WF::MI` / `WF::Refresh` calls |
|---|---|---|
| `grt-ai-71` | `--ai-fuzz --ai-fuzz-every 6 --seconds 540 --seed 71` | 398 / 1,254 |
| `grt-ai-72` | `--ai-fuzz --ai-fuzz-every 6 --ticks 300 --seconds 520 --seed 72` | 782 / 833 |

Tracer fix: the AI fuzzer wrote its perturbations inside `WF::MI`'s onEnter, so they landed in the diff the
replay attributes to `FRISSIT_Grwar`'s own part (570 false "grwar unit" mismatches). `FRISSIT_Grwar`'s diff
now goes out first as an `aifuzz` event; for the older `grt-ai-61..63` the grwar check masks the fuzzed
fields (weapons, air units, hit markers).

## Result
- `WF::MI`: 11,527 calls, 11,148 exact, 379 with the known heap over-read target pick (`ground-ai.md` §8);
  **146 air strikes (74 big-ship, 72 fighter) with every bomb and fighter exact**.
- `WF::Refresh`: **24,036 battle ticks, all exact** (1,765 with a fighter wave up, 2,062 with air-strike bombs
  falling; 26 with heap-garbage debris slots as before); `FRISSIT_Grwar`: 10,896 passes, all exact.
- Mutation check: the speed formula of the big-ship bomb (f64), the fighters' start offsets and velocities
  (f64), the bomb cooldown (≥ 10) and the phase boundaries are pinned; the f64 division by 3.333 in the
  fighters' bomb lead is asm-only (the int truncation of the aim hides it).

## Findings
- Bombs hit defence buildings directly, which exercised `WF::DamageBuilding` (exe 0x46d270) for the first
  time: the port had only reported the hit as an event. A direct hit on a building of kinds 0x1c..=0x23 is
  split among its placed turrets (`dmg / turrets left`, `WF_Tank::Damage`), and the building's damage is then
  reset to its hit points minus the turrets' remaining hit points (now ported, spec §5.5). First seen in
  `grt-ai-61` tick 54: four 15 000-hp turrets dropping to 3 750 after five bomb blasts.
- When that kills the last turret, `RemoveTankGroup` deletes the building inside `DamageBuilding`; the
  recompute and `WF::Damage` then read kind 0 (`hp[0]` = 10 000 in the shipped table) and delete the emptied
  slot again: the building count drops twice [original bug], while the second `Remove3dBuilding` finds no
  3D model and leaves no ruin (one burst of 24 pieces = 121 libc draws on screen).
- A bomb blasts every list-B round while its height is within the band (100 / 40) of the ground, so one bomb
  can hit the same units on consecutive rounds.
