<div align="center">

<img src="site/assets/logo.svg" alt="openIG2" width="160">

# openIG2

**An open-source engine for _Imperium Galactica II_: the original game on a new engine, built for multiplayer,
modern displays and today's hardware.**

[![Build](https://github.com/openIG2-org/openIG2/actions/workflows/ci.yml/badge.svg)](https://github.com/openIG2-org/openIG2/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/openIG2-org/openIG2?include_prereleases&label=release)](https://github.com/openIG2-org/openIG2/releases)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-lightgrey)
![Game assets](https://img.shields.io/badge/game%20assets-not%20included-lightgrey)

[Website](https://openig2-org.github.io/openIG2/) · [Download](https://github.com/openIG2-org/openIG2/releases/latest) ·
[User guide](docs/USER_GUIDE.md) · [Changelog](CHANGELOG.md)

</div>

> **Beta.** Every screen and system of the original is in, and long games run without errors. Some campaign
> stretches and rare screen states are still being checked against the original; expect bugs and please report
> them.

openIG2 is a new client for *Imperium Galactica II* (1999), in the spirit of **OpenMW**, **OpenRA** and
**openOMSI**. It ships **no original assets and no rule values**: graphics, text, maps, voices and the balance
tables are read from **your own copy** of the GOG release when the game starts.

## What you get
- **The whole game**: the campaigns, scenarios, skirmish, the tutorial and multiplayer, with every screen of the
  original: starmap, colonies and ground war, space battles, research, design, production, trade, diplomacy,
  espionage, info, messages, options and help.
- **The original's rules, bit for bit**: the simulation matches recordings of the original game tick for tick, and
  original save games load and play on.
- **Multiplayer first**: LAN discovery, direct IP, private rooms with join codes, optional UPnP / NAT-PMP port
  mapping, rejoin after a drop, host migration and multiplayer saves.
- **Modern displays**: any resolution and refresh rate (smoothly interpolated between the 20 Hz game ticks), high
  DPI, 4K and ultrawide, window, borderless or fullscreen.
- **Classic by default**: it looks, sounds and handles like the original; every deliberate change is an opt-in
  *enhanced* option with a written decision record.

## Download
1. Own and install **Imperium Galactica II** from [GOG](https://www.gog.com/).
2. Download the latest `openig2-<version>-windows-x64.zip` from
   [Releases](https://github.com/openIG2-org/openIG2/releases/latest).
3. Unzip it **into the game's folder** (next to `ig2.exe`) and run `ig2-client.exe`.

That's it. The client also finds the game on its own (GOG registry, GOG Galaxy, the usual folders) if you unzip it
elsewhere; when it can't, its window tells you where it looked and you can drop the game's folder onto it.
`ig2-check.exe` checks your install and lists any file that fails to load. Settings, saves and keys:
[`docs/USER_GUIDE.md`](docs/USER_GUIDE.md). Other platforms: build from source (below).

## Goals
1. **1:1 with the original**: rules, screens, input, sounds and timing of the GOG release
   ([ADR 0009](docs/decisions/0009-one-to-one-client.md)).
2. **No original code or assets**: everything is read from the player's install; the formats and rules are
   documented from scratch in [`re/`](re/README.md).
3. **A better engine**: stable online play, rejoin, any resolution and refresh rate, faster than the original.

## Documentation
| Document | Contents |
|---|---|
| [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md) | Installing, running, keys, command-line flags, settings |
| [`docs/BUILDING.md`](docs/BUILDING.md) | Toolchain, tests, environment variables, release builds |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crates, data flow, determinism, the original's systems mapped to crates |
| [`docs/VERSIONING.md`](docs/VERSIONING.md) | Version numbers and release channels |
| [`docs/decisions/`](docs/decisions/) | Architecture decision records |
| [`re/README.md`](re/README.md) | The reverse-engineering knowledge base: formats, rules, screens, open questions |
| [`re/deviations.md`](re/deviations.md) | Known differences from the original |
| [`tools/README.md`](tools/README.md) | Analysis, tracing and capture-comparison tools |
| [`TASKS.md`](TASKS.md) | What's left before 1.0 |

## Building from source
You need a stable [Rust](https://rustup.rs/) toolchain and the game installed:

```bash
cd engine
cargo run --release -p ig2-client
```

The client finds the install itself; set `IG2_INSTALL_DIR` only to pick a specific one. Always build with
`--release`. Tests, toolchain notes and the release build: [`docs/BUILDING.md`](docs/BUILDING.md).

## Repository layout
```
engine/      Cargo workspace: ig2-formats, ig2-rules, ig2-sim, ig2-net, ig2-client, ig2-check, ig2-viewer, ig2-screendiff
re/          Specs of the original's formats, rules and screens; trace reports
docs/        User guide, building, architecture, versioning, decision records
tools/       Analysis, tracing, capture-comparison and release tools
site/        The project website (GitHub Pages)
```

## Contributing
Bug reports, play reports and pull requests are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) first: in short,
no original assets, one change per pull request, a description of what a player notices, and tests.
How decisions are made: [`GOVERNANCE.md`](GOVERNANCE.md). Security issues: [`SECURITY.md`](SECURITY.md).

## Legal
openIG2 is an independent, non-commercial fan project. It is not affiliated with or endorsed by the rights
holders of *Imperium Galactica II* or by GOG. **No game files, binaries or extracted data are in this repository
or its releases**; you need your own copy of the game to play.

openIG2 is free software under the [GNU Affero General Public License v3.0 only](LICENSE)
(SPDX: `AGPL-3.0-only`). Forks are welcome under that license; please give them their own name
([`TRADEMARKS.md`](TRADEMARKS.md)).

<div align="center"><sub>Built with 🦀 for a game we still love.</sub></div>
