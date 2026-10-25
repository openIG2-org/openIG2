# ADR 0012: Host migration

Date: 2026-10-22. Status: accepted. Builds on ADR 0005 §5 (host leaving: "host migration picks the next host"),
ADR 0007 (lockstep, hashes, snapshots) and ADR 0011 (holds, away/back). Spec: `re/rules/multiplayer.md` §7.2 (client glue: §7.3)
(host migration); implemented by `ig2_net::session` and `ig2_net::lockstep`.

## Context
The host's process is the session's relay (ADR 0005 §1): it orders the inputs into bundles and runs the session
policy. Every peer has the whole state, so its loss needn't end the game, but three things are only on the host:
the turn order, the admission (`lobby::Door`: settings, session id, password) and the session's humans' names. And
the bundles in flight when it goes are not the same on every survivor: the host sends each bundle to every link in
turn, so a link that dies mid-loop leaves some survivors a bundle or more ahead in *receipt*, and each survivor runs
its bundles at its own pace, so their *run* turns differ by a few turns too. ADR 0005 §5 named "longest connected"
as the rule; a survivor can't know how long the others were connected.

## Decision
1. **Who knows what.** The `Start` every member gets (lobby start, rejoin, resync) carries the session's
   `SessionInfo`: the session id, name, scenario, private and password flags, the host's slot, every human's name, the
   grace and the keep-alive. The password itself never travels: each joiner typed it and the glue keeps it. The lobby
   protocol version goes to 2.
2. **Election.** The successor is the lowest slot among the session's humans other than the host whose empire the
   last `HoldStatus` doesn't list as away, awaited or reconnecting: the survivors share the humans and the status, so
   they agree without a round trip. `ClientSession::successors()` gives the order; the glue connects to the first that
   answers (a candidate that never answers is skipped after the glue's own connect timeout), and the survivor that is
   first itself becomes the host. Slot order replaces "longest connected": every member has it, the lobby's join
   order made it.
3. **No rollback when it can be avoided.** The new host keeps its own state; its relay continues at its own bundle
   frontier with its recent bundles as the log. A survivor reconnects through the same door as a rejoiner (hello,
   join code, password, name) and then sends `Resume`: its turn and the bundles it holds (its last 128 run and those
   not yet run). The new host first adopts the bundles past its frontier (the old host's, so the same on every
   survivor) and relays them to everyone, then resumes the survivor from its own state: `Resumed{from}` and the log
   from the survivor's frontier. The survivor resends the inputs it sent for turns no bundle has covered yet, so no
   command a player gave is lost. A survivor the log can't reach (a gap, or the migration already over) gets the
   rejoin path: a `Start` from the new host's state (the new host's state wins, as in a resync).
4. **The migration holds.** Until every surviving human has resumed, or the grace has passed since the migration
   began, no bundle goes out (the status lists them as reconnecting), so every bundle adopted is the old host's and
   every survivor's frontier stays consistent. A survivor not back by then is dropped like any player.
5. **The old host's player is dropped.** At the migration's end its empire takes the ordinary drop path: awaited
   for the grace during a real-time battle, away otherwise (ADR 0011 §4–§5). If its process lives it can come back
   through the door like any rejoiner.
6. **Afterwards** the new host is an ordinary `HostSession`: its door (from `SessionInfo`, its own rules fingerprint
   and the password the glue kept), its MP save (the session's names and settings), its rejoins and resyncs.

## Consequences
- A host's loss costs the survivors a reconnect and, in a battle, the grace; nobody's state or orders roll back
  unless their bundles can't be reconciled.
- Every member learns the other humans' names and the session id (needed for the door and the save).
- If a status change was in flight when the host went, two survivors can elect different successors; the one that
  isn't reached runs alone once its migration's grace ends. The status changes rarely (drops, pauses) and goes out
  on every change, so this needs a host loss within the same round trip.
- The glue must know every member's address (connection candidates) to reach the successor; that's the connectivity
  ladder's job (ADR 0005 §3), not the session layer's.
