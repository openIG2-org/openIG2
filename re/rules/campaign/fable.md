# Campaign scripting §8.15 `st_Fable_include` (exe 0x5e6ba0, ELF 0x770200): the random fables

Split out of `../campaign.md` (sections, numbering and references unchanged: `campaign.md §8.15` means this file).
Shared terms, the story engine and the notation are in `../campaign.md` §1..§8.8.

Ported in `engine/crates/ig2-sim/src/fable.rs` ([`FableStories`], run after `st_BRnd_include`, before `st_Scen_include`), verified at
runtime (`traces/2026-10-16-fable.md`). Fifteen of the random stories story 400 draws from `RndStoryk` (§8.3):
301..305 the traders (the story merchants of empire 9, fleet ids 10000..=11000), 307 a wormhole map, 308 a spy
network, 309..311 an agent's offer (they start 404, the refit trader), 312..316 small planet episodes; 306 has no
block. Each block runs while its story does; an abort (`throw CStoryWinException`) is caught per block
(`WIN(0, 1)`), then the next block runs. The exe inlines `st_Story`, `st_StoryStart`, `StartStory`, the
variable/counter/planet-id accessors, `st_SpWarPart`, the message predicates, `st_fShow`/`Hide`/`SetVillogas`/
`SetBarat`, `st_fNewPos`, `st_fAttackFleet`, `st_AddFleetToSpWar` and `st_ShowAnim`, and **drops every
`st_oldLetilt` money check** of the ELF (305, 309, 312 disable no answers). Ids are decimal (`v(3011)` =
variable 0xbc3); `c3` = campaign 3, whose messages are the ids + 1000. Notation as in §8.9 (`M`, `A`, `P`, `C`,
`S`), plus `R(b)` = `st_Olvas(b)` (flag 9, `StoryMessageBlokk`); `P_k` = the finished battle's participants
(`SpwFlottak`, `n` of them; owner `+0x18`, fleet slot `+0x20`, story id `+0x24`, name `+0x00`); `own(e)` =
flag 3 and `P_0` or `P_1` (or `P_2` when `n > 2`) is empire `e`'s; `lost(id)` = `st_SpWarLose(id)` (0x602c20:
flag 3, the participant with story id `id` — among the first two when `n < 3`, else all — and `SpWarKiNyert`
0xBAA1EC ≠ its owner); `F[f]` the fleet record; `trader(f)` = story id 10000..=11000. Fleet bits: `+0x18`
bit 0 hidden/has traded, bit 1 leaving; `+0x52` 0x80 blinking, 0x100 always visible, 0x200 friendly, 0x800
custom name.

| Story | What it does |
|---|---|
| 301 | flag 3, `own(EN)`, `own(9)`, `v(3011) == 0`: the first participant `k` with a trader id; then 100 draws `f = st_fSearch(−9)` (stream 5); `f` counts when it is a trader, not `P_k`'s slot, and `d = st_fDistFromBir(f) > 60` (`d` only computed then); the smallest `d` (first of equals) wins. None → `WIN`. Else `v(3011) := 1`, `M(c3 ? 4011 : 3011)`, param 1 = `F[f]`'s name, param 2 = `P_k`'s name, `+0x52 &= ~0x200` (`st_fSetBarat(−f, 0)`), `|= 0x800`, `+0x18 &= ~1`, `+0x16 := 3011`, `AutoAddship(f, 3011)`, counter 3011 := 10, and the chase runs at once. Chase (`C(3011)`, `st_SpWarPart(3011)` or just set up): counter 3011 := 10; `f = st_fSearch(3011)`: none → `WIN`; `g = st_fGetNearest((int) x, (int) y, EN)`: `g` → `FlottaMozgat(f, 2, 2, g, 0)`, else `st_fEliminate(3011)`, `WIN` |
| 302 | start: 304 running → `WIN(0, 0)`. Else up to 100 draws `f = st_fSearch(−9)` until a trader (the 100th draw is not tested: `WIN`); `M(c3 ? 4021 : 3021)`, param 1, `+0x52 |= 0x800`, `v(3021) := +0x16`. `lost(v(3021))`: `st_fSearch(v(3021))` → `FlottaDelete`; `own(EN)` → `M(c3 ? 4022 : 3022)`, else `WIN`. `R(4022)` or `R(3022)`: the human +24,000, `WIN` |
| 303 | start: counter 3031 := 10. `C(3031)`: the nearest (`st_fDistFromBir`, first of equals) fleet of empire 9 with a trader id and `+0x18` bit 0 clear over the fleets 1..=n; none or `d > 79` → counter 3031 := 10; else `v(3031) := +0x16`, `v(3032) := (int) +0x44` (its target), `+0x16 := 3031`, `+0x52 |= 0x200, 0x800, 0x100`, `FlottaMozgat(f, 0, 0, 0, 0)`, `M(3031)`, param 1, counter 3032 := 240. A fleet of the human's arrives at it (flag 0, type 2, target id 3031, `F[ErkezettFlottaNumber]` the human's) or `C(3032)`: `f = st_fSearch(3031)`, `+0x16 := v(3031)` (inlined **without** touching `StoryError`), `+0x52 &= ~0x200, ~0x800, ~0x100`, `FlottaMozgat(f, 1, 1, (f32) v(3032), 0)`; not `C(3032)` → the human +10,000, `M(3033)`; else `M(3032)`; `WIN` (a missing fleet writes slot 0) |
| 304 | start: 305 running → `WIN(0, 0)`. Else 302's draw; `M(c3 ? 4041 : 3041)`, param 1, `+0x52 |= 0x800`, `+0x16 := 3041`. `lost(3041)`: `own(EN)` → `M(c3 ? 4042 : 3042)`, the human +36,000; `WIN` |
| 305 | start: the nearest trader with bit 0 clear (303's search, no distance limit); none → `WIN`; else `M(3051, 100)`, param 1, `v(3051) := +0x16`. `v(3052) == 0` and `lost(v(3051))`: `KiszedUtolsoUzenet(3051, 0)`, `WIN`. `A(3051, 3)`: `v(3052) := 1`, the human −14,000, `StoryAdat+0x28 := st_fSearch(v(3051))`, undo `(14, 0, 305)`, `+0x52 |= 0x800`, counter 3051 := 1440. `A(3051, 4)`: counter 3053 := 60. `A(3051, 5)` or `P(3051)`: `WIN`. `C(3053)`: `M(3051, 100)`, `f = st_fSearch(v(3051))`, none → abort, param 1. `C(3051)`: `t = StoryAdat+0x28`: 0 → `WIN`; `F[t]` bit 0 clear → counter 3051 := 10; else `+0x52 &= ~0x800`, `+0x28 := 0`, `M(3052)`, param 1 = `F[t]`, `WIN`. Daily, `t ≠ 0` and `F[t]` no longer empire 9's: `+0x28 := 0`, `WIN` |
| 307 | start: over the wormholes `w` = 1..=`OsszHanyWormhole` (0xBB61A0; records 0x1c B at 0xBB60F8: `+8` end A known, `+0xa` i16 its fleet, `+0x14` end B known, `+0x16` its fleet) with an end unknown and both fleets > 0, one `RndValasztas(5, 1)` each after `RndInitValasztas` (reset inside the loop: `rnd(5, 1, 1)`, always taken), so the last such `w` wins; none → `WIN`; `v(3071) := w`, `M(3071, 100)`. `A(3071, 4)`: the human −12,000; `w = v(3071)`: both ends known, both fleets `+0x52 |= 0x100`; `WIN`. `A(3071, 3)`, `A(3071, 5)` or `P(3071)`: `WIN` |
| 308 | start: `e = GetBarmilyenBirodalom(1, 3, 5)` (0x5c24e0: `e = rnd(5, 1, 8)`, stepped `e mod 8 + 1` past 1, 3, 5, `EN` and dead empires; the 10th miss aborts); `v(3081) := e`, `M(3081, 100)`, param 1 = `e`'s name. `A(3081, 2)`: the human −2,000; `e = v(3081)`; up to 100 draws `p = st_bSearch(−e, 0)` (stream 5) until four planets: a `p ≠ 0` without info −3081 gets id `3085 − k` (`k` = 4..1: 3081..3084), info −3081 (`BolygoInfoAdd(p, −3081, 1)`) and radar `+0x6e := 4`; counter 3082 := 90. `A(3081, 3)` or `P(3081)`: `WIN`. `C(3082)`: planets 3081..3084 (searched, mode 1) lose info −3081 and the radar (`+0x6e := 0`); scrolling text `_s(5, 3081)` (`ScrollUzenet(…, 1, 0, 0)`); `WIN`. Daily: the four that are no longer `v(3081)`'s lose them |
| 309 | start: `v(3099) ≠ 0` → abort; `v(3099) := 1`. Start or `C(3091)`: `M(3091, 120)`. `A(3091, 2)`: the human −20,000, `S(404)`, `WIN(0, 1)` (a running 404 is `StoryLastChecked` then and ends instead). `A(3091, 3)`: counter 3091 := 90. `A(3091, 4)`: `WIN` |
| 310 | start: `v(3099) ≠ 0` or the human has more than 11 spies (`OsszHanyKem[EN]` 0xBB1F84) → abort; `v(3099) := 1`; `M(3092, 120)`, `v(3101) := 0`. `A(3092, 2)`: `k = Spy_New(EN)` (< 1 → abort), the record below written over it, name `_s(5, 3092)`, `+0x16 := 1`, `+0x3c := rnd(5, KemRangXPHatar[r], KemRangXPHatar[r + 1])` with `r` = `+0x40` (1); `Story_check_locked := 1`, `KemJelentesAdd(2, k, 0, 0, 0, 0, EN, 0…)` (the report, `info-messages.md` §3.3; its `Story_KemEsemeny` sets flag 15 and `KemkedesTipus := 3` for the rest of this check, its `Story_Check` returns at once), unlocked; `S(404)`, counter 3101 := 180, `v(3101) := 1`. `C(3101)`: spy 0 of the human alive (`+0x10 > 0`) with that name → its `+0x10 := 0`; else every spy 1..=n alive, with `+0x16 ≠ 0` and the name. The agent killed (flag 15, `KemkedesTipus == 3`, `KemEsemeny` kind 1 and `b == 1`, spy `a` has the name): `M(3093)`, `st_fEliminate(4041)`, `WIN(0, 1)`, `WIN(404, 1)`. Daily, `v(3101) ≠ 0`, no spy 0..=n alive with the name: the same without the message. `A(3091, 3)` (309's message, a slip kept): `WIN` |
| 311 | start: 309's guard. Start: the human has < 4 planets → `WIN(0, 0)`; else video `_s(6, 3094)`, `M(3094)`, `f = st_fNewEx(_s(5, 4041), 9, 4041, 0.03, 1, 0)` heads for `last_fn_b` (`FlottaMozgat(f, 1, 1, b, 0)`), fleet 4041 `+0x52 |= 0x100, 0x80, 0x200` (each `StoryError` = not found); a pirate `FlottaNew(_s(5, 3111), 10, 3111, x, y, 7.0)` at `x = (f32)(int)(P.x − (P.x − (f32) last_fn_x)·2)`, `y = (f32)(int)((f32)(2·last_fn_y) − P.y)` (f32; `P` = planet `last_fn_b`), `AutoAddship(·, 3111)`; 3111 attacks 4041 (`FlottaMozgat(a, 2, 2, b, 0)` when both exist); `v(3112) := 0`. Flag 1 with 3111 and 4041 the two fleets of `spFlottak` (0xBAA1C6/0xBAA1CC): 4041 missing → abort; every fleet slot `k` = 1..=**`OsszHanyHajo`** (the ship count: a slip kept) of the human's within `(int)(f32) sqrt((f64)(f32)(dy² + dx²)) < 10` of 4041's truncated position joins (`spFlottak` count 0xBAA1E8 below 4: entry `(k, 1)`), `StoryUrharcViszony[9] := 1`, `StoryAdat+0x88 := 1`, `+0x94 := 1`. `st_SpWarPart(3111)`: 3111 gone: 4041 gone → `WIN`; else 4041 `+0x52 &= ~0x80, ~0x100`, `M(3095)`, `v(3112) := 1`. 3111 there, 4041 gone: `st_fEliminate(3111)`, `WIN`. Daily, `v(3112) ≠ 0`, 4041 gone: `KiszedUzenet(3095)`, `WIN`. `A(3095, 2)`: `S(404)`, `WIN(0, 1)`. `A(3095, 3)`: `st_fEliminate(4041)`, the human +13,500, `WIN` |
| 312 | start: `p = st_bSearch(−EN, 0)` gets id 3121 (when non-zero), `M(3120, 100)`, param 1 = `p`. `A(3120 or 3121, 2)`: the human −2,000, info 3120 on planet 3121 (`BolygoInfoAdd(b(3121), 3120, 1)`), `WIN`. `A(3120 or 3121, 3)`: counter 3120 := 70. `A(…, 4)` or `P(…)`: `WIN`. `C(3120)`: `M(3121, 50)`, param 1 = planet 3121 |
| 313 | start: `p = st_bSearch(−EN, 0)` (none → abort) gets id 3130 and info 3130, `M(c3 ? 4130 : 3130)`, param 1, `WIN` |
| 314 | start: as 313 with id 3140 (no info), `M(c3 ? 4140 : 3140)`, param 1, `st_bShow(3140, −1)`, counter 3140 := 30. `C(3140)`: planet 3140 still the human's → `M(c3 ? 4141 : 3141)`, param 1; `WIN` |
| 315 | start: counter 3151 := 10. `C(3151)`: the human's lab levels `LaborSzint[EN][1..=3]` (exe 0x912728, i32 `[19][4]`): the first non-zero one with research going on (`ResearchAktFejlesztett[EN] > 0`) → `M(3150)`, `StoryAdat+0x6c −= 30` (the research speed bonus, `production-research.md`), counter 3150 := 150; else counter 3151 := 10. `C(3150)`: `M(3151)`, `+0x6c += 30`, `WIN` |
| 316 | start: up to 10 draws `p = st_bSearch(−EN, 0)` until a planet with a radar (`+0x6e ≠ 0`); `p == 0` or the 10th draw → `WIN`; else `M(c3 ? 4160 : 3160)`, param 1, counter 3161 := 360, id 3161, `st_bShow(3161, −1)`, `+0x6e := 2`. `C(3161)`: planet 3161 the human's → `M(3161)`, param 1; `+0x6e :=` 1 if it is the human's, else 0; `WIN` |

Helpers:
- **`st_fDistFromBir(f, e)`** (exe 0x604650): `e = −1` → `EN`; over the planets **1..=`HanyBolygo` − 1** (the last
  planet is skipped) owned by `e` that the human sees (`+0xd8[EN] ≥ 0`): `d = (int)(f32) sqrt((f64)(f32)(dx² +
  dy²))` of the truncated coordinates' i32 differences, the smallest; 1,000,000 when there is none.
- **`st_fGetNearest(x, y, e)`** (0x604970): over the fleets 1..=n (`e == 0`: any live one, else owner `e`) the
  first, or one with `(f32)(dy² + dx²) < (f32) best` (f32 from the int point; `best` = the last pick's truncated
  value); 0 when there is none.
- **`st_fEliminate(id)`** (0x604750): `f = id < 0 ? −id : st_fSearch(id)`; `f < 1` → `StoryError := 1`. `+0x18`
  bit 1 set and `+0x40 ≠ 0` (already leaving) → `StoryError := 0`. Else bit 1 set, `a = rnd(5, 0, 1)·600 −
  300`, `b = rnd(5, −300, 300)`, `(dx, dy) = rnd(5, 0, 1) ? (b, a) : (a, b)`; the centre `(cx, cy)` of the
  human's seen planets 1..=n (each running sum `(int)((f32) sum + coord)`, then divided as integers; none → (0,
  0)); `dx` flips its sign when `50 > (f32) cx − x` (f32 `comiss`) differs from `dx ≥ 0`, `dy` likewise;
  `FlottaMozgat(f, 1, 0, dx, dy)`, `StoryError := 0`.
- `st_fSetBarat(x, on)` (0x605160) and the inlined `st_fShow`/`Hide`/`SetVillogas`: `x < 0` → slot `−x`, else
  `st_fSearch(x)`; a slot ≥ 1 gets the bit, `StoryError` = not found.
- `KiszedUtolsoUzenet(id, 0)` (0x506140), `KiszedUzenet` (0x506200), `ScrollUzenet` (0x556b50), `Spy_New`
  (0x58f830) and `KemJelentesAdd` (0x58d000) are events; `AutoAddship` (0x5c1d80, §8.6), `st_bShow` (0x603f20,
  §8.10) are the shared ones.

The story spy's record (0x4c bytes, `Kemek`, `espionage.md`): name `_s(5, 3092)` (the rest of +0x00..+0x0e
zero), `+0x10` i16 1, `+0x12` 1, `+0x14` i16 4, `+0x16` i16 100 (then 1), `+0x18` 0, `+0x1a` i16 4, `+0x1c` 3,
`+0x1e` i16 0, `+0x20` i16 0, `+0x24` 1000, `+0x28` 0, `+0x2c` 0, `+0x30` 0x100, `+0x34` 0, `+0x38` 0, `+0x3c`
the drawn XP, `+0x40` 1 (the rank), `+0x44` 0, `+0x48` 0; the exe leaves `+0x0f`, `+0x13`, `+0x19`, `+0x1d`,
`+0x22..+0x23` as its stack held them (the ELF zeroes them).

Exe vs ELF: no `st_oldLetilt` blocks; 303 restores the fleet id with an inlined read that leaves `StoryError`
alone. Quirks kept: 302/304's 100th draw is never tested; 307's `RndInitValasztas` inside the loop (the last
eligible wormhole, one draw each); 310 ends on 309's message 3091; 311 loops the fleets up to the ship count;
309/311's `S(404)` makes a running 404 the story their `WIN(0, 1)` ends; `st_fDistFromBir` skips the last
planet.
