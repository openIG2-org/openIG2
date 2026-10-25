# The AI empire (`FRISSIT_MI`)

Port status: ported in `engine/crates/ig2-sim/src/ai/`; specified from both binaries, exe-normative; verified
against `traces/2026-10-15-ai-empire.md` (§1–§6) and `traces/2026-10-16-ai-empire-2.md` (§8–§13).

`FRISSIT_MI` is one of the daily systems driven by the sim tick (`Fake_FRISSITES` exe 0x4de720, on a day change,
skipped while the debug switch exe 0xB77244 is set):

```
FRISSIT_Planet (Nap % 6 == 1) → FRISSIT_Viszony → FRISSIT_MI → FRISSIT_Spy → MI_DIPLOMACY
```

It runs one pass over every AI empire and calls seven sub-passes. This part covers the driver, the empire
bookkeeping (`FRISSIT_MI_SZAMOLAS`), the per-planet colony settings, the money sliders, the production choices
(`FRISSIT_MI_GYARTAS`) and the spy missions (`FRISSIT_MI_KEMKEDES`). `FRISSIT_MI_FEJLESZTES` (the next research
pick) is ported (`production-research.md` §2.2, `research::ai_pick`). Part 2 (§8–§13) covers colonisation,
fleets, the AI's designs and the set-up at a new game.

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_MI` | **0x50a6e0** | 0x5b6520 | the driver (§2–§4) |
| `FRISSIT_MI_SZAMOLAS` | 0x50dd00 | 0x5b0840 | empire centres, main fleet, contact age (§2) |
| `FRISSIT_MI_GYARTAS(e)` | 0x50c6d0 | 0x5ae580 | unit counts, production queues, scrapping (§5) |
| `FRISSIT_MI_KEMKEDES(e)` | 0x50d0d0 | 0x5af7e0 | skill points, spy missions (§6) |
| `FRISSIT_MI_KOLONIZALAS(e)` | 0x50d9a0 | 0x5b0d30 | colony ships (§9) |
| `FRISSIT_MI_FLOTTAK(e)` | 0x50b1c0 | 0x5b3ce0 | fleets, attacks, defence (§11) |
| `FRISSIT_MI_FEJLESZTES(e)` | 0x50b0b0 | 0x5ae350 | research pick (`production-research.md` §2.2) |
| `FRISSIT_MI_DESIGN(e, b)` | 0x50abf0 | 0x5ad410 | the AI's designs, `MiTervMutato` (§10) |
| `MoneyTeruletSzorzoCalculate` | 0x540280 | 0x6221c0 | income shares from the sliders (§4) |
| `GetBolygoGazdagsag(p)` | 0x4aca20 | 0x4fb180 | planet richness (`colony.rs::production_base`) |
| `KemEselyGetBest(e, k, m, t)` | 0x5874d0 | 0x6d1a90 | "is there a best parameter" (§6.3) |
| `KemEselyLehetseges(e, k, m, t, p1, p2)` | 0x587740 | 0x6d13d0 | is the mission possible (§6.4) |
| `KemEselySzamolas2(e, k, m, t)` | 0x587de0 | 0x6ca620 | success chance, f32 (§6.5) |
| `MoneyMennyi2(e, k)` / `MoneyHozzaad2(e, k, x)` | 0x540150 / 0x53fae0 | 0x623360 / 0x623390 | read / add to pool `k` directly (no re-sum) |
| `CsokkentettTervAr(e, d)` | 0x4ad300 | 0x506d90 | resale price of a design (§5.3) |
| `GetTervEladasOszto(e, d)` | 0x4b4370 | 0x506c40 | resale divisor (§5.3) |
| `st_eSearch2spy` | 0x6040a0 | 0x7323c0 | `espionage.md` §5.6 (stream 4) |

**The exe is normative.** The exe compiles the sub-passes as separate functions (hookable) and inlines the small
helpers. Where the decompiles differ the asm was checked: they agree everywhere (exe loops unrolled, the ELF's
`x < x` main-planet test is always false, the exe's `KemEselyGetBest` returns the same booleans through
shortcuts). Floats: the planet-centre average is **f32** (`addss`/`divss`), the scrap bucket is `(int)((f64)(f32
x · 100.0f) + 0.5)` (`mulss` then `addsd` @ 0x50ced2), the scrap weight `sqrt` is f64 of the f32 product rounded
back to f32 (= `sqrtf`, @ 0x50cf04..0x50cf23), the 10 % test is f64 (`mulsd` @ 0x50cf9a), the mission cost is
**f32** (`mulss` @ 0x50d932, then `cvtss2sd`), the success chance f32 (as in `espionage.md` §5.1), the
sliders and shares f64.

Randomness: **stream 10** (`rndlog(10, …)`, `RndValasztas(10, …)`) and **stream 4** (`KemEselyGetBest` for
sabotage, `st_eSearch2spy` in `KemEselyLehetseges`). No libc `rand()` in these functions (no call to the import
0x8353ac in 0x50a6e0..0x50df60).

## 1. Data
### 1.1 State (exe VAs; ELF names)
| Name | exe VA | Layout |
|---|---|---|
| `MiFoBolygo` | 0xDB0548 | i32 `[19]`: the AI's main planet |
| `MiFoFlotta` | 0xDB0598 | i32 `[19]`: the main fleet (also read by diplomacy) |
| `MiBirodalomKozeppontX` / `…Y` | 0xDB2848 / 0xDB2898 | i32 `[19]`: centre of the empire's planets |
| `MiVeszelyAllapot` | 0xDB27F8 | i32 `[19]`: danger state 1..=5, set by `FRISSIT_Viszony` (`diplomacy.md` §3.5) |
| `MiTervMutato` / `MiTervMutatoSpec` | 0xDAF668 / 0xDAFA20 | i16 `[19][25]`: the AI's best / special design per design slot (set by `FRISSIT_MI_DESIGN`) |
| `MiGyarthatoCuccok` | 0xDB28E8 | `[19][11]` category records of 0x24 bytes (§1.3) |
| `VanUresBolygo` | 0xDB0724 | u8 `[19]`: there are planets to colonise (set by `FRISSIT_MI_KOLONIZALAS`) |
| `MoneyTeruletCsuszka` | 0xBAD920 | f64 `[19][6]`: income sliders (pools 1..=5) |
| `MoneyTeruletSzorzo` | 0xBADCB0 | f64 `[19][6]`: income shares (`colony-side.md` §6.4) |
| `MoneyTeruletenkent` | 0xBAD200 | f64 `[19][6]`: money pools (pool 4 = espionage) |
| `BirodalomMiotaIsmeri` | 0xBAED28 | i32 `[19][19]`: contact age, −1 a day (`diplomacy.md` §1) |
| `SpyEngedelyezve` | 0xBAE3AF | u8: espionage on |
| `FullGDP` | 0xBAE060 | i32 `[19]` (training mission level, unused by the AI) |
| fleets | *0xBB40A4 / 0xBB40A8 | `+0x00` name (20 chars), `+0x15` owner, `+0x16` story id, `+0x52` u16 flags, `+0x60` attack |
| planets | 0xB85EA8 | `+0x18/+0x1c` position, `+0x20` owner, `+0x28` population, `+0x34` tax level, `+0x58` garrison, `+0x6c/+0x6d` autobuild mode / aggressiveness, `+0x70/+0x74` ship / vehicle capacity, `+0x78..+0xb8` production queue (`production-research.md` §3.1) |

`MiTervMutato` sits right after `DiplomaciaMikorHivtaUtoljara` (0xDAF520): with `ProgramPlayers` 18, the
diplomacy pass's `a·9 + b` indexing (`diplomacy.md` §4) writes day numbers into the AI's design slots of empires
0..=7. With ≤ 8 players (every shipped scenario) the two don't touch.

### 1.2 Rule tables (static; read from the user's exe, `ig2-rules`)
| Name | exe VA | Layout |
|---|---|---|
| `MiGyartasMod` | 0x915F6C | u8 `[11]`: category kind: 1 = the main planet builds it on demand, 2 = default unit, 3 = not counted |
| `MiGyartasMitMin` / `…Max` | 0x915F78 / 0x915F84 | u8 `[11]`: the `MiTervMutato` slot range of a category |
| `FajMiBolygokHanyadReszeGyartas` | 0x915F90 | i32 `[15]`: % of the planets that get a production autobuild mode, by race |
| `FajMiBolygokMilyenKatonaiAutiBuild` | 0x915FCC | i32 `[15]`: the military autobuild mode, by race |
| `KemKuldetesek` | 0x912F48 | `espionage.md` §1.2; here also **+0x10 length in days, +0x14 cost, +0x18 minimum rank** (`FRISSIT_Spy` uses neither) |
| `TrainingKuldetesAr` | 0x927044 | i32 `[6]`: training mission cost by level |
| `KemKuldetesPenzSzorzoFajonkent` | 0x91376C | f32 `[15]`: mission cost multiplier by race |

### 1.3 `MiGyarthatoCuccok[e][c]` (0x24 bytes, `c` 1..=10)
| Off | Type | Meaning |
|---|---|---|
| +0x02 / +0x04 | i16 | design: the last positive `MiTervMutato[e][s]` / `MiTervMutatoSpec[e][s]` for `s` in Min..=Max (§5.1) |
| +0x0c / +0x10 | i32 | units of design +0x02 / +0x04 in service (ships, plus vehicles counted by number) |
| +0x14 | i32 | wanted: categories 1, 2 and 4 are set to 0, category 3 (colony ships) to `VanUresBolygo[e]`, the rest are never written here |
| +0x1c / +0x20 | i32 | queue entries of design +0x02 / +0x04 on the empire's planets |
Other bytes are never touched by this part.

## 2. `FRISSIT_MI_SZAMOLAS`: empire bookkeeping (all empires)
For `e` in 1..=`ProgramPlayers2` alive (human ones included):
1. Centre: over planets 1..=n owned by `e`, f32 sums of x and y (in planet order), `count`; `(int)(sum /
   (f32)count)` into `MiBirodalomKozeppontX/Y[e]` (0 when `count` is 0).
2. Main fleet: over fleets 1..=n owned by `e` with flags bits 3 and 4 clear (`+0x52 & 0x18 == 0`) and story id
   0, the first with the highest attack `+0x60` (start −1, strictly greater), else 0.
3. If `e` is not human and the old `MiFoFlotta[e] > 0` is still `e`'s fleet: clear its bit 11 (`+0x52 & ~0x800`).
   `MiFoFlotta[e]` = the new one; clear bit 5 of the new one (`& ~0x20`): with no main fleet that is **fleet slot
   0**'s record. A new main fleet (> 0) of a non-human empire is renamed `sprintf(MiFoFlottaNev, Faj_Nev[race])`
   (20 characters, localised: an event in the port) and gets bit 11.
4. Contact age: `BirodalomMiotaIsmeri[e][i] -= 1` for `i` in 1..=`ProgramPlayers2` alive with
   `BirodalomIsmeri[e][i]`.

## 3. The driver, per AI empire
For `e` in 1..=`ProgramPlayers2` with `BirodalomEl[e]`, `PlayerKi[e] != 1` and `BirodalomMIzik[e]`:

### 3.1 Main planet
`pick` = 0; over `e`'s planets in order: `pick = p` while `pick == 0` or `pick`'s ship capacity (`+0x70`) is 0
(so: the first planet with a shipyard, else the last planet). `MiFoBolygo[e] = pick` when the old one is 0, not
`e`'s any more, has no ship capacity, or `old.cap + old.pop/20 + 250 < pick.cap + pick.pop/20` (int division;
`pick` 0 reads planet record 0).

### 3.2 Production quota
Wealth class of a planet from `g = GetBolygoGazdagsag(p)`: `g < 75` → 1, `< 100` → 2, `< 125` → 3, `< 150`
→ 4, else 5. `have[c]` = `e`'s planets per class, `n` = all of them. `q = n · FajMiBolygokHanyadReszeGyartas
[race] / 100`; from class 1 up: `take[c] = min(have[c], q)`, `q −= take[c]` (the poorest planets are the
production planets). `ships` / `vehicles` = `e`'s planets with ship / vehicle capacity > 0.

### 3.3 Planet settings (each of `e`'s planets, in order)
```
c = class(p)                                   (GetBolygoGazdagsag again)
+0x6c = take[c] < 1 ? (ships > 0 && vehicles == 0 ? 6 : 2) : 3
if LaborSzintOsszes[e][1] > 18 && [2] > 10 && [3] > 10: +0x6c = 2
take[c] −= 1                                   (goes negative)
+0x6d = MiVeszelyAllapot[e] > 2 ? 2 : 3
+0x34 = BolygoMennyiIdejeLazad[p] > 0 ? 0 : 3  (tax level)
k = MiVeszelyAllapot[e] > 1 ? 9 : 6;  if p == MiFoBolygo[e]: k = min(k + 2, 9)
if BolygoMennyiFoldiEro[p] < (int)((pop − 10000) · k) / 5000:  +0x6c = FajMiBolygokMilyenKatonaiAutiBuild[race]
if e < 9 && HanyKemLehetOsszesen[e] < 12 && pop > 7000 && HanyKemKozpontBolygonkent[e][p] == 0 && race != 5:
    +0x6c = 7
```
(`(pop − 10000) · k` wraps as i32; the exe multiplies unsigned and divides signed.) The autobuild modes are the
`autobldN.txt` profiles (`colony-side.md` §4); the economy step then builds by them.

### 3.4 Sub-passes
`FRISSIT_MI_GYARTAS(e)` (§5); `FRISSIT_MI_KEMKEDES(e)` (§6) if `e ≤ ProgramPlayers` and `SpyEngedelyezve`;
`FRISSIT_MI_KOLONIZALAS(e)`; `FRISSIT_MI_FLOTTAK(e)`; `FRISSIT_MI_FEJLESZTES(e)`.

### 3.5 Money sliders
`MoneyTeruletCsuszka[e][0..=5]` by `MiVeszelyAllapot[e]`: 1 → (0, 40, 30, 30, 10, 10); 2 → (0, 50, 30, 20, 10,
10); else (0, 80, 10, 10, 10, 10).

## 4. After the loop: `MoneyTeruletSzorzoCalculate` (empires 1..=18, f64)
```
s = ((slider[1] + 0.0) + slider[2]) + slider[3]
share[i] = (int)(slider[i] · 100.0 / s) / 100.0           for i in 1..=5
loop:  t = ((share[1] + 0.0) + share[2]) + share[3]
       if t ≥ 1.0: break
       share[1] > 0 ? share[1] += 0.01 : share[2] > 0 ? share[2] += 0.01 : share[3] += 0.01
if t > 1.0: share[1] −= t − 1.0
            if share[1] < 0: share[2] += share[1]; share[1] = 0
            if share[2] < 0: share[3] += share[2]; share[2] = 0
```
It runs for every empire (the human's sliders come from the money screen). `s == 0` gives NaN/∞ → `(int)`
of x87 `cvttsd2si` = `0x80000000`; the loop then adds 0.01 until the sum reaches 1.

## 5. `FRISSIT_MI_GYARTAS(e)`: production choices
### 5.1 Unit counts (categories 1..=10)
Clear `+0x0c, +0x1c, +0x10, +0x20, +0x14`. `+0x02` / `+0x04` = the last positive `MiTervMutato[e][s]` /
`MiTervMutatoSpec[e][s]` for `s` in `Min[c]..=Max[c]` (0 if none). Categories with `Mod[c]` 1 or 2, for `j` in
(1, 2) with `d` = design `+0x02` / `+0x04`:
- queued `+0x18 + 4j` = over `e`'s planets, the four i16 words at `+0x78 + 8·(class(d) − 1)` equal to `d`
  (`class` = design `+0x16`; 0 when `d ≤ 0`). Class 0 or > 2 reads other planet bytes (capacities, queue
  counts) as words: kept;
- in service `+0x08 + 4j` = ships 1..=n with design `d` in `e`'s fleets (deleted ships have design 0x7FFF) +
  the vehicle counts of `Tankok` entries 1..=n with design `d` held by `e` (fleet `f > 0`: `e`'s fleet; `f ≤ 0`:
  planet `−f` owned by `e`).

Then `want`: category 1, 2, 4 = 0, category 3 = `VanUresBolygo[e]`.

### 5.2 Queues (each of `e`'s planets, in order)
```
+0xac (rally) = p == MiFoBolygo[e] ? 0 : (u8)MiFoBolygo[e]
ship = cat[8].+2;  alt = cat[10].+2
if cat[10].units[2] + cat[10].queued[2] < cat[10].units[1] + cat[10].queued[1] && cat[10].+4 > 0: alt = cat[10].+4
if +0x78 == 0 && +0x70 > 0:                     (ship line idle, has a shipyard)
    if (MiVeszelyAllapot[e] < 3 || rndlog(10, 1, 3) == 1) && p == MiFoBolygo[e]:     (draw only when ≥ 3)
        for c in 1..=10 with Mod[c] == 1, cat[c].+2 > 0, units[1] + queued[1] < want: ship = cat[c].+2; alt = 0
    +0x78 = ship; +0x88 = design(ship).+0x36 ? 3 : 1; +0x90 = +0x91 = 0; +0xad = +0xae = 0
    +0x7a = alt;  +0x89 = 1
    +0xa0 = design(ship).+0x20 (build time); +0xa8 = 0
veh = cat[6].+2;  if cat[6].units[2] + queued[2] < units[1] + queued[1] && cat[6].+4 > 0: veh = cat[6].+4
if +0x80 == 0 && +0x58 < 10 && +0x74 > 0:      (vehicle line idle, garrison < 10)
    +0x80 = veh; +0x8c = 1; +0x94 = 0; +0xb1 = 0; +0xa4 = design(veh).+0x20; +0xa9 = 0
```
`design(0)` reads design record 0. When the planet is the one open in the production screen, UI flags are set
(not modelled).

### 5.3 Scrapping (day 20)
On `Nap == 20`: over ships 1..=n alive (design ≠ 0x7FFF) in `e`'s fleets whose design has flags `+0x34`, `+0x35`,
`+0x37`, `+0x38` all 0: bucket `b = clamp((int)((f64)(f32)(design.+0x3c · 100.0f) + 0.5), 0, 16)`, weight `w =
(int)sqrtf((f32)(design.+0x2c · design.+0x30))`; `sum[b] += w`, `total += w`. `a` = the smallest bucket 1..=16
with `sum > 0`. If `a > 0` and `(f64)sum[a] < (f64)total · 0.1`: every such ship (re-checked in order, also needing
its fleet's story id `+0x16 == 0`) in bucket `a` is sold: `MoneyHozzaadLog(e, 0, CsokkentettTervAr(e, design))`
(income, `colony-side.md` §6), then `HajoDelete(i)` (`space-combat.md` §3).

`CsokkentettTervAr(e, d) = f < 0 ? 0 : (int)((f32)price / f)` with price = design `+0x24` and `f =
GetTervEladasOszto(e, d)`: 2.0 (10.0 when design `+0x36`); 0.5 when `StoryAdat+0xac != 0` and design `+0x44 == 5`
and `+0x40 == EN`; −1.0 when the design id `+0x80` is in 0x6a5..=2999 except 0x8fc, 0x8fd, 0x906 and
0x924..=0x937.

## 6. `FRISSIT_MI_KEMKEDES(e)`: spy missions
`roster` = slots 1..=`OsszHanyKem[e]`, **stopping at the first slot with state ≤ 0** (every loop below does).

### 6.1 Selection and firing
1. `list` = the roster's slots in order, `n` = its length; `score[k] = infiltration · 2000 / (counter + 1)` (u8
   skills, unsigned division); `sel[k] = 1`.
2. If `n > HanyKemLehet[e]` or `n ≥ 12`: the spy with the lowest experience (first on ties) is fired (state 0).
   The loops below then stop at that slot.
3. Bubble sort `list` by `score`, descending (`for i in 1..n: for j = i down to 1: if score[list[j]] <
   score[list[j+1]]: swap`).
4. `sel[list[n]] = 0` if `n ≥ 2`; `sel[list[n−1]] = 0` if `n ≥ 3`; `sel[list[n−2]] = 0` if `n ≥ 7`, and again
   (the same slot) if `n ≥ 9`.
5. `best` = the selected spy with the highest infiltration (first on ties).

### 6.2 Per spy (state 1 at home, or state 3 on counter-intelligence, mission 13)
Persistent across spies: `target`, `p1`, `p2` (0 at the start) and `busy` (false).
1. Skill points: while `+0x38 > 0`: raise the lowest (first on ties) of infiltration and combat (selected
   spies) or counter-intelligence and combat (the rest) by 1 (u8), `+0x38 −= 1`.
2. `m = 0`; if `xp < 2000`: `m = 1` (training) if (`sel` and `k == best`) or (not `sel` and not `busy`).
3. If `k == best` and `rndlog(10, 1, 500 − 200·Nehezseg) ≤ 1` and `MoneyMennyi2(e, 4) ≥ 0` (f64 pool):
   `RndInitValasztas`; for `t` in 1..=`ProgramPlayers`, `t ≠ e`, alive, known to `e`, race ≠ 5; for `m'` in 3..=10
   with `KemKuldetesek[m'].+0x18 ≤ rank`:
   ```
   g = KemEselyGetBest(e, k, m', t);  (p1, p2) = m' == 6 ? (0, g) : (g, 0)        (persistent!)
   if KemEselyLehetseges(e, k, m', t, p1, p2):
       w = W[Viszony[e][t].level] · (40 · KemEselySzamolas2(e, k, m', t) / 100)
       m' 3 or 8: w ·= 2;  m' 5: w /= 2 (toward 0);  level 5: w = 0
       if RndValasztas(10, w): pick = (t, m', p1, p2)
   ```
   with `W` = (0, 100, 40, 25, 15, 8) by level. A pick sets `target, m, p1, p2` = the pick. Then for `m ≥ 2`, the
   relation level `L = Viszony[e][target].level` may veto it: `L == 5` and `rndlog(10, 1, 100) < 75`, `L == 4` and
   `< 50`, `L == 3` and `< 25` (one draw, only for that level) → `m = 0`, `MoneyHozzaad2(e, 4, −1000.0)`.
   Without a pick `p1, p2` keep the last evaluated candidate's values.
4. `m == 0`, not `sel` and current mission ≠ 13 → `m = 13`. `m == 0`, `sel` and current mission 13 → state 1,
   mission 0 (called home).
5. `m > 0` and `target` in 1..=8: (not `sel` → `busy = true`) state 3, mission `m`, `+0x1c = target`, `+0x1e =
   p1`, `+0x20 = p2`, length and time left `L = KemKuldetesek[m].+0x10 · 50`, catch moment `rndlog(10, L/3, L −
   30)`; `m ≠ 13`: `MoneyHozzaad2(e, 4, −(f64)(f32)(KemKuldetesPenzSzorzoFajonkent[race] · (f32)cost))` with cost =
   `KemKuldetesek[m].+0x14` (training: `TrainingKuldetesAr[p1]`).

So `p1` is a boolean "a best parameter exists" (`KemEselyGetBest` returns `bool`): the AI targets planet or
tech 1 when it sends a planet- or tech-parameter mission. A mission is assigned only once some spy has picked a
target this day. Original quirks, kept.

### 6.3 `KemEselyGetBest(e, k, m, t)` → bool
1: some `TrainingKuldetesAr[1..=5] ≤ FullGDP[e]` (f64 compare) — always true; 6: `rndlog(4, 1, 5)`, true;
7: some ship of `t` (alive) has design `+0x48 > 0`; 8: some tech 1..=192 with `e`'s status < 5 and `t`'s 5;
9: some planet of `t` with population > 0; others: false.

### 6.4 `KemEselyLehetseges(e, k, m, t, p1, p2)` → bool
`ok = !(+0x1c && !alive(t))`; kind-1 mission with `p1 > 0` and planet `p1` not `t`'s → false. If `ok`:
3: `p1 ≠ 0` → true, else some planet of `t` unseen by `e` (`+0xd8 + 2e == 0`); 4: some tech `t` owns (status 5)
not in `e`'s seen matrix (`+0xc0 + 9e + t == 0`); 5: `MoneyMennyi(t, 0) ≥ 100` and `(int)(MoneyMennyi(t, 0) ·
0.25 / 100.0 · 100.0) > 0`; 6: `st_eSearch2spy(4, t, p1, p2)`, then `(t, p1, 0)`, `(t, 0, p2)`, `(t, 0, 0)`
until one finds a building (stream-4 draws); 7: some ship of `t` alive; 8: `p1 == 0`: some tech with `e`'s status
< 5 and `t`'s 5, else tech `p1` so; 9: some planet of `t` (`p1 == 0` or `= p1`); 10: `t` has a spy in state 1
or 2; 11: no kidnapped member of `t` and (MP or `t ≠ 2`); others: `ok`.

### 6.5 `KemEselySzamolas2(e, k, m, t)` (f32)
The success chance of `espionage.md` §5.1 for spy `k`'s infiltration, then 0 if the mission has `+0x1e`; for
mission 11: 0 if `t == 0`, `t`'s member kidnapped, or (SP and `t == 5`); SP and `t`'s race 5: halved.

## 7. Port
`ig2-sim::ai`: `AiState` (§1.1 and §8.1: the tables the AI owns), `AiRules` (§1.2), `Ai::daily` (the driver),
`bookkeeping`, `planet_settings`, `production`, `spy_missions`, `research_pick`, `income_shares` (part 1) and
`colonise`, `fleets`, `design`, `init`, `init_dropped` (part 2, `ai::units` holds the unit bookkeeping they
share), unit tests per section. Localised texts (the main fleet's name, a design's name) and effects owned by
other systems (the war declaration, the ground-war map, the tutorial) are `AiEvent`s. Inputs come from the
other ports' worlds (`colony_side::ColonyWorld`, `spacewar::CombatWorld`, `fleet::Starmap`, `spy::Spies`,
`diplomacy`'s danger state, contact ages, relations and power).

# Part 2: colonisation, fleets, design

Specified from both binaries, exe-normative (decompiles of both and the exe asm for every float expression);
verified against `traces/2026-10-16-ai-empire-2.md`.

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_MI_KOLONIZALAS(e)` | 0x50d9a0 | 0x5b0d30 | colony ships, colony-ship designs (§9) |
| `FRISSIT_MI_FLOTTAK(e)` | 0x50b1c0 | 0x5b3ce0 | timers, vehicles, war, attack target, fleet orders (§11) |
| `FRISSIT_MI_DESIGN(e, all)` | 0x50abf0 | 0x5ad410 | the AI's designs per slot (§10) |
| `INIT_MI` / `INIT_MI_KIESETT(e)` | (inlined) | 0x5ade70 / 0x5ad2a0 | new game / an empire handed to the AI (§13) |
| `CheckVeszelyesHely(f, x, y, r, …)` | 0x50a2e0 | 0x5b2970 | is a point dangerous for fleet f (§12.1) |
| `CheckVeszelyesVonal(f, x, y, r)` | 0x50a690 | 0x5b2880 | `CheckVeszelyesHely(f, x, y, r, false) ≠ 0` |
| `MenekulesHaKell(f, kind, target)` | 0x50f310 | 0x5b13b0 | flee or regroup (§12.2) |
| `BezartSzog(f, ax, ay, bx, by)` | 0x50a180 | 0x5b3040 | angle at f between two points, degrees (§12.3) |
| `MenekulesSuly(f, own, foe, x, y)` | 0x5100b0 | 0x5b3230 | how safe a point is (§12.3) |
| `MI_HannyalHarcol` / `MI_KitAkarTamadni` | 0x50f040 / 0x50f080 | 0x5b37c0 / 0x5b3850 | `diplomacy.md` §6.2 |
| `ElkuldHaduzenet(a, b)` | 0x4c09c0 | 0x520db0 | declare war (`diplomacy.md` §5) |
| `SearchFlottahozKozeliBolygo(f, lim, own, unprot)` | 0x5411a0 | 0x616110 | nearest planet (§12.4) |
| `FindNearestKolonizalo(x, y, e)` | 0x53cdf0 | 0x6198a0 | nearest colony fleet (§12.4) |
| `IndultEKolonizalo(e, p)` | 0x53eb70 | 0x6192c0 | a colony ship is on its way to p (§12.4) |
| `DeleteKolonizalo(f, p)` | 0x53c2e0 | 0x618eb0 | send one colony ship of f to p (§12.5) |
| `FlottaMozgatHaNincsAzonosito` | 0x53d360 | 0x617a30 | `FlottaMozgat` (0x53d270) unless the fleet has a story id |
| `TankAtpakol(from, to, d, n)` | 0x541410 | 0x617700 | move vehicles (§12.6) |
| `FlottaOsszevon(into, from)` | 0x53d860 | 0x617000 | merge fleets (§12.7) |
| `TervNew` / `TervClear` / `TervAutoDesign` | 0x5422f0 / 0x541be0 / 0x541920 | 0x619b80 / 0x619e80 / 0x61b100 | §10.2 |
| `TalalmanyRarakhato` / `GetMaxPodok` / `GetMaxRarakhato` | 0x4b6ca0 / 0x4b4290 / 0x4b42f0 | 0x4fb540 / 0x4fb390 / 0x4fb4c0 | pod rules (§10.3) |
| `TervDelete(e, d, f, mark)` | 0x541d50 | 0x619f70 | delete a design's units (§12.8) |
| `FlottakUjraSzamol` | 0x53dd90 | 0x618b40 | recount every fleet and garrison (§12.9) |
| `FlottaDelete` / `HajoDelete` / `TankDelete` | 0x53d040 / 0x53e5d0 / 0x5414c0 | | `space-combat.md` §5.3 |

Randomness: **stream 10** only (`rndlog(10, …)` at `mi.cpp` lines 0x4f1, 0x80c, 0x8c1, 0x90e). The war
declaration's own draws (stream 9, libc `rand()` for texts) are diplomacy's.

## 8. Data
### 8.1 State (exe VAs)
| Name | exe VA | Layout |
|---|---|---|
| `MiUtolsoSzarKolonizalas` | 0xDB05E8 | i32 `[19]`: the planet the last fled colony fleet was heading for (skipped once) |
| `MiMennyiIdeigNemPakolhatFelTankot` | 0xDB0638 | i32 `[19]`: days left without loading vehicles |
| `MiMennyiIdeigTartATankPakoloFazis` | 0xDB0688 | i32 `[19]`: days left of the loading phase |
| `MiMennyiIdeigNemLehetTervUpgrade` | 0xDB06D8 | i32 `[19]`: days until the designs are upgraded |
| `MiFelderitesRacs` | 0xDB0738 | u8 `[19][21][21]`: scouting grid, 30×30 cells over −300..300 (0 unknown, 1 to scout, 2 known) |
| `BirodalomEro` | 0xBAEA78 | f32 `[19]`: power (diplomacy) |
| `BirodalomHanyBolygo` | 0xBAEB18 | i32 `[19]`: planets owned (diplomacy) |
| `BirodalomTamadhat` | 0x915F44 | u8 `[19]`: may attack |
| `BolygoMennyiErodEro` | 0xB99CB8 | i32 per planet: fortification strength |
| `StoryAdat+0xb0/+0xb8/+0xbc` | 0xF274A8 / 0xF274B0 / 0xF274B4 | story: no war declarations; protect empire 1 / 5 from the strongest |
| `OsszHanySajatFlotta` | 0xBB40B0 | i32 `[19]`: fleet name counter |
| `RadarHatotav2` / `RadarHatotavSzorzo` | 0xBB61A8 / 0xBB6668 | f32 `[19][8]` / f32 `[19]` (`fleet-movement.md` §6) |
| planets | | `+0x58/+0x5c/+0x60` garrison vehicles / their ground strength / (cleared by §12.9), `+0x100 + 2k` space stations (`+0x114` count) |
| fleets | | `+0x19 + e` seen by e, `+0x38` speed, `+0x40/+0x42/+0x44/+0x48` order, kind, target, `+0x4c/+0x4e` colony ships / terraformers, `+0x52` flags (bit 3 colonising, bit 4 terraforming, bit 5 radar-ignored, bit 10 wormhole, bit 14 protected), `+0x54..+0x78` totals (`space-combat.md` §3) |

### 8.2 Rule tables (static, `ig2-rules`)
`Alapok` (hulls, 0x90BBF8: `+2..+6` flags, `+0xc` hull tech), `PodTipusok` (0x8F8918, 55 × 0x54: `+0` id, `+0x14`
16 techs), `PodFajok` (0x8F9B28, `[14][27][12]` × 8: pod type, max count), `HanyasFajPodTablazatatHasznalja`
(0x902F10), `Talalmanyok2` (0x8EB060, u8 `[193][9]`: ground-hull priority by race). The live tech table (0xB9C0C8,
0x114 per tech) gives `+0x24` max pods per slot (u8), `+0x26` "no special variant" (u8), `+0x3c + race` AI
priority (u8), `+0x45 + e` status.

## 9. `FRISSIT_MI_KOLONIZALAS(e)` (exe 0x50d9a0)
`C3 = MiGyarthatoCuccok[e][3]` (colony ships), `C2 = …[2]`.
```
if C3.units[0] > 0 or (Ev == 0x917 and Honap == 3) or (VanUresBolygo[e] == 0 and Nap == 10):
    pick = 0; best = 1e9 (f32); VanUresBolygo[e] = 0
    for p in 1..=HanyBolygo:
        if (MP or getMiAltalElfoglalhato(p)) and p.owner == 0 and p.visibility[e] >= 0:
            if p.visibility[e] > 0:
                if p != MiUtolsoSzarKolonizalas[e]:
                    d = (cy − p.y)² + (cx − p.x)²          f32 (cx, cy = (f32)centre)
                    SP: d = (f32)(((f64)rndlog(10, 0, 10) · 0.1 + 1.0) · (f64)d)    (one draw per candidate)
                    if (d < best or pick == 0) and not IndultEKolonizalo(e, p)
                       and (not BolygoInfoIs(p, −0x2094) or MP): pick = p; best = d
                VanUresBolygo[e] = 1
            if BirodalomHanyBolygo[e] < 6: VanUresBolygo[e] = 1
    if pick > 0 and C3.units[0] > 0:
        f = FindNearestKolonizalo((int)pick.x, (int)pick.y, e)
        if f > 0 and MenekulesHaKell(f, 0, 0) == 0:
            seen = f.+0x19..+0x2b (19 bytes)
            g = DeleteKolonizalo(f, pick);  g.+0x19..+0x2b = seen
            if g != f and f.owner != 0xff and CheckVeszelyesVonal(g, pick.x, pick.y, 10.0): FlottaOsszevon(f, g)
if VanUresBolygo[e] == 0 and (C2.units[0] > 0 or C2.queued[0] > 0 or C3.units[0] > 0 or C3.queued[0] > 0):
    TervDelete(e, C2.design[0], 0, false); TervDelete(e, C3.design[0], 0, false)
    FRISSIT_MI_DESIGN(e, false)
```
`getMiAltalElfoglalhato(p)` = `p.owner < 9 and p.+0x187 == 0`. `rndlog(10, 0, 10)` is drawn for every candidate,
before the `IndultEKolonizalo` test. The distance is f32, the random factor and the product f64 (`mulsd`/`addsd`
@ 0x50dadc..0x50daec, then `cvtpd2ps`). `FindNearestKolonizalo` gets the planet's coordinates truncated to int. After `DeleteKolonizalo`
(which may create a fleet `g` and delete `f`) the seen-by bytes of the old record are copied into g's; with `g ==
f` they are the same bytes. A dangerous route merges the new fleet back (`FlottaOsszevon(f, g)`, g is freed).
With no colony ship left in demand (`VanUresBolygo 0`), the colony-ship and terraformer designs' units, queue
entries and stations go (`TervDelete` without the delete mark: the designs stay) and the designs are redone
without slots 13 and 14.

## 10. `FRISSIT_MI_DESIGN(e, all)` (exe 0x50abf0)
Skipped for a human-controlled empire (`PlayerKi[e] == 1`) and while the AI is off (exe 0xB77244).
### 10.1 Slots
`n = OsszHanyTerv[e]` (the empire's design count), `upg = MiMennyiIdeigNemLehetTervUpgrade[e] == 0`.
For slot s in 1..=24 and variant v in (best 0, special 1), skipping v 1 in MP and skipping slots 13 and 14 unless
`VanUresBolygo[e]` or `all`; `H = Alapok[s]`, `t = H.tech`:
```
if v == 1 and (tech[t].+0x26 or any of H.+5, +6, +4, +2, +3): MiTervMutatoSpec[e][s] = 0; next
if MiTervMutato[e][s] > n: MiTervMutato[e][s] = 0;  if MiTervMutatoSpec[e][s] > n: MiTervMutatoSpec[e][s] = 0
d = v == 0 ? MiTervMutato[e][s] : MiTervMutatoSpec[e][s];  redo = upg
if d == 0:
    if status(t, e) != 5: next
    for k in 0x29..=n: if v == 0 and design k: +0x16 >= 0, +0x18 != 0, +0x19 == 0, +0x44 == s: d = k  (the last)
    if d == 0: d = TervNew(e, 0); redo = true
    if d < 1: next
    if redo: TervClear(e, d)
    design d: +0x44 = s; +0x00..+0x14 = 0; name (SP: DesignPoenosanElnevez(e, s, v) — a localised text, ported: new-game.md §14;
              MP: the hull tech's name); slot[v][e][s] = d; +0x20 = 0
if d > 0:            (d < 0 is skipped; a fresh d is never 0 here)
    old = design d.+0x20 (build time)
    if redo: TervAutoDesign(e, d, v)
    TervUjraSzamol(e, d, false, true)                   (space-combat.md §2.4)
    if old > 0: queue_retime(d, old)
```
`queue_retime(d, old)`: for e's planets 1..=n, line l in (0, 1): if `+0x78 + 8l` (the line's first design) == d:
`+0xa0 + 4l = max(0, +0xa0 + 4l − old + new)` (i32 wrapping, new = design d's new `+0x20`).
### 10.2 Upgrades and the end
If `upg`: for designs k in 0x29..=n with `+0x16 >= 0` and `+0x18 != 0` that no slot (1..=24, best or special)
points to: `old = +0x20`, `TervAutoDesign(e, k, 0)`, `TervUjraSzamol(e, k, false, true)`, `queue_retime(k, old)`
(here without the `old > 0` test). Then `FlottakUjraSzamol()` (§12.9).

`TervNew(e, id)`: the lowest k in 1..=n with `+0x16 < 0` (deleted), else `n + 1` (the table grows by one record;
bytes the code below doesn't write keep whatever the reallocated block held); name[0] = 0, `TervClear`, `+0x18 = 1`,
`+0x40 = id > 0 ? id : PlayerFaj[e]`. `TervClear(e, k)`: `+0x18 = 1, +0x19 = 0, +0x1a = 0, +0x1c = 0 (i32), +0x6c =
0 (i32), +0x70 = 0, +0x74 = +0x78 = +0x7c = 0 (i32), +0x50..+0x5c = 0, +0x5d..+0x69 = 0, +0x80 = 0 (i32)`.
### 10.3 `TervAutoDesign(e, d, v)`
`r = PlayerFaj[e]` (1 if > 8), hull `h = +0x44`, hull tech `ht = Alapok[h].tech`, pod table `pt = +0x40`. For slot k
in 1..=`GetMaxPodok(pt, h)` (re-evaluated each step): `+0x50 + k = 0`; over techs t 1..=192 with status(t, e) == 5:
```
s = tech[t].byte(0x3c + r) + 100;   if v == 1 and ht in 0x91..=0x95: s = Talalmanyok2[t][r] + 100
if v == 0 and t in 0x1a..=0x1d: s −= 100
if not A[v][r] and t in 0x31..=0x36: s −= 100
if not B[v][r] and t in 0x41..=0x43: s −= 100
if not C[v][r] and t in 0x44..=0x45: s −= 100
if s > 0 and TalalmanyRarakhato(h, t, k, 5, e, pt) and best <= s: best = s, pick = t    (ties: the later tech)
```
with the 18-byte tables `A` = 0,1,1,1,1,1,1,1,1,0,0,0,0,1,1,0,0,0; `B` = 0,0,0,0,1,1,1,0,0,0,1,1,0,0,0,1,0,1;
`C` = 0,0,0,0,0,0,0,0,0,0,0,0,1,1,1,0,1,0 (index `9v + r`). `+0x50 + k = pick` if any; `+0x5d + k =
min(tech[+0x50+k].+0x24, GetMaxRarakhato(pt, h, k))` (tech 0's when the slot stays empty). `e == EN` and t 0x12
on hull tech 5 fires `TutorialCheckEvent(0xb)` (an event). `best` starts at −1000.

`TalalmanyRarakhato(h, t, k, min, e, pt)`: true for techs 0xa1..=0xa9 while the debug switch exe 0xA3F0B0 is set;
false if status(t, e) < min; else true if t is one of the 16 techs of the pod type `PodFajok[table(pt)][h][k −
1].type` (the first `PodTipusok` record with that id; none found reads record −1: not reached with the shipped
tables). `GetMaxPodok(pt, h)` = slots 0..=11 of `PodFajok[table(pt)][h]` with type > 0; `GetMaxRarakhato(pt, h, k)`
= `PodFajok[table(pt)][h][k − 1].max_count`; `table(pt) = HanyasFajPodTablazatatHasznalja[pt]`.

## 11. `FRISSIT_MI_FLOTTAK(e)` (exe 0x50b1c0)
`M = MiFoFlotta[e]` (re-read at every use; slot 0 when there is none), `E` = power `BirodalomEro`.
### 11.1 Story guards, timers
`s1` = the first alive empire 1..=ProgramPlayers with the strictly greatest power, `s2` likewise among the others.
SP only: with `S8 = StoryAdat+0xb8 != 0`, `S12 = +0xbc != 0`: `S8 and S12`: `g1 = s1 ≠ 1 and s2 ≠ 1 and PlayerKi[1] ≠
1`, `g5 = s1 ≠ 5 and s2 ≠ 5 and PlayerKi[5] ≠ 1`; only `S8`: `g1 = s1 ≠ 1 and PlayerKi[1] ≠ 1`; only `S12`: `g5 = s1 ≠
5 and PlayerKi[5] ≠ 1`. A guarded empire's planets are no attack target.
Then `NoLoad −= 1` if > 0, `Phase −= 1` if > 0, `NoUpgrade −= 1` if > 0 and on reaching 0 `FRISSIT_MI_DESIGN(e, true)`.
### 11.2 Vehicles
If `NoLoad < 1`, over vehicle entries 1..=n (design ≠ 0x7FFF, count > 0), with `kind(d) = design d.+0x44 − 0x12`
(the hull; > 0 for the ground hulls):
- held by a planet p of e: `if best[p] < kind: best[p] = kind, bd[p] = d, bn[p] = count; top = max(top, kind)`;
  `spare += count`;
- held by M (fleet number equal, 0 included) with `0 < kind < mk`: `mk = kind, md = d, mn = count` (`mk` starts at 1000).
If `NoLoad < 1` and (`M.+0x5c < M.+0x78` or `mk < top`):
```
p = SearchFlottahozKozeliBolygo(M, 20.0, own, unprotected)
if (M.+0x5c < M.+0x78 or mk < best[p]) and p > 0 and best[p] > 0 and p.+0x58 > 0:
    room = M.+0x78 − M.+0x5c
    if mk < 1000 and mk < best[p] and room < bn[p]:
        x = min(bn[p] − room, mn);  if x > 0: TankAtpakol(M, −p, md, x)        (unload the weaker kind)
        room = M.+0x78 − M.+0x5c;  spare −= x
    x = min(bn[p], room);  if x > 0: TankAtpakol(−p, M, bd[p], x)               (load the best kind)
```
### 11.3 Relations, war
Wars with dead empires end: for u in 1..=ProgramPlayers2, u ≠ e, level[e][u] == 1 and not alive: both levels = 0.
If `StoryAdat+0xb0 == 0` or MP, `MI_HannyalHarcol(e) == 0`, `t = MI_KitAkarTamadni(e) > 0` and
`BirodalomMiotaIsmeri[e][t] < 0`: `rndlog(10, 1, 720) == 1` or `BirodalomHanyBolygo[e] == 0` or (`Nehezseg == 4`,
t human and a second `rndlog(10, 1, 720) == 1`) → `ElkuldHaduzenet(e, t)`.
### 11.4 Target
If `BirodalomTamadhat[e]`, `M > 0` and not (Ev 0x917 and Honap 3): `kind = 0, tgt = 0, best = 1e10f`.
**Fleets** (only if e owns planets): f in 1..=n with owner u ≠ 0xff, story id 0, level[e][u] == 1, `f.+0x19+e`,
`f.+0x60 < M.+0x60`, and (`M.+0x60 / 8 < MiFoFlotta[u].+0x60` or `f.+0x68 > 0`):
```
d = (f32)sqrt((f64)((M.y − f.y)² + (M.x − f.x)²));  if Phase < 1: d = d / 3.0f
k = clamp((int)((f64)((M.+0x38 − f.+0x38) · 50.0f + 5.0f) + 0.5), 0, 10)          (NaN → 0)
r = f.+0x68 > 0 ? G[k] : N[k]      N = 10,10,10,10,10,20,30,35,40,45,50   G = 30,30,30,30,30,100,130,160,200,1000,1000
weak = f.order == 2 and f.kind == 1 and planet (int)f.+0x44 is e's and (f64)f.+0x68 · 1.3 > (f64)(ErodEro[p] + p.+0x5c)
if weak: r = r · 2.0f
if d <= r:
    sc = ((f32)(M.+0x60 / (f.+0x60 + 1) + 3) · d) / (f32)(k + 2);  if weak: sc = sc / 3.0f
    if sc < best: best = sc; kind = 2; tgt = f
        bold = k >= 6 and (d <= r · 0.5f or M.+0x60 < 2·f.+0x60) and (f64)M.+0x60 > (f64)f.+0x60 · 1.3
```
**Planets**: `g = Nehezseg > 2 ? 1.2f : 1.0f`; `free = cand = false`; `best = 1e10f`; p in 1..=HanyBolygo not
guarded (`g1` and owner 1, `g5` and owner 5), owner o ≠ 0, `p.visibility[e] > 0`, level[e][o] == 1,
`getMiAltalElfoglalhato(p)`, o ≠ e: `cand = true`, `def = (int)((f32)(ErodEro[p] + p.+0x5c) · g)`; if `Phase < 1`,
`M.+0x5c > 0`, `M.+0x68 > 0`, (`CheckVeszelyesHely(M, p.x, p.y, 10, false) == 0` or no planets) and (`def <=
M.+0x68` or no planets):
```
a = (f32)sqrt((f64)((p.x − cx)² + (p.y − cy)²)) + 100.0f          (cx, cy: (f32) of the int centre)
b = (f32)sqrt((f64)((p.x − M.x)² + (p.y − M.y)²)) + 50.0f
sc = (((a · b) / (f32)(p.pop + 30000)) / (f32)(p.+0x70 + p.+0x74 + 5000)) · (f32)(M.+0x68 + def · 5)
if (f64)M.+0x68 < (f64)def · 1.5: sc ·= 10
if (f64)def · 2.0 <= (f64)M.+0x68: free = true  else sc ·= 10
if sc < best and (kind == 0 or (not bold and (f64)def · 1.5 <= (f64)M.+0x68) or no planets):
    best = sc; kind = 1; tgt = p
```
(`kind` in the last test is the fleet loop's result.) Then: if not `free`, `Phase < 1`, `kind == 1`, `cand`,
`M.+0x5c < M.+0x78`, `M.+0x5c < spare · 4` and `M.+0x5c < (M.+0x78 − M.+0x5c) · 4`: `Phase = 300`, `kind = 0`
(gather vehicles first). If `Phase > 0` and (`M.+0x78 <= M.+0x5c` or `spare < 1` or not `cand`): `Phase = 0`.
### 11.5 Orders
If `M > 0` and M is e's: `flee = MenekulesHaKell(M, kind, tgt)`; then every other fleet f of e (1..=n) with story
id 0, `MenekulesHaKell(f, f.kind, (int)f.+0x44)` ∉ {1, 2, 3}, flags bits 3, 4 clear, and (bit 5 clear or `f.order
== 0`): `FlottaMozgatHaNincsAzonosito(f, 1, 2, M, 0)` (follow the main fleet), bit 5 set, cleared again if `colony
ships + terraformers == ships`. (`MenekulesHaKell` is called for every candidate before the flag tests.)

If `kind > 0`: `rndlog(10, 1, Nehezseg == 1 ? 50 : 1) != 1` → `kind = 0` (the draw happens on every difficulty).
If `M > 0` and `flee` ∉ {1, 2, 3}:
- `kind > 0`: `FlottaMozgatHaNincsAzonosito(M, 2, kind, tgt, 0)` (attack);
- else if `M.order == 0`: `M.+0x54 − M.+0x58 <= 0` (fighters only) → move to `MiFoBolygo[e]`; else **scout**: for
  every planet of e, cell `cx = (int)((x + 300.0f) / 30.0f + 1.0f)` (f32; likewise cy): the 3×3 block around it
  (cells 1..=20) = 2, the 5×5 block's zeros = 1; M's own cell = 2 (unchecked). Over cells (i, j) 1..=20 (i outer)
  with value 1: `sc = (f32)((f64)(f32)sqrt((f64)((Mx − i)² + (My − j)²)) / 3.0 + (f64)(f32)sqrt((f64)((Cx − i)² +
  (Cy − j)²)))` (the squares f32 of ints; `Cx = (centreX + 300) / 30 + 1` in ints), the first smallest below
  9999999.0f. Then if `cand`, `NoLoad < 1`, (`M.+0x5c < M.+0x78` or `mk < top`) and (no cell or `rndlog(10, 1, 1)
  == 1`): the planet of e with `(M.+0x5c < M.+0x78 or mk < best[p])` and `best[p] == top` and the greatest `(f32)p.+0x5c
  ² / ((p.y − (f32)(int)M.y)² + (p.x − (f32)(int)M.x)² + 1.0f)` (> 0, first on ties) → move there (`1, 1, p`); else
  a cell → move to the point `((i − 11) · 30, (j − 11) · 30)` (`1, 0`); else to `MiFoBolygo[e]`.
### 11.6 Unloading
If `flee` is 2 or 3 and `p = SearchFlottahozKozeliBolygo(M, 30.0, own, unprotected) > 0`: while `M.+0x5c > 0`:
the last vehicle entry of M (design ≠ 0x7FFF, count > 0, `kind(d) > 0`) → `TankAtpakol(M, −p, d, count)`;
then `NoLoad = 60`. (A fleet whose ground units are all of kind ≤ 0 would loop forever: never seen.)

## 12. Helpers
### 12.1 `CheckVeszelyesHely(f, x, y, r, _, &dist, &foe)` → 0 / 2 / 3
`o` = f's owner, `own = (f32)f.+0x6c · (f32)f.+0x60`, `m = Nehezseg > 2 ? 0.9f : 0.7f`, `T` = 1.8, 1.7, 1.5,
1.3, 1.2, 1.0, 0.7, 0.6, 0.5, 0.4, 0.3 (f32). Over fleets g 1..=n seen by o (`+0x19+o`), owner ≠ 0xff, ≠ o, story 0,
not in battle (bit 0):
```
k = clamp((int)((f64)((f.speed − g.speed) · 50.0f + 5.0f) + 0.5), 0, 10)
a = T[k] · r;  d2 = (g.y − y)² + (g.x − x)²                                    (f32)
if d2 < a·a and level[o][g.owner] < 4 and (level == 1 or d2 < (a·0.5f)·(a·0.5f))
   and best < (f32)g.+0x6c · (f32)g.+0x60:
    best = that · m;  dist = (f32)sqrt((f64)d2);  foe = g
    near = (f64)(a·0.25)² > d2 or ((a·0.33)² > d2 and 0.02 >= Δ) or ((a·0.5)² > d2 and 0 >= Δ)
           or ((a·0.6)² > d2 and −0.02 >= Δ) or ((a·0.7)² > d2 and −0.04 >= Δ)        (Δ = f.speed − g.speed; f64)
```
`best = 0` if o has no planets, or has < 2 and `SearchFlottahozKozeliBolygo(f, 20, own, any) < 1`. Result: `own <
best` → (`near` of the last update ? 3 : 2), else 0. `dist`/`foe` keep the last update even when the result is 0.
### 12.2 `MenekulesHaKell(f, kind, t)` → 0 none, 1 regroup, 2 flee, 3 flee (close)
Story fleets: 0. `o` = owner, `b5` = f's bit 5. Target point `(tx, ty)` = planet t (kind 1), fleet t (kind 2), else
(0, 0). `h = CheckVeszelyesHely(f, f.x, f.y, 20.0, …, &dist, &foe)`; if f ≠ `MiFoFlotta[o]` and `(f.y − M.y)² + (f.x −
M.x)² < 400.0f`: h = 0. If h ≠ 0: `chased = foe.order == 2 and foe.kind == 2 and foe.+0x44 == (f32)f`, `idle =
foe.order == 0`; kind 1: `df = |f − target|`, `dg = |foe − target|` (f32 sqrt of f64): `df + 5 <= dg` or (`df + 1 <= dg`
and idle) → **return 0**. h 3 and idle → h = 2.

**h == 0**: kind ≠ 0 or f colonising (bit 3) → 0. Else the nearest other fleet g of o (g ≠ f, `g.+0x60 > 0`, story 0,
bit 3 clear, g ≠ M) by `(f32)(int)((g.x − f.x)² + (g.y − f.y)²)` strictly below 400.0f: both follow each other
(`FlottaMozgatHaNincsAzonosito(f, 1, 2, g)`, `(g, 1, 2, f)`), f's bit 5 = b5, g's bit 5 set (cleared if colony ships +
terraformers == ships) → return 1; none → 0.

**h ≠ 0** (2 or 3): `v = foe − f` (f32), `lv = (f32)sqrt(f64 |v|²)`; `best = −1e6f`, point = (0, 0), `via = 0`.
`ok(a)` = `a >= 80 and (a >= 90 or (idle and t > 0)) and (a >= 100 or not chased) and (a >= 140 or h == 2)`;
`gain(a, x, y)` = (h == 2 and kind > 0 ? 10000.0f − BezartSzog(f, tx, ty, x, y) : MenekulesSuly(f, o, foe.owner, x, y))
\+ (h == 2 ? a · 0.125f : a · 0.25f) (the sum f32; `10000 − b` is x87 from two f32s, rounded once).
1. Directions: φ from 0.0f, `φ = (f32)((f64)φ + 0.39269908169872414)` while `(f64)φ < 2π`: `c = ((f32)cos((f64)φ) ·
   10.0f + f.x, (f32)sin((f64)φ) · 10.0f + f.y)`; a = the angle at f between foe and c (inline `BezartSzog`, 0 when
   either length ≤ 0.001); `ok(a)` and `gain > best` → best, point = c.
2. Fleets g of o (bit 0 clear or MP; g ≠ M; bit 3 clear; story 0; g == f included), `d2 = (f.x − g.x)² + (f.y − g.y)²`:
   (f == M or d2 < 25) and d2 < 2500 and (h ≠ 2 or kind == 0 or d2 < 9); `a = BezartSzog(f, foe, g)`; if `ok(a)`:
   `w = base + 5` (base = the `gain` term without the angle part), `+5` if `(f64)d2 < 225`, `+5` if `d2 < 64`, `+10` if
   `own/4 <= (f32)g.+0x6c · (f32)g.+0x60` (`own = (f32)f.+0x6c · (f32)f.+0x60 · 0.25f`), plus the angle part; `> best`
   → best, point = g's position, `via = g`.
3. kind > 0: a = angle between foe and the target; `ok(a)` → `go = (h == 2 ? 10000 − BezartSzog(f, t, t) :
   MenekulesSuly(f, o, foe.owner, tx, ty)) + angle part + 1000.0f > best` (f32, left to right).
Then: f colonising with order (1, 1) → `MiUtolsoSzarKolonizalas[o] = (int)f.+0x44`. If `Nehezseg < 4` and `Ido %
(9·Nehezseg) == 0`: `FlottaMozgatHaNincsAzonosito(f, 0, 0, 0, 0)` (stop), return 0 (Nehezseg 0 divides by zero:
the game never sets it). Else `go` → return 0 (keep going for the target). Else `via > 0`: f and via follow each other,
via's bit 5 set; else f moves to the point (`1, 0`); f's bit 5 = b5, cleared if colony + terraformers == ships; the
same clear test on `via` (slot 0 when none); return h.
### 12.3 `BezartSzog` and `MenekulesSuly`
`BezartSzog(f, a, b)`: `u = a − f`, `v = b − f` (f32), `lu = (f32)sqrt((f64)(u.y² + u.x²))`, `lv` likewise; either
`<= 0.001` (f64 compare) → 0; `c = (v.y·u.y + v.x·u.x) / (lv · lu)` (f32), clamped by f64 compares to `−0.99f` /
`0.99f`; result `(f32)((f64)((f32)acos((f64)c) · 180.0f) / π)`.

`MenekulesSuly(f, o, q, x, y)`: `r1 = RadarHatotav2[o][1] · RadarHatotavSzorzo[o]`, `r2` likewise for q (f32).
`s1 = (f32)sqrt` of the smallest `(p.x − x)² + (p.y − y)²` over o's planets (1e10f when none); `w = s1 < r1/3 ? s1·0 +
0 : s1 < r1 ? (s1 − r1/3)·6 + r1·0 : (s1 − r1)·10 + r1·4` (f32), `w = 10000 − w`. `s2` likewise over q's planets:
`s2 < r2·5·0.25 → s2·16 + 0 + w`; `s2 < r2·13·0.25 → (s2 − r2·5·0.25)·4 + r2·20 + w`; else `(f32)((f64)(s2 − r2·13·0.25)
· 0.5 + (f64)(r2·28) + (f64)w)`.
### 12.4 Searches
`SearchFlottahozKozeliBolygo(f, lim, own, unprot)`: with f's owner ≠ 0, the planet (1..=n, owned by f's owner if
`own`) with the smallest `|f.y − p.y| + |f.x − p.x|` strictly below `lim` (f32); 0 if none; then 0 if `unprot` and
the result's `+0x64` (planet 0's when none). `FindNearestKolonizalo(x, y, e)`: e's fleets with colony ships
(`+0x4c > 0`) and bit 3 clear, the first smallest `(int)(|f.x − (f32)x| · 2.0f + |f.y − (f32)y|)` below 99999999.
`IndultEKolonizalo(e, p)`: some fleet of e with `+0x44 == (f32)p`, order (1, 1) and bit 3 or 4.
### 12.5 `DeleteKolonizalo(f, p)`
The first ship i of f whose design (f's owner's) has `+0x34` (colony ship); none → 0. `f.+0x54 < 2`: the name becomes
"Colonization" (13 bytes, NUL included), `FlottaMozgat(f, 1, 1, p)`, bit 3 set, colony pool −1, return f. Else
`g = FlottaNew("Colonization", owner, 0, f.x, f.y, 0)` (`production-research.md` §5.1), `HajoAtpakol(i, g)` (the
ship moves: sums −1 on f, +1 on g), `FlottaMozgat(g, 1, 1, p)`, bit 3, pool −1; f with `+0x54 < 1` → `FlottaDelete(f)`;
return g. `FlottaMozgat` (`fleet-movement.md` §2) first returns colony ships / terraformers of a colonising /
terraforming fleet to the pools.
### 12.6 `TankAtpakol(from, to, d, n)`
`TankDelete(from, d, n)` (a planet holder `−p`: also its ground-war map, `RemoveTankFromPlanet` + `RearrangeTankGroups`,
not modelled: an event), `TankNew(to, d, n)` (`production-research.md` §5.1, garrison sums for planets).
`TankDelete` on a planet subtracts from `+0x58`/`+0x5c` like the fleet sums (`TankFlottahozHozzaszamol` with −n).
### 12.7 `FlottaOsszevon(into, from)` (both in 1..=n, distinct)
radar `+0x7c = max`; if into's name equals `AlapFlottaNev`, `strcpy` from's name (up to and including its NUL); story
id from `from` if into's is 0; bit 11 = name ≠ `AlapFlottaNev`; fleets (≠ into, from) with order > 0, kind 2 and
target `from` → `FlottaMozgat(g, g.order, 2, into)` keeping their bit 5; from's ships → `HajoAtpakol(i, into)`; from's
vehicles → `TankAtpakol(from, into, d, count)`; `OsszHanySajatFlotta[from.owner] −= 1`, from's owner = 0xFF; human
owner → into's bit 5 cleared; colony + terraformers == ships → cleared.
### 12.8 `TervDelete(e, d, f, mark)` (d > 0)
1. Ships of e's fleets (`f == 0` or in fleet f) with design d or a design whose `+0x1c == d` → `HajoDelete`.
2. Vehicles likewise (holder's owner e) → `TankDelete(holder, design, count)`.
3. `f == 0`: e's planets' stations `+0x100..` (count `+0x114`) with d (or `+0x1c == d`) are zeroed, then compacted.
4. Fleets 1..=n alive with `+0x54 + +0x5c < 1` and not bit 10/14 → `FlottaDelete`.
5. `f == 0`: e's planets, line l 0, 1, while the line holds d at k (first of 0..3): shift entries k+1..3 down (design
   words, counts `+0x88`, repeats `+0x90`, into-fleet `+0xad`, raw bytes), last design word = 0 (its bytes stay);
   k == 0: `+0xa0 + 4l` = the new first design's build time (design 0's when empty), `+0xa8 + l = 0`.
6. `mark`: design d `+0x16 = −1` (and the design screen's list for the human).
### 12.9 `FlottakUjraSzamol()`
Alive fleets: `+0x4c..+0x78` totals = 0 (colony ships, terraformers, `+0x54..+0x74`, `+0x78`); planets 1..=n:
`+0x58 = +0x5c = +0x60 = 0`; pools 1..=18 = 0; ships/vehicles of fleets beyond `OsszHanyFlotta` are freed; then every
ship adds its sums (`HajoFlottahozHozzaszamol`, pools and bit 13 included) and every vehicle entry its sums.

## 13. `INIT_MI` (new game) and `INIT_MI_KIESETT(e)` (an empire handed to the AI)
`INIT_MI`, per empire 1..=ProgramPlayers2: `VanUresBolygo 0`, alive, countdown 100, main fleet / planet 0, danger 1,
the four timers and `MiUtolsoSzarKolonizalas` 0, centre 0, scouting grid 0, design slots 1..=24 (both) 0, then
`FRISSIT_MI_DESIGN(e, true)` for AI empires; then relations reset (diplomacy, `diplomacy.md` §2) and
`FRISSIT_Viszony`; empires without planets die; categories 1..=10 cleared (`+0x0c, +0x1c, +0x02, +0x10, +0x20, +0x04,
+0x14`). `INIT_MI_KIESETT(e)` (alive and not human-controlled): the same per-empire reset with `NoUpgrade = 720`
(no grid reset), then `FRISSIT_MI_DESIGN(e, true)` if AI.
