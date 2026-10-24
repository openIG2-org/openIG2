# ADR 0015: The game runs on, visibly, behind the overlay screens

Date: 2026-10-24. Status: accepted (a maintainer's request of 2026-10-24). A deliberate deviation from the classic
client (ADR 0009), with the original's look kept as an option. Spec: `re/screens/overlay-background.md`. Implemented
by `ig2-client::backdrop` and `window.rs`.

## Context
Research, design, diplomacy, spy, trade, info, messages, options and help open over the game. ig2.exe doesn't draw
the starmap or colony behind them: on the way out the screen left takes a screenshot (`SaveScreen`, exe 0x524700),
and the new screen lays it, dimmed to about half, under its own scene (`Copy3hatterkep`, exe 0x4cb690). The picture
is frozen while the game itself runs on (the date advances behind research or info; messages and options pause the
clock, as they always did). The old machines couldn't afford to draw both. The client drew black there, because the
reference captures jumped to the screens from the main menu, where the exe has no picture to draw.

The project wants a running game behind these screens on modern machines.

## Decision
1. **Option "live background", on by default.** The starmap or colony underneath is drawn anew every frame (its
   scene, without its HUD) and dimmed with each screen's own grey, the same values the original uses for its
   picture. Fleets move, planets turn, the battle zones pulse behind the screen.
2. **Classic mode** (`LiveBackground` 0 in the remake's settings file, or `--background classic`): the exe's
   pictures as the exe takes them. 0x34 is the starmap without its HUD at every starmap exit; 0x35 is the screen
   before, for messages, help and options. This includes the original's quirks: from the colony, research draws
   the last starmap picture, or black if the starmap was never left.
3. Where the original's picture shows another overlay screen (options or messages opened over research or info),
   both modes show that screen as it was: there is no game underneath to draw.
4. The simulation rules don't change in either mode. The sim runs, or stands still, behind each screen as in the
   original: research, design, diplomacy, spy and trade stop it only while they animate open or closed, and
   messages and options pause the clock. Multiplayer ignores those pauses, as before.
5. Acceptance compares classic mode: `run_screendiff.py <screen> --via-starmap F` makes the exe leave the starmap
   first, and the client draws the same picture (`--snapshot --backdrop`). The bare-jump comparisons stay as they
   were (black behind).

## Consequences
- Cost: in live mode an overlay frame draws the starmap or colony scene once more, the same work as being on that
  screen. Classic mode draws one full-screen quad. The profile numbers stay within the 2–4 ms worst-frame budget
  (overlay-background.md §4 lists the measurement).
- The overlays' own translucent parts (trade's list panels, the screens' dark grids) now show over a lit picture.
  Differences the black background hid become visible and are recorded in `re/deviations.md`.
- The option is read once at start-up. The Options screen has no row for it yet (a remake row, styled like the
  original's, can follow).
