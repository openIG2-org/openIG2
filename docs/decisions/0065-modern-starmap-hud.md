# ADR 0065: the modern starmap HUD

Date: 2026-11-01. Status: accepted (part 3 of the egui package, after ADR 0062). Implemented by
`ig2-client::starmap_modern` (the panels, their acts), `ui::Hud::modern_view` (what they show, read from the state the
original's HUD reads) and the window (`window.rs`: `modern_up`, the pointer routing, `modern_apply`; the frame's
`Gpu::present_over`); `--snapshot --modern-hud` draws it headless.

## Context
The decision of 2026-11-01 (`docs/ENHANCEMENTS.md` → Decisions, the "Starmap" Ideas row): the most-used screen looks
its age, and the remastered look need not match the original. The starmap's HUD is the original's 640 × 480 artwork
scaled up: the top bar's brushes, the bitmap font, the stone console model with its minimap window, a small faded box
for the selection. The functions behind it are ported hot-spot by hot-spot (re/screens/starmap.md §10, §11,
re/screens/hud.md §2, re/screens/input.md §6) and must stay exactly as they are.

## Decision
1. **An option**, "Modern starmap HUD" (`ModernStarmapHud`, Remake page, Interface), **on by default**; off = the
   original's HUD. Snapshots and capture comparisons draw the original's unless `--modern-hud` is given, and the input
   scripts always click the original's hot-spots, so every capture and replay test stays byte-identical.
2. **What it redraws** on the starmap (nothing over it: no colony, overlay screen, battle, spy, help or options), as
   a ship's console (revised 2026-11-01 after the first version was judged too big and too plain): small dark plates
   with the top-left and bottom-right corners cut, a hairline edge and corner marks in the player's empire colour (lifted
   toward the text colour), drawn icons in one stroke, the theme's text colours, the Jura type in three sizes (money
   largest and white, red below zero; names 15, text 12.5, labels 11 points):
   - top left: pause and the three speeds (the current one lit with an accent bar, the extra speeds' factor beside
     them), the date, the money (thousands apart); beside it the message line: the original's scrolling message, the
     chat entry, the error or the waiting text (pulsing), else the latest history line dim, else only its mail icon
     (left click: the messages screen; right button held: the history drop-down);
   - top right: the research under way (its name, per cent and a progress line; idle: "No research"; click: the quick
     research menu of startable techs by column with their cost, the running one's progress, money left and STOP) and
     the original's screens as a row of icons (the current one lit, the disabled ones dim and not clickable);
   - the selection panel (bottom left): an owner-coloured mark, the name, the owner, the original's info lines in their
     colours, richness and habitability, then icon buttons: details (the long info or the fleet's panel), the colony
     screen or split, tanks, production, and the redirect mode's mark; a click on the panel is the info box's left or
     right click, both buttons there start the rename;
   - the navigation plate (bottom right): the minimap with a faint grid and the view's outline (press or drag: the
     camera flies there), under it one strip of the camera's buttons: rotate, zoom and tilt held as the console's (zoom
     out and tilt down are the console's right-button directions), the top and side views.
   Every button has a hover state and a tooltip: the original's text or the screen's name, with its bound key
   (ADR 0058) in a key cap where it has one.
3. **Same functions**: every act goes through the original's hot-spot code (`toggle_pause`, `set_speed`, the message
   line, `top_menu_screen`, the research order, `console_act`/`console_let_go`/`console_view`, `orders::minimap_target`,
   the info box's left and right click, `Panels::press_id(10 | 11)`), so the orders, sounds, multiplayer time rules
   and tutorial gates are the original's. The keyboard is untouched. The texts are the game's (tooltips
   `GetString(2, 1, n)`, the info lines, the menus) and the remake's own labels (ADR 0042).
4. **Input**: the window gives egui the pointer while the HUD is up; a press over one of its panels is the panel's
   (and so is that button's release); anywhere else it reaches the map as before. While the HUD is up the original's
   positional spots under it are off (the console mask, the info box, the top bar's spots, the panels' 8/10/11), so a
   click on open map where they were picks or orders. The wheel over a panel doesn't zoom.
5. **Unchanged**: the radial icon menu, the select menus, the production view, the transfer panel, the fleet's and
   vehicles' info panels and the planet's long info (whose box keeps the original's info box under it while open).
   They keep their original look; a later part may redraw them.
6. **Scale and ratio**: the plates are laid out in egui points and follow the shared UI scale of the egui layer: the
   top plates are 30 points high and the corner plates about 250 points wide, a small share of the screen at the
   default scale on 1080p and above; a player who wants them bigger or smaller sets the UI scale. The first version's
   plates were sized for the pages' larger layout and took about twice the room. The plates are anchored to the band's
   corners and edges (4:3, 16:9, 21:9) and stay sharp at any resolution; the UI theme (ADR 0056) is their style.

## Consequences
- The original's HUD code stays and draws whenever the option is off (and under the long info).
- No exe capture exists for the new look: screenshots go with the change (`--snapshot --modern-hud`).
- The info card shows no turning globe or fleet model (the original's box does); a colour mark stands for it.
