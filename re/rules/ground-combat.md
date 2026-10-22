# Ground combat (`WF*`): routing, landing, the auto-resolved ground war

Status: **§2–§6 verified at runtime** (§10, `traces/2026-10-16-ground.md`) and ported to
`engine/crates/ig2-sim/src/ground.rs`: 324 staged landings + auto-resolved wars and 150 `GroundWarIndit`
calls reproduced exactly (every unit group, the vehicle table, fleet sums, buildings, stream 2, the count of
libc `rand()` draws). The **real-time** ground war is §11: placement on the map with the landing zone,
terrain heights and orientation, defence-building turrets and the battle start are ported and verified; the
battle tick (`WF::Refresh`, `WF::MI`, shots, projectiles, mines, transporters) is in §9.

A ground war starts when a fleet carrying ground vehicles (design class 2, `space-combat.md` §1) wins a
planet battle or arrives at an undefended enemy planet: `GroundWarIndit(fleet, planet)` (called from
`SpaceWarIndit` after a won planet battle, `space-combat.md` §4 step 6, and from the starmap arrivals,
`fleet-movement.md` §4.3). Like the space battle it either opens the real-time battle (colony screen 3,
`StartGrWar`) or **resolves it at once** with `WF::Simulate`, an abstract tick loop over the same unit
records the real-time battle uses.

Every planet owns one persistent battlefield `WF` (`Warfare[p]`): the planet's garrison vehicles and its
defence-building turrets live there between battles (they are added when built: `TankNew(−p, …)` →
`AddTank`, `WF::AddBuildingTurrets`); attackers are added by `AddTanksToPlanet` when a war starts and
leave it again when it ends.

**The exe is normative.** Its `WF::Simulate` differs from the ELF's in several places (inlined
`NoTarget`/`Stop`, the target refresh rule, the second-enemy search, call arguments left on the stack):
everything below follows the exe and was checked in the asm. Randomness: **stream 2** (`rndlog(2, …)`,
`grwar.cpp` lines in the table below) plus libc `rand()` (§7, ADR 0003). Stream 13 (`WF::MI`) belongs to
the real-time battle only. No rule values here: the tables (§1.3) are read from the user's exe (ADR 0004).

| Function | exe | ELF | Role |
|---|---|---|---|
| `GroundWarIndit(f, p)` | 0x59c2d0 | 0x712370 | routing (§2) and the auto-resolve's tail (§6.3) |
| `AddTanksToPlanet(f, p, sim)` | 0x45e320 | 0x49eb00 | landing (§3) |
| `WF::AddTank(d, hp, owner, origin, merge, x, y, bits)` | 0x45ddb0 | 0x46d0b0 | add one vehicle (§3.2) |
| `WF_Tank::WF_Tank` | 0x457020 | 0x4f0740 | unit constructor (one libc `rand()`, §7) |
| `WF::CheckDesign(u)` | 0x45fd20 | 0x46dd90 | pods → unit abilities (§3.3) |
| `WF_Tank::SetPosG(x, y)` | 0x49c250 | 0x46dbc0 | grid position (§3.4) |
| `WF::RearrangeTankGroups(owner)` | 0x48ca00 | 0x49f1e0 | ≤ 20 groups per owner (§4) |
| `JoinTwoSmallestGroup` / `JoinGroups` (`RakjAt`) | 0x483c60 / 0x48c6c0 | 0x4b0f20 / 0x4b0ed0 | §4 |
| `EqualizeC` / `AtlagoldC` (`RakjAtC`) | 0x474490 / 0x48c800 | 0x4b12b0 / 0x4b5bd0 | §4 |
| `GetFreeGroupSpaceC` / `GreatestGroupSizeC` / `GetRealGreatestGroupC` | 0x47ac70 / 0x47c4b0 / 0x47ba60 | 0x4b1180 / 0x4b1510 / 0x4b1640 | §4 |
| `GetGreatestGroupC` / `GetSmallestGroupC` / `WF_Tank::FirstFreeC` | 0x47ace0 / 0x47be00 / 0x477890 | 0x4b56c0 / 0x4b57c0 / 0x4b5910 | §4 |
| `HalveGroupC(u)` | 0x47ccb0 | 0x4b1720 | split **one** member off (§4) |
| `WF::Simulate` | 0x4a3730 (ported name `MegkeresHozzavaloTalalmanyok`: wrong) | 0x4a8b10 | the battle loop (§5) |
| `WF::GetStatus` | 0x47be90 | 0x496010 | who holds the field (§5.1) |
| `WF::GetEnemy` / `GetEnemyNJ` | 0x47a980 / 0x47aa40 | 0x4a82c0 / 0x4a84c0 | targeting (§5.2) |
| `WF::GetThirdEnemy` (second: inlined) | 0x47c0d0 | 0x4a8a60 (0x4a89b0) | §5.2 |
| `WF::CheckFighterAttack` / `CheckBigShipAttack` | 0x460770 / 0x45fc90 | 0x4a4950 / 0x4a4a90 | §5.3 |
| `WF_Tank::SetTarget(t)` | 0x49c670 | 0x487f60 | §5.2 |
| `WF::Damage(u, dmg, by)` | 0x469820 | 0x45fa50 | §5.5 |
| `WF_Tank::Damage(dmg, by)` | 0x46bfa0 | 0x4662f0 | §5.6 |
| `WF_Tank::MegHaltValamelyik(k)` | 0x48b2d0 | 0x473ae0 | a member dies (§5.6) |
| `WF::RemoveTankGroup(u)` | 0x497c90 | 0x467f50 | a unit dies (§5.7) |
| `WF::RemoveTarget(t)` | 0x499070 | 0x463060 | forget a target (§5.7) |
| `WF::RemoveBuildingTurrets(slot)` | 0x497a70 | 0x471630 | §5.7 |
| `EpuletDamage(p, slot, dmg, turret, by)` | 0x53c6e0 | 0x61cc60 | §5.5 |
| `EpuletDelete(p, slot)` | 0x53c730 | 0x61cb00 | §5.7 |
| `GroundWarFinished(p, status, sim)` | 0x47c5d0 (ported name `KiszamolErtek`: wrong) | 0x4968c0 | §6.1 |
| `WF::InitMineSystem` / `WF::RepairAllTanks` | 0x483640 / 0x4991e0 | 0x4a54c0 / 0x497640 | §6.1 |
| `GroundWarSetBattleStatus` | 0x47caa0 | 0x49e820 | §6.4 |
| `TankDelete(f, d, n)` (core) / `TankNew(f, d, n)` | 0x541510 / 0x541750 | 0x616f70 / 0x618a70 | `space-combat.md` §5.3, `production-research.md` §5.1 |
| `SearchBuilding(p, type, working)` | 0x541150 | 0x616720 | finished buildings of a type |
| `LocateTankSuitablePosition(p, &x, &y, sx, sy)` | 0x486b90 | 0x488d80 | free map cell (§3.4, §11.1; `placement::locate_tank`) |
| `KemkedesFoldiharcLekezel(f, p)` | 0x53ef00 | 0x61f9d0 | spies learn designs/buildings (§2) |
| `Story_GroundBattleSimulateCheck` / `…Started` / `…FinishedSave` | 0x5c8e70 / 0x5c8fc0 / 0x5c8de0 | | campaign hooks (§2, §6.3) |

## 1. Data
### 1.1 The battlefield `WF` (exe `0x9C3244 + p·0x8EC` for planet p ≥ 1; ELF `Warfare + (p−1)·0xA20`)
| exe | ELF | Type | Field |
|---|---|---|---|
| +0x180 | +0x268 | i32 | unit slot count `n` |
| +0x188 | +0x270 | u8* | `alive[n]` |
| +0x18c | +0x278 | i32 | planet |
| +0x330 | +0x458 | `WF_Tank*[n]` | units (a dead slot's object is freed; `AddTank` reuses dead slots) |
| +0x334 | +0x45c | i32 | self-destruct countdown (`FRISSIT_Grwar`; −1 idle, also set when the `'+'` (0x2b) building is deleted) |
| +0x338 | +0x464 | i32 | the attacking fleet (`AddTanksToPlanet`) |
| +0x148 | +0x1f8 | i32 | end-of-battle countdown; −1 = idle (§5.1; `GetStatus` answers −1 unless idle or MP) |
| +0x88c | +0x9b8 | u8 | `WF::Simulate` running |
| +0x89f.. | +0x9cb.. | u8 × 18 | cleared by `GroundWarFinished` [unk; real-time] |
| +0x8b1 + s | +0x9dd + s | u8 `[19]` | side `s` has units on the field (`GetStatus`) |
| +0x8c8 | +0x9f8 | i32 | last status (§5.1) |
| +0x8cc | +0x9fc | i32 | status cache valid (cleared by `AddTank`, `Damage`, `RemoveTankGroup`, `RemoveBuildingTurrets`) |
| +0x8d8 + e | +0xa08 + e | u8 `[19]` | empire `e` has a radar unit (flag 0x800000) on the field |
| +0xd4/+0xd8/+0xdc, +0xe0/+0xe8/+0xe4 | | lists | projectiles (real-time; `RemoveTarget` also clears their targets) |

### 1.2 A unit (`WF_Tank`, exe 0x290 bytes, ELF 0x2a8): a **group** of 1..n vehicles of one design
| exe | ELF | Type | Field (constructor value) |
|---|---|---|---|
| +0x154 | +0x158 | i32* | member hit points `hp[size]` |
| +0x158 | +0x160 | i32 | max hit points per member (`TankAlapHP[hull]`, §1.3) |
| +0x15c | +0x164 | i32 | design (of the owner's `Tervek`) |
| +0x190 | +0x198 | i32 | `size` (0) |
| +0x194 | +0x19c | i32 | owner: who controls it now (a seized unit changes it, §5.4) |
| +0x198 | +0x1a0 | i32 | side: the original owner (status, §5.1) |
| +0x19c | +0x1a8 | i32* | member origin `origin[size]`: fleet index (attackers), −p (garrison of p), −1 (partner) |
| +0x188 | +0x190 | u32 | flags (**1**), table below |
| +0x18c | +0x194 | u32 | flags2 (0): bit 1 = speed reset pending (§3.2/§4), bit 2 = excluded from status |
| +0x1d0 | +0x1e0 | u32 | weapons (0), from the pods (§3.3) |
| +0x11c | | i32 | weapon pods: the most slots carrying the same weapon (§3.3) |
| +0x120 | | i32 | ammunition of weapon 0x10: `size × 3` (§3.3, §6.1); −3 per lost member, ≥ 0 |
| +0x1cc | +0x1dc | i32 | regeneration per tick (pod 0x9e: 20; else 0) |
| +0x288 | | i32 | air-strike units (`size` if weapons & 0x28, else 0); −1 per lost member, ≥ 0 |
| +0x130 | | u8 | has a gun (§3.3) |
| +0x134 | +0x13c | i32 | ground hull `design.hull − 0x13` (−1) |
| +0x138 | +0x140 | i32 | size before the last `AddTank` (−1; `JoinGroups` and `RearrangeTankGroups` reset it) |
| +0x1a0/+0x1a4 | | i32 | grid cell x, y (§3.4) |
| +0x1a8/+0x1ac, +0x1b0/+0x1b4 | | i32 | grid copies: last cell, move destination |
| +0x1b8, +0x1bc | | i32 | movement state (0, −1 after a reset) |
| +0x1d4/+0x1d8 | +0x1e4/+0x1e8 | f32 | map position (§3.4) |
| +0x1e0 | +0x1f0 | i32 | carrier: the unit this partner belongs to |
| +0x1e4 | +0x1f4 | i32 | partner unit (−1): the transporter pod's escort (§3.1) |
| +0x1e8 | +0x1f8 | i32 | paralysis countdown (0) |
| +0x1ec | +0x1fc | i32 | seizure countdown (0) |
| +0x238 | +0x250 | i32 | building slot (turrets) (0) |
| +0x23c | +0x254 | i32 | target (meaningful only with flag 0x100; 10000 = none) |
| +0x244 | +0x25c | i32 | own index |
| +0x25c / +0x260 | +0x274 / +0x278 | i32 | last hit: 10000 / attacker (written by `WF_Tank::Damage`) |
| +0x264 / +0x268 / +0x26c | +0x27c.. | f32 | the dropship (`WF_Hordozo`): offset on the map axes, height (§11.16) |
| +0x278 | +0x290 | u32 | dropship state (1: away, §11.16) |
| +0x27c | | i32 | (10000) cleared to 10000 by `RemoveTarget` [real-time aim] |
| +0x280 | +0x298 | i32 | planet |

Flags (`+0x188`): 1 damage ×1.2 (set by the constructor; nothing in the auto path clears it), 2 not
placed (in a transporter; for turrets: destroyed), 0x10 mesh loaded, 0x40/0x80/0x400/0x800 movement
(`SetPosG` sets 0x40, clears the rest), 0x100 has a target, 0x200 [real-time], 0x8000 partner (the escort
of a transporter pod), 0x10000 paralysed, 0x20000 mine, 0x40000 stealth (pod 0x96), 0x80000 / 0x100000 /
0x200000 pods 0x97 / 0x98 / 0x99 (0x200000 also for hull 2), 0x400000 transporter pod 0x9f,
0x800000 radar pod 0xa0, 0x1000000 seized, 0x4000000 building turret, 0x8000000 landed by
`AddTanksToPlanet` (SP), 0x20000000 keep orders at a battle start, 0x40000000 (with 2: not counted).
"Destroyed turret" below = flags 2 and 0x4000000 both set.

Weapons (`+0x1d0`) and their `WF_Weapons` entry (§1.3): 1 → W0, 0x2000 → W1, 0x10 → W2, 4 (paralyser) → W3,
0x100 → W4, 0x200 → W5, 0x400 → W6, 0x800 → W7, 2 → W8, 0x1000 (manipulator) → W9, 0x40 (kamikaze) → W10,
0x80 (big kamikaze) → W11, 8 (big-ship strike) → W12, 0x20 (fighter strike) → W13; 0x4000..0x20000
[real-time special weapons].

### 1.3 Tables (rule values: read from the user's exe)
| Table | exe | ELF | Layout |
|---|---|---|---|
| `WF_Weapons` | 0x8F82C0 | 0x11645c0 | 14 × {i32 min, i32 max, i32 period, i32 [unk]}: auto-resolve damage |
| `WF_NonSimulateWeapons` | 0x8F83A0 | 0x11646a0 | 14 × {min, max, reload, range}: real-time (`CalcDamage`; `CheckDesign` reads reload/range) |
| `WF_FortressWeapons` | 0x8F8480 | 0x1164780 | 14 × {min, max, reload, range}: real-time turrets |
| `TankAlapHP` | 0x8F1F14 | 0x1164860 | i32 × 5: member hit points of ground hulls 0x13..=0x17 |
| `EpuletekLeiro[type].+8` | 0x8EBA40 | | building hit points (`colony-economy.md`) |

## 2. Routing (`GroundWarIndit(f, p)`)
1. Fleet `f` without ground vehicles (`+0x5c < 1`) → nothing. `BolygoInfoIs(p, −0x21ac)` (a story
   planet) → `Grwar8020hoz(f)` (campaign) and nothing else.
2. `KemkedesFoldiharcLekezel(f, p)` (exe 0x53ef00): for owners < 9, the planet's owner sees every class-2 vehicle
   design of `f` and the attacker every class-2 design of the garrison (vehicles with fleet `−p`)
   (`KemkedesTervLekezel` 0x53f060: hull tech and pod techs, `space-combat.md` §4 step 2), and the attacker sees
   the tech of every building in slots `1..=HanyEpuletLefoglalva[p]` with a kind (finished or not; seen matrix,
   `colony-side.md` §2).
3. **Lopsided** (exe precision: f64 division, rounded to f32):
   `(f32)(((f64)f.+0x68 + 0.1) / ((f64)(p.+0x5c + BolygoMennyiErodEro[p]) + 0.1)) > 5.0f`, unless debug
   switch 0xB7723C is set; then cleared again if 0xB77230 is set. (`+0x68` fleet ground strength, `p.+0x5c`
   garrison strength, `BolygoMennyiErodEro` exe 0xB99CB8 the planet's fortress strength.)
4. `Story_GroundBattleSimulateCheck(f, p)` (exe 0x5c8e70) **clears** `Story_GrWarSimulateTiltas` (0xF42670) and
   sets it again when a running campaign script forbids simulating this planet: story 0x328 with story planet
   0x1F90, 0x32E with 0x1FCC, or 0x338 with 0x2031 while variable 0x2034 is 0 (`st_Story(id, 1)` sets
   `StoryLastChecked`), equal to `p`; its value before the call doesn't matter (runtime-confirmed).
5. **Simulate** (SP): `(((0xB77234 ≠ 0 or lopsided) and not tiltas) or (neither f's owner nor p's owner
   is human-controlled))`. MP: real-time only when `MultiplayerBattleStatusz ∈ {0, 2}` and **both**
   owners are human-controlled; otherwise simulate.
6. `AddTanksToPlanet(f, p, simulate)` (§3); `Story_GroundBattleStarted(f, p)`; unless `StoryAdat+0x88`
   (SP) the attack is a diplomatic incident (`DiplomaciaMegtamadas`, `BirodalomMegtamad`,
   `diplomacy.md`); `StoryAdat+0x88 := 0`.
7. Real-time: `GrWar_MultiCsata_bolygo := p` (MP: in-progress flag), `StartGrWar(p)`. Simulate: §5, then §6.3.
   Either way `GroundWarSetBattleStatus` (§6.4).

Port: `Game::ground_war` (`game/battles.rs`) runs §2–§6.4 on the save state where the starmap calls it (and after a
won planet battle), with the planet's battlefield decoded from the save (`SaveState::ground_field`); the real-time
route is reported (`BattleEvent::GroundRealTime`). Verified in the save replay (`traces/2026-10-16-save-replay.md`,
`replay-quick1-battles-ground-x16`): five wars against fortress turrets and garrisons, four won (two lopsided),
with the captures, the messages (§6.3) and the repairs after them (§6.5).

## 3. Landing (`AddTanksToPlanet(f, p, sim)`)
### 3.1 The loop
`WF.attack_fleet := f`. For each `Tankok` entry `t = 1..=count` with `design ≠ 0x7fff`, `fleet == f` and
a class-2 design (of `f`'s owner `o`), for each of its `count` vehicles:
1. Position: `sim` → cell (100, 100); else `LocateTankSuitablePosition(p, start (0x78, 0x78))`, falling
   back to the default start.
2. `u = AddTank(d, TankAlapHP[design.hull], o, f, merge = true, x, y, 0)`.
3. SP: `u.flags |= 0x8000000 | 2` (landed, still in the transporter).
4. `o` not human-controlled (`PlayerKi[o] ≠ 1`): `SetPosG(u, x, y)` again, `u.flags &= ~2`, transporter
   state `+0x278 = (… & ~0x25) | 0x1a` (unloaded at once).
5. `u.flags & 0x400000` (transporter pod): `LocateTankSuitablePosition(p, default)` — **also when `sim`** —
   `v = AddTank(d, 0x8000, o, −1, merge = true, …)` (`hp` 0x8000 = "partner": the member gets 1 hit point
   and `v.flags |= 0x8000`; it never merges, since the stored max is 1), `SetPosG(v)`, `u.partner := v`,
   `v.carrier := u`, speed reset, `v.flags |= 0x8000000 | 2` (also in MP).
After the entry's vehicles: `TankDelete(f, d, count)` (they left the fleet; fleet sums as `space-combat.md` §3).
Finally `RearrangeTankGroups(o)` (§4).

### 3.2 `AddTank(d, hp, owner, origin, merge, x, y, bits)` (cache `+0x8cc := 0`)
Slot: the first `i` that is dead, or alive with `design == d`, `owner == owner`, `max_hp == hp` (hp as
passed: 0x8000 never matches) and `merge`; none → append a dead slot. A dead slot gets a **new unit**
(constructor: one libc `rand()`, §7; defaults in §1.2). Then: `+0x138 == −1` → `+0x138 := size`;
`alive := 1`; `design := d`; append a member; `hp == 0x8000` → `hp := 1`, `flags |= 0x8000`; the member's
`origin := origin`, `hp[last] := hp`, `max_hp := hp`.
A new group (`size == 1`): `owner := side := owner`; `+0x1c0 ≠ 0.0` → `flags |= 0x200`, `+0x1c0 := 0`;
movement/aim defaults; `+0x280 := planet`, `+0x244 := i`; `SetPosG(x, y)`; `hull := design.hull − 0x13`;
`flags2 |= 2`; `bits` 2 / 4 / 0x8000000 → flags; `CheckDesign(i)`; real-time speed from `TankAlapSpeed`.

### 3.3 `CheckDesign(u)`: pods → abilities
Clears `+0x130`, weapons, regeneration and flags 0x40000..0x800000 (six bits); fire range `+0x148 := 1.0`.
Walks the design's pod slots `k < GetMaxPodok(u.owner, design.hull)` — **the unit's owner (empire) is
passed as the pod-table index**, unlike `TervUjraSzamol` (`space-combat.md` §2.4) — reading
`tPodTalalmany[k]` (`+0x51 + k`; the counts `+0x5e..` are not read):

| Pod tech | Effect | Counter |
|---|---|---|
| 0x96 / 0x97 / 0x98 / 0x99 | flags 0x40000 / 0x80000 / 0x100000 / 0x200000 | |
| 0x9a / 0x9b | weapons 0x40 / 0x80 | c10 / c11 |
| 0x9c / 0x9d | weapons 0x20 / 8 | c13 / c12 |
| 0x9e | regeneration := 20 | |
| 0x9f / 0xa0 | flags 0x400000 / 0x800000 | |
| 0xa1 / 0xa2 / 0xa3 / 0xa4 / 0xa5 | weapons 0x100 / 0x200 / 2 / 0x400 / 0x800 | c4 / c5 / c8 / c6 / c7 |
| 0xa6 / 0xa7 / 0xa8 / 0xa9 | weapons 1 / 0x2000 / 4 / 0x1000 | c0 / c1 / c3 / c9 |
| 0xaa..=0xad | weapons 0x4000 / 0x8000 / 0x10000 / 0x20000, "special" | |

`pods (+0x11c) := max(c0..c13)` (c2 is never counted). Hull index 4 with a special weapon → weapons
`|= 0x10`. `+0x130 := 1` if any of 0x100, 0x200, 2, 0x400, 0x800, 1, 0x2000, 4, 0x1000. Hull index 2 →
flags `|= 0x200000`. Real-time only: fire range raised to `(f32)(int)((flags>>20 & 1)·2 + range)` per
weapon, reload `+0x1f0..+0x214 := (f32)(reload / count)` (integer division), both from
`WF_NonSimulateWeapons`. Finally `ammo := weapons & 0x10 ? size·3 : 0` and `air := weapons & 0x28 ?
size : 0` (with the size at that moment: 1 for a new group).

### 3.4 Positions
`SetPosG(x, y)`: cell `(x, y)` (also `+0x1a8/+0x1ac`); map position (f32, exe constants `W` 0x8EC960,
`H` 0x8EC964): `px = −(((x + 0.5)·0.5/99·100·W)/99 − W·0.5)`, `py = ((y + 0.5)·0.5/99·100·H)/99 − H·0.5`;
flags `= flags & ~0xC80 | 0x40`; flags2 `&= ~2`. Positions only matter to mine explosions (§5.7) and
the real-time battle. `LocateTankSuitablePosition(p, …, sx, sy)` searches the colony map like the
building placement (`colony-placement.md`: grid from the map, buildings and placed units marked, square
spiral `LocatePos` 0x486610 with predicate 0x46db30 from the start cell, default start = the buildings'
integer centroid or the surface's start) [not ported: a hook].

## 4. Group arrangement (`RearrangeTankGroups(o)`)
Each owner keeps at most **20 groups** on a field. "Free" = `max(0, 20 − #alive units of o without flags
0x4028000)` (turrets, mines and partners don't count).
```
while free == 0 and JoinTwoSmallest(o) and JoinTwoSmallest(o) and JoinTwoSmallest(o): (repeat)
EqualizeC(o)
while free > 0 and GreatestGroupSizeC(o) > 1:
    g = GetRealGreatestGroupC(o); if g.size == 1: return        (skips the final pass) [original quirk]
    HalveGroupC(g); EqualizeC(o)
for every alive unit: +0x138 := −1; flags2 & 2 → speed reset, flags2 &= ~2
```
- `JoinTwoSmallest(o)`: over pairs `i, j` (both from the last slot down; alive, owner `o`, not 0x4020000,
  same design, `i ≠ j`) pick the first pair with the strictly smallest `i.size`; move all of `i`'s members
  into `j` (`JoinGroups`: one by one, last member first, each into `j`'s first member slot with `hp < 1` or
  appended; `+0x138` of `i` drops to −1 once `size ≤ +0x138`), then `RemoveTankGroup(i)` (no `RemoveTarget`).
  None → false.
- `EqualizeC(o)`: the designs of `o`'s units (not 0x4020000) in slot order; per design: repeat
  `g = GetGreatestGroupC(o, d)` (first strictly largest, **all** of `o`'s units of design `d`, also
  turrets/mines/partners), `s = GetSmallestGroupC(o, d)` (first strictly smallest, not 0x4020000); stop when
  `g` or `s` is −1 or `g == s`; `n = min((g.size + s.size)/2 − s.size, g.size)`; stop when `n < 1`;
  move `n` members `g → s` (`AtlagoldC`: last member first, as `JoinGroups` but without the `+0x138` rule).
- `GreatestGroupSizeC(o)`: the largest size (not 0x4020000); `GetRealGreatestGroupC(o)`: the first unit of
  `o` with the strictly largest size, **any** flags.
- `HalveGroupC(g)` splits off **one** vehicle: position by `LocateTankSuitablePosition` (start (0x78, 0x78)
  first unless `o` owns the planet); `n = AddTank(g.design, g.max_hp, o, g.origin[last], merge = false,
  x, y, g.flags)` — a new group whose member has **full hit points** (the split vehicle is healed)
  [original quirk]; `n.flags := g.flags` except bits 0x10/0x20 (kept), `n.flags2 := g.flags2`, transporter
  state copied; orders reset unless 0x20000000; `g` loses its last member.

## 5. The battle (`WF::Simulate`, exe 0x4a3730)
```
timing := 0 (global 0x9B2EB8); simulating := 1
for each alive unit: flags &= ~0x900; unless 0x20000000: orders reset, target := 10000, flags &= ~0xd80
s = GetStatus()
loop:
    if s ≠ −1: GroundWarFinished(planet, GetStatus(), sim = true); simulating := 0; return
    repeat:
        any = false
        for e in 1..=ProgramPlayers2: air strikes (§5.3)
        for each slot i (stop early on an attrition kill): unit step (§5.4)
        timing += 1
        if timing % 10 == 0: cache := 0; break
    until any
    s = GetStatus()
```
`Poll(true)` (event pump) runs once per pass; it doesn't touch the battle.

### 5.1 `GetStatus()`
Story hold `h`: `p` is the campaign planet `StoryAdat+0x40` (exe 0xF27438) and has a finished building of
type 0xe, 0xf or 0x10 (`SearchBuilding`). If `countdown ≠ −1` and SP → −1. With the cache valid,
`s = last`; else: clear `present[1..=18]` and `radar[1..=18]` (remember the old `present[EN]`); `s = −10`;
for each alive unit that is not (flag 2 with 0x40000000), not flags2 & 4 and not a mine:
`s = (s == −10 ? side : s == side ? s : −1)`, `present[side] = 1`, flag 0x800000 → `radar[owner] = 1`.
Then if SP, `last == −1`, `s ≠ −1`, (planet owner == EN or EN had units) and not simulating: `h` → −1
(nothing stored); else `last := s`, valid, `countdown := 0x50`, −1 (the real-time end delay). Otherwise
`last := s`, valid, and return `s` if `s == planet owner` or not `h` or `s == −10`, else −1.
So: −10 = nobody left, a side number = that side holds the field, −1 = still contested.

**Hazard (original):** the countdown only runs down in `WF::Refresh`, which in SP runs for the planet on
the colony screen only (`FRISSIT_Grwar` ELF 0x49a7f0; at −1 it calls `GroundWarFinished(p, s, false)`).
A countdown started for any other planet stays, `GetStatus` answers −1 there for good, and a later
`WF::Simulate` on that planet never ends. `GroundWarSetBattleStatus` (§6.4) can start it for every planet
of the human whose `last == −1` and whose field isn't contested. In normal play `last` is a side or −10
once a field has been evaluated (the tracer hit the hang after aborting a real-time war by hand: §10).

### 5.2 Targets
- `GetEnemyNJ(e)`: the first alive unit not paralysed (0x10000), not a destroyed turret, a turret only if
  its building is working and not switched off (building flags `+0x19` bits 0 / 5), owner `≠ e`, not
  stealthy (0x40000); else 10000.
- `GetEnemy(e)`: `GetEnemyNJ(e)`; else the first alive, not paralysed, not a destroyed turret, owner ≠ e;
  else the first alive, not a destroyed turret, owner ≠ e; else 10000.
- Second enemy (inlined in the exe): the first alive slot after `GetEnemy(e)` whose owner ≠ e (no other
  test), else 10000 (also when `GetEnemy` found none). `GetThirdEnemy(e)`: the same search after the second.
- `SetTarget(u, t)`: ignored when `t == u.index`, a turret's own building (`t == −u.building`), flags &
  0xA0000000, or `t == 10000`. A new target resets the aim (`+0x1b8 = 0`, `+0x1bc = −1`, `+0xf8 = 300.0`,
  `+0x100 = −1`, `+0xfc = 0`); `target := t`, `flags |= 0x100`. A unit target whose owner ≠ side and
  owner == `u.owner` gets its paralysis countdown zeroed (unreachable here: `GetEnemy` never returns `u`'s
  own units). A building target (`t < 0`) sets the real-time aim distance.

### 5.3 Air strikes (per empire `e`, every pass)
`CheckFighterAttack(e)`: `e ≠` planet owner and some alive unit of owner `e` with weapons 0x20, `air > 0`
and **not flag 2** (in SP, the human's landed units still carry flag 2: §3.1 step 3).
`CheckBigShipAttack(e)`: the same with weapons 8. Each, when `timing % period == 0` (W13 / W12), hits
`GetEnemy(e)`, the second and the third enemy (each if ≠ 10000), in that order:
`dmg = (int)((f64)rndlog(2, W.min, W.max) · ((f64)(target.flags & 1)·0.2 + 1.0))` (the **target's** flag;
f64, truncated), `Damage(t, dmg, 0)`; killed → `RemoveTankGroup(t)`, `RemoveTarget(t)`, `any = true`.

### 5.4 Unit step (slot `i`)
1. **Attrition**: alive and not a destroyed turret → `Damage(i, 10, 10000)`; killed → `RemoveTankGroup(i)`,
   `RemoveTarget(i)`, `any = true` and **leave the slot loop**; else `any = true`. Every unit loses 10
   hit points per pass.
2. Acts when alive, not a destroyed turret, and (not a turret or its building is working and not switched
   off):
   - paralysed: countdown −1, at 0 the flag clears; nothing else.
   - else: seized (0x1000000): countdown −1; at 0 the flag clears, `RemoveTarget(i)`, `owner := side`.
     Retarget when no 0x100, `target == 10000` or the target is paralysed: `SetTarget(i, GetEnemy(owner))`.
     Not seized: regeneration, per member: `hp < max/2` (C division) → `hp += regen`, capped at `max/2`.
     Then, with flag 0x100, `target ≠ 10000` and not a partner (0x8000), the weapons fire in this order
     at the target `t` (read once; a dead `t` makes `Damage` return 0 after the draw); `m = (f64)(flags & 1)·0.2
     + 1.0` of the **attacker**; `r(W) = rndlog(2, W.min, W.max)`; each gun only when `timing % W.period == 0`:

| Weapon | Damage (f64, truncated) |
|---|---|
| 0x100, 0x200, 2, 0x800, 0x400 (in that order) | `r·m·size·pods` |
| 0x10 | `r·m` |
| 4 (paralyser, W3) | `Damage(t, size·1000, 10000)`; survived → `rndlog(2, 0, 100) ≤ (int)((f64)size·40.0/(f64)t.size)` → `t` paralysed for 2 passes; killed → removed (no `any`) |
| 0x1000 (manipulator, W9) | as 4, success → `t` seized for 2 passes: flag 0x1000000, `RemoveTarget(t)`, `t.owner := u.owner` |
| 1 (W0) | only if `(t.flags & (rndlog(2, 0, 1) << 18)) == 0` (a stealthy target dodges half): `r·m·size` |
| 0x2000 (W1) | `r·m·size` |
| 0x40 (kamikaze, W10; no period) | `rndlog(2, 0, 10000) < 50`: `Damage(t, r(W10), 0)`; second enemy of `u.owner`: `Damage(s, rndlog(2, W10.min/2, W10.max/2), 0)`; a kill by either removes **`t`** [original quirk]; then `u` removes itself, `any = true` |
| 0x80 (big kamikaze, W11) | only if `u` is still alive; as 0x40 plus a third enemy with `W11/3` (drawn before the third enemy is searched) |

   All but the last two pass the attacker index **0** to `Damage` (the exe leaves a 0 on the stack): the
   story damage bonus (§5.5) applies when **unit 0** is alive and the human's. A kill by a gun →
   `RemoveTankGroup(t)`, `RemoveTarget(t)`, `any = true`.

### 5.5 `WF::Damage(u, dmg, by)` → killed?
Cache := 0. `u == 10000`, or `u ≥ 0` and dead → false. `by ≠ 10000`, `by` alive, owned by the human,
SP → `dmg = (int)((f64)((StoryAdat+0x70 + StoryAdat+0x74 + 100)·dmg)/100.0)` (exe 0xF27468/0xF2746C).
- `u ≥ 0`: one libc `rand()` (`% 15 == 0` → a voice line; §7). Not a turret → `WF_Tank::Damage(dmg, by)`.
  A turret: `EpuletDamage(p, slot, dmg, turret = true)` (building damage `+4 = max(0, +4 + dmg)`); still
  below the building's hit points → `WF_Tank::Damage(dmg, by)`; else the building falls: `EpuletDelete(p,
  slot)`, `RemoveTarget(slot)` — the **slot number as a unit index** [original bug: units aiming at unit
  #slot lose their target] — and true.
- `u < 0` (a building, real-time only): `EpuletDamage(p, −u, dmg, false, by)`, which also calls
  `WF::DamageBuilding(p, slot, dmg, by)` (exe 0x46d270) for a defence building (kinds 0x1c..=0x23): its placed
  turrets (alive, flag 2 clear, slot order) share the damage, `dmg / (turrets left)` each, through
  `WF_Tank::Damage` (a destroyed turret: `RemoveTankGroup` + `RemoveTarget`; the last one deletes the
  building, §11.10); with no placed turret the building is deleted at once (`EpuletDelete`, `RemoveTarget(−slot)`)
  and nothing else happens; otherwise the building's damage `+4 := hp[kind] − Σ` the turrets' first-member hit
  points, with the kind **read again** (a building deleted meanwhile gives `hp[0] − 0`, 10 000 in the
  shipped table). Negative damage repairs: every placed turret below its maximum `+0x158` gets
  `heal / (left)` (capped, hit marker `(10000, by)`); with none, the last unplaced turret of the slot is
  placed again with `min(heal, max)`. Back in `WF::Damage` the kind is read again too: damage at or above
  `hp[kind]` → a burst of 24 on screen, `EpuletDelete`, `RemoveTarget(u)`, true — also for a slot that
  `DamageBuilding` already emptied (a second `EpuletDelete`: the building count drops twice [original bug];
  its `Remove3dBuilding` finds no 3D model and leaves no ruin).
On the colony screen of `p` the exe adds particles, debris and voices (more libc `rand()`, §7).

### 5.6 `WF_Tank::Damage(dmg, by)` → all dead?
```
loop:
    rndlog(2, 0, size − 1)                 (drawn every round; the pick is overridden below)
    if size < 1 or no member has hp > 0: return true
    k = first member with hp > 0
    hp[k] −= dmg; carry = max(0, −hp[k])
    dmg > 0 → last_hit := (10000, by)
    hp[k] < 1:
        partner ≠ −1: Damage(partner, 1, 10000) killed → RemoveTankGroup(partner), RemoveTarget(partner)
        MegHalt(k)
        not (flags & 0x4008000) and origin[k] < 0 → TankDelete(origin[k], design, 1)  (a garrison vehicle)
    dead = size < 1 or no member has hp > 0
    dead: a partner (0x8000) clears its carrier's partner link; return true
    carry == 0: return false
    dmg = carry                            (the rest hits the next member)
```
`MegHalt(k)` (a member dies): with `size == 1` nothing (the member stays at `hp ≤ 0`); else member `k` is
removed (arrays shift down), `size −= 1`, `ammo −= 3` (≥ 0), `air −= 1` if > 0.
**Original quirk:** `origin[k]` is read *after* the shift: the next member's origin, or (when `k` was the
last) the stale slot past the shrunk array, i.e. the dead member's own.

### 5.7 Removal
`RemoveTankGroup(u)` (`u ≥ 0`, ≠ 10000, alive; cache := 0):
- A turret (unless `RemoveBuilding` is set, i.e. inside `RemoveBuildingTurrets`): `flags |= 2`,
  `hp[0] := 0`; if no other alive turret of the same slot is still standing (flag 2 clear) and the slot
  still holds a building: `EpuletDelete(p, slot)`, `RemoveTarget(−slot)`.
- Otherwise: `alive := 0`, the object is freed. A mine (0x20000) also explodes: one libc `rand()` (sound),
  units on its cells forget it, and units within 100 map units take `(int)((100 − dist)·500)` (unreachable
  in the auto-resolve: mines exist only during a real-time battle and `GroundWarFinished` clears them).
`RemoveTarget(t)` (≠ 10000): every alive unit aiming at `t` (flag 0x100) drops flags 0x900; units with
`+0x27c == t` reset it to 10000; projectiles aiming at `t` too.
`EpuletDelete(p, slot)`: type `'+'` (0x2b) → `WF.+0x334 := −1`; `RemoveBuildingTurrets(slot)` (every alive
turret of the slot: `RemoveTankGroup` + `RemoveTarget`, fully removed); slot type := 0; the planet's
building count (exe 0xBA9518) −1; `RemoveTarget(−slot)`.

## 6. The end
### 6.1 `GroundWarFinished(p, s, sim)` (exe 0x47c5d0)
1. `InitMineSystem`: mines removed (§5.7); every unit: weapons & 0x10 → `ammo := size·3`; `air :=
   weapons & 0x28 ? size : 0`.
2. The attacking fleet (`WF.+0x338 > 0`) loses its in-battle bit (`+0x52 &= ~1`); planet `+0x64 := 0`.
3. `s == −10` → `s :=` planet owner.
4. Not `sim`: story save, tutorial, screen restore (UI).
5. **Re-embarking**: each alive unit with side ≠ planet owner, not a partner or mine (0x28000):
   fleet `g = origin[0]` (< 0 → 0; owner ≠ side → 0); if `g` is full (`+0x5c ≥ +0x78`) or 0, search fleets
   `1..=n` of that owner with room within Manhattan distance ≤ 20 of **fleet `g`'s position**
   (`g = 0` at the start of the search: fleet slot 0's position; then the last match's) — the last match
   wins. Found → `TankNew(g, design, size)` (all members, whatever room is left). Then its partner (if any)
   and the unit are removed (`RemoveTankGroup` + `RemoveTarget`): **the side that didn't own the planet
   always leaves the field**, the survivors that find no fleet are lost.
6. Partners (0x8000) are removed.
7. Not `sim` and `s ≠` owner: messages, `BolygoElfoglal(p, s)` (`colony-side.md` §4), `BattlesWon[s] += 1`.
8. The planet's building count is 0 → `BolygoElfoglal(p, 0)` (abandoned).
9. Not `sim`: `Story_GroundBattleFinishedCheck`. Landing zone and real-time lists freed; `+0x89f..` cleared.
10. `RepairAllTanks`: every member with `hp > 0` of a non-turret unit is healed to `max_hp`.
11. `RearrangeTankGroups(Bolygok[p − 1].owner)` — the owner of the **previous planet** [original bug; the
    ELF has it too].
12. MP in-progress flag cleared.

### 6.2 Outcome of a simulated war
`GroundWarFinished(…, sim = true)` captures nothing: in the auto path the capture is decided afterwards by
the **attacking fleet's** ground-vehicle count (§6.3). The landing emptied it (§3.1), so `f.+0x5c > 0` means
that surviving attackers re-embarked into `f` itself (step 5): some attackers outlived the defenders and `f`
had room. Survivors that went to another nearby
fleet don't count, and neither does a field won by the attackers when `f` was full.

### 6.3 `GroundWarIndit` after `Simulate`
`Story_GroundBattleFinishedSave(f, p, f.+0x5c > 0)`; messages for the human (lopsided: 0x16/0x17 won,
0x18/0x19 lost; else 0x0b/0x0c when `f.+0x5c > 0`); `f.+0x5c > 0` → `BolygoElfoglal(p, f.owner)`,
`BattlesWon[f.owner] += 1`.

### 6.4 `GroundWarSetBattleStatus` (after every `GroundWarIndit`, and every tick in `FRISSIT_Starmap`)
For every owned planet `q`: `+0x64 := (GetStatus(WF[q]) == −1)` (a status call: may refresh the cache and
start a countdown, §5.1); while contested, every counted unit (not flag 2/0x40000000/flags2&4) of an owner
≠ the planet's, not 0x1028000, with `origin[0] > 0` marks its fleet in battle (`+0x52 |= 1`). Unowned
planets get 0. `FRISSIT_Starmap` calls it on every tick too (exe 0x595d61), after the radar refresh (which clears
the fleets' battle bit) and the real-time battles' marks, before the movement loop: its marks stop the attackers
that tick (`Game::starmap_ground_marks`, handed to the starmap with the space battles').

### 6.6 Units entering and leaving a battlefield outside a war (verified in the save replay)
The planet's battlefield is the exe's live state, saved with the game (`re/formats/save.md` §4.13); these keep it in
step with the strategic side (`Game::battlefield_hand_offs`, in event order):
- `TankNew(−p, d, n)` (exe 0x541750; production, trade deliveries, the AI's `TankAtpakol`): after the `Tankok`
  entry, n × `AddOwnTankToPlanet(d, p)` (0x45cbf0: `LocateTankSuitablePosition(p, −1, −1)` (0x486b90; the outputs
  are uninitialised locals when no cell is found), `AddTank(d, TankAlapHP[hull of Tervek[owner][d] +0x44], owner,
  −p, merge, x, y, 0)`, then `+0x150 = 3.0, +0x108 = 2.5, +0x110 = 0.15, +0x114 = 0.3`), then
  `RearrangeTankGroups(owner)` (§4). The search sees only the planet's used building slots (`Epuletek[p][1..=
  slots]`, 0xBA9840; the rest of the block holds garbage whose coordinates would wreck the centroid) and the
  heights come from the shown planet's map (§11.2).
- `RemoveTankFromPlanet(d, p, owner)` (0x497c70 → `WF::RemoveTank(d, owner)` 0x497af0): the **last** alive group of
  design `d` and `owner` that isn't a partner loses its last member (count −1, `+0x125 = 0`); a group of one goes
  (`RemoveTarget`, alive 0, the object freed with the pointer kept). Callers, once per vehicle: `BolygoElfoglal`
  (the garrison), `FlottaDelete` of a planet holder (0x53d040), `TankAtpakol` from a planet (0x541410 via 0x5414c0,
  then `RearrangeTankGroups(owner)` before the `TankNew` of the destination) and `TervDelete` (0x541d50).
  `TankDelete` (0x541510) itself never touches the battlefield.
- `BolygoElfoglal(p, new)` (0x53b720), after recalling the attackers: every garrison entry (`Tankok` fleet −p, in
  order) is `TankDelete`d and its vehicles leave the battlefield one by one with the **old** owner; then every alive
  turret (flag 0x4000000) gets `+0x194 = +0x198 = new`, and `+0x8cc := 0` (exe 0x53b812..0x53b8ff).
- `EpuletNew` → `AddBuildingTurrets(kind, 100, owner, slot, p)` and `EpuletFelepult` → `BuildingBuilt(slot)`
  (§11.3) for buildings the economy places (autobuild) and finishes.
Not handed off yet: planets changing hands in a diplomatic deal (diplomacy's internal `SideEvent`s).

### 6.5 Between battles: repairs (`FRISSIT_Epuletek`, exe 0x4746f0)
Every tick, after `FRISSIT_Diplomacy` and before `FRISSIT_Starmap` (`time-and-rng.md` §1): for every planet `p`
with an owner, if it isn't at war (`+0x64 == 0`), each building in slots `1..=HanyEpuletLefoglalva[p]` with a kind
and `damage > 0` is repaired: `EpuletDamage(p, slot, −min(damage, BolygoMennyiTuzolto[p]) × IdoSpeedSzorzo, 0,
10000)` (exe 0x53c6e0: `+4 := max(0, damage + x)`, then `WF::DamageBuilding`'s repair branch heals the building's
turrets, §5.5). A planet that repaired something, and every planet at war, gets `FRISSIT_Planet(false, p)`
(`colony-economy.md`: the building pass and production, so the repaired buildings' flags and efficiency follow
at once). Port: `Game::buildings_tick` (`game/battles.rs`); verified in the save replay after the staged ground wars
(their damaged buildings repaired tick by tick; skipping it is caught one tick after the first war).

Not ported in the strategic tick: `FRISSIT_Grwar` and `FRISSIT_Colony` after it (`FRISSIT_Grwar` ticks a pending
self-destruct and `WF::Collapse` on every battlefield; `FRISSIT_Colony` frees passability maps after 600 ticks),
nor the battlefields' write-back into the save (`WF::LoadSave`): `Game::ground_fields` keeps the fields a battle
touched (the others come from the loaded save, `SaveState::ground_field`), and the replay reports the `WF` chunks
without comparing them.

## 7. Randomness
- **Stream 2** draws (exe call sites): `WF_Tank::Damage` 0x46bfa0 (`grwar.cpp` 0xacc, every round),
  `Simulate` 0x2ef9/0x2f06/0x2f12 (fighters), 0x2f27/0x2f34/0x2f40 (big ships), 0x2fa1..0x305f (guns),
  0x3018/0x3032 (paralyser/manipulator rolls), 0x3041 (stealth dodge), 0x307c..0x30af (kamikaze).
  Bit-exact (`rng::Streams`).
- **libc `rand()`** in the auto path: one per `WF::Damage` on an alive unit (voice pick, `% 15`), one per
  new unit object (`WF_Tank` constructor: a cosmetic float), one per exploding mine (sound pick). None of
  them changes the outcome; on the colony screen of the battle planet the exe adds many more (particles,
  debris). ADR 0003: the port draws them from an injected `LibcRand` (default: a dedicated seeded stream;
  the trace replay uses the MSVC LCG to match the exe's draw count). The real-time battle's draws: §11.12.

## 8. Original quirks kept
**Crash (original bug, found by fuzzing):** a group split off a partner group (§4 `HalveGroupC`, reachable
when `EqualizeC`/`JoinGroups` mix a transporter's partners with its vehicles: `GetGreatestGroupC` ignores
the partner flag, `GetSmallestGroupC` doesn't) copies flag 0x8000 but never gets a carrier index (the
constructor leaves `+0x1e0` uninitialised); when it dies `WF_Tank::Damage` writes `−1` to
`units[garbage].partner`: an access violation in the exe (twice in 300 fuzzed wars with transporter pods).
The port's constructor sets the carrier to −1 and ignores a link outside the field.

Story bonus keyed to unit 0 (§5.4); `RemoveTarget(slot)` after a building falls (§5.5); `origin[k]` read
after the shift (§5.6); the kamikaze's secondary kills remove the primary target (§5.4); the split
vehicle is healed (§4); `GetRealGreatestGroupC` counts every unit and returns early (§4); rearranging the
previous planet's owner (§6.1); the second enemy skips the paralysis/turret tests; partners never merge.
Real time: heights read on the shown planet's map (§11.2), the slope speed only for units landed on screen
(§11.5), the in-place heap priority (§11.9), the machine gun's double member factor and `RemoveTarget`
marking units moved (§11.10), the self-destruct burst's height (§11.7), the voices' stuck mute (§11.12),
the passability heights read past the colony map's grid (§11.13), the auto-aim kept on a unit freed in
the same shot (§11.10).

## 9. Not ported (real-time ground war)
Ported and verified (§11): `StartGrWar`, placement with the landing zone, heights and orientation, building
turrets, `FRISSIT_Grwar`'s own part (self-destruct, `WF::Collapse`), `WF::Refresh` with its three projectile
lists, idle fire and the end countdown, `WF_Tank::Refresh` (movement, path search, aiming, firing, mines),
the shots and `SetRealDamage`, removal, the voices' libc draws, the passability maps (`GenerateMap`, §11.13).
Air strikes (bombs and the fighter wave, §11.15) and a building's turrets sharing a
direct hit (§5.5) are ported too, and the transporters (the dropships of `WF_Hordozo`, the landing orders and
the pod-0x9f partners, §11.16), the laser (§11.10, its mesh intersection is render-only) and the story mad
tank (§11.8). Left (§11.11): `GenerateMap`'s editor mode, the rest of `Refresh2` (visuals), `UpgradeTanks`,
the MP battle protocol, `LoadSave`.

## 10. Verification
`tools/trace/run_ground.py` + `ground.js` (`traces/2026-10-16-ground.md`): `--mode war` stages a landing
(a fleet of an empire's ground vehicles next to another empire's planet, optionally garrison vehicles and,
with `--fuzz`, random ground pods on the designs, building flags/damage, a human-controlled attacker and the
story damage bonus) and calls `AddTanksToPlanet(f, p, 1)` and `WF::Simulate` from the frame hook, with
snapshots before, between and after and libc `rand()` pinned by `srand`; `--mode indit` calls
`GroundWarIndit` with `StartGrWar` patched out. `engine/crates/ig2-sim/tests/ground_trace.rs` replays both:
324 wars (4,597 passes, 136,217 libc draws, 3,759 groups destroyed, 73 buildings fell, 214 paralyses,
56 seizures, 2,584 splits) and 150 routings (111 real-time, 39 simulated) exact; the recorded war the exe
crashed on (§8) finishes in the port. Not traced: MP, the story hold, mines (unreachable here), transporter
partners taking hits (rare; the crash above), the 0x10 ammunition weapon, big-ship strikes, the stealth dodge.

## 11. The real-time ground war
Port: `engine/crates/ig2-sim/src/ground/` `realtime.rs` (placement, heights, turrets, the start),
`battle.rs` (§11.7, §11.10), `tick.rs` (§11.8), `path.rs` (§11.9), `voices.rs` (§11.12) and
`placement::{landing_zone, locate_tank}`. Verified in `traces/2026-10-16-ground-realtime.md` and `traces/2026-10-16-laser.md` (the laser, the mad tank).

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Grwar` | 0x4747c0 | 0x49a7f0 | the 20 Hz driver (§11.6) |
| `StartGrWar(p)` | 0x4a4a70 | 0x4a4f80 | battle start (§11.4) |
| `LocateTankSuitablePosition(p, &x, &y, sx, sy)` | 0x486b90 | 0x488d80 | a free map cell (§11.1) |
| `LandingZone::Generate` | 0x478dd0 | 0x46b610 | the landing zone (§11.1) |
| `GetHeight(fx, fy, p − 1)` / line–plane `0x44ff80` | 0x47ad60 | 0x46bff0 | terrain height (§11.2) |
| `WF_Tank::SetTankPositionAndMatrix` | 0x49c4f0 | 0x4773b0 | orientation, height, slope speed (§11.2) |
| terrain basis / smoothing | 0x4a8b60 / 0x48b560 | | §11.2 |
| matrix product, transpose, axis rotation, Euler, basis | 0x44f4c0 / 0x44f480 / 0x44f9f0 / 0x44f170 / 0x44ef70 | | §11.2 |
| `WF::AddBuildingTurrets(kind, 100, side, slot, p)` | 0x45af60 | 0x470190 | turrets of a defence building (§11.3) |
| `WF::AddTurret(pod, hp, side, slot, x, y, z)` | 0x45e670 | 0x470d80 | §11.3 |
| `WF::BuildingBuilt(slot)` | 0x45faf0 | 0x471730 | §11.3 |
| `WF_Tank::LoadMesh` | 0x4847a0 | 0x46efc0 | mesh loading, with state effects (§11.5) |
| `Real2Grid` (inlined; also 0x48c920) | | 0x4717e0 | map position → cell (§11.3) |
| `WF::Refresh` | 0x490890 | 0x48d750 | the battlefield tick (§11.7) |
| `SelfDestruct(p)` / `WF::Collapse` | 0x499500 / 0x460d70 | | §11.7 |
| `WF::GetNearestEnemy` (on the unit) | 0x47b860 | 0x473210 | idle fire (§11.7) |
| `WF_Tank::Refresh` | 0x494bd0 | | the unit tick (§11.8) |
| `WF_Tank::MoveG` / `Stop` | 0x48bdc0 / 0x4a4cb0 | | orders (§11.8) |
| `grUtkeres` (relax / pop / sight) | 0x614940 (0x6142a0 / 0x614540 / 0x614830) | | path search (§11.9) |
| `ShotOne` / `ShotWithSpecificWeapon` / `CalcDamage` | 0x49c810 / 0x49d050 / 0x45fb40 | | §11.10 |
| projectile makers (shell, rocket, bolt, manipulator, paralyser) | 0x45d5d0 / 0x45d910 / 0x45bba0 / 0x45c300 / 0x45cca0 | | §11.10 |
| `SetRealDamage` / `DamageBuildings` | 0x49c3c0 / 0x46d5f0 | | §11.10 |
| `WF::RemoveTankGroup` / `WF::RemoveTarget` / `EpuletDelete` | 0x497c90 / 0x499070 / 0x53c730 | | §11.10 |
| `WF::AddMine` | 0x45c770 | | §11.10 |
| `AddRepesz` / `Remove3dBuilding` / `Rom::Rom` | 0x45d400 / 0x497760 / 0x455570 | | debris (§11.12) |
| `WF::ShotSpecWeapon` / big-ship bomb / `WF::AddFighters` / `AddFighterLaser` | 0x49cb70 / 0x457ad0 / 0x45b8d0 / 0x45b600 | 0x4ab170 / – / 0x49fd90 / 0x4964c0 | air strikes (§11.15) |
| laser beam end / `villamgenesis` ctor / line test | 0x47af70 / 0x616190 (jitter 0x616620) / `vonalmetszes` 0x53a2b0 | | the laser (§11.10) |
| `StoryMadTankFrissit` / `WF_Tank::SetTarget` | 0x5c5370 / 0x49c670 | | story mad tank (§11.8) |
| `WF::DamageBuilding` / `EpuletDamage` | 0x46d270 / 0x53c6e0 | 0x473410 / 0x61cc60 | a building hit directly (§5.5) |
| `WF::PlaySpeechGrWar` / `SpeakingDirector::Play` / `Speeches::Play` | 0x48c670 / 0x563cd0 / 0x563d50 | 0x45f340 / 0x67fe30 / 0x680200 | voices (§11.12) |
| `WF::Refresh2` / `WF_Hordozo::Refresh2` | 0x48ce10 / 0x48e0a0 | 0x489170 / 0x48bb70 | the dropships (§11.16) |
| `PlaceAllTanks` / `PlaceTank` / `UnplaceTank` / `GetNearestLandingZone` (test 0x4609a0) | 0x48c150 / 0x48c2c0 / 0x4a4f70 / 0x47b980 | 0x4a5a20 / 0x4a57c0 / 0x4a5d00 / 0x488460 | landing orders (§11.16) |

### 11.1 Placement
`LocateTankSuitablePosition(p, &x, &y, sx, sy)` builds the building search's grid (`colony-placement.md`
§2: terrain mask A of map `p.+0x23`, every building's footprint with its frame, the centroid start) with two
changes: every alive unit without flags 2 / 0x40000000 and without flags2 bit 2 blocks a **3 × 3** block (no
frame) around its cell (`+0x1a0/+0x1a4`), and a given start (`sx ≠ −1`) both replaces the start and closes
the **landing zone** (`Decode2LargeMap(…, landing = true, p)`: every cell whose zone level is > 0 becomes 1).
Search: `LocatePos(DecodedCheckAllocation, start, 1, 1)` (1 × 1: no frame test); it returns the search's
bool. On failure the outputs keep the caller's values (uninitialised locals in `AddTanksToPlanet`;
`HalveGroupC` reuses its argument; never reached in the traces). Callers: `AddTanksToPlanet` (real time:
start (0x78, 0x78), else the default; the transporter partner: the default), `HalveGroupC` (start (0x78,
0x78) first unless the group's owner owns the planet), `Retreat`, `InitGrWar`, `PlaceTank`/`PlaceAllTanks`
(the human's placing).

Landing zone `LandingZones[p]` (exe object 0xA34F50 + p·0xc0: grid `+0xb4`, made `+0xb8`, planet `+0xbc`;
ELF 0x11d0340 + (p − 1)·0x138): generated at the first landing search, freed by `GroundWarFinished`. For
every building (slot order, kind ≠ 0) at `(bx, by) = ((int)x, (int)y)` with footprint `(w, h)`: for level
1..=12, `W = w + 22 − 2(level − 1)` (`H` likewise), every cell `x ∈ [bx − W/2, bx + W/2 + W%2)`,
`y ∈ [by − H/2 − H%2, by + H/2)` gets the highest level, written at index `(y − 1)·200 + x` (checked only
against the buffer). So attackers land at least 11 cells from any building.

### 11.2 Heights and orientation
`GetHeight(fx, fy, p − 1)` (`fx, fy` in height cells, a quarter of a grid cell): `ix = (int)fx`, `iy = (int)fy`;
samples `h(x, y) = (f32)((f64)Terepek[m].heights[y·50 + x] · 0.0625) − 700` (`+0x138c`, i16, the map file's
values); triangle corner `A = (ix, iy)` if `(f32)(ix + 1) − fx > fy − (f32)iy`, else `(ix + 1, iy + 1)`;
`B = (ix + 1, iy)`, `C = (ix, iy + 1)`; the vertical line through `(fx, fy)` meets the plane `ABC` (`0x44ff80`:
coordinate differences in f32, products and the division in **f64**, asm-checked); no intersection → 100000.
Result `(f32)(Terepek[m].+0x84 + z)`; the base `+0x84` is stored by the loader as `−(file value − 200)`
(`InitHatterekEsTerepek` ELF 0x4be142; checked on 9 maps).

`SetTankPositionAndMatrix(u)` (from `SetPosG` unless the unit is a turret or mine, and from `LoadMesh`):
1. Terrain basis (`0x4a8b60`) at `(x, y) = (+0x1d4, +0x1d8)` with body angle `a = +0x1c0`: height cells
   `cx(x) = 49 − ((W/2 + x)/W·50·49)/50`, `cy(y) = ((H/2 + y)/H·50·49)/50`; `h0` at `(x, y)`, `h1` at
   `(x + s/100, y + c/100)`, `h2` at `(x + c/100, y − s/100)` with `s = (f32)sin(−a)`, `c = (f32)cos(−a)`;
   `u = normalise(h0 − h1, x − x1, y − y1)`, `v = normalise(h0 − h2, x − x2, y − y2)` (`0x4501f0`: f32, kept
   below 1e-5); basis `0x44ef70(v, u)` (two cross products, normalised, negated); body =
   `Euler(0, 0, −π/2) ⊗ basis`. **The heights are read on the map of the planet on the colony screen
   (`AktPlanet − 1`), not the unit's** [original quirk; the same map whenever the battle is shown].
2. Smoothing (`0x48b560`; `+0x1c` = the smoothed basis, `+0x284` = snap): `R` = rotation about x by `a`
   (`0x44f9f0`), `t = body ⊗ Rᵀ`; snap → `+0x1c := t`; else per row, a difference `d = t_k − old_k` with
   `|d| < 0.3` (f64 compare) takes `t_k`, otherwise `old_k := normalise((f32)((f64)normalise(d)·0.3 + old_k))`
   (f64 blend, asm-checked). `+0x40 := smoothed ⊗ R`.
3. `+0x284 := 0`; `+0x88 := h0`; non-turrets: **driving speed `+0x150 := (b0·0 + b1 + b2·0)·2 + 3`** from the
   smoothed basis' first row (the slope), turret matrix `+0x64 := +0x40 ⊗ rot(column 2 of +0x40,
   −(+0x1c4 − +0x1c0))`, position `+0x1d4/+0x1d8 := x, y`, height `+0x1dc := h0`.

The axis rotation is the quaternion form; its products are `(z·2)·(z·s)` etc. as in the asm (the decompile
groups them wrongly). `sin`/`cos`/`sqrt` are f64 library calls rounded to f32.

`SetPosG(x, y)` (§3.4) also sets the render position `+0xc/+0x10` and `+0x285 := 1`, then calls the above.
`AddTank` (§3.2, a new group) sets: `+0x1c0 ≠ 0` → flag 0x200 and `+0x1c0 := 0`; `+0x1c4 := 0`;
`+0x150 := 3.0`, `+0x108 := 2.5`, `+0x110 := 0.15`, `+0x114 := 0.3`; fire range `+0x148` raised to
`(f32)(int)((f32)(flag 0x100000 · 2) + 1.0)`; turning `+0xe0/+0xe4 := 0.65, 1.0`; `+0x1c8, +0x228, +0x248 := 0`;
`+0x280 := planet`; then `SetPosG`, `CheckDesign` (fire range reset to 1.0 and raised to
`(f32)(int)((f32)(flag 0x200000 · 2) + range_k)` per weapon, reload `+0x1f0 + 4k := (f32)(reload_k / pods_k)`
(int division) from `WF_NonSimulateWeapons[k]`, k as in §1.2); for ground hulls 0..4: `+0x150 :=
(f32)(int)TankAlapSpeed[hull]` (0x8EC9D4), the three constants again, `+0xe0 := t[hull]`, `+0xe4 :=
(f32)((f64)t·1.5)` (turning table exe 0x8F8560, `ig2-rules::ground_turn_speeds`). Every added member clears
`+0x125`. The final pass of `RearrangeTankGroups` **resets the driving speed** of every group with flags2
bit 1 (`+0x150 := 3.0` and the three constants). The constructor (0x457020) draws one libc `rand()` for a
cosmetic phase (`+0x26c = r % 500 − 1650`, `+0x264/+0x268` a third of it), sets the 14 reload times to 30.0
(`+0x1f0..`) and their counters `+0xa8..` to 0, three identity matrices (`+0x1c`, `+0x40`, `+0x64`),
`+0x284 := 1`, `+0x278 := 1`, `+0x108 := 3.0`, `+0x110 := 0.25`, `+0x114 := 0.5`; the last-hit pair
`+0x25c/+0x260`, the render position and `+0x88..+0x94` stay uninitialised.

### 11.3 Defence-building turrets
`EpuletNew` (exe 0x53c7d0) calls `AddBuildingTurrets(kind, 100, owner, slot, p)` for every new building and,
for a finished one, `BuildingBuilt(slot)`; `EpuletFelepult` (construction done) calls `BuildingBuilt` too.
Only kinds 0x1c..=0x23 have turrets, of pod `kind + 0x85` (0xa1..=0xa8): position `(px, py)` = the cell →
map formula of §3.4 **without** the +0.5 (f32 products, f64 centring), height
`z = GetHeight((f32)((f64)x·0.25), (f32)((f64)y·0.25), p − 1)` of the building's own planet (0 beyond 200),
hit points `hp = EpuletekLeiro[kind].+8`:

| Kind | Turrets: hit points at offset (map units) | z offset |
|---|---|---|
| 0x1c..0x1f | `hp` at (−20, −20) | −20 |
| 0x20 | `hp` at (0, 0) | −133 |
| 0x21 | `hp/4` (+50, +50), `rest/3` (−50, +50), `rest/2` (−50, −50), the rest (+50, −50) | −65 |
| 0x22 | `hp/4` (+45, +45), `rest/3` (−85, +45), `rest/2` (−85, −85), the rest (+45, −85) (`±70 − 25 (+10)`) | −60 |
| 0x23 | `hp/6` (+75, +75), `rest/5` (−75, +75), `rest/4` (−75, −75), `rest/3` (+75, −75), `rest/2` (+110, 0), the rest (−110, 0) | −68 |

(`rest` is what the earlier turrets left; C integer division; f32 offsets.) `AddTurret(pod, hp, side, slot,
x, y, z)`: the first dead slot (never merged; appended when none), a new object (one libc `rand()`), one
member with `hp` and origin −1, `max_hp := hp`, owner = side = `side`, the movement constants with
`+0x150 := 0`, fire range `(f32)(int)(… + 1.0)`, turning 0.65/1.0, `+0x1b8 := 0`, `+0x1bc := −1`, last hit
`(0, 10000)`, `+0x1c8, +0x228 := 0`, `+0x280 := planet`; pod 0xa5/0xa6/0xa7/0xa8 → weapon 1/0x100/2/0x400 with
the range and reload of `WF_FortressWeapons` entry 0/4/8/6 (pods 0xa1..0xa4 carry none); flag 0x4000000,
building slot, own index, `+0x248 := 0`, map position and height, pods 1, cell = `Real2Grid(x, y)` (also the
last cell), **flags2 bit 2** (not counted by `GetStatus` until `BuildingBuilt` clears it), the gun bit.
`Real2Grid`: `gx = (int)floor((W/2 − x)·99/W/100·99·2)`, `gy = (int)floor((H/2 + y)·99/H/100·99·2)` (f32, the
floor in f64).

### 11.4 `StartGrWar(p)`
`InitMineSystem` (§6.1 step 1); the per-empire call-in timers `+0x7fc + 8e := 0x8c`, `+0x800 + 8e := 0xf0`
(e = 0..17); `+0x190 := 0`; `s = GetStatus()`; if `s` is −1 or −10 (or `+0x190`): `TamadasSzovegCounter :=
900`, the "not watching" flags `+0x88d..+0x89e := 0`, and when the human owns the planet or has units there
(or an MP battle runs) the colony screen of `p` opens (old screen saved, speed 1, paused: a UI event).
Otherwise `GroundWarFinished(p, s, false)` at once and `+0x190 := 1`. Either way the retreat flags
`+0x89f..+0x8b0 := 0`. `GroundWarFinished` also frees the landing zone and clears the retreat flags.

### 11.5 `LoadMesh` side effects
On the colony screen of the battlefield (`AktKep == 3`, `AktPlanet == p`, not entering or leaving it, exe
0xBAE24C/0xBAE24D) `AddTank` (a new group) and `AddTurret` load the unit's meshes unless flag 0x10 is set.
Besides the 3D objects this sets flags 0x10/0x20/0x200 and, for vehicles, runs `SetTankPositionAndMatrix`
once more: **a vehicle landed on screen drives at the slope speed, one landed off screen at its hull's**
[original quirk]. Mines take their cell's map position and `GetHeightFloat` (0x4a89a0) [not traced].

### 11.6 The battle tick (`FRISSIT_Grwar`, every 20 Hz tick)
Called from the main tick (exe 0x4deb36) after the strategic systems, unless paused; while the colony
screen shows a contested battle (`0x47c150(AktPlanet) == 1`) the strategic systems stop and only this and
`GroundWarSetBattleStatus` run. Per planet: `IdoSpeedSzorzoBattle` (0xBAE260) times the self-destruct
countdown `WF+0x334` (§1.1, the `'+'` building's; at 0 → `SelfDestruct` 0x499500(p), then −1); every 300th call `WF::Collapse` (0x460d70). Then in SP,
only on the colony screen (`AktKep == 3`): planet `+0x64` (ground war) → `WF::MI` (0x486d40, stream 13;
else 18 per-empire counters `WF+0x37c + 0x40·e` are zeroed), and `WF::Refresh` (0x490890) unless a story
script holds the planet (`0xF261F8`). MP: both for the battle planet. Order inside one call: `gvalue += 1`, the per-planet part (§11.7), `WF::MI`,
`WF::Refresh`. The strategic game runs it after every tick (`Game::grwar`: the per-planet part for every planet,
verified with `gvalue` dumped in the save replay; the colony screen's `WF::MI`/`WF::Refresh` belong to the
real-time battle).

### 11.7 The battlefield tick (`WF::Refresh` 0x490890, and `FRISSIT_Grwar`'s own part)
`S` = `IdoSpeedSzorzoBattle` (0xBAE260, battle steps per tick), `run` = (`IdoPause` 0xBAE264 == 0).
**`FRISSIT_Grwar` per planet** (before `WF::MI`/`WF::Refresh` of the shown one; `War::grwar_planet`): `S`
times: `+0x334 > −1` → `+0x334 −= 1`; `+0x334 == 0` → `SelfDestruct(p)` (0x499500), `+0x334 := −1`. Then
`gvalue` (0x9B2900, counted up first) `% 300 == 0` → `WF::Collapse` (0x460d70): list A, list B, the units
(by the alive bytes) and the cosmetic list `+0/+4` shrink to their last used slot (`realloc`); **list C
keeps its length**. `SelfDestruct(p)`: for every building slot `1..=count` (kind ≠ 0): on screen a burst
(§11.12, 16 slow pieces at `(x_cell, map x, map y)`: the height argument is the building's cell x [original
bug]), `EpuletDelete(p, slot)`; for every slot `RemoveTarget(−slot)`; then every alive group takes
`WF_Tank::Damage(10 000 000, 10000)` (no voice pick) and, dead, a scorch mark (cosmetic, vehicles only),
`RemoveTankGroup`, `RemoveTarget`; the colony screen drops its selected building (0xB9C074).

**`WF::Refresh`** (`War::refresh`):
1. Per empire e = 0..17: `+0x7fc + 8e := min(+ S, 200)`, `+0x800 + 8e := min(+ S, 300)` (call-in timers).
2. Planet `+0x64` (in war): every alive unit: its side's retreat flag `+0x89f + side − 1` set → `Retreat`
   (0x499260) else `StopRetreat` (0x4a4d00).
3. `+0x88d + k := (planet ≠ KiMelyikBolygonVan[k])` (0x9ABAE8, the planet empire k + 1 watches).
4. `s0 = GetStatus()`; `s0 == −1` (contested) → `+0x190 := 0`.
5. The fighters of an air strike: `2S` rounds, each only while the wave is up (`+0x32c`), of the five
   fighters' flight (§11.15).
6. `S` rounds of list A, `4S` rounds of list B, `S` rounds of list C (below).
7. `s0 == −1`: **idle fire**. Every alive unit (not flag 2 / 0x40000000, not flags2 bit 2) without a target
   (flag 0x100 off or target 10000) that is either moving (0x400 with `+0x1b8 ≠ 0`) or has no move order:
   with an auto-target `+0x27c`: cell distance (f32, the root in f64) `≥` fire range `+0x148`, or the
   same owner → `+0x27c := 10000`, else `ShotOne(+0x27c)`; without one: `t = GetNearestEnemy` (0x47b860:
   the nearest by squared map distance among alive units without flag 2 / 0x40000000 / flags2 bit 2, of
   another owner, that **have fired** (`+0x28c`), skipping all pairs where neither moved this tick
   (`+0x285`) unless the shooter is a turret) and, within range, `ShotOne(t)`, `+0x27c := t`.
8. Every alive unit without flag 2: `WF_Tank::Refresh` (§11.8).
9. `GetStatus()`; SP: the end countdown `+0x148 ≠ −1` counts down; reaching −1 → `GroundWarFinished(p,
   GetStatus(), false)` (§6.1), `+0x190 := 1`. The countdown (0x50 ticks) is started by `GetStatus` only
   when the human owns the planet or had units there (§5.1); an AI-against-AI battle on screen never
   ends by itself in this tick.

**List A** (`+0xd4`/count `+0xe0`, 0x38-byte homing shells from 0x45d5d0: `+8` velocity and `+0x14`
position as (height, x, y), `+0x20` ticks, `+0x24` target (10000 none, < 0 building), `+0x28` pull, `+0x2c`
damage, `+0x30` shooter, `+0x34` stealth byte). Per round and slot: ticks −1, 0 → freed; else 3 libc draws
(particles); position += velocity; the target point (a unit: its map position and height; a building:
`building_z` and the map position of its int cell without the +0.5); `d` = normalised (target − position)
× pull; `dist` = |target − position|; a target with flag 0x40000 (stealth) within 500: `j = byte ·
(f32)((f64)rndlog(2, 0, 0x45)/100 + 0.3)·(f64)(−(500 − dist)/500)) · 5` (the product in f64, asm
0x49111e) added to the height velocity; velocity += `d`; speed above 20 → scaled to 20; `dist < 60` → hit:
2 libc draws, `SetRealDamage`, `Damage(target, dmg, shooter)` → killed: `RemoveTankGroup` + `RemoveTarget`;
freed.

**List B** (`+0xd8`/count `+0xe8`, 0x44-byte objects: `+0x20` aim point, `+0x2c` ticks, `+0x30` damage,
`+0x34` target, `+0x38` kind, `+0x3c` fade, `+0x40` shooter). Per round: ticks −1, 0 → freed; by kind:
- 0, 4, 5, 6 (shells, rockets): 3 draws, position += velocity; target ≠ 10000 and |aim − position| < 40 →
  2 draws, `SetRealDamage`, `Damage` → kill; freed.
- 3 (manipulator bolt, damage = `100·members + 2·owner + 1`): as above; at the hit an odd value on a unit
  that isn't a turret: `m` = its alive members, chance `(int)((f64)(dmg/100)·40.0/(f64)m)`;
  `Damage(t, 2000, shooter)` killed → `RemoveTankGroup`; else `rndlog(2, 0, 100) ≤ chance` → **seized**
  (flag 0x1000000, owner := `(dmg % 100 − 1)/2`, `+0x1ec := 200`, flags &= ~0x900, orders reset unless
  0x20000000); `RemoveTarget(t)` unless the roll failed; freed.
- 1 (bomb; `+0x34` holds the ground height as an int), 2 (big bomb): 3 draws, position += velocity;
  `|h − target| < 100` (kind 2: < 40) → 5 (kind 2: 5) draws per explosion particle group (3 groups for
  kind 1); every alive unit (not flag 2, not a turret) within √3600 (kind 2 √2500) map units: 2 draws,
  `Damage(i, dmg·20 | (int)((f64)dmg·1.5), shooter)` → kill; `DamageBuildings(p, h, x, y, dmg | dmg/1500)`
  (0x46d5f0: every building within √5000 takes ·900, a fallen one is deleted); |aim − position| < 40 →
  freed. Bombs come only from air strikes (§11.15). A bomb's blast is checked every round while its height
  is within the band, so one bomb can hit several rounds in a row.
- 7 (debris, §11.12) and others: only the ticks. Debris never moves here (`Refresh2` moves it per frame).

**List C** (`+0xdc`/count `+0xe4`, paralyser bolts from 0x45cca0: `+0x2c` ticks, `+0x30` target, `+0x34`
power = `100·members + owner`): ticks; 3 draws; position += velocity; target ≠ 10000 and |aim − position|
< 40: 1 draw; power ≠ 0 and a unit target that isn't a turret: chance `(int)((f64)power·40.0/100.0/(f64)m)`;
`Damage(t, 2000, 10000)` killed → removed; else `rndlog(2, 0, 100) ≤ chance` → **paralysed** (flag 0x10000,
`+0x1e8 := 200`); freed.

New objects take the first empty slot of their list (else the list grows by one): the slot order is the
processing order, so the cosmetic debris in list B decides where later shells land and in which order their
stream-2 damage draws happen.

### 11.8 The unit tick (`WF_Tank::Refresh` 0x494bd0)
`steps = (int)(run·S)`. In order:
1. `+0x285 := 0` (moved), `+0x125 := 0` (the alive cache); path timer `+0x118 := max(0, − S)`; paralysed:
   `+0x1e8 −= S`, < 1 → flag off; still paralysed → return.
2. The 14 reload counters below their reload time: `+= steps`. Mine laying (§11.10). Counters above it are
   clamped. Regeneration `+0x1cc` (not seized): members below `max/2` gain `regen·steps`, capped at `max/2`.
3. **Coasting** (`steps` times, neither 0x400 nor 0x800, speed `+0x10c ≠ 0`): speed := `max(0,
   (f32)((f64)v − (f64)brake·1.5))`, position += direction (`+0x98/+0x9c`)·speed, step point := position,
   `SetTankPositionAndMatrix`, a dust draw (`rand() % 5`), flag 0x40, moved.
4. flags exactly 0x4000001 or 1 → return. No 0x1000 → blocked `+0x104 := 0`. Seized: `+0x1ec −= S`, < 1 →
   flag off, orders reset unless 0x20000000, owner := side, moved.
5. No 0x400: the waypoint list (`+0x22c` count, `+0x230` cursor, `+0x234` cells) exhausted → freed; else
   the next one → `MoveG(x, y)` (0x48bdc0: not for turrets/mines, cells outside 0..200, flags 0x28000000 or a
   transporter in flight; clamped to 10..=190; a new destination resets the path counters `+0x1b8/+0x1bc`
   to (0, −1); target dropped; flags 0x900 off, 0x400 on) unless flag 0x80000000.
6. Flag 4 (a story "mad tank", set by campaign code): `StoryMadTankFrissit(u)` (0x5c5370, stream 5), only
   for the human's units (`owner == EN`). Unless flags & 0x500 (a target or a move): `rndlog(5, 0, 99)`
   (line 0x1345) `< 3` → `t := GetNearestEnemy` (0x47b860, as idle fire), `SetTarget(t)` unless 10000, return;
   else `rndlog(5, 0, 99)` (line 0x134a) `== 0` → the building pick: over the slots `1..=count`
   (0xBA9840[p]) of built (`kind ≠ 0`) and finished (`+8` work left 0) buildings, `d = (int)(f32)sqrt((f64)(f32)
   (dx² + dy²))` with `dx = cell.x − (int)b.x` (int); the slot is taken when none is yet, or when `d < 7` and
   a `RndValasztas` draw with weight 1 wins (0x4e3630 on `{count, sum, stream 5}`: `sum += 1`,
   `rnd(5, 1, sum) ≤ 1`, the plain stream draw 0x542ea0), or when `d <` the best so far; a pick →
   `SetTarget(−slot)`; return either way. Otherwise (and with flags & 0x500): `rndlog(5, 0, 99)` (line 0x135c)
   `< 5` → flags 0x900 off (0x48c0b0).
   `SetTarget(t)` (0x49c670, `ground-ai.md` §6.9) sets, for a building, `+0x14c := (f32)(width/2) + range`.
7. A target (0x100): **aim** `+0x18 = asin(dy/len)` (f64, π − that when dx < 0) at the target's map point;
   gun elevation `+0x1c8` from the ballistic formula (f32 with f64 roots and `tan`); mines aim at once;
   the elevation is render-only (no battle code reads `+0x1c8`); the turret turns at `+0xe0/5` per step (snaps within `+0xe4/5·S`, f64; wraps by 2π beyond π; flags
   0x2000/0x4000); aligned within 0.1: in range (cell distance ≤ `(int)` range, `+0x14c` for buildings)
   every ready weapon fires (`ShotWithSpecificWeapon`, counter := `rndlog(2, 0, 4) + (counter − reload)`);
   out of range and none of 0x2c020000 → **attack-move**: waypoints freed, destination := the target cell
   (a new one resets the path counters), 0x400 off, 0x800 on.
8. Body turn (0x1000): the shortest signed turn (x87 `fmod` of both angles by the f32 2π as a double,
   the alternative `2π − |d|` in f64); within 0.01 done; else by `+0xe0/5·S` per tick (f64), the turret
   counter-rotating to keep its world direction unless it is turning itself, re-oriented, flag 0x200;
   a partial turn returns.
9. Straight driving (0x80 without 0x1000, `MoveTank`): within `speed·S` of the step point → there,
   re-oriented, 0x80 off, return; else position += direction·speed·run·S, the cell from the position, a
   dust draw.
10. Move (0x400) / attack-move (0x800, not while turning): the **path step** every 15 ticks or once at the
    step point: cell from the position, `grUtkeres(cell, destination, radius 1 | the fire range, …)`
    (§11.9); arrived → the order flag off; a new cell → step point := its map centre (cell + 0.5, f64
    centring), `+0x1a8` := the cell, direction and wanted angle `+0x12c` (path moves take
    `(cos, sin)` of the wanted angle), 0x1000 when the body must turn. Then the **drive**: at the step point
    coast to a stop; else, per step, accelerate (`+0x110`; the attack-move adds it twice) while the
    remaining turn is below 1.4959965 rad and not blocked, else brake (`+0x114`); speed capped at `+0x108`;
    moving → position += (cos, sin)(body angle)·speed·run·S, a dust draw; else blocked := 1. Within
    `S·speed` of the step point → arrive (position := step point, re-oriented, cell, flag 0x40) and return.
11. A mine (0x20000 with weapon 0x4000/0x8000): an enemy (not a mine, counted) within √3500 → it blows
    (`RemoveTankGroup` on itself, §11.10).

### 11.9 Path search (`grUtkeres` 0x614940)
`FUN_0045f600(p)` first: the planet's passability maps (`PlanetAtjarasMap` 0xA33388 + p·0x24, five 200×200
u16 maps by hull class, flag byte `+0x1c`) are generated by `GenerateMap` (0x479300) at the first search
(§11.13). Hull class = `design hull − 0x12` (flag 0x200000 → 3). The cost grid (0xFAC940, index
`x·200 + y`) is a copy of that map (free 65000, impassable 65001); every other counted, non-turret, non-mine
unit stamps 65002 (3 × 3) at its cell and its next cell unless it goes for the same goal and its path
counter is ≤ 5; every building stamps 65003 over its footprint (`x ∈ [bx − w/2, bx + w/2 + w%2)`, `y ∈
[by − h/2 − h%2, by + h/2)`). Throttle: `+0x1b8 += 1` (10 → 470); only while ≤ 10, every 20th, or 500/501.
Within the radius → arrived. Goal cells: within the radius `r` (from `+0x1bc` when ≥ 0) every free cell
becomes 0; none → `r + 1` ("widened"); the unit's own cell (1) inside the radius ends the search
(arrived unless crowded). Then a best-first search: a 1000-entry binary heap (priority `g + h`, `h = 7·max
+ 2·min` of the offset to the goal; steps 5 straight, 6 diagonal), bounded by `f ≤ max(3h₀ + 0x46, 0x118)`
and `h ≤ max(2h + 0x23, 0x8c)` (tightening) unless the counter is 500; a cell already queued gets its
priority **overwritten in place without re-sifting** [original quirk]; a full heap overwrites its last
entry. The result is the best `5h + 2g` cell (a goal cell ends the search); the walk back down the cost
gradient gives the path, and the **furthest cell of it in sight** (sampled line, step `1/(n + 0.01)`, no
65001..65003 on it) is the next cell; other units' next steps block it, and a blocked next cell falls back
to the free neighbour closest to it. `+0x1b8 := 0` after a successful step.

### 11.10 Shots, damage, removal and mines
`ShotOne(t)` (0x49c810, idle fire): aim as in §11.8; `+0x18 := a`; aligned within 0.1 (f64) or a mine →
every ready weapon `ShotWithSpecificWeapon(t, k)`, counter := `rndlog(2, 0, 4) + (counter − reload)`;
else turn the turret: beyond π → ±2π (f64), flag 0x2000; within `(f64)+0xe4/5·S` → aligned, flags 0x2000
off, 0x200 on; else `+0x1c4 := (f32)((f64)+0x1c4 − (f64)((d/|d|)·+0xe0)/5·run·S)`, 0x200 on (all f64 in the
exe; the f32 version is 1 ulp off). `ShotWithSpecificWeapon(t, k)` (0x49d050): nothing when paralysed or a
turret of a building that doesn't work / is switched off; the cell distance must be ≤ the weapon's range
(`WF_NonSimulateWeapons` 0x8F83A0, turrets `WF_FortressWeapons` 0x8F8480) + 2 with flag 0x100000 (+ half the
building width for a building target); `+0x28c := 1`, then by k (only the weapon whose bit is set):
- 5 (0x200) / 6 (0x400): `AddRocket` (0x45d910, list B kind 4 / 5) with `CalcDamage(k)·members`; a sound
  draw. Kind 4 flies ×`(f32)(rndlog(2, 10, 20)/100.0 + 1.1)`, kind 6 ×`(f32)(0.87 − …)` (f64), speed 5.
- 0 (1) / 1 (0x2000): `rndlog(2, 0, 9) < 2 / < 5` → no stealth jitter; a list-A shell; a draw.
- 9 (0x1000) manipulator (`0x45c300`, list B kind 3, unless the target is seized and it has nothing
  else: then flags 0x900 off); 8 (2) a straight bolt (`0x45bba0`, list B kind 0, `z − 15`); 3 (4) the
  paralyser (`0x45cca0`, list C) unless the target is already paralysed and it has nothing else.
- 4 (0x100) machine gun, instant: `CalcDamage(4)·members`, 147 libc draws (one, 16 tracer lines of 9, a
  flash and a sound), the damage times the members **again** [original quirk], `SetRealDamage`, `Damage`.
- 7 (0x800) laser, instant: damage `CalcDamage(7)` (0 for a partner), a fixed sound (no draw); the beam runs
  from `(z − 10, x, y)` of the shooter to the target point (a unit's `(+0x1dc, +0x1d4, +0x1d8)`, a building's
  `+0x14` height and `0x47c520` of its `+0xc/+0x10` cell); `0x47af70` copies that end and, **only on the
  battle's colony screen** (`AktKep == 3`, `AktPlanet == WF+0x18c`, not entering/leaving, a 3D scene
  0x93DA34), marks the target's scene object (0x451210, flag 0x100000) and runs the renderer's line test
  `vonalmetszes` (0x53a2b0) from `end + 20·(start − end)` to the end; a hit (0xDE01DF) replaces the end by
  the hit on the target's mesh. Then a `villamgenesis` beam object (0x457350 → 0x616190: 12 points,
  10 inner ones each jittered twice by 0x616620 = 3 libc `rand()`: **60 libc draws, on screen or not**)
  joins the UI effect list 0xA34FD4 (drawn and aged by the colony screen, not by the battle);
  `SetRealDamage(t, members·dmg, end − start)` and `Damage`. The mesh hit lies on the start→end line (the
  tested segment is that line), so `SetRealDamage`'s angle (normalised `(dx, dy)` against the body
  direction) is the target point's up to f32 rounding (≤ 7·10⁻⁷ rad in the traces): the port aims at the
  target point. It could differ only for an angle within ~10⁻⁶ rad of π/4 or 3π/4, or a shooter inside the
  target's mesh [render-dependent; not seen in 210 traced shots, 20 of them mesh hits]. `WF::Refresh`
  (§11.7) calls 0x47af70 too, for a list-A shell within 60 or a list-B projectile within 40 of its hit:
  there the result only places the explosion flash (0x45d270); the damage takes the projectile's velocity.
- An instant kill in idle fire (`ShotOne` before `+0x27c := t`, §11.7): `RemoveTarget` has already run,
  so the shooter keeps auto-aiming at the dead unit, whose object `RemoveTankGroup` has `operator delete`d
  with the pointer kept: later idle fire reads the freed record (cell, owner) and may shoot at it again
  [original quirk: a use after free; the port keeps the dead unit's last record, which matched the exe's
  freed block in the traces].
`CalcDamage(k)` (0x45fb40) = `rndlog(2, min_k, max_k)`. `SetRealDamage(t, &dmg, v)` (0x49c3c0): a ground
vehicle (hull 0..4, not a turret or mine) takes `(int)((1/armour[hull][side])·dmg)` (exe 0x8F858C, 5 × 3
f32) by the angle between its body direction and `v`: < π/4 front, < 3π/4 flank, else rear.
`WF::Damage` (§5.5) on a unit: a libc `rand() % 15 == 0` → a hit voice (§11.12). `WF_Tank::Damage` (§5.6):
when the whole group is dead it clears the alive cache `+0x125` and, on screen, bursts (16 slow pieces +
a sound draw; not for partners).

`RemoveTankGroup(u)` (0x497c90): on the colony screen, outside `RearrangeTankGroups` (0x9B2EBC), a group
whose alive count (the cached `AliveTankNum`) is 0 → a death voice (§11.12); `alive := 0`; a mine: every
unit's last mine cell `+0x254/+0x258` equal to its cell is cleared, a burst on screen, a sound draw, and
every counted unit within 100 takes `(int)((100 − d)·500)` (`d` through f64). `RemoveTarget(t)` (0x499070):
every unit targeting `t` drops it (flags 0x900 off) and every unit auto-aiming at it (`+0x27c`) forgets it,
**both marked moved** (`+0x285 := 1`, which lets `GetNearestEnemy` pick them that same tick); list A's
target and shooter, list C's target and list B's target and shooter (not for bombs, whose `+0x34` is a
height) become 10000. `EpuletDelete(p, slot)` (0x53c730): the `'+'` building cancels the self-destruct
(`+0x334 := −1`); on the colony screen `Remove3dBuilding` (0x497760) leaves a ruin (`Rom::Rom` 0x455570: a
burst of 24 pieces, §11.12); `RemoveBuildingTurrets`; the slot empties; `RemoveTarget(−slot)`.

Mines: a layer (`+0x1d0` mine bits 0x4000/0x8000/0x10000/0x20000, laying flag `+0x250`) drops one per new
cell while it has ammunition `+0x120` (`CalcDamage(2)` and the alive count are drawn for an unused damage):
`AddMine(cell, owner, hp 3000/5000/10000/20000, weapon 0x4000/0x8000/0x100/2)` (0x45c770: none on a cell that
has one; the first dead slot; a new object; flag 0x20000; turning 0; range and reload of non-sim weapons 8
and 4 for the weapons 2 and 0x100), then remembers the cell.

### 11.11 Not ported (real-time)
- `GenerateMap`'s editor mode (`GenerateMap(p, 1)`: layer 0 and mask A written back, §11.13).
- `WF::MI` (0x486d40, stream 13): the battle AI, ported separately (`ground-ai.md`, `War::mi`).
- The rest of `Refresh2` (per UI tick: visuals, debris flight, the effect list 0xA34FD4 with the laser
  beams; its dropship pass is §11.16), the MP battle protocol (the dropships' arrival doesn't run in MP).
- The laser's mesh intersection is renderer work (it needs the shown 3D scene); the port uses the target
  point, which gives the same armour side (§11.10).
- An AI-against-AI battle on screen has no end in `WF::Refresh` (§11.7 step 9).

### 11.12 Cosmetic libc draws
libc `rand()` decides nothing here, but every draw is replayed in order because debris occupies list-B
slots (§11.7). On top of the particles in the projectile rounds (3 per object per round), the hits and
the dust:
- `AddRepesz` (0x45d400, debris, list B kind 7, only while `+0x334 ≠ 0`): 2 draws on screen. Its ticks
  `+0x2c` are **never initialised**: whatever the heap block held (often a small count from a freed object,
  so it expires within a few rounds; else never) [original nondeterminism: the Windows heap's reuse order
  isn't reproducible]. The port gives debris −1 (it never expires in this tick).
- Bursts (1 flash draw + 3 per piece, velocities `((r%60)/10 − 3, (r%60)/10 − 3, (r%30)/10 − 4)`, ÷1.5 when
  slow): a dying group 16 slow + a sound draw, a mine 16 slow, a turret's building falling 16 slow, a
  building hit directly falling 24, a ruin 24, a self-destructing building 16 slow.
- Voices (`ColonySpeaker`, a `SpeakingDirector` at 0xA3EA3C; `InitColonySpeech` ELF 0x4d2250):
  `WF::PlaySpeechGrWar(line, kind)` (0x48c670, on screen and outside `Simulate`) → `SpeakingDirector::Play`
  (0x563cd0: nothing with `Lang == 3` (0x912724); speaker = line + 3 for races 4, 5, + 6 for races 3, 7, 8
  (`EN`); the speaker's first `Speeches` of that kind) → `Speeches::Play` (0x563d50): only when its cooldown
  `+8` is 0: cooldown := 10, and one `rand() % samples` when it has samples. The cooldowns count down once
  per colony-screen frame (`SpeakingDirector::Refresh` from `Colony_Main`), so they are UI state: the port
  takes them as input (`Voices`). Lines: a hit (`rand() % 15 == 0`): the victim's (line `u%3 + 1`, kind 5)
  when it is the human's on screen, else the shooter's (`by%3 + 1`, kind 8; 10000 → line 2); a death
  (`RemoveTankGroup`, §11.10): line `u%3 + 1`, kind 7 (the human's) or 9. `RearrangeTankGroups` sets the
  mute byte 0x9B2EBC while it merges groups and **returns early without clearing it** when its split loop
  finds only single-vehicle groups [original quirk].

### 11.13 Passability maps (`PlanetAtjarasMap::GenerateMap` 0x479300, ELF 0x4ae090)
`FUN_0045f600(p)` (first thing in `grUtkeres`) calls `GenerateMap(p − 1, 0)` when the flag byte
`0xA333A4 + p·0x24` is 0, then sets it; either way it clears `0xA333A0 + p·0x24`. It is a `thiscall` on
`ESI = 0xA33384 + p·0x24`: `[0]` a 320000-byte scratch (8 slope classes per cell), `[1..5]` five 200 × 200
u16 maps, `[6]`, then the flag at `+0x20`. The path search's view of the planet object starts 4 bytes later
(`PlanetAtjarasMap` 0xA33388 + p·0x24, maps `[0..5]` = layer 0..4 and a copy, flag `+0x1c`). Game mode
(`param_2 = 0`) allocates `[2..5]` (layers 1..4), never touches `[1]` (layer 0: null), sets
`[6] := [2]` and frees `[0]`. Cell index `x·200 + y`.

1. **Heights** `h(x, y) = GetHeight((f32)(((f64)x + 0.5)·0.25), (f32)(((f64)y + 0.5)·0.25), p − 1)`
   (`x, y < 200`, §11.2; the `> 200 → 0` guard never fires), the planet's own colony map `+0x23`, stored
   from the x87 return as f32. Samples with `fy ≥ 49` take corners in height row 50, which lies past the
   grid: the first 51 i16 of the **next** record `Terepek[m + 1]` (its background byte, three padding bytes,
   the weather list's heap pointer, its count, then string buffers with leftover bytes)
   [original nondeterminism: per run; it reaches the classes of the cells `y ≥ 195`].
2. **Slope classes** of the inner cells (`x, y ∈ 1..=198`) towards the 8 neighbours, of which only
   `(x, y − 1)`, `(x − 1, y)`, `(x + 1, y)`, `(x, y + 1)` are read: `d = h(nb) − h(cell)` (f32),
   `v = (f32)((f64)d·0.0488)`, `v < −2.9 → −2.9f`, `v > 2.9 → 2.9f` (both compared in **f64**, the f32
   constants stored), class `(i8)(i32)(v·10)` (f32 product, truncated). A class is gentle for layer `k` when
   `(f32)|class| ≤ limit[k]` (`limit` = exe 0x8F8574, f32 × 5, `ig2-rules::ground_slope_limits`).
3. Per layer `k = 1..=4` (map `[k + 1]`, limit `limit[k]`): fill with 65001; an inner cell becomes 1 when
   layer 3 (unconditionally), or 3–4 of its 4 classes are gentle, or exactly 2 are and they lie on
   different axes (one of `y ± 1`, one of `x ± 1`; asm 0x4796e8). The border stays 65001.
4. Loop: every 65000 → 20; layer 1 only: `FUN_0048b1a0(map, 1)` puts the start (globals 0x99644C,
   0x996450) on the first 1 found on square rings around the centre (ring `r = 0..=98`, `lo = 99 − r`,
   `hi = 100 + r`: the row `y = lo` for `x = lo..=hi`, the row `y = hi`, the column `x = lo` for
   `y = lo..=hi`, the column `x = hi`), none → (100, 100); then `FUN_0048af10(map, slopes, 65000, start,
   limit)` floods: the start's value `v` becomes `v + 1` (`v + 2` if that is 65000), and each marked cell
   marks its 4 neighbours (flat `n ± 1`, `n ± 200`) holding `v` whose class from the marked cell is gentle,
   then becomes 65000; sweeps forward and backward until nothing changes; returns the cells marked. Layer 1
   repeats while that is under 7000 (each small region becomes 20 at the next pass, so the search moves
   on); layers 2..4 fill once from layer 1's last start.
5. Every 1 and every 20 → 65001. Obstacles: each set 2-bit block of mask B (`Terepek[m] + 0x330`, 50 rows
   × 13 bytes, cell `col` at bits `(col % 4)·2` of byte `row·13 + col/4`, cols 0..50) makes
   `x ∈ [4·col, 4·col + 4)`, `y ∈ [4·row, 4·row + 4)` 65001 (layers 1..4; editor layer 0 skips this).

So a layer's free cells are the slope-connected region of at least 7000 cells nearest the centre (layer 1)
and the region holding that start (layers 2..4), minus the obstacles. With the shipped limits (layer 1 the
strictest, layer 3's above the largest class magnitude, 29) layer 3 is everything inner, and a start in
layer 1's region is gentle in every looser layer, so layers 2..4 never refill. [Original hang/overrun] When layer 1 has no
region of 7000, the ring search fails, the fill restarts at (100, 100): a 20 there is refilled forever (the
loop never ends), a 65001 floods the blocked cells, the border and the heap around the map (two fuzzed
recordings crashed the game this way); the port stops after that fill (`Passability::hung`). The backward
sweep's slope pointer looked off by 64 cells in the decompile; the asm (`slope + 0x4e1fb` for cell 39999)
shows it is right.

Editor mode (`GenerateMap(p, 1)`, not ported): `+0x84` := 0 first, layer 0 only (`limit[0]`), and the
blocks with fewer than 11 of 16 cells free are written back as mask A (`+0xa6`), then the base height `+0x84`
:= the height at the start cell.

Port: `ground::generate_map(map, mask_b, limits) -> Passability` (`atjaras.rs`; the heights past the grid
come from `GroundMap::heights` beyond 2500, else 0); the maps `[0..6]` as `GroundField::passability`
indexes them, the start, the layer-1 fill count. Verified by `tests/atjaras_trace.rs` (direct calls on every
colony map, fuzzed records) and inside the battle replay (§11.14).

### 11.14 Verification
`tools/trace/run_ground_rt.py --mode battle` + `ground_rt.js` (`traces/2026-10-16-ground-realtime.md`):
stages a landing (`AddTanksToPlanet` + `StartGrWar`), opens the planet's colony screen and hooks the game's
own `FRISSIT_Grwar` (0x4747c0), `WF::MI` and `WF::Refresh`, recording byte-region diffs of the battlefield,
every unit with its hit-point/origin/waypoint arrays, the three projectile lists and their objects, the
passability maps, the planet, the buildings, the landing zone, the fleets and vehicles, the stream seeds
and the voices; libc `rand()` is pinned by `srand` at each call. Options: `--small` (battles small enough
to end), `--human` (the human attacks: the end countdown), `--end-after T` (the attacker's groups are
removed with the exe's `RemoveTankGroup` between two ticks), `--destruct-after T` (the self-destruct timer
set to 3), `--fuzz`, `--speed`, `--ai-fuzz [--ai-fuzz-every N]` (arms AI units with air strikes; its writes
go out as an `aifuzz` event before `WF::MI`), `--laser` (the staged designs carry a laser pod 0xa5; every
beam end is logged as a `beam` event with the mesh probe: scene object, line tests, hit), `--mad` (the
human's units get flag 4 through the same `aifuzz` write, and now and then one mad tank's cell is moved next
to a building so the building pick's reservoir draw runs). `engine/crates/ig2-sim/tests/ground_rt_trace.rs` replays every call from its
recorded pre-state and compares every unit field, the lists, the field globals, the buildings, the voices,
stream 2 and the libc sequence. A planet's passability maps built inside a call are generated by the port
from the live colony-map record (§11.13) and compared with the exe's.
`tools/trace/run_atjmap.py` + `atjmap.js` (`traces/2026-10-16-atjaras.md`) call `GenerateMap` directly on
a scratch object for every colony map and for fuzzed records; `tests/atjaras_trace.rs` replays them.

### 11.15 Air strikes (`WF::ShotSpecWeapon` 0x49cb70 and what it makes)
Port: `ground/battle.rs` (`air_strike_objects`, `add_bomb`, `add_fighters`, `fighters_step`), called from
`ground/ai.rs` (`shot_spec_weapon`, the ground AI's strike: `ground-ai.md` §5.7). Verified in
`traces/2026-10-16-ground-air.md`.

**Impact point.** The AI's cell plus the spread `(rndlog(2, −4, 4), rndlog(2, −4, 4))` (lines 0x3135/0x3136,
fighters 0x3155/0x3156) gives `(cx, cy)`; its map position `(X, Y)` is the cell formula of §11.3 without the
+0.5 (f32 products, f64 centring); the height `h = GetHeight((f32)((f64)cx·0.25), (f32)((f64)cy·0.25), p − 1)`
of the battle planet's own map, 0 when either coordinate is above 200 [cells below 0 read before the height
grid in the exe; the port reads 0 there].

**Big ships** (kind 1): 20 bombs. Per bomb, in order: `a, b = rndlog(2, 0, 399)`, `c, d = rndlog(2, 0, 1999)`
(line 0x3140), all f32: start `(h − 3000, d + X − 1000, c + Y − 1000)`, aim
`((int)(h + 300), (int)(b + X − 200), (int)(a + Y − 200))`; then exe 0x457ad0 makes a list-B object (first
empty slot): velocity = the normalised aim − start (f32, `0x4501f0`: left as is below 1e-5) times
`(f32)(((f64)rndlog(2, 0, 14)/10.0 + 0.5)·3.0)·20.0f` (line 0x27b5), position = start, aim point = the aim
(as f32), ticks 300, damage 10, `+0x34` = `(int)h` (the ground height the blast tests against), kind 1,
shooter 10000; on screen one libc `rand()` picks one of two textures. The fade `+0x3c` stays uninitialised.

**Fighters** (kind 0): `WF::AddFighters(h, X, Y, e, 40, h)` (0x45b8d0) does nothing while a wave is up
(`+0x32c`); else it sets the flag and fills the five fighters `WF+0x1f4 + 0x40k` (the flag byte overlaps
fighter 4's unused `+0x38`): `θ = (f64)((f32)k / 5.0f)·π·2.0`, `s = (f32)(sin θ·500·3)`, `c = (f32)(cos θ·500·3)`
(f64), position `+0 (h − 1500) − 60k`, `+4 (f32)((f64)s·1.35 + X)`, `+8 (f32)((f64)c·1.35 + Y)`; velocity
`+0xc 12.0`, `+0x10 (f32)((f64)(−(s/1500.0f))·3.333·3.0)`, `+0x14` likewise with `c` (they fly across the
strike point); phase `+0x24 := 0`, ground `+0x28 := h`, bomb cooldown `+0x34 := rndlog(2, 0, 4)` (line
0x2991). `+0x18..+0x20` (the last step) keep the previous wave's values.

**The wave's flight** (`WF::Refresh` step 5): `2S` rounds; a round runs only while `+0x32c` is set and then
moves all five fighters: cooldown += 1; three libc draws (a trail particle); by phase `n`:
- `n < 95` (descending): `t = (f32)((f64)((ground − h)·0.5f)/3.333)`; cooldown ≥ 10 → `AddFighterLaser`
  (0x45b600, a list-B bomb of kind 2 from the fighter's position to `((int)ground, (int)(vx·t + x),
  (int)(vy·t + y))`, speed 20 (f32), ticks 300, damage 4000, `+0x34 = (int)ground`, shooter 10000, one libc
  `rand()` on screen) and cooldown := 0; then `h += vh`, step = `vh`;
- `n < 105`: `v = vh·(1 − (n − 95)/10)`, `h = v + h`, step `v`; `n < 145`: level, step 0;
- `n < 155`: `v = vh·2·((n − 145)/10)`, `h −= v`, step `−v`; `n < 200`: `h −= vh·2`, step `vh·−2`;
- `n ≥ 200`: no move.

Every moving phase: `n += 1`, `x += vx`, `y += vy`, the step `(·, vx, vy)` stored at `+0x18`. A fighter with
`n ≥ 200` clears `+0x32c` (the others still finish the round). All f32 unless stated. Blasts (§11.7 list B):
a kind-1 bomb within 100 (height) of its ground height hits units (not flag 2, not turrets) within 60 map
units for 200 and buildings within √5000 for 9000; a kind-2 bomb within 40 hits units within 50 for 6000 and
buildings for 1800 (`4000/1500·900`). Buildings take it through `WF::Damage` (§5.5), so a defence building's
turrets share it.

### 11.16 Transporters (`WF_Hordozo`, the landing orders and the partner pods)
Port: `ground/transport.rs` (`War::dropships`, `dropship`, `place_all_tanks`, `place_tank`, `unplace_tank`).
Verified in `traces/2026-10-16-transporters.md`.

**The dropship.** Every unit carries one (`WF_Hordozo`, inlined into the unit at exe `+0x264`): offsets
`+0x264/+0x268` on the map axes, height `+0x26c` above the unit (negative = up), a mesh `+0x270`, state `+0x278`.
The unit constructor (inlined at exe 0x4570c0; ELF `WF_Hordozo::WF_Hordozo` 0x4f2f70) sets state 1 and, from its
libc `rand()` `r`, height `(f32)(r % 500 − 1650)` and both offsets `height / 3.0f`. State bits: 1 away (no
pass), 2 arriving, 4 leaving, 8 carrying the unit, and the orders 0x10 deliver, 0x20 collect, 0x40 retreat
pick-up. `ArriveTransporter` = `(s & ~5) | 2`, `DepartTransporter` = `(s & ~3) | 4` (inlined in the exe).

**Cadence.** `WF::Refresh2` (exe 0x48ce10) runs once per 100 Hz colony UI tick (0x477dc0, called by the UI loop
at exe 0x4de8e8; it adds 1.0 to the f32 `ColonyCounter` 0x99823c first) for **every** planet's battlefield.
When `(int)ColonyCounter % 5 == 0` it calls `WF_Hordozo::Refresh2(WF, x, y, z, i)` (0x48e0a0) for every alive
unit without flag 2 whose dropship isn't away, on or off screen. So the dropships move at 20 Hz but on the UI
clock, between the battle ticks, and they draw on stream 2, a game stream [original quirk: a UI-paced draw].

**A pass** (`dropship(i)`; all f32 unless stated; `v(h) = max(2, (f32)sqrt((f64)−h))`, a NaN stays NaN):
1. Arriving (not in MP): `v = v(h)`; `h = v + h`; both offsets `+= v / 3`. `h > 0` → touchdown: offsets and
   height 0, then by the order bits, tested in this order:
   - 0x10 deliver: state `(s & ~0xb) | 4` (leaving, empty); the unit loses flag 0x8000000 (it may move).
   - 0x20 collect: state `(s & ~3) | 0xc`; `RemoveTarget(i)`; unit flag 0x8000000; then the common part.
   - 0x40 retreat: state `(s & ~3) | 0xc`; the unit loses the paralysis and seizure flags, owner := side, gets
     flag 0x40000000 and loses flag 8; `RemoveTarget(i)`; `rndlog(2, 0, 19)` (line 0x32b1) `< 6` → **shot
     down**: on the unit's map point a flash (one libc draw) and 16 slow pieces of debris (§11.12, 3 draws
     each), `RemoveTankGroup(i)`, the pass returns true (no climb).
   - none: `RemoveTarget(i)`, state `|= 8`; then the common part.
   - common: the unit loses the paralysis and seizure flags, owner := side.

   Still descending with the unit on board (bit 8 of the state at entry): unit flag 0x40.
2. Leaving (bit 4, also right after a touchdown): with 8 the unit loses flag 8; `v = v(h)`; `h −= v`; offsets
   `+= v / 3`; the exhaust particle draws six times on **stream 0** (lines 0x3300, 0x32fe, 0x32fd, 0x32fc,
   0x32fb, 0x32fa: the exe evaluates the arguments last to first, the ELF the other way round). `h < −1400` →
   away (state `|= 1`), with 8 the unit gets flag 2 (it left with the dropship), a new height
   `(f32)(rndlog(0, 0, 499) − 1650)` (line 0x3307) with offsets a third of it, and the status cache `+0x8cc` is
   cleared. Else, with 8, unit flag 0x40.

**Who sends them.** `AddTanksToPlanet` (§3.1): an AI attacker's vehicles get `(s & ~0x25) | 0x1a` (arriving
with the unit, deliver), so they stand at once but cannot move until touchdown clears flag 0x8000000; a human's
stay away with flags 2 and 0x8000000 until ordered. `Retreat` (0x499260, in the landing zone, SP): `(s &
0xffffffe2) | 0x42`; `StopRetreat` (0x4a4d00): `(s & ~3) | 4`. The human's orders, called by the colony
screen's mouse handler (0x47b350) between ticks:
- `PlaceAllTanks(x, y, owner, all)` (0x48c150): `(x, y)` clamped to 11..=189; every alive unit of `owner`,
  not a turret or mine (0x4020000), dropship away, and flag 0x8000000 unless `all`: `LocateTankSuitablePosition`
  from `(x, y)` (landing zone closed, §11.1), failing that from the default start; `SetPosG`; flags `&= ~2`,
  state `((s & ~5) | 0x1a) & ~0x20`, flags `&= ~0x8000000`.
- `PlaceTank(x, y, owner)` (0x48c2c0): the first alive unit of `owner` with flag 0x8000000 and its dropship away;
  the placement search from `(x, y)` must return `(x, y)` itself (a failed search leaves its outputs at
  `(owner, y)`); `SetPosG(x, y)`; then `GetNearestLandingZone` (0x47b980: `LocatePos` from the unit's cell with
  the test 0x4609a0 `zone[y·200 + x] ≠ 0`, **a row off** the zone's `(y − 1)·200 + x` layout [original quirk];
  the zone is generated first) must also return `(x, y)` (a failed search keeps it); then as above. A unit
  that fails the second test stays moved but away [original quirk].
- `UnplaceTank(i)` (0x4a4f70): only with the dropship away: not arriving → `((s & ~5) | 2) & ~8`, else
  `(s & ~3) | 0xc`; then `& ~0x10 | 0x20`, flags `&= ~2`: the dropship comes back for the unit (a pending
  retreat order 0x40 is then overridden by the collect).

**The transporter pod** (0x9f, flag 0x400000) has no real-time behaviour of its own: it only gives the unit
its partner (§3.1; one hit point, flags 0x8000 | 2 | 0x8000000, never merged). An AI's partner stays on board
(flag 2: skipped by `WF::Refresh` and the dropships); a human's `PlaceAllTanks` places it like any vehicle (it
then fires for zero damage, §11.10). The link (`+0x1e0` carrier, `+0x1e4` partner) is used only by
`WF_Tank::Damage` (§5.6), `RemoveTankGroup`, `GroundWarFinished` (§6.1) and `Simulate`. The exe's crash (§8)
needs a split partner group; a transporter design landed as one vehicle never splits (the tracer's
`--transport` stages it that way).

### 11.17 The MP ground war (`GrWar_MultiCsata_*`)
Exe globals: in progress `GrWar_MultiCsata_folyamatban` 0xBAD13C, the planet `GrWar_MultiCsata_bolygo` 0xBAD140
(ELF 0x14480b4 / 0x14480b8; `…_vegetert` 0x14480c0 and `…_mehet` 0x1183164 have no readers). `GroundWarIndit` (exe
0x59c2d0) routed to real time sets the planet and, in MP, the flag, then `StartGrWar(p)` (§11.4, which in MP also
sends every machine to the colony screen of `p`). While the flag is set, `Fake_FRISSITES` (exe 0x4de720) runs
neither the clock (`FRISSIT_Datum`) nor the strategic systems: only `GroundWarSetBattleStatus` (exe 0x47caa0),
`IdoMultiPlayerBattle` += 1, `FRISSIT_Grwar` (unless a space battle runs) and `FRISSIT_SpaceWar`. `FRISSIT_Grwar`'s
MP branch (exe 0x4747c0 after the per-planet part) runs `WF::MI` and `WF::Refresh` for the flagged planet every tick,
whatever is on screen. `WF::Refresh`'s MP end (exe 0x494b70.., ELF 0x48d750 line `else if`): no countdown;
`GetStatus() ≠ −1` → `GroundWarFinished(p, s, false)`, `+0x190 := 1`; `GroundWarFinished` clears the flag (§6.1
step 12). MP routing (§2) sends a war to real time only when both sides are human; the AI (`WF::MI`) skips human
empires and idle fire needs a target that has fired, so without the players' orders (`MULTIGET_Colony`, ELF
0x4ec2e0: move 0x22, waypoint 0x23, target 0x24, stop 0x25, detonate 0x12d, retreat 0x12f, air strike 0x130, mine
laying 0x134; re/screens/colony.md §9.1, ported as `War::order`) the two sides never engage [original behaviour].

The canonical view (ours, `multiplayer.md` §7) runs exactly this on the sim clock (`game::realtime`): the flag and
planet are synced state, `WF::MI`/`WF::Refresh` of that planet run in the tick, `GroundContext::multiplayer` is set
(MP landing, retreat, status and end rules) and `mp_war` mirrors the flag; the dropships (§11.16) fly once per sim
tick for every battlefield (the 100 Hz UI's every fifth pass is 20 Hz). The capture of `GroundWarFinished` step 7 is
carried out by `Game::with_wars` after the field's part (the same order: before the abandonment test). Tested by
`ig2-net/tests/lockstep.rs::two_humans_fight_a_ground_war_in_lockstep` (with orders), `a_player_drops_mid_battle_and_rejoins`; the MP end branch is not traced against the
exe (MP needs a second machine).
