# Contributing

How work is done in this repository. This file is the shared workflow.

## Ground rules
- **No original code or assets.** Nothing from the install, no decoded files, no extracted rule values, no
  binaries in commits. Tests that need game data read it from `IG2_INSTALL_DIR` and skip when it is unset.
- **Features before polish.** The queue is `TASKS.md`. A task is done when a player can do it in the live window.
- **Fidelity is the default.** The classic ruleset and client match the GOG release; every deliberate difference
  is an opt-in enhanced option with an ADR in `docs/decisions/`.
- **Evidence first.** Write the spec (exe function + address) before the port; every ported formula gets a test with
  values observed at runtime.
- **One task per branch, one change per commit**, described by what the player notices.

## Workflow
1. Take an open line of `TASKS.md`.
2. Spec: read the original's code (Ghidra, `tools/ghidra/`), trace it at runtime when values matter (Frida,
   `tools/trace/`), write or update the spec under `re/`.
3. Port in idiomatic Rust (not assembly line by line), with tests.
4. Check it in the live window, then the scoped test sweep (below). For a **new** screen or screen state, compare a
   capture with the original (`tools/trace/run_screendiff.py`); don't tune existing screens pixel by pixel.
5. Docs: tick `TASKS.md`, one line in `CHANGELOG.md`, the spec's port status, a row in `re/deviations.md` for any
   difference you leave. Record each fact once (in the spec).
6. Commit (signed) on the task branch, rebase on `origin/main`, fast-forward `main`, push.

## Deviations
`re/deviations.md` lists known differences from the original and the original's own bugs. Add a row when you find
one; fix it only if a player would notice (gameplay, a missing animation or sound, wrong text, broken input).
Sub-pixel, colour-tolerance, RNG-stream and sub-0.1 s timing differences are **keep** unless they change gameplay.

## Tests and sweeps
- `cd engine && cargo test --release` with `IG2_INSTALL_DIR` set; keep `cargo clippy --all-targets` clean.
- Scope by what changed: `ig2-sim`/`ig2-rules`/`ig2-formats` → the workspace without the ~40-min campaign replay
  (`-- --skip campaign_matches_exe`); client only → the client tests and the accepted screens; `ig2-net` → `ig2-net`.

