# Changelog

What changed in each version. Versions are MAJOR.MINOR.N (docs/VERSIONING.md).

## 0.7 — Playable 1.0 (in progress)
Getting the game from "every part works" to "a player can install it and play it through".

- Urgent messages open the messages screen and play at once, as in the original.
- Multiplayer: the game no longer stays stuck after a space battle (a player who rejoined or resumed mid-battle, or had Options open when it ended, now leaves it too), and a battle's end no longer pauses everyone.
- Multiplayer: a full-screen video (your own defeat or victory) no longer stalls the others or gets you dropped.
- Multiplayer: opening Options or Help no longer stalls the game for everyone or gets you dropped; the game runs on underneath, as in the original.
- Splitting a fleet works again in single player and multiplayer: the new fleet stays, and ships can be moved into it in the transfer panel.
- The multiplayer join page labels its field "Address or join code", so a pasted join code is where to put it.
- The starmap shows a faint arrow from each of your planets to its rally planet, under the fleet routes.
- Story offers sent from the diplomacy screen's special menu now reach their episodes.
- While the tutorial runs, the Options game page shows "Ground battle simulate" and "Space battle simulate" off and greyed out; your own settings come back afterwards (ADR 0019).
- In multiplayer you can no longer move, rally, sell or change the settings of another player's fleets and planets, or give building orders on their colonies.
- The credits video shows from its first frame instead of staying black for ten seconds.
- Internet games through a relay server you run yourself: type its address on the multiplayer page, and joining by code then works behind any router (`docs/RENDEZVOUS.md`).
- The client finds your GOG install by itself (registry, GOG Galaxy, the default folders) and checks `ig2.exe`.
  When it finds none, a window lists the places it tried and accepts the game folder dropped onto it.
- The main menu works without a save; a first start no longer plays the campaign's start voice and in-game music
  over the menu.
- The Options screen has display settings: window, borderless or exclusive fullscreen, resolution, vsync, the 4:3
  letterbox, the live background and one-language names. They apply at once and are saved.
- Starmap story icons: the story fleet's wormhole, the virus alert and the gigabomb can be used from the radial
  menu, so the campaign episodes that need them can be completed.
- Attacking the campaign's story planet now plays out as in the original.
- In single player, a planet invaded in real time is fought out on its colony screen: the landed vehicles leave
  their dropships, the battle runs and the planet keeps or changes its owner.
- The tutorial locks input like the original (blocked and greyed keys, screens and buttons), and its chapters 1-3
  and 5 play through; chapter 4 runs up to its ground battle. `ig2-client --tutorial N` starts chapter N.
- Internet games: the host can make a room private and share a join code, and can switch on UPnP / NAT-PMP port
  mapping to see the address other players can reach.
- Long multiplayer games (2-4 humans plus AI empires, thousands of turns, battles, saves and rejoins) stay in step;
  a desync after garrisons entered a battlefield and a crash from a malformed diplomacy message are fixed.
- `ig2-client --playthrough` runs whole games headless to the end, with saves and loads on the way. Loaded saves
  now keep their real-time battles and the AI's design names.
- Single-player space battles: ships now obey attack and move orders however you start the battle (top-bar pause,
  Space or P, the speed keys), and re-entering the battle screen no longer brings the formation choice back.
- Colony screen: clicking a building's roof, tower or upper part selects it, as in the original; the war cursor and
  the right-click attack use the same pick.
- `ig2-client --playthrough mp` plays a whole two-machine multiplayer game to its winner on one computer, comparing both
  machines throughout and saving, resuming and rejoining on the way; no desync is found.
- Campaign 1 no longer crashes when the planet-toll raider sets out.
- `ig2-check` loads every file of the install and lists what fails to parse, so a broken install is found before
  playing.
- A Windows release zip (the client, the checker and a readme; no game data). `ig2-client --version` and the window
  title show the version.
- Unzipped into the game's folder, the client always uses that install, even when another one is installed.
- The project is published as openIG2 under the AGPL-3.0, with CI, release builds and a website.
- The website is redesigned after the game's own look: the screens list, a screenshot switcher, install and
  build steps, and the road to 1.0.
- Diplomacy in multiplayer can take back an unanswered message ("cancel last message"); the tutorial's shorter wait
  between diplomacy messages now applies.
- A fleet split off on the starmap, the ground-war tick and the victory check now follow the screen you are on, as in
  the original.
- Not done yet: the campaigns' late episodes are verified end to end only for campaign 1 and campaign 2's second
  storyline (the play-through driver answers the stories, researches, sends spies and fleets); campaign 3 is open.

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
