# User guide

## Installing
You need the GOG release of *Imperium Galactica II*; openIG2 ships no game data and reads your copy. The simplest
setup: unzip the release into the game's folder and run `ig2-client.exe`. Nothing else is needed. At start the
client looks, in this order, in:
1. `IG2_INSTALL_DIR` (optional: an override that wins over everything);
2. the folder remembered in our settings file (`ig2-rework-options.dat`, key `InstallDir`, next to the original's
   `options.dat` under `%LOCALAPPDATA%\GyroscopeGames\ig2`, or `IG2_OPTIONS_DIR`);
3. the client's own folder and its parent (the release unzipped into the game's folder);
4. the GOG registry entries (GOG Galaxy writes the same ones);
5. the default folders `<drive>:\GOG Games\*`, `<drive>:\Games\*` and `<drive>:\Program Files [(x86)]\GOG
   Galaxy\Games\*` (subfolders named "Imperium Galactica…").

A folder counts when it has `dataraw/text/scenario.txt` and the GOG 2017 build of `ig2.exe` (checked by hash). A
folder found by the search is remembered, so later starts skip it. If nothing is found, a window says so, lists
each place tried and why it failed, and takes the game's folder (or its `ig2.exe`) dropped onto it: it is checked,
remembered, and the game restarts. Esc quits. On other systems than Windows the message goes to stderr.

## Running
Download `openig2-<version>-windows-x64.zip` from the
[releases](https://github.com/openIG2-org/openIG2/releases/latest), unzip it into the game's folder (or anywhere)
and run `ig2-client.exe`. Or run from source (`docs/BUILDING.md`):
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

### Display settings
Options → the game tab, under the original's rows and a "Remake" heading. A press cycles the row; changes apply
at once and are saved in `ig2-rework-options.dat`.

| Row (key) | |
|---|---|
| Display (`DisplayMode`) | Window, borderless, or exclusive fullscreen (`0`, `1`, `2`) |
| Resolution (`Resolution`, `WxH`) | The window size, or the fullscreen mode; the monitor's modes plus common sizes that fit it, largest first. A size the monitor has no mode for falls back to borderless. Disabled while borderless |
| Vertical sync (`VSync`) | On by default; `IG2_VSYNC` overrides at start |
| Letterbox 4:3 (`Letterbox`) | Off by default; `--letterbox` overrides at start |
| Live background (`LiveBackground`) | Overlay screens redraw the map live (ADR 0015) |
| Names in game language (`OneLanguage`) | ADR 0017 |

The window opens at 1600x900 until a size is saved.

## Checking your install
`ig2-check [INSTALL_DIR] [--quick]` (in the release zip, or `cargo run --release -p ig2-check`) loads every asset,
table and script of your install the way the game does and lists what fails; it finds the install like the client
when no folder is given. `--quick` skips the sound and video decoding. Exit code 0 = all ok, 1 = failures, 2 = no
install. A full run takes about a minute.

## Asset viewer
`cargo run --release -p ig2-viewer` browses the install's models, textures, colony maps, animations and messages.
