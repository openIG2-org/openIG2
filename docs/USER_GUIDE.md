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
and run `ig2-client.exe`. It opens no console window: its messages and errors (a crash's too) go to
`ig2-client.log` beside the settings (`%LOCALAPPDATA%\GyroscopeGames\ig2`, or `IG2_OPTIONS_DIR`), emptied at
each start; started from a terminal it prints there instead (`--version`, the mod list). Attach the log to a bug
report. Or run from source (`docs/BUILDING.md`):
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
the multiplayer rings (LAN, direct IP or join code, through the project's relay server `ig2relay.justdawee.hu:30000` by default, or
one you or a friend run: `docs/RENDEZVOUS.md`; our MP room replaces the original's IRC screen). To join by code, paste the host's join code
(Ctrl+V) into the "Address or join code" field of the "Join directly" card and press Join. The connection page lists
games in three tabs: LAN games, Internet games (the rooms on the relay server's public list, set by default; a host
switches "Public list" on in its room) and Saved games (a multiplayer save to resume as host); a double click (or Up,
Down and Enter) joins a row, Tab moves between the fields.
In the room every empire is a row: its colour, the race's picture, the race, the holder and a mark ("Host", "Ready",
"Not ready", "You"); the host sets a slot to Open, Computer (Easy, Normal or Hard) or Closed in its drop-down, and the
galaxy settings with the arrows; "Take seat" moves you to a free slot; the host copies the join code with "Copy join
code". If you press Start as host while someone is not ready, the room says so and the button becomes "Start anyway";
pressing it again readies everyone and starts.
For a quick game, click "Quick game" at the bottom right of the main menu: the skirmish settings open on their own
(race, computer players, difficulty, planets, technology, money, population), set to your last skirmish's choices;
Start begins a generated galaxy. The full single-player path is still there.
With "Menu navigation: Enhanced" (Options, Interface) the main menu shows one panel at the left instead of the
rotating rings: Continue (loads your newest save, autosave or quick save), Quick game, New game, Load, Multiplayer,
Tutorial, History, Replays, Credits, Settings and Quit. New game, Multiplayer, Tutorial and the difficulty choice
after picking a race show their choices in the same panel. On every menu screen Esc or a right click on empty space goes one step back, and a Back label at the bottom
left shows where a screen has no back button of its own.
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
| `--smooth-glows` | Smooth glows on for this run, over the settings file |
| `--modern-hud` | Draw the modern starmap HUD in snapshots (without it they draw the original's; input scripts always click the original's) |
| `--modern-menu` | Draw the modern in-game menu in snapshots of the Options screen (without it they draw the original's; input scripts always use the original's) |
| `--sharp-text` | Sharp text on for this run, over the settings file |
| `--upscale 2\|4` | Texture upscaling at 2× or 4× for this run, over the settings file |
| `--modern-lighting` | Anti-aliasing, per-pixel lighting, soft shadows and bloom on for this run, over the settings file |
| `--theme NAME` | UI theme for this run (Classic, Modern or a mod folder's), over the settings file |
| `--battle-effects` | Enhanced battle effects for this run, over the settings file |

## Controls
The controls are the original's; the main menu's Options screen → Controls changes the keys (see below). The most used:

| Key / mouse | |
|---|---|
| Space, 1/2/3 | Pause, game speed (with the Remake row "Extra game speeds": 3 again goes x2, then x4 faster) |
| F1 | Help |
| Mouse wheel | Zoom (on the starmap: Remake row "Starmap mouse camera") |
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
Our own saves go to the Options screen's slots (`IG2_SLOT_DIR`; with the modern in-game menu on, its "Save & load" tab shows them: click a slot, double click or Load loads it, Save asks for a name); original `.sav` files load with `--save`. Our
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
In a game with "Modern in-game menu" off: Options → the REMAKE tab, grouped (Screen, Interface, Game, Accessibility, Language), with a "Page" row to
turn to the next page. A press cycles the row; changes apply at once and are saved in `ig2-rework-options.dat`.

In the main menu, the Settings label at the bottom right (or the `O` key) opens the modern Options screen with every
setting, the original's and ours, in tabs: Display, Sound, Interface, Game, Controls, Accessibility, Multiplayer
(your name in multiplayer games) and Language. Each setting has its description under its label, and every tab scrolls with the wheel. Click a
switch, a drop-down or a slider's track; Apply saves and applies the changes, Back leaves and takes back
what wasn't applied. Keyboard: Tab / Shift+Tab (or Page Down / Up) change the tab, Up / Down move between the settings
and the buttons, Left / Right change a value (sliders by 5), Enter or Space toggles or presses, Esc is Back. Both
screens show the same values.

Controls (ADR 0058) lists every key you can change: the screen keys (F1–F10), M, O, pause, the speeds, quick save,
chat, and each screen's own keys (the starmap's, the colony's, research, design, messages, the space battle), with
the original's keys as the defaults. Click a line (or Enter on it) and press the new key; Esc keeps the old one. A key
already used by another action where both apply is shown in the warning colour, and the line above the buttons names
the other action; it is allowed, but only the first action in the list gets it. "Reset all keys" brings back the
original's. The list scrolls with the wheel or the arrow keys. Esc, Enter, the arrows, the digit groups and Ctrl,
Shift and Alt can't be changed; typing names and chat always uses the real keys. The keys are saved as `Bind<name>`.

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
| Menu navigation (`MenuNavigation`) | Classic by default: the original's menus. Enhanced: the grouped main-menu panel with Continue, and Esc / right click / Back one step back on every menu screen (ADR 0059) |
| Settings in menu (`MenuSettings`) | On by default: the main menu shows a Settings label and opens Options on `O` (ADR 0031), and a Quick game label (ADR 0039); off keeps the original's menu |
| Language (`Language`) | Default (the client's own choice), English, Magyar, Deutsch, Francais, Espanol or Italiano (ADR 0031); applies at the next start, the screen says "Restart to change the language"; greyed while `--lang` or `IG2_LANG` is set |
| Fleet groups (`FleetGroups`) | On by default: on the starmap Ctrl+1…9 put the selected fleet in a group, Alt+1…9 select the group's fleet, pressed again the view centres on it (ADR 0035); the plain 1…3 stay the speed keys; the groups are saved beside the save slot (`save<n>.groups`); off leaves Ctrl/Alt+1…3 as speed keys as in the original |
| Record replays (`RecordReplays`) | On by default: each game writes a replay file (`replays/*.ig2r`); off = the original (none) (ADR 0038) |
| Event log (`EventLog`) | On by default: the messages screen (F10) has an "EVENT LOG" label (or press `L`) that lists every message line of this game, newest first; click All, Battles, Research, Diplomacy, Colonies or Story to filter, type to search (Backspace deletes, Esc closes), scroll with the wheel, the arrow keys or PgUp/PgDn; clicking a line about a planet or a fleet shows it on the starmap. The log is not saved: a loaded game starts with an empty one (ADR 0040); off shows the original screen |
| Subtitles (`Subtitles`) | Off by default: a spoken message line's text stays at the bottom of the screen while its voice plays (ADR 0036). The install has no texts for the videos, the campaign start voice or the units' voices, so these get none; the message screen types its spoken text anyway |
| Colour-blind colours (`ColourBlindColours`) | Off by default: empires 1–8 get colours that stay apart with red-green or blue-yellow colour blindness, in names, the minimap, the info screen, the top bar and the fleet icons (ADR 0036) |
| Sharp UI (`SharpUi`) | Off by default: the panels, text and pointer are drawn with crisp texel edges instead of the original's smooth scaling, for big windows (1440p, 4K) where the 640×480 layout is scaled several times (ADR 0043); the UI's size follows the window either way |
| Sharp text (`SharpText`) | Off by default: on big windows (1440p, 4K) the game's own font is drawn with sharp, smooth edges instead of the original's blur, every letter in the same place and size (ADR 0046); combines with Sharp UI; at 800x600 nothing changes |
| Modern starmap HUD (`ModernStarmapHud`) | On by default, under "Interface": on the starmap the top bar (with the screens as a row of icons), the selection's compact info panel (icon buttons for details, colony or split, tanks and production), and the minimap with a strip of camera buttons under it are redrawn as small console plates in your empire colour that follow the UI scale; the radial icon menu, the select menus (tax, auto-build, priority, a station's sale, the planet list) and a planet's long info get the same look on the original's places; the production, transfer, fleet and vehicle panels stay the original's. Off = the original's HUD (ADR 0065) |
| Modern in-game menu (`ModernGameMenu`) | On by default, under "Interface": Esc or `O` in a game, and the main menu's Load game, open the main menu's Options page with a first tab "Save & load" (the 24 slots, a preview, Load, Save, Quit to main menu, Exit game); settings wait for Apply. Off = the original's Options screen (ADR 0067) |
| Modern colony screen (`ModernColony`) | On by default, under "Interface": the colony screen's panels (build menu, short info with repair, demolish, on/off and self-destruct icons, the ground war's buttons and sides, the radar) are redrawn as small console plates in your empire colour, with the same functions and keys; the planet info mode keeps the original's panels. Off = the original's panels (ADR 0070) |
| UI scale (`UiScale`) | 100 % by default, 50–200 % in steps of 10, under "Interface": the size of the remake's own pages (Options, multiplayer) and the modern starmap HUD; they grow with the window height, and 200 % is their size in earlier versions. On the Options page it is a slider: the new size shows when you let go, Back takes it back, Apply keeps it; no restart. The original screens always fill the window. `--ui-scale P` sets it for one run (ADR 0062) |
| Smooth glows (`SmoothGlows`) | On by default (off = the original's look), under "Graphics": the starmap's star glows, clouds and stars, the main menu's galaxy and the space battle's shot flares are drawn without a visible pixel grid, one-pixel stars as round dots, and the sky's nebulae get a smooth enlarged copy (from the next start); Sharp UI does not apply to these (ADR 0064) |
| Texture upscaling (`TextureUpscale`) | Off / 2x / 4x, off by default: the textures are enlarged as they load, interface art with an edge-keeping filter (xBRZ) and the rest smoothly, so pictures and models look less blocky on big windows; every picture keeps its place and size (ADR 0054). The first visit to a screen takes longer (the starmap about 4 s at 4x); a change shows on the screens opened afterwards. A texture pack (Mods) wins over it |
| Anti-aliasing (`AntiAliasing`) | Off by default, under "Graphics": the 3D scenes (starmap planets, colony, space and ground battles, the models in the panels) are drawn with 4× multisampling, so their edges don't stair-step; the 2D panels and text are not touched (ADR 0055) |
| Per-pixel lighting (`PixelLighting`) | Off by default: the space battle's ships and flying pieces are lit per pixel by the same lights instead of per corner, so their hulls shade smoothly (ADR 0055) |
| Soft shadows (`SoftShadows`) | Off by default: the colony's building shadows get soft edges instead of hard ones (ADR 0055) |
| Bloom (`Bloom`) | Off by default: the glowing 3D effects (fire, blasts, the colony's weather) get a soft glow around them (ADR 0055) |
| Battle effects (`BattleEffects`) | Classic by default, under "Graphics": Enhanced adds sparks, expanding shock rings, embers, smoke and smoke columns to the explosions of the space battle and the ground war, a blue flash where a shield takes a hit and a soft glow round every shot and laser beam, on top of the original's own effects and at the same moments (ADR 0057). Takes effect at once |
| Starmap mouse camera (`StarmapMouseCamera`) | On by default: the wheel zooms the starmap (ADR 0049); off = the original: no wheel. The right click always gives its order at once |
| High-score entry (`HighScoreEntry`) | On by default: after a won scenario the main menu opens its list in high-score view and asks for your name (Enter keeps it; the score is the days the win took, fewer is better); the tables are kept in the remake's settings file (ADR 0050); off = the original, which never asks |
| Computer empires (`EnhancedAi`) | Classic by default: the computer empires of a new single-player game play as in the original. Enhanced (ADR 0045): they expand faster, build the labs their research needs, buy every planet's building when they can pay, set taxes by morale and research a colony ship when there are planets to take. A game keeps the choice it started with (saved with it); a multiplayer room has its own row |
| Title music with intro (`TitleMusicWithIntro`) | On by default: the title video is silent over the game's and the studio's logos and its music starts with the intro (ADR 0051); off = the original: the music from the first logo |
| Campaign story (`StoryFixes`) | Classic by default: a new campaign's story plays as in the original. Enhanced (ADR 0052): the original's story slips are fixed; so far the siege episode's five enemy fleets start at the planet they besiege instead of the galaxy's corner. The game keeps the choice it started with (saved with it) |
| Voice volume (`VoiceVolume`) | Under "Sound": Same as sound (default, the original: speech follows the sound slider) or 0–100 %: the advisors', messages', start and aliens' speech at their own volume, the sound slider then sets the effects only (ADR 0058) |
| Extra game speeds (`ExtraSpeeds`) | Off by default: in single player the fastest speed's key (3) or top-bar button pressed again while it runs makes the game go twice, then four times faster ("x2"/"x4" by the button); any other speed or a slow-down (an enemy near, a message) ends it, a space battle runs at its usual speed (ADR 0058). Multiplayer keeps the room's speeds |
| UI theme (`UiTheme`) | Classic by default: the remake's own screens (main-menu Options, history, replays, lobby, event log) in their usual look; Modern or a mod's theme restyles them at once (ADR 0056, Mods → UI themes). The game's own screens never change |

The window opens at 1600x900 until a size is saved.

## Checking your install
`ig2-check [INSTALL_DIR] [--quick]` (in the release zip, or `cargo run --release -p ig2-check`) loads every asset,
table and script of your install the way the game does and lists what fails; it finds the install like the client
when no folder is given. `--quick` skips the sound and video decoding. Exit code 0 = all ok, 1 = failures, 2 = no
install. A full run takes about a minute. `--mod NAME` (repeatable) checks those mods instead of `mods/enabled.txt`'s;
the "mods" line lists every file each enabled mod replaces or adds and checks each mod's `rules.toml` and text patches, and its files
are loaded with the install's.

## Mods
A mod changes the game with your own files: a sharper texture, another text, another sound, other rule values, a
theme (ADR 0044, 0068). The remake still ships no game files; a mod is files you make or download.

**Installing a mod** in three steps:
1. Drop the mod's `.zip` (or its folder) into the `mods` folder beside the game's executable. Main menu → Mods →
   Open mods folder opens it (and makes it when missing). Don't unpack the zip: the game reads it as it is.
2. Main menu → Mods (the Mods label above Quick game, or the grouped menu's Extras → Mods): switch the mod on, use
   the arrows to order it (the higher mod's files win when two mods have the same file), then Apply.
3. Restart the game (Restart now on the same page). The page shows each mod's problems in red and the files another
   mod above it overrides; a broken mod is skipped and the game starts without it.

**Making a mod** in three steps:
1. Make a folder named after your mod and put each file at the same path it has in the install:
   `sharp-menu\dataraw\textures\main\eng\unselected\0_credits_bmp.pvr` replaces the install's
   `dataraw\textures\main\eng\unselected\0_credits_bmp.pvr`. Names are not case-sensitive. A mod's file also beats
   the install's translated copy under `localization\<language>\`. Files may be plain (textures as PowerVR v3
   `.pvr`, texts in the game's text format) or in the install's own packed form. Rule, text and story changes are
   files at the folder's top (below).
2. Describe it in a `mod.toml` at the folder's top (every line optional):

   ```toml
   name = "Sharp menu"
   author = "Your name"
   version = "1.0"
   description = "Sharper main menu labels."
   picture = "picture.png"   # a PNG in the mod, shown on the Mods page
   game = "0.7"              # the openIG2 version it was made for
   ```
3. Zip the folder (Windows: right click → Send to → Compressed folder; one top folder inside the zip is fine) and
   share the zip. Check it first: `ig2-check --quick` lists the files your mod replaces or adds, and loads them.

Where mods are read: the `mods` folder beside the executable and the one beside the settings files
(`%LOCALAPPDATA%\GyroscopeGames\ig2\mods`, or `IG2_OPTIONS_DIR\mods`); a mod in the first wins over one of the same
name in the second. The enabled list is `enabled.txt` in the settings' `mods` folder: one mod name a line (a folder's
name, or a zip's without `.zip`), the most important first, `#` starts a comment. The Mods page writes it; you can
edit it by hand too. `--mod sharp-menu` (repeatable) replaces the list for one run. A zip may hold stored or
deflated files only (what every zip tool writes), no passwords, no files outside the mod.

`ig2.exe` can't be replaced: the rule tables are read from it. To change rules, put a `rules.toml` at the top of
the mod folder (`mods\cheap-labs\rules.toml`); it sets single fields of the tables by name. A record is
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
When several mods change the same field, the one higher in `enabled.txt` wins. A wrong table, id or field starts the
game without the mods, and the Mods page and `ig2-check` show the message naming the mod and the field.

A `rules.toml` can also add techs and building kinds (ADR 0069). A new tech takes a free field of a research
category (the empty `-` entries of the research screen); a new building is a new kind that starts as a copy of an
original kind (`like`) and does what that kind does: a power plant gives energy, a house living space. Example: a
reactor researched after the Mezon plant, built like it, with more output:

```toml
[[techs.new]]
key = "fusion_reactor"        # your own name for it; other entries use it
category = 9                  # power plants (6 civil, 7 research, 8 military, 9 power plants)
name = "Fusion reactor"
description = ["A plant that fuses the colony's", "waste into energy."]
prerequisites = [135]         # the Mezon plant's tech (numbers as ig2-rules-dump prints them)
research_cost = 40000
research_time = 120
price = 9000
build_time = 30
ai_priority = 8               # computer empires research lower numbers first

[[buildings.new]]
key = "fusion_reactor"
like = 55                     # the Mezon plant: model, footprint, role
tech = "fusion_reactor"       # its price and name come from this tech
output = 400
workers = 600
max_hp = 3000
```

The tech appears on the research screen once the Mezon plant is researched, and the plant in the colony's build list
once the tech is; computer empires research and build it too. Other fields and the limits are in ADR 0069; `ig2-check`
shows where each new entry landed. Saves made with the mod need it to load.

A mod can also add stories: a `stories.toml` at the top of the mod folder lists `[[story]]` tables, each with `when`
triggers and `do` actions, that run after the built-in stories. A story fires once, or again and again with
`repeat = true` (`every` days apart, at most `times` times). Triggers: the day, a planet's owner, a dead empire, a
battle won or lost, a tech researched, money, relations, a spy report, a building, a timer, the mod's flags and
counters, a fleet at a planet, a message answered, combined with `all`, `any` and `not`. Actions: messages and
ticker lines, videos, planets, fleets, ground units, buildings, money, techs, relations and diplomacy (war, peace,
alliance, trade), flags, counters, timers, another story (`chain`), winning or losing. The full list is in ADR 0061;
`ig2-check` validates the file. A worked example, `mods/zorg-ultimatum/stories.toml`:

```toml
# Day 60: a warning on the ticker and a 30-day countdown.
[[story]]
name = "warning"
when = [{ day = 60 }]
do = [{ ticker = 9100 }, { start_timer = { name = "ultimatum", days = 30 } }]

# The human gave in in time (a non-aggression pact or better with empire 3): peace, a gift, the threat is over.
[[story]]
name = "peace"
when = [{ timer = "ultimatum" }, { relation = { a = "human", b = 3, at_least = 4 } }]
do = [{ ticker = 9101 }, { money = { empire = "human", amount = 5000 } }, { set_flag = "zorg-peace" }]

# Otherwise war: empire 3 declares it and lands troops on planet 12.
[[story]]
name = "war"
when = [{ timer = "ultimatum" }, { not_flag = "zorg-peace" }]
do = [{ diplomacy = { a = 3, b = "human", state = "war" } },
      { spawn_fleet = { id = 20001, owner = 3, planet = 12, design = 2, ships = 6 } },
      { ground_units = { fleet = 20001, design = 20, count = 8 } },
      { chain = "raids" }]

# While at war, every 10 days, empire 3 gets 2000 credits (at most 5 times).
[[story]]
name = "raids"
repeat = true
every = 10
times = 5
when = [{ relation = { a = "human", b = 3, at_most = 1 } }]
do = [{ money = { empire = 3, amount = 2000 } }]

# Winning a battle against them, or having tech 40, ends it: peace and a building on planet 12 when the human owns it.
[[story]]
name = "victory"
when = [{ flag = "zorg-war" }, { any = [{ battle = { result = "won" } }, { tech = { tech = 40 } }] }]
do = [{ diplomacy = { a = 3, b = "human", state = "neutral" } }, { add_building = { planet = 12, kind = 7 } }, { ticker = 9102 }]

[[story]]
name = "at-war"
when = [{ relation = { a = "human", b = 3, at_most = 1 } }]
do = [{ set_flag = "zorg-war" }]
```

with its ticker lines in `texts.en.toml`:

```toml
["dataraw/text/s_story.txt"]
9100 = "The Zorgons demand a pact within 30 days."
9101 = "The Zorgons accept your pact and send a gift."
9102 = "The Zorgons sue for peace."
```

Planets are numbered as in the `.scn` maps (from 1); designs, techs and building kinds are the slots and lines of the
game's own tables, so a mod for another map or race picks its own numbers. A story with a typo stops the game at start
with the mod, the story and the entry named.

To change or add single lines of text without replacing a whole file, put a `texts.en.toml` at the top of the mod
folder, and `texts.de.toml`, `texts.fr.toml`, `texts.es.toml`, `texts.it.toml` or `texts.hu.toml` for other
languages; a line your mod has no translation for shows its English one. Each `["<file>"]` heading names a game text
file and the lines under it set that file's entries by their key (ADR 0044 §10). An example for a made-up race:

```toml
# Race names: line 900 is new, so it is added.
["dataraw/text/s_races.txt"]
900 = "Zorgons"

# Interface text: "file section index", as written in the file.
["dataraw/text/txt03.txt"]
"3 1 1" = "Mend"

# Diplomacy offer 001/000/003: two variants, one picked at random; \n starts a new line, #1# is the empire's name.
["dataraw/diplomac/ajanlat.eng"]
"001/000/003" = ["Zorg greets #1#.", "Zorg is hungry.\nVery hungry."]

# Message 0193, part 1: new lines, same speaker and timing.
["dataraw/uzenetek/txt0193.txt"]
1 = "Zorg line one.\nZorg line two."
```

and `texts.hu.toml` beside it:

```toml
["dataraw/text/s_races.txt"]
900 = "Zorgonok"
```

A key the file already has is replaced, a new one is added; a message part must already exist (a new message is a
new file). A wrong file, key or a letter the game's font lacks stops the game with a message naming the mod, the file
and the key, and `ig2-check` says how many lines each file sets.

Story scripts can't be changed: the campaign's stories are program code, not files (ADR 0044 §14). What a mod can
replace is their data: the `.scn` maps (which stories start, their values), `dataraw\text\fleet.txt`,
`building.txt`, `designs.txt`, `values.txt`, the message scripts in `dataraw\uzenetek\` and the texts.

In multiplayer every player needs the same mod files, `rules.toml`s and text patches: the room refuses a player with other mods
("Other mods enabled"; compare your Mods page with the host's), the room page lists its mods, and a multiplayer save made with mods resumes only with them. Texture and model packs (below)
are the exception: they only change what you see, so players with and without them play together.

### UI themes
A theme restyles the remake's own screens (the main-menu Options, match history and statistics, replays, the
multiplayer lobby, the event log); the game's own screens keep the original's look (ADR 0056). Pick one with the
Interface setting "UI theme": Classic (default), Modern, or any mod folder that has a `theme.toml`, listed by the
folder's name (the theme doesn't need `enabled.txt` and doesn't affect multiplayer).

To make one, create `mods\<name>\theme.toml`. Every key is optional and starts from Modern's value; colours are
`[red, green, blue]` from 0 to 255:

```toml
# mods/amber/theme.toml
panel = [24, 14, 0]          # panel fill
panel_edge = [140, 90, 20]   # panel edge
panel_transparency = [0, 0]  # [fill, edge], 0 = opaque, 255 = invisible
tab = [30, 20, 5]            # tabs; tab_selected for the open one
tab_selected = [90, 60, 10]
button = [50, 30, 5]         # buttons: button, button_edge, button_transparency
text = [255, 220, 160]       # text; text_dim, text_off (disabled), accent (values), hover
hover = [255, 255, 255]
track_on = [200, 130, 30]    # switches and sliders: track_on, track_off, knob
font_title = 4               # the game font's size for titles (0-4)
font_text = 2                # and for everything else (0-2)
```

A key it doesn't know, a wrong value or a number out of range is an error that names the key: such a theme isn't
listed, and a stored one falls back to Classic with the error in the log (`ig2-client: theme <name>: …`). The full
list of keys and Modern's values is `engine/crates/ig2-client/src/themes/modern.toml`.

### Texture and model packs
A pack is a mod whose `hires` folder holds better versions of the game's pictures and models (ADR 0054). Unlike a
plain replacement, a pack's file can be any size: the game still places and sizes everything by the install's
original, so a picture four times as large draws in the same spot, only sharper.

- **Pictures**: put a PNG (or a `.pvr`) at `hires\` + the install's path, with `.png` for `.pvr`:
  `mods\hd-colony\hires\dataraw\textures\colony\foo_bmp.png` is drawn for the install's
  `dataraw\textures\colony\foo_bmp.pvr`. Any size and aspect works; keep transparency in the PNG's alpha channel.
  Model textures, interface pictures and the pointer all work this way; the font does not.
- **Models**: put a model file at `hires\` + the install's path: `mods\hd-colony\hires\dataraw\vector\colony\<name>.xv`
  is drawn for the install's `dataraw\vector\colony\<name>.xv` (buildings and every other model). It is scaled so
  its largest side matches the original's, so it fits the original's place whatever units it was built in. Its
  textures are found by name like the original's: put new ones in the mod's `dataraw\textures\colony\`, or sharper
  versions of the install's in `hires\`.
- Enable the pack like any mod (`enabled.txt` or `--mod`). A pack's picture wins over Texture upscaling.
- Packs, like all mods, are yours to make or download; the project ships no graphics.

## Asset viewer
`cargo run --release -p ig2-viewer` browses the install's models, textures, colony maps, animations and messages.
