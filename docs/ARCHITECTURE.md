# Architecture

The engine is a Cargo workspace in `engine/` (~140k lines of Rust). It mirrors the original's systems so that every
ported function has one home, and keeps the simulation free of rendering and networking so it can be replayed
against recordings of the original.

## Crates
| Layer | Crate | What it holds |
|---|---|---|
| Formats | `ig2-formats` | Readers for the original's files: `ecGs`/`neGs` containers and the VFS, PVR textures, `.xv` models, colony maps, `.scn` scenarios, text and keyed strings, dialogue, diplomacy, formations, story data, the tutorial script, MP4 video, high scores, options, original saves; the locale pick |
| Rules | `ig2-rules` | The balance tables read from the user's `ig2.exe` (sha256-gated PE reader): techs, buildings, hulls, designs, weapons, race and difficulty tables, radar, sound and speech ids |
| Simulation | `ig2-sim` | The whole game, deterministic: `time`/`rng` (20 Hz clock, 21 seeded streams), colonies, fleets and radar, production, research, trade, diplomacy, espionage, the AI (`ai/`), space battles (auto and real-time `spacebattle`), ground war (`ground/`), the story engine and campaign scripts (`story`, `elem/`, `campaign`, `fable`), the tutorial interpreter, saves (`save/`), and `game/` which applies player orders and runs the tick |
| Network | `ig2-net` | Lockstep (`lockstep`, `command`, `wire`), QUIC transport, LAN discovery, UPnP/NAT-PMP port mapping, the lobby and the session layer (drops, rejoin, resync, host migration, MP saves) |
| Client | `ig2-client` | The game: winit window and wgpu renderer (`render`, `render3d`, `lighting`, `font`), the screen kit (`kit/`), one module group per screen (`mainscr*`, `starmap*`, `colony*`, `spacewar*`, `research`, `design*`, `trade*`, `diplomacy*`, `spy*`, `info_screen`, `messages_screen`, `options`, `help`, `tutorial_ui`, `mp_lobby`), audio and video, input scripts for headless captures |
| Tools | `ig2-viewer`, `ig2-screendiff` | An asset browser; the pixel comparison of client captures with captures of the original |

## Data flow
```
user's install ──► ig2-formats ──► ig2-rules (tables from ig2.exe)
                                    │
player input ──► ig2-client ──► Command ──► ig2-net (lockstep) ──► ig2-sim tick (20 Hz)
                     ▲                                               │
                     └──────────── read-only world state ◄───────────┘
```
- The client never mutates the simulation directly: every player action is a `Command` that all peers apply on
  the same tick (ADR 0007). Single player runs the same path with one peer.
- The client draws the latest state, interpolated to the display's refresh rate (ADR 0006).
- Saves are the simulation state plus the command stream position; original saves load into the same state.

## Determinism
- The simulation is integer- and float-exact against the original: the exe's f32/f64 precision, rounding and RNG
  call order are kept (`re/README.md` → Hard facts).
- Verification: traces recorded from `ig2.exe` with Frida (`tools/trace/`) are replayed by the tests; a replay must
  match byte for byte (campaign, saves, battles, AI, story, new-game starts).
- Multiplayer compares state hashes each tick; a mismatch is a desync and triggers a resync.

## The original mapped to the crates
| Original system | Crate / module | State |
|---|---|---|
| Asset containers, textures, models, maps, text | `ig2-formats` | done |
| Balance tables in the exe | `ig2-rules` | done |
| Strategic tick (colonies, fleets, radar, production, research, trade) | `ig2-sim` | done |
| AI empires | `ig2-sim::ai` | done |
| Diplomacy, espionage | `ig2-sim::diplomacy`, `spy` | done |
| Space battle (auto, real-time) | `ig2-sim::spacebattle` | done |
| Ground war (auto, real-time) | `ig2-sim::ground` | done except the story-planet war (`Grwar8020hoz`) |
| Story engine, campaign, fables | `ig2-sim::story`, `elem`, `campaign` | done |
| Tutorial interpreter | `ig2-sim::tutorial` | done; the client's input locks are open |
| Original MP (IRC/router) | replaced by `ig2-net` (ADR 0005, 0014) | LAN/direct IP done; internet relay open |
| Every screen | `ig2-client` | done; gaps in `TASKS.md` |
| Audio, video | `ig2-client::audio`, `video` | done |

## Decisions
The key decisions are ADRs in `docs/decisions/`: asset source (0001), toolchain (0002), clock and RNG (0003), rules
from the user's exe (0004), MP hosting (0005), renderer and client (0006), lockstep and saves (0007), LAN discovery
and lobby (0008), the 1:1 client (0009), widescreen (0010), real-time battles in lockstep (0011), host migration
(0012), video decoding (0013), the MP room (0014), the live overlay background (0015), deliberate keeps (0016), one
language (0017).
