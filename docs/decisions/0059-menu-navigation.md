# ADR 0059: Enhanced menu navigation

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::menu_flow` (the main menu's grouped panel, the Back
label, the newest save), `mainscr` (`MainScr::flow`: the back paths) and the window's dispatch; the option is a
Remake row (ADR 0048) on the Interface tab (ADR 0053). The flow it changes is mapped in `re/screens/mainscr.md` §15.

## Context
The original's menus are rings: each step flies the ring set out and the next one in, a campaign is four clicks
away, a saved game three, and the only way back is a Back ring on some sets and Esc on most. The remake added its
own entries (Settings, Quick game, History, Replays) as loose labels in the corners. The enhancement list's
Menus idea asks for clearer navigation, fewer clicks to a game and a back path everywhere.

## Decision
1. **An option**, "Menu navigation" (`MenuNavigation`): Classic (0, the default) keeps the exe's flow and the
   remake's labels exactly; Enhanced (1) turns on 2–4. Apply changes it at once on the main menu.
2. **One panel in place of the rings**, at the left, replaces the rings and the four labels: on the main set
   *Play* (Continue, Quick game, New game, Load, Multiplayer, Tutorial), *Extras* (History, Replays, Credits),
   Settings, Quit; on a sub-set its rings' entries (New game: Campaign, Scenarios; Multiplayer: Local network,
   Internet; Tutorial: Parts 1-5) and over a confirmed race the difficulty rings' (Easy, Normal, Hard). Every entry
   does what its ring does. The rings, their labels and hot-spots are gone (amended 2026-11-01: two menus on top of
   each other read as two main menus); the background (galaxy, meteors, ships, camera) stays, a set changes at once
   instead of flying out and in, and the panel takes presses once the scene is half faded in (as the rings once in
   place). The panel is drawn with the UI theme (ADR 0056) when one is chosen, else in the Settings label's look, so
   Classic theme + Enhanced navigation still looks like the game.
3. **Continue** loads the newest save of the slot folder (slots, autosaves, the quick save) by its file time that reads, one
   click from the menu to the game; disabled when there is none.
4. **Back everywhere**: Esc and the right button on empty space step back one screen on every menu screen; a
   "Back" label shows where a screen has no back of its own (the race choice, the ring sub-sets, the name entry).
   Esc on the main set does nothing instead of the exe's fly-out and back. The high-score name entry keeps the
   exe's Enter-only rule (its score would be lost).
5. **Texts** come from the remake text table (ADR 0042), Hungarian or English.

## Consequences
- Main-menu capture comparisons run with the option off and are unchanged.
- A new menu screen of ours must give Esc and the right button a back step and, without a back spot of its own,
  show the Back label.
- Deviation row 472.
