# Espionage: spies, missions, recruitment

Status: **verified at runtime** (`traces/2026-10-15-espionage.md`): 1,393 daily calls (plain, fuzzed, with warships)
with 1,382 mission results, 346 catch checks and 78 recruits reproduced byte for byte by `engine/crates/ig2-sim`
(`spy.rs`, `tests/spy_trace.rs`).
One daily system driven by the sim tick (`Fake_FRISSITES` exe 0x4de720, on a day change):

```
FRISSIT_Planet (Nap % 6 == 1) → FRISSIT_Viszony → FRISSIT_MI → FRISSIT_Spy → MI_DIPLOMACY
```

Missions are *started* elsewhere: by the human in the spy screen (`Spy_Main`, `MULTIGET_Spy` ELF 0x6d08e0,
through the lockstep queue) and by the AI in `FRISSIT_MI_KEMKEDES` (ELF 0x5af7e0, part of `FRISSIT_MI`).
`FRISSIT_Spy` runs them: experience and ranks, the catch check halfway through a mission, the mission's
effect when its time is up, and the daily chance of a new recruit. All its randomness is **stream 4**
(`rndlog(4, …)` and `RndValasztas(4, …)`); it makes **no libc `rand()` call** (checked: no call to the
`rand` import 0x8353ac in 0x584b30..0x586b00 or in `Spy_New`). Nothing here needs ADR 0003's rerouting.

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Spy` | **0x584b30** | 0x6cc800 | the daily pass (§3–§7) |
| `DontottAKemSorsarol(t, fate, e, k)` | 0x584350 | 0x6d0170 | the fate of a caught spy (§4.2) |
| `Spy_New(e)` | 0x58f830 | 0x6cbe10 | a new recruit (§6.1) |
| `Spy_Copy(e, from, k)` | 0x58d170 | 0x6cc600 | move a record into `e`'s roster (§6.2) |
| `Spy_Beszolas(…)` | 0x58d000 (partly inlined) | 0x6cba60 | the spy screen's message queue (UI) |
| `st_eSearch2spy(s, owner, p, tech, &p, &slot)` | 0x6040a0 | 0x7323c0 | a random finished building (§5.6) |
| `StoryEredmenyKemSikeres` / `…KemElkapasa` | 0x5c4940 / 0x5c48c0 | 0x73ee10 / 0x73ecd0 | story missions: `Story_Check(1)` decides (§2.3) |
| `SpyEsemenyEllenoriz` | inlined | 0x6c5d30 | spy screen: show the next queued message (UI) |
| `getFellazadhat(p)` / `getMiAltalElfoglalhato(p)` | 0x4efe50 / 0x4efe70 | | `owner < 9 && +0x186 == 0` / `owner < 9 && +0x187 == 0` |
| `BolygoFellazad(p, to, force)` | 0x53bc40 | | rebellion (`colony-side.md` §3; streams 1 and 8) |
| `TalalmanyKifejlesztve(e, t)` | 0x524e50 | 0x5ebcb0 | tech developed (`production-research.md` §2.1) |
| `EpuletDelete(p, slot)` | 0x53c730 | 0x61cb00 | building removed (§5.6) |
| `HajoDelete(i)` | 0x53e5d0 | 0x616cc0 | ship removed (`space-combat.md` §3) |
| `MoneyMennyi` / `MoneyHozzaadLog` | 0x540170 / 0x53fb10 | | money (`colony-side.md` §6) |
| `FelfedezBirodalom(e, msg)` | 0x4c1d60 | 0x50f090 | the human meets `e` (contact list) |
| `RndInitValasztas` / `RndValasztas` | 0x540ef0 / 0x540f10 | | weighted choice (`time-and-rng.md` §3) |

**The exe is normative.** It inlines `Spy_Beszolas` and `SpyEsemenyEllenoriz` and loses some call arguments
in the decompile (`Spy_Copy`, `BolygoFellazad`: same arguments as the ELF, checked in the asm). Arithmetic,
draw order and the short-circuit draws match the ELF case by case (§3–§7). Floats: the monthly experience and
the race/difficulty ×1.5 are **f64** (`mulsd` at 0x584d5f, 0x585219); the mission success chance is **f32**
throughout (`divss`/`mulss` at 0x585922..0x585943). No rule values here: the tables (§1.2) are read from the
user's exe (ADR 0004).

## 1. Data
### 1.1 State (exe VAs)
| Name | exe VA | Layout |
|---|---|---|
| `Kemek` | 0xBAFCC8 | `[9][13]` spy records of 0x4c bytes (`e·0x3dc + k·0x4c`); slot 0 is a spare (§6.2), slots 1..=12 the roster |
| `OsszHanyKem` | 0xBB1F84 | i32 `[9]`: roster length (≤ 12; slots past it are unused) |
| `HanyKemVan` | 0xBB23D0 | i32 `[9]`: working spies (§7; the exe's loop writes 0xBB23D4 + 4(e − 1)) |
| `KemKuldetesek[11].+0` | 0x9130A8 | the kidnap mission's "enabled" byte, cleared every day (§3) |
| `CsaladtagElrabolvaKi` / `…Kialtal` | 0xBB405C / 0xBB4080 | i32 `[9]` each: kidnapped family member 1..=5 / by whom (§5.11) |
| `Viszony[t][e].v0` | 0xDABF38 | i16 spy grievance of `t` against `e` (`diplomacy.md` §2): +500 / +2000 here |
| `BirodalomIsmeri[t][e]` | 0xBAEBB8 | `t` has met `e` (a caught spy reveals its empire) |
| `AktKem`, `KemBeszolasok` (0xBAF334..) | 0xBB2384 / … | spy-screen cursor and message queue (UI, not modelled) |
| `DAT_00B77250` | 0xB77250 | SP debug override: 1 = every mission succeeds, 2 = every spy is caught, 3 = a recruit every day |

Spy record (0x4c bytes; fields not listed are copied with the record and otherwise untouched):

| Off | Type | Meaning |
|---|---|---|
| +0x00 | char[16] | name |
| +0x10 | i16 | state: 0 none/dead, 1 at home, 2 travelling home, 3 on a mission, 4 captured (waits for the captor) |
| +0x12 | u8 | race (template index 1..=14) |
| +0x16 | i16 | applicant: offered to the human, not hired yet (not counted, not working) |
| +0x18 | u8 | double agent: the empire it secretly works for (0 = none) |
| +0x1a | u16 | mission (`KemKuldetesek` index; 0 = none) |
| +0x1c | u8 | target empire |
| +0x1e / +0x20 | i16 | mission parameters (planet, tech, training level; per mission) |
| +0x24 | i32 | mission length (set again when a spotted spy is sent home, §4.1) |
| +0x28 | i32 | time left (−50 a day) |
| +0x2c | i32 | catch moment (the check runs when `left − 50 < catch < left`; −1000 afterwards) |
| +0x31..+0x34 | u8 | skills 1..=4: infiltration (+0x31), counter-intelligence (+0x32), combat (+0x33), loyalty (+0x34) |
| +0x38 | i32 | skill points to distribute (+`KemSzintLepesSzetoszthato[race]` per rank) |
| +0x3c | i32 | experience |
| +0x40 | u8 | rank 1..=20 |
| +0x41 + t | u8 | known to empire `t` (`t` = 1..=8; +0x49..+0x4b unused) |

### 1.2 Rule tables (static, byte-identical in both binaries)
| Name | exe VA | Layout |
|---|---|---|
| `KemKuldetesek` | 0x912F48 | 27 missions × 0x20: +0 enabled u8, +4 i32 kind (1 = the parameter is a planet of the target), +0xc i32 experience, +0x10 i32 length in days, +0x14 i32 cost, +0x18 u8 the rank the AI needs (both read by `FRISSIT_MI_KEMKEDES`, `ai-empire.md` §6; earlier versions of this spec had +0x10/+0x14 swapped), +0x19 u8 skill floor, +0x1a u8 skill for full chance, +0x1b u8 full chance %, +0x1c u8 needs a living target (and can be caught), +0x1d u8 story mission, +0x1e u8 no countdown |
| `KemKuldetesAtmenet` | 0x9132A8 | i32 `[5]`: new state per outcome 0..=4 (§4.1) |
| `KemSzintLepesSzetoszthato` | 0x9132BC | i32 `[15]`: skill points per rank, by race |
| `KemTartalekok` | 0x9132F8 | 15 template spy records × 0x4c, by race (§6.1) |
| `KemRangXPHatar` | 0x9137A8 | i32 `[21]`: experience for rank `r`. `Spy_New` reads `[r + 1]` up to `[21]` = the next i32 in the image (0) |
| `TrainingKuldetesXP` | 0x92705C | i32 `[6]`: experience of training mission level `+0x1e` |
| `Spy_New` names | immediates in 0x58f830 | 37 `char*` stored into a local array (`[ebp−0xa0 + 4i]`), NULL-terminated |

Missions 1..=13 are the regular ones (the parameter meanings below); 14..=26 are story missions (`+0x1d`).

## 2. The daily pass (`FRISSIT_Spy`)
```
KemKuldetesek[11].enabled = 0
for e in 1..=8 with PlayerFaj[e] != 5 and BirodalomEl[e]:
    for k in 1..=OsszHanyKem[e] (re-read every step) with state(e,k) > 0:
        ranks (§3.1), counter-intelligence experience (§3.2), double-agent exposure (§3.3)
        if state in {2, 3}: catch check (§4), countdown and arrival (§3.4), mission result (§5)
recruitment (§6)
HanyKemVan (§7)
```

### 2.3 Story missions
A mission with `+0x1d` asks the story script: the catch check calls `StoryEredmenyKemElkapasa(e, k, m, t,
p1, p2)` (true → outcome 1 below, nothing else), the result calls `StoryEredmenyKemSikeres(…)` (the
success flag; then experience, state and +500 grievance as in §5.13). Both set up `Story_Check(1)` and read its
answer; the port asks a hook. Untraced (quick.scn has no story missions).

## 3. Per spy, before the mission
### 3.1 Ranks
`while rank < 20 && KemRangXPHatar[rank + 1] <= xp: rank += 1; points += KemSzintLepesSzetoszthato[race]`
(the human gets a message per rank).

### 3.2 Counter-intelligence experience
State 3 on mission 13 and `Nap == 1` (once a month), **f64**:
`g = xp·0.03 + 1.0; r = rndlog(4, 50, 100); d = (r ≤ g) ? rndlog(4, 50, 100) : g; xp = (int)(xp + d)`
(a `min` macro that draws twice when the first draw is the smaller).

### 3.3 Double-agent exposure
A double agent (`+0x18 != 0`) at home or on counter-intelligence (state 1, or state 3 on mission 13): for every
other own spy `j` on counter-intelligence (state 3, mission 13), in slot order, `d = clamp(k.infiltration + 10 −
j.counter, 0, 20)`, and `rndlog(4, 0, d·5000) < 50` kills `k` (state 0). The loop goes on after a kill (more
draws).

### 3.4 Countdown and arrival
For state 2 or 3 (as it was before the catch check), after the catch check: `left −= 50` unless the spy's
current mission (after the check, which may clear it to 0) has `+0x1e`. Then if `left < 1`: state 2 → 1 (home);
state 3 → the mission result (§5).

## 4. The catch check
State 3, `left − 50 < catch < left`, mission `m`, target `t`, `+0x1d == 0`, `+0x1c != 0` (missions without
`+0x1c` are never checked; `catch` stays):

1. The target's counter-intelligence: its spies in state 3 on mission 13, slot order, count `n`.
2. `a8` = empires `x` in 1..=8 alive with `known_to[x]` set.
3. `n` rounds: `RndInitValasztas`, offer every not-yet-picked counter spy `RndValasztas(4, 1)`; the pick (if
   any) is marked, `defender = pick` (the last round's pick wins), `sum += pick.counter · 10`.
4. `att = rndlog(4, 0, k.infiltration · 140)`, then `def = rndlog(4, 0, (sum / (n + 1)) · 10)` (int division);
   `att /= 3` if `t` knows the spy; `att = (int)(att · 1.5)` (f64) if the owner's race is 3, and again for a
   human owner on difficulty 1 in SP. `def ≤ att` → `defender = 0`.
5. Tutorial → `defender = 0`. `slipped = defender == 0 && master != t` (`master` = `+0x18`); forced 2 (SP
   debug) → `slipped = false`.
6. Not slipped: `c0 = rndlog(4, 0, Kemek[t][defender].combat · 10)` (slot 0 when no defender), `c4 =
   rndlog(4, 0, k.combat · 10)`, `c = c4 · 100 / (c0 + c4 + 1)`, outcome:
   - `master == t` (a double agent spying on its master): **4**, no grievance;
   - `c < 45` or forced 2: **2 caught**: `BirodalomIsmeri[t][e] = 1` (if new and `t` is the human: story
     special 0xb, `FelfedezBirodalom(e, true)`); a human captor gets a message (the spy waits in state 4 for
     the captor's decision, `MULTIGET_Spy`); an AI captor decides at once: `fate = rndlog(4, 2, 4)`; for 2 or 3,
     `r = rndlog(4, 1, Kemek[t][k].loyalty + 3)` (**the captor's** slot `k`, kept), and if `r > 3` or
     `Kemek[t][k].+0x18 != 0`: `fate = (rndlog(4, 1, 2) == 1) ? 5 : 6`. Grievance +2000.
   - `c < 52`: if `a8 < 3`: **3 spotted, sent home**: `length = left = rndlog(4, 50, 100) · 50`, grievance
     +500; else **1 identified and lost**, grievance +2000.
   - else **4 unnoticed**, grievance +500.
   The spy's state becomes `KemKuldetesAtmenet[outcome]` (by the state meanings: lost, captured, travelling home, at home), outcomes
   2 and 3 set `known_to[t]`, the mission becomes 0, then the AI captor's `DontottAKemSorsarol` (§4.2).
7. `catch = −1000` (slipped or not).

Grievance = `Viszony[t][e].v0` (i16, wrapping).

### 4.2 `DontottAKemSorsarol(t, fate, e, k)`
1 / 5 released (state 1); 2 turned: `Spy_Copy(t, e, k)` moves it into `t`'s roster (full roster → 6);
3 double agent: state 1, `+0x18 = t`; 4 / 6 executed (state 0). Messages and the spy screen's cursor are UI.

## 5. The mission result (state 3, `left < 1`, `+0x1d == 0`)
`m` = mission, `t` = target, `p1`/`p2` = `+0x1e`/`+0x20`, `master` = `+0x18`. Report values `e0, e4, e8,
ec` (UI) start at 0; the outcome is always 4.

### 5.1 Success roll (f32)
```
mult = 1.0 (race 3: 2.0); human owner on difficulty 1 in SP: mult *= 2.0
s = infiltration
c = s < floor ? 0
  : (int)((s < full ? max · min(1, (s − floor + 1) / (full − floor + 1)) : max) · mult)     [f32]
c = min(c, max + 10); if c > 90 && m > 1: c = 90; c = min(c, 100)
ok = rndlog(4, 1, 100) ≤ c
```
Then in order: SP `DEBUGX` and the human's spy → ok; tutorial → ok; `+0x1c` and `t` dead → fail;
kind 1 with `p1 > 0` and planet `p1` not `t`'s → fail; forced 1 (SP debug) → ok; `t`'s race 5 → fail;
`+0x1c` and `t` dead → fail (code 997). The effect (§5.2–§5.12) runs only when `ok`, and may still fail
(code 999 "nothing to do", 998 "lost the duel").

### 5.2–5.12 Effects by mission
| m | Effect |
|---|---|
| 1 | training: none (experience from `TrainingKuldetesXP[p1]`) |
| 2 | empire report: counts only (planets and their population, `t`'s ships, `t`'s developed techs) |
| 3 | planet report. `p1 == 0`: over `t`'s planets the spy's empire hasn't seen (`+0xd8 + 2e == 0`), in order, two reservoirs `rndlog(4, 0, i) == 0` → `a`, then `rndlog(4, 0, i) == 0` → `b`, `i` += 1. If `a == b` or `rndlog(4, 0, 2) != 0` (drawn only when `a != b`): `b = 0`. Both become seen (`= 1`). If neither: the last of `t`'s planets already seen becomes the report, none → fail 999. `p1 != 0`: report on `p1` |
| 4 | tech report: techs 1..=192 `t` owns (status 5), in order with a counter `i`: the first of three reservoirs `a, b, c` whose test passes takes it (`a`: `a == 0 or rndlog(4, 0, i) == 0`, then `b`, then `c`, each draw only when its slot is filled; a slot also needs the tech unseen: seen matrix `+0xc0 + 9e + t == 0`), `i` += 1. Then `c = 0` if `b == c` or `a == c`; `b = 0` if `a == b`; the picks become seen; none → fail 999 |
| 5 | steal money: `r = rndlog(4, 0, (int)MoneyMennyi(t, 0))` (x87 truncation), `x = (r / 100) · 100`. `x < 1` and `t` human → fail. Else `t` pays `x` (`MoneyHozzaadLog(t, 0, −x)`), the thief gets `x` (+ `rndlog(4, 2000, 10000)` when `t` is an AI) with its income multiplier |
| 6 | sabotage: `st_eSearch2spy(4, t, p1, p2)`, then `(p1, 0)`, `(0, p2)`, `(0, 0)` until one finds a building (§5.6); none → fail 999; else `EpuletDelete` |
| 7 | destroy a ship: `RndInitValasztas`; over ships 1..=n alive in `t`'s fleets whose design `+0x80 == 0` and (`p1 == 0` or design `+0x48 == p1`), `RndValasztas(4, 1)`. None → the same without the `p1` test (a new reservoir run without a new `RndInitValasztas`); none → fail 999; else `HajoDelete` |
| 8 | steal a tech: `p1 == 0`: `RndInitValasztas`, techs 1..=192 with own status < 5 and `t`'s status 5, `RndValasztas(4, 1)` → `p1`. `p1 < 1` or `t`'s status ≠ 5 → fail 999. Else `TalalmanyKifejlesztve(e, p1)` and, for a double agent, `(master, p1)` (tutorial events 9/10 for techs 0x12/6) |
| 9 | incite a rebellion: `RndInitValasztas`; `t`'s planets (`p1 == 0` or `= p1`) that can rebel and be taken (`getFellazadhat && getMiAltalElfoglalhato`, or MP), `RndValasztas(4, 1)`; none → fail 999; else `BolygoFellazad(p, e, true)` |
| 10 | assassinate a spy: `RndInitValasztas`; `t`'s spies in state 1 or 2, not applicants, `RndValasztas(4, 1)`; none → fail 999; `rndlog(4, 0, victim.combat · 4) < rndlog(4, 0, k.combat · 4)` (drawn in that order) → the victim dies (state 0), else fail 998 |
| 11 | kidnap: `CsaladtagElrabolvaKi[t] == 0` and (MP or `t != 5`) → `Ki[t] = rndlog(4, 1, 5)`, `Kialtal[t] = e`; else fail 999 |
| 12 | mass unrest: `for i = 1; i ≤ rndlog(4, 2, 4); i++` (**redrawn every test**): `RndInitValasztas`; `t`'s planets as in 9, `RndValasztas(4, 100000 / (morale + 100))` (`+0x30`, int division); a pick rebels (`BolygoFellazad(p, e, true)`). Stays a success with no planets |
| 13 | counter-intelligence: none (no countdown, so it only ends from outside) |

### 5.6 `st_eSearch2spy(s, owner, p, tech, &planet, &slot)`
Clears both outputs; over planets 1..=n (`p == 0` or `= p`) owned by `owner`, building slots 1..=count with
`kind != 0`, finished (`+8 == 0`) and (`tech == 0` or `EpuletekLeiro[kind].tech == tech`), in order with a
counter `i`: `rndlog(s, 0, i) == 0` → this one. Returns the planet. `EpuletDelete(p, slot)`: kind ← 0,
`HanyEpulet[p] −= 1` (a kind-'+' building also clears a per-planet colony-screen field; ground-war units
targeting it lose their target: not modelled).

### 5.13 After the effect
`xp += ok ? rndlog(4, X/2, X) : rndlog(4, 200, 400)` with `X = KemKuldetesek[m].+0xc` (training: `X =
TrainingKuldetesXP[p1]`); state = `KemKuldetesAtmenet[4]` (1, at home); mission 0; grievance
`Viszony[t][e].v0 += 500` (success or not).

## 6. Recruitment
`rndlog(4, 0, 12500 / ProgramPlayers) < 50` (or SP forced 3), unless `TutorialUjKemekLetiltva` (0xF66F2B):
`e = rndlog(4, 1, ProgramPlayers)`; if `e` is alive, not race 5 and has no applicant (state > 0 and `+0x16`),
it gets a spy: `Spy_Copy(e, e, 0)` when slot 0 holds one (state ≠ 0), else `Spy_New(e)`. A human owner's new
spy is an applicant (`+0x16 = 1`, message). The draws happen whether or not `e` qualifies.

### 6.1 `Spy_New(e)`
1. Slot: the first `k ≤ OsszHanyKem[e]` with state ≤ 0, else a new one (`OsszHanyKem += 1`; at 12 → −1).
2. Race: `rndlog(4, 1, 14)` redrawn while 5, 9, 10, 11 or 12. Copy `KemTartalekok[race]` into the slot.
3. Name: `rndlog(4, 0, 36)` from the name list, redrawn while any slot `1..=OsszHanyKem` with state > 0
   (the new one included, holding the template's name) has that name (`strcmp`).
4. State 1, race byte. Rank: `hi` = the highest rank among working spies (state > 0, not applicants), at
   least 2; `rank = rndlog(4, 1, hi + 1)`; `rndlog(4, 1, 20) == 1` → +2; SP difficulty 1, human, roster < 2 →
   +2; at most 20. `xp = rndlog(4, RangXP[rank], RangXP[rank + 1])` (rank 20 reads the 0 after the table:
   `lo > hi` → 90000-style `lo`). Own race: loyalty +2. Then `(rank − 1) · KemSzintLepesSzetoszthato[race]`
   times: skill `rndlog(4, 1, 4)` += 1 (u8, wrapping).

### 6.2 `Spy_Copy(e, from, k)`
Slot as in §6.1 (−1 when full), copy the 0x4c bytes of `Kemek[from][k]`, then `Kemek[from][k].state = 0`,
the copy's state = 1.

## 7. Working spies
`HanyKemVan[e]` = spies with state > 0 and not applicants, for `e` in 1..=8 alive and not race 5 (others 0).

## 8. Port notes
- `ig2-sim::spy`: `Spies` (the roster, counts, the kidnap mission switch), `SpyRules` (§1.2 tables and the
  names, read from the exe by `ig2-rules`), `World` = the colony world (`colony_side::ColonyWorld`: planets,
  buildings, money, techs, `BirodalomIsmeri`), the combat world (`spacewar::CombatWorld`: ships, fleets,
  designs), research, the relations (`diplomacy::Relation`), the kidnap table, stream state and a story hook.
  UI messages, the human's reports (`KemJelentesAdd`, `info-messages.md` §3.3), `FelfedezBirodalom`, story
  specials and tutorial events are `SpyEvent`s; `Game::spy_hooks` carries them out.
- Original quirks kept: the AI captor's loyalty roll reads its own slot `k` (§4 step 6); `Spy_New`'s rank 20
  reads one entry past `KemRangXPHatar`; mission 12 redraws its loop bound; mission 7's second pass reuses
  the first pass's weighted-choice total; the kidnap mission is disabled every day.
- Open: the spy screen (`Spy_Main`), `MULTIGET_Spy` (the human's missions, decisions about captured spies,
  hiring), story missions. The AI's mission choice and its costs are ported with the AI (`ai-empire.md` §6).
