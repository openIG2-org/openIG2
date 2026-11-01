# ADR 0063: Responsive multiplayer orders

Date: 2026-11-01. Status: accepted. Builds on ADR 0007 (deterministic lockstep: §2 the turns and the input delay),
ADR 0014 §5 (the client's multiplayer world) and the 2026-11-01 decision in `docs/ENHANCEMENTS.md` (multiplayer
orders must feel as responsive as in single player). Implemented by `ig2_net::Peer` (`in_flight`, `adapt`,
`delay`), `ig2_net::session` (the sessions feed `adapt`), `ig2_sim::game::Game::fork` and `ig2-client::world`
(`World::predict`, `settled`).

## Context
Single player applies an order in the frame it is given (`World::pump`): the path line, the destination, the
fleet's heading and every screen show it at once. In multiplayer the order goes into the peer's next input and runs
on every machine when the host's bundle for that turn comes back. Measured on the lockstep as the client paces it
(`ig2-net/tests/latency.rs`: a turn every two 50 ms ticks, messages every 10 ms frame, orders given at 0..80 ms into a
turn):

| link | input delay | worst | mean |
|---|---|---|---|
| loopback / LAN | 0 | 200 ms | 160 ms |
| 50 ms round trip | 0 | 200 ms | 160 ms |
| 100 ms round trip | 1 | 300 ms | 260 ms |
| 200 ms round trip | 2 | 400 ms | 360 ms |

Where it comes from: the order waits for the next turn boundary to go out (the input for the current turn is already
sent: 0..100 ms), its turn runs one turn later (100 ms) plus the delay's turns, and the screens drew nothing of it
until then. The delay was picked once at the room's start from the slowest member's round trip and applied to every
peer, so one far player slowed everyone's orders for the whole game.

## Decision
1. **A prediction on the issuing machine.** While any of the local player's orders has not run yet
   (`Peer::in_flight`: the commands sent for turns not run, then those waiting for the next input), the world shows
   a fork of the shared game (`Game::fork`) with them applied, in order, for the local empire, exactly as the
   lockstep will apply them at the start of their turn and as single player applies them at once. Every screen reads
   the fork as it read the game, so the path line, destination, heading, selection and any panel change show in the
   same frame. The fork never enters the lockstep: the peer runs the shared game under it, and the fork's events are
   dropped, so outcomes the sim reports (a split's new fleet, a refused satellite, a stored design) still come from
   the real turn, once. When a turn runs or an order is given the fork is rebuilt from the new shared game; when no
   order is in flight the shared game is shown again. A turn that ran other players' orders first simply shows
   their effect under ours. The turn clock and the fleets' interpolation follow the shared game: a predicted pause
   shows at once, the fleets stop when it runs.
2. **Per-machine parts carry over.** What the screens write into the shown game in canonical multiplayer is
   per machine (the local view, the renames shown ahead of the lockstep, the own message line, a spectator's end);
   before the shared game goes into the peer for a save, snapshot, drop, kick or replay, those move from the fork
   into it (`settled`). Classic-view games (single player) are never forked.
3. **Each peer picks its own input delay.** The delay only says which turn a peer's input is for; the host's
   bundles stay the one order every peer runs, so the sim, the state hash, the snapshots and replays never see it
   and no agreement between peers is needed: the bundle stream is the agreement. Each peer feeds `Peer::adapt` one
   round-trip sample of its link to the host per turn (QUIC's smoothed estimate; the host's own peer is in process,
   0 ms) and keeps TCP's smoothed round trip and mean deviation (RFC 6298, gains 1/8 and 1/4). Its delay is
   `input_delay(srtt + 2 × deviation)` (ADR 0007 §2: the round trip within `delay + 1` turns with 50 ms to spare),
   raised at once when that needs more, lowered by one turn once a lower one has sufficed for `LOWER_AFTER` (20)
   turns. Raising sends empty inputs for the skipped turns; lowering sends none until the turn catches up, so no
   turn waits and no order is lost either way. The room's start delay stays the slowest link's (ADR 0007), and every
   peer comes down from there to what its own link allows: a LAN reaches 0 within a few seconds.
4. **The fleets' radar zones glide.** The starmap rebuilt the radar ranges (`RadarRangeKi`, re/screens/starmap.md
   §6.3) once per snapshot while the fleet sprites are interpolated every frame, so a fleet's blue zone stepped
   behind its sprite each tick (20 a second), and each turn (10 a second) in multiplayer. Each frame now moves the
   fleets' circles exactly as `Snapshot::fleets_at` moves their fleets and rebuilds the rings from them
   (`Snapshot::radar_at`); at the end of the way, and whenever nothing moves, they are the snapshot's own, the exe's
   dots bit for bit.
5. **Always on, no option.** It changes no rule and no synced state, and single player is untouched.

## Consequences
- An order shows on the issuing machine in the frame it is given (0 ms, as in single player); the other machines
  see it when its turn runs, as before. A LAN game runs at delay 0 for everyone even if the room started higher, and
  one far player's delay no longer slows the others' orders.
- A fork costs a copy of the game state, made only when a turn runs or an order is given while orders are in flight
  (at most ten a second plus one per order), never per frame.
- A prediction can differ from the real turn when another player's order or the tick in between changes the outcome
  (both bought the last offer); the real state replaces it a turn later. Order sounds are the screens' own and play
  on the click as before.
- The WebRTC link (browser build) reports no round trip yet, so a browser joiner keeps the room's start delay.
