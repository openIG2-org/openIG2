# Space combat: hulls, weapons, designs, routing, auto-resolve and the in-battle mark

Status: **§2–§6 verified at runtime** (§8, `traces/2026-10-15-spacewar.md`) and ported to
`engine/crates/ig2-sim/src/spacewar.rs`; §7 (the real-time battle, `SpwC::*`) is specified from the
decompile only and not ported. **The exe is normative**: precision below is the exe's (asm checked;
`mulss` = f32, `mulsd` = f64). No rule values here (the tables are read from the user's exe,
`ig2-rules`); inline code constants are listed.

Evidence (ELF `ig2` -O0 / shipped exe):

| Function | ELF | exe | Notes |
|---|---|---|---|
| `TervUjraSzamol(empire, d, trade, price)` | 0x61b5f0 | 0x5423b0 | design stats |
| `GetGyartasAr` / `GetGyartasIdo` | 0x4faff0 / 0x4fb100 | 0x4acaf0 / 0x4acb80 | race price/time rules |
| `GetMaxPodok(table, hull)` | 0x4fb390 | 0x4b4290 | slot count |
| `HajoFlottahozHozzaszamol` / `TankFlottahozHozzaszamol` | 0x6180f0 / 0x6185f0 | 0x53e740 / 0x5415d0 | fleet sums |
| `FlottakUjraSzamol` | 0x618b40 | 0x53dd90 | recount all fleets |
| `HajoDelete` / `TankDelete` | 0x616cc0 / — | 0x53e5d0 / 0x5414c0 (core 0x541510) | |
| `FlottaDelete` | 0x6167d0 | 0x53d040 | |
| `SpaceWarIndit(f1, f2, planet)` | 0x712920 | 0x5a7670 | routing |
| `SimulateSpBattle(f1, f2, planet)` | 0x70f710 | 0x5a5e60 | auto-resolve (`starmap.cpp` 0x1f09..0x1fb3) |
| `FlottaArrebPakol(f)` | 0x70f5b0 | inlined in 0x5a5e60 | loser's retreat |
| `SpaceWar_NewBattle` | 0x67b740 | 0x563700 | real-time battle slot |
| `fegyverszamol` | 0x68c5c0 | | weapon damage table at start-up |
| `FRISSIT_Starmap` marking loop | 0x7132b0 (l. 598) | 0x594ae0 (after the radar block) | §6 |
| `SpwC::*` (130 functions) | 0x6851d0..0x6b8670 | partly ported names (`re/symbols/`) | §7 |

## 1. Units
Ships live in `Hajok` (6 B: `+0 i16 fleet`, `+2 i16 design`, 0x7FFF = free; exe pointer 0xBB4100,
count 0xBB4104). Vehicles (fighters and ground units) live in `Tankok` (8 B: `+0 i16 fleet`,
`+2 i16 design`, `+4 i32 count`; a static 1-based array at exe 0xBB41A8, count 0xBB4154, ≤ 1000
entries; fleet ≤ 0 = held by planet −fleet). Designs are per empire: `Tervek[e]` (exe pointer array
0xB9AA18, `OsszHanyTerv[e]` 0xB9C028; 1-based). Planets carry up to 10 defence ships
(`Bolygok +0xfe + 2k`, k = 1..=`+0x114`, design ids of the planet owner) and defence buildings
(types 28..=31, "planet guns"). `BolygoMennyiIonGunEro[p]` (exe 0xB99FE0, i32) is the planet's ion-gun
strength.

## 2. Tables
### 2.1 Hull (`Alapok`, exe 0x90BBF8, 36 B × 25)
| Offset | Type | Field | Evidence |
|---|---|---|---|
| +0 | i16 | class: 0 none, 1 space (ships and fighters), 2 ground vehicle | `TervUjraSzamol` → design `+0x16` |
| +2..+7 | u8 × 6 | colony ship, terraformer, fighter, space base, movable base, satellite | → design `+0x34..+0x39` (exe copies the raw bytes; ELF `& 1`) |
| +8 | i32 | hull id (= index) | static |
| +0xc | i32 | the hull's tech: base price/time (`GetGyartasAr/Ido`), firepower (`TalalmanyokrEro`), hit points (tech `+32`), capacity (tech `+37`) | `TervUjraSzamol` |
| +0x10..+0x23 | | [unk; read by the design screen `Design_Main`, `FRISSIT_MI_DESIGN`, the spy screens] | xrefs |

### 2.2 Space weapon (`fegyverek`, exe 0x91D858, 92 B × 40)
| Offset | Type | Field | Evidence |
|---|---|---|---|
| +0 | i32 | kind (2 is left out of a unit's min/max weapon range) | `SpwC::fegyverhozzaad` |
| +4 | i32 | reload time (battle ticks); a weapon starts at a random point `rndlog(6, 2·r/3, r−1)` | `fegyverhozzaad` |
| +8 | i32 × 12 | damage per target type 0..=11 (type < 8 = capital ships); `fegyverszamol` recomputes it as `firepower(tech) × reload / 250` (weapons 11/12: `/ 3500`; techs 0x44..0x47: firepower × 25; the live table matched the file, `tables.md`) | `fegyverszamol`, `SpwC::Talalat` |
| +0x38 | i32 | hit chance % (human SP: `100 − (100 − c)/((bonus₁+bonus₂)/100 + 1)`, story bonuses `StoryAdat+0x78/+0x7c`) | `SpwC::Loves2` |
| +0x3c | f32 | aim spread (`/1000`, per axis × (rnd(0..99) − 50)) | `Loves2` |
| +0x40 | f32 | [unk] | |
| +0x44 | f32 | projectile speed | `Loves2` |
| +0x48 | f32 | [unk] | |
| +0x4c | i32 | effect duration of disabling (projectile kind 9) and takeover (kind 10) hits | `Talalat` |
| +0x50 | i32 | range | `fegyverhozzaad` (unit min/max range `+0x170/+0x174`) |
| +0x54 | i32 | projectile graphics form (`fegyveralakok`; form 0x15 adds a particle trail) | `Loves2` |
| +0x58 | u8 | 0: fired along the shooter's heading; 1: aimed at the target (lead for kinds 4/5) | `Loves2` |
| +0x59 | u8 | [unk] | |
| +0x5a | u8 | beam: stays on the target, no impact explosion | `Talalat` |

Which tech fits which weapon (`SpwC::AddUjUrhajo`): pod techs 0x11..=0x20 → weapon `t − 0x11`, techs
0x41..=0x48 → weapon `t − 0x34`; planet guns use `bolygofegyverhozzaad` (weapons 20..=24 with
techs 0x71..=0x75).

### 2.3 Design (`Tervek0` defaults exe 0x90BF80 × 201; live `Tervek[e]`; 0x84 B)
`.scn` `TERV` keys in brackets (`SaveScenario` writes them in offset order).

| Offset | Type | Field |
|---|---|---|
| +0x00 | char[20] | [tNev] name |
| +0x16 | i16 | [tTipus] class (from the hull) |
| +0x18 / +0x19 / +0x1a | u8 | [tLathato] visible, [tNemLehetModositani] locked, [tEltuntetniHaNincs] hide when none built |
| +0x1c | i32 | [tUpgradeMire] upgrade target |
| +0x20 / +0x24 | i32 | [tGyartasIdo] build time, [tAr] price |
| +0x28 | u8 | [tMaxTank] vehicle capacity |
| +0x2c / +0x30 | i32 | [tEro] attack, [tHP] hit points |
| +0x34..+0x39 | u8 | [tKolonizalo tTerraformalo tVadasz tUrbazis tUrbazisMozgathato tSatellite] (hull flags) |
| +0x3c | f32 | [tSebesseg] speed |
| +0x40 | i16 | [tEredetiTulaj] original owner: index into `HanyasFajPodTablazatatHasznalja` |
| +0x44 / +0x48 / +0x4c | i32 | [tAlap] hull, [tAlapTalalmany] hull tech, [tHanyPod] slot count |
| +0x51..+0x5c | u8 × 12 | [tPodTalalmany] tech per slot |
| +0x5e..+0x69 | u8 × 12 | [tPodDarab] count per slot |
| +0x6a / +0x6b | u8 | [tRadar] radar tech level, [tFleetCloaking] stealth |
| +0x6c, +0x70, +0x74..+0x7c | | [tKamuDarab, tKamuKills, tKamuBeadasi{Ev,Honap,Nap}] fake (spy-disinformation) figures; `+0x70` is cleared by `TervUjraSzamol` |

### 2.4 Design stats (`TervUjraSzamol(e, d, false, price)`)
With `race = PlayerFaj[e]`, `h = Alapok[+0x44]`, `T = h.tech`:
```
class = h.class; hull_tech = T; slots = GetMaxPodok(+0x40, hull) (slots with pod type > 0)
flags = h.flags (raw); time = Ido(race, T); price = price ? Ar(race, T) : stored price
attack = firepower[T]; hp = tech[T].hp (+32); capacity = tech[T].capacity (+37, u8); +0x70 = 0
speed = class < 2 ? 0.06f : (f32)((f64)TankAlapSpeed[T − 0x91] / 100.0)
radar = stealth = 0
for slot k < slots with pod tech t ≠ 0, count c:
    time += c × Ido(race, t);  price += c × Ar(race, t);  capacity += c × tech[t].capacity (u8)
    t ∉ 0x31..=0x36 and t ≠ 0x25: attack += c × firepower[t];  hp += c × tech[t].hp
    t ∈ 0x39..=0x3d: speed = (f32)((f64)(t − 0x38) × 0.02 + 0.06)     (last engine wins)
    t ∈ 0x21..=0x24: radar = t − 0x20;   t == 0x38: stealth = 1
B_a, B_hp = attack, hp (after the loop)
for slot k < slots: 0x31..=0x35: hp = (int)((f64)B_hp × {0.15, 0.3, 0.45, 0.6, 0.8} + (f64)hp)
                    0x36: hp = (int)((f64)hp + (f64)B_hp);   0x25: attack = (int)((f64)B_a × 0.25 + (f64)attack)
race 2 and class 1: speed = (f32)((f64)speed + 0.02)
```
`Ar(race, t)`: race 4/7 and t = 13 → `(int)(price[13] × 0.75)`; race 5, t ∉ 81..=144, t ≠ 13, 14 →
`(int)(price[t] × 0.7000001)` (double `0x3FE666669C164B9C`); else `price[t]`. `Ido(race, t)`: race 4 and
t = 13 → `time[13] / 2`; else `time[t]` (live tech table; equal to the defaults, `tables.md`).
**Quirk:** with `price = false` the pod prices are added to the stored price again (callers that
don't refresh the price accumulate it).

## 3. Fleet sums (`+0x54..+0x78`)
`HajoFlottahozHozzaszamol(f, d, n)` (owner's design `D`): `+0x54 += n`, `+0x78 += n × D.capacity`,
`+0x60 += n × D.attack`, `+0x6c += n × D.hp`; colony ship flag → `+0x4c += n` and
`OsszHanyKolonizalo[owner] += n`; terraformer → `+0x4e`, `OsszHanyTerraformalo`; **design slot 28**:
`n > 0` sets `+0x52` bit 13, `n < 0` clears it. `TankFlottahozHozzaszamol(f, d, n)`: class 2 (ground)
→ `+0x5c += n`, `+0x68 = (int)((f32)sqrt((f64)(f32)(D.hp × D.attack)) × (f32)n + (f32)+0x68)`
(planets: `+0x58/+0x5c` of the planet record); otherwise (fighters) `+0x54, +0x58 += n`,
`+0x60, +0x64 += n × attack`, `+0x6c, +0x70 += n × hp`.

So: `+0x54` ships incl. fighters, `+0x58` fighters, `+0x5c` ground units, `+0x60` attack, `+0x64`
fighter attack, `+0x68` ground strength, `+0x6c` hit points, `+0x70` fighter hit points, `+0x78`
vehicle capacity (`+0x74` is zeroed by `FlottakUjraSzamol`, [unk]).

## 4. Routing (`SpaceWarIndit(f1, f2, planet)`)
Called by `FRISSIT_Starmap` arrivals (`fleet-movement.md` §4.3: attack a fleet → `(f, target, 0)`;
attack a defended planet → `(f, nearest defender fleet within 15 or 0, planet)`).
1. Participants `spFlottak` = f1 (side 1) and f2 (side 2) if any; the battle planet `spKozeliBolygo` =
   `planet`, else `SearchFlottahozKozeliBolygo(f1, 20.0)` kept only if owned by f1's or f2's owner.
2. `KemkedesUrharcLekezel` (spies), then `Story_SpaceBattleStarted` (ELF 0x73d9c0), which first
   **clears** `Story_SpWarTiltas`, `Story_SpWarSimulateTiltas`, `StoryAdat+0x88/+0x94/+0x98/+0x7c/+0x84`,
   the story battle pacts (`StoryUrharcViszony`), retreat permissions and `StoryUrharcZachkinBarkacs`,
   and then runs the campaign script (story event 1, or the hard-wired fleets 0x439/0x43a), which may
   set them for this battle: their values before the call don't matter (runtime-confirmed). Unless
   `StoryAdat+0x88` (SP) the attack is a diplomatic incident (`DiplomaciaMegtamadas`,
   `BirodalomMegtamad`). `Story_SpWarTiltas` → no battle at all.
3. `P` = planet owner if human-controlled (`PlayerKi == 1`), else 0. `any_human` = a participant's or
   the planet owner's empire is human-controlled; `humans` = human-controlled participants other than
   `P`, + 1 if `P`.
4. With ≤ 2 participants the strength ratio (exe precision):
   `r = (f32)( sqrt((f64)(f32)(hp₁·att₁) + 0.1) / ((f64)((f32)sqrt((f64)(f32)(hp₂·att₂)) + ion) + 0.1) )`
   (one participant: `/ ((f64)ion + 0.1)`), `ion` = `(f32)` ion gun of the battle planet (0 if none),
   hp/att = fleet `+0x6c/+0x60` (products in f32). **Lopsided** = `r > 5.0f` or `0.2 > (f64)r`, unless
   the debug switches 0xB77238 (never) / 0xB77230 (always simulate) are set; never for traders
   (story id 10000..=10999 on either fleet).
5. SP: real-time if `> 2` participants, or `((!always_simulate && !lopsided) || Story_SpWarSimulateTiltas)
   && any_human`; else auto-resolve. MP: real-time only if `MultiplayerBattleStatusz ∈ {0, 1}`,
   `any_human` and `humans == 2` (both sides human), else auto-resolve.
6. Real-time: every participant's order is cleared (`FlottaMozgat(f, 0, …)`), `SpaceWar_NewBattle`
   (joins the battle a participant is already in, else takes a free slot of 10 and `InitSpaceWar`;
   `AddFleet`), then SP: screen 0xB (`AktKepKell`), speed 1, paused; MP: battle-in-progress flags.
   Auto-resolve: `SimulateSpBattle`, `BattlesWon[winner] += 1`, a message for the human (0x0F/0x10 for
   a lopsided SP battle, else 0x11/0x12, won/lost), and after a planet battle won with ground units
   aboard (`+0x54 > 0` and `+0x5c > 0`) `GroundWarIndit(f1, planet)`.

## 5. Auto-resolve (`SimulateSpBattle(f1, f2, planet)`)
Integer only, RNG stream **7** (`time-and-rng.md` §3). Story bookkeeping (`spwsflotta` × 2 at exe
0xF21260: name, owner, result 1 lost / 2 stalemate / 3 won, fleet, story id, hit points before/after),
the human's report text (`AddSzoveg`, English only) and `CTF_value` (attack lost per side) are side
outputs.
### 5.1 Sides
Side 1 = fleet f1 (owner `o₁`), side 2 = fleet f2 (owner `o₂`; with f2 = 0 the planet owner — the ship
and vehicle loops then compare against fleet index **0** literally). Units in this order, each with
`attack`, `hp`, kind:
1. ships of the side's fleet (ship order): design attack, `design hp × 30`, kind 1;
2. vehicles of the side's fleet with a **class-1** design (fighters), one unit per vehicle, kind 2
   (ground vehicles don't fight in space);
3. side 2 only, if `planet > 0`: planet defence ships (designs of `o₂`, which is f2's owner when a fleet
   defends), kind 3; then finished (`+8 == 0`) buildings of types 28..=31 in slot order:
   `attack = firepower[EpuletekLeiro[type].tech]`, `hp = tech[that].hp × 30`, kind 4.
`N₁, N₂` unit counts; `A` alive counts; `F` fighters; `G` guns alive.

### 5.2 Battle
```
rndlog(7, 0, 100)                                    (discarded)
e = rndlog(7, 1, N₁ + N₂);  total = Σ attack (all units; constant)
stale = (A₁ − F₁ < 1 and A₂ − G₂ < 1) or (A₂ − F₂ < 1 and A₁ − G₁ < 1)
target[1] = target[2] = 0
if total > 0: while A₁ > 0 and A₂ > 0 and not stale:
    do e = e % (N₁+N₂) + 1  while unit(e).hp < 1          (round robin; e > N₁ = side 2)
    o = other side; dmg = attack(e)
    repeat:
        if target[o] == 0:
            r = q = rndlog(7, 1, A[o])
            repeat: t = the q-th live unit of o; target[o] = t; q = q % A[o] + 1
            until not (incompatible(e, t) and q ≠ r)      (try every live unit once)
            if incompatible(e, t): dmg = 0
        t = target[o];  d = min(dmg, hp(t));  if incompatible(e, t): d = 0, target[o] = 0
        hp(t) −= d;  dmg −= d;  if hp(t) < 1: A[o] −= 1 (and F/G)
        a side's target with hp < 1 → 0;  recompute stale (only while both A > 0)
    until dmg ≤ 0 or A₁ < 1 or A₂ < 1 or stale
winner = A₁ > 0 ? 1 : 2;  stale: (A₁ − F₁ < 1 and A₂ − G₂ < 1) → 2; (A₂ − F₂ < 1 and A₁ − G₁ < 1) → 1
```
`incompatible` = fighter vs planet gun (either way). Damage carries over to the next target; each side
keeps one target until it dies ("focus fire"). The exe reads unit 0 (uninitialised) when a target is 0:
harmless, it only re-zeroes the target.
### 5.3 Aftermath
1. Destroyed units, side 1 then side 2, unit order: ship → `HajoDelete`; fighter → `TankDelete(fleet,
   design, 1)`; planet ship → its slot zeroed, then the list is compacted; gun → `EpuletDelete(planet, slot)`.
2. `SpWarKiNyert` (exe 0xBAA1EC, also the return value) = winner's owner.
3. The loser's fleet (side 2 → f2, i.e. fleet slot **0** for a planet) backs off unless its slot is
   free: `FlottaArrebPakol`, f32: `d = centre(owner) − pos` (`MiBirodalomKozeppontX/Y`, exe
   0xDB2848/0xDB2898, i32), `l = (f32)sqrt((f64)(dy² + dx²)) + 0.001f`, `pos += d × 6.0 / l`.
4. f1, and f2 if any, with `+0x6c < 1` → `FlottaDelete`.

`HajoDelete(i)` (exe 0x53e5d0): design ← 0x7FFF, sums −1 (§3); then **ground vehicles beyond the
capacity** go: `x = +0x5c − +0x78`; for each vehicle entry of the fleet with a non-fighter design
(`+0x36 == 0`) while `x > 0`: `TankDelete(f, d, min(x, count))` — `x` is not reduced, so every entry
loses up to `x` [original bug]. Then a fleet with `+0x54 < 1` that isn't a wormhole/protected
(`+0x53 & 0x44`) is reported over the network if it's the human's, and deleted if its owner isn't
human-controlled. `TankDelete(f, d, n)` (core 0x541510): the **lowest-index** entry with that fleet
and design loses `min(n, count)`; count < 1 frees it; then the sums (§3) with −n. `FlottaDelete(f)`
(not wormholes): fleets with an attack order on f stop (`FlottaMozgat(g, 0, …)`), every ship
(`HajoDelete`, which can re-enter `FlottaDelete` on the last ship) and vehicle (`TankDelete`) goes,
`OsszHanySajatFlotta[owner] −= 1` (twice on re-entry [original bug]), owner ← 0xFF.

## 6. In-battle mark (`FRISSIT_Starmap`, every tick, after the radar block)
For each of the 10 battle slots (`spw[b]`, exe stride 0x2D9C: unit array pointer 0xE423E0, unit count
0xE423E8, active byte 0xE436EE) that is active, each unit (exe 600 B: `+0` alive, `+0x14` owner,
`+0x28` ship-index list, `+0x1b0` its length; ELF 0x290 B, length at `+0x1bc`) that is alive (exe
`!= 0`, ELF `& 1`) and has an owner (0 = scenery, designs ≥ 0x8000): for each listed ship `s`,
`Flottak[Hajok[s].fleet].+0x52 |= 1`. The fleet index is taken as stored (a freed ship keeps it).
**Original bug:** planet defence ships are listed as `−k`; the loop reads `Hajok[−k]` (memory before
the array) and marks whatever that names. The port skips negative entries (undefined behaviour).
The refresh tick clears bit 0 on every live fleet 1..=n first (slot 0 is never cleared) (`fleet-movement.md` §6.3 step 2); battles only
advance while their screen is shown (`FRISSIT_SpaceWar2` runs on `AktKep == 0xB`), so a background
battle keeps its fleets marked (and immobile, §4.2) until it ends.

## 7. The real-time battle (`SpwC`, not ported)
One `SpwC` per slot (`spw`, ELF 0x16e24e0, 10 × 0x3450). Gameplay randomness is stream **6**
(`spwc.cpp`); explosions, particles and voice use libc `rand()`.
- **Setup** (`InitSpaceWar`, `AddFleet`, `felallit`, formations `formation.md`): one unit
  (`urhajotipus`) per ship, ships of the same type, owner and fleet group into one unit (`+0x1bc` count,
  ≤ 9999). Unit HP `+0x1c` = design hp × **15** per ship (sum), shield capacity `+0x24` from shield pods
  0x31..=0x36 (+15/30/45/60/80/100 % of HP, flags `+0x1b4`), weapons from pods (§2.2; up to 5 extra per
  ship type, `fegyverhozzaad` merges same weapons and adds counts), special pods 0x25/0x27..0x2a/0x37
  set flags (`+0x1b4` bits 5..10; 0x25 = ×1.3 damage). Planet defence ships and guns join the planet
  owner's side (`bolygofegyverhozzaad`). Hostility matrix `+0xd98 [19][19]` (all hostile except self
  and story pacts).
- **Firing** (`Loves2`): projectile damage multiplier starts at 1.0; weapon forms 5/6: `1/(dist²/1000) + 1`;
  unit experience `+0x1b0`: × `(1 + (x/count)/200)`; human SP: × story damage bonus; pod 0x25: × 1.3.
  Hit roll `rndlog(6, 0, 99) < hit chance` (§2.2), spread per axis.
- **Hit** (`Talalat` → `dechp`): `damage = (int)((f32)(weapon.damage[target type] × count) × mult)`,
  then `/ ((4 − |dir − heading|²)/4 + 1)` (rear hits halved), min 1. Kind 9 disables the target for
  `+0x4c − age` ticks, kind 10 takes it over (owner `+0x14` := shooter). Capital ships (type < 8) are
  immune to kinds 9/10. Shields absorb first (`+0x20` used of `+0x24`), the rest accumulates in `+0x18`;
  a single ship dies when `+0x18 > +0x1c`; a group loses `damage / (hp per ship)` ships (one with
  probability when the fraction is small, `1.5/count < damage/hp`); destroyed ships are deleted on the
  starmap at once (`clearship` → `HajoDelete`; planet ships leave the planet list).
- **End** (`vegevizsgal`): the battle goes on while two present, mutually hostile empires can still
  hurt each other (ships/fighters vs ships/fighters, guns vs ships). Winner `+0xd38` = the first
  present empire; `SpaceWarVege` sets `SpWarKiNyert`, `Story_SpaceBattleFinished`. Retreat
  (`retreat`): fleets that fled `FlottaArrebPakol` like §5.3.
Porting it means a fixed-step real-time simulation (Phase 3); the classic randomness split (stream 6
vs libc) is a desync source like ground combat (ADR 0003).

## 8. Verification
`tools/trace/run_spacewar.py` + `spacewar.js` on the GOG exe, replayed by
`engine/crates/ig2-sim/tests/spacewar_trace.rs` (traces in `_local/traces/spacewar*.jsonl`, not
committed). See `traces/2026-10-15-spacewar.md` for counts:
- `--mode design [--fuzz]`: `TervUjraSzamol` on every live design; fuzzed hulls, pod tables, pods,
  counts, races, stored price/capacity and the price flag: all 0x84 bytes compared.
- `--mode auto [--fuzz]`: `SimulateSpBattle` on fleet-vs-fleet, fleet-vs-planet and fleet-at-planet
  pairs; fuzzing moves ships between fleets, redesigns them, adds vehicles, planet ships and guns,
  re-sums the fleets (`FlottakUjraSzamol`) and randomises stream 7. Compared: winner, all 21 streams,
  every ship and vehicle entry, both fleets' owner/position/flags/sums, fleet 0, planet ships, guns.
- `--mode indit [--fuzz]`: `SpaceWarIndit` path (hooked `SimulateSpBattle` / `SpaceWar_NewBattle`) with
  fuzzed debug/story switches and ion guns; tactical battles are left frozen in their slots and the
  following `FRISSIT_Starmap` ticks record the battle units and every fleet's bit 0 (units' alive
  bytes, owners and fleet bits fuzzed).
Frida note: the agent's own Interceptor hooks (rndlog, `SimulateSpBattle`, `SpaceWar_NewBattle`) did not fire inside calls it makes through `NativeFunction` (Frida 17, even with `traps: 'all'`), so the replays compare complete RNG state and the routing probe detects the path from its effects (stream 7 rolled; screen 0xB, a filled battle slot or a cleared order).
