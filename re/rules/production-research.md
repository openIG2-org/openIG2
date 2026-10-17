# Production and research queues

Status: **verified at runtime** (§9): 9,000 research ticks (3,350 completions) and 6,919 production ticks
(1,575 units) reproduced byte for byte by `engine/crates/ig2-sim` (`research.rs`, `production.rs`,
`tests/production_trace.rs`). Two per-tick systems that spend
money from pools 1 (production) and 3 (research) (`colony-side.md` §6.4):

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Research` | **0x51ddd0** | 0x5ebe50 | research progress, every tick (found by the `research.cpp` line `push 0x74a` @ 0x51e06a) |
| `TalalmanyKifejlesztve(e, t)` | 0x524e50 | 0x5ebcb0 | tech developed |
| `Research_TalalmanyUjraSzamol` | 0x5245c0 | 0x5eb450 | status recompute (`colony-side.md` §4.1) |
| `FRISSIT_MI_FEJLESZTES(e)` | 0x50b0b0 | 0x5ae350 | AI picks the next research |
| `FRISSIT_Starmap`, production block | 0x597322..0x597aac | 0x7132b0 (after the fleet loop) | planet production queues, every tick (`starmap.cpp` line `push 0x25ca` @ 0x5974cd) |
| `Legyart(…)` | 0x59d740 | 0x70f250 | a finished unit appears |
| `SearchBolygohozKozeliFlotta(p, lim, own, idle, room, calm, last)` | 0x540f50 | 0x6162b0 | fleet next to a planet |
| `FlottaNew` / `HajoNew` / `TankNew` | 0x53d400 / 0x53e800 / 0x541750 | 0x6154c0 / 0x617f60 / 0x618a70 | new fleet / ship / vehicles |
| `TankNewGrWar` core / `TankFlottahozHozzaszamol` | 0x541800 / 0x5415d0 | 0x6183a0 / 0x6185f0 | vehicle entry / sums |

Call order in a sim tick (`Fake_FRISSITES`, `time-and-rng.md` §1): … `FRISSIT_Info` → **`FRISSIT_Research`**
→ `FRISSIT_Story` → … → `FRISSIT_Starmap` (fleet movement, radar, clean-up, then **production**).
Neither system rolls the RNG. **The exe is normative**; it computes the research speed in f64 where the
ELF uses f32 (§8). No rule values here: tables are read from the user's exe (ADR 0004).

## 1. Data (exe VAs)
| Name | exe VA | Layout |
|---|---|---|
| `ResearchAktFejlesztett` | 0xB9A9B8 | i32 `[19]`: tech being researched (0 = none) |
| `KutatasSurgos` | 0xB9C014 | u8 `[19]`: urgent (double speed, double cost) |
| `LaborSzint` | 0x912728 | i32 `[19][4]`, `[e][1..=3]` lab levels (`colony-side.md` §7) |
| `FejlesztesSebessegSzorzo` | 0x8F885C | f32 `[5]` by `Nehezseg` (0x92827C): AI research speed |
| `NehezsegGyartasSzorzo` | 0x8F8834 | f32 `[5]`: AI production speed |
| `FajSpeckoKutatasIdoSzorzo` / `…MoneySzorzo` | 0x8F87F8 / 0x8F87BC | f32 by race (`PlayerFaj` 0xBAE574) |
| `StoryAdat+0x6c` | 0xF27464 | i32: human research bonus % (SP) |
| research cheat | 0xB7727C | non-zero: the human ignores labs and finishes at once |
| `MoneyHatar[e][1]` / `[e][3]` | 0xBAD598 / 0xBAD5A8 (+0x30·e) | f64 reserve of pools 1 / 3 for the rest of the month |
| `BolygoUtolsoKozeliFlotta` | 0xB99340 | i32 `[201]`: the planet's last delivery fleet |
| `OsszHanySajatFlotta` | 0xBB40B0 | i32 `[19]`: fleet name counter per owner |
| `AlapFlottaNev` | 0xBB6964 | `char*`: default fleet name (string 17/5) |
| `Tervek[e]` / count | 0xB9AA18 / 0xB9C028 | design tables (`space-combat.md` §2.3) |

Tech record (`Talalmanyok` 0xB9C0C8, 0x114 B): `+0x26` u8 / `+0x28` i32 auto-design triggers, `+0x31..+0x33`
labs needed (ship, vehicle, building research), `+0x34` research cost, `+0x38` research time,
`+0x45+e` status (1 unknown, 2 seen, 3 researchable, **4 being researched by the AI**, 5 have),
`+0x58+4e` progress, **`+0xb7+r` AI priority per race `r` 1..=8** (0 = none; lower first).

Planet production (`Bolygok` + offset; `pGyartas0` 0x90B8A8 resets `+0x78..+0xb8`, `colony-side.md` §4):
| Offset | Type | Field |
|---|---|---|
| +0x64 | u8 | non-zero: no production (exe tests the byte, ELF bit 0) |
| +0x70 / +0x74 | i32 | capacity of line 0 (ships) / 1 (vehicles) (`colony-economy.md` §8) |
| +0x78 + 8L + 2k | i16 | queue line `L`, slot `k` 0..=3: design (≤ 0 = empty) |
| +0x88 + 4L + k | u8 | units to build |
| +0x90 + 4L + k | u8 | repeat (re-queue at the end when done) |
| +0x98 + 4L + k | u8 | UI byte, rotated with the queue |
| +0xa0 + 4L | i32 | build time left on the head unit |
| +0xa8 + L | u8 | units of the head entry built so far |
| +0xac | u8 | rally planet for new ship fleets (0 = none) |
| +0xad + 4L + k | u8 | "into a fleet" (only `+0xb1`, line 1 slot 0, is read: §3) |
| +0x100 + 2n / +0x114 | i16 / i32 | space stations `1..=n` / their count |
| +0x58 / +0x5c | i32 | garrison vehicle count / strength |

## 2. Research (`FRISSIT_Research`, every tick)
For `e` in 1..=18, `t = ResearchAktFejlesztett[e]`:
- `t < 1`: `MoneyHatar[e][3] = 0`, next.
- `need_i` = tech `+0x30+i` (i = 1..=3), `L_i = LaborSzint[e][i]`, `extra_i = need_i ≠ 0 ? max(L_i − need_i, 0) : 0`.
```
v = (f32)(((f64)extra1 + 5.0 + (f64)extra2 + (f64)extra3) / 5.0 × (f64)IdoSpeedSzorzo)
urgent: v ×= 2.0f;   AI (PlayerKi[e] != 1): v ×= FejlesztesSebessegSzorzo[Nehezseg]          (f32)
e == EN and SP: v = (f32)(((f64)StoryAdat+0x6c + 100.0) / 100.0 × (f64)v)
limit = (f32)tech.+0x38 × RaceResearchTime[race(e)]                                          (f32)
v = min(v, limit − (f32)progress);   v = max(v, 1.0)
```
- If (`e == EN` and the cheat) or `L_i >= need_i` for all three:
  ```
  c = (f32)tech.+0x34 × RaceResearchMoney[race] × v / limit       (f32, left to right)
  urgent: c ×= 2;  AI: c /= FejlesztesSebessegSzorzo[Nehezseg]
  if MoneyMennyi(e, 3) >= (f64)c: MoneyHozzaad(e, 3, −(f64)c); progress = (int)((f32)progress + v)
  room  = (f64)(limit − (f32)progress)  (progress after the step);  urgent: room ×= 2.0
  month = (f64)(31 − Nap) × 50.0 × (f64)v / (f64)IdoSpeedSzorzo
  MoneyHatar[e][3] = (f64)((f32)tech.+0x34 × RaceResearchMoney[race]) × min(room, month) / (f64)limit
  ```
  else `MoneyHatar[e][3] = 0` (no progress without the labs).
- Cheat: `progress = (int)(limit + 10.0f)`.
- **Done** when `limit <= (f32)progress` or the tech is already owned (status 5): human only: message,
  **slow-down** `SetIdoLelassit(3)`, tutorial events 9 (t = 0x12) / 10 (t = 6); `TalalmanyKifejlesztve(e, t)`;
  `ResearchAktFejlesztett[e] = 0`; story special 0x0D(e, t); `FRISSIT_MI_FEJLESZTES(e)` (§2.2);
  `FRISSIT_MI_DESIGN(e, false)` (AI designs; not ported, an event).

The slow-down (exe 0x541270) acts **at once**: SP, unless the speed is locked (`nemlehetidotallitani`
0xBAD0D4): `IdoSpeed = IdoSpeedSzorzo = 1` (and the battle multiplier); MP: back to `ProgramSebesseg`
(0xBAE54C) 1/2/3 → ×4/×8/×16 (4: unchanged). Empires after the human in this loop, and the rest of the
tick (`FRISSIT_Starmap` movement and production), run at the new multiplier (found by the fuzzed trace).

### 2.1 `TalalmanyKifejlesztve(e, t)`
If `status[t][e] != 5`: `progress = (int)((f32)tech.+0x38 × RaceResearchTime[race(e)])`; status 5; for a
human-controlled `e` with `+0x26 != 0` or `+0x28 != 0` → `TervezAutomatikus(e, t)` (§2.3);
the recompute (`colony-side.md` §4.1); `ResearchAktFejlesztett[e] == t` → 0. Tech **0x95** also develops
**0xAA** (same steps).

### 2.3 `TervezAutomatikus(e, t)` (exe 0x4b72b0, ELF 0x61bc80; verified in the save replay)
The first hull `h` in `Alapok[1..25]` whose tech (`+0xc`) is `t`; none → nothing. Else `d = TervNew(e, 0)`
(ai-empire.md §10.2), `+0x44 = h`, the design's name = `strcpy` of tech `t`'s name (the localised one),
`TervUjraSzamol(e, d, false, true)`, `TervAutoDesign(e, d, 0)` (§10.3), `TervUjraSzamol` again. On the design
screen it also re-sorts the list (UI). Port: `ai::World::automatic_design`, run by `Game` for the
`ResearchEvent::AutoDesign` of the research tick and of message 0x31; the name is reported
(`AiEvent::DesignNamed`).

### 2.4 The research screen's orders (`MULTIGET_Research` exe 0x51ee70, ELF 0x5ec6e0; verified byte-exact)
Lockstep messages from `Research_Main` (re/screens/research.md §6), also in SP. Values after the issuer `e`:
- 0x29 `(4, t)` stop: `ResearchAktFejlesztett[e] = 0`, status(t, e) = 3.
- 0x2a `(4, t)` start: `ResearchAktFejlesztett[e] = t`, status(t, e) = 4.
- 0x2b toggle `KutatasSurgos[e]` (`!= 0` → 0, else 1).
- 0x31 (a debug key): for t from 0xc0 down to 1: if some empire 1..=8 has a non-zero status for t and e doesn't
  own it, `TalalmanyKifejlesztve(e, t)` and, in SP, `Story_Special(0x0d, e, 5)` (5, not t); then
  `Research_TalalmanyUjraSzamol`.
The handler clears `ButtonPressed` for the local issuer (screen state). The values are not range-checked in the
exe (a tech outside the table writes past it); the port refuses those. Port: `Game::research_order`
(`game::ResearchOrder`), `ig2_net::Command::Research`.

### 2.2 AI choice (`FRISSIT_MI_FEJLESZTES(e)`)
Only for `PlayerKi[e] != 1` with `ResearchAktFejlesztett[e] == 0`. `r = race(e)` if < 9, else 1. Over techs
1..=192 with status 3 and the labs (or `e == EN` with the cheat), priority `b = +0xb7+r`, best `B` from −1:
accept if (`b ≠ 0` and (`b < B` or `B < 1`)) or (`b == 0` and `B < 0`). So the lowest non-zero priority
wins, ties to the first; a priority-0 tech only if nothing else. The winner gets status **4** and becomes
`ResearchAktFejlesztett[e]`.

## 3. Production (end of `FRISSIT_Starmap`, every tick)
`MoneyHatar[e][1] = 0` for `e` in 1..=18. Then planets `p` in 1..=`HanyBolygo`, `o = owner(p)`, lines
`L` = 0, 1, while `d = design[L][0] > 0` and `+0x64 == 0`, `D = Tervek[o][d]`, `rate = +0x70+4L`:
```
s    = (f64)(rate × IdoSpeedSzorzo) × 0.03                      (i32 product)
AI:  s = (f64)NehezsegGyartasSzorzo[Nehezseg] × s;    MP and AI: s ×= 0.5
cost = s × (f64)D.price / (f64)D.time
cap  = min((f64)left, (f64)(31 − Nap) × 50.0 × (f64)rate × 0.03)
MoneyHatar[o][1] += cap × (f64)D.price / (f64)D.time
if MoneyMennyi(o, 1) >= cost or cost <= 0: left = (int)((f64)left − s); MoneyHozzaad(o, 1, −cost)
```
(`left` = `+0xa0+4L`, `cap` uses it before the step.) If `left < 1`, a unit is finished:
1. `f = SearchBolygohozKozeliFlotta(p, 6.0, own, idle, room = (L == 1), false, BolygoUtolsoKozeliFlotta[p])`
   (§4), stored back into `BolygoUtolsoKozeliFlotta[p]`. `room_f` = `f > 0` and `f.+0x78 > f.+0x5c`.
2. `Legyart(o, p, f, d, 1, rally, into, D.class, D.fighter, D.station, D.type == 80, false)` with
   `rally = +0xac` (0 for a station design, `+0x37`), `into = D.class == 1 or (room_f and +0xb1 != 0)` (§5).
3. `built[L] += 1` (u8). If `built[L] >= count[L][0]` the head entry is done: remember its design,
   count, repeat, `+0xad` byte; **shift** slots 1..=3 down (design, count, repeat, `+0xad`; the `+0x98`
   bytes are rotated by swaps, so the head's moves to slot 3), `built = 0`, `design[3] = 0`; while the new
   head has a design and count 0, shift again. Repeat set: the old head goes back in at the first slot
   after the last non-empty one among 0..=2 (slot 3 if all are full; design, count, repeat, `+0xad`;
   its `+0x98` byte is swapped with slot 3's).
4. Whatever the head is now: `design[L][0] > 0` → `left = Tervek[o][design].time`.

### 3.1 Notes
- Spending stops when the money runs out but `left` doesn't move then; a unit finishes on the tick
  `left` drops below 1 (it can overshoot to ≤ 0).
- `MoneyHatar[o][1]` (production reserve) and `[o][3]` (research reserve, §2) feed `MoneyMennyi` of the
  other pools (`colony-side.md` §6.4): they keep the month's planned spending from being taken by others.

## 4. Fleet next to a planet (`SearchBolygohozKozeliFlotta(p, lim, own, idle, room, calm, last)`)
A fleet `g` **qualifies** if: owner ≠ 0xFF and ≠ 0; `own` → owner == `p.owner`; `idle` → order 0, or a
move to planet `p` (`order 1, kind 1, +0x44 == (f32)p`); `room` → `+0x5c < +0x78`; not colonising/terraforming
(`+0x52 & 0x18 == 0`); `calm` → not in battle. Distance `|px − gx| + |py − gy|` (f32) `< lim`.
`last` (if > 0) is returned at once when it qualifies (sticky delivery fleet). Otherwise scan 1..=n:
a qualifying fleet nearer than the best so far replaces it, but once a candidate exists **only fleets
without an order** can replace it. Returns 0 if none.

## 5. `Legyart(o, p, f, d, n, rally, into, class, fighter, station, slow, single)`
- `class == −1`: nothing.
- `class == 2` and `into` and `f.+0x78 <= f.+0x5c`: `into = false`.
- **station** (`+0x37`): `p.+0x114 += 1 = k`, station `k` (`p.+0x100 + 2(k − 1)`, i16) = `d` (no bound). Done.
- not `into`: `TankNew(−p, d, n)` (garrison; nothing for `class < 2`). Done.
- `rally > 0` or `f == 0`: for `class == 1` a new fleet `f = FlottaNew(AlapFlottaNev, p.owner, 0, p.x, p.y, 0)`;
  `f == 0` (other classes) → done.
- fighter: `TankNew(f, d, single ? n : 5n)` (5 fighters per unit); `class == 2`: `TankNew(f, d, n)`; else
  `n × HajoNew(f, d)`.
- `rally > 0` and `class == 1`: `FlottaMozgat(f, 1, 1, (f32)rally, 0)` and `+0x52 |= 0x20`.
- `f.+0x54 == 0` → `FlottaDelete(f)` (`space-combat.md` §5.3).

### 5.1 Helpers
- `FlottaNew(name, o, id, x, y, speed)`: `OsszHanySajatFlotta[o] += 1 = N`; if `name` equals string 17/5 the
  fleet is called `"name N"`, where `N` grows by one for every fleet `0..n−1` of owner `o` already named
  `"name N₀"` (the original number, compared each time); else `name`. Slot: the **last** free one
  (owner 0xFF) in 1..=n, else append. Writes: owner, story id, `+0x18 &= ~3`, `+0x1a..+0x2d = 0`
  (visibility 1..=18 and two bytes), `vis[o] = 1`, speed: `speed <= 1`: `(speed == 0 ? 6 : speed) / 100`
  with override 0, else both `speed / 100`; position; order/kind/target 0; colony ships, terraformers,
  waypoint 0; flags: only bit 15 survives, bit 11 = name ≠ `AlapFlottaNev`; `+0x54..+0x7f = 0`.
  The name bytes after its NUL (`strcpy`), `+0x18` bits 2–7, `vis[0]`, `+0x2e/+0x2f` and flag bit 15 keep
  whatever the slot held (uninitialised memory for an appended slot).
- `HajoNew(f, d)` (f > 0): the **last** free ship slot (design 0x7FFF), else append; `fleet, design, +4 = 0`;
  `HajoFlottahozHozzaszamol(f, d, 1)` (`space-combat.md` §3).
- `TankNew(f, d, n)`: only if `class(d) > 1` or `f > 0` (owner = fleet owner, or `p.owner` for `f = −p`).
  Entry: the **last** live entry with this fleet and design, else the **last** free one, else append (at
  most 1000 entries: beyond that nothing); new entry `(f, d, n)`, else `count += n`; sums
  `TankFlottahozHozzaszamol(f, d, n)` (planets: `p.+0x58 += n`, `p.+0x5c = (int)((f32)sqrt((f64)(f32)(hp × attack))
  × (f32)n + (f32)p.+0x5c)`, ground class only). Garrisons (`f < 0`) also enter the planet's ground-war
  map (`AddOwnTankToPlanet` × n, `RearrangeTankGroups`): not ported, an event.

## 6. Events (not ported)
Research: human messages and slow-down, tutorial checks, story special 0x0D, `FRISSIT_MI_DESIGN` (`Game` carries
out 0x0D, `FRISSIT_MI_DESIGN` and `TervezAutomatikus`). Production: the UI refresh flags (`PrShip*`/`PrTank*`), ground-war placement of new
garrison vehicles.

## 7. Order and RNG
Research: empires 1..=18 in order; a completion's recompute is visible to later empires, and the human's
completion slows the clock for them (§2). Production reads `IdoSpeedSzorzo` as it stands at the end of
`FRISSIT_Starmap` (the radar pass's "enemy approaching" slow-down, `fleet-movement.md` §6.3, also acts at once). Production:
planets in order, line 0 before line 1, one unit per line per tick at most. No RNG.

## 8. Exe vs ELF
- Research speed: the exe sums the lab terms, divides and multiplies by `IdoSpeedSzorzo` in **f64**
  (`addsd`/`divsd`/`mulsd` 0x51dea0..0x51dee3, then `cvtpd2ps`); the human's story bonus is f64 too
  (0x51df41..0x51df54). The ELF does both in f32.
- Money tests compare in f64 in both (`comisd` 0x51e05a, 0x5974ae).
- Byte flags tested `!= 0` in the exe (`+0x64`, urgent, `+0x26`), `& 1` in the ELF.
- Inline constants (doubles unless marked): 5.0, 100.0, 50.0, 0.03, 0.5, 2.0; `2.0f`, `1.0f`, `10.0f`.

## 9. Verification
`traces/2026-10-15-production.md`: Frida before/after region diffs of every `FRISSIT_Research` call and
every production block, plain and fuzzed, replayed byte for byte; 18/23 research and 21/27 production
mutations caught, the rest data-equivalent or untraced (listed there).
`traces/2026-10-17-research-orders.md`: 615 research-screen messages (§2.4) staged through the exe's own handler in
three save replays, byte-exact on the whole state with `TervezAutomatikus` (§2.3); 6/7 mutations caught.
