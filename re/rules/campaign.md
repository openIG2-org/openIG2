# Campaign scripting: the story engine and the scenario stories

Status: **engine core, the scenario stories, `InitStory`, the inline campaign stories and the storylines
(`st_Line_include`) verified at runtime** (`traces/2026-10-16-story.md`, `traces/2026-10-16-campaign.md`,
`traces/2026-10-16-line.md`), ported in `engine/crates/ig2-sim/src/story.rs`, `campaign.rs` and `storyline.rs`
(`tests/story_trace.rs`, `tests/campaign_trace.rs`); the MP game modes (`st_Multi_include`, §8.10,
`traces/2026-10-16-multi.md`) in `multi.rs`, the random stories 220..239 (`st_BRnd_include`, §8.11,
`traces/2026-10-16-brnd.md`) in `brnd.rs`. The campaign's other scripts are mapped (§8.8) but not ported yet.

The campaign is not data: the 1999 authors wrote every story as C code (`story.cpp`, `st_begin.cpp`,
`st_rnd.cpp`, `st_brnd.cpp`, `st_fable.cpp`, `st_scen.cpp`, `st_line.cpp`, `st_elem.cpp`, `st_multi.cpp`), each
story a block `if (st_Story(id, 1)) { if (st_StoryStart(0)) {…} if (<trigger>) {<actions>} … }` over a small
engine of story buckets, variables, logic flags, counters, planet ids and event flags. The `.scn` files only
start stories (`Story n`) and set variables (`vSet id value`) and counters (`szStart id days`).
The scripts call the other systems directly (fleets, planets, diplomacy, messages); in the port those calls are
the [`StoryWorld`] reads and [`StoryEvent`]s.

**The exe is normative.** It inlines `st_Story`, `st_StoryStart`, the variable and counter accessors and most
event predicates, and drops the scripts' `SDL_ReportAssertion` loops (release build), which leaves some
`st_StoryStart` checks with an empty body (§6). Where it differs from the ELF it is noted.

## 1. Functions

| Function | exe | ELF | Role |
|---|---|---|---|
| `Story_Check(top)` | **0x5c5610** | 0x739810 | runs every script, then the end-of-check processing (§4) |
| `FRISSIT_Story` | **0x5c1ff0** | 0x73ef50 | every tick: counters, the "daily" check (§3) |
| `StartStory(id, now)` | 0x5c4840 | 0x731320 | queue a story (§2.2) |
| `WIN(id, history)` | 0x5c9770 | 0x730aa0 | a story ends (§2.3); `LOST()` = `WIN(0, 1)`, `DIE()` = `MEGHALAS(EN)` (0x53b630) |
| `MEGNYERES(1)` | 0x53f6b0 | 0x61eb50 | the human wins the game (an event here) |
| `NEWGAME_Story` | 0x5c3040 | 0x7308a0 | reset (§2.6), then reads the story data files (`story-data.md`) |
| `InitStory(mode)` | 0x5c2940 | 0x72ec70 | the campaign set-up (§8.2) |
| `Story_Generate` | 0x5c8d30 | 0x73f0a0 | clears the event flags, `StoryInterregnumIdo` 0xF22738 := 0, `StoryTerroristaCselValoszinusege` 0xF23A18 := 30 |
| `st_Scen_include` | **0x601580** | 0x774c00 | the scenario stories 360..375 (§6) |
| `st_Line_include` | **0x5eb6f0** | 0x7771e0 | the storylines 501..720 (§8.9) |
| `st_BRnd_include` | **0x5c9f20** | 0x768d50 | the random stories 220..239, 2301, 2311 (§8.11) |
| other scripts | 0x5cec20, 0x5f0e20, 0x5c9f20, 0x5e6ba0, 0x5d4b70, 0x5ee4d0 | §8 | `Begin`, `Rnd`, `BRnd`, `Fable`, `Elem`, `Multi` |
| `st_vSet` / `st_vGet` | 0x605780 / 0x605650 | 0x72f530 / 0x72f9d0 | variables (§2.4); `st_vAdd` & co. are inlined in the exe |
| `st_szStart` / `st_szGet` | 0x605570 / 0x6054c0 | 0x735a90 / 0x735a20 | counters (§2.5) |
| `st_bSearch(id, mode)` | 0x603d30 | 0x72f590 | planet by story id, or a random planet of an empire (§2.7, stream 5) |
| `st_fSearch(id, mode)` | 0x605070 | 0x7313d0 | fleet by story id (`+0x16`), or a random fleet of an empire (§2.7, stream 5) |
| `st_bAddNewAzonosito(p, id)` | 0x6031f0 | 0x7353f0 | give planet `p` the story id `id` (§2.7) |
| `st_fNewEx`, `st_fAddShip`, `AutoAddship`, `st_AddKolonizalo` | 0x604bc0, 0x604410, 0x5c1d80, 0x5c9dc0 | 0x733ab0, 0x7303e0, 0x7301d0, 0x7350d0 | story fleets (§8.6) |
| `st_Ozonviz(type, a, owner)` | 0x5f0c90 | 0x7309a0 | register an undo action for `WIN(owner)` (§2.3) |
| `st_AddGameInfo(id, text, …, owner)` / `st_RemoveGameInfo(id)` | 0x5c9d00 / 0x5f0d50 | 0x7338b0 / 0x7339c0 | an info line on the starmap (§2.3) |
| `StoryVodorClass::Add` / `Remove` / `RemoveFirst` | 0x5430a0 / 0x5c4750 / 0x5c47e0 | 0x668f30 / 0x79f530 / 0x7a0c90 | the queue class (§2.1) |

Hooks other systems call (they set an event flag and its payload, most then run `Story_Check(1)`):

| Hook | exe | ELF | Flag | Payload (exe VA) |
|---|---|---|---|---|
| `Story_FleetArrivedSave(f)` | 0x5c8cb0 | 0x7396e0 | 0 (no check) | only if `f.+0x42 ∈ {0,1,2}`: `ErkezettFlotta` 0xF2654C = `f.+0x16`, `…Number` 0xF26550 = `f`, `FleetArrivedCel` 0xF26554 = `(int) f.+0x44`, `…CelTipus` 0xF26558 = `f.+0x42`, `…CelFlottaAzonosito` 0xF2655C = target fleet's `+0x16` when the type is 2, else 0 |
| `Story_FleetArrivedCheck()` | 0x5c8c90 | 0x7397f0 | | `Story_Check(1)` if flag 0 is set |
| `Story_SpaceBattleStarted()` | 0x5c9220 | 0x73d9c0 | 1 | clears the space-battle story switches (`space-combat.md` §3); story 108's special case (fleets 0x439/0x43a) |
| `Story_GroundBattleStarted(f, p)` | 0x5c8fc0 | 0x73e470 | 2 | only if `StoryAdat+0x54 == 0`: fleet (0xF26538), its id and owner, `StoryAdat+0xF4` = `p`'s owner, `+0x40 := 0`, both ground switches := 0, `GrWarBolygo` 0xF26544 = `p` |
| `Story_SpaceBattleFinished(list, n, check)` | 0x5c9180 | 0x73dc50 | 3 | `SpWarYouWasInBattle` 0xF26531 = some `list[i].+0x18 == EN`, `SpwFlottak` 0xF265C4 = `list`, `SpwFlottaNum` 0xF265C8 = `n`, `StoryAdat+0x88 := 0`; check if `check` |
| `Story_SpaceBattleFinishedCheck()` | 0x5c9200 | 0x73e350 | | if flag 3: `StoryAdat+0x88 := 0`, check |
| `Story_GroundBattleFinishedSave(f, p, won)` | 0x5c8de0 | 0x73e590 | 4 (no check) | fleet/id/owner, planet, its owner 0xF26548, attacker won 0xF26532 / defender won 0xF26533, `StoryAdat+0x88 := 0` |
| `Story_GroundBattleFinishedCheck()` | 0x5c8d80 | 0x73e690 | | §5 |
| `Story_GroundBattleSimulateCheck(f, p)` | 0x5c8e70 | 0x73e380 | | `Story_GrWarSimulateTiltas` 0xF42670 for stories 808/814/824 (`ground-combat.md` §2) |
| `Story_MessageSelected(b, r)` / `…Elevules(b)` / `Story_LetiltasCheck(b, r)` / `…Megnezes(b)` | | 0x73e730 / 0x73e770 / 0x73e7a0 / 0x73e7e0 | 6 / 7 / 8 / 9 | message block and part; `Megnezes` only once per block (`OlvasasVodor` 0xF432A8) |
| `Story_ArrivedToRadarRangeFleet(f)` / `…Bolygo(p)` | 0x5c55f0 / 0x5c55d0 | 0x73e850 / 0x73e880 | 10 / 11 | 0xF26580 / 0xF2657C (`fleet-movement.md` §6) |
| `Story_Diplomacy_Special(which, e)` | 0x5c8c60 | 0x73ec40 | 13 | 0xF2656C / 0xF26570, answer 0xF42660 := 0 (`diplomacy.md` §5) |
| `Story_Special(type, d1..d7)` | 0x5c9550 | 0x73e8b0 | 14 | `SpecialType` 0xF265A4, `SpecialData1..7` 0xF265A8.. |
| `Story_KemEsemeny()` | 0x5c9070 | 0x73ec90 | 15 | SP only: `KemkedesTipus` 0xF26588 := 3 (`espionage.md` §2.3) |
| `MULTIGET_Story` | | 0x740670 | | MP: message 0x65 → `StartStory(id, 0)`, 0x66 → `Story_Special(15, e, id, 0…)` |

Counters (flag 5) and the daily check (flag 16) come from `FRISSIT_Story` (§3), the "story finished" signal
(flag 12) from the end of `Story_Check` (§4).

## 2. State (exe VAs)

| Name | exe VA | Layout |
|---|---|---|
| `StoryVodor` | 0xF42678 | 256 queues of 12 bytes `{int *id, int *dual, int n}`: the running stories, bucket `id & 0xff` |
| `HistoryVodor` | 0xF43278 | finished stories: `(id, Ido at the end)` |
| `StartVodor` | 0xF43284 | stories waiting to start: `(id, 0)` |
| `VegzesVodor` | 0xF43290 | "finished" signals waiting: `(id, type)` |
| `TalalmanyVodor` | 0xF4329C | specials waiting: `(type, data1)` |
| `OlvasasVodor` | 0xF432A8 | message blocks already read (`Story_MessageMegnezes`) |
| `sBolygok`, `sHanyBolygo` | 0xF23A20, 0xF23A08 | planet ids, 1-based, 8 bytes `{i32 id, u8 planet}`, room for 300 |
| `sSzamlalok`, `sHanySzamlalo` | 0xF24388, 0xF23A0C | counters, 1-based `{i32 id, i32 ticks}`, max 300 |
| `sLogikaiak`, `sHanyLogikai` | 0xF24CF0, 0xF23A10 | logic flags, 1-based `{i32 id, u8 value}`, max 150 |
| `sValtozok`, `sHanyValtozo` | 0xF251A8, 0xF23A14 | variables, 1-based `{i32 id, i32 value}`, max 500 |
| `StoryError` | 0xF23A1C | 1 when the last accessor found no entry |
| `st_vOzonviz`, `…Mut` | 0xF265E0, 0xF265D8 | undo actions `{type, a, owner}`, 0-based, < 299 |
| `StoryEsemeny` | 0xF2F8AC | 17 event flags (§1) |
| `StoryLastChecked` | 0xF2273C | the story the script is in (set by every `st_Story(id, 1)` that finds `id`) |
| `SzamlaloMelyikVegzett` | 0xF26560 | the counter that just ran out |
| `StorySignalMelyik` / `…Tipus` | 0xF26564 / 0xF26568 | the finished story and the signal type |
| `FRISSIT_Story::OldIdo` | 0xF432B4 | `Ido` of the last daily check |
| `DelayedCheck` | 0xF26530 | run another top-level check at the end |
| `StoryAktKuldetesNowReStarting` | 0xF261FC | the story whose start flag survives the check |
| `Story_check_locked` / `ent_count` / `Story_SkipFirst` | 0xF265DC / 0xF432E0 / 0xF23A04 | re-entrancy (never set to 1 in the exe), depth, a flag cleared at the end |
| `last_addship` | 0xF26200 | a pending `st_fAddShip` (handled before and after the scripts) |
| `StoryAdat` | 0xF273F8 | 0x128 bytes of story switches other systems read (`StoryAdat+0x24`, `+0x6c`, `+0xb0`…, see the other specs) |

### 2.1 Queues (`StoryVodorClass`)
Two parallel `int` arrays and a count. `Add(id, v)` appends (reallocating both arrays); `Remove(x)` removes
the first entry whose id is `x` **or `−x`**; `RemoveFirst` drops entry 0; `isElem(x)` = some id equals `x`;
`GetDual(x)` = the second value of the first entry with id `x`, else 0; `isStart(x)` = `GetDual(x) != 0`.

A story `id` is running when bucket `id & 0xff` holds it (`st_Story(id, mark)`: when found and `mark`,
`StoryLastChecked := id`). Its second value is the **start flag**: 1 for exactly the first `Story_Check` the
story sees (`st_StoryStart(0)` reads it for `StoryLastChecked`).

### 2.2 `StartStory(id, now)`
`id == 0` → false. If `id` is running: `StoryLastChecked := id`, false. Else `now` → `Add(id, 1)` to its bucket
directly; otherwise `Add(id, 0)` to `StartVodor` (the next top-level check starts it, §4). True.

### 2.3 `WIN(id, history)`, the undo list and game infos
`id == 0` → `StoryLastChecked`. If `id` is running, `StoryLastChecked := id`. Then for each undo entry `k` in
order with `owner == id`, apply it and zero the entry:

| type | action | | type | action |
|---|---|---|---|---|
| 1 | planet `a`: `+0xba −= 1` | | 8 | `st_bSetExtra(−a, 0)` |
| 2 | `BolygoInfoRemove(a >> 16, −(a & 0xffff))` | | 9 | `1 ≤ a ≤ HanyBolygo`: planet `a` `+0x6e := 0` |
| 3 | `Diplomacy_RemoveSpecial(a & 0xffff, a >> 16)` | | 10 | `KiszedUtolsoUzenet(a, 1)` |
| 4 | `RemoveGameInfo(a)` | | 11 | the human's designs ≥ 40 with `+0x34`: price `+0x24` and `+0x20` ×2, and planet queues `+0xa0` ×2 for them |
| 5 | `st_DisableKemKuldetes(a)` | | 13 | `StoryAdat[0xd0 + 4a] −= 1` if > 0 |
| 6 | `BolygoInfoRemove(a >> 16, (i16) a)` | | 14 / 15 | `StoryAdat+0x28 := 0` / `+0x24 := 0` |
| 7 | `st_RemoveTerv(a >> 16, a & 0xffff)` | | 0, 12, other | nothing |

Then trailing entries of type 0 are dropped from the count; `Remove(id)` from its bucket; `Add(id, 1)` to
`VegzesVodor`; if `history` and `id` isn't in `HistoryVodor`: `Add(id, Ido)` there. (Not running: the same,
the ELF builds a debug string.) Types 13–15 are story state (ported); the others reach the world (events).

`st_AddGameInfo(id, text, …, owner)`: `owner == −1` → `id / 10` (the story-id convention: variable 3711 belongs
to story 371). The UI line is added; then, if `owner != 0`, no entry `(4, id, owner)` exists and the list has
fewer than 299 entries, `(4, id, owner)` goes into the first entry of type 0 below the count, else is appended.
`st_RemoveGameInfo(id)`: zero every entry `(4, id, *)` (the count stays), remove the UI line.

### 2.4 Variables, logic flags
`st_vSet(id, v)`: the first entry with `id` (1..n), else a new entry if `n < 500` (`n += 1`); writes `id` and
`v`; `StoryError := 0`, or 1 when the table is full. `st_vGet(id)`: the value, else 0; `StoryError` = not found.
`st_vAdd/Sub/Dec(id, x)` change an existing entry only; `st_vInc(id)` = `st_vSet(id, st_vGet(id) + 1)`;
`st_vIsEqual/Greater/Smaller(id, x)`: `v == x` / `v ≥ x` / `v ≤ x`, false when missing. Logic flags (150) the
same with a byte (`st_lSetToTrue/False`, `st_lIsTrue`, `st_lIsFalse` = true when missing).
ELF quirk kept: `st_lSearchCreate` writes the id, but `st_vSearchCreate`/`st_szSearchCreate` don't; their
callers write it.

### 2.5 Counters
`st_szStart(id, days)`: entry as for `st_vSet` (300), `ticks := 50·days`. `st_szStop(id)`: `ticks := −1`
(creating the entry; a new entry keeps the stale id of whatever the slot held). `st_szGet(id)` =
`(ticks + 49) / 50` (C division), 0 when missing. `st_szIsNowGoing` = `ticks > 0`, `…IsEverStarted` = exists.

### 2.6 `NEWGAME_Story`
Undo list empty (the 0xe10 bytes zeroed), the planet/counter/flag/variable counts 0, `StoryAdat` zeroed, the
data-file cursors reset. It does **not** clear the queues (`InitStory` does) or the tables' bytes.

### 2.7 Planet and fleet ids
`st_bAddNewAzonosito(p, id)`: `p == 0` → nothing; else the entry with `id` (or a new one: `n += 1`, no limit
check) gets `(id, (u8) p)`. `st_bSearch(id, mode)`:
- `id > 0`: the planet byte of the first entry with `id`, else 0.
- `id ≤ 0`: count the planets `q` (1..`HanyBolygo`) with `owner == −id` and the human's visibility
  `q.+0xd8[EN] ≥ 0`; if none and `id == 0` and `mode != 1` the script is aborted (a C++ exception caught by
  `Story_Check`, §8.7); `r = rndlog(mode == 2 ? 2 : 5, 1, count)`; the `r`-th such
  planet, else 0.
`st_fSearch(id, mode)` (`mode` unused): `id == 0` means `−EN`. `id > 0`: the first fleet 1..`OsszHanyFlotta`
with `+0x16 == id` and owner `+0x15 != 0xff`, else 0. `id < 0`: `r = rndlog(5, 1, count)` over the fleets with
owner `== −id` (dead slots have owner 0xff), the `r`-th, else 0. Note the scripts then read slot 0 of
`Bolygok`/`Flottak` when a search fails.

## 3. `FRISSIT_Story` (every tick, after `FRISSIT_Research`, before `FRISSIT_Diplomacy`)
1. For each counter `i` = 1..n with `ticks ≥ 0`: `ticks −= IdoSpeedSzorzo`; if `ticks < 1`: flag 5,
   `SzamlaloMelyikVegzett := id`, `ticks := −1`, `Story_Check(1)` (which may add counters; the loop bound is
   re-read after each check).
2. The earthquake reset (`Colony_foldrenges` 0x9ABB70, `Colony_elozo_foldrenges` 0x9ABAE4,
   `…_ido` 0x9ABB74 + 10000 < `AktIdo` 0xBAE270 → both 0): colony screen, not modelled.
3. **Daily:** if `Ido < OldIdo` or `OldIdo + 50 < Ido`: `OldIdo := Ido`, flag 16, `Story_Check(1)`. Note the
   strict `<`: at speed ×1 this is every **51** ticks, at ×4 every 13 ticks (52 `Ido`), at ×16 every 4 (64).

## 4. `Story_Check(top)`
1. If `Story_check_locked`: return. `ent_count += 1`, `DelayedCheck := 0`; a pending `last_addship` is
   handled (event); `StoryAktKuldetesNowReStarting := 0`.
2. SP: `Begin`, `Rnd`, `BRnd`, `Fable`, `Scen`, `Line`, `Elem`; always `Multi` (§8, §6).
3. The inline stories 400..918 (§8), then: if `StoryAdat+0x30` and flag 16: if no planet has `+0xb8 > 0`
   (i16) and no live fleet (owner ≠ 0xff) has `+0x52` bit 2: `StoryAdat+0x30 := 0` and info 0x65 is removed from
   every planet that has it.
4. `last_addship` again. All 17 event flags := 0. In every bucket, every start flag `!= 0` of a story other
   than `StoryAktKuldetesNowReStarting` := 0.
5. If `top`: while `VegzesVodor` isn't empty: `(StorySignalMelyik, …Tipus) :=` its first entry, flag 12,
   `Remove(StorySignalMelyik)` from it, `Story_Check(0)`.
6. If `DelayedCheck`: `Story_Check(1)`.
7. If `top`: while `StartVodor` isn't empty: `Add(first id, 1)` to the id's bucket, `RemoveFirst`,
   `Story_Check(0)` (the story's start). Then: while `TalalmanyVodor` isn't empty: `(SpecialType, SpecialData1)
   :=` its first entry, `RemoveFirst`, flag 14, `Story_Check(0)` (the other `SpecialData` keep their values).
8. `Story_SkipFirst := 0`, `ent_count −= 1`, `Story_check_locked := 0`.

So a story started from a script runs its start block in a nested check of the same top-level call, and a
story that ends (`WIN`) sends its "finished" signal (`st_StoryFinish(id)` = flag 12, `StorySignalMelyik == id`,
type 1; `st_Signal(id, t)`) the same way.

## 5. Event predicates (the scripts' triggers)
All are false unless their flag is set. `0` as a parameter means "any".
- `st_Daily()` flag 16. `st_Counter(id)` flag 5 and `SzamlaloMelyikVegzett == id`.
- `st_ArrivedS(fid)` / `st_ArrivedB(fid, bid)` / `st_ArrivedF(fid, fid2)`: flag 0 with target type 0 / 1 / 2,
  `ErkezettFlotta == fid`, `FleetArrivedCel == st_bSearch(bid, 0)` / `…CelFlottaAzonosito == fid2`.
- `st_SpWarStart(fid)`: flag 1, one of the two fleets in `spFlottak` (exe 0xBAA1C6 / 0xBAA1CC: fleet slots) has
  id `fid`; `…StartID`, `…StartTulaj` compare the slot or the owner.
- `st_SpWarPart(fid)`: flag 3; `fid == 0` → 1; with ≤ 2 participants the 1-based index of the one whose
  `+0x24 == fid` among the first three, else the general search (`st_SpWarPart_slow`); `st_SpWarWin`/`Lose`
  compare `SpWarKiNyert` with the participant's `+0x18`. `…PartID`, `…PartTulaj` compare `+0x20` / `+0x18`.
- `st_GrWarStart(fid, bid)` flag 2 / `st_GrWarPart(fid, bid)` flag 4: fleet id and `GrWarBolygo ==
  st_bSearch(bid, 0)`.
- `st_Message(b, r)` 6, `st_Elevul(b)` 7, `st_Letilt(b, r)` 8, `st_Olvas(b)` 9, `st_RangeF(fid)` 10,
  `st_RangeB(bid)` 11, `st_Signal(id, t)` / `st_StoryFinish(id)` 12, `st_Diplomacia(which, e)` 13,
  `st_Special(type, d1, d2, d3)` 14, `st_KemElkap/Siker(id)` 15 with type 1/2, `st_KemEsemeny()` 15 type 3.
- `st_Diff(a, b, c, d)` by `Nehezseg`: 1 → a, 2 → b, 3 → c, else d. `st_GetValue(key)`: `values.txt` value
  × {0, 2.0, 1.3, 1.3, 1.0}[`Nehezseg`] (f32, truncated), 10 when missing.

## 6. The scenario stories (`st_Scen_include`, exe 0x601580)
Started by the skirmish `.scn` files (`Story 361` with `vSet 3611 …` etc.). Every story ends the same way on
success: `WIN(0, 1)` then `MEGNYERES(1)` (the human wins); `DIE` = `MEGHALAS(EN)` (the human loses). Unless
noted, the test runs on the daily flag. `v(x)` = `st_vGet(x)`, `b(x)` = `st_bSearch(x, 0)`, `f(x)` =
`st_fSearch(x, 0)`, `P[p]` / `F[f]` the planet / fleet record (slot 0 when a search fails).

| Story | Goal |
|---|---|
| 360 | `F[f(3601)].owner != 0` → win (the fleet with id 3601, or slot 0, is owned by anyone but empire 0) |
| 361 | every non-zero `v(3611..3617)` names a dead empire (`BirodalomEl[e] == 0`) → win |
| 362 | `P[b(3621)].owner == EN` → win |
| 363 | `f(3631) == 0` (the fleet is gone) → win |
| 364 | flag 0, target type 1, target planet `== b(3641)` and the arriving fleet `ErkezettFlottaNumber`'s owner is `EN` → win; also daily: `P[b(3641)].owner == EN` → win |
| 365 | `P[b(3651)].+0xd8[EN] > 0` (the human sees the planet) → win |
| 366 | start: `st_szStart(3661, v(3661))`; counter 3661 runs out → `WIN` only (no `MEGNYERES`); daily: `st_szGet(3661)` (no effect) |
| 367 | `(f64) v(3671) < MoneyMennyi(EN, 0)` → win |
| 368 | `v(3681) < BirodalomHanyBolygo[EN]` → win |
| 369 | `P[b(3691)].owner == EN` and `v(3691) < P[b(3691)].+0x28` → win (`b` searched twice) |
| 370 | `BirodalomEl[v(3701)] == 0` → `DIE` (protect empire `v(3701)`) |
| 371 | start: `st_szStart(3711, v(3711))`, `v(3712) := 0`; counter 3711 → `DIE`; daily: if `v(3712)` remove game info `3710 + v(3712)`; `d = st_szGet(3711)`: `d ≥ 361` → info 3711 "years" `(d + 359)/360`, `v(3712) := 1`; `d ≥ 31` → info 3712 "months" `(d + 29)/30`, `v(3712) := 2`; `d ≥ 2` → info 3713 "days" `d`, `v(3712) := 3`; `d < 2` → nothing |
| 372 | flag 1 (a space battle starts): `StoryAdat+0x94 := 1`, `+0x98 := 1` (no retreat); flag 3 (one ends): `f(3721) == 0` → win, else `DIE` |
| 373 | tech `v(3732)` has status 5 for empire `v(3731)` (`Talalmany[t].+0x45+e`) → `DIE` |
| 374 | some non-zero `v(3741..3747)` is allied with the human (`Viszony[EN][e].+0x25 == 5`) → win |
| 375 | `st_bSearch(−v(3751), 1) == 0` (empire `v(3751)` has no planet the human can see; draws stream 5 even then) → `v(3751) < BirodalomHanyBolygo[EN]` → win, else `DIE` |

Exe vs ELF: the start blocks of 360–365, 367–370, 372, 374, 375 only assert their ids exist (the exe evaluates
`st_StoryStart` and does nothing). In 366/371 the start block's work happens in both. 371's "days" info for
`d < 2` leaves `v(3712)` as it was in both.

## 7. Port
`story::Story` holds §2's state; `FRISSIT_Story` is [`Story::tick`], the hooks are methods, `Story_Check` is
[`Story::check`], the scripts are [`Script`]s run in the exe's order (a pair of scripts is one: so far
`((BRndStories, (ScenarioStories, LineStories)), MultiStories)`), then
`campaign::inline_stories` (§8). A [`StoryWorld`] gives the world reads, the RNG, the calls that return something
(`FlottaNew`, `st_IsInRR`, `st_GetTerv`, `LoadScenario`, `Generate_StarNames`, the config and registry reads) and
takes the [`StoryEvent`]s: `MEGNYERES`, `MEGHALAS`, the undo actions that reach the world, game infos, fleet
orders and new ships, the `StoryAdat+0x30` info removal, the campaign's actions (§8). The campaign tables come
from the user's exe through the world (`campaign_tables`).

## 8. The campaign (`InitStory`, the inline stories of `Story_Check`)
Ported in `engine/crates/ig2-sim/src/campaign.rs` (+ the fleet helpers of §8.6 in `story.rs`), verified at
runtime (`traces/2026-10-16-campaign.md`, `tests/campaign_trace.rs`). Of the scripts `st_Line_include` (§8.9),
`st_Multi_include` (§8.10) and `st_BRnd_include` (§8.11) are ported, the others (§8.8) are not yet.

A campaign starts in `InitNewGame(seed, 0, campaign, difficulty)` (exe 0x4e1130; the race menu's
`CampaignGameStart` 0x4f5e90 runs it with `CampaignType` = race 1..=3, human 1/3/5, difficulty 1, 2 or 4):
`NEWGAME_Story` (§2.6) reads the story data, `Story_Generate`, then **`InitStory(0)`**, the starting fleets,
`INIT_MI` and `StartStory(400, false)`; the first daily check starts story 400.

### 8.1 Tables
Contiguous in the exe's `.data` from 0x928340 (411 words, `ig2-rules` `campaign_words`, `CampaignTables` in the
port, which indexes them with the exe's arithmetic: the rows overlap where the exe's do). `ct` = `CampaignType`
1..=3; every row's `[0]` is a count.

| Table | exe VA (row `r`) | Use |
|---|---|---|
| `StoryEleje[6][10]` | 0x928340 + 0x28·r | opening episode groups (401): `[0]` count, then story ids; id 9 is special (§8.4) |
| `StoryElejeSeq[ct][4]` | 0x928420 + 0x10·ct | the group of each opening step; campaign 3's row is campaign 1's `StoryLine` row (unused: 3 has no 401) |
| `StoryLine[ct][10]` | 0x928428 + 0x28·ct | the storylines: `[0]` count, then the storyline stories |
| `StoryScenario[ct][10]` | 0x9284a0 + 0x28·ct | the campaign maps `scenario/<n>.scn` |
| `RndStoryk[5][35]` | 0x928540 + 0x8c·r | random stories; `[0]` is −1 in the image and set to the run of non-zero entries from 1 (all 5 rows) the first time `RndStoryk[0][0] < 0`; entries ≤ 0 are disabled |
| `RndStoryValasztas[ct][5]` | 0x9287e8 + 0x14·ct | the `RndStoryk` rows a campaign draws from |
| `StoryHosok[ct][10]` | 0x928810 + 0x28·ct | heroes: `[0]` candidates, `[1]` how many to start, `[2..]` the hero stories |
| `RndHosok[9]` (0x1c B) | 0x9288b0 + 0x1c·k | `+0`: the hero ships' story design ids (story 917) |

### 8.2 `InitStory(mode)` (exe 0x5c2940)
Clears the 256 buckets, `StoryAdat`, `HistoryVodor`, `VegzesVodor`, `StartVodor` (not the specials or the
read queue) and the storyline story (0xB77288). `mode != 0` (skirmish): nothing else. Campaign (`mode == 0`):
1. Storyline index `line` = `atoi(config "storyline")`; if 0: `ReadRegistrySetting(StoryRegString(ct))` (key
   `Fable_<ct>`); a successful non-zero read `v` gives `3 − v` (the other storyline of two), else
   `rndlog(5, 1, StoryLine[ct][0])`. `StoryLineStory` (0xB77288) = `StoryLine[ct][line]`.
2. Map: `atoi(config "storymap")`, else `rndlog(5, 1, StoryScenario[ct][0])`; `LoadScenario("scenario/<n>.scn",
   1)` with `n = StoryScenario[ct][map]` (its `RandSeed` reseeds streams 1–20; its `vSet` lines and planet
   `pAzon` ids reach the story tables).
3. `st_vSet(4001, line)`, `(4002, n)`, `(4003, StoryLineStory)`; 0xB7728C = `n`.
4. Two mirror flags `rndlog(5, 0, 1)` (x first): every planet's `+0x18` / `+0x1c` gets bit 31 flipped. The
   human's planets get buildings 2 and 6, campaign 2 also 0x32 and 0x34 (`EpuletNewSearchLocation(p, kind,
   true, 0, −1, −1)`). The ELF asserts the required planet ids (table 0xd5d9c0); the exe dropped that.
5. `Generate_StarNames` (0x59b860, stream 1): the constellations; it gives planets the story ids 500.. and sets
   variable 500 to their number.
6. `StoryAdat+0xbc := 1` in campaigns 1, 2; `+0xb8 := 1` in campaigns 2, 3.

### 8.3 Story 400: the campaign driver
Start: `StartStory` 118 (not campaign 3), 142, 199, 865 (campaign 2), 837 (campaign 3). Heroes:
`StoryHosok[ct][1]` times, a reservoir pick (`RndClass(5)`: the `n`-th candidate wins when `rnd(5, 1, n) ≤ 1`)
among the candidates that aren't running, then `StartStory` it; a hero already queued but not yet started can
be picked again and runs twice (seen in the traces). Campaign 3: counter 4001 := `rndlog(5, 300, 450)` days;
the others: `StartStory` 401 and 242. Campaign 3 with `v(4001) == 1`: `StartStory(842)`.
- 401 finished (`st_StoryFinish(401)`) or counter 4001: counter 4002 := `rnd(5, 120, 180)`, 4003 :=
  `rnd(5, 600, 650)`.
- Counter 4002, the storyline: `line = v(4001)`, 0 → `rnd(5, 1, StoryLine[ct][0])`; `StartStory(StoryLine[ct]
  [line])`; `SaveRegistrySetting(Fable_<ct>, line)` (the next campaign takes the other one).
- Counter 4003, a random story: a reservoir pick over the positive entries of the campaign's `RndStoryk` rows;
  started unless it is in `HistoryVodor`; counter 4003 := `rnd(5, 600, 900)`.

### 8.4 Story 401: the opening episodes
Start: counter 4011 := `rnd(5, 220, 280)`, `v(4011) := 0`. Counter 4011: `k = v(4011)`, `v(4011) := k + 1`;
`k == 2` and difficulty > 1: `st_AddKolonizalo(difficulty > 2 ? 2 : 1)` (§8.6); `k ≥ 4`: `WIN`. Otherwise group
`g = StoryElejeSeq[ct][k]`, episode `StoryEleje[g][rnd(5, 1, StoryEleje[g][0])]`, redrawn while it is 9 and the
human knows no live empire 1..=8 other than itself (the scan `e = 1, 2, …` skips `EN` only below 9, so it ends at
the first live known empire, 10 at the latest); `v(4012) := episode`, `StartStory(episode)`, counter 4011 :=
`st_GetValue(g + 1)` (§5) days.

### 8.5 The other inline stories
Each runs only while its story does; "start" = `st_StoryStart(0)`. The scripts (or the MP/tutorial code) start
them.

| Story | What it does |
|---|---|
| 402 | start: `StoryAdat+0x44 := 1`, `v(4021) := 0`. Special 0x12 with `AktPlanet` (0xB9933C) owned by another empire (not 0): `+0x44 := 0`, counter 4022 := 60, `v(4021) := 1`, the planet gets id 4021, info line 4021 "60". Daily while `v(4021) != 0`: info 4021 removed; `d = st_szGet(4022)`; planet 4021 free → `WIN`; `d == 0` → `st_BolygoDestroy(p)` (`StoryError = p < 1`), video `_s(6)`, `WIN`; else info 4021 "d" |
| 403 | start: counters 4039 := 25, 4038 := 12. Daily: escorts = the fleets of owner 10 with ids 4030..=4033 (4034 lands outside the exe's array), base `p` = planet 4031. Owner 10: if `v(4035)` stop counter 4035, `v(4035) := 0`. Otherwise every escort with `+0x5c != 0` is sent home (`FlottaMozgat(f, 2, 1, p, 0)`, no chase); none sent and none at home → `WIN`; else counter 4035 := 60 days and `v(4035) := 1` unless set. Counter 4035 (inside the daily block, so it practically never fires): delete the escorts, `WIN`. While guarding: each human fleet within `(int)(dy² + dx²) < 2500` (f32) of the base is offered to every escort not sent home with weight `2505 − d²` if within 2500 of it (`RndClass(5)` weighted: `rnd(5, 1, Σw) ≤ w`); an escort with a target follows it (`FlottaMozgat(f, 2, 2, target, 0)` unless already following, `+0x42 == 2`), one without stops if it was following. Counter 4038: re-armed 30 days, `turn = (v(4039) + 1) % 8`, idle escorts (`+0x42 == 0`) go to `base + 60·(cos, sin)((phase[k] + turn)·π/4)` (f64, truncated; phase 0, 4, 2, 6). Counter 4039: re-armed 150 days; one missing escort (reservoir pick) is rebuilt while the base is owner 10's: `st_fNewEx("", 10, 4030 + k, 0.08, 2, base)`, ships 3×3, 8×2, tanks 3×1 |
| 404 | start: trader 4041 (`st_fNewEx(…, 9, 4041, 0.1, 1, 0)`, or the existing one's speed `+0x3c := 0.1`), `v(4041) := Ido/50`. Daily: gone → `WIN`; budget `(Ido/50 − v(4041))·40`; it heads (`FlottaMozgat(f, 1, 2, fleet, 0)`) for the nearest (f32 `(int)(dy² + dx²)`, first wins) human fleet holding a ship whose design has a refit target (`+0x1c`) with `0 < price difference ≤ budget`. Arriving at a fleet (`st_ArrivedF(4041, 0)`): refits that fleet's ships in slot order while the difference (the human's designs, no `> 0` check) fits the budget (ship design := target), `v(4041) := Ido/50 − budget/40`, `FlottakUjraSzamol`, the trader stops |
| 405 | start, unless `v(4059)`: empire `StoryAdat+0xcc` sends message 0x23f0 (`NewMessage`, its name, its race's portrait); no empire → the story aborts (§8.7). Message 0x23f0 part 2 (`st_Message`): counter 4051 := 1. Counter 4051: `MEGNYERES(1)` |
| 406 | start: counter 4061 := 1. Counter 4061: `DIE()`, `WIN` |
| 902 | start: a ship of design 0x1c for `AktFleet` (0xB99664); a random planet of the human's (`st_bSearch(−EN, 0)`) gets id 9021. Special 0x11: wormhole (`AddWormhole`, `Wormhole3DSettings`) from fleet `SpecialData1` (truncated x, y) to planet 9021 + `(int)(r·(cos, sin)(a·π/180))` added in f32, `r = rnd(5, 20, 28)`, `a = rnd(5, 1, 360)` |
| 903 | start: a random free planet the human sees (`st_bSearch(0, 0)`, aborts when there is none) gets id 4031 and owner 10 (`st_SetBolygoTulaj(p, 10, −1)`: colonised when free, else conquered and `+0x21 := PlayerFaj[10]`), `StartStory(403)`, `WIN` |
| 904 | start: 5 ships of design 4 for `AktFleet` |
| 905 | start: `f = st_fNewEx(_s(5, 0x1f88), 10, 8072, 0, 1, 0)` heads for `last_fn_b` (`FlottaMozgat(f, 1, 1, b, 0)`), `WIN` |
| 906 | start: the human −100,000 (`FUN_0053fb10(EN, −1, −100000.0)`), `WIN` |
| 908 | start: every empire's designs 0..=40 reset to `Tervek0` with their race, `TervAutoDesign` + `TervUjraSzamol` from 1, `WIN` |
| 909 | start: `Story_VulkanKitores(AktPlanet)`, `WIN` |
| 910 | start: `st_AddDiplomacia(0x217b, 2, −1)`: special 0x217b for empire 2 and the undo entry `(3, 0x2217b, 857)` |
| 911 | start: `AutoAddship(AktFleet, 9999)` if it is the human's, `WIN` |
| 913 | start: `DestoryerATradereTutorialhoz()`, `WIN` |
| 917 | start: 9 ships for `AktFleet`, designs `st_GetTerv(EN, RndHosok[k].id, 3)` |
| 918 | start: for empires 1..=17 a fleet `st_fNewEx("Flotta", e, 0, 0, 1, 0)` with one ship of design 3 |

### 8.6 Story fleets
- **`st_fNewEx(name, owner, id, speed, mode, param)`** (exe 0x604bc0): a pending `last_addship` first gets its
  ship (`st_fAddShip(last, 3, 1)`), then `last_addship := id`. Position by mode (all f64, truncated; `a = (q ·
  3.141592) / 180`):
  - `< 0`: around planet `−mode`, `r = rnd(5, (int)(param·0.7), (int)(param·1.5))` (param 0 → 20), `q = rnd(5,
    1, 360)`;
  - 1: `p` = planet `param >> 16`, else (and from the 100th try on always) a random planet of the human's
    (`st_bSearch(−EN, 0)`); `r = rnd(5, (int)(R + w·0.7), (int)(R + w·1.5))` with `R` = `RadarHatotav2[owner
    (p)][p.+0x6e]` (f32) and `w = param & 0xffff` (0 → 20), `q = rnd(5, 1, 360)`; redrawn while `st_IsInRR(x,
    y, 3)` (the point is inside a radar range), at most 1000 times;
  - 2: planet `param`, else `st_bSearch(−owner, 0)`; its truncated coordinates;
  - 4: fleet `param`'s; 3: `350·(cos, sin)(a)`, `q = rnd(5, 0, 359)`; other modes (0, 0).
  The choice is kept in `last_fn_b/_r/_q/_x/_y` (0xF27524, 0xF273F4, 0xF27520, 0xF27528, 0xF2752C; mode 2/4
  zero `_q`, `_r`). Speed ≤ 1 (or NaN) is ×100 (f32). `FlottaNew(name, owner, id, x, y, speed)`, then
  `AutoAddship(f, id)`.
- **`AutoAddship(f, id)`** (0x5c1d80): `DisableAddShip` (0xF273F0) := 0; level 1..=9 = 1 + the thresholds 2, 4,
  6, 9, 12, 15, 19, 24 the human's planet count reaches; every `flotta.txt` record (`StoryFlottaData` 0xF27530,
  count 0xF2F870) for fleet `id` whose story is 0 or running gets `st_fAddShip(−f, design, counts[difficulty −
  1][level − 1])`, the design `−d` for `d < 1`, else `st_GetTerv(owner, d, 3)` (a story design, created on
  first use); any → `DisableAddShip := id`.
- **`st_fAddShip(id, design, n)`** (0x604410): nothing for `id == DisableAddShip`; else `last_addship := 0`,
  `f = id < 1 ? −id : st_fSearch(id)`; `f < 1` → `StoryError := 1`; a ground design (`+0x16 == 2 || +0x36`)
  → `TankNew(f, design, n)`, else `n` × `HajoNew(f, design)`; `StoryError := 0`. `st_fAddTank` (inlined) is
  the same with `TankNew` only and leaves `last_addship`.
- `st_CheckAddShip` at both ends of `Story_Check`: a pending `last_addship` gets `st_fAddShip(last, 3, 1)` and
  is cleared.
- **`st_AddKolonizalo(n)`** (0x5c9dc0): for each other live empire 1..=8 with a planet the human sees
  (`st_bSearch(−e, 1)`): `st_fNewEx("", e, 0, 0, 2, planet)` with `n` ships of `MiTervMutato[e][13]` (≤ 0 → 4).

### 8.7 Aborts
`st_bSearch(0, mode ≠ 1)` with no planet and the scripts' own `throw` raise `CStoryWinException(StoryLastChecked)`.
`Story_Check`'s catch handler for each story block (e.g. 0x5c79ac) calls `WIN(0, 1)` and resumes with the next
story. The port's story functions return `Result<(), Abort>`.

### 8.8 Not ported yet: the scripts
`st_Begin_include` (exe 0x5cec20, ELF 40 KB), `st_Rnd_include` (0x5f0e20, 125 KB), `st_Fable_include` (0x5e6ba0,
19 KB), `st_Elem_include` (0x5d4b70, 132 KB); `st_Line_include` (§8.9), `st_Multi_include` (§8.10) and
`st_BRnd_include` (§8.11) are ported. They hold the
stories 400's tables start (opening episodes 1..24, heroes 176..227, random stories 101..316,
118/142/199/242/837/842/865), the episodes the storylines start (801..864) and start 402..918. Their calls to
other systems go through the same `StoryWorld`/`StoryEvent` set (extended as needed: diplomacy specials and
relations, spy missions `st_KemEsely`, …). The tracer (`run_story.py --campaign`) patches them out; port them one
by one and drop each from the patch list (`--keep`). Also open: MP (`MULTIGET_Story`).

### 8.9 `st_Line_include` (exe 0x5eb6f0, ELF 0x7771e0): the storylines
Ported in `engine/crates/ig2-sim/src/storyline.rs` (`LineStories`, run after `ScenarioStories`), verified at
runtime (`traces/2026-10-16-line.md`). Eight stories, each a block run while the story is running; an abort
(`throw CStoryWinException(StoryLastChecked)`, raised when an empire the step needs is dead) is caught per block
(the ELF's handler 0x7786b3: `WIN(0, 1)`) and the next block runs. The exe inlines `st_Story`, `st_StoryStart`,
`StartStory`, `st_Counter`/`st_Message`/`st_Elevul`/`st_Letilt`/`st_StoryFinish`/`st_Signal`, `st_szStart`,
`st_GetValue`, `st_bSetBarat`, `st_SetBolygoTulaj` and `BolygoInfoRemove`'s caller code, and drops the ELF's two
`st_bSearch` asserts (0x167, 0x1ce). Notation: `T(x)` = `szStart(x, st_GetValue(x))` (a `values.txt` timer of
the same id), `S(x)` = `StartStory(x, false)` (a running `x` only becomes `StoryLastChecked`: a later `WIN(0)` or
start test in the same block then means `x`), `M(x)` = `NewMessage(x, 0)`, `C(x)` = `st_Counter(x)`, `F(x)` =
`st_StoryFinish(x)`, `A(b, r)` = `st_Message(b, r)` (flag 6, `StoryMessageBlokk`/`Resz` 0xF26574/0xF26578),
`P(b)` = `st_Elevul(b)` (flag 7, same block), `D(b)` = `st_Letilt(b, 0)` (flag 8), `need(e…)` = abort unless every
`BirodalomEl[e]`, `knows(e)` = `BirodalomIsmeri[EN][e]`. Counters are decimal (`C(5011)` = 0x1393).

| Story | Blocks, in order |
|---|---|
| 501 | `C(5011..5017)` → `M(0x2166..0x216c)` |
| 502 | `C(5021/5022/5023)` and `knows(1)` and empire 1 alive → `M(0x218e / 0x2192 / 0x2193)` |
| 530 | start and `Story_SkipFirst == 0`: `S(502)`, `S(801)`, `T(5301)`. `C(5301)`: counter 5021 := 300 days, `v(1921) := 1`, `e = near()` (below); none → counter 8300 := 100; else `v(5208) := e`, `v(5207) := 1`, `p = st_bSearch(−e, 0)` (stream 5) gets id 8080, and when `p`: buildings 0x10 and 0xe (`EpuletNewSearchLocation(p, k, true, 0, −1, −1)`), `NewMessage(0x189c, 200)`, race of `e`, param 1 = `e`'s name, 2 = `p`'s; then `T(5302)`. `F(801)`: `st_bSetBarat(8010, false)`, `(8011, false)` (planet `+0xbc := 0`; `StoryError` from the second). Daily with `v(5207)`: planet 8080's owner ≠ `v(5208)` → `KiszedUzenet(0x189c)`, `v(5207) := 0`. `A(0x189c, 2 or 3)`: `v(5207) := 0`, part 2 also `v(8089) := 1`; `S(808)`. `P(0x189c)`: `v(5207) := 0`, `M(0x189d)`, race and param 1 of `v(5208)`, `S(808)`. `C(5302)`: `S(824)`, `T(5303)`. `C(5303)`: counter 5022 := 300; `v(8001) == 2` → `M(0x1f67)`, else `M(0x1f65)`, `S(817)`, `T(5304)`. `D(0x1f67)`: no planet of the human's has info 0x1f41 or 0x1f42 → `LetiltReszlet(4)`, `(5)`. `A(0x1f67, 4)`: the human −500,000 (`st_AddMoney`), paid, `T(5304)`. `A(0x1f67, 5)`: paid, `rndlog(5, 0, 99) < 50` → counter 5308 := 30, else `T(5304)`. `C(5308)`: `M(0x1f66)`, `S(817)`, `T(5304)`. Paid: each planet of the human's with info 0x1f41 → `st_SetBolygoTulaj(p, 0, −1)` and the info removed; else (0x1f42) `+0x28 −= 9200`, below 300 → `+0x28 := 0` and `st_SetBolygoTulaj(p, 0, −1)`; info 0x1f42 removed. `A(0x1f67, 3 or 6)` or `P(0x1f67)`: `S(817)`, `T(5304)`, counter 5309 := 30. `C(5309)`: `M(0x13a7)`. `C(5304)`: `need(5, 1)`, `S(850)`, counter 5023 := 300, `T(5305)`. `C(5305)`: counter 5310 := 5. `C(5310)`: `need(5, 1)`; `knows(5)` and `knows(1)` → `S(861)`, else counter 5310 := 10. `F(861)`: counter 5306 := 60. `C(5306)`: `need(5, 1)`, `v(8611) < 1` → `S(864)` else `S(862)`. `F(862)`: counter 5307 := 90. `C(5307)`: `need(5, 1)`, `S(863)`. `F(863)`: `WIN(0, 1)`. `F(864)`: `WIN(0, 1)` |
| 550 | start and `Story_SkipFirst == 0`: `S(502)`, `S(802)`, `T(5501)`. `C(5501)`: counter 5021 := 300; `e = near()`; when `e`: not known → `st_FelfedezBirodalom(e)` (`BirodalomIsmeri[EN][e] := 1`, `FelfedezBirodalom(e, true)` 0x4c1d60), `M(0x19c9)`, race and param 1 of `e`, `v(8521) := e`; `T(5503)`. `A(0x19c9, 2)`: `S(852)`. `C(5503)`: counter 5022 := 300, `M(0x139d)`, `StoryInit8160()`, `S(816)`, `T(5505)`. `F(816)`: `M(0x159c)`, `S(813)`. `F(813)`: `v(1921) := 1`. `C(5505)`: `M(0x15a4)`, param 1 = planet 8140's name, `T(5506)`, counter 5023 := 300. `A(0x15a4, 4)` or `A(0x15a5, 4)`: the human +200,000, `S(814)`. `C(5506)`: `S(860)`, `T(5507)`. `C(5507)`: counter 5519 := 5. `C(5519)`: `need(5, 1)`; both known → `S(861)`, else counter **5310** := 10 (530's: a slip kept). `F(861)`: counter 5510 := 60. `C(5510)`: `v(8611) < 1` → `S(864)`, else `S(862)`, `T(5508)`. `C(5508)`: `S(863)`. `F(863)`: `WIN`. `F(864)`, or `Story_SkipFirst == 8` and the start flag: `WIN` |
| 560 | start and `Story_SkipFirst == 0`: `S(501)`, `S(802)`, counter 5011 := 300, `T(5609)`. `C(5609)`: counter 5012 := 300; `v(8020) == 1`: `StoryAdat+0x6c −= 25`, `v(8189) := 1`, `StoryInit8160()`, `M(0x15ea)`, param 1 = planet 8160's name, `S(816)`; else `StoryInit804()`, `M(0x15eb)`, `S(804)`; `T(5602)`. `F(816)`: `v(8181)` (read twice) 0 → `M(0x15ec)`, `+0x6c += 25`, `S(402)`; 1 → `M(0x15ed)`, `S(402)`; else `M(0x15f1)`, `+0x6c += 25`. `C(5602)`: `S(810)`, counter 5014 := 300, `T(5603)`. `C(5603)`: counter 5016 := 300, `M(0x163b)`, video `_s(6)`, `S(809)`, `T(5604)`. Then the episodes: `C(5604)` → `S(856)`; `F(856)` → `T(5605)`; `C(5605)` → `S(857)`; `F(857)` → `T(5606)`; `C(5606)` → `S(858)`; `F(858)` → `WIN` |
| 680 | start: `S(501)`, `S(854)`, counter 5011 := 300, `T(6801)`. `C(6801)`: `S(806)`, counter 5013 := 300, `T(6802)`. `C(6802)`: `S(820)`, counter 5015 := 300, `T(6803)`. `C(6803)`: counter 5016 := 300, `StoryInit8160()`, `M(0x1ab1)`, `S(816)`, `T(6805)`. `F(816)`: `S(807)`. The episodes as in 560 with the timers 6805, 6806, 6807 |
| 710 | start: `T(7101)`. `C(7101..7104)`: `S(841 / 830 / 831 / 832)`, `T(7102..7105)`. `F(832)`: `T(7106)`. `C(7106)`: `M(0x208a)`, `S(815)`. `C(7105)`: `M(0x1c52)`, `v(7250) := 1`, `S(817)`, `T(7107)`. `C(7107)`: `need(1)`, `M(0x2197)`, `S(834)`, `T(7108)`. `C(7108)`: `need(1)`; `e` = the second strongest enemy (below); none → `WIN`; else `M(0x209e)`, `v(8359) := e`, params 1 and 2 = `v(8359)`'s name (read each time), `S(835)`. `F(835)`: `T(7109)`. `C(7109)`: `need(1)`, `M(0x20a8)`, `S(836)`. `F(837)`: `WIN` |
| 720 | start: `S(839)`, `T(7201)`. `C(7201)`: `S(841)`, `T(7202)`. `C(7202)`: `S(840)`, `T(7203)`. `C(7203)`: `M(0x1c8e)`, `StoryInit8160()`, `S(816)`, `T(7204)`. `F(816)`: `M(0x1c98)`, `StoryInit804()`, `S(804)`. `st_Signal(804, 2)`: `M(0x1ca2)`, `S(843)`. `C(7204)`: `S(844)`, `T(7205)`. `C(7205)`: `M(0x2197)`, counter 7240 := 40. `C(7240)`: `need(1)`, the second strongest enemy as in 710 but `M(0x1c48)` and only param 2. `F(835)`: `T(7206)`. `C(7206)`: `need(1)`, `M(0x20a8)`, `S(836)`. `F(837)`: `WIN` |

The decimal story-variable ids: 1921 = 0x781, 5207/5208 = 0x1457/0x1458, 8001 = 0x1f41, 8020 = 0x1f54, 8089 =
0x1f99, 8181 = 0x1ff5, 8189 = 0x1ffd, 8359 = 0x20a7, 8521 = 0x2149, 8611 = 0x21a3, 7250 = 0x1c52; planets 8010/8011
= 0x1f4a/0x1f4b, 8040..8042 = 0x1f68..0x1f6a, 8080 = 0x1f90, 8140 = 0x1fcc, 8160 = 0x1fe0 (the campaign maps'
`pAzon`). Story 720 is the only block the exe tests with a real `st_Story` call (0x602f80).

Helpers:
- **`near()`** = `st_GetBirodalom808hoz` = `…852hoz` (exe 0x5ea400; the ELF's 852 calls 808): `s = st_bSearch(−EN,
  0)` (stream 5; 0 → returns 0, `nearest_bir_planet` 0xF2652C unchanged); over planets `q` = 1..`HanyBolygo` whose
  owner `o` is 2, 4, 6, 7 or 8 (non-zero, ≤ 8, not 1, 3, 5, not `EN`) with `Viszony[EN][o].+0x25 > 2` (the exe;
  the ELF calls `st_HaborubanAllE`): `d = dy² + dx²` (f32, from `s`'s **truncated** coordinates); `q` is taken
  when there is no pick yet or `d < best` (`comiss`: NaN never wins), and — once a known empire's planet was
  taken — `o` is known; `best := (int) d`; a known `o` sets the flag. `nearest_bir_planet := pick`; the result is
  the pick's owner, 0 without one.
- **The second strongest enemy** = `st_GetMasodikLegerosebbEllenseg(1)` = `GetIsmert…(1, GetLegerosebb(0, 0))`
  (exe 0x5ea6a0 / 0x5ea790): `GetLegerosebb(a, b)` = the live empire 1..=8 other than `EN`, `a`, `b` with the
  greatest `BirodalomEro` (0xBAEA78, f32; the first of equals); `GetIsmert…` the same over the known empires.
- **`StoryInit8160()`** (0x5c4d40): planet 8160 (searched, mode 1) → `st_SetBolygoTulaj(p, 10, −1)`, then
  `st_bUnhide(−p)`. **`StoryInit804()`** (0x5c49c0): planets 8040..8042, first `st_bUnhide` each, then
  `st_SetBolygoTulaj(·, 10, −1)` each. Neither checks the search (the ELF asserts): a missing id means slot 0.
- **`st_SetBolygoTulaj(p, owner, −1)`**: race `PlayerFaj[owner]`; free planet → `Generate_Planet(p, owner,
  race, 1, 0, 0, 0)` (0x59af50), else `BolygoElfoglal(p, owner)` and `+0x21 := race` when non-zero.
- **`st_bUnhide(x)`** (0x604040): `p = x > 0 ? st_bSearch(x, 0) : −x`; `p > 0`: `StoryError := 0` and every
  empire's view `+0xda..+0xfc` (i16, empires 1..=18) below 0 := 0; else `StoryError := 1`.
- `BolygoInfoIs(p, i)` (0x53bfe0) = one of the info slots 1..=5 (`+0x16a`, i16) is `i`; `BolygoInfoRemove(p, i,
  false)` (0x53c0b0) zeroes it (and drops `+0xba` for an info type with the flag at 0xBAA1F8). `SetParamStr`
  (0x509570), `KiszedUzenet` (0x506200), `LetiltReszlet` (0x5063d0) and `NewMessage` (0x508fe0) belong to the
  message screen; in the port they are events. `NewMessage` may slow the game down (`SetIdoLelassit`): the
  counter loop of `FRISSIT_Story` then runs the rest of the tick's counters at the new speed (seen in the traces).

### 8.10 `st_Multi_include` (exe 0x5ee4d0, ELF 0x79af20): the multiplayer game modes
Ported in `engine/crates/ig2-sim/src/multi.rs` ([`MultiStories`]). `Story_Check` runs it in SP and MP, after
the other scripts. Its stories are the MP game modes (the game set-up starts them; the campaign never does):
1001 and 1002 king of the hill, 1003 three planets, 1005 capture the flag; 1004 and 1006 are empty (the exe only
sets `StoryLastChecked`). Each story block catches an abort (§8.7: `st_bSearch(0, mode ≠ 1)` without a free
planet the human sees) with `WIN(0, 1)` and goes on with the next story; a story that wins goes on with the
next story too. `b(x)` = `st_bSearch(x, 0)`, `P[p]` the planet record (slot 0 when a search fails),
`PlayerNev[e]` exe 0xBAE588 + 11·e (the players' names), `trig(id)` = flag 4 with `GrWarBolygo == b(id)`
(`id == 0`: any ground battle), or flag 14 with special type 10 (a colonisation, `fleet-movement.md` §4.3), or
`st_Counter(id)`. Info lines are `st_AddGameInfo(…, owner −1)` (owner = id / 10: they belong to their story),
texts `AddSzoveg` (UI, the human's only).

- **`st_bShow(x, owner)`** (exe 0x603f20, ELF 0x7355d0): planet `p` = `−x` for `x ≤ 0`, else `b(x)`; `owner
  == −1` → `x / 10`. `p < 1`: `StoryError := 1`. Else `StoryError := 0`, `st_bUnhide(−p)` (every empire's
  negative view of `p` becomes 0, exe 0x604040); a non-zero owner `o ≠ EN` the human doesn't know is met
  (`FelfedezBirodalom(o, 1)`, `BirodalomIsmeri[EN][o] := 1`); the human's view `+0xd8[EN] < 1` becomes 1; for
  `x > 0` with an info string `−x` (`InfoString_GetIndex` 0x53ec40: its index in the table at exe 0xBAA1F0,
  12-byte records, count 0xBAD0D0; index 0 counts as none): `BolygoInfoAdd(p, −x, true)` (0x53beb0); `owner != 0`: undo
  `(2, p << 16 | x & 0xffff, owner)`.
- **`st_GetBirodalomSulypont(&x, &y, e)`** (exe 0x5ea570): the f32 mean of `+0x18`/`+0x1c` over the planets
  **0**..=`HanyBolygo` owned by `e` that the human sees (`+0xd8[EN] ≥ 0`), summed in order; none → (0, 0).
  **`st_bGetNearest(x, y, owner)`** (exe 0x603980): over the planets 1..=n the human sees with owner `owner`
  (−1: any live owner) the first, or one with `(f32)(dy² + dx²) < (f32) best` (`best` = the last pick's
  truncated value); 0 when there is none.

| Story | What it does |
|---|---|
| 1001 | start: planet 10011 (`st_bSearch(10011, 1)`, else `st_bSearch(0, 0)` gets the id), info 10011 (the planet), `st_bShow(10011, −1)`, `v(10011) := 0`. `trig(10011)`: if `P[b(10011)].owner != v(10011) = o`: remove info `o == 0 ? 10011 : o == EN ? 10012 : 10013`, `v(10011) := owner`, `v(10012) := v(10010)` (0 → 150), counter 10011 := 1 day, info `owner == EN ? 10012 : 10013` (planet, `v(10012)`). Counter 10011 (after the block above, in the same check): `o = v(10011)`, `n = v(10012) − 1`, `v(10012) := n`; `n ≥ 1`: info `o == EN ? 10012 : 10013` replaced (planet, `n`), counter 10011 := 1 day; else text (`o == EN`: "AMAZING! YOU ARE THE WINNER!", else "`PlayerNev[o]` is the winner!"), `WIN` |
| 1002 | as 1001 with planet/variable/counter 10021 and infos 10021/10022/10023, but one count per owner: the start also sets `v(10041 + e) := v(10020)` (0 → 600) for `e` = 1..=8, an owner change doesn't reset it, and the count shown and decremented is `v(10041 + o)` (`10041 + 0` for an owner-less planet: a missing variable reads 0, so that tick wins) |
| 1003 | start: unless planet 10031 exists, 51 tries of three `st_bSearch(0, 2)` (**stream 2**; it aborts when there's no free planet the human sees); for three different planets the sum of the truncated distances `(i32)(f32) sqrt((f64)(f32)(dx² + dy²))` (a–b, b–c, c–a) replaces the best when strictly larger (the best starts as planets 1, 1, 1 with sum 0); the best get ids 10031, 10032, 10033. `st_bShow` all three (−1), info 10031 (no text), counters 10031 := 10, 10034 := 50 days. Counter 10034: message 0x2347 (`NewMessage(0x2347, 300)`, parameter 1 `PlayerNev[EN]`; the ELF also disables answer 2 below 20,000 money, the exe dropped that). Answer 2 (flag 6, block 0x2347, part 2): `Multi_Send(0x66, EN, 4, &10031)` (it comes back as `Story_Special(15, sender, 10031)` through `MULTIGET_Story`); part 4: counter 10034 := 50 days; promotion (flag 7, block 0x2347): counter 10034 := 100 days. Special 15 with `SpecialData2 == 10031`: `FUN_0053fb10(d1 > 0 ? d1 : EN, −1, −20000.0)`; `d1 == EN`: counter 10033 := 1 day. Counter 10033 (the bought radar, re-armed daily): every live fleet of an owner `≠ EN`, `≤ 8` loses `+0x52` bit 8 (always visible); for `k` = 0..=2 and planet `p = b(10031 + k)` owned by the human: over those fleets the nearest `d = (i32)(f32) sqrt((f64)(f32)(dx² + dy²))` (planet − fleet; below 1000, the first of equals; its owner is kept across `k`), fleets with `d < 80` get bit 8; info `10032 + k` removed and, for `d < 50` / `< 80` / `< 110`, added: `PlayerNev[owner]`, "RED" / "YELLOW" / "GREEN", the planet, colour 6 / 3 / 7. `trig(0)`: counter 10031 := 10 days; the three planets' owners equal and non-zero → text (the human: "YOU ARE THE WINNER!", else "… is the winner!"), `WIN` |
| 1005 | start: unless planet 10051 exists: the nearest free planet (`st_bGetNearest(trunc x, trunc y, 0)`) to empire 1's centre (`st_GetBirodalomSulypont(1)`) gets id 10051 (when non-zero) and the name "Base 1" (written into planet slot 0 when there's none), the nearest to empire 2's centre id 10052 and "Base 2" (the same planet → `st_bSearch(0, 0)` instead). Unless `v(10051) != 0`: the "Flag Carrier" design (below) for empire 1 (`TervNew(1, 0)`, the record copied over it, `TervUjraSzamol(1, d, 0, 1)`) → `v(10051)`, the same record for empire 2 → `v(10052)`. `st_bShow` 10051, 10052 (−1), `v(10055) := v(10056) := 0`, `v(10059)` 0 → 250, `v(10060)` 0 → 50, info 10051 (""). Special 10 on planet `d2 ∈ {b(10051), b(10052)}`: `d1 == EN` → "You cannot colonize `<planet>`!"; the planet goes back to owner 0 (`st_SetBolygoTulaj(d2, 0, −1)`: free → `FUN_0059af50(p, 0, PlayerFaj[0], 1, 0)`, else `BolygoElfoglal(p, 0)` and `+0x21 := PlayerFaj[0]` when non-zero). A fleet arrives at a planet (flag 0, type 1): `which` = 1 / 2 for `b(10051)` / `b(10052)`, `o` = the fleet's owner. **Take**: `v(10054) == 0`, `which == 1`, `o == 2` → `x = 2`, else `v(10053) == 0`, `which == 2`, `o == 1` → `x = 1`: texts (`EN == x`: "You have the flag of `PlayerNev[which]`!", `EN == which`: "`PlayerNev[x]` has your flag!"), `v(10052 + x) := 1`, `HajoNew(fleet, v(10050 + x))`. Otherwise, with `which != 0`, **score** when the fleet holds a ship (the last of `Hajok` 1..=n in that fleet) of design `[0, v(10051), v(10052)][o]` (owner > 2: none) and (`v(10054) == 0`, `which == 1`, `o == 1`) or (`v(10053) == 0`, `which == 2`, `o == 2`), `x = which`: texts (`EN == x`: "TOUCHDOWN!!!", `EN == 3 − x`: "`PlayerNev[x]` has captured your flag!"), `v(10052 + x) := 0`, `HajoDelete(ship)`, `v(10054 + x) := v(10056 + x)·v(10060) + v(10054 + x) + v(10059)` (read in that order), `v(10056 + x) += 1`. A space battle ends (flag 3) with empire 1 or 2 among the participants (`SpwFlottak[0]`, `[1]`, and `[2]` when there are more than two; owner `+0x18`): `a` / `b` = 1 or 2 when participant 0 or 1 is empire 1 / 2; carrier 1 / 2 = a ship of design `v(10051)` / `v(10052)` in a fleet of empire 1 / 2; `a != 0`: `v(10055) += CTF_value[3 − a]`, `b != 0`: `v(10056) += CTF_value[3 − b]` (`CTF_value` exe 0xF261EC: per side the attack lost in the last auto-resolved battle, `space-combat.md` §5); `v(10053) != 0` without carrier 1: `v(10053) := 0`, texts (`EN == 1`: "You have lost the flag!", `EN == 2`: "Your flag has been returned!"); the same for `v(10054)` and carrier 2 with the empires swapped. Any of these parts: info 10051 replaced by the score `"%s: %4d/%d %c   %s: %4d/%d %c"` (`PlayerNev[1]`, `v(10055)`, `v(10057)`, `v(10053) ? 'F' : ' '`, `PlayerNev[2]`, `v(10056)`, `v(10058)`, `v(10054) ? 'F' : ' '`; the exe reads 10054, 10058, 10056, 10053, 10057, 10055, so `StoryError` is 10055's) |

The "Flag Carrier" design record (0x84 bytes, the `.scn` `TERV` layout): name "Flag Carrier" (+0x00..+0x14
zero-filled), `+0x16` i16 1, `+0x18` 0, `+0x19` 1, `+0x1a` 1, `+0x1c` 0, `+0x20` 250,000, `+0x24` 3,500, `+0x28`
0, `+0x2c` 0, `+0x30` 0, `+0x34` 0 (4 bytes), `+0x38` 0 (2 bytes), `+0x3c` f32 1.0, `+0x40` i16 `EN` (the race
field gets the player number), `+0x44` 13, `+0x48` 13, `+0x4c..+0x83` 0. The exe leaves `+0x15`, `+0x1b`,
`+0x29..+0x2b`, `+0x3a`, `+0x3b`, `+0x42`, `+0x43` as its stack held them (the port writes 0). `TervNew(e, 0)`
(exe 0x5422f0) takes the first slot 1..=n with `+0x16 < 0`, else appends one.

Exe vs ELF: the exe has no `st_oldLetilt(0x2347)` money check in 1003; it inlines the variable, planet-id and
counter accessors (the same results; the reads that set `StoryError` follow the exe's order above) and
`st_RemoveGameInfo`; its distances are `sqrt` in f64 of an f32 sum, truncated through f32. Quirks kept: 1002
counts `v(10041)` down for an owner-less hill; 1003's best starts as planets (1, 1, 1) (three ids on planet 1
when no try finds three different planets); 1005 reads `SpwFlottak[1]` even after a one-sided battle (the port
reads owner 0 past the list) and the arriving fleet's design list past index 2 for an owner > 2 (stack bytes;
the port finds no carrier).

### 8.11 `st_BRnd_include` (exe 0x5c9f20, ELF 0x768d50): random stories, part 1
Ported in `engine/crates/ig2-sim/src/brnd.rs` ([`BRndStories`], run first: `Story_Check` calls `BRnd` before
`Fable`, `Scen`, `Line`, `Elem`), verified at runtime (`traces/2026-10-16-brnd.md`). Stories 220..=239 (0xdc..0xef,
the random stories story 400 draws) and their follow-ups 2301 (0x8fd) and 2311 (0x907); 225 is empty (the exe
only sets `StoryLastChecked`). Each story block catches an abort with `WIN(0, 1)` and goes on (§8.7). Ids are
decimal: story `s`'s messages, variables and counters are `10·s + k` (2200 = 0x898). `b(x)`, `v(x)`, `C(x)`, `A(b,
r)` (answered), `P(b)` (promoted, flag 7), `D(b)` (`st_Letilt(b, 0)`, flag 8) as in §8.9; `M(id, arg)` =
`NewMessage(id, arg)`; "the human's" = owned by `EN`.

Before the stories, in every check: special 0x19 (flag 14) with `d = Tervek[owner of fleet d1][d2]` and `220 ≤
d.+0x80 / 10 < 230` → `TervDelete(EN, d2, d1, true)` (exe 0x541d50).

**The mercenaries** 220, 221, 222, 223, 224, 226, 227, 228, 229: an offer to hire a fleet.

| Story | `M` | price | start: counter `X` := | offer when (else `X` := 500 days) | battle bonus (`StoryAdat`) |
|---|---|---|---|---|---|
| 220 | 2200 | 80,000 | 2209: rnd(500, 1000) | tech 7 has status 5 and `S/3 > 2020` | `+0x84 += 20` |
| 221 | 2210 | 200,000 | 2219: rnd(500, 1000) | tech 8, `S/3 > 2520` | `+0x7c += 10`, `+0x90 += 22` |
| 222 | 2220 | — | 2229: rnd(500, 1000) | tech 4, `S/3 > 1020` | `+0x90 += 8` |
| 223 | 2230 | 27,000 | 2239: rnd(2000, 3000) | `S/3 ≥ 721` (else `X` := 50 days) | `+0x84 += 5` |
| 224 | 2240 | 230,000 | 2249: rnd(500, 1000) | tech 8, `S/3 > 1520` | `+0x84 += 12`, `+0x90 += 4` |
| 226 | 2260 | — | 2269: rnd(2500, 3500) | `S/2 ≥ 521` (else 50 days) | `+0x84 += 10`, `+0x7c += 10` |
| 227 | 2270 | 10,000 | 2279: rnd(1500, 2000) | always | `+0x90 += 8` |
| 228 | 2280 | 24,000 | 2289: rnd(500, 1000) | tech 6, `S/3 > 250` | `+0x7c += 12` |
| 229 | 2290 | 250,000 | 2299: rnd(500, 1000) | tech 9, `S/2 > 3520` | `+0x84 += 10`, `+0x7c += 16` |

`S` = `st_GetOsszFlottaEro()` (exe 0x5eaaf0): the sum of the human's design `+0x2c` over the ships 1..=`OsszHanyHajo`
with a design (`≠ 0x7fff`) in a fleet of the human's; "tech `t`" = `Talalmany[t].+0x45+EN == 5` (tested first).
Blocks in order: start → `X` := the draw (stream 5) days, `v(X) := 0`. `C(X)` → `M(M, 160)` or the re-arm. Then,
for the priced ones: `A(M, 3)` or `A(M+1, 3)` → counter `M+1` := 360 days (227: 60); `C(M+1)` → `M(M+1, 160)`;
`A(M, 2)` or `A(M+1, 2)` → **hire**: `st_AddMoney(−price)` (`FUN_0053fb10(EN, −1, −price)`), `f =
st_fNewEx(_s(5, M), EN, M, 0, 1, 0)` (§8.6), `FlottaMozgat(f, 1, 1, last_fn_b, 0)`, `st_RemoveFlottaAzonosito(M)` (the
first live fleet with id `M` gets `+0x16 := 0`), `v(X) := 1`; `A(M, 4)`, `P(M)`, `A(M+1, 4)` or `P(M+1)` → `WIN`.
222 and 226 have one message: `A(M, 2)` hires (no money), `A(M, 3)` or `P(M)` → `WIN`. 227's last block brings the
enemy instead (`st_fNewEx(_s(5, M), 10, M, 0, 1, 0)`, no `WIN`), and then `C(2272)` or that: `f = st_fSearch(M)`:
none → `WIN`; else `n = st_fGetNearest((int) f.x, (int) f.y, EN)`: none → `st_fEliminate(M)` and `WIN`, else
`FlottaMozgat(f, 2, 2, n, 0)` (it chases the human's nearest fleet); counter 2272 := 4 days (after a `WIN` too).
Last: a space battle starts (flag 1) while `v(X) != 0`: `f = st_hSearch(EN, M+1)`: 0 → `WIN`, else
`st_SpWarStartID(f)` → the bonus.
- `st_hSearch(e, key)` (exe 0x6052a0): `key > 40`: the design `d` of 40..=`OsszHanyTerv[e]` with `+0x80 == key`
  (none → 0), else `d = key`; the fleet (`Hajok[k].+0`) of the first ship 1..=`OsszHanyHajo` of design `d` in a
  fleet of `e`'s, else 0.
- `st_SpWarStartID(f)` (0x602e30): flag 1 and (`f == 0` or one of `spFlottak` 0xBAA1C6 / 0xBAA1CC is `f`).
- `st_fGetNearest(x, y, owner)` (0x604970): over the fleets 1..=`OsszHanyFlotta` of `owner` (0: any live one) the
  first, or one with `(y − f.y)² + (x − f.x)² < (f32) best` (f32; `best` the last pick's truncated value); 0 if none.
- `st_fEliminate(id)` (0x604750): `f = id < 0 ? −id : st_fSearch(id)`; `f < 1` → `StoryError := 1`. A fleet already
  marked (`+0x18` bit 1) with ships (`+0x40 != 0`) stays; else the mark is set, `x = rnd(0, 1)·600 − 300`, `y =
  rnd(−300, 300)`, `rnd(0, 1) != 0` swaps them; `(cx, cy)` = the mean of the human's visible planets 1..=n
  (truncating: `acc = (int)((f32) acc + p.x)`, then `acc / n`; none → 0); `((f32) cx − f.x < 50) != (x ≥ 0)` → `x :=
  −x`, the same for `y`; `FlottaMozgat(f, 1, 0, x, y)` (it flies off); `StoryError := 0`.

**230 (a fleet in quarantine):** start: `e = GetRndBirodalom(0, 0, 0)` (below), `v(2302) := e`, `M(2300, 70)`,
parameter 1 = `e`'s name, `v(2309) := 0`. `A(2300, 2)`: game info 2301 (`e`'s name); two designs, each `TervNew(EN,
0)` + the record below (names `_s(5, 2300)`, `_s(5, 2301)`) with `+0x80 := 2300` / `2301` (the second also `+0x18 :=
0`), `TervUjraSzamol(EN, d, 0, 1)`, then `+0x20 ×= 5`; the first's slot → `v(2301)` (before its recompute);
counters 2300 := 3 and 2302 := 1080 days; `ScrollUzenet(_s(5, 2302), 1, true, false)`. `A(2300, 3)` or `P(2300)` →
`WIN`. `C(2300)` (re-armed 3 days): the first ship 1..=`OsszHanyHajo` of design `v(2301)` in a fleet `f` of the
human's with ships (`+0x40 != 0`) heading for a planet (`+0x42 == 1`, `p = (int) f.+0x44`) without an epidemic
(`p.+0xb8 < 1`) owned by `v(2302)` within `st_tav < 20` (below) → `M(2302, 0)`, the ship becomes `st_GetTerv(EN,
2301, 3)`, `p.+0x28 /= 2`, `v(2309) += 1` (3 → counter 2302 := 1 day). `C(2302)`: `M(2301, 0)` if a ship of design
`v(2301)` in a fleet of the human's is left, else `v(2309) == 0` → `M(2304, 0)`, else `M(2301, 0)`; every such ship
becomes `st_GetTerv(EN, 2301, 3)`, `TervDelete(EN, v(2301), 0, true)`, `StartStory(2301)`, `WIN` (a running 2301
becomes `StoryLastChecked` first: the `WIN` then ends it instead of 230, §8.9's `S`).
**2301:** start: counter 23011 := 10 days. `C(23011)`: re-armed 10 days; `st_hSearch(EN, 2301) == 0` →
`TervDelete(EN, st_GetTerv(EN, 2301, 3), 0, true)`, `WIN`.
- `GetRndBirodalom(a, b, c)` (exe 0x5c27c0): `e = rnd(1, 8)`; up to 10 tries while `e` is `a`, `b`, `c`, `EN`, dead
  or unknown to the human: `e := e % 8 + 1` (the 10th failure keeps `e`); then the same without "unknown"; both
  failing aborts (§8.7).
- `st_tav(x1, y1, x2, y2)` (inlined) = `(int)(f32) sqrt((f64)(f32)((x1 − x2)² + (y1 − y2)²))` of truncated
  coordinates.
- The design record (0x84 bytes): `+0x16` i16 1, `+0x18` 1, `+0x19` 1, `+0x1a` 1, `+0x20` 150,000, `+0x24` 300,
  `+0x3c` f32 0.1, `+0x40` i16 `EN`, `+0x44` / `+0x48` 12 (231: 13), the rest 0 except the bytes the exe leaves as its
  stack held them (`+0x15`, `+0x1b`, `+0x29..+0x2b`, `+0x3a`, `+0x3b`, `+0x42`, `+0x43`, as in §8.10); the name is
  `strcpy`'d over `+0x00`.

**231 (a planet to blow up):** start: `e = GetRndBirodalom(0, 0, 0)`; unknown to the human → `WIN(0, 0)`; else `p =
st_bSearch(−e, 0)` (none → abort) gets id 2310, `st_bShow(2310, −1)` (§8.10), `M(2310, 0)`, parameter 1 = `p`.
`A(2310, 3)`: a design (13/13, `+0x80 := 2310`, not ×5) → `v(2310)`, recomputed, counter 2310 := 3 days, game info
2311 (planet `b(2310)`). `A(2310, 4)` or `P(2310)` → `WIN`. Daily: `p = b(2310)` the human's → `WIN`; else `d =
v(2310)`, counter 2310 := 3 days, the first ship 1..=n of design `d` in a fleet of the human's within `st_tav(fleet,
p) < 20` → `M(2312, 0)`, `p` gets id 23111, `StartStory(2311)`, `FlottaDelete(fleet)`, `WIN`. After either `WIN`:
`TervDelete(EN, st_GetTerv(EN, 2310, 3), 0, true)`.
**2311:** start: counter 23111 := 2 days. `C(23111)`: video `_s(6, 2312)`, `st_BolygoDestroy(b(23111))`, `WIN`.

**232:** start: over planets 1..`HanyBolygo` − 1: (the human has more than 50 planets or the planet is free) and the
human's view `+0xd8[EN] == 0` and `rnd(0, 2) == 0` (drawn only then) → view := 1; any → `M(2320, 0)`; `WIN`.

**233 (strikes):** start: `st_bGet3BestProd()` (below) none → `WIN(0, 0)`; else `M(2330, 100)`, the three planets
get ids 2330..=2332 and are parameters 1..=3. `A(2330, 2)`: each gets info 2331 (`BolygoInfoAdd(p, 2331, true)`)
and morale −20 (`st_BolygoMoralValt(p, −20)`: `+0x174 += 200`, `+0x178 := 6`), counter 2331 := 1080 days.
`A(2330, 3)` or `P(2330)` → `WIN`. `C(2331)`: `M(2331, 0)`, info 2331 removed, morale +20 (`+0x174 −= 200`), `WIN`.
- `st_bGet3BestProd()` (exe 0x603350): the human has fewer than 3 planets → none. Else over planets 1..`HanyBolygo` −
  1 of the human's that the human sees: `v = +0x70 + +0x74 + 1`; slot `k` = 0, then 1 if `val[1] == 0` or (`val[0]
  != 0` and `val[1] < val[0]`), then 2 if `val[2] == 0` or (`val[k] != 0` and `val[2] < val[k]`); `val[k] == 0` or
  `val[k] < v` → `val[k] := v`, `best[k] := planet`. `val` is local, **`best` static** (exe 0xF432B8): slots not
  refilled keep the planets of an earlier call. Any `best[k] == 0` → none, else `best`.

**234 / 235 (a tax):** start: the human has fewer than 5 planets → `WIN(0, 0)`; the other one (235 / 234) in
`HistoryVodor` → `WIN`; else `M(2340 / 2350, 0)` and `SetParamInt(1..=3, (int)((f64) T · q))`, `q` = 0.9, 0.8, 0.7
(235: 0.85, 0.75, 0.65), `T` = the sum of `+0x28` over the human's planets 1..=`HanyBolygo`. `A(M, r)`, `r` = 2,
3, 4: `q` = the `(r − 1)`th factor (f32); every planet 1..`HanyBolygo` − 1 of the human's: `+0x28 := (int)((f32)
+0x28 · q)` (235 first adds `(1 − q)·(f32) +0x28` to `paid`, f32, truncating; then `v(2351) := paid`); part 2 →
`WIN`, 3 → counter `M` := 50 days, 4 → counter `M+1` := 50 days. 234: `C(2340)` → `M(2341, 0)`, global morale +40
(`st_GlobalMoralValt`: `GlobalisMoralCSokkentoBarkacs` 0xBAEAC4 `−= 400`, 0xBAEB14 := 6), `WIN`; `C(2341)` →
`M(2342, 0)`, morale +70, `t = st_GetRandomTalalmany(−1, 0)`: none → parameter 1 `_s(5, 2341)`, else
`st_SetTalalmanyAllapot(t, 5, −1)` and parameter 1 = the tech's name; `WIN`. 235: `C(2350)` or `C(2351)`: `paid =
v(2351)`, `n = (C(2350) ? 3 : 6) + paid / 140,000`; steps `L` = 7, 15, 30, 50, 80, 140 (then 280): the first `L` with
`paid ≤ 1000·L` (`lo` the step before, 0 at first), `big = clamp((paid − 1000·lo) / (L − lo) · n / 1000, 0, n)` (C
division), `M(2351, 0)`, `f = st_fNewEx(_s(5, 2351), EN, 0, 0, 1, 0)`, `FlottaMozgat(f, 1, 1, last_fn_b, 0)`,
`st_fAddShip(−f, st_GetTerv(EN, 2350 + k, 3), n − big)`, `st_fAddShip(−f, st_GetTerv(EN, 2351 + k, 3), big)`,
`st_fAddTank(−f, st_GetTerv(EN, [901, 901, 902, 902, 903, 903, 903][k], 3), n)` with `k` the step's index, `WIN`.
- `st_GetRandomTalalmany(e, group)` (exe 0x5eab60): for groups `g` = 0..=10 the first tech `t = 16g + j` (`j` =
  1..=16) the empire can be given (`st_TalalmanyAdhato`: status 1..=3 and each prerequisite `+0x2d..+0x2f` 0 or at
  status 5): `rnd(0, n)` (`n` = such groups before), 0 → the pick (`group != 0`: that group only).
- `st_SetTalalmanyAllapot(t, 5, e)` (exe 0x602920): the tech is given (`TalalmanyKifejlesztve`, research time
  `(int)((f32) +0x38 × FajSpeckoKutatasIdoSzorzo[race])`, the recompute 0x5245c0, `ResearchAktFejlesztett := 0` if it
  was being researched, negative prerequisite bytes := 0) and **`(e, t)` is added to `TalalmanyVodor`**: the check's
  end runs a special with `SpecialType = e`, `SpecialData1 = t` (§4 step 7).

**236 (a refugee planet):** start: fewer than 10 planets → `WIN(0, 0)`; else `M(2360, 100)`, the human +12,000, `p =
st_bSearch(−EN, 0)` gets id 2360, parameter 1. `A(2360, 2)` or `P(2360)` → `WIN`. `A(2360, 3)` → counter 2360 := 50
days. `D(2360)` and `b(2360)` not the human's → `LetiltReszlet(4)`. `A(2360, 4)`: `b(2360)` the human's → morale +30
(`+0x174 −= 300`); `WIN`. `C(2360)` → `M(2361, 100)`. `A(2361, 2)`: `p = st_GetUnknownPlanet()` (below) gets id 2360,
`st_SetBolygoTulaj(p, EN, 10)` (race 10), `+0x28 := 10240`, `AutoAddBuilding(2360)`, `AutoAddtank(2360)`,
`st_fNewEx(_s(5, 2360), EN, 2360, 0, −p, 20)`, `st_fAddShip(2360, 3, 3)`, `(2360, 2, 5)`, `st_RemoveFlottaAzonosito(2360)`,
`st_fNewEx(_s(5, 2361), EN, 2361, 0, −p, 20)`, `st_RemoveFlottaAzonosito(2361)`, `WIN`. `A(2361, 3)` → counter 2361 :=
50; `A(2361, 4)` → counter 2362 := 6; `P(2361)` → `WIN`. `C(2361)`: `M(2362, 0)`, morale +10, `WIN`. `C(2362)`:
`M(2363, 0)`; five times: a weighted pick over planets 1..`HanyBolygo` − 1 with the human's view 0
(`RndInitValasztas`, `RndValasztas(5, 1)`: `rnd(5, 1, n) ≤ 1` for the `n`-th); none → stop; else its view := 1. `WIN`.
- `st_GetUnknownPlanet()` (exe 0x5eaea0): up to 101 draws of `st_bSearch(0, 0)` (aborting without a free visible
  planet) until one has the human's view 0 (**the result is thrown away**), then `st_bGetNearestUnknown()`
  (0x603a60): of the planets with ids 11001..=11015 (`st_bSearch(id, 1)`) the human doesn't know (`+0xd8[EN] < 0`)
  the nearest (f32 `(cy − y)² + (cx − x)²` from `st_GetBirodalomSulypont(EN)`, §8.10, against the last pick's
  truncated value); `st_bUnhide(−p)`; none → abort.
- `AutoAddBuilding(id)` (exe 0x5c1c80): `p = b(id)`; for each `StoryBuildingData` record (exe 0xF22740, 16 bytes
  `{story, planet id, kind, level}`, count 0xF23A00) of `id` whose story is 0 or running:
  `EpuletNewSearchLocation(p, kind, true, level, −1, −1)` and `sum += EpuletekLeiro[kind].+0x10` (`sum` from 3000);
  then `p.+0x28 := sum + rnd(−200, 1300)`. `AutoAddtank(id)` (0x5c1ef0): for each `flotta.txt` record of fleet `−id`
  (story 0 or running): `TankNew(−p, d, counts[Nehezseg − 1][1])`, `d` = `−design` for `design < 1`, else
  `st_GetTerv(p's owner, design, 1)`.

**237:** start: `M(2370, 0)`, counter 2370 := 200 days, `Story_Minden_Flotta_Latszik` (exe 0xF265D4) := 1, game info
2371 (no text). `C(2370)`: the switch := 0, `M(2371, 0)`, `WIN`.
**238:** start: `M(2380, 60)`. `A(2380, 2)`: `StoryAdat+0x38 := 1`, counter 2380 := 720 days. `A(2380, 3)` or `P(2380)`
→ `WIN`. `C(2380)`: `M(2381, 0)`, `+0x38 := 0`, `WIN`.
**239 (a wormhole):** start: no wormhole 1..=`OsszHanyWormhole` (exe 0xBB60F8, 0x1c bytes, count 0xBB61A0) has both
ends shown (`+8` and `+0x14`) → `WIN(0, 0)`; else `p = st_bSearch(−EN, 0)` (none → abort), `v(2391) := 0`,
`M(2390, 80)`, `p` gets id 2390, parameter 1. `A(2390, 2)`: `v(2391) := 1`, `st_fNewEx(_s(5, 2390), EN, 2390, 0, 2,
b(2390))`, `st_fAddShip(2390, 0x1c, 1)`, `st_RemoveFlottaAzonosito(2390)`. `A(2390, 3)` or `P(2390)` → `WIN`. Daily
with `v(2391)` and `st_hSearch(EN, 0x1c) == 0` → `M(2391, 0)`, `WIN`. Special 0x11: `M(2392, 0)`, story 902's wormhole
(§8.5: from fleet `d1` to `b(2390)` + `(int)(r·(cos, sin)(a))`, `r = rnd(20, 28)`, `a = rnd(1, 360)`); when
`AddWormhole` returns `k ≠ 0` both ends are shown (`+8`, `+0x14` := 1) and their fleets (`+0xa`, `+0x16`) always
visible (`+0x52` bit 8); `Wormhole3DSettings()`, `WIN`.

Exe vs ELF: the exe inlines `st_Story`/`st_StoryStart`, `st_Counter`/`st_Message`/`st_Elevul`/`st_Letilt`,
`st_szStart` (with an empty assert in 2301), `StartStory`, `st_RemoveFlottaAzonosito`, `st_AddMoney`,
`st_bAddNewAzonosito`, `st_vGet`, `st_tav`, `st_BolygoMoralValt`, `st_GlobalMoralValt`, `st_SetBolygoTulaj`,
`st_TalalmanyAdhato` and `st_fAddTank`, with the same results. Precision (asm): `st_tav` through f32, 234/235's
parameters f64, the scaling and 235's sum f32. Quirks kept: `st_GetUnknownPlanet` throws its draws away; `best`
survives calls; 230's `StartStory(2301)` before `WIN(0)` ends a running 2301 instead of 230.
