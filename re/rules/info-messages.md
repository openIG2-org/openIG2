# Info charts and messages: the statistics history, the scrolling message line and the message inbox

Status: **verified at runtime** for what the recordings reach (`traces/2026-10-16-save-replay.md`): the info
charts every tick of 1,900 (the sample after each load and 17 month changes), the scrolling-message lists with the
year's tax message (twice) and the "enemy fleet spotted" message, the spy reports and messages of a staged spy
recording (§3.3: the report queue with 26 reports, ids 100, 101, 103, 106, 136, 139–141), the battle outcomes of
three staged recordings (§3.2: 11, 12, 15–18, 22–25 from fifteen auto-resolved space battles and ground wars, one
of them a planet battle followed by the winner's landing), the posts 2, 3, 4, 7, 26 of other recordings, the staged
`replay-quick1-messages-x16` (§3.2: 5, 8, 9, 19, 21 and the inbox expiry, §4; `traces/2026-10-23-u25-rest.md`),
byte-exact against `engine/crates/ig2-sim` (`info.rs`, `messages.rs`, `spy.rs`, `game.rs`, `game/battles.rs`,
`tests/save_replay.rs`), and the line's motion (§2.3, `tests/scroll_trace.rs`). Posted by the exe but not replayed:
6 and 10 (`messages-diplo-x16`: the AI's message to the human needs the unported reason codes, `diplomacy.md` §11).
Not traced: 13 (no staged planet had guns), a real-time ground war's 11/12 (ported from the code).

Three per-tick pieces of the strategic tick (`time-and-rng.md` §1) that only feed the UI, but whose state is in
every save (`formats/save.md` §4.3–§4.4):

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Info` | **0x4e59f0** | | every tick after `FRISSIT_Trade`: on a new month, `InfoChartsSample(0)` (§1) |
| `InfoChartsSample(slot)` | 0x4eee00 | | shifts and samples the ten charts of every empire (§1.2) |
| `InfoChartAdd(chart, e − 1, slot, value)` | 0x4eecb0 | | stores a sample and re-smooths the row (§1.3) |
| `CalcSpacePower(f)` | 0x4e4d60 | 0x55d100 | a fleet's space strength (§1.2) |
| `InfoInit` | 0x4eb130 | | from `InitNewGame` 0x4e1130: all chart cells := −1.0, `InfoOldHonap` := −1 |
| `ScrollUzenet(text, id, important, interrupt)` | **0x556b50** | 0x66dc40 | posts to the scrolling line and its history (§2) |
| `ScrollUzenetId(id, p1, important, interrupt)` | 0x556dc0 | | the text of message `id` from `uzenet.txt`, `%1` := `p1` (§3.1) |
| `ScrollUzenetId2(id, p1, p2, important, interrupt)` | 0x557020 | | the same with `%2` := `p2` |
| `FRISSIT_Scroll2` | 0x556210 | 0x66ebd0 | **UI frame**: scrolls the current text, takes the next one (§2.3) |
| `FRISSIT_Messages` | **0x5062d0** (thunk) | | every tick after `FRISSIT_Story`: the inbox's expiry (§4) |
| `Story_MessageElevules(b)` | 0x5c90d0 | 0x73e770 | story event flag 7 with block `b`, `Story_Check` |

## 1. Info charts (`FRISSIT_Info`)

### 1.1 When
`if InfoOldHonap (0xB84C78) != Honap: InfoChartsSample(0); InfoOldHonap = Honap`. `InfoOldHonap` is **not
saved** and `LoadSave` doesn't touch it: after a load it keeps the running session's month (−1 after
`InitNewGame`), so the first tick after loading a save of another month samples at once.

### 1.2 Sampling
`InfoCharts` 0xB7A000 and `InfoChartsSmoothed` 0xB7E4C0 are `f32 [10 charts][8 empires][55 months]` (0x44c0
bytes each; empire `e` in row `e − 1`; the third index is `slot·…`, only slot 0 is used: 55 cells per row). For
`e` in 1..=8:

1. Every one of the empire's ten rows shifts left by one cell (cells 1..=54 → 0..=53; cell 54 keeps its value).
2. If `BirodalomEl[e]` (0xBAE6AC) is clear: `InfoChartAdd(c, e − 1, 0, −1)` for all ten charts.
3. Else the ten values, each through `InfoChartAdd`:

| Chart | Value |
|---|---|
| 0 | Σ planet `+0x44` (last month's tax) over planets 1..=`HanyBolygo` owned by `e` (`+0x20`) |
| 1 | Σ population `+0x28` over the same planets |
| 2 | the number of those planets |
| 3 | Σ `CalcSpacePower(f)` over fleets 1..=`OsszHanyFlotta` owned by `e` (`+0x15`) |
| 4 | ground strength `g` (below) |
| 5 | techs 1..=192 with status byte `+0x45 + e` == 5 (`Talalmanyok` 0xB9C0C8, 0x114 each) |
| 6 | Σ rank `+0x40` of spies `Kemek[e][1..=OsszHanyKem[e]]` with `+0x16` (applicant) == 0 and `+0x10` > 0 |
| 7 | `BattlesWon[e]` (0xB79FB4 + 4e; the save's `BattlesWon` chunk starts at empire 1) |
| 8 | `s = 0`; for `i` in 1..=18 with `BirodalomEl[i]`: `s += Viszony[e][i].+0x22` (i16), then `s = max(s, 0)` |
| 9 | the score (below) |

`CalcSpacePower(f)` (exe 0x4e4d60, all accumulators `i32`, designs `Tervek[owner of f]`, `d = +0x30 · +0x2c`
(hp · attack, i32)): for each ship `Hajok[1..=OsszHanyHajo]` of fleet `f` with design ≠ 0x7fff:
`acc = (i32)((f32) sqrt((f64) d) + (f32) acc)`; then for each vehicle `Tankok[1..=TankokSzama]` of fleet `f`
with design ≠ 0x7fff whose design has `+0x36` ≠ 0: `acc = (i32)((f32) sqrt((f64) d) · (f32) count + (f32) acc)`.

Ground strength `g` (i32): for each fleet of `e` as above, each vehicle of that fleet with design ≠ 0x7fff whose
design (`Tervek[e]`) has `+0x36` == 0: `g = (i32)((f32) sqrt((f64) d) · (f32) count + (f32) g)`; then the same
for every planet slot 1..=**199** (the whole `Bolygok` array, not `HanyBolygo`) owned by `e`, with the
planet's garrison (vehicles with fleet == −p).

The score (f64, `cvttsd2si`): `(pop/100 + tax/10 + planets·100 + space + ground + techs·10 + spies·10) / 10 +
battles·10` in this order (chart values by name).

### 1.3 `InfoChartAdd(c, r, slot, v)`
`raw[c][r][54] = (f32) v`; then for `i` in 0..47 (`f32` throughout, each product clamped to ≥ 0 by `comiss`, so
NaN passes; `raw[i]` itself is clamped too):
`smoothed[c][r][i + 4] = (((((((2·raw[i+1] + raw[i]) + 4·raw[i+2]) + 5·raw[i+3]) + 5·raw[i+4]) + 4·raw[i+5])
+ 2·raw[i+6]) + raw[i+7]) / 24`. Smoothed cells 0..=3 and 51..=54 are never written.

## 2. The scrolling message line (`ScrollUzenet`)

### 2.1 State
| Global | exe | Saved | Meaning |
|---|---|---|---|
| `ScrollUzenetek` | 0xE40CB0 | `(db + 1) · 0x40` | entries 1..=`db` (0x40 each): text (NUL-terminated, ≤ 53 bytes), `+0x38` i32 id, `+0x3c` u8 important |
| `UzenetekDb` | 0xE413D4 | yes | `db` ≤ 10 |
| `ScrollUzenetekHistory` | 0xE40F70 | `(dbh + 1) · 0x40` | entries 1..=`dbh`: text, `+0x3c` important (`+0x38` never written) |
| `UzenetekDbHistory` | 0xE413D8 | yes | `dbh` ≤ 10 |
| `ScrollSzoveg` | 0xE41230 | no | the text scrolling now ("" = none; 0xc8 bytes) |
| `ScrollFontos` | 0xE412F8 | no | its important flag |

Entry 0 of each list is never written. Copies go up to the NUL: the bytes after it keep what the slot held.

### 2.2 Posting
`ScrollUzenet(src, id, important, interrupt)`; nothing during the tutorial (`0xF6915D` or `0xF66F2A` set):
1. `t` = the first 53 bytes of `src` (`strncpy` 0x3c, then cut at 0x35).
2. History: if `dbh > 9`: `dbh -= 1` and entries 2..=`dbh + 1` move to 1..=`dbh`. `dbh += 1`; entry `dbh` :=
   `t`, `+0x3c` := `important`.
3. Duplicate: `t` equals `ScrollSzoveg` or the text of one of entries 1..=`db` → stop.
4. If `important && db == 10`: `db = 9` (the last entry is dropped); else if `db > 9`: stop.
5. `n = db + 1`, `slot = n`. If `important`: for `k = db` down to 1 while entry `k`'s `+0x3c` == 0: entry
   `k + 1` := entry `k` (all 0x40 bytes), `slot = k`. (An important message goes after the last important one.)
6. Entry `slot` := `t`, `+0x3c` := `important`, `+0x38` := `id`; `db = n`.
7. If `ScrollFontos == 0 && important`, or `interrupt`: `ScrollSzoveg[0] = 0` (the current text is cut off).

### 2.3 The UI side (`FRISSIT_Scroll2` exe 0x556210, not the sim)
`Fake_FRISSITES` calls it at 0x4de8ed inside its **10 ms loop** (100 Hz, after `Frissit2_Colony`, before
`FRISSIT_Start2`) when `AktKep < 0xE`: one deterministic step per 10 ms, no frame time. The state is UI state
(unsaved): `ScrollSzoveg` 0xE41230 (the text), `ScrollFontos` 0xE412F8, `UzenetStart` 0xE412FC, `UzenetSebesseg`
0xE41300, `UzenetHatar` 0xE41304 (f32, in characters), the error line `HibaUzenetSzoveg` 0xE41308 / its counter 0xE413D0.

Gate (else nothing moves): not in `IOMode && IOAzonosito == 1` (0xBAE2C3/0xBAE2C4), `Belep` 0xBAE24C, `Atlepes`
0xBAE24D; an error line (`ScrollHibaUzenet`) counts up to 300 instead and is then cleared; and the screen test (ELF
0x66ebd0; the exe tests `help_new_available(AktPlanet)` where the ELF has `GetWarfareStatus`): `AktKep` 10, 0xB in SP
or 0xD, or no pop-up (`VanUzenet`, exe 0x509e00), or (0xB, the colony at war, the options in load-only or battle
mode) without `DEBUGX`, or 10 with choices; and not (`AktKep == 3 && StoryAdat+0xb4`).

The step (asm 0x55638f..0x556688; `len = strlen(ScrollSzoveg)`, `c` = the f64 at 0x859a78 =
`0x3f8de69ad42c3c9e`, one ulp below 0.0146):
1. If `(f32)len <= start` or the text is empty: **pop**. With `db < 1` the text is cleared (nothing moves).
   Otherwise `ScrollSzoveg` := entry 1's text, `ScrollFontos` := entry 1's `+0x3c` byte, `start = −53.0f`,
   `speed = (f32) sqrt((f64)(len' + 53) · c)`, `limit = −((f32)(f64)(53 − len') · 0.5f)` (`len'` the new text's
   length), and unless entry 1's `+0x38` id is 1, 0x14, 0x15 or 0x1A it plays `SOUNDS/SCROLL/BRUTAL/%04d.WAV` (`EN`
   == 5) or `…/HUMAN/%04d.WAV` (when the sound switch 0x919930 is on); entries 2..=`db` move to 1.., `db −= 1`.
2. If the text is not empty, it moves:
   - `limit > start` (arriving): `start += speed` (f32); `speed = (f32)((f64)speed − c)`; if `0.02 > (f64)speed`:
     `speed = 0.02f`; if `start >= limit`: `start = limit`, `speed = 400.0f`.
   - `start == limit` (holding): `speed −= 1.0f`; when it reaches 0.0: `speed = 0.2f`, `start = (f32)((f64)start +
     0.01)` (400 steps = 4 s, then it leaves).
   - else (leaving): `start += speed`; `speed = (f32)((f64)speed + c)`; the next step after `(f32)len <= start` pops.
So the main list drains at the display's pace: the sim posts, the UI pops. Port: `messages::Motion`
(`begin`/`advance`) and `Ticker::scroll` (the pop and the step; returns the popped id for the sound); the client's
`ui::Scroller` steps with `Motion`. Verified bit-exact over 5,012 exe steps (`rules/traces/2026-10-23-u25-rest.md`).

## 3. Which messages the tick posts

### 3.1 Texts
`ScrollUzenetId(id, p1, imp, intr)` reads `uzenet.txt` (`dataraw/text/`, localised: raw bytes in the game's code
page) line by line until `atoi(line) == id` (the first such line; no line → no message), takes the line from
byte 5 on (after `NNNN `), replaces the first `%1` with `p1` (`ScrollUzenetId2`: then the first `%2` with `p2`)
and calls `ScrollUzenet(text, id, imp, intr)`. Numbers are formatted with `%d`. Parameters: a planet's name (the
record's first bytes), a tech's name (the tech record), an empire's name (`BirodalomNev` pointers 0xBB6730 + 4e,
unsaved), or "".

### 3.2 The posting sites
Every site posts only for the human (`EN` 0x912A3C) as stated; `imp`/`intr` as passed.

| Id | Site (exe call) | Condition | `%1` | imp, intr | Port: event |
|---|---|---|---|---|---|
| 2 | `FRISSIT_Research` 0x51e1ab | `EN`'s project done, before `SetIdoLelassit(3)` | tech | 0, 0 | `ResearchEvent::Completed` |
| 3 | `BirodalomMeghal(e)` 0x53b651 (`diplomacy.md` §12) | `e ≤ 8`, `e ≠ EN` | empire `e` | 0, 0 | `DiploEvent::EmpireDied`, `StoryEvent::EmpireDies` |
| 4 | `FRISSIT_Starmap` 0x595ea2 (SP), 0x595f39 (MP) | the human's fleet lost its target from view | "" | 0, 0 | `StarmapEvent::ContactLost`, `ContactLostNetwork` |
| 5 | `FRISSIT_Planet` 0x47738f | the human's planet has no buildings, before `BolygoElfoglal(p, 0)` | planet | 1, 0 | `SideEvent::Abandoned` |
| 6 | `DiplomaciaMegtamadas(a, b)` 0x4bda34 (`Id2`) | (`BirodalomIsmeri[EN][a]` or `[EN][b]` or `DEBUGX` 0xBAE587) and `a, b ≤ 8` | empire `a`, `%2` empire `b` | 1, 0 | `DiploEvent::War` |
| 7 | `BolygoElfoglal(p, n)` 0x53ba72 | `n == EN` got a building's tech | tech | 0, 0 | `SideEvent::TechCaptured` |
| 8 | `FRISSIT_Planet` 0x475644 | unrest counter > 30 (morale < 200), owner `EN`, before `BolygoFellazad` | planet | 1, **1** | `ColonyEvent::Rebellion` |
| 9 | `FRISSIT_Planet` 0x47561f | counter < 30, `% 5 == 0` (morale < 200), owner `EN` | planet | 1, 0 | `ColonyEvent::UnrestWarning` |
| 10 | `ElkuldUzenetMI(a, b)` 0x4c0bb3; `MULTIGET_Diplomacy` 0x4c7a2e | a message to `EN` | empire `a` | 1, 0 | `DiploEvent::Message { to: EN }` |
| 11 / 12 | `GroundWarIndit(f, p)` 0x59c510 / 0x59c5cb | auto-resolved, not lopsided, the attacker won (`f.+0x5c > 0` after `Simulate`): 11 when `EN` owns `p` (before the capture), 12 when `EN` owns `f` and `p ≠ StoryAdat+0x40` | planet | 1, 0 | `BattleEvent::Ground` |
| 13 | `FRISSIT_Planet` 0x477009 | `EN`'s spy satellite over the planet removed (stream 3 roll) | planet | 0, 0 | `ColonyEvent::SpyExpelled` |
| 14 | `FRISSIT_Starmap` 0x595569 | an unseen enemy fleet attacks the human (`fleet-movement.md` §6) | "" | 0, 0 | `StarmapEvent::EnemyApproaching` |
| 15 / 16 / 17 / 18 | `SpaceWarIndit(f1, f2, p)` 0x5a7ad5 | auto-resolved (`SimulateSpBattle`) with `EN` owning a participant or the battle planet (also when the AI plays `EN`): lopsided in SP 15 won / 16 lost, else 17 won / 18 lost (`SpWarKiNyert == EN`) | "" | 0, 0 | `BattleEvent::Space` |
| 19 | `BolygoFellazad` 0x53be88 | the rebels join `EN` (after `BolygoElfoglal`) | planet | 1, 0 | `SideEvent::Rebelled` |
| 20 / 21 | `FRISSIT_Planet` 0x47769c / 0x4776e4 | 1 January, `TAX_LAST[EN]` / `TRADE_LAST[EN]` > 0 | the amount | 0, 0 | `SideEvent::YearClosed` |
| 22 / 23 / 24 / 25 | `GroundWarIndit(f, p)` 0x59c5cb / 0x59c572 / 0x59c5a8 / 0x59c5cb (12, 22 and 25 share one call) | auto-resolved, lopsided (`ground-combat.md` §2): won — 23 when `EN` owns `p`, then 22 when `EN` owns `f`; lost — 24 to the owner, 25 to the attacker (each guarded the same way) | planet | 1, 0 | `BattleEvent::Ground` |
| 26 | `FelfedezBirodalom(e, true)` 0x4c1def | `1 ≤ e ≤ 8`, `e ≠ EN`, new contact | empire `e` | 1, 0 | `DiploEvent::Discovered` |

The battles run inside the starmap's movement loop (`fleet-movement.md` §4.3; `Game` pauses the loop there and runs
`SpaceWarIndit` / `GroundWarIndit`, `game::battles`). The real-time battles: a real-time ground war's end
(`GroundWarFinished(p, s, false)` exe 0x47c5d0, `ground-combat.md` §6.1) posts, when the planet changes hands
(`owner != s`) and before the capture (`BolygoElfoglal(p, s)`, `BattlesWon[s] += 1`): 11 when the old owner is `EN`
(0x47c982), then 12 when `s == EN` and `p ≠ StoryAdat+0x40` (0x47c9ad), both with the planet's name, 1, 0
(`GroundEvent::Taken` → `BattleEvent::messages`); with `sim` true it posts nothing (the simulated war's posts are
`GroundWarIndit`'s). A real-time space battle posts **nothing** at its end: `SpwC::SpaceWarVege` (exe 0x5706f0) only
sets `SpWarKiNyert`, clears `SpWarVesztesVisszaVonult` and runs `Story_SpaceBattleFinished`; `SpaceWar_Kilep` (exe
0x562b50) calls it (SP) or sends 0x44d (MP); the ids 15–18 belong to `SpaceWarIndit`'s simulated branch only. The espionage
posts are §3.3. The UI side
(§2.3) isn't the sim's either: no recording shows the line draining. The story
scripts call `ScrollUzenet` directly (§3.4). `LoadSave` posts one message after a load (in the
save before the replay starts).

### 3.3 Espionage: the spy reports and the victim's messages
`FRISSIT_Spy` (exe 0x584b30, `espionage.md`) reports to its owner's spy screen through **`KemJelentesAdd(kind, a,
b, c, d, x6..x14)`** (exe 0x58d000; inlined three times in `FRISSIT_Spy` with the same effect), always guarded by
"the receiving empire is `EN`":
1. `KemEsemeny` data 0xF2F874.. (unsaved): `kind, a, b` i32, `c` u8 at 0xF2F880, `d, x6..x14` i32; then
   **`Story_KemEsemeny()`** (`campaign.md`: SP only, `KemkedesTipus` := 3, `Story_Check`).
2. The queue `KemBeszolasok` 0xBAF338 (saved, `(n + 1) · 0x30`; entry `k` at `+k·0x30`, entry 0 unused), count
   `KemHanyBeszolas` 0xBAF334 (saved): if `n ≥ 50`: `n -= 1` and entries 2..=`n + 1` move to 1..=`n` (`memcpy`).
   `n += 1`; entry `n`: `+0` u8 `kind`, `+4` i32 `a`, `+8` u8 `b`, `+9` u8 `c`, `+0xa` u8 `d`, `+0xc..+0x2c` i32
   `x6..x14`. Bytes `+1..+3` and `+0xb` keep what the slot held (the inlined copies write `+8` as a u16 0).
3. If `kind ≤ 6` and `b ≠ 2`: `ScrollUzenetId(b == 4 && c == 0 ? 100 : 100 + kind, "", 0, 0)`.

The spy screen pops the queue (`AktKep` 7, `re/screens/spy.md` §4; with it open, `FRISSIT_Spy` also moves its cursor
`AktKem` 0xBB2384 and the fate function skips dead slots); the port pops it with the order `spy::Order::Acknowledge`
(the queue is synced state).

The reports (`e` = the spy's owner, `k` its slot, `t` the target, `m`/`p1`/`p2` its mission and parameters, `s` the
state just set; `KemKuldetesAtmenet` outcomes `o`: 1 lost, 2 caught, 3 spotted, 4 done):

| Kind | Site | To | `a` | `b`, `c`, `d` | `x6..x14` | Post |
|---|---|---|---|---|---|---|
| 0xc | rank up (§3.1), inlined 0x584c5d | `e` | `k` | 0, 0, 0 | `x8` = new rank | – |
| 0x13 | double agent exposed (§3.3) 0x584edd | `e` | `k` | 1, 0, 0 | `x7` = master | – |
| 3 | caught by a human captor, after `Story_Special(0xb)`/`FelfedezBirodalom`, inlined 0x585531 | `t` | defender slot | 0, 0, 0 | `x7 = e`, `x8 = k` | 103 (0x67, inlined 0x5855cd) |
| 4 | the "lost" outcome (spotted, `≥ 3` empires know it) 0x5853b6 | `t` | defender slot | 0, 0, 0 | `x7 = e`, `x8 = k` | 104 |
| 1 | the catch check's outcome (after the state) 0x58563d | `e` | `k` | `o`, 0, `s` | `m, t, p1, p2` | 100 for `o` 4, 101 for 1/3, none for 2 |
| 7 | the same, the spy being `t`'s own double agent (`master == t`) 0x58568e | `t` | 0 | `o`, 0, `s` | `m, t, p1, p2` | – |
| 0xf…0x12 | `DontottAKemSorsarol(t, fate, e, k)` 0x584350 (AI captor) | `e` | `k` | 0, 0, 0 | `x7 = t` | below |
| 1 | a story mission's catch check said "caught" (state `KemKuldetesAtmenet[1]`) 0x585027 | `e` | `k` | 1, `flag`, `s` | `m, t, p1, p2` | 101 |
| 1 | mission result (§5.13) 0x586806 | `e` | `k` | 4, `ok`, `s` | `m, t, p1', p2, r0..r4` | 100 failed, 101 done |
| 7 | the same, to the spy's master 0x586841 | master | 0 | 4, `ok`, `s` | `m, t, p1', p2, r0..r4` | – |
| 1 | a story mission's result 0x58587c | `e` | `k` | 4, `ok`, `s` | `m, t, p1, p2, 0, 0` | 100 / 101 |
| 6 | assassination: the victim's owner, inlined 0x5864fe | `t` | victim slot | 0, 0, 0 | `x7 = e`, `x8 = k` | 106 (0x6a, 0x5865b2) |
| 2 | a recruit becomes an applicant (§6) 0x5869ba | `e` | new slot | 0, 0, 0 | `x7 = e` | 102 |

Fates: 1 (released) kind 0xf, post 101 (0x65, tail jump 0x5843d0); 2 (turned) kind 0x10 and post 113 (0x71, tail
jump 0x58444f) when `Spy_Copy` succeeds, else kind 0x11; 4, 6 (executed) and a failed 2 kind 0x11 and post 106
(0x6a); 3, 5 kind 0x12 and post 101. All `""`, 0, 0. The AI captor draws its fate as `rndlog(4, 2, 4)` (2..=4, then
5/6), so fate 1 only comes from a human captor's verdict (`MULTIGET_Spy`), whose report goes to the spy's owner:
in SP an AI, so the human never sees kind 0xf. The captor's loyalty roll reads `Kemek[t][k]` with the *spy's*
slot `k` (quirk kept, `espionage.md` §4): a `k` past `OsszHanyKem[t]` reads bytes the save doesn't hold (stale
memory in the exe; the port reads what its state holds there, zero after a load).
The mission result's id follows step 3: kind 1 with `b = 4` posts 100 when it failed and 101 when it succeeded;
104 ("We have killed an enemy spy!") is only the kind-4 report of an enemy spy the human identified and lost. `flag` is the pass's last "slipped"/success flag (the local
`FRISSIT_Spy` reuses; an uninitialised stack byte before the first): only the story-caught report shows it.
`p1'` and `r0..r4` (the result's locals `local_1c`, `local_24/30/34/48/50`): planet report `p1'` = the planet
reported when `p1 == 0` found none unseen (else `p1`), `r0, r1` the planets; tech report `r0..r2`; empire report
planets, population, ships, techs; money `r0` = the amount (also the failed `x < 1`); sabotage planet, slot,
building type; ship slot, hull tech; tech theft `p1'` = the tech (no `r`); rebellion planet, `BolygoFellazad`'s
result; assassination the victim's rank; kidnap the family member; `r4` = 997 target dead, 998 lost duel, 999
nothing to do.

The mission's victim, when it is `EN` (`t == EN`, after the effect; `ScrollUzenetId(id, %1, 1, 0)` at 0x58675d):

| Id | Mission | `%1` (`%2`) |
|---|---|---|
| 135 (0x87) | 5 money stolen | "" |
| 136 (0x88) | 6 sabotage: `ScrollUzenetId2` 0x586077 | the building's tech (`EpuletekLeiro[type].+0`), `%2` the planet |
| 137 (0x89) | 7 ship destroyed | the hull tech |
| 139 (0x8b) | 9 rebellion | the planet |
| 140 (0x8c) | 10 assassination, after the kind-6 report | "" |
| 141 (0x8d) | 11 kidnap | `KemCsaladtag_Nev[x]` (0xBB6968, `char *[6]`, unsaved, localised: text group 0x11/3, 10..14, set by 0x4ca65a) |
| 142 (0x8e) | 12 mass unrest | "" |

And 130 (0x82, `""`, 0, 0, 0x585749): `EN`'s spy got home (`espionage.md` §3.4).

### 3.4 The story scripts' lines
Nine calls of `ScrollUzenet(text, 1, imp, intr)` (exe 0x556b50) in the scripts, found as every reference to it
(Ghidra `FindRefs`; the other three callers are `ScrollUzenetId` 0x556fcb, `ScrollHibaUzenet` 0x5572eb and `LoadSave`
0x5440aa). Each posts for whoever plays `EN` (the scripts run for the human); id 1, interrupt 0. The text is
`_s(5, n)` (exe 0x542ce0: the keyed table 5, `s_story.txt` in the game's language, binary search on `5 << 16 | n`;
**a missing key gives ""**, the empty string at 0x837984), formatted where stated with `stringhelper::format`
(exe 0x6b7b40, ELF 0x897a70: `vsnprintf` into a string grown until it fits, i.e. plain C `printf`) or `sprintf`
(0x8353e4). The texts use only `%s` and `%d` (12 and 5 times in `s_story.txt`, every language).

| Call | Story (script) | Text | Arguments | imp |
|---|---|---|---|---|
| 0x5cc200 | 230 (`st_BRnd`, `brnd.md`) | `_s(5, 2302)` | | 1 |
| 0x5d0961 | 51 (`st_Begin`, `begin.md`) | `_s(5, 511)` (0x1ff; the story design's key) | | 1 |
| 0x5d2c40 | 19 (`st_Begin`) sabotage | `sprintf(_s(5, 191), tech name, planet name)`, the tech `EpuletekLeiro[kind].+0` | `%s %s` | 0 |
| 0x5da20c | 814 (`st_Elem`) | `format(_s(5, 8140), x)`, `x` = `rnd(5, 100, 200) · 100` | `%d` | 0 |
| 0x5dd71d | 824 (`st_Elem`) | `format(_s(5, 8242), BirodalomNev[v(5208)], planet name)` | `%s %s` | 0 |
| 0x5df5da | 837 (`st_Elem`) | `_s(5, 8371)` | | 1 |
| 0x5e180a | 850 (`st_Elem`) | `format(_s(5, 8501), kept, paid)` | `%d %d` | 0 |
| 0x5e4dac | 860 (`st_Elem`) | `format(_s(5, 8601), kept, paid)` | `%d %d` | 0 |
| 0x5e82eb | 308 (`st_Fable`, `fable.md`) | `_s(5, 3081)` | | 0 |

`s_story.txt` has no key 8140 (that number is `gameinfo.txt`'s), so 814's line is always `""`: §2.2 puts it in the
history and, with the line empty, drops it as a duplicate of the empty line. The names are the save's bytes
(`Talalmanyok[t]`, `Bolygok[p]`) and the unsaved `BirodalomNev` (`Game::empire_names`). While a tutorial runs nothing
is posted (§2.2).

Port: the scripts emit `StoryEvent::ScrollText { text, important }`, `ScrollSabotage`, `ScrollNumber(s)` and
`ScrollEmpirePlanet`; `messages::story_line` renders them (`Texts::with_story`, `printf`) and `Game::messages_for`
posts them with id 1 in event order. Verified: 574 lines of the campaign recordings exact (all but 308's, which no
recording reaches; `traces/2026-10-22-story-ticker.md`). `Game`'s story tick runs the whole campaign chain now
(`new-game.md` §15), so a campaign-start save replay reaches these lines.

## 4. The inbox expiry (`FRISSIT_Messages`)
`Uzenetek` 0xDA6620 (100 × 0x88: `+0` block id, `+4` u8 kept, `+8` i32 expiry `Ido`), `UzenetekRandomKitol`
0xDABC10 (100 × i32), `UzenetPlaybackek` 0xDA9B40 (100 × 0x54), count `UzenetekSzama` 0xDABDD0. Every tick: the
first `i` in 0..count with `0 < expiry < Ido`, `+4 == 0` and block ≠ `UAktUzenetFajta` 0xDABDBC gets
`Story_MessageElevules(block)` (`StoryMessageBlokk` 0xF26574 := block, story event flag 7, `Story_Check`), then
the three arrays close the gap (entries `i + 1..count` move down one; the last stays duplicated) and the count
drops by one. At most one message expires per tick. Nothing else in the tick writes the inbox (messages arrive
through the stories, diplomacy specials and the lockstep queue). Caller: `Fake_FRISSITES` 0x4deadb (the thunk 0x5053b0)
in the 50 ms loop's strategic branch, after `FRISSIT_Story` (0x4dead6) and before `FRISSIT_Diplomacy` (0x4deae0).
Verified in `replay-quick1-messages-x16` (four staged messages: two expire, one kept, one without expiry).

## 5. Port
`ig2-sim::info::Charts` (§1.2–§1.3) with `InfoWorld` (the inputs), `ig2-sim::messages::{Ticker, Texts,
expire}` (§2, §3.1, §4); `Game::info` runs after trade, `Game::post_messages` maps the tick's events to the
§3.2 postings in event order (the systems emit them where the exe posts; the starmap's arrival events follow its
own), `Game::messages_tick` after the story tick. Unsaved inputs: `InfoOldHonap`, `ScrollSzoveg`/`ScrollFontos`
(UI state, mem), the empire names (`Game::empire_names`), the texts (`GameRules::messages`).
Battles (§3.2): `BattleEvent::Space` / `Ground` carry the outcome, and `BattleEvent::messages(EN)` gives the ids in the
exe's order (the owner's message before the attacker's). Espionage (§3.3): the spy pass emits `SpyEvent::Report` (`spy::Report`: the call's arguments; `Report::messages`
= step 3 and the fate posts) for the human only and `SpyEvent::Struck` where the victim's message is posted;
`Game::spy_hooks` carries out each report's queue entry (`SaveState::push_spy_report`) and `Story_KemEsemeny`,
and a caught spy's `Story_Special(0xb)` + `FelfedezBirodalom`, in event order after the pass (inside it in the
exe; nothing later in the pass reads them). The family names are `Game::family_names` (unsaved, dumped by
`replay.js` as `famnames`). A destroyed ship (mission 7) that empties the human's own fleet makes `HajoDelete`
send lockstep message 0x1b instead of deleting it (`space-combat.md` §5.3): the spy pass's
`SpyEvent::Combat(HumanFleetEmptied)` becomes `GameEvent::FleetEmptied`, and the driver calls
`Game::fleet_emptied(f)` (`MULTIGET_Starmap`'s handler: `FlottaDelete` if the fleet still holds no ships) before
the next tick. The story-caught report's flag byte is the pass's leftover local (0 before the
first spy that sets it; the exe's is uninitialised); the spy screen's cursor moves (`AktKep` 7) are UI.


## 6. The stories' inbox writes
Called from the story scripts inside `Story_Check`; the port applies their `StoryEvent`s in order after the check
(`Game::inbox_events`; nothing in a check reads the inbox back). Entry layout as §4, plus `+5` u8 seen (`Megnezes`
sent), `+0xc` arrival `Ido`, `+0x10 + 0x14·k` parameter `k` (NUL-ended, no length check).
- `NewMessage(id, days)` (exe **0x508fe0**, ELF 0x5a6d20): `SetIdoLelassit(2)` (exe 0x541270), then only while
  `UzenetekSzama < 100`: `UVoltUjUzenet` 0xDA65FF := 1, `LastNewMessage` 0xDABDC8 := n (the new index),
  `UUjUzenetLejarasa` 0xDABDCC := `days == 0 ? 0 : days·50 + Ido`; `UzenetekRandomKitol[n]` := 0; parameters 1..=5 :=
  `"#=#"` (0 kept); `+0` id, `+8` expiry, `+4` = `+5` = 0, `+0xc` `Ido`; count + 1. An automatic kind (below) with
  `AktKep ≠ 10` sets `AktKepKell` 0xBAE250 := 10 (the client opens the messages screen; UI). The kinds:
  `UzenetFajtak` 0xD90DF0 (0x110 each: `+0` id, `+4` conversation, `+5` automatic; count `HanyUzenetFajta` 0xDABE38;
  the inlined lookups take the first match, none reads the slot past the table).
- `SetMessageFajOverride(r)` (0x5094c0): `UzenetekRandomKitol[LastNewMessage]` := r for 1..=8, 13, 14, else 1.
- `SetParamStr(k, s)` / `SetParamInt(k, v)` (0x509570 / 0x509500, `_itoa` base 10): for `k < 6`, the text with its
  NUL into entry `LastNewMessage`'s parameter `k`. `StoryEvent::Message { id, empire }` = `NewMessage(id, 0)`,
  parameter 1 the empire's name, its race as the portrait.
- `KiszedUzenet(id)` (0x506200): the **first** entry with block `id` (read or not) leaves (§4's shift;
  the loop index runs to the end after the move, asm 0x5062a2..0x5062b9, the ELF the same).
- `KiszedUtolsoUzenet(id, _)` (0x506140): from index `UzenetekSzama` (one past the end) down, the first entry with
  block `id`; when its `+4 == 0` the count drops and, below the new count, the rest moves down.
- `LetiltReszlet(part)` (0x5063d0): `Tiltolista[MaxTiltasok++]` := part (0xDABDE8, 20 slots, no bound check).

Gaps: the parameters `MessageText::Star`, `Planets`, `Design`, `Hero`, `Position`, `Participant`, `Spy` aren't
resolved by the sim yet (the slot keeps `"#=#"`).

## 7. The messages screen's orders
The screen's writes to saved state and its story hand-offs. The original makes them in the UI frame without a
lockstep message; the port makes them `MessageOrder`s (`Command::Message`, ours) that `Game::message_order` applies
for the human only (an MP player's inbox and stories are open, §7.4). Saved globals (save chunk names):
`FolyamatbanU`/`UH`/`U3`/`UP` 0xDABDA0.. (the dialogue in progress: block, entry, row, pointer; −1 none),
`FolyamatbanPA` 0xDA65E7, `FolyamatbanOldBlokk` 0xDABDB0, `VoltUjValasz` 0xDABDB4, `VoltUjUzenet` 0xDABDB8,
`UAktUzenetFajta` 0xDABDBC, `UAktUzenet` 0xDABDC0, `AktBlokk` 0xDABDC4, `AktUzenetPointer` 0xDABDD4,
`AutomataUzenet`/`…Hanyadik` 0xDABDD8/DC, `Szovegvegen` 0xDABDE0, `MaxTiltasok`/`Tiltolista` 0xDABDE4/E8,
`UzenetPlaybackek` (0x54: `+0` count, `+4 + 4·i` the parts played, no bound). Unsaved but read by the next order
(the port keeps them in memory at the same VAs; `replay.js` dumps them as `msg_*`): `ParbeszedVege` 0xD909BA,
`ParbeszedVegeJon` 0xD909BB, `KiirtaMar` 0xDA6607, `AktUzenet0to3` 0xD90DC8, `AktValasz0to3` 0xD90DCC,
`AktValasztasok` 0xDABE3C, `Valasztasok` 0xD8E110 (four of 0x5c: label `+0`, `+0x50` part, `+0x54` next block, `+0x58`
disabled; `ParbeszPlace`, the dialogue text, follows at 0xD8E280).
Story hand-offs (`StoryEsemeny` 0xF2F8AC + flag; `MessageSelectedBlokk`/`Reszlet` 0xF26574/78), each then
`Story_Check(true)`: `Story_MessageSelected(b, r)` exe 0x5c9150 flag 6; `Story_LetiltasCheck(b, r)` 0x5c90a0 flag 8;
`Story_MessageMegnezes(b)` 0x5c90f0 flag 9, nothing when `b` is in `OlvasasVodor`, else added (0x5430a0).

### 7.1 `KeresBlokk(n)` 0x505930 and `KiertekelBlokk(off)` 0x505c20
`KeresBlokk`: `FolyamatbanOldBlokk` := n; the first line starting with `%02d` gives the offset past it and past empty
lines (`\r` + 1 byte per line end) and `AktBlokk` := n; else −1 (`AktBlokk` kept). `KiertekelBlokk`: `MaxTiltasok`
:= 0, `Story_LetiltasCheck(UAktUzenetFajta, AktBlokk)`; a line not starting with `-` gives `AktValasztasok` := 0 (the
old choices stay); else each `- PP NN label` line is a choice, written in this order: disabled (`PP` in
`Tiltolista`), part `PP`, next `NN` (two-character `atoi`), then the label (`mesAtteszSorvegeig` 0x509f20: column 8 to
the `\n`, kept, no NUL; `#k#` → the open entry's parameter k with its NUL, overwritten by the next byte; **no bound**,
so a label over 0x50 bytes runs over its own part/next/disabled and on; traced: labels reach `ParbeszPlace`);
`AktValasztasok` := their count.

### 7.2 `KiertekelUjuzenet` 0x505d60 (an entry opened)
`UAktUzenet` := `AktUzenetPointer + AktUzenet0to3`; `Szovegvegen` := 0, `KiirtaMar` := 0; `BetoltFajlok` 0x504a90:
`UAktUzenetFajta` := the entry's block, `msg/txt%04d.txt` loaded. `Folyamatban*` := block, `UAktUzenet`, row, pointer.
- Plain kind: `ParbeszedVege` := 1, `AktValasztasok` := 0, `+4` := 1; if `+5 == 0`: `+5` := 1 and `Megnezes(block)`.
- Conversation: unread → `UzenetPlaybackek[e].count` := 0 (read → the replay index 0xD90DD4 := 0);
  `KiertekelBlokk(KeresBlokk(0))` with `ParbeszedVege` = `…Jon` := 0; with choices, the first's part (unless −1) is
  recorded when unread, and its next block evaluated. Then if `+5 == 0`: **`Uzenetek[UzenetekSzama].+5`** := 1 (the
  slot past the end, asm 0x505fea..0x505ff9, the ELF too: the entry's own `+5` stays 0) and `Megnezes(block)`.

### 7.3 The orders
- `Open { row }` (`Messages_Main` 0x508560, hot-spot 3; only with `FolyamatbanU == −1`, `ParbeszedVege`, no automatic
  message): `VoltUjUzenet` := −1; a row holding a message: `AktUzenet0to3` := row, `VoltUjUzenet` := 1;
  `AktUzenet0to3 > UzenetekSzama` → −1; `AutomataUzenetKiertekelo` 0x504a20; `VoltUjUzenet == 1` → §7.2.
- `Scroll { pointer, row }` (hot-spots 1, 2): the arrows' `AktUzenetPointer` and row.
- `Answer { k }` (hot-spot 4 once `KiirtaMar`, or `Melyikvalaszfolottvanazeger` 0x506750: one answer alone, or a read
  message's recorded one): `AktValasz0to3` := k, `VoltUjValasz` := 1, then `ValaszKivalasztva` 0x509d10: unless choice
  k is disabled, `VoltUjValasz` := 0; unread → `Story_MessageSelected(UAktUzenetFajta, part)` and the part recorded;
  `ParbeszedVegeJon` := 0; next ≠ −1 → `KiertekelBlokk(KeresBlokk(next))`, else `AktValasztasok` := 0,
  `ParbeszedVegeJon` := 1, `FolyamatbanOldBlokk` := −1; `KiirtaMar` := 0. (The exe retries a disabled pick every
  frame while `VoltUjValasz` stays 1; the port tries once per order.)
- `TypedOut` (`MessagesGfx2D_2`, exe 0x507b43..0x507c1b, once per text: `KiirtaMar` 0 → 1): `Szovegvegen` := 1; a
  plain kind: `FolyamatbanU`/`UH` := −1; `ParbeszedVegeJon` → `ParbeszedVege` := 1, `FolyamatbanU`/`UH`/`OldBlokk`
  := −1, `+4` := 1 (the conversation is read), `VoltUjUzenet` := −1; always `FolyamatbanPA` := `ParbeszedVege`.
- `Auto` (`FRISSIT_Messages2` 0x505180, every 11th UI frame on screen 10): when `VoltUjUzenet == −1` or
  (`Szovegvegen == 1` and `ParbeszedVege`): `KeresAutomataUzenet` 0x5058b0 (`AutomataUzenet` := the first unread entry
  of an automatic kind, its index; else −1); found → `VoltUjUzenet` := 1, `AutomataUzenetKiertekelo` (pointer and row
  so the entry shows), §7.2.
- `Enter` (`Messages_Belep` 0x507f90): `UAktUzenet ≠ −1` → `BetoltFajlok`. No dialogue in progress
  (`FolyamatbanU == −1`): after a finished one (`FolyamatbanPA`) the first unread entry (0x505bd0) is scrolled into
  view (0x509cb0); `KeresAutomataUzenet`; `KiirtaMar` := none found. Else a plain kind opens again (§7.2 from
  `Folyamatban*`; the exe copies `FolyamatbanU`, the block, into `UAktUzenet`, which §7.2 overwrites); a conversation
  replays its recorded parts (UI) and, unread with `FolyamatbanOldBlokk ≠ −1`, re-evaluates that block (and its next
  when the first choice's part is −1). `Messages_Kilep` 0x5083d0 writes nothing saved.

### 7.4 Port and verification
`ig2-sim::game::message_orders` (`MessageOrder`, `Game::message_order`, `Game::inbox_call` for calls made outside a
check), `messages::{InboxRules, find_block, block_choices}` (`GameRules::inbox`: `uzenetek.txt`'s kinds, the raw
`msg%04d.txt`), `ig2-net::Command::Message`. Verified byte-exact in `replay-quick1-answers-x16` (`run_replay.py
--answers 4 --answers-every 1 --seed 777`, 60 ticks, saved chunks and the `msg_*` bits; `rules/traces/2026-10-23-
messages-answers.md`). Not traced: `Enter` (calling `Messages_Belep` re-initialises the screen; ported from the
decompile), `TypedOut` (inside `MessagesGfx2D_2`: the tracer and the port both transcribe the asm). Multiplayer: the
original keeps the inbox and its stories per player (`campaign/multi.md`); the port's single inbox is the
`EN`'s, other issuers' orders are ignored (open).
