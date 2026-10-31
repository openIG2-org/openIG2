# Building and testing

## Toolchain
- Rust stable. On Windows the project uses `stable-x86_64-pc-windows-gnu` (no MSVC C++ tools needed).
- A GPU with Vulkan, DirectX 12 or Metal (wgpu).
- For reverse engineering only: Ghidra (headless), Python 3 with Frida 17 (`_local/venv`), the GOG `ig2.exe`.

## Build
```bash
cd engine
cargo build --release -p ig2-client
```
Always use `--release` to play: the debug build runs at a few fps.

## Release bundle
`python -I tools/release/build_windows.py [--out DIR] [--keep-target]` builds `ig2-client` and `ig2-check` in
`_local/release-target` (its own target dir, about 2 GB, removed afterwards unless `--keep-target`) and stages
`_local/release/openig2-<version>-windows-x64/` (both exes, `README.txt`, `VERSION.txt`) plus the zip (about
14 MB; no game data). The version is MAJOR.MINOR.N (`docs/VERSIONING.md`, `tools/release/version.py`), shown in the client's window title,
`ig2-client --version` and `ig2-check`; builds outside the release script show `VERSION` with `-dev`. The gnu toolchain's C++ runtime (openh264) and C runtime are linked
statically (`CXXSTDLIB=static=stdc++`, `crt-static`), which needs a MinGW-w64 `g++` with static libs (e.g. WinLibs)
on the PATH; the script fails if a binary imports a non-system DLL. A full build takes about 5 minutes.
The release workflow (`.github/workflows/release.yml`) runs it and publishes the zip.

`ig2-check` (`engine/crates/ig2-check`) validates an install: `cargo run --release -p ig2-check -- [DIR] [--quick]`.
`ig2-rendezvous` (`engine/crates/ig2-rendezvous`) is the optional internet relay service (`docs/RENDEZVOUS.md`).

## Tests
```bash
cd engine
IG2_INSTALL_DIR="/path/to/Imperium Galactica II" cargo test --release
cargo clippy --all-targets
```
- Without `IG2_INSTALL_DIR` the tests that need game data are skipped (they don't search; the client does,
  `docs/USER_GUIDE.md` → Installing). `IG2_INSTALL_SEARCH=0` limits the client's search to `IG2_INSTALL_DIR` and
  the remembered folder.
- The trace replays need recordings in `_local/traces/` (`IG2_TRACE_DIR` points elsewhere); the save tests read
  `IG2_SAVE_DIR`. Recordings are made with the tracers in `tools/trace/` from your own install.
- The campaign replay (`campaign_matches_exe`) takes about 40 minutes; skip it in everyday runs with
  `-- --skip campaign_matches_exe`.

## Environment variables
| Variable | Meaning |
|---|---|
| `IG2_INSTALL_DIR` | The GOG install (optional override; else detected) |
| `IG2_INSTALL_SEARCH` | `0` limits detection to `IG2_INSTALL_DIR` and the remembered folder |
| `IG2_LANG` | UI language `en hu de fr es it` (else `--lang`, else the exe's own pick) |
| `IG2_SAVE_DIR` | Where saves are read from (default `_local/saves`) |
| `IG2_SLOT_DIR`, `IG2_OPTIONS_DIR` | Our save slots and settings file |
| `IG2_TRACE_DIR` | Recordings for the replay tests |
| `IG2_VSYNC` | `0` turns vsync off at start (V toggles it; the Options screen's `VSync` setting is the saved default) |
| `IG2_PROFILE`, `IG2_PROFILE_BATTLE`, `IG2_PROFILE_STEP`, `IG2_PROFILE_TOUR` | Per-phase frame timings (`docs/perf.md`) |
| `IG2_SOAK_TURNS`, `IG2_SOAK_SEED`, `IG2_SOAK_SEEDS`, `IG2_SOAK_STOP`, `SOAK_EVENTS` | Desync soak (`ig2-net/tests/soak.rs`, `re/rules/multiplayer.md` §7.1.1) |
| `IG2_GRAB_DIR` | Where headless capture frames go |
| `IG2_NO_LAND`, `IG2_DIP_DEBUG`, `IG2_DIP_DUMP`, `IG2_SPWC_INFO` | Debug switches for specific screens |

## Headless captures
`ig2-client --snapshot out.png [--save f.sav] [--screen <name>] [--ticks N] [--script S] [--size WxH]` renders a
frame or an input script's frames without a window; it is used by the capture comparison
(`tools/trace/run_screendiff.py`, `tools/README.md`). The full flag list is the doc comment at the top of
`engine/crates/ig2-client/src/main.rs`.

## Play-through harness
`ig2-client --playthrough <runs> [--ticks N] [--seed S] [--save-every M] [--stall M] [--human ai|idle] [--compare-every N]` plays whole games
headless at
the fastest speed on the player's path (`World::tick`, the commands a screen sends), with the sim's AI playing the
human's empire (`Game::autopilot`). Runs: `campaign:C[:D[:L]]` (campaign 1..=3, difficulty 1/2/4, storyline 1/2; 0 or
none: the registry's or a random one), `scenario:<name|all>`, `skirmish[:RACE[:AIS]]`, `mp[:AIS[:PLANETS]]` (below),
or `all` (the three campaigns on difficulty 1, every scenario of the menu, a skirmish). The harness reads the inbox
(the first answer each dialogue allows), leaves finished real-time battles, takes the pause off and saves
and loads our `.ig2s` every M game months (default 6). Each run ends in victory, defeat, the tick budget (default
400,000 ticks of 50 ms), a stall (nothing changed hands and no story moved for M months, default 120) or a stuck
clock, and reports panics, broken invariants (money, fleets or planets of dead empires, empty fleets, a save that
changes the state), the stories left running and what the client was asked to show (videos, real-time battles,
the end's: `video Won` is the race's win clip and `misc/end_campaign_win.bik`). In a campaign the harness also
plays the story's part a player plays (`playthrough/story.rs`): it answers the episodes' messages the way that keeps
the storyline going (or leaves one to expire), researches the techs an episode asks for, sends a spy on the story's
spy missions, sends the story specials (8571 with 2,000,000 credits, 8581, 8601) once each, and gives up to two
fleets the story's targets (the story planets the rebels and the episodes' invaders hold, the raiders to hunt, a planet to guard or visit, empire 5's planet of story 858); everything else
stays the AI's. Whether the AI-played human wins the war depends on the seed: on difficulty 1, `campaign:1:1:1` and
`campaign:1:1:2` with `--seed 2` and `campaign:2:1:2` with the default seed end won (the end video); campaign 3
isn't won yet, and story 863's end (CampaignOver) waits on the story spy missions (861).
`--human idle` keeps the human human and idle instead: every battle it is in runs in real time (the harness lets
the units fight it out) and the diplomacy questions reach its inbox. The exit code is 1 when a run failed (a panic,
a stuck clock, a broken invariant; a stall is reported, not failed). `IG2_PLAYTHROUGH_DEBUG=1` prints each inbox visit.

`mp[:AIS[:PLANETS]]` (default `mp:2:40`) plays a generated multiplayer galaxy on two machines in one process, host and
joiner, each on the player's multiplayer path (the room, `World::new_mp`, the session pumped by `World::tick`) over an
in-process loopback link. Both humans are played by the AI (`--human idle`: idle humans; their real-time space battles
are left when over). Every `--compare-every N` ticks (default 200) both machines stop at the same turn and their
snapshots of the shared state must be equal; the lockstep's per-turn hashes are checked throughout. Every
`--save-every` months the host's MP save is read back, resumed in a new room and rejoined, and the game goes on from
it. A desync fails the run with the first turn whose hashes differ and the parts and memory addresses that differ;
`--compare-every 2` narrows it to the turn; `IG2_PLAYTHROUGH_DEBUG=1` prints each empire's planets, fleets and money now and then. The galaxy grows slowly, so a whole game (to its winner, about 50 years and
60,000 ticks) needs a longer stall limit:
```bash
ig2-client --playthrough mp --stall 240
```

The same as a test, off by default (it takes minutes per run):
```bash
cd engine
IG2_PLAYTHROUGH=campaign:1,skirmish IG2_PLAYTHROUGH_TICKS=100000 cargo test --release -p ig2-client playthrough -- --ignored --nocapture
```
