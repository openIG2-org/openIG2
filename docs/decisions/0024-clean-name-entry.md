# ADR 0024: The save name entry hides the slot's old name

Date: 2026-10-31. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
look kept as an option. Spec: `re/screens/options.md` §5.2 and `Gfx2D_2` step 6. Implemented by `ig2-client::options`
and `ig2-client::display`.

## Context
Saving over a used slot opens a name entry on the slot's line with the slot's name in it; the first key clears the
text (`IOGombnyomasraTorles`), so the save gets exactly what was typed. But `OptionsGfx2D_2` (ELF 0x5d71c0) draws the
slot list first, the old name included, and then the entry's box only as wide as the typed text (colour 0x27 until
the first key, then 0). Typing "asd" over "Vaig Elment" shows "asdaig Elment" until Enter: the field looks as if it
overwrote the old name character by character, and players delete the "tail" by hand.

## Decision
1. **Option "Clean name entry", on by default.** While the name entry is up, the slot list leaves out the edited
   slot's old name (or its dashes): only the entry's own text shows on that line. The dates stay.
2. **Classic mode** (`CleanNameEntry` 0 in the remake's settings file, or the Remake row on the Options screen's game
   page): the exe's drawing, old name under the entry.
3. Nothing else changes: the entry's text, cursor, keys and what is saved are the original's in both modes.

## Consequences
- Capture comparisons of the Options screen with an open entry use classic mode (a snapshot has no remake rows, so it
  draws classic).
- The fleet and planet renames on the starmap and the colony screen edit the name in place and draw it alone, so they
  need no such option.
