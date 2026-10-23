# Game end: victory, defeat, the return to the main menu, high scores

Exe normative, ELF the symbol oracle. Decompiles: `_local/decomp/gameend-exe/`, `gameend-elf/`. The deaths and
the victory check themselves are `diplomacy.md` §12; the video player is `screens/video.md`; the scenario panel
around the table is `screens/mainscr.md` §6–§7. Port: `ig2-sim::game::death`, `ig2-client` (`window.rs`
`exit_to_main_menu`, `video::Request`), `ig2-formats::highscore`. Runtime check: §10.

## 1. What ends a game
Every end goes through `MEGHALAS` (the human dies) or `MEGNYERES(videos)` (the human wins). Both act only while
`GameStarted` (0xBAE297) is set, and `ExitToMainScreen` clears it, so only the first end of a game counts.

| caller (exe) | call |
|---|---|
| `FRISSIT_Viszony` 0x50e12a, the daily death check (`diplomacy.md` §3.2) | `BirodalomMeghal(e)` (exe `MEGHALAS` 0x53b630 is `BirodalomMeghal` with ELF `MEGHALAS` 0x61e800 inlined) |
| `Fake_FRISSITES` 0x4deaf4, every sim tick; and the end of every `BirodalomMeghal` | `MegnyeresEllenoriz` 0x53f8c0 (`MEGNYERES(true)` inlined twice) |
| `Story_Check` 0x5c7b24: story 405 (counter 4051) | `MEGNYERES(1)` |
| `Story_Check` 0x5c7cd8: story 406 (counter 4061), `DIE()` | `BirodalomMeghal(EN)` |
| story 858 (`FUN_005e4370` 0x5e492d, counter 8589) | `MEGNYERES(1)` |
| story 863 (`FUN_005e5b46` 0x5e63f4, counter 8638): plays its own two videos first | `MEGNYERES(0)` |
| story 832 (`FUN_005ddb02` 0x5de449, counter 8329) | `BirodalomMeghal(EN)` |
| `st_Scen_include` 0x601580: the scenarios' success (13 sites 0x601657 … 0x6024e3) / failure (0x601e81, 0x601f5c, 0x6020c0, 0x602182, 0x60257b) | `WIN(0,1)` + `MEGNYERES(1)` / `BirodalomMeghal(EN)` |
| `ExecuteString` 0x4dc51b / 0x4dc66e, the cheat console (`kill`, string 0x85431C; the second one's name unread) | `BirodalomMeghal(e)` |

`CStoryWinException(StoryLastChecked)` is not a game end: `Story_Check` catches it per story block (e.g. handler
0x5c79ac) and runs `WIN(0, 1)`, which ends that story only (`campaign.md` §8.7).

## 2. `MEGHALAS`, `MEGNYERES`, `ExitToMainScreen`
- `BirodalomMeghal(e)` step 5 (`diplomacy.md` §12.1): `e == EN && GameStarted`: `ExitToMainScreen()`; if the human's
  race `PlayerFaj[EN]` (0xBAE574) < 9: `st_ShowAnim(CampaignType > 0 ? VideoLoss[race] (0x919724) :
  misc/altalanos_elvesztes.bik (0x91976C))`, `st_ShowAnim("misc/end_gameover.bik")`; `AktKepKell = 0xF` (also for
  race ≥ 9).
- `MEGNYERES(v)` 0x53f6b0 (ELF 0x61eb50): `if GameStarted`: `ExitToMainScreen()`; if `v` and race < 9:
  `CampaignType > 0` → `VideoWin[race]` (0x919748) and `misc/end_campaign_win.bik`, else
  `misc/altalanos_megnyeres.bik` (0x919770) and `misc/end_victory.bik`; `AktKepKell = 0xF` (also for `v == 0`).
- `ExitToMainScreen` 0x5117f0 (ELF 0x5bc0b0): in multiplayer (`MultiPlayerMode` 0xBAE2C0) `ig2server_sendinfo("nam:,
  stat:0")`, the info socket closed, `MultiCloseClient(1)` (`SuperVisor` 0x912A3A == 0) or `MultiCloseServer(-1,
  true)`, `MultiPlayerMode = 0`; then `KapcsolatEl` (0xBAE2C1), `GameStarted`, `Desync` (0xB771E8), the tutorial's
  two flags (0xF66F2A, 0xF6915D) and `WaitingForVector` (0xDB9748) cleared. Nothing else: the empires and planets
  stay until the next `InitNewGame`, no music, no screen, no options file.
- The videos play inside the sim call (`st_ShowAnim` blocks: `video.md` §5). `IG2_gameUpdate` 0x4e0440 then moves
  `AktKepKell` into `AktKep` (0xBAE0FC) when no change is pending, runs the old screen's `*_Kilep` and
  `StartDemoMode` (= `MainScr_Belep` 0x4fe3c0): the main menu, `PlayMusicMenu(false, true)`, the scenario list
  (and its high-score tables, §4) built again.
- `AktKep < 0xE` in `MegnyeresEllenoriz`'s first block = a game screen (1..=0xD, Options included): no "all others
  dead" win in the lobby (0xE), the main menu (0xF) or IRC (0x10). The campaign blocks have no screen test.
- Every mode ends this way (skirmish, scenario, campaign, multiplayer: each machine on its own `EN`);
  `CampaignType > 0` only picks the race's videos and enables `MegnyeresEllenoriz`'s campaign blocks (single
  player only).

## 3. Port
- `ig2-sim`: `GameEvent::GameOver(Lost | Won | Ended)` (`Ended` = `MEGNYERES(0)`), raised by `Game::game_over`
  only while `Game::over` is `None` (= `GameStarted`); the story's `StoryEvent::Won` / `CampaignOver` /
  `EmpireDies` are carried out in order after the story call (`Game::story_ends`). `Game::over` is not saved: a
  loaded game runs again (`GameStarted` is set by the load).
- `ig2-client`: `video::Request::names(…, race, campaign)` (§2's names; none for race ≥ 9 or `Ended`), then
  `AfterVideo::MainMenu` → `exit_to_main_menu` (`MainScr::enter`, the menu music), at once when there is no video.
  The world is kept until a new game or a load replaces it. Multiplayer: the session's teardown is the lobby's
  (`ig2-net`), not ported here.

## 4. The high-score table
Ten slots per scenario (kind 0 entries of the scenario list only; skirmish and campaign have none), in the
scenario's `ScenarioDescription` (exe 0x9c bytes, ctor 0x542ef0; ELF 0x100): +0x3c key (the `.scn` file's base
name, compared with `stricmp`), +0x40 `score[10]` (i32), +0x68 `name[10]` (CString), +0x90 kind. A score is a time
in `Ido` units (50 a day), shown as `score / 50` days: lower is better, 0 an empty slot.
`AddHighscore(key, name, score)` 0x543150 (ELF 0x629f70): `LoadHighscores` into a fresh description; `i` = the first
slot with `score[i] == 0 || score[i] > score` (ties go below the equal ones); if `i < 10` slots `i..9` move down one
(the last falls off) and slot `i` takes the entry; `SaveHighscores` always (also when nothing went in).

## 5. Storage: the options registry
The "registry" is the `std::map<string,string>` that `options.dat` holds (`screens/options.md` §7,
`ig2_formats::options`); `SaveHighscores` only fills the map, the file is written by the next
`SaveGlobalRegistrySettings` 0x51b4e0. Keys for scenario key `K`, `NN` = `%02d` 0..9: `K_point_NN` (the score,
decimal; read with `atoi`), `K_name_NN` (the name; `ReadRegistryString` 0x51b4a0 copies at most 50 bytes),
`K_crc` (decimal).
- Check value (`CountSmartCrc(3, &j, a, data, len)` ELF 0x629010, inlined and unrolled in the exe): `a[0..3] = 0`,
  one index `j = 0` running across everything; per slot the 4 score bytes (little-endian) then the name's bytes
  (no NUL), each `a[j] ^= byte; j = (j + 1) % 3`; `crc = a0 + a1·0x100 + a2·0x10000`.
- `LoadHighscores(K, desc)` 0x543750 (ELF 0x629440): the table is taken when `K_crc` and all ten pairs exist and the
  check value matches; otherwise every slot is cleared and seeded from `text/fabscr.txt` (`text.md`: lines
  `K/name:days` with `0 < '/' < ':'`, days > 0, matched case-insensitively, stored as days·50 in file order; the exe
  writes on past slot 9 unchecked, the shipped file has at most two lines a scenario). The seed is not written back.
- Called by `LoadScenarioDescription` 0x552fc0 for every scenario while `MainScr_Belep` builds the list (call
  0x4fe429 → 0x553400 → 0x553485), i.e. at every entry to the main menu, and by the name entry (§6).

## 6. Recording a score: dead in the shipped game
`MainScr_Belep` opens the name entry when `scenario_nevbevitelkell` (0xBAD10C) is set (`mainscr.md` §2.1 step 7:
the list on the scenario `scenario_registry_name` 0xBAD0E8 in high-score view, `EnterHighScoreNameMode`, the IO
field 0x99 seeded with "Player", 20 characters at (0x91, 0xDF)). Enter (`DoKeyboard` 0x4db1a0 → 0x4fe950, ported as
`MultiOpenInfoSocket`: it is the IO commit) trims the name, keeps the dialog for an empty one, else
`AddHighscore(scenario_registry_name, HighScoreNev, scenario_score)` and the table reloaded; no screen change. Esc
does nothing for 0x99 (`MainScr_CancelIPEnter` handles 0x97, 0x98, 0x9A only). The dialog: fader (60, 134, 330 ×
130); `sprintf(GetString(15,3,3), score/50)` ("You managed to win in %d days") at (0xE1, 0xA1, 0x28, 3, 7, 1);
`GetString(15,3,2)` at (0xE1, 0xBF, 0x28, 4, 7, 1); the edit text at (IOX, IOY, IOMaxLen, IOCharSize, 5).
But nothing sets `scenario_nevbevitelkell` or writes `scenario_score` (0xBAD110): in the exe the only write of
0xBAD10C is `InitNewGame`'s clear (0x4e128e), 0xBAD110 is only read (0x4fdbcb, 0x4feb1f); the ELF likewise, and its
`ElertEHighscore` (0x629e40, "does this score enter the table") has no caller. `MainScr_Kilep` (0x4fef45..63) still
records `scenario_kezdesi_ido` (0xBAD108) = `Ido` and `scenario_registry_name` when a scenario starts (both saved).
So a won scenario never reaches the entry, no formula for the score exists, and the tables only ever show the
`fabscr.txt` seed (or what an `options.dat` holds). The port keeps the table code (§4–§5) and does not open the
entry (`deviations.md`: a working entry, score = `Ido − scenario_kezdesi_ido` [guess], is an enhanced option).

## 7. The table on the scenario panel (`MainScrGfx2D_2` 0x4fc8d0; `KiirStr(text, x, y, len, size, colour, style)`)
Shown while `ScenarioHighScoreMode` (0xBBB22E, hot-spot 0x27 toggles it; label `GetString(15,2,13/14)`) and the entry
is a scenario. Fade `KiirStrSzinSzorzo = (int)(ScenarioKepNyitottsag · ScenarioValasztasNyitottsag / 100.0f)` (f32,
0x4fc929/0x4fc944; the ELF uses f64).
- header `sprintf(GetString(15,2,15), title)` ("Highscores for '%s' scenario") at (0x2D, 0x15A, 0x32, 3, 4, 1);
- slot i: x = 0x2D (i < 5) or 0xE1, y = 0x173 + 13·(i mod 5); `"%02d."` (i + 1) at (x, y, 5, 2, 4, 0x22); an empty
  slot (`score < 1`) `GetString(15,2,17)` ("None!") at (x + 0x16, y, 10, 2, 4, 1); else the name at (x + 0x16, y,
  0x14, 2, 4, 2) and `sprintf("%6d " + GetString(15,2,16), score / 50)` at (x + 0x5C, y, 0xC, 2, 4, 2).
The panel itself is MainScr part 2's (`ig2-client::mainscr`); it reads `ig2_formats::highscore::Table::load`.

## 8. Not covered
The multiplayer teardown (`ExitToMainScreen`'s socket part) waits for the lobby port. Whether quitting flushes a
registry changed by `SaveHighscores` is moot (§6).

## 9. Exe vs ELF
The exe inlines `MEGHALAS` into `BirodalomMeghal`, `MEGNYERES(true)` into `MegnyeresEllenoriz`, the socket closes and
`WaitingForVector.Disable` into `ExitToMainScreen`, `CountSmartCrc` into Load/Save. CString is 4 bytes (exe) / 8
(ELF). The exe seeds the entry with the literal "Player" (0x914800), the ELF with `MultiplayerNev`.

## 10. Runtime check
`tools/trace/run_gameend.py` (`gameend.js`), quick.scn (EN 6, race 6, `CampaignType` 0), report
`traces/2026-10-23-game-end.md`.
