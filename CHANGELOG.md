# Changelog

What changed in each version. Versions are MAJOR.MINOR.N (docs/VERSIONING.md).

## 0.7 — Playable 1.0 (in progress)
Getting the game from "every part works" to "a player can install it and play it through".

### Campaign and gameplay
- All three campaigns play through to their endings, campaign 2's own ending included: story spy missions, story offers, the story icons (wormhole, virus alert, gigabomb) and the attack on the story planet work as in the original.
- Single player: a planet invaded in real time is fought out on its colony screen, with dropships setting the tanks down and the original's sounds, explosions, tracers, smoke, debris and collapsing buildings; the rest of the galaxy pauses meanwhile, and losing every tank no longer leaves the fleet stuck.
- Space battles: ships obey orders however the battle is started, the retreat button works (losing computer fleets retreat too), and the "simulate" and "overrun" options change how battles are fought.
- The tutorial locks input as the original does and its chapters play through (chapter 4 up to its ground battle).
- Custom game: set the galaxy's planet count and seed and, per empire, the race, name, money, technology, start planets, computer level, aggressiveness and start relation; a map of the galaxy lets you pick your start star.
- "Computer empires: Enhanced" (for new games and multiplayer rooms): the computer empires expand, research and build up better, gather fleets before striking, invade only with enough troops, defend their colonies, make peace and pacts, and no longer gang up on empires they have never met; Classic plays as the original.
- Auto research: three switches on the research screen (ships, buildings, tanks) let an idle research slot pick its next project.
- Quick save (F12) and quick load (Shift+F12), autosaves every 10, 30 or 90 game days, fleet groups (Ctrl+1…9, Alt+1…9), and an optional starmap camera with wheel zoom and W/A/S/D.
- Every game records a replay you can watch from the main menu; after a win or a loss a statistics screen shows how the game went, History lists every finished game, and a won scenario can take your name into its high-score table.
- Messages: an event log with filters and search; urgent messages open at once; errors and the campaign's info lines show on the message line as in the original.
- "Campaign story: Enhanced" starts the besieging fleets of one story episode at their planet instead of the far corner of the map.
- New games name the starting ship designs and technologies in the game's language instead of Hungarian.

### Multiplayer
- Internet games work out of the box: the project's relay server is the default, rooms can be private with a join code or on a public list, and hosting asks your router to open the game port; you can also run your own relay server (a Docker image is published).
- The game runs at the single-player pace, your own orders show at once, and each player's input delay follows their own connection (none on a LAN); fleets glide instead of jumping.
- A player who drops, leaves or is kicked is replaced by the computer and gets the empire back on rejoining; a saved game can be resumed without every player; defeated players can stay to watch, and others can join a running game to watch.
- The room: a card per empire, computer difficulty per seat, the Custom game settings, "Start anyway", the original's game modes (`story 1001` etc. in the chat) and optional space-battle formations.
- Mods in the room: a refused player is told which mods are missing or differ, and "Download and join" installs them and joins again.
- Games stay in step and running: battle ends, videos, Options and Help no longer stall anyone, you can't give orders to other players' fleets and planets, and a misbehaving player is dropped without stalling the game.
- Discord Rich Presence shows what you are doing, and a host's open room has a Join button for friends.
- The multiplayer pages fit at any UI scale: the panels no longer run over the Back, Host game and room footer buttons, and a column taller than the window scrolls.

### Screens and controls
- Modern interface (Options, Interface; on by default; one switch for every screen): the starmap, colony, production, research, ship design, information, messages, diplomacy, spy and trade screens, the fleet and transfer panels and the space battle are redrawn with the same functions and keys; Classic keeps the original's screens.
- Modern menus: the main menu, new game with race portraits, the pause menu with a save and load page, help with search, the end-of-game screen, and pages for multiplayer, the tutorial, replays and scenarios.
- Settings in seven tabs, with key rebinding, a separate voice volume, extra game speeds, a language choice, display modes (window, borderless, fullscreen, resolution, vsync, 4:3 letterbox) and a UI scale from 50 to 200 %.
- The modern screens open and close with short fades and slides, use one line-icon set, show tooltips at once and have animated pointers.
- Many fixes to the modern screens reported by players: blueprint selection on the design screen, the research list's height, the colony's building chooser, tabs and zoom, the main menu's Back and Load pages, and the production tooltips.
- Accessibility: subtitles for spoken messages and colour-blind empire colours.
- Closer to the original: the starmap's wormhole and rally arrows, radar zones, info lines and timings, the spy screen's lists, the space battle's radar and retreat box, and the colony's building pick.
- Optional fixes for small quirks of the original (clean name entry, models inside their boxes, centred Exit labels, Hungarian articles, title music with the intro).
- Hungarian: the remake's own menus and notices are translated and read more naturally.

### Graphics, options and mods
- Graphics options: anti-aliasing, per-pixel lighting, soft colony shadows, glow, smooth glows, sharp text and UI on 1440p and 4K, texture upscaling (2x or 4x) and richer battle effects; all off keeps the original's look.
- Mods: drop a zip into the `mods` folder and switch it on in the Mods page, or install, update and remove mods with their dependencies from the Browse tab; a broken mod no longer stops the game.
- Mods can change rule values, texts, textures and sounds, and add techs, buildings, ship hulls, weapons, parts, tanks, races, planet types and their own stories; computer empires, saves and multiplayer games use them.
- Visual mods (`visuals.toml`) can switch on smoother edges, per-pixel lighting on every 3D model, soft shadows, scene glow and planet halos; they change only the look, so they work in multiplayer with players who don't have them.
- For mod makers: a "Your first mod" guide, mod metadata (version, dependencies, conflicts), and `openIG2-check` to check and package a mod.

### Install, updates and platforms
- The game finds GOG and Steam installs by itself, checks them, and accepts a game folder dropped onto its window when it finds none.
- Builds for Windows, 64-bit Linux and Apple Silicon Macs; the game is now `openIG2.exe` (`./openIG2` elsewhere) with its own icons.
- Browser version: single player with saves, multiplayer through a relay server, and videos; a server can host it with the game files, which download only for the chosen language, and the website has a "Play online" button.
- The game checks for a newer openIG2 at start; on Windows "Update and restart" installs it, keeping settings, saves and mods (Options, Game turns the check off).
- The game starts about three times faster, shows no white window or console window while loading, and keeps a log with crash details for bug reports.

### Fixes
- Entering planets and space battles over and over no longer crashes with "Not enough memory left".
- The galaxy simulation and the modern interface do less work every frame, and a ground war no longer stutters when a building falls.
- Fixed: a crash in campaign 1, a black screen after minimising, the spy screen's buttons, a desync after garrisons entered a battle, and a crash from a malformed diplomacy message.
- Fleet splitting, single clicks on the production buttons, the messages screen's list and the credits video work again.

## 0.6 — Fixes against the original
Every known difference from the original was sorted into fix, enhanced option or keep, and most fixes were made.

- Global input as in the original: function keys, key repeat, the top bar's pause, speed and message line on every
  screen, and the right button acting like the left on buttons.
- The game's own pointer and tooltips on every screen.
- Starmap, colony, ground war, space battle, design, options, trade, research, diplomacy, info, messages and spy
  screens take keys and clicks the way the original does.
- Looks: material transparency, levels of detail, environment mapping, panel and planet lighting, shield rings,
  cloud shells; colonies get construction scaffolding, snow and rain, and building smoke.
- The tutorial is fully wired: all five chapters, their gates and effects match the original.
- The campaign's stories run inside the game's own tick.
- Fixes from play testing: starmap right-click, Shift waypoint chains, hidden unseen planets, the speed button
  after an automatic slow-down.
- Multiplayer: chat, host commands to kick players, host-controlled pause and speed, saves resumed from the room,
  a new host taking over when the host leaves, renames shared with every player.
- Faster simulation ticks and screen drawing.
- Deliberate differences are documented: a live game behind overlay screens, kept behaviours, one language.

## 0.5 — Multiplayer playable
Multiplayer games can be set up and played from the client.

- A connection page and a game room with the original's room rules; the generated galaxy is built on every
  machine.
- Real-time battles in multiplayer stay in step, with session rules for drops and rejoin.
- Sessions survive problems: drop grace, rejoin and admission after the start, desync resync, pause, keep-alive,
  host migration.
- Multiplayer saves.
- For contributors: lockstep commands with a state hash, QUIC transport, LAN discovery and optional port mapping.

## 0.4 — Input and feel
The screens are checked against the original's input and at modern window sizes.

- Keys and hot-spots of every screen audited against the original's keyboard handlers, with fixes.
- Every screen checked at 16:9, 21:9, 4K and 5:4: 2D stays in the centred 4:3 frame, 3D scenes and backgrounds
  span the full width, narrower windows are letterboxed.
- Wireframe lines are one output pixel wide at every resolution.

## 0.3 — Every screen
The remaining screens arrive and every screen is compared with captures of the original.

- The tutorial overlay: the tutorial runs beside the game with its text window.
- The help screen with its topic list, pictures and scrolling; the credits with the remaster's text overlay.
- The high-score table.
- AI and story merchant ships get their proper design names.
- A comparison of every screen against captures of the original.

## 0.2 — Playable start to finish
A game can be started from the main menu and played to its end.

- New games: campaign, scenario, multiplayer galaxy, skirmish and tutorial starts, identical to the original's.
- Main menu: title, ring menus, race and difficulty, scenario and skirmish setup, name entry, exit animation.
- Options: settings, save and load slots, quit.
- Space battles on screen: camera, orders, formations, fighters, effects, sounds.
- The ground war on the colony screen: unit selection, orders, landing, camera, voices.
- Ship design with hulls, pods and tech models; the trade screen with purchases.
- The starmap's fleet and vehicle panels; the colony's planet info mode; answers to messages.
- Audio: music selection and crossfade, every screen's sounds, speech with ducking, alien voices.
- Video: the remaster's MP4 videos (title, campaign intros, story clips, endings, message heads).
- Game end: victory and defeat, the end videos by race, then the main menu.

## 0.1 — Foundations and the campaign
The game's data and rules are understood and the whole simulation runs.

- Readers for the game's files: containers, textures, models, colony maps, scenarios, texts, dialogue, story data
  and original save games. Rule values are read from your own `ig2.exe`; nothing from the game is shipped.
- The whole simulation, checked tick for tick against recordings of the original: colonies, production,
  research, fleets and radar, trade, info and messages.
- Diplomacy, espionage and the AI empires.
- Space and ground battles, auto-resolved and real-time, with the ground AI.
- Every campaign script and the story engine: storylines, random stories, fables, the multiplayer game modes and
  the tutorial's script interpreter.
- The first client screens: starmap, colony, research, diplomacy, spy, info and messages, drawn and lit like the
  original.
- The remaster's widescreen rule and a shared screen kit.
- For contributors: a capture-comparison tool and a sound tracer for checking the client against the original.
