# ADR 0040: a searchable event log on the messages screen

Date: 2026-11-01. Status: accepted. A deliberate addition to the classic client (ADR 0009), off = the original
screen. Spec: `re/screens/messages.md` §9. Implemented by `ig2-client::event_log`, `ig2-client::world` (recording) and
`ig2-client::window` (input).

## Context
The original keeps the human's scrolling-line texts in a ten-entry history (`ScrollUzenetekHistory`, the top bar's
drop-down, `GFX_ScrollHistory` exe 0x556ac0) and the inbox (`Uzenetek`, the messages screen). Older lines are gone:
a battle, a revolt or a finished research of a few minutes ago can't be looked up, and nothing leads from a line to
its planet or fleet.

Every scrolling line the sim posts comes from a tick event through `Game::messages_for` (`re/rules/info-messages.md`
§3.2); the event names the planet or fleet the line is about.

## Decision
1. **Option "Event log"** (Remake row on the Options screen's game page, `EventLog` in the remake's settings file):
   on (1, the default) or off (0 = the original messages screen, nothing recorded).
2. The client records, per tick, every line `messages_for` gives for the local empire (the same call that fills the
   line, so the texts are the line's own), with the game date, a category and a target. Kept per game in memory, the
   newest 1000 lines; not saved (a loaded game starts an empty log). Nothing goes into the sim or the lockstep.
3. Categories, by the event: battles (space and ground battles, an enemy fleet approaching), research (a finished
   or captured tech), diplomacy (messages, war, contact, empires discovered or gone, espionage), colonies (revolts,
   unrest, abandoned or rebelled planets, a spy expelled, the yearly tax and trade income), story (the campaign
   scripts' own lines); a lost contact is in none but "All".
4. On the messages screen an "EVENT LOG" label (and the `L` key) opens a panel over the right half: a filter row
   (All, Battles, Research, Diplomacy, Colonies, Story), a search field (letters typed while the panel is up; case
   ignored), the matching lines newest first (wheel, arrow keys, PgUp/PgDn scroll). A line with a planet or a fleet
   is lit under the pointer; clicking it leaves for the starmap with that planet or fleet selected and centred. Esc
   or the label closes the panel.
5. Targets: a battle's planet (a space battle away from a planet: its attacking fleet), a colony line's planet, a
   spy strike's planet, an approaching fleet. A fleet that is gone or no longer seen recalls nothing.

## Consequences
- The texts are the language's lines from `uzenet.txt` and the scripts; the panel's own labels are English.
- Lines posted before the client started recording (a loaded game's earlier ones) are only in the original history.
