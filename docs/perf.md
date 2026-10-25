# Client performance: how to profile

## How to run it fast

```bash
cd engine
cargo run --release -p ig2-client -- --save path/to/save.sav
```

Always `--release`. A debug build runs one strategic tick in several ms (the sim crates are unoptimised;
`[profile.dev.package."*"]` optimises only the dependencies), and the tick clock keeps the original's 500 ms catch-up
clamp (`ig2_sim::time::TickClock`, `re/rules/time-and-rng.md`): a slow tick makes every frame run the 10 ticks the
clamp lets through. `IG2_VSYNC=0` starts with vsync off (the classic client has no vsync key, re/deviations.md #159;
the tour's `vsync` step toggles it).

## Measuring: `IG2_PROFILE`

`IG2_PROFILE=1` makes the window log one line per second to stderr (`ig2-client::prof`): the screen, fps, mean and worst
frame time, the mean ms per frame of each phase, and per-frame counters.

| Phase | What |
|---|---|
| `sim` | the message pass and the due sim ticks (`World::pump`, `World::tick`); inside it `t_run` (`Game::run_tick`), `t_snap` (the starmap snapshot), `t_ui`, `t_colony` |
| `ui` | the UI clocks, panels, menus and screen steps of the frame |
| `offscreen` | the panels' and screens' render-to-texture passes (info, research, starmap panel; on SpaceWar the whole screen's build) |
| `scene` | the screen's frame build (starmap, colony, diplomacy) |
| `backdrop` | the live overlay background (ADR 0015) |
| `hud` | the HUD and screen overlays |
| `acquire`, `draw`, `present` | the swap chain image, encoding and submit, present |

Counters per frame: `ticks`, `passes` (renderer draws), `quads`, `batches`, `tex` / `tex_kb` (texture uploads), `mesh`
(mesh uploads), `target` (new render targets). A non-zero `tex`, `mesh` or `target` median on a screen that stays put is
a per-frame upload or leak.

`IG2_PROFILE_TOUR=-,F9,F4,...` walks the screens: every `IG2_PROFILE_STEP` seconds (default 4) the next key goes in as
if pressed (`F1`..`F10`, `Esc`, `Enter`, `Tab`, `Space`, a letter, `vsync`; `-` does nothing; any key skips a video),
and the window closes after the last. `IG2_PROFILE_BATTLE=<dump.json>` puts a SpaceWar capture's dump in battle slot 0
and opens the battle screen (as `--screen spacewar --battle --script` does headless).

## Method

Release, vsync off, a fixed window size (800 × 600) and fixed saves per screen (for example `quick-2.sav` for the
starmap and the overlay screens, a SpaceWar capture's dump for the battle, a ground-war save). Take the median of the
1-second lines and drop the first second on a screen. Compare before and after with the same profiling build, back to
back; absolute fps are noisy while other builds run, so the worst frame and the per-phase ms are the useful part. For
the sim alone, compare `t_run` (one `Game::run_tick`) mean and worst over 100 ticks: a tick that costs more than a
frame budget is a hitch 20 times a second (the 20 Hz tick).

## Things to keep cheap

- `ig2-sim::save::Memory` (the sparse image of the exe's statics every system reads and writes through): a write
  inside one run that no other run touches goes in place, and the fixed-size reads fill an array. The merging path
  must keep the same run structure (test `memory_writes_in_place_keep_the_runs_of_the_merging_write`), so saves,
  snapshots (ADR 0007) and `Memory`'s equality don't depend on the write path.
- Offscreen passes draw into render targets kept in `Gfx` (`Renderer::draw_into`), never new targets per frame.
- Models and their `.pvr` data are cached, not re-read per frame (`FrameModels::globes_scaled`, `Rc<Model>`).
- The depth buffer is cached per target size (`render3d::Pipe3d::depth`).
