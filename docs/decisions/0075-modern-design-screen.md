# ADR 0075: the modern ship and vehicle design screen

Date: 2026-11-03. Status: accepted (the design screen of the "Modern versions of every original screen" package,
`docs/ENHANCEMENTS.md` → UI). Implemented by `ig2-client::design_modern` (the panels and their acts),
`ui::Hud::design_modern_view` (what they show, read from the state `DesignGfx2D_1` and `_2` read),
`design::Design::modern_act` (the acts through the original's hot-spot code), `design_scene::DesignScene::modern`
(the console left out), the option `ModernDesign` (`display.rs` row 47) and the window (`design_modern_up`,
`design_modern_apply`); `--snapshot --screen design --modern-design [--design-row N --design-edit K]` draws it
headless. Uses ADR 0073 (the design kit).

## Context
The design screen (re/screens/design.md) was the last of the main game screens without a modern version. Its 2D
parts are faders and texts around the 3D console (the design list, the data, the pod rows, the four 3D buttons, the
popups held open by the left button over hot-spots of a mask picture). The ship design board (1920 × 1080 at 100 %
UI scale) lays the same functions out as kit panels around the turning hull.

## Decision
1. **Option**: "Modern design screen" (Interface, on by default); off, in input scripts and in captures the
   original's screen draws and takes the input as before.
2. **Layout** (the board's, one design pixel one point; 4:3 and 21:9 keep working):
   - top left, the design list (400 wide, deep panel): each design with its hull at the right, the chosen one
     selected, obsolete ones muted; it scrolls past what fits;
   - top middle, the data (up to 780 wide, its right edge 16 from the parts): the hull and the price; the
     production time, the units and the tank capacity;
   - top right, the parts (540 wide): "n POD", each pod with its number, its class ("WEAPON · MAX 4") and tech, the
     count on an LCD, its −/+ while editing, "Choose" on an empty pod while editing; the chosen pod's tech list
     opens inline under it;
   - bottom left, the name field (a click starts the original's name input; the text and a blinking caret show it)
     with speed, firepower and hit points;
   - bottom middle, the console: the previous and next design keys around the four plates (new, redesign, modify,
     delete; while editing OK, cancel, auto, clear), their popups (the hulls of "new", the sale, the refit choice)
     above the plate; when the window is too narrow for it (a large UI scale) it moves above the exit;
   - bottom right, the exit (the top bar's starmap pick).
3. **The 3D view stays**: the hull slides in and turns as the original's, with its pods' wire boxes while editing,
   the tech models and the wire sphere; only the console (`desbck.xvb`) is not drawn. The right button still turns
   the hull; the left button's hidden mask hot-spots are off under the panels.
4. **Same functions**: every act runs the hot-spot's own code (`Design_Main`, §6.1): a list row is spot 9, a plate is
   its console button (`edit_press`), a popup's pick is the release over its row (`dSelected`, `edit_release`), a pod
   row is spot 0x54 (the pod and, while `CanModify`, its tech list), a held −/+ is the pod's cell (0x35..=0x52) with
   the `KezdoIdo` repeat, the name is 0x56. The orders (0x33, 0x34) and the tutorial events are the original's; the
   keys are untouched.
5. **Popups stay up** until a pick or a press elsewhere, instead of following the held left button: a modern menu
   is clicked, not dragged.

## Consequences
- The screen opens without the console's animation (as from the options screen).
- The 3D pod pick on the hull and the tech models' tooltips are not reachable with the left button under the
  panels; the parts panel does the same.
- No exe capture exists for the new look: screenshots go with the change.

## Amended 2026-11-03: board compare pass
The chosen pod's tech list lists every fitting tech one under another (it showed only the first row), the fitted one marked; the pod count buttons have the board's 8-pt gaps; the properties row's labels shrink to fit; the kit's controls and the rows show the hand pointer, and the classic top bar's tooltips are off under the modern bar. Not done: the board crops the hull to the middle well, and large hulls overlap the panels' edges.
