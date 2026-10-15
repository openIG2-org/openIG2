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
| **Asset formats** | `ecGs`/`neGs` containers, PVR textures, `.xv` 3D archives, colony maps, `.scn` scenarios, `.ani`, UI and keyed string tables, dialogue, diplomacy, formations, story data (6 languages, golden-tested) |
| **Rules from your exe** | Techs, buildings, hulls, designs, weapons, race/difficulty tables, radar and footprints read from the user's `ig2.exe` (sha256-gated) |
| **Asset viewer** | `ig2-viewer` (eframe + wgpu): models, textures, colony maps, messages; headless `--snapshot` |
| **Time & RNG** | 20 Hz sim clock, calendar, 21 bit-exact RNG streams |
| **Colony economy** | Buildings, energy and workers, 19 problems and morale, population, taxes and GDP, construction |
| **Colony side systems** | Rebellion, conquest and tech capture, colonisation, autobuild AI, money pools, yearly income |
| **Building placement** | Colony-map search with terrain masks, footprints and a spiral search |
| **Fleets** | Movement, waypoints and patrols, following, merging, traders, wormholes; colonisation and terraforming arrivals |
| **Radar & visibility** | Per-empire radar, supply fade, stealth, alliances, discovery, "enemy approaching" slow-down |
| **Space combat (strategic)** | Design stats, battle routing, auto-resolve |
| **Research & production** | Research progress, AI research picks, production queues, ship/vehicle building |

### 🚧 Planned

| Area | |
|---|---|
| Real-time space battles (`SpwC`) as a fixed-step sim | 📝 specified |
| Ground combat (`WF*`) | ⏳ |
| Diplomacy, espionage and the AI empires | ⏳ |
| Campaign and story scripting | ⏳ |
| wgpu game renderer and UI | ⏳ |
| Lockstep multiplayer over QUIC: LAN discovery, direct IP, UPnP, rejoin | ⏳ |
| Saves (single and multiplayer) | ⏳ |
| *Enhanced* ruleset options | ⏳ |

## 🚀 Quick start

You need the **GOG release of Imperium Galactica II** and a stable Rust toolchain.

```bash
cd engine
IG2_INSTALL_DIR="/path/to/Imperium Galactica II" cargo test --release
cargo run --release -p ig2-viewer
```

Without `IG2_INSTALL_DIR`, the tests that need the game data are skipped.

## 🗂️ Repository

```
engine/   Cargo workspace: ig2-formats · ig2-rules · ig2-sim · ig2-viewer
re/       Reverse-engineering specs: formats, rules, runtime verification reports, STATUS
tools/    Ghidra scripts, Frida tracers, symbol porting, decoders
docs/     Architecture decision records (ADRs)
```

## ⚖️ Legal

This is an independent, non-commercial fan project. It is not affiliated with or endorsed by the rights
holders of *Imperium Galactica II*. **No game files, binaries or extracted data are in this repository.**
You must own the game to use it. The license is not yet decided; until one is chosen, all rights are reserved.

<div align="center"><sub>Built with 🦀 and a lot of disassembly.</sub></div>
