# ADR 0045: Enhanced computer empires, part 1 (economy and research)

Date: 2026-11-01. Status: accepted. An enhanced ruleset beside the classic one (classic = the original, bit for
bit). Spec: `re/rules/ai-empire.md` §15 (the classic AI's weaknesses, the enhanced rules, the measured effect).
Implemented by `ig2-sim::ai::enhanced`, `AiContext::enhanced`, `EconomyContext::enhanced_ai` (the autobuild's
per-planet purchase in `colony_side`), `Game::enhanced_ai` (snapshot version 12), `GalaxyOptions::enhanced_ai`
(lobby protocol 11), the Options row "Computer empires" (`EnhancedAi`) and the multiplayer room's row.

## Context
The original's computer empires are easy to beat (`docs/ENHANCEMENTS.md` → Ideas → AI). Played AI against AI in the
play-through harness they grow very slowly: in a skirmish they keep their 4 start planets for about three years,
let 150,000–400,000 credits pile up, and their research slot stands idle most of the time because the open techs
need more labs than the one-building-at-a-time autobuild ever adds (§15.1). A player who simply expands and
builds labs outgrows them within a few years.

## Decision
1. **A per-game ruleset**: "Computer empires: classic / enhanced". Classic (the default) runs the original's AI
   untouched. Enhanced changes only the economy and research decisions of every computer-played empire (a dropped
   multiplayer player handed to the computer included); fleets, war, spies and diplomacy stay the original's (parts
   2 and 3).
2. **Where it is set**: single player takes it from the Options screen's Remake row "Computer empires" when a new
   game starts; a multiplayer room has a host row of the same name. The game keeps its own choice: it is part of
   the synced state (`Game::enhanced_ai`, snapshot version 12, in the state hash), so a save, a rejoin and a host
   migration keep it. Older saves load as classic.
3. **Lockstep-safe by construction**: the enhanced rules run inside the sim's daily AI pass and economy step, read
   only synced state and draw no random numbers, so every peer computes the same decisions and the classic draws
   stay where they were. A building the enhanced AI places goes through the colony screen's own build path.
4. **The rules** (spec §15.2): every planet's autobuild pick is bought when affordable (not one per empire per step);
   while research waits for labs, labs are placed for the nearest open tech and production planets switch to the
   lab profile; taxes follow morale bands; research picks a colony-ship hull while planets wait, else the race's
   priority with ties to the cheaper tech (the auto research pick of ADR 0037), urgent when credits pile up; colony
   ships are wanted per free planet seen (up to 3) on every shipyard and, at peace, replace a warship batch; at peace
   no more warships than 4 + 2 per planet.

## Consequences
- Classic games, replays and saves are unchanged (every enhanced branch is behind the flag).
- The save format (12) and the lobby protocol (11) changed: older builds can't read a newer save or join a newer
  room.
- Measured over 8 skirmish seeds, after 8 game years the enhanced empires hold about 2.2× the planets, over a
  third more population and 7–8 more techs than the classic ones (§15.3). Multiplayer galaxies gain less: the original
  halves a computer's income and production there, which part 1 leaves alone.
- Shipped as enhancement row 459 (`docs/ENHANCEMENTS.md`); fleets, war and diplomacy follow as parts 2 and 3.
