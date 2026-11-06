# ADR 0074: the modern colony screen on the design kit

Date: 2026-11-03. Status: accepted. Amends ADR 0070 (the modern colony screen) and uses ADR 0073 (the design kit).
Implemented by `ig2-client::colony_modern` (the panels, the picture wells, the colony's line icons),
`ui::Hud::colony_modern_view` and `ui::Hud::colony_long_info` (what they show), `colony_ui::ColonyUi::modern_act`
(the new acts through the original's hot-spot code), `window.rs` (the planet info mode under the modern panels, the
pictures in the top frame) and `render.rs` (`Renderer::draw_to_image_over_top`: the snapshot's top frame); the kit
gains `ds::round_button_sized`.

## Context
ADR 0070 drew the colony's 2D parts as small console plates in the starmap HUD's style and left the planet info mode
on the original's panels; the planet panel's turning building and the short info's turning model were not shown.
The design kit (ADR 0073) and its colony boards (building selected, ground war, planet long info; 1920 × 1080 at
100 % UI scale) give the look every modern screen now follows: glass panels with a rim and a lit top edge, bevelled
plates, metal keys, rust round buttons, LCD readouts, Oxanium type, text that fits its box.

## Decision
1. **Layout and look from the boards**, one design pixel one point, anchored to the corners (4:3, 16:9, 21:9 keep
   working; the war's sides panel narrows between the card and the radar):
   - top left, the build menu (430 wide): a picture well with the current kind's turning building, its name, price,
     energy and workers with their icons, the "Build" plate; the four groups as plates, the current pressed; the
     group's entries in a recessed list (name, the planet's count in the empire green, the price in info blue), the
     current kind's row selected, at most as many rows as fit above the short info (the rest scroll);
   - bottom left, the workers' and energy's surplus as two deep plates (red below zero) over the short info card: a
     picture well with the original's info model (the building, the units, the planet's globe between its richness
     and the race's affinity), the title with the details key and the on/off key (pressed in while switched off), the
     building's data as table lines (status and efficiency, product with its icon, the damage and construction bars)
     or the units' empire and weapon or the planet's lines;
   - right of the card, the building's repair, demolish and self-destruct as 60-point round buttons with their label
     on a black tag (lit while their window is open), the repair or demolish window as a raised panel over them;
   - bottom centre in a ground war, the sides panel: each side's empire, units, role (attacker, defender) and a bar
     of its units against the larger side's, the retreat between them as a round button; the other war buttons (air
     strike and laser strike with their charge bar, detonator, devastator, mines, cancel order) as round buttons
     above it;
   - bottom right, the radar in a cast-metal frame (the buildings, the units, the view's outline in the kit's
     radar, `ds::radar`: a sunk black well, no grid) with three keys under it: zoom out, zoom in (held: the numpad − / + camera flags, §11.9) and
     the starmap (hot-spot 4's right release);
   - under the top bar, centred: the attack banner, the retreat and landing lines, the self-destruct's countdown on
     an LCD with a pulsing red glow and its cancel plate.
2. **The planet info mode gets its own modern panel** (the long info board, 760 wide right of centre) instead of the
   original's: the planet's name and owner, the long info's lines as a table (surface, race, population, morale,
   income, tanks, special), the workers' and energy's NEEDED blocks with a bar (red when short), the problems, and on
   the player's planet the tax, auto-build and priority rows: a drop-down listing the items `open_menu` lists, "ALL",
   and −/+ where the original has them; under it the plates back to the colony (0x20) and to the starmap. Each runs
   the original's info-mode code: a button is its one-shot press (`info_press`), a drop-down item is the menu opened
   on its spot, the item hovered and released (`info_release`); a click on open ground leaves the mode (hot-spot 0x20
   under the panels). The 3D view keeps its keys.
3. **Pictures**: the panel's turning building (`PanelBuilding::render`) and the short info's model
   (`render_kis_info`) are the classic render targets; the panels leave wells and the window (and the snapshot)
   draws the targets' regions fitted into them in the top frame, after the egui layer and before the pointer.
4. **Motion and pointers**: the kit's hover fades and presses everywhere; the confirm window and the info panel fade
   and slide in; the build cursor's mark lights in the well and the armed "Build" plate breathes; full strike charges
   glow; the side bars glide; the radar shows the crosshair pointer; the build, attack and move pointers over the 3D
   view are the game's pointer types, drawn by the kit's set under "Modern cursors" (ADR 0073 §6).
5. **Text**: every label goes through `ds::Text` (shrunk and cut with "…", the whole text as a tooltip) in a box of
   the board's size; long or translated names never resize a panel (test `long_texts_keep_the_panels`); the
   original's padded labels lose their colons ("Damage :" → DAMAGE). New remake texts: Build, Attacker, Defender.
6. **Unchanged**: the functions and keys (ADR 0070 §3, §4); the classic panels when the option is off, in input
   scripts and in captures without `--modern-colony`; the "Build" plate is live only with the build cursor up, as
   before.

## Consequences
- The planet info mode no longer shows the original's panels with the option on.

## Amendment (2026-11-03)
The modern colony screen was compared with its four design boards (building selected, ground war, planet long info,
tank info) at 1920x1080, and the tank info board was added.

1. **Tank info board.** The starmap's vehicle info panel (`StarmapTanksInfo`, re/screens/starmap.md section 20) is drawn
   as the tank info board under the modern starmap HUD (`fleet_modern::View::Tanks`; ADR 0077's remap of the pointer
   onto the original's hot-spots: row k = cell 0x43+2k, the exit plate = hot-spot 9, the rest of the panels = 0x58).
   A title plate "planet · Tanks"; the list panel (560) with "Stationed tanks · n" and up to the original's eight
   designs as rows (the turning model, name, count; the original's highlight is the selected row, hover lit); the data
   panel (760) with the selected design's turning model in a 330-point view, its name and hull, armour, firepower,
   speed (and the ships' row where the original shows it) and the equipment rows; the exit plate under it, kept
   clear of the radar; the starmap dimmed under the panels.
2. **Fixes from the comparison.**
   - The radar shows the terrain's picture (the classic radar's) under its dots and a white view outline, the metal
     drawn as a frame around it.
   - The war banner and the card lines resolve the Hungarian article markers (no stray "•").
   - The round buttons' labels get room for long game texts ("Visszavonulás be/ki").
   - The war sides read "Attacker · Tanks" and "Defender · Tanks and turrets" (turrets = defence turrets among the
     side's units).
   - The "Build" plate is in the game's language.
   - The building and short-info pictures are cropped to their models.
   - The info mode's plates sit 12 under the panel.
3. **Stays different.**
   - Prices have thousands separators (the kit's money format, shared with the starmap HUD).
   - Game texts where the board's placeholder differs: tab names in capitals, "Kereskedelmi", "Sérült", "Lebontás",
     "Csillagtérkép", the exit button's English "OFF".
   - The repair button shows only when the building is damaged (the original's rule).
   - The planet info mode keeps the colony's 3D view (the original's mode is a flag over the live colony, colony.md
     section 12); the board shows a starmap backdrop with a large globe.
   - The attack banner sits under the top bar rather than in the ticker.
   - The exit plate is left of the radar instead of under it.
   - The top bar, the selection card and the starmap radar belong to the starmap HUD (ADR 0079).
