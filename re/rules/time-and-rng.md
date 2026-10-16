# Time model and RNG

Status: **verified at runtime** on the GOG `ig2.exe` (`traces/2026-10-15-time-rng.md`): 1,114 RNG
calls bit-exact, 872 sim ticks at 20.00 Hz with `Ido` steps 1/4/16, calendar formulas exact, scenario
`RandSeed` → streams 1–20 confirmed. Static evidence: ELF `ig2` (-O0) decompilation.

## 1. Clocks and the main loop

`IG2_gameUpdate` (0x587400) → `FRISSITES` (0x585120, re-entrancy guard) → `Fake_FRISSITES` (0x585160).
Both clocks are driven by `SDL_GetTicks()` and **clamp the backlog to 500 ms**
(`if (Old < now - 500) Old = now - 500`).

| Clock | Period | What runs |
|---|---|---|
| `OldIdo2` | **10 ms** (100 Hz), `while (now - OldIdo2 > 10)` | input (`DoKeyboard`, gestures), per-screen visual updates `FRISSIT_*2`, sound, autoscroll, tutorial counter; `Poll(true)` every 10th tick |
| `OldIdo` | **50 ms** (20 Hz), `while (now - OldIdo >= 51)`, step `+50` | the simulation tick, below |

So the original sim is a fixed-step 20 Hz simulation with catch-up of at most 9 ticks per frame
(500 ms clamp, strict `> 50` test).
Frame-rate-dependent behaviour is limited to the drop of backlog over 500 ms.

### Simulation tick (every 50 ms of real time)
```
Counter += 1                                   (unless paused by a screen transition)
if GameStarted:
  if running (not IdoPause, not in a screen transition) or (MP and not IdoPause):
    if no MP battle in progress:
      if SP and (screen == SpaceWar(0x0b) or (screen == Colony(3) and planet at ground war)):
          [strategic time frozen] FRISSIT_Epuletek (colony only); GroundWarSetBattleStatus
      else:
          FRISSIT_Datum                        Ido += IdoSpeedSzorzo; Ido2 += 1
          if IdoNapValtozott():                (day-of-month changed)
              if Nap % 6 == 1: FRISSIT_Planet(true, 0)     ← economy every 6th day (days 1, 7, 13, 19, 25)
              FRISSIT_Viszony                  relations
              FRISSIT_MI                       AI (unless disabled flag)
              FRISSIT_Spy
              MI_DIPLOMACY
          every tick: FRISSIT_Trade, FRISSIT_Info, FRISSIT_Research, FRISSIT_Story,
                      FRISSIT_Messages, FRISSIT_Diplomacy, FRISSIT_Design,
                      FRISSIT_Epuletek, FRISSIT_Starmap, MegnyeresEllenoriz (victory check)
    else: MULTISEND_SpWarSynch; GroundWarSetBattleStatus
    IdoMultiPlayerBattle += 1
    if no MP space battle and (screen != SpaceWar or MP): FRISSIT_Grwar; FRISSIT_Colony
          (Game::grwar after the tick: gvalue 0x9B2900 += 1, self-destruct countdowns, WF::Collapse every
          300th; FRISSIT_Colony saves nothing: passability-map expiry after 600 ticks, smoke emitters 0xA3E9C8
          with libc draws, debris 0xA33390, the UI effect list; not ported)
    FRISSIT_SpaceWar
  else if paused and screen == Colony: every 60th tick (3 s) FRISSIT_Planet(false, 0)  (display refresh)
MP: supervisor sends a ping when (Ido / IdoSpeedSzorzo) % 111 == 0.
MP pacing: ticks continue in the same frame until (Ido / IdoSpeedSzorzo) % Multiplayer_speed == 0;
           then MULTISEND_Starmap + MultiSendPackets (periodic state broadcast). SocketBlock stalls the sim.
```

## 2. Calendar (`Ido`)

Game time is an integer `Ido`: **50 units per day, 30-day months, 12-month years**.
```
DatumToIdo(ev, honap, nap, x) = ev*18000 + (honap-1)*1500 + (nap-1)*50 + x       (0x620660)
Ev    = Ido / 18000                                                               (0x6206a0)
Honap = (Ido / 1500) % 12 + 1                                                     (0x6206c0)
Nap   = (Ido / 50) % 30 + 1                                                       (0x6206f0)
```
`IdoNapValtozott` (0x620720) compares `Nap` with `OldNap`. On a change it returns true, and on
day 1 of months 1 and 7 (`Honap % 6 == 1`) it autosaves (SP, not in tutorial, option enabled).
The largest step (16) is below 50, so a day is never skipped; it may be detected up to one tick late.

### Speed (`SetIdoSpeed(speed, force)`, 0x6233d0)
| Mode | speed 1 | 2 | 3 | battle multiplier (1/2/3) |
|---|---|---|---|---|
| single player | `IdoSpeedSzorzo` = 1 | 4 | 16 | 1 / 2 / 4 |
| multiplayer (`ProgramSebesseg`) | 4 | 8 | 16 | 1 / 2 / 2 |

At 20 ticks/s: SP speed 1 = **2.5 s per day** (75 s per month), speed 2 = 0.625 s/day, speed 3 ≈ 0.156 s/day.
Speed can't change when `nemlehetidotallitani` is set (scripted), and in MP with `ProgramSebesseg == 4`
only a forced call changes it. `SetIdoPause` (0x623670) is ignored in MP unless forced, under the same rule.
`.scn`: `Ev`/`Honap`/`Nap`/`Ido` store the clock (`Ido 41890761` = year 2327, month 4, day 6 ✔).

## 3. Random numbers

### 3.1 Seeded streams (`rndold`, 0x622130)
21 independent 32-bit states `RandSeed[0..20]` (.bss 0x1415d00).
```
rndold(stream, lo, hi):
    s = RandSeed[stream] * 214013 + 2531011          (mod 2^32; MSVC LCG constants)
    s = rotl32(s, 21)                                 (s << 21 | s >> 11)
    RandSeed[stream] = s
    return lo > hi ? lo : ((s >> 16) & 0x7FFF) % (hi - lo + 1) + lo
```
`rndlog(stream, lo, hi, file, line)` (0x61e170) strips the path from `file`, then calls `rndold`
(the logging itself is compiled out). `RndValasztas(stream, weight)` (0x61e100) does weighted choice:
the running total grows by `weight`, and it returns `rndlog(stream, 1, total) <= weight`
(after `RndInitValasztas` resets it).

**Seeding.** `RandomInit(seed)` (0x6220e0) calls `srand(seed)`, then sets every stream to
`random(-0x80000000)`. In the ELF, `random(n)` returns 0 for n < 1, so all streams start at 0.
**The Windows build differs** (runtime trace): after loading `quick.scn` stream 0 held 19263, a 15-bit
MSVC `rand()` value, so the exe's `RandomInit` fills streams with `rand()` output. Either way stream 0
is unused and streams 1–20 are overwritten by the scenario, so gameplay doesn't depend on it. Real stream state comes from scenario/save data: the `.scn` `RandSeed` line
carries 20 values, which `LoadScenario` (loop at 0x62e481, `for i in 1..=20`) stores to
**`RandSeed[1..=20]`**. Stream 0 is never loaded or used. `RandomInit` is
called from `IG2_gameInit` with `SDL_GetTicks()` (affects only libc `rand`) and from `InitNewGame`.

**Stream allocation** (constant first argument at the 471 call sites):

| Stream | Sites | Subsystem |
|---|---|---|
| 1 | 32 | galaxy and new-game generation (`InitNewGame`, `Generate_Planet`, `Generate_Star*`, skirmish planets) |
| 2 | 31 | ground combat (`WF::Simulate`, `WF_Tank::Shot/CalcDamage`, special weapons) |
| 3 | 9 | planet economy (`FRISSIT_Planet`) |
| 4 | 37 | espionage (`FRISSIT_Spy`, `Spy_New`, `KemEselyGetBest`) |
| 5 | 150 | story/campaign scripting (`st_Rnd_include`, `st_Elem_include`, `Story_Check`, `InitStory`) |
| 6 | 67 | space combat (`SpwC::AddFleet`, `Loves2` shooting, targeting, fighters) |
| 7 | 2 | auto-resolved battles (`SimulateSpBattle`) |
| 8 | 4 | rebellion and virus spread (`BolygoFellazad`, `Virus_CheckTerjedes`) |
| 9 | 32 | AI diplomacy (`MI_DIPLOMACY`, answer reasoning) |
| 10 | 9 | AI (`FRISSIT_MI_KEMKEDES/FLOTTAK/GYARTAS`) |
| 11 | 18 | trade (`FRISSIT_Trade`) |
| 12 | 2 | water animation (`UpdateWater`; cosmetic) |
| 13 | 4 | ground-combat AI (`WF::MI`) |
| 14 | 1 | starmap |
| var. | 44 | stream chosen at runtime (`WF::Simulate` 17, `WF_Hordozo::Refresh2` 7, story 9, …) |

Streams 0 and 15–20 aren't used by constant call sites.

### 3.2 libc `rand()` (`random(n)` = `n < 1 ? 0 : rand() % n`, 0x4f0200)
Seeded by `srand(SDL_GetTicks())` at start-up, so **not reproducible**, and not synchronised in MP.
Mostly cosmetic (particles, weather, explosions, music choice, UI), **but these gameplay paths use it too**:
- ground combat: `WF::Damage`, `WF_Tank::Shot`, `WF_Tank::ShotWithSpecificWeapon`, `WF_Tank::Detonate`, `WF_Tank::Damage`, `WF::RemoveTankGroup`
- diplomacy decisions: `FeldolgozAjanlat`, `FeldolgozValasz`, `EldontIgenNemValasz`
- trade: `FRISSIT_Trade` (one draw per new month, only the trade screen's default delivery time: `rules/trade.md`
  §3.1); `TradeRandMerchant` isn't called by it
- economy: `FRISSIT_Planet` (2 sites), `SelfDestruct`, `colony_start_build`
- generation: `GenerateGalaxy`, `NEWGAME_Diplomacy`, `LoadBolygoUrhajok`

(Call counts are per call instruction in the -O0 ELF; full list in `traces/rand-callers.txt`.)
[inf] **This is a prime suspect for the original's MP desyncs**: identical inputs can give different
ground-combat and diplomacy outcomes on different machines.

## 4. Consequences for the Rust engine (ADR 0003)
- Keep the **20 Hz fixed sim tick** and the 50-unit day exactly. Rendering interpolates; input is sampled per sim tick.
- Implement `rndold` bit-exactly with 21 streams (classic stream allocation), so scenario `RandSeed`
  values reproduce the original sequences.
- Route every gameplay use of libc `rand()` to a dedicated seeded stream (new streams 15+). Cosmetic
  randomness goes to a separate, non-simulation RNG. This is a documented deviation: per-call
  distributions are kept (`rand() % n` uses 15-bit MSVC `rand`, see below) but values aren't bit-identical.
- Note on the platforms: Windows `rand()` is the MSVC LCG (`x = x*214013 + 2531011; return (x >> 16) & 0x7FFF`);
  glibc `rand()` differs. The two original builds therefore produce different cosmetic and libc-random sequences.
