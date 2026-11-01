# ADR 0041: Player cards, ready marks and a forced start in the multiplayer room

Date: 2026-11-01. Status: accepted. Changes the look of our multiplayer room (ADR 0014, `re/screens/start.md` §6);
the room's rules (§4) and the lobby protocol stay as they are. Implemented in `ig2_net::lobby`
(`LobbyHost::ready_all`) and the client's room (`mp_lobby.rs`; `--multiplayer` in `main.rs` / `window.rs`).

## Context
Our room was a list of text rows: the empire's number, its race's name, the holder and a "Ready" word once a joiner
pressed Ready. Who is who and who still holds the start up was hard to see. The original's room (`StartGfx2D_1..3`)
is deliberately not copied (ADR 0009 §2), so the room's look is ours to choose. The lobby protocol already carries
everything a card needs: each slot's empire, race and holder, and each member's ready flag (`LobbyState`), and chat
(`LobbyMsg::Chat` / `Said`).

The original's start waits for every human's ready (§3, button 0x44); a joiner who walked away keeps everyone
waiting, and the only way out was a kick.

## Decision
1. **A card per slot.** Each of the eight slots is a `kirak_fader` box of its own (the scenario panel's style) with
   a stripe in the empire's colour (palette entry `0xf + empire`, the colour `ColonyKisInfo` gives an owner,
   `colony_war.rs`) and the race's head (`textures/main/fejek3/<race>a.bmp`, 64 × 64, the scenario panel's race
   picture, `MainScrBrushok` 4..10) drawn at 24 × 24. Then the race's name, the holder and, for a human, the ready
   mark: the host's card says "Host", a joiner's "Ready" (14/5/6, bright) or "Not ready" (dim).
2. **A forced start.** The host's Start with a human not ready shows "Not everyone is ready: press again to start
   anyway" and turns the button into "Start anyway"; the next press marks every seated member ready
   (`LobbyHost::ready_all`, sent as a normal room state) and starts. Any other press disarms it. The two-empire rule
   stays.
3. **No protocol change.** No new message: the cards read the state every peer already gets, and the forced start
   is the host's own ready flags. The lobby protocol stays at 8, so rooms with older peers work as before.
4. **`--multiplayer`.** The client's command line flag opens the main menu on the multiplayer connection page (for a
   LAN party's shortcut, and for driving two windows in a check).

## Consequences
- Every machine draws the same cards from the same state; the race heads are read from each player's own install.
- A forced start starts a joiner who did not press Ready; it can still leave the game like any player.
- The room's ready marks are drawn left of the card's edge by their width (the font's right alignment needs a field
  width, which the room's other right-hand numbers never had).
- Tests: `ready_all` over loopback links in `ig2-net`; the client's room tests drive the forced start through the
  screen's own input.
