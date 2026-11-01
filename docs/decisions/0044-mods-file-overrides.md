# ADR 0044: mods, part 1: files that override the install's

Date: 2026-11-01. Status: accepted. Implemented by `ig2-formats::vfs` (`Install::open_with_mods`, `Mod`), the
client's `--mod` and `mods/enabled.txt`, the multiplayer room's fingerprint (`ig2-net::session::modded_fingerprint`,
`Refusal::Mods`) and `ig2-check`'s mods report.

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

## Consequences
- A mod replaces whole files; changing one value inside a table or a script is part 2 (rule tables and scripts).
- The engine still ships nothing: a mod is the player's own files; tests build tiny synthetic installs and mods.
- Single player needs no agreement; replays and saves record nothing about mods yet.
