# Campaign data tables: `text/building.txt`, `text/designs.txt`, `text/values.txt`

Status: **verified** (`ig2_formats::story_data`, golden test `story_data_tables_parse`).

**Not** the building/ship balance rules. Those are compiled into the binary (`tables.md`).
All three files are read by `story.cpp` at start-up into the `Story*Data` arrays used by campaign
scripting. They aren't localised.

Shared tokenizer: separators are `' '`, `'\t'`, `','`; numbers use `atoi`, so a line whose first
token isn't a number (`//`, `**`, `;` comments) yields 0 and is skipped.

## building.txt: `ReadBuildingData` @0x72d500 → `StoryBuildingData[<300]` (16 bytes each)
```
[<story>/]<planet> <building type> [<damage>]        e.g.  0011  2 // Kis lakoepulet
```
- `story/planet`: story = atoi(before `/`); if story is 0 the line is skipped; otherwise planet = atoi(after `/`).
  Without `/`: story = 0 (all stories).
- building type must be non-zero (asserted, story.cpp:0x1f6); damage defaults to 0.
- The file header (Hungarian) lists the building type IDs: 1 HQ, 2 small housing, 3 large housing,
  4 food factory, 5 auto food, 6 hospital, 7 police, 8 fire station, 10 park, 11 bar, 12 entertainment,
  13 stadium, 14 ship R&D, 15 tank R&D, 16 building R&D, 19 shipyard, 20 auto shipyard, 21 tank factory,
  22 auto tank factory, 25 war factory, 26 spy centre, 28 ground cannon 1, … (17/18/23/24/27 marked "doesn't exist").
- The HQ ("főépület") is always pre-placed and not listed.
Shipped: 228 entries; types used {2,4,6,7,8,10,11,14,15,16,19,21,26,28,29,32,33,43,52,53}.

## designs.txt: `ReadDesignData` @0x72e1b0 → `ParseDesignData` @0x72ad70 → `StoryDesignData` (0x94 bytes each)
```
[<id>] "<name>", <II|NN>, <owner>, <hull>, {<pod>/<count>}            (≤12 pods)
C<id> <values…>                                                       → ParseDesignColorData @0x72cc70 (<200)
```
- No numeric ID and the line starts with `"`: ID = `StoryDesignElsoTerv++`, which starts at **10000**
  (set at 0x730937). All 217 shipped designs use auto IDs (10000–10216 in file order).
- Name ≤ 20 characters (asserted, story.cpp:0x24a). Visibility `II`/`NN` (case-insensitive, asserted).
- Hull: a number, or a name from `DesignStrKonv` (.data 0x119bce0), case-insensitive:

| ID | Hull | ID | Hull | ID | Hull |
|---|---|---|---|---|---|
| 1–4 | Fighter1–4 | 9 | Cruiser | 15–18 | Base1–4 |
| 5 | Destroyer | 10 | HCruiser | 19–23 | Tank1–5 |
| 6 | HDestroyer | 11 | Battleship | | |
| 7 | Corvette | 12 | Transport | | |
| 8 | HCorvette | 13 / 14 | Colonization / Terraformer | | |

- Record (0x94 bytes, exe 0xF2F8C0, ≤ 500 asserted; ELF `ParseDesignData` 0x72ad70): +0 id, +4 name (strcpy, rest
  zero to +0x1c), +0x1c `II` = 1 / `NN` = 0, +0x20 owner, +0x24 hull, +0x28 pod count n, for pair j = 1..=n:
  +0x60+4j pod id (before `/`), +0x2c+4j count (after `/`).
- fleet.txt's `D` lines go through the same `ParseDesignData` in `ReadFlottaData`, which `NEWGAME_Story` calls
  first, so the table is fleet.txt's 208 designs (explicit ids 11…9998), then designs.txt's 217; the counter is
  shared. Observed in ig2.exe: `StoryDesignDataMut` = 425, `StoryDesignElsoTerv` = 10217 after both.
  `LoadSave` runs `NEWGAME_Story` too, so a loaded game has the table. Port: `StoryRules::designs`
  (`ig2-sim` game.rs), written at new game step 12 and by `Game::new` when a save left it empty; test
  `ig2-sim/tests/story_designs.rs` (byte for byte against the exe's live table).
- Commented examples document the base-design ID scheme `9xyz`: x = race, y = 1 destroyer …
  7 battleship, z = equipment level 0–9 (these are generated elsewhere, not loaded here).

## values.txt: `ReadValues` @0x72e3b0 → `StoryValueData[<100]` (8 bytes each)
```
<key> <value>            e.g. 5301 1600 // comments allowed
```
Keys are story event / message IDs (`5301`…`7207`, plus intro missions 1–6). Values are durations
[inf: `Ido` units, 50 per day; 1600 ≈ 32 days]. The authors themselves weren't sure about some
entries (comment: "what are these? surely not the times meant for the rnd-s?"). 64 entries shipped.

## Open
- Where `StoryBuildingData`/`StoryDesignData`/`StoryValueData` are consumed (campaign scripting
  `st_*`, `Story_Check`): the campaign spec.
- Pod IDs → equipment table (compiled-in rules).
