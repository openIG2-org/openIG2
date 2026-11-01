# ADR 0060: A public game list on the rendezvous service

Date: 2026-11-01. Status: accepted. The public instance it is meant for runs at `ig2relay.justdawee.hu:30000`, the game's default
relay server (ADR 0018, amended); the list works with any service a player runs. Builds on ADR
0018 (the rendezvous and relay service), ADR 0008 (LAN discovery's announcement) and ADR 0014's amendment (join
codes). Implemented by `ig2_net::rendezvous` (`Listing`, the packets, `Service`'s list, `list`),
`ig2_net::quic::QuicHost::publish` and `ig2-client::mp_lobby` (the host's switch, the connection page's list).

## Context
An internet game needs its join code passed from host to joiners by some other channel. ADR 0018 left the
service's directory, an internet session browser, as a separate step. A list of open games on the same service lets
players find each other without that channel, and one click can join through the existing join-code path.

## Decision
1. **Opt-in per room.** A host with a relay server set gets a "Public list: no / yes" switch in its room (off by
   default). Only a room that is on and not private is listed; leaving the room, starting the game or switching off
   takes it off at once. Without a relay server nothing changes and nothing is sent.
2. **The entry is the LAN announcement.** A `Listing` is ADR 0008's `Announcement` (session, name, scenario, humans
   and slots, lobby version, rules fingerprint, password flag, certificate fingerprint) plus the enabled mods'
   fingerprint, `wire`-encoded, at most 512 bytes, names at most 32 characters. The host's public address comes from
   the service, not from the listing. Joiners need no new trust: the listing carries the session id and certificate
   pin a join code would, so a click builds the room's join code and joins through the join-code path (direct, hole
   punch, relay), checking the certificate as before. A password still protects a listed room.
3. **The protocol** (ADR 0018's packets, version unchanged; an older service drops the new kinds):

   | kind | from → to | body | meaning |
   |---|---|---|---|
   | 11 Publish | host → service | an encoded `Listing` | list the room (repeated with every Register, 5 s) |
   | 12 Unlist | host → service | | off the list now |
   | 13 List | browser → service | start u16, zero padding to 1200 bytes | entries from `start` on |
   | 14 Rooms | service → browser | total u16, start u16, entries (address, u16 length, listing) | one page |

   The service accepts a Publish only from the address holding the room, for the room's own session, with a
   certificate fingerprint that starts with the room's pin, so nobody lists or changes another host's room. A listing
   not repeated for 15 s leaves the list (the room stays); a closed or expired room leaves it with its listing.
4. **No amplification, bounded cost.** A List is padded to 1200 bytes and a Rooms answer is never larger; each costs
   one control token (20/s per address, burst 40, ADR 0018 §5). The list is sorted by session id, so pages are
   stable; the client asks for up to 32 pages (2 to 6 rooms each), each up to three times, 700 ms apiece.
5. **The client.** The connection page gets "Browse internet games" under its list (the relay field must name a
   service; the page says so otherwise) and, while browsing, "LAN games" to go back and "Refresh". Rows show the
   name (a `*` for a password), the scenario and humans/slots; a room this game can't join (another lobby version,
   rules or mods, or started) is dimmed. A click selects, a second click or Join joins: the room's join code is put
   in the address field and joined as typed, so a failure leaves the code there to retry or share.
6. **The service address is the relay field** (`relay.txt`); without one it is the public instance, `ig2relay.justdawee.hu:30000`
   (amended 2026-11-01: first empty by default).

## Consequences
- Anyone who can reach the service can read the listed rooms' names and the hosts' public addresses (a Lookup with
  the code reveals the same); private and unlisted rooms stay unfindable without their code.
- An operator's service holds a listing per room in memory (at most `--max-rooms`), nothing on disk.
- Tests run the service on 127.0.0.1 only: the packets, publishing rules, expiry, paging and the blocking browser
  against a served socket (`ig2_net::rendezvous`), and two `MpScreen`s over QUIC where the host switches the list
  on and the joiner browses and joins with two clicks (`ig2-client::mp_lobby`).
