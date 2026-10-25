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
| `IG2_VSYNC` | `0` turns vsync off at start (V toggles it) |
| `IG2_PROFILE`, `IG2_PROFILE_BATTLE`, `IG2_PROFILE_STEP`, `IG2_PROFILE_TOUR` | Per-phase frame timings (`docs/perf.md`) |
| `IG2_GRAB_DIR` | Where headless capture frames go |
| `IG2_NO_LAND`, `IG2_DIP_DEBUG`, `IG2_DIP_DUMP`, `IG2_SPWC_INFO` | Debug switches for specific screens |

## Headless captures
`ig2-client --snapshot out.png [--save f.sav] [--screen <name>] [--ticks N] [--script S] [--size WxH]` renders a
frame or an input script's frames without a window; it is used by the capture comparison
(`tools/trace/run_screendiff.py`, `tools/README.md`). The full flag list is the doc comment at the top of
`engine/crates/ig2-client/src/main.rs`.
