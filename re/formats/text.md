# Text data files

Encoding: 8-bit codepage per language (`ig2_formats::text`: HU = Windows-1250, others
Windows-1252; inferred from data). Localized copies override by path (`localization/<lang>/...`).
The 1999 code has **one loader per file family**. Each was found through its filename string and
decompiled with `tools/ghidra/DecompileFunctions.java`. Addresses are ELF `ig2`.

Golden tests: `tests/install.rs` (string tables) and `tests/text_formats.rs` (everything below),
run against all 6 languages.

## 1. UI string table: `text/txt01..txt19.txt` (no txt12): `ig2_formats::strings::UiStrings`
Loader `StringReadAll` @0x531e40 → `StringRead` → `StringProcess` (`file.cpp`); lookup
`GetString(file, section, index)` @0x533650 into `Stringek[file][1..11][1..52]`.

Line grammar:
1. Scan from the end for `;` or `"`; if `;` comes first, cut there (translator max-length comment).
2. Trim; skip leading spaces; `;` at the start = comment.
3. Skip `!`, `*`, `#` markers.
4. Three non-zero decimals: file, section, index.
5. `|` = text starts immediately; otherwise skip whitespace.
6. Strip surrounding quotes if the line ends in `"`.

Result: 0 rejected lines in any language, 1,473 entries (HU 1,471). Golden: `(17,1,6)` = "Solarian".

## 2. Keyed string table: `text/s_story|s_anims|s_races|s_stname|s_design.txt`: `strings::KeyedStrings`
`LoadStrTable(table, path, reset)` @0x623760, tables **5, 6, 10, 29, 59**; key = `table << 16 | n`,
which must strictly increase; fewer than 998 entries total. Lines: `n[|] text`, optional quotes.

## 3. Messages: `ig2_formats::dialogue`

### `uzenetek/uzenetek.txt` (index): `LoadMessagesFile` @0x5a6f50
Fixed columns on lines starting with a digit (others are comments):
```
cols 0-4  id (atoi)        then skip to the first space (≤6 chars), p = that + 1
p+0  II/NN  conversation ("beszélgetés": has an msgNNNN dialogue)
p+3  II/NN  automatic
p+6  II/NN  replaces ("leváltás")
p+9  20 chars sender      p+30  20 chars subject 1      p+51..EOL  subject 2
```
761 entries in every language.

### `uzenetek/msgNNNN.txt` (dialogue control): `KeresBlokk` @0x5a69c0, `KiertekelBlokk` @0x5a6ac0
A line that is two digits (then space/EOL) opens block NN; block 00 is the entry. Choice lines:
```
- PP NN label          cols 2-3 part to print (−1 = none), cols 5-6 next block (−1 = end), label from col 8
```
`KiertekelUjuzenet` @0x5a8550: for a conversation, evaluate block 0; if its first choice has part −1,
jump straight to the next block. Choices are offered to the player.

### `uzenetek/txtNNNN.txt` (text parts): `KeresReszlet` @0x5a68f0, `KiirReszlet` @0x5a80b0, `GetTxtFileCommands` @0x5a7c70
Part header (fixed columns): `PP AAAA SS WW[ #m]`. `PP` = part number, `AAAA` = anim start,
`SS` = typing speed (per-char advance = speed / OSZTO; can be overridden), `WW` = `EN` (the player
speaks) or `TE` (the other side speaks, which triggers the talking-head clip lookup), `#m` at
col 14 = story-map animation. Body lines: `+` forces a new line, `-` flows on; the text starts after
the marker (one more character is skipped if it isn't a space). Inline markup: `~z`/`~s` colour
switches [inf], `#1#`… placeholders. Some files start with a bare title line (`1013`) that matches
the game's `%02d` prefix scan but is never requested as a part.

**Data anomalies (EN, pinned in the test):** references the game can't resolve either (it prints nothing):
`0193` part 2 (both parts are labelled `01`), `1561`, `1685` parts 2–5, `2402`/`2412` part 4, `5532`, `5540`,
`8130`, `8315`. Message scripts that exist but are unreachable: `1561`, `5532`, `8130`, `8443` (index
flag `NN`), plus **31 orphan msg scripts** (5150–8051) whose ids aren't in the index and have no txt file
(cut content). FR and ES each have one empty part (FR `txt1844`).

### `text/uzenetanim_{1,3,5}.txt` (talking-head clips): `ReadUzenetAnimLista` @0x5a4910, `GetUzenetAnimLista` @0x5a4a80
The file is chosen by the player's empire `EN` ∈ {1, 3, 5}, else 1. Group header = a line without `!`
containing message ids (`5540, 5541`); label lines without numbers (`Kereskedok` = traders) are
grouping comments. Part lines: `PP !clip [!loopclip] [#face]`. Clip → `messages/<clip>.bik`
(path kept as-is if it contains `misc/`); the remaster ships `anim/mpeg4/messages/<clip>.mp4`. `#face`
is clamped to 1..19. Audio: `messages/%04d_%02d.ogg` (message, part) and `%04d_00-LOOP.ogg`.
Result: 467 groups, 1,316 clip refs; only 3 missing videos (`human/advisor_11`, `human/hero1_01`,
`human/hero1_looped`, all in list 1).

## 4. Diplomacy: `ig2_formats::diplomacy`

### `diplomac/ajanlat.eng` (offers), `valaszok.eng` (answers): `GetSzoveg` @0x521ab0
```
#AAA/BBB/CCC[/DDD] ; author note       (A-prefixed `A001/000` = fallback answer codes)
- text line (placeholders #1# #2# #3#)
- …
```
The file header (Hungarian) documents the codes: `000/xxx` negotiate (cease fire, peace, war,
non-aggression, alliance, …), `001` offer-for (two parameters), `002` demand, `003` offer,
`004` special, `006/xxx` talk-about; the third field selects the answer case by relationship and
relative strength (7 cases for offers, 3 for talk/negotiate).
Lookup: build `#%03d/%03d[/%03d[/%03d]]`, **raw prefix scan of the whole buffer**, count matches n,
pick `random(n)` (or `n·seed/10001` with a seed), then print the following `-` lines. If nothing
matches: try `A<CCC>/<DDD>`, then a hardcoded "deep space storm" excuse. Consequences:
- headers repeated with a second comment (ES/FR) count as extra variants sharing the next text block;
- typos still match by prefix (ES `001/000/000/0025` behaves as `…/002`);
- `viszontv.eng` is referenced by the binary but not shipped.

### `diplomac/special.eng`: `Diplomacy_ReadSpecials` @0x529f30
`#id/n` (n = 1..5) then `- ` lines: title, text A, text B, n × (option, result). EN and HU ship 6
specials (ids 141, 1471, 1481, 8601, 8571, 8581); **DE/FR/ES/IT ship only the last 3** (data gap).

## 5. Other texts: `ig2_formats::misc_text`

| File | Loader | Grammar |
|---|---|---|
| `text/infostr.txt` | `InfoString_Read` @0x620a30 | `<id> <II|NN> text` (text from id-end + 3, ≥4 chars), ≤1000 entries; **a malformed line aborts the rest of the load** |
| `text/res_info.txt` | `LoadTalalmanyInfok` @0x5e7df0 | strictly 4 lines per tech 1..189: `;  "Name"` (HU: unquoted), then 3 × 180-byte lines. 51 unused slots (`;  ""` + blank/dotted) |
| `scenario/*.dsc` | `LoadScenarioDescription` @0x62a1c0 | first word (case-insensitive) `title`/`pic`/`desc` (repeatable)/`difficulty`/`race`; `pic` gets `..\scenario\` prefixed |
| `help/helpmain.txt` | `Help_Belep` @0x54db70 | `<TOPICname,file>`, `<SUBTOPICname>`; other lines are commentary. `helpmain_mobile.txt` on mobile |
| `help/<topic>.txt` | `Help_Belep`, `HelpKikeresKepek` @0x54d8f0 | `*` heading, `+` new line, `-` flowing; inline `<Fn>` font, `<Cn>` colour, `<Pfile.bmp>` picture → remaster `help/<file>_bmp.pvr` (all resolve) |
| `colony/terepleiro.txt` | `InitHatterekEsTerepek` @ELF 0x4bd5b0 | `[hatter]` (4 values) / `[felszin]` (11 values) sections, one value per line, `;` comments outside quotes; `[felszin]` `n` = `gen_maps/<n>.map` (75 in shipped data). `ig2_formats::colony_map::parse_surfaces` |

## 6. Not yet implemented (rules data, Phase 2)

| Files | Loader | Shape |
|---|---|---|
| `text/building.txt` | `ReadBuildingData` | `****`-separated building records |
| `text/designs.txt` | `ReadDesignData` | `****`-separated designs |
| `text/fleet.txt` | `ReadFlottaData` | `[<story>/]<fleet-id> …`, `//` comments |
| `text/values.txt` | `ReadValues` | C-like `{…}` tables |
| `text/autobld1..10.txt` | `INIT_AutoBuild` | `:SECTION` + rules |
| `text/fajinfo.txt`, `gameinfo.txt`, `maffia.txt`, `fabscr.txt`, `tozsde.txt` | `LoadFajInfoText`, `LoadGameInfo`, `NEWGAME_Colony`, `LoadHighscores` | simple id/quoted tables |
| `tutorial/*.tut`, `*.mou` | `TutorialLoadScenario`, `TutorialStartMouseSave` | tutorial script and recorded input |
| `gfx/*.json`, `texts.json` | `TextServer::Initialise` | remaster C-preprocessed JSON (UI, Phase 3) |
