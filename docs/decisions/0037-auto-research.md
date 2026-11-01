# ADR 0037: Auto research

Date: 2026-11-01. Status: accepted. An enhanced option (off = the original). Spec: `re/rules/production-research.md`
§2.5. Implemented by `ig2-sim::research::auto_pick`, `Game::auto_research` and `ResearchOrder::Auto`
(`ig2-net::ResearchCmd::Auto`); the switches by `ig2-client::research` (`AUTO`, `Research::auto_press`;
re/screens/research.md §5.8).

## Context
In the original a human picks every research project on the research screen, and an idle slot stays idle until
they come back; the computer empires pick their own with `FRISSIT_MI_FEJLESZTES` (exe 0x50b0b0) from a priority
byte per race in the tech table. In a long game the research screen becomes a chore, and an empty slot wastes
months.

## Decision
1. **Three switches per empire**: ships (the research screen's categories 1–5), buildings (6–9) and tanks (10–11).
   The special category (12, the story's techs) is never picked.
2. **Sim, not client**: the pick runs inside the research tick for an idle human-controlled empire with a switch on,
   so every peer computes the same project; the switches are set by a lockstep order (`ResearchOrder::Auto`) like
   the screen's own start and stop.
3. **The computer empires' choice**: the tech table's AI priority for the player's race, lowest non-zero first,
   priority 0 last, restricted to the switched-on categories and to what the labs allow; a priority tie goes to the
   cheaper tech, then the lower number.
4. A project the player starts by hand runs untouched; a stopped one is replaced on the next tick while a switch is
   on.
5. **Switches on the research screen**: three text toggles ("Auto ships", "Auto buildings", "Auto tanks", ON/OFF)
   in the empty strip under the left list panel, with English tooltips (the game has no strings for them); a press
   sends the order with all three. No Options rows: the switches belong to the empire in the running game (saved
   with it), not to the program's settings.
6. **Saved** in our save format (snapshot version 10, part of the state hash); an original save loads with all off.

## Consequences
- All switches off (the default) leaves the research tick as the original's: classic replays and saves stay
  byte-exact.
- Older saves of ours (versions 4–9) load with the switches off.
- Shipped as enhancement row 451 (`docs/ENHANCEMENTS.md`).
