# ADR 0088: the remaster ships as mods; a mod's `visuals.toml`

Date: 2026-11-10. Status: accepted. Builds on ADR 0044 and 0068 (mods), ADR 0054 (texture and model packs), ADR 0055
(modern lighting), ADR 0057 (battle effects), ADR 0064 (smooth glows) and ADR 0086 (the mod repository). Implemented
by `ig2-formats::visuals` (the file), `ig2-formats::vfs` (found in a mod), `ig2-client::visuals` (laid over the
settings), `ig2-client::starmap` (the planets' halo), `ig2-client::hdr` (the floating-point scene) and `ig2-client::lighting`
with `ig2-client::render3d` (the engine's light per pixel), `ig2-formats::effects` with `ig2-client::battle_fx` (effects
as data).

## Context
The game is to get a remastered look: better lights and effects, a richer starmap, sharper textures, smooth
animations in place of the frame-by-frame sprites, more detailed models; true to the original's shapes, colours and
timing. The classic look must stay exactly the original's, and the project ships no game files.

A mod holds data only (ADR 0086: no programs, no scripts), so a mod cannot bring a shader or an effect of its own.

## Decision
1. **Nothing is replaced in the game; the remaster is a set of mods** the player installs from the Mods page's
   Browse tab (ADR 0086) or by hand. Without them the game looks as before.
2. **The engine gains abilities, the mods switch them on.** Each new effect is engine code that does nothing by
   default. A mod turns it on and sets its values in `visuals.toml`; pictures and models come as a `hires/` pack
   (ADR 0054).
3. **`visuals.toml`** at a mod's top (not a game path):

   ```toml
   [lighting]
   anti_aliasing = true     # ADR 0055's four switches
   per_pixel = true         # §9: on every 3D screen
   soft_shadows = true
   bloom = true
   hdr = true               # §8: the scene in a floating-point picture, its glow, the tone curve
   bloom_threshold = 0.8    # where the glow and the tone curve start, 0..1
   bloom_strength = 0.6     # how much of the glow is added, 0..2
   battle_shadows = true    # §11: the battle's ships shadow one another and the planet
   blast_light = true       # §11: blasts and shots light the hulls

   [effects]
   battle = true            # ADR 0057
   smooth_glows = true      # ADR 0064

   [starmap]
   atmosphere = 0.6             # the planets' halo: strength 0..1
   atmosphere_size = 1.35       # its radius in planet radii, 1..3
   atmosphere_color = "#7fb2ff"
   star_layers = 2              # §12: more far stars from the original's generator, 1..4
   cloud_layers = 2             # §12: more galaxy clouds sharing their light, 1..4
   star_rays = 0.5              # §12: the systems' stars glow with rays, 0..1
   night_side = 0.6             # §12: the planets' unlit side darkened, 0..1
   cloud_shadows = 0.5          # §12: the clouds' shadows on the long info's planet, 0..1
   engine_trails = 0.6          # §12: a trail behind moving fleets, 0..1
   sky_detail = 0.7             # §13: finer wisps and dust over the sky's nebulae, 0..1
   cloud_detail = 0.7           # §13: soft structure inside the galaxy's clouds, 0..1
   ```

   - A switch set `true` is on while the mod is enabled, whatever the player's own setting; a switch left out (or
     `false`) leaves the player's setting alone. A mod never switches anything off.
   - With several mods a switch on in any is on; a value comes from the highest-priority mod that sets it.
   - An unknown table or key, or a value out of range, is an error: the file is skipped and named in the log, on
     the Mods page (the mod's problems) and by `openIG2-check` (also in `--package`).
   - The file is display only: it is never in the multiplayer fingerprint, the save or the lockstep, whatever the
     mod's `kind`. Players with and without it play together.
4. **The planets' halo** (the first new ability): on the starmap each drawn planet gets an additive glow under its
   mesh, full at the planet's limb and fading to `atmosphere_size` radii, in `atmosphere_color` × `atmosphere`;
   it fades with the planets behind a battle. Halos under 1.5 pixels are not drawn.
5. **The remaster's packages**, each a row of `docs/ENHANCEMENTS.md` (562–569), in this order:
   1. Look switches and effects in the engine (562–566): this file, HDR with tone mapping and bloom over the whole
      scene, per-pixel lights and shadows on every 3D screen, a richer starmap, effect definitions as data.
   2. Textures (567): a tool that makes a `hires/` pack from the player's own install on their machine.
   3. Sprite animations (568): the frame-by-frame effects as smooth particle effects, the rest interpolated.
   4. Models (569): more detailed models as `hires/` packs.
6. **What the index can carry.** A mod in the index holds no file copied or derived from the install (ADR 0086).
   Look files, newly made pictures, effects and models can be downloaded; a pack upscaled from the original's
   textures cannot, so the tool of 567 makes it on the player's machine.
7. **Fidelity rule for every remaster mod**: outlines, palette, timing and layout stay the original's; resolution,
   light and detail may change.
8. **The floating-point scene** (`hdr`, row 563; `ig2-client::hdr`, `Renderer::draw`):
   - **What the scene is.** A screen marks where its scene ends (`Frame::mark_scene`): the starmap (sky, stars,
     clouds, planets, routes, fleets), the space battle (ships, shots, blasts, particles) and the colony's 3D view.
     The panels, markers, names, text, the modern pages and the pointer come after the mark and are drawn as before.
   - **How the gamma-space scene maps into the picture.** The original draws 8-bit gamma-space values, and so do
     our scene passes. With `hdr` the same passes draw the same numbers into an `Rgba16Float` picture: a stored
     value `v` there is the value the 8-bit frame would hold, `v = byte / 255`, without the clamp at 1. Blends
     work on these numbers as before, so sums of additive glows can pass 1 instead of being cut off. The few
     linear-light quads inside a scene (route arrows, fallback markers) are sRGB-encoded by their shader and
     blended on the encoded values, as in the browser build (ADR 0047 §4). A screen whose 3D pass is the
     linear-light one is left as it is.
   - **Glow.** The picture at half size, each colour times `clamp((max(r, g, b) − threshold) / (1 − threshold), 0, 1)`,
     blurred twice with a 9-tap Gaussian both ways (the step grows with the frame's height over 1080), times
     `bloom_strength`, added to the picture. Inside the picture this glow takes the place of ADR 0055's bloom of the
     additive mesh parts.
   - **Tone curve**, per channel, onto the frame: `x` for `x ≤ t`, else `t + (1 − t)·(1 − e^((t − x)/(1 − t)))`
     with `t = bloom_threshold`. So a colour below the threshold that no glow reaches is the original's byte, and
     the palette stays (rule 7); above it, values that the 8-bit frame cut off at 1 keep their differences.
   - Defaults: threshold 0.8, strength 0.6. No exposure setting: it would shift every colour.
   - The browser build ignores `hdr` (WebGL2 may lack floating-point render targets). Anti-aliasing works inside
     the picture.
9. **Per-pixel lighting on every 3D screen** (`per_pixel`, row 564; `lighting::Lights::vertex`, `render3d`'s
   `set_light`). ADR 0055's switch lit only the space battle's ships per pixel. Everything else the engine lights
   per vertex (`fenyekszamolasa`, re/screens/research.md §2.1): the models of the research, design, spy, diplomacy,
   trade, info and messages screens and the main menu, the colony's panel models, the starmap's and the battle's
   planets and the starmap panels' models. (The colony's terrain and buildings already take their one light per
   pixel.) With the switch on these get the same light per pixel:
   - **What "per pixel" means for the table light.** The engine's light of a vertex is the sum of its node's
     ambient colours and, per parallel light, `colour · shade[float2int(|n − L|²·2047)] >> 8`, saturating at 255,
     with `n` the vertex's lighting normal and `shade` the table `sqrt(1 − i/2896)·255`. Per pixel the same sum is
     taken, bytes, rounding and saturation included, with `n` the lighting normal interpolated over the triangle
     and brought back to the length it has at the vertices. At a vertex this is the engine's colour exactly (a
     test compares the two over a sphere of normals), so the lit colours and the lights' tints stay the
     original's; between the vertices the shade follows the surface instead of being blended from the corners,
     which removes the facets and lets a highlight fall inside a large triangle.
   - **How.** A scene carries up to 16 light sets (the ambient sum and at most four parallel lights each); a
     vertex names its set by the length of its normal attribute. A node with more parallel lights, or the 17th
     set of a frame, stays lit per vertex. Wire lines, the cloud material and the video discs are as before.
   - The player's own "Per-pixel lighting" option (ADR 0055) switches the same thing on. Normal maps are not part
     of this (row 567 may bring them).
   - Shadows between the battle's ships and light from blasts and shots on the hulls: §11.
10. **Effects as data** (`effects.toml`, row 566; `ig2-formats::effects`, `ig2-client::battle_fx`). A mod's
    `effects.toml` (at its top, beside `visuals.toml`) describes ADR 0057's particle layers as emitters:

    ```toml
    [pictures]                                  # the mod's own pictures: a PNG in the mod, added or darkening
    fire = { file = "effects/fire.png", blend = "add" }

    [[blast]]                # one emitter; a moment may list several
    sprite = "fire"          # glow (flare.bmp), spark (the soft dot), ring, smoke, or a [pictures] name
    count = [10, 16]         # particles per start: a number or [least, most], 0..256
    life = [0.4, 1.0]        # seconds, 0.05..30
    size = [0.3, 1.5]        # half size at birth and at death, in the moment's radius, 0..20
    color = "#ffcc66"        # 1 if left out
    speed = [0.5, 3.0]       # radii a second in a random direction, 0..50
    drag = 0.05              # the share of the speed kept after a second, 0..1 (1)
    fall = 0                 # radii a second², down; rise = radii a second, up (the ground war only)
    offset = 0.2             # born this far out along its direction, in radii, 0..10
    ```

    - **The moments** are where and when ADR 0057's layers start (re/screens/spacewar.md §6, re/screens/colony.md
      §5), each with its radius `r`: `blast` (a ship dying, `urhajomeghalas` at 16 (`r` = scale × 1000) and 30/40
      (× 500); a planet gun, 40; a fighter group, 8; the ground war's `Flash`, its size × 2), `hit` (a non-beam hit
      on an unshielded ship, scale × 300; the ground war's `HitFlash`, size × 1.5), `shield` (a hit on a ship whose
      shields hold, scale × 300; the planet's shield, 30), `smoke` (each puff of a smoke column, every third 20 Hz
      tick, `r` the column's), `engine` (each thruster of `Hajtomu`, every battle tick, scale × 300) and `shot`
      (each projectile in flight, every battle tick, `r` its drawn size, the colour times the shot's own).
    - A moment a file sets replaces the engine's recipe for it; the others keep it. `engine` and `shot` have no
      recipe of the engine's (the beam halos stay). With several mods a moment comes from the highest-priority file
      that sets it, a picture name likewise.
    - Per particle: a random direction, then the speed and the life (each drawn only when a range), from the
      layers' own stream. Added pictures fade out linearly; darkening ones (and `smoke`) fade in over the first
      fifth, grow fast then slow and darken what lies below, and like the smoke they ignore the ground war's
      dimming. Colours are the gamma-space values the classic particles add (§8).
    - A mod with an `effects.toml` switches the Enhanced battle effects on (§3). The 4096-particle cap holds.
    - **On the graphics card**: the particles are camera-facing quads in the battle's 3D pass (ADR 0057 §4), the
      pictures textures; their step (move, drag, age) stays on the CPU, once a battle tick, where the cap keeps it
      cheap and the layers deterministic.
    - Errors as §3: an unknown moment or key, a value out of range or a sprite naming no picture skips the file;
      `openIG2-check` also names a picture missing from the mod. A picture that doesn't decode is drawn as the soft
      dot and named in the log.
    - Row 568 builds on this: the frame-by-frame animations become emitters at their own moments (new table names),
      and pictures may gain cells for animated sheets.
11. **Shadows and blast light in the space battle** (`battle_shadows`, `blast_light`, row 564 parts 2 and 3;
   `render3d::BattleLight`, `spacewar_screen::battle_light`). The original has neither: its ships and planet take
   the scene's ambient 0x40 and the parallel light 0x80 from (−1, −1, 1) (re/screens/spacewar.md §1.0) and nothing
   else. Both switches are off by default and come only from a mod; without them the battle's draws, pipelines
   and pixels are the classic ones.
   - **What casts and what receives.** The shown ships cast and receive; the planet receives only (it never stands
     between the light and a ship in the battle's layout, and as a caster its own night side would darken twice).
     Shots, blasts, smoke, rings, the grid and the wire lines neither cast nor receive.
   - **Shadows** (`battle_shadows`). A depth map (2048², 32-bit) seen along the parallel light, orthographic,
     fitted each frame round the shown ships' boxes. A receiver's point, moved 1.5 map texels along its normal,
     is tested against the map with a 4 × 4 grid of filtered comparisons (soft edges, about six texels wide). A
     shadowed point loses the parallel light's share of its light and keeps the ambient one: the light `a + s·d`
     (`a` ambient, `s` the parallel light, `d = max(n·L, 0)`) becomes `a + s·d·v`, `v` the lit fraction 0..1; a
     point lit per vertex is scaled by the same ratio `(a + s·d·v)/(a + s·d)`, with `n` its interpolated normal
     (per-pixel lighting) or else its face's normal turned towards the eye. Faces away from the light (`d = 0`)
     don't change, so nothing is darkened twice.
   - **Blast light** (`blast_light`). Up to 16 point lights a frame: the live particles of effect sets 0 and 1
     (`animated.bmp`, `anim2.bmp`: the explosions' fire; not the smoke, stars or trails) and the flying
     projectiles, each in its own sprite colour (`particle_color`) weighted by its size. Sources closer than a
     light's radius to a brighter one are merged into it (colour weights summed, the position weighted); the 16
     strongest are kept. A light's radius is 4× its source's size (at least 12 battle units), its colour the
     source's colour times `min(size/16, 2)`. A receiver adds `texel · Σ colour · max(n·l, 0) · (1 − (r/R)²)²`
     (`l` towards the light, `r` the distance, `R` the radius) on top of its light, in the same gamma-space
     values as the rest of the pass.
12. **A richer starmap** (`[starmap]` `star_layers`, `cloud_layers`, `star_rays`, `night_side`, `cloud_shadows`,
    `engine_trails`, row 565; `ig2-client::starmap`, `starmap_planet`; re/screens/starmap.md §6.6). Each adds to the
    original's drawing and leaves its places, colours and sizes alone; left out (1 layer, strength 0) it adds
    nothing.
    - **Layers.** Layer `k` (2..4) is the original's generator (`GenerateStarParticles`) run from `srand(k)`: its far
      stars (the shell pushed out `1 + 0.4·(k − 1)` times, for depth) twinkling out of step, and its two cloud sets,
      drifting as the original's. The fog clumps are not repeated. The clouds of all layers share the original
      clouds' light (each × 1/`cloud_layers`), so the sky gets finer, not brighter.
    - **Star rays.** Round each system star's own glow, faint additive procedural rays 1.6 times its size: a
      short cross about 1.5 pixels wide and fainter diagonals, left out over the sprite itself so its colour still
      tells the star type, in the sprite's hue (its texels' colour weighted by brightness, brightest channel 1) ×
      0.7 × `star_rays`.
    - **Night side.** The planet's mesh drawn again in black after it, each corner's alpha `night_side·(1 −
      smoothstep(−0.05, 0.45, n·s))` with `n` its outward direction and `s` the side the scene root's light falls on
      (`(−1, −1, 1)` in the world): the engine's lit side keeps its colours, the side it leaves to the ambient light
      goes dark. The shade table ends at cos 0.29 (0.41 at some corners), so the ramp starts inside that band.
    - **Cloud shadows.** On the long info's planet, before its clouds: the same cloud material in black at
      `cloud_shadows`, on the planet's sphere and turned 0.1 further, so each cloud's shadow shows beside it.
    - **Engine trails.** Under a fleet whose order moves it, from the sprite away from its target, five sprite
      heights long and 0.4 wide, bright at the ship and fading out, the owner's colour half mixed with white ×
      `engine_trails`.
    - All of it procedural: no texel or square point shows at any size (the remastered look's rule).
13. **A detailed starmap background** (`[starmap]` `sky_detail`, `cloud_detail`, row 565's follow-up;
    `ig2-client::starmap`, `render3d`'s `sky_detail`, `render`'s quad kind 23; re/screens/starmap.md §6.6). The
    original's sky and clouds are a few low-resolution textures magnified many times; these keys draw finer
    structure into them without moving or recolouring anything: every detail is a factor on the original's own
    colour at that pixel, so black stays black and each nebula keeps its hue and place. Left out (0) they add nothing.
    - **Sky detail.** On the sky sphere (`gomb.xvb`), per pixel, from the direction it is seen in (so the detail
      stays fixed to the sky as the camera turns and zooms): domain-warped value-noise wisps (5 octaves), soft
      filaments, and darker dust patches (down to 0.55), all only where the nebula glows. The factor runs from
      about 0.3 to 2 round a mean near 1, so the sky gets structure, not light.
    - **Cloud detail.** Each galaxy cloud and fog clump sprite (`felho2`, `felho3`) times its own domain-warped
      wisps in the quad, seeded by its set and index, so the clouds stay the same shape as they drift; the factor
      runs from 0.15 to about 2.9 round a mean near 1.
    - Both procedural and smooth at any zoom: no texel or square point (the remastered look's rule).

## Consequences
- The classic look and every capture comparison are untouched: with no mod enabled no code path changes a pixel.
- The remaster can grow without releases of the game once an ability exists, and players can make their own looks.
- Each new ability costs a `visuals.toml` key and its check; shaders stay in the engine.
- Rejected: shaders inside mods (code from a download, and three back ends to keep working); a built-in "Remaster"
  switch that replaces the classic drawing (the remaster is to be optional content, not the game's default).
