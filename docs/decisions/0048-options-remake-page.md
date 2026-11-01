# ADR 0048: the Options screen's Remake page

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::options` (the tab, the page's layout) and
`ig2-client::display` (the rows and their groups). Supersedes where ADRs 0030–0046 place their rows ("a Remake row on
the game page").

## Context
Since ADR 0031 the remake's own options (display mode, language, autosave, subtitles, …) were drawn as extra rows
under the original's seven on the game page, below a "Remake" heading. Twenty-three rows later the list runs past
the bottom of the left panel (the panel ends at y 446, the rows step 15 from y 92: 23 lines fit), so the newest
rows can't be reached, and the game page no longer looks like the exe's. The decision of 2026-11-01
(`docs/ENHANCEMENTS.md` → Decisions) keeps the in-game screen's original look and tidies the remake rows; a modern
main-menu Options screen with tabs follows separately.

## Decision
1. **A third tab, "Remake"**, beside the original's sound and game tabs, drawn like them: a fader box
   (13 + 2 × 95 = 203, 66, 90, 20) with the tabs' fader colour, the label in font 3 centred at x 248, y 70 in colour 2
   under the pointer or on its page, else 1; hot-spot id 12 (below the panel's 17, so it wins), its press sets
   page 3 (`OptionsMelyikMod` 3, kept in the remake's own settings file only), its tooltip "The remake's own
   settings". No sliders on page 3.
2. **The original pages are the exe's again**: pages 0–2 hold only the exe's rows at the exe's places.
3. **Groups**: each remake row names a group (display, interface, game, accessibility, multiplayer); the page draws
   the groups in that order, each under a heading line (colour 1, x 30), its rows in their declared order; an empty
   group has no heading.
4. **Sub-pages**: lines step 15 from y 92 like the game page's rows; 21 lines a sub-page (a heading never ends one,
   it moves to the next; a group carried over repeats its heading on the next sub-page), line 21 the "Restart to change the language" notice while it applies, line 22 a "Page"
   row with the value "n/m" when there is more than one sub-page; a press on it shows the next sub-page (after the
   last, the first), as a press on a row steps its value. Rows react as on the other pages (`y..y+15`,
   `31 ≤ x ≤ 279`, colours 0x22/0x25, 2 hovered, 1 disabled).
5. **Texts** through the remake text table (ADR 0042): the tab, the group headings, "Page".

## Rejected
- **Scrolling the rows with the wheel**: the original screen has no scroll anywhere; a page row reads like the
  rest and works with the mouse alone.
- **Shrinking the line height to fit**: the font and row height are the exe's; future rows would overflow again.
- **One tab per group**: the panel holds three tabs of the original's width, not seven.
