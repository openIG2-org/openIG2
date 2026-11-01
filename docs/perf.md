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
| `sim` | the message pass and the due sim ticks (`World::pump`, `World::tick`); inside it `t_run` (`Game::run_tick`), `t_snap` (the starmap snapshot), `t_ui`, `t_colony`, `reload` (the colony scene rebuilt after a building change), `autosave` |
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

The start-up logs each step's time since the process started (`install`, `rules`, `scenario globals`, `save`, `tables`,
`world`, `audio`, `app`, `event loop`, `gpu`, `assets`, `hud`, `spy scene`) and the first drawn frame (`first frame
(starmap) 722 ms after the start`): the load time. The play-through harness (`--playthrough`, docs/BUILDING.md) with
`IG2_PROFILE=1` logs a line per second over its ticks (`tick`: `t_run`, `t_snap`, ...) and every 10,000 ticks the
game date with the mean and worst `World::tick`.

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

## Measurements (part 1, 2026-11-01)

One machine (RTX 5080, 3840 × 2160 monitor), release, vsync off; 1920 × 1080 in a window, 3840 × 2160 borderless.
Starmap and main menu over `quick-2.sav`, the ground war over a `run_screen.py --grwar` state save of planet 17 (F3),
the space battle over a `run_spwc.py --grab-ticks` dump (`IG2_PROFILE_BATTLE`); medians of the 1-second lines over
16 s. The sim: `--playthrough skirmish:1:7 --seed 7` (eight empires, played to its end in 2357, 34,834 ticks).

| | before | after |
|---|---|---|
| Frame, main menu (1080 / 2160) | 1.0 / 1.2 ms (≈ 900 fps) | unchanged |
| Frame, starmap | 0.8 / 1.0 ms | unchanged |
| Frame, ground war | 1.1 / 1.1 ms, worst 16–25 ms at each building change | 1.0 / 1.1 ms, the change's frame ≈ 5 ms |
| Frame, space battle | 1.3 / 1.4 ms | unchanged |
| Colony scene rebuild after a building change (`reload`) | 24 ms | 4.3 ms |
| Sim tick late in the game (`World::tick` mean, 2353) | 1.19 ms | 1.00 ms |
| Load to the starmap (`--save`, first frame) | 2.1–2.4 s | 0.7 s |
| Load to the main menu (first frame, the title video) | 2.3–2.7 s | 0.75 s |

The frames are far inside a 240 Hz budget on this machine: GPU-light, CPU-bound in `scene` and `draw` (the ground
war's `scene` 0.5 ms, the battle's whole-screen build 1.3–2.0 ms at 2160). What a player felt were the hitches and
the start:

- **The colony scene rebuilt from disk** (`ColonyScene::load` on every building built, destroyed or finished: in a
  ground war at each building lost) read and uploaded every texture again (and kept the old ones on the GPU) and
  decoded every building's levels of detail again. `ColonyScene::reload` keeps the scene's textures and models.
- **The spy recount of every tick** (`MULTIGET_Spy`'s `HanyKemVan`, `Game::run_tick`) went through `with_spies`,
  which reads and writes back the colony, combat, research, relations, kidnap and random worlds for a count of the
  rosters: a sixth of a late tick. It now reads and writes the rosters only.
- **The install index** (`Install::open`) asked the file system for each of the install's 17,575 files whether it is
  a folder (640 ms), and the window opened the install a second time for the audio. It now reads the entry's own type
  (a link is still followed) and the audio takes a copy of the index.

Where a late tick goes (`Game::run_tick`, means per tick in the run above, after the fix): `starmap` 0.38 ms (the
whole starmap, colony and combat worlds read and written back by `starmap_pass`), `daily` 0.21 ms (the AI, once a
day), `diplomacy_tick` 0.08 ms, the rest under 0.06 ms. The start: `gpu` (wgpu instance, adapter, device) 355 ms,
`assets` 110 ms, `spy scene` 50 ms, the rest under 50 ms.

Next candidates: the state views (`state.colony()`, `combat()`, `starmap()`) read and written back whole by each
system per tick (`starmap_pass`, `with_ai`, `with_diplomacy`): a view that stays decoded between the systems, or
writes only what changed, would cut most of the tick; the autosave (12.7 ms on the frame thread at each save: write
it on a worker); the colony rebuild's last 4 ms (keep the meshes too, rebuild only the buildings' mesh); the GPU and
asset start-up (≈ 0.5 s: create the window and device while the rules load).

## Measurements: the modern lighting (ADR 0055, 2026-11-01)

Same machine, release, vsync off, a window of the size; `--modern-lighting` (all four rows on) against all off.
Starmap over `quick-2.sav`, the colony (F3) over a colony state save, the space battle over a `run_spwc.py --script`
entry dump (`IG2_PROFILE_BATTLE`); medians of the 1-second lines over 16 s.

| Mean frame | 1920 × 1080 off / on | 3840 × 2160 off / on |
|---|---|---|
| Starmap | 1.5 / 2.4 ms | 2.1 / 2.4 ms |
| Colony | 1.3 / 1.4 ms | 2.3 / 2.3 ms |
| Space battle | 3.4 / 3.1 ms | 3.8 / 2.9 ms |

The cost is the 4× pass and its resolve (the starmap's late planets add a second resolve and a full-screen lay-over);
the bloom's half-size blur and the 16-tap shadow read stay under the noise. The space battle's numbers vary more than
the difference. The targets are made once per frame size (`render3d::Pipe3d::targets`, six sizes): a frame that draws
more sizes would make them every frame (12 ms in the battle while the cache held three).
