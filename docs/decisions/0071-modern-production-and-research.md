# ADR 0071: the modern production view and research screen

Date: 2026-11-03. Status: accepted; amended below (the design kit) (the production and research screens of the "Modern versions of every original
screen" package, `docs/ENHANCEMENTS.md` → UI). Implemented by `ig2-client::production_modern` and
`ig2-client::research_modern` (the panels and their acts), `ui::Hud::production_modern_view` and
`research_modern_view` (what they show, read from the state `StarmapGfx2D_3` and `ResearchGfx2D_2` read),
`starmap_panels::Panels::{modern, modern_pick}` and `research::Research::modern_act` (the acts through the original's
code), and the window (`window.rs`: `production_modern_up`, `research_modern_up`, `production_modern_apply`,
`research_modern_apply`); `--snapshot --modern-hud --modern-research --panel production` and `--snapshot --screen
research --modern-research` draw them headless.

## Context
The decision of 2026-11-01 (ADR 0065's package) gives every original screen a modern egui version close to the
original's layout, functions and keys, with the modern starmap HUD's look. The production view is a starmap panel
(re/screens/starmap.md §12): the `gyartas.xvb` console with two build lines, four slots each, count buttons that
repeat while held, repeat and "into a fleet" lamps and a design list held open under the pointer. The research
screen (re/screens/research.md) is a 3D console: three wheels of category buttons, two lists of the category's techs,
an info box, the lab dials, start/stop, the urgent button and the running project's box. Both are ported hot-spot by
hot-spot and stay as they are.

## Decision
1. **An option**, "Modern research screen" (`ModernResearch`, Remake page, Interface, row 43), **on by default**; it
   covers the research screen and, under the modern starmap HUD, the production view; off = the original's. Snapshots
   draw the original's unless `--modern-research` is given, and the input scripts always click the original's
   hot-spots, so every capture and replay test stays byte-identical.
2. **The production view** (with the modern HUD up): the planet's plate under the bar (previous and next planet
   0xc/0xd, the planet list 0x32 opened on the press and picked on the release, the exit 9 under the tutorial's lock),
   and a plate per line, ships left and vehicles right: the capacity (or "none"), the head unit's bar, the four slots
   with the design's name and price (a click opens the slot's design list beside it: design 0 removes, a ninth
   station is dead), the count with − and + (none for stations), the repeat lamp, the vehicles' "into a fleet" lamp.
   The head slot is the one that builds; the original has no other priority control.
3. **The research screen**: the starmap HUD's bar over the screen's backdrop, and three plates: the twelve categories
   (the wheels' buttons, by their tooltips' names, with known and owned counts and the running project's mark); the
   category's known techs in their status (owned, researching, researchable, seen) with the per cent; the shown tech's
   info lines, prerequisites, labs needed against the player's, days and daily cost, start or stop, the running project
   with its money and days left (a click shows it, hot-spot 0x19), the urgent button (double money, double speed: the
   original's only money control) and the three auto-research switches (§5.8). The screen opens without the 3D
   console's animation (as from the options screen).
4. **Same functions**: the production acts run the panel's code by hot-spot id (`press_id`); a held count button is
   the original's held hot-spot at the half that lowers or raises (`count_spot`), so the repeat's pace is the
   original's (the first frame, then from the seventh 60 Hz frame, up to 5 at a time); a design pick is the list
   opened at its hot-spot and released on the design (`modern_pick`), so the station limit, the same-design check, the
   click and the 0x1d(4) are the release's. The research acts are `TT_*`, `TalalmanyKivalasztva`, `ResearchStartStop`,
   `ResearchDoubleGomb`, the 0x19 jump and the switches' order (`modern_act`). The orders are the classic screens',
   so the lockstep is unchanged. The keys are untouched (the research screen's arrows, Enter, PgUp/PgDn; Esc).
5. **Input**: egui gets the pointer while the panels are up; under the modern production view the original's
   positional hot-spots 0xc..=0x36 and 9 are off (the panels press them by id), the info box's 8, 10 and 11 stay the
   HUD card's; under the modern research screen the mask's hot-spots, the ball and the models take no presses.
6. **Not shown**: the production view's 3D console and head-unit models, the research screen's console, ball, models,
   lasers and button lines. The transfer, fleet and vehicle panels stay the original's.

## Consequences
- The original's production view and research screen stay and draw whenever the option (or the modern HUD, for the
  production view) is off.
- No exe capture exists for the new look: screenshots go with the change.
- The research screen's opening and closing sounds are not played in the modern screen (it has no animation).

## Amendment: the design kit (2026-11-03)
The production view and the research screen are redrawn with the design kit (ADR 0073) after the production and
research boards (1920 × 1080 at 100 % UI scale, one design pixel one point; 4:3 and 21:9 keep working):
- **Production view**: the planet on the kit's title plate between the previous key and the next and planet-list
  keys; per line a 600-wide glass panel (title, capacity on an LCD, "none" dimmed) with four 90-high slots: the
  slot's number, a 128 × 72 picture well, the design's name, its hull and its price, the head unit's bar with its
  per cent, the count on an LCD between round −/+ buttons, the repeat and "into a fleet" lamp plates; an empty slot
  says so and has a "+ Blueprint" plate. Each queued design's hull turns in its well
  (`starmap_panels::PanelModels::render_thumbs`: one cell per slot of a window-sized texture, as the head units are
  posed and lit, drawn into the wells in the window's top frame). The design list is a deep panel under its slot
  with each design's unit price, the slot's design selected, a ninth station muted. Exit is the board's plate at
  the bottom right. The panels slide in from their sides; under 4:3 the panels go under the plate.
- **Research screen**: the category's techs (top left, deep panel) with their status at the right (done, the bar
  and per cent, the cost, locked); the auto-research switches as settings rows under it; the shown tech at the
  right (name, lines, prerequisites with the owned ones in info blue, labs red when short, days), the funding as
  two plates (normal, double money: the urgent button), start or stop, the running project with its jump; the
  twelve categories as tiles in three groups at the bottom (ships and weapons with the special one, buildings,
  vehicles), each with owned / known and a progress line; an exit plate (the top bar's starmap pick). The original's
  wire sphere, ball and tech models turn in the middle (`research::ResearchModels::modern` leaves the console out).
- **Input**: the buttons, lamps, keys, tiles and plates act on the press of either button, as the original's
  latched hot-spots; a held count keeps repeating with the pointer off it until the button is up.
- `--snapshot --stage-queue` gives the snapshot's planet a production queue (a still of the filled view).

## Amended 2026-11-03: board compare pass
Compared with the boards at 1920 × 1080 (Hungarian and English; `--research-pick A,F` shows category A's field F in the research snapshot) and fixed: the research tech's stat pair ("Firepower: 25") is a table row; table labels shrink to their column; the funding plates' labels are upper case; the category tiles' columns are sized by their content; count buttons 8 pt apart and lamps 26 high; the exit plate says "Exit". The kit's controls and the rows, tiles, slots and lamps show the hand pointer, and under the modern bar the classic top bar's tooltips are off and the pointer comes from the modern panels. Not done, on purpose:
- Slot priority up/down: the original's queue order (0x1d) has no such act.
- "Into fleet" lamp on ship lines (the original has it for vehicles only) and the price per unit (the original shows count × price).
- Locked techs last after a gap (the original keeps field order); category counts on upright digits (the kit's LCD text slants).
- The planet card is the starmap HUD's own, with its extra rows and buttons; the live background behind the research screen is the live-background option's.
