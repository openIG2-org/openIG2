# Campaign scripting §8.11 `st_BRnd_include` (exe 0x5c9f20, ELF 0x768d50): random stories, part 1

Split out of `../campaign.md` (sections, numbering and references unchanged: `campaign.md §8.11` means this file).
Shared terms, the story engine and the notation are in `../campaign.md` §1..§8.8.

Ported in `engine/crates/ig2-sim/src/brnd.rs` ([`BRndStories`], run first: `Story_Check` calls `BRnd` before
`Fable`, `Scen`, `Line`, `Elem`), verified at runtime (`traces/2026-10-16-brnd.md`). Stories 220..=239 (0xdc..0xef,
the random stories story 400 draws) and their follow-ups 2301 (0x8fd) and 2311 (0x907); 225 is empty (the exe
only sets `StoryLastChecked`). Each story block catches an abort with `WIN(0, 1)` and goes on (§8.7). Ids are
decimal: story `s`'s messages, variables and counters are `10·s + k` (2200 = 0x898). `b(x)`, `v(x)`, `C(x)`, `A(b,
r)` (answered), `P(b)` (promoted, flag 7), `D(b)` (`st_Letilt(b, 0)`, flag 8) as in §8.9; `M(id, arg)` =
`NewMessage(id, arg)`; "the human's" = owned by `EN`.

Before the stories, in every check: special 0x19 (flag 14) with `d = Tervek[owner of fleet d1][d2]` and `220 ≤
d.+0x80 / 10 < 230` → `TervDelete(EN, d2, d1, true)` (exe 0x541d50).

**The mercenaries** 220, 221, 222, 223, 224, 226, 227, 228, 229: an offer to hire a fleet.

| Story | `M` | price | start: counter `X` := | offer when (else `X` := 500 days) | battle bonus (`StoryAdat`) |
|---|---|---|---|---|---|
| 220 | 2200 | 80,000 | 2209: rnd(500, 1000) | tech 7 has status 5 and `S/3 > 2020` | `+0x84 += 20` |
| 221 | 2210 | 200,000 | 2219: rnd(500, 1000) | tech 8, `S/3 > 2520` | `+0x7c += 10`, `+0x90 += 22` |
| 222 | 2220 | — | 2229: rnd(500, 1000) | tech 4, `S/3 > 1020` | `+0x90 += 8` |
| 223 | 2230 | 27,000 | 2239: rnd(2000, 3000) | `S/3 ≥ 721` (else `X` := 50 days) | `+0x84 += 5` |
| 224 | 2240 | 230,000 | 2249: rnd(500, 1000) | tech 8, `S/3 > 1520` | `+0x84 += 12`, `+0x90 += 4` |
| 226 | 2260 | — | 2269: rnd(2500, 3500) | `S/2 ≥ 521` (else 50 days) | `+0x84 += 10`, `+0x7c += 10` |
| 227 | 2270 | 10,000 | 2279: rnd(1500, 2000) | always | `+0x90 += 8` |
| 228 | 2280 | 24,000 | 2289: rnd(500, 1000) | tech 6, `S/3 > 250` | `+0x7c += 12` |
| 229 | 2290 | 250,000 | 2299: rnd(500, 1000) | tech 9, `S/2 > 3520` | `+0x84 += 10`, `+0x7c += 16` |

`S` = `st_GetOsszFlottaEro()` (exe 0x5eaaf0): the sum of the human's design `+0x2c` over the ships 1..=`OsszHanyHajo`
with a design (`≠ 0x7fff`) in a fleet of the human's; "tech `t`" = `Talalmany[t].+0x45+EN == 5` (tested first).
Blocks in order: start → `X` := the draw (stream 5) days, `v(X) := 0`. `C(X)` → `M(M, 160)` or the re-arm. Then,
for the priced ones: `A(M, 3)` or `A(M+1, 3)` → counter `M+1` := 360 days (227: 60); `C(M+1)` → `M(M+1, 160)`;
`A(M, 2)` or `A(M+1, 2)` → **hire**: `st_AddMoney(−price)` (`FUN_0053fb10(EN, −1, −price)`), `f =
st_fNewEx(_s(5, M), EN, M, 0, 1, 0)` (§8.6), `FlottaMozgat(f, 1, 1, last_fn_b, 0)`, `st_RemoveFlottaAzonosito(M)` (the
first live fleet with id `M` gets `+0x16 := 0`), `v(X) := 1`; `A(M, 4)`, `P(M)`, `A(M+1, 4)` or `P(M+1)` → `WIN`.
222 and 226 have one message: `A(M, 2)` hires (no money), `A(M, 3)` or `P(M)` → `WIN`. 227's last block brings the
enemy instead (`st_fNewEx(_s(5, M), 10, M, 0, 1, 0)`, no `WIN`), and then `C(2272)` or that: `f = st_fSearch(M)`:
none → `WIN`; else `n = st_fGetNearest((int) f.x, (int) f.y, EN)`: none → `st_fEliminate(M)` and `WIN`, else
`FlottaMozgat(f, 2, 2, n, 0)` (it chases the human's nearest fleet); counter 2272 := 4 days (after a `WIN` too).
Last: a space battle starts (flag 1) while `v(X) != 0`: `f = st_hSearch(EN, M+1)`: 0 → `WIN`, else
`st_SpWarStartID(f)` → the bonus.
- `st_hSearch(e, key)` (exe 0x6052a0): `key > 40`: the design `d` of 40..=`OsszHanyTerv[e]` with `+0x80 == key`
  (none → 0), else `d = key`; the fleet (`Hajok[k].+0`) of the first ship 1..=`OsszHanyHajo` of design `d` in a
  fleet of `e`'s, else 0.
- `st_SpWarStartID(f)` (0x602e30): flag 1 and (`f == 0` or one of `spFlottak` 0xBAA1C6 / 0xBAA1CC is `f`).
- `st_fGetNearest(x, y, owner)` (0x604970): over the fleets 1..=`OsszHanyFlotta` of `owner` (0: any live one) the
  first, or one with `(y − f.y)² + (x − f.x)² < (f32) best` (f32; `best` the last pick's truncated value); 0 if none.
- `st_fEliminate(id)` (0x604750): `f = id < 0 ? −id : st_fSearch(id)`; `f < 1` → `StoryError := 1`. A fleet already
  marked (`+0x18` bit 1) with ships (`+0x40 != 0`) stays; else the mark is set, `x = rnd(0, 1)·600 − 300`, `y =
  rnd(−300, 300)`, `rnd(0, 1) != 0` swaps them; `(cx, cy)` = the mean of the human's visible planets 1..=n
  (truncating: `acc = (int)((f32) acc + p.x)`, then `acc / n`; none → 0); `((f32) cx − f.x < 50) != (x ≥ 0)` → `x :=
  −x`, the same for `y`; `FlottaMozgat(f, 1, 0, x, y)` (it flies off); `StoryError := 0`.

**230 (a fleet in quarantine):** start: `e = GetRndBirodalom(0, 0, 0)` (below), `v(2302) := e`, `M(2300, 70)`,
parameter 1 = `e`'s name, `v(2309) := 0`. `A(2300, 2)`: game info 2301 (`e`'s name); two designs, each `TervNew(EN,
0)` + the record below (names `_s(5, 2300)`, `_s(5, 2301)`) with `+0x80 := 2300` / `2301` (the second also `+0x18 :=
0`), `TervUjraSzamol(EN, d, 0, 1)`, then `+0x20 ×= 5`; the first's slot → `v(2301)` (before its recompute);
counters 2300 := 3 and 2302 := 1080 days; `ScrollUzenet(_s(5, 2302), 1, true, false)`. `A(2300, 3)` or `P(2300)` →
`WIN`. `C(2300)` (re-armed 3 days): the first ship 1..=`OsszHanyHajo` of design `v(2301)` in a fleet `f` of the
human's with ships (`+0x40 != 0`) heading for a planet (`+0x42 == 1`, `p = (int) f.+0x44`) without an epidemic
(`p.+0xb8 < 1`) owned by `v(2302)` within `st_tav < 20` (below) → `M(2302, 0)`, the ship becomes `st_GetTerv(EN,
2301, 3)`, `p.+0x28 /= 2`, `v(2309) += 1` (3 → counter 2302 := 1 day). `C(2302)`: `M(2301, 0)` if a ship of design
`v(2301)` in a fleet of the human's is left, else `v(2309) == 0` → `M(2304, 0)`, else `M(2301, 0)`; every such ship
becomes `st_GetTerv(EN, 2301, 3)`, `TervDelete(EN, v(2301), 0, true)`, `StartStory(2301)`, `WIN` (a running 2301
becomes `StoryLastChecked` first: the `WIN` then ends it instead of 230, §8.9's `S`).
**2301:** start: counter 23011 := 10 days. `C(23011)`: re-armed 10 days; `st_hSearch(EN, 2301) == 0` →
`TervDelete(EN, st_GetTerv(EN, 2301, 3), 0, true)`, `WIN`.
- `GetRndBirodalom(a, b, c)` (exe 0x5c27c0): `e = rnd(1, 8)`; up to 10 tries while `e` is `a`, `b`, `c`, `EN`, dead
  or unknown to the human: `e := e % 8 + 1` (the 10th failure keeps `e`); then the same without "unknown"; both
  failing aborts (§8.7).
- `st_tav(x1, y1, x2, y2)` (inlined) = `(int)(f32) sqrt((f64)(f32)((x1 − x2)² + (y1 − y2)²))` of truncated
  coordinates.
- The design record (0x84 bytes): `+0x16` i16 1, `+0x18` 1, `+0x19` 1, `+0x1a` 1, `+0x20` 150,000, `+0x24` 300,
  `+0x3c` f32 0.1, `+0x40` i16 `EN`, `+0x44` / `+0x48` 12 (231: 13), the rest 0 except the bytes the exe leaves as its
  stack held them (`+0x15`, `+0x1b`, `+0x29..+0x2b`, `+0x3a`, `+0x3b`, `+0x42`, `+0x43`, as in §8.10); the name is
  `strcpy`'d over `+0x00`.

**231 (a planet to blow up):** start: `e = GetRndBirodalom(0, 0, 0)`; unknown to the human → `WIN(0, 0)`; else `p =
st_bSearch(−e, 0)` (none → abort) gets id 2310, `st_bShow(2310, −1)` (§8.10), `M(2310, 0)`, parameter 1 = `p`.
`A(2310, 3)`: a design (13/13, `+0x80 := 2310`, not ×5) → `v(2310)`, recomputed, counter 2310 := 3 days, game info
2311 (planet `b(2310)`). `A(2310, 4)` or `P(2310)` → `WIN`. Daily: `p = b(2310)` the human's → `WIN`; else `d =
v(2310)`, counter 2310 := 3 days, the first ship 1..=n of design `d` in a fleet of the human's within `st_tav(fleet,
p) < 20` → `M(2312, 0)`, `p` gets id 23111, `StartStory(2311)`, `FlottaDelete(fleet)`, `WIN`. After either `WIN`:
`TervDelete(EN, st_GetTerv(EN, 2310, 3), 0, true)`.
**2311:** start: counter 23111 := 2 days. `C(23111)`: video `_s(6, 2312)`, `st_BolygoDestroy(b(23111))`, `WIN`.

**232:** start: over planets 1..`HanyBolygo` − 1: (the human has more than 50 planets or the planet is free) and the
human's view `+0xd8[EN] == 0` and `rnd(0, 2) == 0` (drawn only then) → view := 1; any → `M(2320, 0)`; `WIN`.

**233 (strikes):** start: `st_bGet3BestProd()` (below) none → `WIN(0, 0)`; else `M(2330, 100)`, the three planets
get ids 2330..=2332 and are parameters 1..=3. `A(2330, 2)`: each gets info 2331 (`BolygoInfoAdd(p, 2331, true)`)
and morale −20 (`st_BolygoMoralValt(p, −20)`: `+0x174 += 200`, `+0x178 := 6`), counter 2331 := 1080 days.
`A(2330, 3)` or `P(2330)` → `WIN`. `C(2331)`: `M(2331, 0)`, info 2331 removed, morale +20 (`+0x174 −= 200`), `WIN`.
- `st_bGet3BestProd()` (exe 0x603350): the human has fewer than 3 planets → none. Else over planets 1..`HanyBolygo` −
  1 of the human's that the human sees: `v = +0x70 + +0x74 + 1`; slot `k` = 0, then 1 if `val[1] == 0` or (`val[0]
  != 0` and `val[1] < val[0]`), then 2 if `val[2] == 0` or (`val[k] != 0` and `val[2] < val[k]`); `val[k] == 0` or
  `val[k] < v` → `val[k] := v`, `best[k] := planet`. `val` is local, **`best` static** (exe 0xF432B8): slots not
  refilled keep the planets of an earlier call. Any `best[k] == 0` → none, else `best`.

**234 / 235 (a tax):** start: the human has fewer than 5 planets → `WIN(0, 0)`; the other one (235 / 234) in
`HistoryVodor` → `WIN`; else `M(2340 / 2350, 0)` and `SetParamInt(1..=3, (int)((f64) T · q))`, `q` = 0.9, 0.8, 0.7
(235: 0.85, 0.75, 0.65), `T` = the sum of `+0x28` over the human's planets 1..=`HanyBolygo`. `A(M, r)`, `r` = 2,
3, 4: `q` = the `(r − 1)`th factor (f32); every planet 1..`HanyBolygo` − 1 of the human's: `+0x28 := (int)((f32)
+0x28 · q)` (235 first adds `(1 − q)·(f32) +0x28` to `paid`, f32, truncating; then `v(2351) := paid`); part 2 →
`WIN`, 3 → counter `M` := 50 days, 4 → counter `M+1` := 50 days. 234: `C(2340)` → `M(2341, 0)`, global morale +40
(`st_GlobalMoralValt`: `GlobalisMoralCSokkentoBarkacs` 0xBAEAC4 `−= 400`, 0xBAEB14 := 6), `WIN`; `C(2341)` →
`M(2342, 0)`, morale +70, `t = st_GetRandomTalalmany(−1, 0)`: none → parameter 1 `_s(5, 2341)`, else
`st_SetTalalmanyAllapot(t, 5, −1)` and parameter 1 = the tech's name; `WIN`. 235: `C(2350)` or `C(2351)`: `paid =
v(2351)`, `n = (C(2350) ? 3 : 6) + paid / 140,000`; steps `L` = 7, 15, 30, 50, 80, 140 (then 280): the first `L` with
`paid ≤ 1000·L` (`lo` the step before, 0 at first), `big = clamp((paid − 1000·lo) / (L − lo) · n / 1000, 0, n)` (C
division), `M(2351, 0)`, `f = st_fNewEx(_s(5, 2351), EN, 0, 0, 1, 0)`, `FlottaMozgat(f, 1, 1, last_fn_b, 0)`,
`st_fAddShip(−f, st_GetTerv(EN, 2350 + k, 3), n − big)`, `st_fAddShip(−f, st_GetTerv(EN, 2351 + k, 3), big)`,
`st_fAddTank(−f, st_GetTerv(EN, [901, 901, 902, 902, 903, 903, 903][k], 3), n)` with `k` the step's index, `WIN`.
- `st_GetRandomTalalmany(e, group)` (exe 0x5eab60): for groups `g` = 0..=10 the first tech `t = 16g + j` (`j` =
  1..=16) the empire can be given (`st_TalalmanyAdhato`: status 1..=3 and each prerequisite `+0x2d..+0x2f` 0 or at
  status 5): `rnd(0, n)` (`n` = such groups before), 0 → the pick (`group != 0`: that group only).
- `st_SetTalalmanyAllapot(t, 5, e)` (exe 0x602920): the tech is given (`TalalmanyKifejlesztve`, research time
  `(int)((f32) +0x38 × FajSpeckoKutatasIdoSzorzo[race])`, the recompute 0x5245c0, `ResearchAktFejlesztett := 0` if it
  was being researched, negative prerequisite bytes := 0) and **`(e, t)` is added to `TalalmanyVodor`**: the check's
  end runs a special with `SpecialType = e`, `SpecialData1 = t` (§4 step 7).

**236 (a refugee planet):** start: fewer than 10 planets → `WIN(0, 0)`; else `M(2360, 100)`, the human +12,000, `p =
st_bSearch(−EN, 0)` gets id 2360, parameter 1. `A(2360, 2)` or `P(2360)` → `WIN`. `A(2360, 3)` → counter 2360 := 50
days. `D(2360)` and `b(2360)` not the human's → `LetiltReszlet(4)`. `A(2360, 4)`: `b(2360)` the human's → morale +30
(`+0x174 −= 300`); `WIN`. `C(2360)` → `M(2361, 100)`. `A(2361, 2)`: `p = st_GetUnknownPlanet()` (below) gets id 2360,
`st_SetBolygoTulaj(p, EN, 10)` (race 10), `+0x28 := 10240`, `AutoAddBuilding(2360)`, `AutoAddtank(2360)`,
`st_fNewEx(_s(5, 2360), EN, 2360, 0, −p, 20)`, `st_fAddShip(2360, 3, 3)`, `(2360, 2, 5)`, `st_RemoveFlottaAzonosito(2360)`,
`st_fNewEx(_s(5, 2361), EN, 2361, 0, −p, 20)`, `st_RemoveFlottaAzonosito(2361)`, `WIN`. `A(2361, 3)` → counter 2361 :=
50; `A(2361, 4)` → counter 2362 := 6; `P(2361)` → `WIN`. `C(2361)`: `M(2362, 0)`, morale +10, `WIN`. `C(2362)`:
`M(2363, 0)`; five times: a weighted pick over planets 1..`HanyBolygo` − 1 with the human's view 0
(`RndInitValasztas`, `RndValasztas(5, 1)`: `rnd(5, 1, n) ≤ 1` for the `n`-th); none → stop; else its view := 1. `WIN`.
- `st_GetUnknownPlanet()` (exe 0x5eaea0): up to 101 draws of `st_bSearch(0, 0)` (aborting without a free visible
  planet) until one has the human's view 0 (**the result is thrown away**), then `st_bGetNearestUnknown()`
  (0x603a60): of the planets with ids 11001..=11015 (`st_bSearch(id, 1)`) the human doesn't know (`+0xd8[EN] < 0`)
  the nearest (f32 `(cy − y)² + (cx − x)²` from `st_GetBirodalomSulypont(EN)`, §8.10, against the last pick's
  truncated value); `st_bUnhide(−p)`; none → abort.
- `AutoAddBuilding(id)` (exe 0x5c1c80): `p = b(id)`; for each `StoryBuildingData` record (exe 0xF22740, 16 bytes
  `{story, planet id, kind, level}`, count 0xF23A00) of `id` whose story is 0 or running:
  `EpuletNewSearchLocation(p, kind, true, level, −1, −1)` and `sum += EpuletekLeiro[kind].+0x10` (`sum` from 3000);
  then `p.+0x28 := sum + rnd(−200, 1300)`. `AutoAddtank(id)` (0x5c1ef0): for each `flotta.txt` record of fleet `−id`
  (story 0 or running): `TankNew(−p, d, counts[Nehezseg − 1][1])`, `d` = `−design` for `design < 1`, else
  `st_GetTerv(p's owner, design, 1)`.

**237:** start: `M(2370, 0)`, counter 2370 := 200 days, `Story_Minden_Flotta_Latszik` (exe 0xF265D4) := 1, game info
2371 (no text). `C(2370)`: the switch := 0, `M(2371, 0)`, `WIN`.
**238:** start: `M(2380, 60)`. `A(2380, 2)`: `StoryAdat+0x38 := 1`, counter 2380 := 720 days. `A(2380, 3)` or `P(2380)`
→ `WIN`. `C(2380)`: `M(2381, 0)`, `+0x38 := 0`, `WIN`.
**239 (a wormhole):** start: no wormhole 1..=`OsszHanyWormhole` (exe 0xBB60F8, 0x1c bytes, count 0xBB61A0) has both
ends shown (`+8` and `+0x14`) → `WIN(0, 0)`; else `p = st_bSearch(−EN, 0)` (none → abort), `v(2391) := 0`,
`M(2390, 80)`, `p` gets id 2390, parameter 1. `A(2390, 2)`: `v(2391) := 1`, `st_fNewEx(_s(5, 2390), EN, 2390, 0, 2,
b(2390))`, `st_fAddShip(2390, 0x1c, 1)`, `st_RemoveFlottaAzonosito(2390)`. `A(2390, 3)` or `P(2390)` → `WIN`. Daily
with `v(2391)` and `st_hSearch(EN, 0x1c) == 0` → `M(2391, 0)`, `WIN`. Special 0x11: `M(2392, 0)`, story 902's wormhole
(§8.5: from fleet `d1` to `b(2390)` + `(int)(r·(cos, sin)(a))`, `r = rnd(20, 28)`, `a = rnd(1, 360)`); when
`AddWormhole` returns `k ≠ 0` both ends are shown (`+8`, `+0x14` := 1) and their fleets (`+0xa`, `+0x16`) always
visible (`+0x52` bit 8); `Wormhole3DSettings()`, `WIN`.

Exe vs ELF: the exe inlines `st_Story`/`st_StoryStart`, `st_Counter`/`st_Message`/`st_Elevul`/`st_Letilt`,
`st_szStart` (with an empty assert in 2301), `StartStory`, `st_RemoveFlottaAzonosito`, `st_AddMoney`,
`st_bAddNewAzonosito`, `st_vGet`, `st_tav`, `st_BolygoMoralValt`, `st_GlobalMoralValt`, `st_SetBolygoTulaj`,
`st_TalalmanyAdhato` and `st_fAddTank`, with the same results. Precision (asm): `st_tav` through f32, 234/235's
parameters f64, the scaling and 235's sum f32. Quirks kept: `st_GetUnknownPlanet` throws its draws away; `best`
survives calls; 230's `StartStory(2301)` before `WIN(0)` ends a running 2301 instead of 230.
