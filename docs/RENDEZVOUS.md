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
the firewall), and the same port number over **TCP** for players in the browser (their WebRTC signalling, plain
HTTP; without it the service runs UDP only and says so). Nothing else: no database, no files, no accounts. It prints its room and relay counts every
minute (`--status-secs N`, 0 = never) and never logs addresses.

| Flag | Default | |
|---|---|---|
| `--listen ADDR:PORT` | `0.0.0.0:29172` | the UDP address (IPv4) |
| `--max-rooms N` | 1024 | rooms at once |
| `--rooms-per-ip N` | 4 | rooms one address may hold |
| `--peers-per-room N` | 8 | relayed joiners per room |
| `--room-kib-per-sec N` | 256 | relayed bandwidth per room (burst: 4 s worth) |
| `--control-per-sec N` | 20 | room and lookup packets per second per address (burst: 2 s worth) |

### With Docker
Every change to the service on `main` publishes a public image, `ghcr.io/openig2-org/openig2-rendezvous` (tags
`latest` and the version, e.g. `0.7.150`). On a Linux server with Docker, copy `deploy/rendezvous/compose.yaml`
there and run:
```bash
docker compose -f compose.yaml pull
docker compose -f compose.yaml up -d
```
or, from a clone of the repository, build it from source: `docker compose -f deploy/rendezvous/compose.yaml up -d
--build`. Either way the service runs from a small image (no shell, a non-root user); the compose file publishes
UDP 29172 (`IG2_RENDEZVOUS_PORT=N` publishes another port), restarts it after a crash or reboot, and keeps the last
30 MB of its log (`docker compose -f compose.yaml logs -f`). The flags above go in the compose file's
`command`. To update, `pull` and `up -d` again (or `git pull` and the `--build` command).

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

## Players in the browser
The browser build (ADR 0047 part 4) joins through the service only: it types the relay server and the host's join
code, posts its WebRTC offer to `http://<relay>/offer/<session>/<pin>`, and the service passes it to the room's host
(any native client with that relay set), whose answer comes back the same way. The game then runs over a WebRTC data
channel between the two machines, not through the service. The page can't host. A page served over HTTPS can only
reach an HTTPS service, which this one isn't yet: run the page from `http://127.0.0.1` (or the LAN) for now.

## The public game list
The same service keeps a list of open games (ADR 0060); nothing extra to run or configure. A host lists its room by
switching **Public list: yes** in the room (shown when a relay server is set; off by default, a private room is
never listed). Joiners press **Browse internet games** under the connection page's list: the service's public rooms
show with their name, scenario and players (dimmed: another game version, install or mods, or already started);
**Refresh** asks again, **LAN games** goes back. A second click on a row (or Join) joins it with its join code,
which stays in the address field if the connection fails. A room leaves the list when its host switches it off,
leaves or starts, and 15 s after its host stops refreshing it.

The project's own public instance is decided but not running yet; until it is, use a service you or a friend run.

To try it on one machine: start `ig2-rendezvous --listen 127.0.0.1:29172`, open two clients (separate
`IG2_OPTIONS_DIR` and `IG2_SLOT_DIR` folders), type `127.0.0.1` as the relay server in both, host and switch the list
on in one, browse in the other.

## What the service can and can't see
It sees the public IPv4 addresses and ports of hosts and joiners while their rooms are open, and the size and
timing of the packets it relays. A listed room's name, scenario, players and host address are visible to anyone who
asks the service for its list. It can't read or change the game: the QUIC connection is encrypted end to end
and the joiner checks the host's certificate against the join code. Without the join code (session id and
certificate pin) nobody can look a room up.

## Testing locally
`cargo test --release -p ig2-net --features quic --test relay` runs a service on 127.0.0.1 and plays a short
lockstep game through it, relay-only. `ig2-client`'s `a_room_is_joined_through_the_relay` does the same through
the MP screens; `a_public_room_is_browsed_and_joined_with_a_click` the public list.
