# ADR 0079: the starmap HUD on the design kit

Date: 2026-11-03. Status: accepted. Amends ADR 0065 (the modern starmap HUD: its look and the long info's place) on
the design kit of ADR 0073. Implemented by `ig2-client::starmap_modern` and the window's pointer
(`window.rs`: `starmap_virtual`).

## Context
The design system's starmap boards ("Starmap: default", "Starmap: planet selected", 1920 × 1080) redraw the HUD of
ADR 0065 in the kit's language: a full-width metal top bar, a deep glass selection card, a cast-metal navigation plate
and the planet's long info as a large panel at the screen's right under the bar. ADR 0065's console look (small dark
plates with cut corners) predates the kit; the long info sat on the original's box in the middle of the screen,
because the original's hot-spot code hit-tests it there.

## Decision
1. **Top bar** (`.ig-topbar`): one metal strip across the screen, 44 points high, 12 × 4 inside and 10 between its
   parts: the pause and speed keys as one key block (the running one pressed in and lit in LCD ink), the money and the
   date as LCD readouts (fixed widths, so the bar doesn't move as the digits change; money below zero in the enemy
   colour), the message ticker taking the room between (the original's scrolling message, the waiting text pulsing in
   LCD ink, "Pause" in LCD ink while paused and nothing scrolls, else the latest history line dim), the research button
   (300 wide: flask, project, progress bar, per cent in the LCD face, or the idle text) and the screens as metal keys
   in two blocks (the screens; help, messages and options), a lit dot on the messages key while a message waits. The
   colony, info and research screens keep using this bar (`starmap_modern::bar`).
2. **Selection card** (`.ig-panel.deep` with `.ig-card`, 380 wide, 24 from the bottom-left corner): a shaded globe in
   the owner's colour (a fleet's mark for a fleet) between the richness and habitability ratios, the name, the
   original's lines upper case in their colours, then the plates (the colony or split, the details) and the tanks and
   production keys. The card is still the info box: left and right click are the original's.
3. **Navigation plate** (cast metal, 300 wide, 24 from the bottom-right corner): the radar sunk into it at 280 wide in
   the classic window's ratio (the view's outline in the focus colour, the target pointer over it), under it the
   camera's keys in two blocks (rotate, zoom, tilt held; top and side view).
4. **The long info as the board's panel**: 460 wide, 24 from the screen's right, 28 under the bar, sliding in and
   fading with the camera's flight: a lit header (the globe, the name, the owner glowing in its colour, the ratios), the
   original's lines as a data table (labels muted and upper case without the game's colons, values in the info colour,
   shortages in the enemy colour, the needed amount after the value as "have / need"), the problems, the settings rows
   (a field with a chevron that opens its menu, ALL, the round − and +), and the production and exit plates.
   **Same functions**: the panel takes no input itself; its controls are the original's hot-spots moved there. The
   window maps the pointer before the original's code sees it (`State::info_pointer`, from where the panel drew its
   controls the frame before): over a control, the middle of the original's spot; over the rest of the panel or over
   the original's controls' old places, the original panel's empty part (it takes the press, as the original's
   does); over an open settings menu, the menu's original place, since the menu is drawn dropping from the modern
   field (or over it at the screen's bottom). So the hover, press-and-hold, menus, orders and sounds are the
   original's. The production plate is the card's production button (hot-spot 11) and the exit plate the info box's
   left click (hot-spot 8): both leave the long info as before. The selection card is hidden while the long info is up
   (it shows the same planet).
5. **Radial menu** on the kit: each icon on a metal key (a lit ring round a dark face), the one under the pointer
   lit in LCD ink with a glow and its name on a command plate beside it; places, timing and hit circles unchanged.
   The select menus are deep glass with the list's selected look on the row under the pointer.
6. **Motion and pointers**: hover fades (120 ms) on every key, plate, field and the ticker; the research menu drops
   with a fade and a 12-point slide; the hand over everything a click works, the target pointer over the radar
   (ADR 0073 §6). Snapshots draw without motion.
7. **Text**: every label through the kit's fitted text in a fixed box (cut with "…" and its whole text as a tooltip,
   or shrunk to a floor); long or translated strings never resize the bar, the card, the plate or the long info.

## Consequences
- The boards' planet pictures (the card's, the long info's header) and the radial icons' pictures stay drawn: a shaded
  globe and the HUD's line icons on metal keys; the radar is drawn, not the game's rendered one.
- The board shows a colony plate in the long info; the original's long info has no colony function (the info box's
  right button does nothing there), so there is none.
- The production and exit plates act on the click (the release), the original's hot-spots 9 and 11 on the press: the
  same effect a moment later, as the card's buttons already did.
- The space stations' sale strips (hot-spots 0x65.., at the original screen's left) stay where the original has them;
  the modern panel draws no station list.
- Only the starmap's hot-spot code sees the remap: the drawn pointer, its tooltip's place, the replay bar and the
  multiplayer wait overlay use the real pointer.
- Classic and every capture and replay are unchanged (the option and `--modern-hud` as in ADR 0065).
