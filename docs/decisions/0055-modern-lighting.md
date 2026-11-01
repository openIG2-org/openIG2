# ADR 0055: modern lighting options

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::render3d` (the mesh pass, `Lighting`),
`ig2-client::spacewar_screen` (the ships' and pieces' vertices) and `ig2-client::display` (four rows).

## Context
The 3D screens draw as the GOG remaster does (re/screens/render.md): no anti-aliasing (one sample per pixel), the
engine's per-vertex light (`fenyekszamolasa` ELF 0x606530, re/screens/research.md §2.1) or, on the colony, the
scene's parallel light per fragment on the vertex normals, the shadow map read by `ig2_fragment.shader`'s `BIT2`
with four taps a quarter texel apart (§4), and the engine's additive objects (property 8 off: `ONE`/`ONE`, §2.3)
added as they are. On a large window the triangle edges of buildings, ships and planets stair-step, the battle's
ships show their per-vertex light as flat facets, and the colony's shadows end in a hard one-texel edge.

## Decision
Four rows on the Options screen's Remake page, under a "Graphics" heading, each off by default and switchable alone. With all four
off the mesh pass runs the same pipelines and shader branches as before, so the captures and snapshots are unchanged.
`--modern-lighting` turns all four on for one run.

1. **Anti-aliasing** (`AntiAliasing`): the mesh pass draws into a 4× multisampled colour and depth target. A pass
   that clears (the scene's own) resolves straight into the frame; a pass that draws over what is there (the
   starmap's late planets, a scene over a backdrop) is cleared transparent, resolved into a scratch target and laid
   over the frame premultiplied (the gamma pass's glass blend already keeps that coverage, re/screens/info.md §6.6).
   The 2D pass is not touched: the pictures and text stay as sharp as before. Thin geometry is averaged with what
   lies behind it: the battle's grid lines, a pixel wide, come out fainter and smooth instead of stepped.
2. **Per-pixel lighting** (`PixelLighting`): the space battle's ships and flying pieces, lit per vertex by ambient
   0x40 and the planet's parallel light 0x80 (`0x40 + 0x80·max(n·L, 0)`, re/screens/spacewar.md), keep their normal in
   the vertex and get the same two lights per fragment (`SceneUniforms::flash[1][3]` = 1 picks the plain Lambert
   term in the engine path). The colony's terrain and buildings are already lit per fragment from their normals,
   and the starmap's planets are spheres whose per-vertex light is already smooth; the 3D-framed screens' models
   (research, info, trade, design) keep the exe's per-vertex light.
3. **Soft shadows** (`SoftShadows`): the shadow map is read with a 4 × 4 grid of bilinear taps two texels apart
   (percentage-closer filtering over about six texels) instead of the remaster's four close taps; the map, its
   casters and its darkness are the same. Only the colony draws a shadow map.
4. **Bloom** (`Bloom`): the additive translucent parts of a mesh pass (the colony's rain clouds, the ground war's
   blasts, fire and shots) are drawn again into a half-size target, blurred with a 9-tap Gaussian each way and added
   back at half strength. The additive lines (the battle's grid) and everything 2D are left out, the space battle's
   blasts and shots among them (2D sprites, `Blend::GammaAdditive`).

## Rejected
- **FXAA over the whole frame**: one pass and no extra pipelines, but it blurs the 2D pictures and the font as well,
  which the "Sharp UI"/"Sharp text" options (ADR 0043, 0046) work to keep crisp.
- **Per-pixel lights on every screen**: each screen builds its own lit vertices from up to 15 lights per node; a
  general per-draw light table touches every screen for models the player sees small inside frames. The battle's
  ships are where the facets show.
- **A depth-aware bloom**: the glow of a blast behind a ship bleeds over the ship's edge by a few pixels, which reads
  as glare; a depth copy per pass was not worth it.

## Known limits
- A blob shadow (the colony's units, `Shadow::Blob`) in a pass that draws over the frame darkens only the scratch
  target with anti-aliasing on; the colony's own scene clears, so its blob shadows are unaffected.
- 4× targets cost memory: about 260 MB at 3840 × 2160 (colour and depth); frame times are in `docs/perf.md`.
- The targets are kept per frame size, six sizes at most (the battle draws five a frame); more sizes in one frame
  would make them again every frame.
