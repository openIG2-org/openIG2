openIG2 {version}
=================

An open-source engine for Imperium Galactica II (1999). It plays the game from YOUR OWN copy
of the GOG release: this archive contains no game data, art, sound, video or text
from the original. Without the game installed it shows a message and does nothing.

Requirements
- Windows 10 or 11, 64-bit, a GPU with Direct3D 12 or Vulkan.
- Imperium Galactica II from GOG (the 2017 release, ig2.exe build checked at start).

Install and start
1. Unzip this folder into the game's folder (the one with ig2.exe), or anywhere.
2. Run ig2-client.exe.
   Inside the game's folder it uses that install. Elsewhere it finds the game through GOG's registry entries and the usual GOG folders. If it
   can't, its window says where it looked: drop the game's folder (or its ig2.exe)
   onto that window. The folder is remembered.

Settings, saves
- Options screen (Esc or O in the game), game page, "Remake" rows: window, borderless
  or fullscreen, resolution, vertical sync, 4:3 letterbox, live background, names in
  the game's language. Saved in %LOCALAPPDATA%\GyroscopeGames\ig2\ig2-rework-options.dat,
  beside the original's options.dat (which openIG2 never rewrites).
- Save slots are in the same folder (save<n>.ig2s; the original's save<n>.sav load too).
- ig2-check.exe (if included) checks every file of your install and lists what fails.

Uninstall: delete this folder and, if you like, ig2-rework-options.dat and save*.ig2s.

This is a beta build. Not affiliated with the original's makers or with GOG.
Source, license (AGPL-3.0-only) and bug reports: https://github.com/openIG2-org/openIG2
