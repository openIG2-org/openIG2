# ADR 0020: Port mapping on by default when hosting

Date: 2026-10-31. Status: accepted (issue #12). Replaces the opt-in of ADR 0008 §2. A deliberate deviation from the
classic client (ADR 0009); the original has no port mapping at all, so there is no classic behaviour to keep.
Implemented in `ig2-client::mp_lobby::MpScreen::host` and `host_save` (`Listener::map_port(true)` once the room opens).

## Context
ADR 0008 §2 made UPnP / NAT-PMP port mapping opt-in: the host had to press the room's "Port mapping" row before
the router was asked. Most hosts behind a home router never pressed it, so players from outside couldn't reach the
room and nothing on screen said why.

## Decision
Opening a room (a new galaxy or a saved game) asks the router at once. The room's row shows "asking the router...",
then the external address or why it failed, as before. One press switches it off and removes the mapping; another
press maps again. The choice is per room and isn't saved. Lease, renewal and removal stay as in ADR 0008 §2.

No classic option: the original has no network layer of ours to match (ADR 0008: none of it is the original's
protocol).

## Consequences
- A host whose router has UPnP / NAT-PMP off sees the failure line as soon as the room opens.
- A LAN-only room also asks; the mapping is harmless there and removed when the room ends.
- A new host after host migration doesn't map (unchanged).
- Tests use loopback listeners; nothing touches a real router.
