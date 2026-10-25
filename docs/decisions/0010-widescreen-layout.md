# ADR 0010: Widescreen follows the remaster's layout rule

Date: 2026-10-18. Status: **accepted** (the project's goal of 2026-10-17). Amends ADR 0009 §3 ("Widescreen,
high DPI and 4K"); the rest of ADR 0009 stands.

## Context
ADR 0009 §3 made a letterboxed 4:3 layout the default and offered "fill the screen with the 3D maps" as an opt-in.
The owner then noticed that ig2.exe itself doesn't letterbox at 16:9. Its rule is now specified and verified
(`re/screens/widescreen.md`: `ig2_resize` exe 0x4D84D0, `ig2_render_first` 0x4D7FE0, `ModifyViewPort` 0x4AC4F0,
`Copy3hatterkep` ELF 0x534AC0; the layout read out of the running exe at six window sizes; captures at 800 × 600,
1920 × 1080 and 2520 × 1080):
- the 2D layer (panels, buttons, text, hot-spots, videos) stays in the centred 4:3 frame, 480 virtual lines high;
- every 3D scene is drawn over the full window width at the same vertical field of view, so the sides show more of
  the starmap, the colony, the battle or the screen's 3D backdrop;
- background pictures are stretched across the full width;
- windows narrower than 4:3 are letterboxed top and bottom;
- the mouse maps through the 4:3 frame without clamping; edge scrolling uses the window's edges.

So what ADR 0009 offered as an enhanced option is the original's own default.

## Decision
1. **Default: the remaster's rule** (`re/screens/widescreen.md` §2–§4), exactly as above, with its integer rounding.
   It replaces ADR 0009 §3's first two bullets: the letterboxed default and the "fill the screen with the 3D maps"
   option both go.
2. **Option "letterbox" (off by default):** the 3D scenes and the background pictures are clipped to the 4:3 frame
   and the sides are black. This is the letterboxing that stays an option.
3. The acceptance diff stays at 800 × 600 (ADR 0009 §4); the M4 widescreen checks compare 16:9 and 21:9 captures
   (`run_screen.py --size WxH`) with the client's snapshot at the same size.
4. ADR 0009 §3's other points stand: sharp 2D from the 2× pictures, the font and the 3D drawn at the output
   resolution, free refresh rate.
5. The mobile toolbar (`toolbar=1`) is not carried over.

Correction (2026-10-24, M4 checks): `Copy3hatterkep` copies the background picture 1:1 from `HatarX1` (a wider window
reads past its 640 columns) instead of stretching it (`re/screens/widescreen.md` §3, §7).

## Consequences
- `ig2-client` already follows the rule for windows at least as wide as 4:3 (`font::Virtual`), so no screen built so
  far has to be redone. What's still missing (the narrow-window letterbox, one exe-exact layout type, the offscreen
  3D targets that are only 640 virtual pixels wide) goes into the shared screen kit.
- Deviations ledger entry 1 is closed: the premise ("we letterbox") was wrong for 16:9.
- Hot-spots never extend into the side strips, so input needs no widescreen-specific handling beyond the unclamped
  mapping.
