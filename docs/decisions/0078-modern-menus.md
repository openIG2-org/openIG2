# ADR 0078: the menus and options in the design's layout

Date: 2026-11-03. Status: accepted. Amends ADR 0062 (the Options page's tabs and layout), ADR 0066 (the multiplayer
pages' layout) and ADR 0067 (the in-game menu: a pause menu with its own save and load page). Implemented by `ig2-client::options::modern` (the Options page,
the pause menu, the save and load page), `gui::board` (a board's page: a deep panel over the dimmed screen) and the
design kit (ADR 0073: `ds::MenuEntry::hint`, `ds::option_row_in`).

## Context
The design system's screen boards (1920 × 1080 at 100 % UI scale, a design pixel a point) include the menus: the main
menu, a new game, the multiplayer browser and room, five Options pages, the in-game menu, load and save, help and the
end of a game. The modern Options page (ADR 0062) had eight tabs of settings with a description under each; the boards
group the same settings into five tabs of cards side by side, with the description of the row under the pointer or
the keyboard in the footer. The in-game menu (ADR 0067) was the Options page with a first Save & load tab; the boards
have a small pause menu (Continue, Save, Load, Options, Help, Quit, Exit) with save and load as a page of its own.

## Decision
1. **Options** (the five Options boards): a 1640 × 920 deep panel (28 × 32 inside) over the screen dimmed by half;
   the page title, a 280-point rail of five tabs (`ds::menu`, `MenuStyle::RAIL`: Display, Accessibility, Controls,
   Sound, Game), the tab's cards in three columns (Sound two), each card to the shortest column, the columns scrolling
   together when a tab is taller than the panel; the footer a rim line, the status and the 180-point Back and Apply
   plates (Controls adds Original keys). The cards: Display — Screen (mode, resolution, vertical sync, letterbox, live
   background, gamma), Graphics (the modern lighting, battle effects, texture upscaling), Interface (the theme, the
   modern screens, cursors, sharp UI and the interface rows), Detail (the exe's graphics rows); Accessibility —
   Readability (UI scale, sharp text, slider under label, clean name entry), Subtitles and colours, Language;
   Controls — the keys by where they act; Sound — Volume (music, sound, voice), Music; Game — Battles and saving (the
   exe's game rows, autosave), Enhancements, Handling (camera, fleet groups, event log, menu navigation, the player
   name). A row the table doesn't place goes by its group, so every setting stays on exactly one tab.
2. **Rows** (`.orow`, 50 high; a key's `.krow` 40): the label at the left, fitted (shrinks up to 3 points, then "…"
   with a tooltip), the control at the right: a two-word toggle for on/off, segments for two to four values (three or
   four long ones under the label across the card, `.orow.wide`), a stepper for more (resolution, language, voice),
   a slider for the volumes, gamma and UI scale, a recessed field for the name, a key cap. The status line shows what
   is wanted (the new key, a shared key, the language's restart) or the description of the row under the pointer, else
   of the focused row: one line for the page, not one per row.
3. **The pause menu** (the in-game menu board): Esc or `O` in a game opens a 520-point panel: PAUSE, the newest save
   (how long ago, its name), Continue (Esc), Save, Load, Options, Help (their keys at the right, from the bindings), a
   rim line, Quit to main menu, Exit game; under the tutorial Continue tutorial, Options, Quit the tutorial. Save and
   Load open the save and load page (Save with the name entry when the slot takes a save), Options the settings, Help
   closes the menu and opens the help screen over the game (`Act::Help`); Back or Esc on a page returns to the menu,
   Esc on the menu continues the game. Quit and Exit ask in the kit's confirm box.
4. **Save and load** (the load and save board): a 1440 × 880 panel: the slots as a table (#, name, game date in the
   LCD face, how long ago), the autosaves (24, 23 when filled, "A") above a rim line, then slots 1–22; the selected
   save's card (name, game date, slot, saved) and the name card (the field, Save and Load, the overwrite hint) at the
   right; the footer's hint (why Save or Load is off, the entry's keys) and Back. A click selects, a double click loads,
   a click on the field starts the name entry. The main menu's Load game opens this page alone (Back to the menu).
   The original's gates stay (ADR 0067 §3).
5. **Multiplayer** (the browser and room boards): full-screen pages over the main menu's scene dimmed by 45 %, laid
   out from the screen's edges (a 96-point margin at 1920, a 584-point right column), so 16:9, 21:9 and 4:3 keep the
   board's arrangement. The browser: the title (the ring's name) and "Playing as", the lists' panel with the LAN /
   internet / saved games tabs in a recessed well and Refresh, the games as a table (game, host or scenario, players
   in the LCD face, status tags: started, incompatible, password), the empty states centred; the Join directly panel
   (address or join code, password, Join) and the relay server's panel at the right; Back and Host at the bottom right
   with the status at their left. The room: the room's name with its tags and the ready count; the seats panel (46-point
   seats: the empire's stripe, the race's head, the race and holder drop-downs, ready / host / you tags, Take seat
   under the pointer, your seat lit); the galaxy's settings four a row with steppers; the host's Connection panel (the
   join code in an LCD window with Copy, port mapping, private and public list as toggles with their lines under);
   the chat; Leave and Ready or Start. Same acts, keys and fields as before.
6. **The main menu** (the main menu board): a Remake option, "Modern main menu" (`ModernMainMenu`, Interface, on by
   default; `--modern-menu` in snapshots; off = the original's rings), draws the grouped menu of ADR 0059 (it turns the
   grouped panel on with it) as an egui page over the game's own scene: IMPERIUM GALACTICA II and OPENIG2 at the top
   left, a deep panel of 48-point entries (`ds::menu`) in groups (Play: Continue with the newest save's name and game
   date under it, Quick game, New game, Load, Multiplayer, Tutorial; Extras: History, Replays, Mods, Credits; Settings,
   Quit), a sub-menu's entries under its name with Back (Esc), the version at the bottom right. The entries act as the
   panel's (`MainScr::act_item`); the old panel's boxes and Back label are off while it is up. At a large UI scale the
   entries get lower (to 36 points) and the panel rises over the title's place, so it always fits.
7. **The kit** gains two small generic pieces: a menu entry's key hint (`MenuEntry::hint`) and a settings row of any
   height and label style (`option_row_in`); option rows' and segments' labels shrink before they elide.

8. **The new game page** (`newgame_modern.rs`; with "Modern main menu"): the campaign's race choice (Mode_Fajvalasztas,
   mainscr.md §9.2) as an egui page: Campaign and Scenarios at the left with a hint, the three races (Solarian, Shinari,
   Kra'hen = races 1, 3, 5) as round portraits (the diplomacy screen's 128 × 128 heads), the race's panel (the empire's
   name, the fajinfo blurb, Difficulty as large segments Easy / Normal / Hard = the exe's levels 0 / 2 / 1), Back and
   Start. One click picks, Start runs the campaign start, Esc, Back and the right button close at once; the 3D heads and
   the blurb are not drawn under it. The kit's `ds::segmented_plates` draws lit plates of a given height.
9. **The help page** (`help_modern.rs`; with "Modern in-game menu"): the Help screen (AktKep 1) as a page under the top
   bar: the topics of helpmain.txt with a search field (it types while focused; the F-keys pass), the open topic's
   sub-topics (a click scrolls to the section), Back; the topic file's markup read into title, paragraphs, framed
   pictures (2.22 × up to 766 wide), term lists and section headings, with a scroll track; the wheel scrolls (the
   original ignores it); the original's opening and closing frame fades it; the original's tooltips are gone.
10. **The end-of-game page** (`stats_screen/modern.rs`; with "Modern main menu"): the statistics screen (ADR 0034) as a
   page: the outcome title (own, enemy, silver), mode, days and end date, the empires table (colour stripe from the
   palette, race head, LCD values, score in LCD yellow, your row lit, a beaten empire muted), the graph with segment
   tabs, grid, day marks and line names; the match history as a table that scrolls instead of Newer and Older pages.
11. **The name prompt**: the multiplayer name entry (game question 15/3/2) is a modern prompt with a field, Back and OK.
12. **Alignment with the boards**: the main menu's entries at the board's 52-point spacing; the pause menu's "Resume"
   and last-save line; the save and load page 12 points lower (below the top bar); settings cards' headings 18 high,
   wide rows 80 high, the boards' row order inside cards (UI scale, language, theme, autosave cadence, WASD first;
   texture upscaling and sharp UI last); the global keys split into "Everywhere: screens" and "Everywhere: time and
   saving"; volume rows with mute buttons and longer tracks (`slider_sized`, `Icon::Speaker`, `Icon::SpeakerOff`);
   multiplayer panel titles as written; the help topic list scrolls; the new game and end pages fit a large UI scale.
   Staying different: the race traits table (the game has no such data), the classic top bar on help and statistics,
   the room's name "<name>'s galaxy" in English (shared over the network), the mods line under the galaxy settings,
   the load list's scroll bar, the autosave toggle beside the cadence row.

## Consequences
- The Options page and the in-game menu look as the boards at 1920 × 1080 and keep their functions, keys and storage;
  the classic screens are unchanged (their capture comparisons don't move).
- The per-row descriptions move into the footer's status line.
- Not drawn yet: the colour-blind preview strip.
