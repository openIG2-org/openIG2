# Space combat: hulls, weapons, designs, routing, auto-resolve and the in-battle mark

Status: **§2–§6 verified at runtime** (§8, `traces/2026-10-15-spacewar.md`) and ported to
`engine/crates/ig2-sim/src/spacewar.rs`; §7 (the real-time battle tick, `SpwC::SpaceWarFrissit`) is ported to
`spacebattle.rs` and verified bit-exact (`traces/2026-10-16-spacebattle.md`); its setup and end are not. **The exe is normative**: precision below is the exe's (asm checked;
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
| `SpwC::*` (130 functions) | 0x6851d0..0x6b8670 | the tick's functions mapped in §7 | §7 |

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
0. `f2 == 0 && planet == 0` → nothing.
1. Participants `spFlottak` (exe 0xBAA1C0, 6-byte entries from index 1: `+0` fleet i16, `+2` side i16) = f1
   (side 1) and f2 (side 2; written even when f2 is 0), `SpWarHanyFlotta` 0xBAA1E8 = 2, or 1 without f2; the
   battle planet `spKozeliBolygo` (0xBAA1E4) = `planet` (flag 0xB9C027 := 1), else (flag := 0)
   `SearchFlottahozKozeliBolygo(f1, 20.0)` kept only if owned by f1's or f2's owner.
2. `KemkedesUrharcLekezel` (exe 0x53f0c0): for every pair `(a, b)` of participants, `a = b` included, with both
   owners < 9: every ship and every class-1 vehicle (fighter) of `a` is seen by `b`'s owner
   (`KemkedesTervLekezel`, exe 0x53f060: the design's hull tech `+0x48` and its pods' techs `+0x51..` (`+0x4c` of
   them) into the seen matrix `[tech].+0xc0 + 9·spy + owner`, `colony-side.md` §2). Then
   `Story_SpaceBattleStarted` (exe 0x5c9220, ELF 0x73d9c0), which first
   **clears** `Story_SpWarTiltas`, `Story_SpWarSimulateTiltas`, `StoryAdat+0x88/+0x94/+0x98/+0x7c/+0x84`,
   the story battle pacts (`StoryUrharcViszony`), retreat permissions and `StoryUrharcZachkinBarkacs`,
   and then runs the campaign script (story event 1, or the hard-wired fleets 0x439/0x43a), which may
   set them for this battle: their values before the call don't matter (runtime-confirmed). Unless
   `StoryAdat+0x88` (SP) the attack is a diplomatic incident (`DiplomaciaMegtamadas(o₁, o₂, 0, 0)`; the
   exe's `BirodalomMegtamad` call goes to an empty function, 0x559ad0), then `StoryAdat+0x88 := 0`.
   `Story_SpWarTiltas` → no battle at all.
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
   Auto-resolve: `SimulateSpBattle(f1, f2, spKozeliBolygo)`, `BattlesWon[winner] += 1`, a message when `EN` owns
   a participant or the battle planet — whoever plays it (0x0F/0x10 for a lopsided SP battle, else 0x11/0x12,
   won/lost; `info-messages.md` §3.2), and when f1 still has ships and ground units (`+0x54 > 0`, `+0x5c > 0`)
   after an attack on a planet (the `planet` argument, not the nearby battle planet) `GroundWarIndit(f1, planet)`.

Port: `Game::space_war` (`game/battles.rs`) runs steps 0–6 where the starmap's loop calls it (the loop pauses there,
`fleet-movement.md` §4.3); a real-time route is reported (`BattleEvent::SpaceRealTime`) and not run. Verified in the
save replay (`traces/2026-10-16-save-replay.md`, `replay-quick1-battles-*`): 11 battles incl. one against a battle
planet whose defence ships fought for the defending fleet.

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
   design, 1)`; planet ship `k` → its slot `+0xfe + 2k` := 0; gun → `EpuletDelete(planet, slot)`. Then the
   defence-ship list closes up in place: for `i = 1` while `i ≤ n` (`+0x114`): a zero slot → `n −= 1`, stop if
   `n < i`, else slots `i..=n` := slots `i+1..=n+1` (and test slot `i` again); a non-zero one → `i += 1`. The old
   last entry stays behind the new end (a destroyed last ship stays 0; runtime-confirmed).
2. `SpWarKiNyert` (exe 0xBAA1EC, also the return value) = winner's owner (`spwsflotta[winner − 1].owner`: the
   planet's owner for side 2 without f2). `spwsflotta` gets the results (3 won, 1 lost; a stalemate gives side 1 a 2 and side 2 a 3) and
   `Story_SpaceBattleFinished(spwsflotta, 2, false)` stores them for the story (`campaign.md`).
3. The loser's fleet (side 2 → f2, i.e. fleet slot **0** for a planet) backs off unless its slot is
   free: `FlottaArrebPakol`, f32: `d = centre(owner) − pos` (`MiBirodalomKozeppontX/Y`, exe
   0xDB2848/0xDB2898, i32), `l = (f32)sqrt((f64)(dy² + dx²)) + 0.001f`, `pos += d × 6.0 / l`.
4. f1, and f2 if any, with `+0x6c < 1` → `FlottaDelete`.

`HajoDelete(i)` (exe 0x53e5d0): design ← 0x7FFF, sums −1 (§3); then **ground vehicles beyond the
capacity** go: `x = +0x5c − +0x78`; for each vehicle entry of the fleet with a non-fighter design
(`+0x36 == 0`) while `x > 0`: `TankDelete(f, d, min(x, count))` — `x` is not reduced, so every entry
loses up to `x` [original bug]. Then a fleet with `+0x54 < 1` that isn't a wormhole/protected
(`+0x53 & 0x44`) is reported over the network if its owner is `EN` (`Multi_Send(0x1b, EN, f)`; not while the
battle screen holds it as one of its two fleets), and deleted if its owner isn't human-controlled
(`PlayerKi != 1`). The 0x1b message reaches `MULTIGET_Starmap` (ELF 0x719680) in a later frame, even in SP, and
deletes the fleet (`FlottaDelete`) if it still holds no ships: `Game::fleet_emptied`. `TankDelete(f, d, n)` (core 0x541510): the **lowest-index** entry with that fleet
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

## 7. The real-time battle (`SpwC`)
Status: **the battle tick, its setup and its end are ported** (`engine/crates/ig2-sim/src/spacebattle.rs`) and
verified bit-exact against the exe (§8; tick: `traces/2026-10-16-spacebattle.md`, setup and end:
`traces/2026-10-16-spacebattle-setup.md`). Not ported: the battle screen (camera, selection, drawing); its orders are (§7.12), and so is the MP flow (§7.13).
Gameplay randomness is stream **6** only; every libc `rand()` in a battle is cosmetic (§7.9).

| Function | ELF | exe |
|---|---|---|
| `FRISSIT_SpaceWar` / `SpwC::frissit` | 0x67c380 / 0x6a6450 | 0x55e7f0 / 0x573c70 |
| `SpaceWarFrissit` (one tick) | 0x6a60c0 | 0x56dff0 |
| reload countdown (ELF name `initgombsugar`) | — | 0x576220 |
| `SpaceWarMI` / `MIcsapatokraoszt2` / `MIvisszavonulasvizsgalat` / `retreat` / `MIellensegmegtamad` | 0x6a63a0 / 0x6abac0 / 0x6ac8c0 / 0x6acb40 / 0x6ac100 | 0x56f3c0 / 0x56acd0 / 0x56b3b0 / 0x57ee20 / 0x56af60 |
| `mitamadjabeallit` | 0x6a5f00 | 0x57c3a0 |
| `SpaceWarBolygo` (planet guns) | 0x6a2490 | 0x56d740 |
| `SpaceWarLovedekek` / `…LovedekMozgat` / `…LovedekKirajzol` | 0x69f6a0 / 0x69da70 / 0x69ebc0 | 0x56f320 / 0x56e890 / 0x56e1c0 |
| `setlovedekrotate/speed/vector` (homing) | 0x696320.. | 0x580130 |
| `Talalat` / `VadaszTalalat` / `FoldTalalat` | 0x69bae0 / 0x69c320 / 0x69c460 | 0x570d60 / 0x5711a0 / 0x566ff0 |
| `dechp` / `felrobban` / `clearship` / `resztvevobirodalmakujraszamol` | 0x697730 / 0x69b610 / 0x69b3e0 / 0x693e50 | 0x572260 / 0x5737e0 / 0x571fe0 / 0x57ec00 |
| `SpaceWarSpecialpodfrissit` | 0x6a2800 | 0x570480 (called twice a tick) |
| `SpaceVadaszMozgat` / `setvadasztarget` / `setvadaszvector` / `vadaszloves` | 0x6a08f0 / 0x6a01b0 / 0x69fb60 / 0x6a05e0 | 0x56d530 / 0x581100 / 0x580f70 / 0x582dd0 |
| `Iranyitas` / `Lov` / `Lov2` / `Lov3` / `Lov4` / `Loves2` | 0x699f20 / 0x6996f0 / 0x699850 / 0x699b60 / 0x699dc0 / 0x6983a0 | 0x568dc0 / 0x569cf0 / 0x569910 / 0x569ab0 / 0x569c40 / 0x569fa0 |
| `Mozgatas` / `setvector22` / `seturhajopos` / `nextwaypoint` / `moveship` | 0x696770 / 0x695940 / 0x686aa0 / 0x68a0d0 / 0x68a1b0 | 0x56b590 / 0x581380 / 0x580540 / 0x57d200 / 0x57c4c0 |
| `Utkeres` / `KifejtBestNyitott` / `CheckNyitott` / `TeglaRajzol` / `CheckLine` | 0x7bf3d0 / 0x7be300 / 0x7bdce0 / 0x7bea70 / 0x7befe0 | 0x6133a0 / 0x612e50 / 0x612b50 / 0x613290 / 0x612a10 |
| `urhajomeghalas` / `vadaszmeghalas` / `VadaszRobban` | 0x6a1690 / 0x6a2300 / 0x69c1a0 | 0x582550 / 0x582fb0 / 0x5710f0 |
| `vegevizsgal` | 0x6a0d50 | 0x5833d0 |
| `SpaceWar_NewBattle` / `InitSpaceWar` / `AddFleet` / `SpwC::Init` (slot defaults) | 0x67b740 / 0x68c840 / 0x68f070 / — | 0x563700 / 0x5684e0 / 0x5646b0 / — |
| `AddUjUrhajo` / `fegyverhozzaad` / `terv2tipus` / `pos2racspos` | 0x68a730 / 0x68bee0 / 0x689a40 / — | 0x565800 / 0x572650 / 0x582010 / inlined |
| `FelallasLoad` / `felallit` (with `gethajokcenter` inlined) / `felallasforgat` / `helykeres` / `tipus2felallashely` | 0x691700 / 0x68de10 / 0x689e70 / 0x689bc0 / 0x689b80 | 0x566eb0 / 0x572ef0 / 0x572d30 / 0x573cd0 / 0x5820a0 |
| `AddUjVadasz` / `bolygofegyverhozzaad` / `setshippos` / `racspos2pos` / `utkereselfer` / `TeglaRajzol2` | — / 0x691040 / 0x691520 / — / 0x7bedc0 / 0x7bec10 | 0x566420 / 0x571d00 / 0x5802d0 / 0x57e610 / 0x614190 / 0x613180 |
| `fegyverszamol` / `Lovedekinit` | 0x68c5c0 / — | 0x572bf0 / 0x569db0 |
| `SpaceWar_Kilep` / `SpaceWarVege` (+ `EndSpaceWar` inlined) / `Story_SpaceBattleFinished` / `Starmapvisszaallit` | 0x67c090 / 0x6a1560 / 0x73dc50 / 0x6b4020 | 0x562b50 / 0x5706f0 / 0x5c9180 / 0x570c10 |
| vector helpers `tav` / `tavnegyzet` / `normalizevektor` / `spwnormalizevektor` / `vektorszog` / `vektormul` / `creatematrixfromaxis` / `rotatepointmatrix` | | 0x583bf0 / 0x583c50 / 0x4501f0 / 0x583a80 / 0x450a60 / 0x450a00 / 0x44f9f0 / 0x4504b0 |

### 7.1 Slots and the tick
Ten slots `spw[b]` (exe 0xE417C0 + 0x2D9C·b; ELF 0x16e24e0 + 0x3450·b), active byte `+0x1f2e`. On screen 0xB
(or in MP) `FRISSIT_SpaceWar` runs every slot's `frissit`, which calls the tick `IdoSpeedSzorzoBattle`
(exe 0xBAE260) times. The projectile pools are **global** (`fegyveralakok`, exe 0x9256F8, 40 forms × 0x78)
and every slot's tick moves every projectile: two battles running at once move and hit each other's
projectiles [original bug]. The port gives each battle its own pools (traces record one battle at a time).

One tick (exe 0x56dff0), in order:
1. `tick` (+0xc4c) += 1. Reload countdown (0x576220, also when paused): every mount timer −1 unless the unit is
   disabled; a live unit's hit radius +0x170 = type scale × 1000 (f32) while it is 0.
2. Unless paused (`IdoPause` 0xBAE264): +0x2d84 → +0x2d86, +0x2d84 = 0; AI timer +0xc40 (f32) −= 1, below 0
   → 10 and the battle AI (§7.7) for every empire whose AI byte (`+0x6c·e + 0x50`) is set (debug 0xB77244
   off); then attackers (§7.3), planet guns (§7.4), projectiles (§7.5), special pods (§7.6), fighters
   (§7.6), firing (§7.4), movement (§7.2), unit deaths and fighter deaths (§7.8). The engine trails
   (`Hajtomu`), debris (`Darabok`) and fighter sprites (0x56d460) are drawing only.
3. Particles (when shown), the drawing pass `frissit2` (0x573b40) — which ends with the special-pod pass once
   more, paused or not (so the pod countdown runs twice a tick).
4. Unless closed (+0x1fb8) or in the editor: the end check (§7.8); true → +0x1fba, back to the starmap,
   `SetIdoLelassit(0)`, and after a won planet battle with ground units aboard `GroundWarIndit`.

### 7.2 Units and movement
A unit (exe 600 B, ELF 0x290; key fields): +0 alive, +8 type (< 8 capital ships, 0x12 scenery), +0x14 owner
(0 = scenery), +0x18 damage, +0x1c hit points (design hp × 15 per ship, summed), +0x20/+0x24 shield used /
capacity, +0x28 ship list (`Hajok` indices, −k = planet defence ship k; the array keeps a stale tail) with
+0x1b0 its length, +0x1b4 = 1 (never raised when ships join: the hit-point share of one ship is the whole
unit's [quirk]), +0x2c/+0x74 queued waypoints (4 × 16 B grid cells), +0x44 position (x height, y, z),
+0x54 grid cell, +0x64 destination, +0xb4/+0xc4 next path step and its cell, +0xd4 velocity, +0xe4 drawn
orientation matrix (cosmetic), +0x108 speed, +0x110 heading, +0x120 drawn half-way heading, +0x130/+0x140
position and tick of the last path search, +0x148 scenery spin, +0x154 target (unit or gun by +0x158),
+0x15c order (0 none, 1 move, 3 attack), +0x160 last turn, +0x168/+0x16c shortest/longest weapon range,
+0x17c explosion animation tick, +0x180 path stopped short, +0x190/+0x194 nearest attacker and its squared
distance, +0x198 the unit it picked this tick, +0x19c id, +0x1a4 experience, +0x1a8 special pods, +0x1bc
weapon mask, +0x1c0 disabled ticks, +0x1c4 taken-over ticks, +0x1c8 original owner, +0x1cc retreat ticks,
+0x1d0 explodes when it speeds past 3, +0x1d4 retreating, +0x1d5 jammed, +0x1d6 scanned, +0x1d7 cloaked,
+0x1d8 cloak cooldown, +0x214 ship-type record (scale +0x10, top speed +0x14, acceleration +0x1c, turn
+0x24, footprint +0x28/+0x2c), +0x218/+0x21c weapon mounts (0x70 B: weapon, kind, barrels, timer, timer
set, reload, damage copy, … range +0x60), +0x220 idle ticks, +0x244/+0x248 chain-lightning arc and its
projectile, +0x254 short path searches in a row. +0x181 (selection) and +0x184..+0x18f are UI.

The arena: cells of 3 (+0x804..), grid 128 × 128 × 10 (+0x810, globals 0xF691E8..), bounds ±192/±192/±15
(+0x81c..), width 384 (+0x834). Cell of a position: `(int)((cell·0.5 + p.y)/cell + 64)`, from z, from x
(+5); back: `(c − 64)·cell` etc. (f32). Points are kept two cells inside the bounds.

`Mozgatas` per unit: a retreating unit (alive or not) heads 300 away from the retreat point (+0x1f64;
`(1,0,0)` when on it) at boosted speed. A live unit with an order re-plans when it drifted `(f64)cell·1.6 <
d²` from the last plan or 50 ticks passed: an attack order first snaps the destination to the target's
cell; the search radius is 1 for a move, else `(int)(range_min / cell)`; an attack whose next step is
within 1.5 cells searches from that step, otherwise from the unit's cell (counting short searches: seven
in a row cancel a move); the step becomes the result. Then `setvector22` (below); on arrival the unit sits
on the step (speed and velocity 0), a move order at its last waypoint ends (`nextwaypoint` re-sends the
reached point with its x cell for both y and z [original bug]); position += velocity, cell recomputed; an
attack on a cloaked unit is dropped. Every unit: scenery spins `((i·K) % 43)/1500`; idle counter (`(f64)speed
≤ 0.01`); bobbing `x = (f32)((f64)(f32)sin((f64)(idle/((i·0x16df2b % 42)·0.25 + 5))) · 0.01 + x)`; the idle
turn (`seturhajopos`); a unit struck by a chain-lightning arc takes weapon 12's damage/2 per ship.

`setvector22(boost)`: top speed, acceleration, turn = type values (top ×10) when boosting, else
`(f32)((f64)v / 2.5)`; top `< 0.01` (f64) → nothing. `d = step − pos`; `|d| < 0.1` (f64): arrived if the
path ended short, else keep the heading. Turn: `a = vektorszog(n(d), n(h))`; above 0.01 (f64): axis
`h × d` (x-axis when null, or when `a > 3` and `h.x < 0.1`), step `min(turn, a/4, a)`, heading =
`creatematrixfromaxis(axis, step) · n(h)` (not re-normalised). Brake by the acceleration when
`(f32)((f64)(|d|/3)/2.5) ≤ a`; otherwise accelerate, and on a short path aim to stop on the step
(`v = speed²/(2|d|)`, `+accel` if `v < accel` else `−v`); cap at the top speed; velocity = speed × heading.
`creatematrixfromaxis` is quaternion-based with `qz = z·s`, `zz = 2qz·qz` (the decompile's grouping is wrong;
the asm is normative). `seturhajopos` (idle turn, standing on the step with speed 0 and a turn rate): face the
attack target, else the nearest attacker, else the picked unit; with nothing picked level out (flatten the
heading's x when |x| > 0.01); turn by `min(0.1, a/4)` and re-normalise.

**Path search** (`Utkeres`, A*-like on `tomb` u16[128·128·8] at exe 0xF69218, x-major, filled with 0xFDE8):
the start (clamped 0..127/0..7) is 1, the goal 0; every other live unit except hostile cloaked ones blocks
a box of its footprint plus ours − 2 (0xFDE9; the box opens on the searcher's side if it stands inside);
the goal sphere (`r²`, z up to 9 — it aliases the next row) is cleared, growing until it holds a free cell
(and the search then "can't reach perfectly"). Open list: a 1-based binary heap of 1000 (x, y, z, f);
relaxation weights 5 (face), 6/7 (edge), 8 (corner), heuristic `7·max(dx,dy) + 2·(min + dz) + 5·max(0, dz −
max)`, cost and heuristic caps (`MaxSuly`, `MaxHeu`), best-so-far score `5h + 2g` (u16 arithmetic); an
already-open cell is updated in place without sifting [quirk]. Then, if the start is closer to the goal than
the best cell, the best is the start; walk back down the costs over the 26 neighbours in a fixed order
(flat-index offsets, so z wraps into the neighbouring row), and take the earliest path cell the start can
see (`CheckLine`, f32 interpolation + 0.5 in f64); the path is stored as u16, so a cell that wrapped below
z = 0 is checked and taken as z + 65536 [quirk]. Results: the step, "one step short", "nothing to do".

### 7.3 Targets
`mitamadjabeallit`: every unit's attacker = the nearest unit (squared distance) whose pick names it; a gun
targeted by a unit records it (+0x9c). `Iranyitas` per live unit: pick reset, anti-planet weapons first
(`Lov2`, unless it targets a gun), disabled/taken-over countdowns (takeover ends → original owner), an
ordered attack fires when in range (`Lov`, the pick becomes the target's id) or at the ordered gun
(`Lov3`); otherwise, every tick (timer +0xc44), the nearest hostile fighter group within the longest range
(`Lov4`, squared, ties to the later), the nearest hostile visible unit (`Lov`, pick = its index) and the last
standing gun when the planet's owner is hostile (`Lov3`). Hostility is `+0xca0 [19][19]` (1 = fight; read
`[shooter][planet owner]` for `Lov2`/`Lov3`, `[planet owner][shooter]` for the auto gun pick).

### 7.4 Firing (`Loves2`)
Mount ready: timer < 0 and the weapon's bit in the mask; firing rolls the timer `rndlog(6, 2r/3, r − 1)`.
`Lov` needs kind ≠ 2 and `distance < range`; `Lov2` kind 2 at a random live gun (timer first, then the pick);
`Lov3` any weapon while a gun stands; `Lov4` weapon ids < 5. Planet guns: when their timer runs out
(`rndlog(6, 0, r/3 − 1) + r`) at the forced target or else the first hostile visible unit.
A projectile takes the first free slot of its form's pool except the last; unloaded forms can't fire; a
jammed unit can't fire weapons 9..12. Damage factor 1.0; ship shots: weapons 5/6 `1/(d²/1000) + 1` (f32),
experience `(f32)(((f64)(xp/ships)/200 + 1)·mult)`; SP human: hit chance `(int)(100 − (100 − c)/b)` and
factor × `(f32)((f64)bonus/100 + 1)` from `StoryAdat` — **tested against the slot's previous owner**, the new
one isn't written yet [quirk]; pod 0x20: `(f32)((f64)mult·1.3)`. A firing ship's cloak cooldown =
`rndlog(6, 0, 20) + 40`. Unaimed weapons (only ships fire them): heading + spread `(rnd − 50)·s/1000` per
axis, velocity × 0.2, speed field = the ship's speed. Aimed: origin jittered by `(rnd − 50)/100 · scale ·
1024` (ships), 40 % of shots at a moving ship fly straight, shots at guns always; fighters are led by their
velocity; direction normalised + spread (none at fighters), re-normalised; hit roll `rndlog(6, 0, 99) <
chance` (shots at fighters always "hit").

### 7.5 Projectiles and hits
Per form, per active slot: missiles 13..15 at a ship are lured away by decoys (pod 0x80 unless the shooter
scans, 1 %/tick; pod 0x100, 2 %); flight: homing (turn ≠ 0, not straight) turns `dir += min(m, turn)·e/m`
(`e = n(goal − pos) − dir`, astray goals `(y·−10, z·−8, y·70)`), speed towards the weapon speed (×3 at guns)
by ±accel; position += velocity; an attached beam sits on its target. A shot that won't hit stops homing.
Hits: at fighters within 3 and passing (`|n(c − p − v) − n(c − p)| > 1.414`, f64) → `VadaszTalalat` (loses
`damage[1]·count/hp`, ≥ 1, the shot flies on); at guns within 100 → `FoldTalalat`; at units within
`(radius·3)²` and passing → `Talalat`, and every scenery unit within its scale × 1024 is hit too (the shot
is retargeted; a freed shot keeps hitting asteroids that tick [quirk]). Weapon 11 damages every unit
within its sprite size/3 of its sprite — sprites are in drawing scale (+0x1f88 = 0.0004) and units are not,
so the area sits near the origin [quirk]; both this and weapon 12's burn hit a group once per ship, re-reading the
ship count every round (a group wiped by the first round takes no more). Weapon 12 (kind 0) burns its target every tick and arcs to every
unit and fighter within `(int)(d²(shooter, target)·0.0625)` squared; its hit chance is 0 in the table.
Lifetime: duration (×10 at guns).
`Talalat`: dead or retreating targets → the shot ends (beams stay); weapons 9/10 at types ≥ 8 end;
ship `rndlog(6, 0, n − 1)`; damage `(int)((f32)(damage[type]·count)·mult)` (scenery types read past the
12-entry table into the record's next words [quirk]) divided by `(4 − |heading − n(dir)|²)·0.25 + 1`
(rear hits count half), ≥ 1. Beams attach (velocity and speed 0). Weapon 9 disables for `duration − age`
(or ends on a disabled target), 10 takes the unit over (owner := shooter's) for `duration − age`, order :=
move. Gun shots count hits. Then `dechp`.
`dechp`: shields absorb first; a single ship dies past its hit points (the "hull failing" speech draws
stream 6); a group: all die past the total; else `m = (int)(rest/(hp/+0x1b4))`: m < 1 → one dies when
`1.5/(f64)n < (f64)((f32)damage/hp)`, m = 1 → one, m > 1 → m (which can exceed what is left [quirk]).
`felrobban`: the owner's lost attack (+0x58) += the ships' design attack (planet ships: the mounts'
damage[4] copy); the last ship → dead (explosion counter, end-check countdown reset), otherwise the ship
leaves the list, damage and barrels scale by `n/(n+1)` (a barrel count of 1 → 0, 0 → 1). `clearship`: a
planet ship leaves the planet's list; a ship is `HajoDelete`d and an emptied fleet `FlottaDelete`d (its
participant record: 1 destroyed, 2 if retreating). An asteroid that a takeover gave an owner and that then dies
runs `clearship` on its ship 0: `HajoDelete(0)` and the fleet `Hajok[0]` names (garbage, past `Flottak`)
[original bug; seen in `spwc-setup-202`]. `FoldTalalat` (weapons 9..12 do nothing): building
damage `(int)((f64)(int)((f32)(damage[5]·count)·mult/armour)·6.666666666666667·2)` through 0x474160 (adds
while below the type's hit points, exe 0x8EBA40 × 0x24; `EpuletDelete` at the limit); the gun is gone when the
slot's (possibly cleared) type's hit points ≤ its damage. The present-empire list (+0xe09/+0xe1c/+0xe30)
is recomputed after every loss.

### 7.6 Special pods and fighters
Pods (`+0x1a8`): 0x20 damage ×1.3, 0x40 jammer, 0x80/0x100 decoys, 0x200 scanner, 0x400 cloak. Every
tenth call (+0x1f84; two calls a tick): clear jammed/scanned/cloaked, cloak cooldowns −1; jammers and
scanners mark hostile units within 100; a cloak meets a hostile within 20 → cooldown 50; cloaks with no
cooldown hide; then every unit's idle turn.
Fighter groups (0xa8 B: +4 position, +0x14 goal, +0x24 velocity, +0x34/+0x38 target, +0x40 direction,
+0x50 turn, +0x54 speed, +0x58/+0x5c goal distance and the previous, +0x60 state, +0x64 jitter, +0x68 fire
timer, +0x6c design, +0x70 owner, +0x78 weapon − 30, +0x7c fleet, +0x80 count, +0x84 hp each, +0x88..+0x8c
retreat). States: 0 picks once it stops closing (`rndlog(6, 0, 99) <` the side's aggression +0x40 → ship
first): the enemy unit with the lowest `(int)(distance + damage %)` (−40 if busy with this owner) or a
random enemy group (enemy = the last present hostile empire with ships or fighters); 1 chases until within
scale·1024 + 20 (8 at fighters), then overshoots by `(pos − target) + rndlog(6, −100, 100)/60` → 2; 2 → 0
within 7; a dead, cloaked or retreating target is dropped. Steering: `dir += min(m, 0.08 (f64)) · e`; speed
towards `(i%5)·0.025 + 0.5` (×7 retreating) by ±0.1 (f64 compare, f32 store); jitter re-rolled each tick
(`rndlog(6, 0, 2) + 3`); fires within 35 when `|n(d) − dir| < 0.6` (f64): at ships one craft, at fighters
the whole count, timer `rndlog(6, 0, r/3 − 1) + r`. Chain lightning kills `max(1, damage[1]/8)/hp`.

### 7.7 Battle AI (every 10 ticks per AI empire)
First turn: squads (`MIcsapatokraoszt2(e, 2)`): `(n − 1)/2 + 1` units each, grown greedily (truncated
squared distances) from the unit farthest from the centre, each next unit nearest to the squad's centre.
Then: the retreat check (MP, or the human / no story ban, and no scripted lock): with more than 2 units,
`(f64)strength < (f64)lost·0.6` and `2·strength < Σ hostile strengths` → retreat; all its units' weapon masks
:= all. Retreat (if allowed, +0x68; story ban for the human): mobile units of that original owner start
retreating (`rndlog(6, 0, 2) < 1` explodes later), fighters head 600 away (`rndlog(6, 0, 2) < 1` doomed), its
fleets back off on the starmap (exe 0x597cb0, = `FlottaArrebPakol`). Attack: squad centres; gathering squads
spread away from the common centre (`2d`, normalised to 3 within 3, to 10 beyond 10) and move there, then
attack: each idle unit attacks the hostile visible unit with the lowest `max((int)d, (int)range_min)` +
**its own** damage % [quirk].

### 7.8 Losses and the end
`urhajomeghalas`: scenery spin (f64 frac), retreating units: counter, blow up when they explode-flagged
and faster than 3, leave beyond the arena width; the dead's explosion counter runs (reset after 45).
`vadaszmeghalas`: retreating fighters: doomed ones die when `rndlog(6, 20, 260) < ticks` and `rndlog(6, 0,
5) < 1`; beyond the arena width they leave. End check (`vegevizsgal`, every +0xc58 = 100 ticks; not during a
screen fade 0 < 0xE72E14 < 1): counts per **original** owner (+0x2b40); over unless two present hostile
empires can still hurt each other (ships/fighters vs ships/fighters, guns vs ships); winner +0xc48 = the
first present empire. On screen, a won planet battle with ground troops waiting first sends every unit to
the planet (+0x1f30, then checks every tick).

### 7.9 Randomness
Stream 6 only, in the orders above. libc `rand()` (cosmetic, no battle state): `Talalat` sparks (4 per
non-beam hit), explosions and debris (`urhajomeghalas`, ~10 a dying ship), lightning jitter (`villam`,
0x616620), `dechp` speech (never: `rand() % 1`). The port draws none and reports [`BattleEvent`]s.
The setup (§7.10) draws stream 6 too and no libc `rand()`.

### 7.10 Setup (`SpaceWar_NewBattle` exe 0x563700)
Input: the participant list `spFlottak` (exe 0xBAA1C6, 6 B each: i16 fleet, −1 = none; count 0xBAA1E8, ≤ 4),
the battle planet `spKozeliBolygo` (0xBAA1E4, 0 = none), `spFoldiharcElottiUrharc` (0xB9C027: ground units
wait), and what `SpaceWarIndit` (§4) left in the story arrays.
1. **Join or new.** For each active slot (in slot order, until one matches): every participant record's
   fleet is compared with every list entry; a match sets that entry to −1 and selects the slot (the rest of
   that slot's records still compare). No match: the first inactive slot (none → −1, no battle) runs
   `InitSpaceWar`. Either way `AddFleet` follows.
2. **Hostility.** After `AddFleet`, for `a, b` in 1..=18: `hostile[a][b] = 1`, `hostile[a][a] = 0`; in SP a story
   pact `StoryUrharcViszony[a] == 1` (exe 0xF26150) clears `[a][EN]` and `[EN][a]`, and
   `StoryUrharcZachkinBarkacs` (0xF2619C) `= z ≠ 0` clears `[z][9]` and `[9][z]`, row by row, so a later row's
   reset undoes an earlier clear: with a pact empire `a < EN`, `[EN][a]` ends up 1 again [quirk]. Row and
   column 0 stay 0. The
   relation-based matrix `InitSpaceWar` builds (`Viszony +0x25 == 1` → 1, else 2) is thus always overwritten:
   everybody fights everybody [quirk]. `+0xe34 + 4a` = `StoryUrharcVisszavonulhate[a]` (0xF261A0).

`InitSpaceWar` (exe 0x5684e0) resets the slot in place: bytes 0..0x804 (the 19 side records of 0x6c) are zeroed;
cell 3.0 × 3 (+0x804), grid 128 × 128 × 10 (+0x810; globals 0xF691E8..), bounds ±192/±192/±15, widths 384/384/30,
side `e`: `+0x1c..+0x24` = 0 (its fleet centre), rotation `+0x2c/+0x30/+0x34` = (π/2, 0, −π/2), fighter
aggression `+0x40` = 50; side 0's centre z = `cell_y · −20` = −60, side 1's = +60. Counts and lists zeroed (units,
fighters, guns, participants, present empires), the unit array is a fresh `malloc(120 × 600)` (contents
undefined), planet 0, planet owner −1, not shown, pod timer +0x1f84 = 1, drawing scale +0x1f88 = 0, end flags
+0x1fb8/+0x1fba = 0, victory +0x1f30 = 0, retreat point 0, the per-empire counts +0x2b40 = 0, +0x2005 = 0,
`+0x2d84` = 1 (u16, so +0x2d85 = 0), +0x2d86 = 0, +0x2d90/+0x2d91 = 0, tick +0xc4c = 0, the end countdown +0xc50
and the debug +0xc54 = 0. **Not reset**: the AI timer +0xc40, the auto-target timer +0xc44, the winner +0xc48
and the end period +0xc58 (100), which keep the slot's previous battle's values (`SpwC::Init` set them once)
[quirk]; the planet position +0x84c..+0x854 (1200, 200, 1200) and size +0x858 (1) are constants from
`SpwC::Init`. `Lovedekinit` (0x569db0) empties every loaded form's pool (all slots free) — the pools are
global, so this also empties other running battles' projectiles [original bug; the port's pools are per
battle]. Asteroids: if `asteroidak` (exe 0xE72DD2) is off, `rndlog(6, 0, 1)`; 0 → none (+0x2d70 = 0, counts
0); otherwise (or with the switch on) +0x2d70 = 1 and counts `rndlog(6, 0, 2)`, `rndlog(6, 0, 5)`,
`rndlog(6, 0, 7)` (+0x2d74..+0x2d7c). Then `fegyverszamol` (0x572bf0, §2.2; idempotent) and the screen fade
0xE72E14 = 0.

`AddFleet` (exe 0x5646b0), in order:
1. Active (+0x1f2e = 1). Every side's setup list (`+0x5c`/`+0x60`, 0x1c-byte entries) is emptied, its
   fighter count +0x64 and centre +0x1c..+0x24 zeroed (also on a join). `new = (participant count == 0)`. Planet +0x1f28 = `spKozeliBolygo` (also on a
   join), human +0xc3c = `EN`, ground war +0x2004 = `spFoldiharcElottiUrharc`.
2. Per list entry `f ≥ 0`, owner `o = Flottak[f].+0x15`: `o` joins the present list (+0xe09, count +0xe30) if
   new; the side's AI byte `+0x50 = (PlayerKi[o] != 1)`; a participant record (+0xe80 + 0x30k): the fleet's
   name (strcpy), owner +0x18, status +0x1c = 3, fleet +0x20, story id +0x24 (`+0x16`, i16), hit points +0x28
   and +0x2c = `+0x6c + +0x70`. Its vehicles (`Tankok` 1-based, in order) whose design (of `o`) is a fighter
   (`+0x36 != 0`) and whose fleet is `f`: merged by (fleet, design) into a temporary list of fighter groups
   (design, owner, race = design `+0x40`, class = design `+0x44`, fleet, count summed).
3. Planet `p ≠ 0`: planet owner +0xc8c = `Bolygok[p].+0x20`, else −1. For each building slot 1..=n of `p` whose
   type's tech (`EpuletekLeiro[type].+0`, exe 0x8EBA38 × 0x24) is 0x71..=0x74 or 0x79..=0x7b, finished
   (`+8 == 0`) and switched on (`+0x19 & 1`): `bolygofegyverhozzaad(tech, 1, 0, slot)`. Then `min(+0x114, 8)`
   defence ships `k = 1..`: setup entries of the planet owner's side `(race = owner, design = +0xfe + 2k, owner,
   ship = −k, colour 0, unit −1, experience −1)`. The planet owner joins the present list.
4. Per list entry `f ≥ 0`: every ship `i` (`Hajok` 1-based) of fleet `f` with a design: an entry `(race = design
   +0x40, design, owner, ship = i, colour = getDDSzin(design) [cosmetic], unit −1, experience = Hajok[i].+4)` on
   the owner's side list (lists accumulate over the owner's fleets); then the whole side list is sorted:
   (a) selection sort, descending by slot class `c(t) = t < 8 ? min(t, 3) : 11` of `t = terv2tipus(owner,
   design)` (strict `<`, first maximum kept); (b) in each run of equal class, a selection by "the smallest key
   strictly above the previous one", key = `race · 1000 + design +0x48` (starting at 0; entries whose key
   repeats are left where they are, and the run stops when nothing is above) [quirk].
5. New battle: +0x1f74 = 0, +0x1f78/+0x1f7c = the first list entry's fleet position (`+0x30/+0x34`). With the
   asteroid switch on and +0x2d70: N₀ asteroids `AddUjUrhajo(0, 0x8000, 0, 0, 0, −1, −1)`, then N₁ of 0x8001, N₂ of
   0x8002; each is placed by `repeat { cell = (rndlog(6, 0, gx), rndlog(6, 0, gy), rndlog(6, 0, gz));
   setshippos(0, cell); spin = rndlog(6, 0, 100)/20f × 3 } until utkereselfer`. The cell's fourth word is
   uninitialised stack [undefined; also copied to the destination and waypoint 0].
6. Per side `e` 0..=18 with list entries: `FelallasLoad("felallas0")` if fewer than 6 entries, else
   `"felallas1"` (`dataraw/spacewar/felallas<n>.bin`, `formats/formation.md`; for the human's side +0x2d88/+0x2d8c
   = −1/−1 resp. 0/0, UI); `felallit(e)`; a side whose fighter count +0x64 is 0 counts in `s` (+0x64 is only filled in step 7,
   so `s` = the number of sides with entries).
7. Every fighter group adds its count to its owner's side +0x64.
8. Sides with entries or fighters (+0x60 or +0x64 ≠ 0), in empire order, get a centre by a counter `k` that
   starts at `s`: k = 0 → (0, 0, −50), 1 → (0, 0, 50), 2 → (0, −50, 0), 3 → (0, 50, 0), ≥ 4 →
   unchanged; `k += 1` each.
9. Fighters: `scale = min(1, 60 / (f32)Σcount)` (1 when no fighters); each group with `count ≠ 0`:
   `AddUjVadasz(race, class − 1, owner, fleet, design, count, max(1, (int)((f32)count · scale)))`.
10. +0x2d85 = (human's side has ≥ 6 entries and SP), +0x2d86 = 0. Shown: the graphics; else +0x1568..+0x1570 = 0.
    `resztvevobirodalmakujraszamol` (§7.5).

`felallit(e)` (exe 0x572ef0): the side's entries that already have a unit lose it (alive := 0; only after a
previous `felallit` in a join). Per present empire with live units (owner ≠ 0, in empire order): its mean
unit cell (`Σcell / n`, integer), as `d_k = (cx − gx/2, cy − gy/2, 0)` (f32), normalised. Direction: no empire →
(0, −1, 0); one → `−d₀`; two → `m = (d₀ + d₁)·0.5`: `−m` if `|m| ≠ 0`, else `(d₀.y, −d₀.x, d₀.z)`; three or more →
**uninitialised** (the exe leaves the stack value) [undefined; the port uses (0, −1, 0)]. Normalised, x and y
negated, then `felallasforgat(formation, dir)` (0x572d30): `a, b = n(dir.x, dir.y, 0)`; every slot cell and
waypoint cell `(fx, fy) = (c₀ − gx/2, c₁ − gy/2)` (f32) becomes `((int)(fx·b + fy·a + gx/2), (int)((fy·b −
fx·a) + gy/2))`, `c₂, c₃` kept. Then every live unit (any owner, asteroids too) closes the slots `k` with `w_k
≥ 0` (slot class, the cell's fourth word) within `tav(unit, pos(k)) < scale(unit's race, unit's type)·1024 +
scale(unit's race, w_k)·1024` (`w_k := −1`). Then `AddUjUrhajo` for each entry (stores the unit index), then
every live unit of side `e`: heading `= n(−pos)` (`(0, 0, −1)` at the origin) and, with more than one
waypoint, `nextwaypoint` (§7.2). `pos(c) = ((c₂ − gz/2)·cell_z, (c₀ − gx/2)·cell_x, (c₁ − gy/2)·cell_y)`.

**Formation pick** (`loadfelallas(name)` exe 0x578850 / ELF 0x6a5a60, outside the editor 0xE72DD3): `SpaceWarFrissit2` (exe 0x56d870) calls it with `"felallas<+0x2d88 + 1>"` whenever `+0x2d88 != +0x2d8c` (then `+0x2d8c = +0x2d88`), once the screen is past its first frame and fade-in (`+0x1f98`, `+0x1fb9`); the check is not guarded by the phase flag `+0x2d85`. With entries in the human's list (side `+0xc3c`: pointer `+0x5c`, count `+0x60`; 0x1c-byte entries race, design, owner, ship, colour, unit, experience; `AddFleet` frees every side's list when it starts, so it is the last `AddFleet`'s): `FelallasLoad` (0x566eb0) into `aktfelallas` 0xE72DB8, `felallit(human)`, `aktfelallas` freed. `felallit` kills the entries' units and `AddUjUrhajo` reuses the first dead slots, so the stream-6 draws are each new unit's look and its mounts' timers (§7.10). Verified bit-exact on 29 exe picks (`run_spwc.py --formation N`, `formation_pick_matches_exe`; `spwc-form-1/3`, `spwf-form-2`: units, the list, the ship-type footprints and all 21 streams).

`helykeres(formation, t, race)` (0x573cd0): with `c = c(t)` (§step 4a): for `c < 8` the slot with the smallest
`w` such that `c ≤ w ≤ 10` (first of equals); for `c = 11` the first slot with `w = 11`; none → −1. The chosen
slot then closes every slot `j` (itself included) with `w_j ≥ 0` and `tav(pos(chosen), pos(j)) < scale(race,
t)·1024 + scale(race, w_j)·1024`.

`AddUjUrhajo(race, design, owner, ship, colour, unit = −1, xp)` (0x565800):
- A design (`< 0x8000`): `model = design +0x48`, `hp = design +0x30 · 15`, `t = terv2tipus(owner, design)`, and
  the ship-type record `T = tipusnevek[race·12 + t]` (exe 0x91F488, 0x30 B) gets its footprint:
  `+0x28 = (int)(((T.scale·1024)/cell + cell)·0.5 + 1)`, `+0x2c = (int)(((T.scale·1024)/cell + cell_z)·0.5)`
  (f32). Scenery (`0x8000 + t`): type `t`, hp 10000, `model` uninitialised. Negative `t` → 0.
- Owner ≠ 0 (outside the editor 0xE72DD3): `s = helykeres(formation, t, race)`. A slot: position `pos(slot)`.
  No slot: if some live unit has this owner, race, model, fewer than 10000 ships and its first ship in the same
  fleet as `ship`, the ship **joins** the live unit of this owner and type with the fewest ships whose first
  ship is in that fleet (a defence ship `−k` has no record: the exe reads `Hajok[−k].fleet` from the heap
  bytes before the array [undefined]; seen deciding merges both ways, the port uses `−k`): a unit with ≥ 10000 ships → return 0; else hp += hp, experience += xp, the ship is
  appended (count +1), and the pods and weapons below are added to it. If none qualifies [original bug]: the
  pods and weapons go to the **last unit examined** and the function returns `ship`. Otherwise the ship gets
  a new unit at the origin.
- A new unit: the first dead unit if 119 or more exist (none free → −1), else the first dead one, else a new
  one appended; `+0x88c + 4·(t + owner·12)` += 1. Fields: alive, `+1` = 1, race +4, type +8, design +0xc, model
  +0x10, owner +0x14, hp +0x1c, damage/shield 0, ships `[ship]` (count +0x1b0 = 1, hp share +0x1b4 = 1),
  waypoints 1 (+0x2c), position = destination = step, velocity and speed 0, spin = the side's rotation
  (`+0x2c..+0x34`), target/order 0, heading (0, 0, −1), the drawn and half-way headings 0, plan 0,
  +0x168..+0x174 0, explosion 0, +0x178 = 10, +0x17c..+0x180 0, attacker 0, picked −1 (+0x198), id = index
  (+0x19c), experience +0x1a4 = xp, `+0x1ac = rndlog(6, 0, 3)` [cosmetic], weapon mask +0x1bc = all, +0x1b8 = −1,
  disable/takeover/retreat 0, original owner +0x1c8 = owner, pods 0, cloak 0, idle 0, colour +0x224, type record
  +0x214 = `&T`, lightning 0, path fails 0; the cells (`pos2racspos`: `(int)((cell·0.5 + p)/cell + g/2)`, §7.2)
  of position, destination and step. A slot sets the waypoint count = the slot's count and copies its
  waypoint cells.
- Then (new or joined) the hull's built-in weapons `alapfegyverek[race·12 + t]` (exe 0xE5E08C, 5 × (i32, i32)):
  this BSS table is never written, so none [dead code]; a design's pods `k = 1..=+0x4c` (`id = +0x50+k`, `n =
  +0x5d+k`): 0x11..=0x20 → `fegyverhozzaad(weapon id − 0x11, n)`, 0x41..=0x48 → weapon `id − 0x34`; shields 0x31..=
  0x36: `shield = (int)((f64)hp · {0.15, 0.3, 0.45, 0.6, 0.8} + (f64)shield)` resp. `+ hp`, flag bits 1, 2, 4,
  8, 0x10, 0x800; 0x25 → pod 0x20, 0x27 → 0x40, 0x28 → 0x80, 0x29 → 0x100, 0x2a → 0x200, 0x37 → 0x400 (`hp` = the
  joining ship's).

`fegyverhozzaad(unit, w, n, ship)` (0x572650): a mount of weapon `w` already there gains `n` barrels; else a new
mount: weapon, kind, barrels `n`, reload, timer = timer set = `rndlog(6, 2r/3, r − 1)`, the weapon's
damage[12], +0x48..+0x64 (hit, spread, turn, speed, accel, duration, range, form), the three flags, `+0x6c =
ship`; a kind ≠ 2 mount sets the shortest range (+0x168, when 0 or larger) and the longest (+0x16c).

`terv2tipus(owner, design)` (0x582010) of `v = design +0x48`: 0x4d..=0x50 → `v − 0x45` (8..=11; copies a name
into the type record, cosmetic); `v > 0x11` → 6; 0xc/0xd/0xe → 4/5/6; else `(v − 5)/2` (C division).

`AddUjVadasz(race, class, owner, fleet, design, count, groups)` (0x566420): `c = clamp(class, 0, 3)`; `base =
count / groups`, `rest = count`, `r = base · groups`; each group gets `base + 1` when `r < rest`, else
`base`; then `rest −=` its count and `r −= base` (the first `count mod groups` groups get one more). Position = the owner's side
centre + `(f32)(((rndlog(6, 200, 999) − 500)·12)/500)` per axis (x, y, z drawn in that order, integer
division); destination = position; hit points `+0x84` = design `+0x2c`; design +0x6c, owner +0x70, race +0x74,
class +0x78, fleet +0x7c, count +0x80; target 0; state, timers, speed, goal distances, retreat 0; direction
`spwnormalizevektor(−pos)`, velocity = direction · 0.2f; +0xa0 = c. The turn +0x50 is not written
[undefined; never read before the tick writes it].

`bolygofegyverhozzaad(tech, n, x, slot)` (0x571d00), at most 50 guns: weapon 20..=23 for techs 0x71..=0x74,
else −1 (0x79..=0x7b: an armour-only gun); armour +0xa0 = 1.3/1.5/1.8 for 0x79/0x7a/0x7b, else 1. With a
weapon: kind, count +0x1c = n, reload +0x28, timer = timer set = `rndlog(6, 0, r − 1)` (integer), the damage
and weapon fields, `+0x80 = x`. Then `v = n(−100, rndlog(6, 0, 99) − 80, rndlog(6, 0, 99) − 180)`, `s =
(f32)(size << 10)`; position `(P.x + v.z·s, P.y + v.y·s, P.z + v.x·s)` (P = +0x84c); slot +0x84, active +0x88 = 1,
+0x8c/+0x90 = 0, target −1, attacker 0, index +0xa4, `+0x10 = 1000` if the planet is the human's, else 1.

`utkereselfer(i)` (0x614190): fill the search grid (§7.2) with 0xFDE8; every other live unit `j` blocks the box
`cell(j) ± (sxy_i + sxy_j − 2, ·, sz_i + sz_j − 2)` (`TeglaRajzol`); true when unit `i` is dead or the box
`cell(i) ± (2·sxy_i − 2, ·, 2·sz_i − 2)` holds no blocked cell (`TeglaRajzol2`, clamped like `TeglaRajzol`).

### 7.11 The end
In the tick (§7.1 step 4), when the end check finds the battle over on screen 0xB: `AktKepKell = 2` (leave the
screen) unless 0xBAE24D; if shown and `AktKepKell ≠ 3`: `SetIdoLelassit(0)` and, when ground units wait
(+0x2004), not yet started (+0x2005), the first participant's original owner still has units (`counts[owner]
> 0`) and its fleet carries ground units (`+0x5c > 0`): `GroundWarIndit(participant 0's fleet, planet)`,
+0x2005 = 1. Leaving the screen (`SpaceWar_Kilep` 0x562b50): a battle not yet closed (+0x1fb8) zooms out
(`Starmapvisszaallit`, which sets +0x1fb8) and pauses; a closed one that is not finished (+0x1fba) stays and
pauses (a background battle, §6); a finished one: SP `SpaceWarVege` (MP: message 0x44d to the host).
`SpaceWarVege` (0x5706f0): `SpWarKiNyert` (0xBAA1EC) = winner +0xc48, `SpWarVesztesVisszaVonult` (0xBAA1DE) = 0,
`Story_SpaceBattleFinished(participants, n, 1)` (0x5c9180: `SpWarYouWasInBattle` 0xF26531 = some participant
is `EN`'s, `SpwFlottak`/`SpwFlottaNum` 0xF265C4/0xF265C8 = the records, `StoryEsemeny[3]` 0xF2F8AF = 1,
`StoryAdat+0x88` = 0, `Story_Check(1)`: the campaign script), then (`EndSpaceWar`, inlined) every unit's ship
and mount lists, the arrays and the side lists are freed and the slot goes inactive and hidden (+0x1f2e,
+0x1f2c = 0). Everything the battle did to the starmap (ships, fighters and fleets lost, planet ships and
buildings, fleets retreating) already happened during the ticks (§7.5, §7.7).

### 7.12 The human's orders (`MULTIGET_SpaceWar`)
Status: **ported and verified byte-exact** (`spacebattle::Order`, `Battle::order`, `Battle::arm_all`;
`traces/2026-10-16-spacebattle-orders.md`). The battle screen never calls the order functions itself: it sends
lockstep messages, in SP too, and `MULTIGET_SpaceWar` (exe 0x5615b0, ELF 0x67ef60) carries them out between frames
(two `Multi_GetNextTipus` loops: 0x6e..0x77, then 0x44c..0x456). Each names its battle slot; the handler points
`AktSpaceWarp` at it for the call and restores `AktSpaceWar` (0xE41788) after. No index is checked.

| Msg | Sender (ELF) | Values | Handler (exe) | Effect |
|---|---|---|---|---|
| 0x6f | none in the shipped code | fleet list | `SpaceWar_NewBattle` 0x563700 | a battle from `spFlottak`, screen 0xB (not ported) |
| 0x70 | `SpwC::JobbClick` 0x6b0210 | x, y, z, unit | `moveship` 0x57c4c0 | outside the editor (`spacewareditormode` 0xE72DD3 = 0): unless disabled (+0x1c0): destination (+0x64) := point, `pos2racspos` into waypoint 0 (+0x74), destination := the cell's centre, step (+0xb4) := it, order := 1, +0x174 := −1.0, step cell := waypoint 0, path short (+0x180) := 1, plan position 0, plan tick −1000, path fails (+0x254) 0. The editor branches (scenery, waypoint queue +0x2c) are the editor's |
| 0x71 | `JobbClick` | unit, target | `attack` 0x571a50 | target ≠ −1, not disabled, has a weapon range (+0x168 ≠ 0): destination := the target's position clamped two cells inside, its cell and centre, +0x174 := −1, target, target kind 1, order 3, path short 1, plan 0 / −1000 |
| 0x72 | `HandleRetreatWindow` 0x6814e0, `SpaceWarKeyboardHandler` | — (the sender) | `retreat(sender)` 0x57ee20 | §7.7 |
| 0x73 | `JobbClick` | unit, gun | `shipattackscolonygun` 0x5817a0 | gun ≠ −1 and a weapon range: target := gun, kind 2, order 3 (no disabled test) |
| 0x74 | `JobbClick` | gun, unit | `colonygunattacksship` 0x5720d0 | unit ≠ −1: the gun's forced target (+0x98) := unit |
| 0x75 | cheat console `ExecuteString` | — (the sender) | `fegyverhozzaad` 0x572650 | every active slot, every live unit of the sender: one barrel of each weapon 0..19 (`fegyverhozzaad(u, w, 1, ships[0])`: a new mount draws its reload on stream 6, line 0x12ed) |
| 0x76 | cheat console | battle, unit | `felrobban` 0x5737e0 | `felrobban(unit, 0, 0, 1, 1)` as many times as the unit's ship count (+0x1b0) before the loop (dead units too) |
| 0x77 | `SpwC::infoszamol` | value, side | — | side record `+0x6c·side + 0x40` (fighter aggression) := value |
| 0x44d | `SpaceWar_Kilep` 0x562b50 (MP) | — (the sender) | — | `SpWar_MultiCsata_vegetert[sender]` (0xBAD118 + 4e) := 1 (§7.13) |
| 0x44e | `JobbClick` (direct-fire cursor) | unit, target, mount | `DirectFire` 0x566ba0 | live unit: `rndlog(6, 2r/3, r − 1)` (line 0x3702, r = mount reload +0x14) into the mount's timer and its set value, then `Loves2(owner, position, 0, unit, target, weapon, barrels)` (§7.4) |
| 0x44f | `SpwC::main` | unit, keep, set | `FegyverKibeKapcsol` 0x566e80 | live unit: weapon mask (+0x1bc) := `(mask & keep) | set` |

`JobbClick` sends moves for every selected own unit with the point clamped two cells inside the arena (y, z,
then the height x; the 2D cursor sends the unit's own height), attacks for every selected own unit on the hovered
enemy, gun orders only for the human's own planet guns; the port's `Battle::screen_point` is that clamp. The
handler doesn't clamp: a point outside makes the path search diverge (seen in a trace with points the screen
can't send; not a reachable input). `MULTISEND_SpWarSynch` (exe 0x59f780, ELF 0x71d520) only sends message 0x402
every 81st call from the host while an MP battle runs: `MULTIGET_Starmap` answers with the `SynchStruct` resync
(0x401) — the original's desync repair, replaced by ADR 0007's hashes; no state.

### 7.13 The real-time battle in MP
Exe globals: `SpWar_MultiCsata_folyamatban` 0xBAD114 (ELF 0x144808c), `…_vegetert[e]` 0xBAD118 + 4e (ELF
0x1448090; `…_mehet` has no readers). `SpaceWarIndit` (§4) routes an MP battle to real time only with two human
sides (`MultiplayerBattleStatusz` 0 or 1); it clears the listed fleets' orders, `SpaceWar_NewBattle`, and on every
machine (the strategic sim is the same everywhere) `AktKepKell = 0xB`, the flag := 1 and `vegetert[1..=8]` := 0.
While the flag is set, `Fake_FRISSITES` runs no clock and no strategic system (§`ground-combat.md` 11.17: only
`MULTISEND_SpWarSynch`, `GroundWarSetBattleStatus`, `IdoMultiPlayerBattle`, `FRISSIT_SpaceWar`; not
`FRISSIT_Grwar`), and `FRISSIT_SpaceWar` ticks **every** slot in MP (`frissit` → `SpaceWarFrissit`
`IdoSpeedSzorzoBattle` times, §7.1) on the 20 Hz loop. `IdoSpeedSzorzoBattle` is set with the game speed by
`SetIdoSpeed` (ELF 0x6233d0; MP: ×4 → 1, ×8 → 2, ×16 → 2; SP 1/2/4) and isn't saved. The end check closes the
battle on every screen (§7.11); each player leaving it sends 0x44d, and `MULTIGET_SpaceWar`'s tail (every frame):
while the flag is set, it stays set as long as a human empire 1..=8 (`PlayerKi == 1`) hasn't left; else 0 and
`SpaceWarVege` on `AktSpaceWar` (the slot `SpaceWar_NewBattle` used last).

The canonical view (ours, `multiplayer.md` §7) runs this on the sim clock (`game::realtime`, `Game::space_order`,
`Game::space_battle_left`, `Game::space_battles_settle` before each clock step): the slots, the live ship-type table
and `AktSpaceWar` are synced state (snapshot v4), the battle is shown (every human is on its screen, so the victory
flight and the ground war after a won planet battle run), the screen fade is 0, the battle sees `MultiPlayerMode`
= 1, and a loaded game without a battle speed gets the MP `SetIdoSpeed` value. Tested by
`ig2-net/tests/lockstep.rs::two_humans_fight_a_real_time_battle_in_lockstep`.

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
- Real-time battle (`tools/trace/run_spwc.py` + `spwc.js`, replayed by `tests/spacebattle_trace.rs`): real
  battles on the battle screen; every `SpaceWarFrissit` call recorded as byte diffs of the battle, its units,
  fighters, guns, pools, squads, the strategic records and the streams; each tick decoded, stepped by the
  port and compared (`--fuzz`, `--planets`, `--ai-human`; see the report).
