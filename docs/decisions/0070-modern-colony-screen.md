# ADR 0070: the modern colony screen

Date: 2026-11-02. Status: accepted; amended by ADR 0074 (the colony screen of the "Modern versions of every original screen" package,
`docs/ENHANCEMENTS.md` → UI). Implemented by `ig2-client::colony_modern` (the panels, their acts),
`ui::Hud::colony_modern_view` (what they show, read from the state the original's panels read),
`colony_ui::ColonyUi::modern_act` and `modern_parts` (the acts through the original's hot-spot code, the data), and the
window (`window.rs`: `colony_modern_up`, the pointer routing, `colony_modern_apply`, `bar_act`);
`--snapshot --colony P --modern-colony` draws it headless.

## Context
The decision of 2026-11-01 (ADR 0065's package): every original screen gets a modern egui version close to the
original's layout, functions and keys, with the modern starmap HUD's look; classic keeps the original. The colony
screen's 2D parts are the original's 640 × 480 artwork: the planet panel with the turning building, the pick list held
open under the pointer, a row of 31-pixel buttons, the radar frame, the short info's small text, faded windows for
repair, demolish and self-destruct. Their functions are ported hot-spot by hot-spot (re/screens/colony.md §6, §10,
§11, §12) and must stay as they are.

## Decision
1. **An option**, "Modern colony screen" (`ModernColony`, Remake page, Interface), **on by default**; off = the
   original's panels. Snapshots and capture comparisons draw the original's unless `--modern-colony` is given, and the
   input scripts always click the original's hot-spots, so every capture and replay test stays byte-identical.
2. **What it redraws** with the colony up (nothing over it, not in the planet info mode), as the starmap HUD's console
   plates in the empire colour (ADR 0065), following the shared UI scale:
   - top: the starmap HUD's bar (pause and speeds, date, money, the message line, the research under way and the
     screens as icons, the colony's lit);
   - top left, the build menu (EN's planet at peace, as hot-spot 6): the current kind's name with its price, energy and
     workers, the build cursor's toggle (the pick list's short press) and "Build" (hot-spot 0x13); the four groups
     (`Epulet_Terulet_Nev`) as tabs, the current kind's first; each offered building with the planet's count, its price,
     dim and dead where the original's colour 1 or 0xc marks it (no working hub, the tutorial's lock) or while build
     orders are on their way; a click is the held pick list's release on that entry (the kind, the cursor up);
   - bottom left, the short info (hot-spot 4): the selected units (count or design, empire, weapon), else the selected
     building (name, working and efficiency, product with its icon, the damage and construction bars), else the
     planet's short info; on EN's planet at peace the workers' and energy surplus; a left click is the planet info
     mode, a right click the starmap, both buttons with nothing selected the planet's rename (typed into the title);
     under it the icons: details, starmap, and the buttons that are on: repair, demolish, on/off (lit while working),
     self-destruct;
   - beside it, the repair or demolish window as a small plate: the question, the repair's price, Yes and No;
   - bottom centre, in a ground war, the war's buttons that are on (air strike and laser strike with their charge
     under them, detonator, devastator, mines, retreat, cancel order), either button presses as in the original, and
     the sides on the field with their units;
   - under the bar, centred: the attack banner (fading as the original's), the retreat and landing lines, the
     self-destruct's countdown with its cancel button;
   - bottom right, the radar: the buildings, the units (own green, the rest red) and the view's outline; a press glides
     the camera there, a drag follows.
   Every button has a hover state and the original's tooltip.
3. **Same functions**: `ColonyUi::modern_act` runs the original's code for each act: a button is pressed at its
   hot-spot's centre (so its enabled test, `TutorialPaused`, the tutorial's lock, the click sound and the war buttons'
   act-on-press are the original's), the short info is pressed and released at hot-spot 4 with the button the player
   used, a pick is the pick list's release, Yes and No are the window's rows. The bar's acts are the starmap HUD's
   (`bar_act`). The keyboard is untouched (E, I, PageUp/PageDown, S, a, 4..9, Ctrl+4..9, the camera keys).
4. **Input**: egui gets the pointer while the panels are up; a press over a panel is the panel's, anywhere else it
   reaches the 3D view as before. While they are up the original's positional hot-spots are off except the view (0x14)
   and so are the war's bottom-row buttons, so a click on open ground where they were selects or builds.
5. **Unchanged**: the 3D view with its building names, selection marks, the build cursor's model, the units' group
   sizes and the selection box; the planet info mode, which shows the original's panels, long info and menus while it
   is on (a later part may redraw it). The planet panel's turning building and the small turning model beside the
   short info are not shown in the modern panels.
6. Ours: the big ships' charge bar fills at 300, the timer's full value; the original's bar divides by 200 and runs
   past its cell (re/screens/colony.md §11.1, [original quirk]).

## Consequences
- The original's colony panels stay and draw whenever the option is off, and in the planet info mode.
- No exe capture exists for the new look: screenshots go with the change (`--snapshot --modern-colony`).
- `starmap_modern::bar` and `ui::Hud::bar_view` are the shared top bar for the next modern screens.
