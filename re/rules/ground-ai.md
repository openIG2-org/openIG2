# Ground battle AI (`WF::MI`, `grwarmi.cpp`)

Status: **verified at runtime** and ported to `engine/crates/ig2-sim/src/ground/ai.rs` (`War::mi`,
`War::ai_forget`): all 10,347 recorded `WF::MI` calls are reproduced exactly (AI blocks, unit orders, groups,
hit markers, air-strike charges, streams 2 and 13, libc), except 379 target picks that read heap garbage in
the exe (§8). Report: `traces/2026-10-16-ground-ai.md`. Background: the battlefield,
the unit record and the auto-resolve are `ground-combat.md`; this is the AI that steers the computer
empires' vehicles on the **real-time** battlefield (colony screen, `StartGrWar`), 20 times a second.

**The exe is normative.** It inlines `CsoportFelallit2`, `PontMeghatarozas`, `VektorNormalizal`,
`WF_Tank::GetMoving` and `WF::TankUsed` into `WF::MI`; the arithmetic below is the exe's (asm checked:
`mulss`/`divss` f32, cos/sin/sqrt via `libm_sse2_*_precise` in f64). Randomness: **stream 13 only**
(`rndlog(13, …)`, `grwarmi.cpp` lines in §7). The helper names come from the ELF; the ported-symbol
table has none of them (found via the call graph of exe 0x486d40 and the `grwarmi.cpp` line numbers).

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Grwar()` | 0x4747c0 | 0x49a7f0 | caller (§1) |
| `WF::MI()` | 0x486d40 | 0x49ad20 | the AI pass (§3–§5) |
| `WF::CsoportbaRendezestKitorol()` | 0x4697a0 | 0x4b7b50 | clear every unit's group (§6.1) |
| `WF::CsoportbaRendez()` | 0x469280 | 0x4b7c00 | assign groups (§6.2), one stream-13 draw |
| `WF::CsoportFelallit(e, g, x, y, fx, fy, b)` | 0x468b30 | 0x4b6b80 | line a group up at a point (§6.3) |
| `WF::CsoportFelallit2(e, g, ex, ey, ax, ay, deg, dist, fx, fy, b)` | inlined | 0x4b6e60 | rotated formation point (§6.4) |
| `PontMeghatarozas(ex, ey, ax, ay, deg, dist, &x, &y)` | inlined | 0x4b6870 | §6.4 |
| `VektorNormalizal(dx, dy, len, &ox, &oy)` | inlined | 0x4b6a20 | Chebyshev step (§5.4) |
| `WF::CsoportTamad(e, g, t)` | 0x468ff0 | 0x4b7290 | group attacks (§6.5) |
| `WF_Tank::GetMITarget(wf)` | 0x47b110 | 0x4b6430 | the AI's target choice (§6.6) |
| `WF::CsoportSerultE(e, g, decay)` | 0x468f20 | 0x4b7440 | "was the group hit?" (§6.7) |
| `WF::CsoportKilotte(e, g, decay)` | 0x468d30 | 0x4b7620 | "who hit the group?" (§6.7) |
| `WF::CsoportMozgat2(e, g)` | 0x468e60 | 0x4b70c0 | back to the home fortress (§6.8) |
| `WF::CsoportVanBenneTurret(e, g)` | 0x4691e0 | 0x4b7830 | §6.8 |
| `WF::CsoportTudjaELoniAzEllenseg(e, g, t, extra, turrets)` | 0x4690b0 | 0x4b7930 | "can enemy `t` reach the group?" (§6.8) |
| `WF::TankUsed(i)` | 0x4a4f20 (also inlined) | 0x463520 | §2 |
| `WF_Tank::GetMoving()` | inlined | 0x4f1a70 | flags & (0x100 \| 0x400) |
| `WF_Tank::MoveG(x, y)` | 0x48bdc0 | 0x47cd30 | move order (§6.9) |
| `WF_Tank::Stop()` | 0x4a4cb0 | 0x47cef0 | §6.9 |
| `WF_Tank::SetTarget(t)` | 0x49c670 | 0x487f60 | `ground-combat.md` §5.2, real-time side effects §6.9 |
| `WF::CheckFighterAttack(e)` / `CheckBigShipAttack(e)` | 0x460770 / 0x45fc90 | 0x4a4950 / 0x4a4a90 | `ground-combat.md` §5.3 |
| `WF::ShotSpecWeapon(x, y, kind, e)` | 0x49cb70 | 0x4ab170 | air strike at a cell (§5.6; effects belong to the real-time battle) |
| `WF::Refresh()` | 0x490890 | 0x48d750 | the battle tick (not this spec) |
| `rndlog(s, lo, hi, file, line)` | 0x542da0 | 0x61e170 | `rng::value` (inclusive `lo..=hi`) |

Not reached from `WF::MI` (they belong to `WF::Refresh` / `WF_Tank::Refresh` or to the player's commands):
`WF_Tank::AttackG` ELF 0x47cfa0, `WF_Tank::Retreat` 0x488ae0, `WF_Tank::StopRetreat` 0x4890b0,
`KetSzogAltalBezartSzog` 0x483ca0, `WF::AddMine` 0x487990 (all called by `WF_Tank::Refresh`/`WF::Refresh`),
`WF::DetonateSelected` 0x4abe50, `WF::SwitchSelectedMineLayers` 0x4abd00 (UI), `WF::CsoportMozgat` 0x4b6f10
(unused by MI). The AI never lays or detonates mines itself.

## 1. When it runs (`FRISSIT_Grwar`, exe 0x4747c0, every 20 Hz tick)
```
FRISSIT_Grwar:
    counter 0x9B2900 += 1; per planet: WF+0x334 countdowns (×IdoSpeedSzorzoBattle), every 300th call 0x460d70
    MP (0xBAE2C0):  if 0xBAD13C ≠ 0:  WF(0xBAD140).MI();  WF::Refresh
    SP:  AktKep 0xBAE0FC ≠ 3 (not the colony screen) → return
         p = AktBolygo 0xB9933C
         planet p +0x64 (ground war) ≠ 0 → WF(p).MI()
         else → WF(p)+0x33c + 0x40·e := 0 for e = 1..=18      (every AI forgets its strategy)
         p ≠ 0xF261F8 → WF(p).Refresh()
```
So in SP the AI runs **once per tick** (not once per battle sub-step) and only for the battle on screen,
**before** that tick's `WF::Refresh`. Timers inside MI scale with `IdoSpeedSzorzoBattle` (exe 0xBAE260).

## 2. Data
`WF(p)` = exe `0x9C3244 + p·0x8EC` (`ground-combat.md` §1.1); units `WF+0x330`, alive `WF+0x188`, count
`WF+0x180`, planet `WF+0x18c`.

**Used unit** (`TankUsed(i)`): `alive[i]` and not flag 2 and not flag 0x40000000 and not (flags2 & 4).
Filters below: *field unit* = used and not a mine (flag 0x20000); *mobile* = field unit and not a turret
(flag 0x4000000). "Group `g` of `e`" = units with owner `+0x194 == e` and AI group `+0x248 == g`.

### 2.1 Per-empire AI state (`WF + 0x40·e`, e = 1..=18)
| exe | ELF | Type | Field |
|---|---|---|---|
| +0x33c | +0x468 | i32 | strategy 0 (none yet) / 1..=5 attacker plans / 6 defend the fortresses |
| +0x340 | +0x46c | i32 | mode: 1, or 3 = "the planet owner has no mobile units" (skip the waiting phase) |
| +0x344 | +0x470 | i32 | phase 1 approach / 2 form up / 3 attack |
| +0x348 / +0x34c | +0x474 / +0x478 | f32 | rally point A (x, y) |
| +0x350 / +0x354 | +0x47c / +0x480 | f32 | rally point B (x, y; strategy 2's group 3) |
| +0x358 / +0x35c | +0x484 / +0x488 | f32 | enemy centroid at the start (written, never read) |
| +0x360 | +0x48c | i32 | timer (−1 idle) |
| +0x364 | +0x490 | i32 | wait counter (strategies 4/5) |
| +0x368 + 4g | +0x494 + 4g | i32 `[5]` | target per group (10000 = none; strategy 6, g = 1..=4) |

Other WF fields: `+0x7f4 + 8e` / `+0x7f8 + 8e` (ELF `+0x928 + 8(e−1)` / `+0x92c + …`) air-strike charge
(fighters / big ships): `WF::Refresh` adds `IdoSpeedSzorzoBattle` per tick, capped at 200 / 300;
`ShotSpecWeapon` zeroes it. `+0x8d8 + e` empire `e` has radar on the field.

### 2.2 Unit fields MI reads or writes (exe offsets in the 0x290-byte unit; ELF = exe + 0x18 from +0x1d0 up)
| exe | ELF | Type | Use here |
|---|---|---|---|
| +0x148 | +0x148 | f32 | fire range (`CheckDesign`); GetMITarget, TudjaELoni |
| +0x14c | | f32 | aim distance (SetTarget on a building: range + footprint/2) |
| +0xf8 / +0xfc / +0x100 | | f32 / i32 / i32 | SetTarget on a new target: 300.0 / 0 / −1 [real-time aim state] |
| +0x154 | +0x158 | i32* | member hp (GetMITarget reads `hp[my_size − 1]` of the **target**, §6.6) |
| +0x188 | +0x190 | u32 | flags (2, 0x100 has target, 0x400 moving, 0x800, 0x80, 0x20000 mine, 0x4000000 turret, 0x8000 partner, 0x20000000 keep orders, 0x40000000) |
| +0x18c | +0x194 | u32 | flags2 (bit 4 excluded) |
| +0x190 | +0x198 | i32 | size |
| +0x194 / +0x198 | +0x19c / +0x1a0 | i32 | owner / side |
| +0x1a0 / +0x1a4 | +0x1b0 / +0x1b4 | i32 | grid cell x, y |
| +0x1b0 / +0x1b4 | +0x1c0 / +0x1c4 | i32 | move destination x, y |
| +0x1b8 / +0x1bc | +0x1c8 / +0x1cc | i32 | movement state (path step counter; 499 = blocked) / path index |
| +0x1d0 | +0x1e0 | u32 | weapons (group assignment by kind, §6.2; air strikes 0x20 / 8) |
| +0x1e8 | +0x1f8 | i32 | paralysis countdown (SetTarget zeroes a seized ally's) |
| +0x238 | +0x250 | i32 | building slot (turrets) |
| +0x23c | +0x254 | i32 | target (10000 = none) |
| +0x244 | +0x25c | i32 | own index |
| **+0x248** | +0x260 | i32 | **AI group** 0..=4 (0 = unassigned / main body) |
| **+0x24c** | +0x264 | i32 | **home**: −slot of the fortress its group guards (strategy 6) |
| **+0x25c** | +0x274 | i32 | **hit marker**: `WF_Tank::Damage` sets 10000 on a hit; MI decays it (§6.7) |
| **+0x260** | +0x278 | i32 | **last attacker** (10000 = none) |
| +0x278 | | u32 | transporter state bits (MoveG refuses with bit 8) |
| +0x280 | +0x298 | i32 | planet |
| +0x288 | | i32 | air-strike units left |
| +0x28c | +0x2a4 | u8 | mine revealed (GetMITarget may target a revealed mine) |

Buildings (`Epuletek[p]` exe 0xBA9E90, count 0xBA9840, 0x20 B, slot 1-based, `colony-economy.md` §2.2):
type `+0`, construction work left `+8` (u16, 0 = finished), map position `+0xc/+0x10` f32, flags `+0x19`
(bit 0 working, bit 5 switched off). **Fortresses** = types 0x20..=0x23.

## 3. `WF::MI()`: census (every tick)
Over all field units in slot order, for owner `o = +0x194`, group `g = +0x248`, cell `(cx, cy)`:
```
own[o][0].n += 1;  turret? no → own[o][0].mobile += 1;  own[o][0].sx += (f32)cx;  own[o][0].sy += (f32)cy
for e in 1..=18, e ≠ o:  enemy[e][0] gets the same (n, mobile, sx, sy)
g > 0: the same for own[o][g] and enemy[e][g]
```
Sums are f32 in slot order; then each bucket with `n > 0` divides `sx /= (f32)n`, `sy /= (f32)n` (f32).
Only `own[e][0]`, `enemy[e][0]` and `own[e][3].n` are read later. "Enemy" = every unit not owned by `e`
(allies included).

## 4. Per empire e = 1..=18, in order
```
if PlayerKi[e] == 1 (human, 0xBAE560) or own[e][0].n < 1 or enemy[e][0].n < 1:
    strategy(e) := 0; next e                              (no air strike either)
init = false
if strategy(e) == 0:  INIT (§4.1); init = true
moving = [false; 5]                                       (§4.2)
timer(e) > 0 → timer -= IdoSpeedSzorzoBattle, ≥ 0
init == false → DECIDE (§5.1–§5.5)
ACT (§5.6), AIR STRIKE (§5.7)
```

### 4.1 INIT (stream 13)
```
mode := 1
if planet owner (Bolygok[p]+0x20, exe 0xB85EC8 + 0x188·p) == e:
    strategy := 6
    if enemy[e][0].mobile < own[e][0].mobile  or  rndlog(13, 1, 3) == 1 [line 0x297]:
        strategy := rndlog(13, 1, 5) [line 0x297]         (2 stays 2 here)
else:
    strategy := rndlog(13, 1, 5) [line 0x29b];  2 → 3
    own[owner][0].mobile < 1 → strategy := 3, mode := 3
phase := 1;  timer := −1;  counter := 0
A := B := (own[e][0].x, own[e][0].y);  start enemy := (enemy[e][0].x, enemy[e][0].y);  targets[0..5] := 10000
CsoportbaRendezestKitorol();  CsoportbaRendez()           (§6.1–§6.2: regroups EVERY AI empire's units)
```
The `||` short-circuits: the 1..3 draw happens only when the owner is not outnumbering the enemy's mobile
units. `CsoportbaRendez` may turn the owner's strategy 6 into 1 (no fortress).

### 4.2 Group movement flags
For each mobile unit of `e` (field unit, no turret, no 0x40000000) that is moving (`flags & 0x500`):
if `(int)(f32)sqrt((f64)(f32)(dx² + dy²)) > 0` and movement state `+0x1b8 < 2`, with `dx, dy` = destination −
cell **squared in wrapping i32** and `(int)` = `cvttss2si` (NaN or out of range → `INT_MIN`): a unit whose
destination was never set holds uninitialised memory there (seen: 1781186419), the sum wraps negative,
sqrt is NaN and the unit does **not** count as moving:
`moving[0] = true`, and `moving[g] = true` if its group `g > 0`.

## 5. DECIDE and ACT
`d = (Ex − A.x)² + (Ey − A.y)²` in f32 (`E = enemy[e][0]` this tick); thresholds are f32 squares.
`hit(g)` = `CsoportSerultE(e, g, 10000) ≠ 0` (side effect: clears the group's markers, §6.7).
`step`, `stepB` start at 0 each tick.

### 5.1 Strategies 1 and 3
Block P runs for strategies 1 **and** 3; block Q runs after it for strategy 3 only (exe and ELF alike).
```
P: phase 1 and not moving[0]:
       d ≤ 225 → phase := 2, timer := 140; mode 3 → phase := 3  (else the phase-2 rule runs now)
       d > 225 → step := 3; d > 625 → 6; d > 1225 → 10
   phase 2 (incl. just entered):  d > 256 → phase := 1;  mode 1 and timer < 1 → phase := 3
   d ≤ 121 → phase := 3
   hit(1) → 3;  hit(2) → 3                                   (both always evaluated)
Q: phase ∈ {1, 2} and 2·enemy[e][0].mobile < own[e][0].mobile → phase := 3   (overwhelming)
   else phase 1 and not moving[0]:  d ≤ 225 → as in P without the mode rule (unreachable after P)
                                    d > 225 → step := 5; d > 625 → 7; d > 1225 → 10   (overrides P's)
   else phase 2: the phase-2 rule of P
   d ≤ 121 → phase := 3;  hit(1), hit(2) again (markers were just cleared: no effect)
```

### 5.2 Strategy 2 (flanks + a reserve at B)
```
phase 1:
    not moving[0]:  d ≤ 484 → phase := 2
                    else step := stepB := 3 (d > 900: 6, d > 1600: 10), timer := 0
    hit(1) or hit(2) (short-circuit) → phase := 3
phase ∈ {2, 3}:
    not moving[3]:
        dB = (Ex − B.x)² + (Ey − B.y)²;  h = CsoportSerultE(e, 3, 100)
        dB > 144 and h == 0 → stepB := 1
        dB < 81 or h == 10000 → stepB := −3                  (a fresh hit: fall back)
    phase 2:  phase := d ≤ 81 ? 3 : 2;  own[e][3].n == 0 → 3;  hit(1) or hit(2) (short-circuit) → 3
```

### 5.3 Strategies 4 and 5 (pincer)
```
phase 1 and not moving[0]:  d ≤ 225 → phase := 2, counter := 0  (then the phase-2 rule below runs now)
                            d > 225 → step := 3 (> 625: 6, > 1225: 10)
phase 2 and not moving[0]:  counter += 1; counter > 10 → phase := 3
d ≤ 64 → phase := 3
hit(1), hit(2), hit(3), hit(4): each → phase := 3          (all four always evaluated)
```

### 5.4 Strategy 6 (defender): `for g in 1..=4: t = CsoportKilotte(e, g, 10000); t ≠ 10000 and targets[g] == 10000 → targets[g] := t`.
Then for every strategy, the rally points advance (`VektorNormalizal` inlined, f32, Chebyshev scaling):
```
step ≠ 0:  timer == −1 → timer := 0
           timer == 0  → A += advance(E − A, step); timer := −1     (so A moves every other tick)
stepB ≠ 0: B += advance(E − B, stepB)                              (every tick)
advance((dx, dy), s): if |dx| + |dy| < 0.1f → dx := 1.0;  m = max(|dx|, |dy|)
                      → ((dx·(f32)s)/m, (dy·(f32)s)/m)             (product first, then the division)
phase(e) := phase
```

### 5.5 Precision
All census sums, centroids, `d`, `dB`, the advance and the rally points are f32 (`mulss`, `addss`, `divss`).
`|x|` is taken in f64 (`andpd`) and narrowed back: exact.

### 5.6 ACT (also on the INIT tick, with phase 1)
`F(g, P, deg, dist)` = `CsoportFelallit(e, g, X, Y, Ex, Ey)` at the point `dist` from `P` towards `E`, rotated
by `deg` (§6.4). `T(g)` = `CsoportTamad(e, g, 10000)`.

| strategy | phase 1 | phase 2 | phase 3 |
|---|---|---|---|
| 1 | F(1, A, 90, 10), F(2, A, −90, 10) | same | T(0), T(1), T(2) |
| 2 | F(1, A, 90, 10), F(2, A, −90, 10), F(3, B, 180, 0) | same | T(0..=3) |
| 3 | F(1, A, 180, 3), F(2, A, 180, 0) | same | T(0), T(1), T(2) |
| 4 | F(1, A, 90, 10), F(2, A, −90, 10), F(3, A, 80, 18), F(4, A, −80, 18) | F(1), F(2) as phase 1; F(3, A, 80 − 5c, 18 + c), F(4, A, 5c − 80, 18 + c) with `c = counter` | T(0..=4) |
| 5 | F(1, A, 180, 3), F(2, A, 180, 0), F(3, A, 80, 18), F(4, A, −80, 18) | F(1), F(2) as phase 1; F(3)/F(4) as strategy 4 phase 2 | T(0..=4) |
| 6 | see below | | |

Group 0 of strategies 1..=5 gets no order until phase 3 (it keeps what it was doing).
Strategy 6, every tick, for `g in 1..=4`:
```
if not CsoportVanBenneTurret(e, g):  CsoportTamad(e, g, 10000)          (its fortress has no turret left)
else:
    t = targets[g]
    t ≥ 1 and t ≠ 10000 and not TankUsed(t) → t := 10000
    t ≥ 1 and t ≠ 10000 and not CsoportTudjaELoniAzEllenseg(e, g, t, 2, true) → t := 10000
    t == 10000 → CsoportMozgat2(e, g)  else CsoportTamad(e, g, t)     (t = 0 is attacked unchecked)
    targets[g] := t
CsoportTamad(e, 0, 10000)
```

### 5.7 Air strike (every tick, after ACT)
```
kind = −1
charge_f(e) == 200 and CheckFighterAttack(e) → kind := 0
charge_b(e) == 300 and CheckBigShipAttack(e) → kind := 1
kind < 0 → done
R = [8.0, 5.0][kind];  best = −1, (bx, by) = (0, 0)
for x in 0..200, for y in 0..200:                          (40,000 cells, x outer)
    s = 0
    for every field unit u in slot order:
        r = (f32)sqrt((f64)(f32)((u.cx − x)² + (u.cy − y)²))
        r ≤ R:  u.owner == e → s = (int)((f32)s − (R − r)·3.0f)
                else         → s = (int)((R − r) + (f32)s)
    s > best → best = s, (bx, by) = (x, y)                  (first maximum wins)
best > 0 → ShotSpecWeapon(bx, by, kind, e)
```
`ShotSpecWeapon` (exe 0x49cb70) zeroes the charge, takes one air unit (`+0x288 −= 1`) from the first unit of
`e` carrying the strike (alive, the weapon bit, `air > 0`; no other filter), then moves the cell by two
`rndlog(2, −4, 4)` (x then y; lines 0x3135/0x3136 or 0x3155/0x3156): `(X, Y)`.

Big ships (20 bombs):
```
fx = (f32)X·0.5f/99f·100f·W/99f;  fy likewise with Y and H        (W, H = map size 0x8EC960/0x8EC964, f32)
A = (f32)−((f64)fx − (f64)W·0.5);  B = (f32)((f64)fy − (f64)H·0.5)
gx = (f32)((f64)X·0.25), gy likewise;  z = gx > 200 or gy > 200 ? 0 : GetHeight(gx, gy)  (0x47ad60, f32)
20 times:  d6 = (int)((f32)rndlog(2,0,399) + B − 200);  d5 = (int)((f32)rndlog(2,0,399) + A − 200)
           s3 = (f32)rndlog(2,0,1999) + B − 1000;        s2 = (f32)rndlog(2,0,1999) + A − 1000   (line 0x3140)
           bomb(pos (z − 3000, s2, s3), dest ((int)(z + 300), d5, d6), life 300, dmg 10, +0x34 = (int)z)
```
The bomb (exe 0x457ad0): a list-B object of kind 1 in the first free slot, velocity = normalised(dest − pos)
(`0x4501f0`: f32 sum, f64 sqrt, unchanged below 1e-5) × `(f32)(((f64)rndlog(2,0,14)/10 + 0.5)·3)·20f` (line
0x27b5), shooter 10000, `+0x3c` uninitialised heap; on screen one libc `rand()` (its texture). The bombs then
fly and burst in `WF::Refresh` (`ground-combat.md` §11.10, list B kind 1).
Fighters: `WF::AddFighters` (0x45b8d0) only when no wave is up (`WF+0x32c`, which it sets): five fighters,
`rndlog(2, 0, 4)` each (line 0x2991). The fighter objects (`WF+0x1f4 + 0x40k`, their lasers and the bombs
they drop) are not ported (`ground-combat.md` §11.11); the port reports `GroundEvent::AirStrike`.

## 6. Helpers (exe)
### 6.1 `CsoportbaRendezestKitorol()`: every used unit: group := 0, hit marker := 0, attacker := 10000.

### 6.2 `CsoportbaRendez()`
```
forts = []  (≤ 4): planet buildings slot 1..=count with type 0x20..=0x23 and +8 == 0, in slot order
best = index (1-based in forts) of the first fort with the highest type, 0 if none
coin = rndlog(13, 0, 1) [line 0x1bc] ≠ 0 and best ≥ 1               (drawn on every call)
forts empty and strategy(owner of p) == 6 → strategy(owner) := 1
cnt[e][0..8] = 0, size[e][0..5] = 0
pass 1: every used unit, not mine/turret/partner (0x4028000), group == 0, PlayerKi[owner] ≠ 1, in slot order:
    k = kind by weapons: 0x10 → 6; 0x2001 → 1; 0xf02 → 2; 4 → 3; 0x1000 → 4; 0xc0 → 5; else 7 (first match)
    cnt[o][k] += 1; cnt[o][0] += 1
    g = by strategy(o):
        1, 3: k == 1 → 1;  k == 2 → 2;  else 0
        2:    cnt[o][0] == 1 → 3;  else size[o][2] ≤ size[o][1] ? 2 : 1
        4, 5: the first smallest of size[o][1..=4]
        6:    the first smallest of size[o][1..=n] (n = forts.len(); 1 if n < 2); coin → best
              home := −forts[g]
        else: 0
    group := g;  size[o][g] += 1
pass 2: every used turret with group 0, PlayerKi[owner] ≠ 1 and strategy(owner) == 6:
    g = the last i in 1..=4 with forts[i] == building slot (+0x238), else 0
    group := g;  home := −forts[g];  size[o][g] += 1
```
`forts[0]` is never written (uninitialised stack in the exe): a turret matching no fort gets `home =
−garbage`; nothing reads a group-0 home. The port stores 0.

### 6.3 `CsoportFelallit(e, g, x, y, fx, fy)` (exe 0x468b30; the 7th argument is ignored)
```
dy = fy − y;  nx = −(fx − x)
|dy| + |nx| < 0.1f → dy := 1.0
m = max(|nx|, |dy|);  sx = (dy·2.0f)/m;  sy = (nx·2.0f)/m
n = 0; for every mobile unit of group g of e (slot order):
    n += 1;  k = (f32)(n / 2)
    n even: X = k·sx + x;  Y = k·sy + y      n odd: X = x − k·sx;  Y = y − k·sy
    |cy − (int)Y| + |cx − (int)X| < 5 and +0x1b8 > 9 → Stop()  else MoveG((int)X, (int)Y)
```
The group lines up across the direction to the facing point, 2 cells apart, alternating sides
(1st at the point, 2nd +1, 3rd −1, 4th +2, …). `(int)` truncates.

### 6.4 The formation point (inlined `CsoportFelallit2` + `PontMeghatarozas`)
```
dx = Ex − P.x;  dy = Ey − P.y;  len = (f32)sqrt((f64)(dx·dx + dy·dy))     (f32 sum)
(f64)len < 0.05 → dx = 1, dy = 0, len = 1
s = dist / len  (f32);  dx' = dx·s;  dy' = dy·s
c = (f32)cos(θ), sn = (f32)sin(θ)  with θ = deg·π/180 in f64
X = (f32)(int)((c·dx' + P.x) − sn·dy');   Y = (f32)(int)((sn·dx' + P.y) + c·dy')
```
The exe folds the constant angles (±π/2, π, ±1.3962634015954636 = 80°); the variable ones are
`((f64)((16 − c)·5)·π)/180.0` and `((f64)((c − 16)·5)·π)/180.0`. `dist` is f32 (`(f32)(c + 18)`). The facing
point is always `E`.

### 6.5 `CsoportTamad(e, g, t)`: every mobile unit of group `g` of `e`:
`t == 10000 → SetTarget(GetMITarget())`; else `TankUsed(t)` and `owner(t) ≠ e` → `SetTarget(t)`.

### 6.6 `WF_Tank::GetMITarget(wf)` (exe 0x47b110), unit `u` (owner `o`, size `n`)
```
best = 1e10f, pick = 10000
for i in slot order, TankUsed(i), owner(i) ≠ o, n ≥ 1,
    and (not a mine, or mine revealed +0x28c, or WF radar[o] +0x8d8+o):
    h = i.hp[n − 1]          (quirk: the TARGET's hp array indexed with the CHOOSER's size: out of bounds
                              when the target is smaller; the port reads hp.get(n−1) and must match the
                              trace, see §8)
    r = (f32)sqrt((f64)((f32)(u.cx − i.cx)² + (f32)(u.cy − i.cy)²));  r' = max(r, range)  (r ≤ range → range)
    v = (f32)((h / n) / 500) + r'                       (integer divisions)
    i == u.target:  (u.flags & ~0x100) ≠ 0 → v ·= 0.5f;  u.+0x1b8 == 499 → v /= 1000.0f
    i is a turret and its building (Epuletek[i.planet] slot i.+0x238) has +0x19 bit0 clear or bit5 set → v ·= 1000.0f
    r ≤ range → v /= 3.0f
    v < best → best = v, pick = i
return pick
```

### 6.7 Hit markers
`CsoportSerultE(e, g, k)`: over the field units (turrets included) of group `g` of `e`: result = max of the
positive markers; then each marker `−= k` (≥ 0) and attacker := 10000. `CsoportKilotte(e, g, k)`: the same
walk; a unit with marker == 10000 (hit since the last walk) and attacker ≠ 10000, `TankUsed(attacker)` and
`owner(attacker) ≠ e` sets the result (last one wins; else 10000); then marker `−= k` (≥ 0), attacker := 10000.

### 6.8 Fortress groups
`CsoportVanBenneTurret(e, g)`: some unit (`alive`, not flag 2 / 0x40000000 / flags2 4; mines count) of `e`,
group `g`, with the turret flag. `CsoportTudjaELoniAzEllenseg(e, g, t, extra, turrets)`: `t ≠ 10000` and some
field unit of group `g` of `e` (turrets only if `turrets`) with `(f32)sqrt((f64)(dx² + dy²)) ≤ (f32)((int)t.range
+ extra)` (`dx, dy` = cell differences to `t`, squared in f32). `CsoportMozgat2(e, g)`: every mobile unit of the
group: `home ≤ 0` → `MoveG((int)b.x, (int)b.y)` of building slot `−home` (f32 map position `+0xc/+0x10`),
else `MoveG` to unit `home`'s cell.

### 6.9 Orders (real-time unit state)
`MoveG(x, y)`: refused when flags & (0x4020000 | 0x28000000) or transport bits & 8 or `x, y ∉ 0..200`
(unsigned compare); clamp to `10..=190`; a new destination → `+0x1b8 := 0`, `+0x1bc := −1`; dest := (x, y);
target := 10000; flags := flags & ~0x900 | 0x400.
`Stop()`: unless flag 0x20000000: `+0x1b8 := 0`, flags &= ~0xd80, dest := cell, `+0x1bc := −1`, target := 10000.
`SetTarget(t)` (beyond `ground-combat.md` §5.2): a changed target also sets `+0xf8 := 300.0f`, `+0x100 := −1`,
`+0xfc := 0`; a building target (`t < 0`) sets `+0x14c := (f32)(i32 at 0x8F1F14 + 0x6c·type / 2) + range`
(the footprint table of `colony-placement.md`, integer halving).

## 7. Stream 13 draws, in order
Per tick, per empire `e` = 1..=18 that initialises (strategy 0, alive on both sides):
1. owner of the planet: `rndlog(13, 1, 3)` line 0x297 (only if not outnumbering), then `rndlog(13, 1, 5)`
   line 0x297 (if that was 1, or always when outnumbering); others: `rndlog(13, 1, 5)` line 0x29b;
2. `CsoportbaRendez`: `rndlog(13, 0, 1)` line 0x1bc.

Nothing else in MI draws stream 13; `ShotSpecWeapon` draws stream 2 (§5.7). Since `FRISSIT_Grwar` zeroes all
strategies whenever the planet's ground war is over and an AI's strategy resets when it has no units or no
enemies, new landings in a running battle re-roll that empire's plan and regroup everyone.

## 8. Original quirks kept
- `GetMITarget` reads the target's hp with the chooser's size: the hp arrays are `realloc`ed to exactly
  `size` ints (`WF_Tank::IncGroupSize`), so a smaller target contributes heap garbage. Confirmed in the trace:
  379 calls (one fuzzed battle with 3-vehicle groups against single turrets) pick a farther target than the
  in-bounds formula allows. **Original nondeterminism**: the port reads 0 past the end.
- A never-set move destination is uninitialised memory; the "moving" test wraps it (§4.2).
- Every `CsoportbaRendez` call regroups **all** AI empires' units with their current strategies, so an empire
  initialising later in the same tick reshuffles the earlier ones' groups (their rally state stays).
- A group-0 turret's `home` is uninitialised stack.
- The owner's random strategy can be 2 (the remap 2 → 3 is only for attackers).
- Strategy 2 short-circuits `hit(1) || hit(2)`: group 2's markers are not cleared that tick when group 1 was hit.

## 9. Verification (`traces/2026-10-16-ground-ai.md`)
The real-time tracer `tools/trace/run_ground_rt.py --mode battle` records the game's own `WF::MI` calls (`"mi"`
events: region diffs of the battlefield, every unit with its hp/origin/waypoints, the planet and buildings,
streams, a pinned libc pair); `--ai-fuzz [--ai-fuzz-every N]` perturbs the AI blocks (strategy 0..=6, phase,
mode, timer, counter), hit markers, air-strike charges and carriers before some calls, so every plan, phase
and both air strikes occur. `tests/ground_rt_trace.rs::ground_ai_matches_trace` rebuilds the state before
each call, runs `War::mi` and compares the AI blocks, every unit, the charges, the fighter flag, streams 2
and 13 and the libc draws.
