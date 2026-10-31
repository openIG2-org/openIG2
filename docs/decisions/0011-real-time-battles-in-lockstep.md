# ADR 0011: Real-time battles in a lockstep session

Date: 2026-10-22. Status: accepted. Builds on ADR 0005 (player-hosted sessions, rejoin) and ADR 0007 (lockstep
commands, hashes, snapshots, the canonical view). Spec: `re/rules/multiplayer.md` §7; implemented by
`ig2_sim::game::realtime` and `ig2_net::lockstep`.

## Context
The original runs its MP battles by session rules (`multiplayer.md` §7): a space battle or ground war between two
human sides sets an in-progress flag on every machine, the strategic clock stops, the battle ticks on the 20 Hz loop
and the players' orders travel as lockstep messages (`MULTIGET_SpaceWar`, `MULTIGET_Colony`). The port already runs
that on the sim tick in the canonical view. What the original leaves to its host-relayed layer, or doesn't handle at
all, is new here:
- the ground war's orders (0x22..0x25, 0x12d, 0x12f, 0x130, 0x134) are applied by the exe for any unit index a
  machine sends: one player could order the other's tanks;
- a space battle closes only when **every** human has left its screen (`MULTIGET_SpaceWar`'s tail): a player who
  drops mid-battle keeps the whole session's clock stopped for good;
- a dropped participant's side gets no orders while the session runs on without it;
- an MP session started from a single-player save keeps `MultiPlayerMode` = 0 and its `PlayerKi` row, so battles are
  routed by the SP rules and the joined humans count as AI.

## Decision
1. **Who fights.** `Game::start_multiplayer(en, humans)` (the host, on the start state, before the snapshot that
   goes to every member) sets `MultiPlayerMode` = 1, `PlayerKi` = 1 for exactly the session's human empires (a
   former human not in the list becomes 0, AI) and switches to the canonical view of `en`. The original's MP
   routing then decides: two human sides fight in real time, anything with an AI side is resolved as in the
   original. Every human is on the battle's screen (the original sends every machine there): the **participants**
   (humans with units or the planet in the battle) give orders, the others **watch**.
2. **Orders.** Space orders keep their rule (ADR 0007 table: units or guns that don't exist or aren't the issuer's
   are refused). Ground orders in the canonical view are refused unless the planet is the flagged MP ground war's
   (`GrWar_MultiCsata_bolygo`), the named unit is alive and the issuer's, and, for retreat and air strikes, the
   issuer is a participant (owns the planet or a live unit on the field). The classic view (single player, every
   replay) applies them as the exe does.
3. **Cadence.** The battles tick on the sim tick (20 Hz, `TURN_TICKS` = 2 per turn, the same input delay), not on a
   faster loop of their own; `IdoSpeedSzorzoBattle` comes from the session speed (ADR 0007 §2). The strategic clock
   stands still while a flag is set, as in the original; the turn clock does not.
4. **Pause and drops: the host holds the turn clock.** `Host::hold(true)` stops sending bundles (every peer stalls
   at the same turn; nothing in the sim changes), `hold(false)` resumes. The session layer holds when a human drops
   while `Game::realtime_running()`, for a grace period it chooses (wall-clock, host-side only, default 60 s), and
   releases on the rejoin or at the end of the grace. A player's pause during a battle is the same hold.
5. **Away and back.** Away also means the computer plays the empire; a rejoin hands it back; a kick keeps it with the computer. After the grace the host injects `Command::PlayerAway` for the dropped empire
   (`Host::inject`, stamped with that empire, in the next bundle); it counts as having left every running space
   battle's screen (so `SpaceWarVege` can run once the others leave) and its units fight on with no new orders.
   `Command::PlayerBack` on the rejoin clears the left mark while the battle runs, so the returning player is on
   the screen again. Only the host may send them: a peer's input carrying either is dropped by `Host::pump`.
6. **Desyncs, rejoin, saves.** The battle state (flags, planet, battle slots, ship types, `AktSpaceWar`, the
   battlefields) is synced state in the snapshot and the per-turn hash (snapshot version 4), so a desync inside a
   battle is reported at its turn and resynced by the rejoin path, and a rejoin or an MP save mid-battle restores the
   battle where it stood.
7. **Parity.** A lockstep session is the single-player path with the bundles as the command log: replaying the
   host's log into one `Game` (commands at the first tick boundary of their turn, sim-originated ones as the peers
   queue them) gives every turn's hash the peers reported.

## Consequences
- A disconnect never deadlocks a battle, and a short one costs the others only a pause.
- The away/back commands and the ground-order refusals are new behaviour; neither changes single player or any
  replay (classic view).
- The grace period and the pause UI belong to the client's session screens (M5).
