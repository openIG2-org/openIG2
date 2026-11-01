# ADR 0044: mods: files that override the install's, and rule patches

Date: 2026-11-01. Status: accepted. Implemented by `ig2-formats::vfs` (`Install::open_with_mods`, `Mod`), the
client's `--mod` and `mods/enabled.txt`, the multiplayer room's fingerprint (`ig2-net::session::modded_fingerprint`,
`Refusal::Mods`) and `ig2-check`'s mods report; part 2 (rule patches, §6–§9) by `ig2-rules::patch`
(`Rules::apply_patch`, applied in `Rules::from_install`) and `Install::rule_patches`.

## Context
Players want to change the game's content: a sharper texture, a reworded text, a new music track. The engine reads
every game file through `ig2_formats::Install`, one index of the install's loose files (lowercase, `/`-separated
relative paths, the localized copy under `localization/<lang>/` preferred, `vfs.rs`). The remaster has no archives:
every texture (`.pvr`), model (`.xv`), text and sound is its own file in `dataraw/`, so "a file" and "an archive member"
are the same thing here. The rule tables are read from `ig2.exe` itself (ADR 0004), not from a file the index holds.
The original has no mod support.

## Decision
1. **Where**: a `mods/` folder beside the settings files (`Options::path`'s folder,
   `%LOCALAPPDATA%/GyroscopeGames/ig2/mods/`, or `IG2_OPTIONS_DIR/mods/`). Each mod is a subfolder whose files mirror
   the install's layout: `mods/<name>/dataraw/textures/colony/foo_bmp.pvr` replaces the install's
   `dataraw/textures/colony/foo_bmp.pvr`; a path the install lacks is added (a new file only matters where something
   looks for it). Plain files and the game's own container formats both load (`container::decode` passes plain data
   through).
2. **Which, in what order**: `mods/enabled.txt` lists the enabled mods, one folder name a line (`#` starts a comment,
   blank lines skipped), first line = highest priority: the first listed mod that has a file wins. `--mod <name>`
   (repeatable, same order rule) replaces the list for one run. A named folder that is missing is reported and
   skipped. The list is a text file and not a row in `ig2-rework-options.dat` because that file is binary and the
   Options screen has no mod page yet; a page that writes the same list is a later step.
3. **Lookup**: the install's index and every enabled mod's files are merged into one index; for a game path the
   candidates stay the localized copy, then the plain path, but a candidate a mod provides comes before any the
   install provides, so a mod's `dataraw/x` beats the install's `localization/hu/dataraw/x`. Texture name fallbacks
   (`.bmp` → `.bmt` → `.bmf`) and listings see mod files like install files. `ig2.exe` and other `.exe` files are
   never overridden (rule tables and scripts are part 2).
4. **Multiplayer**: the room's rules fingerprint (SHA-256 of `ig2.exe`, ADR 0005 §4) becomes SHA-256 of that
   fingerprint and a mods fingerprint when any mod file is active; the mods fingerprint is SHA-256 over every active
   override in path order (path, a 0 byte, the file's length and bytes), so two players with the same files in the
   same winning order match whatever their folders are called. Without mods the fingerprint is unchanged, so old
   saves and unmodded rooms still match. The joiner's hello carries the mods fingerprint too (`LOBBY_VERSION` 10); a
   rules mismatch whose mods fingerprints differ is refused as `Refusal::Mods` ("Other mods enabled"), so the player
   knows what to fix. MP saves made with mods only resume with the same mods.
5. **`ig2-check`** lists each active mod file: the mod, the path, and whether it replaces an install file or adds one.

## Part 2: rule patches
The rule tables live in `ig2.exe` (ADR 0004), which a mod never replaces (§3), and a balance change is a handful of
numbers, not a new file. So a mod may change named fields of the tables `ig2_rules::Rules` reads.

6. **The file**: `rules.toml` at the mod folder's top (`mods/<name>/rules.toml`), TOML. It is not a game path: the
   file index skips it. A record table's entry is a table named `<table>.<id>` (the id is the record's index, as
   `ig2-rules-dump` prints it) holding `field = value`; a list's entry is a table named after the list holding
   `<index> = value`. Values are integers, floats (integers are accepted for floats) or arrays of the field's length:
   ```toml
   [techs.12]
   price = 900
   research_time = 40
   [buildings.3]
   energy = 7
   [race_production_mult]
   2 = 1.5
   ```
   The patchable tables and fields (the names `Rules` uses, so `ig2-rules-dump`'s RON shows the values):

   | Table | Fields |
   |---|---|
   | `techs` | `price`, `build_time`, `hp`, `capacity`, `prerequisites` (3), `lab_levels` (3), `research_cost`, `research_time`, `firepower` |
   | `buildings` | `tech`, `priority`, `build_work`, `max_hp`, `energy`, `workers`, `output`, `flags` (2) |
   | `hulls` | `class`, `flags` (6), `tech` |
   | `space_weapons` | `kind`, `reload`, `damage` (12), `accuracy`, `spread`, `speed`, `duration`, `range`, `graphic`, `flags` (4) |
   | `ship_types` | `scale`, `top_speed`, `accel`, `turn`, `size_xy`, `size_z` |
   | `spy_missions` | `kind`, `xp`, `days`, `cost`, `min_rank`, `skill_floor`, `skill_full`, `chance` |
   | `problem_curves` | `a`, `b`, `c`, `d`, `rate`, `weight` |
   | `ground_weapons` | `min`, `max`, `period`, `unknown` |
   | `ground_rt_weapons`, `ground_fortress_weapons` | `min`, `max`, `reload`, `range` |
   | lists | `race_production_mult`, `race_research_time_mult`, `race_research_money_mult`, `research_size_mult`, `difficulty_production_mult`, `difficulty_money_mult`, `difficulty_development_mult`, `race_peace_threshold`, `race_trade_demand`, `race_fear_threshold`, `race_ai_production_share`, `race_ai_military_autobuild`, `kidnap_morale`, `radar_ranges`, `planet_radar_ranges`, `satellite_prices`, `tank_speeds`, `ground_hull_hp`, `ground_turn_speeds`, `ground_armour`, `spy_training_cost`, `spy_training_days`, `spy_rank_xp`, `spy_race_cost_mult`, `spy_hire_cost_mult` |

   Integer fields are range-checked against their width (`u8` 0..=255).
7. **Order and consistency**: `Rules::from_install` reads the exe's tables, then applies the enabled mods' patches
   lowest priority first, so the first mod in `enabled.txt` has the last word on a field, as with files. A tech
   patch also rewrites the same field in `tech_defaults` (the raw `Talalmanyok0` records the live tech table starts
   from, ADR 0004), so every reader — the sim's rules, the live tech table, the screens — sees one table. Saves keep
   only the per-empire tech state over those defaults, so a patch applies to loaded saves too.
8. **Errors**: a patch that doesn't parse, or names an unknown table, field or id, or gives a value of the wrong kind
   or out of range, fails the load with the mod, table, id and field named (for an unknown field the known ones are
   listed). Nothing is half-applied: the client stops with the message. `ig2-check` checks each mod's `rules.toml`
   on its own, against the unpatched tables, and says how many fields it sets.
9. **Multiplayer**: each active `rules.toml` is hashed into the mods fingerprint (§4) as the path `rules.toml#<k>`,
   `k` its 0-based place in the order patches apply, so the same patches in the same order match.

Story scripts are game files in `dataraw/` (`re/rules/campaign.md`), so part 1 already lets a mod replace one whole;
the campaign tables the story engine indexes from the exe (`campaign_words`) are raw words addressed by exe address,
without named fields, so they have no patch table.

## Consequences
- A mod replaces whole files, and changes rule-table fields by name (`rules.toml`); the campaign's exe words are not
  patchable.
- The engine still ships nothing: a mod is the player's own files; tests build tiny synthetic installs and mods.
- Single player needs no agreement; replays and saves record nothing about mods yet.
