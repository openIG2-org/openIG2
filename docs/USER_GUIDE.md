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
Options → the game tab, under the original's rows and a "Remake" heading. A press cycles the row; changes apply
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

The window opens at 1600x900 until a size is saved.

## Checking your install
`ig2-check [INSTALL_DIR] [--quick]` (in the release zip, or `cargo run --release -p ig2-check`) loads every asset,
table and script of your install the way the game does and lists what fails; it finds the install like the client
when no folder is given. `--quick` skips the sound and video decoding. Exit code 0 = all ok, 1 = failures, 2 = no
install. A full run takes about a minute. `--mod NAME` (repeatable) checks those mods instead of `mods/enabled.txt`'s;
the "mods" line lists every file each enabled mod replaces or adds, and its files are loaded with the install's.

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

`ig2.exe` can't be replaced: the rule tables are read from it (changing rules and scripts is a later step). In
multiplayer every player needs the same mod files: the room refuses a player with other mods ("Other mods
enabled"), and a multiplayer save made with mods resumes only with them.

## Asset viewer
`cargo run --release -p ig2-viewer` browses the install's models, textures, colony maps, animations and messages.
