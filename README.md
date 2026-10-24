<div align="center">

# 🌌 IG2 Rust Rework

**A complete remake of the _Imperium Galactica II_ game client in Rust: the original game, rebuilt for multiplayer, modern displays and today's hardware.**

![Rust](https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white)
![wgpu](https://img.shields.io/badge/renderer-wgpu-5E5CE6)
![Phase](https://img.shields.io/badge/phase-3%20%C2%B7%20game%20client-0A84FF)
![Simulation](https://img.shields.io/badge/simulation-matches%20the%20original-2EA44F)
![Assets](https://img.shields.io/badge/game%20assets-not%20included-lightgrey)

</div>

---

IG2 Rust Rework is a new game client for *Imperium Galactica II*, in the spirit of **OpenMW** and **OpenRA**.
You play the game you know (its campaign, skirmishes, starmap, colonies, space and ground battles, diplomacy
and espionage) on a new engine. It plays online properly, runs at any refresh rate and resolution, and
loads your existing saves.

The remake ships **no original assets and no rule values**. It reads graphics, text, maps and even the
balance tables from **your own copy** of the GOG release when it starts.

## ✨ What it will be

| | |
|---|---|
| 🎮 **The whole game** | Single-player campaign, skirmish scenarios and multiplayer, with every screen of the original |
| 🌐 **Multiplayer first** | Lockstep over QUIC: LAN discovery, direct IP, opt-in UPnP, password lobbies, rejoin after a drop, multiplayer saves |
| 🖥️ **Modern displays** | High refresh rate (smooth interpolation between the 20 Hz game ticks), high DPI, 4K and ultrawide |
| ⚡ **Faster than the original** | A fixed-step simulation decoupled from rendering |
| 💾 **Your saves keep working** | Original save games load and play on; our own save format is versioned, compressed and hash-checked |
| 🪞 **A 1:1 remake of the original client** | The same menus, screens, layout, input, sounds and timing: you can't tell which one you started, except by the extras you switch on and the new multiplayer screens (styled like the original's). On widescreen and 4K the original layout is scaled up and letterboxed; an option lets the 3D maps fill the screen. Every screen is accepted only after a pixel comparison with the original ([ADR 0009](docs/decisions/0009-one-to-one-client.md)) |
| 🎯 **Classic fidelity, optional improvements** | The classic ruleset plays exactly like the original; every deliberate change is opt-in through an *enhanced* mode |

## 📍 Where we are

The **game simulation is done**: every strategic and battle system runs and matches the original game.
Work has moved to the **game client**: the screens, the input and the multiplayer front end that make it
playable from start to finish.

| Part | Status | |
|---|---|---|
| Game simulation | ✅ done | Economy, fleets, radar, research, production, diplomacy, espionage, trade, the AI empires, auto-resolved and real-time space and ground battles, info and messages; original saves run whole game days identical to the original |
| Game end and high scores | ✅ done | Victory/defeat/ended flow, the end videos by race, then the main menu; the high-score table format and insert (the original never records a score), checked against the original; the table draw comes with the main menu |
| Tutorial interpreter | ✅ done | The tutorial script's 74 commands and events run as a sim state machine, checked against the original over all five chapters; the client plays it as an overlay on the game screens (window matches the original; some event sites and gate flags still open) |
| Story engine and skirmish scenarios | ✅ done | Scenario victory and defeat, the campaign start and driver, the six storylines, the multiplayer game modes |
| Campaign scripts | ✅ done | `st_Elem_include` complete (part 2g: 844, 856, 862, 863, 0 left) |
| New game start | 🚧 campaign, scenario, MP galaxy, skirmish done | `Game::new_campaign`, `start_scenario`, `start_mp_galaxy` and `start_skirmish` build the start state and equal 35 starts recorded in the original; the client builds its world from a new campaign (`--new-campaign`); the load path follows |
| Game client: starmap | 🚧 playable | The original's 3D camera, stars, orbiting planets, fleets, names and selection; speed and pause, selecting, move/attack/waypoint orders through the lockstep commands; the control panel, planet/fleet info box and minimap; the production view and the transfer panel; the radial icon menu, the planet's long info with its tax, auto-build, priority and station-sale menus; the fleet and vehicle info panels (their sales go through `MULTIGET_Design`) |
| Game client: colony screen | 🚧 playable | Terrain, buildings, roads and ground units, planet panel, radar, building names; select, build (the build menu's list and the placing cursor), repair, demolish and switch buildings, all as the original's commands. The ground war's orders are ported (`ColonyOrder::Ground`). The ground war's controls are in (war buttons, selection, orders, rings, banner, unit info). Landing clicks, landing zone, shadows, unit picture, self-destruct window, pointer shapes and camera keys are in. The planet info mode is in (on/off, tax, auto-build and priority menus). To do: the shots' effects |
| Game client: HUD, text, info, messages | 🚧 drawn and driven | Top bar (money, date, speed), the game's bitmap font, the scrolling message line; the info screen's 3D frame, race heads, empire panel, spending limit, money triangle, charts, the planet/fleet list with its turning planets and fleets (pick a race or a chart, scroll the list), a row's detail view (click a planet or fleet) and the star log; the planet settings menus in the detail view, the money triangle and the spending limit (sent as lockstep commands); the messages screen's frame, inbox and open messages with their dialogue and answers. The screen's writes and story hand-offs are sent as orders (`Command::Message`). |
| Game client: diplomacy screen | 🚧 playable | The contacts' heads and portraits, the leader's video, the info panel, the proposal and answer menus and the dialogue texts; proposals and answers go out as lockstep commands. The opening and closing animation, the special (story) menu and the sounds are in. To do: the MP-only menus, lip sync, the sim writing the special list |
| Game client: spy screen | 🚧 playable | The roster carousel with the spies' figures, names and status icons, the head video, the spy's stats, the mission planner with its lists and chance, reports, verdicts and applicants; every action through the lockstep commands; the frame's opening and closing animation, the tooltips. To do: sounds |
| Game client: research screen | 🚧 playable | The console, its category wheels, the ball with the category's tech models, the tech's texts, lists, labs and the running project; start, stop, urgent and the category/tech choice by mouse and keys, as the original's commands; the opening and closing animation with its lasers, the buttons' press animations, the chosen row's animated frame and the tooltips. the ring of arrowheads round the last pressed category button (research.md §5.6). To do: sounds, the select-menu entry |
| Game client: main menu | 🚧 playable | The title (with its video) and the main menu with its ring menus, hover labels, the galaxy, clouds, dust, meteors and ships, the credits video; the window starts on it (the load ring opens Options in load-only mode). Part 2 done: race choice and difficulty, scenario/skirmish list, tutorial start, multiplayer name entry, exit animation. Starts campaigns, scenarios, skirmishes and tutorials; the local/inet rings open the MP lobby (M5) |
| Game client: other screens | 🚧 started | The Help screen (F1: topic list, wrapped text with pictures, scroll bar, opening/closing; matches the exe's capture). The SpaceWar screen is drawn at rest from a capture's dump (`--snapshot --screen spacewar`) and runs live from the starmap's SP battle with camera, selection and orders (part 5 done: lightning, fighters, tooltips, cursor, voices, big info box; formation re-placement verified bit-exact; part 7: station models, the info view, zoom names; not accepted yet: console and radar regions differ). The Design screen is drawn and driven in the viewing and editing modes (F5; orders verified; part 3 done: pod pick, POD boxes, tech models and fit, effects; hull look done). The Trade screen is drawn and driven (F8; purchases through `Command::Buy`, matches the exe's money and delivery; part 2: hull model, info card, pod list, Options return). To do: the ground-war screen, menus |
| Game client: audio and video | 🚧 audio part 3, video done | Audio: Ogg decoding, a mixer, music with the exe's selection and crossfade, the research, starmap and colony sound ids (verified against the exe), the click sites of info, spy, diplomacy and the typer, colony ambients and build sounds, scroll voices. Video: MP4 (H.264/AAC) decoded with OpenH264 + Symphonia; the title (verified against the exe), credits (with the remaster's text overlay), story, ending and message-head clips. Audio part 3: the speakers, message speech with ducking, alien voices, thunder (verified against the exe). The ground war's voices (verified against the exe). Campaign intros and message clips verified against the exe |
| Screen comparison with the original | 🚧 started | Every screen is listed with the original's functions ([`re/screens/`](re/screens/README.md)); a tool captures the original and our client on the same save and diffs them pixel by pixel. Starmap, colony, info, messages and the spy screen are compared; the spy screen's carousel and panels match, no screen passes whole yet ([results](re/screens/diff-results.md)) |
| Input → game commands | 🚧 started | Speed and pause work; every screen's actions become lockstep commands as they are ported |
| Multiplayer | 🚧 engine done | Lockstep, desync detection and rejoin, QUIC, LAN discovery, port mapping and the lobby work; real-time battles run in lockstep, the ground war's orders are commands (`ColonyOrder::Ground`); a session layer runs a game from lobby to end (drop grace, rejoin and its admission, host migration, resync, pause, multiplayer saves). The lobby and connection screens (our MP room, ADR 0014) work; to do: MP game-loop polish (drop/rejoin UI, MP save, status line) |
| Saves | 🚧 engine done | Original saves load; our own format saves and restores the whole game exactly. To do: the save/load screens |
| *Enhanced* ruleset options | ⏳ later | |

<details>
<summary><b>Simulation systems in detail</b></summary>

| Area | What works |
|---|---|
| **Asset formats** | `ecGs`/`neGs` containers, PVR textures, `.xv` 3D archives, colony maps, `.scn` scenarios, `.ani`, UI and keyed string tables, dialogue, diplomacy, formations, story data and story fleets, the tutorial script and its input recordings (6 languages); original save games and the multiplayer state blob |
| **Rules from your exe** | Techs, buildings, hulls, designs, weapons, race/difficulty tables, radar and footprints read from the user's `ig2.exe` (sha256-gated) |
| **Time & RNG** | 20 Hz sim clock, calendar, 21 seeded RNG streams |
| **Colonies** | Buildings, energy and workers, 19 problems and morale, population, taxes and GDP, construction; rebellion, conquest and tech capture, colonisation, autobuild, money pools, yearly income; building placement on the colony map |
| **Fleets & radar** | Movement, waypoints and patrols, following, merging, traders, wormholes, colonisation and terraforming arrivals; per-empire radar (SP and MP), stealth, alliances, discovery, the "enemy approaching" slow-down |
| **Space battles** | Design stats, battle routing, auto-resolve; the real-time battle (setup, formations, weapons, shields, pods, fighters, planet guns, asteroids, battle AI, path search, retreats, the player's orders) |
| **Ground battles** | Landing, the auto-resolved war, capture; the real-time battle (terrain, path search, every weapon, mines, self-destruct, air strikes, building turrets, transporters, the battle AI's plans, the campaign's "mad tanks") |
| **Research & production** | Research progress, AI research picks, production queues, ship and vehicle building; the design orders (store, sell, refit) |
| **Diplomacy** | Relations, the AI's proposals, threats and declarations, AI-to-AI answers, wars and peace; the player's messages, deals, demands and counter-proposals; an empire's death and the victory check |
| **Espionage** | Spy ranks, double agents, the catch check, 13 missions, recruitment |
| **AI empires** | Empire centres and main fleets, colony management, income, production, spies, colonisation, war declarations, attack targets, fleeing and regrouping, vehicle transport, scouting, its own ship designs |
| **Trade** | Merchant offers (random and story), purchases, deliveries, refunds, expiry; trader income |
| **Info & messages** | Monthly statistics and smoothed charts, the message line and its history, the inbox |
| **Story engine** | Story queues, variables, flags, hooks; skirmish victory/defeat; the campaign start, driver, storylines, random stories, opening episodes and fables; the multiplayer game modes |
| **Original saves** | Load into the simulation and run whole game ticks, matching the original byte for byte over 2,500 ticks |

</details>

### 🧭 What's left

The full list is tracked in [`TASKS.md`](TASKS.md). In short:

- **Playable start to finish (M2):** scenario and skirmish starts, the
  main menu's race/difficulty and game setup, the last parts of the colony war, SpaceWar, Design and Trade
  screens, the campaign intro videos, the game end and high scores.
- **Every screen exact (M3):** help, credits, demo mode, the tutorial overlay, and a final comparison sweep of
  every screen against the original.
- **Input and feel (M4):** a keyboard and mouse audit per screen; 16:9, 21:9 and 4K checks.
- **Multiplayer (M5):** the lobby and connection screens are done (ADR 0014); the MP game-loop polish is left.
- **Polish (M6, after the prototype):** fixes for the original's bugs, enhanced options, graphics upgrades. Deviation triage done (222 rows: 129 fix, 31 investigate, 6 enhanced, 18 keep, 38 closed; queue in `docs/roadmap.md` §6).

## 🚀 Try it

You need the **GOG release of Imperium Galactica II** and a stable Rust toolchain.

```bash
cd engine
export IG2_INSTALL_DIR="/path/to/Imperium Galactica II"
cargo run --release -p ig2-client -- --save path/to/save.sav
```

The client opens an original save and runs it live.

| Key / mouse | |
|---|---|
| Space | pause |
| 1 / 2 / 3 | game speed |
| Mouse wheel | zoom |
| Starmap: right drag, left press | pan; select and open the radial icons (release over one to use it) |
| Starmap: left click on a planet's info box | the planet's long info: tax, auto-build, priority, station sale |
| Starmap: right click with the player's planet selected | its rally planet |
| Starmap: Q / E, Page Up / Down | turn and tilt the camera |
| Starmap: R | show every fleet |
| Starmap: the info box's buttons | the planet's production queues; the transfer between a fleet and the planet |
| Starmap: right click on the fleet's info box | split the fleet |
| C | colony screen of the selected planet (and back) |
| Colony: left drag, right drag | pan, turn and tilt |
| Colony: left click, hold on the planet panel | select a building; the build menu (release on a building, then click where it goes) |
| Colony: the building's buttons, E, Shift+Delete, Page Up / Down | repair / demolish (drag to "Yes"), switch on/off, demolish, the previous / next colony |
| I | info screen (the mouse over a race picks it; the chart buttons, Tab: next chart) |
| M | messages screen (click a message to open it, an answer to answer; a click elsewhere leaves) |
| D | diplomacy screen (click a head to pick an empire; hold the button on a menu entry, release on the item) |
| V | vsync on/off |
| Esc | back to the starmap / quit |

`ig2-viewer` (`cargo run --release -p ig2-viewer`) browses the game's models, textures, colony maps and
messages.

Tests: `cargo test --release` with `IG2_INSTALL_DIR` set. Without it, the tests that need the game data
are skipped.

## 🏗️ Architecture

```
ig2-formats   reads the original files (containers, textures, models, maps, text, saves)
ig2-rules     the game's balance tables, read from your ig2.exe
ig2-sim       the deterministic game simulation (20 Hz ticks, seeded RNG, lockstep commands)
ig2-net       multiplayer: lockstep sessions, QUIC transport, LAN discovery, port mapping, lobby
ig2-client    the game client: wgpu renderer, screens, input
ig2-viewer    asset browser for development
ig2-screendiff  compares our client's screens with captures of the original, pixel by pixel
```

The simulation knows nothing about rendering or the network. The client draws whatever the simulation
state is, interpolated to the display's refresh rate. In multiplayer, every player action is a
command that all peers apply on the same tick. Key decisions are in [`docs/decisions/`](docs/decisions/)
(renderer and client: ADR 0006; lockstep and saves: ADR 0007; lobby: ADR 0008).

## 🎯 How we stay faithful

Every game rule is taken from the shipped executable and written up as a spec under [`re/`](re/)
(start with [`re/STATUS.md`](re/STATUS.md)). It is then ported to Rust and replayed against recordings
of the original game until the results match **bit for bit**. Screens are compared pixel by pixel with
captures of the original (`tools/trace/run_screendiff.py`, inventory in [`re/screens/`](re/screens/README.md)). Any intended difference from the original goes through an ADR and stays optional.

## 🗂️ Repository

```
engine/   Cargo workspace (the crates above)
docs/     Architecture decision records
re/       Specs of the original's formats, rules and screens; verification reports
tools/    Analysis and recording tools used to write the specs
TASKS.md  The remaining tasks, in short
```

## ⚖️ Legal

This is an independent, non-commercial fan project. It is not affiliated with or endorsed by the rights
holders of *Imperium Galactica II*. **No game files, binaries or extracted data are in this repository.**
You must own the game to play it. The license is not yet decided; until one is chosen, all rights are reserved.

<div align="center"><sub>Built with 🦀 for a game we still love.</sub></div>
