# ADR 0003: Simulation clock and random numbers

Date: 2026-10-15. Status: accepted. Evidence: `re/rules/time-and-rng.md` (verified at runtime).

## Context
The original runs a fixed 20 Hz simulation tick (50 ms, catch-up clamped to 500 ms) and a 100 Hz
input/UI tick, with game time `Ido` in 1/50-day units. Seeded randomness uses 21 rotated-LCG streams
(`rndold`), which we reproduced bit-exactly against 1,114 traced calls. But many gameplay paths
(ground combat damage, diplomacy decisions, trade, galaxy generation) use unseeded libc `rand()`,
which can't be reproduced and isn't synchronised in multiplayer.

## Decision
1. **Sim tick = 50 ms, fixed**, as in the original, in both rulesets. `Ido` steps and speed
   multipliers stay exactly as recovered (SP 1/4/16, MP 4/8/16). The 500 ms backlog clamp is kept for
   single player. In multiplayer the tick is driven by lockstep turns, not wall time.
2. **Rendering is decoupled**: it runs at any refresh rate and interpolates between sim states.
   Input is sampled per 10 ms UI tick (as in the original) and fed to the sim as commands at tick boundaries.
3. **RNG**: `ig2_sim::rng::Streams` implements `rndold` bit-exactly for streams 0–20 with the classic
   stream allocation, and scenario/save `RandSeed` values load into streams 1–20.
4. **Deviation (both rulesets): every gameplay use of libc `rand()` goes to dedicated seeded
   streams** (21+, one per subsystem), with the same `% n` distribution on 15-bit outputs. Cosmetic
   randomness uses a separate RNG that never touches sim state. That makes the whole sim deterministic
   from (scenario, seeds, command log), which lockstep multiplayer and replays need. It deliberately
   gives up bit-identity with the original for those call sites, which was never reproducible anyway
   (the original seeds `rand()` from `SDL_GetTicks()`).

## Consequences
- Golden tests can replay traced RNG sequences exactly (`ig2-sim/tests`).
- Classic-mode outcomes match the original statistically, and bit-exactly wherever the original was itself deterministic.
- MP desyncs from unsynchronised `rand()` (a likely cause of the original's fragility) can't happen.
