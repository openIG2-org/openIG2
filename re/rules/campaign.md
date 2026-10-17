# Campaign scripting: the story engine and the scenario stories

Status: **engine core, the scenario stories, `InitStory`, the inline campaign stories and the storylines
(`st_Line_include`) verified at runtime** (`traces/2026-10-16-story.md`, `traces/2026-10-16-campaign.md`,
`traces/2026-10-16-line.md`), ported in `engine/crates/ig2-sim/src/story.rs`, `campaign.rs` and `storyline.rs`
(`tests/story_trace.rs`, `tests/campaign_trace.rs`); the MP game modes (`st_Multi_include`, §8.10,
`traces/2026-10-16-multi.md`) in `multi.rs`, the random stories 220..239 (`st_BRnd_include`, §8.11,
`traces/2026-10-16-brnd.md`) in `brnd.rs`, the opening episodes (`st_Begin_include`, §8.12,
`traces/2026-10-16-begin.md`) in `begin.rs`, 36 of the 62 stories of `st_Rnd_include` (§8.13,
`traces/2026-10-16-rnd.md`, `traces/2026-10-16-rnd2a.md`) in `rnd.rs` and `rnd/`, the storyline episodes of `st_Elem_include` part 1 (§8.14,
`traces/2026-10-16-elem.md`) in `elem.rs`, the random fables (`st_Fable_include`, §8.15,
`traces/2026-10-16-fable.md`) in `fable.rs`. The campaign's other scripts are mapped (§8.8, §8.13, §8.14) but not
ported yet.

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
| `WIN(id, history)` | 0x5c9770 | 0x730aa0 | a story ends (§2.3); `LOST()` = `WIN(0, 1)`, `DIE()` = `BirodalomMeghal(EN)` (0x53b630, `diplomacy.md` §12) |
| `MEGNYERES(1)` | 0x53f6b0 | 0x61eb50 | the human wins the game (an event here) |
| `NEWGAME_Story` | 0x5c3040 | 0x7308a0 | reset (§2.6), then reads the story data files (`story-data.md`) |
| `InitStory(mode)` | 0x5c2940 | 0x72ec70 | the campaign set-up (§8.2) |
| `Story_Generate` | 0x5c8d30 | 0x73f0a0 | clears the event flags, `StoryInterregnumIdo` 0xF22738 := 0, `StoryTerroristaCselValoszinusege` 0xF23A18 := 30 |
| `st_Scen_include` | **0x601580** | 0x774c00 | the scenario stories 360..375 (§6) |
| `st_Line_include` | **0x5eb6f0** | 0x7771e0 | the storylines 501..720 (§8.9) |
| `st_BRnd_include` | **0x5c9f20** | 0x768d50 | the random stories 220..239, 2301, 2311 (§8.11) |
| `st_Begin_include` | **0x5cec20** | 0x7407b0 | the opening episodes 1..24 (§8.12) |
| `st_Rnd_include` | **0x5f0e20** | 0x74a2c0 | the random stories 101..199, 242, 302 (§8.13; 36 ported) |
| `st_Elem_include` | **0x5d4b70** | 0x77ac00 | the storyline episodes 801..868 (§8.14; part 1 ported) |
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
| 1 | planet `a`: `+0xba −= 1` | | 8 | `st_bSetExtra(−a, 0)` (it registers `(8, a, id)` again, §8.14) |
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
success: `WIN(0, 1)` then `MEGNYERES(1)` (the human wins); `DIE` = `BirodalomMeghal(EN)` (the human loses, `diplomacy.md` §12). Unless
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
`Begin`, `Rnd`, `BRnd`, `Fable`, `Scen`, `Line`, `Multi` where kept), then
`campaign::inline_stories` (§8). A [`StoryWorld`] gives the world reads, the RNG, the calls that return something
(`FlottaNew`, `st_IsInRR`, `st_GetTerv`, `LoadScenario`, `Generate_StarNames`, the config and registry reads) and
takes the [`StoryEvent`]s: `MEGNYERES`, `MEGHALAS`, the undo actions that reach the world, game infos, fleet
orders and new ships, the `StoryAdat+0x30` info removal, the campaign's actions (§8). The campaign tables come
from the user's exe through the world (`campaign_tables`).

## 8. The campaign (`InitStory`, the inline stories of `Story_Check`)
Ported in `engine/crates/ig2-sim/src/campaign.rs` (+ the fleet helpers of §8.6 in `story.rs`), verified at
runtime (`traces/2026-10-16-campaign.md`, `tests/campaign_trace.rs`). Of the scripts `st_Line_include` (§8.9),
`st_Multi_include` (§8.10), `st_BRnd_include` (§8.11), `st_Begin_include` (§8.12), part of `st_Rnd_include` (§8.13)
part 1 of `st_Elem_include` (§8.14) and
`st_Fable_include` (§8.15) are ported, the others (§8.8) are not yet.

A campaign starts in `InitNewGame(seed, 0, campaign, difficulty)` (exe 0x4e1130; the race menu's
`CampaignGameStart` 0x4f5e90 runs it with `CampaignType` = race 1..=3, human 1/3/5, difficulty 1, 2 or 4):
`NEWGAME_Story` (§2.6) reads the story data, `Story_Generate`, then **`InitStory(0)`**, the starting fleets,
`INIT_MI` and `StartStory(400, false)`; the first daily check starts story 400.

### 8.1 Tables
Contiguous in the exe's `.data` from 0x928340 (423 words, `ig2-rules` `campaign_words`, `CampaignTables` in the
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
| `StoryKuldetes[12]` | 0x9289ac + 4·k | the story whose spy mission is `KemKuldetesek[14 + k]` (`st_Enable/DisableKemKuldetes`, `st_Rnd` 127) |

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
`st_Rnd_include` (0x5f0e20, 125 KB), `st_Elem_include` (0x5d4b70, 132 KB; part 1 ported, §8.14); `st_Line_include` (§8.9), `st_Multi_include` (§8.10) and
`st_BRnd_include` (§8.11) , `st_Begin_include` (§8.12) and `st_Fable_include` (§8.15) are ported, `st_Rnd_include` in part (§8.13: 36 of its 62 stories). They hold the
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

### 8.12 `st_Begin_include` (exe 0x5cec20, ELF 0x7407b0): the opening episodes
Ported in `engine/crates/ig2-sim/src/begin.rs` ([`BeginStories`]; `Story_Check` runs it first, before `BRnd`).
The 17 episodes story 401 draws from `StoryEleje` (§8.4) and the ones they start: 1, 4, 5, 51, 6, 7, 8, 9, 10,
11, 12, 19, 20, 21, 22, 23, 24, in that order. Each story block catches an abort (§8.7) with `WIN(0, 1)` and goes
on with the next story; a `WIN` inside a block does not stop it (its later tests still run). Ids are decimal
(story `s`'s ids are mostly `10·s + k`: 41 = 0x29); notation as in §8.9 and §8.11 (`b(x)`, `v(x)`, `l(x)` the logic
flag, `C(x)`, `A(b, r)`, `P(b)` promoted (flag 7), `R(b)` read (flag 9, `st_Olvas`: `StoryMessageBlokk == b`),
`M(id, arg)`, `Diff(a, b, c, d)` (§5), `N` = `Nehezseg`). Helpers (the fleet ones set `StoryError`):
- `atkF(id, x)` = `st_fAttackFleet` (exe 0x604520): `f = st_fSearch(id)`, `g = st_fSearch(x)` (`x < 0`: a random
  fleet of empire `−x`, stream 5); both ≥ 1 → `FlottaMozgat(f, 2, 2, g, 0)`, true. `atkP(id, x)` =
  `st_fAttackPlanet` (0x604590): the same with `g = st_bSearch(x, 0)` (`x < 0`: a random visible planet of `−x`,
  stream 5) and `FlottaMozgat(f, 2, 1, g, 0)`. `attack(id)` = `atkF(id, −EN)`, else `atkP(id, −EN)`.
- `moveP(id, b)` = `st_fMoveToPlanet` (0x604b40): `FlottaMozgat(f, 1, 1, st_bSearch(b), 0)`. `stands(f)` =
  `st_FlottaAll(−f)` (0x5ea3b0): fleet slot `f`'s `+0x40` (i16) is 0. `del(−f)` = `st_fDelete`: `FlottaDelete(f)`.
  `show(id)` / `hide(id)` = `st_fShow` / `st_fHide`: fleet `+0x52` bit 0x100 set / cleared. `away(id)` =
  `st_fEliminate` (§8.11).
- `tav` = `st_tav` (§8.11, through f32). `dist(q)` = `st_bDistFromBir(q, −1)` (exe 0x603240): over the planets
  1..`HanyBolygo` − 1 of `EN` the human sees, the least `(int)(f32) sqrt((f64)(f32)(dy² + dx²))` (f32
  coordinates, `dy` first) to planet `q`, from 1,000,000. `fdist(f)` = `st_fDistFromBir(f, −1)` (0x604650): the
  same with `tav` to fleet `f`'s truncated position.
- `far(lo, hi)` = `st_bGetFromDist` (exe 0x6036e0): `hi == 0` → `2·lo`. `st_bSearch(0, 1) == 0` (no free planet
  the human sees; a stream-5 draw when there is one) → `st_bSearch(0, 0)` (aborts). Else 100 tries: `q =
  st_bSearch(0, 0)`, `d` = the least distance (as `dist`) from the human's visible planets 1..n − 1 to `q`; `hi < d
  < best` (`best` from 1,000,000 + 500) → pick `q`, `best := d`; else `lo < d < hi` → `rnd(5, 1, k) == 1` (`k` = 1,
  2, … per such try) → pick `q`, `best := d`. No pick → `st_bSearch(0, 0)`. The ELF also asserts `hi > lo + 30`
  and asks for an unknown planet (`st_UnknownPlanetReq`) when there's no free one; the exe doesn't.
- `near(x, y)` = `st_GetNearestBir` (exe 0x5ea800): over planets 1..n owned by 1..8 other than `EN` the first, or
  one with `(f32)((y − py)² + (x − px)²) < (f32) best` (`best` the last pick's truncated value);
  `nearest_bir_planet` (0xF2652C) := the pick (0 before); returns its owner, 0 without one.
- `Relation(e, x)` = `st_ViszonyValt(EN, e, x)`: `Viszony[e][EN]` value 2 (`+4`, i16) `+= 100·x`.
- `infect(p)`: planet `+0xb8` (i16) += 20, then `Virus_CheckAdd(p)` (exe 0x542930): info 100 added unless a slot
  1..5 holds it, `StoryAdat+0x30 := 1` (the epidemic switch, §4).
- Battles as §5: `st_SpWarStart(id)`: one of `spFlottak`'s fleets has story id `id`; `st_SpWarWin/Lose(id)`: the
  participant's owner is / isn't `SpWarKiNyert`; `st_SpWarPartTulaj(e)`: the first of the first three entries (the
  third only with more than two) owned by `e`.

| Story | What it does |
|---|---|
| 1 (a base nearby) | start: `p = b(11)`, else `p = far(35, 80)` gets id 11; `st_bUnhide(11)`, `st_SetBolygoTulaj(p, 10)`, `st_bSetRadarMsg(11)` (planet `+0xbb := 1`), `AutoAddBuilding(11)`, `AutoAddtank(11)`, the human's view `+0xd8[EN] := 0`; fleet 11 (`st_fNewEx("", 10, 11, 0.06, 2, p)`, ships 3×2, 2×3, tanks 1×2), counter 18 := 30, `attack(11)`, `l(11) := 0`, counter 19 := 10. `C(18)`: fleet 12 the same at `b(11)` (no counters). `C(12)` and `b(11)` owner 10's: `l(11) := 1`, `M(11, 0)`, `SetParamStr(11, planet)` (slot 11: a slip kept). `R(11)`: `st_bShow(11, −1)`, info 11 (the planet's name). A space battle starts with fleet 11 or 12: `StoryAdat+0x94 := 1`. A ground battle on `b(11)` won by the attacker: `M(12, 0)`, the human +12,000, info 11 removed, fleets 11, 12 and 14 all gone → `WIN`. `C(19)`: each of fleets 11, 12, 14 that exists and stands: `attack(id)`; counter 19 := 10. `C(11)`: fleet 14 (`st_fNewEx("", 10, 14, 0.06, 2, st_fSearch(11))`: fleet 11's slot as the planet, a slip kept; ships 3×3, 2×5, tanks 1×4), `attack(14)`, `M(13, 0)`. `RangeB(11)` (flag 11 for `b(11)`) with `l(11)` false: `l(11) := 1`, counter 12 := 1, counter 11 := [0, 500, 350, 350, 250][N]. A battle ends with fleet 11, 12 or 14: `l(11)` false → `l(11) := 1`, counter 12 := 5, counter 11 := the same; then fleets 11, 12, 14 gone and `b(11)` not owner 10's → `WIN`. Then fleets 14, 12, 11 in that order: won their battle → `attack(id)` |
| 4 (the plague ship) | start: the human's planet with the most buildings (`HanyEpulet` 0xBA9518, strictly more, from 0; none → abort) gets id 41, `M(41, 100)`, `l(41) := 0`. `A(41, 7)`: the human −7,000, `WIN`. `A(41, 4)`, `A(41, 6)` or `P(41)`: `p = b(41)` the human's → `f = st_fNewEx("", 10, 41, 0.04, 1, p << 16)`, info 41 (no text), `k = Diff(1, 5, 15, 25)`, `d = (f32) sqrt((f64)(dy² + dx²))` planet − fleet (f32; 1 when 0), `FlottaMozgat(f, 1, 0, (int)(px − k·dx/d), (int)(py − k·dy/d))` (f32: it stops `k` short of the planet); else `WIN`. `st_ArrivedS(41)`: counter 49 := [0, 60, 30, 20, 10][N]. `C(49)`: fleet 41 gone or `b(41)` not the human's → `WIN`; else info 41 removed, info 42, `away(41)`, `infect(b(41))`, `l(41) := 1`, `M(44, 0)`. Daily with `l(41)`: `f = st_fSearch(41)` 0 → `WIN`, else `stands(f)` → `away(−f)`. A battle with fleet 41 lost and fleet 41 gone: `l(41)` → (`b(41)` infected and the human's: `M(46, 0)`, `+0xb8 := min(+0xb8, 5)`), `WIN`; else `M(42, 0)`, `WIN` |
| 5 (the hunter) | start or `C(59)`: no fleet of the human's (`st_fSearch(−EN)`, stream 5) → abort; `p = st_bSearch(−EN, 0)` gets id 51 (none → abort); fleet 51 (`st_fNewEx("", 13, 51, Diff(5, 6, 7, 8), 1, 120)`), counter 51 := 1, `atkP(51, 51)`, `M(51, 0)`, param 1 = `p`, `v(51) = v(52) = v(53) := 0`. `R(51)`: info 51, `show(51)`. Daily: `f = st_fSearch(51)` (none → abort); mode 1 for `N == 1`, 2 for `N == 2`, else 2 when planet 51 is owner 13's or fleet `+0x5c == 0` (then 4 if `N == 4` and the human has no fleet, a stream-5 draw), else 3. 4: `moveP(51, 51)`; 3: `atkP(51, 51)`; 2: over the human's fleets 1..n the lowest level (1: `+0x60 ≤ s/2`, 2: `≤ s`, else 3; `s` = fleet 51's `+0x60`; from 3), nearest by `tav` among equals (strictly) → `FlottaMozgat(f, 2, 2, g, 0)`; 1: the nearest by `tav` → the same; none → `away(51)`, `hide(51)`, `v(53) := 1`. Daily with `v(53)` and fleet 51 standing: `away(−f)`. Special 16 with `d1 == 51`: `WIN`. A battle with fleet 51 and the human: fleet 51 lost → global morale +20, `M(52, 0)`, `StartStory(51)`, `WIN`; else `v(52) == 0`, `N > 2` and `st_bSearch(−EN, 1)` (stream 5) → `v(52) := 1`, `M(53, 0)`. A ground battle on `b(51)`: attacker won → `v(51) == 0` → `v(51) := 1`, `st_bSearch(−EN, 1)` → `M(54, 0)`; defender won → fleet 51 gone → `M(52, 0)`, `StartStory(51)`, `WIN`; else `away(51)`, `hide(51)`, `v(53) := 1` |
| 51 | start: `d = st_GetTerv(EN, 511, 3)`, its price `+0x24 /= 2`, ticker `_s(5, …)`, `WIN` |
| 6 (rebels) | start: `p = b(61)` (unchecked), `st_SetBolygoTulaj(p, 10)`, `AutoAddBuilding(61)`, `AutoAddtank(61)`, fleet 61 (`st_fNewEx("", 10, 61, 0.1, 2, p)`, ships 3×2, 2×4), counters 61 := 250, 62 := 150, `M(61, 0)`, param 1 = `p`, `v(61) := 0`. `R(61)`: `st_bShow(61, −1)`, info 61 (planet), `v(61) := 2`. Daily: fleet 63 standing → `atkP(63, −EN)`, else `atkF(63, −EN)`; fleet 69 standing → planet 61 owner 10's → `atkP(69, 61)`, else `del(−f)`; `v(61) == 1`: planet 61 owner 10's → info 61, `v(61) := 2`, else info 61 removed, `v(61) := 1`. A ground battle on `b(61)` by fleet 69 → fleet 69 deleted, `WIN`; else won by the attacker: `v(69) == 0` → `M(62, 0)`, +15,000, `Relation(9, 10)`; else `M(64, 0)`, +10,000, `Relation(9, 8)`; `away(69)`, `WIN`. `C(62)` and planet 61 owner 10's: fleet 63 (`st_fNewEx("", 10, 63, 0.09, 2, b(61))`, ship 3×1), `atkP(63, −EN)`. `C(61)`: planet 61 not owner 10's → `M(62, 0)`, +15,000, `Relation(9, 10)`, `WIN`; else `st_SendSignal(6, 2)` (onto `VegzesVodor`), `M(63, 0)`, `e = near(planet 61)`, `v(68) := e`; 0 → `WIN`; else fleet 69 for `e` (`st_fNewEx("", e, 69, 0.1, 2, nearest_bir_planet)`, ships 3×3, 2×6, tanks 1×6 via `st_fAddTank`), `atkP(69, 61)`, `v(69) := 1` |
| 7 (troops wanted) | start: counter 71 := 20, `M(71, 0)`, `l(71) := l(72) := 0`. `C(71)`: the human's visible planet with the most `+0x74` + buildings (the first, then strictly more) gets id 73 (none → abort), `v(71) := 4`, `M(72, 150)`, int params 1 = `v(71)`, 2 = 15,000·`v(71)`, param 3 = the planet, int 4 = [0, 1000, 780, 630, 500][N]. `A(72, 2)`: counter 72 := the same, `l(71) := 1`. Daily with `l(71)`: info 71 replaced (planet 73, `st_szGet(72)` days). `A(72, 3)` or `P(72)`: `WIN`. Daily with `l(71)`: the units of size 19 (design `+0x16 == 2`, `+0x44 == 19` of their place's owner) on planet 73 (`Tankok` 0xBB41A8, place `−p`) sum to `v(71)` or more → `v(71)` of them go (`TankDelete(place, design, min(left, count))` record by record), counter 72 stopped, the human +15,000·`v(71)`; `l(72)` → `M(75, 0)`, `WIN`; else `far(80, 0)` 0 → `WIN`, else `M(73, 0)`; `l(71) := 0`, info 71 removed. `A(73, 2)`: info 73, counter 73 := 360, `StoryAdat+0xac := 1`. `A(73, 3)`: `WIN`. `C(72)`: `M(74, 0)`, `WIN`. `C(73)`: `+0xac := 0`, `WIN` |
| 8 (a design for sale) | start: `M(81, 0)`, `d = st_GetTerv(EN, 81, 3)`: `+0x6c := 3`, `+0x74/+0x78/+0x7c := Ev/Honap/Nap`, `+0x24 /= 3`, the record copied to `TradeTervek[TradeHanyTradeCuccVan++]` (exe 0xB9AAF0, count 0xF439CC, `trade.md`), `v(82) :=` its index, `v(89) := 0`. `R(81)`: info 81. Daily: no offer with `+0x16 > 0` and `+0x80 == 81` left → `M(82, 0)`, `WIN`. Special 0x13 for `EN` with `v(89) == 0` and the human's design `SpecialData2` keyed 81: `M(83, 0)`, `v(89) := 1` |
| 9 (a trade treaty) | start: `e = GetRndBirodalom(0, 0, 0)` (§8.11); unknown to the human or with a trade agreement (`Viszony[EN][e].+0x24`) → abort; `v(91) := 1000`, `v(92) := e`, `M(91, 120)`, param 1 = `e`'s name, int 2 = `v(91)`. `C(91)`: `v(92)` dead → abort; `M(92, 120)`, the same params. `A(91, 2)` or `A(92, 2)`: `x = v(91)`, the human −`x`; the trade agreement is written for `x` **as the empire** (`Viszony[EN][x].+0x24` and `[x][EN]`, outside the table: the original reads `v(91)` instead of `v(92)`), `EpuletNewSearchLocation(st_bSearch(−x, 0), 0x30, true, 0, −1, −1)` (no planet: a `rnd(5, 1, 0)` draw, planet 0), `Viszony[x][EN]` value 2 += 2000, `WIN`. `A(91, 3)`: counter 91 := 300. `A(91, 4)`, `A(92, 3)`, `A(92, 4)`, `P(91)`, `P(92)`: `WIN`. (The ELF's money checks `st_oldLetilt` are empty functions.) |
| 10 (a colony) | start: the human has 3 planets or more → `WIN`; else counter 109 := [0, 2000, 1600, 1200, 1000][N], `v(101) = v(102) := 0`, `M(111, 0)`, `v(103) := 0`. `R(111)`: `v(103) := 1`. `C(109)`: `M(113, 0)`, `WIN`. Special 10 (a colonisation) by `EN` with `v(101) == 0`: counter 109 stopped, planet `SpecialData2` gets id 101, counter 101 := 1080, `M(101, 0)`, param 1 = the planet, int 2 = [0, 5, 7, 8, 10][N], `st_bShow(101, −1)`, `v(101) := 1`, info 103 removed. `R(101)`: `v(102) := 1`. `R(101)`, or daily with `v(102)`: infos 101, 102, 105 removed; `st_eSearch3(b(101), 1)` (below) 0 → info 105 (planet); else `d = st_szGet(101)`: `d ≥ 31` → info 101 ("(d + 29)/30", planet), `2 ≤ d ≤ 30` → info 102 ("d", planet). Daily with `st_szGet(101) != 0`, or `C(101)`: planet 101's `+0x28 ≥ 1000·`[0, 5, 7, 8, 10][N] → `M(102, 0)`, the human +`(d/90 + 4)·5000` (`d = st_szGet(101)`), int 1 = that / 1000, `WIN`; else `C(101)` → `M(104, 0)`, `WIN`. Daily while still running: `v(101)` set and planet 101 not the human's → (`v(102) == 0` → `KiszedUzenet(101)`), `WIN`; else with `v(103)`: `v(101)` → infos 103 and 104 removed; else a ship of the human's (1..n, with a design) of a colony design (`+0x34`) → info 103 removed, info 104; else info 104 removed, info 103 |
| 11 (colonise in time) | start: `M(111, 90)`. `R(111)`: counter 111 := 920, `v(111) := 1`. `P(111)`: `WIN`. Special 10 by `EN` with `v(111)`: `M(112, 0)`, global morale +30, `WIN`. `C(111)`: `M(113, 0)`, `WIN` |
| 12 (a research grant) | start: `v(121) := 0`; nothing being researched (`ResearchAktFejlesztett[EN] == 0`) → pick now; else `v(129) := 1`, counter 129 := 180. `C(129)` with `v(129)`, or special 13 (research done) by `EN` with `v(129)`: `v(129) := 0`, pick. Pick: of techs 1..192 the human can be given (§8.11 `st_TalalmanyAdhato`) the lowest priority `+0xb8` (0 → 20; the first strictly lower, from 100); none → `WIN`; else `v(121) := t`, `v(122) := +0x38`, then `:= +0x34`, `M(121, 0)`, param 1 = the tech, its `+0x38`, the human's `+0x58 + 4·EN` and `+0x34` divided by 3. Special 13 by `EN` with `SpecialData2 == v(121)`: `M(122, 0)`, `WIN`. Daily: tech `v(121)` done by the human → `WIN` |
| 19 (a saboteur) | start: `st_CounterIntelligence(20)` (below) → caught; else counter 196 := 10, mode 1. `C(196)`: `M(191, 0)`, video, param 1 = `b(191)` (unchecked), counters 191 := 300, 195 := 80, 197 := 140, 198 := 200. `C(195)`, `C(198)` or `C(197)`: `st_CounterIntelligence(40)` → caught (`WIN`), else mode 2. `C(191)`: `st_CounterIntelligence(40)` → `M(193, 0)`, race of `Kemek[EN][CounterIntelligence_kem].+0x12` (1 when 0); `WIN`. With a mode: `st_eSearch(EN, mode == 1)` (below) none → `WIN`; else the planet gets id 191, the building's damage `+4 += hp·rnd(5, 40, 90)/100` (`hp` = `EpuletekLeiro[kind].+8`), at most `hp − 1`; ticker `sprintf(_s(5, …), the name of tech EpuletekLeiro[kind].+0, the planet's name)`. Caught: `e = GetRndBirodalom(5, 1, 0)`, its last spy `k` (1..`OsszHanyKem[e]`, `+0x10 > 0`) → `KemJelentesAdd(3, CounterIntelligence_kem, 0, 0, 0, 0, e, k, 0…)` |
| 20 (a tech to steal) | start: the first live known empire 1..8 other than `EN`; none → the nearest planet (`dist`, strictly less, from 1,000,000) owned by someone other than 0, `EN` and 5 (none → abort) gets id 201, `st_bShow(201, −1)`, its owner `e` is met (`BirodalomIsmeri`, `FelfedezBirodalom(e, 1)` when new), `M(201, 0)`, params `e`'s name, the planet; counter 201 := 20, `v(201) := e`. `C(201)`: `M(202, 0)`, `v(201)`'s name, `v(202) := 1`. Spy news (`KemEsemeny`, below) of mission 4 (a tech list) against `v(201)` with `0 < v(202) < 3`: of the 5 listed techs the human lacks (status 1..4) a reservoir pick (`rnd(5, 1, n) ≤ 1`); none → `v(202) == 1` ? (`M(206, 0)`, `v(202) := 2`) : (`M(207, 0)`, `WIN`); else `M(203, 0)`, param the tech, `v(203) := t`, `v(202) := 3`. Spy news of mission 8 (a tech stolen) against `v(201)` with `v(202) == 3` and the tech `v(203)` → `M(204, 0)`, race of the spy (`Kemek[EN][spy].+0x12`), `WIN`; otherwise `v(202) == 3` and tech `v(203)` done → `M(205, 0)`, `WIN` |
| 21 (a spy's target) | start: a weighted pick (10 known, 1 not) among the live empires 1..8 other than `EN` and 5 (none → abort); over the human's spies 1..`OsszHanyKem[EN]` − 1 alive (`+0x10 != 0`): the first with a target `+0x18` gives it as the empire and ends the search, else a weighted pick (10 when `+0x10 == 1`); none → abort; the spy's `+0x18 :=` the empire, `v(211) := k`, `M(211, 0)`, counter 211 := 360. `C(211)`: `M(213, 0)`, global morale −4, `WIN`. Spy news of a death (`+8 == 1`) of spy `v(211)` (or any while `v(211) == 0`) → `M(214, 0)` and `WIN` if the dead spy is `v(211)`; without such news: daily with spy `v(211)` gone (`+0x10 == 0`) → `M(215, 0)`, `WIN` |
| 22 (a far planet) | start: `n = v(500)` (the constellations, §8.2); no fleet of the human's (`st_fSearch(−EN, 1)`, stream 5) or `n == 0` → abort; of planets `b(500)..b(500 + n − 1)` (one missing → abort) the one the human has never seen (`+0xd8[EN] == 0`) nearest by `dist` (strictly less, from 1,000,000) gets id 221 (none → abort), `M(221, 0)` (campaign 2: 9221), param 1 = `_s(29, …)`. Special 10 by `EN` on `b(221)`, or a fleet of the human's arrives at `b(221)`: free or the human's → `M(223, 0)` (9223), +20,000, param 1 = the planet; `WIN`. Special 10 by another empire on `b(221)`: `WIN` |
| 23 (a fugitive) | start: fleet 231 (`st_fNewEx("", 9, 231, [0, 6, 7, 7, 8][N], 1, 0)`, ship 3×2), `M(231, 0)`, counter 231 := 40·N + 160, `FlottaMozgat(f, 1, 1, last_fn_b, 0)`, `v(232) := 1`. Daily with `v(232)`: fleet 231 (none → abort); the nearest visible planet of the human's and the nearest fleet of the human's by `(int)(f32)(dy² + dx²)` (strictly less, from 1,000,000; no planet → abort); the fleet within 144 → flee from it; else the planet within 99 → flee from it; else `FlottaMozgat(f, 1, 1, planet, 0)`. Flee from `q`: `dx, dy` = fleet − `q` (f32), `d = (f32) sqrt((f64)(f32)(dy² + dx²))`, `(f64) d < 0.1` → `d := 0.1f`, `dx := 0.05f`; `dx` clamped to `±d`; `a = (f32) acos((f64)(dx/d))`, `dy < 0` → `a := (f32)(2π − (f64) a)`; `a := (f32)((f64) rnd(5, −50, 50)·π/180 + (f64) a)`; `FlottaMozgat(f, 1, 0, (x + dx) + (f32) cos a·30, (y + dy) + (f32) sin a·30)` (f32; `dx` clamped, `dy` not). `C(231)`: fleet 231 (none → abort); fleet 232 (`st_fNewEx("", 9, 232, 0, 2, st_bSearch(−EN, 0))`, ships 3×3, 2×5) chases it (`FlottaMozgat(g, 1, 2, f, 0)`), `v(231) := 1`. Fleet 232 reaches fleet 231: fleet 232 (if any) takes id 231; the first fleet 231 (none → abort) stops (`+0x3c := 0`) and is renamed `_s(5, …)`, `M(232, 0)`, `away(231)`, `v(232) := 0`. A battle with fleet 232 and it gone: `M(236, 0)`, `away(231)`, `v(232) := 0`. A battle with fleet 231 and it gone: `v(231) == 0` → `M(234, 0)`, +25,000; else `M(233, 0)`, +14,000; `WIN`. Special 16 with `d1 == 231`: `M(235, 0)`, `WIN` |
| 24 (an escort) | start: counter 241 := 10. `C(241)`: of empire 9's fleets with ids 10000..=11000 and `+0x18` bit 0 clear the first, or one with a smaller `fdist`; none or `fdist ≥ 80` → counter 241 := 10; else `v(241) :=` its id, `v(242) := (int)` its target `+0x44`, its id := 241, `+0x52 |= 0x200, 0x800, 0x100`, it stops, `M(241, 0)`, param 1 = the fleet's name, counter 242 := 400. A fleet of the human's reaches fleet 241, or `C(242)`: fleet 241 (none → abort) gets id `v(241)` back, the three bits cleared, `FlottaMozgat(f, 1, 1, v(242), 0)`, `ErkezettFlottaNumber` stops (a stale slot on the counter path); `C(242)` → `M(243, 0)`, param 1 = its name, `WIN`; else `v(249) := 1`, counter 242 stopped, `M(242, 0)`, +4,000, param 1, `WIN` |

- `st_CounterIntelligence(x)` (exe 0x5d4900): `CounterIntelligence_kem` (0xF265D0) := 0; over the human's spies
  1..`OsszHanyKem[EN]` in state 3 (`+0x10`) on mission 13 (`+0x1a`, counter-espionage): chance =
  `clamp((int)((f64)((+0x33·40 + +0x32·60)·10)/100.0 + x), 10, 100)`, `rndlog(5, 0, 99) < chance` → caught by that
  spy (`CounterIntelligence_kem := k`) and the loop stops before the next spy. Returns whether one caught.
- `st_eSearch(e, mode)` (exe 0x604290): over the planets of `e` the human sees and their buildings (`Epuletek[p]`
  1..count, 0x20 bytes: `+0` kind, `+4` damage, `+8` i16) with a kind and `+8 == 0`: the first one is the answer
  unless a fitting one comes (`mode == 0`, or a kind 0x34..=0x37 for `mode == 1`): undamaged (`+4 == 0`) fitting
  ones win (a reservoir `rnd(5, 0, n) == 0`, restarted at the first undamaged one when the very first building was
  damaged); damaged fitting ones join only while no undamaged one was found.
- `st_eSearch3(p, kind)` (0x6041c0): over planet `p`'s (0: every planet's) buildings with a kind, `+8 == 0` and the
  kind `kind` (0: any) a reservoir (`rnd(5, 0, n) == 0`); the building's slot, 0 when none.
- `KemEsemeny` (exe 0xF2F874, with flag 15 `KemkedesTipus == 3`, from `KemJelentesAdd`, `espionage.md`): `+0` 1 for
  news, `+4` the spy, `+8` 1 when it died, `+0xc` success, `+0x14` the mission, `+0x18` the target empire, `+0x1c`
  the tech, `+0x24` 5 techs. `st_KemTalalmanyLista()` = news, success, mission 4; `st_KemTalalmany()` = news,
  success, mission 8; `st_KemMeghalt(k)` = news, died, `k == 0` or the spy `k`.

Exe vs ELF: the exe inlines the logic-flag and planet-id accessors, `st_bSetRadarMsg`, `st_fShow`/`st_fHide`,
`st_fAddTank` (except story 6's), `st_ViszonyValt`, `st_GlobalMoralValt`, `st_SpWarPart*`, `GetIsmertBirodalom`
(`GetRndBirodalom` + the "known" abort) and `Virus_CheckAdd`'s test, with the same results; it drops
`st_bGetFromDist`'s assertion and unknown-planet request, the two `st_bSearch` assertions (stories 1, 6) and story
9's `st_oldLetilt` money checks (empty in the ELF too). Precision (asm): every distance through f32 like `st_tav`;
story 4's approach and story 23's flight as above. Quirks kept: story 1's `SetParamStr(11, …)` and fleet 14 placed at
"planet = fleet 11's slot", story 9's trade agreement for the price as the empire, story 24 stopping a stale
arriving fleet on its counter, `st_eSearch` answering with the first building when none fits the mode.

### 8.13 `st_Rnd_include` (exe 0x5f0e20, ELF 0x74a2c0): random stories, part 2
Run right after `st_Begin_include` (`Story_Check` order: `Begin`, `Rnd`, `BRnd`, …). The exe calls `st_Story`
(0x602f80) and `st_StoryStart` (0x602fe0) here instead of inlining them, and drops the ELF's `st_oldLetilt`
branches (the ELF's `st_oldLetilt` always returns false). Each story block catches an abort with `WIN(0, 1)` (§8.7).
Notation as in §8.9/§8.11; ids are decimal in the text, the exe's are their hex (1181 = 0x49d).

**Map (all 62 stories).** Block starts in the exe (`push id; call st_Story`): 101 0x5f0e5f, 102 0x5f14e8, 108 0x5f1afa,
116 0x5f23b4, 117 0x5f257c, 118 0x5f27b8 (302 tested at 0x5f283e), 124 0x5f29a7, 127 0x5f310f, 128 (inlined test),
129 0x5f3c27, 131 0x5f434a, 132 0x5f4c8f, 133 0x5f50c9, 134 0x5f5b0b, 136 0x5f5c79, 137 0x5f6576, 138 0x5f681a, 139
0x5f6f21, 142 0x5f73ec, 144 0x5f75a2, 145 0x5f788e, 146 0x5f7e2b, 147 0x5f7fd5, 148 0x5f83ad, 149 0x5f879c, 150
0x5f8a94, 151 0x5f8b58, 152 0x5f97c8, 153 0x5f9964, 155 0x5f9b12, 156 0x5f9c89, 161 0x5f9d97, 163 0x5f9f13, 164
0x5fa074, 165 0x5fa15d, 166 0x5fa2ea, 169 0x5fa45a, 170 0x5fa4c2, 171 0x5fa92c, 172 0x5fab45, 173 0x5fadaf, 174
0x5faf8f, the heroes 176..192 0x5fb243..0x600a8f, 199 0x600b4f, 242 0x600c0f. What they reach: escorts
and pirates with fleet orders and battles (101, 102, 108, 124, 131), relations (`st_ViszonyValt` 116, 117, 145, 147,
148, 181, 183, 185, 190), diplomacy specials (147, 148), spy missions (127, 174: `st_KemEsely`, `st_EnableKemKuldetes`,
138: `Spy_New`, `Spy_Beszolas`), planets shown/colonised/owned (128..139, 145, 174), buildings written directly
(151..156, 161, 165, 166: `Epuletek` records, `EpuletDelete`, `st_Foldrenges`), the trade market (192
`st_SetTozsdeOlcso`), the terrorist switch (188), the heroes 176..192 (fleets for hire, `st_HajoKigyujt`, undo
entries).

**Ported (part 1):** 118, 142, 144, 146, 149, 150, 163, 164, 169, 170, 171, 172, 173, 199, 242 (`engine/crates/ig2-sim/src/rnd.rs`,
[`RndStories`]; verified, `traces/2026-10-16-rnd.md`).

| Story | What it does |
|---|---|
| 118 | (story 400 starts it) start: `v(1181) := v(1182) := 0`. A space battle ends (flag 3) with participants owned by `EN` and by 9 (`st_SpWarPartTulaj` 0x602d40: the first of list[0], list[1] (read even past a one-entry list), list[2] with more than two) and `SpWarKiNyert` (0xBAA1EC) `== EN`: the pirate's `+0x24` id `x`; story 302 running (`st_Story(302, 0)`, no mark) and `v(3021) == x` → nothing; `10000 ≤ x ≤ 11000`: `n = v(1181) + 1` (and `v(1182)` read) stored; `n == 5` → `M(0x4a7, 0)`; else `rnd(1, 3) == 1` → `v(1182) += 1`, `M(0x49d, 0)`, `a = rnd(3, 5)·2000`, `SetParamInt(1, a)`, `st_AddMoney(a)` |
| 142 | (400) start: some wormhole (1..=`OsszHanyWormhole`) with an end shown (`+8` or `+0x14`), or the count 0 → `WIN`; `v(1421) := 0`. Daily otherwise with `v(1421) == 0` and some end shown: some wormhole with both ends shown → `WIN`, else `v(1421) := 1`, `M(0x58d)` (campaign 3: 0x1145). Special 0x14 with `d1 == EN`: `d2 == 1` → `M(0x591)`, `WIN`; else `M(0x58f)` (0x1147) |
| 144 | start: counter 1441 := 30, `v(1441) := 0`. `C(1441)`: a weighted pick (`RndClass`) over planets 1..=n with morale `+0x30 / 10 < 200`; none → counter 1441 := 30; else `M(0x5a1)`, param 1 the planet, counter 1442 := 120, the planet gets id 1441. Daily with `st_szGet(1442) != 0` and `b(1441)` not the human's → `st_szStop(1442)`. `C(1442)` with morale `/10 > 300` → `M(0x5a6)`, param 1 `b(1441)`. Special 0x15 / 0x16 with `v(1441) == 0`: `M(0x5a4)` / `M(0x5a5)`, param 1 planet `d1` (0x16: param 2 its owner's name), `v(1441) := 1` |
| 146 | start: `v(1469) := 0`, counter 1469 := 10. `C(1469)`: a pick over the human's planets with `+0xd8[EN] > 0` and morale `< 350`; one → id 1461, counter 1461 := 2; none → `v(1469) < 11`: `+1`, counter 1469 := 360, else `WIN`. `C(1461)`: `M(0x5b5)`, param 1 `b(1461)`, counter 1462 := 30 (tested, no effect) |
| 149 | start: fewer than 8 planets → `WIN`; else `s = st_bSearch(−EN, 0)` (0 → abort); four times the human's planet (`+0x22 != 7`, no info 1491) nearest to `s` (`(int)(dy² + dx²)`, f32, against the last pick, first of equals) gets info 1491 (`BolygoInfoAdd(p, 1491, true)`, seen by the next round); no name at all → abort; `M(0x5d3)`, param 1 the names joined by ", ", counter 1491 := 150. `C(1491)`: a pick among the planets with info 1491 loses it, the count `n` of them; `n < 2` → `M(0x5d4)`, `WIN`; else counter 1491 := 20 |
| 150 | start: `M(0x5dd)`, `p = st_bSearch(−EN, 0)` gets id 1501, `+0xb8 += 50` (i16), `Virus_CheckAdd(p)` (exe 0x542930: info 100 shown unless present, `StoryAdat+0x30 := 1`), param 1, `BolygoInfoAdd(p, 100, false)`, `WIN` |
| 163 | start: for groups `g` = 0..=11 the first tech `16g + j` (`j` 1..=16) the human researches (status 4): `rnd(0, n)` (`n` such groups before), 0 → the pick; none → `WIN(0, 0)`; else `st_SetTalalmanyAllapot(t, 5, EN)` (the specials queue gets `(EN, t)`), `M(0x65f)` (campaign 3: 0x1217), param 1 the tech, `WIN` |
| 164 | start: `p = st_bSearch(−EN, 0)`; without info 1551: id 1641, `+0x24 += 20` (campaign 3: 16), `M(0x669)` (0x1221), param 1; `WIN` |
| 169 | start: `M(0x69b)`, `WIN` |
| 170 | first, while it runs: `StoryMessageBlokk` 0x125d → 0x6a5, 0x125e → 0x6a6 (a global write). Start: `t = st_GetRandomTalalmany(−1, 0)` (§8.11; −1 = `EN`), none → `WIN`; else `v(1701) := t`, `v(1702) := +0x34 / 2` (C division), `M(0x6a5, 50)` (0x125d), `v(1709) := 1`, param 1 the tech, `SetParamInt(2, price)`. Daily with the tech at status 5: `v(1709) == 1` → `KiszedUzenet(0x6a5)`, `== 2` → `(0x6a6)` (campaign 3's ids). `A(0x6a5/0x6a6/0x125d/0x125e, 2)` → `st_AddMoney(−price)`, `st_SetTalalmanyAllapot(t, 5, EN)`, `WIN`. `A(0x6a5 or 0x125d, 3)` → counter 1701 := 140, `v(1709) := 0`. Promotions of any of the four, `A(0x6a6 or 0x125e, 4)` → `WIN`. `C(1701)`: status < 5 → `M(0x6a6, 50)` (0x125e), `v(1709) := 2`, params as above; else `WIN`. `A(0x6a6 or 0x125e, 2)` again: pays `min(v(1702), (int) MoneyMennyi(EN, 0))` (read after the first payment), `WIN` (so the reminder is paid twice) |
| 171 | a tribute of 7,000: start `M(0x6af, 50)`, `SetParamInt(1, 7)`. `A(0x6af, 2)` or `A(0x6b0, 2)` → −7,000, counter 1711 := 15. `A(0x6af, 3)` → counter 1712 := `rnd(30, 90)`. `A(0x6af, 4)`, `P(0x6af)` → `WIN`. `C(1712)` → `M(0x6b0, 50)`, param 7. `A(0x6b0, 3)`, `P(0x6b0)` → `WIN`. `C(1711)` → `M(0x6b1)` (campaign 2: 0x1269), `WIN` |
| 172 | 9,000 (0x6b9/0x6ba, counters 1721/1722): only `A(0x6b9, 2)` pays; `P(0x6b9)`, `A(0x6b9, 4)` → `WIN`; reminder `A(0x6ba, 3 or 6)`, `P(0x6ba)` → `WIN`; `A(0x6ba, 5)` → `MoneyMennyi` read and dropped, −9 (the source's slip for −9,000), counter 1721 := 15; thanks 0x6bb (0x1273) |
| 173 | 3,000 (0x6c3/0x6c4, counters 1731/1732): `A(0x6c3, 4)` or `A(0x6c4, 2)` pays; `A(0x6c3, 5)` asks again; `P(0x6c3)`, `P(0x6c4)`, `A(0x6c4, 3)`, `A(0x6c3, 6)` → `WIN`; thanks 0x6c5 (0x127d) |
| 199 | (400) start: `v(1991) := 0`. Special 0x18 with `d1 == EN`: `v = v(1991)`; not campaign 1 → video `_s(6, 1991)`; `v == 3` → `WIN` |
| 242 | (400) the hero ships (`RndHosok[k]` 0x9288b0, 7 words: id, tech `T`, bonuses to `StoryAdat+0x84`, `+0x90`, `+0x7c`, fee (thousands), strength). Start: counter 0x975 := `rnd(1800, 2880)`. `C(0x975)`: `S = st_GetOsszFlottaEro() / 3`; a pick over the heroes with `v(id) == 0`, (`T < 3` or tech `T + 3` at status 5), (`T > 4` or tech `T + 7` not at 5), strength `< S` (tested in that order, the pick last); one → `st_vInc(0x975)`, `v(0x976) := k`, `v(id) := 1`, `M(0x975, 120)`, param 1 the name of `st_GetTerv(EN, id, 3)`, 2 a text, `SetParamInt(3, fee)`, 4 the bonuses' text; then `v(0x975) < 4` → counter 0x975 := `rnd(2520, 3600)`. `A(0x975, 2)`: hero `v(0x976)`: −fee·1000, `st_GetTerv`, `f = st_fNewEx(<design name>, EN, 0, 0, 1, 0)`, `HajoNew(f, st_GetTerv(EN, id, 3))`, `FlottaMozgat(f, 1, 1, last_fn_b, 0)`, counter `id` := 360. Any counter (`st_Counter(0)`) whose id is a hero with `v(id) == 1`: no ship of it (`st_hSearch(EN, id)`) → `v(id) := 2`, else −fee·1000 and counter `id` := 360. A space battle starts: each hero with `v(id) == 1` whose fleet (`st_hSearch`) is in `spFlottak` adds its three bonuses. Special 0x19 with `Tervek[owner(d1)][d2].+0x80` a hero's id: every ship of `st_GetTerv(EN, id, 3)` in a fleet of the human's → `HajoDelete`, `v(id) := 2` |

Port: the new world reads are the planets' morale/surface/richness (`+0x30`/`+0x22`/`+0x24`), the techs' research
cost (`+0x34`, st_Begin's read) and the wormholes; the new events `AddPlanetInfoHidden`, `SetPlanetRichness`, `SetTechState` (the epidemic
uses st_Begin's `SetEpidemic` + `VirusCheck`, the research cost its `tech_research`). The exe keeps the ELF's results everywhere checked (`st_oldLetilt` dead, C
division for the price, the two `CampaignType` id swaps). Untraced: 118's
exclusion of story 302's pirate (302 never ran in the traces), 146's last empty year. (149's four picks: a
planet whose five info slots are full doesn't get info 1491, so the next pick takes it again.)

**Ported (part 2a):** 116, 117, 127, 128, 129, 132, 133, 134, 136, 137, 139, 145, 147, 148, 152, 153, 155, 156,
161, 165, 166 (`engine/crates/ig2-sim/src/rnd/{relations,spies,finds,buildings}.rs`; verified,
`traces/2026-10-16-rnd2a.md`). The exe keeps the ELF's logic for all of them; its differences are draw orders
(below) and inlined helpers. Helpers (stream 5 unless said; `M`, `A`, `P`, `R`, `C`, `b`, `v`, `l` as above):
- `Pick(x)` = `RndClass(5)`'s `Next(x)` (ELF 0x588b90): `Σx += x`, the candidate wins when `rnd(1, Σx) ≤ x`.
- `st_HaborubanAllE(a, b)`: `Viszony[a][b].+0x25 < 3` (war or cease-fire). `st_GetEllensegesBirodalom(x)` (exe
  0x5ea640): over `e` 1..=8 (not `EN`, not `x`, alive, at war with the human) `Pick(1)`; 0 without one.
  `st_GetNemEllensegesBirodalom(x, y)` (0x5eaa60): the same for empires not at war, `Pick(10)` when the human knows
  them, else `Pick(1)`.
- `st_SetSemlegesViszony(a, b)` (exe 0x6028a0): `BirodalomBeketKot(a, b)`, both levels `+0x25 := 3`.
  `st_ViszonyValt(a, b, x)`: `Viszony[b][a].+4 += 100·x` (i16; the exe hoists the constants into registers).
- `st_EnableKemKuldetes(id)` (exe 0x5e6ac0) / `st_DisableKemKuldetes(id)` (0x5d4ab0): `k` with `StoryKuldetes[k]
  == id` (§8.1) → `KemKuldetesek[14 + k].+0 := 1 / 0`; enabling adds the undo entry `(5, id, StoryLastChecked)`
  (§2.3 type 5 disables it again). `st_KemSiker(id)`: flag 15, `KemkedesTipus` (0xF26588) `== 2`, `id == 0` or
  `KemkedesAzonosito` (0xF26584) `== id`; 127 then clears `KemkedesValasz` (0xF2659C), story state in the port.
- `st_RangeF(f)` / `st_RangeB(b)`: flag 10 / 11 and (`id == 0` or `ArrivedToRadarFleet == st_fSearch(f)` /
  `ArrivedToRadarBolygo == st_bSearch(b)`). `H(b)` = `st_ArrivedB(0, b)` with the arriving fleet the human's.
- `st_bGetNearestUnknown()` (exe 0x603a60): §8.11's second half (aborts without one). `st_bNearOutRR(lo, mode)`
  (exe 0x603b60): over free planets the human sees (mode 1: view `== 0`) not in its radar (`st_IsInRR((int) x,
  (int) y, 3)`) with `d = st_bDistFromBir(p)` (§8.12) `< best` (150 at first) or `< lo + 100`: once `best ≤ lo +
  100` only a `Pick(|lo − d|)` win takes it; the pick sets `best := d`. None → abort.
- `st_Colonise(p, e, 0)`: `Generate_Planet(p, e, e, false, false, 0, 0)` (exe 0x59af50) unless `e` owns `p`.
- `st_Foldrenges(p)` (inlined): `AktKepKell := 3`, `AktPlanetKell := p`, `Colony_elozo_foldrenges` 0x9ABAE4 := 0,
  `Colony_foldrenges` 0x9ABB70 := p (the colony screen shows the quake).
- `make_point_in_dist` (inlined, 136): `d = (f32) sqrt((f64)(dy² + dx²))` from the planet to `(last_fn_x,
  last_fn_y)`, the point `(to − from)·25 / d + from` (f32).
- Position texts: `X(p, r) = (f64)(p.x + 300.0f) + r / 100.0` (`"%.2f"`), the depth `Z = rnd(0, 600) + rnd(0, 99) /
  100.0`. 128/129: parameters 2..4 `X(x, rnd(0, 99))`, `X(y, rnd(0, 99))`, `Z`, drawn in that order; 132/133:
  parameter 2 `"%.2f, %.2f, %.2f"` of x, y, Z, the exe drawing `Z` first, then y's and x's hundredths.
- Buildings `Epuletek[p][k]` (0x20 B): kind `+0`, damage `+4`, `+8` (i16), strength `+0x18`, flags `+0x19`;
  `HanyEpuletLefoglalva[p]` (0xBA9840) of them; hit points `EpuletekLeiro[kind].+8` (0x8EBA40 + 0x24·kind).

| Story | What it does |
|---|---|
| 116 | start: `e = st_GetEllensegesBirodalom(5)`; `e == 0` or level 2 (cease-fire) → `WIN(0, 0)`; else `v(1161) := e`, `M(0x489, 0x8c)`, params `EN`'s and `e`'s names. `A(0x489, 2)`: `st_SetSemlegesViszony(EN, v)`, pirates (9) +3, `WIN`. Daily: not at war with `v` → `KiszedUzenet(0x489)`, `WIN`. `A(0x489, 3)` → `WIN`. `P(0x425)` (the source's slip for 0x489) → pirates −2, `WIN` |
| 117 | start: `e = st_GetNemEllensegesBirodalom(5, 0)`; 0, unknown or with a trade agreement → abort; `v(1171) := e`, `M(0x493, 0x28)`, params. Daily: trade agreement with `v` → `KiszedUzenet`, `WIN`. `A(0x493, 2)`: `Viszony[v][EN].+0x24 := Viszony[EN][v].+0x24 := 1`, `v` +8, pirates +3, `WIN`. `A(0x493, 3)`, `P(0x493)`: pirates −1, `WIN` |
| 127 | start: `e = GetRndBirodalom(5, 0, 0)`, `v(1271) := e`, `M(0x4f7, 0xa1)`, param `e`. `R(0x4f7)`: counter 1271 := 60, `st_EnableKemKuldetes(1271)`, `st_AddGameInfo(1271, …, 1, −1)`. `st_KemSiker(1271)` → `KemkedesValasz := 0`. `C(1271)`: `st_DisableKemKuldetes(1271)`, mission `14 + k` (`k` = 12 when not found); the human's spies 1..=`OsszHanyKem[EN]` with state (`+0x10`) 3 on it get state 1, `n` counted, the last kept. `n == 0` → `WIN`; one of rank (`+0x40`) < 2: `M(0x4f9)`, its race (`+0x12`) as the portrait, state 0, `WIN`; one of rank ≥ 2: `M(0x4f8)`, portrait, state 1, `+0x3c += rnd(800, 1200)`, the human meets `v` (unless known), three times `p = st_bSearch(−v, 0)` → the human's view `+0xd8[EN] := 1`, every tech `v` has (status 5) is seen (`+0xc0 + 9·EN + v := 1`), `WIN`; more → `M(0x4fa)`, `WIN` |
| 128 | start: `p = st_bGetFromDist(100, 150)` (§8.12), `b(1281)`; `t = st_GetRandomTalalmany(−1, 0)`; 0 → `WIN`; else `v(1281) := t`, counter 1289 := 2, `v(1289) := 0`. `C(1289)`: the human has `t` (5) → `WIN`; else `M(v(1289) ? 0x507 : 0x501, 0x28)`, params the planet and its position (128/129 texts), `v(1282) := v(1283) := 1`. Daily, `v(1283) ≠ 0` and the planet owned → `v(1282)` → `KiszedUzenet(0x501)`; `WIN`. `A(0x501, 2)`: `v(1282) := 0`, −10,000, `st_bShow(1281, −1)`. `A(…, 3)`, `P` → `WIN`. `A(…, 4)` → counter 1289 := 90. `H(1281)` with `v(1283)`: `M(0x506)`, counter 1281 := 20, `v(1283) := 0`. `C(1281)`: `rnd(1, 20) == 1` → `M(0x503)`, `WIN`; else `M(0x502)`, video, counter 1282 := 15. `C(1282)`: `r = rnd(1, 100)`; `v(1281)` not givable (`st_TalalmanyAdhato`) → a new random one; `r < 8` or none → `M(0x504)`; else `M(0x505)`, `rnd(0, 2)` (unused), tech given (`st_SetTalalmanyAllapot(t, 5, EN)`), param; `WIN` |
| 129 | start: `p = st_bGetNearestUnknown()`, `b(1291)`; `t` random, `v(1291) := t`, `v(1298) := 0`; 0 → `WIN`; else `M(0x50b, 0x14)`, params the planet and its position, `v(1293) := v(1294) := 1`, `v(1295) := 0`. `A(0x50b, 2)`: −3,000, shown, counter 1295 := 400, `v(1293) := 0`. `A(…, 3)`, `P` → `WIN`. Daily, `v(1294)`, owned → `v(1293)` → `KiszedUzenet`; `WIN`. `C(1295)` → `v(1295) := 1`. `H(1291)` with `v(1298) == 0`: `v(1298) := 1`, info `−1291` removed, `v(1291) := 1` (the tech lost: the source's slip), `M(0x510)`, counter 1291 := 20, `v(1294) := 0`, `st_fMoveToPlanet(1292, 1292)` and `st_RemoveFlottaAzonosito(1292)` (no such fleet). `C(1291)`: `rnd(1, 20)` (unused); `v(1295) == 1` → `M(0x50d)`, `WIN`; else `M(0x50c)`, video, counter 1292 := 15. `C(1292)`: `M(0x50f)`; tech `v(1291)` givable or a new random one: none → param a text; else given, param; `WIN` |
| 132 | start: `p = st_bGetNearestUnknown()`, `b(1321)`, `AutoAddBuilding(1321)`, `M(0x529, 0x1e)`, params the planet and its position (132/133 text), `v(1321) := 1`. `A(0x529, 2)`: −1,000, shown, `st_bSetRadarMsg`, `v(1329) := 0`. `A(…, 3)`, `P` → `WIN`. Daily, owned → `v(1329)` → `KiszedUzenet`; `WIN`. `st_RangeB(1321)` or `H(1321)`: free → `M(0x52a)`; `WIN` |
| 133 | start: `p = st_bGetNearestUnknown()`, `b(1331)`; `a = GetRndBirodalom(0, 0, 0)`, `b = GetRndBirodalom(a, 0, 0)`; `v(1331) := a`, `v(1332) := b`, `M(0x533, 0x1e)`, params, `AutoAddBuilding(1331)`, `v(1339) := 1`, `v(1338) := 0`. `A(0x533, 2)`: −3,000, shown, radar notice, counters 1331 := `rnd(260, 318)`, 1332 := `rnd(240, 344)`, `v(1339) := 0`. `A(…, 3)`: −6,000, shown, notice, counter 1339 := 20, `v(1339) := 0`. `A(…, 4)`, `P` → `WIN`. `C(1331)` / `C(1332)`: empire `v(1331)` / `v(1332)`: one of its planets gets id 1332 / 1333, `st_fNewEx("", e, id, 0.05, −b(id), 0)`, ships 4×1 and 3×3 (3×4), tanks 1×3 (1×4), `st_fMoveToPlanet(id, 1331)`. Arrival of 1332 or 1333 (any target): the planet free → `st_Colonise(b(1331), e, 0)`, the fleet deleted; else it goes home. Daily, owned: the settlers home (`st_fSearch` → `st_fMoveToPlanet(id, id)`, id removed), `v(1339) == 1` → `KiszedUzenet`, `WIN`. `st_RangeB(1331)` or `H(1331)`, `v(1338) == 0`: free → `M(0x534)`, else not the human's → `M(0x535)`; info `−1331` removed, `v(1338) := 1`. Special 10 for `EN` on `b(1331)` → settlers home, `WIN` |
| 134 | start: `p = st_bGetNearestUnknown()`, `b(1341)`, `AutoAddBuilding`, `M(0x53d)`, param, shown. Daily, owned → `WIN`. `H(1341)`: free → `M(0x53e)`; `WIN` |
| 136 | start: `p = st_bGetNearestUnknown()`, `b(1361)`; `st_fNewEx("", 10, 1362, 0.07, 1, 0)`, `b(last_fn_b) := 1362`, the fleet to the point 25 from that planet towards `(last_fn_x, last_fn_y)` (`FlottaMozgat(f, 1, 0, x, y)`), `st_fSetRadarMsg(1362, true)` (fleet `+0x52 |= 0x40`), counter 1364 := 600. `C(1364)` → `st_fEliminate(1362)`, `WIN`. `st_RangeF(1362)`: stop 1364; `v(1360) == 1` → counter 1361 := 30; else `M(0x551, 0x78)`, param planet 1361, `v(1360) := 1`. `A(0x551, 2)`: shown, `p = b(1361)`, `ry = rnd(−10, 10)`, `rx = rnd(−10, 10)` (exe order), fleet to `p + (rx, ry)`. `A(0x551, 3)` → eliminate, `WIN`; `P(0x551)` → owner 10 −1, eliminate, `WIN`. `H(1361)`: stop 1361, counter 1362 := 14, info `−1361` removed. Battle of 1362 with the human: `v(1366) ≥ 1` (`st_vGreater`)? won by 1362 → −10, +5,000, `M(0x556)`; else −5, `M(0x558)`, eliminate; `v(1366) < 1`: won by 1362 → −5, `M(0x558)`, eliminate; else −5, `M(0x559)`; `WIN`. `C(1361)` → `M(0x557)`, eliminate, `WIN`. `C(1362)`: `rnd(1, 10) == 1` → `M(0x553)`, eliminate, `WIN`; else a random tech given, `M(0x552 + (rnd(0, 1) == 0))`. `A(0x552, 2 or 3)`: `v(1366) := 2`, −7, eliminate, `WIN`. `A(0x552, 4)`: +9, eliminate, `l(1361) := 1` (147's other message), `WIN`. `A(0x553, 2 / 5 / 6)`: −7 / −10 / −17 and +25,000 / 30,000 / 50,000, eliminate, `WIN`. `A(0x553, 4)`: +9, eliminate, `WIN` (relations: the human with owner 10) |
| 137 | start: `p = st_bNearOutRR(0, 1)`, `b(1371)`, `M(0x55b)` (campaign 2: 0x14fb, 3: 0x1113), param, shown, radar notice. Daily, owned → `WIN`. `st_RangeB(1371)`, owned: not the human's → `M(0x55f)`; `WIN`. `H(1371)`: `r = rnd(1, 10)`, `t` random; `t == 0` and `r > 5` → `rnd(1, 5)` (unused); `t ≠ 0` → video, given, `M(0x55c)`, param; `WIN` |
| 139 | start: the free planet the human sees nearest its planets (`< 10000`) outside its radar; none → abort; but `p = st_bSearch(0, 0)` (a random free one: the source's slip), `b(1391)`, `AutoAddBuilding`, radar notice, `v(1391) := 0`. `st_RangeB(1391)`: owned → `WIN`; else `v(1395) == 1` (never set by the script): stop counters 1397, 1395, `v(1395) := 0`, `rnd(1, 10) < 4` → `M(0x576)`, param, `WIN`; else `M(0x575)`, param, the human's (`st_SetBolygoTulaj`), `WIN`; otherwise `M(0x56f)`, the human's, `WIN`, param. Daily or special 10, owned: `v(1391)` → `KiszedUzenet(0x570)`; `WIN`. `A(0x570, 2)`: `v(1391) := 0`, counter 1391 := 26 (0x570 is never sent). `A(0x570, 3 or 4)`, `P(0x570)` → `WIN`. `C(1391)`: `M(0x572)`, the human's, `WIN` |
| 145 | start: `e = GetRndBirodalom(0, 0, 0)`, `v(1459) := e`, `p = st_bSearch(−e, 0)`, `b(1451)`, pirates' (`st_SetBolygoTulaj(p, 9, −1)`), counter 1451 := 30. `C(1451)`: `M(0x5ab)`, params planet, `e`. `A(0x5ab, 2)`: −50,000, `v(1451) := 7`, counter 1452 := 15; `A(…, 3)`: −10,000, `v := 2`, 15; `A(…, 4)`: `v := 0`, 5. `P(0x5ab)` → counter 1453 := 7. `C(1452)`: `v(1451) < rnd(1, 10)` → counter 1453 := 15; else the human's, `e` −10, `M(0x5ac)`, params, `WIN`. `C(1453)`: `st_fNewEx("", e, 1451, 0.05, 2, st_bSearch(−e, 0))`, to planet 1451. Arrival of 1451: `e`'s again, fleet deleted. Battle of 1451: not won by it → the human's, `M(0x5ad)`, `e` −18; `WIN` |
| 147 | start: `e = GetRndBirodalom(0, 0, 0)`, `v(1479) := e`, `p = st_bSearch(−e, 0)`, `b(1471)`; up to 100 picks for another `q`; none → `WIN`; else `b(q) := 1482` (148's id; the source's slip), `M(l(1361) ? 0x5c7 : 0x5bf)`, params `e`, `e`, `p`, `q`. `A(0x5bf or 0x5c7, 2)`: counter 1471 := 25, `v(1471) := 2`, `st_AddDiplomacia(1471, e, −1)`; `A(…, 3)`: `v := 1`, the same. `P` of either → `WIN`. `C(1471)`: `v < 2`: `v ≠ 1` → owner 10 −6; `WIN`; else `st_fNewEx("", e, 1471, 0.05, 2, b(1471))` to planet 1472 (none). `st_Diplomacia(1471, 0)` (flag 13) → `e` +10, `v(1471) := 0`. Battle of 1471 with the human not won by 1481 (148's fleet) → `M(0x5c0)`, `WIN` |
| 148 | the same with ids 1481/1482, `v(1489)`, `M(0x5c9, 0xf)` (params `e`, `p`, `q`; `b` ids added twice), 20 days, no −6, fleet 1481 to planet 1482. Battle of 1481 with the human: not won by it → `M(0x5ca)`, +100,000, `WIN`; won → `M(0x5cc)`, `e` −15 |
| 152 | start: `M(0x5f1)`, `p = st_bSearch(−EN, 0)`, `b(1521)`, param; each building of kind ≠ 0: `rnd(0, 100) < 40` → damage `+= hp·rnd(20, 75)/100` (C division), at least `hp` → `hp − 1`; `st_Foldrenges(p)`, `WIN` |
| 153 | start: counter 1531 := 10. `C(1531)`: the human has tech 133 (status 5), `p = st_bSearch(−EN, 0)`, `st_eSearch3(p, 0x35) ≠ 0` (§8.12) → `M(0x5fb)` (campaign 3: 0x11b3), `r = rnd(10, 80)`, `v(1531) := r`, param, the kind-0x35 buildings' `+0x18 := byte·r/100`, `BolygoInfoAdd(p, 0x5fb, true)`, `b(1531)`, `WIN`; else counter 1531 := 60 |
| 155 | start: 6..=39 planets (`BirodalomHanyBolygo`), else `WIN(0, 0)`; up to 30 `p = st_bSearch(−EN, 0)` while `+0x24 > 100`; without info 0x669 (164's): `+0x24 /= 2`, `M` 0x60f with a mine (kinds 0x13..=0x16) else 0x610 (campaign 3: 0x11c7 / 0x11c8), `b(1551)`, param; `WIN` |
| 156 | start: `M(0x619)` (campaign 3: 0x11d1), `p = st_bSearch(−EN, 0)`, `b(1561)`, kind-0x35 `+0x18 := 0`, param, info 0x619, `WIN` |
| 161 | 153 with tech 134, kind 0x36, counter/id/info 1611 (0x64b; `b` before the message), `M(0x64b)` (campaign 3: 0x1203), `+0x18 := (u8)(int)((f64) byte · 1.33)`, no variable |
| 165 | start: counter 1651 := 60, `p = st_bSearch(−EN, 0)`, morale −200 (`st_BolygoMoralValt(p, 20)`), `b(1651)`, kinds 0x13..=0x16: `+0x19 &= 0xfe`, shown, +100,000, `M(0x673)`, video, param. `C(1651)` → `WIN` |
| 166 | start: up to 20 `p = st_bSearch(−EN, 0)` for one with a kind-13 building with `+8 == 0`; none → `WIN(0, 0)`; else video, `M(0x67d)`, `b(1661)`, params 2 the planet, 1 `EN`'s name, global morale −50 (`st_GlobalMoralValt(5)`), the planet's −100, `WIN` |

Port: the new world reads are the others' relation levels and trade agreements (st_Begin's), the human's spies'
rank (`spy_rank`), every empire's tech statuses (127), the buildings' `+0x18`/`+0x19` (`building_bytes`); the new
events `MakeNeutral`, `SpyMission`, `SpyState`, `SpyExperience`, `TechSeen`, `Colonise`, `BuildingByte`,
`Earthquake` and the texts `MessageText::{Number, Position}`; `KemkedesAzonosito`/`KemkedesValasz` are story state
(`Story::spy_story`/`spy_answer`).


**Ported (part 2b):** 101, 102, 108, 124, 131, 151 (`engine/crates/ig2-sim/src/rnd/battles.rs`; verified,
`traces/2026-10-16-rnd2b.md`). Helpers (exe):
- `st_SpWarStart(id)`: flag 1 and `id == 0` or one of the two fleets in `spFlottak` (§5) has story id `id`.
- The escorts' battle (101, 102, 131): fleet `f`'s position truncated, then every fleet slot 1..=`OsszHanyHajo`
  (the ship count: the source's slip; slots past the fleet count hold heap garbage, read as they are: `cvttss2si`
  turns a NaN or huge distance into INT_MIN, so such a slot joins) of the human's with
  `(int)(f32) sqrt((f64)(dy² + dx²)) < 10` (f32 from the truncated position) joins the starting battle
  (`st_AddFleetToSpWar(g)` exe 0x5c9cd0: while `SpWarHanyFlotta` (0xBAA1E8) < 4, `spFlottak[++n] := (g, 1)`; it
  writes `spFlottak[n + 1]` from the story's own count, so below 2 it overwrites the battle's entries 1–2, which
  later stories in the same check read), and `StoryUrharcViszony[9] := 1`, `StoryAdat+0x88 := StoryAdat+0x94 := 1`.
- `GetNewKereskedoAzonosito()` (exe 0x5c2770): one above the highest id in 10101..10999 of a fleet of owner 9 among
  slots 0..`OsszHanyFlotta` − 1, at least 10101. "Back to trading" (101, 108, 124): that id, `+0x52` bits 0x800 and
  0x100 cleared, speed `+0x3c := 0`, `+0x18` bit 0 cleared, `FlottaMozgat(f, 1, 1, b, 0)`.
- `st_fSetBarat(id, on)` / `st_fSetVillogas` / `st_fSetRadarMsg`: `+0x52` bit 0x200 / 0x80 / 0x40 (`id < 0`: slot
  `−id`; `StoryError` = no fleet). `st_fShow`/`st_fHide`: bit 0x100 (§8.12).
- `st_fNewPos(name, owner, id, x, y, speed)` (exe 0x605010): `FlottaNew(name, owner, id, x, y, speed ≤ 1 ? 100·speed
  : speed)`, `AutoAddship(f, id)` (§8.6).
- `st_bTav(a, b)` (inlined): `(int)(f32) sqrt((f64)(dy² + dx²))` (the ELF skips the f32 rounding).
- `SearchBolygohozKozeliFlotta(p, 15, true, true, false, false, 0)` (exe 0x540f50, `production-research.md`) and
  `EpuletDelete(p, k)` (exe 0x53c730: kind := 0, `HanyEpulet[p]` −1, the battlefield's turrets and target) are calls.
- A conquest by a story (`st_SetBolygoTulaj` → `BolygoElfoglal`, its cure exe 0x542980, colony-side.md §4 step 11):
  with no planet `+0xb8 > 0` left and no fleet with `+0x52` bit 2 (the new owner's fleets within `d² < 100` of the
  planet are cleared first), `StoryAdat+0x30 := 0` and info 0x65 is removed from every planet
  (`StoryWorld::conquest_ends_epidemic`, `StoryEvent::EpidemicOver`).
- `HajoDelete` drops the units the fleet can no longer carry (a nested `TankDelete`); `FlottaDelete` deletes the
  fleet's units.

| Story | What it does |
|---|---|
| 101 | start: 15 planets or more → abort; `v(1011) := 0`, `b(1011)` a planet of the human's, `b(1012) := st_bGetFromDist(100, 0)` (200), `M(0x3f3, 0x14)`, params `EN`'s name and planet 1011. `A(0x3f3, 2)` → counter 1019 := 60. `C(1019)`: `st_fNewEx("", 9, 1011, 0.04, 1, b(1011) << 16)`, friendly, to planet 1011. `A(0x3f3, 3)`, `P` → `WIN`. Arrival of 1011 at a planet: at 1011 → `M(0x3fb)`, param planet 1012, to planet 1012, counter 1011 := 15, shown; at 1012 → `+0x54 < 2` (ships) → `M(0x3f5)`, pirates +3, 3,500, else `M(0x3f4)`, +7, 10,000; fleet 1012 deleted; the trader (if any) not friendly and back to trading towards a planet of the human's; `WIN`. `C(1011)`: `st_fNewEx("", 10, 1012, 0.05, −b(1012), 0)`, `st_fAttackFleet(1012, 1011)`. Battle start with both: fleet 1011 (none → abort) escorted. Battle of 1011: gone → `M(0x3f8)`, 1012 home (none → `WIN`); else 1011 to planet 1012. Battle of 1012: 1011 gone and 1012 not in it (never: the source's slip) → `WIN`; else 1012 home. 1012 at a planet → deleted, `WIN` |
| 102 | start: 15 planets or more → abort; `e = GetRndBirodalom(0, 0, 0)`, `b(1021)` the human's, `b(1022) := st_bSearch(−e, 0)` (0 → abort), `M(0x3fd, 0x14)`, param `e`. `A(0x3fd, 2)` → counter 1029 := 60. `C(1029)`: trader 1021 as 101's. `A(…, 3)`, `P` → `WIN`. Arrival of 1021: at 1021 → to 1022, `rnd(1, 100) < 60` → counter 1021 := 4; `M(0x405)`, shown; at 1022 → 1021 and 1022 deleted, `M(0x3fe)`, 10,000, pirates +8, `WIN`. `C(1021)`: `st_fNewEx("", 10, 1022, 0.06, −b(1022), 0)`, ships 3×4, 2×16, attacks 1021. Battle start: escorted. Battle of 1021: gone (`st_fSearch`) → pirates −14, `M(0x402)`, `st_fEliminate(1022)`, `WIN`; else to 1022. 1022 at a planet → deleted, `WIN`. `P(0x3fd)` → pirates −2, `WIN` (after the first `WIN`) |
| 108 | start: `p` a planet of the human's (`b(1081)`), trader `st_fNewEx("", 9, 1081, 0.03, 1, p << 16)`, ships 8×3, friendly; target `t = (rnd(−20, 20) + p.x, rnd(−20, 20) + p.y)` (f32, x drawn first), the trader to `f + (t − f)·300 / d` (`d = (f32) sqrt((f64)(dy² + dx²))`); `r = last_fn_r + rnd(35, 55)`, `q = rnd(−25, 25) + last_fn_q`, pirate `st_fNewPos("", 10, 1082, (int)(planet(last_fn_b).x + r·cos(q·3.141592/180)), (int)(p.y + r·sin(…)), 0.05)` (f64), ships 3×2, 2×8, attacks the trader; `M(0x439)`, logic 1081 and 1082 set, the trader shown and blinking. `st_ArrivedF(1082, 1081)`: logic 1081 off, counter 1089 := 35, both stop, the human's fleet nearest the pirate (`(int)(dx² + dy²)` f32 `< 50`) follows it. Else both in a finished battle: the trader gone or the human not in it → the pirate sent away, pirates −6, `WIN`; else `M(0x43a)`, +8, `k = rnd(0, 10) + 4·GetRndStoryLevel()` thousands, param `k`, `WIN`. Else the pirate in it, gone, the human in it: the trader gone → pirates −2; else +12, `M(0x43b)`, `k = rnd(0, 10) + 5·level` thousands; `WIN`. Daily, logic 1081, the pirate idle → it attacks the trader. Ended (one of those `WIN`s) and the trader alive: not blinking, not friendly, back to trading towards a planet of the human's. `C(1089)`: logic 1082 off, the pirate attacks. The pirate at a planet → deleted, pirates −2, `WIN`. (`Story_SpaceBattleStarted`'s 1081/1082 cases, §8.5, are the engine's.) |
| 124 | start: `e = st_GetEllensegesBirodalom(0)` (none → `WIN(0, 0)`), `v(1241) := e`, `r = rnd(10, 15)`; trader `st_fNewEx("", 9, 1242, 0.03, 1, 0)`, ships 8×2, friendly; up to 100 tries `(x, y) = (int)(last_fn_x + r·cos(last_fn_q·π')), (int)(last_fn_y + r·sin(…))` (`π' = 3.141592/180`, f64), `r += 1`, while `st_IsInRR(x, y, 3)`; the last try → abort; pirate `st_fNewPos("", 10, 1241, x, y, 0.07)`, ships 3×3, 2×5; the trader shown; both radar-reported; the trader blinking; `M(0x4d9, 0x1e0)`, video, counter 1249 := 480. `C(1249)`: both hidden, the trader not friendly, the pirate deleted, `WIN`. `st_RangeF(1242)` or `(1241)`: counter 1249 stopped, the other's radar report cleared (`1241 + !RangeF(1242)`), the trader hidden and not friendly; the pirate (none → abort) hunts: `st_fGetNearest((int) x, (int) y, EN)` → `FlottaMozgat(f, 2, 2, n, 0)`, none → `st_fEliminate(1241)`, `WIN`; `M(0x4da` or `0x4db` when the pirate was seen`)`, `e`'s portrait and name, `v(1249) := 1`. Daily with `v(1249)`: gone → `WIN`, else hunts. Battle start with it → `StoryAdat+0x88 := 1`. Battle of it: gone → `WIN`, else hunts. Ended by the counter or a contact and the trader alive: not blinking, not friendly, back to trading towards `st_bSearch(−e, 1)` (0 → a planet of the human's) |
| 131 | start (tech 57 done, else `WIN(0, 0)`): 101 times two random planets of the human's, each kept only with an intact (`+8 == 0`) building of kinds 14..=16; two different ones farther apart (`st_bTav`) than the best become the best; `t = ResearchAktFejlesztett[EN]`; no pair or `t == 0` → `WIN(0, 0)`; else `b(1311)`, `b(1312)`, both `st_bShow(…, −1)`, `v(1319) := t`, the first empty of `t`'s prerequisites 1..2 (else the third) := 0xb8 (a tech nobody has), `st_SetTalalmanyAllapot(t, 2, EN)`, `M(0x51f)`, params the planets and the tech, counter 1311 := 5. `C(1311)`: `st_fNewEx("", 9, 1311, 0.01, 2, b(1311))` to planet 1312, friendly, shown; `st_fNewEx("", 10, 1312, 0.06, 1, b(1312) << 16)` attacks it; `M(0x522, 0x14)`, param planet 1312, an info line. `st_ArrivedB(1311, 1312)`: `M(0x520)`, video, prerequisites 0xb8 cleared, `t` to status 4 when nothing is researched (`ResearchAktFejlesztett[EN] == 0`) else 3, 1311 eliminated, hidden, its id removed, 1312 eliminated, `WIN`. Battle start with both: 1311 (none → abort) escorted. Battle of 1311, gone: `M(0x521)`, 1312 eliminated, prerequisites cleared, `t` to 3 and its research time `+0x58 + 4·EN := 0`, `WIN`. Daily, 1311 idle → to planet 1312 |
| 151 | start: `p` a planet of the human's (`b(1511)`), monster `st_fNewEx("", 0, 1511, 0.07, 3, 0)`, `last_addship := 0`, `+0x52 |= 0x4000` (story-protected), `FlottaMozgat(f, 2, 1, p, 0)`, `v(1511) := 200` (its hit points), `v(1512) := v(1513) := 1`. Daily with `v(1512)` and `st_fDistFromBir(f) < 150` (§8.12): a video (not campaign 1), shown, `v(1512) := 0`, `M(0x5e7)` (campaign 3: 0x119f), param the planet, an info line. Daily, idle: `FlottaMozgat(f, owner(p) == 0 ? 1 : 2, 1, p, 0)`. Arrived at 1511: free → deleted, `WIN`; else attacks it. Special 0x12 (data 2 = 1: `SearchBolygohozKozeliFlotta(b(1511), 15, …)`; 2: data 3) → fleet `a`: `a.+0x60 < hp` → deleted, `hp −= +0x60`, `M(0x5e9)` (0x11a1), ints `200 − hp`, `(200 − hp)·100/200`; else draws while `hp > 0` (`RndClass(5)`): ships of `a` (design ≠ 0x7fff; weight 1, or 0 when the design's attack `+0x2c` is 0 and something was picked), then `Tankok` of `a` with non-ground designs (`+0x16 ≠ 2`; picked on `Next(attack)` or when nothing was picked); none → stop; a ship: `hp −= attack`, deleted (`HajoDelete`) unless `−attack/2 > hp`; units: `hp −= attack`, `TankDelete(a, design, 1)`. `v(1511) := hp`; `hp < 1`: a video (campaign 1 only), the monster deleted, `M(0x5e8)` without losses, else `M(0x5ea)` (0x11a2) with the ships and units lost, `WIN`; else `M(0x5e9)` (0x11a1). Special 0x12 with data 2 = 1 while the monster lives: `total = Σ SearchBuilding(p, 0x1c + k, true)·(2 + k)` (k 0..3: intact, `+0x19` bit 0); the human's planet: `total == 0` → video, `hp == 200` → `M(0x5ef)` (0x11a7) else `M(0x5ed)`; `total < hp` → video, `M(0x5ec)`; else video, `M(0x5eb)` (0x11a3). `total < hp`: `pct = ((hp − total)·100/200)/2`; every intact building (`+8 == 0`, kind ≠ 0): kind 1 or `rnd(1, 100) > pct` → with `rnd(1, 100) ≤ pct` damaged to `max(damage, hp_k·rnd(5, 95)/100)` (a second draw when the first falls short); else `EpuletDelete`; the planet's `+0x28 −= pct·+0x28/100`; ints destroyed, damaged, lost. The attacking fleet (data 1) deleted, `WIN` |

Port: the new world reads are the fleets' ship counts (`fleet_ship_count`), `near_fleet`, the designs' kind
(`design_kind`) and attack, the epidemic test; the new events `FleetState` (`+0x52` bits), `DeleteBuilding`,
`TechPrereq`, `TechResearchTime` and `EpidemicOver`. `game::StoryView` doesn't carry out story owner changes yet,
so its `conquest_ends_epidemic` is the default `false`.

**Ported (part 2c):** 138, 174 (`engine/crates/ig2-sim/src/rnd/spies.rs`), the heroes 176..=192
(`engine/crates/ig2-sim/src/rnd/heroes.rs`; verified, `traces/2026-10-17-rnd2c.md`). The exe keeps the ELF's logic
except where noted. Helpers (exe):
- `st_oldLetilt(b, 0)` is exe 0x48fce0, `return 0`: every `LetiltReszlet` branch of the heroes is dead code.
- `st_HajoKigyujt(story, owner)` (exe 0x5eb040): over ships 1..=`OsszHanyHajo` (re-read each step) with design
  `≠ 0x7fff` whose owner's design key `/ 10` is `story` and whose fleet isn't `owner`'s: `last_hajo_kigyujt_flotta`
  (0xF265CC) := that fleet; the first one makes fleet `nf = st_fNewEx("", owner, 0, 0.0, −1, 0)` (its two stream-5
  draws around planet 1 included) placed on that fleet's `+0x30/+0x34`; `HajoDelete(k)`, `HajoNew(nf,
  st_GetTerv(owner, key, 3))`. Then `Tankok` 1..=`OsszHanyTank` with a fleet (`> 0`) the same way (`TankDelete(place,
  design, n)`, `st_GetTerv`, `TankNew(nf, d, n)`; `last_hajo_kigyujt_flotta` untouched). Returns `nf`.
- A hero leaves (176, 177, 179, 181, 182): `f = st_HajoKigyujt(story, 10)` (181: to empire `v(1811)`); 0 → the abort;
  `f`'s name := `_s(5, id)`; `last_hajo_kigyujt_flotta` still named `_s(5, id)` → its `+0x52 &= ~0x800`, name :=
  `AlapFlottaNev`; `st_fEliminate(−f)`.
- `st_KemEsely(kind, a, lo, hi, roll)` (exe 0x5eb4e0) on `Kemek[KemkedesTulaj][KemkedesAktKem]` (0xF26598,
  0xF265A0, set by `StoryEredmenyKemSikeres`): `x` = kinds 1, 2: `100·+0x31`; 3: `70·+0x33 + 30·+0x31`; 4: `70·+0x32 +
  30·+0x33`; else 0; `c = (x·50/a)/10` capped at `hi`, then raised to `lo`; with `roll` `rnd(0, 99) < c`.
  `st_IsKemKuldetesEnabled(id)` (0x5eb430): `KemKuldetesek[14 + k].+0` for `StoryKuldetes[k] == id`.
- `GetIsmertBirodalom(a, b, c)` (exe 0x5c2720): `GetRndBirodalom(a, b, c)`; one the human doesn't know aborts.
- `st_SpWarPartID(f)` (0x602cf0): the finished battle's entry `+0x20` (slot) is `f` among the first two (three with
  more); `st_SpWarStartTulaj(e)` (0x602e70): one of `spFlottak`'s two fleets is `e`'s; `TervCopy(e, a, b)` (0x541cd0);
  `make_point_in_dist` (0x5c9bf0): `p0 + (p1 − p0)·d / (f32) sqrt((f64)(dy² + dx²))` in f32.

| Story | What it does |
|---|---|
| 138 | start: `p = st_bGetNearestUnknown()` (aborts without one), `b(1381)`, `M(0x565)`, param the planet, `st_bShow(1381, −1)`, `v(1381) := 1`, every other empire's view `+0xd8 + 2e := −1`, `v(1382) := 0`. Daily: owned → `st_bUnhide(1381)`, `WIN`. A fleet of the human's arriving at it with `v(1382) == 0` and counter 1381 not running: first visit (`v(1381)`) → `v(1381) := 0`, `v(1384) := 1`, info `−1381` removed, the planet named `_s(5, 1381)`; `k = max(2 − n, 0)` for `n` the human's live race-14 spies (slots 0..=`OsszHanyKem`): `k == 2` or first visit → `M(0x566)`, `k == 1` → `M(0x567)` (both `v(1382) := 1`), else `M(0x568)` and counter 1381 := 7; `v(1383) := k`. Answer 3 → `v(1382) := 0`. Answer 2: `v(1382) := 0`, `k = min(2 − min(n, 2), v(1383))`, `v(1384)` → 2 (cleared); `k` times: up to 30 picks `rnd(0, 6)` of the seven names (exe 0x85E0BC + 8·i) while a live race-14 spy has it; `Spy_New(EN)` (its own stream-4 draws) `> 0` → 138's record (below), the name, `+0x16 := 1`, `+0x3c := rnd(4: KemRangXPHatar[2], [3])`, `KemJelentesAdd(2, k, …)` under `Story_check_locked` (§8.15's 310) |
| 174 | start: `p = st_bSearch(−EN, 0)`, `b(1741)`, `M(0x6cd)`, `st_BolygoMoralValt(0x6cd, −4)` (the slip: planet slot 1741, `+0x174 += 40`, `+0x178 := 6`, exe 0xC2CA04/08, past the planets), mission enabled, counters 1741..1744 := 30, 60, 90, 150. Planet not the human's → `M(0x6ce)`, `WIN`. `st_KemSiker(1741)`: `KemkedesValasz := ` enabled ? `st_KemEsely(4, 50, 20, 100, true)` : 0; non-zero → mission withdrawn, `M(0x6d0)`, `WIN`. `C(1744)`: planet to empire 0 (`st_SetBolygoTulaj(p, 0, −1)`), `M(0x6d1)`, counter 1745 := 90. `C(1745)`: `M(0x6d2)`, `WIN`. Ground battle on it won by an attacker of the human's → `M(0x6d3)`, `WIN` |
| 176 | start: `v(1768) := 0`, counter 1769 := `rnd(1700, 2200)`, `v(1761) := 0`, undo `(10, 1762, 176)`. `C(1769)`: `M(0x6e1, 60)`, a random empire's name. Answer 2: −3,000, `st_fNewEx(_s(5, 0x6e1), EN, 1761, 0, 1, 0)`, `v(1761) := 1`, to `last_fn_b`, counter 1761 := 360, id removed. 3 or lapsing → `WIN`. `C(1761)`: no ship of key 1761 (`st_hSearch`) → `WIN`; else `M(0x6e2, 60)` (with `v(1768)`: −3,000), counter 360. 0x6e2: 2 or 4 (`v(1768) := 1`) −3,000; 3 → leaves, counter stopped, `WIN`; lapsing → leaves, `M(0x6e3)`, `WIN`. A battle starting with its ship's fleet → `StoryAdat+0x84 += 14`. Special 0x19 of key `/10 == 176` → leaves, `WIN` |
| 177 | counter 1779 := 1, `v(1770) = v(1771) = v(1778) := 0`, undos 1772, 1774, 1777. `C(1779)`: tech 7 and attack/3 > 320 → `M(0x6eb, 120)`, else 500 days. Hired (answer 2, −8,500) like 176 (counter 1771). Finished battles with its fleet (`st_SpWarPartID`) count `v(1770)`; the second → `M(0x6ee, 100)`, the fee counter cut to 120 days when shorter. 0x6ee: 4 → `v(1778) := 1`; 3 → leaves, counter 1772 := 10; lapsing → the same and `M(0x6ef)`. `C(1772)`: `M(0x6f0)`, `rnd(1, 100) < 70` → `GetRndBirodalom` (its draws only), `WIN`. `C(1771)`: no ship → `WIN`; `v(1770) < 2` → `M(0x6ec, 60)`; else `v(1778)` ? −10,000 : `M(0x6f1)`; counter 360. 0x6ec 2: −8,500; 0x6f1 2/4: −10,000 (4: `v(1778) := 1`); 0x6ec/0x6f1 3 → leaves, counter 1771 stopped, 1772 := 15; lapsing → the same and `M(0x6ef)`. The battle bonus (`+0x90 += 10`) when its ship is *gone* (the source's slip). Special 0x19 → leaves |
| 178 | start or `C(1789)`: `v(1788) := 0`; techs 8 and 148 and attack/3 > 1000 → `M(0x6f5, 60)`, else counter 1789 := 530. Answer 6: −20,000, `st_fNewEx(_s(5, 0x6f5), 10, 1781, 0, 1, 0)` to `last_fn_b`, `v(1789) := 1`. 3, 5, 7, lapsing → `WIN`. Daily with `v(1789)`: fleet 1781 gone → `WIN`, idle → to a planet of the human's. Battle with 1781 and `v(1789)` → it's sent away, `WIN`. Battle of its ship's fleet with `v(1788)` → `M(0x6f6)`, leaves, `WIN` (no ship → `WIN`); a ground battle with it → `M(0x6f9)`, leaves; `WIN`. 1781 arriving with `v(1789)`: `v(1789) := 0`, `v(1788) := 1`; its ships' and units' story designs become the human's (`st_GetTerv(EN, key, 3)`), the fleet the human's, id removed, counters 1781 := 1000, 1782 := 900. `C(1782)`: any ship or unit of key `/10 == 178` → `M(0x6f8)`, else `WIN`. Leaving or `C(1781)`: each fleet of the human's gives its 178 ships (deleted, made anew for empire 10) and units to a new pirate fleet on its position, which then gets `+0x5c − +0x78` ships of design 0x6f6; the old fleet named `_s(5, 0x6f5)` loses its name; the new one is sent away. `C(1781)` with any → `M(0x6f7)`, +10,000, `WIN` |
| 179 | 176's pattern: counter 1799 := 1 (tech 78, attack/3 > 1500, else `rnd(490, 1200)` days), `M(0x6ff, 60)`, 17,000 and yearly 17,000 (`M(0x700, 0)`: arg 0, unlike the others), `v(1798)`. Answer 4 or lapsing: a random empire gets the fleet (`st_fNewEx(_s(5, 0x6ff), e, 1791, 0, 2, 0)`, id removed), `WIN`. 0x700 3 or lapsing → leaves, `WIN`. Battle bonus `+0x84 += 13`, `+0x90 += 18` |
| 180 | start: of the human's ships the strongest design (`+0x2c`, first of equals) with `+0x16 == 1` and no key; none → `WIN(0, 0)`; else `M(0x709, 60)`, the design's name, `v(1801) := 0`, `v(1802) := d`. 2 or 4: `t = st_GetTerv(EN, 0x709, 3)`, `TervCopy(EN, d, t)`, `t.+0x80 := 0x709`, `+0x18 := 0`, `+0x19 := 1`, named `_s(5, 0x709)`; the first such ship of the human's gets `t`; `v(1801) := 1`. Battle with its ship's fleet → `+0x7c += 30` (none → `WIN`). Special 0x19 of key 180: `t` named after its tech (`+0x48`), key 0, `+0x18 := 1`, `+0x19 := 0`, `WIN`. 5 or lapsing → `WIN` |
| 181 | counter 1819 := 1: tech 9, more than 50,000 (`MoneyMennyi`, f64) and attack/3 ≥ 3001, else 570 days; `e = GetRndBirodalom`, `M(0x713, 60)`, `v(1811) := e`, its name. 2 or 5: hired (25,000), counters 1811 := 360, 1813 := 100, `v(1819) := 1`. 4, 6, lapsing → `WIN`. A finished battle or a daily check with `v(1819)` and its ship gone → messages 0x714, 0x716 taken back, `WIN`. Fee `M(0x714, 60)` (`v(1818)`); 3 → leaves; lapsing → `M(0x715)`, leaves. 0x716 (never sent): 2 or lapsing → relations −10, 3 → leaves. Bonus `+0x7c += 18`, `+0x90 += 20`. Leaving: its ships to empire `v(1811)` |
| 182 | counter 1829 := 1: tech 10, attack/3 > 2000, else 590 days; `M(0x71d, 60)`; hired (28,000; `v(1821) := 1`); daily, its ship gone → `WIN`; fee `M(0x71e, 60)` (`v(1828)`); lapsing → `M(0x71f)`. The bonus looks for 181's ship (`st_hSearch(EN, 0x713)`: the slip): `+0x7c += 28`, `+0x90 += 22` |
| 183 | start: `e = GetIsmertBirodalom(5, 0, 0)`, `v(1839) := e`, `M(0x727, 60)`, `e`'s race and name, `v(1838) = v(1837) := 0`. 4: −4,000, `v(1831) = v(1832) := 1`, `StoryAdat+0xa0`, `+0xa4 += 1`, `+0x6c += 20`, counter 1831 := 180, `rnd(1, 100) > 60` → counter 1832 := 20 (it never fires anything). 7 or 9: −3,400, `v(1831) := 1`, a random tech for the human, the same counters. 3, 6, 10: relations +2, `WIN`; lapsing → `WIN`. `C(1831)`: `v(1838) += 1`, below 9 → again 180 days; `v(1837) == 0` → `M(0x728, 60)` the first time (`v(1832) := 1`) else `M(0x72a, 60)` (race), else −4,000. 0x728 2/4/6 and 0x72a 2/4: −4,000 (0x728 6, 0x72a 4: `v(1837) := 1`). 0x728 5, 0x72a 3 → ends; lapsing → `M(0x729)` (race), ends; ending takes the bonuses back |
| 184 | counter 1849 := 1: tech 11, attack/3 > 3000, else 610 days; `M(0x731, 60)`. 2: hired for 100,000, to `last_fn_b`'s coordinates (`FlottaMozgat(f, 1, 0, x, y)`), counter 1841 := 720; 3: 50,000, 360 days. 4 or lapsing: `rnd(0, 99) < 70` → `st_fNewEx(_s(5, 0x731), e, 1841, 0, 2, st_bSearch(−e, 0))` (the planet drawn first) with id 0; `WIN`. Fee `M(0x732, 60)` (`v(1848)`, 50,000). 0x732 3: counter stopped, `rnd(1, 2) == 1` → leaves (sent away), `WIN`, else revenge; lapsing → revenge, `M(0x733)`. Fleet 1842 arriving with `v(1849)` (the target empire) → its ships go to that empire, `WIN`. Daily with `v(1849)`: fleet 1842 gone → `WIN`, idle → to planet 1841. Bonus looks for 181's ship: `+0x7c += 20`, `+0x90 += 6`. Special 0x19 → leaves. Leaving: `st_HajoKigyujt(184, n)` (n: 10, or `v(1849)` on arrival; none → abort), named `_s(5, 0x731)`, id 1842; arrived → id 0; revenge → `v(1849) := GetRndBirodalom`, its planet `b(1841)`, the fleet to it; leaving → sent away |
| 185 | start: `e = GetRndBirodalom`, `v(1859) := e`, `M(0x73b, 60)`. 2: −1,400, counter 1851 := 100; the human sees (view 1) `e`'s planets it knows and the five free planets it knows nearest to them (the exe's `sqr` is x87: `(int)(f32)((f64)(f32)dy² + dx²)` with f32 differences, `cvttss2si`; the ELF's f32 throughout; insertion into a list of ten from 10,000), `StoryAdat+0x9c += 1`. 3 or lapsing → `WIN`. `C(1851)` with `e` alive → `M(0x73c, 100)`, its name and race. 0x73c 2 or lapsing → relations −9, `WIN`; 3 → `+0x9c −= 1`, `WIN` |
| 186 | start: `e = GetIsmertBirodalom(0, 0, 0)`, `M(0x745, 60)`, `e`'s name second, `v(1869) := e`, `v(1868) := 0`. 2: `StoryAdat+0xa8 += 3`, −2,000, counter 1862 := 180. 3 or lapsing → `WIN`. `C(1862)`: `n = v(1838) + 1` (183's: the slip), `v(1638) := n`, below 6 → again; `M(0x746, 60)`; 2 → −2,000; 3 → `+0xa8 −= 3`, `WIN`; lapsing → `M(0x747)`, the same |
| 187 | start: a random empire's name, `M(0x74f, 60)`. 2: −3,500, `StoryAdat+0x80 += 10`, `+0x78 += 15`, `+0x70 += 20`, counter 1872 := 180. `C(1872)`: `M(0x750, 60)` (with `v(1879)`: −3,500), again. 0x750 2/4 −3,500 (4: `v(1879) := 1`); 3 or lapsing (`M(0x751)`) → bonuses back, `WIN` |
| 188 | start: `M(0x759, 60)`. 3 or 6: −20,000, `WIN` (the ELF's `st_TerrorCselValt(3)` is not in the exe). 5 or 7: `rnd(1, 10) ≤ 4` → `WIN`, else counter 1881 := `rnd(4, 10)·10`; lapsing: `≤ 2`. `C(1881)`: `M(0x75a, 60)`: 2 or 6 → −45,000, counter 1885 := 30; 3 → counter 1883 := 20; 5 → counter 1884 := 30; lapsing → 10. `C(1883)`: `M(0x75c, 60)`: 2 → −45,000, counter 1885 := 20; 3 → counter 1884 := 20; lapsing → 10. `C(1884)`: `rnd(1, 10) > 5` → `M(0x75f)`, counter 1886 := 25; else `M(0x75e)`, `WIN`. `C(1885)`: `M(0x75d)`, `WIN`. `C(1886)`: `rnd(1, 10) > 5` → `M(0x760)`, morale −6, else `M(0x761)`, −8; `WIN` |
| 189 | start: `M(0x763, 60)`, two empires `rnd(1, 8)` (the human's → the next, `% 8 + 1`) in `v(1898)`, `v(1899)`, params `v(1898)`'s and the human's names. Read → counters 1891 := 200, 1892 := 40; lapsing → `M(0x76b)`, counter 1891 := 10. `C(1892)`: `rnd(1, 10)`. Nothing ends it |
| 190 | start: `e = GetIsmertBirodalom(5, 0, 0)`, `v(1909) := e`, `p0 = st_bSearch(−EN, 0)`, `q = st_bGetNearest(centre of the human's planets, e)` (none → abort); `q` in the human's radar (`st_IsInRR`) → `WIN(0, 0)`; else spy ship `st_fNewPos(_s(5, 0x76d), 10, 1901, q, 0.08)`, radar-reported; target `make_point_in_dist(p0, q, −1000)` clamped to ±300 along the line from `q` (f32, `+ 0.0001`), `+ rnd(−20, 20)` x then y, `v(1902)`, `v(1903)` the target, `FlottaMozgat(f, 1, 0, x, y)`, `+0x18 |= 2`, `M(0x76d)`, video, `e`'s race and name. Seen (`st_RangeF`) → `M(0x775)`. Special 0x10 for 1901: `v(1901) == 0` → `M(0x774)` (race); `WIN`. Battle starting with it and the human → escort `st_fNewEx(_s(5, 0x76d), 9, 1902, 0.06, 4, f)` joins (`st_AddFleetToSpWar`), `StoryUrharcViszony[9] := 1`, `StoryAdat+0x88 = +0x94 = +0x98 := 1`. That battle finished with the human: ship gone → escort gone → counter 1903 := 5, else counter 1901 := 5 and the escort deleted; ship alive → escort gone → ship sent away, `WIN`, else the ship to its target and the escort deleted; `v(1901) := 1`. `C(1903)`: `M(0x76f)`, `WIN`. `C(1901)`: `M(0x76e, 70)`, video (not campaign 1), `e`'s name; 2 → counter 1902 := 10; 3 → counter 1904 := 20. Message 0x770 lapsing → `M(0x773)`, 5 days. `C(1902)`: `rnd(0, 1) == 0` → `M(0x771)`, +5,000, a random tech, else `M(0x770)`, +30,000; race, name, relations +15, `WIN`. `C(1904)`: `M(0x772)`, race, name second, relations −8, `WIN` |
| 191 | start: up to 20 planets of the human's (`st_bSearch(−EN, 0)`, 0 → abort) for one of surface 1, 2 or 5; found → info 0x776 (`BolygoInfoAdd(p, 0x776, true)`); `WIN` |
| 192 | start: `v(1921)` (story 530's) 0 → `WIN(0, 0)`; else `M(0x1fc2)`, counter 0x1fc3 := 360, cheap market (`st_SetTozsdeOlcso(1)`). `C(0x1fc3)`: market back, `M(0x1fc3)`, `WIN` |

138's spy record (stack template): `+0x10 = 1` (then `+0x16 := 1`), `+0x12 = 14`, `+0x14 = 4`, `+0x16 = 100`, `+0x1a =
4`, `+0x1c = 3`, `+0x24 = 1000`, `+0x31..+0x34 = 5, 2, 6, 1`, `+0x3c = 0x260c` (overwritten), `+0x40 = 2`, the rest 0
except `+0x0f`, `+0x13`, `+0x19`, `+0x1d`, `+0x22..+0x23` (left as the stack held them).

Port: the new world reads are spy mission switches (`spy_mission_on`), fleet names (`fleet_named`), a fleet's units
and room (`fleet_units`), 138's names (`recruit_name`), the spies' `+0x31`, `+0x16` and names; the new events
`SetPlanetViewOf`, `NamePlanet`, `StoryRecruit`, `NameFleet`, `SetFleetPos`, `SetFleetOwner`, `SetTankDesign`,
`CopyDesign`, `SetDesignKey`, `DesignFlags`, `NameDesign`; story state `KemkedesTulaj`/`KemkedesAktKem`
(`Story::spy_owner`/`spy_slot`) and `last_hajo_kigyujt_flotta` (`Story::last_collect_fleet`). `game::StoryView`
doesn't answer the new reads yet (defaults).

All 62 stories of `st_Rnd_include` are ported; the replay of a trace recorded before part 2c leaves its stories
out (`RndStories::without`: the exe had them patched out).

### 8.14 `st_Elem_include` (exe 0x5d4b70, ELF 0x77ac00): the storyline episodes, part 1
The episodes 801..=868 the storylines (§8.9) start: 49 story blocks, in the same order in both binaries; unlike
the other scripts the exe calls `st_Story` (0x602f80) and `st_StoryStart` (0x602fe0) per block instead of
inlining them. Each block catches an abort with `WIN(0, 1)` and goes on (§8.7). **Part 1 is ported** in
`engine/crates/ig2-sim/src/elem.rs` ([`ElemStories`], run after `LineStories`) and verified at runtime
(`traces/2026-10-16-elem.md`): 805, 811, 813, 816, 817, 818, 820, 830, 831, 835, 837, 841, 842, 864..=868. **Not
ported yet** (the tracer patches their `st_Story` ids out): 801, 802, 804, 806..=810, 814, 815, 821, 824, 832,
834, 836, 839, 840, 843, 844, 850..=863 (spy missions `st_Kem*`, the real-time ground war's `WF_Tank` madness,
diplomacy specials and relations, `StoryInit839_GenerateBolygok`, `st_bGet3Nearest`, `st_bGet4Brutal`, …; the
ELF decompile is `_local/decomp/elem/0x77ac00.c`, the exe's `_local/decomp/elem-exe/`). Notation as in §8.9 and
§8.11; `R(b)` = `st_Olvas(b)`: flag 9 with `StoryMessageBlokk == b`; `G(f, b)` = `st_GrWarPart(f, b)`: flag 4,
fleet id `f` (0: any) and `GrWarBolygo == b(b)` (0: any); `RB(b)` = `st_RangeB(b)`: flag 11 with
`ArrivedToRadarBolygo == b(b)`; `AW` = `GrWarAttackerWin` (0xF26532). Ids are decimal.

| Story | What it does |
|---|---|
| 805 | start → `WIN` |
| 811 | start: counter 8110 := 30 days. `C(8110)` → `WIN` |
| 813 | start: counter 8131 := 360, `st_SetTozsdeOlcso(1)`. `C(8131)`: `st_SetTozsdeOlcso(0)`, `M(8131)`, `WIN` |
| 816 | start: `st_bShow(8160, −1)`, planet 8160 also gets id 4031, `AutoAddtank(8160)`, `AutoAddBuilding(8160)`, `StartStory(403)`, game info 8161 (the planet), `v(8161) := 1`. A ground battle ends (flag 4) or a space battle ends (flag 3) with 8160 not empire 10's: `v(8161)` → info 8161 removed, `BolygoInfoRemove(p, −8160)`, `v(8161) := 0`; no fleet with id 4030..=4039 left (`st_fSearch`) → `WIN`. `C(8183)` with 8160 empire 10's → `M(5614)`, counter 8189 := 80. `C(8189)`: the raider `st_fNewEx(_s(5, 8185), 10, 8185, 0.06, 2, b(8160))`, `st_fEliminate(8185)` (§8.11), `v(8181) := 1`. `st_ArrivedS(8185)` → `WIN`. `st_SpWarPartTulaj(EN)` and `st_SpWarPart(8185)` → `M(5615)`, counter 5619 := 10. `C(5619)`: `v(8181) := 2`, `WIN` |
| 817 | start: `st_fNewEx(_s(5, 8170), 11, 8170, 0.07, 3, 0)`; it attacks (`st_fAttackPlanet`) `−EN` when `v(7250)`, else planet 8011 when the human owns it, else `−EN`; counter 8179 := 20, game info 8171. `C(8179)`: fleet 8170 gone → `WIN`; else `st_FlottaAll(8170)` → attack `−EN` again; counter 8179 := 10. Daily at difficulty 4: every planet of empire 11 loses its buildings (`EpuletDelete` 0x53c730 for each non-empty slot 1..=`HanyEpuletLefoglalva[p]`) and goes to owner 0 (`BolygoElfoglal(p, 0)`) |
| 818 | start: `v(8189) := 1`, `StartStory(816)`. `st_StoryFinish(816)` → `WIN` |
| 820 | start: planet 8200 (`st_bSearch(8200, 1)`, unchecked) → `st_SetBolygoTulaj(p, 10, −1)`, `AutoAddtank`, `AutoAddBuilding`, counter 8201 := 1. `C(8201)`: the human has fewer than 2 planets → counter 8201 := 10; else `M(8206)`, `StartStory(821)`, `st_bAddInfo(8200, −8200, −1)`, `v(8209) := 0`. `R(8206)`: `st_bUnhide(8200)`, `st_bSetRadarMsg(8200, 1)`, `st_SetBolygoCoord(b(8200), 50)`, the guard `st_fNewEx(_s(5, 8201), 10, 8201, 0.06, 2, p)`, counter 8209 := 420. (`RB(8200)` or `C(8209)`) and `v(8209) == 0`: `M(8203)`, param 1 = the planet, `st_bShow(8200, −1)`, `v(8209) := 1`. `G(0, 8200)` with 8200 not empire 10's: `M(8205)`, `SetParamInt(1, 234000)`, the human +234,000, `WIN` |
| 830 | start: the supernova planets (below, ids 8301/8302; an abort without them), `M(8309)`, params the human's empire name and planet 8301, counters 8301 := 900, 8302 := 15, `st_bShow(8301, −1)`, `st_bSetRadarMsg(8302, 1)`, `AutoAddtank` 8301 and 8302. `RB(8302)`: `M(8308)`, `v(8300) := 1`. `C(8302)`: `M(8300)`. `R(8300)`: game info 8301. `G(0, 8301)` and `AW`: `v(8301) := 1`, info 8301 removed; `v(8300) == 0` → `M(8301)`, `st_bShow(8302, −1)`, `st_bSetRadarMsg(8302, 0)`, game info 8302; else 8302 empire 16's → `M(8307)` and game info 8302, else `M(8306)`. `G(0, 8302)` and `AW`: `RemoveGameInfo(8302)` (the UI line only: the undo entry stays). `C(8301)`: `M(v(8301) == 1 ? 8302 : 8303)`, `st_BolygoDestroy(b(8301))` (`StoryError = p < 1`), video `_s(6, 8301)`, `WIN` |
| 831 | `d = [0, 12, 10, 7, 5][Nehezseg]`. start: tech 162 done (status 5) → `WIN`, else `M(8310)`. `R(8310)`: game info 8311, counters 8312 := 4, 8311 := `30·d`. `C(8312)`: `M(8311)`, `SetParamInt(1, d + 4)`, `(2, d)`; techs 161..=171: research time `+0x38` and the human's time left `+0x58 + 4·EN` halved (C division); `v(8319) := 1`. Special 13 (a tech researched) with `d1 == EN` and `161 ≤ d2 ≤ 171`: counter 8311 running (`st_szIsNowGoing`) → `M(8312)`, param 1 = the tech's name, done; else `M(8313)`; `WIN`. `C(8311)`: some of the techs at status 4 or with time left → `M(8314)`, else `M(8315)`; done. Done and `v(8319)`: `v(8319) := 0`, the halved fields doubled (`<< 1`) |
| 835 | start: game info 8351 (empire `v(8359)`'s name; the German build, `Lang == 4`, shows its race's name with a suffix and reads the variable once more), `v(8351) := 0`. A ground battle ends (flag 4) or daily: over the planets of empire `v(8359)` the human sees (`+0xd8[EN] ≥ 0`): count them, unmark each marked one (`st_bIsExtra`: `+0x187`; `st_bSetExtra(−p, 0)`); exactly one → it is marked; none and `v(8351) == 0` → `M(8352)`, info 8351 removed, counter 8351 := 20, `v(8351) := 1`. `C(8351)`: `M(8353)`, `st_bGetLegerosebb()` (below) gets id 8351, param 1 = it, `WIN`. `C(8350)`: `M(8351)`, `WIN` |
| 837 | start: counter 8370 := 5. `C(8370)`: of the owned planets the human sees (`+0xd8[EN] ≥ 0`) the human's are more than two thirds (`3·own > 2·seen`) → `ScrollUzenet(_s(5, 8371), 1, true, false)`, `f = st_fNewEx(_s(5, 8370), EN, 8370, 0, 1, 0)`, `FlottaMozgat(f, 1, 1, last_fn_b, 0)`, video `_s(6, 8370)`, `M(8370)`, param 1 = planet `last_fn_b`, `WIN`; else counter 8370 := 5 |
| 841 | start: `M(8410)`, counter 8411 := 280 (nothing tests it); tech 6 done → `WIN`; else `v(8418) := 6`, tech 6's `+0x38`, `+0x34` (price) and the human's time left halved, `v(8419) := 1`, game info 8411 (the tech's name). Daily with tech 6 done: `M(8412)`, `WIN` (the halving is never undone) |
| 842 | start: `v(8429) := 1`. Daily with `v(8429)`: the human has a ship (`Hajok`, design ≠ 0x7fff, in a fleet of the human's) whose design's byte `+0x34` is non-zero (the exe; the ELF tests bit 0) → `M(8420)`, counter 8429 := `[0, 1080, 900, 540, 540][Nehezseg]` days, `v(8420) := v(8429) := 0`, game info 8421. Special 10 (a colonisation) with `d1 == EN` and `v(8429) == 0`: `v(8420) += 1`; info 8421 (first) or 8422 removed; 2 → `M(8421)`, global morale +20 (`st_GlobalMoralValt`), `WIN`; else game info 8422. `C(8429)`: `M(8429)`, `WIN` |
| 864 | start: empire 1 or 5 dead → abort; `v(8651) := 0`, `M(8640)`, counter 8640 := 200, `st_SetSzovetsegesViszony(EN, 5)` (`BirodalomBeketKot(EN, 5)` 0x53b280, `Viszony` level 5 both ways). Daily, at war with 5 (`st_HaborubanAllE`: level < 3) and `v(8460) == 0`: `v(8460) := 1`, `M(8649)`, `StartStory(866)`, `WIN`. `C(8640)`: `MoneyMennyi(EN, 0) ≤ 100000.0` → counter **8460** := 52 (named like the variable; nothing tests it), else `M(8641)`, param 1 = empire 5's name. `A(8641, 2)` / `A(8641, 3)`: `x = (int)(MoneyMennyi(EN, 0) · 0.75` / `· 0.4)` (f64): the human `−x`, empire 5 `+x`. `A(8641, 4)` or `P(8641)`: counter 8640 := 100. `A(8644, 2)`: counter 8642 := 60. `A(8644, 3)` or `P(8644)`: counter 8643 := 200 (8642 and 8643 are tested by nothing). `st_ArrivedS(8642)`: `st_fAttackFleet(8642, 8640)`. Daily: empire 1 dead → `M(8645)`, `st_SetHaborusViszony(EN, 5)` (`Viszony[5][EN].+4 −= 10000`, level 1 both ways), `WIN`; else empire 5 dead → `M(8646)`, `v(8661) := 1`, `StartStory(866)`, `WIN` |
| 865 | daily with `v(8651)`: empire 5 dead → `M(8623)`, `WIN`; else empire 1 dead → unless story 405 runs (`st_Story(405, 0)`): `M(8622)`, `StoryAdat+0xc8 := 1`, `StartStory(867)`; `WIN` |
| 866 | daily: `v(8661) == 0`: empire 5 dead → `M(8646)`, `v(8661) := 1`; else empire 1 dead → as 865's. `v(8661) != 0` and empire 1 dead → `M(8647)`, `WIN` |
| 867 | start: counter 8671 := 120. `C(8671)`: `StoryAdat+0xc8 := 0`, `WIN` |
| 868 | start: `v(8681) := 0` (its daily test has an empty body) |

Helpers (exe; the ELF's are functions, the exe inlines some):
- **`st_SetTozsdeOlcso(on)`** (exe 0x602b20): `StoryAdat+0x60` counts the requests (`on == 0` with the count below 1
  does nothing); the trade goods' prices (`TradeTervek[k]` `+0x24`, exe 0xB9AAF0 stride 0x84, count 0xF439CC) become
  `(int)((f64) price × 0.8)` on the first request (0 → 1) and `× 1.25` when the last one ends (1 → 0).
- **`st_fAttackPlanet(f, b)`** (0x604590) / **`st_fAttackFleet(f, g)`** (0x604520): `st_fSearch(f)`, then `st_bSearch(b,
  0)` (`b ≤ 0`: a random planet of empire `−b`, stream 5) / `st_fSearch(g)`; both found → `FlottaMozgat(f, 2, 1 or 2,
  target, 0)`, `StoryError := 0`, else 1. **`st_FlottaAll(x)`**: fleet `−x` (`x < 1`) or `st_fSearch(x)`; `+0x40 ==
  0`; `StoryError` = no fleet. **`st_SpWarPartTulaj(o)`** (0x602d40): flag 3; the 1-based participant whose `+0x18
  == o` among the first two (three when there are more than two).
- **`st_bSetExtra(x, on)`** (0x603ed0): planet `−x` (`x < 1`) or `b(x)`; non-zero → `+0x186` and `+0x187 := on` and the
  undo entry `(8, p, StoryLastChecked)`. `WIN`'s type-8 undo calls it with `on = 0`, which registers `(8, p, id)`
  again (§2.3: the first free slot, or appended past the cursor; the loop re-reads the count and applies that one
  too, which moves it into the slot just zeroed), so an extra-marked planet's undo entry outlives its story.
- **`st_bAddInfo(x, info, owner)`** (0x603170): `BolygoInfoAdd(p, info, true)` and the undo entry `(6, p << 16 | info &
  0xffff, owner)` (`owner == −1`: `StoryLastChecked`); `StoryError` = no planet. **`st_bSetRadarMsg(id, on)`**
  (inlined): planet `+0xbb`.
- **`st_SetBolygoCoord(p, w)`** (0x6025d0; `w` 0 → 40): up to 999 tries: `q = st_bSearch(−EN, 0)` (none → false), `r
  = rnd(5, (int)((f64) w·0.7 + R), (int)((f64) w·1.5 + R))` with `R = RadarHatotav2[owner(q)][q.+0x6e]` (f32 widened),
  `a = rnd(5, 1, 360)·3.141592/180` (f64), `x = (f32)(cos a · r + q.x)`, `y = (f32)(sin a · r + q.y)`; the point
  fits when no planet 1..=`HanyBolygo` has `(x − px)² + (y − py)² < 400` (f32) and `st_IsInRR((int) x, (int) y, 3)`
  (the f32 values truncated; the ELF truncates the f64) is false; planet `p` moves there.
- **`st_bGetLegerosebb()`** (0x6038a0): over the human's planets with `+0xd8[EN] > 0`: per building slot
  1..=`HanyEpuletLefoglalva[p]` (exe 0xBA9840; `Epuletek[p]` 0xBA9E90, 0x20 bytes) whose kind is non-zero and `+8 ==
  0`: `sum += 1`, plus `3·kind − 0x5b` for the kinds 0x20..=0x23; after every slot (empty ones too) `best < sum` →
  `best := sum`, pick `p` (`best` runs over all planets, `sum` per planet). 0 when none.
- **The supernova planets** (`StoryInit_GenerateSupernovaBolygok_de_nem_kell_elore_meghivni(k)`, exe 0x5c5230): ids
  8010/8011 (`k`: 8301/8302) searched with mode 1, either missing → abort; both `st_bUnhide`, both
  `st_SetBolygoTulaj(·, 16, 11)` (race 11: `Generate_Planet(p, 16, 11, 1, 0, 0, 0)` when free, else `BolygoElfoglal(p,
  16)` and `+0x21 := 11`), the first `+0x28 := 14800`, `AutoAddBuilding` of both ids, `st_bSetExtra(−first, 1)`.
- `st_AddMoney(x, e)` (0x5c9e40): `FUN_0053fb10(e > 0 ? e : EN, −1, (f64) x)`; `st_GlobalMoralValt(x)`: global
  morale `−= 10·x`, speed 6.

Exe vs ELF: the exe calls `st_Story`/`st_StoryStart` and inlines `st_Counter`, `st_GrWarPart`, `st_SpWarPart(0)`,
`st_ArrivedS`, `st_RangeB`, `st_Olvas`, `st_bSetRadarMsg`, `st_FlottaAll`, `st_SetBolygoTulaj` and
`st_GlobalMoralValt` with the same results; 842's design test (the whole byte) and `st_SetBolygoCoord`'s truncation
differ (above). Quirks kept: 864 arms counter 8460 but tests variable 8460; the counters 8411, 8642, 8643 and 8319 are
tested by nothing; 841 never restores tech 6; a `StartStory` of a running story inside these blocks makes it
`StoryLastChecked`, so the block's `WIN(0)` then ends that story (§8.9's `S`).

### 8.15 `st_Fable_include` (exe 0x5e6ba0, ELF 0x770200): the random fables
Ported in `engine/crates/ig2-sim/src/fable.rs` ([`FableStories`], run after `st_BRnd_include`, before `st_Scen_include`), verified at
runtime (`traces/2026-10-16-fable.md`). Fifteen of the random stories story 400 draws from `RndStoryk` (§8.3):
301..305 the traders (the story merchants of empire 9, fleet ids 10000..=11000), 307 a wormhole map, 308 a spy
network, 309..311 an agent's offer (they start 404, the refit trader), 312..316 small planet episodes; 306 has no
block. Each block runs while its story does; an abort (`throw CStoryWinException`) is caught per block
(`WIN(0, 1)`), then the next block runs. The exe inlines `st_Story`, `st_StoryStart`, `StartStory`, the
variable/counter/planet-id accessors, `st_SpWarPart`, the message predicates, `st_fShow`/`Hide`/`SetVillogas`/
`SetBarat`, `st_fNewPos`, `st_fAttackFleet`, `st_AddFleetToSpWar` and `st_ShowAnim`, and **drops every
`st_oldLetilt` money check** of the ELF (305, 309, 312 disable no answers). Ids are decimal (`v(3011)` =
variable 0xbc3); `c3` = campaign 3, whose messages are the ids + 1000. Notation as in §8.9 (`M`, `A`, `P`, `C`,
`S`), plus `R(b)` = `st_Olvas(b)` (flag 9, `StoryMessageBlokk`); `P_k` = the finished battle's participants
(`SpwFlottak`, `n` of them; owner `+0x18`, fleet slot `+0x20`, story id `+0x24`, name `+0x00`); `own(e)` =
flag 3 and `P_0` or `P_1` (or `P_2` when `n > 2`) is empire `e`'s; `lost(id)` = `st_SpWarLose(id)` (0x602c20:
flag 3, the participant with story id `id` — among the first two when `n < 3`, else all — and `SpWarKiNyert`
0xBAA1EC ≠ its owner); `F[f]` the fleet record; `trader(f)` = story id 10000..=11000. Fleet bits: `+0x18`
bit 0 hidden/has traded, bit 1 leaving; `+0x52` 0x80 blinking, 0x100 always visible, 0x200 friendly, 0x800
custom name.

| Story | What it does |
|---|---|
| 301 | flag 3, `own(EN)`, `own(9)`, `v(3011) == 0`: the first participant `k` with a trader id; then 100 draws `f = st_fSearch(−9)` (stream 5); `f` counts when it is a trader, not `P_k`'s slot, and `d = st_fDistFromBir(f) > 60` (`d` only computed then); the smallest `d` (first of equals) wins. None → `WIN`. Else `v(3011) := 1`, `M(c3 ? 4011 : 3011)`, param 1 = `F[f]`'s name, param 2 = `P_k`'s name, `+0x52 &= ~0x200` (`st_fSetBarat(−f, 0)`), `|= 0x800`, `+0x18 &= ~1`, `+0x16 := 3011`, `AutoAddship(f, 3011)`, counter 3011 := 10, and the chase runs at once. Chase (`C(3011)`, `st_SpWarPart(3011)` or just set up): counter 3011 := 10; `f = st_fSearch(3011)`: none → `WIN`; `g = st_fGetNearest((int) x, (int) y, EN)`: `g` → `FlottaMozgat(f, 2, 2, g, 0)`, else `st_fEliminate(3011)`, `WIN` |
| 302 | start: 304 running → `WIN(0, 0)`. Else up to 100 draws `f = st_fSearch(−9)` until a trader (the 100th draw is not tested: `WIN`); `M(c3 ? 4021 : 3021)`, param 1, `+0x52 |= 0x800`, `v(3021) := +0x16`. `lost(v(3021))`: `st_fSearch(v(3021))` → `FlottaDelete`; `own(EN)` → `M(c3 ? 4022 : 3022)`, else `WIN`. `R(4022)` or `R(3022)`: the human +24,000, `WIN` |
| 303 | start: counter 3031 := 10. `C(3031)`: the nearest (`st_fDistFromBir`, first of equals) fleet of empire 9 with a trader id and `+0x18` bit 0 clear over the fleets 1..=n; none or `d > 79` → counter 3031 := 10; else `v(3031) := +0x16`, `v(3032) := (int) +0x44` (its target), `+0x16 := 3031`, `+0x52 |= 0x200, 0x800, 0x100`, `FlottaMozgat(f, 0, 0, 0, 0)`, `M(3031)`, param 1, counter 3032 := 240. A fleet of the human's arrives at it (flag 0, type 2, target id 3031, `F[ErkezettFlottaNumber]` the human's) or `C(3032)`: `f = st_fSearch(3031)`, `+0x16 := v(3031)` (inlined **without** touching `StoryError`), `+0x52 &= ~0x200, ~0x800, ~0x100`, `FlottaMozgat(f, 1, 1, (f32) v(3032), 0)`; not `C(3032)` → the human +10,000, `M(3033)`; else `M(3032)`; `WIN` (a missing fleet writes slot 0) |
| 304 | start: 305 running → `WIN(0, 0)`. Else 302's draw; `M(c3 ? 4041 : 3041)`, param 1, `+0x52 |= 0x800`, `+0x16 := 3041`. `lost(3041)`: `own(EN)` → `M(c3 ? 4042 : 3042)`, the human +36,000; `WIN` |
| 305 | start: the nearest trader with bit 0 clear (303's search, no distance limit); none → `WIN`; else `M(3051, 100)`, param 1, `v(3051) := +0x16`. `v(3052) == 0` and `lost(v(3051))`: `KiszedUtolsoUzenet(3051, 0)`, `WIN`. `A(3051, 3)`: `v(3052) := 1`, the human −14,000, `StoryAdat+0x28 := st_fSearch(v(3051))`, undo `(14, 0, 305)`, `+0x52 |= 0x800`, counter 3051 := 1440. `A(3051, 4)`: counter 3053 := 60. `A(3051, 5)` or `P(3051)`: `WIN`. `C(3053)`: `M(3051, 100)`, `f = st_fSearch(v(3051))`, none → abort, param 1. `C(3051)`: `t = StoryAdat+0x28`: 0 → `WIN`; `F[t]` bit 0 clear → counter 3051 := 10; else `+0x52 &= ~0x800`, `+0x28 := 0`, `M(3052)`, param 1 = `F[t]`, `WIN`. Daily, `t ≠ 0` and `F[t]` no longer empire 9's: `+0x28 := 0`, `WIN` |
| 307 | start: over the wormholes `w` = 1..=`OsszHanyWormhole` (0xBB61A0; records 0x1c B at 0xBB60F8: `+8` end A known, `+0xa` i16 its fleet, `+0x14` end B known, `+0x16` its fleet) with an end unknown and both fleets > 0, one `RndValasztas(5, 1)` each after `RndInitValasztas` (reset inside the loop: `rnd(5, 1, 1)`, always taken), so the last such `w` wins; none → `WIN`; `v(3071) := w`, `M(3071, 100)`. `A(3071, 4)`: the human −12,000; `w = v(3071)`: both ends known, both fleets `+0x52 |= 0x100`; `WIN`. `A(3071, 3)`, `A(3071, 5)` or `P(3071)`: `WIN` |
| 308 | start: `e = GetBarmilyenBirodalom(1, 3, 5)` (0x5c24e0: `e = rnd(5, 1, 8)`, stepped `e mod 8 + 1` past 1, 3, 5, `EN` and dead empires; the 10th miss aborts); `v(3081) := e`, `M(3081, 100)`, param 1 = `e`'s name. `A(3081, 2)`: the human −2,000; `e = v(3081)`; up to 100 draws `p = st_bSearch(−e, 0)` (stream 5) until four planets: a `p ≠ 0` without info −3081 gets id `3085 − k` (`k` = 4..1: 3081..3084), info −3081 (`BolygoInfoAdd(p, −3081, 1)`) and radar `+0x6e := 4`; counter 3082 := 90. `A(3081, 3)` or `P(3081)`: `WIN`. `C(3082)`: planets 3081..3084 (searched, mode 1) lose info −3081 and the radar (`+0x6e := 0`); scrolling text `_s(5, 3081)` (`ScrollUzenet(…, 1, 0, 0)`); `WIN`. Daily: the four that are no longer `v(3081)`'s lose them |
| 309 | start: `v(3099) ≠ 0` → abort; `v(3099) := 1`. Start or `C(3091)`: `M(3091, 120)`. `A(3091, 2)`: the human −20,000, `S(404)`, `WIN(0, 1)` (a running 404 is `StoryLastChecked` then and ends instead). `A(3091, 3)`: counter 3091 := 90. `A(3091, 4)`: `WIN` |
| 310 | start: `v(3099) ≠ 0` or the human has more than 11 spies (`OsszHanyKem[EN]` 0xBB1F84) → abort; `v(3099) := 1`; `M(3092, 120)`, `v(3101) := 0`. `A(3092, 2)`: `k = Spy_New(EN)` (< 1 → abort), the record below written over it, name `_s(5, 3092)`, `+0x16 := 1`, `+0x3c := rnd(5, KemRangXPHatar[r], KemRangXPHatar[r + 1])` with `r` = `+0x40` (1); `Story_check_locked := 1`, `KemJelentesAdd(2, k, 0, 0, 0, 0, EN, 0…)` (the report, `info-messages.md` §3.3; its `Story_KemEsemeny` sets flag 15 and `KemkedesTipus := 3` for the rest of this check, its `Story_Check` returns at once), unlocked; `S(404)`, counter 3101 := 180, `v(3101) := 1`. `C(3101)`: spy 0 of the human alive (`+0x10 > 0`) with that name → its `+0x10 := 0`; else every spy 1..=n alive, with `+0x16 ≠ 0` and the name. The agent killed (flag 15, `KemkedesTipus == 3`, `KemEsemeny` kind 1 and `b == 1`, spy `a` has the name): `M(3093)`, `st_fEliminate(4041)`, `WIN(0, 1)`, `WIN(404, 1)`. Daily, `v(3101) ≠ 0`, no spy 0..=n alive with the name: the same without the message. `A(3091, 3)` (309's message, a slip kept): `WIN` |
| 311 | start: 309's guard. Start: the human has < 4 planets → `WIN(0, 0)`; else video `_s(6, 3094)`, `M(3094)`, `f = st_fNewEx(_s(5, 4041), 9, 4041, 0.03, 1, 0)` heads for `last_fn_b` (`FlottaMozgat(f, 1, 1, b, 0)`), fleet 4041 `+0x52 |= 0x100, 0x80, 0x200` (each `StoryError` = not found); a pirate `FlottaNew(_s(5, 3111), 10, 3111, x, y, 7.0)` at `x = (f32)(int)(P.x − (P.x − (f32) last_fn_x)·2)`, `y = (f32)(int)((f32)(2·last_fn_y) − P.y)` (f32; `P` = planet `last_fn_b`), `AutoAddship(·, 3111)`; 3111 attacks 4041 (`FlottaMozgat(a, 2, 2, b, 0)` when both exist); `v(3112) := 0`. Flag 1 with 3111 and 4041 the two fleets of `spFlottak` (0xBAA1C6/0xBAA1CC): 4041 missing → abort; every fleet slot `k` = 1..=**`OsszHanyHajo`** (the ship count: a slip kept) of the human's within `(int)(f32) sqrt((f64)(f32)(dy² + dx²)) < 10` of 4041's truncated position joins (`spFlottak` count 0xBAA1E8 below 4: entry `(k, 1)`), `StoryUrharcViszony[9] := 1`, `StoryAdat+0x88 := 1`, `+0x94 := 1`. `st_SpWarPart(3111)`: 3111 gone: 4041 gone → `WIN`; else 4041 `+0x52 &= ~0x80, ~0x100`, `M(3095)`, `v(3112) := 1`. 3111 there, 4041 gone: `st_fEliminate(3111)`, `WIN`. Daily, `v(3112) ≠ 0`, 4041 gone: `KiszedUzenet(3095)`, `WIN`. `A(3095, 2)`: `S(404)`, `WIN(0, 1)`. `A(3095, 3)`: `st_fEliminate(4041)`, the human +13,500, `WIN` |
| 312 | start: `p = st_bSearch(−EN, 0)` gets id 3121 (when non-zero), `M(3120, 100)`, param 1 = `p`. `A(3120 or 3121, 2)`: the human −2,000, info 3120 on planet 3121 (`BolygoInfoAdd(b(3121), 3120, 1)`), `WIN`. `A(3120 or 3121, 3)`: counter 3120 := 70. `A(…, 4)` or `P(…)`: `WIN`. `C(3120)`: `M(3121, 50)`, param 1 = planet 3121 |
| 313 | start: `p = st_bSearch(−EN, 0)` (none → abort) gets id 3130 and info 3130, `M(c3 ? 4130 : 3130)`, param 1, `WIN` |
| 314 | start: as 313 with id 3140 (no info), `M(c3 ? 4140 : 3140)`, param 1, `st_bShow(3140, −1)`, counter 3140 := 30. `C(3140)`: planet 3140 still the human's → `M(c3 ? 4141 : 3141)`, param 1; `WIN` |
| 315 | start: counter 3151 := 10. `C(3151)`: the human's lab levels `LaborSzint[EN][1..=3]` (exe 0x912728, i32 `[19][4]`): the first non-zero one with research going on (`ResearchAktFejlesztett[EN] > 0`) → `M(3150)`, `StoryAdat+0x6c −= 30` (the research speed bonus, `production-research.md`), counter 3150 := 150; else counter 3151 := 10. `C(3150)`: `M(3151)`, `+0x6c += 30`, `WIN` |
| 316 | start: up to 10 draws `p = st_bSearch(−EN, 0)` until a planet with a radar (`+0x6e ≠ 0`); `p == 0` or the 10th draw → `WIN`; else `M(c3 ? 4160 : 3160)`, param 1, counter 3161 := 360, id 3161, `st_bShow(3161, −1)`, `+0x6e := 2`. `C(3161)`: planet 3161 the human's → `M(3161)`, param 1; `+0x6e :=` 1 if it is the human's, else 0; `WIN` |

Helpers:
- **`st_fDistFromBir(f, e)`** (exe 0x604650): `e = −1` → `EN`; over the planets **1..=`HanyBolygo` − 1** (the last
  planet is skipped) owned by `e` that the human sees (`+0xd8[EN] ≥ 0`): `d = (int)(f32) sqrt((f64)(f32)(dx² +
  dy²))` of the truncated coordinates' i32 differences, the smallest; 1,000,000 when there is none.
- **`st_fGetNearest(x, y, e)`** (0x604970): over the fleets 1..=n (`e == 0`: any live one, else owner `e`) the
  first, or one with `(f32)(dy² + dx²) < (f32) best` (f32 from the int point; `best` = the last pick's truncated
  value); 0 when there is none.
- **`st_fEliminate(id)`** (0x604750): `f = id < 0 ? −id : st_fSearch(id)`; `f < 1` → `StoryError := 1`. `+0x18`
  bit 1 set and `+0x40 ≠ 0` (already leaving) → `StoryError := 0`. Else bit 1 set, `a = rnd(5, 0, 1)·600 −
  300`, `b = rnd(5, −300, 300)`, `(dx, dy) = rnd(5, 0, 1) ? (b, a) : (a, b)`; the centre `(cx, cy)` of the
  human's seen planets 1..=n (each running sum `(int)((f32) sum + coord)`, then divided as integers; none → (0,
  0)); `dx` flips its sign when `50 > (f32) cx − x` (f32 `comiss`) differs from `dx ≥ 0`, `dy` likewise;
  `FlottaMozgat(f, 1, 0, dx, dy)`, `StoryError := 0`.
- `st_fSetBarat(x, on)` (0x605160) and the inlined `st_fShow`/`Hide`/`SetVillogas`: `x < 0` → slot `−x`, else
  `st_fSearch(x)`; a slot ≥ 1 gets the bit, `StoryError` = not found.
- `KiszedUtolsoUzenet(id, 0)` (0x506140), `KiszedUzenet` (0x506200), `ScrollUzenet` (0x556b50), `Spy_New`
  (0x58f830) and `KemJelentesAdd` (0x58d000) are events; `AutoAddship` (0x5c1d80, §8.6), `st_bShow` (0x603f20,
  §8.10) were ported before.

The story spy's record (0x4c bytes, `Kemek`, `espionage.md`): name `_s(5, 3092)` (the rest of +0x00..+0x0e
zero), `+0x10` i16 1, `+0x12` 1, `+0x14` i16 4, `+0x16` i16 100 (then 1), `+0x18` 0, `+0x1a` i16 4, `+0x1c` 3,
`+0x1e` i16 0, `+0x20` i16 0, `+0x24` 1000, `+0x28` 0, `+0x2c` 0, `+0x30` 0x100, `+0x34` 0, `+0x38` 0, `+0x3c`
the drawn XP, `+0x40` 1 (the rank), `+0x44` 0, `+0x48` 0; the exe leaves `+0x0f`, `+0x13`, `+0x19`, `+0x1d`,
`+0x22..+0x23` as its stack held them (the ELF zeroes them).

Exe vs ELF: no `st_oldLetilt` blocks; 303 restores the fleet id with an inlined read that leaves `StoryError`
alone. Quirks kept: 302/304's 100th draw is never tested; 307's `RndInitValasztas` inside the loop (the last
eligible wormhole, one draw each); 310 ends on 309's message 3091; 311 loops the fleets up to the ship count;
309/311's `S(404)` makes a running 404 the story their `WIN(0, 1)` ends; `st_fDistFromBir` skips the last
planet.
