<div align="center">

# 🌌 IG2 Rust Rework

**A from-scratch, Rust-powered reimplementation of _Imperium Galactica II_ — built for multiplayer, modern displays and bit-exact fidelity.**

![Rust](https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white)
![wgpu](https://img.shields.io/badge/renderer-wgpu-5E5CE6)
![Phase](https://img.shields.io/badge/phase-2%20%C2%B7%20game%20rules-0A84FF)
![Verified](https://img.shields.io/badge/rules-Frida--verified%20bit--exact-2EA44F)
![Assets](https://img.shields.io/badge/game%20assets-not%20included-lightgrey)

</div>

---

In the spirit of **OpenMW** and **OpenRA**: the engine ships **no original assets and no rule values**.
It reads everything (graphics, text, maps and even the balance tables) from **your own copy** of the GOG
release at runtime.

Every rule is reverse-engineered from the shipped executable, written up as a spec with decompiler
evidence, ported to idiomatic Rust, then **replayed against live Frida traces of the original game until
it matches bit for bit**.

## ✨ Goals

| | |
|---|---|
| 🌐 **Multiplayer first** | Stable online play, rejoin, multiplayer saves (deterministic lockstep, seeded RNG everywhere) |
| 🖥️ **Modern displays** | High refresh rate, high DPI, 4K and ultrawide |
| ⚡ **Faster than the original** | A fixed-step simulation decoupled from rendering |
| 🎯 **Classic fidelity** | The classic ruleset reproduces the original exactly; every deliberate change gets an ADR and an optional *enhanced* mode |

## 📊 Status

### ✅ Done

| Area | What works |
|---|---|
| **Asset formats** | `ecGs`/`neGs` containers, PVR textures, `.xv` 3D archives, colony maps, `.scn` scenarios, `.ani`, UI and keyed string tables, dialogue, diplomacy, formations, story data and story fleets, the tutorial script and its input recordings (6 languages, golden-tested); original save games and the multiplayer state blob |
| **Rules from your exe** | Techs, buildings, hulls, designs, weapons, race/difficulty tables, radar and footprints read from the user's `ig2.exe` (sha256-gated) |
| **Asset viewer** | `ig2-viewer` (eframe + wgpu): models, textures, colony maps, messages; headless `--snapshot` |
| **Time & RNG** | 20 Hz sim clock, calendar, 21 bit-exact RNG streams |
| **Colony economy** | Buildings, energy and workers, 19 problems and morale, population, taxes and GDP, construction |
| **Colony side systems** | Rebellion, conquest and tech capture, colonisation, autobuild AI, money pools, yearly income |
| **Building placement** | Colony-map search with terrain masks, footprints and a spiral search |
| **Fleets** | Movement, waypoints and patrols, following, merging, traders, wormholes; colonisation and terraforming arrivals |
| **Radar & visibility** | Per-empire radar, supply fade, stealth, alliances, discovery, "enemy approaching" slow-down |
| **Space combat (strategic)** | Design stats, battle routing, auto-resolve |
| **Real-time space battles** | Setup from the strategic state (fleets joining a running battle, formations, ships grouped into units, weapons, shields and pods, fighter wings, planet guns and defence ships, asteroids), the 20 Hz battle tick (battle AI, targeting, projectiles, damage, special pods, fighters, path search, retreats), the end and its result |
| **Research & production** | Research progress, AI research picks, production queues, ship/vehicle building |
| **Diplomacy** | Daily relations and opinion, the AI's proposals, threats and declarations, AI-to-AI answers, wars and peace treaties; the human's messages, deals (money, planets, techs, radar maps, hostages, wars, peace), demands, answers and counter-proposals, and the auto-cancel of stale offers; an empire's death (its fleets deleted) and the victory check |
| **Espionage** | Spy ranks and experience, double agents, the catch check and the captor's verdict, 13 missions (reports, theft of money and techs, sabotage, ship strikes, rebellions, assassinations, kidnapping), recruitment |
| **AI empire** | Empire centres and main fleets, the main planet, colony autobuild profiles and tax by wealth and danger, income sliders and shares, production choices (colony ships, default warships and vehicles, scrapping obsolete ships), spy missions (skill points, firing, targets, mission choice and cost); colony ships to the nearest free planets, war declarations, attack targets (fleets and planets), fleeing and regrouping, loading and unloading vehicles, scouting; the AI's own designs and their upgrades |
| **Story engine** | Story queues and start flags, variables, logic flags, counters, story planet and fleet ids, event flags and the hooks other systems call, the per-tick counters and daily check, story endings with their undo list and info lines; the skirmish scenarios' victory and defeat conditions (stories 360–375); the campaign start (storyline and map choice, mirroring, first buildings) and the campaign driver (heroes, storylines, random stories, the opening episodes) with the short scripted stories and story fleets; the six storylines' scripts (their timers, messages and answers, the episodes they start, the planets and empires they hand around); the multiplayer game modes (king of the hill, three planets, capture the flag); the first random stories (mercenaries for hire, a fleet in quarantine, a planet to blow up, strikes, a tax, a refugee planet, a wormhole); the opening episodes (a pirate base, the plague ship, the hunter, rebels, troops wanted, a design for sale, a trade treaty, a colony, a research grant, a saboteur, a tech to steal, a spy's target, a far planet, a fugitive, an escort); the random stories the campaign always runs (the pirate bounty, the wormhole warning, the hero ships for hire) with the small planet, tech and tribute events |
| **Trade** | The merchants' offers (random and story), purchases, delivery countdowns and arrivals, lost destinations re-targeted or refunded, offer expiry |
| **Info and messages** | The info screen's monthly statistics and their smoothed charts; the scrolling message line with its history and the messages the game posts to the player (including espionage, the spy screen's report queue, and the outcomes of auto-resolved space battles and ground wars); the message inbox's expiry |
| **Original saves in the sim** | An original save loads into the simulation and whole game ticks run on it (economy, relations, AI, spies, trade, info charts, research, story, messages, fleets, production, auto-resolved space battles and ground wars with their captures, landings and repairs), and the planets' ground battlefields are written back into the save, matching the original game byte for byte over 2,500 ticks |
| **Ground combat (strategic)** | Routing to the real-time or auto-resolved ground war, landing and group arrangement, the auto-resolved war (attrition, guns, paralysers, manipulators, kamikazes, air strikes, building turrets), survivors re-embarking, capture |
| **Real-time ground battles** | Landing placement and the landing zone, terrain heights and unit orientation, defence-building turrets, the 20 Hz battle tick (driving, passability maps and path search, attack-moves, turret and body turning, aiming, shells, rockets, bolts, paralysers, manipulators, the machine gun, lasers, armour, mines, self-destruct), the campaign's "mad tanks" that pick their own targets, air strikes (big-ship bomb carpets, fighter waves), defence-building turrets sharing hits, the end countdown and its result, transporters (dropships setting units down and picking them up, the player's landing orders, retreats that can be shot down, transporter-pod escorts) |
| **Ground battle AI** | The computer's plans on the battlefield (flanks, reserve, pincer, fortress guards), rally points and formation lines, target choice, reactions to hits, the air-strike target scan |

### 🚧 Planned

| Area | |
|---|---|
| Campaign story scripts (`st_*_include`) | 📝 engine, scenario stories, campaign start and driver, storylines, MP game modes, random stories 220–239, opening episodes 1–24 and 15 of `st_Rnd_include`'s 62 stories done |
| wgpu game renderer and UI | 📝 architecture decided (ADR 0006); `ig2-client` runs an original save at 20 Hz and draws the starmap the way the original does (its 3D camera, star glows, orbiting planets, fleet sprites and palette; re/screens/starmap.md; fleets interpolated at any refresh rate, hover and selection, high DPI and ultrawide, headless `--snapshot`); screens, text and input commands to do |
| Lockstep multiplayer over QUIC: LAN discovery, direct IP, UPnP, rejoin | 📝 command model, lockstep turns with input delay, desync detection by state hash and rejoin from a snapshot + command log work in process, and peers with different empires and screens stay identical (canonical view, `re/rules/multiplayer.md`) (ADR 0007, `ig2-net`); QUIC transport with a pinned per-session certificate works on the local machine; LAN discovery, opt-in UPnP/NAT-PMP port mapping (tested without touching a real router) and the lobby (open, password and private sessions, empire slots, ready flags, chat, start into lockstep; passwords checked by SPAKE2, never sent) work (ADR 0008); lobby UI to do |
| Saves (single and multiplayer) | 📝 original format read (files and MP blob) and loaded into the sim; our own format (versioned, compressed, hash-checked) saves and restores the whole game exactly; save/load UI to do |
| *Enhanced* ruleset options | ⏳ |

## 🚀 Quick start

You need the **GOG release of Imperium Galactica II** and a stable Rust toolchain.

```bash
cd engine
IG2_INSTALL_DIR="/path/to/Imperium Galactica II" cargo test --release
cargo run --release -p ig2-viewer
cargo run --release -p ig2-client -- --save path/to/save.sav   # starmap of an original save
```

Without `IG2_INSTALL_DIR`, the tests that need the game data are skipped.

## 🗂️ Repository

```
engine/   Cargo workspace: ig2-formats · ig2-rules · ig2-sim · ig2-net · ig2-viewer · ig2-client
re/       Reverse-engineering specs: formats, rules, runtime verification reports, STATUS
tools/    Ghidra scripts, Frida tracers, symbol porting, decoders
docs/     Architecture decision records (ADRs)
```

## ⚖️ Legal

This is an independent, non-commercial fan project. It is not affiliated with or endorsed by the rights
holders of *Imperium Galactica II*. **No game files, binaries or extracted data are in this repository.**
You must own the game to use it. The license is not yet decided; until one is chosen, all rights are reserved.

<div align="center"><sub>Built with 🦀 and a lot of disassembly.</sub></div>
