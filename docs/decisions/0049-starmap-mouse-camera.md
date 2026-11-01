# ADR 0049: wheel zoom on the starmap

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::window` (`wheel`) and `ig2-client::display` (the "Starmap mouse
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
3. **The right button stays the exe's**: its press gives the order at once and never moves the camera. Panning is
   the edges, the minimap, the arrows and W/A/S/D (ADR 0030).

## Rejected
- **A right-drag pan** (shipped first, removed the same day): with the order moved to an unmoved release, quick
  repeated orders often moved the map by a few pixels and dropped the order; W/A/S/D already pans.
- **Keeping the order on the press and a right-drag pan too**: a right-drag with a fleet selected would send it to the press
  point every time the player pans.
- **Middle-button drag**: the original's mice had no middle button reachable by every player.
