# ADR 0067: the modern in-game menu

Date: 2026-11-01. Status: accepted (part 4 of the egui package, after ADR 0062, 0065 and 0066); amended by ADR 0078 (a pause menu first, save and load on their own page). Implemented by
`ig2-client::options::modern` (the Save & load tab, its keys and acts), `OptionsScreen` (the same gates and acts as
the original's screen) and the window (`window.rs`: `open_options`, `options_game_frame`'s egui frame, `options_act`);
`--snapshot --screen options --modern-menu` draws it headless.

## Context
The decision of 2026-11-01 (`docs/ENHANCEMENTS.md` → Decisions): the in-game Options screen was to keep the original's
look, with the remake rows tidied onto their own page. The main menu's Options page (ADR 0053 on egui, ADR 0062) and
the multiplayer pages are modern; the screen a player opens most, Esc or `O` in a game, was still the original's 640 ×
480 faders and bitmap text, with its settings split over three small pages. The original has no separate game menu:
Esc and `O` toggle this screen from every game screen (re/screens/options.md §5.1), and its Load, Save and Quit sit
beside the settings. So the in-game menu is this one screen.

## Decision
1. **An option**, "Modern in-game menu" (`ModernGameMenu`, Remake page, Interface), **on by default**; off = the
   original's screen. Snapshots and capture comparisons draw the original's unless `--modern-menu` is given, so every
   capture stays byte-identical.
2. **The main menu's page, with one more tab**: the in-game screen and the main menu's Load game open the same egui
   page as the main menu's Settings (the eight tabs, their settings, Apply and Back, the keys), with a first tab
   **Save & load**, the one shown when the page opens:
   - the slot list: the 24 slots with their number, name and game date; slot 23 is the previous autosave, 24 the
     autosave (ADR 0032); a click selects a slot (the original's hover alone doesn't), a double click loads it;
   - the selected slot's preview: its name, game date, kind and how long ago its file was written;
   - the actions: **Load** (a filled slot), **Save** (a name field over the preview with the slot's name or "Save
     Game nn", at most 32 letters; Enter or Save writes, Esc cancels), **Quit to main menu** and **Exit game**, each
     asking first (the original's quit question; Exit is ours, the window's close does the same without asking).
3. **The same gates as the original's screen** (`OptionsScreen::spot_on`): Load needs a filled slot and is off in
   multiplayer; Save is off when loading only, in a battle without `DEBUGX`, in multiplayer except for the host
   (ADR 0007's MP saves) and on slots 23 and 24; under the tutorial the slots, Load and Save are gone and the actions
   are "Continue tutorial" (back) and Quit (to the main menu without the question, as `Options_Kilep`'s
   `TutorialStop(1)`); from the main menu's Load game only Load is offered, and Back returns to the menu. A
   save leaves the screen except over the space battle (`Options_EnterPressed`'s dead `0xb` arm), as before.
4. **Settings**: as on the main menu, changes wait for Apply and Back takes them back; Load, Quit and Exit also take
   back what wasn't applied. The volumes are heard as they are dragged.
5. **Keys**: the page's own (ADR 0062 §4): Tab and Page Up/Down change the tab, the arrows move through the slots and
   the actions, Enter or Space acts (on a slot: Load if it is filled, else Save), Esc or `O` cancels the question or
   the name field, then leaves. Typing into the name field takes the letters (and Home, End, Delete); the slots don't
   take clicks meanwhile, as the original's.
6. **Leaving** is at once: the original's fade-out (`OptionsKilepesValtozo`) is the faders' look and the page has none.
   Nothing of the game is drawn behind the page (the original's black, its top bar left out under the page).

## Consequences
- With the option off, or in a snapshot without `--modern-menu`, the original's screen runs unchanged; its tests and
  capture comparison don't move.
- The page has no exe capture: screenshots go with the commit.
