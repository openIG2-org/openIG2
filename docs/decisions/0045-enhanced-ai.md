# ADR 0045: Enhanced computer empires (part 1: economy and research; part 2: fleets and war)

Date: 2026-11-01. Status: accepted. An enhanced ruleset beside the classic one (classic = the original, bit for
bit). Spec: `re/rules/ai-empire.md` §15 (the classic AI's weaknesses, the enhanced rules §15.2 and §15.4, the
measured effect §15.3 and §15.5).
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
   multiplayer player handed to the computer included); part 2 adds fleets and war (decision 5); spies and diplomacy
   stay the original's (part 3).
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
5. **Part 2, fleets and war** (spec §15.4): spaceship factories go onto the biggest yard while the ship capacity is
   short (tank factories first at war), one wanted colony ship starts on one yard a day, the peace cap counts real
   warships (not fighters), garrisons refill to 25 at war; a war is declared once the main fleet is 1.5 × the
   target's, a planet is invaded only with 1.5 × its defence aboard, an enemy attacking an own colony that the main
   fleet outguns comes first, the main fleet waits for followers close by before an invasion, and a fleeing fleet
   no longer stops every few ticks. The original's random draws are made as before; no save or protocol change.

## Consequences
- Classic games, replays and saves are unchanged (every enhanced branch is behind the flag).
- The save format (12) and the lobby protocol (11) changed: older builds can't read a newer save or join a newer
  room.
- Measured over 8 skirmish seeds, after 8 game years the enhanced empires hold about 2.2× the planets, over a
  third more population and 7–8 more techs than the classic ones (§15.3). Multiplayer galaxies gain less: the original
  halves a computer's income and production there, which part 1 leaves alone.
- Part 2 (§15.5, 8 skirmish seeds, 10 years): the main fleets reach about 4 × part 1's attack while the empires hold
  over twice the classic planets, and an idle human falls about ten months sooner; the main fleets still carry few
  ground troops (troop room and money are the next limits).
- Shipped as enhancement row 459 (`docs/ENHANCEMENTS.md`); diplomacy follows as part 3.
