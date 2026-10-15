# Colony economy (`FRISSIT_Planet`)

Status: **verified at runtime** (§12): 103 traced economy steps, 1,043 planet checks and 1,516 logged
RNG rolls reproduced exactly by `engine/crates/ig2-sim` (`colony.rs`, `tests/colony_trace.rs`). Evidence: ELF `ig2` (-O0)
`FRISSIT_Planet(bool, int)` @ 0x4e6ef0 (source `../sources_base/colony.cpp`), and the shipped exe
`FRISSIT_Planet` @ **0x474930** (identified by the same `colony.cpp` line numbers at its `rndlog` call
sites, e.g. `push 0x12ef` @ 0x4766c3). The exe implements the same algorithm; its floating-point
precision differs in places (§10). **The exe is normative.** No rule-table values are given here; the
engine reads them from the user's `ig2.exe` (ADR 0004). Constants written inline in the code are part
of the formula and are listed.

Problem names are the game's own (`txt17` group 17/4, strings 21–40, loaded into `Problemak_Nev`).

## 1. When it runs
| Call | Effect |
|---|---|
| `FRISSIT_Planet(true, 0)` on a day change with `Nap % 6 == 1` (days 1, 7, 13, 19, 25; `time-and-rng.md` §1) | **economy step**, all planets |
| `FRISSIT_Planet(false, 0)` every 60 ticks while paused on the colony screen | display refresh: building pass (§4) and production (§8); also the `only == 0` prologue (global-morale decay, per-empire resets) |
| `FRISSIT_Planet(false, p)` from inside the economy step when a building on planet `p` finishes (§9) | refresh of planet `p` only |

"Monthly" below means `Nap == 1 && full && only == 0` (ELF `local_579`; exe `bVar26`).

## 2. Data
### 2.1 Planet (`BolygoTipus`, 0x188 bytes, `Bolygok[1..=HanyBolygo]`, exe 0xB85EA8)
| Offset | Type | Field |
|---|---|---|
| +0x20 | u8 | owner empire (0 = none) |
| +0x21 | u8 | population race |
| +0x22 | u8 | surface type (index into `FajBolygoViszony[race][surface]`) |
| +0x24 | i32 | base production capacity ("richness", `GetBolygoGazdagsag`) |
| +0x28 | i32 | population |
| +0x2c | i32 | population change at the last monthly step |
| +0x30 | i32 | morale, 0..=999 (displayed /10 as %) |
| +0x34 | u8 | tax level 0..=7 |
| +0x38 / +0x3c | i32 | GDP accumulator / last month's GDP |
| +0x40 / +0x44 | i32 | tax accumulator / last month's tax |
| +0x48 / +0x4c | i32 | energy produced / energy demanded |
| +0x50 | i32 | workers demanded |
| +0x64 | u8 | non-zero (the exe tests the whole byte, `cmp byte [esi+0x64], 0` at 0x4755a3/0x47733f; the ELF tests bit 0): no unrest counting (SP), never abandoned |
| +0x68 | i32 | autobuild counter, +1 per economy step, max 100 |
| +0x6c / +0x6d | u8 | autobuild mode / autobuild aggressiveness |
| +0x70 / +0x74 | i32 | ship / vehicle production capacity |
| +0xb8 | i16 | epidemic (virus) strength, 0 = healthy |
| +0xc4 + e | u8 | spy of empire `e` on this planet: level 0..=2 |
| +0x118 + 4p | i32 | current level of problem `p` (1..=19), ×10000 scale |
| +0x16a | i16[5] | planet event codes, slots 1..=5 (`BolygoInfoIs` 0x53bfe0, `BolygoInfoIsAny` 0x53c020 and `BolygoInfoRange` 0x53c050 scan slots 1..=5 at `+0x168 + 2k`; `+0x168` itself is never read: verified by fuzzing) |
| +0x174 / +0x178 | i32 | scripted morale penalty / its slew rate (SP only) |
| +0x186 | u8 | non-zero: can't rebel (`getFellazadhat` 0x4efe50 = owner < 9 && this == 0) |

### 2.2 Building instance (32 bytes, slot `j` of `Epuletek[p]`, `j = 1..=HanyEpuletLefoglalva[p]`)
From `EpuletNew` (exe 0x53c7d0) and `FRISSIT_Planet`.
| Offset | Type | Field |
|---|---|---|
| +0 | u8 | building type (0 = empty slot) |
| +1 | u8 | owner empire at construction |
| +4 | i32 | damage (`max_hp × initial% / 100` at creation) |
| +8 | u16 | construction work remaining; 0 = finished. Initial `desc.build_work × 50` (race 4: `/1.5`) |
| +0xc / +0x10 | f32 | map position |
| +0x18 | u8 | **yield %** (100; solar plant on surface 3 and magma plant on surface 4 ×1.5; factories 19–22 ×1.2 under event 0x669; scripted overrides) |
| +0x19 | u8 | flags: bit0 working, bit2 disabled by damage, bit3 no energy, bit5 switched off (human only), bit6 no workers |
| +0x1a / +0x1c | i16 | displayed energy / workers (= descriptor values) |
| +0x1e | i16 | efficiency = `256 × (max_hp − damage) / max_hp` |

### 2.3 Building descriptor (`EpuletekLeiro`, 36 bytes × 56, exe 0x8EBA38): pinned
| Offset | Type | Field | Evidence |
|---|---|---|---|
| +0 | u8 | tech (name, price) | `EpuletJavitasAr`, autobuild |
| +2 | u8 | **priority class 0..=6**: energy and workers are cut from class 6 downwards | §4 steps 2–5 |
| +4 | i32 | **construction work** in days (×50 at creation) | `EpuletNew` |
| +8 | i32 | max HP | §4 |
| +12 | i32 | **energy demand** | §4 step 5 |
| +16 | i32 | **worker demand** | §4 step 3 |
| +20 | i32 | **output** (scaled by yield % and efficiency, §4 step 6) | §4, §6 |
| +32 | u8 | bit0: effect = scaled output (else effect = 1 per building) | §4 step 6 |
| +33 | u8 | bit0: effect persists while not working (houses, defences) | §4 step 6 |
`+1`, `+3`, `+24`, `+28` aren't read by the economy.

### 2.4 Building types used (index = descriptor index)
1 colony hub · 2 small house · 3 large house · 4 food factory · 5 automatic food factory · 6 hospital ·
7 police station · 8 fire brigade · 9 water plant · 10 park · 11 bar · 12 recreation centre · 13 stadium ·
14–16 ship/tank/building research labs · 19–22 ship, advanced ship, tank, advanced tank factories ·
25 production improvement · 26 spy centre · 28–43 defences (guns, fortresses, shields, bunker,
self-destruct = 43) · 48 trade centre · 49 bank · 50 trade port · 52–55 fusion/solar/magma/mezon plants.

### 2.5 Other tables (read from the exe)
| Table | exe VA | Use |
|---|---|---|
| `ProblemakLeiro` | 0x912858, 20 × 24 B: `{a, b, c, d, rate, weight}` (i32) | problem curves §5; entry 0 = morale curve. **Entry 4's rate/weight is overwritten at runtime** (§5.3) |
| `FajBolygoViszony` | 0x90B9D8, i32[15][9] | race × surface growth factor (%) §7 |
| `FajFelelemOnmegsemmisitotolSzint` | 0x90B99C, i32[15] | fear of self-destruct, per race §5.2 |
| `CsaladtagMoralCsokkenesLeiro` | 0x913838, i32[6] | morale penalty per kidnapped family member state §5.2 |
| `FajSpeckoGyartasKapacitasSzorzo` | 0x8F8870, f32[15] | production multiplier per race §8 |

## 3. Economy step, overview (`full = true`, `only = 0`)
```
prologue:  ground-war flags for every planet (libc rand(); not economy)          [not ported]
           monthly: FullGDP[e] = FullTax[e] = 0
           reset per-empire lab levels, spy capacity, trade-port and defence totals
           GlobalisMoral (exe 0xBAEAC4) moves toward 0 by GlobalisMoralSebesseg (0xBAEB14)
for p in 1..=HanyBolygo with owner != 0:
           building pass (§4) → construction (§9) → rebellion counter (§11) → problems (§5)
           → morale (§6) → monthly population (§7) → taxes (§8) → autobuild scoring (§11)
           → spy expulsion (§11) → production capacity (§8) → per-empire totals (§8)
           → abandon planet if it has no buildings (§11)
epilogue:  one autobuild per empire (§11); monthly tax totals; tech availability matrix
```

## 4. Building pass (every call, per planet)
1. **Eligibility.** For each slot with type `t != 0`: count it; if under construction (`+8 != 0`) it isn't
   working. If the owner isn't human, clear "switched off". Set "damaged" when
   `max_hp < 2 × damage` and `t ∉ 28..=43` (defences keep working when damaged). Eligible = not damaged
   and not switched off. Eligible buildings add `desc.energy` to `E[class]` and `desc.workers` to `W[class]`
   and are provisionally marked working.
2. **Worker cut.** `planet.workers_demand (+0x50) = ΣW`. While `ΣW > population`, take from class 6, 5, …, 0:
   a class is zeroed if its whole amount is needed, otherwise reduced by just the excess.
3. **Worker assignment** (slot order). Event 0xF98D (`BolygoInfoIs(p, -0x673)`) stops factories 19–22.
   Clear bits 3 and 6. A working building whose class pool `W[class] < desc.workers` loses "working" and gets
   "no workers"; otherwise `W[class] -= desc.workers`.
4. **Energy production** = Σ over working buildings of type 1 or 52..=55:
   `output × yield × (max_hp − damage) / max_hp` (integer, per building). → `+0x48`. `+0x4c = ΣE`.
5. **Energy cut and assignment**: as steps 2–3 with `E` against production (class 6 cut first); a working
   building whose class pool is short loses "working" and gets "no energy".
6. **Effects** per type `t` (`effect[t]`), and counts `active[t]`, `inactive[t]`, `building[t]`:
   - working: `+0x1a/+0x1c` = descriptor energy/workers, `+0x1e` = `256·(max_hp − damage)/max_hp`;
     `effect[t] += desc.flag32 ? output × yield × eff / 256 : 1`; `active[t] += 1`.
   - not working: if finished and `desc.flag33`: `effect[t]` += the same with the **stored** `+0x1e`;
     `inactive[t] += 1`; if under construction `building[t] += 1`.
   (`/256` is C division: rounds toward zero.)

## 5. Problems (economy step only)
### 5.1 Curve
`curve(e, x, cap)` with `ProblemakLeiro[e] = {a, b, c, d}`, all in f64:
```
cap == 0 → c          otherwise   (int)( c × (a/c) ^ ( (x/cap × 100 / d) ^ (−b) ) )
```
`c` is the maximum, `a` the value at `x = d% of cap`, `b` the steepness. Each problem first returns 0 when
its own "nothing to worry about" condition holds (usually population 0).

### 5.2 Targets (`target[p]`, before ×10000)
| p | Name | Target |
|---|---|---|
| 1 | Electricity | demand 0 → 0; else `curve(1, demand, production)`, with demand/production from **this step's building pass** (locals; a construction refresh in §9 rewrites `+0x48/+0x4c` but not them) |
| 2 | Food | `cap = (effect[4] + 5000 + effect[5]) × f`; pop 0 → 0; `cap == 0` (f32) → `c`; else `curve(2, pop, cap)` |
| 3 | Living space | pop 0 → 0; `curve(3, pop, 5000 + 7000·effect[2] + 12000·effect[3])` |
| 4 | Hospital needed | `cap = 7500·(effect[6] + 1)`, halved (integer) during an epidemic; `curve(4, pop, cap)` |
| 6 | Family member captured | `CsaladtagMoral[CsaladtagElrabolvaKi[owner]]` (owner < 9, else index 0) |
| 10 | Recreational facilities | `curve(10, pop, cap)`, `cap = (int)(3000 + 4000·g(effect[10]) + 8000·g(effect[11]) + 14000·g(effect[12]) + 30000·g(effect[13]))`, `g(n) = 2 − 2/(n+1)` (f32) |
| 11 | Fear of self-destruct bld | no self-destruct (`effect[43] < 1`) → 0; else roll `rndlog(3,−1,1)` and clamp to `[F/2, F]`, `F = Fear[race]` (in practice `F/2`) |
| 12 | Colony hub needed | no hub (`effect[1] == 0`): roll `rndlog(3,−1,1)` clamped to [10, 20] (= 10); else 0 |
| 13 | Stadium needed | stadium present → 0; else `curve(13, pop, 30000)` |
| 16 | Criminals | `curve(16, pop, effect[7] + 10000)` (police effect) |
| 18 | Environment contamination | `x = Σ effect[19..=24]` (factories); x 0 → 0; else `curve(18, x, 8000·effect[10] + 5000)` |
| 19 | High taxes | `max(0, ((Ev + Honap + p)·3) % 7 − 3 + ((pop + 250000)/250000) × T[tax])`, `T = [0, 6, 10, 15, 21, 27, 35, 43]` |
Others are 0. Food's `f`: event codes 200..=204 (`BolygoInfoRange`) give `f = 0.9, 0.85, 0.75, 0.65, 0.5`,
and the lost share `cap × (1 − f)` is added to `StoryAdat+0x2c` (f32 add, truncated). Event 0x5D3 or
`StoryAdat+0x20 == p` halves `f` and the lost share (f32).
Epidemic (`+0xb8 != 0`): `q = (cap/2) / pop`, `frac = (f32)(cap/2)/(f32)pop − (f32)q`;
`+0xb8 −= q + (rndlog(3, 1, 100) < frac × 100 ? 1 : 0)`; at ≤ 0 it ends (info 100 removed, virus cleanup).
Without an epidemic and `StoryAdat+0x30 != 0`, info 0x65 is set while `pop < cap/2`, else removed.

### 5.3 Smoothing
Before the loop, `ProblemakLeiro[4].rate/weight` := `200/400` during an epidemic, else `10/100`
(a persistent write to the global table). Then for p = 1..=19, with `target ×= 10000`, `cur = +0x118+4p`:
```
r = (f32)cur / (f32)(target + 1)
step = 50;  if r > 1.5f or (f64)r < 0.66: step = 250;  if r > 2.0f or r < 0.5f: step *= 5
cur moves toward target by at most step × rate[p]
S_now    = trunc(S_now    + w(weight[p] × cur    / 10000))      (integer division inside)
S_target = trunc(S_target + w(weight[p] × target / 10000))
w(n) = n < 1 ? 0 : exp( (f64)(f32)ln((f64)(f32)n) × 1.5 )          (exe precision, §10)
```

## 6. Morale
```
level(S) = S == 0 ? 0 : (int) exp( (f64)(f32)ln((f64)(f32)S) / 1.5 )
m(S)     = level(S) == −10000 ? 0 : curve(0, level(S) + 10000, 10000)     (ratio = (L+10000)/10000)
morale   = 1000 − 10 × m(S_now)
needed   = clamp(1000 − 10 × m(S_target), 0, 999)  → BolygoNeededMoral[p]  (exe 0xB9A630)
SP only:  penalty (+0x174) slews toward (event 0x91B ? 200 : 0) by +0x178;
          morale −= penalty + GlobalisMoral;  event 0xE084 (-0x1f7c) caps morale at 190 when > 200;
          clamp to 0..=999
→ +0x30
```
(In MP morale isn't clamped; it can be 1000.)

## 7. Population (monthly)
```
mr = morale − morale % 100
f  = (f32)FajBolygoViszony[race][surface] / 100f;  water plant (effect[9] > 0): f = (f32)((f64)f + 0.1)
if mr < 400: f = 1.0
target = (int)((f32)(mr/10 − 40) × (f × 300f) / 50f)           (f32 throughout)
spread = (int)((f × 300f) / 5f)
roll   = rndlog(3, spread/2, (int)((f64)spread × 1.5))
M = target < 1 ? last − roll : last + roll                       (last = +0x2c)
if |M| > |target|: M = target + rndlog(3, −30, 30)
if no colony hub and M > 0: M = 0
if !(event 0xE0B6 (-0x1f4a)) or MP: pop += M + rndlog(3, −10, 10)
if pop < 1100: pop = 1100 + rndlog(3, 0, 100)
+0x2c = pop − pop_before
```
So growth has momentum: it ramps by ~`spread` per month towards `target` (positive above 40% morale).

## 8. Taxes, GDP and production
Every economy step (`Nap % 6 == 1`):
```
g = pop / 500;  event 0xC30: g = (int)((f64)g × 1.1)
unless event 0xF3BC (-0xc44):  GDP_acc (+0x38) += g;  tax_acc (+0x40) += g × tax_level × 5
monthly: FullGDP[owner] += GDP_acc; FullTax[owner] += tax_acc; money pool 0 (tax) += tax_acc;
         last (+0x3c, +0x44) = acc; acc = 0
```
Taxes are therefore paid monthly for five accrual days (7, 13, 19, 25, then 1).
On 1 January `TaxIncomeThisYear` rolls into `TaxIncomeLastYear` (and the trade equivalents) with a message.

**Production capacity** (every call, including refreshes):
```
rich = (int)((f32)planet.+0x24 × RaceProdMult[PlayerFaj[owner or EN]] × 1.0 + (f32)bonus)
       bonus (SP, if any event code set): 20 for 0xC3A/0x1F41, then 10 for 0x1F42, then 15 for 0x91B (last wins)
ships (+0x70)    = rich × (effect[19] + effect[20]) / 100
vehicles (+0x74) = rich × (effect[21] + effect[22]) / 100
production improvement (effect[25] > 0): both = (int)((f64)x × 1.5)
```
Per-empire totals (only = 0): `LaborSzint[owner][1..=3] += effect[14..=16]` (also credited to EN when
`StoryAdat+0x24 == owner`, SP); `LaborSzintOsszes += active + inactive`; ground/fortress/ion-gun
strength from types 28..=43 via `TalalmanyokrEro`/`TalalmanyEroHP`; fire brigades `8·active[8] + 1`;
spy capacity `min(12, HanyKemLehet + effect[26])`; spy centres and trade ports counted.

## 9. Construction (economy step)
For each slot under construction: `remaining = max(0, remaining − (3·effect[1] + 1) × 300)`
(`6 days × 50`; `effect[1]` = number of working hubs). On reaching 0: `EpuletFelepult` (exe 0x4741a0;
3D/ground-war bookkeeping only) and a refresh `FRISSIT_Planet(false, p)`, which reruns §4 and §8 for `p`
and writes building flags and `+0x48/+0x4c/+0x50/+0x70/+0x74`. The outer step keeps using the effects it
computed before the completion, and recomputes `+0x70/+0x74` from them at the end of the planet.

## 10. Exe vs ELF precision (normative: exe)
- `w(n)` and `level(S)`: the exe converts the int via f32 and rounds `ln()` to f32 before ×1.5 / ÷1.5
  (`cvtsd2ss; cvtss2sd` after the `log` import, 0x4762be); the ELF stays in f64. The two differ for
  almost every `n`, but morale (`1000 − 10·m`, integer `m`) absorbs the difference: no traced step
  tells them apart. The exe form is kept on instruction-level evidence.
- Curves (§5.1) are f64 in both.
- Growth (§7) is f32 in the exe except `+0.1` and `×1.5` (f64).
- Smoothing ratio is f32; it's compared with `1.5f`, `2.0f`, `0.5f` and the **double** `0.66`.

## 11. Side systems
Ported: the rebellion counter, spy expulsion and abandonment. The rebellion itself (`BolygoFellazad`),
the change of owner, autobuild, money, the per-empire totals of §8 and the yearly income roll-over are
specified in `colony-side.md` and ported in `ig2-sim::colony_side`.
- **Rebellion counter** (`BolygoMennyiIdejeLazad`, exe 0xB99668), economy step, if `getFellazadhat` and
  (MP or `+0x64` bit0 clear): `+1` if morale < 100, `+1` if < 200, `−1` if ≥ 300, `= 0` if ≥ 400. While
  morale < 200: a warning every 5th count below 30 (human owner); above 30 the planet rebels
  (`BolygoFellazad`, exe 0x53bc40; rolls on streams 1 and 8 seen at runtime). **This happens in the
  middle of the planet's step** (before construction): the rebellion rewrites the planet (owner, tax
  level, morale, epidemic, …) and the rest of the step runs on the new state, so it changes which
  stream-3 rolls follow (`colony-side.md` §3).
- **Spy expulsion** (stream 3, after taxes): `def` = highest of 1..=4 with `active[28..=31] > 0` (guns),
  else 0. If `def > 0`, for each empire `e = 1..=ProgramPlayers2` with a spy level `s = +0xc4+e`:
  `rndlog(3, 1, K[5s + def] / 6) == 1` removes the spy (message to EN).
  `K = [0,0,0,0,0, −1,120,90,60,30, −1,1440,720,540,360]` (built inline in the exe).
- **Autobuild** (per planet scoring, then one purchase per empire in the epilogue): `colony-side.md` §6.
- **Abandonment**: a planet with no buildings (and `+0x64` bit0 clear) is lost (`BolygoElfoglal(p, 0)`).

### RNG consumption per planet (stream 3, in order)
epidemic roll → problem 11 roll → problem 12 roll → (monthly) growth roll, clamp roll, population roll,
floor roll → spy-expulsion rolls. Nested refreshes (§9) consume nothing.

## 12. Verification
`traces/2026-10-15-colony.md`: Frida before/after snapshots of every economy call on the GOG exe,
replayed by `engine/crates/ig2-sim/tests/colony_trace.rs`, including a fuzzed run that randomises
live planet state before each call. All modelled fields, `BolygoNeededMoral`, the unrest counters,
the monthly `FullGDP`/`FullTax` and the stream-3 state match. Whole steps including rebellions are
replayed by `tests/colony_side_trace.rs` (`traces/2026-10-15-colony-side.md`), which found the two
corrections above (problem 1's inputs, the `+0x64` byte test).
