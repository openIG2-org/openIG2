# ADR 0006: Game renderer and client architecture

Date: 2026-10-16. Status: **accepted as proposed** (first slice implemented: `ig2-client`, the starmap of an
original save). Builds on ADR 0001 (assets from the user's install), 0003 (20 Hz sim, rendering decoupled),
0004 (rules from the user's exe) and 0005 (lockstep multiplayer).

## Context
Phase 2 gives us `ig2-sim`: an original save loads into `SaveState`, and `Game::advance_clock` + `Game::run_tick`
run whole strategic ticks byte-exact against the exe. Phase 3 needs a client that shows it and takes input.
The project's goals: multiplayer first, high refresh rate, high DPI, 4K and ultrawide, better performance than
the GOG build.

How the original draws (`re/00-overview.md` §3): a per-frame `IG2_gameUpdate` runs a screen state machine
(`Starmap`, `Colony`, `Research`, `Design`, … each with `_Belep` / `_Main` / `_Kilep`), calls the sim tick
`Fake_FRISSITES` from the frame (20 Hz, catch-up ≤ 500 ms), and renders through the remaster's OpenGL layer:
3D `.xv` scenes for the starmap, colony and battles, and 640×480-era 2D screens of "brushes" (sprites cut
from PVR atlases) scaled up. The remaster's starmap textures are in `dataraw/textures/starmap/`: `p1..p9`
are equirectangular planet maps (one per surface type), `star`/`n1..n8` star flares, `select` the
selection glow, `flotta` the fleet model's texture; `textures/main/galaxy_*` are the galaxy backdrops.

The asset viewer (`ig2-viewer`) already uses wgpu 25 through eframe 0.32 (egui), with a headless
`--snapshot` → PNG mode that lets tests and contributors check renders without a display.

## Decision

### 1. Graphics API and windowing: wgpu 25 + winit 0.30, our own frame loop
- **wgpu** (the version the viewer pins; one wgpu in the workspace). Backends: D3D12/Vulkan on Windows,
  Vulkan on Linux, Metal on macOS; GL is not targeted.
- **winit** directly, not eframe: the game owns its frame loop, present mode and surface configuration.
  eframe stays the viewer's (tooling) framework.
- **One render path for window and snapshot.** The renderer draws a frame into any `TextureView`. The window
  passes the swap-chain image; `--snapshot` passes an offscreen texture and reads it back to PNG. Every
  visual change is checkable headlessly (the main way of checking renders in tests).

### 2. Frame pacing: high refresh rate, sim at 20 Hz, interpolation
- The sim keeps the original's **fixed 50 ms tick** and catch-up rules (`ig2_sim::time::TickClock`: a tick
  fires while `now − last > 50 ms`, backlog clamped to 500 ms). Game speed (×1/×4/×16) changes the `Ido`
  added per tick (`IdoSpeedSzorzo`), never the tick rate, exactly as in the original.
- The renderer runs at the display's rate (`PresentMode::Fifo` = vsync at any refresh rate, `Mailbox` when
  the user turns vsync off; frame cap optional). Each frame draws the state **between the last two ticks**
  at `alpha = time since the last tick / 50 ms` (`TickClock::phase`): one tick of latency (50 ms), smooth
  motion at 60–360 Hz.
- What interpolates: positions that move continuously (fleets, later battle units and projectiles,
  camera). Discrete state (owners, counts, visibility) shows the newest tick. A fleet pairs with the
  same slot in the previous tick; a new fleet, a reused slot or a jump (wormhole) snaps instead of gliding.
- The UI tick (the original's 100 Hz `Mouse_Lekerdez`) becomes event-driven input; input becomes sim
  commands at tick boundaries (ADR 0003 §2), so single player and multiplayer take the same path.

### 3. How the client reads the sim: snapshots, never live references
- The client owns a `Game` (single player) or a lockstep peer that owns it (multiplayer, ADR 0005). After
  each tick it extracts a **snapshot** per screen (`Snapshot::read(&SaveState)` for the starmap: planets,
  fleets, targets, the date) through the sim's decoders (`SaveState::starmap()`, `colony()`, …). The
  renderer only sees the previous and current snapshots.
- The client never writes sim state directly. Player actions are **commands** applied at tick boundaries
  through the sim's entry points (today `Game::buy`, `Game::fleet_emptied`, `Diplomacy::receive_message`; a
  `Command` enum when the lockstep layer lands), so a recorded command log replays the game and lockstep
  peers stay identical.
- Snapshots decouple threads later: the sim can move to its own thread and hand over snapshots through a
  channel without touching the renderer. Single-threaded for now (a tick costs well under a millisecond).
- Globals a save doesn't carry that the scenario normally sets (`BirodalomMIzik`, `BirodalomTamadhat`) come
  from the user's exe image (ADR 0004). Loading a scenario (`.scn`) instead of a save is a later slice.

### 4. Display: high DPI, 4K, ultrawide
- Everything is drawn in **logical pixels × the window's scale factor** (winit), at native resolution: no
  fixed 640×480 framebuffer upscaled. Line widths, markers and UI sizes are logical; world sizes follow the
  camera zoom.
- **Aspect**: 3D/map views fill any aspect. The vertical field of view is fixed, so ultrawide shows more
  of the galaxy sideways instead of stretching or cropping. 2D screens keep their 4:3 layout centred, scaled
  to the height, with the margins filled by the screen's background (pillarbox), unless a screen has an
  enhanced wide layout.
- **Sharpness at 4K**: the original's assets are low resolution. Planets are drawn as analytic spheres in a
  fragment shader sampling the equirectangular maps (sharp edges at any size, rotation by scrolling the
  longitude); textures get mipmaps and linear filtering; sprites that the original colour-keyed on black get
  alpha at load time. Asset upscaling is a separate *enhanced* option, never on by default.
- sRGB swap chain; colours from the assets are treated as sRGB.

### 5. Assets
- Loaded at startup or on screen entry from the user's install through `ig2-formats` (`Install::read`, PVR
  decode, `.xv` scenes), never cached to disk, never shipped (ADR 0001). A missing or unreadable asset falls
  back to a plain shape so a partial install still runs.
- Textures are uploaded once per screen and kept in a cache keyed by game path.

### 6. Renderer structure
- A small **batched 2D/2.5D renderer**: one instanced pipeline for quads with per-instance kind (textured
  sprite, lit sphere, ring, marker, line segment), batched per texture, alpha-blended in submission order.
  Most of the strategic game (starmap, colony overview, info screens) is this.
- A **3D mesh pipeline** for `.xv` scenes (colony buildings and terrain, battle ships, the starmap's
  3D models) reuses the viewer's `.xv` loading (`ig2-viewer/src/model_view.rs` moves into a shared renderer
  module when the colony screen lands), with depth, the original's node animation keys and materials.
- Video (MP4 building animations, cutscenes) is a later ADR (ADR 0001 consequence).

### 7. UI: retained screens in the original's layout, immediate-mode for tools
- The game's screens are **retained**: one Rust type per original screen (`Starmap`, `Colony`, `Research`,
  `Design`, `Diplomacy`, `Spy`, `Trade`, `Info`, `Messages`, …) mirroring the original's
  `_Belep`/`_Main`/`_Kilep` lifecycle (enter, per-frame update and draw, exit) and its screen state
  machine. Each screen lays out its widgets from the original's coordinates and brushes (PVR atlas rects),
  scaled per §4. Hit-testing, hover and focus live in the screen, not in a generic toolkit, because the
  original's screens are fixed bitmaps with hot spots, not flowing layouts.
- Text uses the remaster's fonts from the install (`Jura`, `Inconsolata`, FreeType-rendered there; for us a
  pure-Rust rasteriser such as `fontdue` or `ab_glyph` into a glyph atlas) and the original's string tables.
- **egui** (immediate mode) is for developer overlays only (sim inspector, network stats, debug toggles),
  drawn over the game through `egui-wgpu` when enabled. It is never the game's own UI.

### 8. Crates
- `ig2-client` (binary): window, frame loop, screens, the sim driver, snapshot mode. The renderer lives in it
  for now (`render.rs`); it splits into an `ig2-render` library when the viewer or the battle screens share it.

## Consequences
- Rendering never affects the sim: lockstep determinism (ADR 0003/0005) holds whatever the frame rate.
- One tick (50 ms) of display latency for moving objects; the cursor, camera and hover react per frame.
- Snapshots copy a few KB per tick; negligible, and they make a threaded sim straightforward.
- The headless snapshot is the regression tool for visuals: tests render a save at chosen ticks and
  compare PNGs.
- Open: the exact look of the original starmap (its 3D camera, `StarmapRotate/Pitch/Zoom` 0xB9A968..,
  system stars, fleet models, nebulae) is not reverse-engineered yet; the first slice is a top-down view
  using the original's textures. A spec of `Starmap_Main`'s drawing (`re/rules/` or `re/screens/`) comes
  before the full screen.
