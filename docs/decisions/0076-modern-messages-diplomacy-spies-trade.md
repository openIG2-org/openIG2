# ADR 0076: modern messages, diplomacy, spy and trade screens

Date: 2026-11-03. Status: accepted for the messages screen; diplomacy, spies and trade follow under the same rules.
Part of the "Modern versions of every original screen" package (`docs/ENHANCEMENTS.md` → UI), drawn with the design
kit (ADR 0073) after the design boards (messages, diplomacy, spies, trade; 1920 × 1080 at 100 % UI scale, one board
pixel one point). The info screen's boards amend ADR 0072. Implemented by `ig2-client::messages_modern` (the page,
its view), `messages_screen::Dialogue::pieces` (the typed text as coloured pieces), `ui::Hud::modern_messages`
(the classic frame, inbox and text not drawn) and the window (`window.rs`: `messages_modern_up`,
`messages_modern_apply`, the clip in the head's well); `--snapshot --screen messages --modern-messages` draws it.

## Context
The messages screen (re/screens/messages.md) is a 3D frame with a talking head's clip, an inbox of four rows with
two arrows, the open message typed out at the part's speed with its colour codes, up to four answers under it and
hot-spots around the frame that leave the screen. Its texts sit in a 640 × 480 box of fixed cells (33 characters a
line). The board puts the same parts in glass panels: the inbox left, the head in a cast-metal frame, the message's
card beside it, the text wrapped in a wide panel under them, the "full text" and exit plates at the foot.

## Decision
1. **An option per screen**, on by default (Remake page, Interface): "Modern messages screen" (`ModernMessages`);
   off = the original's screen. Snapshots and capture comparisons draw the original's unless `--modern-messages`
   is given, so every capture and replay test stays byte-identical.
2. **Layout** (anchored to the sides; 4:3, 16:9, 21:9 and a larger UI scale keep working): under the bar, left, the
   inbox panel (540 wide): its title, the new and total counts, the newer and older arrows as round keys (hot-spots
   1 and 2, live only when the original's would move the list), the original's four rows from `AktUzenetPointer`
   (the sender in LCD yellow while unread, silver once read, the date, the two subject lines; the open one
   selected), the colour hint; right of it the head's well (720 × 405, 16:9) where the window draws the talking
   head's clip and its interlace, and the message's card (subject, sender, date; the answers as plates with their
   key `A`..`D`, a disabled one dimmed); under them the text panel: the message typed so far as wrapped body text
   (19/32) in the original's colours, the newest lines kept in view, the typer's caret blinking at its end; at the
   foot "Full text" (types the rest at once) and Exit.
3. **Same functions**: each act is the original's press at its hot-spot's virtual point (`screen_press_at`): a row
   the quarter of spot 3 that holds it, the arrows spots 1 and 2 (their debounce and the sim's `Scroll` orders), the
   text a press in the text box (`hurry`), Exit spot 7 (live only when the original's 7..=9 are: no answer pending
   and the text typed); an answer is its key (`a`..`d`, as `MessagesScreen::key`). The keyboard is untouched.
4. **Input**: egui takes the pointer on the whole screen while the page is up; the classic hot-spots get no hover
   or press. Every part takes either button (as the original's spots); a disabled answer is still asked for and
   stays pending (`ValaszKivalasztva`); the event log's press path is off under the page.
5. **Text**: every label goes through `ds::Text` in a fixed box (test `panels_and_presses`: long texts never move a
   panel); the body is re-wrapped from the original's lines (`Dialogue::pieces`: a line break inside a paragraph is
   a space, an empty line a paragraph break, the `|C` and `~` codes colours). New remake texts: "new", "Sender",
   "Full text", "Yellow: unread".

## Consequences
- The event log's label and panel (ADR 0040) are not on the modern messages page; switch the option off to use it.
- The original's frame animation (opening, closing) isn't drawn; the screen's timing (`mesframe`) still runs, so
  the text appears at the same moment.
- No exe capture exists for the new look: snapshots go with the change.
