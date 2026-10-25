# Contributing to openIG2

Thanks for helping. Bug reports and play reports are as valuable as code: say what you did, what you expected
(ideally: what the original does) and what happened, with the version (`ig2-client --version`) and a save if you
have one.

## Ground rules
- **No original code or assets.** Nothing from the game's install, no decoded or extracted files, no binaries in
  commits, issues or pull requests. Rule values and formats are described in the specs under `re/`; the engine
  reads the actual data from the player's install at runtime. Tests that need game data read it from
  `IG2_INSTALL_DIR` and skip when it is unset.
- **Fidelity is the default.** The classic ruleset and client match the GOG release. A deliberate difference is an
  opt-in *enhanced* option with a decision record in `docs/decisions/`; known differences are listed in
  `re/deviations.md`.
- **Evidence first.** For anything ported from the original, write or update the spec (the original's function and
  address) before the code, and test the ported formula with values observed in the original at runtime.
- **Features before polish.** What a player can't do yet comes before tuning what already works. The open work is
  in `TASKS.md`.
- **AI-assisted contributions are welcome.** You are the author: you have read, understood and tested what you
  submit, and you answer for it in review.

## Pull requests
- **One change per pull request**, on its own branch, rebased on `main`.
- **Describe what a player notices** (or what a contributor gets), not how the code changed.
- **Tests**: `cd engine && cargo test --release` and `cargo clippy --all-targets` pass; new behaviour has a test.
  For a new screen or screen state, compare a capture with the original (`tools/trace/run_screendiff.py`).
- **Docs in the same pull request**: tick `TASKS.md`, one line in `CHANGELOG.md`, the spec's port status, a row in
  `re/deviations.md` for any difference you leave. Record each fact once, in its spec.
- Match the style of the surrounding code (not rustfmt-default); idiomatic Rust, not the original's assembly line
  by line.
- CI rejects files with asset extensions and files over 1 MB.

## Deviations
Add a row to `re/deviations.md` when you find a difference from the original (or one of the original's own bugs).
Fix it when a player would notice it (gameplay, a missing animation or sound, wrong text, broken input).
Sub-pixel, colour-tolerance, random-stream and sub-0.1 s timing differences are kept unless they change gameplay.

## Reverse-engineering workflow
1. Read the original's code (Ghidra, `tools/ghidra/`) and trace it at runtime when values matter (Frida,
   `tools/trace/`), always on your own copy of the game.
2. Write or update the spec under `re/`.
3. Port it with tests; check it in the live window.
4. Long test runs: the campaign replay takes about 40 minutes; skip it in everyday runs with
   `-- --skip campaign_matches_exe`.

Details: `docs/BUILDING.md`, `tools/README.md`, `re/README.md`.

## License
By contributing you agree that your contribution is licensed under the project's license, AGPL-3.0-only.
