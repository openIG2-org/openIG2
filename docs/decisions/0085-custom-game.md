# ADR 0085: Custom game, a generated galaxy with every start setting

Date: 2026-11-08. Status: accepted, part 1 (sim) and part 2 (the modern page, without the preview: `ig2-client::custom_game`) done. Row 546 of `docs/ENHANCEMENTS.md`. Builds on the skirmish start
(`re/rules/new-game.md` §13.1, `Game::start_skirmish` / `NewSkirmish`), the quick game (ADR 0039), the computer levels
(ADR 0033) and the enhanced AI (ADR 0045).

## Context
The original's skirmish panel has seven settings, each a step of a fixed table (`re/screens/mainscr.md`, skirmish
options): the player's race (1..8), the computer opponents (1..7), the difficulty (1..4), the galaxy's planets (5 steps,
`ScenarioKezdoBolygoSzamok`), the start technology (0..3, `ScirmishTalalmanyFejlesztes`), the start money (6 steps,
`ScenarioKezdoPenzek`) and the population (0..3, which also sets the start planets: `InitScirmishBolygok`, exe
0x4e2a10, one to `HanyBolygo / 15` planets an empire). Every computer empire gets the same money and technology, its
own race (`Start_Init`'s defaults), the original's start relations and AI, and a home planet drawn at random
(`st_bSearch(-e, 2)`). Players want to set up a game the way they like: who they play against, how strong and how
friendly those opponents are, how large the galaxy is and where they start.

## Decision
1. **One description of a custom game.** A `CustomGame` (sim, saved with the game, part of a multiplayer room's start
   data, so every machine builds the same galaxy) holds:
   - the galaxy: planet count as a number (20..150; the exe hangs at 200, `new-game.md` §280, so 150 is the cap until
     that is understood), the random seed (shown, typeable, so a galaxy can be replayed);
   - per empire (the human or humans and 1..7 computer empires): race, empire name, start money, start technology
     (0..3, the same `ScirmishTalalmanyFejlesztes` levels), start planets (1..8), computer level (easy / normal / hard,
     ADR 0033), aggressiveness (low / normal / high) and the start relation towards each other empire (hostile /
     neutral / friendly);
   - the start position: random (the original's draw) or a chosen star.
2. **The original's settings are a custom game.** `NewSkirmish` maps onto a `CustomGame` whose values are the tables'
   steps; a custom game with only those values starts byte for byte as the original's skirmish (the existing
   `tests/new_game.rs` skirmish starts stay the proof). Every other value is applied after the original's steps, at the
   place the exe sets that value, without extra draws on the original's RNG streams when the value equals the
   original's; the extra draws a changed value needs come from their own stream (`re/rules/time-and-rng.md`), so the
   rest of the galaxy stays the seed's.
3. **Aggressiveness** scales the AI's attack decisions (the main attack's strength threshold and the war declaration
   chance, `re/rules/ai-empire.md`); normal is the original's. The start relation writes the diplomacy table's start
   value (`re/rules/diplomacy.md`); neutral is the original's. Both are measured in the play-through harness as ADR 0045
   was (8 seeds, 10 years) before they ship.
4. **The modern interface** gets a Custom game page beside the Quick game (main menu → New game): galaxy settings at
   the left, a table of empires (one row each, add / remove computer empires, the race's head, the settings as
   drop-downs and steppers), a preview of the generated starmap (the seed's stars and the start planets; a click on a
   star picks the start position), Back and Start. The multiplayer room's galaxy settings (ADR 0066) take the same
   fields: the host sets them, every seat shows them.
5. **The classic interface** keeps the original's skirmish panel as it is. With the enhanced ruleset its panel gets an
   "Advanced" label that opens the same Custom game page; with the classic ruleset the label is not there and only the
   original's seven settings exist.
6. **In parts:** (1) the sim's `CustomGame` and its tests (the original's starts unchanged, each new value's effect),
   (2) the modern page without the map preview, (3) the map preview and the chosen start, (4) aggressiveness and start
   relations with the harness measurements, (5) the multiplayer room and the classic "Advanced" label.

## Consequences
- A saved custom game records its `CustomGame`, so a load or a replay rebuilds nothing from the menu.
- Settings far outside the original's (one empire with 8 start planets and full technology against easy opponents) can
  make games the original never had; the page shows them as they are, without a warning.
- No exe capture exists for the new values; the tests hold them, and the original's values keep their exe-recorded
  starts.
