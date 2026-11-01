# ADR 0046: sharp text at any window size

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::render` (the glyph shader's `cubic` and `sharpen`)
and `ig2-client::display` (the "Sharp text" row).

## Context
`KiirStr`'s font (re/screens/messages.md §1) is the remaster's `screen/charset_bmp`: 512 × 512 anti-aliased grey
glyphs, addressed as a 256-unit atlas, so a size-2 glyph (6 × 7 virtual pixels) is 12 × 14 texels. The exe samples it
bilinearly (`GL_LINEAR`, re/screens/hud.md §1.1). At 800 × 600 a texel covers 0.625 screen pixels and the glyphs look
crisp; at 2560 × 1440 it covers 1.5, at 3840 × 2160 2.25, and every edge becomes a ramp two or three pixels wide.
"Sharp UI" (ADR 0043) keeps the texels square, which is sharp but shows the texel grid on every diagonal and curve.

## Decision
1. **A "Sharp text" row** (`SharpText`, `--sharp-text`; off by default = the exe's bilinear text; the captures and
   snapshots draw it). It combines with "Sharp UI": that one keeps the pictures and the pointer, this one the glyphs.
2. **The glyph's coverage read as a distance field** when the glyph is magnified (a texel wider than a screen pixel):
   the coverage is interpolated Catmull-Rom from the 4 × 4 texels around the point, each tap clamped to the glyph's
   cell (no neighbour bleeds in), and its ramp around 0.5 is narrowed to one screen pixel:
   `clamp((c − 0.5)·s + 0.5, 0, 1)` with `s` the screen pixels per texel. The 0.5 edge, where the font's strokes
   are, stays where it is, so strokes keep their width and place; edges come out one pixel wide and diagonals and
   curves smooth at any scale. Minified or 1:1 (`s` ≤ 1, a 4:3 frame up to 960 lines), the shader takes
   the bilinear path unchanged: an 800 × 600 frame is the same pixels with the option on or off.
3. **No metrics change**: the quads, advances, heights and atlas cells are `KiirStr`'s; only the fragment shader
   differs (test `font::tests::sharp_text_keeps_the_metrics`, and `render::tests::sharpen_keeps_the_edge` for the
   formula).

## Rejected
- **EPX/xBR upscaling of the atlas at load**: those filters look for equal neighbours; the atlas has 256 grey levels
  (anti-aliased), so they act like nearest-neighbour on most edges.
- **A true signed distance field built at load**: needs a threshold on the grey levels first, loses the same soft
  pixels the shader's ramp does, and adds a second atlas; the coverage itself is already a usable distance proxy
  for strokes two or more texels wide.
- **Vector or system fonts**: no new fonts are shipped, and the game's glyph shapes are part of its look.
