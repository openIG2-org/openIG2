# ADR 0062: smooth glows

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::render` (`kind::SOFT`, the B-spline and the point
dots), `ig2-client::starmap` (the glows, particles and sky sphere), `ig2-client::mainscr_scene` (the menu's galaxy),
`ig2-client::spacewar_screen` (the shots' flares) and `ig2-client::display` (one row).

## Context
The starmap and the main menu are lit by additive sprites: the systems' star glows (`n1..n8.bmp`, 128 × 128), the
galaxy clouds and fog clumps (`felho2/3.bmp`, 128 × 128), the radar dots (`radar.bmp`, 16 × 16), the menu galaxy's
four clouds and its dust (`n2.bmp` and `felho2.bmp` of `textures/main`, 16 × 16) and its pictures (`galaxy_1.bmp`,
`texcsor.bmp`, 256 × 256), plus the far stars and the menu's dust points drawn as one white texel. The exe samples
them bilinearly; a 16 × 16 glow drawn 60 pixels wide at 1080p shows its texel grid as a faint diamond pattern, a
one-texel star is a square two or three pixels wide, and the sky sphere's nebulae (`texcso3.bmp` 256 × 256 and
`ujtext.bmp` 128 × 128 on a sphere of radius 2048) are magnified about ten times. With "Sharp UI" (ADR 0043) on,
the glows were sampled sharp bilinear like the pictures they are not: each texel a flat square, so the menu's
lights and the starmap's clouds turned into blocks.

The remastered look need not match the original where the original shows pixels (ENHANCEMENTS.md, Decisions,
2026-11-01); the classic look stays the exe's.

## Decision
1. **A sprite kind for a scene's glows and particles** (`kind::SOFT`): the starmap's star glows, clouds, fog
   clumps, far stars, radar dots and the selection ring's lights, the main menu's galaxy clouds,
   dust and pictures, and the space battle's shot flares. Off the option it samples exactly as `kind::SPRITE`
   does without "Sharp UI" (the same `textureSample`), so the captures are unchanged; "Sharp UI" no longer touches
   these sprites (it keeps the pictures, panels and text sharp, as its row says).
2. **"Smooth glows"** (`SmoothGlows`, a Remake row under "Graphics", off by default; `--smooth-glows`):
   - a magnified `SOFT` texture is read through a cubic B-spline (four bilinear taps): smooth everywhere, no texel
     grid at any size; at 1:1 or minified the bilinear (mipmapped) sample as before;
   - a one-texel (white) point is a round dot inside its square, anti-aliased, its colour × 4/π so it adds the
     square's light;
   - the sky sphere's textures load as a 4× Catmull-Rom copy (unless a texture pack or the upscaling row already
     gave one), so the nebulae show no texel grid either. This part takes effect when the starmap's textures load
     (the next start); the rest at once.

## Rejected
- **A procedural glow in place of each texture** (a radial falloff tinted by the texture's mean): the star and cloud
  textures are not plain discs (rays, noise), and the B-spline keeps their shape while removing the grid.
- **Linear and mipmaps alone**: they already sample bilinearly; the grid is the bilinear filter's own.
- **Upscaling every glow texture at load**: needs a reload on toggling and more memory, for the same result the
  B-spline gives per pixel.
