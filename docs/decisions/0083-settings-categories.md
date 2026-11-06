# ADR 0083: The settings page's categories and dependent settings

Date: 2026-11-06. Status: accepted. Builds on ADR 0053 (the modern Options page), ADR 0078 (the boards' layout) and
ADR 0082 (one Interface setting). Implemented by `ig2-client::options::modern` (`Tab`, `place`, `note`, `child`,
`card_note`) and `ig2-client::display` (`parent`, `dependencies`). The classic Options screen keeps the original's
pages; its Remake rows follow the same dependencies.

## Context
The modern settings page had five tabs. The Display tab mixed the screen, the 3D graphics, the original's detail
rows and every interface tweak in one Interface card, whether it served the modern or the classic interface. Some
settings only work while another allows them (Resolution while not Borderless, the classic interface's tweaks while
the Classic interface is chosen, the grouped menu that the modern main menu always has), and nothing showed it.

## Decision
1. **Seven tabs, each with its own cards.** Display (Screen; Picture: live background, sharp UI), Graphics (Effects;
   Detail: texture upscaling first, then the original's detail rows), Interface (Interface: the Modern / Classic
   choice; Modern interface: the UI scale; Classic interface: models inside boxes, slider under
   label, centred Exit label, settings in menu, event log, menu navigation), Accessibility, Controls, Sound, Game.
   The UI theme is not part of the categories: the Modern interface keeps the design boards' colours and the theme
   setting is being retired.
2. **Dependencies in one table.** `display::parent` names a row's parent, the parent values that allow it and a
   short note. While the parent doesn't allow it the row is off (no input) and shows its note under it; a card whose
   every row is off shows the note once under its heading, a row with another note (menu navigation: the Modern
   interface always uses the grouped menu) keeps its own. A row whose parent is on the same card sits indented under
   it (Resolution under Display).
3. **Live on the page.** A change on the page re-runs the dependencies at once, before Apply, so choosing Classic
   lights the classic card and greys the modern one.

## Consequences
A setting added later with a parent gets its place and note through the table. The keyboard order follows the
cards; disabled rows keep their stored values.
