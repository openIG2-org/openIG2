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
- Install pictures drawn as they are go through `Renderer::file_texture` (one upload per path a run), not
  `Image::load` + `add_texture`.

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
- **Textures were never freed** (video memory ran out after many colony visits and space battles, worse with texture
  upscaling). Install pictures are uploaded once per run by path (`Renderer::file_texture`), there is one shared white
  texture, and a dropped render target hands its slot back so a reopened screen reuses it
  (`render::tests::reopened_screens_reuse_their_textures`).
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

Battle effects (ADR 0057, 2026-11-01, same machine and method): the space battle of the `fx8` capture (two fleets
fighting, ships exploding; `IG2_PROFILE_BATTLE`, unpaused by the tour's `Space`), 1920 × 1080, 21 one-second lines
each: Classic 1.23 ms median (worst frame 3.6 ms median), Enhanced 1.24 ms (3.4 ms); the layers' particles (at most
4096 a battle) cost nothing measurable next to the screen's build.

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

## Measurements (part 2, 2026-11-01)

Same machine and method as part 1, after the modern lighting, battle effects, texture upscaling, sharp text and UI,
themes and the enhanced computer empires landed. "Off" is the settings file's defaults, "on" is
`--modern-lighting --battle-effects --sharp-text --sharp-ui --upscale 4`; a saved settings file with a
row on counts too (`LOCALAPPDATA` pointed at an empty folder for the "off" runs). The battle is the `fx8` capture's
entry dump.

| Mean frame (worst-frame median) | 1920 × 1080 off / on | 3840 × 2160 off / on |
|---|---|---|
| Main menu | 2.2 / 2.5 ms (4.5 / 4.5) | 2.4 / 1.8 ms (5.3 / 4.3) |
| Starmap | 1.6 / 1.8 ms (4.9 / 5.0) | 1.5 / 1.4 ms (5.3 / 4.4) |
| Ground war | 1.6 / 1.8 ms (5.0 / 7.4) | 1.8 / 1.9 ms (6.0 / 6.9) |
| Space battle | 1.7 / 1.2 ms (5.2 / 3.2) | 1.5 / 1.6 ms (4.2 / 4.0) |

No screen's frame is a hot spot with or without the options (the main menu's 20,460 quads a frame included). What a
player felt was the start with the texture upscaling on, and the sim tick:

| | before | after |
|---|---|---|
| Load to the first frame, options on | 5.6–7.1 s (`assets` 3.7–4.8 s) | 2.5–3.9 s (`assets` 1.4–1.7 s) |
| Load to the first frame, options off | 0.9–1.4 s | unchanged |
| `starmap_ground_marks` per tick (enhanced AI run) | 0.15 ms | 0.09 ms |
| Sim tick, `--playthrough skirmish:1:7 --seed 7`, best of 3, at 10,000 / 40,000 ticks | 0.93 / 1.08 ms | 0.87 / 1.03 ms |

- **The upscaling filters** (`upscale::xbrz`, `upscale::cubic`) ran on one core on the frame thread for every texture
  a screen loads. They now fill bands of rows on every core (`par_rows`): the cubic filter with the same arithmetic per
  texel, xBRZ with 8 rows of context round each band; test `banded_filters_match_the_whole_picture` keeps them equal
  to the whole picture's. The rest of `assets` is the upload and the box-filtered levels of the 16× larger pictures.
- **The starmap's ground-war marks** (`Game::starmap_ground_marks`, each tick) decoded and stored the whole combat
  view twice to clear and restore one flag bit per fleet; they now touch the fleets' flag words only. The rest of its
  cost is `with_wars` over every planet.

Where an enhanced-AI tick goes (means per tick over the run, before the second fix): `starmap` 0.52 ms (the
ground-war marks 0.15, the starmap, colony and combat views loaded 0.13 and stored 0.05, production 0.08, the fleet
movement itself 0.006), `daily` 0.31 ms (the AI 0.14: the enhanced empires add 0.02 ms over the classic ones),
`diplomacy_tick` 0.11, `grwar` 0.11, the rest under 0.04. The autosave's 12.7 ms runs twice a game year at the default
cadence: left on the frame thread.

Next candidates: `with_wars` over every planet for the battle status (skip planets without a battlefield); the
state views kept decoded across a tick's systems; the hires textures' levels made on the GPU or on a worker.

## Measurements (part 3, the sim tick, 2026-11-10)

One machine, release; `--playthrough skirmish:1:7 --seed 7` (34,834 ticks to 2357), `IG2_PROFILE=1`'s `World::tick`
mean per 10,000 ticks, the old and new builds run in turn, best of 3. The state hash
(`ig2_net::replay::sp_hash`) at 10,000, 20,000 and 30,000 ticks is the same before and after: the change is
behaviour-neutral.

| `World::tick` mean at 10,000 / 20,000 / 30,000 ticks | before | after |
|---|---|---|
| Sim tick | 1.63 / 1.61 / 1.24 ms | 0.95 / 1.03 / 1.03 ms |

Where a late tick went before (per tick, ticks 20,000–30,000 of an instrumented build; the fleet movement itself,
`Starmap::tick_until_battle`, was 0.007 ms): `starmap` 0.62 ms (the colony world decoded and stored for the arrival
hooks 0.19, the ground marks 0.16, production 0.12), `daily` 0.41 ms (the AI 0.17, of which its own work 0.02 and
the views' decode and store the rest), `diplomacy_tick` 0.16 ms (the planets' visibility tables written back 0.08),
`grwar` 0.11 ms, the rest under 0.07. The fixes:

- **Bulk reads of the memory image** (`Memory::i32s`, `i32_vec`, `f32s`, the AI's tables): a lookup of the runs per
  value made a decode of the colony world about 3,400 lookups (`per_planet`, the spy centres per planet). They now
  read each table once (test `memory_bulk_reads_match_the_values_read_one_by_one`); the AI's tables and the
  diplomacy's visibility tables are written back in one write per row.
- **The starmap pass decodes the colony world only for an arrival that needs it** (`ColonyArrivals::world` is
  loaded on the first colonisation, terraforming or trader payment, and stored only then).
- **The ground marks** (`GroundWarSetBattleStatus` over every planet) skip the war's setup for an unowned planet and
  for one whose cached status is a settled side (`Field::cached_status`, test
  `cached_status_is_what_status_returns`): they only clear its war flag.
- **`FRISSIT_Grwar`'s self-destruct check** read every planet's battlefield as a copy; it reads it in place
  (`SaveState::ground_field_with`).
- Production's view moves the combat view's ships, vehicles and designs instead of copying them.

Checked and left alone: the space battle's step (`Battle::step` over the 73,998 traced SpwC ticks: 0.031 ms mean)
and its candidates (`live_guns`, the scenery list, the weapon clones, the enemy list, `fighter_pick_ship`,
`tick_rules`): the whole step is a small part of a frame; `with_battle`'s state round-trip around it costs about what
the views above cost and now shares their faster reads. The spy recount (0.02 ms), `market_context` (the whole trade
pass 0.015 ms), the `starmap_context`s (0.007–0.01 ms), the radar and the colony-ship heading (inside the 0.007 ms
movement and the 0.02 ms AI work) are below the noise.

Next candidates: the views still decoded and stored whole by each system (production 0.06, the AI 0.17 per day,
the diplomacy 0.05 per tick): a view kept decoded across a tick's systems.

## Measurements (part 4, memory and the start, 2026-11-13)

One machine, release, vsync off, the saved settings with texture upscaling x4 on, `--save quick-2.sav`,
`IG2_PROFILE_TOUR=-,F3,-,F1,-` with a 3 s step, memory read 11 s after the start on the colony screen; warm runs.

| | before | after |
|---|---|---|
| Video memory (dedicated, the process) | 2884 MB | 1953 MB |
| Memory, private / working set | 4162 / 1506 MB | 2564 / 809 MB |
| Start-up `assets` step | 1585 ms | 840 ms |
| Start-up `hud` step | 281 ms | 165 ms |
| Frame (starmap / colony / help) | 1.8 / 1.4 / 0.9 ms | unchanged |

- **Install pictures uploaded more than once**: the starmap's planet sprites and the planet meshes each uploaded
  their own copy of `p1..p9` (89 MB each at x4), the battle screen loaded its planet meshes and their textures again
  at every battle (never freed), the starmap console uploaded `starpan` once per mesh part, Options and Help uploaded
  the laser glow at each opening, and the screens' own texture maps (kit, colony UI, spy, research, diplomacy, main
  menu) each uploaded their copy. They now go through `Renderer::file_texture` (one upload per path a run);
  `Planets::load_battle` loads once.
- **The levels of an upscaled picture** (`render::mip_chain`): the top level was copied (64 MB for a 4096 x 4096
  picture) and the box filter ran on one core. The levels are borrowed and `halve` fills row bands on every core
  (`upscale::par_rows`; test `halved_levels_average_the_four_texels_clamped`).
- **The upscaled pictures' cache** (`upscale::CACHE_LIMIT`) held up to 1 GiB of pixels already on the GPU; now
  256 MiB (ADR 0054 section 4). The design, trade and diplomacy scenes kept each texture's levels and upscaled copy
  beside the pixels they sample; they keep the pixels only.
- **PVR decode** (`pvr::decode_generic`, nearly every texture is RGB565 or RGBA4444): a table per channel instead of
  a 64-bit division per value.
- **The modern pointer set** (183 distance-field pictures at start) is drawn on every core.
- **Present queue**: `desired_maximum_frame_latency` 1 instead of 2 (the pointer is drawn in the frame, so each
  queued frame was pointer lag with vsync on).
- **A paused colony screen** decoded the whole colony world again on every 10 ms UI tick; now only when the
  dropships stepped (`Game::colony_ui_tick` returns it).

Next candidates: what is left of the video memory with x4 on is the nine 4096 x 4096 planet maps (about 800 MB;
stored BC1 in the install, decoded to RGBA8: uploading them compressed, or capping the planets' upscaling, would cut
it), the window-sized offscreen targets kept per screen, the leader heads' 100 textures per race, input sampled
before the swap-chain acquire, no pacing for a minimised window, the colony entry's ~190 ms frame.
