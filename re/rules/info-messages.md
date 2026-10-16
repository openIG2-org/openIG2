# Info charts and messages: the statistics history, the scrolling message line and the message inbox

Status: **verified at runtime** for what the recordings reach (`traces/2026-10-16-save-replay.md`): the info
charts every tick of 1,900 (the sample after each load and 17 month changes), the scrolling-message lists with the
year's tax message (twice) and the "enemy fleet spotted" message, the spy reports and messages of a staged spy
recording (§3.3: the report queue with 26 reports, ids 100, 101, 103, 106, 136, 139–141), the battle outcomes of
three staged recordings (§3.2: 11, 12, 15–18, 22–25 from fifteen auto-resolved space battles and ground wars, one
of them a planet battle followed by the winner's landing), the inbox
(nothing expired), byte-exact against `engine/crates/ig2-sim` (`info.rs`, `messages.rs`, `spy.rs`, `game.rs`,
`game/battles.rs`, `tests/save_replay.rs`). The other posting sites (§3.2) are ported from the code, not traced.

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

### 2.3 The UI side (`FRISSIT_Scroll2`, not the sim)
Runs in the UI frame (`Fake_FRISSITES`, after the tick loop), only on some screens and not while a dialog is
up; it moves the current text by float steps and, when it has left the line (or there is none), takes the next:
`ScrollSzoveg` := entry 1's text, `ScrollFontos` := entry 1's `+0x3c` (its `+0x38` picks
`SOUNDS/SCROLL/{HUMAN,BRUTAL}/%04d.WAV` except ids 1, 0x14, 0x15, 0x1a), `db -= 1`, entries 2..=`db + 1` move to
1..=`db`; with `db == 0` it clears `ScrollSzoveg`. So the main list drains at the display's pace: the sim posts,
the UI pops (`messages::Ticker::pop`).

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
`SpaceWarIndit` / `GroundWarIndit`, `game::battles`). Not ported: the real-time battles' own posts — a real-time
ground war's end (`GroundWarFinished(p, s, false)` 0x47c5d0 posts 11/12 there) and the space battle screen's
result; the routing reports them (`BattleEvent::SpaceRealTime`, `GroundRealTime`) and leaves them. The espionage
posts are §3.3. The UI side
(§2.3) isn't the sim's either: no recording shows the line draining. The story
scripts call `ScrollUzenet` directly (unported scripts, U24). `LoadSave` posts one message after a load (in the
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

The spy screen pops the queue (UI, `AktKep` 7; with it open, `FRISSIT_Spy` also moves its cursor `AktKem` 0xBB2384
and the fate function skips dead slots): not the sim's.

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

And 130 (0x82, `""`, 0, 0, 0x585749): `EN`'s spy got home (§3.4).

## 4. The inbox expiry (`FRISSIT_Messages`)
`Uzenetek` 0xDA6620 (100 × 0x88: `+0` block id, `+4` u8 kept, `+8` i32 expiry `Ido`), `UzenetekRandomKitol`
0xDABC10 (100 × i32), `UzenetPlaybackek` 0xDA9B40 (100 × 0x54), count `UzenetekSzama` 0xDABDD0. Every tick: the
first `i` in 0..count with `0 < expiry < Ido`, `+4 == 0` and block ≠ `UAktUzenetFajta` 0xDABDBC gets
`Story_MessageElevules(block)` (`StoryMessageBlokk` 0xF26574 := block, story event flag 7, `Story_Check`), then
the three arrays close the gap (entries `i + 1..count` move down one; the last stays duplicated) and the count
drops by one. At most one message expires per tick. Nothing else in the tick writes the inbox (messages arrive
through the stories, diplomacy specials and the lockstep queue).

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

