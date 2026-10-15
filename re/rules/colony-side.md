# Colony side systems (rebellion, conquest, autobuild, money, per-empire totals)

Status: **verified at runtime** (§12): whole economy steps with rebellions and autobuild, and direct calls of the conquest, rebellion, colonisation and money functions, reproduced bit-exactly by `engine/crates/ig2-sim` (`colony_side.rs`, `tests/colony_side_trace.rs`). Companion to `colony-economy.md`,
which covers the planet economy itself. Everything here runs inside or next to the exe's
`FRISSIT_Planet` (**0x474930**) or is called from it. ELF names are the symbol oracle; the exe is normative.
No rule values are given; tables are read from the user's exe (ADR 0004) or install text.

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Planet` | 0x474930 | 0x4e6ef0 | economy step; prologue, rebellion call, autobuild, totals, epilogue |
| `BolygoFellazad(p, to, force)` | 0x53bc40 | 0x61dcb0 | rebellion outcome (streams 1 and 8) |
| `BolygoElfoglal(p, owner)` | 0x53b720 | 0x61ce70 | planet changes hands (rebellion, conquest, abandonment, colonisation) |
| `Research_TalalmanyUjraSzamol` | 0x5245c0 | 0x5eb450 | tech status recompute (called by `BolygoElfoglal`) |
| `getMiAltalElfoglalhato` / `getFellazadhat` | 0x4efe70 / 0x4efe50 | | `owner < 9 && +0x187 == 0` / `owner < 9 && +0x186 == 0` |
| `MoneyMennyi(e, pool)` | 0x540170 | 0x622720 | spendable money |
| `MoneyHozzaadOld(e, pool, x)` | 0x53fbd0 (via the `…Log` wrapper 0x53fb10) | | add/spend money |
| `GetGyartasAr(e, tech)` | 0x4acaf0 | 0x4faff0 | price |
| `EpuletNew(p, kind, done, x, y, dmg%)` | 0x53c7d0 | 0x61c170 | new building record |
| `EpuletNewSearchLocation` | 0x53cca0 | 0x61ca80 | placement (`LocateSuitableLocation` 0x4869d0) + `EpuletNew` |
| `Generate_Planet0(p, owner, race, big)` | 0x59adf0 | 0x7075a0 | fresh colony (colonisation arrival) |
| `INIT_AutoBuild` | 0x47d280 | 0x4ed5f0 | reads `autobld1..10.txt` |

## 1. Data (exe VAs)
### 1.1 Per-empire and per-planet globals
| Name | exe VA | Layout |
|---|---|---|
| `LaborSzint` | 0x912728 | i32 `[19][4]` (index `[e][1..=3]`): lab levels (ship, vehicle, building research) |
| `LaborSzintOsszes` | 0xBA90E0 | i32 `[19][4]`: lab buildings (finished, working or not) |
| `HanyKemLehet` / `…Osszesen` | 0xBB2388 / 0xBB23AC | i32 `[9]`: spy capacity (max 12) / spy centres |
| `HanyKemKozpontBolygonkent` | 0xBB2418 | i32 `[9][201]`: spy centres per planet (**never reset**, §7) |
| `HanyTradePortVan` | 0xBB23F4 | i32 `[9]` |
| `BolygoMennyiFoldiEro` / `ErodEro` / `IonGunEro` / `Tuzoltosag` | 0xB99990 / 0xB99CB8 / 0xB99FE0 / 0xB9A308 | i32 `[201]` (index 1..=200) |
| `BolygoMennyiIdejeLazad` | 0xB99668 | i32 `[201]` (unrest counter, `colony-economy.md` §11) |
| `BolygoUtolsoKozeliFlotta` | 0xB99340 | i32 `[201]` |
| `FullGDP` / `FullTax` | 0xBAE060 / 0xBAE0B0 | i32 `[19]` |
| `TradeIncomeLastYear` / `…ThisYear` | 0xBAE100 / 0xBAE150 | i32 `[19]` |
| `TaxIncomeLastYear` / `…ThisYear` | 0xBAE1A0 / 0xBAE1F0 | i32 `[19]` |
| `Money` | 0xBAD168 | f64 `[19]` = pools 1 + 2 + 3 |
| `MoneyTeruletenkent` | 0xBAD200 | f64 `[19][6]`: pools 1..=5 (1..=3 spendable; 2 = buildings) |
| `MoneyHatar` | 0xBAD590 | f64 `[19][6]`: `[e][0]` reserve, `[e][k]` per-pool reserve (autobuild writes `[e][2]`) |
| `MoneyTeruletSzorzo` | 0xBADCB0 | f64 `[19][6]`: income share per pool |
| `NehezsegPenzSzorzo[Nehezseg]` | 0x8F8848 f32[5], index 0x92827C | AI income multiplier |
| `BirodalomEl` / `BirodalomIsmeri` | 0xBAE6AC u8[19] / 0xBAEBB8 u8[19][19] | alive / `[a][b]`: a knows b |
| `RndHanyadik` / `RndOsszEsely` | 0xBAEB64 / 0xBAEBB4 | weighted-choice state (§2) |
| tech table `Talalmanyok` | 0xB9C0C8, 0x114 B × 193 | per empire `e`: status `+0x45+e` (u8; 1 unknown, 2 seen, 3 researchable, 5 have), research time `+0x58+4e` (i32), "given" flag `+0xa4+e`, seen-building matrix `+0xc0 + 9·i + o` (§9) |
| all-techs flag | 0xB77254 | non-zero: statuses 1/2 become 3 |

### 1.2 Planet fields used here (beyond `colony-economy.md` §2.1)
`+0x18/+0x1c` f32 position · `+0x23` colony map · `+0x54` u16 · `+0x58/+0x5c` i32 · `+0x68` autobuild counter ·
`+0x6c` autobuild mode 1..=10 (`autobldN.txt`) · `+0x6d` aggressiveness 1..=3 · `+0x78..+0xb8` production
queue (reset from `pGyartas0` 0x90B8A8, 64 bytes; `+0xac` lies in it) · `+0xc4+e` spy levels ·
`+0xd8+2e` i16 visibility per empire · `+0x114` i32 · `+0x186` can't rebel · `+0x187` can't be taken by AI.

### 1.3 Autobuild tables
Compiled into the exe (index 1..=28, ig2-rules): `AutoBuildHanyasProblema` 0x8F1D50 (category → problem
slot; 0 = "listed" category, −1 none), `AutoBuildMitEpitMin` 0x8F1DD0 / `…Max` 0x8F1E50 (building type
range of the category), `AutoBuildNevek` 0x8F1ED0 (char*[17], names of categories 1..=16).
From `text/autobld1.txt`..`autobld10.txt` (`INIT_AutoBuild`; mode `m` = file number), lines whose first
character is `:`: 1st = mode name (ignored), 2nd = `atoi` → `ProblemaAlsoSzazalek[m]` (0x996504), 3rd →
`EgyebSzazalek[m]` (0x9965A4), then names matched exactly against `AutoBuildNevek[1..=16]` and appended to
`EgyebEpitendok[m][1..=50]` (0x9965D0, i32 `[11][51]`) until `END` or 50 entries. Unknown names are skipped.

## 2. Weighted choice (`RndInitValasztas` / `RndValasztas`, inlined in the exe)
`init`: `n = 0, total = 0`. `choose(stream, w)`: `n += 1; total += w; return rndlog(stream, 1, total) <= w`.
Calling it for candidates in order and keeping the last `true` picks candidate `i` with probability
`w_i / Σw` (reservoir sampling).

## 3. Rebellion (`BolygoFellazad(p, to, force)`, exe 0x53bc40)
Called by the economy step with `(p, 0, true)` when the unrest counter exceeds 30 (`colony-economy.md` §11),
**in the middle of the planet's step**, after the counter and before construction.
```
if !(owner < 9 && +0x187 == 0) and SP: return 4                         (no RNG)
choice = 0; init
for e in 1..=8 (e != owner, BirodalomIsmeri[owner][e]):
    count = 0f; sum = 0f
    for q in 1..=HanyBolygo with owner(q) == e:
        count += 1f;  sum += |p.y − q.y| + |p.x − q.x|                  (f32; this grouping)
    avg = count != 0 ? sum / count : 0
    if BirodalomEl[e] and count > 0:  if choose(1, (int)(1000f / (avg + 1f))): choice = e
r = rndlog(8, 1, 100)
if r < morale / 20 and !force:  +0x174 = 400, +0x178 = 1; return 4        (suppressed)
k = rndlog(8, 1, 3)
new = choice == 0 or k == 2 ? 10 (k := 2 when choice == 0)
    : k == 1 and to ≥ 1 ? to : choice
BolygoElfoglal(p, new);  message 0x13 if new == EN
return (new == to and to > 0) ? 1 : k
```
The economy step then fires story special 0x15 for result 2 (independents, owner 10) and 0x16 for 3.
The ELF sums `(sum + |dx|) + |dy|`; the exe `sum + (|dy| + |dx|)`: exe kept. Owner 10 can't rebel again
(`owner < 9`) and is skipped by the per-empire totals that require `owner < 9`.

## 4. Change of owner (`BolygoElfoglal(p, new)`, exe 0x53b720)
For `p > 0`, in this order (UI-only effects omitted):
1. Production queue `+0x78..+0xb8` := `pGyartas0`.
2. If the planet was owned: every live fleet with `order 2 (attack), kind 1 (planet), target == p` gets
   `FlottaMozgat(f, 0, 0, 0, 0)` (order cleared; `fleet-movement.md` §2).
3. Every garrison (`Tankok` entry with fleet `−p`, design ≠ 0x7FFF) is deleted (`TankDelete`; plus ground-war
   bookkeeping per tank).
4. Ground-war state of `p` (`Warfare`, stride 0x8EC from 0x9C33C4): units with flag 0x4000000 change side.
5. Every owned planet whose `+0xac` (in the production queue) is `p` gets `+0xac = 0`.
6. **Tech capture**: for each finished building (kind ≠ 0, work 0) with tech `t` and `status[t][new] < 5`:
   `given[t][new] = 1`, `research_time[t][new] = (int)((f32)tech[t].research_time × RaceResearchTime[race(new)])`,
   `status[t][new] = 5`, then `Research_TalalmanyUjraSzamol` (§4.1); message 7 to the human.
   `new` is used as an index even when 0 or ≥ 9 (the table has room for 19 empires).
7. `BolygoUtolsoKozeliFlotta[p] = 0`, unrest `= 0`, owner `= new`, `+0x6c = 1`, `+0x6d = 2`, tax `= 3`,
   `+0x68 = 0`, morale `= 500`, `+0x114 = 0`.
8. For `new` in 1..=8: every living empire `e ≠ new` with `vis[e] > 0` on `p` that doesn't know `new`
   now does (`BirodalomIsmeri[e][new] = 1`; for `e == EN` story special 0xb and `FelfedezBirodalom`).
9. `Warfare[p].+0x460 = −1`.
10. For `new > 0`: negative visibility entries `vis[1..=18]` become 0.
11. SP and `new != EN` and epidemic `> 0`: remove planet event 100, epidemic `= 0`, virus cleanup.

### 4.1 Tech status recompute (exe 0x5245c0)
For `e` in 1..=`ProgramPlayers2`, tech `t` in 1..=192 with prerequisites `a, b, c` (`+0x2d..+0x2f`):
```
have(x) = x != 0 and status[x][e] == 5
if status == 1 and (have(a) or have(b) or have(c) or a == b == c == 0 or given): status = 2
if status == 2 and for each x in (a, b, c): (status[x][e] == 5 or x == 0 or given): status = 3
if all-techs flag: 1 → 3, 2 → 3
```
(The second test reads `status[0][e]` for a missing prerequisite; tech 0's status is never 5 in practice
but the formula uses it, so the port does too.)

## 5. Colonisation arrival (`fleet-movement.md` §4.3, case 1)
A colonising fleet (bit3) arriving with `order 1, kind 1` at an unowned planet `p`:
`BolygoElfoglal(p, owner)` → `Generate_Planet0(p, owner, race(owner), false)` → `d = rndlog(1, 0, 0)`
→ `EpuletNewSearchLocation(p, 1, false, d)` (colony hub, under construction) → story special 10 →
`FlottaDelete(f)` (the per-owner fleet count 0xBB40B0 drops by 1).
`Generate_Planet0` (exe 0x59adf0): owner, race; population `rndlog(1, 2900, 3100)` (big = true:
`2000 + rndlog(1, 8900, 9100)`); `+0x2c = 0`, morale 500, tax 3, `+0x38/+0x3c/+0x40/+0x44 = 0`,
`+0x58/+0x5c = 0`, `+0x54 = 0`, `+0x68 = 0`, mode/aggressiveness `1/2`, production queue reset,
`+0x70/+0x74 = 0`, `vis[owner] = 1000`.

### 5.1 Terraforming arrival (`fleet-movement.md` §4.3, case 2)
Surface `s` = first argmax over 1..=8 of `FajBolygoViszony[owner][s]` (the owner's *empire number* as
the row; exe 0x596585..0x59664e). Colony map = `NextColonyMap(s − 1)` (exe 0x486920), a per-kind
round-robin over the terrain sections (`Terepek[i]+0` = the `[felszin]` type, `colony/terepleiro.txt`;
cursor `u8[k]` at exe 0x996420, `hanyterep` = count at 0x9b2ec8):
```
u = cursor[k]; scan i = u..count for Terepek[i].type == k → u = i, done
not found: u = 0; while u < count and Terepek[u].type != k: u += 1      (u = count if none)
if u + 1 != count: cursor[k] = u + 1  else: cursor[k] = 0               (k ≥ 8 would abort; unreachable here)
return u == 60 ? 63 : u                                                  (map 60 is never used: 63 instead)
```
Verified with real fleet arrivals (`traces/2026-10-15-arrivals.md`: 127 colonisations, 51 terraformings,
owner races randomised so the empire-number row is pinned).

## 6. Autobuild
### 6.1 Per planet (inside the economy step, after taxes and the `+0x68` counter, before spy expulsion)
Skipped in March 2327 (`Ev == 0x917 && Honap == 3`) and when mode or aggressiveness is 0. Uses the
building-pass results (§4 of the economy) from **before** any rebellion of this step, the smoothed
problem levels `P[k]` (`+0x118+4k`) and the targets `T[k]` (×10000) of this step.
```
if active[1] < 1:                                    (no working colony hub)
    if inactive[1] == 0: best[owner] = (kind 1, score 2_000_000_000, planet p)   (unconditional)
    → done
for c in 1..=28: MitEpit[c] = last t in Min[c]..=Max[c] with status[tech(t)][owner] == 5, else 0
have[c] (c 1..=16) = Σ_{t=Min[c]..=Max[c]} active[t] + inactive[t]
need[] = 0; j = 1
loop:  c = Egyeb[m][j]; if c > 0: need[c] += 1
       last = j; j += 1
       if c == 16 and owner < 9 and tradeports_before[owner] > 0: c = 0          (prologue snapshot)
       continue while j < 51 and Egyeb[m][j] > 0 and (have[c] >= need[c] or MitEpit[c] == 0)
want = Egyeb[m][last];  if !(have[want] < need[want]): want = 0
if want in 5..=7: L = labs_before[owner] (LaborSzintOsszes snapshot, [1..=3])
    want = 5; if L2·18 < L1·10: want = 6; if L3·18 < L1·10 and L3·10 < L2·10: want = 7
if want == 13 and owner < 9 and spies_before[owner] > 11: want = 0
if energy_demand < energy_produced:
    lim = ProblemaAlso[m];  aggr 1: (int)((f64)lim × 1.5);  aggr 3: (int)((f64)lim × 0.66)
    for c in 1..=28 with k = HanyasProblema[c] ≥ 0 and b = MitEpit[c] > 0:
        eligible = (k == 0 or (lim·10000 ≤ P[k] and lim·10000 ≤ T[k]))
               and (k > 0 or (workers_demand + desc[b].workers ≤ pop and c == want))
               and inactive[b] == 0
        if eligible:
            n = (k == 10 ? Σ inactive[10..=13] : Σ_{Min[c]..=Max[c]} inactive) + Σ_{t=1..=51} building[t]
            if n == 0 and (k != 10 or effect[b] < 4):
                s = P[k] / lim;  k == 0: s = Egyeb%[m] × 10000 / 10
                k == 10: s = (int)((f32)s / (3f − 2f / ((f32)effect[b] + 1f)))
                if s > best_s: best_s = s, best_c = c                       (first maximum wins)
else: best_c = 1, best_s = 1_000_000
if best_c == 0 or inactive[MitEpit[best_c]] > 0: best_s = 0     (MitEpit 0: see below)
score = (counter / 20 + (aggr + 2) × 10) × best_s
if score > best[owner].score: best[owner] = (MitEpit[best_c], score, p)
```
When the energy branch picks category 1 and no power plant is researched, `MitEpit[1] = 0` and the test
reads `inactive[0]`: the exe zeroes the per-type arrays only for types 1..=55 (0x4755a3 onwards), so this
is an uninitialised stack slot (`[ebp−0x58c]`, only ever accessed indexed; type 0 is never counted).
Both outcomes occur at runtime: positive → no purchase (most traced sessions); ≤ 0 → building 0 is
"bought": its price is checked, then the colony-map search reads before the footprint table and fails,
so the planet's counter becomes −1000 (9 steps of one fuzzed session). The port makes this explicit
(`ZeroPick::Skip`, default, or `ZeroPick::Attempt`); the replay accepts a step when either reproduces it.
`active/inactive/building/effect` are the economy's per-type counts (`colony-economy.md` §4 step 6).
Abandoning planet `p` (§8) clears `best[owner]` if it points at `p`.

### 6.2 Purchase (epilogue, once per empire 1..=18, `full && only == 0`)
```
if best.score < 1: MoneyHatar[e][2] = 0
else: money = MoneyMennyi(e, 2); price = GetGyartasAr(e, tech(best.kind))
      if (f64)price <= money:
          slot = EpuletNewSearchLocation(best.planet, best.kind, false, 0)
          if no slot: planet.+0x68 = −1000
          else: MoneyHozzaad(e, 2, −price); planet.+0x68 = 0; kind 50: HanyTradePortVan[owner] += 1 (unbounded)
          MoneyHatar[e][2] = 0
      else: MoneyHatar[e][2] = price + 100
```
The trade-port increment has no bound check: for owners ≥ 9 (the rebels, 10, seen at runtime) it writes
`HanyKemKozpontBolygonkent[0][owner − 9]`, which follows `HanyTradePortVan[9]` in memory. Kept.
`GetGyartasAr(e, t)`: race 4 or 7 and `t == 13`: `(int)((f64)price[13] × 0.75)`; race 5 and `t ∉ 81..=144`,
`t ∉ {13, 14}`: `(int)((f64)price[t] × 0.7000001)`; else `price[t]` (tech `+0x14`).

### 6.3 Building record (`EpuletNew`, exe 0x53c7d0)
Slot: if occupied buildings (`HanyEpulet[p]`, recounted by the building pass) < slots, the first empty
slot (none found: returns `slots + 1` without writing, which the caller takes as success), else a new
slot appended (`realloc` in steps of 20). Record: kind, `+1` = owner, flags `&= 0x80` (the three masks
0xfd, 0x9f, 0xe2: bit 7 survives from whatever was in the slot; a freshly appended slot is uninitialised
memory, seen as 1 under Frida's debug heap — undefined, the port writes 0), damage `max_hp × dmg% / 100`,
work `build_work × 50` (owner race 4: `(int)((f64)w / 1.5)`; `done`: 0), position, `+0xa` = planet (u8),
yield 100 with: solar (53): event 0x619 → 0, else
event 0x5FB → story value `st_v(0x5FB)`; factories 19..=22: event 0x60F → `st_v(0x60F)`, else event 0x669
→ 120, then event −0x673 → 0; magma (54): event 0x64B → `(u8)(int)(yield × 1.33)`; solar on surface 3 /
magma on surface 4 → `(u8)(int)(yield × 1.5)`. Flags `&= 0xe2`; shown energy/workers = descriptor; `+0x1e = 0`,
`+0x1f = 1`. The position (`+0xc/+0x10`) comes from the
colony-map search (`LocateSuitableLocation`, `colony-placement.md`, ported as `ig2-sim::placement`; the
`Placement` callback gets the planet's current state).

### 6.4 Money (`MoneyMennyi` 0x540170, `MoneyHozzaadOld` 0x53fbd0), all f64
`S[k] = max(0, pool[k] − Hatar[k])`, `others(k) = ((0 + S[a]) + S[b]) + …` over pools 1..=3 except `k`
(in order 1, 2, 3; for `k = 0` all three).
- `MoneyMennyi(e, k ≥ 1)`: `x = pool[k] + others(k)`; if `Money − x < Hatar[0]`: `x = Money − Hatar[0]`;
  if `Szorzo[k] < 0.01`: 0. `k < 1`: `Money`.
- `MoneyHozzaad(e, k, x)`, `x > 0`: non-human owner: `x = (f64)NehezsegPenzSzorzo[d] × x`, and in MP `× 0.5`.
  `k < 1`: pools 1..=5 `+= Szorzo[i] × x`; else `pool[k] += x`.
- `x ≤ 0`, `a = −x`: `own = k > 0 ? min(pool[k], a) : 0`; `sur = others(k)`.
  `k == 0` and `((pool1 + 0) + pool2) + pool3 < a`: every pool 1..=3 `= 0 − Szorzo[i] × (((a − p1) − p2) − p3)`,
  `Money` recomputed, **fails** (the debt is booked anyway). `k > 0` and `own + sur < a`: fails, no change.
  Otherwise `a −= own; pool[k] −= own`; `r = sur > 0.001 ? (a ≤ sur ? a/sur : 1) : 0`; for pools `i ≠ k`
  (1..=3): `d = S[i]` (recomputed), `a −= d·r; pool[i] −= d·r`. `k == 0` and `a > 0.01`:
  `tot = ((p1 + 0) + p2) + p3`, `r = tot > 0.001 ? (a ≤ tot ? a/tot : 1) : 0`, `pool[i] −= pool[i]·r`.
- Always (unless failed early): `Money = ((p1 + 0) + p2) + p3`.
The monthly tax payout is `MoneyHozzaad(owner, 0, tax)` (`colony-economy.md` §8), so AI taxes are scaled
by the difficulty multiplier.

## 7. Per-empire totals
**Prologue** (`only == 0`, every call): for `e` 1..=18, `i` 1..=3: snapshot `labs_before = LaborSzintOsszes`,
`LaborSzint[e][i] = (e == EN && SP) ? StoryAdat[0x9c + 4(i−1)] : 0`, `LaborSzintOsszes = 0`.
`e` 1..=8: snapshot `spies_before = HanyKemLehetOsszesen`, `HanyKemLehet = HanyKemLehetOsszesen = 1`;
snapshot `tradeports_before`, `HanyTradePortVan = 0`. Planets 1..=200: Foldi/Erod/IonGun `= 0`, Tuzoltosag `= 1`.

**Per planet** (`only == 0`, after production capacity, owner read **after** any rebellion):
```
for i in 1..=3:  if StoryAdat+0x24 == owner and SP: LaborSzint[EN][i] += effect[13+i]
                 LaborSzint[owner][i] += effect[13+i];  LaborSzintOsszes[owner][i] += active + inactive of 13+i
Foldi[p]   += Σ_{t=28..=43} (active+inactive)[t] × TalalmanyokrEro[tech(t)]
Erod[p]    += Σ_{t=32..=35} active[t] × TalalmanyEroHP[tech(t)]
IonGun[p]  += Σ_{t=28..=31} active[t] × TalalmanyEroHP[tech(t)]
Tuzoltosag[p] = 8 × active[8] + 1
owner < 9:  HanyKemLehet[o] = min(12, HanyKemLehet[o] + effect[26])
            HanyKemLehetOsszesen[o] += (active+inactive)[26];  HanyKemKozpontBolygonkent[o][p] += same
            HanyTradePortVan[o] += (active+inactive)[50]
```
(`TalalmanyokrEro` is static, 0x8EB730. `TalalmanyEroHP` 0xBA9210 is filled by `InitNewGame` (ELF loop at
0x583577): `(int)sqrt((f64)((f32)TalalmanyokrEro[t] × (f32)tech[t].hp))` for `t` 1..=192, `hp` = tech
`+0x20`; checked against the live table.)

## 8. Abandonment
`full`, no occupied slot (`HanyEpulet == 0`), `+0x64 == 0`: clear `best[owner]` if it is this planet,
message 5 to the human, `BolygoElfoglal(p, 0)` (§4 with `new = 0`: no meetings, no visibility fix-up,
the tech-capture loop finds no buildings).

## 9. Epilogue
After the autobuild purchase (§6.2):
- **Yearly income** (monthly: `Nap == 1 && full && only == 0`): for `e` 1..=18:
  `TaxIncomeThisYear[e] += FullTax[e]`; on 1 January `TaxIncomeLastYear = TaxIncomeThisYear`,
  `TaxIncomeThisYear = 0`, `TradeIncomeLastYear = TradeIncomeThisYear`, `TradeIncomeThisYear = 0`
  (messages 0x14/0x15 to the human when positive).
- **Seen buildings** (`full && only == 0`): for every owned planet `p` (owner `o`) and `i` 1..=18 with
  `pMuholdakMennyi(p, i)` (`i == 7 && spy[7] == 0`, or `spy[i] != 0` — so empire 7 always qualifies),
  every non-empty slot sets byte `tech(kind)·0x114 + 0xc0 + 9i + o` of the tech table to 1.
  For `9i + o ≥ 0x54` this writes into the **next** tech's record (bytes 0..), an original overflow kept
  as is (it only fires for spies of empires ≥ 9 or `i` large).

## 10. Order and RNG
Per planet: building pass → unrest counter → **rebellion** (streams 1, then 8) → construction → problems
→ morale → growth → taxes → counter → **autobuild scoring** (no RNG) → spy expulsion → production →
**totals** → **abandonment**. Epilogue: **purchase** → **yearly income** → **seen buildings**.
Stream 1: one roll per rebellion candidate; stream 8: one or two rolls per rebellion.

## 11. Exe vs ELF
- Rebellion distance sum grouping (§3); the exe inlines `RndValasztas`.
- `GetGyartasAr` multipliers are f64 in both; autobuild limits ×1.5/×0.66 are f64 (`mulsd` 0x476bd2,
  0x476bf1); the recreation score is all f32 (0x476de4..0x476e12).
- The exe snapshots `LaborSzintOsszes` with stride 16 from `+4`: same indices as the ELF.

## 12. Verification
See `traces/2026-10-15-colony-side.md`.
