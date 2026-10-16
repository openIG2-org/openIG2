# Fleet movement and the starmap tick (`FRISSIT_Starmap`)

Status: **verified at runtime** (§11): 4,000 traced ticks (400 radar refreshes), 25,853 fleet moves,
593 arrivals, 1,808 RNG rolls, 19 trader payments, and all radar/visibility state (§6) reproduced
bit-exactly by `engine/crates/ig2-sim` (`fleet.rs`, `radar.rs`, `tests/fleet_trace.rs`). Evidence: ELF `ig2` (-O0) `FRISSIT_Starmap()` @ 0x7132b0
(`../sources_base/starmap.cpp`) and the shipped exe `FRISSIT_Starmap` @ **0x594ae0** (found by the
`starmap.cpp` line numbers it pushes to `rndlog`: `push 0x2532` @ 0x5970d5). Helpers (ELF / exe):
`FlottaMozgat` 0x616af0 / 0x53d270, `FlottaAddWaypoint` 0x617830 / 0x53cf70, `FlottaTovabbMozgat`
0x617a90, `FlottaRemoveNextWaypoint` 0x617b60, `FlottaRemoveAllWaypoint` 0x617990 / 0x53dba0,
`GetFreeWaypoint` 0x617d30 / 0x53e3d0, `SearchFlottahozKozeliBolygo` 0x616110 / 0x5411a0,
`SearchBuilding` 0x616720 / 0x541150, `RndInitValasztas`/`RndValasztas` exe 0x540ef0 / 0x540f10.
**The exe is normative.** No rule values here; inline code constants are listed.

`FRISSIT_Starmap` runs on every 20 Hz sim tick while strategic time runs (`time-and-rng.md` §1).

## 1. Data
### 1.1 Fleet (`FlottaTipus`, 0x80 bytes, `Flottak[1..=OsszHanyFlotta]`, exe pointer 0xBB40A4, count 0xBB40A8)
| Offset | Type | Field |
|---|---|---|
| +0x00 | char[21] | name (`+0x52` bit11 = differs from the default name `AlapFlottaNev`, string 17/5/46) |
| +0x15 | u8 | owner empire; 0xFF = deleted slot (the rest of the record stays and is still read, §4.1). Empire 9 = traders/neutral |
| +0x16 | i16 | story/script id (0 = none; 10000..=10999 = trader) |
| +0x18 | u8 | bit0: hidden/inactive (no radar, can't be tracked; for traders "has traded"); bit1: delete on arrival (scripted) |
| +0x19 + e | u8 | visible to empire `e` (written by the radar pass, §6) |
| +0x30 / +0x34 | f32 | position x / y |
| +0x38 | f32 | speed (refreshed every 10th tick, §3) |
| +0x3c | f32 | speed override (> 0 replaces the computed speed) |
| +0x40 | i16 | order: 0 none, 1 move, 2 attack (13/14 are only `FlottaMozgat` arguments, §2) |
| +0x42 | i16 | target kind: 0 point, 1 planet, 2 fleet |
| +0x44 / +0x48 | f32 | target point; for kinds 1/2, `+0x44` holds the planet/fleet index as a float |
| +0x4c / +0x4e | i16 | colony ships / terraformers aboard |
| +0x50 | i16 | first waypoint (0 = none) |
| +0x52 | u16 | flags: bit0 in battle · bit1 patrol (loop waypoints) · bit2 carries a virus · bit3 colonising · bit4 terraforming · bit5 ignored by radar · bit6 story radar notice · bit8 always visible · bit10 wormhole · bit11 custom name · bit12 stealth · bit14 story-protected |
| +0x54 | i32 | ship count (fleets with < 1 and not wormhole/protected are deleted, AI only) |
| +0x60 | i32 | fleet strength (merge choice, §4.3) |
| +0x7c | f32 | radar range |

### 1.2 Waypoint (12 bytes, pool `Waypointek`, exe pointer 0xBB41A4, count 0xBB60F0)
`+0 f32 x`, `+4 f32 y`, `+8 i16 next` (0 = last, −1 = free). `GetFreeWaypoint` returns the first free
node (1-based), else appends one.

### 1.3 Ship (6 bytes, `Hajok`, exe pointer 0xBB4100, count 0xBB4104)
`+0 i16 fleet`, `+2 i16 design` (0x7FFF = none). Designs: `Tervek[empire]` (exe pointer array 0xB9AA18),
0x84 bytes: `+0x3c f32 speed`, `+0x48 i32 type` (80 = slow class, speed × 0.25), `+0x6a u8 radar`,
`+0x6b u8 stealth` (both §6).

## 2. Orders (`FlottaMozgat(f, order, kind, x, y)`)
- `order` 13 or 14 (add waypoint) on a fleet with `order 1, kind 0`: append `(x, y)` to its waypoint chain
  (`FlottaAddWaypoint`; 14 also sets the patrol flag) and return. Otherwise 13/14 become a plain move (1).
- Else: if the fleet was colonising (bit3) or terraforming (bit4), give the colony ship/terraformer back
  to the empire's pool (`OsszHanyKolonizalo`/`OsszHanyTerraformalo` += 1) and clear the bit. Human owner,
  or ship count == colonisers + terraformers: clear bit5. Store `order, kind, x, y`, drop all waypoints
  (and the patrol flag).
- `FlottaAddWaypoint` on a fleet with no order is a plain move to the point.
- `FlottaTovabbMozgat` (on arrival): if it has waypoints and is a point move, the next waypoint becomes the
  target and is removed from the head of the chain; with the patrol flag the removed node is re-appended
  at the tail, holding the fleet's **current position** (so a patrol returns to where each leg ended).
  Otherwise the order is cleared (`FlottaMozgat(f, 0, 0, 0, 0)`).

## 3. Speed refresh (every tick with `Ido2 % 10 == 1`)
For every fleet: `speed = 0`, `radar = 0`, clear "in battle" and bit12. Then for every ship with a design:
```
v = design.speed;  if design.type == 80: v *= 0.25       (skipped when v <= 0 and type != 80)
if (f64)fleet.speed <= 1e-5: fleet.speed = v            (1e-5 is a double: comisd [0x85da60])
fleet.speed = override > 0 ? override : min(v, fleet.speed)
```
(Then debug ×100 when the debug-speed global is set.) After the radar pass, a fleet with `speed <= 0`
gets `0.06`. Ships are visited in ship-index order. The same pass sets radar ranges and bit12 (§6).

## 4. Movement (every tick, fleets in index order)
A battle started by an arrival (§4.3) stops the loop: **later fleets don't move this tick.**
### 4.1 Order upkeep (before moving)
- Fleet target (kind 2) deleted → clear the order. The visibility test below still runs, on the freed
  slot's **stale record** (`FlottaDelete` only sets the owner byte), so usually nothing more happens.
- Fleet target no longer visible to the owner (`target.visible[owner] == 0`, not always-visible (bit8 or the
  story "all fleets visible" flag) — or the target is hidden (+0x18 bit0)), SP owner < 9, not debug:
  convert to a move to the target's current position (`FlottaMozgat(f, 1, 0, tx, ty)`; "contact lost"
  message to the human). MP: the human owner sends that order over the network; AI owners do it locally.
- Attack on a planet the owner now holds → clear. Colonising/terraforming fleet moving to a planet that
  became owned → clear (§2 returns the ship to the pool).
- Owner 1..=8 no longer alive (`BirodalomEl`) → `FlottaDelete`.
### 4.2 Step
Only if `order > 0` and `speed > 0`; a fleet in battle (bit0) instead gets its order cleared.
```
step = (f32)((f64)speed × 0.3)               (f32 elsewhere; the constants 0.3, 0.003 and 0.0001 are doubles)
kind 0: aim = cur = (x, y) of the order
kind 1: aim = cur = planet (x, y + 2.0)
kind 2: step = (f32)((f64)step + 0.003);  cur = target fleet position;  aim = cur, plus a lead when it moves
        (order != 0 and speed > 0): dest = target's own target (point / planet without +2 / fleet),
        ts = (f32)((f64)target.speed × 0.3),  dd = |dest − cur|,  me = |cur − pos|
        k = min(dd / ts, (me / step) × 0.5);  if (f64)dd > 0.0001: aim = cur + (k·ts·(dest − cur))/dd
d   = |cur − pos|,  da = |aim − pos|;  if da == 0: aim = cur, da = d
reach = (f32)IdoSpeedSzorzo × step          (read per fleet; the radar pass may have just lowered it, §6.3 step 6)
if d <= reach or (kind 2 and d <= 1.0):  pos = cur, then arrival (§4.3)
else:                                     pos += ((aim − pos) × reach) / da      (per axis)
```
`|v|` = `(f32)sqrt((f64)(dy·dy + dx·dx))`, the sum in f32.

### 4.3 Arrival
In this order of precedence:
(First, every arrival records the fleet for the story, `0x5c8cb0(f)`: `StoryAdat` arrival fields, a flag.)
1. **Colonising** (bit3), `order 1, kind 1` to a planet with owner 0 (exe 0x59649d..0x596541):
   `BolygoElfoglal(p, owner)`, `Generate_Planet0(p, owner, PlayerFaj[owner], false)`, `d = rndlog(1, 0, 0)`,
   `EpuletNewSearchLocation(p, 1, false, d, −1, −1)` (colony hub under construction), story special 10,
   `FlottaDelete(f)` (`colony-side.md` §5). All of it **inside the movement loop**: fleets moved later
   in the same tick see the planet owned (another colonist to it no longer colonises; upkeep §4.1).
2. **Terraforming** (bit4), `order 1, kind 1` to a planet with owner 0 (0x596546..0x5968fa): the surface
   `+0x22` = the first `s` in 1..=8 with the largest `FajBolygoViszony[owner][s]` — **indexed by the owner
   empire number, not its race** (`lea (owner, owner, 8)`; 15 × 9 table 0x90b9d8) — and the colony map
   `+0x23` = `NextColonyMap(s − 1)` (`colony-side.md` §5.1); then the star sprites are rebuilt (UI only:
   the loop pumps frames through `Fake_FRISSITES` with 0xb79f78 = 1), story special 0x18, `FlottaDelete(f)`.
   Nothing else of the planet changes (it stays unowned).
   Cases 1–2 are verified end to end (`traces/2026-10-15-arrivals.md`): `ig2-sim` runs them inside
   `Starmap::tick` through the `Arrivals` hook (`colony_side::ColonyArrivals`).
3. **Scripted** (+0x18 bit1): story event 0x10, fleet deleted.

The arrivals' story calls go **inside the loop**, before the fleet is deleted: colonisation
`Story_Special(10, owner, p)`, terraforming `Story_Special(0x18, owner, p)`, scripted `Story_Special(0x10, id, f)`,
a wormhole trip `Story_Special(0x14, owner, 1, f, OsszHanyWormhole)` (the last argument is the wormhole loop's
last index: the exe copies the counter to a local before each increment, 0x596c71; with no wormhole the local is
never written and the call passes an uninitialised stack word [original quirk]; the ELF's counter after the loop
would be `+ 1`; verified, `traces/2026-10-16-wormholes.md`) and an attack on a protected target `Story_Special(0x12, t, 2, f)`
(fleet) / `Story_Special(0x12, f, 1, p)` (planet); then `Story_FleetArrivedCheck` at the end of the fleet's step.
4. Otherwise:
   - move to a planet: virus bookkeeping (bit2: picks up a virus at a planet with an epidemic; spreads it
     via `Virus_CheckTerjedes` at an own planet, or any owned planet for empire 9); then **merge** into the
     strongest (`+0x60`) idle fleet of the same owner within Manhattan distance < 10 that isn't
     colonising/terraforming/hidden (owners 9 and 10 never merge) — `FlottaOsszevon(other, f)`.
     A merge keeps the larger radar range; a default-named destination takes the source's name and
     story id, and bit11 is recomputed from the name; followers of the source now follow the
     destination; the ships and vehicles move over; the source slot is freed (§4.6).
   - move/attack a **wormhole** fleet (bit10): teleport to the other end, both ends become visible, radar 0.
   - attack a fleet (not protected, not a wormhole): `SpaceWarIndit` — battle, stop the loop.
   - attack a planet (not protected; exe 0x596d61..0x596e8e): planet events −0x1F90/−0x2031 (`BolygoInfoIs`) →
     `GroundWarIndit(f, p)`; else `d = SearchBolygohozKozeliFlotta(p, 15.0, own, idle, 0, 0, 0)` (the owner's
     nearest idle fleet, `production-research.md` §4) and, if `d == 0`, no defence ship (`+0x114 == 0`) and no
     finished working building (`SearchBuilding(p, k, 1)`) of kinds 0x25, 0x26, 0x24, 0x1F, 0x1E, 0x1D, 0x1C →
     `GroundWarIndit(f, p)` (which does nothing without ground vehicles), else `SpaceWarIndit(f, d, p)`. Stop the
     loop.
   - The battle runs inside the step (`space-combat.md` §4, `ground-combat.md` §2); afterwards the step goes on
     with the record as the battle left it (a destroyed fleet skips the rest; a deleted target stopped its
     followers), and the position the step computed is written back even over the loser's retreat
     (`FlottaArrebPakol` moved f1; the step's locals win: the write at 0x59705d is unconditional). Then, as after every arrival,
     `Story_SpaceBattleFinishedCheck`, `Story_FleetArrivedCheck` and `Story_GroundBattleFinishedCheck` (exe
     0x5c9200, 0x5c8c90, 0x5c8d80; the battle's own story check has usually cleared their flags). Port:
     `Starmap::tick_until_battle` pauses at the battle, `Game` runs it (`game::battles`) and `Starmap::resume` ends
     the step; production doesn't run in that tick.
   - move to an own fleet: merge into it.
   - then: traders (id 10000..=10999) are paid (below) and stop; other fleets `FlottaTovabbMozgat` unless
     the order was "move to a (non-wormhole) fleet", which keeps following.
   **Trader income** (exe 0x596f09..0x596fdc, `starmap.cpp` line 0x2512): unless the trader has already traded
   (+0x18 bit0): nearest planet `n` (§5, limit 1e6), `o = owner(n)` (0 for an unowned planet: it is booked to
   empire 0 all the same), centres = working buildings of kind 48, banks = kind 49 (`0x541150(n, kind, 1)`),
   ```
   amount = (int)(((f64)min(banks, 1) × 0.5 + 1.0) × (f64)(centres × 10000) × (f64)(PlayerFaj[o] == 3 ? 2 : 1))
   MoneyHozzaadLog(o, 0, (f64)amount)          // 0x53fb10 → MoneyHozzaadOld: pool 0, split by the shares;
                                               // an AI's income × NehezsegPenzSzorzo (× 0.5 in MP), colony-side.md §6.4
   TradeIncomeThisYear[o] += amount            // i32 at 0xBAE150 + 4·o, the unscaled amount
   ```
   all f64 (`cvtdq2pd`/`mulsd`, truncated by `cvttsd2si`). The call happens even for `amount = 0`, which takes
   `MoneyHozzaadOld`'s spending branch with `a = 0` (pools in debt are re-split by the shares, `Money` re-summed). Nothing later in the
   fleet loop reads money; the production block after it does (pool 1), so the booking must land before it.
   Port: `Arrivals::trader_paid` (`ColonyArrivals` books it into the colony world), plus `StarmapEvent::TraderPaid`.
   The trader gets +0x18 bit0 and stops **where it was** (it doesn't snap to the planet).

### 4.4 Idle traders (after the step)
A trader (owner 9, id 10000..=10999) with no order picks a destination: if it has traded (+0x18 bit0)
only with a 1% chance (`rndlog(14, 0, 100) == 0`). `n` = nearest planet (§5); clear bit0; then a weighted
pick on stream 14 (`RndInitValasztas`, `RndValasztas(14, w)`, `time-and-rng.md` §3.1) over planets `p`
owned by empires 1..=8, `p != n`:
```
a = owner(n) (or owner(p) if n is unowned), b = owner(p)
level = Viszony[a][b].+0x25, ack = Viszony[a][b].+0x24 bit0 + 1     (both 1 if a or b > 8)
w = (900 − (int)sqrt_f64(dx² + dy²)) × (trade_ports(p) > 0 ? 3 : 1) × ack × level × 3
```
The last accepted `p` becomes a move-to-planet order. (Weights can be ≤ 0 far away; `RndValasztas` then
never accepts.)
### 4.5 Clean-up
A fleet with ship count < 1 that isn't a wormhole/protected (`+0x53 & 0x44 == 0`): the human's is
reported over the network (MP), an AI's is deleted.

### 4.6 Merges and deletions: the units
Evidence: ELF `FlottaOsszevon` 0x617000 / exe 0x53d860, `HajoAtpakol` 0x617540 / exe 0x53e510, `TankAtpakol`
0x617700 / exe 0x541410, `FlottaDelete` 0x6167d0 / exe 0x53d040 (exe = ELF, no inlining differences).

**`FlottaOsszevon(into, from)`** (both in 1..=`OsszHanyFlotta`, distinct), in this order:
1. radar `+0x7c` = the larger; `into` named exactly `AlapFlottaNev` → `strcpy` from `from` (up to and including the
   NUL: the bytes after it stay); story id from `from` if `into`'s is 0; bit 11 = name ≠ `AlapFlottaNev`.
2. Fleets `g` ≠ `into`, `from` (live, order > 0, kind 2, target `from`) → `FlottaMozgat(g, g.order, 2, into)`,
   keeping their bit 5.
3. Every ship of `from` in ship order → `HajoAtpakol(i, into)`: the ship's fleet becomes `into`;
   `HajoFlottahozHozzaszamol(from, d, −1)` then `(into, d, +1)` (`space-combat.md` §3: totals `+0x54..+0x78`, colony
   ships `+0x4c` / terraformers `+0x4e` with their pools ±1, design 28's flag). The starmap's transfer screen
   flags (`StarmapTransferValtozott*`, `StarmapAktFlottaValtozott`) are UI only.
4. Every vehicle entry `k` of `from` in entry order → `TankAtpakol(from, into, d_k, count_k)` = `TankDelete(from, d,
   n)` (the lowest-index entry of `from` with design `d`) + `TankNew(into, d, n)` (`production-research.md` §5.1:
   the last live entry of `into` with `d` grows, else the last free entry, else a new one).
5. `OsszHanySajatFlotta[from.owner] −= 1`, `from.owner = 0xFF`.
6. `into`'s bit 5 is cleared for a human owner (`PlayerKi == 1`) and when colony ships + terraformers == ships
   (**after** the transfer: `+0x54` is the merged count).
7. `AktFleet` (the selection, saved) == `from` → `into`; likewise the transfer screen's two fleets (UI).

Later fleets of the same tick see the merged strengths (`+0x60`, the next merge choice) and counts (clean-up §4.5).

**`FlottaDelete(f)`** (not a wormhole): followers' orders cleared; every ship of `f` → `HajoDelete` (the ship
goes, its sums leave `f`, vehicles beyond the capacity go; an emptied **AI** fleet is deleted **from inside**:
the last `HajoDelete` re-enters `FlottaDelete(f)`, which deletes the remaining vehicles, counts the owner down
and sets the owner to 0xFF); every vehicle entry → `TankDelete`; `OsszHanySajatFlotta[owner] −= 1` with the owner
read **now**: after the re-entry that is 0xFF, whose counter is exe 0xBB40B0 + 0x3FC = 0xBB44AC =
`Tankok[96].count` [original bug]; the owner = 0xFF. So a colony ship's or terraformer's arrival, a scripted
arrival and a dead empire's fleet (§4.1) take their ships with them. `HajoDelete` takes the colony ship
out of the pool once more (`OsszHanyKolonizalo −= 1`; the colonising order had already done so, §2), and so the
pools of a colonising or terraforming empire run one below the count until the next refresh (§6.3 step 2).

**`LoadSave`** (exe 0x543c80) ends, after loading **and after writing** a save, with `KemKuldetesek[0x160] = 0`,
`FRISSITES_IGAZIT()` (UI) and `FlottakUjraSzamol()` (`ai-empire.md` §12.9: every total, colony-ship and terraformer
count, pool and garrison recounted from the ships and vehicles). Saving therefore changes the game: the
pools count colonising fleets' ships again. The save replay models it (`Game::saved`) after every traced save.

## 5. Nearest planet (`SearchFlottahozKozeliBolygo(f, limit, own_only, skip_protected)`)
Manhattan distance `|dx| + |dy|` (f32), strictly smaller than the running best (initially `limit`), over
planets 1..=n in order; 0 if the fleet's owner is 0.

## 6. Radar and visibility
Evidence: ELF `FRISSIT_Starmap` (the `Ido2 % 10 == 1` block before the space-war loop) and
`GetENRadarKifejlesztve` @ 0x61bdb0; exe `FRISSIT_Starmap` 0x594ae0..0x595c00 and `GetENRadarKifejlesztve`
= **0x53e020** (called first thing, every tick). Story hooks: `Story_ArrivedToRadarRangeFleet` exe 0x5c55f0,
`Story_ArrivedToRadarRangeBolygo` exe 0x5c55d0. **Precision below is the exe's** (asm checked: `mulss`
unless marked f64).

### 6.1 Data
| Global (ELF name) | exe VA | Layout | Meaning |
|---|---|---|---|
| `RadarHatotav` | 0x913898 | f32 × 6 (compiled in) | radar range per radar tech level 0..=5 |
| `RadarHatotav2Init` | 0x9138b0 | f32 × 8 (compiled in, right after `RadarHatotav`) | per-level planet radar range, copied to every empire at new game (`InitNewGame`) |
| `RadarHatotav2` | 0xbb61a8 | f32 [19][8] | per-empire planet radar range by planet radar level; `[e][1]` is rewritten every tick (§6.2); row 0 = copy of the human's row |
| `RadarHatotav2Negyzet` | 0xbb6408 | f32 [19][8] | squares of the above (`[e][k≠1]` set at new game, `[e][1]` every tick) |
| `RadarHatotavSzorzo` / `…Negyzet` | 0xbb6668 / 0xbb66b8 | f32 [19] | per-empire radar multiplier and its square |
| `BirodalomIsmeri` | 0xbaebb8 | u8 [19][19] | `[e][o]`: empire `e` has met empire `o` |
| `OsszHanyKolonizalo` / `…Terraformalo` | 0xbb4108 / 0xbb4158 | i32 [19] | idle colony ships / terraformers aboard fleets |
| `ProgramPlayers` / `ProgramPlayers2` | 0x912acc / 0x912ad0 | i32 | empires that can discover (8) / empires in the visibility and alliance loops (16 in quick.scn) |
| `Wormhole` / `OsszHanyWormhole` | 0xbb60f8 / 0xbb61a0 | 0x1c B, 1-based, ≤ 5 | `+0/+4` f32 end A, `+8 u8` end A revealed, `+10 i16` end A fleet, `+0xc/+0x10` end B, `+0x14 u8`, `+0x16 i16` end B fleet, `+0x18..+0x1a` u8 tint (below) |
| `StoryAdat` | 0xf273f8 | | `+0x28` tracked story fleet (`T`), `+0x38` human radar bonus, `+0x48` no supply limit, `+0x50` supply planet |
| tutorial | 0xf6915d / 0xf66f2a / 0xf66f40 | u8 / u8 / i32 | active, paused, current event |

Planet (`Bolygok`, 0x188 B): `+0x18/+0x1c` position, `+0x20` owner, **`+0x6e` radar level** (0 none,
1 colony hub, ≥ 2 set by scenarios/story: `.scn` save key, `st_Elem` sets 5), **`+0xbb`** story "notify
when seen", **`+0xd8 + 2e` i16 visibility per empire** (never cleared by this pass). Fleet: `+0x19 + e`
visibility, `+0x52` bits 0/5/6/12, `+0x7c` radar range (§1.1). Design: `+0x6a` radar tech level,
`+0x6b` stealth.

### 6.2 Radar tables (`GetENRadarKifejlesztve`, every tick)
For `e` in 1..=`ProgramPlayers2`:
- `lvl` = 0; for `c` in 1..=4: if tech `0x20 + c` is developed by `e` (live tech table +0x45+e == 5):
  `lvl = c + (e == 2)` (empire **2**, the literal index, gets one level more once it has any radar).
- SP and `StoryAdat+0x38`: the human's `lvl += 1.0` (applied once, to the human's row, after the loop).
- `RadarHatotav2[e][1] = RadarHatotav[(int)lvl]` (index 6 reads the next word, `RadarHatotav2Init[0]`),
  `…Negyzet[e][1] = r × r` (f32).
- `Szorzo[e]` = 1.0 if `e` is human-controlled (`PlayerKi[e] == 1`) or allied (`Viszony[e][j].+0x25 == 5`)
  with a human-controlled `j` (1..=`ProgramPlayers2`), else **1.5**; `SzorzoNegyzet[e] = s × s`.
Then row 0 of all four tables = the human's row. The function returns the human's level (unused).

### 6.3 Refresh tick (`Ido2 % 10 == 1`), in this order
1. **Planet radar level**: every planet with `+0x6e < 2` gets 1 if it has an owner `o`, its own visibility
   `vis[p][o] >= 0` (i16), and a building slot `j` in 1..=n with type 1 (colony hub) and `+8 == 0`
   (finished; working or not), else 0.
2. **Fleet reset** (live fleets in index order): clear `+0x52` bit0 (in battle) and bit12 (stealth),
   `speed = 0`, `radar = 0`. The tracked story fleet `T` (if not hidden, `+0x18` bit0) gets
   `radar = RadarHatotav[2] × 0.25`. Fleets not colonising (bit3) add their colony ships `+0x4c` to
   `OsszHanyKolonizalo[owner]`, not terraforming (bit4) their terraformers `+0x4e` to `…Terraformalo`
   (both zeroed first, 1..=18). Fleets with story id 1011, 1021 or 1311 are **radar boosters** (`B`, in order).
3. **Ships** (in ship order; only ships whose design speed is > 0 or type 80, §3): speed as §3 (the
   `fleet.speed <= 1e-5` test is f64); then, unless the fleet is hidden,
   `radar = max(radar, RadarHatotav[design.radar] × 0.5 × Szorzo[owner])` (hidden: `radar = 0`);
   bit12 |= `design.stealth != 0`.
4. **Supply** (live fleets): `D` = 1e20 (f32); if `StoryAdat+0x48 == 0` and the fleet isn't `T`: `D` =
   min over planets `p` with ((`p.owner == owner` or `Viszony[owner][p.owner].+0x25 == 5`) and level ≠ 0)
   or `p == StoryAdat+0x50` of `dy² + dx²`; else `D = 0`. Then for each booster `b`:
   `D = min(D, (f32)((f64)(dy² × 2.0 + dx²) + (f64)RadarHatotav2[owner][1] × 1.5))` (the only f64 step;
   note y is doubled). `R = SzorzoNegyzet[owner] × Negyzet[owner][1]`. If `D > 4R`: `D <= 9R` →
   `radar = (3.0 − (f32)sqrt((f64)(D / R))) × radar`, else `radar = 0`.
5. **Alliances**: for `a`, `j` in 1..=`ProgramPlayers2`: `allies[a]` = the `j` with `Viszony[a][j].+0x25 == 5`
   or `j == a`, in order; `n[a]` its length (0 for `a` outside the range).
6. **Fleet visibility** (live fleets `f` in order; `EN` = human empire):
   - `speed <= 0` → `0.06` (§3). `old` = `vis[0..19]`; `vis[1..=ProgramPlayers2] = 0`; `vis[owner] = 1`.
   - Planets `p` with level `L ≠ 0`, `o = p.owner` (SP: 0 → `EN`): `r = Negyzet[o][L] × SzorzoNegyzet[o]`;
     a stealth fleet (bit12) against `RadarHatotav2[o][1] < RadarHatotav[4]` gets `r / 25.0`. If
     `dy² + dx² < r`: for `k` in 1..=`n[o]` set `vis[allies[o][k]] = 1`; then in SP also `vis[EN] = 1`
     if `L > 1` and the distance is within the human's own level-`L` circle (`Negyzet[EN][L] ×
     SzorzoNegyzet[EN]`, no stealth division). So scenario radar stations (level ≥ 2) of **any**
     empire also show their surroundings to the human.
   - Fleets `g` (live, not bit5): `r = radar_g²` (/25 as above with `g`'s owner); `dx² + dy² < r` →
     `vis[g.owner] = 1`, and `vis[EN] = 1` if `g == T`.
   - `f == T` and not hidden → `vis[EN] = 1`.
   - Every wormhole: end A revealed → `vis[EN] = 1` on fleet A, likewise B (written into those fleets'
     records, which are reset again when the loop reaches them).
   - **Enemy approaching**: newly `vis[EN]` (`old[EN] == 0`), order attack (2) with `+0x44 > 0`, target a
     planet (kind 1) or fleet (kind 2) owned by `EN`, owner ≠ 0, story id ≠ 0x1F88 → message 0x0E and
     slow-down (`SetIdoLelassit(1)`, call @ 0x595570). `SetIdoLelassit` (0x541270) writes
     `IdoSpeedSzorzo` at once (SP: 1 unless `nemlehetidotallitani` 0xBAD0D4; MP: ×4/×8/×16 from
     `ProgramSebesseg` 0xBAE54C, 4 = unchanged), and the movement step (§4.2) reloads the global for every
     fleet (`movd 0xbae25c` @ 0x596424): **the same tick's movement already runs at the slowed speed**,
     for every fleet (earlier ones included, the radar block precedes the movement loop).
   - For `e` in 1..=`ProgramPlayers`: if `vis[e]` and (`old[e] == 0` or bit6): bit6 and `e == EN` → clear
     bit6, `Story_ArrivedToRadarRangeFleet(f)`. If `!BirodalomIsmeri[e][owner]`: for `EN` story event
     0x0B(`e`, owner) and `FelfedezBirodalom(owner)` (adds the empire to the diplomacy list; message 0x1A
     for 1..=8); set it.
7. **Planet visibility** (planets `p` in order; `vis` = `p+0xd8`, `notice` = `p+0xbb`):
   - Radar planets `q` (level `L ≠ 0`, `o = q.owner`, SP: 0 → `EN`) are skipped unless `vis[o] == 0` or
     (`o == EN` and `notice`) or `n[o] >= 2` or `L >= 2`. In range (`dy² + dx² < Negyzet[o][L] ×
     SzorzoNegyzet[o]`, no stealth): for each `e` as in step 6 (allies of `o`, then `EN` in SP under the
     same level/own-circle test) with `vis[e] == 0` or (`e == EN` and `notice`): `vis[e] = 1`; `e == EN`
     with `notice` → clear it, `Story_ArrivedToRadarRangeBolygo(p)`; `e > 0`, `p` owned, not met → meet.
   - Fleets `g` (live, not bit5) with `vis[g.owner] == 0` or (`g.owner == EN` and `notice`) or `g == T`,
     and `dy² + dx² < radar_g²`: `vis[g.owner] = 1`; tutorial event 1 pending → the planet gets its name
     (string 2/4/6) and `TutorialCheckEvent(1)`; notice as above; owner ≠ 0 meets `p.owner`. `g == T`:
     `vis[EN] = 1` and the human meets `p.owner`.
8. (After the block, every tick.) Fleets whose ships sit in an active space war get bit0 (in battle).

"Meet" = `BirodalomIsmeri[e][o] = 1`, with story event 0x0B and `FelfedezBirodalom` when `e == EN`. The
exe tests all these byte flags with `!= 0` (the ELF with `& 1`).

## 7. Production (every tick, after the clean-up)
Specified and verified in `production-research.md` §3–§5 (`ig2-sim::production`).

## 8. RNG
Stream 14: trader rolls only (§4.4). The exe's colonisation adds one `rndlog(1, 0, 0)` (§4.3).

## 9. Exe vs ELF
Same algorithm. Differences: the exe inlines `fMindigLathato` as `+0x53 bit0 || Story_Minden_Flotta_Latszik`
(0xF265D4); trader distances use an f64 `sqrt` truncated (ELF: `sqrtf`); colonisation makes the extra
calls in §4.3. Radar (§6): the exe tests the byte flags (visibility, `BirodalomIsmeri`, `+0xbb`, wormhole
reveals, design stealth) with `!= 0` where the ELF uses `& 1`; the supply fade multiplies in f32
(`(3.0f − (f32)sqrt) × radar`; ELF: f64); `GetENRadarKifejlesztve` is unrolled but equivalent.

## 10. Not ported
The space-war in-battle marking (§6.3 step 8: an input, the list of fleets in battle),
and side effects owned by other systems, reported as events: arrivals (battles, virus spread) and radar consequences (story event 0x0B / `FelfedezBirodalom` on
meeting an empire, `Story_ArrivedToRadarRange*`, the "enemy approaching" message, the tutorial rename).

### Opening a wormhole (`AddWormhole(x1, y1, x2, y2)`, exe 0x53aec0)
Called by stories 902 and 239 (`campaign.md`). With `OsszHanyWormhole > 4` it returns 0; else `w = ++OsszHanyWormhole`,
end A = `((f32)x1, (f32)y1)` not revealed, its fleet `FlottaNew("- Wormhole -", owner 0, id 0, (f32)x1, (f32)y1, 0)`
with `+0x53 |= 4` (bit 10, wormhole) and `|= 2` (bit 9); end B likewise from `(x2, y2)`. The tint
`+0x18 + i = 255 − (255 − c[w][i]) / 2` (C division, stored as a byte) from `i32 [6][3]` at exe 0x913850 (the
original's colours, just before `RadarHatotav`); then four `WormHole` scene objects (render only: both ends, two
colour orders). Returns `w`. Ported as `Game::add_wormhole`, which carries out the stories' `StoryEvent::Wormhole`;
story 239's follow-up (`StoryEvent::RevealWormhole`, exe 0x5ceb86: when `w ≠ 0`, `+8`/`+0x14` := 1 and both fleets'
`+0x53 |= 1`) is `Game::reveal_wormhole`. Verified byte-exact against the exe (`traces/2026-10-16-addwormhole.md`).

## 11. Verification
`traces/2026-10-15-fleet.md`: Frida before/after snapshots of every `FRISSIT_Starmap` tick on the GOG
exe (plain quick.scn and a fuzzed run with random orders, waypoints, patrols, follow orders, speed
overrides and trade buildings), replayed by `engine/crates/ig2-sim/tests/fleet_trace.rs`. The double
constants of §4.2 were found by this check (f32 0.3 left 1-ulp position errors).
`traces/2026-10-15-radar.md`: the same traces recaptured with the radar state (tables, planet radar
levels/visibility/notices, `BirodalomIsmeri`, pools, radar techs) and a `--fuzz-radar` run (alliances,
owners, radar stations, stealth and radar designs, boosters, story fields); §6 is computed, not copied.
`traces/2026-10-15-radar.md` §"Enemy approaching": `run_fleet.py --fuzz-approach` (×16, staged unseen
attacks on the human on refresh ticks, sometimes with the SP speed lock) pins the same-tick slow-down of
§6.3 step 6: 57 slow-downs and the locked ticks replay exactly; keeping the old speed gives 279 mismatches.
Merges, colonisation, terraforming and deletions with their units (§4.3, §4.6) are verified byte-exact in the save
replay (`traces/2026-10-16-merges.md`: `run_replay.py --merges`, 36 ticks: 7 merges with vehicles, a name copy, a
retargeted follower, colonisation and terraforming by the human and the AI, a dead empire's fleet).
`ig2-sim` runs them inside `Starmap::tick` through `Arrivals::merge_units` / `Arrivals::delete_units`
(`game::StarmapHooks` on the combat view: `CombatWorld::merge_units`, `delete_ship`). Wormhole trips are verified in
the save replay (`traces/2026-10-16-wormholes.md`: `run_replay.py --wormholes` opens a wormhole with the exe's own
`AddWormhole` 0x53aec0 and sends three fleets through it, moving and attacking, from both ends; it corrected the
story special's last argument, §4.3).
The f64 booster sum (§6.3 step 4) is pinned by the asm (`cvtss2sd`/`addsd`/`cvtsd2ss` @ 0x594fc2..0x594fca) and
provably equal to an f32 sum on every reachable input (`traces/2026-10-16-mp-radar.md`, "Booster sum"). The MP branches
of §6 are verified by staging `MultiPlayerMode` during refresh ticks (`run_fleet.py --mp-radar`, same report).
