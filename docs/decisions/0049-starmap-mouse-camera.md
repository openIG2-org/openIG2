# ADR 0049: wheel zoom and right-drag pan on the starmap

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::starmap` (`Camera::drag`), `ig2-client::window`
(`wheel`, the starmap's right press and release, `cursor_moved`) and `ig2-client::display` (the "Starmap mouse
camera" row).

## Context
The original's starmap camera moves only by the screen edges, the minimap, the arrow keys and numpad + / −
(re/screens/starmap.md §10.3, §10.6); `Starmap_Main` (ELF 0x6f50a0) reads no wheel, and the right button gives the
selected fleet's order (0x15) or the rally order (0x400) on the press. Players used to strategy games reach for the
wheel and a right-drag first. The colony screen already zooms with the wheel (docs/ENHANCEMENTS.md row 133).

## Decision
1. **A "Starmap mouse camera" row** (`StarmapMouseCamera`, on by default). Off = the exe: no wheel, the right press
   gives its order at once and never moves the camera.
2. **The wheel** zooms as the zoom keys do, one `StarmapKeyboardDoModifies` step per notch (`Zoom ± (640 − Zoom)/50`,
   clamp [2, 580]); not while the long info, the zoom-in view or the fleet panel is up (the keys' own gate).
3. **The right drag**: a right press on the map grabs the galaxy point under the pointer; once the pointer has moved
   more than 4 pixels (the left click's tolerance) with only the right button held, the pivot moves so that point
   stays under the pointer, inside `Starmap_Move0`'s ±300 square. Rotation, tilt and zoom are kept.
4. **The order moves to the release**: with the row on, the right press on the map gives no order; an unmoved
   release gives the press's order at the press's point (the modifiers as at the release). A drag gives none. The
   scripted capture runs keep the exe's controls. The
   info box's right release (colony, split) and the console and panel spots' right presses are unchanged. This is
   the only timing change: an order arrives a press-to-release later (one or two frames), which no rule reads.

## Rejected
- **Keeping the order on the press and panning too**: a right-drag with a fleet selected would send it to the press
  point every time the player pans.
- **Middle-button drag**: the original's mice had no middle button reachable by every player, and the right drag is
  the common convention in the genre.
