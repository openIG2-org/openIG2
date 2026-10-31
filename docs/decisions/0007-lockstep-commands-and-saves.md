# ADR 0007: Lockstep commands, desync detection, rejoin and our save format

Date: 2026-10-16. Status: accepted (Phase 3 start). Builds on ADR 0003 (20 Hz sim tick, seeded RNG) and ADR 0005
(player-hosted lockstep over QUIC; v1 LAN, direct IP, UPnP). Implemented by `engine/crates/ig2-net`.

## Context
- The sim is a pure function of its state: `ig2_sim::game::Game` = a `SaveState` (the exe's memory as a sparse image
  plus the record arrays) and a few fields (`Libc`, the touched ground battlefields, names). `Game::advance_clock` +
  `Game::run_tick` are one exe tick (`FRISSIT_Datum` .. `FRISSIT_Starmap`), verified byte-exact against the exe.
- The original already routes the player's actions through a lockstep-style message queue, **also in single
  player**: the UI calls `Multi_Send(type, EN, …)` and `MultiSendPackets` hands each packet to a `MULTIGET_*`
  handler a frame later. Handlers ported so far: `MULTIGET_Trade` exe 0x6070b0 (message 0x51, a purchase:
  `Game::buy`), `MULTIGET_Diplomacy` exe 0x4c7880 (0x3e message → `FeldolgozAjanlat`, 0x3f answer →
  `FeldolgozValasz`: `Diplomacy::receive_message` / `receive_answer`), `MULTIGET_Starmap` ELF 0x719680 (0x1b, an
  emptied fleet: `Game::fleet_emptied`), `MULTIGET_Story` (0x66 → `Story_Special(15, sender, id)`). Others are mapped
  but not ported (`MULTIGET_Spy` ELF 0x6d08e0, `MULTIGET_SpaceWar`, the 0x3d/0x40/0x41 diplomacy handlers).
- The original MP layer (`FableSocket`, `ReceiveFullGameData`) is host-relayed with full-state resyncs (U7, `re/README.md`);
  its libc `rand()` was seeded from `SDL_GetTicks()` per machine, a likely desync source.

## Decision
### 1. Command model
Every player action is a typed **command** (`ig2_net::Command`, serde), applied by every peer at a **tick
boundary**, before `advance_clock`, in the bundle's order. One variant per original lockstep message, mirroring its
handler, so the classic ruleset stays the original's order of effects:

| Command | Original | Applied as |
|---|---|---|
| `Buy { delivery }` | 0x51 `MULTIGET_Trade` | `Game::buy(issuer, …)` |
| `DiplomacyMessage { to, declaration, msg }` | 0x3e `MULTIGET_Diplomacy` → `FeldolgozAjanlat` | `Game::diplomacy_message` (from = issuer) |
| `DiplomacyAnswer { to, msg, counter, reply, yes }` | 0x3f → `FeldolgozValasz` | `Game::diplomacy_answer` (from = issuer) |
| `StoryChoice { id }` | 0x66 `MULTIGET_Story` | `Game::story_special([15, issuer, id, 0…])` |
| `FleetEmptied { fleet }` | 0x1b `MULTIGET_Starmap` | `Game::fleet_emptied` |
| `Spy { order }` | 0x47..0x4c `MULTIGET_Spy` exe 0x5883c0 | `Game::spy_order(issuer, …)` (`espionage.md` §8, verified) |
| `StoryStart { id }` | 0x65 `MULTIGET_Story` | `Game::story_start` (`StartStory(id, false)`) |

- The **issuer** is the empire of the peer that sent it, stamped by the host (a peer can't act for another
  empire). Commands are validated by the sim exactly as the handler validates its message (a refused purchase is a
  no-op on every peer), so a stale command can't desync.
- **Sim-originated messages** (the sim calling `Multi_Send` itself, like 0x1b after `HajoDelete`) don't go over the
  network: every peer computes the same tick, so each queues them locally (`GameEvent::FleetEmptied`) and applies
  them at the next boundary, before the player commands (they were sent first). They're part of the synced state
  (in the snapshot).
- Commands that change UI-only state (screens, selections, the shown planet) are never commands; see §5.
- New handlers become new variants (spy missions and verdicts, fleet orders, production queues, taxes, research,
  battle orders). The enum is append-only within a protocol version.

### 2. Lockstep scheduling
- A **turn** is `TURN_TICKS` sim ticks (2 = 100 ms at 20 Hz). Every peer sends one `Input { turn, commands }` per
  turn, empty or not (the heartbeat). Commands issued while the local peer is at turn `t` are scheduled for turn
  `t + delay` (**input delay**); the input for a turn goes out when the turn `delay` before it becomes the next to
  run, so a command waits for the next input (up to a turn) and its round trip has `delay + 1` turns. Amended
  2026-10-31 (issue #6): adaptive. The room picks it at the start (also a resumed save's) from its members' measured
  round trips (QUIC's estimate, end to end through a relay too): `ceil((rtt + 50 ms) / 100 ms) - 1` for the slowest
  member, at least 0 (a LAN: one turn's round trip, an order runs 100-200 ms after it is given), at most 9
  (`ig2_net::input_delay`). The value counts turns beyond the one every input already has: `delay` 0 is a one-turn (100 ms)
  delay, 1 two turns. A bundle later than its turn stalls that peer until it comes; the peer then catches up
  (below), so a late packet costs a pause, not a desync. It stays fixed for the session; changing it mid-session (announced for a future turn so
  every peer switches together) is not built.
- Every peer runs one turn per `TURN_TICKS` ticks of its own clock (a turn every 100 ms; the ticks between move
  messages only; the transport is pumped every frame), so the session runs at 20 ticks/s like single player. A peer
  holding more bundles than `delay + 1` is behind and runs a turn on those ticks too.
- The **host** collects the inputs for turn `t` from every active peer and broadcasts one ordered `Bundle { turn,
  commands }` (by peer slot, then send order, each stamped with its issuer). A peer runs turn `t` only when it holds
  bundle `t`; otherwise it stalls (the sim never runs ahead or rolls back). A peer that fell behind catches up with
  at most 9 ticks per frame (the original's clamp, ADR 0003).
- The host is also a peer (its own inputs go through the same path), so it has no advantage.
- Pause, speed (MP 4/8/16) and the turn delay are host commands in the same bundles (they change sim state: speed
  is `IdoSpeedSzorzo`); a frozen session stalls the turn clock, not the sim.

### 3. Desync detection
- After each turn every peer hashes its sim state and sends `Hash { turn, hash }`; the host compares them and
  reports the first turn whose hashes differ (`Desync { turn, peers }`). The hash is SHA-256 (first 8 bytes) of the
  canonical encoding of the synced state (§4: the same bytes a snapshot writes, before compression), so "same hash"
  means "same snapshot".
- On a desync the host freezes the session and resyncs the diverged peers with a snapshot (the rejoin path);
  debug builds also dump both snapshots so the first differing field can be found.

### 4. Snapshot, rejoin and our save format
- A **snapshot** is the synced state at a turn boundary: the whole `SaveState` (memory runs verbatim, fleets, ships,
  waypoints, designs, buildings, story queues, the chunk template of unported chunks, pending fresh buildings), the
  `Game` fields that persist (`Libc`, the ground battlefields a battle touched, the default fleet name, empire and
  family names, the view mode), the pending sim-originated commands and the turn/tick counters. It holds nothing
  per-peer (`Game::local`, `Game::cosmetic` stay out).
- **Rejoin**: the host sends the newest snapshot it holds (it takes one at the join request, at a turn boundary)
  plus every bundle since; the joiner loads, replays the bundles at full speed, checks its hash against the host's
  for that turn, then takes part in the live turns. The same path resyncs a desynced peer.
- **Our save format** (`.ig2s`, single and multiplayer alike): a 64-byte header (`IG2RSAVE`, format version,
  flags, turn, tick, state hash, rules fingerprint, payload length) followed by the zlib-compressed snapshot. The
  payload is our own compact serde encoding (`ig2_net::wire`: little-endian fixed-width integers, floats as raw
  bits so NaN payloads and −0.0 survive, `u32` lengths, `u32` enum variant indices; not self-describing). Loading
  checks magic, version and the stored hash. An MP save adds the session (slots, empires, settings) in the same
  file. The format **version** bumps whenever an encoded type changes; old versions are read by migration code, or
  refused with a clear message. Original saves still load through `ig2_formats::save` + `SaveState::from_save`.
- The round trip is exact: `load(save(g))` gives the same snapshot bytes and so the same hash, and the next ticks
  stay identical (tested).

### 5. Determinism rules (ADR 0003 made concrete)
- **libc `rand()`**: the port keeps the original's MSVC LCG as `Game::libc` (one stream, the same draw order as the
  exe, which the byte-exact replays depend on). It's synced state: in the snapshot and the hash. A new session seeds
  it from the session seed the host sends with the start message (`Libc(seed)`); an original save, which doesn't
  store it, takes the session seed too. This satisfies ADR 0003's requirement (no unseeded `rand()`); splitting it
  into per-subsystem streams (ADR 0003 §4) stays possible as an *enhanced*-ruleset option, since it changes outcomes.
- **No per-peer state in the sim**: `EN` (the local human), the screen on show (`AktKep`, `AktPlanet`, the starmap
  mode) and UI timers are read by some original code paths (the battlefield weather skips the shown planet; the
  ground war's on-screen debris, dust and voice lines draw libc `rand()`). In MP the synced state holds canonical
  values (`EN` = the host's empire, nothing shown), and each peer's view lives outside the sim. Cosmetic draws that
  depend on the view move to a per-peer cosmetic RNG (ADR 0003 §4). Audited in `re/rules/multiplayer.md` (2026-10-16):
  `game::ViewMode::Canonical { en }` (synced, in the snapshot) makes the sim read `EN` = the host's empire, no shown
  planet, selection or inbox message, sends the original's `Multi_Send(.., EN, ..)` (0x1b) for every human, keeps
  the scrolling line and `AktFleet` out of the state, and draws the receiver's dialogue text from the per-peer
  `Game::cosmetic`; each peer's view is `Game::local` (`Peer::show`). `ViewMode::Classic` (SP, replays) is unchanged.
- No wall-clock, thread or hash-map iteration order in the sim (the state uses `BTreeMap`); floats are the exe's
  f32/f64 operations, which are IEEE-deterministic on every target we build (x86-64 SSE2, aarch64).

### 6. Transport
`ig2_net::Transport` carries the lockstep messages (`Input`, `Bundle`, `Hash`, `Desync`, snapshots). Phase 3 starts
with the in-process **loopback** transport (tests, single player, hot-seat) and **QUIC** (`ig2_net::quic`, feature
`quic`: `quinn` 0.11 on tokio, rustls with `ring`, ADR 0005 §2). The host's `QuicHost` listens on one UDP port with a
fresh self-signed certificate (`rcgen`) per session; a joiner pins that certificate (`quic::connect(addr, cert)`;
the certificate travels with the LAN announcement or join details), opens one bidirectional stream, sends a hello
(magic + protocol version) and then length-prefixed `Msg` frames flow both ways on that reliable stream (datagrams
for turn traffic can come later). Keep-alive every 2 s. Single player runs the same lockstep with one peer and zero
input delay.

## Consequences
- Single player, multiplayer, replays and rejoin share one path: (snapshot, command log) → state. A replay file is a
  snapshot plus the bundles.
- Every new UI action needs its command variant and handler port first; that keeps the MP surface explicit.
- A hash per turn costs one encoding of the state (a few MB, milliseconds); the interval can grow if it shows up in
  profiles.
- Per-human state (contact lists, spy reports, `st_Multi_include` effects) is kept per human in canonical MP
  (`multiplayer.md` §5, snapshot version 3). Open: `MULTIGET_SpaceWar`; the real-time battles' per-frame UI input (`MULTISEND_SpWarSynch`) as commands (LAN discovery, UPnP and the
  lobby: ADR 0008); changing the input delay mid-session.
