# ADR 0005: Multiplayer hosting, connectivity and lobbies

Date: 2026-10-15. Status: **accepted for the first version** (owner, 2026-10-15): LAN, direct IP join and UPnP/NAT-PMP hosting first, without a rendezvous service; hosting a rendezvous service is decided later.
Builds on ADR 0003 (deterministic sim: fixed 20 Hz tick, seeded RNG, `rand()` rerouted).

## Context
The owner's requirements:
- A player hosts a session **from inside the game**; others join easily. No dedicated servers as in the
  original (the GOG remaster's lobbies lived on a central server; a server package of the remaster is probably
  that server, not yet inspected, STATUS U7).
- **No manual port forwarding and no user-run tunnels** (Hamachi, VPNs): UPnP and automatic fallbacks.
- **LAN play** with automatic discovery.
- **Lobbies**: open sessions anyone can join until the game starts, password-protected sessions.

Facts that shape the design:
- UPnP alone can't reach everyone. Many routers ship with UPnP off, and carrier-grade NAT (common on
  mobile and some fibre/cable ISPs) or double NAT makes a mapped port unreachable from outside. A host
  behind such a NAT can't accept inbound connections however the game tries. Covering those players
  automatically needs NAT hole punching and, as a last resort, a relay.
- Listing internet sessions in a browser needs *some* public directory. It can be tiny (session
  announcements, no game traffic), but a peer-to-peer game can't find strangers' sessions without it.
- Our sim is deterministic (ADR 0003), so the game can run as **lockstep**: only player commands go over
  the network, and every machine runs the same simulation. Bandwidth is tiny and the host needs no
  special hardware.

## Decision
### 1. Topology: player-hosted listen server, deterministic lockstep
- The host's game process runs the session: admission, lobby state, turn timing and the command
  relay. It doesn't own the simulation; every player runs it.
- Each turn the host collects every player's commands and broadcasts them in one ordered bundle; all
  peers execute the same ticks. A hash of sim state every N turns detects desyncs; a desynced or
  rejoining player gets a state snapshot from the host (the same mechanism as MP saves).
- The session code also runs **headless** (`ig2-host` binary) for players who want an always-on host.
  That's an option, not a requirement.

### 2. Transport
QUIC (`quinn`) over **one UDP port**: encrypted, reliable streams for lobby/chat/snapshots, datagrams
for turn traffic, built-in keep-alive and connection migration. Each session gets a fresh self-signed
certificate; joiners pin its fingerprint (delivered with the session listing, join code or LAN
announcement), so no certificate authority is needed. One UDP port keeps NAT traversal simple.

### 3. Connectivity ladder (automatic, tried in order, the user sees only "Connecting…")
1. **LAN**: hosts announce on the local network (UDP broadcast plus mDNS); the LAN tab lists them and
   joining connects directly. No internet needed.
2. **IPv6 direct** when both sides have global IPv6.
3. **Port mapping on the host**: UPnP-IGD, then NAT-PMP/PCP. The mapping is renewed while the session
   lives and removed when it ends. The rendezvous service (below) checks that the mapped port is
   actually reachable from outside.
4. **UDP hole punching**: the rendezvous service tells both sides each other's public address and they
   send simultaneously. This works for most home NATs, including routers with UPnP off.
5. **Relay fallback** for what's left (symmetric NAT, CGNAT on both sides): traffic is forwarded by a
   relay. It runs no game logic and can't read the traffic (QUIC end-to-end encryption), and the
   turn traffic is a few KB/s, so a relay is cheap.

### 4. Discovery, lobby and joining
- **Session browser** (internet): sessions announce to a lightweight **lobby directory** with name,
  scenario, ruleset, players/slots, password flag, game version, rules fingerprint and connection
  candidates, refreshed by heartbeat. The directory holds no game state; when the host closes, the
  listing disappears.
- **Join code**: every session gets a short code (e.g. `KX7-42Q`) to share in chat. **Direct join** by
  IP:port works too, and needs no directory.
- **Lobby types**: *open* (listed, anyone may join until the game starts), *password* (listed with a
  lock; checked by the host with a challenge over the encrypted connection, the password is never stored
  or sent in clear), *private* (unlisted, join code or invite only).
- **Lobby features**: slots (human/AI/closed, empire, race, colour), ready flags, chat, kick/ban, host
  settings (scenario, speed MP 4/8/16, ruleset classic/enhanced, AI difficulty), start when all are ready.
- **Compatibility check on join**: same game version and the same **rules fingerprint** (a hash of the
  rule tables read from each player's own `ig2.exe`, ADR 0004). A mismatch is refused with a clear message.
- **After the start** the session is closed to newcomers, but a dropped player can **rejoin** their
  empire (state snapshot from the host). Optionally the host can let a newcomer take over an AI
  empire (later).

### 5. Host leaving
Every peer has the full state, so the session can continue: **host migration** picks the next host
(longest connected), it re-runs the connectivity ladder, and the others reconnect; or everyone saves
and resumes later (MP saves). Migration comes after the basic flow works.

### 6. Rendezvous service
Directory, reachability check, hole-punch coordination and relay are **one small open-source server**
(`ig2-rendezvous`, no game logic). The game has a configurable list of rendezvous addresses. Without
any, LAN, direct IP join and UPnP-only hosting still work; only the internet browser, join codes, hole
punching and relay need it.

## Consequences
- Hosting and joining need no setup for LAN, for UPnP-capable routers, and (with a rendezvous service)
  for nearly everyone else.
- Lockstep makes desyncs fatal, so determinism (ADR 0003, bit-exact rules) is a hard requirement;
  per-turn state hashes catch regressions early. Rejoin and MP saves share the snapshot format.
- Lockstep latency = one turn plus the slowest player's round trip. That's fine for a strategy game;
  the real-time space and ground battles run inside the same lockstep (no separate netcode).
- Crates to evaluate at implementation time: `quinn` (QUIC), `igd-next` (UPnP-IGD), a NAT-PMP/PCP client,
  `mdns-sd` (LAN discovery).

## First version (owner decision, 2026-10-15)
No rendezvous service at first. Version 1 ships ladder steps 1–3 (LAN discovery, IPv6 direct, UPnP-IGD /
NAT-PMP / PCP port mapping) plus **direct join by IP:port**; open, password and private sessions work
over those paths. Without the service there's no internet session browser, no join codes, no hole
punching and no relay, so a host behind CGNAT or a router without UPnP can't accept internet players
yet (LAN still works). The protocol keeps room for those features (connection candidates in the
handshake, a session id usable as a join code) so they can be added without breaking it.
Reachability check without the service: the host shows its external address and whether the port
mapping succeeded; a joiner's failed attempt explains the likely cause.

## Open questions
1. Hosting the rendezvous service later: project-hosted or community-hosted (we publish the binary)?
2. Steam/GOG Galaxy friend invites as an extra join path (their relays could serve as fallback)?
3. Should the original multiplayer protocol stay compatible with the GOG game? Assumed **no** (new
   protocol); the old one is still worth studying for its failure modes (U7).
