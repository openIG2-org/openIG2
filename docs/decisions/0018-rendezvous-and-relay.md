# ADR 0018: The rendezvous and relay service

Date: 2026-10-30. Status: accepted (the software). Running a public instance is not decided: the game ships no
service address and nothing is hosted by the project. Builds on ADR 0005 §3–§6 (NAT traversal ladder, the
rendezvous service), ADR 0008 (the lobby protocol and its password binding) and ADR 0014's amendment (join codes).
Implemented by `ig2_net::rendezvous` (protocol and service state), `ig2_net::quic` (the client side) and the
`ig2-rendezvous` binary; how to run one: `docs/RENDEZVOUS.md`.

## Context
Join codes reach a host only at its LAN address or at a port its router mapped (UPnP / NAT-PMP). Behind a router
without either, or behind carrier-grade NAT, nobody can join. ADR 0005 planned the next rungs: a rendezvous service
that tells both sides each other's public address so they can punch a hole through their NATs, and a relay for the
rest. Its directory (an internet session browser) is a separate step and not part of this decision.

## Decision
1. **One small UDP service, `ig2-rendezvous`.** No game logic, no accounts, no disk: rooms live in memory and
   expire. Default port 29172 (discovery 29170, game 29171). IPv4 only, like join codes. Players opt in by typing
   a relay address on the connection page; without one, nothing changes and nothing is sent.
2. **The relay forwards QUIC datagrams, not lockstep messages.** The game's QUIC packets travel through the relay
   wrapped in a 12-byte header, so TLS stays end to end: the join code's certificate pin, the lobby password's
   channel binding (ADR 0008 §3.1) and the encryption work unchanged, and the relay can neither read nor alter
   the game. The host's endpoint and the joiner's run on a socket wrapper (`quic::RelaySocket`) that speaks the
   service's packets on the side and hands QUIC the peers' real public addresses, so a relayed link looks like a
   direct one to the session code. (Relaying the lockstep frames instead would need a second encryption layer to
   keep the pin and the binding meaningful.) MTU discovery stays off on these sockets (1200-byte packets) so the
   header always fits.
3. **The protocol.** Every packet starts `IG2R`, version 1, a kind byte:

   | kind | from → to | body | meaning |
   |---|---|---|---|
   | 1 Register | host → service | session u64 LE, pin 12 bytes | open or refresh the room (every 5 s) |
   | 2 Registered | service → host | the host's public IPv4:port | the room is up; the address joins the join code |
   | 3 Lookup | joiner → service | session, pin | find a room: both must match the join code |
   | 4 Found | service → joiner | the host's public address | try it directly first |
   | 5 Incoming | service → host | the joiner's public address | punch towards it |
   | 6 Refused | service → either | reason: 1 unknown room, 2 session taken, 3 full | |
   | 7 Unregister | host → service | | the room closes now |
   | 8 Punch | host → joiner | | opens the host's NAT towards the joiner; dropped on arrival |
   | 9 To | peer → service | destination address, QUIC datagram | relay this |
   | 10 From | service → peer | source address, QUIC datagram | relayed to you |

   The room's key is the session id from the join code; the lookup needs the code's certificate pin too, so the
   service never hands out a host's address to someone without the code, and nobody can squat a room they could
   not join. A room is held by the address that registered it.
4. **The joiner's ladder.** The join code's direct addresses are tried as before; in parallel, with a relay set,
   Lookup → the service sends Incoming to the host, which punches → the joiner connects to the host's public
   address (4 s) → on failure the joiner marks that address relayed and connects through the service (10 s). The
   first link up wins (ADR 0014's amendment).
5. **Abuse limits** (`rendezvous::Limits`, the binary's flags): control packets per source IP (token bucket, 20/s,
   burst 40; excess dropped silently); rooms total (1024) and per IP (4); peers per room (8); a room expires 30 s
   after its last Register or relayed packet from its host, a relayed peer 60 s after its last packet; relayed
   bandwidth per room (token bucket, 256 KiB/s, burst 1 MiB; excess dropped, QUIC backs off and retransmits).
   Only a host and a peer that looked its room up can exchange data, so the service can't be used to send to
   arbitrary addresses, and no reply is larger than the request it answers (no amplification). Nothing is logged
   but counts.
6. **Host migration** stays direct-only: a successor's listener does not register (a later step if needed).

## Consequences
- With any relay running, everybody who can reach it can host and join, whatever their NAT; the price is one hop
  of latency for relayed games (fine for lockstep, ADR 0005 §1) and the relay's bandwidth.
- The project publishes the software only. Who runs a public instance, where, and whether the game lists it by
  default is a maintainers' decision (it would need an operator, a host with a public IPv4 and UDP port 29172,
  monitoring, and an abuse contact); until then players run their own (`docs/RENDEZVOUS.md`).
- Tests run the service on 127.0.0.1 only: room and limit rules on the service state, two QUIC peers through a
  local relay (punch path and relay-only), and a short lockstep game relay-only with identical hashes.
