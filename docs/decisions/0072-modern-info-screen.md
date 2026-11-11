# ADR 0072: the modern info screen

Date: 2026-11-03. Status: accepted (the info screen of the "Modern versions of every original screen" package,
`docs/ENHANCEMENTS.md` → UI). Implemented by `ig2-client::info_modern` (the panels, their acts, what they show),
`info_screen::entry_lines` and `detail_lines` (the rows' and the detail mode's texts, shared with the original's
drawing), and the window (`window.rs`: `info_modern_up`, the pointer routing, `info_modern_apply`);
`--snapshot --screen info --modern-info` draws it headless.

## Context
The decision of 2026-11-01 (ADR 0065's package): every original screen gets a modern egui version close to the
original's layout, functions and keys, with the modern starmap HUD's look; classic keeps the original. The info screen
(re/screens/info.md) is a 3D frame with eight race columns picked by the pointer passing over them, a list of four
rows with arrows and a scroll bar, a detail mode reached by a click on a row, the planet settings as menus held open
under the pointer, a money triangle dragged with the button held, a spending limit set by the pointer resting on its
bar, ten chart buttons and, in a scenario game, the swap to the star log.

## Decision
1. **An option**, "Modern info screen" (`ModernInfo`, Remake page, Interface), **on by default**; off = the original's
   screen. Snapshots and capture comparisons draw the original's unless `--modern-info` is given, so every capture and
   replay test stays byte-identical.
2. **What it shows**, as the starmap HUD's console plates in the empire colour (ADR 0065), following the shared UI
   scale, over the game's picture dimmed:
   - top: the starmap HUD's bar (the info screen's icon lit);
   - left, the empires met and alive (the race columns' set, in the relation colours of `KirakFajNeveket`), sortable by
     name, planets or fleets seen; under them the empire panel's rows (`KirakBirodalomInfo`) and, for the player, the
     spending limit as a bar and the money triangle with its corner names and shares (blinking after a drag);
   - centre, the selected empire's planets and fleets as one scrolling list with every entry (not four at a time),
     each row the original's lines (`KirakBolygoInfo`/`KirakFlottaInfo`); sortable by the original's order, name, size
     (population or ships) or alerts (the worst problem or the fleet's power), each key only where the player sees the
     value; a planet row's problem line steps through the morale and the planet's own problems every 30 `Counter` (the
     original's step is shared by the four rows);
   - centre, instead of the list in the detail mode, the detail rows (`InfoNagyKiirBolygo`/`InfoNagyKiirFlotta`) with
     Back; the planet settings as drop-down menus; a fleet's whole design list (the original's shows nine and its
     arrows shorten it, [quirk]);
   - right, the ten charts as a two-column button grid and the selected one drawn large (every known empire's line on
     one scale), or the star log as a scrolling list; the swap between them only in a scenario game, as hot-spots 14/15.
3. **Same functions**: `info_modern_apply` runs the original's code for each act at the hot-spot the original's player
   would have used: an empire is `InfoScreen::hover` on its column (1..=8), a chart `press(16 + c)`, the swap
   `press(15/14)`, a row `press(9 + k)` with `BolygoFlottaPointer` moved so the entry is among the four shown, Back
   `press(37)`, a setting the menu's release over its item (`orders` on 26..=28 → 0x19 8/9/10), the triangle `orders`
   on 29 with the press, held and release frames (→ 0x5b), the limit `orders` on 32 at the bar's virtual x (→ 0x5c).
   Each is gated by `spot_on` as the original's hot-spot. The orders are the lockstep commands the classic screen
   sends, so multiplayer is unchanged. The keyboard is untouched.
4. **Input**: egui takes the pointer on the whole screen while it is up; the classic hot-spots get no hover, press or
   per-frame held spot, and their tooltips and pointer shapes are off.
5. Ours: an empire is picked by a click (the original's pointer passing over its column picks it); the panels answer
   the left button only (the original's rows, buttons and menus also take the right one); the limit follows a press or
   drag on its bar (the original sets it whenever the pointer rests on the bar, no
   button needed); the list shows every entry and sorts; the 3D frame, its globes, heads and button animation aren't
   drawn.

## Consequences
- The original's info screen stays and draws whenever the option is off.
- No exe capture exists for the new look: screenshots go with the change (`--snapshot --modern-info`).
- `info_screen::Put` (a positioned text) lets a modern screen reuse the original's text builders while the classic
  drawing stays the same calls in the same order.
- A window too short for the panels (a larger UI scale) scrolls the page between the top bar and its bottom with the
  mouse wheel; the list keeps at least 366 points, the charts stop above the Exit plate, and nothing covers the
  footer buttons.

## Amendment (2026-11-03): the design boards
The info boards of the design kit (ADR 0073: the list, the detail mode; 1920 × 1080 at 100 % UI scale, one board
pixel one point) replace §2's console plates. Implemented by `info_modern.rs` on `gui::ds`; the kit gains
`ds::globe` (a surface map drawn as a lit, turning sphere).
1. **Layout**, anchored to the sides (4:3, 16:9, 21:9 keep working; a shorter window, a larger UI scale, closes the
   gaps and never the panels): under the bar, the eight race columns as metal rings, four left and four right of the
   money panel, a met empire's leader (`textures/diplomac/<race>a.bmp`, the race its empire plays) in its ring and
   its name under it in the relation's colour, the others empty and faded, the selected one with a silver ring; the
   money panel (deep, 400 wide) with the triangle, its knob and the three shares on LCDs (blinking after a drag);
   under them the selected empire's panel (960 wide): `KirakBirodalomInfo`'s rows as two table columns and the
   spending limit as a bar with a metal thumb and the limit on an LCD; bottom left the list panel (half the width):
   tabs for the planets, the fleets and the empires met with their counts, the empire's name, sortable column heads
   (name, population or ships, alerts; the middle column, morale or orders, has none), 92-point rows (a planet's
   globe turning from its surface's map `textures/starmap/p<n>.bmp`, a fleet's mark; the name over the owner and a
   planet's production line; population, morale and the worst problem, or a fleet's ships, order and speed, on one
   baseline), under them the range shown, a scroll bar and the row's hint; bottom right the ten charts on the
   original's button plates (`textures/info/info_b1..5` left, `info_j1..5` right; the star log's swap under the left
   column, `info_b6` / `info_j6`) around the chart panel (the chart's title, the known empires' names, the lines on
   one scale with the original's bright spot running along each) or the star log; an Exit plate bottom right.
2. **The detail mode** takes the list's place (a short slide and crossfade): the entry's globe (220), name, owner and
   Back (37) on the left, the detail rows as a table (labels without their colons), the planet settings as
   drop-downs.
3. **Exit** is the bar's starmap key (`AktKepKell` 2), as the top bar's screen menu; the original's info screen is
   left through the same menu.
4. **Text**: every label goes through `ds::Text` in a box of the board's size (test `long_texts_keep_the_panels`);
   the morale column's head is ours ("Morale"), the fleet columns' are ours ("Ships", "Orders", "Speed").
5. Still differs from the boards: the race rings show the 2D head picture (the boards show the game's rendered 3D
   head); the detail values keep the original's palette colours; the detail mode has Back where the board shows
   Colony and Map (the original's detail mode has neither).
