# Running a rendezvous and relay service

Players behind routers without UPnP / NAT-PMP, or behind carrier-grade NAT, can't be reached by a join code's
addresses alone. A rendezvous service fixes that: it opens a room per hosted game, helps both sides punch a hole
through their NATs, and relays the game's (encrypted) packets when punching fails. Design and protocol: ADR 0018.

The project runs no public service and the game has no built-in address: whoever wants internet games runs one
(a friend's always-on machine, a small VPS) and tells the other players its address.

## Build and run
```bash
cd engine
cargo build --release -p ig2-rendezvous
./target/release/ig2-rendezvous --listen 0.0.0.0:29172
```
It needs one **UDP** port reachable from the internet (29172 by default; forward it on the router or open it in
the firewall). Nothing else: no database, no files, no accounts. It prints its room and relay counts every
minute (`--status-secs N`, 0 = never) and never logs addresses.

| Flag | Default | |
|---|---|---|
| `--listen ADDR:PORT` | `0.0.0.0:29172` | the UDP address (IPv4) |
| `--max-rooms N` | 1024 | rooms at once |
| `--rooms-per-ip N` | 4 | rooms one address may hold |
| `--peers-per-room N` | 8 | relayed joiners per room |
| `--room-kib-per-sec N` | 256 | relayed bandwidth per room (burst: 4 s worth) |
| `--control-per-sec N` | 20 | room and lookup packets per second per address (burst: 2 s worth) |

A room closes 30 s after its host stops refreshing it (the host refreshes every 5 s), a relayed joiner 60 s after
its last packet. A lockstep game needs a few KB/s; a rejoin snapshot a few hundred KB once.

## Use it in the game
On the multiplayer connection page, type the service's address in **Relay server (optional)** (`host` or
`host:port`); it is kept in `relay.txt` in the save folder. Both the host and the joiners set it:
- **Host game**: the room opens at the service; the room shows "Relay server: room open, seen at …" and the join
  code carries that public address too.
- **Join** with a join code: the code's own addresses are tried as before, and at the same time the service is
  asked for the host: a direct connection to the host's public address (the hole punch), then the relay.

Joining by typed address or from the LAN list doesn't use the service. Host migration after a host leaves stays
direct-only.

## What the service can and can't see
It sees the public IPv4 addresses and ports of hosts and joiners while their rooms are open, and the size and
timing of the packets it relays. It can't read or change the game: the QUIC connection is encrypted end to end
and the joiner checks the host's certificate against the join code. Without the join code (session id and
certificate pin) nobody can look a room up.

## Testing locally
`cargo test --release -p ig2-net --features quic --test relay` runs a service on 127.0.0.1 and plays a short
lockstep game through it, relay-only. `ig2-client`'s `a_room_is_joined_through_the_relay` does the same through
the MP screens.
