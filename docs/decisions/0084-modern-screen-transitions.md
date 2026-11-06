# ADR 0084: the modern diplomacy, spy and trade screens open and close on their own

Date: 2026-11-06. Status: accepted. Amends ADR 0076 (its "Kept 3D" part: the modern pages no longer run over the
original's opening and closing). Implemented by `ig2-client::gui::transition` (`Transition`), `gui::ds::entering`
(the panels' entrance in `ds::place`) and the screens' `modern` fields: `diplomacy::Screen::modern`,
`spy_screen::SpyScreen::modern`, `trade::Trade::modern`.

## Context
ADR 0076 drew the modern diplomacy, spy and trade pages over the classic scenes and only hid parts of them, so with
Interface = Modern the original's animations still played: the diplomacy ring opening and the leader's frame
turning in, the spy screen's telescopes opening and closing (the screen went only once they were shut), the trade
frame's opening and the hull sliding in. The modern look wants its own short transitions, and the game's renders
(leader video, spy figures, trade hull) inside the modern frames without the camera and frame moves around them.

## Decision
1. **One helper.** `Transition` counts a modern screen's opening (30 UI ticks, 0.3 s) and closing (18 ticks) in the
   screens' own 10 ms ticks. Its `panels` part eases the pages' panels in: every panel `ds::place`d under
   `ds::entering(t, …)` fades in and slides 48 points in from the side of the screen it stands on (left or right,
   else from above or below); the top bar stays. Its `renders` part starts later (at 30 % of the opening) and ends
   earlier on closing: `Transition::veil` lays the screen's backdrop picture (black without one) over the 3D drawn so
   far, so the leader, the spies and the hull appear in their wells. With reduced motion (ADR 0073) both are at once.
2. **Interface = Modern** enters the three screens without the original's opening (`animated` off: the scene at
   rest, frame 80 / 50 / 50) and with `Transition::opening`; leaving starts the transition's closing and the screen
   goes when it has run out (`closed`), never waiting for the original's closing. Towards the messages, help and
   options screens the screens still go at once, as in the original. While closing the page takes no presses and
   counts as a screen change (`screen_changing`). The original's opening and closing sounds go with their
   animations and are not played.
3. **Diplomacy**: the leader's ring and video rest facing the camera (no turn, no push towards the camera); a newly
   chosen leader fades in over the first third of the original's turn time instead.
4. **Spy**: the telescopes stay open (frame 50); the head disc shows at once.
5. **Trade**: the frame and wire sphere stay hidden as before; the hull turns in its well.
6. **Interface = Classic** is unchanged: every screen plays the original's animations, and snapshots, captures and
   replay tests draw the original's (the transitions only run in the live window).
7. **The main menu's pages** (the menu and its sub-menus, new game, the name prompt, Settings and Load, the end-of-game
   statistics, replays, mods, multiplayer) open and close with the same `Transition`, page after page: a pick waits for its
   page to close (`App::after_page`, the click heard at once), then the next page opens. Their own areas slide as the
   placed panels do (`transition::slide_areas`) and the whole page fades (`Gui::alpha`); while closing a page takes
   no input. The menu itself goes on under them (ADR 0078): back from a page only the page's transition plays.

## Consequences
- The modern screens open and close in about a third of a second instead of the original's one to three.
- A spy changing under the cursor still flips the head disc as the original does (it is the selection's change, not
  the screen's opening).
- No exe capture exists for the transitions; the unit test `gui::transition::tests::opens_and_closes` holds the
  timing.

## Amendment: research, design, info and messages
The same transition on the modern research, design, info and messages screens (`research::Research::modern`,
`design::Design::transition`, `info_screen::InfoScreen::modern` and `leave`, `messages_screen::MessagesScreen::modern`):
each opens with it and leaves once its closing has run out (the info screen, which the original leaves at once, waits
for it too). Their pages' anchored areas take the entrance through `gui::ds::entrance`; the research and design 3D and
the talking head's clip appear with the renders. The messages screen's own timing (`mesframe`) still runs under the
page, so the text starts typing at the original's moment.
