# Trace report: colony side systems (2026-10-15)

Spec: `re/rules/colony-side.md`. Port: `engine/crates/ig2-sim/src/colony_side.rs`.
Replay: `engine/crates/ig2-sim/tests/colony_side_trace.rs` (skips without `IG2_INSTALL_DIR` and traces).
Agent: `tools/trace/colony_side.js` + `run_colony_side.py` (Frida 17, GOG `ig2.exe`, `scenario/quick.scn`).
Traces (game state, local only): `_local/traces/side-*.jsonl`.

## Sessions
| File | Mode | Content |
|---|---|---|
| `side-economy.jsonl` | `economy` | 220 economy steps as played (2327-09 → 2330), 44 monthly |
| `side-economy-fuzz-{1,2,3,4}.jsonl` | `economy --fuzz --seed 1..4` | 67 + 45 + 42 + 57 steps; low morale and unrest 27..31 (rebellions), autobuild modes/aggressiveness/counters, money pools and reserves, contacts, spies (empires up to 18), visibility, `+0x64` 0..3, `+0x186/+0x187`, emptied and retyped building slots, mass spy centres, `StoryAdat+0x24/+0x9c` |
| `side-elfoglal.jsonl` | `elfoglal` | 80 direct `BolygoElfoglal(p, 0..10)` on owned planets |
| `side-fellazad{,-2}.jsonl` | `fellazad` | 120 + 250 direct `BolygoFellazad(p, to, force)`; unforced calls at morale 20..2019 |
| `side-colonise.jsonl` | `colonise` | 60 colonisations of free planets (`BolygoElfoglal`, `Generate_Planet0`, `rndlog(1,0,0)`, hub via `EpuletNewSearchLocation`) |
| `side-money{,-2}.jsonl` | `money` | 2,800 calls of `MoneyMennyi`, `MoneyHozzaadOld` and `GetGyartasAr` on fuzzed pools, reserves and shares; `MultiPlayerMode` and the empire's race toggled around single calls |

## Results (all bit-exact)
- **Economy**: 431 steps (85 monthly) replayed as a whole: every planet record and building slot, the
  21 RNG streams and the weighted-choice state, `BolygoNeededMoral`, unrest, occupied-slot counts,
  per-planet strengths, labs, spy capacity/centres (incl. per planet), trade ports, `FullGDP/FullTax`,
  tax/trade income this/last year, all 19 treasuries (f64 bits), the whole tech table (52 KB) and the
  contact matrix, `AutoBuildMitEpit`. Covered: 54 rebellions mid-step, 161 autobuild purchases,
  178 techs captured, 8 year-ends.
- **Direct calls**: 80 conquests, 370 rebellions (outcome 1: 5, 2: 20, 3: 7, 4: 338), 60 colonisations,
  139 techs captured — same comparison as above.
- **Money**: 2,800 calls (402 price checks, 113 for race 5), results and pools bit-exact.
- Tables: `autobld1..10.txt` parsed with the exe's `AutoBuildNevek` equal the live
  `AutoBuildEgyebEpitendok`/thresholds/priorities; `AutoBuildHanyasProblema/Min/Max` and `pGyartas0`
  read from the exe file equal live memory; `TalalmanyEroHP` equals the formula in spec §7.

## Findings (spec updated)
1. **Problem 1 (electricity) uses the building pass's locals**, not `+0x48/+0x4c` (a construction
   refresh rewrites those mid-step). A latent bug in the earlier colony port; fixed (`colony-economy.md` §5.2).
2. **`+0x64` is tested as a whole byte** in the exe (`cmp byte [esi+0x64], 0`); the ELF tests bit 0.
   Pinned with fuzzed values 2 and 3.
3. **Trade-port overflow**: the autobuild purchase increments `HanyTradePortVan[owner]` unbounded; for
   the rebels (owner 10) it writes `HanyKemKozpontBolygonkent[0][1]`. Kept.
4. **Undefined read in autobuild**: with no power plant researched, the energy branch picks building 0 and
   tests `inactive[0]`, an uninitialised stack slot. Both outcomes were observed (no purchase; or a
   failed purchase that sets the counter to −1000 in 9 steps of fuzz-4). Modelled as `ZeroPick`.
5. **Fresh building slots** keep flag bit 7 of uninitialised `realloc` memory (`flags &= 0x80`); ignored
   for appended slots.
6. `HanyKemKozpontBolygonkent` is never reset (accumulates every step), as in the ELF.
7. `TalalmanyEroHP[t] = (int)sqrt((f32)firepower × (f32)hp)` (computed by `InitNewGame`).

## Harness lessons
- `rndlog` called from an agent needs a source path with a backslash (it strips the directory; no
  backslash → NULL dereference).
- Frida's `NativeFunction` doesn't read an x87 `st0` return on this ABI (`MoneyMennyi` returns
  `float10`): call it through an `X86Writer` thunk that does `fstp qword [out]`.
- `serde_json` needs the `float_roundtrip` feature for exact f64 parsing (else 1-ulp errors).
- Direct-call modes must apply their set-up **before** the "before" snapshot.
- Conquering free, building-less planets with `BolygoElfoglal` crashes the game a few calls later: use
  owned planets for conquest and the colonisation sequence for free ones.

## Mutation check
42 one-line mutations of `colony_side.rs`/`colony.rs` (scratchpad script, each run against all traces):
**32 caught**. The 10 not caught, and why:
- f32 vs f64 rounding that never crossed an integer in the traced data, so it's pinned by the asm only:
  the rebellion distance grouping and its f32 weight (`addss`/`divss` 0x53bd25..0x53bd83), the recreation
  score (0x476de4..0x476e12).
- Exact-equality edges never hit: rebellion suppression `r < morale/20`, autobuild energy test
  `demand < production`, purchase `price <= money`.
- Equivalent on the shipped data: aggressiveness 3 `×0.66` (every `autobldN.txt` threshold is 10, so
  6 either way).
- Rare branches the fuzz didn't reach: the score multiplier `(aggr + 2)` vs `(aggr + 1)` (never changed
  a winner), the lab-type ratio `L2·18 < L1·10`, the spy-capacity cap of 12.

## Not covered
Multiplayer economy steps (only the money MP path is exercised); a real "no room" placement failure;
`EpuletNew` yields from story values `st_v(0x5FB/0x60F)`; the epidemic clean-up's `+0xba` side effect
(`BolygoInfoRemove`, not modelled); fleets/garrisons/ground-war effects of a change of owner (events only;
the replay checks that attacking fleets lost their orders).
