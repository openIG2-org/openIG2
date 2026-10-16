# Wormhole trips in the save replay (2026-10-16)

Spec: `re/rules/fleet-movement.md` §4.3 ("move/attack a wormhole fleet"), the `Wormhole` table and `AddWormhole`.
Port: `fleet::Starmap` (the arrival step's wormhole loop, `StarmapEvent::Wormhole` + story special 0x14), the radar
pass's revealed ends (§6.3). Test: `engine/crates/ig2-sim/tests/save_replay.rs` `replay_matches_the_exe_tick_by_tick`.
Tracer: `tools/trace/run_replay.py --wormholes` + `replay.js` `stageWormholes()`.

## Evidence

Exe `FRISSIT_Starmap` 0x596b40..0x596ca7: a fleet whose target kind (`+0x42`) is 2 and whose target fleet has
`+0x53` bit 2 (bit 10) loops `w` = 1..=`OsszHanyWormhole` (0xBB61A0); when `(f32)Wormhole[w].fleet_a == +0x44` the
fleet's new position is end B, both ends are revealed (`+8`, `+0x14`), the far end's fleet is seen by the owner
(`+0x19 + owner = 1`) and the radar cleared (`+0x7c = 0`); likewise from end B to A (both tests run for every `w`).
Then `Story_Special(0x14, owner, 1, f, last)` 0x5c9550 where `last` = a local copied from the counter **before**
each increment (0x596c71): the loop's last index, `OsszHanyWormhole`. With no wormhole the jump at 0x596b72 skips
the loop and pushes the never-written local (0x596c8a): an uninitialised stack word [original quirk]. The spec and
port had `OsszHanyWormhole + 1` (the ELF's counter after the loop); the trace corrected it.
`AddWormhole` exe 0x53aec0 (decompiled; called by stories 902/239): spec'd in fleet-movement.md, not ported (below).

## Recording

quick.scn's two "- Wormhole -" fleets (bit 10) belong to no wormhole (`OsszHanyWormhole` 0 in `quick-1.sav`, and the
save keeps neither fleet), so `--wormholes` calls the exe's own `AddWormhole` before save 0: wormhole 1 from next to
the human's home planet (end A, fleet 10) to next to AI 3's (end B, fleet 9). Then, built with
`FlottaNew`/`HajoNew`/`FlottaMozgat`: a human fleet moving to end A from 0.5 away (out at B the first tick), an AI fleet
attacking end B (out at A), a human fleet moving to end A from 25 away (through some ticks later). The fourth case
(a fleet moving to an unlisted wormhole fleet) found no such fleet in the save.

| Recording | Speed | Ticks | Events |
|---|---|---|---|
| `replay-quick1-wormholes-x16` | ×16 | 60 | 3 wormhole trips (moving and attacking, A→B and B→A), 3 story specials 0x14, revealed ends seen by the radar, 12 arrivals |

## Result

All 60 ticks byte-exact after the fix. Before it, tick 2 failed on the unsaved `StorySpecialData` (0xF265A4 + 0x10:
port 2, exe 1). Mutations: the old `+ 1` (above) and dropping the far end's sighting (`+0x19 + owner`) each fail at
tick 2.

## Open

`AddWormhole` itself: `Game` doesn't yet carry out `StoryEvent::Wormhole` (stories 902/239); it needs the tint table
0x913850 in `ig2-rules` and `FlottaNew` for owner 0. The no-wormhole case's stack word is unknowable (the port passes 0).
