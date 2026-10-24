# Client performance (2026-10-24)

The owner saw **2 fps** in `ig2-client`. Cause: the **debug build** (`cargo run` without `--release`) runs one
strategic tick in ~50 ms, and the tick clock keeps the original's 500 ms catch-up clamp (`ig2_sim::time::TickClock`):
every frame runs the 10 ticks the clamp lets through, 10 × 50 ms = one frame every 0.5 s. The release build was fast on
every screen, but its tick still cost ~7 ms, a dropped frame 20 times a second at 144 Hz. Both came from one place in
`ig2-sim`, fixed below.

## How to run it fast

```bash
cd engine
cargo run --release -p ig2-client -- --save path/to/save.sav
```

Always `--release`. `V` toggles vsync in the window; `IG2_VSYNC=0` starts with it off.

## Measuring: `IG2_PROFILE`

`IG2_PROFILE=1` makes the window log one line per second to stderr (`ig2-client::prof`): the screen, fps, mean and worst
frame time, the mean ms per frame of each phase, and per-frame counters.

| Phase | What |
|---|---|
| `sim` | the message pass and the due sim ticks (`World::pump`, `World::tick`); inside it `t_run` (`Game::run_tick`), `t_snap` (the starmap snapshot), `t_ui`, `t_colony` |
| `ui` | the UI clocks, panels, menus and screen steps of the frame |
| `offscreen` | the panels' and screens' render-to-texture passes (info, research, starmap panel; on SpaceWar the whole screen's build) |
| `scene` | the screen's frame build (starmap, colony, diplomacy) |
| `hud` | the HUD and screen overlays |
| `acquire`, `draw`, `present` | the swap chain image, encoding and submit, present |

Counters per frame: `ticks`, `passes` (renderer draws), `quads`, `batches`, `tex` / `tex_kb` (texture uploads), `mesh`
(mesh uploads), `target` (new render targets). A non-zero `tex`, `mesh` or `target` median on a screen that stays put is
a per-frame upload or leak.

`IG2_PROFILE_TOUR=-,F9,F4,...` walks the screens: every `IG2_PROFILE_STEP` seconds (default 4) the next key goes in as
if pressed (`F1`..`F10`, `Esc`, `Enter`, `Tab`, `Space`, a letter, `vsync`; `-` does nothing; any key skips a video),
and the window closes after the last. `IG2_PROFILE_BATTLE=<dump.json>` puts a SpaceWar capture's dump in battle slot 0
and opens the battle screen (as `--screen spacewar --battle --script` does headless).

## Numbers

Release, vsync off, 800 × 600 window, `quick-2.sav` (starmap and the screens), `ui5f.sav` + `ui5f-entry.json`
(SpaceWar), `grwar17-state.sav` (ground war, planet 17). Median of the 1-second lines, the first second on a screen
dropped. Measured with the same profiling build before and after the fixes, back to back, while other builds
were compiling (the absolute fps are noisy; the worst frame and the per-phase ms are the useful part).

| Screen | fps before | fps after | worst frame before | worst after | sim ms/frame before → after |
|---|---|---|---|---|---|
| Main menu | 790 | 831 | 2.6 ms | 2.4 ms | — |
| Starmap | 741 | 1267 | 16.8 ms | 2.6 ms | 0.21 → 0.02 |
| Colony | 1420 | 1794 | 20.7 ms | 2.7 ms | 0.17 → 0.01 |
| Ground war | 1270 | 1709 | 15.6 ms | 2.4 ms | 0.14 → 0.01 |
| SpaceWar | 281 | 439 | 7.4 ms | 3.6 ms | — |
| Info | 219 | 250 | 18.1 ms | 6.4 ms | 0.65 → 0.07 |
| Research | 1144 | 1402 | 16.5 ms | 3.5 ms | 0.14 → 0.01 |
| Design | 1482 | 1748 | 16.6 ms | 2.4 ms | 0.12 → 0.01 |
| Trade | 1248 | 1710 | 18.0 ms | 3.5 ms | 0.15 → 0.01 |
| Diplomacy | 634 | 1156 | 20.9 ms | 3.6 ms | 0.35 → 0.02 |
| Messages | 1972 | 2479 | 13.9 ms | 1.9 ms | 0.08 → 0.01 |

One strategic tick (`Game::run_tick`, mean / worst over 100 ticks, starmap): **7.0 / 13–17 ms before, 0.84 / 1.4–3 ms
after**. Before, a frame with a tick took 14–21 ms on every screen: a hitch 20 times a second at any refresh rate above
50 Hz.

Debug build (`target/debug`, starmap): **2.0 fps before** (sim 497 ms per frame), **105 fps after** (sim 2.7 ms, scene
4.7 ms; a debug tick is ~12 ms).

## What was slow and what changed

1. **`Memory::write` re-merged the whole run on every write** (`ig2-sim::save`, the sparse image of the exe's statics
   every system reads and writes through). A 1-byte write inside a 100 KB run removed the run, allocated a new one and
   copied it; reads and writes also walked every run below the address, and every `u8`/`i32` read allocated a `Vec`.
   `store_diplo_planets` alone (19 two-byte writes per planet) took 2.3 ms per call, three or more calls a tick. Now: the
   touched runs are found from the end of the range back (the runs don't overlap, so their ends rise with their starts),
   a write inside one run that no other run touches goes in place, and the fixed-size reads fill an array. The run
   structure after any write is the same as before (the merging path is unchanged; the test
   `memory_writes_in_place_keep_the_runs_of_the_merging_write` checks 12,000 random writes against the old algorithm),
   so saves, snapshots (ADR 0007) and `Memory`'s equality are unchanged. The HUD's reads got cheaper too (starmap
   `hud` 0.37 → 0.08 ms).
2. **SpaceWar made two new render targets every frame** (`spacewar_screen::fixed_texture`, the weapon column and the
   small info box): new GPU textures and bind groups that were never freed, so GPU memory grew for as long as a battle
   was shown. They are now two targets kept in `Gfx` and drawn into again (`Renderer::draw_into`), like every other
   screen's offscreen pass.

Not changed: the 500 ms catch-up clamp (the original's rule, re/rules/time-and-rng.md); the debug build stays unoptimised for
our crates (`[profile.dev.package."*"]` already optimises the dependencies).

## Left

- ~~Info screen~~ fixed (second commit): the list globes re-read and re-parsed their `.pvr` from disk every frame for
  `uv_mul` and the pass deep-copied its models; now cached (`FrameModels::globes_scaled`, `Rc<Model>`). Info 300 →
  ~2,700 fps (offscreen 3.04 → 0.20 ms); headless info (150, 400 UI frames) and messages snapshots byte-identical.
- SpaceWar: 2.2 ms a frame building the screen.
- Main menu: 20,480 quads a frame.
- The sim's per-tick decode/encode of whole systems (`with_diplomacy`, `starmap_pass`: `state.starmap()`,
  `state.colony()` …) is now the largest part of a tick (`starmap` 0.42 ms, `daily` 0.2 ms per tick).

## Checks

`cargo test --release --workspace -- --skip campaign_matches_exe`: 556 passed, 1 failed:
`save_replay::replay_matches_the_exe_tick_by_tick` (`replay-campaign-c1-x16` tick 2: 15 chunk groups,
`replay-campaign-opening-c1-x16` tick 11: 93). It fails the same way with `save.rs` as on `main` (checked 2026-10-24), so
it predates this change. Captures: colony 42 and research unchanged or better; the SpaceWar sequence's weapons region
better, its console region within 0.3 of the last stored run (another client build).
