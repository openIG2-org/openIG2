# User guide

## Installing
You need the GOG release of *Imperium Galactica II*; openIG2 ships no game data and reads your copy. The simplest
setup: unzip the release into the game's folder and run `ig2-client.exe`. Nothing else is needed. At start the
client looks, in this order, in:
1. `IG2_INSTALL_DIR` (optional: an override that wins over everything);
2. the folder remembered in our settings file (`ig2-rework-options.dat`, key `InstallDir`, next to the original's
   `options.dat` under `%LOCALAPPDATA%\GyroscopeGames\ig2`, on Linux `~/.local/share/GyroscopeGames/ig2`
   (`$XDG_DATA_HOME` if set), on macOS `~/Library/Application Support/GyroscopeGames/ig2`, or `IG2_OPTIONS_DIR`);
3. the client's own folder and its parent (the release unzipped into the game's folder);
4. the GOG registry entries (GOG Galaxy writes the same ones); on Linux and macOS, Heroic's GOG
   library (`~/.config/heroic/gog_store/installed.json`, the Flatpak's, `~/Library/Application Support/heroic`);
5. the default folders `<drive>:\GOG Games\*`, `<drive>:\Games\*` and `<drive>:\Program Files [(x86)]\GOG
   Galaxy\Games\*` (subfolders named "Imperium Galactica…"); on Linux `~/GOG Games/*`, `~/Games/*`,
   `~/Games/Heroic/*` and the same GOG folders inside the Wine prefixes `~/.wine/drive_c` and Lutris'
   `~/Games/<game>/drive_c`; on macOS also `~/Applications/*`, `/Applications/*` and the CrossOver and Whisky
   bottles (`~/Library/Application Support/CrossOver/Bottles/*/drive_c`,
   `~/Library/Containers/com.isaacmarovitz.Whisky/Bottles/*/drive_c`).

A folder counts when it has `dataraw/text/scenario.txt` and the GOG 2017 build of `ig2.exe` (checked by hash). A
folder found by the search is remembered, so later starts skip it. If nothing is found, a window says so, lists
each place tried and why it failed, and takes the game's folder (or its `ig2.exe`) dropped onto it: it is checked,
remembered, and the game restarts. Esc quits.

## Running
Download `openig2-<version>-windows-x64.zip` from the
[releases](https://github.com/openIG2-org/openIG2/releases/latest), unzip it into the game's folder (or anywhere)
and run `ig2-client.exe`. Or run from source (`docs/BUILDING.md`):
```bash
cd engine
cargo run --release -p ig2-client
```
On Linux (x86-64, glibc 2.35 or newer, Vulkan drivers, ALSA or PipeWire/PulseAudio, X11 or Wayland; the Steam
Deck's desktop mode works the same) build from source, or take `openig2-<version>-linux-x64.tar.gz` from the
releases once one carries it, unpack it anywhere and run `./ig2-client`. The game itself has to be installed
once: the GOG offline installer through Wine, Heroic or Lutris all leave the files where the search above finds
them (or drop the folder with `ig2.exe` onto the not-found window). The credits line uses an outline font instead
of Windows' Arial. Linux builds are new: please report anything that differs from the Windows build.

On macOS (11 or newer, Apple Silicon; Metal) build from source, or take `openig2-<version>-macos-arm64.tar.gz`
from the releases once one carries it. Its binaries are not signed: clear the download quarantine once with
`xattr -dr com.apple.quarantine .` in the unpacked folder (or right-click `ig2-client` in Finder → Open), then run
`./ig2-client`. The game is Windows-only, so copy the folder of a Windows install to the Mac (anywhere: point
`IG2_INSTALL_DIR` at it or drop it onto the not-found window), or install it through CrossOver, Whisky or Heroic.
macOS builds are new and tested by CI only: please report what goes wrong.
The game starts at the title and main menu, like the original: campaign, scenario, skirmish, tutorial, load, and
the multiplayer rings (LAN, direct IP or join code, optionally through a relay server you or a friend run:
`docs/RENDEZVOUS.md`; our MP room replaces the original's IRC screen). To join by code, paste the host's join code
(Ctrl+V) into the "Address or join code" field with no LAN game selected and press Join.
In the room every empire is a card: its colour, the race's picture, the holder and a mark ("Host", "Ready" or "Not ready"). If you press Start as host while someone is not ready, the room says so and the button becomes "Start anyway"; pressing it again readies everyone and starts.
For a quick game, click "Quick game" at the bottom right of the main menu: the skirmish settings open on their own
(race, computer players, difficulty, planets, technology, money, population), set to your last skirmish's choices;
Start begins a generated galaxy. The full single-player path is still there.
Hosting asks your router to forward the game port (UPnP / NAT-PMP); the room shows whether it worked, and a
click on that line switches it off. A generated galaxy's room also has an off-by-default "Space battle formations"
row: with it on, each human side with six or more ships picks a formation before a real-time space battle starts.
Its "Defeated players" row (off: leave, as in the original; on: stay and watch) lets a player whose empire falls
return to the starmap, see the whole map and chat, without giving orders, until they leave. With it on, others can
also join the running game with Join under a new name to watch it.
Its "Computer empires" row (classic, as in the original; or enhanced, ADR 0045) sets how the computer empires run
their economy and research for the whole game.
Each computer slot shows its level after "Computer": easy (the default, as in the original), normal or hard. The
host clicks it to raise the level and right-clicks to lower it; the computer empire then plays at that difficulty.

| Flag | |
|---|---|
| `--save <file.sav>` | Start straight into an original or our own save |
| `--new-campaign` | Start a new campaign at once |
| `--speed 1\|2\|3` | Starting game speed |
| `--lang en\|hu\|de\|fr\|es\|it` | UI language; wins over `IG2_LANG`, which wins over the Options Language row, then the original's own choice |
| `--multiplayer` | Open the main menu on the multiplayer connection page |
| `--size WxH`, `--letterbox` | Window size; letterbox wide windows instead of the remaster's layout |
| `--sharp-ui` | Sharp UI on for this run, over the settings file |
| `--sharp-text` | Sharp text on for this run, over the settings file |

## Controls
The controls are the original's. The most used:

| Key / mouse | |
|---|---|
| Space, 1/2/3 | Pause, game speed |
| F1 | Help |
| Mouse wheel | Zoom (on the starmap: Remake row "Starmap mouse camera") |
| Starmap: right drag | Move the map (Remake row "Starmap mouse camera") |
| Starmap: left press | Select; hold for the radial icons, release over one to use it |
| Starmap: left click on a planet's info box | The planet's long info (tax, auto-build, priority, station sale) |
| Starmap: Shift + right click | Waypoint chains |
| Starmap: arrows, numpad + / −, Page Up / Down, C | Turn, tilt, zoom; step through own planets and fleets; colonise |
| C, I, M, D | Colony, info, messages, diplomacy screens |
| Colony: both buttons dragged | Turn and tilt the camera |
| Colony: hold on the planet panel | The build menu |
| Research: Auto ships / buildings / tanks | Ours: while on, an idle research slot starts the next tech of that kind by itself (off by default) |
| Esc | Back |
| V | Vsync on/off |
| F12 / Shift+F12 | Quick save / quick load (single player; in multiplayer F12 saves on the host) |

The full key and hot-spot tables per screen are in `re/screens/input.md`.

## Saves and settings
Our own saves go to the Options screen's slots (`IG2_SLOT_DIR`); original `.sav` files load with `--save`. Our
settings are kept in our own file (`IG2_OPTIONS_DIR`), never in the original's.

F12 quick-saves into `quicksave.ig2s` beside the slots and Shift+F12 loads it back (ADR 0032); both work where the
Options screen could save (not in the tutorial or a battle). The game autosaves into slot 24 ("Autosave …") on the
first day of months 1 and 7, as the original does, while the original's "Auto Save" row is on. The Remake row
"Autosave every" saves every 10, 30 or 90 game days instead and keeps the autosave before it in slot 23, so the
load list shows the last two.

Every game also writes a replay (`replays/<date>_<time>_sp.ig2r` or `_mp`) beside the slots, and the newest 50 are
kept (ADR 0038). The main menu's "Replays" item (bottom left, above "History") lists them, newest first; a click
plays one back read-only: the whole map is shown, the screens can be looked at but take no orders. The bar under the
top bar has Pause/Play (Space), the speeds 1x, 2x, 4x and 8x (keys 1-4), the progress line and Exit; at the end the
list comes back. A replay only plays on the version that recorded it: one that drifts stops and says where. The Remake
row "Record replays" (`RecordReplays`, on by default) turns recording off.

### Display settings
Options → the REMAKE tab, grouped (Screen, Interface, Game, Accessibility), with a "Page" row to turn to the next page. A press cycles the row; changes apply
at once and are saved in `ig2-rework-options.dat`. The Options screen also opens from the main menu (the Settings label at the bottom right, or the `O` key), without its slot list, Load and Save.

| Row (key) | |
|---|---|
| Display (`DisplayMode`) | Window, borderless, or exclusive fullscreen (`0`, `1`, `2`) |
| Resolution (`Resolution`, `WxH`) | The window size, or the fullscreen mode; the monitor's modes plus common sizes that fit it, largest first. A size the monitor has no mode for falls back to borderless. Disabled while borderless |
| Vertical sync (`VSync`) | On by default; `IG2_VSYNC` overrides at start |
| Letterbox 4:3 (`Letterbox`) | Off by default; `--letterbox` overrides at start |
| Live background (`LiveBackground`) | Overlay screens redraw the map live (ADR 0015) |
| Names in game language (`OneLanguage`) | ADR 0017 |
| Clean name entry (`CleanNameEntry`) | On by default: typing a save name hides the slot's old name behind it (ADR 0024); off draws the original's look |
| Models inside boxes (`BoxModelsInside`) | On by default: the turning planet or ship model in the starmap's and space battle's bottom-left info box is cut to the box (ADR 0025); off lets it reach past the edge as in the original |
| Slider under label (`SliderUnderLabel`) | On by default: the space battle's defend/attack slider sits under its label instead of running through it (ADR 0027); off draws the original's look |
| Hungarian articles (`HungarianArticles`) | On by default: in a Hungarian game the article markers become "a" or "az" in messages, diplomacy and the battle banners (ADR 0028); off shows the original's raw marker character |
| Centred Exit label (`CentredExitLabel`) | On by default: the Exit label of the production view, transfer panel and fleet panel is centred on its button (ADR 0029); off draws it left of centre as in the original |
| Game statistics (`GameStatistics`) | On by default (ADR 0034): after a game's end videos a statistics screen shows the game's length and result, a table per empire (planets, population, credits, ships, ships built and lost, techs, battles won, score) and graphs over time (pick the value with its tabs or Left/Right; Esc, Enter or Continue goes on to the menu). Every finished game is added to `match-history.json` (in `IG2_SAVE_DIR`, else beside the save slots); the main menu's "History" item lists them, newest first, and opens each one's screen. Multiplayer games are recorded on every machine. Off: the original's end, nothing recorded |
| WASD camera (`WasdCamera`) | Held W/A/S/D move the camera at the edge-scroll speed (ADR 0030): Starmap (default), Both (also the colony screen, where they replace the `a` select-all and `s` stop keys) or Off; never while you type a name or chat |
| Autosave every (`AutosaveEvery`) | Original (default: slot 24 twice a game year), or every 10, 30 or 90 game days with the previous autosave kept in slot 23 (ADR 0032); in multiplayer only these cadences autosave, on the host |
| Settings in menu (`MenuSettings`) | On by default: the main menu shows a Settings label and opens Options on `O` (ADR 0031), and a Quick game label (ADR 0039); off keeps the original's menu |
| Language (`Language`) | Default (the client's own choice), English, Magyar, Deutsch, Francais, Espanol or Italiano (ADR 0031); applies at the next start, the screen says "Restart to change the language"; greyed while `--lang` or `IG2_LANG` is set |
| Fleet groups (`FleetGroups`) | On by default: on the starmap Ctrl+1…9 put the selected fleet in a group, Alt+1…9 select the group's fleet, pressed again the view centres on it (ADR 0035); the plain 1…3 stay the speed keys; the groups are saved beside the save slot (`save<n>.groups`); off leaves Ctrl/Alt+1…3 as speed keys as in the original |
| Record replays (`RecordReplays`) | On by default: each game writes a replay file (`replays/*.ig2r`); off = the original (none) (ADR 0038) |
| Event log (`EventLog`) | On by default: the messages screen (F10) has an "EVENT LOG" label (or press `L`) that lists every message line of this game, newest first; click All, Battles, Research, Diplomacy, Colonies or Story to filter, type to search (Backspace deletes, Esc closes), scroll with the wheel, the arrow keys or PgUp/PgDn; clicking a line about a planet or a fleet shows it on the starmap. The log is not saved: a loaded game starts with an empty one (ADR 0040); off shows the original screen |
| Subtitles (`Subtitles`) | Off by default: a spoken message line's text stays at the bottom of the screen while its voice plays (ADR 0036). The install has no texts for the videos, the campaign start voice or the units' voices, so these get none; the message screen types its spoken text anyway |
| Colour-blind colours (`ColourBlindColours`) | Off by default: empires 1–8 get colours that stay apart with red-green or blue-yellow colour blindness, in names, the minimap, the info screen, the top bar and the fleet icons (ADR 0036) |
| Sharp UI (`SharpUi`) | Off by default: the panels, text and pointer are drawn with crisp texel edges instead of the original's smooth scaling, for big windows (1440p, 4K) where the 640×480 layout is scaled several times (ADR 0043); the UI's size follows the window either way |
| Sharp text (`SharpText`) | Off by default: on big windows (1440p, 4K) the game's own font is drawn with sharp, smooth edges instead of the original's blur, every letter in the same place and size (ADR 0046); combines with Sharp UI; at 800x600 nothing changes |
| Starmap mouse camera (`StarmapMouseCamera`) | On by default: the wheel zooms the starmap and a right drag moves it; an unmoved right click gives its order on the release (ADR 0049); off = the original: no wheel, the right press orders at once |
| High-score entry (`HighScoreEntry`) | On by default: after a won scenario the main menu opens its list in high-score view and asks for your name (Enter keeps it; the score is the days the win took, fewer is better); the tables are kept in the remake's settings file (ADR 0050); off = the original, which never asks |
| Computer empires (`EnhancedAi`) | Classic by default: the computer empires of a new single-player game play as in the original. Enhanced (ADR 0045): they expand faster, build the labs their research needs, buy every planet's building when they can pay, set taxes by morale and research a colony ship when there are planets to take. A game keeps the choice it started with (saved with it); a multiplayer room has its own row |

The window opens at 1600x900 until a size is saved.

## Checking your install
`ig2-check [INSTALL_DIR] [--quick]` (in the release zip, or `cargo run --release -p ig2-check`) loads every asset,
table and script of your install the way the game does and lists what fails; it finds the install like the client
when no folder is given. `--quick` skips the sound and video decoding. Exit code 0 = all ok, 1 = failures, 2 = no
install. A full run takes about a minute. `--mod NAME` (repeatable) checks those mods instead of `mods/enabled.txt`'s;
the "mods" line lists every file each enabled mod replaces or adds and checks each mod's `rules.toml`, and its files
are loaded with the install's.

## Mods
A mod replaces the install's files with your own: a sharper texture, another text, another sound (ADR 0044). The
remake still ships no game files; a mod is files you make or download.

1. Find the mods folder: `mods` beside the settings files (`%LOCALAPPDATA%\GyroscopeGames\ig2\mods`, or
   `IG2_OPTIONS_DIR\mods`). Make it if it is missing.
2. Make a folder for your mod in it, for example `mods\sharp-menu`.
3. Put each file at the same path it has in the install:
   `mods\sharp-menu\dataraw\textures\main\eng\unselected\0_credits_bmp.pvr` replaces the install's
   `dataraw\textures\main\eng\unselected\0_credits_bmp.pvr`. Names are not case-sensitive. A mod's file also beats
   the install's translated copy under `localization\<language>\`. Files may be plain (textures as PowerVR v3
   `.pvr`, texts in the game's text format) or in the install's own packed form.
4. Enable it: write its folder name in `mods\enabled.txt`, one mod a line, the most important first (when two mods
   have the same file, the one higher in the list wins; `#` starts a comment). Or start the client with
   `--mod sharp-menu` (repeatable) for one run, which replaces the list.
5. Check it: `ig2-check --quick` lists the files your mod replaces or adds, and loads them.

`ig2.exe` can't be replaced: the rule tables are read from it. To change rules, put a `rules.toml` at the top of
the mod folder (`mods\cheap-labsules.toml`); it sets single fields of the tables by name. A record is
`[<table>.<id>]` with `field = value` lines; a list is `[<list>]` with `<index> = value` lines. The ids and the
current values are in the file `cargo run --release -p ig2-rules --bin ig2-rules-dump <install> rules.ron` writes
(keep it to yourself: it holds the game's values). An example (made-up numbers):

```toml
# Tech 12 is cheaper and quicker to research.
[techs.12]
research_cost = 500
research_time = 20
lab_levels = [1, 0, 0]

# Building 4 needs less energy.
[buildings.4]
energy = 3

# Race 2 produces half again as fast.
[race_production_mult]
2 = 1.5

# Ground weapon 3 fires faster in the real-time ground war.
[ground_rt_weapons.3]
reload = 10
```

The tables and their fields are listed in ADR 0044 §6 (`techs`, `buildings`, `hulls`, `space_weapons`,
`ship_types`, `spy_missions`, `problem_curves`, the ground weapons and the race, difficulty, radar and spy lists).
When several mods change the same field, the one higher in `enabled.txt` wins. A wrong table, id or field stops the
game with a message naming the mod and the field, and `ig2-check` shows the same message. Story scripts are files in
`dataraw\`, so a mod replaces them whole like any other file.

In multiplayer every player needs the same mod files and `rules.toml`s: the room refuses a player with other mods
("Other mods enabled"), and a multiplayer save made with mods resumes only with them.

## Asset viewer
`cargo run --release -p ig2-viewer` browses the install's models, textures, colony maps, animations and messages.
