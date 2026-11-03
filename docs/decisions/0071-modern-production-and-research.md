# ADR 0071: the modern production view and research screen

Date: 2026-11-03. Status: accepted (the production and research screens of the "Modern versions of every original
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
