# ADR 0054: texture upscaling and high-resolution packs

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::upscale` (the filters, the pack lookup, the cache),
`ig2-client::assets` (`Image::hires`), `ig2-client::render` (the upload's mip chain), `ig2-client::scene` (pack
models) and `ig2-client::display` (the "Texture upscaling" row).

## Context
The install's textures (re/formats/pvr.md) are 64 to 1024 texels a side, made for 800 × 600; at 1440p and 4K every
picture and model texture is magnified two to five times and blurs. The project ships no graphics (ADR 0001, the
2026-11-01 decision in `docs/ENHANCEMENTS.md`), so better textures can only come from the player's own install, by a
filter at load, or from packs players make and drop into `mods/` (ADR 0044).

Every install texture the client draws goes through `Image::load` (decode → RGBA8 + the PVR's own levels) and
`Renderer::add_texture` (upload with a mip chain). Sprites address textures with normalised UVs (`Instance::uv`) and
meshes with the model's UVs times the PVR's `uv_mul`, and the shaders read the texel size from the bound texture
(`textureDimensions`), so a bound texture of another size draws in the same place. What must stay the original's is
the image's own size and pixels: screens lay out with `Image::width/height` (the pointer, the help pictures, the
info globes), and some read pixels (hit masks, the charset).

## Decision
1. **A "Texture upscaling" row** (`TextureUpscale`, Display group; values Off / 2x / 4x; `--upscale 2|4` for one
   run). Off by default: nothing is filtered and the 800 × 600 captures and snapshots are the exe's. A change applies
   to the textures a screen loads from then on (opening a screen again, or the next start, picks it up).
2. **The image keeps its size**: `Image` gains `hires`, the pixels the renderer uploads instead of `rgba` (another
   size, any aspect); `width`, `height`, `rgba` and `mips` stay the file's, so layout and pixel reads don't change.
   The renderer uploads `hires` as level 0 and box-filters it down; an upscaled level reaches the image's own size
   after one or two halvings and that level is the original's pixels, followed by the PVR's own levels (the
   alpha-tested scaffolding keeps its stored levels, re/formats/pvr.md). A pack's picture is box-filtered all the
   way down. A `hires` larger than the device's limit is dropped (the original is uploaded).
3. **Two filters, chosen by the picture**: flat-colour art (at least 40 % of horizontally adjacent opaque texels
   exactly equal: interface pictures, icons, masks) is scaled with xBRZ (the `xbrz-rs` crate, GPL-3.0-only, which
   AGPL-3.0 §13 allows combining with), which keeps edges and turns staircases into lines; everything else (the
   rendered pictures, model and planet textures: a few equal neighbours) is scaled Catmull-Rom on premultiplied
   colour, which is smooth and never brings a transparent texel's colour into the edge. The factor drops from 4 to 2,
   then to none, so that no side exceeds 4096 texels. The font atlas is never filtered (`Image::load_original`, ADR
   0046 sharpens it in the shader).
4. **Cache**: the filtered pictures are kept in memory per (path, factor) and shared between screens; the cache is
   emptied when it passes 1 GiB. Nothing is written to disk: the filter runs on load only for pictures a screen
   draws, and the cache never reaches the repository or the install.
5. **Texture packs**: an enabled mod's `hires/` folder holds pictures by game path, at any size:
   `mods/<name>/hires/dataraw/textures/colony/foo_bmp.png` (or the same name with `.pvr`) replaces how
   `dataraw/textures/colony/foo_bmp.pvr` is drawn. The original still decides the size, the `uv_mul` and the pixels
   read, so a pack picture of any resolution or aspect draws in the original's place. A pack picture wins over the
   filter. A plain ADR 0044 override (`mods/<name>/dataraw/…`) still replaces the file itself, size included.
6. **Model packs**: `mods/<name>/hires/dataraw/vector/colony/<stem>.xv` is drawn instead of the install's
   `<stem>.xv` (buildings, and any other model loaded by name), its root's half-extents scaled uniformly so that the
   largest equals the original's: `AddToScene` already sizes a model by its own box, and `AddToSceneGenesis`
   (absolute half-extents) then gets the original's size too, so a model built at any scale fits the original's
   place. Its textures are looked up by name like the original's, so it can bring its own (`hires/` for higher
   resolution versions of the install's, or new names in the mod's `dataraw/textures/…`).
7. **Multiplayer**: `hires/` files only change what is drawn, so they stay out of the room's mods fingerprint (ADR
   0044 §4): players with and without a pack can play together.

## Rejected
- **A disk cache**: the filters take about 0.35 s per million texels at 4× on one core (the starmap's set, the
  largest, about 4 s the first time it opens, then nothing: the memory cache; since 2026-11-01 they run on every core,
  about 1.5 s, `docs/perf.md` part 2); a cache folder would hold data derived from the
  install outside of it, to be kept in sync with the install and the mods.
- **One filter for everything**: xBRZ on the rendered pictures and model textures (no flat areas) posterises them;
  a smooth filter on interface art blurs its edges like the GPU's bilinear magnification already does.
- **Pack pictures replacing the file**: a different size would move every picture laid out by its size.
