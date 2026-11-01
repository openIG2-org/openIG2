# ADR 0052: story 834's siege fleets start at the besieged planet (enhanced ruleset)

Date: 2026-11-01. Status: accepted. Implemented by `ig2-sim::elem::threats` (`story_834`), `ig2-sim::game`
(`Game::story_fixes`), `ig2-sim::story` (`StoryWorld::story_fixes`), `ig2-net::snapshot` (version 13) and
`ig2-client::display` (the "Campaign story" row).

## Context
Story 834 (re/rules/campaign/elem.md §8.14, exe block 0x5de47e) sends empire 1's five siege fleets against planet
8340 with `st_fNewEx(_s(5, 834k), 1, 834k, 0.09, 2, −1)`: mode 2 places a fleet on a planet, and the planet passed
is −1. The exe reads the truncated `Bolygok[−1]` position (0xB85D38/0xB85D3C), the bytes before the planet table,
which are (0, 0) in every recorded game; so the fleets appear at the galaxy's corner and fly across the map. The
classic port keeps this bit for bit (the campaign traces check it).

## Decision
1. **A "Campaign story" row** (`StoryFixes`: Classic, the default, or Enhanced), read when a new game starts, like
   the computer empires' row (ADR 0045): the game keeps its own choice (`Game::story_fixes`, in our saves from
   snapshot version 13, in the lockstep hash). Older saves and the original's `.sav` files are classic.
2. **Enhanced**: the five fleets are placed on planet 8340 itself (`b(8340)`), the planet they besiege; the rest of
   the story (their orders, the arrival check, the win and loss) is unchanged. Classic passes −1 as the exe does.
3. Further story fixes of the enhanced ruleset go behind the same flag.

## Rejected
- **A start at one of empire 1's planets** (`b(−1)`, what mode 2 does for parameter 0): plausible too, but the
  accepted change names planet 8340, and starting at the target is the reading that keeps the story's timings
  (the counters 240..755 then are the waves' arrival times).
- **Fixing it in classic**: the classic ruleset stays the original's, bit for bit.
