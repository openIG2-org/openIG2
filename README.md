<div align="center">

# 🌌 IG2 Rust Rework

**A remake of the _Imperium Galactica II_ game client in Rust: the original game on a new engine, built for
multiplayer, modern displays and today's hardware.**

![Rust](https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white)
![wgpu](https://img.shields.io/badge/renderer-wgpu-5E5CE6)
![Milestone](https://img.shields.io/badge/milestone-M7%20playable%201.0-0A84FF)
![Assets](https://img.shields.io/badge/game%20assets-not%20included-lightgrey)

</div>

> **Early development.** Every screen of the original is in, but no game has been played through to the end yet
> and there is no packaged build. Expect bugs.

IG2 Rust Rework is a new client for *Imperium Galactica II*, in the spirit of **OpenMW**, **OpenRA** and
**openOMSI**. It ships **no original assets and no rule values**: graphics, text, maps and the balance tables are
read from **your own copy** of the GOG release at start-up.

## What you get
- **The whole game**: the campaign, scenarios, skirmish, the tutorial and multiplayer, with every screen of the
  original: starmap, colony and ground war, space battles, research, design, production, trade, diplomacy,
  espionage, info, messages, options and help.
- **The original's rules, bit for bit**: the simulation matches recordings of the original game tick for tick, and
  original save games load and play on.
- **Multiplayer first**: lockstep over QUIC, LAN discovery, direct IP, password rooms, rejoin after a drop, host
  migration and multiplayer saves.
- **Modern displays**: any refresh rate (interpolated between the 20 Hz game ticks), high DPI, 4K and ultrawide
  following the remaster's own layout rule.
- **Classic by default**: the client looks, sounds and handles like the original; every deliberate change is an
  opt-in *enhanced* option with an ADR.

## Status
| Part | State |
|---|---|
| Simulation (economy, fleets, combat, ground war, AI, diplomacy, espionage, trade, story engine, campaign, tutorial) | ✅ done, verified against the original |
| Asset formats, rules from your `ig2.exe`, original saves | ✅ done |
| Every screen of the original, reachable in the live client | ✅ done, a few gaps (below) |
| Audio and video | ✅ done |
| Multiplayer over LAN / direct IP | ✅ done |
| A whole game played to the end | ❌ not yet |
| Packaged build (install detection, display settings, release bundle) | ❌ not yet |
| Internet multiplayer without port forwarding | ❌ not yet |

The work left for a playable 1.0 is in [`TASKS.md`](TASKS.md); what changed is in [`CHANGELOG.md`](CHANGELOG.md).

## Installation
There is no release yet. To run it from source you need the **GOG release of Imperium Galactica II** and a stable
Rust toolchain:

```bash
cd engine
export IG2_INSTALL_DIR="/path/to/Imperium Galactica II"
cargo run --release -p ig2-client
```

The client starts at the title and main menu. Always build with `--release`. Keys, flags and settings:
[`docs/USER_GUIDE.md`](docs/USER_GUIDE.md).

## Goals
1. **1:1 with the original**: rules, screens, input, sounds and timing of the GOG client
   ([ADR 0009](docs/decisions/0009-one-to-one-client.md)).
2. **No original code or assets**: everything is read from the user's install; formats and rules are documented in
   [`re/`](re/README.md).
3. **A better engine**: stable online play, rejoin, any resolution and refresh rate, faster than the original.

## Documentation
| Document | Contents |
|---|---|
| [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md) | Running the game, keys, command-line flags, settings |
| [`docs/BUILDING.md`](docs/BUILDING.md) | Toolchain, tests, environment variables, profiling |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crates, data flow, determinism, the original's systems mapped to crates |
| [`docs/decisions/`](docs/decisions/) | Architecture decision records |
| [`re/README.md`](re/README.md) | The reverse-engineering knowledge base: formats, rules, screens, open unknowns |
| [`re/deviations.md`](re/deviations.md) | Known differences from the original |
| [`tools/README.md`](tools/README.md) | Ghidra scripts, Frida tracers, capture comparison |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How work is done here |

## Repository layout
```
engine/      Cargo workspace: ig2-formats, ig2-rules, ig2-sim, ig2-net, ig2-client, ig2-viewer, ig2-screendiff
re/          Specs of the original's formats, rules and screens; trace reports
docs/        User guide, building, architecture, decision records
tools/       Analysis, tracing and capture-comparison tools
TASKS.md     The work queue
CHANGELOG.md What was finished, by milestone
```

## Legal
This is an independent, non-commercial fan project, not affiliated with or endorsed by the rights holders of
*Imperium Galactica II*. **No game files, binaries or extracted data are in this repository**; you must own the game
to play it. The license is not yet decided; until one is chosen, all rights are reserved.

<div align="center"><sub>Built with 🦀 for a game we still love.</sub></div>
