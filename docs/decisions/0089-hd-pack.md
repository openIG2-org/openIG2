# ADR 0089: the HD pack: every picture, video, talking head, the font and the models' detail, made from the player's install

Date: 2026-11-12. Status: accepted; §1–3 built (row 570), §4 but `--smooth` (row 571), §5 (row 572). Builds on ADR 0054 (texture and model packs), ADR 0088 (the remaster as mods;
§5–6: the pack tool of row 567) and ADR 0013 (video). Rows 570–573 of `docs/ENHANCEMENTS.md`.

## Context
Row 567's `openIG2 --make-pack` (`ig2-client::pack_tool`) upscales the install's `.pvr` textures on the player's
machine with Real-ESRGAN's compact model (`ig2-client::learned`, on the graphics card through wgpu) and ADR 0054's
filters. The rest of what a player sees is still at its 1999–2002 size:
- the videos: H.264 High at 25 fps, 640×400 (cut-scenes, story videos, intro, endings), 332×188 (the messages
  screen's clips), `tutorial.mp4` 512×384 (`re/screens/video.md`);
- the talking heads: 14 SpidyAni `.ani` files in `anim/fejek/`, 128×128, palettised, 100 frames each
  (`re/formats/ani.md`);
- the font atlas (`dataraw/screen/charset_bmp.pvr`), which the pack tool skips;
- the models: low-polygon meshes with no surface detail beyond their texture.

The goal: a sharper, more modern look that still reads as the original: same outlines, colours, faces, timing.

## Decision
1. **One tool, one pack.** `--make-pack` grows to cover everything below; `--only <stage>` (`pictures`, `videos`,
   `heads`, `font`, `maps`) runs one part. The result stays the mod folder `texture-pack` with a `hires/` tree
   (ADR 0054 §5): never shared, never in the mod index (ADR 0088 §6). The tool skips files already made unless
   `--force`, so a stopped run continues.
2. **The fidelity lock, on every stage.** A learned upscaler may invent detail and drift colours. Each result is
   checked against its source: box-downscaled by the factor it must match the original within ±2 per channel. Where
   it does not, the low-frequency difference is added back (iterative back-projection, at most 3 rounds). Alpha is
   never run through a learned model: it takes ADR 0054's edge-keeping filter. No diffusion or generative model is
   used anywhere (they change faces and shapes).
3. **Pictures (row 570).** Every `.pvr` but the font atlas:
   - scene textures and the interface's pre-rendered art through Real-ESRGAN x4plus (RRDBNet, BSD-3, about 64 MB) as
     the quality model, the compact model staying the fast choice (`--fast`); the RRDBNet layers join
     `ig2-client::learned`'s wgpu kernels;
   - flat-colour art (at most 32 colours) through xBRZ, as ADR 0054 §3 decides at load time;
   - textures the models wrap (u or v outside 0..1 in any mesh using them) are padded by wrapping before the
     upscale, so no seam appears;
   - big pictures in overlapping tiles (32 texels), blended over the overlap.
   The release bundles the x4plus weights beside the compact model, both pinned by SHA-256, with their BSD-3 licence.
4. **Videos and talking heads (row 571).**
   - Videos: each frame upscaled ×2 by default (`--factor 4` allowed) with the compact video model, the fidelity lock
     applied, encoded H.264 with openh264 (already a dependency) into `hires/<game path>.mp4`; the same frame count,
     25 fps and audio track (copied, not re-encoded). `ig2-formats::mp4` gains a writer for that one layout.
   - Optional smoothing (`--smooth`): RIFE (MIT) frame interpolation to 50 fps; the original frames stay at their
     original times, only frames in between are added. The player (`ig2-client::video`) takes the frame rate from
     the file, so the length and the sound sync stay the original's.
   - Talking heads: every frame ×4 (128 → 512), the fidelity lock on the palette's colours, written as
     `hires/anim/fejek/<name>/<frame>.png`; with `--smooth` one in-between frame each, drawn at the original frame's
     time + half. The client reads the pack's frames in place of the decoded `.ani` frames when present.
5. **The font (row 572).** The tool cuts the original glyphs from the atlas, upscales each ×8 with the edge-keeping
   filter, thresholds it and writes a signed-distance-field atlas (`hires/dataraw/screen/charset_bmp.sdf.png`) with
   the original's cells. When a mod's `visuals.toml` sets `[text] sdf_font = true` and the atlas is present, the
   client draws text from it: the original's advances, baselines and colours, sharp at any window size and UI scale.
   Without it the classic font draws as before (ADR 0088 §1).
6. **The models' detail (row 573)**, engine abilities a mod's `visuals.toml` `[models]` switches on:
   - `smooth = 0..2`: subdivision levels applied when a mesh loads (Loop on the triangles); edges sharper than
     40° stay hard, so outlines and panel lines keep their shape;
   - `normal_maps = true`: the tool writes `hires/<texture>_n.png` from each upscaled scene texture (height from
     luminance, Sobel, strength per material kind); the per-pixel lighting (ADR 0088 §9) reads it with tangents from
     the mesh's UVs. A texture without a map lights as before.
   Hand-made models stay row 569.
7. **Order.** 570 first (its tiling, lock and RRDBNet kernels are reused by the others), then 571, 572 and 573 in
   any order.
8. **Disk and time.** The tool says how much space a stage needs before it starts and refuses when the disk lacks
   it. Videos at ×2 are the largest part (a few GB); ×4 is opt-in.

## Consequences
- The classic look is untouched: nothing is read from the pack unless the player made it and its mod is enabled.
- A full pack takes hours on a mid-range graphics card; every stage can run alone and resumes.
- The release grows by the bundled weights (x4plus about 64 MB, RIFE about 10 MB).
- Rejected: upscaling videos while they play (the compact model at 640×400 costs about 10¹¹ multiply-adds a frame,
  too much for most graphics cards at 25 fps); diffusion upscalers and generated 3D models (they do not keep the
  original's faces and shapes); community upscaler models under non-commercial licences (they don't fit
  AGPL-3.0-only).

## Implementation
**Pictures (row 570)**, `ig2-client::pack_tool`, `ig2-client::learned` and `learned::rrdb`:
- `--only pictures` (the other stages answer "not made by this version yet"); `--prefix <game path prefix>` makes
  part of a stage; a stage sums its pictures' sizes (4× RGBA halved, about what the PNGs take) against the disk's
  free space first and refuses when it lacks it.
- x4plus is read from `RealESRGAN_x4plus.pth` (v0.1.0 release, SHA-256 `4fa0d389…82f1`): a PyTorch zip whose
  `data.pkl` (`params_ema`) names each tensor's storage `data/<key>`; the tool pairs the 702 names with their keys
  from the pickle's strings. The 351 convolutions run as 352 operations over one buffer of 2048 floats a tile texel
  (the first features, three 192-channel dense buffers, then the ×2 steps reusing the space), each
  `dst = a·act(conv(src)) + b·res1 + c·res2`, one compute shader, one operation a submission. Tiles are the largest of
  192/160/128/96 texels a side the card's buffer limits allow; neighbours overlap by 32 texels, each tile's outer 8
  texels of an overlap are dropped and the middle 16 blended linearly. The CPU runs the same operations in the tests.
  About 1 s a 256 × 256 texture on a current graphics card.
- The lock compares RGB only where the source's alpha is above 0; two bilinear rounds, then the remaining difference
  over each texel's 4 × 4 (or 2 × 2) block, repeated (at most 8 rounds in all) where texels sit at 0 or 255 and
  the rest must take up the difference. Real-ESRGAN smooths the originals' dithering,
  so most pictures take a round or more.
- Wrapped textures: every `.xv` archive's meshes, each face's UVs; the material's `texture` and `texture2` names
  (`foo.bmp` → `foo_bmp`) mark every install texture of that name, padded with 16 texels of its opposite side per
  wrapped axis before the upscale and cropped after.

**Videos and talking heads (row 571)**, `ig2-client::pack_tool`, `ig2_formats::mp4::write`, `ig2-client::video`:
- `--only videos`: every `anim/mpeg4/**.mp4` decoded (`video::decode::Frames`, display order), each frame through the
  compact model (its 4× halved for ×2; `--filters-only`: Catmull-Rom) and the lock, then turned into Y′CbCr 4:2:0 by
  the exact inverse of the player's `video_fragment.shader` arithmetic (so a frame plays back in the colours it was
  given; chroma the mean of each 2 × 2) and encoded by OpenH264 (Baseline, no frame skipping, a key frame at least every
  250 frames). Bit rate: the original's per pixel × 2 (at × 1 the rate control starved fast motion, 5.7 levels a
  channel off in `misc/end_x.mp4`'s worst frame; at × 2 2.8), about 4.5 times the original's size at ×2; the stage's
  space check takes the install's sizes × factor² × 2. OpenH264 ignores a QP cap in its bit-rate mode.
- The written file: `ftyp`, `mdat`, `moov`; each sample its own chunk with 64-bit offsets; the video track keeps the
  original's timescale and each sample's duration (no B-frames, so no `ctts`), the sound track the original's AAC
  samples, timescale, durations and `esds`. Both tracks' samples are laid out in the original's file order (each
  frame takes its original sample's place): the player's sound start (`Clip::sound_wait`, re/screens/video.md §4)
  counts the frames before the first sound chunk, so it stays the original's.
- The player opens `hires/<game path>` from an enabled mod in place of the install's file and draws it in the
  original's rectangle (`Player::size` stays the install's); the frame rate and the clock are the file's, unchanged.
  About 0.5 s a 640 × 400 frame at ×2 on a current graphics card.
- `--only heads`: every frame of the 14 `anim/fejek/*.ani` (decoded with transparency where no frame wrote yet) through
  the picture path at ×4 (x4plus, or the compact model with `--fast`) and the lock, as
  `hires/anim/fejek/<name>/<frame>.png`, frames counted from 0; a head counts as made once its last frame is there.
  The diplomacy and spy screens load the heads through `upscale::head_frames`, which hangs each pack frame on its
  decoded frame as the picture the renderer uploads (the spy disc's opaque fill applied to both). About 340 MB for
  the 14 heads, 6 s a head.
- `--smooth` (RIFE) is not built yet.

**The font (row 572)**, `ig2-client::font_sdf`, `font`, the quad shader's kind 24:
- `--only font`: the atlas (512 × 512, two texels a `KiirStr` unit) cut into `KiirStr`'s cells for the four sizes and
  every glyph of the longer order string (`cells`: `(g % per_row)·step_x`, `(g / per_row)·step_y + start_y`, width ×
  height); each cell alone (so no neighbour's ink reaches its field) through xBRZ ×4 then ×2, ink where the brightest
  channel reaches 128, the signed distance measured there exactly (Felzenszwalb–Huttenlocher, the edge half a texel
  past the outermost ink texel's centre), averaged over 2 × 2 and stored at ×4 (2048 × 2048, grey, 0.5 on the edge,
  ±8 stored texels = one `KiirStr` unit of reach). Under a second.
- The lock's counterpart: the field read back at the atlas's texel centres has ink exactly where the original passes
  half, but for under 2 % of the ink texels (xBRZ rounding pixel corners).
- The client: with `[text] sdf_font` and the file in an enabled mod, `Font` uploads the field (linear) and draws every
  glyph as kind 24 instead of 7: the same quad, uv and colour, coverage `clamp((d − 0.5) / fwidth(d) + 0.5)` (one
  screen pixel of ramp at any scale). Without either the font draws as before; a missing file is reported once.
