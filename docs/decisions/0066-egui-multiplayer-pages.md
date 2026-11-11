# ADR 0066: the multiplayer pages on egui

Date: 2026-11-01. Status: accepted (part 2 of ADR 0062); amended by ADR 0078 (the pages in the browser and room boards' layout). Implemented by `ig2-client::mp_lobby::page` (the pages),
`mp_lobby` (`MpScreen::act`, `apply`, `next_field`, `step_list`: the screens' logic, unchanged in what it sends) and
the window's frame and pointer routing (`window.rs`: `front_frame`, the "ADR 0062" block of `window_event`); the
snapshot's `--script` drives the pages too (`mainscr_scene::sequence`).

## Context
Our multiplayer screens (ADR 0014, 0018, 0020, 0033, 0041, 0060) were drawn in the scenario panel's style: boxes in
the 640 × 480 virtual pixels, the game's bitmap font, hot spots for every control, "<-" and "->" for the options, the
right button to cycle a slot. They worked but looked rough and hid what could be clicked. ADR 0062 put an egui layer
and a designed Options page into the client; the multiplayer pages are the second part.

## Decision
1. **Same screens, same logic, new pages.** The connection page and the room are egui pages (`gui::page`, `card`,
   `switch`, `primary`, `secondary` and `gui::style`, ADR 0062). A frame of a page returns `Act`s; `MpScreen::apply`
   carries them out with the calls the old hot spots made (`MpScreen::act` takes the old ids), so the lobby's
   messages, the room's rules and every test of them are unchanged.
2. **The connection page**: a heading with the player's name; the game lists as tabs (LAN games, Internet games,
   Saved games) with rows (name, address or scenario, players, badges: Password, Started, Incompatible), a selected
   row marked, a second click or a double click joining it (hosting a save); an empty list says why and what to do
   (no game on the network yet, no relay server, no public game, no saves); Refresh and a spinner while the relay
   server answers. A "Join directly" card holds the address or join code and the password, a "Relay server" card the
   relay with what it is for. The footer: Cancel, the status (a refusal, a connection error, Disconnected, a spinner
   while connecting), Host game (Host saved game) and Join, off while there is nothing to join.
3. **The room**: the name with badges (Host or Joined, Private, Password) and how many are ready; the eight slots as
   player rows: the empire's colour, the race's head (now an egui texture), the race as a drop-down for the host and
   the slot's holder, the holder (a name, or the host's drop-down of Open, Computer · Easy / Normal / Hard, Closed:
   the old right-click cycle and the level button in one control), badges (Host, Ready, Not ready, You) and "Take
   seat" on a free seat under the pointer; the galaxy's eight settings as a grid of steppers (‹ value ›, the arrows
   off at the ends, values only for joiners); the host's Connection card (port mapping, private game, public list as
   switches with their state under them, Copy join code with the code in its tooltip); the chat with the newest line
   at the bottom and its field; Leave, the status and Ready / Start (Start anyway).
4. **Input**: the pages take the pointer as the Options page does (ADR 0062 §4), the right button too: with the
   menu's navigation on (ADR 0059) a right click is Back, except while a drop-down is open (the click closes it); a
   left release still reaches the menu, as before. The keys stay ours: the text fields are our own (their keys,
   Ctrl+V and Ctrl+C as before; the exe's key filter lets Tab, Delete, Home and End through to the pages), Tab moves
   between the connection page's fields, Up and Down through its list (kept in view), Enter joins the selected row
   (hosts a saved game) or the typed address, Esc closes an open drop-down, else leaves the room, then the page.
   The old right-click shortcuts (previous race, the slot's holder cycle, the previous level) gave way to the
   drop-downs.
5. **Kept as they are**: the in-game wait overlay is the original's `MOUSE_DrawOver` (exe 0x4c95b0, `mp_wait`),
   ported with a capture comparison; it stays the original's screen (ADR 0009). The in-game multiplayer notices are
   lines on the message line, part of the starmap's HUD (ADR 0065).

## Consequences
- The pages scale and stay sharp like the Options page; at 4:3 they fit the screen's 720 points, wider windows give
  them their 760; at any UI scale each column scrolls when the page is too short, never running under the footer.
- New remake strings in `tr.rs`; the words the old page drew only (the "<-"/"->" rows, "Private game: yes/no",
  "Browse internet games") are gone.
- A page's look has no exe capture: screenshots go with the commit (`--snapshot --screen main --script`, offline: the
  host's room on a listener nobody reaches).
