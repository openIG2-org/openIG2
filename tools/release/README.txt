openIG2 {version}
=================

An open-source engine for Imperium Galactica II (1999). It plays the game from YOUR OWN copy
of the GOG release: this archive contains no game data, art, sound, video or text
from the original. Without the game installed it shows a message and does nothing.

Requirements
- Windows 10 or 11, 64-bit, a GPU with Direct3D 12 or Vulkan; or Linux x86-64 (glibc 2.35 or
  newer, ALSA or PipeWire/PulseAudio, X11 or Wayland), a GPU with Vulkan; or macOS 11 or newer
  (Apple Silicon: the macos-arm64 archive), Metal.
- Imperium Galactica II from GOG (the 2017 release, ig2.exe build checked at start).

Install and start
1. Unzip this folder into the game's folder (the one with ig2.exe), or anywhere.
2. Run openIG2.exe (Windows) or ./openIG2 (Linux, macOS).
   macOS: the binaries are not signed. Once, in a terminal in this folder:
   xattr -dr com.apple.quarantine .   (or right-click openIG2 in Finder, Open, Open).
   The game is Windows-only: copy your Windows install's folder to the Mac (or install it
   through CrossOver, Whisky or Heroic) and, if it isn't found, set IG2_INSTALL_DIR to it.
   Inside the game's folder it uses that install. Elsewhere it finds the game through GOG's registry entries and the usual GOG folders
   (Linux, macOS: Heroic's GOG library, ~/GOG Games, ~/Games and the Wine/Lutris prefixes' GOG folders;
   macOS also ~/Applications, /Applications and the CrossOver and Whisky bottles). If it
   can't, its window says where it looked: drop the game's folder (or its ig2.exe)
   onto that window. The folder is remembered.

Settings, saves
- Options screen (Esc or O in the game), game page, "Remake" rows: window, borderless
  or fullscreen, resolution, vertical sync, 4:3 letterbox, live background, names in
  the game's language. Saved in %LOCALAPPDATA%\GyroscopeGames\ig2\ig2-rework-options.dat
  (Linux: ~/.local/share/GyroscopeGames/ig2/, macOS: ~/Library/Application Support/GyroscopeGames/ig2/),
  beside the original's options.dat (which openIG2 never rewrites).
- Save slots are in the same folder (save<n>.ig2s; the original's save<n>.sav load too).
- ig2-client.log in the same folder (Windows): the game's messages and errors from its last start (ig2-client.log.old = the start before; a crash
  adds the version and call path); attach it to a bug report.
- openIG2-check checks every file of your install and lists what fails. For mod makers, --dump-rules FILE and
  --dump-texts FILE write your install's rule values and text keys (for your own use only: never share them).

Sharper textures
- openIG2 --make-pack (in a terminal in this folder) makes a texture pack from your install:
  every texture upscaled 4x into the mods folder's texture-pack (about 1 GB, a few minutes;
  --factor 2 for a smaller one; a stopped run continues where it left off). Then enable
  "texture-pack" on the Mods page. It is made from the game's files: don't share it.
  The scene textures go through Real-ESRGAN (models/, run on your graphics card),
  the interface art through xBRZ; --filters-only leaves the model out.

Third-party files
- models/realesr-general-x4v3.pth: Real-ESRGAN's model, BSD-3-Clause, Copyright (c) 2021
  Xintao Wang (models/LICENSE-Real-ESRGAN.txt).

Uninstall: delete this folder and, if you like, ig2-rework-options.dat and save*.ig2s.

This is a beta build. Not affiliated with the original's makers or with GOG.
Source, license (AGPL-3.0-only) and bug reports: https://github.com/openIG2-org/openIG2
