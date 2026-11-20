# ADR 0092: modder tools: a new-mod template, live reload and a story console

Date: 2026-11-20. Status: **accepted** (2026-11-20). Extends ADR 0086 §4–§5 (tutorials, `openIG2-check`'s modder
tools) and ADR 0068 (the Mods page, Apply). Rows 579 (template), 580 (live reload) and 581 (story console, planned)
in `docs/ENHANCEMENTS.md`.

## Context

Starting a mod means copying `mod.toml` from the tutorial and guessing the file names and their shapes; every change
then needs the main menu, Mods, Apply (which loads the whole start again) and a game brought back to the moment the
change shows. Looking at a texture, a text or a battle effect takes minutes per try.

## Decision

### 1. A new-mod template (row 579)

`openIG2-check --new-mod FOLDER [--kind rules|texts|story|visual]` writes a ready mod into `FOLDER` (default kind
`story`, the first tutorial's):

- `mod.toml` version 2 (ADR 0086 §2) with every field and a comment line above each: `id` (the folder's name made
  valid: lower case, other characters `-`), `name`, `version = "0.1.0"`, `author`, `description`, `requires` (this
  version's MAJOR.MINOR and newer, `">=0.7"`), `license = "CC-BY-4.0"`, and `depends`, `conflicts`, `picture`,
  `kind` commented out with an example each. A `visual` template sets `kind = "visual"`.
- One commented example file for the kind, which loads and changes something visible:
  - `rules`: `rules.toml`, one rule value changed, the dump tool named for the ids;
  - `texts`: `texts.en.toml`, one line of the interface texts changed;
  - `story`: `stories.toml` (a ticker line and credits on day 2) and its `texts.en.toml` line;
  - `visual`: `visuals.toml` with one switch on and the others listed off.
- `README.md`: what the files are, the check command, the tutorials' and the user guide's addresses.

It refuses a folder that exists and isn't empty (nothing is overwritten) and writes nothing that comes from the
game: every line is ours. The result passes `openIG2-check --mod` and `--package` unchanged (tests).

The Mods page gets a **New mod** button with a kind drop-down: it makes the template in the `mods` folder beside the
executable under the first free name `my-mod`, `my-mod-2`, …, lists it (switched off) and opens its folder.

Port status: shipped (`ig2-formats/src/mod_template.rs`, `ig2-check` `--new-mod`, `ig2-client/src/mods_page.rs`).

### 2. Live reload of a folder mod (row 580)

**Opt-in**: `ig2-client --mod-dev` (off by default; the browser build has no mods folder and no watcher). Without the
flag nothing is read and no timer runs.

**What is watched**: the enabled mods that are folders (a zip is a finished mod), every file under each (dot files
left out), by modification time and size, polled once a second on the window's frame (no file-watcher dependency;
an `std::fs::metadata` per file per second is cheap for a mod's few hundred files).

**What reloads live** (look-only data, never in the sim's state):

| Changed file | What happens |
|---|---|
| a texture (`.pvr`), or its `hires/` picture | every texture the renderer loaded from that game path is decoded again and replaced in place, on every screen that drew it through the shared file cache |
| a sound (`.ogg`) | the sound records are read again; the next play uses the new file |
| `texts.<lang>.toml`, or a replaced text file | the install is opened again (the text patches apply as they load) and the interface texts read again |
| `visuals.toml`, `effects.toml` | the mods' look and battle effects are read again and switched on |
| a video (`.mp4`) | nothing: a video is read when it plays |

A screen that made its own copy of a texture or text when it opened (not through the shared caches) shows the change
when it opens again; that is said in the user guide.

**What only asks for Apply**: `rules.toml`, `stories.toml`, `mod.toml`, the game's data files (`building.txt`,
`designs.txt`, maps, messages, models and anything else not in the table): rule tables, stories and data are in the
sim's state and the multiplayer fingerprint, and models are built into meshes when a screen opens. The change is
named once on the message line in a game ("… changed: main menu → Mods → Apply reloads it"), and the Mods page marks
the mod "changed, Apply to reload" and enables Apply even when the list is the same; Apply runs the in-place load of
ADR 0068 §5.

**A file that fails**: a texture that doesn't decode, a text patch, `visuals.toml` or `effects.toml` that doesn't
parse keeps the version loaded before; the problem goes to the message line in a game and to the Mods page's problem
line; the next save of the file is tried again.

**Multiplayer**: no reload runs while a multiplayer game or room is open (the files are hashed into the room's
fingerprint at its start); changes made meanwhile are picked up after it.

Port status: shipped (`ig2-client/src/window/mod_dev.rs`, `ig2-formats/src/mod_watch.rs`, `Renderer::reload_file`, `Audio::reload`, `upscale::forget`).

### 3. Story console (row 581): planned

An in-game console for story mods, opened with `--mod-dev`: the stories loaded, which have fired, their timers and
flags, and commands to fire a story, set a flag or a timer, jump the date; single player only. Its design is written
when it is built.

## Consequences

- A mod starts with one command or one button, valid and commented; the reference is a step away in its README.
- With `--mod-dev`, a texture, sound, text or look file saved in an editor shows in the running game within a second;
  data that changes the game asks for Apply, so a game's state never changes under it.
- Nothing changes without the flag; nothing changes in multiplayer.
- Rejected: a file-watcher crate (a new dependency, platform differences, and a poll of a mod's few files costs
  nothing); reloading rules or stories into a running game (the sim's state and saves assume one set per game); live
  reload of zips (a zip is a finished, shared mod).
