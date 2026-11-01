# ADR 0042: the remake's own texts in Hungarian and English

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::tr` (the table) and the screens that draw the
remake's labels.

## Context
The remake adds rows, labels and notices the original never had: the Options screen's Remake rows and their values
(display mode, autosave cadence, …), the main menu's Settings, Quick game and History labels, the statistics and
match history screen, the messages screen's event log, the multiplayer lobby and room, the research screen's auto
research switches, the multiplayer notices on the message line, the quick save's notices. The install has no strings for them, so they were written in English
and a Hungarian game showed them in English.

The game's own texts come from the install (`dataraw/text`, the dialogue and message files) in its language's code
page (1250 for Hungarian, 1252 for the others) and stay so: the remake ships none of them.

## Decision
1. **One table in the engine source** (`ig2-client/src/tr.rs`): one row per remake label, `(English, Hungarian)`,
   keyed by the English text the code writes. Our own wording, never text taken from the game's files.
2. **Languages**: a Hungarian game shows the Hungarian, every other language the English (the remake has no German,
   French, Spanish or Italian wording of its own; a row for them can be added later the same way).
3. **Routing**: a label is translated where it is drawn, in the texts' language (`UiTexts::ours`, `tr::get`), then
   encoded in that language's code page like the game's texts. Rows and lines built before the language is known
   (the Options rows, the lobby's lines) keep their English and are translated at the draw (`tr::bytes`); a line
   made from a template ending in `{}` keeps its tail (an address, a system error). Templates with names or numbers
   inside go through `tr::fill`. Notices made where no texts are at hand (the multiplayer and quick save lines on
   the message line) use the install's language, set once when the client opens it (`tr::set_game`).
4. **Layout**: fixed fields keep their width (the Options value field holds 10 letters, the event log label 9, the
   statistics columns 7), so the Hungarian is written to fit; widths computed from a label's length (the event
   log's filter tabs, the lobby's relay field, the search field) follow the translated length.
5. **Tests**: every row has both languages, no key twice, every Hungarian letter is in code page 1250 and has a
   glyph in the Hungarian font (`charset.order_hu`, with the install); every literal passed to the routing calls,
   every lobby word and refusal, every Remake row, value label and event log filter, and every statistics label is
   in the table.

**A new label** joins with one row in the table (English and Hungarian, fitting its field) and is drawn through
`d.texts.ours("…")` (or `tr::get` / `tr::fill` with the screen's language); the coverage test finds it when it is
written as a literal in one of those calls.

## Not covered
- Literals the exe itself writes in English (the multiplayer winner and flag lines, the console's "No such player.",
  the fleet names "Colonization", "Terminator", …, "Tutorial END.", the colony's "Build" tooltip, the Options rows
  "Cutscenes" and "View Rotation" compiled into its row table, `re/screens/options.md` §6) stay English, as in the
  original's Hungarian version.
- The network layer's error details after a lobby line's prefix ("Can't host: …") and the fallback player names
  ("Empire 3", "Player 2") stay English.
- The window's title bar, the command line's and the log's messages are not in-game text.
- `re/README.md` U13's missing lines are in the German, French, Spanish and Italian files (diplomacy specials) or
  are clips; the Hungarian and English files have them, so nothing of the game's own text is filled in here.

## Consequences
- A Hungarian game shows no English remake label; captures and `--snapshot` runs in English are unchanged.
- Saves made in a Hungarian game name their quick save and autosave slots in Hungarian.
