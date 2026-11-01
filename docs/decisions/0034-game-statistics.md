# ADR 0034: Game statistics and a match history

Date: 2026-11-01. Status: accepted. An enhanced option (enhancements 444–446); on by default, `GameStatistics` 0 =
the original (no screen, nothing recorded). Spec: `re/rules/game-end.md` §11. Implemented in `ig2_sim::stats`
(`Game::stats`), `ig2_net::snapshot` (version 11) and the client's `stats_screen`, window and main menu.

## Context
The original ends a game with its end videos and the main menu (`MEGHALAS`/`MEGNYERES`, `ExitToMainScreen`,
`re/rules/game-end.md` §2). Nothing says how the game went, and nothing is kept: its high-score table is dead code
(§6), and the info screen's charts cover the last 55 months of a running game only.

## Decision
- The sim samples every empire once a game day (planets, population, credits, ships, techs, battles won, the info
  score; ships built and lost as running totals) into `Game::stats`, from the synced state only, so every
  multiplayer peer has the same numbers. The samples thin out (at most 240, the step doubling) so a long game costs a
  few kilobytes. They are saved in our save format (version 11) and covered by the state hash; nothing the original
  keeps is written, so classic replays stay byte-exact.
- After a win or a loss (and after the end videos) the client shows a statistics screen: the game's length and
  result, a table per empire, and line graphs over time, drawn with the screen kit's panels, font and palette.
- Every finished game is appended to our own `match-history.json` beside the saves; the main menu gets a "History"
  item that lists them and opens each one's statistics screen. Multiplayer games are recorded on every machine,
  each with its own result.
- One option, `GameStatistics` (Options → Remake, default on); off = the original: no screen, no record.

## Consequences
- Save format version 11 and lobby protocol 9; older saves load with empty statistics.
- The record is written at the game's end; a game quit from the Options screen's "Main menu" isn't a finished game
  and isn't recorded.
- Not covered: the original's high-score table (proposal 146), exporting the history, graphs of more than one value
  at a time.
