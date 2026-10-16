# Compiled-in rules tables

Status: **catalogued and runtime-verified** (`traces/2026-10-15-tables.md`). The engine reads them
from the user's own `ig2.exe` (`engine/crates/ig2-rules`, ADR 0004). No values are committed here.

## How they were found
1. ELF `.data` symbols with sizes (`nm -S`); element strides from `imul` constants at the access sites.
2. The same bytes searched for in `ig2.exe`'s `.data`. Most tables are **byte-identical** between the
   64-bit ELF and the 32-bit exe: plain-old-data structs without pointers.
3. Field offsets from accessor code (Ghidra C of the ELF), and from correlating `.scn` keys with table values.
4. Frida (`tools/trace/verify_tables.py`): live memory dumped at the main menu and after
   `Betolt_Scenario("scenario/quick.scn")`, compared with the exe file.

## Catalogue (GOG `ig2.exe`, sha256 2baa530b…0442)

| Table (ELF name) | exe VA | stride × count | Contents | Runtime check |
|---|---|---|---|---|
| `Talalmanyok0` | 0x8DE048 | 276 × 193 | technology defaults (copied to live `Talalmanyok` @0xB9C0C8) | names localised at start-up (bytes 0–15); all else = file |
| `TalalmanyokrEro` | 0x8EB730 | 4 × 193 | per-tech firepower/strength (Fighter 10, … = res_info "FIREPOWER") | = file |
| `ScirmishTalalmanyFejlesztes` | 0x8EC218 | 4 × 193 | per-tech skirmish start state [inf] | = file |
| `Talalmanyok2` | 0x8EB060 | 9 × 193 | u8 `[tech][race]`: the AI's tech priority for the special variant of a ground hull (`TervAutoDesign`, `ai-empire.md` §10.3) | = file |
| `EpuletekLeiro` | 0x8EBA38 | 36 × 56 | building descriptors | = file |
| `PodTipusok` | 0x8F8918 | 84 × 55 | equipment pod types | names set at new game (bytes 4–16) |
| `PodFajok` | 0x8F9B28 | 8 × (14·27·12) | design slot grid: [pod table][hull][slot] → (pod type, max) | = file |
| `HanyasFajPodTablazatatHasznalja` | 0x902F10 | 4 × 19 | race → pod table (races 9–18 share table 1) | = file |
| `Alapok` | 0x90BBF8 | 36 × 25 | hull records: class, role flags, id, hull tech (`space-combat.md` §2.1) | = file |
| `Tervek0` | 0x90BF80 | 132 × 201 | default designs, same layout as live `Tervek[e]` (`space-combat.md` §2.3) | names localised (0–19) |
| `fegyverek` | 0x91D858 | 92 × 40 | space weapons: kind, reload, damage per target type (`fegyverszamol` recomputes it from tech firepower; live values matched the file), hit chance, spread, speed, duration, range, graphics, flags (`space-combat.md` §2.2) | = file |
| `TankAlapSpeed` | 0x8EC9D4 | f32 × 5 | ground hull base speeds (hull techs 145..=149; design speed = value / 100) | = file (ELF bytes identical) |
| `FajBolygoViszony` | 0x90B9D8 | 4 × 15 × 9 | race × planet surface affinity [inf] | = file |
| `Research_MeretSzorzok` | 0x9185D0 | f32 × 192 | research size multipliers (all 1.0) | = file |
| `tipusnevek` | 0x91F488 | 11520 B | ship type model names (space combat) | = file |
| `TransfAlap`, `TankAlap`, `TankTorony`, `WF_*Weapons` | see `verify_tables.py` | — | trade/ground-combat graphics and weapon lists | = file |
| `FajSpeckoGyartasKapacitasSzorzo` | 0x8F8870 | f32 × 15 | per-race production capacity (race 5: 1.25) | = file |
| `FajSpeckoKutatasIdoSzorzo` | 0x8F87F8 | f32 × 15 | per-race research time (race 1: 0.7, race 5: 1.2) | = file |
| `FajSpeckoKutatasMoneySzorzo` | 0x8F87BC (read by `FRISSIT_Research` exe 0x51e006, `production-research.md` §2) | f32 × 15 | per-race research cost (same values as time) | = file |
| `NehezsegGyartasSzorzo` / `NehezsegPenzSzorzo` | 0x8F8834 / 0x8F8848 | f32 × 5 | difficulty: production (0.75/1.2/2.0/1.8), money (0.75/1.2/1.6/1.5) | = file |
| `FejlesztesSebessegSzorzo` | 0x8F885C | f32 × 5 | development speed (1.0/1.6/2.5/2.5) | = file |
| `FajBekeMogalomSzint`, `FajKereskedelemIgenySzint`, `FajFelelemOnmegsemmisitotolSzint` | 0x90B924, 0x90B960, 0x90B99C | i32 × 15 | per-race diplomacy thresholds (peace, trade demand, fear) | = file |
| `Megbizhatatlansag_DeclareWar` | 0x902FC0 (read by `DiplomaciaMegtamadas` exe 0x4bd850) | i32 × 2 × 4 × 2 | unreliability a war adds: `[undeclared][relation row][victim, others]` (`diplomacy.md` §8.1) | (not dumped) |
| `FajMiBolygokHanyadReszeGyartas`, `FajMiBolygokMilyenKatonaiAutiBuild` | 0x915F90, 0x915FCC | i32 × 15 | AI production share (50%), AI military autobuild level | = file |
| `ProblemakLeiro` | 0x912858 | 24 × 20 | colony problem curves `{a, b, c, d, rate, weight}` (`colony-economy.md` §5) | = file, except entry 4 rate/weight (rewritten per planet) |
| `CsaladtagMoralCsokkenesLeiro` | 0x913838 | 4 × 6 | morale loss per kidnapped-family-member state | = file |
| (wormhole colours) | 0x913850 | i32 × 6 × 3 | wormhole `w`'s RGB; `AddWormhole` 0x53aec0 tints both ends `255 − (255 − c) / 2` (`fleet-movement.md` "Opening a wormhole") | = file |
| `RadarHatotav` | 0x913898 | f32 × 6 | radar range per radar tech level 0..=5 (`fleet-movement.md` §6; code reads index 6 = next table's word 0) | = file (Frida, every traced tick) |
| `RadarHatotav2Init` | 0x9138B0 | f32 × 8 | planet radar range per planet radar level, copied to every empire at new game; levels 3..=7 repeat `RadarHatotav[1..=5]` | = file |

[amb]: identical contents occur at several addresses. The address used by code is confirmed only where noted.

## Technology record (276 bytes)
| Offset | Type | Field | Evidence |
|---|---|---|---|
| 0 | char[20] | name (English built-in; localised at start-up into bytes 0–15) | runtime diff |
| 20 | i32 | production price | `GetGyartasAr` (race 5: ×0.7 outside techs 81–144 except 13/14; races 4, 7: tech 13 ×0.75 of tech 13's value) |
| 24 | i32 | production time | `GetGyartasIdo` (race 4: tech 13 halved) |
| 38 / 40 | u8 / i32 | auto-design trigger | `TalalmanyKifejlesztve` |
| 45–47 | u8[3] | prerequisite techs | `.scn` rNeeded correlation; all valid indices |
| 49–51 | u8[3] | required lab levels | `.scn` rKutatasSzint correlation |
| 52 | i32 | research cost | `.scn` rKutatasMoney |
| 56 | i32 | research time | `.scn` rKutatasIdo; `TalalmanyKifejlesztve` scales by race multiplier |
| 32 | i32 | hit points (designs, defence buildings: `TervUjraSzamol`, `SimulateSpBattle`) | `space-combat.md` §2.4 |
| 37 | u8 | vehicle capacity added to a design | `TervUjraSzamol` |
| 28, 36 (bytes 36, 38–39), 60–68 | — | [unk] | |
| 69 + e (e = 1..18) | u8 | per-empire project state (5 = developed) | SaveScenario rProjectAllapot; runtime |
| 88 + 4e | i32 | per-empire research progress | rKutatasAktIdo |
| 164 + e | u8 | per-empire "needed reset" flag | rNeededNullazva |
| 192 + 9i + j | u8 | per-empire spy knowledge matrix | rKemkedeshezIsmeri |

**Finding:** after a scenario load, the live table's balance fields equal the exe defaults exactly.
The `.scn` `TALALMANY` cost/time/prerequisite values (which disagree for 5–34 of 192 records in
`quick.scn`) are **not authoritative**: they're stale and the game ends up with the built-in values.

## Building descriptor (36 bytes): pinned in `colony-economy.md` §2.3 (runtime-verified)
`+0 u8 tech` (cost = that tech's price/time) · `+1 u8 [unk]` · `+2 u8 priority class 0..=6` (energy and
workers are cut from class 6 down) · `+4 i32 construction work` (days; ×50 at placement) · `+8 i32 max HP` ·
`+12 i32 energy demand` · `+16 i32 worker demand` · `+20 i32 output` (effect = output × yield% × efficiency/256) ·
`+24, +28 i32 [unk; unused by the economy]` · `+32 bit0 effect = scaled output` · `+33 bit0 effect persists while idle`.

## Building models (`Building`, exe 0x8F1F28, 60 × 0x6c; ELF .data 0x1164980, 60 × 0x70)
Kind `k` at record `k − 1`: `+0` model name (`.xvb`), `+0x58 i32` footprint width, `+0x5c i32` height on the
200 × 200 colony grid (`colony-placement.md`). Kinds 1..=55 are real; 56..=60 are zero. `ig2-rules`:
`building_footprints` (by kind). Other fields [unk].

## Pods
`PodTipusok`: `+0 i32 id`, `+4 char[16] name`, `+20 i32[16]` allowed techs (0-terminated).
`PodFajok[table][hull][slot]` = `(pod_type, max_count)`, with the table chosen by
`HanyasFajPodTablazatatHasznalja[race]` (`GetMaxPodok`, `GetMaxRarakhato`, `TalalmanyRarakhato`).
Techs 161–169 can be fitted anywhere when `DesignBarmiRarakhato` is set.

Per-empire radar tables derived from these at runtime (`RadarHatotav2` 0xBB61A8, `…Negyzet` 0xBB6408,
`RadarHatotavSzorzo` 0xBB6668, `…Negyzet` 0xBB66B8) are game state, not rules: `fleet-movement.md` §6.1.

## Open
- Hull +0x10..+0x23 (design/spy screens), weapon +0x40/+0x48/+0x59 and kinds other than 2 (tactical battle, not ported).
- Tech words +28/+36/+60..; building +1/+4/+24/+28; the leading `-` on some built-in names (`-Fighter`) [unk marker].
- `FajSpeckoKutatasMoneySzorzo` exact address. (`Talalmanyok2` is `[193][9]` u8, read by `TervAutoDesign`: `ai-empire.md` §10.3.)
