# ADR 0068: easy mods: zip mods, `mod.toml` and the mod manager

Date: 2026-11-01. Status: accepted (extends ADR 0044). Implemented by `ig2-formats::zipfs` (the zip reader),
`ig2-formats::vfs` (`Mod::folders`, `Mod::available`, `Mod::info`, zip mods in `Install::open_from`), and the client's
mod manager page (`ig2-client::mods_page`, main menu → Mods; the room page's mods line, `mp_lobby::page`).

## Context
ADR 0044 made mods possible: a folder under the settings folder's `mods` (`%LOCALAPPDATA%\GyroscopeGames\ig2\mods`),
enabled by editing `mods\enabled.txt`. Most players never find that folder, can't unpack a download into the right
layout, and don't edit text files. A mod should be one download dropped into one obvious folder, and switched on in
the game.

## Decision
1. **Where**: two `mods` folders are searched: one beside the game's executable (the obvious place for a download)
   and the settings folder's (ADR 0044 §1, still read). A mod is a folder or a `.zip` file in either; its name is the
   folder's name or the zip's name without `.zip`. When both folders hold a mod of the same name, the one beside the
   executable wins (it is the one a player just dropped there); a folder wins over a zip of the same name in one
   folder.
2. **Zip mods are read in place**: the game reads a zip's central directory once and each file from its offset when
   it is needed (stored or deflate entries, the two any zip tool writes); nothing is unpacked. The zip's files mirror
   the install's layout like a folder mod's; one top folder that wraps everything (`mymod.zip/mymod/dataraw/...`, what
   "compress this folder" makes) is stripped. Safety: an entry whose name is absolute, has a drive letter or a `..`
   part, or holds a NUL is refused (a zip is untrusted input; nothing is written, but no name may point outside the
   mod); encrypted entries, ZIP64 and other compression methods are refused; at most 65,536 entries and 1 GiB per file
   (inflation stops at the entry's stated size, so a small zip can't inflate without bound). A zip that breaks a rule
   is a broken mod: it is skipped with its reason (the client's console, `ig2-check`, the mod manager) and the game
   starts without it. A mod that loads but breaks the start (a `rules.toml` or text patch that fails, ADR 0044 §8,
   §12) can't lock the player out either: the client then starts without any mods and the mod manager names the
   error, so the player can switch the mod off.
3. **`mod.toml`** at the mod's top describes it: `name`, `author`, `version`, `description`, `picture` (a PNG in the
   mod, shown by the mod manager) and `game` (the openIG2 version it was made for, MAJOR.MINOR). Every key is
   optional; without the file the mod manager shows the folder name. `mod.toml` and its picture are not game paths
   (not indexed, not in the multiplayer fingerprint). A `game` that differs from the running version is a warning,
   not an error.
4. **Enabled list and priority**: unchanged from ADR 0044 §2: `enabled.txt` in the settings folder's `mods`, first
   line = highest priority; `--mod` still overrides it for a run. The mod manager writes this file (a comment header,
   then the enabled names in priority order); disabled mods are simply not listed, so the file stays the format a
   player or an older build reads.
5. **The mod manager** is an egui page (ADR 0062) in the main-menu Options page's style: main menu → Mods (the grouped
   panel's Extras entry, ADR 0059; the classic menu's Mods label above Quick game). Every mod found is a card: picture,
   name, version, author, description, where it lies (folder or zip), a switch, up and down arrows for priority among
   the enabled ones (the enabled come first, in priority order). The same checks `ig2-check` runs on mods (ADR 0044 §8,
   §12; ADR 0061 §7) run for each enabled mod alone, and the files two enabled mods both provide are listed as
   conflicts under the mod that loses them. Buttons: Open mods folder (the one beside the executable, created when
   missing), Back, Apply (writes `enabled.txt`). The install and rules are loaded once at start, so a changed list
   takes effect on the next start: the page says so and offers Restart now (the executable again with the same
   arguments). Nothing about mods is changed in a running game. Keys, as the other egui pages (ADR 0053, 0059): Esc
   or the right button Back; Enter Apply, or Back with nothing to apply; Up/Down (Tab, Shift+Tab), Home/End the
   focused mod (an accent ring); Space its switch; Shift+Up/Down its priority; the wheel scrolls.
6. **Multiplayer**: the fingerprint (ADR 0044 §4) is unchanged: it hashes files, not names, so a zip mod and the same
   files in a folder match. The room page lists the enabled mods (name and version) under the room's settings: a
   player in the room has the same mods as the host (a mismatch is refused, `Refusal::Mods`), so their list is the
   room's. A refused joiner's message points to the Mods page. ADR 0086 §3 sends the host's mod list to a refused
   joiner (lobby protocol 15).

## Consequences
- Installing a mod is: download, drop the zip into `mods` beside the game, enable it in Mods, restart.
- The settings folder's mods and `enabled.txt` keep working; old folder mods need no change.
- A broken download can't stop the game from starting, and a zip can't name a file outside its mod.
- Load time: a zip's directory is read at start; its files are read when used, like the install's.
- The browser build (ADR 0047) has no mods folder and no mod manager entry.
