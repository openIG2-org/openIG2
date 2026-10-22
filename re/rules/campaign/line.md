# Campaign scripting §8.9 `st_Line_include` (exe 0x5eb6f0, ELF 0x7771e0): the storylines

Split out of `../campaign.md` (sections, numbering and references unchanged: `campaign.md §8.9` means this file).
Shared terms, the story engine and the notation are in `../campaign.md` §1..§8.8.

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
