# Save games (`save<n>.sav`) and the multiplayer state blob

Port status: reader `ig2-formats::save` (`SaveGame::parse`, `SaveGame::parse_multiplayer`), writer and state
`ig2-sim::save` (§8); verified (§9).

Sources (ELF = unstripped x64 `ig2`, symbol oracle; exe = shipped `ig2.exe`, normative):

| Function | ELF | exe |
|---|---|---|
| `LoadSave(slot, name, mode)` (mode 1 load, 2 save) | 0x628180 | 0x543c80 |
| `LoadSaveVariable(ptr, len, mode)` | 0x624d90 | 0x5440d0 |
| `LoadSaveVariables(mode)` (the stream body) | 0x6254b0 | 0x544220 |
| `Diplomacy_Save` / `Info_Save` / `Research_Save` | 0x50dfa0 / 0x5694e0 / 0x5eca70 | 0x4c0080 / 0x4ea780 / 0x524590 |
| `WF::LoadSave` / `WF_Tank::LoadSave` | 0x4a6420 / 0x4a6e70 | 0x485420 / 0x4859a0 |
| `loadsave_ptr<T>` (pointer arrays) | 0x4f0fd0.. (4 instances) | 0x4552f0 |
| `GetSavedGame(player)` / `LoadSavedGame(blob)` (multiplayer) | 0x628a50 / 0x628c50 | 0x543670 / 0x545a90 |
| `GetSavePath` | 0x624d80 | 0x543660 → 0x5579c0 |

## 1. Files

`GetSavePath()` = the user data directory (`%LOCALAPPDATA%\GyroscopeGames\ig2\` on Windows). Slot `n` is
`save<n>.sav`; the game's autosave uses slot 24 (name `"Autosave <year> <month> <day>"`). Saving writes
`safesave.sav` first, then copies it to `save<n>.sav` in 1 KiB blocks (so `safesave.sav` is always the last save
made). Loading (`mode 1`) calls `InitNewGame(0,5,0,0)` first, then reads the slot file.

## 2. Raw header (80 bytes, little-endian)

Written with compression off (`UseZ = 0`), every field **without** a length prefix:

| Offset | Size | Field |
|---|---|---|
| 0 | 9 | `"VERS0002\0"`. On load `atoi(&magic[4]) == 2` switches compression on. |
| 9 | 31 | Save name: `strncpy(buf, name, 0x1e)` of a 200-byte stack buffer: NUL-padded, **byte 30 is uninitialised stack** (seen `0xff`). |
| 40 | 4 | Version (written as the constant 0x6a = 106) |
| 44 | 4×5 | `Ev`, `Honap`, `Nap`, `Ido`, `OldNap` (exe 0xBAE040/44/48/4C/58) |
| 64 | 4×2 | `EN` (0x912A3C), `CampaignType` (0x928278): only when version > 100 |
| 72 | 4 | zlib stream length `compress_length` |
| 76 | 4 | `decompress_length`: the **buffer capacity**, not the used length (§3) |
| 80 | … | zlib stream (`compress()`, default level) |

The header duplicates the calendar so the load menu can list saves without inflating them.

## 3. The stream and its framing

`LoadSaveVariable(ptr, len, mode)` with compression on appends `[i32 len][len bytes]` to a heap buffer (save) or
consumes it (load); `len <= 0` writes and reads nothing (no prefix). The exe's load path skips the 4-byte prefix
without checking it (the ELF asserts `prefix == len`, save.cpp line 0x75), so the stream must be walked with the
schema: a count saved earlier sizes the later chunks.

The buffer starts at capacity 100,000 (`operator new[](0x18704)`) and grows by `len + 10000` when
`cursor + len + 4 >= capacity`. `compress()` is then run on the **whole capacity**, so the stream carries a padding
tail after the last chunk (fresh heap memory: zero in every observed save) and `decompress_length` = capacity.
Example: 976,391 bytes used of 989,491. A reader stops after the last schema chunk; a writer may pad or not
(the loader never reads the tail).

After loading: `KemKuldetesek[0x160] = 0` (exe 0x9130A8), `FRISSITES_IGAZIT()`, `FlottakUjraSzamol()` (fleet totals
are recomputed, not trusted); `IdoPauseMertAtlepesAlattVan` (0xBAE265) brackets the call. A failed save shows
"Failed to save game!".

## 4. Section order (`LoadSaveVariables`, exe 0x544220)

`v` = the version chunk. `[n]` = an i32 chunk whose value sizes later chunks. All sizes are bytes. Exe VAs are
image addresses (the exe is relocated at runtime). Names are the ELF's; the reader uses them as chunk names.

### 4.1 Calendar and settings
| Chunk | exe | Size |
|---|---|---|
| version | (local) | 4 (= 0x6a) |
| `Ev`, `Honap`, `Nap`, `Ido` | 0xBAE040..4C | 4 each |
| `IdoMultiPlayerBattle` (v > 0x68) | 0xBAE054 | 4 |
| `OldNap`, `Nehezseg`, `Ido2` | 0xBAE058, 0x92827C, 0xBAE050 | 4 each |
| `scenario_kezdesi_ido`, `scenario_registry_name` (v > 0x5b) | 0xBAD108, 0xBAD0E8 | 4, 0x1e |
| `Multi_ido`, `Multiplayer_speed` (multiplayer blob only) | 0xDB949C, 0xBAE0AC | 4, 4 |

### 4.2 Diplomacy (`Diplomacy_Save`, exe 0x4c0080)
`AktPlayerek` 0xA41E98 (0xf50), `BirodalmiMacskak` 0xA41CD8, `…Papagajok` 0xA41D28, `…Evfordulok` 0xA41D78,
`…Gyengesegek` 0xA41DC8, `HanyevesAzUralkodo` 0xA41E48 (0x4c each), `VoltmarUdvozloUzenetEbbenAzEvben` 0xA41CC4
(0x13), `AktDipUzenet` 0xA41E14 (0x34), `AktDipValasz` 0xA43748 (0x30), `SpecialListaMut` 0xA41DC4 (4, **saved
twice**), an unused i32 (v < 0x60 only), `PlayerSelected` 0x902F70, `PlayerSelectedOld` 0x902F74,
`MostHanyPlayerVan` 0xA41D24 (4 each), `SpecialLista` 0xA42DE8 (0x960), `MaxPlayerek` 0xAB64DC (4).
Then `DiplomaciaEngedelyezve`/`TradeEngedelyezve`/`SpyEngedelyezve` 0xBAE3AD/AE/AF (1 each).

### 4.3 Info screen (`Info_Save`, exe 0x4ea780)
16 small fields 0xB84BF8..0xB84C10 / 0xB79FB1..B4 / 0xB82980..88 (4,4,4,4,4,4,1,1,1,4,4,4,4,4,4,4: chart
selection, triangle position, max-design pointer, modes, star log counters, selection), `InfoCharts` 0xB7A000
(0x44c0), `InfoChartsSmoothed` 0xB7E4C0 (0x44c0), `StarLog` 0xB82990 (0x2260).
`BattlesWon` 0xB79FB8 (0x48; v ≥ 0x62, else zeroed). `Research_Save` (exe 0x524590): `ResearchOldForgato` 0x919598
(4), `Terulet_Akt` 0x919200 (0x34).

### 4.4 Messages
`Uzenetek` 0xDA6620 (0x3520); `UzenetekRandomKitol` 0xDABC10 (4 if 0x5b ≤ v < 99, 400 if v ≥ 99, else zeroed);
`UzenetPlaybackek` 0xDA9B40 (0x20d0); 19 message-state fields 0xDABDA0..0xDABDE0 with two bytes 0xDA65E7, 0xDA65FF
(sizes 4,4,4,4,1,4,4,4,4,4,4,1,4,4,4,4,4,4,4); `Tiltolista` 0xDABDE8 (0x50), `MaxTiltasok` 0xDABDE4,
`HanyUzenetFajta` 0xDABE38, `[UzenetekDb]` 0xE413D4, `[UzenetekDbHistory]` 0xE413D8, `ScrollUzenetek` 0xE40CB0
((db+1)·0x40), `ScrollUzenetekHistory` 0xE40F70 ((dbh+1)·0x40).

### 4.5 Campaign script state (`st_*`)
Four key/value queues, each `[n]` then n × (`[key]` 4, `[value]` 4):
`StoryVodor` (256 bins `StoryVodorClass` at 0xF42678, stride 0xc in the exe; one total over all bins, the
pairs bin by bin; on load each pair is re-added to bin `key & 0xff`), `HistoryVodor` (0xF43278), `StartVodor`
(0xF43284, v > 0x5c), `OlvasasVodor` (0xF432A8, v > 0x67). Then `[st_vOzonvizMut]` 0xF265D8 + `st_vOzonviz`
0xF265E0 (n·0xc), `StoryInterregnumIdo` 0xF22738 (4), `StoryAdat` 0xF273F8 (0x128), `CampaignType` 0x928278 (4),
`Story_Minden_Flotta_Latszik` 0xF265D4 (4), then four lists `[n]` + (n+1)·8: `sBolygok` 0xF23A08/0xF23A20,
`sSzamlalok` 0xF23A0C/0xF24388, `sLogikaiak` 0xF23A10/0xF24CF0, `sValtozok` 0xF23A14/0xF251A8;
`StoryTerroristaCselValoszinusege` 0xF23A18 (4).
Eight on-screen texts: `[len]` + text bytes (no NUL; from the CString header in the exe, `CString::GetLength` in
the ELF) + `SzovegSzin` 0xF16EB8+4k (4); `UtolsoSzovegIdo` 0xF16ED8, `SzovegEltolas` 0xF16EDC.
`[GameInfoMut]` 0xE3E3A4, then per entry of `GameInfo` (0xE3E3A0 → array, stride 0xc in the exe: +0 i32, +4
CString, +8 i32): `[len]` + text, `+0` (4), `+8` (4).

### 4.6 Empires
`EN` 0x912A3C (4) — in a multiplayer blob `Multiplayer_save_player` 0xE40C30 instead (the receiving player; a
loaded blob still writes it to `EN`); on load `ENGRAFIKA` (0xBB6798) = `EN`.
`Money` 0xBAD168 (0x98 = 19 × f64), `MoneyTeruletenkent` 0xBAD200, `MoneyHatar` 0xBAD590, `MoneyTeruletCsuszka`
0xBAD920, `MoneyTeruletSzorzo` 0xBADCB0 (0x390 = 19 × 0x30 each). `ProgramMode` 0xBAE544, `…Difficulty` 0xBAE548,
`…Sebesseg` 0xBAE54C, `ProgramPlayers` 0x912ACC, `ProgramPlayers2` 0x912AD0, `ProgramMachines` 0xBAE550,
`ProgramGalaxyShape` 0xBAE554 (4 each); `MultiplayerBattleStatusz` 0xBAE558, `OnlyServerCanModifyTime` 0xBAE55C
(v > 0x69). `PlayerKi` 0xBAE560, `PlayerFaj` 0xBAE574 (0x13), `PlayerNev` 0xBAE588 (0xd1 = 19 × 11),
`BirodalomEro` 0xBAEA78, `BirodalomUrEro` 0xBAEAC8, `BirodalomHanyBolygo` 0xBAEB18 (0x4c), `BirodalomEl` 0xBAE6AC
(0x13), `BirodalomVisszaSzamlalas` 0xBAEB68 (0x4c), `BirodalomIsmeri` 0xBAEBB8 (0x169 = 19²),
`BirodalomMiotaIsmeri` 0xBAED28 (0x5a4 = 19² × 4), `MiVeszelyAllapot` 0xDB27F8 (0x4c), two morale globals
0xBAEAC4/0xBAEB14 (4), **`RandSeed` 0xBAF2D0 (0x54 = 21 streams)**, `FullGDP`, `FullTax`, `TradeIncome{Last,This}Year`,
`TaxIncome{Last,This}Year` 0xBAE060..0xBAE1F0 (0x4c each).

### 4.7 Spies
`[KemHanyBeszolas]` 0xBAF334 + `KemBeszolasok` 0xBAF338 ((n+1)·0x30); `OsszHanyKem` 0xBB1F84 (0x24 = 9 i32);
for e = 1..8: `Kemek[e][0..=OsszHanyKem[e]]` (0xBAFCC8 + e·0x3DC, (n+1)·0x4c); `HanyKemLehet` 0xBB2388,
`HanyKemVan` 0xBB23D0, `HanyKemLehetOsszesen` 0xBB23AC (0x24 each), `HanyKemKozpontBolygonkent` 0xBB2418
(0x1c44), `CsaladtagElrabolvaKi` 0xBB405C, `…Kialtal` 0xBB4080 (0x24), `KemKuldetesek` 0x912F48 (0x360 = 27 × 0x20:
the mission table is saved, `+0x160` is cleared after load).

### 4.8 Fleets, ships, vehicles, wormholes, waypoints
`[OsszHanyFlotta]` 0xBB40A8 + `Flottak` (*0xBB40A4, (n+1)·0x80; load reallocates to n + 100),
`OsszHanySajatFlotta` 0xBB40B0 (0x4c), `[OsszHanyHajo]` 0xBB4104 + `Hajok` (*0xBB4100, (n+1)·6),
`[OsszHanyTank]` 0xBB4154 + `Tankok` 0xBB41A8 ((n+1)·8), `OsszHanyKolonizalo` 0xBB4108, `OsszHanyTerraformalo`
0xBB4158 (0x4c), `OsszHanyWormhole` 0xBB61A0 (4), `Wormhole` 0xBB60F8 (0xa8 = 6 × 0x1c; on load the four
`WormHole` scene objects per pair are rebuilt), `[OsszHanyWaypoint]` 0xBB60F0 + `Waypointek` (*0xBB41A4,
(n+1)·0xc, **only when n ≠ 0**; load reallocates to n + 100).

### 4.9 Planets and starmap view
`HanyBolygo` 0x912AC8, `HanyBolygo2` 0xBB6B58 (4; load sets `HanyBolygo2 = HanyBolygo`), `Bolygok` 0xB85EA8
((n+1)·0x188), then 0x324-byte (201 × i32) per-planet arrays: `BolygoUtolsoKozeliFlotta` 0xB99340,
`BolygoMennyiIdejeLazad` 0xB99668, `BolygoMennyiFoldiEro` 0xB99990, `BolygoMennyiErodEro` 0xB99CB8,
`BolygoMennyiIonGunEro` 0xB99FE0 (v > 0x58), `BolygoMennyiTuzoltosag` 0xB9A308, `BolygoNeededMoral` 0xB9A630
(v > 0x5d). `AktPlanet` 0xB9933C, `AktFleet` 0xB99664 (load: if beyond the fleets, the first fleet of `EN`, else 1),
`StarmapMode` 0xB9998C, `StarmapXStart/YStart/Rotate/Pitch/Zoom/PlanetKering` 0xB9A968..7C (load resets NaN or
|x|,|y| > 100000 to 0 and clamps zoom to 580).

### 4.10 Techs and research
For t = 1..192 (`Talalmanyok` 0xB9C0C8 + t·0x114): `+0x45` (0x13 status per empire), `+0x58` (0x4c time per
empire), `+0xa4` (0x13 given), `+0x2c`, `+0x30`, `+0x34`, `+0x38` (4 each), `+0xc0` (0x51 seen matrix) — 8 chunks
per tech. Then `KutatasSurgos` 0xB9C014 (0x13), `ResearchAktFejlesztett` 0xB9A9B8 (0x4c), `ResearchTerulet`
0xB9A9B4, `ResearchMezo` 0xB9AA04, `ResearchTalalmany` 0xB9AA08 (4), `LaborSzint` 0x912728, `LaborSzintOsszes`
0xBA90E0 (0x130 each), `DesignTervSorszam` 0xB9AA10, `AktTervMutato` 0xB9AAEC (4).

### 4.11 Designs and trade
`OsszHanyTerv` 0xB9C028 (0x4c); `TervekReallocate()`; for e = 1..18: `Tervek[e][0..=n]` (*(0xB9AA18 + 4e),
(n+1)·0x84). `[TradeHanyTradeCuccVan]` 0xF439CC + `TradeTervek` 0xB9AAF0 (n·0x84), `TradeHanySzallitas` 0xF66DE0
(0x4c), for e = 1..18 `TradeSzallitas[e]` 0xF439D0 + e·0x1DB0 (n·0x98; empire 1 at 0xF45780), `TradeOldNap`
0xF66E2C (4). Layouts: `rules/trade.md` §1.

### 4.12 Buildings
`AktEpuletTipus` 0xB9C0C4 (4), `HanyEpulet` 0xBA9518, `HanyEpuletLefoglalva` 0xBA9840, `HanyEpuletBlokk` 0xBA9B68
(0x324 each; the block counts are saved from a copy taken before the loop), then for p = 1..200
`Epuletek[p][0..=HanyEpuletBlokk[p]]` (*(0xBA9E90 + 4p), (n+1)·0x20; load reallocates each to the saved block count).

### 4.13 Ground battlefields (200 × `WF::LoadSave`)
Battlefield i = 0..199 at exe 0x9C3B30 + i·0x8EC (ELF `Warfare` + i·0xA20); load calls `WF::Free` first. Offsets
below are the exe's. Per battlefield:
- `+0x8c8`, `+0x8cc` (v > 0x52), `+0x18`, `+0x148` end-of-battle countdown (v ≥ 100, else −1), `[+0x180]` unit
  slots, `[+0xe0]` rockets, `[+0xe4]` paralysers, `[+0xe8]` lasers (4 each); `+0x190` (1), `+0x1ec` (0x140), `+0x32c`
  (1), `+0x184` (1), `+0x18c`, `+0x1a8`, `+0x1ac`, `+0x1b0`, `+0x1b4`, `+0x1b8`, `+0x334`, `+0x338` (4), `+0x33c`
  (0x4c0), `+0x7fc` (0x90), `+0x88c` (1), `+0x88d`, `+0x89f` (0x12).
- Pointer arrays (`loadsave_ptr`): a chunk of n u32 presence flags (pointer ≠ NULL), then per present entry:
  rockets 11 × i32 (`+8..+0x30`) + 1 byte `+0x34` (v ≥ 0x57, else 1); paralysers one 0x38 chunk; lasers one 0x44
  chunk.
- `+0x188` alive bytes (unit-slot count), the unit pointers' presence flags (4·slots), then one `WF_Tank::LoadSave`
  per **alive** slot (a new 0x290-byte object on load), then `+0x8d0`, `+0x8d4` (v > 0x4e).

`WF_Tank::LoadSave(mode, v, planet)` (exe 0x4859a0; group record 0x290 B, `re/rules/ground-combat.md` §1.2). Let
`short` = flags `+0x188` bit 0x4000000 (v ≥ 0x54); a short record skips the fields marked *:
`+0x188` flags (v ≥ 0x55; earlier read after `+0x15c`), `+0x10c`, `+0x104`* (1 byte; v ≥ 0x4c), `+0x1b0`,
`+0x1b4`, `+0x23c`, `+0x14`, `+0xe8`, `+0x158`, `+0x15c`, `+0x190` count, `+0x194` owner, `+0x198` side, `+0x19c`*,
`+0x108`/`+0x110`/`+0x114`/`+0x118`* (v ≥ 0x57), `+0x1a0`, `+0x1a4`, `+0x1a8`/`+0x1ac`/`+0x1b8`/`+0x1cc`*, `+0x1d0`
weapon mask, `+0x1e0..+0x1ec`*, `+0xc`, `+0x10`, `+0x12c`*, `+0x18`, `+0x150`/`+0x98`/`+0x9c`*, `+0x148`,
`+0x11c` (v ≥ 0x66; clamped to 0..2 on load), `+0x288`* (v ≥ 0x5f), `+0x120`* (v ≥ 0x56); ammunition `+0xa8..+0xdc`:
one i32 for every set bit 0..13 of the weapon mask (v ≥ 0x55; 14 words before, one copied to all before 0x51);
`+0xe0`*, `+0xe4`, `+0x1c0`*, `+0x1c4`, `+0x228`, `+0x1c8`, `+0x1d4`, `+0x1d8`, `+0x1dc`; the second ammunition
array `+0x1f0..+0x224` (same rule); `+0x1c` (0x24), `+0x244`, `+0x248`; hit points `*(+0x154)` and origins
`*(+0x19c)` (count·4 each), `+0x18c`, `+0x138`*, `+0x146` (1), `+0x13c` (10), `+0x240`*, `+0x24c`,
`+0x250` (1)/`+0x254`/`+0x258`*, `+0x25c`, `+0x260`, `+0x238`, `+0x264` (0x18)*, `+0x130` (1), `+0x134`.
On load `+0x280` = the planet, `+0xe0` = 0.65f, and a building-bound group (`+0x23c < 0`, ≠ 10000, flag 0x100)
recomputes `+0x14c` from the building's footprint.

**Writing** (`mode` 2, verified in the save replay: `rules/traces/2026-10-16-save-replay.md`): the same calls in the
same order, so a save holds exactly the fields above (at version 0x6a). Details that matter for a writer
(`ig2-sim::save::wf`, `SaveState::{ground_field, take_ground_field, store_ground_field}`):
- `WF::LoadSave` sets `+0x8cc = 0` **before** the variable in either direction: a save always writes 0 and leaves
  every battlefield's status cache invalid (`SaveState::battlefields_saved`, from `Game::saved`).
- The presence flags are `pointer ≠ NULL`. `RemoveTankGroup` (0x497c90) only clears the alive byte and
  `WF::RemoveTank` (0x497af0) frees the object without clearing the pointer, so a slot stays "present" once a unit
  was there; only slots loaded as absent stay 0 until a unit takes them. List objects (`ElementList` refresh
  0x4903f0) are freed and nulled. Every alive unit is written whole (`+0x19c`'s scalar is the origin array's heap
  address: per run, taken from the exe in a replay).
- The ammunition words travel in a fixed weapon-bit order: bit `b` of `+0x1d0` → word `AMMO[b]` of `+0xa8`/`+0x1f0`
  with `AMMO = [0, 8, 3, 12, 2, 13, 10, 11, 4, 5, 6, 7, 9, 1]`.
- `+0x8d0` is the battlefield's weather and `+0x8d4` its background (`melyikhatter`, read by `InitWeather`, ELF
  0x4c1160), both drawn in `FRISSIT_Planet`'s prologue (`rules/colony-economy.md` §3); the background cursor
  0x996488 isn't saved.
- `+0x18c` is the battlefield's planet (passed to every `WF_Tank::LoadSave`); `+0x14`, `+0xe8`, `+0x13c..+0x146`,
  `+0x240`, `+0x270/+0x274`, the battlefield's `+0x18`, `+0x184`, `+0x1a8..+0x1b8`, empire 0's AI block and the
  first 8 bytes of the list-B/C objects aren't read by any ported system: the port keeps them as loaded
  (`ground::{UnitTail, FieldTail}`). A unit object made since the load has the constructor's values; the saved
  fields the constructor (`WF_Tank::WF_Tank` 0x457020) and `AddTank` leave uninitialised are listed in
  `ground::UNINIT` (heap garbage in the exe: `+0xc/+0x10` for turrets, `+0x14`, `+0x18`, `+0x98/+0x9c`,
  `+0x1b0/+0x1b4`, `+0x23c`, `+0x1e0`, `+0x24c`, `+0x25c/+0x260`, `+0x274`, the `+0x19c` pointer).

### 4.14 Colony view, relations, AI
`ColonyXStart/YStart/Rotate/Pitch/Zoom` 0xBA90DC, 0xBA9514, 0xBA983C, 0xBA9B64, 0xBA9E8C (4 each), `Viszony`
0xDABF38 (0x3596 = 19² × 0x26), `DiplomaciaMikorHivtaUtoljara` 0xDAF520 (0x144, v > 0x57), `MiTervMutato` 0xDAF668,
`MiTervMutatoSpec` 0xDAFA20 (0x3b6), `MiTervHanyVan` 0xDAFDD8 (0x76c), `MiFoBolygo` 0xDB0548, `MiFoFlotta` 0xDB0598
(0x4c), `MiFelderitesRacs` 0xDB0738 (0x20bb = 19 × 21 × 21), `VanUresBolygo` 0xDB0724 (**1, then 0x13 again**:
the first byte is saved twice), `MiUtolsoSzarKolonizalas` 0xDB05E8, `MiMennyiIdeigNemPakolhatFelTankot` 0xDB0638,
`MiMennyiIdeigTartATankPakoloFazis` 0xDB0688 (0x4c), `MiMennyiIdeigNemLehetTervUpgrade` 0xDB06D8 (0x4c, v > 0x66),
`MiGyarthatoCuccok` 0xDB28E8 (0x1d64), `MiBirodalomKozeppontX/Y` 0xDB2848/0xDB2898 (0x4c; v > 0x60).
After the stream: two `st_vGet(0xfa3/0xfa2)` results into 0xB77288/0xB7728C.

## 5. Versioning

The version is a plain integer; the shipped exe and the ELF both write 0x6a and keep loading every older
version through the gates above (0x4c, 0x4e, 0x51, 0x52, 0x54–0x58, 0x5b–0x5d, 0x5f, 0x60, 0x62, 99, 100, 0x66,
0x67, 0x68, 0x69, 0x6a; header EN/CampaignType from 101). Fields absent from old versions get defaults on load
(`BattlesWon` 0, countdown −1, the WF_Tank floats 2.5/0.15/0.3/0, `+0x11c` 1, `+0x120` 3). Only 0x6a was
observed; the reader implements every gate.

## 6. Multiplayer state blob

`GetSavedGame(player)` (exe 0x543670) runs the same `LoadSaveVariables(2)` with `Multiplayer_save` (0xE40C2C)
set and `Multiplayer_save_player` = `player`, and returns a heap block `[u32 zlib length][u32 buffer capacity]
[zlib]` (no `VERS` header, no calendar copy). Differences from a file stream: `Multi_ido` and `Multiplayer_speed`
after the scenario name, and `player` in the `EN` slot (2 more chunks; everything else identical — verified on two
blobs taken at the same moment as file saves). `LoadSavedGame(blob)` (0x545a90) inflates and loads it with
`Multiplayer_save` set; nothing resets the flag but `LoadSave` (which clears it at its start). This is how the
original game hands a joining or rejoining client the host's state.

## 7. Win32 versus x64

**The stream is the same on both builds.** Every chunk length is an explicit constant or a saved count, never
`sizeof` of a pointer-bearing type; the size sequences of `LoadSaveVariables` and `WF_Tank::LoadSave` are
identical between the ELF and the exe. Pointer-sized state never reaches the file:
- Pointer arrays (`Flottak`, `Hajok`, `Tervek[e]`, `Epuletek[p]`, `Waypointek`, the WF unit/rocket/laser/paralyser
  arrays) are saved as their pointees' records; `loadsave_ptr<T>` turns a `T*[n]` into n **u32** presence flags
  (`std::vector<unsigned>` in the ELF, the same in the exe).
- `CString`s are saved as `[i32 length][bytes]` (the exe reads the length from the CString header at −8).
- `compress_length`/`decompress_length` are `unsigned long` (8 bytes) in the ELF but saved with length 4
  (little-endian low half), the same as the exe's 32-bit ints.
- Record sizes in the stream (planet 0x188, fleet 0x80, design 0x84, building 0x20, spy 0x4c, ship 6, vehicle 8,
  WF rocket 11 × 4 + 1, paralyser 0x38, laser 0x44, WF_Tank +0x1c block 0x24) are the same on both. The in-memory
  layouts that differ (ELF `WF` stride 0xA20 vs exe 0x8EC, `WF_Tank` 0x2A8 vs 0x290, `StoryVodorClass` 0x18 vs 0xc,
  `GameInfo` 0x18 vs 0xc) are re-serialised field by field, so their offsets don't matter to the format.
- Raw records with pointers inside would differ, but none is dumped raw: the WF_Tank's `+0x154`/`+0x19c` arrays are
  written by their element count.

So saves are portable between the Linux/x64 and Win32 builds of the remaster, and a reimplementation can treat
the format as architecture-independent.

## 8. Notes for the remake

- A parsed save gives the full original state at a day boundary of the player's choosing: usable to seed replay
  tests (load a save, run the port and the exe side by side) and as the import path for original saves. Port:
  `ig2-sim::save::SaveState` keeps the chunks as the exe's memory (`chunk_va` maps each chunk name to its global;
  `Talalmanyok0` from `ig2-rules` under the per-tech chunks, `LoadSave`'s `HanyBolygo2` and `KemKuldetesek+0x160`
  fix-ups) plus the pointer-held records, decodes every system's state from it and stores it back, and turns
  itself back into chunks (all saves round-trip byte for byte). `ig2-sim::game::Game` runs whole ticks on it; the
  replay against the exe is `rules/traces/2026-10-16-save-replay.md`. Not in the save but read by the sim (dump
  them from a live game, or keep the new-game values): `BirodalomMIzik` 0x915F58, `BirodalomTamadhat` 0x915F44,
  the radar tables, the story engine's statics and its finish/special queues, `DipOldNap` 0xAB6FFC, the
  weighted-choice state 0xBAEB64/0xBAEBB4, `AutoBuildMitEpit` 0x996530, `NextColonyMap`'s cursor 0x996420,
  `HanyTradePortVan` 0xBB23F4, `IdoSpeedSzorzo` and the debug/tutorial switches (list in `tools/trace/replay.js`).
- Our own save/rejoin format is a separate decision (Phase 3 ADR); the original's blob shows the minimum the host
  must send (the whole stream) and that the RNG seeds (`RandSeed`) are part of it, but libc `rand()` state is not
  (ADR 0003 moves those draws to seeded streams).
- Original quirks kept by the reader: byte 30 of the name is uninitialised; `SpecialListaMut` and
  `VanUresBolygo[0]` are saved twice; the padding tail after the last chunk; `KemKuldetesek` (a rules table) is
  saved and loaded.

## 9. Verification

`tools/trace/run_save.py <install> _local/saves --days 3,40,150 [--ships] [--mp] --tag <t>` loads
`scenario/quick.scn`, snapshots the save's source globals at the listed days and calls the exe's `LoadSave(97+k,
"Probe k", 2)` from the frame hook (same thread, nothing runs in between), then moves `save<97+k>.sav` to
`_local/saves/<t>-<k>.sav`. `ig2-formats/tests/saves.rs` parses every `.sav` under `IG2_SAVE_DIR` (default the
repository's `_local/saves`) completely (every chunk length checked, zero tail) and compares the calendar, `EN`,
money, seeds, relations, story data, spies, fleets, ships, vehicles, planets, techs, designs, building counts and
buildings, and the AI production table with the snapshot byte for byte; `--mp` blobs are checked against their
file save. Result: 9 saves (3 quick, 2 with warships, 2 with MP blobs, 2 game autosaves from other traced runs;
29,741 chunks, 175 fleets, 339 ground-war groups) parse and match.
