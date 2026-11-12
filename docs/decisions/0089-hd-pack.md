# ADR 0089: the HD pack: every picture, video, talking head, the font and the models' detail, made from the player's install

Date: 2026-11-12. Status: accepted. Builds on ADR 0054 (texture and model packs), ADR 0088 (the remaster as mods;
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
