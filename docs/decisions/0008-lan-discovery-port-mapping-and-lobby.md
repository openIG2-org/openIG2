# ADR 0008: LAN discovery, port mapping and the lobby protocol

Date: 2026-10-16. Status: accepted (implements ADR 0005 version 1). Builds on ADR 0005 (player-hosted lockstep over
QUIC; v1 = LAN, direct IP, UPnP/NAT-PMP, no rendezvous service) and ADR 0007 (commands, lockstep, snapshots).
Implemented by `engine/crates/ig2-net` (`discovery`, `portmap`, `lobby`). None of this is the original's protocol
(ADR 0005 open question 3: a new protocol); everything here is our behaviour.

## 1. LAN discovery (`ig2_net::discovery`)

Query/answer over UDP, IPv4 broadcast on the LAN only. mDNS (ADR 0005 §3 step 1) is left out of v1: broadcast
covers a home LAN and needs no extra crate; it can be added beside it without a protocol change.

- **Port**: UDP `29170` (`DISCOVERY_PORT`). The host's `Announcer` binds `0.0.0.0:29170` with `SO_REUSEADDR`, so
  several hosts on one machine all receive the broadcast queries.
- **Query** (browser → `255.255.255.255:29170`, from an ephemeral port, once a second while the LAN tab is open):
  `"IG2D"`, version `1`, kind `0`, a random `u64` nonce, zero padding to exactly **256 bytes**. Shorter or
  longer queries are ignored, so an answer (< 256 bytes) never amplifies the query.
- **Answer** (host → the query's source address, unicast): `"IG2D"`, version, kind `1`, the echoed nonce, then the
  `Announcement` in our wire encoding (ADR 0007 §4): session id (random `u64`), session name (≤ 32 chars), QUIC
  game port (the address is the packet's source IP), the session certificate's SHA-256 fingerprint, password flag,
  protocol version, rules fingerprint (32 bytes, ADR 0005 §4), scenario name (≤ 32 chars), humans joined, human
  slots, started flag. Longer than 255 bytes → not sent (names are truncated first).
- **LAN only, both ways**: the host answers, and the browser accepts answers, only from loopback, RFC 1918
  (10/8, 172.16/12, 192.168/16), IPv4 link-local (169.254/16), IPv6 link-local (fe80::/10) or ULA (fc00::/7)
  addresses. Limited broadcast doesn't cross routers anyway; the filter also drops spoofed or forwarded packets.
- **Private** sessions never answer (the announcer is told `None`). Started sessions answer with `started = true`
  (the browser shows them for rejoin only).
- A browser forgets a session it hasn't heard for 3 s. The answer's nonce must match one of the browser's last
  queries (stray answers are dropped).
- The joiner pins the announced fingerprint when it connects (`quic::Pin::Fingerprint`), so a LAN join is
  authenticated without a certificate authority (ADR 0005 §2).

## 2. Port mapping (`ig2_net::portmap`)

The host may ask its router to forward the session's UDP port (ADR 0005 §3 step 3). **Opt-in**: nothing maps a
port unless the host's player enabled it for this session; the mapping is renewed while the session lives and
removed when it ends (`Drop` of the `Mapping` handle, and an explicit `close`).

- Protocols, tried in order: **UPnP IGD** (`igd-next`, behind feature `upnp`: SSDP search, `AddPortMapping` for UDP,
  `GetExternalIPAddress`) and **NAT-PMP** (RFC 6886, our own ~100 lines: UDP to the gateway's port 5351, opcode 1
  UDP mapping, opcode 0 external address; a lifetime-0 request deletes). PCP (RFC 6887) is left for later.
- Lease: 2 hours requested, renewed at half the granted lifetime by a background thread; description
  `"IG2 session"`. The external port asked for is the internal one; the router may grant another (reported).
- Result shown to the host (ADR 0005 "reachability check without the service"): the external address and port, or
  why mapping failed. No outside reachability test (that needs the rendezvous service).
- The NAT-PMP gateway is the UPnP gateway's address when UPnP answered, else the caller passes it (the client
  takes it from the OS route table later; v1: the conventional `x.y.z.1` of the host's LAN address, overridable in
  the settings).
- **Tests never touch the real router**: the NAT-PMP client is tested against a fake gateway on `127.0.0.1`, the
  UPnP client against a fake IGD control endpoint on `127.0.0.1` (no SSDP); the live search is only reached through
  `portmap::open_live`, which no test calls.

## 3. The lobby (`ig2_net::lobby`)

Lobby messages travel over the same link as the lockstep (`Msg::Lobby(LobbyMsg)`, `Transport`), so the lobby is
tested over the loopback transport and over QUIC alike. The host's `LobbyHost` owns the joiners' links until the
start, then hands them to a lockstep `Host`.

### 3.1 Session kinds
- **Open**: announced on the LAN; anyone with a compatible game may join until the start.
- **Password**: announced with a lock. The password is checked with **SPAKE2** (`spake2` crate, Ed25519 group,
  identities `"ig2-joiner"` / `"ig2-host"`) over the QUIC stream, followed by key confirmation bound to the TLS
  channel: each side sends `HMAC-SHA256(K, role ‖ binding ‖ session id)` where `binding` is the QUIC connection's
  exported keying material (RFC 5705 exporter, label `"EXPORTER-ig2-lobby"`, 32 bytes; both ends of one TLS session
  see the same value, a man in the middle sees two different ones). The password is never sent, in clear or
  encrypted; a man in the middle (an unpinned direct-IP join) learns nothing it could test offline, and the joiner
  knows it reached the real host once the host's confirmation checks out. The host waits 1 s before answering a
  wrong confirmation and closes the link (one guess per connection).
- **Private**: never announced; a joiner must present the session id (the join code, shown to the host as
  12 base-32 characters) in its hello. It may also have a password.

### 3.2 Join flow
```text
joiner                                   host
Hello{version, rules, name, code?}  ─▶   version / rules / code / full / started checks → Refused{reason}
                                    ◀─   Pake{msg}            (password sessions only)
Pake{msg}                           ─▶
Confirm{mac}                        ─▶   wrong → (1 s) Refused{WrongPassword}
                                    ◀─   Confirm{mac}         (joiner checks it, else drops the link)
                                    ◀─   Welcome{member id} + State{lobby}
Pick{empire} / Ready(b) / Chat(s)   ─▶   State{lobby} to everyone after each change
                                    ◀─   Start{member's slot, empire, delay, first_input, snapshot}
                     (lockstep Msg::Input / Bundle / Hash from here on)
```
- **Compatibility**: protocol version and rules fingerprint must match (`Refused{Version | Rules}`).
- **Slots**: the host lists the scenario's playable empires (number + race). Each member (the host too) picks a free
  one (`Pick`); the host may close a slot or give it to the AI (a human in it loses it). Only a member holding an
  empire can be ready; any change of picks or slots clears every ready flag. Every member must hold an empire and
  be ready before the host may start. Capacity = the open and human-held slots; a joiner beyond it is refused
  (`Full`).
- **Start**: the host applies `Game::canonical_view(host empire)`, takes a snapshot of the session's start state,
  and sends each member `Start` with its lockstep slot (= member id, the host 0), empire, input delay, first input
  turn, every human's (slot, empire) and the snapshot; each member builds a `Peer::from_snapshot`
  (`Start::peer`); the host's own peer runs on a loopback link to the lockstep `Host`. Marking the picked empires
  as human players in the start state belongs to the scenario set-up (not ported yet; a save keeps its own). From then on the links carry lockstep messages; lobby messages are ignored.
- **Leaving**: a member who leaves before the start frees its slot; kick (`LobbyHost::kick`) sends `Refused{Kicked}`
  and drops the link.

## Consequences
- LAN play needs no setup and no internet; direct IP joins work with or without a password; UPnP/NAT-PMP hosting
  works where the router allows it and is the player's explicit choice.
- No relay, hole punching, join-code directory or internet browser yet (ADR 0005 defers the rendezvous service);
  the session id is already a join code and the hello already carries it. Direct join codes (addresses + pin) now
  exist (ADR 0014 amendment); the directory and short codes still need the service.
- New dependencies: `socket2` (already in the tree via tokio), `getrandom` (in the tree), `spake2` (password check;
  HMAC-SHA256 is ours on `sha2`), `igd-next` (feature `upnp`).
