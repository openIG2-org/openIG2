# ADR 0043: sharp 2D pictures and text on big screens

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::render` (the 2D shader's sampling) and
`ig2-client::display` (the "Sharp UI" row).

## Context
The client draws the 640 × 480 virtual screen straight into the window (ADR 0010, `kit::Layout`): every panel,
glyph, tooltip and the game's own pointer (`UpdateFrame`) is placed in virtual units times the frame's scale, so
the UI already has the right size at any resolution; at 3840 × 2160 the 4:3 frame is 2880 × 2160, a scale of 4.5.
The window's DPI factor is respected the same way: the default window is 1600 × 900 logical pixels, a chosen
resolution is physical pixels, and the few things drawn in window pixels (the setup screen's text, the starmap's
smallest markers and pick radius) already multiply by `scale_factor`.

What looks wrong on a big screen is the sampling. The exe's 2D pass samples its pictures and font with `GL_LINEAR`
(re/screens/hud.md §1.1). The screen pictures are drawn at twice the virtual size, so at 800 × 600 they are
minified and look crisp, but at 3840 × 2160 each texel is magnified 2.25 times (the 1× pictures 4.5 times) and
becomes a soft ramp: panel edges, the bitmap font and the pointer look blurred.

## Decision
1. **A "Sharp UI" row** (`SharpUi`, off by default = the exe's bilinear look; the captures and snapshots draw it).
2. **Sharp bilinear** when on, for the 2D sprites and glyphs (instance kinds 0 and 7): each texel is drawn as a flat
   square and only its border pixel is blended with the neighbour (the texel coordinate's fraction is stretched by
   the screen pixels per texel, `1 / fwidth`, before the bilinear fetch). Any fractional scale stays sharp without
   shimmer; at a scale of 1 or below (pictures drawn 1:1 or minified, the 3D scene's render targets) the result is
   exactly the bilinear one. The fetch keeps the unmodified coordinate's derivatives (`textureSampleGrad`), so the
   mip level is unchanged.
3. **The pointer** is one of those sprites, so it follows the UI's size and the option without extra code.

## Rejected
- **Integer scaling** (the 4:3 frame shrunk to the largest whole multiple, 4 × at 2160 lines): sharp pixels but a
  smaller frame with wider black bars, and a second mouse mapping through every screen; sharp bilinear gets the same
  edges at the full size.
- **Nearest filtering**: uneven texel widths at a fractional scale (at 2.25 some texels 2, some 3 pixels wide),
  visible as wobbling lines.
- **An offscreen 640 × 480 2D pass upscaled at the end**: the 2D pass interleaves with the 3D scenes and the
  remaster's wide layout draws outside the 4:3 frame; the per-sprite sampling needs neither.
