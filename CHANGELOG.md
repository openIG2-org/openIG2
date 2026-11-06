# Changelog

What changed in each version. Versions are MAJOR.MINOR.N (docs/VERSIONING.md).

## 0.7 — Playable 1.0 (in progress)
Getting the game from "every part works" to "a player can install it and play it through".

- Modern colony screen: a more compact building chooser (narrower, a smaller picture, tighter tabs and list rows), so more of the colony stays in view (issue #86).
- Modern colony screen: the building groups' tabs are as wide as their names, so every name (KERESKEDELMI, COMMERCIAL) shows whole and all in the same size (issue #85).
- Colony screen: the mouse wheel zooms within the zoom keys' range and glides to a stop at the nearest and farthest view instead of jumping back (issue #88).
- Modern main menu: its pages (sub-menus, new game, settings, load, statistics, replays, mods, multiplayer) slide and fade in and out one after the other, as the modern in-game screens do (issue #91).
- Modern main menu: back from Settings or Load game the menu carries on where it was (its scene and music keep running) instead of starting over (issue #93).
- Modern main menu: the mouse pointer is drawn over the menu panels and pages instead of behind them (issue #92).
- Modern starmap: the selection card's tooltip lists its mouse actions one per row, each with a small left, right or both-buttons mouse, instead of one dense line; the shared look has this tooltip for any control with several mouse actions (issue #80).
- Modern starmap: renaming a planet or fleet types into the selection card's own text field, without the classic rename text drawn behind and beside the card (issue #79).
- Modern starmap: the selection card no longer draws a black disc behind a turning ship, fleet or trader; the model turns on the card itself (planets keep their globe) (issue #84).
- Modern starmap, fleet and space battle screens on the shared look: the tooltips with their key, the camera and radial menu icons, the settings lists and the planet list from the shared parts; the long info's settings row, the lists, the fleet headers, the battle's radar and info box on their design boards (issue #75).
- Modern multiplayer (connection page and room), statistics and save/load screens use the shared look for tables, tags, text fields and drop-downs (the room's race and seat choices are the shared drop-downs), with upright table numbers and titles, panels and rows aligned on their design boards; the relay panel has its "Address" label (issue #75).
- Modern colony, production, research and ship design screens line up with their design boards: the colony's planet info settings use the shared drop-downs and its plates are equal, the production lamps and research switches are the shared small plates, research's category groups and tables sit on the boards' columns, and the ship name field is the shared text field (issue #75).
- Modern starmap and colony screens share one radar look (cast-metal plate, black well, keys centred under it); the starmap's eight camera keys sit in one even row (issue #67).
- Modern starmap: Production is its own round button with its label under it, right of the selection card, instead of a small key squeezed into the card's button row (issue #66).
- Modern starmap's selection card (bottom left) matches its design board: solid navy glass, board-sized plates, the globe and its two ratios centred on the text, richness shown with a hammer and habitability with people (issue #64).
- The "UI theme" setting is gone: every modern screen always has the design boards' look (navy glass, white titles, blue plates) instead of the near-black, grey-titled look the default theme gave them; the classic screens keep the game's look, and `--theme` and mod `theme.toml` files are no longer used.
- Modern top bar matches its design board: one even row of keys and LCDs, wider money and date readouts, line icons on the screen keys, and a yellow count of unread messages on the Messages key.
- Settings page in seven tabs (Display, Graphics, Interface, Accessibility, Controls, Sound, Game); the Interface tab keeps the modern and the classic interface's own settings apart, and a setting that depends on another sits under it, greyed with a note while it can't apply (ADR 0083).
- One Interface setting (Options, Interface): Modern or Classic for every screen at once, switching at once during a game and on the main menu; old per-screen choices carry over (ADR 0082).
- Play in the browser from a server: `deploy/web` runs the browser version with Docker Compose; a server can offer its own copy of the game files, which visitors download into their browser once they agree (only the files the game reads: 645 MB of the GOG install's 690 MB), and the main menu loads (ADR 0081).
- Discord Rich Presence: your Discord profile shows the menu page, the multiplayer room, or the game's mode, race, difficulty, screen, date and empire numbers; a host's open room has a Join button that starts a friend's game and joins it (Options, Game: off, status, or with join; ADR 0080).
- Modern colony screen: the build menu's group tabs, Build and the round buttons look like the other modern screens', Repair sits beside Demolish (greyed out while nothing needs repair), and prices on every modern screen read `$10000` as in the original.
- Modern production view: a second click on the same slot (or its "+ Design") closes the design list, a click on another slot switches the list to it, and a click on the bare starmap closes it too.
- Fixes from a code review: the modern spy screen's buttons and lists work again; loading a game or going to the main menu from Options over the design screen no longer keeps the old design screen running; the battle and spy screens recover when the window's surface is lost (no black screen after minimising); hover tooltips with two or three lines fit their panel and empty ones are no longer shown; a damaged save is refused with a message instead of crashing; damaged or hostile video, model, texture and text files are rejected instead of crashing.
- Multiplayer is sturdier against a misbehaving player: one undecodable message drops only that player instead of stalling the game, the host no longer stores unbounded future turns or hash reports, member ids no longer collide after many rejoins, and the signalling and port-mapping code can't be held up by slow or stray traffic.
- Modern UI: tooltips appear at once and while the mouse moves; the top bar's research readout is itself the progress bar, with the project's name on the full width and the per cent at the right; the quick-research drop-down no longer grows wider while a project runs.
- Modern starmap HUD: the selected planet shows its own turning globe in the card and the long info's header, the date reads as in the original's top bar, and the screen keys stay on a narrow screen.
- Production view, research screen and ship design screen compared with their design boards and fixed: tech stats as table rows, labels that fit their columns in every language, every fitting tech listed for a pod, evenly spaced count buttons, and the hand pointer over every button, row and tile (ADR 0071, ADR 0075).
- Modern messages screen (Options, Interface; on by default): the inbox, the talking head in a metal frame, the message's card with its answers as buttons and the text typed out in wide, readable lines with a blinking caret, a "Full text" button and Exit; the inbox lists every message from the top, the newest at the bottom, and scrolls when it is long; the same answers and keys as the original (ADR 0076).
- Information screen in the new design: the eight empires as rings with their leaders around the money triangle, the selected empire's panel with the spending limit, the planets with turning globes, the fleets and the empires met in tabs with sortable columns, the detail mode with the planet's globe and settings, the charts on the game's own buttons with a light running along each line, and an Exit button (ADR 0072).
- Diplomacy screen in the new design (Options, Interface; on by default): the contacts as cards with the leader's picture and relation (a green blink while a message waits), the selected empire's card, the proposal menus as click-open panels, the dialogue with each side's lines under its empire's name and the answers as buttons, with the 3D leader ring kept in the middle; switch it off for the original's screen (ADR 0076).
- Diplomacy, spy and trade screens in the new design open and close with their own short fades and slides (the leader, agents and hull appear a little later, the screen closes at once); the original's ring, telescope and frame animations play only with the Classic interface (ADR 0084).
- Info screen in the new design: each leader sits round inside its ring, the rings a little larger.
- Pointers in the new design move as the original's do: the darts rock, the building and the target ring turn, a light runs along the green cross, the sight's ring glows.
- Research, design, info and messages screens in the new design open and close with the same short fades and slides.
- Spy screen in the new design (Options, Interface; on by default): the agents keep turning on their pedestals, an agent card with the four skills and + buttons, and one panel for planning, mission and report with drop-down lists and buttons; switch it off for the original's screen (ADR 0076).
- Trade screen in the new design (Options, Interface; on by default): all ships' and vehicles' offers in a scrolling list, the chosen hull turning beside its card, an order bar with quantity, destination and days, the deliveries under way; switch it off for the original's screen (ADR 0076).
- Information screen, planet detail: the settings (tax, auto-build, priority) are drop-down fields, and your own planets have a Colony button that opens the colony screen.
- Modern main menu (Options, Interface; on by default): the main menu as a grouped list over the game's own scene (Play, Extras, Settings, Quit), Continue shows the newest save, sub-menus have Back; switch it off for the original's rings (ADR 0078).
- New game in the new design (with the modern main menu): pick Solarian, Shinari or Kra'hen from round portraits, read the race's panel, choose Easy, Normal or Hard and Start (ADR 0078).
- Help in the new design (with the modern in-game menu): topics with a search field, sub-topics that scroll to their section, pictures, and the wheel scrolls the text.
- End of game in the new design: the outcome, a table of the empires with scores, the graph with tabs, and the match history as a scrolling table.
- Menus compared with the design boards: the main menu's spacing, a modern name prompt for the multiplayer name, "Resume" and the last save in the pause menu, mute buttons beside the volume sliders, settings cards in the boards' order and sizes.
- Multiplayer pages in the new design: the game list as a table with status tags, the join and relay panels beside it; the room with large seats (race heads, drop-downs, ready tags), the galaxy settings in a row of steppers, the join code in an LCD window and the chat at the right (ADR 0078).
- Options and the in-game menu in the new design: Options has five tabs (Display, Accessibility, Controls, Sound, Game) with the settings in cards side by side and the hovered setting explained at the bottom; Esc in a game opens a pause menu (Continue, Save, Load, Options, Help, Quit, Exit) and saving and loading have their own page with the slots as a table (name, game date, how long ago) (ADR 0078).
- New look for the modern screens (Options, multiplayer, starmap HUD, colony, production, research, information): a deep-space palette, new fonts (Oxanium and Share Tech Mono), plates, tabs, switches and sliders that react to the pointer with short animations, and text that is shortened with "…" (full text in a tooltip) or shrunk instead of running out of its box; new Modern cursors option (Options, Interface; on by default): an animated pointer set with the modern UI themes, Classic keeps the original's (ADR 0073).
- Colony screen and vehicle panel closer to their design boards: the vehicle panel on the starmap in the new look with its turning models, the radar showing the ground, readable war banner and button labels, the war sides' unit kinds, larger pictures
- Modern info screen (Options, Interface; on by default): the information screen is redrawn as the starmap HUD's console plates: pick an empire, scroll its planets and fleets as one list you can sort by order, name, size or alerts, open a row's detail with Back, set the planet's taxes and other settings from drop-down menus, drag the money triangle, set the spending limit on its bar, and pick any of the ten charts or the star log; same functions and keys, switch it off for the original's screen (ADR 0072).
- Modern production view and research screen (Options, Interface, "Modern research screen"; on by default): the production view under the starmap HUD (planet plate, ships and vehicles with capacity, four slots, count buttons that repeat while held) and the research screen (categories, techs with status, prerequisites, labs, days and cost, the running project, urgent and the auto-research switches) are redrawn as console plates; same functions and keys; switch it off for the originals (ADR 0071).
- Production and research on the new design look: the production view has two line panels of four slots with the queued design's hull turning in a picture well, count buttons that repeat while held, and the design list with unit prices; the research screen has the tech list, a detail panel with the two funding plates, category tiles and the original's turning wire models (ADR 0071).
- Modern design screen (Options, Interface, "Modern design screen"; on by default): the ship and vehicle designer on the new design look (the design list, the data, the parts with their counts, the name, the four buttons and their menus around the turning hull) with the same functions and keys; switch it off for the original's screen (ADR 0075).
- Ship design (modern screen): Redesign edits the chosen blueprint and saves over it, only New makes a new blueprint, and two blueprints can't share a name.
- Multiplayer: Local network opens the LAN game list and Internet the public room list at once; each page shows which one it is, with only its own list and the saved games as tabs.
- Tutorial (modern menu): each part shows a short line on what it teaches.
- Replays (modern menu): the list of recorded games is a modern page with the selected replay's card and a Watch button.
- Modern colony screen (Options, Interface; on by default): the colony screen's panels are redrawn as the starmap HUD's console plates: the top bar, a build menu (current kind with price, energy and workers, Build, four group tabs, each building with its count and price), a short info card with repair, demolish, on/off and self-destruct icons, the ground war's strike buttons, mines and retreat, and a radar you can press or drag; same functions and keys, the planet info mode keeps the original's panels; switch it off for the original's screen (ADR 0070).
- The modern colony screen is redrawn on the new design kit: a build menu with the turning building in a picture well, cost icons and a Build plate, surplus plates, a short info card with the unit or building model, round repair, demolish and self-destruct buttons, a ground war sides panel with unit bars and the retreat between them, a metal-framed radar with zoom and starmap keys, and a countdown on an LCD; the planet info mode has its own modern panel with the needed-resources bars and the tax, auto-build and priority drop-downs (ADR 0074).
- Fleet, transfer and space battle panels compared with their design boards: the fleet header's owner and status (with the speed) centred, a larger preview, the transfer panel with one exchange mark between the holders, the planet settings menus opening on their own field, the planet list under its plate, the original's tooltips in the new look, the battle's ship info sliding out from under the weapon rail, and the battlefield beyond the 4:3 picture answering selection and orders (ADR 0077).
- Mods can add new races and planet types: each is a copy of an original one with its own name and values (a race's production, research, diplomacy, AI shares and liking for each planet type; a planet type's growth by race and how many of a new game's planets it takes); the multiplayer room offers the new races, computer empires play them, and saves and multiplayer games carry them (ADR 0069).
- Mods can add new ship hulls, weapons, ship parts and tanks: each is a copy of an original one (its model, slots and role) with its own research entry, cost and strength; the design screen offers them once researched, ships and tanks built from them fight with their own values, computer empires fit them too, and saves and multiplayer games carry them (ADR 0069).
- Mods can add new techs and buildings: a `rules.toml` entry puts a tech in a free place of the research screen and adds a building that works like an original one (a power plant, a house, a factory), with its own cost, output, model and name; computer empires research and build them too, and `ig2-check` shows where each landed (ADR 0069). Tech and building changes from mods now also apply to new games, not only to loaded saves.
- Mods are easy now: drop a `.zip` (or a folder) into a `mods` folder next to the game and switch it on in the new Mods page (main menu), with its picture, author and description, up/down priority arrows, notices about problems and overridden files, and a restart button; a broken mod no longer stops the game from starting, and the multiplayer room lists the mods in use.
- Modern in-game menu (Options, Interface; on by default): Esc or O in a game, and the main menu's Load game, open the same designed page as the main menu's Settings, with a first tab "Save & load": the 24 slots with name and game date and a preview of the selected one, Load (double click too), Save with a name field, Quit to main menu and Exit game (both ask first), under the original's rules for what may be saved or loaded; switch it off for the original's screen (ADR 0067).
- Esc in the chat line, a design's name and a save's name no longer acts while the screen is changing.
- The planet in the starmap's planet details no longer trembles while the game is paused: the camera now stays on the planet instead of swinging between its last two positions.
- Hungarian: the remake's own texts read more naturally ("Keret nélküli" and "Teljes képernyő" window modes, "Bővített" instead of "Fejlett", "házigazda" for the multiplayer host, "képernyő" for screens, and more).
- UI scale (Options, Interface; 50–200 %, 100 % by default): the redesigned pages and the modern starmap HUD are now a sensible size at 1080p and above (the old size is 200 %) and can be made smaller or larger; the change shows at once while you move the slider, Back takes it back.
- The multiplayer pages are redesigned: the connection page shows LAN, internet and saved games as tabs with clear rows and badges, says what to do when a list is empty, and has its own cards for joining by address or join code and for the relay server; the room shows each player with their race, colour and ready state, the galaxy settings with arrows, the host's connection switches and a Copy join code button, and the chat. Tab, the arrow keys and Enter work on the connection page.
- Internet games work out of the box: the project's own relay server (`ig2relay.justdawee.hu:30000`) is now the default, so hosting, joining by code behind any router and "Browse internet games" need no address typed in; clear the relay field to turn it off, or type your own server.
- Modern starmap HUD (Options, Interface; on by default): the starmap gets a compact ship's console in your empire colour: small dark plates for the pause and speeds, date, money and messages, the research under way, the selection with icon buttons, and the minimap with a strip of camera buttons; the screens are a row of icons. Same functions and keys as before, crisp at any window size; switch it off for the original's HUD (ADR 0065).
- Modern starmap HUD: the long-press icon menu, the tax/auto-build/priority/sale/planet menus and a planet's details panel now share the HUD's console look, in the same places and with the same clicks (ADR 0065).
- The modern starmap's selection card shows the selected fleet's turning model (own, enemy and trader fleets), and a wormhole's or meteor's picture, as the classic info box does.
- Modern starmap HUD redrawn on the new design kit: a full-width metal top bar with LCD money and date and a message ticker, a deep-glass selection card, a cast-metal minimap plate with the camera keys, the planet's long info as a large panel at the screen's right with drop-down menus, and the radial menu on metal keys; hover fades and hand/target pointers, text always fitted (ADR 0079).
- Under the modern starmap HUD the fleet info panel and the transfer panel are redrawn on the design kit: unit cards with the turning models in picture wells, the selected design's model, data and pod list, a capacity bar, count displays with round transfer buttons, and the press-and-hold sale menu as a glass list; the planet settings menus (tax, autobuild, priority) open over their field with the current value marked (ADR 0077).
- Under the modern starmap HUD the space battle gets a modern HUD too: the metal top bar, a weapon rail at the right edge with the turning weapon icons, names and reload bars (the formations during the formation phase), a ship info key, the selection card at the bottom left (selected units, guns, or the battle overview with the fighters' stance slider), the ship info as a panel with the unit's model, the radar on a navigation plate with the camera keys, and the retreat question as a popup; the 3D battle, the keys and both mouse buttons work as before (ADR 0077).
- A moving fleet's blue radar zone now glides with the fleet on the starmap instead of following it in small steps, most visibly in multiplayer (ADR 0063).
- Responsive multiplayer orders: an order you give shows at once on your own screen, as in single player, instead of a few ticks later, and each player's input delay now follows their own connection's round trip, so a LAN game plays with none (ADR 0063).
- Smooth glows (Options, Graphics; on by default, off = the original's look): the starmap's star glows, clouds and stars, the main menu's galaxy and the battle's shot flares are drawn without visible pixels at 1080p and above. Sharp UI no longer turns these glows into blocks.
- No more black console window beside the game on Windows: the game's messages and errors now go to `ig2-client.log` in the settings folder (or to the terminal it was started from).
- The main menu's Options screen is redesigned: a sidebar of tabs, settings grouped in cards with a description under each, switches, drop-downs and sliders, text that stays sharp at any window size, in the remaster's Jura font.
- Relay server in Docker: `deploy/rendezvous/compose.yaml` runs the rendezvous and relay service on a Linux server, from the published image `ghcr.io/openig2-org/openig2-rendezvous` or built from source (`docs/RENDEZVOUS.md`).
- Ground war on the colony screen: a destroyed building now sinks through its collapse animation instead of only fading, and the road ring and pad around it stay until the war ends, as in the original. The colony camera keeps its tank view (C pressed three times) when you leave and return, holding the right button no longer selects a building with a left click, and the building under a still pointer is picked again when the camera moves.
- Story mods can do much more: stories can wait for a battle won or lost, a tech, money, relations, a spy report, a building or a timer, combine conditions with all, any and not, repeat every few days, and give or take techs, change relations, declare war or make peace, alliances and trade, add buildings and ground troops, start timers and set off other stories; the user guide has a worked example mod. Space battles now tell the campaign's stories who won, as in the original (ADR 0061).
- Mods can now add their own stories: a `stories.toml` in a mod folder lists stories that fire once on a day, a planet owner, a fleet arriving or a reply, and show messages, give planets, spawn fleets, change money, or win or lose the game (ADR 0061).
- Multiplayer start story: the story chosen for a multiplayer game now begins the moment the game starts, on every machine; an `init/config` file in the game folder (as the original reads) can set the start story (`mpstory`), the storyline and the story map; and "cancel last message" in multiplayer diplomacy ignores a click until the previous one is released, as in the original.
- Message line and starmap details as in the original: a known wormhole pair shows its two arrows on the starmap; an
  error such as "not enough money" now stands in the top bar's message line for three seconds, blinking, instead of
  scrolling by; "new message" no longer pulses on the messages and options screens; the list arrows of the messages
  screen work while a conversation runs and wait while an automatic message is coming; Esc no longer cancels a rename
  while the screen is changing.
- Space battles look closer to the original: the radar shows soft glowing dots, the zoom into a battle keeps the selection ring and the planet picture in the info box as they fade out, and at the end of a battle the pointer over an enemy shows the brackets instead of the attack sight.
- Menu navigation, Enhanced: the rotating rings are gone; the main menu shows only its background and the panel at the left, which now also holds Tutorial and Credits and leads through New game, Multiplayer, the tutorial parts and the difficulty choice, switching at once instead of flying the rings out and in. Classic keeps the original's rings.
- Menu navigation: a new Interface setting "Menu navigation: Enhanced" groups the main menu into one panel (Continue, Quick game, New game, Load, Multiplayer, History, Replays, Settings, Quit), Continue resumes your latest save in one click, and Esc, the right mouse button or a Back label take you one step back on every menu screen; Classic keeps the original's menus.
- Smarter computer empires, part 3: with "Computer empires: Enhanced" the computer empires make peace when a war is going badly, team up against a runaway leader or an easy target, sign pacts and trade agreements when it pays, hold grudges over spies and broken treaties and keep their word; Classic plays as the original.
- Internet game list: with a relay server set, a host can put the room on the server's public list ("Public list: yes" in the room), and other players find it under "Browse internet games" on the connection page and join with a double click; the project's own public server is not running yet, so this works with a relay server you or a friend run.
- More options: the main menu's Options screen has a Controls tab to rebind the game's keys (with a warning when two actions share a key and a reset button), a "Voice volume" row sets the speech apart from the sound effects, and "Extra game speeds" lets the fastest speed go on to x2 and x4 in single player (all default to the original's behaviour).
- Faster start with "Texture upscaling" on: the pictures are upscaled on every processor core, so the game opens in about half the time (6 s down to 3 s on a test machine).
- Richer battles: a new Remake row "Battle effects" (Enhanced) adds sparks, shock waves, embers, smoke columns, shield flashes and glowing shots to the space battles and ground wars, on top of the original's effects (Classic keeps the original's look).
- UI themes: a new Interface setting "UI theme" restyles the remake's own screens (the main-menu Options, match history and statistics, replays, the multiplayer lobby, the event log) in the Modern look or a theme from the mods folder; Classic keeps today's look and the game's own screens never change.
- Smarter computer empires, part 2: with "Computer empires: Enhanced" the computer empires build up shipyards, gather their fleets before striking, declare war when clearly stronger, invade only with enough troops, defend their attacked colonies and retreat when beaten; Classic plays as the original.
- Sharper textures on big screens: a new Remake row "Texture upscaling" (2x or 4x) smooths the game's pictures and model textures as they load, and texture packs in the mods folder can replace any picture or model with a higher-resolution one; every picture keeps its place (off keeps the original's look).
- Campaign: with the new Remake row "Campaign story" set to Enhanced, the five fleets besieging a planet in one of the story episodes start at that planet instead of the far corner of the map (Classic, the default, plays as the original).
- The title video stays quiet over the two logos and its music starts with the intro (Remake row "Title music with intro"; off plays the music from the first logo as the original).
- A new Options screen in the main menu (Settings or `O`): every setting in tabs (display, sound, interface, game, accessibility, multiplayer, language) with switches, sliders and choices, a line explaining each one, and Apply/Back; it works with the keyboard alone, in English and Hungarian, and the in-game Options screen sees the same values.
- New Remake rows under "Graphics": anti-aliasing (smooth edges on buildings, ships and planets), per-pixel lighting for the space battle's ships, soft colony shadows and a gentle glow on fire, blasts and weather; each one separately, all off keeps the original's look.
- Browser build, multiplayer and videos: the page joins a game hosted by a desktop player through a relay server you run (type the relay server and the host's join code; the game itself runs directly between the two machines), and the intro, cut-scenes and message clips play in the browser. The page can't host yet, and no public page is hosted.
- Browser build, single player: games start, save and load in the page (settings, slots, quick save, autosaves and
  replays stay in the browser's private storage), and browsers without WebGPU draw through WebGL2. Videos and
  multiplayer are not there yet.
- Browser build, first steps: the client runs in a web page built locally (`tools/build_web.py`, WebGPU browsers): pick your install folder once, the browser keeps a private copy, and the main menu shows with its music. Videos, saves, settings and multiplayer are not there yet; no public page is hosted.
- Win a scenario and you can put your name in its high-score table: the list opens on it and asks for your name; the table shows how many days the win took (Remake row "High-score entry"; off keeps the original, which never asks).
- Starmap: the mouse wheel zooms the map; the right click gives the fleet its order at once, as in the original, and W/A/S/D move the map (Remake row "Starmap mouse camera"; off keeps the original's controls).
- Options: the remake's own settings have their own REMAKE page, grouped and paged, so every one can be reached; the game page looks like the original again.
- Mods can add or change single lines of text: a `texts.en.toml` (and `texts.hu.toml`, `texts.de.toml`…) in the mod folder sets race names, interface texts, diplomacy phrases and message lines by key, so a new race gets its own lines without replacing whole files; a line with no translation shows in English, and `ig2-check` points at any mistake.
- Mods can change the rules without replacing files: a `rules.toml` in the mod folder sets single values (a tech's price or research time, a building's energy, a weapon's damage, a race's production speed…) by name; `ig2-check` points at any mistyped name, and multiplayer rooms check that everyone has the same changes.
- macOS: openIG2 builds for Apple Silicon Macs (an unsigned archive in releases), finds a copied Windows install in the Applications folders, CrossOver, Whisky or Heroic (or through `IG2_INSTALL_DIR`), and keeps its settings and saves in `~/Library/Application Support/GyroscopeGames/ig2`.
- Ground wars show the original's effects: smoke from damaged vehicles, dust behind moving ones and under bomb blasts, dropship exhaust, flying debris, collapsing buildings leaving fading ruins, scorch marks, and hits and laser beams landing on the target.
- Linux: openIG2 builds for 64-bit Linux, finds an install made through Heroic, Lutris or Wine, and keeps its settings and saves in `~/.local/share/GyroscopeGames/ig2`; releases can carry a Linux archive beside the Windows zip.
- Mods: put your own textures, texts or sounds in a folder under `mods` beside the settings and list it in `mods/enabled.txt` (or start with `--mod NAME`) to use them instead of the install's; multiplayer rooms make sure everyone has the same mods, and `ig2-check` lists what each mod replaces.
- New Remake row "Sharp text": on 1440p and 4K screens the game's own font keeps sharp edges and smooth curves instead of blurring, with every word in the same place (off keeps the original's look).
- New Remake row "Sharp UI": on 1440p and 4K screens the panels, text and pointer stay crisp instead of blurring when scaled up (off keeps the original's smooth look).
- A Hungarian game now shows the remake's own menus and notices in Hungarian too (the extra Options rows, the main menu's Settings, Quick game and History, the statistics screen, the event log, the auto research switches, the multiplayer lobby and its messages, quick save notices).
- After a win or a loss a statistics screen shows how the game went (planets, population, credits, ships built and lost, research, battles won, per empire, with graphs), and the main menu's new "History" lists every finished game, multiplayer ones too (Remake row "Game statistics"; off keeps the original's end).
- Ground battles now show the original's explosions, projectile models flying nose-first with their trails, machine-gun tracers and laser beams.
- Smarter computer empires, part 1: with "Computer empires: Enhanced" (a Remake row for new single-player games, a room option in multiplayer) the computer empires expand, build labs, set taxes by morale and research far better than the original's; Classic (the default) plays as the original.
- Auto research: three switches on the research screen (ships, buildings, tanks) let an idle research slot pick its next project by itself, in single player and multiplayer; all off plays as the original.
- The game starts about three times faster (under a second to the main menu), and a ground war no longer stutters each time a building falls.

### 0.7.81 (beta)
Since 0.7.56.

**Game**

- The starmap now shows the campaign's info lines (story hints such as countdowns) top left, with the chat lines below them.
- The ground-war music now ends with the battle: the main track starts when the war is over or you return to the starmap.
- In a ground war, tanks stand on the planet's ground instead of floating above it, and the landing ships fly in and set them down on screen, also while the game is paused.
- Space battle: the retreat question fits inside its box and shows over the radar, as in the original.
- Spy screen: an open drop-down list now covers the panel texts under it, as in the original.
- Spy screen: it opens without a short freeze.
- The game window, the taskbar and the program file have an icon of their own.
- Space battles: the mouse pointer and its tooltip stay on top of the top bar.
- Auto research groundwork: the game can now pick your next research by itself per category (ships, buildings, tanks); the switches arrive on the research screen and in Options next.

**New Remake options** (Options, REMAKE page)

- F12 quick-saves and Shift+F12 quick-loads; the game autosaves twice a game year into slot 24 as the original does, and a new Remake option "Autosave every" saves every 10, 30 or 90 game days instead, keeping the last two autosaves in the load list.
- Starmap: Ctrl+1…9 put the selected fleet in a group, Alt+1…9 select it again and a second press centres the view on it (Remake row "Fleet groups"; off keeps the original's keys). The groups are saved with the game.
- Messages screen: an event log (the "EVENT LOG" label or `L`) lists every message line of the game with filters (battles, research, diplomacy, colonies, story) and a search; clicking a line about a planet or fleet shows it on the starmap (Remake row "Event log"; off keeps the original screen).
- Accessibility: new Remake options "Subtitles" (a spoken message line's text stays at the bottom while it is spoken) and "Colour-blind colours" (empire colours that stay apart with colour blindness); both off by default.
- W/A/S/D move the camera on the starmap; a new Remake option "WASD camera" can also turn them on in the colony view.
- Options: typing a save name over a used slot no longer shows the old name behind it (Remake row "Clean name entry"; off shows the original's look).
- Starmap and space battle: the turning model in the bottom-left info box stays inside its box (Remake row "Models inside boxes"; off shows the original's look).
- Every game now records a replay beside your saves (`replays` folder, the newest 50 kept) that plays back exactly; the viewer comes later. Switch it off under Options, REMAKE page, "Record replays".
- Replays can be watched: the main menu's "Replays" item lists them and plays one back with the whole map shown, pause, speeds up to 8x and a progress line.
- Space battle: the defend/attack slider no longer runs through its label (Remake row "Slider under label"; off shows the original's look).
- Hungarian games: the article before a name reads "a" or "az" instead of a stray character on the message line, in messages, diplomacy and battle banners (Remake row "Hungarian articles"; off shows the original's look).
- Production view, transfer panel and fleet panel: the Exit label is centred on its button (Remake row "Centred Exit label"; off shows the original's look).
- Main menu: a Settings label (bottom right) and the O key open the Options screen without the slot list, Load and Save (Remake row "Settings in menu"; off keeps the original's menu).
- Main menu: a Quick game label above Settings opens the skirmish settings alone, with the last skirmish's choices; Start begins a generated galaxy in two clicks (shown with "Settings in menu").
- Options: a Language row picks the game's text language; it applies after a restart (Remake row "Language"; `--lang` and `IG2_LANG` still win).

**Multiplayer**

- The host can let defeated players stay and watch the game (room option "Defeated players"; off keeps the original's exit to the menu).
- With the same room option, anyone can join a running game with Join under a new name to watch it.
- The host can set each computer player's difficulty in the room (easy, normal or hard; easy, the default, is how the original plays).
- Multiplayer: the room shows a card per empire (empire colour, race picture, holder, ready mark), and the host can start without waiting for everyone ("Start anyway").

### 0.7.56 (beta)
Since 0.7.48.

**Game**

- Starmap: no white circle around the fleet or planet under the mouse, as in the original.
- Starmap: the light running along a fleet's waypoint arrows moves at the original's pace again.
- Space battle: the retreat button works: answering Yes sends your ships out of the battle, and computer fleets that are losing retreat too, as in the original.
- A short click on a production plus or minus button adds one ship again; holding it still repeats, as in the original.
- Where a fleet's radar area overlaps a planet's on the starmap, only the planet's red shows, as in the original.
- The starmap info panel's production and morale lines, and the planet info's rotating lines, switch every 1.5 seconds, as in the original.
- Leaving a colony for the starmap now cuts off its wind, thunder and war sounds at once, as in the original.
- Messages: Hungarian senders and subjects in the message list are shown whole, with names filled in.
- Starmap: renaming a planet or fleet no longer opens the colony, the info panel or a fleet split when you let go of the buttons.

**Multiplayer**

- The pause button stops the date, the money and the fleets for every player.
- Fleets no longer move across the starmap in jerks: they glide, and stand still while the game is paused.
- The spy screen's report dialogs (hire, reject, verdicts, OK) no longer get stuck as empty boxes after a button press.

### 0.7.48 (beta)
Since 0.7.2.

**Game**

- Options: the ground and space battle "simulate" and "overrun" switches now change how battles are fought.
- The messages screen opens at the right place in the list, and a click opens the message you clicked.
- Urgent messages open the messages screen and play at once, as in the original.
- A waiting message makes the top bar's "new message" text pulse, and the message line pauses until you read it, as in the original.
- Splitting a fleet works again in single player and multiplayer: the new fleet stays, and ships can be moved into it in the transfer panel.
- The starmap shows a faint arrow from each of your planets to its rally planet, under the fleet routes.
- Story spy missions now succeed or get caught, so campaign 2's ending can be reached.
- Story offers sent from the diplomacy screen's special menu now reach their episodes.
- While the tutorial runs, the Options game page shows "Ground battle simulate" and "Space battle simulate" off and greyed out; your own settings come back afterwards (ADR 0019).
- The credits video shows from its first frame instead of staying black for ten seconds.
- In single player, a planet invaded in real time is fought out on its colony screen: the landed vehicles leave
  their dropships, the battle runs and the planet keeps or changes its owner. The war has its sounds (shots, hits,
  explosions, mines, air strikes) and its hit flashes, shot models and debris.
- Single-player space battles: ships now obey attack and move orders however you start the battle (top-bar pause,
  Space or P, the speed keys), and re-entering the battle screen no longer brings the formation choice back.
- Colony screen: clicking a building's roof, tower or upper part selects it, as in the original; the war cursor and
  the right-click attack use the same pick.
- Campaign 1 no longer crashes when the planet-toll raider sets out.
- A fleet split off on the starmap, the ground-war tick and the victory check now follow the screen you are on, as in
  the original.
- The late episodes and endings of all three campaigns are now verified in headless play-throughs (the driver answers
  the stories, researches, sends spies and fleets, builds colony ships), including campaign 2's own ending; campaign
  3's second storyline isn't won yet.

**Multiplayer**

- The game runs at the single-player pace (it ran twice as fast), and orders run within 100-200 ms on a LAN; over the internet the delay follows the slowest player's connection.
- Hosting asks your router to open the game port (UPnP / NAT-PMP) at once; the room's port-mapping line switches it off.
- When a player drops, leaves or is kicked, the computer plays their empire; a player who rejoins gets it back.
- An optional room setting, "Space battle formations" (off by default), lets every human side with six or more ships pick a formation before a real-time space battle; the battle waits for the other players.
- A joining player's diplomacy screen lists the empires they have met, not the host's.
- A colony can build more than one building per visit (the build button comes back once the building is placed).
- In a space battle the joining player can select and order their own ships (the screen used to treat the host's ships as theirs).
- The host can start a game mode (king of the hill, three planets, capture the flag) by typing `story 1001`, `story 1003` etc. in the room chat before starting, as in the original; the room confirms "Starting story set to N" on the host.
- The game no longer stays stuck after a space battle (a player who rejoined or resumed mid-battle, or had Options open when it ended, now leaves it too), and a battle's end no longer pauses everyone.
- A saved game can be resumed without every player; the computer plays the empires of the players who are missing.
- A full-screen video (your own defeat or victory) no longer stalls the others or gets you dropped.
- Opening Options or Help no longer stalls the game for everyone or gets you dropped; the game runs on underneath, as in the original.
- The message line shows your own empire's news again (research done, wars, offers, revolts), with its history and voices.
- A real-time ground war opens its colony screen on every machine and the game goes back to the old screen when it ends.
- Renaming a ship design no longer puts the players' games out of step.
- The multiplayer join page labels its field "Address or join code", so a pasted join code is where to put it.
- In multiplayer you can no longer move, rally, sell or change the settings of another player's fleets and planets, or give building orders on their colonies.
- Internet games through a relay server you run yourself: type its address on the multiplayer page, and joining by code then works behind any router (`docs/RENDEZVOUS.md`).
- When a game gets out of step and is put back in sync, the players see a line on the message bar.
- Diplomacy in multiplayer can take back an unanswered message ("cancel last message"); the tutorial's shorter wait
  between diplomacy messages now applies.

**Tools and project**

- `ig2-client --playthrough mp` plays a whole two-machine multiplayer game to its winner on one computer, comparing both
  machines throughout and saving, resuming and rejoining on the way; no desync is found.
- The website is redesigned after the game's own look: the screens list, a screenshot switcher, install and
  build steps, and the road to 1.0.

### 0.7.2 (beta)

- The client finds your GOG install by itself (registry, GOG Galaxy, the default folders) and checks `ig2.exe`.
  When it finds none, a window lists the places it tried and accepts the game folder dropped onto it.
- The main menu works without a save; a first start no longer plays the campaign's start voice and in-game music
  over the menu.
- The Options screen has display settings: window, borderless or exclusive fullscreen, resolution, vsync, the 4:3
  letterbox, the live background and one-language names. They apply at once and are saved.
- Starmap story icons: the story fleet's wormhole, the virus alert and the gigabomb can be used from the radial
  menu, so the campaign episodes that need them can be completed.
- Attacking the campaign's story planet now plays out as in the original.
- The tutorial locks input like the original (blocked and greyed keys, screens and buttons), and its chapters 1-3
  and 5 play through; chapter 4 runs up to its ground battle. `ig2-client --tutorial N` starts chapter N.
- Internet games: the host can make a room private and share a join code, and can switch on UPnP / NAT-PMP port
  mapping to see the address other players can reach.
- Long multiplayer games (2-4 humans plus AI empires, thousands of turns, battles, saves and rejoins) stay in step;
  a desync after garrisons entered a battlefield and a crash from a malformed diplomacy message are fixed.
- `ig2-client --playthrough` runs whole games headless to the end, with saves and loads on the way. Loaded saves
  now keep their real-time battles and the AI's design names.
- `ig2-check` loads every file of the install and lists what fails to parse, so a broken install is found before
  playing.
- A Windows release zip (the client, the checker and a readme; no game data). `ig2-client --version` and the window
  title show the version.
- Unzipped into the game's folder, the client always uses that install, even when another one is installed.
- The project is published as openIG2 under the AGPL-3.0, with CI, release builds and a website.

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
