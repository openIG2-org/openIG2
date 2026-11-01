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
2. **What it redraws** on the starmap (nothing over it: no colony, overlay screen, battle, spy, help or options):
   - the top bar as three glass strips: pause and the three speeds (vector icons, the current one lit, the extra
     speeds' factor beside them), the date, the money (thousands apart); the message line (left click: the messages
     screen; right button held: the history drop-down; the chat entry and the error text show in it); the quick
     research (the running project and its per cent; click: the menu of startable techs by column with their cost,
     the running one's progress, money left and STOP) and the screen menu (the original's screens with their keys,
     the disabled ones greyed);
   - the selection's card (bottom left): an owner-coloured mark, the name, the owner and the original's info lines in
     their colours, richness and habitability, the left and the right action as buttons ("Details": the long info or
     the fleet's panel; the colony screen or "Split"), the tanks and production buttons as icons, the redirect mode's
     mark; a click on the card is the info box's left or right click, both buttons there start the rename;
   - the console and the minimap (bottom right): rotate, zoom and tilt buttons held as the console's (zoom out and
     tilt down are the console's right-button directions), the top and side views, the minimap with the view's
     outline (press or drag: the camera flies there).
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
6. **Scale and ratio**: the panels are laid out in ADR 0062's points (540 high), anchored to the band's corners and
   edges, so they keep their size relative to the height at 4:3, 16:9 and 21:9 and stay sharp at any resolution; the
   UI theme (ADR 0056) is their style.

## Consequences
- The original's HUD code stays and draws whenever the option is off (and under the long info).
- No exe capture exists for the new look: screenshots go with the change (`--snapshot --modern-hud`).
- The info card shows no turning globe or fleet model (the original's box does); a colour mark stands for it.
