# Campaign scripting: the story engine and the scenario stories

Status: **engine core, the scenario stories, `InitStory`, the inline campaign stories and the storylines
(`st_Line_include`) verified at runtime** (`traces/2026-10-16-story.md`, `traces/2026-10-16-campaign.md`,
`traces/2026-10-16-line.md`), ported in `engine/crates/ig2-sim/src/story.rs`, `campaign.rs` and `storyline.rs`
(`tests/story_trace.rs`, `tests/campaign_trace.rs`); the MP game modes (`st_Multi_include`, §8.10,
`traces/2026-10-16-multi.md`) in `multi.rs`, the random stories 220..239 (`st_BRnd_include`, §8.11,
`traces/2026-10-16-brnd.md`) in `brnd.rs`, the opening episodes (`st_Begin_include`, §8.12,
`traces/2026-10-16-begin.md`) in `begin.rs`, 36 of the 62 stories of `st_Rnd_include` (§8.13,
`traces/2026-10-16-rnd.md`, `traces/2026-10-16-rnd2a.md`) in `rnd.rs` and `rnd/`, the storyline episodes of `st_Elem_include` parts 1, 2a, 2b, 2c and 2d (§8.14,
`traces/2026-10-16-elem.md`, `traces/2026-10-17-elem2a.md` .. `2026-10-17-elem2d.md`) in `elem.rs` and `elem/`, the random fables (`st_Fable_include`, §8.15,
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

Moved to [`campaign/line.md`](campaign/line.md).

### 8.10 `st_Multi_include` (exe 0x5ee4d0, ELF 0x79af20): the multiplayer game modes

Moved to [`campaign/multi.md`](campaign/multi.md).

### 8.11 `st_BRnd_include` (exe 0x5c9f20, ELF 0x768d50): random stories, part 1

Moved to [`campaign/brnd.md`](campaign/brnd.md).

### 8.12 `st_Begin_include` (exe 0x5cec20, ELF 0x7407b0): the opening episodes

Moved to [`campaign/begin.md`](campaign/begin.md).

### 8.13 `st_Rnd_include` (exe 0x5f0e20, ELF 0x74a2c0): random stories, part 2

Moved to [`campaign/rnd.md`](campaign/rnd.md).

### 8.14 `st_Elem_include` (exe 0x5d4b70, ELF 0x77ac00): the storyline episodes, part 1

Moved to [`campaign/elem.md`](campaign/elem.md).

### 8.15 `st_Fable_include` (exe 0x5e6ba0, ELF 0x770200): the random fables

Moved to [`campaign/fable.md`](campaign/fable.md).
