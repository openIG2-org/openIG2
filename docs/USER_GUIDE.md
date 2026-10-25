# User guide

## Installing
You need the GOG release of *Imperium Galactica II*; the remake ships no game data and reads your copy. At start
it looks, in this order, in:
1. `IG2_INSTALL_DIR` (an override; wins over everything);
2. the folder remembered in our settings file (`ig2-rework-options.dat`, key `InstallDir`, next to the original's
   `options.dat` under `%LOCALAPPDATA%\GyroscopeGames\ig2`, or `IG2_OPTIONS_DIR`);
3. the GOG registry entries (GOG Galaxy writes the same ones);
4. the default folders `<drive>:\GOG Games\*`, `<drive>:\Games\*` and `<drive>:\Program Files [(x86)]\GOG
   Galaxy\Games\*` (subfolders named "Imperium Galactica…");
5. the remake's own folder and its parent.

A folder counts when it has `dataraw/text/scenario.txt` and the GOG 2017 build of `ig2.exe` (checked by hash). A
folder found by the search is remembered, so later starts skip it. If nothing is found, a window says so, lists
each place tried and why it failed, and takes the game's folder (or its `ig2.exe`) dropped onto it: it is checked,
remembered, and the game restarts. Esc quits. On other systems than Windows the message goes to stderr.

## Running
Until there is a release build (see `TASKS.md`), run from source (`docs/BUILDING.md`):
```bash
cd engine
cargo run --release -p ig2-client
```
The game starts at the title and main menu, like the original: campaign, scenario, skirmish, tutorial, load, and
the multiplayer rings (LAN or direct IP; our MP room replaces the original's IRC screen).

| Flag | |
|---|---|
| `--save <file.sav>` | Start straight into an original or our own save |
| `--new-campaign` | Start a new campaign at once |
| `--speed 1\|2\|3` | Starting game speed |
| `--lang en\|hu\|de\|fr\|es\|it` | UI language |
| `--size WxH`, `--letterbox` | Window size; letterbox wide windows instead of the remaster's layout |

## Controls
The controls are the original's. The most used:

| Key / mouse | |
|---|---|
| Space, 1/2/3 | Pause, game speed |
| F1 | Help |
| Mouse wheel | Zoom |
| Starmap: left press | Select; hold for the radial icons, release over one to use it |
| Starmap: left click on a planet's info box | The planet's long info (tax, auto-build, priority, station sale) |
| Starmap: Shift + right click | Waypoint chains |
| Starmap: arrows, numpad + / −, Page Up / Down, C | Turn, tilt, zoom; step through own planets and fleets; colonise |
| C, I, M, D | Colony, info, messages, diplomacy screens |
| Colony: both buttons dragged | Turn and tilt the camera |
| Colony: hold on the planet panel | The build menu |
| Esc | Back |
| V | Vsync on/off |

The full key and hot-spot tables per screen are in `re/screens/input.md`.

## Saves and settings
Our own saves go to the Options screen's slots (`IG2_SLOT_DIR`); original `.sav` files load with `--save`. Our
settings are kept in our own file (`IG2_OPTIONS_DIR`), never in the original's.

## Asset viewer
`cargo run --release -p ig2-viewer` browses the install's models, textures, colony maps, animations and messages.
