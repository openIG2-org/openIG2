# ADR 0014: The multiplayer room and connection screens

Date: 2026-10-24. Status: accepted. Builds on ADR 0005 (hosting), ADR 0007 (lockstep), ADR 0008 (discovery and
the lobby protocol), ADR 0009 §2 (our MP screens styled like the original's menus), ADR 0011/0012 (the session).
Spec: `re/screens/start.md` (the original room's rules §1–§3, ours §4–§6). Implemented by `ig2_net::lobby`,
`ig2_net::galaxy`, `ig2_net::session` and `ig2-client::mp_lobby`.

## Context
The original's MP game is a generated galaxy (`InitNewGame(seed, 2, 0, 1)`) set up in its game room (`Start`,
`AktKep` 14): eight slots, races, the AI count, planets, galaxy shape, battle status and the host's speed control;
the host broadcasts the room, then a seed, and every machine generates the galaxy itself. Our lobby protocol
(ADR 0008) was built for scenario and save starts: the host picks the slots, every member picks one, and the start
ships the whole start state as a snapshot. The client had no screens for either.

## Decision
1. **One protocol, two kinds of lobby.** `LobbyState::galaxy: Option<GalaxyOptions>` marks a generated-galaxy room.
   In it the original's rules apply (spec §4): eight slots, a joiner seated at once, moves to an empty or AI slot
   (the AI swaps over), races per slot, the AI count re-rolled by the host, start at every human ready with two or
   more empires. Without it the ADR 0008 rules stay (scenarios, MP saves). The protocol version goes to 3 (new
   `LobbyMsg::Race`, `LobbyState::galaxy`, `Start::galaxy`).
2. **Every machine generates the galaxy.** The galaxy's `Start` carries `GalaxyStart { seed, ticks, options, races,
   ais, hash }` instead of a snapshot; `GalaxyStart::build` runs `Game::before_start`, `Game::start_mp_galaxy` (the
   host's empire as `EN`), `Game::start_multiplayer` and the session id as libc on every machine, host included, so
   all of them start from the same code path. The joiner compares the state hash with the host's and refuses a
   different start (`session::Error::StartMismatch`: a different install or language; rules fingerprints already
   catch a different exe). Rejoins and resyncs still get snapshots (the state has moved on).
3. **Shared ticks.** The original lets each machine reseed libc from its own `SDL_GetTicks()` in
   `NEWGAME_Diplomacy`; lockstep needs one state, so the host's ticks travel with the seed (deviation 223).
4. **The screens.** The main menu's "local" and "inet" rings (after the name entry) open our connection screen:
   the LAN sessions (`discovery::Browser`), host (QUIC on a UDP port plus the LAN announcement), join from the list
   (pinned by the announcement's fingerprint) or by address (unpinned until a password binds the channel), a
   password field for either. "inet" is the same screen: no internet matchmaking or public server (ADR 0005 defers
   the rendezvous service; anything beyond the LAN needs a maintainers' decision). The room draws in the scenario
   panel's style over the main menu scene (spec §6). The words the original has come from its text files
   (`txt14`, `txt15`); the rest are ours, in English.
5. **The game starts in the session.** The room's start yields a `HostSession` or `ClientSession` on each machine;
   the client runs its world from the session's peer.

## Consequences
- The original's room rules are kept where they matter for the game (slots, AI pick, start condition); the room's
  look and its two-step "start now / ack" handshake are not (ours: the lockstep's own first turns).
- A joiner needs the same install language as the host for the generated names (a mismatch is refused, not
  desynced).
- Tests: the lobby and the galaxy start over loopback links in `ig2-net`, two and three clients through the room
  model in `ig2-client`; no internet service is touched.
